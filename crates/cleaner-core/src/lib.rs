mod action_policy;
mod executor;
mod model;
mod planner;
mod safety;
mod scanner;
mod target;

pub use action_policy::{
    ActionPolicyError, CategoryActionPolicy, CleanupAction, supports_permanent_delete,
};
pub use executor::{
    CleanupBackend, CleanupExecutor, ExecutionFailure, ExecutionRecord, ExecutionReport,
    PermanentDeleteBackend, TrashBackend,
};
pub use model::{CleanupCategory, CleanupPlan, CleanupPlanItem, ScanItem, ScanSummary};
pub use planner::{AllowedRoot, ExecutionPolicy, Planner, SafetyError};
pub use scanner::{
    CancellationToken, FileSystemScanner, ScanEvent, ScanEventSink, ScanRequest, Scanner,
};
pub use target::{
    CategoryScanTarget, HomebrewScan, NodeScan, SystemCacheScan, UserCacheScan, XcodeScan,
};
