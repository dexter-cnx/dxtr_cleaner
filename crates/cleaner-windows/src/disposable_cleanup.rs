use std::path::PathBuf;

use cleaner_core::{
    AllowedRoot, CancellationToken, CategoryActionPolicy, CleanupBackend, CleanupCategory,
    CleanupExecutor, CleanupPlan, ExecutionPolicy, ExecutionReport, Planner, SafetyError, ScanItem,
};

use crate::WindowsTrashBackend;

/// Trash-only cleanup boundary for the Windows GPUI disposable-directory smoke flow.
///
/// This intentionally does not enable mutation for the discovered Smart Scan provider set yet.
/// The caller must supply the disposable directory that was used as the manual scan root.
pub struct WindowsDisposableCleanup;

impl WindowsDisposableCleanup {
    pub fn build_plan(items: Vec<ScanItem>) -> CleanupPlan {
        Planner::build(items)
    }

    pub fn execute(
        plan: &CleanupPlan,
        root: impl Into<PathBuf>,
        cancellation: &CancellationToken,
    ) -> Result<ExecutionReport, SafetyError> {
        Self::execute_with_backend(plan, root.into(), cancellation, &WindowsTrashBackend)
    }

    fn execute_with_backend(
        plan: &CleanupPlan,
        root: PathBuf,
        cancellation: &CancellationToken,
        backend: &dyn CleanupBackend,
    ) -> Result<ExecutionReport, SafetyError> {
        let policy =
            ExecutionPolicy::enabled(vec![AllowedRoot::new(CleanupCategory::UserCache, root)]);
        let action_policy = CategoryActionPolicy::trash_only();

        CleanupExecutor::execute(plan, &policy, &action_policy, cancellation, backend)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::Path,
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
        let path = root.join("cache.bin");
        fs::write(&path, b"cache").expect("write fixture");
        let plan = WindowsDisposableCleanup::build_plan(vec![scan_item(path)]);
        let backend = RecordingBackend::default();

        let report = WindowsDisposableCleanup::execute_with_backend(
            &plan,
            root.clone(),
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
        let outside = temp_root("outside");
        let path = outside.join("cache.bin");
        fs::write(&path, b"cache").expect("write fixture");
        let plan = WindowsDisposableCleanup::build_plan(vec![scan_item(path)]);
        let backend = RecordingBackend::default();

        let report = WindowsDisposableCleanup::execute_with_backend(
            &plan,
            root.clone(),
            &CancellationToken::new(),
            &backend,
        )
        .expect("execution returns per-item safety report");

        assert_eq!(report.succeeded_count(), 0);
        assert_eq!(report.failed_count(), 1);
        assert!(
            backend
                .trashed
                .lock()
                .expect("lock trashed paths")
                .is_empty()
        );

        fs::remove_dir_all(root).expect("remove root fixture");
        fs::remove_dir_all(outside).expect("remove outside fixture");
    }
}
