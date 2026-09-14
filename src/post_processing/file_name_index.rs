use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock, mpsc};
use std::time::{Duration, Instant};

use ignore::WalkBuilder;
use notify_debouncer_full::notify::{
    Config, EventKind, RecommendedWatcher, RecursiveMode, event::ModifyKind,
};
use notify_debouncer_full::{DebounceEventResult, NoCache, new_debouncer_opt};
use serde::Serialize;

use super::{file_name_matcher::FileNames, supported_file_types::is_supported_extension};

#[derive(Clone, Default, Serialize)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub enum IndexStatus {
    #[default]
    Disabled,
    Indexing,
    Ready,
    Error(String),
}

#[derive(Default)]
struct IndexState {
    generation: u64,
    root: Option<PathBuf>,
    status: IndexStatus,
    snapshot: Option<Arc<FileNames>>,
}

struct Worker {
    cancelled: Arc<AtomicBool>,
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

#[derive(Default)]
pub struct FileNameIndex {
    state: Arc<RwLock<IndexState>>,
    worker: Mutex<Option<Worker>>,
}

impl FileNameIndex {
    pub fn configure(&self, root: Option<PathBuf>) {
        let mut worker = self.worker.lock().unwrap();
        worker.take();
        let generation = {
            let mut state = self.state.write().unwrap();
            state.generation += 1;
            state.root = root.clone();
            state.snapshot = None;
            state.status = if root.is_some() {
                IndexStatus::Indexing
            } else {
                IndexStatus::Disabled
            };
            state.generation
        };
        if let Some(root) = root {
            *worker = Some(self.start_worker(root, generation));
        } else {
            log::debug!("filename index disabled");
        }
    }

    pub fn status(&self) -> IndexStatus {
        self.state.read().unwrap().status.clone()
    }

    pub(super) fn snapshot(&self) -> Option<Arc<FileNames>> {
        self.state.read().unwrap().snapshot.clone()
    }

    pub(super) fn retry_after_error(&self) {
        let mut worker = self.worker.lock().unwrap();
        let mut state = self.state.write().unwrap();
        if !matches!(state.status, IndexStatus::Error(_)) {
            return;
        }
        let Some(root) = state.root.clone() else {
            return;
        };
        worker.take();
        state.generation += 1;
        state.status = IndexStatus::Indexing;
        let generation = state.generation;
        drop(state);
        log::debug!("retrying filename index after error");
        *worker = Some(self.start_worker(root, generation));
    }

