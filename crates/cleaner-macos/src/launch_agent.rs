use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

pub const DEFAULT_LAUNCH_AGENT_LABEL: &str = "com.cnxdev.dxtr-cleaner.smart-scan";
pub const MIN_START_INTERVAL_SECONDS: u64 = 15 * 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchAgentConfig {
    pub label: String,
    pub executable: PathBuf,
    pub start_interval_seconds: u64,
    pub stdout_path: PathBuf,
    pub stderr_path: PathBuf,
}

impl LaunchAgentConfig {
    pub fn smart_scan(home: &Path, executable: PathBuf, start_interval_seconds: u64) -> Self {
        let log_dir = home.join("Library/Logs/DxtrCleaner");
        Self {
            label: DEFAULT_LAUNCH_AGENT_LABEL.into(),
            executable,
            start_interval_seconds,
            stdout_path: log_dir.join("scheduled-scan.log"),
            stderr_path: log_dir.join("scheduled-scan.error.log"),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.label.is_empty() {
            return Err("LaunchAgent label must not be empty".into());
        }
        if !self.executable.is_absolute() {
            return Err("scheduled cleaner executable path must be absolute".into());
        }
        if self.start_interval_seconds < MIN_START_INTERVAL_SECONDS {
            return Err(format!(
                "scheduled scan interval must be at least {MIN_START_INTERVAL_SECONDS} seconds"
            ));
        }
        if !self.stdout_path.is_absolute() || !self.stderr_path.is_absolute() {
            return Err("LaunchAgent log paths must be absolute".into());
        }
        Ok(())
    }

    pub fn plist_path(&self, home: &Path) -> PathBuf {
        home.join("Library/LaunchAgents")
            .join(format!("{}.plist", self.label))
    }
}

pub fn render_launch_agent_plist(config: &LaunchAgentConfig) -> Result<String, String> {
    config.validate()?;
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{executable}</string>
        <string>scan</string>
        <string>--category</string>
        <string>user</string>
    </array>
    <key>StartInterval</key>
    <integer>{interval}</integer>
    <key>RunAtLoad</key>
    <false/>
    <key>StandardOutPath</key>
    <string>{stdout}</string>
    <key>StandardErrorPath</key>
    <string>{stderr}</string>
</dict>
</plist>
"#,
        label = xml_escape(&config.label),
        executable = xml_escape(&config.executable.to_string_lossy()),
        interval = config.start_interval_seconds,
        stdout = xml_escape(&config.stdout_path.to_string_lossy()),
        stderr = xml_escape(&config.stderr_path.to_string_lossy()),
    ))
}

#[cfg(target_os = "macos")]
pub fn install_launch_agent(home: &Path, config: &LaunchAgentConfig) -> Result<PathBuf, String> {
    config.validate()?;
    let plist = render_launch_agent_plist(config)?;
    let plist_path = config.plist_path(home);
    let launch_agents = plist_path
        .parent()
        .ok_or_else(|| "LaunchAgent path has no parent directory".to_string())?;
    fs::create_dir_all(launch_agents).map_err(|error| error.to_string())?;
    if let Some(parent) = config.stdout_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let previous_plist = fs::read(&plist_path).ok();
    let temporary_path = plist_path.with_extension("plist.tmp");
    fs::write(&temporary_path, plist).map_err(|error| error.to_string())?;
    fs::rename(&temporary_path, &plist_path).map_err(|error| error.to_string())?;

    let domain = gui_domain(home)?;
    let _ = Command::new("launchctl")
        .args(["bootout", &domain])
        .arg(&plist_path)
        .status();

    let status = Command::new("launchctl")
        .args(["bootstrap", &domain])
        .arg(&plist_path)
        .status()
        .map_err(|error| {
            let rollback = rollback_plist(&plist_path, previous_plist.as_deref());
            format_bootstrap_error(error.to_string(), rollback)
        })?;
    if !status.success() {
        let rollback = rollback_plist(&plist_path, previous_plist.as_deref());
        return Err(format_bootstrap_error(
            format!("launchctl bootstrap failed with status {status}"),
            rollback,
        ));
    }

    Ok(plist_path)
}

