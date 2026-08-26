use std::{
    env,
    path::PathBuf,
    sync::mpsc::{self, TryRecvError},
    thread,
    time::Duration,
};

use cleaner_core::{
    CancellationToken, CleanupCategory, FileSystemScanner, ScanEvent, ScanRequest, Scanner,
};
use cleaner_windows::{WindowsPaths, WindowsScanSet};
use gpui::{
    App, Bounds, Context, Render, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb,
    size,
};
use gpui_platform::application;

const MAX_MESSAGES_PER_TICK: usize = 128;
const EVENT_BATCH_SIZE: usize = 256;

#[derive(Default)]
struct ScanDelta {
    items: usize,
    bytes: u64,
    permission_denied: usize,
}

impl ScanDelta {
    fn is_empty(&self) -> bool {
        self.items == 0 && self.bytes == 0 && self.permission_denied == 0
    }
}

enum ScanMessage {
    Delta(ScanDelta),
    Complete,
    Failed(String),
}

struct WindowsSpike {
    root: Option<PathBuf>,
    scanning: bool,
    items: usize,
    bytes: u64,
    permission_denied: usize,
    error: Option<String>,
}

impl WindowsSpike {
    fn new() -> Self {
        Self {
            root: env::args_os().nth(1).map(PathBuf::from),
            scanning: false,
            items: 0,
            bytes: 0,
            permission_denied: 0,
            error: None,
        }
    }

    fn start_scan(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.scanning {
            return;
        }

        self.scanning = true;
        self.items = 0;
        self.bytes = 0;
        self.permission_denied = 0;
        self.error = None;

        let root = self.root.clone();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let requests = match root {
                Some(root) => vec![ScanRequest {
                    category: CleanupCategory::UserCache,
                    roots: vec![root],
                    excluded_roots: Vec::new(),
                }],
                None => {
                    let paths = match WindowsPaths::discover() {
                        Ok(paths) => paths,
                        Err(error) => {
                            let _ = tx.send(ScanMessage::Failed(error));
                            return;
                        }
                    };
                    match WindowsScanSet::discover(&paths) {
                        Ok(set) => set.into_requests(),
                        Err(error) => {
                            let _ = tx.send(ScanMessage::Failed(error.to_string()));
                            return;
                        }
                    }
                }
            };

            let cancellation = CancellationToken::new();
            let scanner = FileSystemScanner;
            let mut pending = ScanDelta::default();
            let mut pending_events = 0usize;

            for request in requests {
                let mut sink = |event| {
                    match event {
                        ScanEvent::ItemFound { item } => {
                            pending.items += 1;
                            pending.bytes = pending.bytes.saturating_add(item.bytes);
                            pending_events += 1;
                        }
                        ScanEvent::PermissionDenied { .. } => {
                            pending.permission_denied += 1;
                            pending_events += 1;
                        }
                        ScanEvent::Started { .. }
                        | ScanEvent::Finished { .. }
                        | ScanEvent::Cancelled { .. } => {}
                    }

                    if pending_events >= EVENT_BATCH_SIZE {
                        let delta = std::mem::take(&mut pending);
                        pending_events = 0;
                        let _ = tx.send(ScanMessage::Delta(delta));
                    }
                };
                if let Err(error) = scanner.scan_with(&request, &cancellation, &mut sink) {
                    if !pending.is_empty() {
                        let _ = tx.send(ScanMessage::Delta(std::mem::take(&mut pending)));
                    }
                    let _ = tx.send(ScanMessage::Failed(error.to_string()));
                    return;
                }
            }

            if !pending.is_empty() {
                let _ = tx.send(ScanMessage::Delta(pending));
            }
            let _ = tx.send(ScanMessage::Complete);
        });

        let entity = cx.entity();
        window
            .spawn(cx, async move |cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(50))
                        .await;

                    let mut terminal = false;
                    for _ in 0..MAX_MESSAGES_PER_TICK {
                        match rx.try_recv() {
                            Ok(ScanMessage::Delta(delta)) => {
                                entity.update(cx, |this, cx| {
                                    this.items = this.items.saturating_add(delta.items);
                                    this.bytes = this.bytes.saturating_add(delta.bytes);
                                    this.permission_denied = this
                                        .permission_denied
                                        .saturating_add(delta.permission_denied);
                                    cx.notify();
                                });
                            }
                            Ok(ScanMessage::Complete) => {
                                entity.update(cx, |this, cx| {
                                    this.scanning = false;
                                    cx.notify();
                                });
                                terminal = true;
                                break;
                            }
                            Ok(ScanMessage::Failed(error)) => {
                                entity.update(cx, |this, cx| {
                                    this.scanning = false;
                                    this.error = Some(error);
                                    cx.notify();
                                });
                                terminal = true;
                                break;
                            }
                            Err(TryRecvError::Empty) => break,
                            Err(TryRecvError::Disconnected) => {
                                entity.update(cx, |this, cx| {
                                    this.scanning = false;
                                    cx.notify();
                                });
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
}

impl Render for WindowsSpike {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = self
            .root
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "Windows Smart Scan providers".into());
        let status = if self.scanning {
            "Scanning…"
        } else {
            "Ready"
        };
        let action = if self.scanning {
            "Scanning…"
        } else if self.root.is_some() {
            "Scan directory"
        } else {
            "Run Smart Scan"
        };

        div()
            .size_full()
            .p_8()
            .bg(rgb(0x0f1115))
            .text_color(rgb(0xe8eaed))
            .flex()
            .flex_col()
            .gap_5()
            .child(div().text_2xl().child("Dxtr Cleaner · Windows GPUI Spike"))
            .child(div().text_color(rgb(0xa9afb8)).child(root))
            .child(div().child(format!(
                "{status} · {} item(s) · {} bytes · {} permission-denied path(s)",
                self.items, self.bytes, self.permission_denied
            )))
            .when_some(self.error.clone(), |view, error| {
                view.child(div().text_color(rgb(0xffa6a6)).child(error))
            })
            .child(
                div()
                    .id("scan")
                    .px_5()
                    .py_3()
                    .rounded_lg()
                    .bg(rgb(0x4f7cff))
                    .cursor_pointer()
                    .child(action)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.start_scan(window, cx);
                    })),
            )
            .child(div().text_color(rgb(0xa9afb8)).child(
                "Read-only Windows scan flow: Smart Scan uses the shared provider set; passing a directory keeps the disposable manual smoke-test mode.",
            ))
    }
}

fn main() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(900.0), px(560.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| WindowsSpike::new()),
        )
        .expect("open Windows GPUI spike window");
        cx.activate(true);
    });
}
