use std::{env, path::PathBuf, sync::mpsc, thread};

use cleaner_core::{
    ApplicationInventory, CancellationToken, InstalledApplication, RelatedFileExecutionRoot,
    RelatedFileMatcher, UninstallExecutionPolicy, UninstallExecutionReport, UninstallExecutor,
    UninstallPlan,
};
use cleaner_macos::SystemMacPlatform;

pub enum InventoryMessage {
    Loaded(Vec<InstalledApplication>),
    Failed(String),
}

pub enum PlanMessage {
    Ready(UninstallPlan),
    Failed(String),
}

pub enum UninstallMessage {
    Completed(UninstallExecutionReport),
    Failed(String),
}

pub fn spawn_inventory() -> mpsc::Receiver<InventoryMessage> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        if env::var_os("HOME").is_none() {
            let _ = tx.send(InventoryMessage::Failed(
                "HOME is not set; application inventory is incomplete".into(),
            ));
            return;
        }

        let platform = SystemMacPlatform;
        let report = platform.inventory();
        if !report.issues.is_empty() {
            let detail = report
                .issues
                .iter()
                .take(3)
                .map(|issue| format!("{}: {}", issue.path.display(), issue.message))
                .collect::<Vec<_>>()
                .join("; ");
            let _ = tx.send(InventoryMessage::Failed(format!(
                "application inventory is incomplete: {detail}"
            )));
            return;
        }

        let _ = tx.send(InventoryMessage::Loaded(report.applications));
    });
    rx
}

pub fn spawn_plan(application: InstalledApplication) -> mpsc::Receiver<PlanMessage> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        if env::var_os("HOME").is_none() {
            let _ = tx.send(PlanMessage::Failed(
                "HOME is not set; related-file review cannot be complete".into(),
            ));
            return;
        }

        let platform = SystemMacPlatform;
        let related = platform.related_files(&application);
        let _ = tx.send(PlanMessage::Ready(UninstallPlan::build(
            application,
            related,
        )));
    });
    rx
}

pub fn spawn_uninstall(
    plan: UninstallPlan,
    cancellation: CancellationToken,
) -> mpsc::Receiver<UninstallMessage> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let Some(home) = env::var_os("HOME").map(PathBuf::from) else {
            let _ = tx.send(UninstallMessage::Failed(
                "HOME is not set; uninstall execution is blocked".into(),
            ));
            return;
        };

        let platform = SystemMacPlatform;
        let inventory = platform.inventory();
        if !inventory.issues.is_empty() {
            let _ = tx.send(UninstallMessage::Failed(
                "application inventory changed or is incomplete; refresh and review again".into(),
            ));
            return;
        }

        let reviewed = plan.application();
        let current = inventory.applications.iter().find(|application| {
            application.path == reviewed.path
                && application.location == reviewed.location
                && application.metadata.bundle_identifier == reviewed.metadata.bundle_identifier
        });
        let Some(current) = current else {
            let _ = tx.send(UninstallMessage::Failed(
                "application identity changed; refresh and review again".into(),
            ));
            return;
        };

        let fresh_related = platform.related_files(current);
        let roots = execution_roots(&home);
        let policy = match UninstallExecutionPolicy::pin(&plan, &fresh_related, &roots) {
            Ok(policy) => policy,
            Err(error) => {
                let _ = tx.send(UninstallMessage::Failed(format!(
                    "uninstall safety validation failed: {error:?}"
                )));
                return;
            }
        };

        match UninstallExecutor::execute(&plan, &policy, current, &cancellation, &platform) {
            Ok(report) => {
                let _ = tx.send(UninstallMessage::Completed(report));
            }
            Err(error) => {
                let _ = tx.send(UninstallMessage::Failed(format!(
                    "uninstall execution blocked: {error:?}"
                )));
            }
        }
    });
    rx
}

fn execution_roots(home: &std::path::Path) -> Vec<RelatedFileExecutionRoot> {
    use cleaner_core::RelatedFileKind;

    let library = home.join("Library");
    vec![
        RelatedFileExecutionRoot::new(
            RelatedFileKind::ApplicationSupport,
            library.join("Application Support"),
        ),
        RelatedFileExecutionRoot::new(RelatedFileKind::Cache, library.join("Caches")),
        RelatedFileExecutionRoot::new(RelatedFileKind::Container, library.join("Containers")),
        RelatedFileExecutionRoot::new(RelatedFileKind::HttpStorage, library.join("HTTPStorages")),
        RelatedFileExecutionRoot::new(RelatedFileKind::Preference, library.join("Preferences")),
        RelatedFileExecutionRoot::new(
            RelatedFileKind::SavedState,
            library.join("Saved Application State"),
        ),
    ]
}
