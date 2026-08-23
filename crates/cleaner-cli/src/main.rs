use std::{env, path::PathBuf, process::ExitCode};

use cleaner_core::{
    ApplicationInventory, ApplicationProtectionPolicy, CategoryScanTarget, CleanupCategory,
    FileSystemScanner, HomebrewScan, NodeScan, OrphanFinder, Planner, RelatedFileMatcher,
    ScanSummary, Scanner, SystemCacheScan, UninstallPlan, UninstallPlanItemKind, UserCacheScan,
    XcodeScan,
};
use cleaner_macos::SystemMacPlatform;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("scan") => run_scan(&args),
        Some("apps") => run_app_inventory(),
        Some("related") => run_related_files(&args),
        Some("orphans") => run_orphan_finder(),
        Some("uninstall-plan") => run_uninstall_plan(&args),
        _ => {
            print_usage();
            ExitCode::SUCCESS
        }
    }
}

fn run_scan(args: &[String]) -> ExitCode {
    let category = match parse_category(args) {
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
            println!("mode: dry-run");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("scan failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_app_inventory() -> ExitCode {
    let report = SystemMacPlatform.inventory();
    let protection_policy = ApplicationProtectionPolicy;

    for application in &report.applications {
        let protection = protection_policy.evaluate(application);
        let protection_reasons = protection
            .reasons()
            .iter()
            .map(|reason| reason.label())
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "{}\t{}\t{}\tbundle={}\tversion={}\tbuild={}\tteam={}\tprotected={}\treasons={}",
            application.location.label(),
            application.name,
            application.path.display(),
            application
                .metadata
                .bundle_identifier
                .as_deref()
                .unwrap_or("-"),
            application.metadata.short_version.as_deref().unwrap_or("-"),
            application
                .metadata
                .bundle_version
                .as_deref()
                .unwrap_or("-"),
            application
                .metadata
                .team_identifier
                .as_deref()
                .unwrap_or("-"),
            protection.is_protected(),
            if protection_reasons.is_empty() {
                "-"
            } else {
                &protection_reasons
            }
        );
    }
    for issue in &report.issues {
        eprintln!("warning: {}: {}", issue.path.display(), issue.message);
    }

    println!("applications: {}", report.applications.len());
    println!("warnings: {}", report.issues.len());
    ExitCode::SUCCESS
}

fn run_related_files(args: &[String]) -> ExitCode {
    let Some(bundle_identifier) = args.get(1) else {
        eprintln!("related requires a bundle identifier");
        print_usage();
        return ExitCode::FAILURE;
    };

    let platform = SystemMacPlatform;
    let inventory = platform.inventory();
    let matches: Vec<_> = inventory
        .applications
        .iter()
        .filter(|application| {
            application.metadata.bundle_identifier.as_deref() == Some(bundle_identifier.as_str())
        })
        .collect();

    if matches.is_empty() {
        eprintln!("no installed application found for bundle identifier: {bundle_identifier}");
        return ExitCode::FAILURE;
    }

    for application in matches {
        println!(
            "application\t{}\t{}",
            application.name,
            application.path.display()
        );
        let report = platform.related_files(application);
        for candidate in report.candidates {
            println!(
                "{}\t{}\t{}\treview_only={}",
                candidate.confidence.label(),
                candidate.kind.label(),
                candidate.path.display(),
                candidate.confidence.is_review_only()
            );
        }
    }

    ExitCode::SUCCESS
}

fn run_orphan_finder() -> ExitCode {
    let platform = SystemMacPlatform;
    let inventory = platform.inventory();
    let report = platform.find_orphans(&inventory);

    for candidate in &report.candidates {
        println!(
            "{}\t{}\t{}\t{}\treview_only={}",
            candidate.confidence.label(),
            candidate.kind.label(),
            candidate.bundle_identifier,
            candidate.path.display(),
            candidate.confidence.is_review_only()
        );
    }
    for issue in &inventory.issues {
        eprintln!(
            "inventory warning: {}: {}",
            issue.path.display(),
            issue.message
        );
    }
    for issue in &report.issues {
        eprintln!(
            "orphan warning: {}: {}",
            issue.path.display(),
            issue.message
        );
    }

    println!("orphans: {}", report.candidates.len());
    println!("inventory warnings: {}", inventory.issues.len());
    println!("orphan warnings: {}", report.issues.len());

    if report.issues.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn run_uninstall_plan(args: &[String]) -> ExitCode {
    let Some(bundle_identifier) = args.get(1) else {
        eprintln!("uninstall-plan requires a bundle identifier");
        print_usage();
        return ExitCode::FAILURE;
    };

    if env::var_os("HOME").is_none() {
        eprintln!("cannot build uninstall plan: HOME is not set, so user applications and related files cannot be inventoried completely");
        return ExitCode::FAILURE;
    }

    let platform = SystemMacPlatform;
    let inventory = platform.inventory();
    if !inventory.issues.is_empty() {
        for issue in &inventory.issues {
            eprintln!(
                "inventory warning: {}: {}",
                issue.path.display(),
                issue.message
            );
        }
        eprintln!("cannot build uninstall plan from incomplete application inventory");
        return ExitCode::FAILURE;
    }

    let matches: Vec<_> = inventory
        .applications
        .iter()
        .filter(|application| {
            application.metadata.bundle_identifier.as_deref() == Some(bundle_identifier.as_str())
        })
        .collect();

    let application = match matches.as_slice() {
        [] => {
            eprintln!("no installed application found for bundle identifier: {bundle_identifier}");
            return ExitCode::FAILURE;
        }
        [application] => (*application).clone(),
        _ => {
            eprintln!(
                "multiple installed applications found for bundle identifier: {bundle_identifier}; select by path in a future review surface"
            );
            return ExitCode::FAILURE;
        }
    };

    let related = platform.related_files(&application);
    let plan = UninstallPlan::build(application, related);
    let application = plan.application();

    println!(
        "application\t{}\t{}\tprotected={}",
        application.name,
        application.path.display(),
        plan.is_protected()
    );
    for item in plan.items() {
        let kind = match item.kind() {
            UninstallPlanItemKind::ApplicationBundle => "application",
            UninstallPlanItemKind::RelatedFile(kind) => kind.label(),
        };
        println!(
            "{}\t{}\t{}\tselected={}\tselectable={}\trequired={}\treview_only={}",
            item.confidence().label(),
            kind,
            item.path().display(),
            item.is_selected(),
            item.is_selectable(),
            item.is_required(),
            item.is_review_only()
        );
    }
    println!("selected items: {}", plan.selected_count());
    println!("mode: review-only; no filesystem mutation");
    ExitCode::SUCCESS
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
    scan_request_with_home(category, env::var_os("HOME").map(PathBuf::from))
}

fn scan_request_with_home(
    category: CleanupCategory,
    home: Option<PathBuf>,
) -> Result<cleaner_core::ScanRequest, String> {
    if category == CleanupCategory::SystemCache {
        return Ok(SystemCacheScan.request());
    }

    let home = home.ok_or_else(|| "HOME is not set".to_string())?;

    match category {
        CleanupCategory::UserCache => Ok(UserCacheScan::new(home).request()),
        CleanupCategory::Xcode => Ok(XcodeScan::new(home).request()),
        CleanupCategory::Homebrew => Ok(HomebrewScan::new(home).request()),
        CleanupCategory::Node => Ok(NodeScan::new(home).request()),
        CleanupCategory::SystemCache => unreachable!("handled before HOME lookup"),
        CleanupCategory::Docker | CleanupCategory::LargeFiles => Err(format!(
            "category '{}' is not implemented in M1",
            category.label()
        )),
    }
}

fn print_usage() {
    println!("dxtr-cleaner scan [--category user|system|dev|brew|node]");
    println!("dxtr-cleaner apps");
    println!("dxtr-cleaner related <bundle-id>");
    println!("dxtr-cleaner orphans");
    println!("dxtr-cleaner uninstall-plan <bundle-id>");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_scan_request_does_not_require_home() {
        let request = scan_request_with_home(CleanupCategory::SystemCache, None)
            .expect("system request must not require HOME");
        assert_eq!(request.category, CleanupCategory::SystemCache);
        assert_eq!(request.roots, vec![PathBuf::from("/Library/Caches")]);
    }

    #[test]
    fn user_relative_scan_still_requires_home() {
        assert_eq!(
            scan_request_with_home(CleanupCategory::UserCache, None).unwrap_err(),
            "HOME is not set"
        );
    }
}
