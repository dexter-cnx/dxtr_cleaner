use std::path::{Path, PathBuf};

use crate::{
    CancellationToken, CategoryActionPolicy, CleanupAction, CleanupCategory, CleanupPlan,
    ExecutionPolicy, Planner, SafetyError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionFailure {
    Safety(SafetyError),
    Backend(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionRecord {
    pub path: PathBuf,
    pub category: CleanupCategory,
    pub action: CleanupAction,
    pub bytes: u64,
    pub result: Result<(), ExecutionFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExecutionReport {
    pub records: Vec<ExecutionRecord>,
    pub cancelled: bool,
}

impl ExecutionReport {
    pub fn succeeded_count(&self) -> usize {
        self.records
            .iter()
            .filter(|record| record.result.is_ok())
            .count()
    }

    pub fn skipped_count(&self) -> usize {
        self.records
            .iter()
            .filter(|record| is_raced_away_record(record))
            .count()
    }

    pub fn failed_count(&self) -> usize {
        self.records
            .iter()
            .filter(|record| record.result.is_err() && !is_raced_away_record(record))
            .count()
    }

    pub fn moved_bytes(&self) -> u64 {
        self.records
            .iter()
            .filter(|record| record.result.is_ok() && record.action == CleanupAction::MoveToTrash)
            .map(|record| record.bytes)
            .sum()
    }

    pub fn permanently_deleted_bytes(&self) -> u64 {
        self.records
            .iter()
            .filter(|record| {
                record.result.is_ok() && record.action == CleanupAction::PermanentDelete
            })
            .map(|record| record.bytes)
            .sum()
    }
}

fn is_raced_away_record(record: &ExecutionRecord) -> bool {
    match &record.result {
        Err(ExecutionFailure::Safety(SafetyError::MissingPath)) => true,
        Err(ExecutionFailure::Backend(message)) => {
            let normalized = message.to_ascii_lowercase();
            normalized.contains("path no longer exists")
                || normalized.contains("path disappeared before finder")
        }
        _ => false,
    }
}

pub trait TrashBackend {
    fn move_to_trash(&self, path: &Path) -> Result<(), String>;
}

pub trait PermanentDeleteBackend {
    fn permanent_delete(&self, path: &Path) -> Result<(), String>;
}

pub trait CleanupBackend: TrashBackend + PermanentDeleteBackend {}

impl<T> CleanupBackend for T where T: TrashBackend + PermanentDeleteBackend {}

pub struct CleanupExecutor;

impl CleanupExecutor {
    pub fn execute(
        plan: &CleanupPlan,
        execution_policy: &ExecutionPolicy,
        action_policy: &CategoryActionPolicy,
        cancellation: &CancellationToken,
        backend: &dyn CleanupBackend,
    ) -> Result<ExecutionReport, SafetyError> {
        if !execution_policy.destructive_actions_enabled {
            return Err(SafetyError::DestructiveActionsDisabled);
        }

        let mut report = ExecutionReport::default();

        for entry in plan.items.iter().filter(|entry| entry.selected) {
            if cancellation.is_cancelled() {
                report.cancelled = true;
                break;
            }

            let action = action_policy.action_for(entry.item.category);
            let canonical_path =
                match Planner::validate_item_for_execution(&entry.item, execution_policy) {
                    Ok(path) => path,
                    Err(error) => {
                        report.records.push(ExecutionRecord {
                            path: entry.item.path.clone(),
                            category: entry.item.category,
                            action,
                            bytes: entry.item.bytes,
                            result: Err(ExecutionFailure::Safety(error)),
                        });
                        continue;
                    }
                };

            let result = match action {
                CleanupAction::MoveToTrash => backend.move_to_trash(&canonical_path),
                CleanupAction::PermanentDelete => backend.permanent_delete(&canonical_path),
            }
            .map_err(ExecutionFailure::Backend);

            report.records.push(ExecutionRecord {
                path: canonical_path,
                category: entry.item.category,
                action,
                bytes: entry.item.bytes,
                result,
            });
        }

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, sync::Mutex};

    use super::*;
    use crate::{AllowedRoot, CleanupPlanItem, ScanItem};

    #[derive(Default)]
    struct RecordingBackend {
        trashed: Mutex<Vec<PathBuf>>,
        deleted: Mutex<Vec<PathBuf>>,
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
        fn permanent_delete(&self, path: &Path) -> Result<(), String> {
            self.deleted
                .lock()
                .expect("lock deleted paths")
                .push(path.to_path_buf());
            Ok(())
        }
    }

    struct RemovingBackend;

    impl TrashBackend for RemovingBackend {
        fn move_to_trash(&self, path: &Path) -> Result<(), String> {
            fs::remove_file(path).map_err(|error| error.to_string())
        }
    }

    impl PermanentDeleteBackend for RemovingBackend {
        fn permanent_delete(&self, path: &Path) -> Result<(), String> {
            fs::remove_file(path).map_err(|error| error.to_string())
        }
    }

    struct FinderRaceBackend;

    impl TrashBackend for FinderRaceBackend {
        fn move_to_trash(&self, _path: &Path) -> Result<(), String> {
            Err("move to Trash failed: path disappeared before Finder could move it".into())
        }
    }

    impl PermanentDeleteBackend for FinderRaceBackend {
        fn permanent_delete(&self, _path: &Path) -> Result<(), String> {
            unreachable!("fixture uses Trash-only action policy")
        }
    }

    fn plan_for(paths: &[PathBuf]) -> CleanupPlan {
        CleanupPlan {
            items: paths
                .iter()
                .cloned()
                .map(|path| CleanupPlanItem {
                    item: ScanItem {
                        path,
                        category: CleanupCategory::UserCache,
                        bytes: 1,
                        is_symlink: false,
                    },
                    selected: true,
                })
                .collect(),
        }
    }

    fn execution_policy(root: PathBuf) -> ExecutionPolicy {
        ExecutionPolicy::enabled(vec![AllowedRoot::new(CleanupCategory::UserCache, root)])
    }

    #[test]
    fn cancellation_stops_before_validation_or_backend_work() {
        let root =
            std::env::temp_dir().join(format!("dxtr-cleaner-executor-{}", std::process::id()));
        fs::create_dir_all(&root).expect("create root");
        let first = root.join("a");
        let second = root.join("b");
        fs::write(&first, b"a").expect("write first");
        fs::write(&second, b"b").expect("write second");

        let plan = plan_for(&[first, second]);
        let policy = execution_policy(root.clone());
        let action_policy = CategoryActionPolicy::trash_only();
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let backend = RecordingBackend::default();

        let report =
            CleanupExecutor::execute(&plan, &policy, &action_policy, &cancellation, &backend)
                .expect("execution report");
        assert!(report.cancelled);
        assert!(report.records.is_empty());
        assert!(
            backend
                .trashed
                .lock()
                .expect("lock trashed paths")
                .is_empty()
        );
        assert!(
            backend
                .deleted
                .lock()
                .expect("lock deleted paths")
                .is_empty()
        );

        fs::remove_dir_all(root).expect("remove root");
    }

    #[test]
    fn revalidation_missing_path_is_reported_as_skipped_not_failed() {
        let root = std::env::temp_dir().join(format!(
            "dxtr-cleaner-partial-report-{}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create root");
        let path = root.join("duplicate");
        fs::write(&path, b"cache").expect("write file");

        let plan = plan_for(&[path.clone(), path]);
        let policy = execution_policy(root.clone());
        let action_policy = CategoryActionPolicy::trash_only();
        let cancellation = CancellationToken::new();

        let report = CleanupExecutor::execute(
            &plan,
            &policy,
            &action_policy,
            &cancellation,
            &RemovingBackend,
        )
        .expect("execution report");

        assert_eq!(report.succeeded_count(), 1);
        assert_eq!(report.skipped_count(), 1);
        assert_eq!(report.failed_count(), 0);
        assert_eq!(report.moved_bytes(), 1);
        assert_eq!(report.permanently_deleted_bytes(), 0);
        assert!(matches!(
            report.records[1].result,
            Err(ExecutionFailure::Safety(SafetyError::MissingPath))
        ));

        fs::remove_dir_all(root).expect("remove root");
    }

    #[test]
    fn finder_race_backend_message_is_reported_as_skipped_not_failed() {
        let root = std::env::temp_dir().join(format!(
            "dxtr-cleaner-finder-race-{}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create root");
        let path = root.join("cache");
        fs::write(&path, b"cache").expect("write file");

        let report = CleanupExecutor::execute(
            &plan_for(&[path]),
            &execution_policy(root.clone()),
            &CategoryActionPolicy::trash_only(),
            &CancellationToken::new(),
            &FinderRaceBackend,
        )
        .expect("execution report");

        assert_eq!(report.succeeded_count(), 0);
        assert_eq!(report.skipped_count(), 1);
        assert_eq!(report.failed_count(), 0);
        assert_eq!(report.moved_bytes(), 0);

        fs::remove_dir_all(root).expect("remove root");
    }

    #[test]
    fn category_action_policy_drives_the_backend_operation() {
        let root =
            std::env::temp_dir().join(format!("dxtr-cleaner-action-policy-{}", std::process::id()));
        fs::create_dir_all(&root).expect("create root");
        let path = root.join("cache");
        fs::write(&path, b"cache").expect("write file");

        let plan = plan_for(&[path]);
        let policy = execution_policy(root.clone());
        let mut action_policy = CategoryActionPolicy::trash_only();
        action_policy
            .enable_permanent_delete(CleanupCategory::UserCache)
            .expect("user cache can opt in");
        let cancellation = CancellationToken::new();
        let backend = RecordingBackend::default();

        let report =
            CleanupExecutor::execute(&plan, &policy, &action_policy, &cancellation, &backend)
                .expect("execution report");

        assert_eq!(report.succeeded_count(), 1);
        assert_eq!(report.skipped_count(), 0);
        assert_eq!(report.moved_bytes(), 0);
        assert_eq!(report.permanently_deleted_bytes(), 1);
        assert!(
            backend
                .trashed
                .lock()
                .expect("lock trashed paths")
                .is_empty()
        );
        assert_eq!(backend.deleted.lock().expect("lock deleted paths").len(), 1);

        fs::remove_dir_all(root).expect("remove root");
    }
}
