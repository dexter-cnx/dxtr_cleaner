use std::{fs, path::Path};

use cleaner_core::{
    InstalledApplication, MatchConfidence, MatchEvidence, RelatedFileCandidate, RelatedFileKind,
    RelatedFileReport,
};

pub(crate) fn related_files_for_home(
    application: &InstalledApplication,
    home: &Path,
) -> RelatedFileReport {
    let library = home.join("Library");
    let mut report = RelatedFileReport::default();

    if let Some(bundle_identifier) = application
        .metadata
        .bundle_identifier
        .as_deref()
        .filter(|identifier| is_safe_bundle_identifier(identifier))
    {
        let exact = MatchEvidence::ExactBundleIdentifier(bundle_identifier.to_owned());
        push_if_safe(
            &mut report,
            library.join("Application Support").join(bundle_identifier),
            RelatedFileKind::ApplicationSupport,
            MatchConfidence::High,
            exact.clone(),
        );
        push_if_safe(
            &mut report,
            library.join("Caches").join(bundle_identifier),
            RelatedFileKind::Cache,
            MatchConfidence::High,
            exact.clone(),
        );
        push_if_safe(
            &mut report,
            library.join("Containers").join(bundle_identifier),
            RelatedFileKind::Container,
            MatchConfidence::High,
            exact.clone(),
        );
        push_if_safe(
            &mut report,
            library.join("HTTPStorages").join(bundle_identifier),
            RelatedFileKind::HttpStorage,
            MatchConfidence::High,
            exact.clone(),
        );
        push_if_safe(
            &mut report,
            library
                .join("Preferences")
                .join(format!("{bundle_identifier}.plist")),
            RelatedFileKind::Preference,
            MatchConfidence::High,
            exact.clone(),
        );
        push_if_safe(
            &mut report,
            library
                .join("Saved Application State")
                .join(format!("{bundle_identifier}.savedState")),
            RelatedFileKind::SavedState,
            MatchConfidence::High,
            exact,
        );

        collect_bundle_prefixed_entries(
            &mut report,
            &library.join("Preferences/ByHost"),
            bundle_identifier,
            RelatedFileKind::Preference,
        );
    }

    if !application.name.is_empty() {
        let evidence = MatchEvidence::ExactDisplayName(application.name.clone());
        push_if_safe(
            &mut report,
            library.join("Application Support").join(&application.name),
            RelatedFileKind::ApplicationSupport,
            MatchConfidence::Low,
            evidence.clone(),
        );
        push_if_safe(
            &mut report,
            library.join("Caches").join(&application.name),
            RelatedFileKind::Cache,
            MatchConfidence::Low,
            evidence,
        );
    }

    report.sort_deterministically();
    report
}

fn is_safe_bundle_identifier(identifier: &str) -> bool {
    if identifier.is_empty() || identifier.starts_with('.') || identifier.ends_with('.') {
        return false;
    }

    identifier.split('.').all(|component| {
        !component.is_empty()
            && component != "."
            && component != ".."
            && component
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    })
}

fn collect_bundle_prefixed_entries(
    report: &mut RelatedFileReport,
    directory: &Path,
    bundle_identifier: &str,
    kind: RelatedFileKind,
) {
    let metadata = match fs::symlink_metadata(directory) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => metadata,
        _ => return,
    };
    let _ = metadata;

    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let prefix = format!("{bundle_identifier}.");

    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !name.starts_with(&prefix) {
            continue;
        }
        report.candidates.push(RelatedFileCandidate::new(
            entry.path(),
            kind,
            MatchConfidence::Medium,
            MatchEvidence::BundleIdentifierPrefix(bundle_identifier.to_owned()),
        ));
    }
}

