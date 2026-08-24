mod execution;
mod uninstaller;

use std::{
    env,
    path::{Path, PathBuf},
    sync::mpsc::{self, TryRecvError},
    thread,
    time::Duration,
};

use cleaner_core::{
    CancellationToken, CategoryScanTarget, CleanupCategory, CleanupPlan, ExecutionPolicy,
    ExecutionReport, FileSystemScanner, HomebrewScan, InstalledApplication, NodeScan, Planner,
    ScanEvent, ScanItem, ScanRequest, Scanner, SystemCacheScan, UninstallExecutionReport,
    UninstallPlan, UserCacheScan, XcodeScan,
};
use execution::{ExecutionMessage, policy_from_requests, spawn_trash_only_execution};
use gpui::{
    App, Bounds, Context, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
};
use gpui_platform::application;
use uninstaller::{
    InventoryMessage, PlanMessage, UninstallMessage, spawn_inventory, spawn_plan, spawn_uninstall,
};

const MAX_EVENTS_PER_TICK: usize = 128;
const MAX_REVIEW_ROWS: usize = 10;
const MAX_APP_ROWS: usize = 14;
const MAX_UNINSTALL_ROWS: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    SmartCare,
    Uninstaller,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScanState {
    Idle,
    Scanning,
    Cancelling,
    Completed,
    Cancelled,
    Failed,
}

