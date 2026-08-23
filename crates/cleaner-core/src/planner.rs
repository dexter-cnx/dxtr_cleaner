use crate::{CleanupPlan, CleanupPlanItem, ScanItem, safety::is_protected_broad_root};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExecutionPolicy {
    pub destructive_actions_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SafetyError {
    DestructiveActionsDisabled,
    SymlinkSelected,
    ProtectedRoot,
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
        policy: ExecutionPolicy,
    ) -> Result<(), SafetyError> {
        if !policy.destructive_actions_enabled {
            return Err(SafetyError::DestructiveActionsDisabled);
        }

        for entry in plan.items.iter().filter(|entry| entry.selected) {
            if entry.item.is_symlink {
                return Err(SafetyError::SymlinkSelected);
            }
            if is_protected_broad_root(&entry.item.path) {
                return Err(SafetyError::ProtectedRoot);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::CleanupCategory;

    #[test]
    fn destructive_execution_is_off_by_default() {
        let plan = CleanupPlan::default();
        assert_eq!(
            Planner::validate_for_execution(&plan, ExecutionPolicy::default()),
            Err(SafetyError::DestructiveActionsDisabled)
        );
    }

    #[test]
    fn symlinks_are_not_selected_by_default() {
        let plan = Planner::build(vec![ScanItem {
            path: PathBuf::from("/tmp/link"),
            category: CleanupCategory::UserCache,
            bytes: 0,
            is_symlink: true,
        }]);
        assert!(!plan.items[0].selected);
    }

    #[test]
    fn broad_system_root_is_rejected_for_execution() {
        let plan = CleanupPlan {
            items: vec![CleanupPlanItem {
                item: ScanItem {
                    path: PathBuf::from("/Library"),
                    category: CleanupCategory::SystemCache,
                    bytes: 0,
                    is_symlink: false,
                },
                selected: true,
            }],
        };

        assert_eq!(
            Planner::validate_for_execution(
                &plan,
                ExecutionPolicy {
                    destructive_actions_enabled: true,
                },
            ),
            Err(SafetyError::ProtectedRoot)
        );
    }
}
