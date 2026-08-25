use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::{CleanupCategory, ScanItem, ScanSummary, safety::is_protected_broad_root};

#[derive(Debug, Clone)]
pub struct ScanRequest {
    pub category: CleanupCategory,
    pub roots: Vec<PathBuf>,
    pub excluded_roots: Vec<PathBuf>,
}

impl ScanRequest {
    fn validate(&self) -> io::Result<()> {
        if let Some(root) = self
            .roots
            .iter()
            .find(|root| is_protected_broad_root(root.as_path()))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("refusing to scan protected broad root: {}", root.display()),
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanEvent {
    Started {
        category: CleanupCategory,
        root_count: usize,
    },
    ItemFound {
        item: ScanItem,
    },
    PermissionDenied {
        path: PathBuf,
    },
    Finished {
        summary: ScanSummary,
    },
    Cancelled {
        summary: ScanSummary,
    },
}

pub trait ScanEventSink {
    fn emit(&mut self, event: ScanEvent);
}

impl<F> ScanEventSink for F
where
    F: FnMut(ScanEvent),
{
    fn emit(&mut self, event: ScanEvent) {
        self(event);
    }
}

#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

pub trait Scanner {
    fn scan(&self, request: &ScanRequest) -> io::Result<Vec<ScanItem>> {
        let cancellation = CancellationToken::new();
        let mut sink = |_: ScanEvent| {};
        self.scan_with(request, &cancellation, &mut sink)
    }

    fn scan_with(
        &self,
        request: &ScanRequest,
        cancellation: &CancellationToken,
        sink: &mut dyn ScanEventSink,
    ) -> io::Result<Vec<ScanItem>>;
}

#[derive(Debug, Default)]
pub struct FileSystemScanner;

impl FileSystemScanner {
    fn walk(
        root: PathBuf,
        request: &ScanRequest,
        cancellation: &CancellationToken,
        sink: &mut dyn ScanEventSink,
        out: &mut Vec<ScanItem>,
    ) -> io::Result<bool> {
        if cancellation.is_cancelled() {
            return Ok(false);
        }

        if is_excluded(&root, &request.excluded_roots) {
            return Ok(true);
        }

        let metadata = match fs::symlink_metadata(&root) {
            Ok(metadata) => metadata,
            Err(error) => return handle_optional_path_error(&root, error, sink),
        };
        let file_type = metadata.file_type();

        if file_type.is_symlink() {
            Self::push_item(
                ScanItem {
                    path: root,
                    category: request.category,
                    bytes: 0,
                    is_symlink: true,
                },
                sink,
                out,
            );
            return Ok(true);
        }

        if metadata.is_file() {
            Self::push_item(
                ScanItem {
                    path: root,
                    category: request.category,
                    bytes: metadata.len(),
                    is_symlink: false,
                },
                sink,
                out,
            );
            return Ok(true);
        }

        if metadata.is_dir() {
            let entries = match fs::read_dir(&root) {
                Ok(entries) => entries,
                Err(error) => return handle_optional_path_error(&root, error, sink),
            };

            for entry in entries {
                if cancellation.is_cancelled() {
                    return Ok(false);
                }

                let entry = match entry {
                    Ok(entry) => entry,
                    Err(error) => {
                        if handle_optional_path_error(&root, error, sink)? {
                            continue;
                        }
                        return Ok(false);
                    }
                };

                if !Self::walk(entry.path(), request, cancellation, sink, out)? {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    fn push_item(item: ScanItem, sink: &mut dyn ScanEventSink, out: &mut Vec<ScanItem>) {
        sink.emit(ScanEvent::ItemFound { item: item.clone() });
        out.push(item);
    }
}

impl Scanner for FileSystemScanner {
    fn scan_with(
        &self,
        request: &ScanRequest,
        cancellation: &CancellationToken,
        sink: &mut dyn ScanEventSink,
    ) -> io::Result<Vec<ScanItem>> {
        request.validate()?;

        sink.emit(ScanEvent::Started {
            category: request.category,
            root_count: request.roots.len(),
        });

        let mut items = Vec::new();
        let mut completed = true;

        for root in &request.roots {
            if cancellation.is_cancelled() {
                completed = false;
                break;
            }

            if !Self::walk(root.clone(), request, cancellation, sink, &mut items)? {
                completed = false;
                break;
            }
        }

        let summary = ScanSummary::from_items(&items);
        if completed && !cancellation.is_cancelled() {
            sink.emit(ScanEvent::Finished { summary });
        } else {
            sink.emit(ScanEvent::Cancelled { summary });
        }

        Ok(items)
    }
}

fn handle_optional_path_error(
    path: &Path,
    error: io::Error,
    sink: &mut dyn ScanEventSink,
) -> io::Result<bool> {
    match error.kind() {
        io::ErrorKind::NotFound => Ok(true),
        io::ErrorKind::PermissionDenied => {
            sink.emit(ScanEvent::PermissionDenied {
                path: path.to_path_buf(),
            });
            Ok(true)
        }
        _ => Err(error),
    }
}

fn is_excluded(path: &Path, excluded_roots: &[PathBuf]) -> bool {
    excluded_roots
        .iter()
        .any(|excluded| path.starts_with(excluded))
}

#[cfg(test)]
mod tests {
    use std::{fs, time::SystemTime};

    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        std::env::temp_dir().join(format!("cleaner-core-{name}-{nonce}"))
    }

    fn request(root: PathBuf) -> ScanRequest {
        ScanRequest {
            category: CleanupCategory::UserCache,
            roots: vec![root],
            excluded_roots: Vec::new(),
        }
    }

    #[cfg(windows)]
    fn protected_scan_root() -> PathBuf {
        PathBuf::from(r"C:\Windows")
    }

    #[cfg(not(windows))]
    fn protected_scan_root() -> PathBuf {
        PathBuf::from("/System")
    }

    #[test]
    fn scans_regular_files_and_emits_events() {
        let root = temp_root("events");
        fs::create_dir_all(&root).expect("create temp dir");
        fs::write(root.join("cache.bin"), [1_u8, 2, 3, 4]).expect("write fixture");

        let scanner = FileSystemScanner;
        let cancellation = CancellationToken::new();
        let mut events = Vec::new();
        let mut sink = |event| events.push(event);
        let items = scanner
            .scan_with(&request(root.clone()), &cancellation, &mut sink)
            .expect("scan succeeds");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].bytes, 4);
        assert!(matches!(events.first(), Some(ScanEvent::Started { .. })));
        assert!(
            events
                .iter()
                .any(|event| matches!(event, ScanEvent::ItemFound { .. }))
        );
        assert!(matches!(events.last(), Some(ScanEvent::Finished { .. })));

        fs::remove_dir_all(root).expect("remove temp dir");
    }

    #[test]
    fn ignores_missing_optional_roots() {
        let root = temp_root("missing");
        let scanner = FileSystemScanner;
        let cancellation = CancellationToken::new();
        let mut events = Vec::new();
        let mut sink = |event| events.push(event);
        let items = scanner
            .scan_with(&request(root), &cancellation, &mut sink)
            .expect("missing root is optional");

        assert!(items.is_empty());
        assert!(matches!(events.first(), Some(ScanEvent::Started { .. })));
        assert!(matches!(events.last(), Some(ScanEvent::Finished { .. })));
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, ScanEvent::PermissionDenied { .. }))
        );
    }

