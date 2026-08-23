use std::{collections::HashSet, fs, path::Path};

use cleaner_core::{
    InstalledApplication, MatchConfidence, OrphanCandidate, OrphanFinderIssue, OrphanReport,
    RelatedFileKind, is_apple_bundle_identifier, is_safe_bundle_identifier,
};

#[derive(Clone, Copy)]
enum ExpectedEntryKind {
    Directory,
    File,
}

pub(crate) fn find_orphans_for_home(
    installed: &[InstalledApplication],
    home: &Path,
) -> OrphanReport {
    let live_bundle_identifiers: HashSet<&str> = installed
        .iter()
        .filter_map(|application| application.metadata.bundle_identifier.as_deref())
        .filter(|identifier| is_safe_bundle_identifier(identifier))
        .collect();

    let library = home.join("Library");
    let mut report = OrphanReport::default();

    for (relative, kind) in [
        ("Application Support", RelatedFileKind::ApplicationSupport),
        ("Caches", RelatedFileKind::Cache),
        ("Containers", RelatedFileKind::Container),
        ("HTTPStorages", RelatedFileKind::HttpStorage),
    ] {
        collect_directory_bundle_entries(
            &mut report,
            &library.join(relative),
            kind,
            &live_bundle_identifiers,
        );
    }

    collect_suffixed_entries(
        &mut report,
        &library.join("Preferences"),
        ".plist",
        RelatedFileKind::Preference,
        ExpectedEntryKind::File,
        &live_bundle_identifiers,
    );
    collect_suffixed_entries(
        &mut report,
        &library.join("Saved Application State"),
        ".savedState",
        RelatedFileKind::SavedState,
        ExpectedEntryKind::Directory,
        &live_bundle_identifiers,
    );

    report.sort_deterministically();
    report
}

fn collect_directory_bundle_entries(
    report: &mut OrphanReport,
    root: &Path,
    kind: RelatedFileKind,
    live_bundle_identifiers: &HashSet<&str>,
) {
    collect_entries(
        report,
        root,
        kind,
        ExpectedEntryKind::Directory,
        live_bundle_identifiers,
        |name| looks_like_bundle_identifier(name).then(|| name.to_owned()),
    );
}

fn collect_suffixed_entries(
    report: &mut OrphanReport,
    root: &Path,
    suffix: &str,
    kind: RelatedFileKind,
    expected_entry_kind: ExpectedEntryKind,
    live_bundle_identifiers: &HashSet<&str>,
) {
    collect_entries(
        report,
        root,
        kind,
        expected_entry_kind,
        live_bundle_identifiers,
        |name| {
            let identifier = name.strip_suffix(suffix)?;
            looks_like_bundle_identifier(identifier).then(|| identifier.to_owned())
        },
    );
}

fn collect_entries<F>(
    report: &mut OrphanReport,
    root: &Path,
    kind: RelatedFileKind,
    expected_entry_kind: ExpectedEntryKind,
    live_bundle_identifiers: &HashSet<&str>,
    bundle_identifier_from_name: F,
) where
    F: Fn(&str) -> Option<String>,
{
    let metadata = match fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => {
            report.issues.push(OrphanFinderIssue::new(
                root.to_path_buf(),
                error.to_string(),
            ));
            return;
        }
    };

    if metadata.file_type().is_symlink() {
        report.issues.push(OrphanFinderIssue::new(
            root.to_path_buf(),
            "orphan finder root is a symlink and was skipped",
        ));
        return;
    }
    if !metadata.is_dir() {
        report.issues.push(OrphanFinderIssue::new(
            root.to_path_buf(),
            "orphan finder root is not a directory",
        ));
        return;
    }

    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) => {
            report.issues.push(OrphanFinderIssue::new(
                root.to_path_buf(),
                error.to_string(),
            ));
            return;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                report.issues.push(OrphanFinderIssue::new(
                    root.to_path_buf(),
                    error.to_string(),
                ));
                continue;
            }
        };
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                report
                    .issues
                    .push(OrphanFinderIssue::new(path, error.to_string()));
                continue;
            }
        };
        if file_type.is_symlink() || !matches_expected_entry_kind(&file_type, expected_entry_kind) {
            continue;
        }

        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(bundle_identifier) = bundle_identifier_from_name(name) else {
            continue;
        };

        if live_bundle_identifiers.contains(bundle_identifier.as_str())
            || is_apple_bundle_identifier(&bundle_identifier)
        {
            continue;
        }

        report.candidates.push(OrphanCandidate::new(
            bundle_identifier,
            path,
            kind,
            MatchConfidence::High,
        ));
    }
}

