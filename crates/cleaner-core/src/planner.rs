use std::{fs, path::PathBuf};

use crate::{
    CleanupCategory, CleanupPlan, CleanupPlanItem, ScanItem, safety::is_protected_broad_root,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowedRoot {
    pub category: CleanupCategory,
    pub path: PathBuf,
    canonical_path: Option<PathBuf>,
}

impl AllowedRoot {
    pub fn new(category: CleanupCategory, path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let canonical_path = fs::canonicalize(&path).ok();
        Self {
            category,
            path,
            canonical_path,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExecutionPolicy {
    pub destructive_actions_enabled: bool,
    pub allowed_roots: Vec<AllowedRoot>,
}

impl ExecutionPolicy {
    pub fn enabled(allowed_roots: Vec<AllowedRoot>) -> Self {
        Self {
            destructive_actions_enabled: true,
            allowed_roots,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SafetyError {
    DestructiveActionsDisabled,
    SymlinkSelected,
    ProtectedRoot,
    MissingPath,
    PathRevalidationFailed,
    OutsideAllowedRoots,
}

pub struct Planner;

impl Planner {
    pub fn build(items: Vec<ScanItem>) -> CleanupPlan {
        CleanupPlan {
            items: items
                .into_iter()
                .map(|item| CleanupPlanItem {
                    selected: !item.is_symlink,
                    item,
                })
                .collect(),
        }
    }

    pub fn validate_for_execution(
        plan: &CleanupPlan,
        policy: &ExecutionPolicy,
    ) -> Result<(), SafetyError> {
        if !policy.destructive_actions_enabled {
            return Err(SafetyError::DestructiveActionsDisabled);
        }

        for entry in plan.items.iter().filter(|entry| entry.selected) {
            Self::validate_item_for_execution(&entry.item, policy)?;
        }

        Ok(())
    }

    pub fn validate_item_for_execution(
        item: &ScanItem,
        policy: &ExecutionPolicy,
    ) -> Result<PathBuf, SafetyError> {
        if !policy.destructive_actions_enabled {
            return Err(SafetyError::DestructiveActionsDisabled);
        }
        if item.is_symlink {
            return Err(SafetyError::SymlinkSelected);
        }
        if is_protected_broad_root(&item.path) {
            return Err(SafetyError::ProtectedRoot);
        }

        let metadata = fs::symlink_metadata(&item.path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                SafetyError::MissingPath
            } else {
                SafetyError::PathRevalidationFailed
            }
        })?;
        if metadata.file_type().is_symlink() {
            return Err(SafetyError::SymlinkSelected);
        }

        let canonical_path =
            fs::canonicalize(&item.path).map_err(|_| SafetyError::PathRevalidationFailed)?;
        if is_protected_broad_root(&canonical_path) {
            return Err(SafetyError::ProtectedRoot);
        }

        let allowed = policy.allowed_roots.iter().any(|allowed| {
            if allowed.category != item.category {
                return false;
            }

            let Some(pinned_root) = allowed.canonical_path.as_ref() else {
                return false;
            };

            let Ok(root_metadata) = fs::symlink_metadata(&allowed.path) else {
                return false;
            };
            if root_metadata.file_type().is_symlink() {
                return false;
            }

            let Ok(current_root) = fs::canonicalize(&allowed.path) else {
                return false;
            };
            if current_root != *pinned_root {
                return false;
            }

            canonical_path != *pinned_root && canonical_path.starts_with(pinned_root)
        });

        if !allowed {
            return Err(SafetyError::OutsideAllowedRoots);
        }

        Ok(canonical_path)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "dxtr-cleaner-{name}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    fn item(path: PathBuf, category: CleanupCategory, is_symlink: bool) -> ScanItem {
        ScanItem {
            path,
            category,
            bytes: 0,
            is_symlink,
        }
    }

    #[cfg(windows)]
    fn protected_system_root() -> PathBuf {
        PathBuf::from(r"C:\Windows")
    }

    #[cfg(windows)]
    fn protected_system_descendant() -> PathBuf {
        PathBuf::from(r"C:\Windows\Temp")
    }

    #[cfg(not(windows))]
    fn protected_system_root() -> PathBuf {
        PathBuf::from("/Library")
    }

    #[cfg(not(windows))]
    fn protected_system_descendant() -> PathBuf {
        PathBuf::from("/Library/Caches")
    }

    #[test]
    fn destructive_execution_is_off_by_default() {
        let plan = CleanupPlan::default();
        assert_eq!(
            Planner::validate_for_execution(&plan, &ExecutionPolicy::default()),
            Err(SafetyError::DestructiveActionsDisabled)
        );
    }

    #[test]
    fn symlinks_are_not_selected_by_default() {
        let plan = Planner::build(vec![item(
            PathBuf::from("/tmp/link"),
            CleanupCategory::UserCache,
            true,
        )]);
        assert!(!plan.items[0].selected);
    }

    #[test]
    fn broad_system_root_is_rejected_for_execution() {
        let plan = CleanupPlan {
            items: vec![CleanupPlanItem {
                item: item(protected_system_root(), CleanupCategory::SystemCache, false),
                selected: true,
            }],
        };

        assert_eq!(
            Planner::validate_for_execution(
                &plan,
                &ExecutionPolicy::enabled(vec![AllowedRoot::new(
                    CleanupCategory::SystemCache,
                    protected_system_descendant(),
                )]),
            ),
            Err(SafetyError::ProtectedRoot)
        );
    }

    #[test]
    fn execution_requires_a_category_matching_allow_list_root() {
        let root = temp_root("allow-list");
        let child = root.join("cache.bin");
        fs::write(&child, b"cache").expect("write cache file");
        let scan_item = item(child, CleanupCategory::UserCache, false);

        let allowed = ExecutionPolicy::enabled(vec![AllowedRoot::new(
            CleanupCategory::UserCache,
            root.clone(),
        )]);
        assert!(Planner::validate_item_for_execution(&scan_item, &allowed).is_ok());

        let wrong_category =
            ExecutionPolicy::enabled(vec![AllowedRoot::new(CleanupCategory::Node, root.clone())]);
        assert_eq!(
            Planner::validate_item_for_execution(&scan_item, &wrong_category),
            Err(SafetyError::OutsideAllowedRoots)
        );

        fs::remove_dir_all(root).expect("remove temp root");
    }

    #[test]
    fn execution_never_allows_the_allow_list_root_itself() {
        let root = temp_root("root-itself");
        let scan_item = item(root.clone(), CleanupCategory::UserCache, false);
        let policy = ExecutionPolicy::enabled(vec![AllowedRoot::new(
            CleanupCategory::UserCache,
            root.clone(),
        )]);

        assert_eq!(
            Planner::validate_item_for_execution(&scan_item, &policy),
            Err(SafetyError::OutsideAllowedRoots)
        );

        fs::remove_dir_all(root).expect("remove temp root");
    }

    #[cfg(unix)]
    #[test]
    fn execution_revalidates_symlinks_created_after_scan() {
        use std::os::unix::fs::symlink;

        let root = temp_root("symlink-swap");
        let path = root.join("cache.bin");
        let target = root.join("target.bin");
        fs::write(&path, b"old cache").expect("write original file");
        fs::write(&target, b"target").expect("write target file");

        let scan_item = item(path.clone(), CleanupCategory::UserCache, false);
        fs::remove_file(&path).expect("remove original file");
        symlink(&target, &path).expect("replace with symlink");

        let policy = ExecutionPolicy::enabled(vec![AllowedRoot::new(
            CleanupCategory::UserCache,
            root.clone(),
        )]);
        assert_eq!(
            Planner::validate_item_for_execution(&scan_item, &policy),
            Err(SafetyError::SymlinkSelected)
        );

        fs::remove_dir_all(root).expect("remove temp root");
    }

    #[cfg(unix)]
    #[test]
    fn execution_rejects_allow_list_root_swapped_to_symlink() {
        use std::os::unix::fs::symlink;

        let parent = temp_root("root-swap-parent");
        let root = parent.join("cache");
        let moved_root = parent.join("cache-original");
        let outside = temp_root("root-swap-outside");
        fs::create_dir_all(&root).expect("create allow-list root");
        fs::write(root.join("cache.bin"), b"cache").expect("write cache file");
        fs::write(outside.join("cache.bin"), b"outside").expect("write outside file");

        let policy = ExecutionPolicy::enabled(vec![AllowedRoot::new(
            CleanupCategory::UserCache,
            root.clone(),
        )]);
        let scan_item = item(root.join("cache.bin"), CleanupCategory::UserCache, false);

        fs::rename(&root, &moved_root).expect("move original root");
        symlink(&outside, &root).expect("replace root with symlink");

        assert_eq!(
            Planner::validate_item_for_execution(&scan_item, &policy),
            Err(SafetyError::OutsideAllowedRoots)
        );

        fs::remove_file(&root).expect("remove root symlink");
        fs::remove_dir_all(parent).expect("remove parent");
        fs::remove_dir_all(outside).expect("remove outside root");
    }

    #[test]
    fn missing_paths_fail_closed_at_execution_time() {
        let root = temp_root("missing");
        let scan_item = item(
            root.join(Path::new("gone.bin")),
            CleanupCategory::UserCache,
            false,
        );
        let policy = ExecutionPolicy::enabled(vec![AllowedRoot::new(
            CleanupCategory::UserCache,
            root.clone(),
        )]);

        assert_eq!(
            Planner::validate_item_for_execution(&scan_item, &policy),
            Err(SafetyError::MissingPath)
        );

        fs::remove_dir_all(root).expect("remove temp root");
    }
}