    fn start_worker(&self, root: PathBuf, generation: u64) -> Worker {
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancel = cancelled.clone();
        let shared = self.state.clone();
        let spawn = std::thread::Builder::new()
            .name("filename-index".into())
            .spawn(move || {
                if let Err(message) = watch_files(&root, &cancel, &shared, generation) {
                    if !cancel.load(Ordering::Relaxed) {
                        log::error!("filename index: {message}");
                        publish(&shared, generation, IndexStatus::Error(message), None);
                    }
                }
            });
        if let Err(error) = spawn {
            publish(
                &self.state,
                generation,
                IndexStatus::Error(format!("Unable to start filename index worker: {error}")),
                None,
            );
        }
        Worker { cancelled }
    }
}

fn publish(
    shared: &RwLock<IndexState>,
    generation: u64,
    status: IndexStatus,
    snapshot: Option<Arc<FileNames>>,
) {
    let mut state = shared.write().unwrap();
    if state.generation == generation {
        state.status = status;
        state.snapshot = snapshot;
    }
}

fn walker(root: &Path) -> WalkBuilder {
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(false)
        .follow_links(false)
        .filter_entry(|entry| entry.file_name() != ".git");
    builder
}

fn watch_files(
    root: &Path,
    cancelled: &AtomicBool,
    shared: &RwLock<IndexState>,
    generation: u64,
) -> Result<(), String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("Unable to access directory {}: {e}", root.display()))?;
    if !root.is_dir() {
        return Err(format!(
            "Selected path is not a directory: {}",
            root.display()
        ));
    }
    let (sender, receiver) = mpsc::channel();
    let mut watcher = new_debouncer_opt::<_, RecommendedWatcher, NoCache>(
        Duration::from_secs(1),
        None,
        move |events: DebounceEventResult| {
            let _ = sender.send(events);
        },
        NoCache::new(),
        Config::default(),
    )
    .map_err(|e| format!("Unable to initialize directory notifications: {e}"))?;
    watcher
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|e| format!("Unable to watch {}: {e}", root.display()))?;
    // Watch the parent for moves/deletion of the selected root itself.
    if let Some(parent) = root.parent() {
        watcher
            .watch(parent, RecursiveMode::NonRecursive)
            .map_err(|e| format!("Unable to watch parent {}: {e}", parent.display()))?;
    }
    for ancestor in root.ancestors().skip(1) {
        for name in [".gitignore", ".ignore", ".git/info/exclude"] {
            let path = ancestor.join(name);
            if path.is_file() {
                watcher
                    .watch(&path, RecursiveMode::NonRecursive)
                    .map_err(|e| format!("Unable to watch ignore file {}: {e}", path.display()))?;
            }
        }
    }
    let mut watched_ignore_parents = BTreeSet::new();
    watch_global_ignore(&mut watcher, &mut watched_ignore_parents)?;
    let mut paths = HashMap::new();
    let mut matcher = walker(&root).build_matchers().remove(0);
    scan_into(&root, &mut paths, cancelled)?;
    loop {
        if cancelled.load(Ordering::Relaxed) {
            return Ok(());
        }
        while let Ok(events) = receiver.try_recv() {
            update_paths(
                events,
                &root,
                &mut paths,
                &mut matcher,
                cancelled,
                shared,
                generation,
            )?;
        }
        watch_global_ignore(&mut watcher, &mut watched_ignore_parents)?;
        let names = paths.values().cloned().collect::<BTreeSet<_>>();
        log::debug!(
            "filename index ready: files={}, distinct_names={}",
            paths.len(),
            names.len()
        );
        publish(
            shared,
            generation,
            IndexStatus::Ready,
            Some(Arc::new(FileNames::new(names))),
        );
        loop {
            if cancelled.load(Ordering::Relaxed) {
                return Ok(());
            }
            match receiver.recv_timeout(Duration::from_millis(250)) {
                Ok(events) => {
                    if update_paths(
                        events,
                        &root,
                        &mut paths,
                        &mut matcher,
                        cancelled,
                        shared,
                        generation,
                    )? {
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => return Err("Directory notification stream stopped unexpectedly".into()),
            }
        }
    }
}

fn watch_global_ignore(
    watcher: &mut notify_debouncer_full::Debouncer<RecommendedWatcher, NoCache>,
    watched: &mut BTreeSet<PathBuf>,
) -> Result<(), String> {
    if let Some(path) = ignore::gitignore::gitconfig_excludes_path() {
        if let Some(parent) = path.ancestors().skip(1).find(|parent| parent.is_dir()) {
            if !watched.contains(parent) {
                watcher
                    .watch(parent, RecursiveMode::NonRecursive)
                    .map_err(|e| {
                        format!(
                            "Unable to watch global ignore directory {}: {e}",
                            parent.display()
                        )
                    })?;
                watched.insert(parent.to_owned());
            }
        }
    }
    Ok(())
}

fn update_paths(
    events: DebounceEventResult,
    root: &Path,
    paths: &mut HashMap<PathBuf, String>,
    matcher: &mut ignore::IncrementalIgnore,
    cancelled: &AtomicBool,
    shared: &RwLock<IndexState>,
    generation: u64,
) -> Result<bool, String> {
    let events = events.map_err(|errors| {
        errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    if !root.is_dir() {
        return Err(format!("Directory does not exist: {}", root.display()));
    }
    let global_ignore = ignore::gitignore::gitconfig_excludes_path();
    let rebuild = events.iter().any(|event| {
        event.need_rescan()
            || event.paths.iter().any(|p| {
                global_ignore
                    .as_ref()
                    .is_some_and(|global| global.starts_with(p))
                    || matches!(
                        p.file_name().and_then(|n| n.to_str()),
                        Some(".gitignore" | ".ignore" | "exclude" | ".gitconfig")
                    )
            })
    });
    if rebuild {
        log::debug!("rebuilding filename index after ignore-rule change or lost notifications");
        publish(shared, generation, IndexStatus::Indexing, None);
        paths.clear();
        *matcher = walker(root).build_matchers().remove(0);
        scan_into(root, paths, cancelled)?;
        return Ok(true);
    }
    let mut affected = BTreeSet::new();
    for event in events {
        if matches!(
            event.kind,
            EventKind::Access(_) | EventKind::Modify(ModifyKind::Data(_) | ModifyKind::Metadata(_))
        ) {
            continue;
        }
        for path in &event.paths {
            if path.starts_with(root) {
                affected.insert(path.clone());
            }
        }
    }
    // Directory moves/replacements can invalidate cached ignore rules.
    if !affected.is_empty() {
        *matcher = walker(root).build_matchers().remove(0);
    }
    let mut changed = false;
    let mut refreshed: Vec<PathBuf> = Vec::new();
    for path in affected {
        if cancelled.load(Ordering::Relaxed) {
            return Ok(false);
        }
        if refreshed.iter().any(|parent| path.starts_with(parent)) {
            continue;
        }
        let relative = path.strip_prefix(root).unwrap();
        if relative.components().any(|c| c.as_os_str() == ".git") {
            continue;
        }
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let before = paths.len();
                paths.retain(|existing, _| !existing.starts_with(&path));
                changed |= before != paths.len();
                continue;
            }
            Err(e) => return Err(format!("Unable to inspect {}: {e}", path.display())),
        };
        let before = paths.len();
        paths.retain(|existing, _| !existing.starts_with(&path));
        changed |= before != paths.len();
        if metadata.is_symlink() {
            continue;
        }
        let mut ancestor = path.parent();
        let mut linked = false;
        while let Some(parent) =
            ancestor.filter(|parent| parent.starts_with(root) && *parent != root)
        {
            match std::fs::symlink_metadata(parent) {
                Ok(metadata) if metadata.is_symlink() => {
                    linked = true;
                    break;
                }
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    linked = true;
                    break;
                }
                Err(e) => return Err(format!("Unable to inspect {}: {e}", parent.display())),
            }
            ancestor = parent.parent();
        }
        if linked {
            continue;
        }
        let (matched, error) = matcher.matched_with_errors(relative, metadata.is_dir());
        if let Some(error) = error {
            return Err(format!(
                "Unable to apply ignore rules for {}: {error}",
                path.display()
            ));
        }
        if matched.is_ignore() {
            continue;
        }
        let before = paths.len();
        scan_into(&path, paths, cancelled)?;
        changed |= before != paths.len();
        refreshed.push(path);
    }
    log::debug!(
        "filename index updated: refreshed_paths={}",
        refreshed.len()
    );
    Ok(changed)
}

fn scan_into(
    root: &Path,
    paths: &mut HashMap<PathBuf, String>,
    cancelled: &AtomicBool,
) -> Result<(), String> {
    let started = Instant::now();
    for entry in walker(root).build() {
        if cancelled.load(Ordering::Relaxed) {
            return Ok(());
        }
        let entry = entry.map_err(|e| format!("Indexing incomplete at {}: {e}", root.display()))?;
        if let Some(error) = entry.error() {
            return Err(format!(
                "Unable to apply ignore rules at {}: {error}",
                entry.path().display()
            ));
        }
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        let path = entry.path();
        if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(is_supported_extension)
        {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                paths.insert(path.to_owned(), name.to_owned());
            }
        }
    }
    log::debug!(
        "filename scan completed: duration_ms={}",
        started.elapsed().as_millis()
    );
    Ok(())
}
