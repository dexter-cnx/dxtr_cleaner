use std::path::PathBuf;

use crate::{CleanupCategory, ScanRequest};

pub trait CategoryScanTarget {
    fn category(&self) -> CleanupCategory;
    fn roots(&self) -> Vec<PathBuf>;

    fn excluded_roots(&self) -> Vec<PathBuf> {
        Vec::new()
    }

    fn request(&self) -> ScanRequest {
        ScanRequest {
            category: self.category(),
            roots: self.roots(),
            excluded_roots: self.excluded_roots(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserCacheScan {
    home: PathBuf,
}

impl UserCacheScan {
    pub fn new(home: impl Into<PathBuf>) -> Self {
        Self { home: home.into() }
    }
}

impl CategoryScanTarget for UserCacheScan {
    fn category(&self) -> CleanupCategory {
        CleanupCategory::UserCache
    }

    fn roots(&self) -> Vec<PathBuf> {
        vec![self.home.join("Library/Caches")]
    }

    fn excluded_roots(&self) -> Vec<PathBuf> {
        vec![self.home.join("Library/Caches/Homebrew")]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XcodeScan {
    home: PathBuf,
}

impl XcodeScan {
    pub fn new(home: impl Into<PathBuf>) -> Self {
        Self { home: home.into() }
    }
}

impl CategoryScanTarget for XcodeScan {
    fn category(&self) -> CleanupCategory {
        CleanupCategory::Xcode
    }

    fn roots(&self) -> Vec<PathBuf> {
        vec![self.home.join("Library/Developer/Xcode/DerivedData")]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomebrewScan {
    home: PathBuf,
}

impl HomebrewScan {
    pub fn new(home: impl Into<PathBuf>) -> Self {
        Self { home: home.into() }
    }
}

impl CategoryScanTarget for HomebrewScan {
    fn category(&self) -> CleanupCategory {
        CleanupCategory::Homebrew
    }

    fn roots(&self) -> Vec<PathBuf> {
        vec![self.home.join("Library/Caches/Homebrew")]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeScan {
    home: PathBuf,
}

impl NodeScan {
    pub fn new(home: impl Into<PathBuf>) -> Self {
        Self { home: home.into() }
    }
}

impl CategoryScanTarget for NodeScan {
    fn category(&self) -> CleanupCategory {
        CleanupCategory::Node
    }

    fn roots(&self) -> Vec<PathBuf> {
        vec![self.home.join(".npm"), self.home.join("Library/pnpm/store")]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_targets_build_expected_requests() {
        let home = PathBuf::from("/Users/tester");

        let user = UserCacheScan::new(home.clone()).request();
        assert_eq!(user.category, CleanupCategory::UserCache);
        assert_eq!(user.roots, vec![home.join("Library/Caches")]);
        assert_eq!(
            user.excluded_roots,
            vec![home.join("Library/Caches/Homebrew")]
        );

        let xcode = XcodeScan::new(home.clone()).request();
        assert_eq!(xcode.category, CleanupCategory::Xcode);
        assert_eq!(
            xcode.roots,
            vec![home.join("Library/Developer/Xcode/DerivedData")]
        );
    }
}
