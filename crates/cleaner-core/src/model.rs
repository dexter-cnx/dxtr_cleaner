use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CleanupCategory {
    UserCache,
    SystemCache,
    Xcode,
    Homebrew,
    Node,
    Docker,
    LargeFiles,
}

impl CleanupCategory {
    pub const fn label(self) -> &'static str {
        match self {
            Self::UserCache => "User Cache",
            Self::SystemCache => "System Cache",
            Self::Xcode => "Xcode",
            Self::Homebrew => "Homebrew",
            Self::Node => "Node",
            Self::Docker => "Docker",
            Self::LargeFiles => "Large Files",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanItem {
    pub path: PathBuf,
    pub category: CleanupCategory,
    pub bytes: u64,
    pub is_symlink: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScanSummary {
    pub item_count: usize,
    pub total_bytes: u64,
}

impl ScanSummary {
    pub fn from_items(items: &[ScanItem]) -> Self {
        Self {
            item_count: items.len(),
            total_bytes: items.iter().map(|item| item.bytes).sum(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupPlanItem {
    pub item: ScanItem,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CleanupPlan {
    pub items: Vec<CleanupPlanItem>,
}

impl CleanupPlan {
    pub fn selected_count(&self) -> usize {
        self.items.iter().filter(|entry| entry.selected).count()
    }

    pub fn selected_bytes(&self) -> u64 {
        self.items
            .iter()
            .filter(|entry| entry.selected)
            .map(|entry| entry.item.bytes)
            .sum()
    }

    pub fn set_all_selected(&mut self, selected: bool) {
        for entry in &mut self.items {
            entry.selected = selected && !entry.item.is_symlink;
        }
    }

    pub fn set_category_selected(&mut self, category: CleanupCategory, selected: bool) {
        for entry in &mut self.items {
            if entry.item.category == category {
                entry.selected = selected && !entry.item.is_symlink;
            }
        }
    }

    pub fn toggle_path(&mut self, path: &Path) -> bool {
        let Some(entry) = self.items.iter_mut().find(|entry| entry.item.path == path) else {
            return false;
        };

        if entry.item.is_symlink {
            return false;
        }

        entry.selected = !entry.selected;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(path: &str, category: CleanupCategory, bytes: u64, is_symlink: bool) -> ScanItem {
        ScanItem {
            path: PathBuf::from(path),
            category,
            bytes,
            is_symlink,
        }
    }

    #[test]
    fn plan_review_helpers_keep_symlinks_unselected() {
        let mut plan = CleanupPlan {
            items: vec![
                CleanupPlanItem {
                    item: item("/tmp/cache", CleanupCategory::UserCache, 10, false),
                    selected: false,
                },
                CleanupPlanItem {
                    item: item("/tmp/link", CleanupCategory::UserCache, 20, true),
                    selected: false,
                },
            ],
        };

        plan.set_all_selected(true);

        assert!(plan.items[0].selected);
        assert!(!plan.items[1].selected);
        assert_eq!(plan.selected_count(), 1);
        assert_eq!(plan.selected_bytes(), 10);
    }

    #[test]
    fn bulk_selection_normalizes_preselected_symlinks() {
        let mut plan = CleanupPlan {
            items: vec![
                CleanupPlanItem {
                    item: item("/tmp/cache", CleanupCategory::UserCache, 10, false),
                    selected: true,
                },
                CleanupPlanItem {
                    item: item("/tmp/link", CleanupCategory::UserCache, 20, true),
                    selected: true,
                },
            ],
        };

        plan.set_all_selected(false);
        assert!(!plan.items[0].selected);
        assert!(!plan.items[1].selected);

        plan.items[1].selected = true;
        plan.set_category_selected(CleanupCategory::UserCache, true);
        assert!(plan.items[0].selected);
        assert!(!plan.items[1].selected);
    }

    #[test]
    fn category_and_path_selection_are_frontend_neutral() {
        let mut plan = CleanupPlan {
            items: vec![
                CleanupPlanItem {
                    item: item("/tmp/a", CleanupCategory::UserCache, 10, false),
                    selected: true,
                },
                CleanupPlanItem {
                    item: item("/tmp/b", CleanupCategory::Node, 20, false),
                    selected: true,
                },
            ],
        };

        plan.set_category_selected(CleanupCategory::Node, false);
        assert!(plan.items[0].selected);
        assert!(!plan.items[1].selected);

        assert!(plan.toggle_path(Path::new("/tmp/a")));
        assert!(!plan.items[0].selected);
        assert!(!plan.toggle_path(Path::new("/tmp/missing")));
    }
}