#[cfg(target_os = "macos")]
fn rollback_plist(plist_path: &Path, previous_plist: Option<&[u8]>) -> Result<(), String> {
    match previous_plist {
        Some(previous) => fs::write(plist_path, previous).map_err(|error| error.to_string()),
        None => match fs::remove_file(plist_path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
        },
    }
}

#[cfg(target_os = "macos")]
fn format_bootstrap_error(message: String, rollback: Result<(), String>) -> String {
    match rollback {
        Ok(()) => message,
        Err(rollback_error) => format!("{message}; additionally failed to roll back plist: {rollback_error}"),
    }
}

#[cfg(not(target_os = "macos"))]
pub fn install_launch_agent(home: &Path, config: &LaunchAgentConfig) -> Result<PathBuf, String> {
    let _ = (home, config);
    Err("LaunchAgent scheduling is available only on macOS".into())
}

#[cfg(target_os = "macos")]
pub fn uninstall_launch_agent(home: &Path, label: &str) -> Result<(), String> {
    if label.is_empty() {
        return Err("LaunchAgent label must not be empty".into());
    }
    let plist_path = home
        .join("Library/LaunchAgents")
        .join(format!("{label}.plist"));
    let domain = gui_domain(home)?;

    if plist_path.exists() {
        let status = Command::new("launchctl")
            .args(["bootout", &domain])
            .arg(&plist_path)
            .status()
            .map_err(|error| error.to_string())?;
        if !status.success() {
            return Err(format!("launchctl bootout failed with status {status}"));
        }
        fs::remove_file(&plist_path).map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn uninstall_launch_agent(home: &Path, label: &str) -> Result<(), String> {
    let _ = (home, label);
    Err("LaunchAgent scheduling is available only on macOS".into())
}

#[cfg(target_os = "macos")]
fn gui_domain(home: &Path) -> Result<String, String> {
    use std::os::unix::fs::MetadataExt;

    let uid = fs::metadata(home).map_err(|error| error.to_string())?.uid();
    Ok(format!("gui/{uid}"))
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smart_scan_uses_user_launch_agent_and_log_locations() {
        let home = Path::new("/Users/example");
        let config = LaunchAgentConfig::smart_scan(
            home,
            PathBuf::from("/Applications/Dxtr Cleaner.app/Contents/MacOS/dxtr-cleaner"),
            3600,
        );

        assert_eq!(
            config.plist_path(home),
            Path::new(
                "/Users/example/Library/LaunchAgents/com.cnxdev.dxtr-cleaner.smart-scan.plist"
            )
        );
        assert_eq!(
            config.stdout_path,
            Path::new("/Users/example/Library/Logs/DxtrCleaner/scheduled-scan.log")
        );
    }

    #[test]
    fn rejects_relative_executable_and_too_frequent_schedule() {
        let home = Path::new("/Users/example");
        let relative = LaunchAgentConfig::smart_scan(home, PathBuf::from("dxtr-cleaner"), 3600);
        assert_eq!(
            relative.validate().unwrap_err(),
            "scheduled cleaner executable path must be absolute"
        );

        let frequent = LaunchAgentConfig::smart_scan(
            home,
            PathBuf::from("/usr/local/bin/dxtr-cleaner"),
            MIN_START_INTERVAL_SECONDS - 1,
        );
        assert!(frequent.validate().unwrap_err().contains("at least"));
    }

    #[test]
    fn rendered_plist_is_read_only_scan_and_xml_escapes_paths() {
        let home = Path::new("/Users/example");
        let config = LaunchAgentConfig::smart_scan(
            home,
            PathBuf::from("/Applications/Dxtr & Cleaner.app/Contents/MacOS/dxtr-cleaner"),
            3600,
        );
        let plist = render_launch_agent_plist(&config).expect("valid config must render");

        assert!(plist.contains("<string>scan</string>"));
        assert!(plist.contains("<string>--category</string>"));
        assert!(plist.contains("<string>user</string>"));
        assert!(!plist.contains("trash"));
        assert!(!plist.contains("delete"));
        assert!(plist.contains("Dxtr &amp; Cleaner.app"));
        assert!(plist.contains("<integer>3600</integer>"));
    }
}
