use std::path::PathBuf;

use crate::InstalledApplication;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatchConfidence {
    Low,
    Medium,
    High,
}

impl MatchConfidence {
    pub fn label(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    pub fn is_review_only(self) -> bool {
        self != Self::High
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RelatedFileKind {
    ApplicationSupport,
    Cache,
    Container,
    HttpStorage,
    Preference,
    SavedState,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatchEvidence {
    ExactBundleIdentifier(String),
    ExactDisplayName(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedFileCandidate {
    pub path: PathBuf,
    pub kind: RelatedFileKind,
    pub confidence: MatchConfidence,
    pub evidence: MatchEvidence,
}

impl RelatedFileCandidate {
    pub fn new(
        path: PathBuf,
        kind: RelatedFileKind,
        confidence: MatchConfidence,
        evidence: MatchEvidence,
    ) -> Self {
        Self {
            path,
            kind,
            confidence,
            evidence,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RelatedFileReport {
    pub candidates: Vec<RelatedFileCandidate>,
}

impl RelatedFileReport {
    pub fn sort_deterministically(&mut self) {
        self.candidates.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| right.confidence.cmp(&left.confidence))
                .then_with(|| left.kind.cmp(&right.kind))
                .then_with(|| left.evidence.cmp(&right.evidence))
        });
        self.candidates.dedup_by(|left, right| left.path == right.path);
    }
}

pub trait RelatedFileMatcher {
    fn related_files(&self, application: &InstalledApplication) -> RelatedFileReport;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_high_confidence_candidates_are_not_review_only() {
        assert!(!MatchConfidence::High.is_review_only());
        assert!(MatchConfidence::Medium.is_review_only());
        assert!(MatchConfidence::Low.is_review_only());
    }

    #[test]
    fn report_sort_deduplicates_paths_and_keeps_strongest_evidence() {
        let path = PathBuf::from("/tmp/com.example.app");
        let mut report = RelatedFileReport {
            candidates: vec![
                RelatedFileCandidate::new(
                    path.clone(),
                    RelatedFileKind::Cache,
                    MatchConfidence::Low,
                    MatchEvidence::ExactDisplayName("Example".into()),
                ),
                RelatedFileCandidate::new(
                    path.clone(),
                    RelatedFileKind::Cache,
                    MatchConfidence::High,
                    MatchEvidence::ExactBundleIdentifier("com.example.app".into()),
                ),
            ],
        };

        report.sort_deterministically();

        assert_eq!(report.candidates.len(), 1);
        assert_eq!(report.candidates[0].confidence, MatchConfidence::High);
    }
}
