use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    safety::is_protected_broad_root, ApplicationLocation, ApplicationProtectionPolicy,
    CancellationToken, InstalledApplication, RelatedFileKind, RelatedFileReport, TrashBackend,
    UninstallPlan, UninstallPlanItemKind,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UninstallExecutionError {
    ProtectedApplication,
    StalePlan,
    UnverifiedPath(PathBuf),
    MissingExecutionRoot(RelatedFileKind),
    MissingPath(PathBuf),
    Symlink(PathBuf),
    PathChanged(PathBuf),
    UnexpectedEntryType(PathBuf),
    ProtectedRoot(PathBuf),
    OutsideExecutionRoot(PathBuf),
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
    pub safety_failure: Option<UninstallExecutionError>,
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
pub struct RelatedFileExecutionRoot {
    pub kind: RelatedFileKind,
    pub path: PathBuf,
}

impl RelatedFileExecutionRoot {
    pub fn new(kind: RelatedFileKind, path: impl Into<PathBuf>) -> Self {
        Self {
            kind,
            path: path.into(),
        }
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
struct PinnedExecutionRoot {
    kind: RelatedFileKind,
    path: PathBuf,
    canonical_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PinnedUninstallItem {
    path: PathBuf,
    canonical_path: PathBuf,
    kind: UninstallPlanItemKind,
    root: Option<PinnedExecutionRoot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UninstallExecutionPolicy {
    application: ApplicationIdentity,
    selected_paths: Vec<PathBuf>,
    pinned_items: Vec<PinnedUninstallItem>,
}

impl UninstallExecutionPolicy {
    pub fn pin(
        plan: &UninstallPlan,
        verified_related: &RelatedFileReport,
        execution_roots: &[RelatedFileExecutionRoot],
    ) -> Result<Self, UninstallExecutionError> {
        if plan.is_protected() {
            return Err(UninstallExecutionError::ProtectedApplication);
        }

        let mut pinned_items = Vec::new();
        for item in plan.items().iter().filter(|item| item.is_selected()) {
            let path = item.path().to_path_buf();
            let root = match item.kind() {
                UninstallPlanItemKind::ApplicationBundle => {
                    if item.path() != plan.application().path {
                        return Err(UninstallExecutionError::UnverifiedPath(path));
                    }
                    None
                }
                UninstallPlanItemKind::RelatedFile(kind) => {
                    let verified = verified_related.candidates.iter().any(|candidate| {
                        candidate.path == item.path()
                            && candidate.kind == kind
                            && candidate.confidence == item.confidence()
                    });
                    if !verified {
                        return Err(UninstallExecutionError::UnverifiedPath(path));
                    }
                    Some(pin_execution_root(kind, execution_roots)?)
                }
            };

            let canonical_path = validate_and_canonicalize(&path, item.kind(), root.as_ref())?;
            pinned_items.push(PinnedUninstallItem {
                path,
                canonical_path,
                kind: item.kind(),
                root,
            });
        }

        // Related data first, required application bundle last. Cancellation then preferentially
        // leaves the application installed rather than removing it before its related data.
        pinned_items
            .sort_by_key(|item| matches!(item.kind, UninstallPlanItemKind::ApplicationBundle));
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

            let canonical_path = match validate_and_canonicalize(&item.path, item.kind, item.root.as_ref()) {
                Ok(path) => path,
                Err(error) => {
                    report.safety_failure = Some(error);
                    break;
                }
            };
            if canonical_path != item.canonical_path {
                report.safety_failure = Some(UninstallExecutionError::PathChanged(item.path.clone()));
                break;
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

fn pin_execution_root(
    kind: RelatedFileKind,
    roots: &[RelatedFileExecutionRoot],
) -> Result<PinnedExecutionRoot, UninstallExecutionError> {
    let root = roots
        .iter()
        .find(|root| root.kind == kind)
        .ok_or(UninstallExecutionError::MissingExecutionRoot(kind))?;
    let metadata = fs::symlink_metadata(&root.path)
        .map_err(|_| UninstallExecutionError::PathChanged(root.path.clone()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(UninstallExecutionError::PathChanged(root.path.clone()));
    }
    let canonical_path = fs::canonicalize(&root.path)
        .map_err(|_| UninstallExecutionError::PathChanged(root.path.clone()))?;
    if is_protected_broad_root(&canonical_path) {
        return Err(UninstallExecutionError::ProtectedRoot(canonical_path));
    }
    Ok(PinnedExecutionRoot {
        kind,
        path: root.path.clone(),
        canonical_path,
    })
}

fn validate_and_canonicalize(
    path: &Path,
    kind: UninstallPlanItemKind,
    root: Option<&PinnedExecutionRoot>,
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
            RelatedFileKind::Preference => metadata.is_file(),
            RelatedFileKind::ApplicationSupport
            | RelatedFileKind::Cache
            | RelatedFileKind::Container
            | RelatedFileKind::HttpStorage
            | RelatedFileKind::SavedState => metadata.is_dir(),
        },
    };
    if !type_matches {
        return Err(UninstallExecutionError::UnexpectedEntryType(path.to_path_buf()));
    }

    let canonical_path = fs::canonicalize(path)
        .map_err(|_| UninstallExecutionError::PathChanged(path.to_path_buf()))?;
    if is_protected_broad_root(&canonical_path) {
        return Err(UninstallExecutionError::ProtectedRoot(canonical_path));
    }

    if let Some(root) = root {
        if root.kind != match kind {
            UninstallPlanItemKind::RelatedFile(kind) => kind,
            UninstallPlanItemKind::ApplicationBundle => unreachable!("application has no related root"),
        } {
            return Err(UninstallExecutionError::OutsideExecutionRoot(path.to_path_buf()));
        }

        let root_metadata = fs::symlink_metadata(&root.path)
            .map_err(|_| UninstallExecutionError::PathChanged(root.path.clone()))?;
        if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
            return Err(UninstallExecutionError::PathChanged(root.path.clone()));
        }
        let current_root = fs::canonicalize(&root.path)
            .map_err(|_| UninstallExecutionError::PathChanged(root.path.clone()))?;
        if current_root != root.canonical_path {
            return Err(UninstallExecutionError::PathChanged(root.path.clone()));
        }
        if canonical_path == root.canonical_path || !canonical_path.starts_with(&root.canonical_path) {
            return Err(UninstallExecutionError::OutsideExecutionRoot(path.to_path_buf()));
        }
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
    use crate::{ApplicationMetadata, MatchConfidence, MatchEvidence, RelatedFileCandidate};

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

    struct RemovingTrash;

    impl TrashBackend for RemovingTrash {
        fn move_to_trash(&self, path: &Path) -> Result<(), String> {
            if path.is_dir() {
                fs::remove_dir_all(path).map_err(|error| error.to_string())
            } else {
                fs::remove_file(path).map_err(|error| error.to_string())
            }
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

    fn related_report(cache_root: &Path) -> RelatedFileReport {
        RelatedFileReport {
            candidates: vec![RelatedFileCandidate::new(
                cache_root.join("com.example.app"),
                RelatedFileKind::Cache,
                MatchConfidence::High,
                MatchEvidence::ExactBundleIdentifier("com.example.app".into()),
            )],
        }
    }

    fn plan_fixture(root: &Path) -> (UninstallPlan, RelatedFileReport, Vec<RelatedFileExecutionRoot>) {
        let app_path = root.join("Example.app");
        let cache_root = root.join("Caches");
        let cache_path = cache_root.join("com.example.app");
        fs::create_dir_all(&app_path).expect("create app");
        fs::create_dir_all(&cache_path).expect("create cache");
        let related = related_report(&cache_root);
        let roots = vec![RelatedFileExecutionRoot::new(RelatedFileKind::Cache, cache_root)];
        let plan = UninstallPlan::build(application(app_path, "com.example.app"), related.clone());
        (plan, related, roots)
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
            UninstallExecutionPolicy::pin(&plan, &RelatedFileReport::default(), &[]),
            Err(UninstallExecutionError::ProtectedApplication)
        );
        fs::remove_dir_all(root).expect("remove temp root");
    }

    #[test]
    fn policy_rejects_related_path_missing_from_fresh_evidence() {
        let root = temp_root("unverified");
        let (plan, _, roots) = plan_fixture(&root);
        let expected = root.join("Caches/com.example.app");
        assert_eq!(
            UninstallExecutionPolicy::pin(&plan, &RelatedFileReport::default(), &roots),
            Err(UninstallExecutionError::UnverifiedPath(expected))
        );
        fs::remove_dir_all(root).expect("remove temp root");
    }

    #[cfg(unix)]
    #[test]
    fn policy_rejects_symlinked_related_root_escape() {
        use std::os::unix::fs::symlink;

        let root = temp_root("root-escape");
        let outside = temp_root("root-escape-outside");
        let cache_root = root.join("Caches");
        fs::create_dir_all(outside.join("com.example.app")).expect("outside candidate");
        symlink(&outside, &cache_root).expect("link cache root");
        let app_path = root.join("Example.app");
        fs::create_dir_all(&app_path).expect("create app");
        let related = related_report(&cache_root);
        let plan = UninstallPlan::build(application(app_path, "com.example.app"), related.clone());
        let roots = vec![RelatedFileExecutionRoot::new(RelatedFileKind::Cache, cache_root.clone())];

        assert_eq!(
            UninstallExecutionPolicy::pin(&plan, &related, &roots),
            Err(UninstallExecutionError::PathChanged(cache_root.clone()))
        );

        fs::remove_file(cache_root).expect("remove symlink");
        fs::remove_dir_all(root).expect("remove root");
        fs::remove_dir_all(outside).expect("remove outside");
    }

    #[test]
    fn selection_change_after_pinning_is_rejected_as_stale() {
        let root = temp_root("stale-selection");
        let (mut plan, related, roots) = plan_fixture(&root);
        let policy = UninstallExecutionPolicy::pin(&plan, &related, &roots).expect("pin plan");
        let cache = root.join("Caches/com.example.app");
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
        let (plan, related, roots) = plan_fixture(&root);
        let policy = UninstallExecutionPolicy::pin(&plan, &related, &roots).expect("pin plan");
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
        let (plan, related, roots) = plan_fixture(&root);
        let policy = UninstallExecutionPolicy::pin(&plan, &related, &roots).expect("pin plan");
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
        assert!(report.safety_failure.is_none());
        let paths = backend.paths.lock().expect("trash lock");
        assert_eq!(paths[0], fs::canonicalize(root.join("Caches/com.example.app")).unwrap());
        assert_eq!(paths[1], fs::canonicalize(root.join("Example.app")).unwrap());
        fs::remove_dir_all(root).expect("remove temp root");
    }

    #[test]
    fn runtime_revalidation_failure_preserves_prior_records() {
        let root = temp_root("partial-report");
        let (plan, related, roots) = plan_fixture(&root);
        let policy = UninstallExecutionPolicy::pin(&plan, &related, &roots).expect("pin plan");
        let current = plan.application().clone();

        // Related data is first. RemovingTrash deletes it. The next application item remains valid,
        // so force a later revalidation failure by replacing the application directory with a file
        // after pinning and before execution.
        fs::remove_dir_all(root.join("Example.app")).expect("remove app dir");
        fs::write(root.join("Example.app"), b"not a directory").expect("replace app with file");

        let report = UninstallExecutor::execute(
            &plan,
            &policy,
            &current,
            &CancellationToken::new(),
            &RemovingTrash,
        )
        .expect("partial report");

        assert_eq!(report.succeeded_count(), 1);
        assert_eq!(report.records.len(), 1);
        assert!(matches!(
            report.safety_failure,
            Some(UninstallExecutionError::UnexpectedEntryType(_))
        ));
        fs::remove_file(root.join("Example.app")).expect("remove replacement app");
        fs::remove_dir_all(root).expect("remove root");
    }
}
