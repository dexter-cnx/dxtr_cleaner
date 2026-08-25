use std::path::PathBuf;

use cleaner_core::{
    CleanupAction, CleanupCategory, ExecutionFailure, ExecutionRecord, ExecutionReport,
};

#[test]
fn ordinary_backend_failure_is_not_classified_as_skipped() {
    let report = ExecutionReport {
        records: vec![ExecutionRecord {
            path: PathBuf::from("/tmp/cache.bin"),
            category: CleanupCategory::UserCache,
            action: CleanupAction::MoveToTrash,
            bytes: 128,
            result: Err(ExecutionFailure::Backend("permission denied".into())),
        }],
        cancelled: false,
    };

    assert_eq!(report.succeeded_count(), 0);
    assert_eq!(report.skipped_count(), 0);
    assert_eq!(report.failed_count(), 1);
    assert_eq!(report.moved_bytes(), 0);
}

#[test]
fn raced_away_backend_failure_is_classified_as_skipped() {
    let report = ExecutionReport {
        records: vec![ExecutionRecord {
            path: PathBuf::from("/tmp/cache.bin"),
            category: CleanupCategory::UserCache,
            action: CleanupAction::MoveToTrash,
            bytes: 128,
            result: Err(ExecutionFailure::Backend(
                "move to Trash failed: path disappeared before Finder could move it".into(),
            )),
        }],
        cancelled: false,
    };

    assert_eq!(report.succeeded_count(), 0);
    assert_eq!(report.skipped_count(), 1);
    assert_eq!(report.failed_count(), 0);
    assert_eq!(report.moved_bytes(), 0);
}