impl ScanState {
    fn label(self) -> &'static str {
        match self {
            Self::Idle => "Ready",
            Self::Scanning => "Scanning…",
            Self::Cancelling => "Cancelling…",
            Self::Completed => "Review ready",
            Self::Cancelled => "Cancelled",
            Self::Failed => "Failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExecutionState {
    Idle,
    Executing,
    Cancelling,
    Completed,
    Cancelled,
    Failed,
}

impl ExecutionState {
    fn label(self) -> &'static str {
        match self {
            Self::Idle => "Not started",
            Self::Executing => "Moving selected items to Trash…",
            Self::Cancelling => "Cancelling cleanup…",
            Self::Completed => "Cleanup complete",
            Self::Cancelled => "Cleanup cancelled",
            Self::Failed => "Cleanup failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UninstallState {
    Idle,
    Loading,
    Review,
    Executing,
    Cancelling,
    Completed,
    Failed,
}

impl UninstallState {
    fn label(self) -> &'static str {
        match self {
            Self::Idle => "Ready",
            Self::Loading => "Loading…",
            Self::Review => "Review before uninstall",
            Self::Executing => "Moving reviewed items to Trash…",
            Self::Cancelling => "Cancelling uninstall…",
            Self::Completed => "Uninstall attempt complete",
            Self::Failed => "Uninstall blocked or incomplete",
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct Metric {
    items: usize,
    bytes: u64,
}

enum UiMessage {
    Event(ScanEvent),
    Completed,
    Cancelled,
    Failed(String),
}

struct CleanerApp {
    view: ViewMode,
    state: ScanState,
    execution_state: ExecutionState,
    user_cache: Metric,
    system_cache: Metric,
    xcode: Metric,
    homebrew: Metric,
    node: Metric,
    permission_denied: usize,
    error: Option<String>,
    execution_error: Option<String>,
    cancellation: Option<CancellationToken>,
    execution_cancellation: Option<CancellationToken>,
    scanned_items: Vec<ScanItem>,
    scan_requests: Vec<ScanRequest>,
    cleanup_plan: Option<CleanupPlan>,
    execution_policy: Option<ExecutionPolicy>,
    execution_report: Option<ExecutionReport>,
    uninstall_state: UninstallState,
    uninstall_error: Option<String>,
    applications: Vec<InstalledApplication>,
    uninstall_plan: Option<UninstallPlan>,
    uninstall_report: Option<UninstallExecutionReport>,
    uninstall_cancellation: Option<CancellationToken>,
}

impl CleanerApp {
    fn new() -> Self {
        Self {
            view: ViewMode::SmartCare,
            state: ScanState::Idle,
            execution_state: ExecutionState::Idle,
            user_cache: Metric::default(),
            system_cache: Metric::default(),
            xcode: Metric::default(),
            homebrew: Metric::default(),
            node: Metric::default(),
            permission_denied: 0,
            error: None,
            execution_error: None,
            cancellation: None,
            execution_cancellation: None,
            scanned_items: Vec::new(),
            scan_requests: Vec::new(),
            cleanup_plan: None,
            execution_policy: None,
            execution_report: None,
            uninstall_state: UninstallState::Idle,
            uninstall_error: None,
            applications: Vec::new(),
            uninstall_plan: None,
            uninstall_report: None,
            uninstall_cancellation: None,
        }
    }

    fn reset_metrics(&mut self) {
        self.user_cache = Metric::default();
        self.system_cache = Metric::default();
        self.xcode = Metric::default();
        self.homebrew = Metric::default();
        self.node = Metric::default();
        self.permission_denied = 0;
        self.error = None;
        self.execution_error = None;
        self.scanned_items.clear();
        self.scan_requests.clear();
        self.cleanup_plan = None;
        self.execution_policy = None;
        self.execution_report = None;
        self.execution_state = ExecutionState::Idle;
        self.execution_cancellation = None;
    }

    fn metric_mut(&mut self, category: CleanupCategory) -> Option<&mut Metric> {
        match category {
            CleanupCategory::UserCache => Some(&mut self.user_cache),
            CleanupCategory::SystemCache => Some(&mut self.system_cache),
            CleanupCategory::Xcode => Some(&mut self.xcode),
            CleanupCategory::Homebrew => Some(&mut self.homebrew),
            CleanupCategory::Node => Some(&mut self.node),
            _ => None,
        }
    }

    fn apply_message(&mut self, message: UiMessage) {
        match message {
            UiMessage::Event(ScanEvent::ItemFound { item }) => {
                if let Some(metric) = self.metric_mut(item.category) {
                    metric.items += 1;
                    metric.bytes = metric.bytes.saturating_add(item.bytes);
                }
                self.scanned_items.push(item);
            }
            UiMessage::Event(ScanEvent::PermissionDenied { .. }) => self.permission_denied += 1,
            UiMessage::Event(_) => {}
            UiMessage::Completed => {
                self.state = ScanState::Completed;
                self.cancellation = None;
                self.cleanup_plan = Some(Planner::build(std::mem::take(&mut self.scanned_items)));
                self.execution_policy = Some(policy_from_requests(&self.scan_requests));
            }
            UiMessage::Cancelled => {
                self.state = ScanState::Cancelled;
                self.cancellation = None;
                self.scanned_items.clear();
                self.execution_policy = None;
            }
            UiMessage::Failed(error) => {
                self.state = ScanState::Failed;
                self.error = Some(error);
                self.cancellation = None;
                self.scanned_items.clear();
                self.execution_policy = None;
            }
        }
    }

    fn start_scan(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.state, ScanState::Scanning | ScanState::Cancelling)
            || matches!(
                self.execution_state,
                ExecutionState::Executing | ExecutionState::Cancelling
            )
        {
            return;
        }

        let Some(home) = env::var_os("HOME").map(PathBuf::from) else {
            self.state = ScanState::Failed;
            self.error = Some("HOME is not set".into());
            cx.notify();
            return;
        };

        self.reset_metrics();
        self.state = ScanState::Scanning;
        let cancellation = CancellationToken::new();
        self.cancellation = Some(cancellation.clone());

        let requests = vec![
            UserCacheScan::new(home.clone()).request(),
            SystemCacheScan.request(),
            XcodeScan::new(home.clone()).request(),
            HomebrewScan::new(home.clone()).request(),
            NodeScan::new(home).request(),
        ];
        self.scan_requests = requests.clone();

        let (tx, rx) = mpsc::channel::<UiMessage>();
        thread::spawn(move || {
            let scanner = FileSystemScanner;
            for request in requests {
                if cancellation.is_cancelled() {
                    let _ = tx.send(UiMessage::Cancelled);
                    return;
                }
                let event_tx = tx.clone();
                let mut sink = move |event| {
                    let _ = event_tx.send(UiMessage::Event(event));
                };
                if let Err(error) = scanner.scan_with(&request, &cancellation, &mut sink) {
                    let _ = tx.send(UiMessage::Failed(error.to_string()));
                    return;
                }
            }
            let _ = tx.send(if cancellation.is_cancelled() {
                UiMessage::Cancelled
            } else {
                UiMessage::Completed
            });
        });

        let entity = cx.entity();
        window
            .spawn(cx, async move |cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(50))
                        .await;
                    let mut terminal = false;
                    for _ in 0..MAX_EVENTS_PER_TICK {
                        match rx.try_recv() {
                            Ok(message) => {
                                terminal = matches!(
                                    message,
                                    UiMessage::Completed
                                        | UiMessage::Cancelled
                                        | UiMessage::Failed(_)
                                );
                                entity.update(cx, |this, cx| {
                                    this.apply_message(message);
                                    cx.notify();
                                });
                                if terminal {
                                    break;
                                }
                            }
                            Err(TryRecvError::Empty) => break,
                            Err(TryRecvError::Disconnected) => {
                                terminal = true;
                                break;
                            }
                        }
                    }
                    if terminal {
                        break;
                    }
                }
            })
            .detach();
        cx.notify();
    }

