use crate::CleanupCategory;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupAction {
    MoveToTrash,
    PermanentDelete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionPolicyError {
    PermanentDeleteNotAllowed(CleanupCategory),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CategoryActionPolicy {
    permanent_delete_categories: Vec<CleanupCategory>,
}

impl CategoryActionPolicy {
    pub fn trash_only() -> Self {
        Self::default()
    }

    pub fn action_for(&self, category: CleanupCategory) -> CleanupAction {
        if self.permanent_delete_categories.contains(&category) {
            CleanupAction::PermanentDelete
        } else {
            CleanupAction::MoveToTrash
        }
    }

    pub fn enable_permanent_delete(
        &mut self,
        category: CleanupCategory,
    ) -> Result<(), ActionPolicyError> {
        if !supports_permanent_delete(category) {
            return Err(ActionPolicyError::PermanentDeleteNotAllowed(category));
        }

        if !self.permanent_delete_categories.contains(&category) {
            self.permanent_delete_categories.push(category);
        }

        Ok(())
    }
}

pub const fn supports_permanent_delete(category: CleanupCategory) -> bool {
    matches!(
        category,
        CleanupCategory::UserCache
            | CleanupCategory::Xcode
            | CleanupCategory::Homebrew
            | CleanupCategory::Node
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trash_is_the_default_for_every_category() {
        let policy = CategoryActionPolicy::trash_only();

        for category in [
            CleanupCategory::UserCache,
            CleanupCategory::SystemCache,
            CleanupCategory::Xcode,
            CleanupCategory::Homebrew,
            CleanupCategory::Node,
            CleanupCategory::Docker,
            CleanupCategory::LargeFiles,
        ] {
            assert_eq!(policy.action_for(category), CleanupAction::MoveToTrash);
        }
    }

    #[test]
    fn permanent_delete_requires_an_explicit_safe_category_opt_in() {
        let mut policy = CategoryActionPolicy::trash_only();

        policy
            .enable_permanent_delete(CleanupCategory::Xcode)
            .expect("xcode generated data can opt in");
        assert_eq!(
            policy.action_for(CleanupCategory::Xcode),
            CleanupAction::PermanentDelete
        );
        assert_eq!(
            policy.action_for(CleanupCategory::LargeFiles),
            CleanupAction::MoveToTrash
        );
    }

    #[test]
    fn permanent_delete_is_blocked_for_sensitive_or_ambiguous_categories() {
        let mut policy = CategoryActionPolicy::trash_only();

        for category in [
            CleanupCategory::SystemCache,
            CleanupCategory::Docker,
            CleanupCategory::LargeFiles,
        ] {
            assert_eq!(
                policy.enable_permanent_delete(category),
                Err(ActionPolicyError::PermanentDeleteNotAllowed(category))
            );
        }
    }
}
