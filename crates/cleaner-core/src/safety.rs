use std::{
    fs,
    path::{Component, Path, PathBuf},
};

#[cfg(not(windows))]
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

#[cfg(windows)]
const WINDOWS_PROTECTED_TOP_LEVEL: &[&str] = &[
    "windows",
    "program files",
    "program files (x86)",
    "program files (arm)",
    "programdata",
    "users",
];

pub(crate) fn is_protected_broad_root(path: &Path) -> bool {
    let candidate = resolve_for_policy(path);

    #[cfg(windows)]
    {
        return is_windows_protected_broad_root(&candidate);
    }

    #[cfg(not(windows))]
    {
        PROTECTED_BROAD_ROOTS
            .iter()
            .any(|protected| candidate == resolve_for_policy(Path::new(protected)))
    }
}

#[cfg(windows)]
fn is_windows_protected_broad_root(path: &Path) -> bool {
    let mut components = path.components();

    if !matches!(components.next(), Some(Component::Prefix(_))) {
        return false;
    }
    if !matches!(components.next(), Some(Component::RootDir)) {
        return false;
    }

    let first = components.next();
    if components.next().is_some() {
        return false;
    }

    match first {
        None => true,
        Some(Component::Normal(name)) => WINDOWS_PROTECTED_TOP_LEVEL
            .iter()
            .any(|protected| name.to_string_lossy().eq_ignore_ascii_case(protected)),
        _ => false,
    }
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

    #[cfg(not(windows))]
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

    #[cfg(not(windows))]
    #[test]
    fn protects_case_variants_of_broad_roots() {
        assert!(is_protected_broad_root(Path::new("/system")));
        assert!(is_protected_broad_root(Path::new("/LIBRARY")));
        assert!(is_protected_broad_root(Path::new("/users")));
    }

    #[cfg(not(windows))]
    #[test]
    fn normalizes_parent_components_before_policy_check() {
        assert!(is_protected_broad_root(Path::new("/System/..")));
        assert!(is_protected_broad_root(Path::new("/Library/../System")));
        assert!(!is_protected_broad_root(Path::new(
            "/Library/../Library/Caches"
        )));
    }

    #[cfg(windows)]
    #[test]
    fn protects_windows_drive_and_system_roots_but_allows_descendants() {
        assert!(is_protected_broad_root(Path::new(r"C:\")));
        assert!(is_protected_broad_root(Path::new(r"C:\Windows")));
        assert!(is_protected_broad_root(Path::new(r"D:\Program Files")));
        assert!(is_protected_broad_root(Path::new(
            r"C:\Program Files (x86)"
        )));
        assert!(is_protected_broad_root(Path::new(
            r"C:\Program Files (Arm)"
        )));
        assert!(is_protected_broad_root(Path::new(r"C:\ProgramData")));
        assert!(is_protected_broad_root(Path::new(r"C:\Users")));
        assert!(!is_protected_broad_root(Path::new(r"C:\Windows\Temp")));
        assert!(!is_protected_broad_root(Path::new(
            r"C:\Users\tester\AppData\Local\Temp"
        )));
    }

    #[cfg(windows)]
    #[test]
    fn protects_windows_case_variants_and_parent_normalization() {
        assert!(is_protected_broad_root(Path::new(r"c:\WINDOWS")));
        assert!(is_protected_broad_root(Path::new(r"C:\Temp\..\Windows")));
        assert!(!is_protected_broad_root(Path::new(
            r"C:\Windows\Temp\..\Temp"
        )));
    }

    #[cfg(windows)]
    #[test]
    fn protects_unc_share_root() {
        assert!(is_protected_broad_root(Path::new(r"\\server\share\")));
        assert!(!is_protected_broad_root(Path::new(r"\\server\share\cache")));
    }
}
