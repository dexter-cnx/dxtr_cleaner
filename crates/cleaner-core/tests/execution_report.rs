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
    assert_eq!(
        report.outcome_count(ExecutionOutcome::PermissionDenied),
        1
    );
    assert_eq!(
        report.records[0].outcome(),
        ExecutionOutcome::PermissionDenied
    );
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
fn trash_accounting_does_not_claim_reclaimed_space() {
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
    assert_eq!(accounting.moved_logical_bytes, 128);
    assert_eq!(accounting.reclaimed_bytes, None);
    assert_eq!(report.records[0].outcome(), ExecutionOutcome::Executed);
}
