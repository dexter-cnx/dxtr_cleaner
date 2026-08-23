use std::path::PathBuf;

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
    pub fn selected_bytes(&self) -> u64 {
        self.items
            .iter()
            .filter(|entry| entry.selected)
            .map(|entry| entry.item.bytes)
            .sum()
    }
}