    fn cancel_scan(&mut self, cx: &mut Context<Self>) {
        if let Some(cancellation) = &self.cancellation {
            cancellation.cancel();
            self.state = ScanState::Cancelling;
            cx.notify();
        }
    }

    fn start_cleanup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(
            self.execution_state,
            ExecutionState::Executing | ExecutionState::Cancelling
        ) {
            return;
        }
        let Some(plan) = self.cleanup_plan.clone() else {
            return;
        };
        if plan.selected_count() == 0 {
            return;
        }
        let Some(policy) = self.execution_policy.clone() else {
            self.execution_state = ExecutionState::Failed;
            self.execution_error = Some("execution policy is unavailable".into());
            cx.notify();
            return;
        };

        self.execution_state = ExecutionState::Executing;
        self.execution_error = None;
        self.execution_report = None;
        let cancellation = CancellationToken::new();
        self.execution_cancellation = Some(cancellation.clone());
        let rx = spawn_trash_only_execution(plan, policy, cancellation);
        let entity = cx.entity();

        window
            .spawn(cx, async move |cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(50))
                        .await;
                    match rx.try_recv() {
                        Ok(ExecutionMessage::Completed(report)) => {
                            entity.update(cx, |this, cx| {
                                this.execution_state = if report.cancelled {
                                    ExecutionState::Cancelled
                                } else if report.failed_count() > 0 {
                                    ExecutionState::Failed
                                } else {
                                    ExecutionState::Completed
                                };
                                this.execution_report = Some(report);
                                this.execution_cancellation = None;
                                this.cleanup_plan = None;
                                cx.notify();
                            });
                            break;
                        }
                        Ok(ExecutionMessage::Failed(error)) => {
                            entity.update(cx, |this, cx| {
                                this.execution_state = ExecutionState::Failed;
                                this.execution_error = Some(error);
                                this.execution_cancellation = None;
                                this.cleanup_plan = None;
                                cx.notify();
                            });
                            break;
                        }
                        Err(TryRecvError::Empty) => {}
                        Err(TryRecvError::Disconnected) => {
                            entity.update(cx, |this, cx| {
                                this.execution_state = ExecutionState::Failed;
                                this.execution_error = Some("cleanup worker disconnected".into());
                                this.execution_cancellation = None;
                                this.cleanup_plan = None;
                                cx.notify();
                            });
                            break;
                        }
                    }
                }
            })
            .detach();
        cx.notify();
    }

    fn cancel_execution(&mut self, cx: &mut Context<Self>) {
        if let Some(cancellation) = &self.execution_cancellation {
            cancellation.cancel();
            self.execution_state = ExecutionState::Cancelling;
            cx.notify();
        }
    }

    fn set_all_review_items(&mut self, selected: bool, cx: &mut Context<Self>) {
        if let Some(plan) = &mut self.cleanup_plan {
            plan.set_all_selected(selected);
            cx.notify();
        }
    }

    fn open_smart_care(&mut self, cx: &mut Context<Self>) {
        self.view = ViewMode::SmartCare;
        cx.notify();
    }

    fn open_uninstaller(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.view = ViewMode::Uninstaller;
        if self.applications.is_empty() && self.uninstall_state != UninstallState::Loading {
            self.load_applications(window, cx);
        } else {
            cx.notify();
        }
    }

    fn load_applications(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(
            self.uninstall_state,
            UninstallState::Loading | UninstallState::Executing | UninstallState::Cancelling
        ) {
            return;
        }
        self.uninstall_state = UninstallState::Loading;
        self.uninstall_error = None;
        self.uninstall_plan = None;
        self.uninstall_report = None;
        let rx = spawn_inventory();
        let entity = cx.entity();

        window
            .spawn(cx, async move |cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(50))
                        .await;
                    match rx.try_recv() {
                        Ok(InventoryMessage::Loaded(apps)) => {
                            entity.update(cx, |this, cx| {
                                this.applications = apps;
                                this.uninstall_state = UninstallState::Idle;
                                cx.notify();
                            });
                            break;
                        }
                        Ok(InventoryMessage::Failed(error)) => {
                            entity.update(cx, |this, cx| {
                                this.applications.clear();
                                this.uninstall_state = UninstallState::Failed;
                                this.uninstall_error = Some(error);
                                cx.notify();
                            });
                            break;
                        }
                        Err(TryRecvError::Empty) => {}
                        Err(TryRecvError::Disconnected) => {
                            entity.update(cx, |this, cx| {
                                this.uninstall_state = UninstallState::Failed;
                                this.uninstall_error = Some("inventory worker disconnected".into());
                                cx.notify();
                            });
                            break;
                        }
                    }
                }
            })
            .detach();
        cx.notify();
    }

    fn prepare_uninstall(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(
            self.uninstall_state,
            UninstallState::Loading | UninstallState::Executing | UninstallState::Cancelling
        ) {
            return;
        }
        let Some(application) = self.applications.get(index).cloned() else {
            return;
        };
        self.uninstall_state = UninstallState::Loading;
        self.uninstall_error = None;
        self.uninstall_report = None;
        self.uninstall_plan = None;
        let rx = spawn_plan(application);
        let entity = cx.entity();

        window
            .spawn(cx, async move |cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(50))
                        .await;
                    match rx.try_recv() {
                        Ok(PlanMessage::Ready(plan)) => {
                            entity.update(cx, |this, cx| {
                                this.uninstall_state = UninstallState::Review;
                                this.uninstall_plan = Some(plan);
                                cx.notify();
                            });
                            break;
                        }
                        Ok(PlanMessage::Failed(error)) => {
                            entity.update(cx, |this, cx| {
                                this.uninstall_state = UninstallState::Failed;
                                this.uninstall_error = Some(error);
                                cx.notify();
                            });
                            break;
                        }
                        Err(TryRecvError::Empty) => {}
                        Err(TryRecvError::Disconnected) => {
                            entity.update(cx, |this, cx| {
                                this.uninstall_state = UninstallState::Failed;
                                this.uninstall_error = Some("plan worker disconnected".into());
                                cx.notify();
                            });
                            break;
                        }
                    }
                }
            })
            .detach();
        cx.notify();
    }

    fn toggle_uninstall_item(&mut self, path: &Path, cx: &mut Context<Self>) {
        if self.uninstall_state != UninstallState::Review {
            return;
        }
        let Some(plan) = &mut self.uninstall_plan else {
            return;
        };
        let Some(item) = plan.items().iter().find(|item| item.path() == path) else {
            return;
        };
        let selected = item.is_selected();
        if plan.set_selected(path, !selected) {
            cx.notify();
        }
    }

    fn start_uninstall(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.uninstall_state != UninstallState::Review {
            return;
        }
        let Some(plan) = self.uninstall_plan.clone() else {
            return;
        };
        if plan.is_protected() || plan.selected_count() == 0 {
            return;
        }

        self.uninstall_state = UninstallState::Executing;
        self.uninstall_error = None;
        self.uninstall_report = None;
        let cancellation = CancellationToken::new();
        self.uninstall_cancellation = Some(cancellation.clone());
        let rx = spawn_uninstall(plan, cancellation);
        let entity = cx.entity();

        window
            .spawn(cx, async move |cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(50))
                        .await;
                    match rx.try_recv() {
                        Ok(UninstallMessage::Completed(report)) => {
                            entity.update(cx, |this, cx| {
                                let incomplete =
                                    report.safety_failure.is_some() || report.failed_count() > 0;
                                this.uninstall_state = if report.cancelled {
                                    UninstallState::Idle
                                } else if incomplete {
                                    UninstallState::Failed
                                } else {
                                    UninstallState::Completed
                                };
                                this.uninstall_report = Some(report);
                                this.uninstall_cancellation = None;
                                this.uninstall_plan = None;
                                this.applications.clear();
                                cx.notify();
                            });
                            break;
                        }
                        Ok(UninstallMessage::Failed(error)) => {
                            entity.update(cx, |this, cx| {
                                this.uninstall_state = UninstallState::Failed;
                                this.uninstall_error = Some(error);
                                this.uninstall_cancellation = None;
                                this.uninstall_plan = None;
                                this.applications.clear();
                                cx.notify();
                            });
                            break;
                        }
                        Err(TryRecvError::Empty) => {}
                        Err(TryRecvError::Disconnected) => {
                            entity.update(cx, |this, cx| {
                                this.uninstall_state = UninstallState::Failed;
                                this.uninstall_error = Some("uninstall worker disconnected".into());
                                this.uninstall_cancellation = None;
                                this.uninstall_plan = None;
                                this.applications.clear();
                                cx.notify();
                            });
                            break;
                        }
                    }
                }
            })
            .detach();
        cx.notify();
    }

    fn cancel_uninstall(&mut self, cx: &mut Context<Self>) {
        if let Some(cancellation) = &self.uninstall_cancellation {
            cancellation.cancel();
            self.uninstall_state = UninstallState::Cancelling;
            cx.notify();
        }
    }
}

