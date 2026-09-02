use std::path::PathBuf;

use cleaner_core::{
    CleanupAction, CleanupCategory, ExecutionFailure, ExecutionOutcome, ExecutionRecord,
    ExecutionReport, SafetyError,
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
    assert_eq!(report.outcome_count(ExecutionOutcome::PermissionDenied), 1);
    assert_eq!(
        report.records[0].outcome(),
        ExecutionOutcome::PermissionDenied
    );
}

#[test]
fn finder_permission_wording_is_classified_as_permission_denied() {
    let record = ExecutionRecord {
        path: PathBuf::from("/tmp/cache.bin"),
        category: CleanupCategory::UserCache,
        action: CleanupAction::MoveToTrash,
        bytes: 128,
        result: Err(ExecutionFailure::Backend(
            "Finder says: You don’t have permission to move this item to the Trash.".into(),
        )),
    };

    assert_eq!(record.outcome(), ExecutionOutcome::PermissionDenied);
}

#[test]
fn planner_permission_failure_is_classified_as_permission_denied() {
    let record = ExecutionRecord {
        path: PathBuf::from("/tmp/cache.bin"),
        category: CleanupCategory::UserCache,
        action: CleanupAction::MoveToTrash,
        bytes: 128,
        result: Err(ExecutionFailure::Safety(SafetyError::PermissionDenied)),
    };

    assert_eq!(record.outcome(), ExecutionOutcome::PermissionDenied);
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
    assert_eq!(report.records[0].outcome(), ExecutionOutcome::Missing);
}

#[test]
fn windows_recycle_bin_missing_path_is_classified_as_missing() {
    let record = ExecutionRecord {
        path: PathBuf::from(r"C:\Temp\gone.bin"),
        category: CleanupCategory::UserCache,
        action: CleanupAction::MoveToTrash,
        bytes: 128,
        result: Err(ExecutionFailure::Backend(
            r"Recycle Bin path does not exist: C:\Temp\gone.bin".into(),
        )),
    };

    assert_eq!(record.outcome(), ExecutionOutcome::Missing);
}

#[test]
fn revalidation_and_protection_have_distinct_outcomes() {
    let changed = ExecutionRecord {
        path: PathBuf::from("/tmp/changed"),
        category: CleanupCategory::UserCache,
        action: CleanupAction::MoveToTrash,
        bytes: 32,
        result: Err(ExecutionFailure::Safety(
            SafetyError::PathRevalidationFailed,
        )),
    };
    let protected = ExecutionRecord {
        path: PathBuf::from("/tmp/protected"),
        category: CleanupCategory::UserCache,
        action: CleanupAction::MoveToTrash,
        bytes: 64,
        result: Err(ExecutionFailure::Safety(SafetyError::ProtectedRoot)),
    };

    assert_eq!(changed.outcome(), ExecutionOutcome::ChangedSinceScan);
    assert_eq!(protected.outcome(), ExecutionOutcome::Protected);
}

#[test]
fn trash_accounting_labels_moved_bytes_as_scan_estimate_and_not_reclaimed() {
    let report = ExecutionReport {
        records: vec![ExecutionRecord {
            path: PathBuf::from("/tmp/cache.bin"),
            category: CleanupCategory::UserCache,
            action: CleanupAction::MoveToTrash,
            bytes: 128,
            result: Ok(()),
        }],
        cancelled: false,
    };

    let accounting = report.trash_accounting();
    assert_eq!(accounting.moved_scan_estimate_bytes, 128);
    assert_eq!(accounting.reclaimed_bytes, None);
    assert_eq!(report.records[0].outcome(), ExecutionOutcome::Executed);
}
