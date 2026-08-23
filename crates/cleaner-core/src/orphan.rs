use std::path::PathBuf;

use crate::{InstalledApplication, MatchConfidence, RelatedFileKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrphanCandidate {
    pub bundle_identifier: String,
    pub path: PathBuf,
    pub kind: RelatedFileKind,
    pub confidence: MatchConfidence,
}

impl OrphanCandidate {
    pub fn new(
        bundle_identifier: impl Into<String>,
        path: PathBuf,
        kind: RelatedFileKind,
        confidence: MatchConfidence,
    ) -> Self {
        Self {
            bundle_identifier: bundle_identifier.into(),
            path,
            kind,
            confidence,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrphanFinderIssue {
    pub path: PathBuf,
    pub message: String,
}

impl OrphanFinderIssue {
    pub fn new(path: PathBuf, message: impl Into<String>) -> Self {
        Self {
            path,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OrphanReport {
    pub candidates: Vec<OrphanCandidate>,
    pub issues: Vec<OrphanFinderIssue>,
}

impl OrphanReport {
    pub fn sort_deterministically(&mut self) {
        self.candidates.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| right.confidence.cmp(&left.confidence))
                .then_with(|| left.bundle_identifier.cmp(&right.bundle_identifier))
                .then_with(|| left.kind.cmp(&right.kind))
        });
        self.candidates
            .dedup_by(|left, right| left.path == right.path);
        self.issues.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| left.message.cmp(&right.message))
        });
    }
}

pub trait OrphanFinder {
    fn find_orphans(&self, installed: &[InstalledApplication]) -> OrphanReport;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_sort_is_deterministic_and_keeps_strongest_duplicate() {
        let path = PathBuf::from("/tmp/com.example.app");
        let mut report = OrphanReport {
            candidates: vec![
                OrphanCandidate::new(
                    "z.example.app",
                    path.clone(),
                    RelatedFileKind::Preference,
                    MatchConfidence::Medium,
                ),
                OrphanCandidate::new(
                    "a.example.app",
                    path,
                    RelatedFileKind::Cache,
                    MatchConfidence::High,
                ),
            ],
            issues: Vec::new(),
        };

        report.sort_deterministically();

        assert_eq!(report.candidates.len(), 1);
        assert_eq!(report.candidates[0].confidence, MatchConfidence::High);
        assert_eq!(report.candidates[0].bundle_identifier, "a.example.app");
    }
}