impl Render for CleanerApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .size_full()
            .bg(rgb(0x0f1115))
            .text_color(rgb(0xe8eaed))
            .child(self.sidebar(cx))
            .child(match self.view {
                ViewMode::SmartCare => self.smart_care_page(cx),
                ViewMode::Uninstaller => self.uninstaller_page(cx),
            })
    }
}

impl CleanerApp {
    fn sidebar(&self, cx: &mut Context<Self>) -> gpui::Div {
        div()
            .w(px(220.0))
            .h_full()
            .p_5()
            .bg(rgb(0x13161b))
            .border_r_1()
            .border_color(rgb(0x262a33))
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_xl().child("Dxtr Cleaner"))
            .child(
                nav_item("Smart Care", self.view == ViewMode::SmartCare)
                    .id("nav-smart-care")
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| this.open_smart_care(cx))),
            )
            .child(
                nav_item("Uninstaller", self.view == ViewMode::Uninstaller)
                    .id("nav-uninstaller")
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_uninstaller(window, cx);
                    })),
            )
            .child(nav_item("Orphans", false))
            .child(nav_item("Settings", false))
    }

    fn smart_care_page(&self, cx: &mut Context<Self>) -> gpui::Div {
        let scan_active = matches!(self.state, ScanState::Scanning | ScanState::Cancelling);
        let execution_active = matches!(
            self.execution_state,
            ExecutionState::Executing | ExecutionState::Cancelling
        );
        let status_text = match &self.error {
            Some(error) => format!("{} · {error}", self.state.label()),
            None if self.permission_denied > 0 => format!(
                "{} · {} permission-denied path(s)",
                self.state.label(), self.permission_denied
            ),
            None => self.state.label().to_string(),
        };

        let mut page = content_shell("Smart Care")
            .child(
                card()
                    .child(div().text_xl().child("Reclaim your Mac"))
                    .child(div().text_color(rgb(0xa9afb8)).child(status_text))
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .child(
                                button(
                                    "scan",
                                    if scan_active {
                                        self.state.label()
                                    } else {
                                        "Start Smart Scan"
                                    },
                                )
                                .on_click(cx.listener(|this, _, window, cx| {
                                    if !matches!(
                                        this.state,
                                        ScanState::Scanning | ScanState::Cancelling
                                    ) {
                                        this.start_scan(window, cx);
                                    }
                                })),
                            )
                            .when(scan_active, |row| {
                                row.child(
                                    secondary_button("cancel", "Cancel").on_click(cx.listener(
                                        |this, _, _, cx| this.cancel_scan(cx),
                                    )),
                                )
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_4()
                    .child(metric_card("User Cache", self.user_cache))
                    .child(metric_card("System Cache", self.system_cache))
                    .child(metric_card("Xcode", self.xcode))
                    .child(metric_card("Homebrew", self.homebrew))
                    .child(metric_card("Node", self.node)),
            );

        if let Some(plan) = &self.cleanup_plan {
            let rows = plan
                .items
                .iter()
                .take(MAX_REVIEW_ROWS)
                .map(|entry| {
                    let label = if entry.item.is_symlink {
                        "Protected symlink"
                    } else if entry.selected {
                        "Selected"
                    } else {
                        "Skipped"
                    };
                    review_row(
                        entry.item.path.display().to_string(),
                        format_bytes(entry.item.bytes),
                        label,
                    )
                })
                .collect::<Vec<_>>();
            let hidden = plan.items.len().saturating_sub(MAX_REVIEW_ROWS);
            let selected = plan.selected_count();
            page = page.child(
                card()
                    .child(div().text_lg().child(format!(
                        "Cleanup plan · {selected} of {} selected",
                        plan.items.len()
                    )))
                    .when(!execution_active, |panel| {
                        panel.child(
                            div()
                                .flex()
                                .gap_2()
                                .child(
                                    secondary_button("select-all", "Select all safe items")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.set_all_review_items(true, cx);
                                        })),
                                )
                                .child(
                                    secondary_button("deselect-all", "Deselect all").on_click(
                                        cx.listener(|this, _, _, cx| {
                                            this.set_all_review_items(false, cx);
                                        }),
                                    ),
                                ),
                        )
                    })
                    .children(rows)
                    .when(hidden > 0, |panel| {
                        panel.child(
                            div()
                                .text_color(rgb(0xa9afb8))
                                .child(format!("+ {hidden} more item(s)")),
                        )
                    })
                    .when(selected > 0 && !execution_active, |panel| {
                        panel.child(
                            button("execute-trash", "Move selected to Trash").on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.start_cleanup(window, cx);
                                }),
                            ),
                        )
                    })
                    .when(execution_active, |panel| {
                        panel.child(
                            secondary_button("cancel-cleanup", "Cancel cleanup")
                                .on_click(cx.listener(|this, _, _, cx| this.cancel_execution(cx))),
                        )
                    }),
            );
        }

        if self.execution_state != ExecutionState::Idle {
            let detail = match (&self.execution_report, &self.execution_error) {
                (Some(report), _) => format!(
                    "{} · {} moved · {} failed · {}",
                    self.execution_state.label(),
                    report.succeeded_count(),
                    report.failed_count(),
                    format_bytes(report.moved_bytes())
                ),
                (_, Some(error)) => format!("{} · {error}", self.execution_state.label()),
                _ => self.execution_state.label().into(),
            };
            page = page.child(
                card()
                    .child(div().text_lg().child("Execution report"))
                    .child(detail),
            );
        }

        page.child(info_card(
            "Cleanup execution is Trash-only. Permanent delete remains safety-locked.",
        ))
    }

    fn uninstaller_page(&self, cx: &mut Context<Self>) -> gpui::Div {
        let mut page = content_shell("Uninstaller").child(
            card()
                .child(div().text_xl().child("Applications"))
                .child(div().text_color(rgb(0xa9afb8)).child(
                    "Select an application, review every related file, then move the reviewed set to Trash.",
                ))
                .child(
                    div()
                        .text_color(rgb(0xa9afb8))
                        .child(self.uninstall_state.label()),
                )
                .child(
                    secondary_button("refresh-apps", "Refresh applications").on_click(
                        cx.listener(|this, _, window, cx| {
                            this.load_applications(window, cx);
                        }),
                    ),
                ),
        );

        if let Some(error) = &self.uninstall_error {
            page = page.child(
                card().child(div().text_color(rgb(0xffa6a6)).child(error.clone())),
            );
        }

        if self.uninstall_plan.is_none() && !self.applications.is_empty() {
            let rows = self
                .applications
                .iter()
                .take(MAX_APP_ROWS)
                .enumerate()
                .map(|(index, app)| {
                    let bundle = app
                        .metadata
                        .bundle_identifier
                        .as_deref()
                        .unwrap_or("no bundle identifier");
                    div()
                        .id(format!("app-{index}"))
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .bg(rgb(0x13161b))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .child(div().flex_1().child(app.name.clone()))
                        .child(div().text_color(rgb(0xa9afb8)).child(bundle.to_string()))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.prepare_uninstall(index, window, cx);
                        }))
                })
                .collect::<Vec<_>>();
            let hidden = self.applications.len().saturating_sub(MAX_APP_ROWS);
            page = page.child(
                card()
                    .child(div().text_lg().child(format!(
                        "{} installed application(s)",
                        self.applications.len()
                    )))
                    .children(rows)
                    .when(hidden > 0, |panel| {
                        panel.child(div().text_color(rgb(0xa9afb8)).child(format!(
                            "+ {hidden} more application(s); filtering/search comes in the next UI refinement"
                        )))
                    }),
            );
        }

        if let Some(plan) = &self.uninstall_plan {
            let protected = plan.is_protected();
            let app_name = plan.application().name.clone();
            let rows = plan
                .items()
                .iter()
                .take(MAX_UNINSTALL_ROWS)
                .map(|item| {
                    let path = item.path().to_path_buf();
                    let label = if item.is_required() {
                        "Required app"
                    } else if item.is_review_only() && item.is_selected() {
                        "Review-only · opted in"
                    } else if item.is_review_only() {
                        "Review-only · off"
                    } else if item.is_selected() {
                        "Selected"
                    } else {
                        "Skipped"
                    };
                    let row = review_row(
                        item.path().display().to_string(),
                        format!("{:?} · {}", item.confidence(), label),
                        if item.is_selectable() { "Toggle" } else { "Locked" },
                    )
                    .id(format!("uninstall-item-{}", item.path().display()));
                    if item.is_selectable() {
                        row.cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_uninstall_item(&path, cx);
                            }))
                    } else {
                        row
                    }
                })
                .collect::<Vec<_>>();
            let hidden = plan.items().len().saturating_sub(MAX_UNINSTALL_ROWS);
            let selected = plan.selected_count();
            page = page.child(
                card()
                    .child(div().text_lg().child(format!("Review uninstall · {app_name}")))
                    .child(
                        div()
                            .text_color(if protected {
                                rgb(0xffa6a6)
                            } else {
                                rgb(0xa9afb8)
                            })
                            .child(if protected {
                                "Protected Apple/system application. Execution is locked."
                            } else {
                                "High-confidence related files default on. Medium/Low evidence requires explicit opt-in."
                            }),
                    )
                    .children(rows)
                    .when(hidden > 0, |panel| {
                        panel.child(div().text_color(rgb(0xa9afb8)).child(format!(
                            "+ {hidden} more reviewed item(s)"
                        )))
                    })
                    .when(
                        !protected
                            && selected > 0
                            && self.uninstall_state == UninstallState::Review,
                        |panel| {
                            panel.child(
                                button(
                                    "execute-uninstall",
                                    "Move reviewed uninstall to Trash",
                                )
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.start_uninstall(window, cx);
                                })),
                            )
                        },
                    )
                    .when(
                        matches!(
                            self.uninstall_state,
                            UninstallState::Executing | UninstallState::Cancelling
                        ),
                        |panel| {
                            panel.child(
                                secondary_button("cancel-uninstall", "Cancel uninstall")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.cancel_uninstall(cx)
                                    })),
                            )
                        },
                    ),
            );
        }

        if let Some(report) = &self.uninstall_report {
            let safety = report
                .safety_failure
                .as_ref()
                .map(|error| format!(" · safety stop: {error:?}"))
                .unwrap_or_default();
            page = page.child(
                card()
                    .child(div().text_lg().child("Uninstall execution report"))
                    .child(format!(
                        "{} moved to Trash · {} backend failure(s){}",
                        report.succeeded_count(),
                        report.failed_count(),
                        safety
                    )),
            );
        }

        page.child(info_card(
            "Every execution attempt refreshes application inventory and related-file evidence, re-pins safety roots, then discards the reviewed plan. Permanent deletion is not available.",
        ))
    }
}

