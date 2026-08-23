use std::{path::Path, process::Command};

use cleaner_core::ApplicationMetadata;

pub(crate) fn extract_application_metadata(app_path: &Path) -> ApplicationMetadata {
    ApplicationMetadata {
        bundle_identifier: plist_string(app_path, "CFBundleIdentifier"),
        bundle_version: plist_string(app_path, "CFBundleVersion"),
        short_version: plist_string(app_path, "CFBundleShortVersionString"),
        team_identifier: signing_team_identifier(app_path),
    }
}

fn plist_string(app_path: &Path, key: &str) -> Option<String> {
    let info_plist = app_path.join("Contents/Info.plist");
    let output = Command::new("plutil")
        .arg("-extract")
        .arg(key)
        .arg("raw")
        .arg("-o")
        .arg("-")
        .arg(info_plist)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    non_empty_utf8(&output.stdout)
}

fn signing_team_identifier(app_path: &Path) -> Option<String> {
    let output = Command::new("codesign")
        .arg("-d")
        .arg("--verbose=4")
        .arg(app_path)
        .output()
        .ok()?;

    let diagnostics = String::from_utf8_lossy(&output.stderr);
    parse_team_identifier(&diagnostics)
}

fn parse_team_identifier(diagnostics: &str) -> Option<String> {
    diagnostics.lines().find_map(|line| {
        let value = line.strip_prefix("TeamIdentifier=")?.trim();
        (!value.is_empty() && value != "not set").then(|| value.to_string())
    })
}

fn non_empty_utf8(bytes: &[u8]) -> Option<String> {
    let value = String::from_utf8(bytes.to_vec()).ok()?;
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_team_identifier_from_codesign_diagnostics() {
        let diagnostics = "Executable=/Applications/Example.app/Contents/MacOS/Example\nIdentifier=com.example.app\nTeamIdentifier=ABCDE12345\n";
        assert_eq!(
            parse_team_identifier(diagnostics).as_deref(),
            Some("ABCDE12345")
        );
    }

    #[test]
    fn treats_missing_team_identifier_as_unknown() {
        assert_eq!(parse_team_identifier("TeamIdentifier=not set\n"), None);
        assert_eq!(parse_team_identifier("Identifier=com.example.app\n"), None);
    }
}
