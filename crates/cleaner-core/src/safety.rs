use std::{
    fs,
    path::{Component, Path, PathBuf},
};

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
    let candidate = resolve_for_policy(path);

    PROTECTED_BROAD_ROOTS
        .iter()
        .any(|protected| candidate == resolve_for_policy(Path::new(protected)))
}

fn resolve_for_policy(path: &Path) -> PathBuf {
    let resolved = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    normalize_lexically_case_insensitive(&resolved)
}

fn normalize_lexically_case_insensitive(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => {
                normalized.push(prefix.as_os_str().to_string_lossy().to_lowercase())
            }
            Component::RootDir => normalized.push(Path::new("/")),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part.to_string_lossy().to_lowercase()),
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
    fn protects_case_variants_of_broad_roots() {
        assert!(is_protected_broad_root(Path::new("/system")));
        assert!(is_protected_broad_root(Path::new("/LIBRARY")));
        assert!(is_protected_broad_root(Path::new("/users")));
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
