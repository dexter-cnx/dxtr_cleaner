mod action_policy;
mod app_inventory;
mod app_protection;
mod bundle_identifier;
mod executor;
mod model;
mod orphan;
mod planner;
mod related_files;
mod safety;
mod scanner;
mod target;

pub use action_policy::{
    ActionPolicyError, CategoryActionPolicy, CleanupAction, supports_permanent_delete,
};
pub use app_inventory::{
    ApplicationInventory, ApplicationInventoryIssue, ApplicationInventoryReport,
    ApplicationLocation, ApplicationMetadata, InstalledApplication,
};
pub use app_protection::{
    ApplicationProtection, ApplicationProtectionPolicy, ApplicationProtectionReason,
};
pub use bundle_identifier::{is_apple_bundle_identifier, is_safe_bundle_identifier};
pub use executor::{
    CleanupBackend, CleanupExecutor, ExecutionFailure, ExecutionRecord, ExecutionReport,
    PermanentDeleteBackend, TrashBackend,
};
pub use model::{CleanupCategory, CleanupPlan, CleanupPlanItem, ScanItem, ScanSummary};
pub use orphan::{OrphanCandidate, OrphanFinder, OrphanFinderIssue, OrphanReport};
pub use planner::{AllowedRoot, ExecutionPolicy, Planner, SafetyError};
pub use related_files::{
    MatchConfidence, MatchEvidence, RelatedFileCandidate, RelatedFileKind, RelatedFileMatcher,
    RelatedFileReport,
};
pub use scanner::{
    CancellationToken, FileSystemScanner, ScanEvent, ScanEventSink, ScanRequest, Scanner,
};
pub use target::{
    CategoryScanTarget, HomebrewScan, NodeScan, SystemCacheScan, UserCacheScan, XcodeScan,
};
