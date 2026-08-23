use std::path::PathBuf;

use crate::{
    ApplicationProtection, ApplicationProtectionPolicy, InstalledApplication, MatchConfidence,
    RelatedFileCandidate, RelatedFileKind, RelatedFileReport,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UninstallPlanItemKind {
    ApplicationBundle,
    RelatedFile(RelatedFileKind),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UninstallPlanItem {
    pub path: PathBuf,
    pub kind: UninstallPlanItemKind,
    pub confidence: MatchConfidence,
    pub selected: bool,
    pub selectable: bool,
    pub required: bool,
    pub review_only: bool,
}

impl UninstallPlanItem {
    fn application(path: PathBuf, protected: bool) -> Self {
        Self {
            path,
            kind: UninstallPlanItemKind::ApplicationBundle,
            confidence: MatchConfidence::High,
            selected: !protected,
            selectable: false,
            required: !protected,
            review_only: false,
        }
    }

    fn related(candidate: RelatedFileCandidate, protected: bool) -> Self {
        let review_only = candidate.confidence.is_review_only();
        Self {
            path: candidate.path,
            kind: UninstallPlanItemKind::RelatedFile(candidate.kind),
            confidence: candidate.confidence,
            selected: !protected && !review_only,
            selectable: !protected,
            required: false,
            review_only,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UninstallPlan {
    pub application: InstalledApplication,
    pub protection: ApplicationProtection,
    pub items: Vec<UninstallPlanItem>,
}

impl UninstallPlan {
    pub fn build(application: InstalledApplication, related: RelatedFileReport) -> Self {
        let protection = ApplicationProtectionPolicy.evaluate(&application);
        let protected = protection.is_protected();
        let mut items = Vec::with_capacity(1 + related.candidates.len());
        items.push(UninstallPlanItem::application(
            application.path.clone(),
            protected,
        ));
        items.extend(
            related
                .candidates
                .into_iter()
                .map(|candidate| UninstallPlanItem::related(candidate, protected)),
        );
        items.sort_by(|left, right| {
            matches!(right.kind, UninstallPlanItemKind::ApplicationBundle)
                .cmp(&matches!(
                    left.kind,
                    UninstallPlanItemKind::ApplicationBundle
                ))
                .then_with(|| left.path.cmp(&right.path))
        });

        Self {
            application,
            protection,
            items,
        }
    }

    pub fn is_protected(&self) -> bool {
        self.protection.is_protected()
    }

    pub fn selected_count(&self) -> usize {
        self.items.iter().filter(|item| item.selected).count()
    }

    pub fn set_selected(&mut self, path: &std::path::Path, selected: bool) -> bool {
        let Some(item) = self.items.iter_mut().find(|item| item.path == path) else {
            return false;
        };
        if !item.selectable || item.required {
            return false;
        }
        item.selected = selected;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ApplicationLocation, ApplicationMetadata, MatchEvidence, RelatedFileCandidate};
    use std::path::Path;

    fn app(bundle_identifier: &str) -> InstalledApplication {
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

    fn related(confidence: MatchConfidence, path: &str) -> RelatedFileCandidate {
        RelatedFileCandidate::new(
            PathBuf::from(path),
            RelatedFileKind::Cache,
            confidence,
            MatchEvidence::ExactBundleIdentifier("com.example.app".into()),
        )
    }

    #[test]
    fn unprotected_plan_selects_required_app_and_high_confidence_related_by_default() {
        let mut plan = UninstallPlan::build(
            app("com.example.app"),
            RelatedFileReport {
                candidates: vec![
                    related(MatchConfidence::High, "/tmp/high"),
                    related(MatchConfidence::Medium, "/tmp/medium"),
                    related(MatchConfidence::Low, "/tmp/low"),
                ],
            },
        );

        assert!(!plan.is_protected());
        assert_eq!(plan.selected_count(), 2);
        assert!(plan.items.iter().any(|item| {
            item.path.as_path() == Path::new("/Applications/Example.app")
                && item.selected
                && item.required
                && !item.selectable
        }));
        assert!(!plan.set_selected(Path::new("/Applications/Example.app"), false));
        assert!(plan.items.iter().any(|item| {
            item.path.as_path() == Path::new("/tmp/high") && item.selected && !item.review_only
        }));
        assert!(plan.items.iter().any(|item| {
            item.path.as_path() == Path::new("/tmp/medium") && !item.selected && item.review_only
        }));
        assert!(plan.items.iter().any(|item| {
            item.path.as_path() == Path::new("/tmp/low") && !item.selected && item.review_only
        }));
    }

    #[test]
    fn protected_plan_locks_every_item() {
        let mut plan = UninstallPlan::build(
            app("com.apple.Safari"),
            RelatedFileReport {
                candidates: vec![related(MatchConfidence::High, "/tmp/high")],
            },
        );

        assert!(plan.is_protected());
        assert_eq!(plan.selected_count(), 0);
        assert!(
            plan.items
                .iter()
                .all(|item| !item.selected && !item.selectable && !item.required)
        );
        assert!(!plan.set_selected(Path::new("/tmp/high"), true));
    }

    #[test]
    fn review_only_item_can_be_explicitly_selected_on_unprotected_plan() {
        let mut plan = UninstallPlan::build(
            app("com.example.app"),
            RelatedFileReport {
                candidates: vec![related(MatchConfidence::Medium, "/tmp/medium")],
            },
        );

        assert!(plan.set_selected(Path::new("/tmp/medium"), true));
        assert!(plan.items.iter().any(|item| {
            item.path.as_path() == Path::new("/tmp/medium") && item.selected && item.review_only
        }));
    }
}
