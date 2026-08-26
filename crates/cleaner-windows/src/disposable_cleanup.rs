use std::path::{Path, PathBuf};

use cleaner_core::{
    AllowedRoot, CancellationToken, CategoryActionPolicy, CleanupBackend, CleanupCategory,
    CleanupExecutor, CleanupPlan, ExecutionPolicy, ExecutionReport, Planner, SafetyError, ScanItem,
};
use same_file::Handle;

use crate::WindowsTrashBackend;

#[derive(Debug)]
pub struct WindowsDisposableCleanupRoot {
    path: PathBuf,
    handle: Handle,
}

#[derive(Debug, PartialEq, Eq)]
pub enum WindowsDisposableCleanupError {
    RootUnavailable(String),
    RootIdentityChanged,
    Safety(SafetyError),
}

/// Trash-only cleanup boundary for the Windows GPUI disposable-directory smoke flow.
///
/// This intentionally does not enable mutation for the discovered Smart Scan provider set yet.
/// The manual scan root must be pinned before scanning and the same filesystem identity must still
/// be present when the reviewed plan is executed.
pub struct WindowsDisposableCleanup;

impl WindowsDisposableCleanup {
    pub fn pin_root(root: impl Into<PathBuf>) -> Result<WindowsDisposableCleanupRoot, String> {
        let path = root.into();
        let handle = Handle::from_path(&path).map_err(|error| error.to_string())?;
        Ok(WindowsDisposableCleanupRoot { path, handle })
    }

    pub fn build_plan(items: Vec<ScanItem>) -> CleanupPlan {
        Planner::build(items)
    }

    pub fn execute(
        plan: &CleanupPlan,
        root: &WindowsDisposableCleanupRoot,
        cancellation: &CancellationToken,
    ) -> Result<ExecutionReport, WindowsDisposableCleanupError> {
        Self::execute_with_backend(plan, root, cancellation, &WindowsTrashBackend)
    }

    fn execute_with_backend(
        plan: &CleanupPlan,
        root: &WindowsDisposableCleanupRoot,
        cancellation: &CancellationToken,
        backend: &dyn CleanupBackend,
    ) -> Result<ExecutionReport, WindowsDisposableCleanupError> {
        let current = Handle::from_path(&root.path)
            .map_err(|error| WindowsDisposableCleanupError::RootUnavailable(error.to_string()))?;
        if current != root.handle {
            return Err(WindowsDisposableCleanupError::RootIdentityChanged);
        }

        let policy = ExecutionPolicy::enabled(vec![AllowedRoot::new(
            CleanupCategory::UserCache,
            root.path.clone(),
        )]);
        let action_policy = CategoryActionPolicy::trash_only();

        CleanupExecutor::execute(plan, &policy, &action_policy, cancellation, backend)
            .map_err(WindowsDisposableCleanupError::Safety)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::Mutex,
        time::{SystemTime, UNIX_EPOCH},
    };

    use cleaner_core::{PermanentDeleteBackend, TrashBackend};

    use super::*;

    #[derive(Default)]
    struct RecordingBackend {
        trashed: Mutex<Vec<PathBuf>>,
    }

    impl TrashBackend for RecordingBackend {
        fn move_to_trash(&self, path: &Path) -> Result<(), String> {
            self.trashed
                .lock()
                .expect("lock trashed paths")
                .push(path.to_path_buf());
            Ok(())
        }
    }

    impl PermanentDeleteBackend for RecordingBackend {
        fn permanent_delete(&self, _path: &Path) -> Result<(), String> {
            panic!("disposable cleanup must remain Trash-only")
        }
    }

    fn temp_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "dxtr-cleaner-windows-{name}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    fn scan_item(path: PathBuf) -> ScanItem {
        ScanItem {
            path,
            category: CleanupCategory::UserCache,
            bytes: 5,
            is_symlink: false,
        }
    }

    #[test]
    fn disposable_cleanup_uses_shared_planner_and_trash_only_action() {
        let root = temp_root("cleanup");
        let pinned = WindowsDisposableCleanup::pin_root(root.clone()).expect("pin root");
        let path = root.join("cache.bin");
        fs::write(&path, b"cache").expect("write fixture");
        let plan = WindowsDisposableCleanup::build_plan(vec![scan_item(path)]);
        let backend = RecordingBackend::default();

        let report = WindowsDisposableCleanup::execute_with_backend(
            &plan,
            &pinned,
            &CancellationToken::new(),
            &backend,
        )
        .expect("execute disposable cleanup");

        assert_eq!(report.succeeded_count(), 1);
        assert_eq!(report.failed_count(), 0);
        assert_eq!(report.moved_bytes(), 5);
        assert_eq!(backend.trashed.lock().expect("lock trashed paths").len(), 1);

        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn disposable_cleanup_fails_closed_outside_the_manual_root() {
        let root = temp_root("root");
        let pinned = WindowsDisposableCleanup::pin_root(root.clone()).expect("pin root");
        let outside = temp_root("outside");
        let path = outside.join("cache.bin");
        fs::write(&path, b"cache").expect("write fixture");
        let plan = WindowsDisposableCleanup::build_plan(vec![scan_item(path)]);
        let backend = RecordingBackend::default();

        let report = WindowsDisposableCleanup::execute_with_backend(
            &plan,
            &pinned,
            &CancellationToken::new(),
            &backend,
        )
        .expect("execution returns per-item safety report");

        assert_eq!(report.succeeded_count(), 0);
        assert_eq!(report.failed_count(), 1);
        assert!(backend.trashed.lock().expect("lock trashed paths").is_empty());

        fs::remove_dir_all(root).expect("remove root fixture");
        fs::remove_dir_all(outside).expect("remove outside fixture");
    }

    #[test]
    fn disposable_cleanup_rejects_replaced_root_identity() {
        let parent = temp_root("root-swap-parent");
        let root = parent.join("scan-root");
        let moved = parent.join("scan-root-original");
        fs::create_dir_all(&root).expect("create scan root");
        let pinned = WindowsDisposableCleanup::pin_root(root.clone()).expect("pin root");
        let scanned_path = root.join("cache.bin");
        fs::write(&scanned_path, b"scanned").expect("write scanned fixture");
        let plan = WindowsDisposableCleanup::build_plan(vec![scan_item(scanned_path.clone())]);

        fs::rename(&root, &moved).expect("rename original root");
        fs::create_dir_all(&root).expect("create replacement root");
        fs::write(root.join("cache.bin"), b"replacement").expect("write replacement fixture");

        let backend = RecordingBackend::default();
        let result = WindowsDisposableCleanup::execute_with_backend(
            &plan,
            &pinned,
            &CancellationToken::new(),
            &backend,
        );

        assert_eq!(result, Err(WindowsDisposableCleanupError::RootIdentityChanged));
        assert!(backend.trashed.lock().expect("lock trashed paths").is_empty());

        fs::remove_dir_all(parent).expect("remove fixture");
    }
}
