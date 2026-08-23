use std::{fs, path::{Path, PathBuf}};

use crate::{
    safety::is_protected_broad_root, ApplicationLocation, ApplicationProtectionPolicy,
    CancellationToken, InstalledApplication, TrashBackend, UninstallPlan, UninstallPlanItemKind,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UninstallExecutionError {
    ProtectedApplication,
    StalePlan,
    MissingPath(PathBuf),
    Symlink(PathBuf),
    PathChanged(PathBuf),
    UnexpectedEntryType(PathBuf),
    ProtectedRoot(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UninstallExecutionRecord {
    pub path: PathBuf,
    pub kind: UninstallPlanItemKind,
    pub result: Result<(), String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UninstallExecutionReport {
    pub records: Vec<UninstallExecutionRecord>,
    pub cancelled: bool,
}

impl UninstallExecutionReport {
    pub fn succeeded_count(&self) -> usize {
        self.records.iter().filter(|record| record.result.is_ok()).count()
    }

    pub fn failed_count(&self) -> usize {
        self.records.iter().filter(|record| record.result.is_err()).count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ApplicationIdentity {
    path: PathBuf,
    location: ApplicationLocation,
    bundle_identifier: Option<String>,
}

impl ApplicationIdentity {
    fn from_application(application: &InstalledApplication) -> Self {
        Self {
            path: application.path.clone(),
            location: application.location,
            bundle_identifier: application.metadata.bundle_identifier.clone(),
        }
    }

    fn matches(&self, application: &InstalledApplication) -> bool {
        self.path == application.path
            && self.location == application.location
            && self.bundle_identifier == application.metadata.bundle_identifier
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PinnedUninstallItem {
    path: PathBuf,
    canonical_path: PathBuf,
    kind: UninstallPlanItemKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UninstallExecutionPolicy {
    application: ApplicationIdentity,
    selected_paths: Vec<PathBuf>,
    pinned_items: Vec<PinnedUninstallItem>,
}

impl UninstallExecutionPolicy {
    pub fn pin(plan: &UninstallPlan) -> Result<Self, UninstallExecutionError> {
        if plan.is_protected() {
            return Err(UninstallExecutionError::ProtectedApplication);
        }

        let mut pinned_items = Vec::new();
        for item in plan.items().iter().filter(|item| item.is_selected()) {
            let path = item.path().to_path_buf();
            let canonical_path = validate_and_canonicalize(&path, item.kind())?;
            pinned_items.push(PinnedUninstallItem {
                path,
                canonical_path,
                kind: item.kind(),
            });
        }

        // Move related data first and the required application bundle last. If execution is
        // cancelled part-way through, leaving the app installed is safer than removing it first.
        pinned_items.sort_by_key(|item| matches!(item.kind, UninstallPlanItemKind::ApplicationBundle));
        let selected_paths = pinned_items.iter().map(|item| item.path.clone()).collect();

        Ok(Self {
            application: ApplicationIdentity::from_application(plan.application()),
            selected_paths,
            pinned_items,
        })
    }
}

pub struct UninstallExecutor;

impl UninstallExecutor {
    pub fn execute(
        plan: &UninstallPlan,
        policy: &UninstallExecutionPolicy,
        current_application: &InstalledApplication,
        cancellation: &CancellationToken,
        backend: &dyn TrashBackend,
    ) -> Result<UninstallExecutionReport, UninstallExecutionError> {
        if plan.is_protected()
            || ApplicationProtectionPolicy
                .evaluate(current_application)
                .is_protected()
        {
            return Err(UninstallExecutionError::ProtectedApplication);
        }

        if !policy.application.matches(current_application) {
            return Err(UninstallExecutionError::StalePlan);
        }

        let mut selected_paths: Vec<PathBuf> = plan
            .items()
            .iter()
            .filter(|item| item.is_selected())
            .map(|item| item.path().to_path_buf())
            .collect();
        selected_paths.sort();
        let mut pinned_paths = policy.selected_paths.clone();
        pinned_paths.sort();
        if selected_paths != pinned_paths {
            return Err(UninstallExecutionError::StalePlan);
        }

        let mut report = UninstallExecutionReport::default();
        for item in &policy.pinned_items {
            if cancellation.is_cancelled() {
                report.cancelled = true;
                break;
            }

            let canonical_path = validate_and_canonicalize(&item.path, item.kind)?;
            if canonical_path != item.canonical_path {
                return Err(UninstallExecutionError::PathChanged(item.path.clone()));
            }

            let result = backend.move_to_trash(&canonical_path);
            report.records.push(UninstallExecutionRecord {
                path: canonical_path,
                kind: item.kind,
                result,
            });
        }

        Ok(report)
    }
}

fn validate_and_canonicalize(
    path: &Path,
    kind: UninstallPlanItemKind,
) -> Result<PathBuf, UninstallExecutionError> {
    if is_protected_broad_root(path) {
        return Err(UninstallExecutionError::ProtectedRoot(path.to_path_buf()));
    }

    let metadata = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            UninstallExecutionError::MissingPath(path.to_path_buf())
        } else {
            UninstallExecutionError::PathChanged(path.to_path_buf())
        }
    })?;

    if metadata.file_type().is_symlink() {
        return Err(UninstallExecutionError::Symlink(path.to_path_buf()));
    }

    let type_matches = match kind {
        UninstallPlanItemKind::ApplicationBundle => metadata.is_dir(),
        UninstallPlanItemKind::RelatedFile(related_kind) => match related_kind {
            crate::RelatedFileKind::Preference => metadata.is_file(),
            crate::RelatedFileKind::ApplicationSupport
            | crate::RelatedFileKind::Cache
            | crate::RelatedFileKind::Container
            | crate::RelatedFileKind::HttpStorage
            | crate::RelatedFileKind::SavedState => metadata.is_dir(),
        },
    };
    if !type_matches {
        return Err(UninstallExecutionError::UnexpectedEntryType(
            path.to_path_buf(),
        ));
    }

    let canonical_path =
        fs::canonicalize(path).map_err(|_| UninstallExecutionError::PathChanged(path.to_path_buf()))?;
    if is_protected_broad_root(&canonical_path) {
        return Err(UninstallExecutionError::ProtectedRoot(canonical_path));
    }
    Ok(canonical_path)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::Mutex,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;
    use crate::{
        ApplicationMetadata, MatchConfidence, MatchEvidence, RelatedFileCandidate,
        RelatedFileKind, RelatedFileReport,
    };

    #[derive(Default)]
    struct RecordingTrash {
        paths: Mutex<Vec<PathBuf>>,
    }

    impl TrashBackend for RecordingTrash {
        fn move_to_trash(&self, path: &Path) -> Result<(), String> {
            self.paths.lock().expect("trash lock").push(path.to_path_buf());
            Ok(())
        }
    }

    fn temp_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "dxtr-cleaner-uninstall-{name}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    fn application(path: PathBuf, bundle_identifier: &str) -> InstalledApplication {
        InstalledApplication::new(path, "Example", ApplicationLocation::Local).with_metadata(
            ApplicationMetadata {
                bundle_identifier: Some(bundle_identifier.into()),
                ..ApplicationMetadata::default()
            },
        )
    }

    fn plan_fixture(root: &Path) -> UninstallPlan {
        let app_path = root.join("Example.app");
        let cache_path = root.join("com.example.app");
        fs::create_dir_all(&app_path).expect("create app");
        fs::create_dir_all(&cache_path).expect("create cache");
        UninstallPlan::build(
            application(app_path, "com.example.app"),
            RelatedFileReport {
                candidates: vec![RelatedFileCandidate::new(
                    cache_path,
                    RelatedFileKind::Cache,
                    MatchConfidence::High,
                    MatchEvidence::ExactBundleIdentifier("com.example.app".into()),
                )],
            },
        )
    }

    #[test]
    fn policy_refuses_protected_application() {
        let root = temp_root("protected");
        let app_path = root.join("Safari.app");
        fs::create_dir_all(&app_path).expect("create app");
        let plan = UninstallPlan::build(
            application(app_path, "com.apple.Safari"),
            RelatedFileReport::default(),
        );

        assert_eq!(
            UninstallExecutionPolicy::pin(&plan),
            Err(UninstallExecutionError::ProtectedApplication)
        );
        fs::remove_dir_all(root).expect("remove temp root");
    }

    #[test]
    fn selection_change_after_pinning_is_rejected_as_stale() {
        let root = temp_root("stale-selection");
        let mut plan = plan_fixture(&root);
        let policy = UninstallExecutionPolicy::pin(&plan).expect("pin plan");
        let cache = root.join("com.example.app");
        assert!(plan.set_selected(&cache, false));
        let current = plan.application().clone();

        assert_eq!(
            UninstallExecutor::execute(
                &plan,
                &policy,
                &current,
                &CancellationToken::new(),
                &RecordingTrash::default(),
            ),
            Err(UninstallExecutionError::StalePlan)
        );
        fs::remove_dir_all(root).expect("remove temp root");
    }

    #[test]
    fn application_identity_change_is_rejected_as_stale() {
        let root = temp_root("stale-app");
        let plan = plan_fixture(&root);
        let policy = UninstallExecutionPolicy::pin(&plan).expect("pin plan");
        let current = application(root.join("Example.app"), "com.example.changed");

        assert_eq!(
            UninstallExecutor::execute(
                &plan,
                &policy,
                &current,
                &CancellationToken::new(),
                &RecordingTrash::default(),
            ),
            Err(UninstallExecutionError::StalePlan)
        );
        fs::remove_dir_all(root).expect("remove temp root");
    }

    #[test]
    fn trash_order_keeps_application_bundle_last() {
        let root = temp_root("order");
        let plan = plan_fixture(&root);
        let policy = UninstallExecutionPolicy::pin(&plan).expect("pin plan");
        let current = plan.application().clone();
        let backend = RecordingTrash::default();

        let report = UninstallExecutor::execute(
            &plan,
            &policy,
            &current,
            &CancellationToken::new(),
            &backend,
        )
        .expect("execute plan");

        assert_eq!(report.succeeded_count(), 2);
        let paths = backend.paths.lock().expect("trash lock");
        assert_eq!(paths[0], fs::canonicalize(root.join("com.example.app")).unwrap());
        assert_eq!(paths[1], fs::canonicalize(root.join("Example.app")).unwrap());
        fs::remove_dir_all(root).expect("remove temp root");
    }
}