fn matches_expected_entry_kind(
    file_type: &std::fs::FileType,
    expected_entry_kind: ExpectedEntryKind,
) -> bool {
    match expected_entry_kind {
        ExpectedEntryKind::Directory => file_type.is_dir(),
        ExpectedEntryKind::File => file_type.is_file(),
    }
}

fn looks_like_bundle_identifier(identifier: &str) -> bool {
    is_safe_bundle_identifier(identifier) && identifier.split('.').count() >= 2
}

#[cfg(test)]
mod tests {
    use super::*;
    use cleaner_core::{ApplicationLocation, ApplicationMetadata};
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_root(test_name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock must be after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "dxtr-cleaner-orphan-{test_name}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("temp root must be created");
        path
    }

    fn installed(bundle_identifier: &str) -> InstalledApplication {
        InstalledApplication::new(
            PathBuf::from("/Applications/Example.app"),
            "Example",
            ApplicationLocation::Local,
        )
        .with_metadata(ApplicationMetadata {
            bundle_identifier: Some(bundle_identifier.into()),
            ..ApplicationMetadata::default()
        })
    }

    #[test]
    fn reports_bundle_shaped_entries_missing_from_live_app_set() {
        let home = temp_root("missing");
        fs::create_dir_all(home.join("Library/Caches/com.example.removed"))
            .expect("orphan fixture must be created");
        fs::create_dir_all(home.join("Library/Caches/com.example.live"))
            .expect("live fixture must be created");

        let report = find_orphans_for_home(&[installed("com.example.live")], &home);

        assert_eq!(report.candidates.len(), 1);
        assert_eq!(
            report.candidates[0].bundle_identifier,
            "com.example.removed"
        );
        assert_eq!(report.candidates[0].confidence, MatchConfidence::High);

        fs::remove_dir_all(home).expect("temp root must be removed");
    }

    #[test]
    fn skips_generic_directories_and_apple_namespace() {
        let home = temp_root("protected");
        fs::create_dir_all(home.join("Library/Caches/Adobe"))
            .expect("generic fixture must be created");
        fs::create_dir_all(home.join("Library/Caches/com.apple.Safari"))
            .expect("Apple fixture must be created");
        fs::create_dir_all(home.join("Library/Caches/com.example.orphan"))
            .expect("orphan fixture must be created");

        let report = find_orphans_for_home(&[], &home);

        assert_eq!(report.candidates.len(), 1);
        assert_eq!(report.candidates[0].bundle_identifier, "com.example.orphan");

        fs::remove_dir_all(home).expect("temp root must be removed");
    }

    #[test]
    fn recognizes_preference_and_saved_state_suffixes() {
        let home = temp_root("suffixes");
        fs::create_dir_all(home.join("Library/Preferences"))
            .expect("preferences root must be created");
        fs::create_dir_all(
            home.join("Library/Saved Application State/com.example.state.savedState"),
        )
        .expect("saved state fixture must be created");
        fs::write(
            home.join("Library/Preferences/com.example.pref.plist"),
            b"fixture",
        )
        .expect("preference fixture must be created");

        let report = find_orphans_for_home(&[], &home);

        assert_eq!(report.candidates.len(), 2);
        assert!(report.candidates.iter().any(|candidate| {
            candidate.bundle_identifier == "com.example.pref"
                && candidate.kind == RelatedFileKind::Preference
        }));
        assert!(report.candidates.iter().any(|candidate| {
            candidate.bundle_identifier == "com.example.state"
                && candidate.kind == RelatedFileKind::SavedState
        }));

        fs::remove_dir_all(home).expect("temp root must be removed");
    }

    #[test]
    fn requires_expected_filesystem_entry_kind() {
        let home = temp_root("entry-kind");
        fs::create_dir_all(home.join("Library/Preferences/com.example.not-a-file.plist"))
            .expect("directory preference fixture must be created");
        fs::create_dir_all(home.join("Library/Caches")).expect("cache root must be created");
        fs::write(
            home.join("Library/Caches/com.example.not-a-directory"),
            b"fixture",
        )
        .expect("cache file fixture must be created");

        let report = find_orphans_for_home(&[], &home);

        assert!(report.candidates.is_empty());
        fs::remove_dir_all(home).expect("temp root must be removed");
    }

    #[cfg(unix)]
    #[test]
    fn never_returns_symlink_candidates() {
        use std::os::unix::fs::symlink;

        let home = temp_root("symlink");
        let outside = temp_root("symlink-outside");
        fs::create_dir_all(home.join("Library/Caches")).expect("cache root must be created");
        symlink(&outside, home.join("Library/Caches/com.example.orphan"))
            .expect("symlink fixture must be created");

        let report = find_orphans_for_home(&[], &home);

        assert!(report.candidates.is_empty());

        fs::remove_dir_all(home).expect("temp root must be removed");
        fs::remove_dir_all(outside).expect("outside root must be removed");
    }
}
