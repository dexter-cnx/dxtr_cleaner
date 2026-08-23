use std::path::{Component, Path, PathBuf};

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
    let normalized = normalize_lexically(path);
    PROTECTED_BROAD_ROOTS
        .iter()
        .any(|protected| normalized == Path::new(protected))
}

fn normalize_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Path::new("/")),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }

    normalized
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

    #[test]
    fn normalizes_parent_components_before_policy_check() {
        assert!(is_protected_broad_root(Path::new("/System/..")));
        assert!(is_protected_broad_root(Path::new("/Library/../System")));
        assert!(!is_protected_broad_root(Path::new(
            "/Library/../Library/Caches"
        )));
    }
}
