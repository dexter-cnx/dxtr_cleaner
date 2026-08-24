use std::{
    env,
    path::PathBuf,
    sync::mpsc::{self, Receiver},
    thread,
};

use cleaner_macos::launch_agent::{LaunchAgentCoordinator, LaunchAgentCoordinatorStatus};

pub enum SchedulingMessage {
    Loaded(Result<LaunchAgentCoordinatorStatus, String>),
    Updated(Result<LaunchAgentCoordinatorStatus, String>),
}

pub fn spawn_status() -> Receiver<SchedulingMessage> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let result = coordinator().and_then(|coordinator| coordinator.status());
        let _ = tx.send(SchedulingMessage::Loaded(result));
    });
    rx
}

pub fn spawn_set_enabled(enabled: bool) -> Receiver<SchedulingMessage> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let result = coordinator().and_then(|coordinator| {
            if enabled {
                coordinator.enable_daily()?;
            } else {
                coordinator.disable()?;
            }
            coordinator.status()
        });
        let _ = tx.send(SchedulingMessage::Updated(result));
    });
    rx
}

fn coordinator() -> Result<LaunchAgentCoordinator, String> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is not set".to_string())?;
    let current_executable = env::current_exe()
        .map_err(|error| format!("current executable path is unavailable: {error}"))?;
    LaunchAgentCoordinator::for_current_process(home, current_executable)
}