    #[test]
    fn skips_excluded_roots() {
        let root = temp_root("excluded");
        let keep = root.join("keep");
        let excluded = root.join("excluded");
        fs::create_dir_all(&keep).expect("create keep dir");
        fs::create_dir_all(&excluded).expect("create excluded dir");
        fs::write(keep.join("keep.bin"), [1_u8]).expect("write keep fixture");
        fs::write(excluded.join("skip.bin"), [2_u8]).expect("write excluded fixture");

        let scanner = FileSystemScanner;
        let items = scanner
            .scan(&ScanRequest {
                category: CleanupCategory::UserCache,
                roots: vec![root.clone()],
                excluded_roots: vec![excluded],
            })
            .expect("scan succeeds");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].path, keep.join("keep.bin"));

        fs::remove_dir_all(root).expect("remove temp dir");
    }

    #[test]
    fn rejects_protected_broad_scan_root() {
        let scanner = FileSystemScanner;
        let error = scanner
            .scan(&request(protected_scan_root()))
            .expect_err("protected broad root must be rejected");

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn reports_permission_denied_from_error_seam() {
        let path = PathBuf::from("/fixture/denied");
        let mut events = Vec::new();
        let mut sink = |event| events.push(event);

        let handled = handle_optional_path_error(
            &path,
            io::Error::from(io::ErrorKind::PermissionDenied),
            &mut sink,
        )
        .expect("permission denied is handled");

        assert!(handled);
        assert_eq!(events, vec![ScanEvent::PermissionDenied { path }]);
    }

    #[test]
    fn honours_pre_cancelled_token() {
        let root = temp_root("cancelled");
        fs::create_dir_all(&root).expect("create temp dir");
        fs::write(root.join("cache.bin"), [1_u8]).expect("write fixture");

        let scanner = FileSystemScanner;
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let mut events = Vec::new();
        let mut sink = |event| events.push(event);
        let items = scanner
            .scan_with(&request(root.clone()), &cancellation, &mut sink)
            .expect("scan succeeds");

        assert!(items.is_empty());
        assert!(matches!(events.first(), Some(ScanEvent::Started { .. })));
        assert!(matches!(events.last(), Some(ScanEvent::Cancelled { .. })));

        fs::remove_dir_all(root).expect("remove temp dir");
    }

    #[cfg(unix)]
    #[test]
    fn reports_symlink_without_traversing_it() {
        use std::os::unix::fs::symlink;

        let root = temp_root("symlink");
        let target = root.join("target");
        fs::create_dir_all(&target).expect("create target dir");
        fs::write(target.join("inside.bin"), [1_u8, 2]).expect("write target fixture");
        symlink(&target, root.join("link")).expect("create symlink");

        let scanner = FileSystemScanner;
        let items = scanner.scan(&request(root.clone())).expect("scan succeeds");

        assert_eq!(items.len(), 2);
        assert_eq!(items.iter().filter(|item| item.is_symlink).count(), 1);

        fs::remove_dir_all(root).expect("remove temp dir");
    }
}
