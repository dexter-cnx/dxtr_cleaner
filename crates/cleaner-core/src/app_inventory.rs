use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ApplicationLocation {
    User,
    Local,
    System,
}

impl ApplicationLocation {
    pub fn label(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Local => "local",
            Self::System => "system",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledApplication {
    pub path: PathBuf,
    pub name: String,
    pub location: ApplicationLocation,
}

impl InstalledApplication {
    pub fn new(path: PathBuf, name: impl Into<String>, location: ApplicationLocation) -> Self {
        Self {
            path,
            name: name.into(),
            location,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationInventoryIssue {
    pub path: PathBuf,
    pub message: String,
}

impl ApplicationInventoryIssue {
    pub fn new(path: PathBuf, message: impl Into<String>) -> Self {
        Self {
            path,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplicationInventoryReport {
    pub applications: Vec<InstalledApplication>,
    pub issues: Vec<ApplicationInventoryIssue>,
}

impl ApplicationInventoryReport {
    pub fn sort_deterministically(&mut self) {
        self.applications.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| left.location.cmp(&right.location))
                .then_with(|| left.name.cmp(&right.name))
        });
        self.issues.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| left.message.cmp(&right.message))
        });
    }
}

pub trait ApplicationInventory {
    fn inventory(&self) -> ApplicationInventoryReport;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_sort_is_deterministic() {
        let mut report = ApplicationInventoryReport {
            applications: vec![
                InstalledApplication::new(
                    PathBuf::from("/Applications/Zeta.app"),
                    "Zeta",
                    ApplicationLocation::Local,
                ),
                InstalledApplication::new(
                    PathBuf::from("/Applications/Alpha.app"),
                    "Alpha",
                    ApplicationLocation::Local,
                ),
            ],
            issues: vec![
                ApplicationInventoryIssue::new(PathBuf::from("/z"), "z"),
                ApplicationInventoryIssue::new(PathBuf::from("/a"), "a"),
            ],
        };

        report.sort_deterministically();

        assert_eq!(report.applications[0].name, "Alpha");
        assert_eq!(report.issues[0].path, PathBuf::from("/a"));
    }
}