fn push_if_safe(
    report: &mut RelatedFileReport,
    path: std::path::PathBuf,
    kind: RelatedFileKind,
    confidence: MatchConfidence,
    evidence: MatchEvidence,
) {
    let Ok(metadata) = fs::symlink_metadata(&path) else {
        return;
    };
    if metadata.file_type().is_symlink() {
        return;
    }
    report
        .candidates
        .push(RelatedFileCandidate::new(path, kind, confidence, evidence));
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
            "dxtr-cleaner-related-{test_name}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("temp root must be created");
        path
    }

    fn application() -> InstalledApplication {
        InstalledApplication::new(
            PathBuf::from("/Applications/Example.app"),
            "Example",
            ApplicationLocation::Local,
        )
        .with_metadata(ApplicationMetadata {
            bundle_identifier: Some("com.example.app".into()),
            ..ApplicationMetadata::default()
        })
    }

    #[test]
    fn exact_bundle_paths_are_high_confidence_and_name_paths_are_low() {
        let home = temp_root("tiers");
        fs::create_dir_all(home.join("Library/Caches/com.example.app"))
            .expect("bundle cache fixture must be created");
        fs::create_dir_all(home.join("Library/Application Support/Example"))
            .expect("name support fixture must be created");

        let report = related_files_for_home(&application(), &home);

        assert_eq!(report.candidates.len(), 2);
        assert!(report.candidates.iter().any(|candidate| {
            candidate.path == home.join("Library/Caches/com.example.app")
                && candidate.confidence == MatchConfidence::High
        }));
        assert!(report.candidates.iter().any(|candidate| {
            candidate.path == home.join("Library/Application Support/Example")
                && candidate.confidence == MatchConfidence::Low
                && candidate.confidence.is_review_only()
        }));

        fs::remove_dir_all(home).expect("temp root must be removed");
    }

    #[test]
    fn rejects_unsafe_bundle_identifiers_before_joining_paths() {
        for identifier in [
            "..",
            "/",
            "com/example/app",
            "com..example",
            ".com.example",
            "com.example.",
        ] {
            assert!(!is_safe_bundle_identifier(identifier), "{identifier}");
        }
        assert!(is_safe_bundle_identifier("com.example.app"));
        assert!(is_safe_bundle_identifier("com.example-app_2"));
    }

    #[test]
    fn unsafe_bundle_identifier_never_creates_high_confidence_candidate() {
        let home = temp_root("unsafe-bundle-id");
        fs::create_dir_all(home.join("Library")).expect("Library fixture must be created");
        let application = InstalledApplication::new(
            PathBuf::from("/Applications/Example.app"),
            "",
            ApplicationLocation::Local,
        )
        .with_metadata(ApplicationMetadata {
            bundle_identifier: Some("..".into()),
            ..ApplicationMetadata::default()
        });

        let report = related_files_for_home(&application, &home);

        assert!(report.candidates.is_empty());
        fs::remove_dir_all(home).expect("temp root must be removed");
    }

    #[test]
    fn byhost_bundle_prefix_is_medium_confidence() {
        let home = temp_root("byhost");
        let by_host = home.join("Library/Preferences/ByHost");
        fs::create_dir_all(&by_host).expect("ByHost fixture must be created");
        fs::write(by_host.join("com.example.app.ABC.plist"), b"fixture")
            .expect("preference fixture must be created");
        fs::write(
            by_host.join("com.example.application.ABC.plist"),
            b"fixture",
        )
        .expect("non-match fixture must be created");

        let report = related_files_for_home(&application(), &home);

        assert_eq!(report.candidates.len(), 1);
        assert_eq!(report.candidates[0].confidence, MatchConfidence::Medium);
        assert!(report.candidates[0].confidence.is_review_only());

        fs::remove_dir_all(home).expect("temp root must be removed");
    }

    #[cfg(unix)]
    #[test]
    fn matcher_never_returns_symlink_candidates() {
        use std::os::unix::fs::symlink;

        let home = temp_root("symlink");
        let outside = temp_root("symlink-outside");
        fs::create_dir_all(home.join("Library/Caches")).expect("cache root must be created");
        symlink(&outside, home.join("Library/Caches/com.example.app"))
            .expect("symlink fixture must be created");

        let report = related_files_for_home(&application(), &home);

        assert!(report.candidates.is_empty());

        fs::remove_dir_all(home).expect("temp root must be removed");
        fs::remove_dir_all(outside).expect("outside root must be removed");
    }
}
