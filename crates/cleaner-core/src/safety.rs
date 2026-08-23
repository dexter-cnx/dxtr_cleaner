use std::path::Path;

const PROTECTED_BROAD_ROOTS: &[&str] = &[
    "/",
    "/Applications",
    "/Library",
    "/System",
    "/Users",
    "/bin",
    "/private",
    "/sbin",
    "/usr",
];

pub(crate) fn is_protected_broad_root(path: &Path) -> bool {
    PROTECTED_BROAD_ROOTS
        .iter()
        .any(|protected| path == Path::new(protected))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protects_broad_system_roots_but_allows_descendants() {
        assert!(is_protected_broad_root(Path::new("/")));
        assert!(is_protected_broad_root(Path::new("/System")));
        assert!(is_protected_broad_root(Path::new("/Library")));
        assert!(!is_protected_broad_root(Path::new("/Library/Caches")));
        assert!(!is_protected_broad_root(Path::new(
            "/Users/tester/Library/Caches"
        )));
    }
}
