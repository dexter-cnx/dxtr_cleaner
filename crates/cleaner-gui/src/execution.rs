use std::{sync::mpsc, thread};

use cleaner_core::{
    AllowedRoot, CancellationToken, CategoryActionPolicy, CleanupExecutor, CleanupPlan,
    ExecutionPolicy, ExecutionReport, ScanRequest,
};
use cleaner_macos::SystemMacPlatform;

pub enum ExecutionMessage {
    Completed(ExecutionReport),
    Failed(String),
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
        let action_policy = CategoryActionPolicy::trash_only();
        let backend = SystemMacPlatform;
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
