use std::{sync::mpsc, thread};

use cleaner_core::{ExecutionOutcome, ExecutionReport};
use cleaner_macos::{
    MacPlatform, PermissionStatus, SystemMacPlatform, full_disk_access::FullDiskAccessReport,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanCoverage {
    Full,
    Partial,
    Unknown,
}

impl ScanCoverage {
    pub fn label(self) -> &'static str {
        match self {
            Self::Full => "Full scan",
            Self::Partial => "Partial scan",
            Self::Unknown => "Scan coverage unknown",
        }
    }
}

pub fn scan_coverage(
    full_disk_access: Option<&FullDiskAccessReport>,
    permission_denied_paths: usize,
) -> ScanCoverage {
    if permission_denied_paths > 0 {
        return ScanCoverage::Partial;
    }

    match full_disk_access.map(|report| report.status) {
        Some(PermissionStatus::Granted) => ScanCoverage::Full,
        Some(PermissionStatus::Denied) => ScanCoverage::Partial,
        Some(PermissionStatus::Unknown) | None => ScanCoverage::Unknown,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionTrustSummary {
    pub executed: usize,
    pub missing: usize,
    pub changed_since_scan: usize,
    pub permission_denied: usize,
    pub protected: usize,
    pub failed: usize,
    pub moved_scan_estimate_bytes: u64,
    pub reclaimed_bytes: Option<u64>,
    pub cancelled: bool,
}

impl ExecutionTrustSummary {
    pub fn from_report(report: &ExecutionReport) -> Self {
        let accounting = report.trash_accounting();
        Self {
            executed: report.outcome_count(ExecutionOutcome::Executed),
            missing: report.outcome_count(ExecutionOutcome::Missing),
            changed_since_scan: report.outcome_count(ExecutionOutcome::ChangedSinceScan),
            permission_denied: report.outcome_count(ExecutionOutcome::PermissionDenied),
            protected: report.outcome_count(ExecutionOutcome::Protected),
            failed: report.outcome_count(ExecutionOutcome::Failed),
            moved_scan_estimate_bytes: accounting.moved_scan_estimate_bytes,
            reclaimed_bytes: accounting.reclaimed_bytes,
            cancelled: report.cancelled,
        }
    }

    pub fn attention_count(self) -> usize {
        self.missing
            + self.changed_since_scan
            + self.permission_denied
            + self.protected
            + self.failed
    }
}

pub enum FullDiskAccessMessage {
    Loaded(FullDiskAccessReport),
    OpenedSettings(Result<(), String>),
}

pub fn spawn_full_disk_access_status() -> mpsc::Receiver<FullDiskAccessMessage> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let report = SystemMacPlatform.full_disk_access_report();
        let _ = tx.send(FullDiskAccessMessage::Loaded(report));
    });
    rx
}

pub fn spawn_open_full_disk_access_settings() -> mpsc::Receiver<FullDiskAccessMessage> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let result = SystemMacPlatform.open_full_disk_access_settings();
        let _ = tx.send(FullDiskAccessMessage::OpenedSettings(result));
    });
    rx
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use cleaner_core::{
        CleanupAction, CleanupCategory, ExecutionFailure, ExecutionRecord, SafetyError,
    };

    use super::*;

    fn fda(status: PermissionStatus) -> FullDiskAccessReport {
        FullDiskAccessReport {
            status,
            probe_path: None,
            detail: "fixture".into(),
        }
    }

    #[test]
    fn permission_denied_paths_force_partial_coverage() {
        assert_eq!(
            scan_coverage(Some(&fda(PermissionStatus::Granted)), 1),
            ScanCoverage::Partial
        );
    }

    #[test]
    fn granted_fda_without_scan_denials_is_full_coverage() {
        assert_eq!(
            scan_coverage(Some(&fda(PermissionStatus::Granted)), 0),
            ScanCoverage::Full
        );
    }

    #[test]
    fn denied_fda_is_partial_even_before_scan_denials_are_observed() {
        assert_eq!(
            scan_coverage(Some(&fda(PermissionStatus::Denied)), 0),
            ScanCoverage::Partial
        );
    }

    #[test]
    fn unknown_fda_does_not_claim_full_coverage() {
        assert_eq!(
            scan_coverage(Some(&fda(PermissionStatus::Unknown)), 0),
            ScanCoverage::Unknown
        );
        assert_eq!(scan_coverage(None, 0), ScanCoverage::Unknown);
    }

    #[test]
    fn execution_summary_preserves_typed_partial_success() {
        let report = ExecutionReport {
            records: vec![
                ExecutionRecord {
                    path: PathBuf::from("/tmp/moved"),
                    category: CleanupCategory::UserCache,
                    action: CleanupAction::MoveToTrash,
                    bytes: 128,
                    result: Ok(()),
                },
                ExecutionRecord {
                    path: PathBuf::from("/tmp/denied"),
                    category: CleanupCategory::UserCache,
                    action: CleanupAction::MoveToTrash,
                    bytes: 64,
                    result: Err(ExecutionFailure::Safety(SafetyError::PermissionDenied)),
                },
                ExecutionRecord {
                    path: PathBuf::from("/tmp/missing"),
                    category: CleanupCategory::UserCache,
                    action: CleanupAction::MoveToTrash,
                    bytes: 32,
                    result: Err(ExecutionFailure::Safety(SafetyError::MissingPath)),
                },
            ],
            cancelled: false,
        };

        let summary = ExecutionTrustSummary::from_report(&report);
        assert_eq!(summary.executed, 1);
        assert_eq!(summary.permission_denied, 1);
        assert_eq!(summary.missing, 1);
        assert_eq!(summary.attention_count(), 2);
        assert_eq!(summary.moved_scan_estimate_bytes, 128);
        assert_eq!(summary.reclaimed_bytes, None);
    }
}
