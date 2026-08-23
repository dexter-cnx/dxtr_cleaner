use std::path::{Path, PathBuf};

use crate::{ApplicationLocation, InstalledApplication};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplicationProtectionReason {
    SystemLocation,
    SystemPath(PathBuf),
    AppleBundleIdentifier(String),
}

impl ApplicationProtectionReason {
    pub fn label(&self) -> &'static str {
        match self {
            Self::SystemLocation => "system location",
            Self::SystemPath(_) => "system application path",
            Self::AppleBundleIdentifier(_) => "Apple bundle identifier",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplicationProtection {
    Unprotected,
    Protected(Vec<ApplicationProtectionReason>),
}

impl ApplicationProtection {
    pub fn is_protected(&self) -> bool {
        matches!(self, Self::Protected(_))
    }

    pub fn reasons(&self) -> &[ApplicationProtectionReason] {
        match self {
            Self::Unprotected => &[],
            Self::Protected(reasons) => reasons,
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ApplicationProtectionPolicy;

impl ApplicationProtectionPolicy {
    pub fn evaluate(self, application: &InstalledApplication) -> ApplicationProtection {
        let mut reasons = Vec::new();

        if application.location == ApplicationLocation::System {
            reasons.push(ApplicationProtectionReason::SystemLocation);
        }

        if let Some(root) = matching_system_application_root(&application.path) {
            reasons.push(ApplicationProtectionReason::SystemPath(root.to_path_buf()));
        }

        if let Some(bundle_identifier) = application.metadata.bundle_identifier.as_deref()
            && is_apple_bundle_identifier(bundle_identifier)
        {
            reasons.push(ApplicationProtectionReason::AppleBundleIdentifier(
                bundle_identifier.to_owned(),
            ));
        }

        if reasons.is_empty() {
            ApplicationProtection::Unprotected
        } else {
            ApplicationProtection::Protected(reasons)
        }
    }
}

const SYSTEM_APPLICATION_ROOTS: [&str; 3] = [
    "/System/Applications",
    "/System/Library/CoreServices/Applications",
    "/System/Library/CoreServices",
];

fn matching_system_application_root(path: &Path) -> Option<&Path> {
    SYSTEM_APPLICATION_ROOTS
        .iter()
        .map(Path::new)
        .find(|root| path.starts_with(root))
}

fn is_apple_bundle_identifier(bundle_identifier: &str) -> bool {
    bundle_identifier == "com.apple" || bundle_identifier.starts_with("com.apple.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ApplicationMetadata, InstalledApplication};

    fn application(path: &str, location: ApplicationLocation) -> InstalledApplication {
        InstalledApplication::new(PathBuf::from(path), "Example", location)
    }

    #[test]
    fn protects_system_location_even_outside_known_roots() {
        let app = application("/Applications/Fixture.app", ApplicationLocation::System);
        let protection = ApplicationProtectionPolicy.evaluate(&app);

        assert!(protection.is_protected());
        assert!(protection
            .reasons()
            .contains(&ApplicationProtectionReason::SystemLocation));
    }

    #[test]
    fn protects_known_system_application_roots_defensively() {
        let app = application(
            "/System/Applications/Utilities/Terminal.app",
            ApplicationLocation::Local,
        );
        let protection = ApplicationProtectionPolicy.evaluate(&app);

        assert!(protection.is_protected());
        assert!(protection.reasons().iter().any(|reason| matches!(
            reason,
            ApplicationProtectionReason::SystemPath(root)
                if root == Path::new("/System/Applications")
        )));
    }

    #[test]
    fn protects_apple_bundle_identifier_even_in_local_applications() {
        let app = application("/Applications/Safari.app", ApplicationLocation::Local).with_metadata(
            ApplicationMetadata {
                bundle_identifier: Some("com.apple.Safari".into()),
                ..ApplicationMetadata::default()
            },
        );
        let protection = ApplicationProtectionPolicy.evaluate(&app);

        assert!(protection.is_protected());
        assert!(protection.reasons().iter().any(|reason| matches!(
            reason,
            ApplicationProtectionReason::AppleBundleIdentifier(identifier)
                if identifier == "com.apple.Safari"
        )));
    }

    #[test]
    fn does_not_treat_lookalike_bundle_prefix_as_apple() {
        let app = application("/Applications/Fake.app", ApplicationLocation::Local).with_metadata(
            ApplicationMetadata {
                bundle_identifier: Some("com.appleish.fake".into()),
                ..ApplicationMetadata::default()
            },
        );

        assert_eq!(
            ApplicationProtectionPolicy.evaluate(&app),
            ApplicationProtection::Unprotected
        );
    }

    #[test]
    fn ordinary_local_application_remains_unprotected() {
        let app = application("/Applications/Example.app", ApplicationLocation::Local).with_metadata(
            ApplicationMetadata {
                bundle_identifier: Some("com.example.app".into()),
                ..ApplicationMetadata::default()
            },
        );

        assert_eq!(
            ApplicationProtectionPolicy.evaluate(&app),
            ApplicationProtection::Unprotected
        );
    }
}
