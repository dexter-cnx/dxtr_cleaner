use std::{env, path::PathBuf, process::ExitCode};

use cleaner_core::{
    CleanupCategory, FileSystemScanner, Planner, ScanRequest, ScanSummary, Scanner,
};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some("scan") {
        print_usage();
        return ExitCode::SUCCESS;
    }

    let category = parse_category(&args).unwrap_or(CleanupCategory::UserCache);
    let roots = default_roots(category);
    let scanner = FileSystemScanner;

    match scanner.scan(&ScanRequest { category, roots }) {
        Ok(items) => {
            let summary = ScanSummary::from_items(&items);
            let plan = Planner::build(items);
            println!("category: {}", category.label());
            println!("items: {}", summary.item_count);
            println!("bytes: {}", summary.total_bytes);
            println!("selected bytes: {}", plan.selected_bytes());
            println!("mode: dry-run (M0 destructive execution disabled)");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("scan failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn parse_category(args: &[String]) -> Option<CleanupCategory> {
    let value = args
        .windows(2)
        .find(|pair| pair[0] == "--category")
        .map(|pair| pair[1].as_str())?;

    match value {
        "user" | "cache" => Some(CleanupCategory::UserCache),
        "system" => Some(CleanupCategory::SystemCache),
        "dev" | "xcode" => Some(CleanupCategory::Xcode),
        "brew" | "homebrew" => Some(CleanupCategory::Homebrew),
        "node" => Some(CleanupCategory::Node),
        "docker" => Some(CleanupCategory::Docker),
        "large" => Some(CleanupCategory::LargeFiles),
        _ => None,
    }
}

fn default_roots(category: CleanupCategory) -> Vec<PathBuf> {
    let home = env::var_os("HOME").map(PathBuf::from);
    match (category, home) {
        (CleanupCategory::UserCache, Some(home)) => vec![home.join("Library/Caches")],
        (CleanupCategory::Xcode, Some(home)) => {
            vec![home.join("Library/Developer/Xcode/DerivedData")]
        }
        (CleanupCategory::Homebrew, Some(home)) => vec![home.join("Library/Caches/Homebrew")],
        (CleanupCategory::Node, Some(home)) => {
            vec![home.join(".npm"), home.join("Library/pnpm/store")]
        }
        _ => Vec::new(),
    }
}

fn print_usage() {
    println!("dxtr-cleaner scan [--category user|system|dev|brew|node|docker|large]");
}
