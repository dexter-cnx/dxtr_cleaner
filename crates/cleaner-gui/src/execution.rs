use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
};

use cleaner_core::{
    AllowedRoot, CancellationToken, CategoryActionPolicy, CleanupExecutor, CleanupPlan,
    ExecutionPolicy, ExecutionReport, PermanentDeleteBackend, ScanRequest, TrashBackend,
};
use cleaner_macos::{MacPlatform, SystemMacPlatform};

pub enum ExecutionMessage {
    Progress { completed: usize, total: usize },
    Completed(ExecutionReport),
    Failed(String),
}

struct ProgressBackend {
    platform: SystemMacPlatform,
    tx: mpsc::Sender<ExecutionMessage>,
    completed: Arc<AtomicUsize>,
    total: usize,
}

impl ProgressBackend {
    fn report_progress(&self) {
        let completed = self.completed.fetch_add(1, Ordering::Relaxed) + 1;
        let _ = self.tx.send(ExecutionMessage::Progress {
            completed,
            total: self.total,
        });
    }
}

impl TrashBackend for ProgressBackend {
    fn move_to_trash(&self, path: &Path) -> Result<(), String> {
        let result = MacPlatform::move_to_trash(&self.platform, path);
        self.report_progress();
        result
    }
}

impl PermanentDeleteBackend for ProgressBackend {
    fn permanent_delete(&self, path: &Path) -> Result<(), String> {
        let result = MacPlatform::permanent_delete(&self.platform, path);
        self.report_progress();
        result
    }
}

pub fn policy_from_requests(requests: &[ScanRequest]) -> ExecutionPolicy {
    let allowed_roots = requests
        .iter()
        .flat_map(|request| {
            request
                .roots
                .iter()
                .cloned()
                .map(move |root| AllowedRoot::new(request.category, root))
        })
        .collect();

    ExecutionPolicy::enabled(allowed_roots)
}

pub fn spawn_trash_only_execution(
    plan: CleanupPlan,
    policy: ExecutionPolicy,
    cancellation: CancellationToken,
) -> mpsc::Receiver<ExecutionMessage> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let total = plan.selected_count();
        let action_policy = CategoryActionPolicy::trash_only();
        let backend = ProgressBackend {
            platform: SystemMacPlatform,
            tx: tx.clone(),
            completed: Arc::new(AtomicUsize::new(0)),
            total,
        };
        let message =
            match CleanupExecutor::execute(&plan, &policy, &action_policy, &cancellation, &backend)
            {
                Ok(report) => ExecutionMessage::Completed(report),
                Err(error) => ExecutionMessage::Failed(format!("{error:?}")),
            };
        let _ = tx.send(message);
    });

    rx
}
