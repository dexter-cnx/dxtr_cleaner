use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsPaths {
    pub user_profile: PathBuf,
    pub local_app_data: PathBuf,
    pub program_data: PathBuf,
    pub system_root: PathBuf,
    pub temp: PathBuf,
}

impl WindowsPaths {
    #[cfg(windows)]
    pub fn discover() -> Result<Self, String> {
        Self::from_lookup_and_temp(
            |key| std::env::var_os(key).map(PathBuf::from),
            std::env::temp_dir(),
        )
    }

    #[cfg(not(windows))]
    pub fn discover() -> Result<Self, String> {
        Err("Windows path discovery is unavailable on this platform".into())
    }

    #[cfg(windows)]
    fn from_lookup_and_temp(
        mut lookup: impl FnMut(&str) -> Option<PathBuf>,
        temp: PathBuf,
    ) -> Result<Self, String> {
        Ok(Self {
            user_profile: required_absolute(&mut lookup, "USERPROFILE")?,
            local_app_data: required_absolute(&mut lookup, "LOCALAPPDATA")?,
            program_data: required_absolute(&mut lookup, "PROGRAMDATA")?,
            system_root: required_absolute(&mut lookup, "SystemRoot")?,
            temp: validate_absolute(temp, "Windows temporary directory")?,
        })
    }
}

#[cfg(windows)]
fn required_absolute(
    lookup: &mut impl FnMut(&str) -> Option<PathBuf>,
    key: &str,
) -> Result<PathBuf, String> {
    let path =
        lookup(key).ok_or_else(|| format!("missing required Windows path variable: {key}"))?;
    validate_absolute(path, &format!("Windows path variable {key}"))
}

#[cfg(windows)]
fn validate_absolute(path: PathBuf, label: &str) -> Result<PathBuf, String> {
    if path.as_os_str().is_empty() {
        return Err(format!("empty {label}"));
    }
    if !path.is_absolute() {
        return Err(format!("{label} must be absolute: {}", path.display()));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn discovery_fails_closed_off_windows() {
        assert!(WindowsPaths::discover().is_err());
    }

    #[cfg(windows)]
    #[test]
    fn discovers_required_paths_on_windows_runner() {
        let paths = WindowsPaths::discover().expect("discover Windows path snapshot");
        assert!(paths.user_profile.is_absolute());
        assert!(paths.local_app_data.is_absolute());
        assert!(paths.program_data.is_absolute());
        assert!(paths.system_root.is_absolute());
        assert!(paths.temp.is_absolute());
    }

    #[cfg(windows)]
    #[test]
    fn rejects_missing_or_relative_required_paths() {
        let missing = WindowsPaths::from_lookup_and_temp(
            |key| match key {
                "USERPROFILE" => Some(PathBuf::from(r"C:\\Users\\tester")),
                _ => None,
            },
            PathBuf::from(r"C:\\Temp"),
        )
        .expect_err("missing required paths must fail");
        assert!(missing.contains("LOCALAPPDATA"));

        let relative = WindowsPaths::from_lookup_and_temp(
            |key| {
                Some(match key {
                    "USERPROFILE" => PathBuf::from(r"C:\\Users\\tester"),
                    "LOCALAPPDATA" => PathBuf::from(r"C:\\Users\\tester\\AppData\\Local"),
                    "PROGRAMDATA" => PathBuf::from(r"C:\\ProgramData"),
                    "SystemRoot" => PathBuf::from(r"C:\\Windows"),
                    _ => unreachable!("unexpected key"),
                })
            },
            PathBuf::from(r"relative\\temp"),
        )
        .expect_err("relative temp path must fail");
        assert!(relative.contains("temporary directory"));
    }
}
