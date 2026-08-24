use std::{env, path::PathBuf, process::ExitCode};

use cleaner_macos::launch_agent::{
    DEFAULT_LAUNCH_AGENT_LABEL, LaunchAgentConfig, install_launch_agent, render_launch_agent_plist,
    uninstall_launch_agent,
};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let Some(command) = args.first().map(String::as_str) else {
        print_usage();
        return ExitCode::FAILURE;
    };

    let Some(home) = env::var_os("HOME").map(PathBuf::from) else {
        eprintln!("HOME is not set");
        return ExitCode::FAILURE;
    };

    match command {
        "print" | "install" => {
            let executable = match required_path_arg(&args, "--executable") {
                Ok(path) => path,
                Err(error) => {
                    eprintln!("{error}");
                    print_usage();
                    return ExitCode::FAILURE;
                }
            };
            let interval = match interval_arg(&args) {
                Ok(interval) => interval,
                Err(error) => {
                    eprintln!("{error}");
                    return ExitCode::FAILURE;
                }
            };
            let config = LaunchAgentConfig::smart_scan(&home, executable, interval);

            if command == "print" {
                match render_launch_agent_plist(&config) {
                    Ok(plist) => {
                        print!("{plist}");
                        ExitCode::SUCCESS
                    }
                    Err(error) => {
                        eprintln!("invalid LaunchAgent configuration: {error}");
                        ExitCode::FAILURE
                    }
                }
            } else {
                match install_launch_agent(&home, &config) {
                    Ok(path) => {
                        println!("installed: {}", path.display());
                        println!("mode: scheduled read-only Smart Scan; no cleanup mutation");
                        ExitCode::SUCCESS
                    }
                    Err(error) => {
                        eprintln!("failed to install LaunchAgent: {error}");
                        ExitCode::FAILURE
                    }
                }
            }
        }
        "uninstall" => match uninstall_launch_agent(&home, DEFAULT_LAUNCH_AGENT_LABEL) {
            Ok(()) => {
                println!("LaunchAgent removed");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("failed to remove LaunchAgent: {error}");
                ExitCode::FAILURE
            }
        },
        _ => {
            print_usage();
            ExitCode::FAILURE
        }
    }
}

fn required_path_arg(args: &[String], name: &str) -> Result<PathBuf, String> {
    let Some(index) = args.iter().position(|arg| arg == name) else {
        return Err(format!("{name} is required"));
    };
    args.get(index + 1)
        .map(PathBuf::from)
        .ok_or_else(|| format!("{name} requires a path"))
}

fn interval_arg(args: &[String]) -> Result<u64, String> {
    let Some(index) = args.iter().position(|arg| arg == "--interval") else {
        return Ok(24 * 60 * 60);
    };
    let value = args
        .get(index + 1)
        .ok_or_else(|| "--interval requires seconds".to_string())?;
    value
        .parse::<u64>()
        .map_err(|_| "--interval must be an integer number of seconds".to_string())
}

fn print_usage() {
    println!(
        "dxtr-cleaner-launch-agent print --executable /absolute/path/to/dxtr-cleaner [--interval seconds]"
    );
    println!(
        "dxtr-cleaner-launch-agent install --executable /absolute/path/to/dxtr-cleaner [--interval seconds]"
    );
    println!("dxtr-cleaner-launch-agent uninstall");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_defaults_to_one_day() {
        assert_eq!(interval_arg(&["install".into()]).unwrap(), 86_400);
    }

    #[test]
    fn interval_rejects_non_integer_input() {
        let args = vec!["install".into(), "--interval".into(), "hourly".into()];
        assert_eq!(
            interval_arg(&args).unwrap_err(),
            "--interval must be an integer number of seconds"
        );
    }

    #[test]
    fn executable_argument_requires_a_following_path() {
        let args = vec!["install".into(), "--executable".into()];
        assert_eq!(
            required_path_arg(&args, "--executable").unwrap_err(),
            "--executable requires a path"
        );
    }
}
