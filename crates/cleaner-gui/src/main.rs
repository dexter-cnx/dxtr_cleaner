mod execution;

use std::{
    env,
    path::PathBuf,
    sync::mpsc::{self, TryRecvError},
    thread,
    time::Duration,
};

use cleaner_core::{
    CancellationToken, CategoryScanTarget, CleanupCategory, CleanupPlan, ExecutionPolicy,
    ExecutionReport, FileSystemScanner, HomebrewScan, NodeScan, Planner, ScanEvent, ScanItem,
    ScanRequest, Scanner, SystemCacheScan, UserCacheScan, XcodeScan,
};
use execution::{ExecutionMessage, policy_from_requests, spawn_trash_only_execution};
use gpui::{
    App, Bounds, Context, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
};
use gpui_platform::application;

const MAX_EVENTS_PER_TICK: usize = 128;
const MAX_REVIEW_ROWS: usize = 12;

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
}

impl CleanerApp {
    fn new() -> Self {
        Self {
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
            UiMessage::Event(ScanEvent::PermissionDenied { .. }) => {
                self.permission_denied += 1;
            }
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
            self.error = Some("HOME is not set".to_string());
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
        let worker_cancellation = cancellation.clone();

        thread::spawn(move || {
            let scanner = FileSystemScanner;

            for request in requests {
                if worker_cancellation.is_cancelled() {
                    let _ = tx.send(UiMessage::Cancelled);
                    return;
                }

                let event_tx = tx.clone();
                let mut sink = move |event| {
                    let _ = event_tx.send(UiMessage::Event(event));
                };

                if let Err(error) = scanner.scan_with(&request, &worker_cancellation, &mut sink) {
                    let _ = tx.send(UiMessage::Failed(error.to_string()));
                    return;
                }
            }

            let terminal = if worker_cancellation.is_cancelled() {
                UiMessage::Cancelled
            } else {
                UiMessage::Completed
            };
            let _ = tx.send(terminal);
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
        let Some(cancellation) = &self.cancellation else {
            return;
        };

        cancellation.cancel();
        self.state = ScanState::Cancelling;
        cx.notify();
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
            self.execution_error = Some("execution policy is unavailable".to_string());
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
                                cx.notify();
                            });
                            break;
                        }
                        Err(TryRecvError::Empty) => {}
                        Err(TryRecvError::Disconnected) => {
                            entity.update(cx, |this, cx| {
                                this.execution_state = ExecutionState::Failed;
                                this.execution_error =
                                    Some("cleanup worker disconnected".to_string());
                                this.execution_cancellation = None;
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
        let Some(cancellation) = &self.execution_cancellation else {
            return;
        };

        cancellation.cancel();
        self.execution_state = ExecutionState::Cancelling;
        cx.notify();
    }

    fn set_all_review_items(&mut self, selected: bool, cx: &mut Context<Self>) {
        if let Some(plan) = &mut self.cleanup_plan {
            plan.set_all_selected(selected);
            cx.notify();
        }
    }
}

impl Render for CleanerApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let scan_active = matches!(self.state, ScanState::Scanning | ScanState::Cancelling);
        let execution_active = matches!(
            self.execution_state,
            ExecutionState::Executing | ExecutionState::Cancelling
        );
        let primary_label = if scan_active {
            self.state.label()
        } else {
            "Start Smart Scan"
        };

        let status_text = match &self.error {
            Some(error) => format!("{} · {error}", self.state.label()),
            None if self.permission_denied > 0 => format!(
                "{} · {} permission-denied path(s)",
                self.state.label(),
                self.permission_denied
            ),
            None => self.state.label().to_string(),
        };

        let execution_status = match (&self.execution_report, &self.execution_error) {
            (Some(report), _) => {
                let cancelled = if report.cancelled { " · cancelled" } else { "" };
                format!(
                    "{} · {} moved to Trash · {} failed · {}{}",
                    self.execution_state.label(),
                    report.succeeded_count(),
                    report.failed_count(),
                    format_bytes(report.moved_bytes()),
                    cancelled
                )
            }
            (None, Some(error)) => format!("{} · {error}", self.execution_state.label()),
            _ => self.execution_state.label().to_string(),
        };

        let review_panel = self.cleanup_plan.as_ref().map(|plan| {
            let selected_count = plan.selected_count();
            let selected_bytes = plan.selected_bytes();
            let total_count = plan.items.len();
            let hidden_count = total_count.saturating_sub(MAX_REVIEW_ROWS);
            let rows = plan
                .items
                .iter()
                .take(MAX_REVIEW_ROWS)
                .map(|entry| {
                    let marker = if entry.item.is_symlink {
                        "Protected symlink"
                    } else if entry.selected {
                        "Selected"
                    } else {
                        "Skipped"
                    };
                    let marker_color = if entry.selected {
                        rgb(0x83e6a2)
                    } else {
                        rgb(0xa9afb8)
                    };

                    div()
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .bg(rgb(0x13161b))
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .child(
                            div()
                                .flex_1()
                                .child(entry.item.path.display().to_string()),
                        )
                        .child(
                            div()
                                .text_color(rgb(0xa9afb8))
                                .child(format_bytes(entry.item.bytes)),
                        )
                        .child(div().text_color(marker_color).child(marker))
                })
                .collect::<Vec<_>>();

            div()
                .p_5()
                .rounded_xl()
                .bg(rgb(0x171a20))
                .border_1()
                .border_color(rgb(0x262a33))
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(div().text_lg().child("Cleanup plan review"))
                        .child(
                            div().text_color(rgb(0xa9afb8)).child(format!(
                                "{} of {} selected · {}",
                                selected_count,
                                total_count,
                                format_bytes(selected_bytes)
                            )),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            div()
                                .id("select-all")
                                .px_3()
                                .py_2()
                                .rounded_md()
                                .bg(rgb(0x2b303a))
                                .cursor_pointer()
                                .child("Select all safe items")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.set_all_review_items(true, cx);
                                })),
                        )
                        .child(
                            div()
                                .id("deselect-all")
                                .px_3()
                                .py_2()
                                .rounded_md()
                                .bg(rgb(0x2b303a))
                                .cursor_pointer()
                                .child("Deselect all")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.set_all_review_items(false, cx);
                                })),
                        ),
                )
                .children(rows)
                .when(hidden_count > 0, |panel| {
                    panel.child(
                        div().text_color(rgb(0xa9afb8)).child(format!(
                            "+ {hidden_count} more item(s) in this plan"
                        )),
                    )
                })
                .when(selected_count > 0, |panel| {
                    panel.child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .id("execute-trash")
                                    .px_5()
                                    .py_3()
                                    .rounded_lg()
                                    .bg(rgb(0x4f7cff))
                                    .cursor_pointer()
                                    .child(if execution_active {
                                        "Moving to Trash…"
                                    } else {
                                        "Move selected to Trash"
                                    })
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.start_cleanup(window, cx);
                                    })),
                            )
                            .when(execution_active, |row| {
                                row.child(
                                    div()
                                        .id("cancel-cleanup")
                                        .px_5()
                                        .py_3()
                                        .rounded_lg()
                                        .bg(rgb(0x2b303a))
                                        .cursor_pointer()
                                        .child("Cancel cleanup")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.cancel_execution(cx);
                                        })),
                                )
                            }),
                    )
                })
        });

        div()
            .flex()
            .size_full()
            .bg(rgb(0x0f1115))
            .text_color(rgb(0xe8eaed))
            .child(sidebar())
            .child(
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
                            .child(div().text_2xl().child("Smart Care"))
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
                    .child(
                        div()
                            .p_6()
                            .rounded_xl()
                            .bg(rgb(0x171a20))
                            .border_1()
                            .border_color(rgb(0x262a33))
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child(div().text_xl().child("Reclaim your Mac"))
                            .child(
                                div().text_color(rgb(0xa9afb8)).child(
                                    "Scan first. Review every cleanup plan before execution.",
                                ),
                            )
                            .child(div().text_color(rgb(0xa9afb8)).child(status_text))
                            .child(
                                div()
                                    .flex()
                                    .gap_3()
                                    .child(
                                        div()
                                            .id("scan")
                                            .w(px(160.0))
                                            .px_5()
                                            .py_3()
                                            .rounded_lg()
                                            .bg(rgb(0x4f7cff))
                                            .cursor_pointer()
                                            .child(primary_label)
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
                                            div()
                                                .id("cancel")
                                                .px_5()
                                                .py_3()
                                                .rounded_lg()
                                                .bg(rgb(0x2b303a))
                                                .cursor_pointer()
                                                .child("Cancel")
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.cancel_scan(cx);
                                                })),
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
                    )
                    .when_some(review_panel, |page, panel| page.child(panel))
                    .when(
                        self.execution_state != ExecutionState::Idle,
                        |page| {
                            page.child(
                                div()
                                    .p_5()
                                    .rounded_xl()
                                    .bg(rgb(0x171a20))
                                    .border_1()
                                    .border_color(rgb(0x262a33))
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(div().text_lg().child("Execution report"))
                                    .child(
                                        div()
                                            .text_color(rgb(0xa9afb8))
                                            .child(execution_status),
                                    ),
                            )
                        },
                    )
                    .child(
                        div()
                            .p_5()
                            .rounded_xl()
                            .bg(rgb(0x171a20))
                            .border_1()
                            .border_color(rgb(0x262a33))
                            .child(
                                "Cleanup execution is Trash-only. Permanent delete remains safety-locked in the macOS backend.",
                            ),
                    ),
            )
    }
}

fn sidebar() -> impl IntoElement {
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
        .child(nav_item("Smart Care", true))
        .child(nav_item("Cleanup", false))
        .child(nav_item("Uninstaller", false))
        .child(nav_item("Orphans", false))
        .child(nav_item("Settings", false))
}

fn nav_item(label: &'static str, selected: bool) -> impl IntoElement {
    div()
        .px_3()
        .py_2()
        .rounded_md()
        .when(selected, |item| item.bg(rgb(0x222733)))
        .child(label)
}

fn metric_card(title: &'static str, metric: Metric) -> impl IntoElement {
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
        .child(
            div()
                .text_lg()
                .child(format_bytes(metric.bytes))
                .child(format!(" · {} item(s)", metric.items)),
        )
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
        let bounds = Bounds::centered(None, size(px(1080.0), px(720.0)), cx);
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
