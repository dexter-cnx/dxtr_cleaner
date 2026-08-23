mod model;
mod planner;
mod safety;
mod scanner;
mod target;

pub use model::{CleanupCategory, CleanupPlan, CleanupPlanItem, ScanItem, ScanSummary};
pub use planner::{AllowedRoot, ExecutionPolicy, Planner, SafetyError};
pub use scanner::{
    CancellationToken, FileSystemScanner, ScanEvent, ScanEventSink, ScanRequest, Scanner,
};
pub use target::{
    CategoryScanTarget, HomebrewScan, NodeScan, SystemCacheScan, UserCacheScan, XcodeScan,
};
