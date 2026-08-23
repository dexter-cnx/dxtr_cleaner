use std::path::{Path, PathBuf};

use crate::{CancellationToken, CleanupCategory, CleanupPlan, ExecutionPolicy, Planner, SafetyError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupAction {
    MoveToTrash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionRecord {
    pub path: PathBuf,
    pub category: CleanupCategory,
    pub action: CleanupAction,
    pub bytes: u64,
    pub result: Result<(), String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExecutionReport {
    pub records: Vec<ExecutionRecord>,
    pub cancelled: bool,
}

impl ExecutionReport {
    pub fn succeeded_count(&self) -> usize {
        self.records.iter().filter(|record| record.result.is_ok()).count()
    }

    pub fn failed_count(&self) -> usize {
        self.records.iter().filter(|record| record.result.is_err()).count()
    }

    pub fn reclaimed_bytes(&self) -> u64 {
        self.records
            .iter()
            .filter(|record| record.result.is_ok())
            .map(|record| record.bytes)
            .sum()
    }
}

pub trait TrashBackend {
    fn move_to_trash(&self, path: &Path) -> Result<(), String>;
}

pub struct CleanupExecutor;

impl CleanupExecutor {
    pub fn execute(
        plan: &CleanupPlan,
        policy: &ExecutionPolicy,
        cancellation: &CancellationToken,
        backend: &dyn TrashBackend,
    ) -> Result<ExecutionReport, SafetyError> {
        Planner::validate_for_execution(plan, policy)?;

        let mut report = ExecutionReport::default();

        for entry in plan.items.iter().filter(|entry| entry.selected) {
            if cancellation.is_cancelled() {
                report.cancelled = true;
                break;
            }

            let canonical_path = Planner::validate_item_for_execution(&entry.item, policy)?;
            let result = backend.move_to_trash(&canonical_path);
            report.records.push(ExecutionRecord {
                path: canonical_path,
                category: entry.item.category,
                action: CleanupAction::MoveToTrash,
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
    struct RecordingTrash {
        paths: Mutex<Vec<PathBuf>>,
    }

    impl TrashBackend for RecordingTrash {
        fn move_to_trash(&self, path: &Path) -> Result<(), String> {
            self.paths
                .lock()
                .expect("lock paths")
                .push(path.to_path_buf());
            Ok(())
        }
    }

    #[test]
    fn cancellation_stops_before_next_item() {
        let root = std::env::temp_dir().join(format!(
            "dxtr-cleaner-executor-{}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create root");
        let first = root.join("a");
        let second = root.join("b");
        fs::write(&first, b"a").expect("write first");
        fs::write(&second, b"b").expect("write second");

        let plan = CleanupPlan {
            items: vec![first.clone(), second.clone()]
                .into_iter()
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
        };
        let policy = ExecutionPolicy::enabled(vec![AllowedRoot::new(
            CleanupCategory::UserCache,
            root.clone(),
        )]);
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let backend = RecordingTrash::default();

        let report = CleanupExecutor::execute(&plan, &policy, &cancellation, &backend)
            .expect("execution report");
        assert!(report.cancelled);
        assert!(report.records.is_empty());

        fs::remove_dir_all(root).expect("remove root");
    }
}
