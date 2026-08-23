use std::{env, path::PathBuf, process::ExitCode};

use cleaner_core::{
    CategoryScanTarget, CleanupCategory, FileSystemScanner, HomebrewScan, NodeScan, Planner,
    ScanSummary, Scanner, UserCacheScan, XcodeScan,
};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some("scan") {
        print_usage();
        return ExitCode::SUCCESS;
    }

    let category = match parse_category(&args) {
        Ok(category) => category,
        Err(error) => {
            eprintln!("{error}");
            print_usage();
            return ExitCode::FAILURE;
        }
    };

    let request = match scan_request(category) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    let scanner = FileSystemScanner;

    match scanner.scan(&request) {
        Ok(items) => {
            let summary = ScanSummary::from_items(&items);
            let plan = Planner::build(items);
            println!("category: {}", category.label());
            println!("items: {}", summary.item_count);
            println!("bytes: {}", summary.total_bytes);
            println!("selected bytes: {}", plan.selected_bytes());
            println!("mode: dry-run (M1 destructive execution disabled)");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("scan failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn parse_category(args: &[String]) -> Result<CleanupCategory, String> {
    let Some(index) = args.iter().position(|arg| arg == "--category") else {
        return Ok(CleanupCategory::UserCache);
    };

    let value = args
        .get(index + 1)
        .ok_or_else(|| "--category requires a value".to_string())?;

    match value.as_str() {
        "user" | "cache" => Ok(CleanupCategory::UserCache),
        "system" => Ok(CleanupCategory::SystemCache),
        "dev" | "xcode" => Ok(CleanupCategory::Xcode),
        "brew" | "homebrew" => Ok(CleanupCategory::Homebrew),
        "node" => Ok(CleanupCategory::Node),
        "docker" => Ok(CleanupCategory::Docker),
        "large" => Ok(CleanupCategory::LargeFiles),
        _ => Err(format!("unknown category: {value}")),
    }
}

fn scan_request(category: CleanupCategory) -> Result<cleaner_core::ScanRequest, String> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is not set".to_string())?;

    match category {
        CleanupCategory::UserCache => Ok(UserCacheScan::new(home).request()),
        CleanupCategory::Xcode => Ok(XcodeScan::new(home).request()),
        CleanupCategory::Homebrew => Ok(HomebrewScan::new(home).request()),
        CleanupCategory::Node => Ok(NodeScan::new(home).request()),
        CleanupCategory::SystemCache | CleanupCategory::Docker | CleanupCategory::LargeFiles => {
            Err(format!(
                "category '{}' is not implemented in M1 yet",
                category.label()
            ))
        }
    }
}

fn print_usage() {
    println!("dxtr-cleaner scan [--category user|dev|brew|node]");
}