fn content_shell(title: &'static str) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .p_8()
        .gap_6()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(div().text_2xl().child(title))
                .child(
                    div()
                        .px_3()
                        .py_1()
                        .rounded_full()
                        .bg(rgb(0x173624))
                        .text_color(rgb(0x83e6a2))
                        .child("Safe mode · Trash only"),
                ),
        )
}

fn card() -> gpui::Div {
    div()
        .p_5()
        .rounded_xl()
        .bg(rgb(0x171a20))
        .border_1()
        .border_color(rgb(0x262a33))
        .flex()
        .flex_col()
        .gap_3()
}

fn info_card(text: &'static str) -> gpui::Div {
    card().child(text)
}

fn button(id: &'static str, label: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px_5()
        .py_3()
        .rounded_lg()
        .bg(rgb(0x4f7cff))
        .cursor_pointer()
        .child(label)
}

fn secondary_button(id: &'static str, label: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px_4()
        .py_2()
        .rounded_lg()
        .bg(rgb(0x2b303a))
        .cursor_pointer()
        .child(label)
}

fn nav_item(label: &'static str, selected: bool) -> gpui::Div {
    div()
        .px_3()
        .py_2()
        .rounded_md()
        .when(selected, |item| item.bg(rgb(0x222733)))
        .child(label)
}

fn metric_card(title: &'static str, metric: Metric) -> gpui::Div {
    div()
        .flex_1()
        .p_4()
        .rounded_lg()
        .bg(rgb(0x171a20))
        .border_1()
        .border_color(rgb(0x262a33))
        .flex()
        .flex_col()
        .gap_2()
        .child(div().text_color(rgb(0xa9afb8)).child(title))
        .child(div().text_lg().child(format_bytes(metric.bytes)))
        .child(format!("{} item(s)", metric.items))
}

fn review_row(primary: String, secondary: String, marker: &'static str) -> gpui::Div {
    div()
        .px_3()
        .py_2()
        .rounded_md()
        .bg(rgb(0x13161b))
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .child(div().flex_1().child(primary))
        .child(div().text_color(rgb(0xa9afb8)).child(secondary))
        .child(div().text_color(rgb(0x83e6a2)).child(marker))
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let bytes = bytes as f64;
    if bytes >= GB {
        format!("{:.1} GB", bytes / GB)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes / KB)
    } else {
        format!("{} B", bytes as u64)
    }
}

fn main() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1180.0), px(780.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| CleanerApp::new()),
        )
        .expect("open main window");
        cx.activate(true);
    });
}
