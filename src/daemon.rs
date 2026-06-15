use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, UNIX_EPOCH};

use anyhow::Result;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use parking_lot::Mutex;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Notify;
use tracing::{error, info, warn};

use crate::config;
use crate::embedding::{EmbeddingModel, create_model};
use crate::indexer::{
    index_workspace, index_workspace_for_watcher, index_workspace_paths_for_watcher,
    remove_workspace_index,
};
use crate::jobs::{self, JobKind, JobUpdate};
use crate::protocol::{
    BUILD_VERSION, DAEMON_PROTOCOL_VERSION, DaemonRequest, DaemonRequestEnvelope, DaemonResponse,
};
use crate::regex_search::regex_search;
use crate::search::{
    SearchContext, SearchOptions, hybrid_search_with_context, literal_search_with_context,
};
use crate::workspace::{Workspace, WorkspaceScope, list_workspaces};

const WATCH_QUIET_PERIOD: Duration = Duration::from_secs(2);
const WATCH_MAX_DEBOUNCE: Duration = Duration::from_secs(30);
const MAX_DAEMON_LOG_BYTES: u64 = 10 * 1024 * 1024;
const MAX_DAEMON_REQUEST_BYTES: usize = 1024 * 1024;
const MAX_QUERY_CACHE_ENTRIES: usize = 128;
/// Cap on cached workspace/dimension keys. Idle contexts additionally share a
/// global retention cap, keeping open SQLite/Tantivy/vector views bounded when
/// `--all` searches touch many workspaces.
const MAX_SEARCH_CONTEXTS: usize = 32;
/// Retain a small number of read-only contexts per workspace/dimension so
/// concurrent queries do not serialize on a single SQLite/Tantivy/vector view.
/// In-flight contexts are additionally bounded by `cpu_permits`.
const MAX_IDLE_SEARCH_CONTEXTS_PER_KEY: usize = 4;
/// Preserve the original worst-case retained context bound across all pools.
const MAX_IDLE_SEARCH_CONTEXTS: usize = 32;
/// Don't cache result sets larger than this (each hit carries preview/reason
/// strings; large `--no-limit` results would bloat the query cache).
const MAX_CACHEABLE_HITS: usize = 2_000;

struct WatchRegistration {
    _watcher: RecommendedWatcher,
    control: Arc<WatchControl>,
}

#[derive(Clone)]
struct WatchEventFilter {
    root: PathBuf,
    skip_gitignore: bool,
    root_gitignore: Option<ignore::gitignore::Gitignore>,
}

struct WatchControl {
    workspace: Workspace,
    notify: Notify,
    dirty: AtomicBool,
    indexing: AtomicBool,
    active: AtomicBool,
    pending_events: AtomicU64,
    coalesced_events: AtomicU64,
    dirty_paths: Mutex<HashSet<PathBuf>>,
}

impl WatchControl {
    fn new(workspace: Workspace) -> Self {
        Self {
            workspace,
            notify: Notify::new(),
            dirty: AtomicBool::new(false),
            indexing: AtomicBool::new(false),
            active: AtomicBool::new(true),
            pending_events: AtomicU64::new(0),
            coalesced_events: AtomicU64::new(0),
            dirty_paths: Mutex::new(HashSet::new()),
        }
    }

    fn mark_dirty(&self, paths: impl IntoIterator<Item = PathBuf>) {
        self.dirty_paths.lock().extend(paths);
        self.dirty.store(true, Ordering::Relaxed);
        self.pending_events.fetch_add(1, Ordering::Relaxed);
        self.notify.notify_one();
    }

    fn take_dirty_paths(&self) -> Vec<PathBuf> {
        self.dirty_paths.lock().drain().collect()
    }

    fn snapshot_phase(&self) -> (&'static str, bool, bool, u64, u64) {
        let indexing = self.indexing.load(Ordering::Relaxed);
        let dirty = self.dirty.load(Ordering::Relaxed);
        let pending_events = self.pending_events.load(Ordering::Relaxed);
        let coalesced_events = self.coalesced_events.load(Ordering::Relaxed);
        let phase = if indexing {
            "indexing"
        } else if dirty {
            "dirty"
        } else {
            "idle"
        };
        (phase, indexing, dirty, pending_events, coalesced_events)
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct SearchContextCacheKey {
    workspace_id: String,
    emb_dim: Option<usize>,
    wants_neural: bool,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct SearchContextSignature {
    index_generation: Option<u64>,
    sqlite: Option<FileStamp>,
    tantivy: Option<DirStamp>,
    hash_vectors: Option<FileStamp>,
    neural_vectors: Option<FileStamp>,
    base_sqlite: Option<FileStamp>,
    base_tantivy: Option<DirStamp>,
    base_hash_vectors: Option<FileStamp>,
    base_neural_vectors: Option<FileStamp>,
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
struct FileStamp {
    len: u64,
    modified_nanos: u128,
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
struct DirStamp {
    files: u64,
    len: u64,
    newest_modified_nanos: u128,
}

struct CachedSearchContext {
    signature: SearchContextSignature,
    pool: Arc<SearchContextPool>,
}

struct SearchContextPool {
    idle: Mutex<Vec<SearchContext>>,
    idle_context_count: Arc<AtomicUsize>,
}

impl SearchContextPool {
    fn take_idle(&self) -> Option<SearchContext> {
        let context = self.idle.lock().pop();
        if context.is_some() {
            self.idle_context_count.fetch_sub(1, Ordering::Relaxed);
        }
        context
    }

    fn retain_idle(&self, context: SearchContext) {
        let mut idle = self.idle.lock();
        if idle.len() >= MAX_IDLE_SEARCH_CONTEXTS_PER_KEY {
            return;
        }
        if self
            .idle_context_count
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
                (count < MAX_IDLE_SEARCH_CONTEXTS).then_some(count + 1)
            })
            .is_ok()
        {
            idle.push(context);
        }
    }
}

impl Drop for SearchContextPool {
    fn drop(&mut self) {
        let retained = self.idle.lock().len();
        if retained > 0 {
            self.idle_context_count
                .fetch_sub(retained, Ordering::Relaxed);
        }
    }
}

struct SearchContextLease {
    context: Option<SearchContext>,
    pool: Arc<SearchContextPool>,
}

impl std::ops::Deref for SearchContextLease {
    type Target = SearchContext;

    fn deref(&self) -> &Self::Target {
        self.context.as_ref().expect("search context lease is live")
    }
}

impl Drop for SearchContextLease {
    fn drop(&mut self) {
        let Some(context) = self.context.take() else {
            return;
        };
        self.pool.retain_idle(context);
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct QueryCacheKey {
    workspace_ids: Vec<String>,
    signatures: Vec<SearchContextSignature>,
    all_indices: bool,
    query: String,
    limit: Option<usize>,
    context: usize,
    type_filter: Option<String>,
    include_globs: Vec<String>,
    exclude_globs: Vec<String>,
    scope_filter: Option<WorkspaceScope>,
    skip_gitignore: bool,
    emb_dim: usize,
    wants_neural: bool,
}

#[derive(Default)]
struct QueryResultCache {
    results: HashMap<QueryCacheKey, Vec<crate::protocol::SearchHit>>,
    order: VecDeque<QueryCacheKey>,
}

impl QueryResultCache {
    fn get(&self, key: &QueryCacheKey) -> Option<Vec<crate::protocol::SearchHit>> {
        self.results.get(key).cloned()
    }

    fn insert(&mut self, key: QueryCacheKey, hits: Vec<crate::protocol::SearchHit>) {
        if !self.results.contains_key(&key) {
            self.order.push_back(key.clone());
        }
        self.results.insert(key, hits);

        while self.results.len() > MAX_QUERY_CACHE_ENTRIES {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            self.results.remove(&oldest);
        }
    }

    fn clear(&mut self) {
        self.results.clear();
        self.order.clear();
    }
}

impl WatchEventFilter {
    fn new(workspace: &Workspace) -> Self {
        let skip_gitignore = workspace
            .read_metadata()
            .ok()
            .flatten()
            .is_some_and(|metadata| metadata.skip_gitignore);
        let root_gitignore = (!skip_gitignore)
            .then(|| build_root_gitignore(&workspace.root))
            .flatten();

        Self {
            root: workspace.root.clone(),
            skip_gitignore,
            root_gitignore,
        }
    }

    fn paths_to_reindex(&self, event: &notify::Event) -> Vec<PathBuf> {
        event
            .paths
            .iter()
            .filter(|path| self.path_should_reindex(path))
            .filter_map(|path| self.normalize_watch_path(path).map(|(_, rel)| rel))
            .collect()
    }

    fn path_should_reindex(&self, path: &Path) -> bool {
        let Some((normalized_path, rel)) = self.normalize_watch_path(path) else {
            return false;
        };
        if rel.as_os_str().is_empty() || is_always_ignored_watch_path(&rel) {
            return false;
        }

        if !self.skip_gitignore {
            if is_common_build_output_path(&rel) {
                return false;
            }

            if let Some(gitignore) = &self.root_gitignore
                && gitignore
                    .matched_path_or_any_parents(&normalized_path, normalized_path.is_dir())
                    .is_ignore()
            {
                return false;
            }
        }

        true
    }

    fn normalize_watch_path(&self, path: &Path) -> Option<(PathBuf, PathBuf)> {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        };

        if let Ok(rel) = absolute.strip_prefix(&self.root) {
            return Some((absolute.clone(), rel.to_path_buf()));
        }

        let normalized = canonicalize_existing_prefix(&absolute)?;
        let rel = normalized.strip_prefix(&self.root).ok()?.to_path_buf();
        Some((normalized, rel))
    }
}

fn canonicalize_existing_prefix(path: &Path) -> Option<PathBuf> {
    let mut cursor = path;
    let mut missing = Vec::<OsString>::new();

    loop {
        if let Ok(mut normalized) = cursor.canonicalize() {
            for component in missing.iter().rev() {
                normalized.push(component);
            }
            return Some(normalized);
        }

        missing.push(cursor.file_name()?.to_os_string());
        cursor = cursor.parent()?;
    }
}

#[derive(Clone)]
struct DaemonState {
    lazy_model: Arc<std::sync::OnceLock<Arc<dyn EmbeddingModel>>>,
    model_loading: Arc<AtomicBool>,
    watchers: Arc<Mutex<HashMap<String, WatchRegistration>>>,
    search_contexts: Arc<Mutex<HashMap<SearchContextCacheKey, CachedSearchContext>>>,
    idle_search_context_count: Arc<AtomicUsize>,
    query_results: Arc<Mutex<QueryResultCache>>,
    /// Bounds concurrent CPU-heavy work (hybrid/literal/regex search + index).
    /// Without this, a burst of clients each spawn a `spawn_blocking` task on
    /// Tokio's blocking pool (default cap 512), oversubscribing CPU and memory
    /// with no backpressure. See #58.
    cpu_permits: Arc<tokio::sync::Semaphore>,
}

impl DaemonState {
    /// Try to get the neural model without blocking. If it is not loaded yet,
    /// return a fast hash-based model so searches don't stall during startup.
    fn get_model_or_fallback(&self) -> Arc<dyn EmbeddingModel> {
        match self.lazy_model.get() {
            Some(model) => model.clone(),
            None => cached_hash_model(),
        }
    }

    fn maybe_start_model_load(&self) {
        if self.lazy_model.get().is_some() || self.model_loading.swap(true, Ordering::Relaxed) {
            return;
        }

        let lazy = self.lazy_model.clone();
        let loading = self.model_loading.clone();
        std::thread::spawn(move || {
            daemon_log("loading embedding model...");
            lazy.get_or_init(|| Arc::from(create_model(false)));
            loading.store(false, Ordering::Relaxed);
            daemon_log("embedding model ready");
        });
    }

    fn cached_search_context(
        &self,
        workspace: &Workspace,
        emb_dim: Option<usize>,
        wants_neural: bool,
    ) -> Result<SearchContextLease> {
        let key = SearchContextCacheKey {
            workspace_id: workspace.id.clone(),
            emb_dim,
            wants_neural,
        };
        let signature = search_context_signature(workspace, emb_dim, wants_neural);

        let pool = {
            let mut cache = self.search_contexts.lock();
            if let Some(entry) = cache.get(&key)
                && entry.signature == signature
            {
                entry.pool.clone()
            } else {
                // Bound cached workspace/dimension keys; each key retains only
                // MAX_IDLE_SEARCH_CONTEXTS_PER_KEY contexts after a burst.
                if cache.len() >= MAX_SEARCH_CONTEXTS
                    && !cache.contains_key(&key)
                    && let Some(victim) = cache.keys().find(|k| **k != key).cloned()
                {
                    cache.remove(&victim);
                }
                let pool = Arc::new(SearchContextPool {
                    idle: Mutex::new(Vec::new()),
                    idle_context_count: self.idle_search_context_count.clone(),
                });
                cache.insert(
                    key,
                    CachedSearchContext {
                        signature,
                        pool: pool.clone(),
                    },
                );
                pool
            }
        };

        let context = pool
            .take_idle()
            .map(Ok)
            .unwrap_or_else(|| SearchContext::load(workspace, emb_dim, wants_neural))?;
        Ok(SearchContextLease {
            context: Some(context),
            pool,
        })
    }

    fn clear_workspace_contexts(&self, workspace: &Workspace) {
        self.search_contexts
            .lock()
            .retain(|key, _| key.workspace_id != workspace.id);
        self.query_results.lock().clear();
    }

    fn cached_query_results(&self, key: &QueryCacheKey) -> Option<Vec<crate::protocol::SearchHit>> {
        self.query_results.lock().get(key)
    }

    fn store_query_results(&self, key: QueryCacheKey, hits: &[crate::protocol::SearchHit]) {
        // Don't cache very large result sets (e.g. --no-limit / file_name_only
        // on a big repo): with up to MAX_QUERY_CACHE_ENTRIES of them, each
        // carrying preview/reason strings, this would bloat daemon memory.
        if hits.len() > MAX_CACHEABLE_HITS {
            return;
        }
        self.query_results.lock().insert(key, hits.to_vec());
    }
}

pub async fn run_daemon() -> Result<()> {
    config::ensure_app_dirs()?;

    // Single-instance guard: acquire an exclusive lock before binding the
    // socket so two daemons can't both bind and steal it from each other
    // (which would leave the loser a zombie still holding file watchers).
    // Held for the daemon's lifetime.
    let _daemon_lock = match crate::ipc::acquire_daemon_lock()? {
        Some(file) => file,
        None => {
            daemon_log("another ivygrep daemon is already running; exiting");
            return Ok(());
        }
    };

    let (listener, socket_path) = crate::ipc::bind().await?;
    daemon_log(&format!(
        "ivygrep daemon listening on {}",
        socket_path.display()
    ));

    // Defer model creation; model artifact download happens on first neural use.
    let lazy_model: Arc<std::sync::OnceLock<Arc<dyn EmbeddingModel>>> =
        Arc::new(std::sync::OnceLock::new());

    let state = DaemonState {
        lazy_model: lazy_model.clone(),
        model_loading: Arc::new(AtomicBool::new(false)),
        watchers: Arc::new(Mutex::new(HashMap::new())),
        search_contexts: Arc::new(Mutex::new(HashMap::new())),
        idle_search_context_count: Arc::new(AtomicUsize::new(0)),
        query_results: Arc::new(Mutex::new(QueryResultCache::default())),
        cpu_permits: Arc::new(tokio::sync::Semaphore::new(num_cpus::get().max(1))),
    };

    restore_configured_watchers(&state);

    // Graceful shutdown on SIGTERM/SIGINT (e.g. service stop): stop watchers
    // and remove the socket before exiting, instead of leaving them dangling.
    #[cfg(unix)]
    {
        let shutdown_state = state.clone();
        tokio::spawn(async move {
            use tokio::signal::unix::{SignalKind, signal};
            let mut term = match signal(SignalKind::terminate()) {
                Ok(s) => s,
                Err(_) => return,
            };
            let mut int = match signal(SignalKind::interrupt()) {
                Ok(s) => s,
                Err(_) => return,
            };
            tokio::select! {
                _ = term.recv() => {}
                _ = int.recv() => {}
            }
            daemon_log("received shutdown signal; stopping watchers and cleaning up");
            stop_all_watchers(&shutdown_state);
            crate::ipc::cleanup_socket();
            std::process::exit(0);
        });
    }

    info!("ivygrep daemon listening on {}", socket_path.display());

    loop {
        let (stream, _addr) = match listener.accept().await {
            Ok(pair) => pair,
            Err(err) => {
                // Don't tear down the whole daemon on a transient accept error
                // (e.g. EMFILE under fd pressure); log, back off, and continue.
                warn!("daemon accept error: {err:#}; backing off");
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                continue;
            }
        };

        // The socket exposes cross-workspace search/index/delete; only serve
        // connections from the daemon's own user.
        if !crate::ipc::peer_is_owner(&stream) {
            warn!("rejected daemon connection from a different uid");
            continue;
        }

        let state = state.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_connection(stream, state).await {
                error!("daemon connection error: {err:#}");
            }
        });
    }
}

fn restore_configured_watchers(state: &DaemonState) {
    let workspaces = match list_workspaces() {
        Ok(workspaces) => workspaces,
        Err(err) => {
            warn!("failed to enumerate workspaces for watcher restore: {err:#}");
            return;
        }
    };

    for workspace in workspaces {
        if !workspace.watch_enabled || workspace.last_indexed_at_unix.is_none() {
            continue;
        }

        if let Err(err) = register_watcher(state, &workspace.root) {
            warn!(
                "failed to restore watcher for {}: {err:#}",
                workspace.root.display()
            );
        }
    }
}

fn stop_watcher(workspace: &Workspace, registration: WatchRegistration) {
    registration.control.active.store(false, Ordering::Relaxed);
    registration.control.notify.notify_waiters();
    let _ = jobs::finish_job(workspace, JobKind::Watcher, "stopped", None);
    let _ = std::fs::remove_file(workspace.watcher_pid_path());
}

fn stop_all_watchers(state: &DaemonState) {
    let registrations: Vec<_> = state.watchers.lock().drain().collect();
    for (_, registration) in registrations {
        let workspace = registration.control.workspace.clone();
        stop_watcher(&workspace, registration);
    }
}

async fn handle_connection(stream: crate::ipc::IpcStream, state: DaemonState) -> Result<()> {
    let mut reader = BufReader::new(stream);
    let response = match read_daemon_request(&mut reader).await {
        Ok(Some(request)) => handle_request(state, request).await,
        Ok(None) => return Ok(()),
        Err(response) => response,
    };

    let payload = serde_json::to_vec(&response)?;
    let mut stream = reader.into_inner();
    stream.write_all(&payload).await?;
    stream.write_all(b"\n").await?;

    Ok(())
}

async fn read_daemon_request<R>(
    reader: &mut R,
) -> std::result::Result<Option<DaemonRequest>, DaemonResponse>
where
    R: AsyncBufRead + Unpin,
{
    let mut line = Vec::new();
    let bytes = match (&mut *reader)
        .take((MAX_DAEMON_REQUEST_BYTES as u64) + 1)
        .read_until(b'\n', &mut line)
        .await
    {
        Ok(bytes) => bytes,
        Err(err) => {
            return Err(DaemonResponse::Error {
                message: format!("failed to read daemon request: {err}"),
            });
        }
    };
    if bytes == 0 {
        return Ok(None);
    }
    if line.len() > MAX_DAEMON_REQUEST_BYTES {
        return Err(DaemonResponse::Error {
            message: format!("daemon request exceeds maximum of {MAX_DAEMON_REQUEST_BYTES} bytes"),
        });
    }

    parse_daemon_request(&line).map(Some)
}

fn parse_daemon_request(line: &[u8]) -> std::result::Result<DaemonRequest, DaemonResponse> {
    let envelope: DaemonRequestEnvelope =
        serde_json::from_slice(line).map_err(|err| DaemonResponse::Error {
            message: format!("invalid daemon request: {err}"),
        })?;
    if envelope.protocol_version != DAEMON_PROTOCOL_VERSION {
        return Err(DaemonResponse::Error {
            message: format!(
                "unsupported daemon protocol version {}; expected {DAEMON_PROTOCOL_VERSION}",
                envelope.protocol_version
            ),
        });
    }
    Ok(envelope.request)
}

async fn handle_request(state: DaemonState, request: DaemonRequest) -> DaemonResponse {
    match request {
        DaemonRequest::Status => match list_workspaces() {
            Ok(workspaces) => DaemonResponse::Status {
                workspaces,
                version: Some(BUILD_VERSION.to_string()),
            },
            Err(err) => DaemonResponse::Error {
                message: err.to_string(),
            },
        },
        DaemonRequest::Index {
            path,
            watch,
            skip_gitignore,
        } => {
            let workspace = match Workspace::resolve(&path) {
                Ok(workspace) => workspace,
                Err(err) => {
                    return DaemonResponse::Error {
                        message: err.to_string(),
                    };
                }
            };

            // Respect skip_gitignore by updating metadata before indexing
            let _ = workspace.ensure_dirs();
            let mut meta = workspace
                .read_metadata()
                .unwrap_or(None)
                .unwrap_or_else(|| crate::workspace::WorkspaceMetadata {
                    id: workspace.id.clone(),
                    root: workspace.root.clone(),
                    created_at_unix: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    last_indexed_at_unix: None,
                    watch_enabled: watch,
                    skip_gitignore: false,
                    index_generation: 0,
                });

            if meta.skip_gitignore != skip_gitignore {
                meta.skip_gitignore = skip_gitignore;
            }
            meta.watch_enabled = watch;
            let _ = workspace.write_metadata(&meta);

            // Bound concurrent heavy index work (see #58).
            let permit = state.cpu_permits.clone().acquire_owned().await.ok();
            let index_workspace_target = workspace.clone();
            let index_result = tokio::task::spawn_blocking(move || {
                let _permit = permit;
                let hash_model = cached_hash_model();
                index_workspace(&index_workspace_target, hash_model.as_ref())
            })
            .await
            .unwrap_or_else(|join_err| Err(anyhow::anyhow!(join_err.to_string())));

            match index_result {
                Ok(summary) => {
                    state.clear_workspace_contexts(&workspace);
                    if watch {
                        if let Err(err) = register_watcher(&state, &path) {
                            return DaemonResponse::Error {
                                message: format!("indexed but failed to watch: {err:#}"),
                            };
                        }
                    } else if let Some(registration) = state.watchers.lock().remove(&workspace.id) {
                        stop_watcher(&workspace, registration);
                    }

                    DaemonResponse::Ack {
                        message: format!(
                            "indexed {} files ({} chunks)",
                            summary.indexed_files, summary.total_chunks
                        ),
                    }
                }
                Err(err) => DaemonResponse::Error {
                    message: err.to_string(),
                },
            }
        }
        DaemonRequest::Search {
            path,
            query,
            limit,
            context,
            type_filter,
            include_globs,
            exclude_globs,
            scope_path,
            scope_is_file,
            skip_gitignore,
        } => {
            let state_clone = state.clone();

            let workspaces = if let Some(ref p) = path {
                match Workspace::resolve(p) {
                    Ok(workspace) => vec![workspace],
                    Err(err) => {
                        return DaemonResponse::Error {
                            message: err.to_string(),
                        };
                    }
                }
            } else {
                match list_workspaces() {
                    Ok(ws) => ws
                        .into_iter()
                        .filter(|w| w.last_indexed_at_unix.is_some())
                        .filter_map(|w| Workspace::resolve(&w.root).ok())
                        .collect(),
                    Err(err) => {
                        return DaemonResponse::Error {
                            message: err.to_string(),
                        };
                    }
                }
            };

            let options = SearchOptions {
                limit,
                context,
                type_filter,
                include_globs,
                exclude_globs,
                scope_filter: scope_from_request(scope_path, scope_is_file),
                skip_gitignore,
                progress_tx: None,
                cancel_token: None,
            };
            let all_indices = path.is_none();

            if workspaces.iter().any(Workspace::has_neural_vectors) {
                state_clone.maybe_start_model_load();
            }

            // Bound concurrent heavy search work (see #58). The permit is held
            // for the whole blocking task and released when it completes.
            let permit = state_clone.cpu_permits.clone().acquire_owned().await.ok();
            let result = tokio::task::spawn_blocking(move || {
                let _permit = permit;
                let model = state_clone.get_model_or_fallback();
                let mut all_hits = Vec::new();
                let mut all_errors: Vec<String> = Vec::new();
                let ws_neural_missing: Vec<PathBuf> = workspaces
                    .iter()
                    .filter(|w| w.needs_neural_enhancement())
                    .map(|w| w.root.clone())
                    .collect();

                let cache_key = query_cache_key(
                    &workspaces,
                    &query,
                    &options,
                    model.dimensions(),
                    model.model_identity().is_some(),
                    all_indices,
                );
                if let Some(cached_hits) = state_clone.cached_query_results(&cache_key) {
                    if crate::config::background_enhancement_enabled() {
                        for root in ws_neural_missing {
                            if let Ok(ws) = Workspace::resolve(&root) {
                                let _ = ws.trigger_background_enhancement();
                            }
                        }
                    }
                    return (cached_hits, all_errors);
                }

                for workspace in &workspaces {
                    let context = match state_clone.cached_search_context(
                        workspace,
                        Some(model.dimensions()),
                        model.model_identity().is_some(),
                    ) {
                        Ok(context) => context,
                        Err(err) => {
                            warn!(
                                "failed to load search context for {}: {err:#}",
                                workspace.root.display()
                            );
                            all_errors.push(format!("{}: {err:#}", workspace.root.display()));
                            continue;
                        }
                    };
                    match hybrid_search_with_context(
                        &context,
                        workspace,
                        &query,
                        Some(model.as_ref()),
                        &options,
                    ) {
                        Ok(mut hits) => {
                            if all_indices {
                                for hit in &mut hits {
                                    hit.file_path = workspace.root.join(&hit.file_path);
                                }
                            }
                            all_hits.append(&mut hits);
                        }
                        Err(err) => {
                            warn!(
                                "hybrid_search failed for {}: {err:#}",
                                workspace.root.display()
                            );
                            all_errors.push(format!("{}: {err:#}", workspace.root.display()));
                        }
                    }
                }
                all_hits.sort_by(|a, b| {
                    b.score
                        .partial_cmp(&a.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                if let Some(l) = options.limit {
                    all_hits.truncate(l);
                }
                if all_errors.is_empty() {
                    state_clone.store_query_results(cache_key, &all_hits);
                }
                // Spawn background hash and neural enhancement for workspaces that need it.
                if crate::config::background_enhancement_enabled() {
                    for root in ws_neural_missing {
                        if let Ok(ws) = Workspace::resolve(&root) {
                            let _ = ws.trigger_background_enhancement();
                        }
                    }
                }
                (all_hits, all_errors)
            })
            .await
            .unwrap_or_else(|join_err| {
                warn!("search task panicked: {join_err:#}");
                (
                    Vec::new(),
                    vec![format!("search task panicked: {join_err:#}")],
                )
            });

            // If ALL workspaces failed (no hits and at least one error),
            // propagate as Error so the CLI can fall back to local search.
            if result.0.is_empty() && !result.1.is_empty() {
                DaemonResponse::Error {
                    message: format!("search failed: {}", result.1.join("; ")),
                }
            } else {
                DaemonResponse::SearchResults { hits: result.0 }
            }
        }
        DaemonRequest::RegexSearch {
            path,
            pattern,
            limit,
            include_globs,
            exclude_globs,
            scope_path,
            scope_is_file,
            skip_gitignore,
        } => {
            let workspaces = if let Some(ref p) = path {
                match Workspace::resolve(p) {
                    Ok(workspace) => vec![workspace],
                    Err(err) => {
                        return DaemonResponse::Error {
                            message: err.to_string(),
                        };
                    }
                }
            } else {
                match list_workspaces() {
                    Ok(ws) => ws
                        .into_iter()
                        .filter(|w| w.last_indexed_at_unix.is_some())
                        .filter_map(|w| Workspace::resolve(&w.root).ok())
                        .collect(),
                    Err(err) => {
                        return DaemonResponse::Error {
                            message: err.to_string(),
                        };
                    }
                }
            };

            let scope_filter = scope_from_request(scope_path, scope_is_file);
            // Bound concurrent heavy regex work (see #58).
            let permit = state.cpu_permits.clone().acquire_owned().await.ok();
            let result = tokio::task::spawn_blocking(move || {
                let _permit = permit;
                let mut all_hits = Vec::new();
                for workspace in &workspaces {
                    match regex_search(
                        workspace,
                        &pattern,
                        limit,
                        scope_filter.as_ref(),
                        &include_globs,
                        &exclude_globs,
                        skip_gitignore,
                    ) {
                        Ok(mut hits) => {
                            if path.is_none() {
                                for hit in &mut hits {
                                    hit.file_path = workspace.root.join(&hit.file_path);
                                }
                            }
                            all_hits.append(&mut hits);
                        }
                        Err(err) => {
                            warn!(
                                "regex_search failed for {}: {err:#}",
                                workspace.root.display()
                            );
                        }
                    }
                }

                if let Some(l) = limit {
                    all_hits.truncate(l);
                }

                all_hits
            })
            .await
            .unwrap_or_else(|join_err| {
                warn!("regex search task panicked: {join_err:#}");
                Vec::new()
            });

            DaemonResponse::SearchResults { hits: result }
        }
        DaemonRequest::LiteralSearch {
            path,
            query,
            limit,
            context,
            type_filter,
            include_globs,
            exclude_globs,
            scope_path,
            scope_is_file,
            skip_gitignore,
        } => {
            let workspaces = if let Some(ref p) = path {
                match Workspace::resolve(p) {
                    Ok(workspace) => vec![workspace],
                    Err(err) => {
                        return DaemonResponse::Error {
                            message: err.to_string(),
                        };
                    }
                }
            } else {
                match list_workspaces() {
                    Ok(ws) => ws
                        .into_iter()
                        .filter(|w| w.last_indexed_at_unix.is_some())
                        .filter_map(|w| Workspace::resolve(&w.root).ok())
                        .collect(),
                    Err(err) => {
                        return DaemonResponse::Error {
                            message: err.to_string(),
                        };
                    }
                }
            };

            let scope_filter = scope_from_request(scope_path, scope_is_file);
            let options = SearchOptions {
                limit,
                context,
                type_filter,
                include_globs,
                exclude_globs,
                scope_filter,
                skip_gitignore,
                progress_tx: None,
                cancel_token: None,
            };

            let state_clone = state.clone();
            // Bound concurrent heavy literal work (see #58).
            let permit = state_clone.cpu_permits.clone().acquire_owned().await.ok();
            let result = tokio::task::spawn_blocking(move || {
                let _permit = permit;
                let mut all_hits = Vec::new();
                let mut all_errors: Vec<String> = Vec::new();
                for workspace in &workspaces {
                    let context = match state_clone.cached_search_context(workspace, None, false) {
                        Ok(context) => context,
                        Err(err) => {
                            warn!(
                                "failed to load literal search context for {}: {err:#}",
                                workspace.root.display()
                            );
                            all_errors.push(format!("{}: {err:#}", workspace.root.display()));
                            continue;
                        }
                    };
                    match literal_search_with_context(&context, workspace, &query, &options) {
                        Ok(mut hits) => {
                            if path.is_none() {
                                for hit in &mut hits {
                                    hit.file_path = workspace.root.join(&hit.file_path);
                                }
                            }
                            all_hits.append(&mut hits);
                        }
                        Err(err) => {
                            warn!(
                                "literal_search failed for {}: {err:#}",
                                workspace.root.display()
                            );
                            all_errors.push(format!("{}: {err:#}", workspace.root.display()));
                        }
                    }
                }

                if all_hits.is_empty() && !all_errors.is_empty() {
                    return Err(all_errors.join("; "));
                }

                if let Some(l) = options.limit {
                    all_hits.truncate(l);
                }
                Ok(all_hits)
            })
            .await
            .unwrap_or_else(|join_err| {
                warn!("literal search task panicked: {join_err:#}");
                Err(join_err.to_string())
            });

            match result {
                Ok(hits) => DaemonResponse::SearchResults { hits },
                Err(message) => DaemonResponse::Error { message },
            }
        }
        DaemonRequest::Remove { path } => match Workspace::resolve(&path) {
            Ok(workspace) => {
                let workspace_for_cache = workspace.clone();
                // Stop watcher so no new indexing is triggered.
                if let Some(registration) = state.watchers.lock().remove(&workspace.id) {
                    stop_watcher(&workspace, registration);
                }
                if let Ok(Some(mut metadata)) = workspace.read_metadata() {
                    metadata.watch_enabled = false;
                    let _ = workspace.write_metadata(&metadata);
                }

                // Acquire the same fs2 lock that index_workspace holds to
                // wait for any in-progress indexing before deleting.
                match tokio::task::spawn_blocking(move || {
                    workspace.ensure_dirs().ok();
                    let lock_path = workspace.lock_path();
                    if let Ok(lock_file) = std::fs::OpenOptions::new()
                        .create(true)
                        .write(true)
                        .truncate(false)
                        .open(&lock_path)
                    {
                        // Blocking: waits for any running indexer to release.
                        let _ = fs2::FileExt::lock_exclusive(&lock_file);
                        let result = remove_workspace_index(&workspace);
                        let _ = fs2::FileExt::unlock(&lock_file);
                        result
                    } else {
                        remove_workspace_index(&workspace)
                    }
                })
                .await
                .unwrap_or_else(|join_err| Err(anyhow::anyhow!(join_err.to_string())))
                {
                    Ok(_) => {
                        state.clear_workspace_contexts(&workspace_for_cache);
                        DaemonResponse::Ack {
                            message: format!("removed workspace index {}", path.display()),
                        }
                    }
                    Err(err) => DaemonResponse::Error {
                        message: err.to_string(),
                    },
                }
            }
            Err(err) => DaemonResponse::Error {
                message: err.to_string(),
            },
        },
        DaemonRequest::Restart => {
            info!("restart requested, shutting down");
            stop_all_watchers(&state);
            // Clean up socket so the new daemon can bind immediately
            crate::ipc::cleanup_socket();
            // Schedule exit after the response is sent
            tokio::spawn(async {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                std::process::exit(0);
            });
            DaemonResponse::Ack {
                message: "restarting".to_string(),
            }
        }
    }
}

fn register_watcher(state: &DaemonState, path: &std::path::Path) -> Result<()> {
    let workspace = Workspace::resolve(path)?;

    let mut watchers = state.watchers.lock();
    if watchers.contains_key(&workspace.id) {
        return Ok(());
    }

    let control = Arc::new(WatchControl::new(workspace.clone()));
    let callback_control = control.clone();
    let event_filter = WatchEventFilter::new(&workspace);

    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        if let Ok(event) = event {
            let paths = event_filter.paths_to_reindex(&event);
            if !paths.is_empty() {
                callback_control.mark_dirty(paths);
            }
        }
    })?;

    match watcher.watch(&workspace.root, RecursiveMode::Recursive) {
        Ok(()) => {}
        Err(err) => {
            // On Linux, inotify has a system-wide watch limit. Exceeding it
            // causes ENOSPC, which can cascade and break other watchers
            // (editors, file managers) on the system.
            let msg = format!("{err:#}");
            if msg.contains("No space left on device") || msg.contains("ENOSPC") {
                warn!(
                    "inotify watch limit exhausted for {}. \
                     Increase the limit with: \
                     echo fs.inotify.max_user_watches=524288 | sudo tee -a /etc/sysctl.conf && sudo sysctl -p",
                    workspace.root.display()
                );
                daemon_log(&format!(
                    "WARNING: inotify limit exhausted for {}. Run: \
                     echo fs.inotify.max_user_watches=524288 | sudo tee -a /etc/sysctl.conf && sudo sysctl -p",
                    workspace.root.display()
                ));
            }
            return Err(err.into());
        }
    }
    watchers.insert(
        workspace.id.clone(),
        WatchRegistration {
            _watcher: watcher,
            control: control.clone(),
        },
    );
    drop(watchers);

    let _ = jobs::start_job(&workspace, JobKind::Watcher, "idle", 1);
    spawn_watch_heartbeat(control.clone());
    spawn_watch_worker(state.clone(), control);

    if let Ok(Some(mut metadata)) = workspace.read_metadata()
        && !metadata.watch_enabled
    {
        metadata.watch_enabled = true;
        let _ = workspace.write_metadata(&metadata);
    }

    // Write the daemon PID so the CLI can verify the watcher is alive
    // and skip expensive Merkle scans ("trust but verify").
    let _ = std::fs::write(workspace.watcher_pid_path(), std::process::id().to_string());

    daemon_log(&format!("watching {}", workspace.root.display()));

    Ok(())
}

fn spawn_watch_heartbeat(control: Arc<WatchControl>) {
    tokio::spawn(async move {
        loop {
            if !control.active.load(Ordering::Relaxed) {
                break;
            }

            let (phase, indexing, dirty, pending_events, coalesced_events) =
                control.snapshot_phase();
            let mut update = JobUpdate {
                phase: Some(phase.to_string()),
                active: Some(true),
                ..Default::default()
            };
            update
                .details
                .insert("indexing".to_string(), indexing.to_string());
            update
                .details
                .insert("dirty".to_string(), dirty.to_string());
            update
                .details
                .insert("pending_events".to_string(), pending_events.to_string());
            update
                .details
                .insert("coalesced_events".to_string(), coalesced_events.to_string());
            let _ = jobs::heartbeat_job(&control.workspace, JobKind::Watcher, update);
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    });
}

fn spawn_watch_worker(state: DaemonState, control: Arc<WatchControl>) {
    tokio::spawn(async move {
        loop {
            control.notify.notified().await;
            if !control.active.load(Ordering::Relaxed) {
                break;
            }

            wait_for_watch_quiet(&control).await;

            if control.indexing.swap(true, Ordering::Relaxed) {
                continue;
            }

            loop {
                if !control.dirty.swap(false, Ordering::Relaxed) {
                    break;
                }

                let pending = control.pending_events.swap(0, Ordering::Relaxed);
                control
                    .coalesced_events
                    .fetch_add(pending.saturating_sub(1), Ordering::Relaxed);

                let mut update = JobUpdate {
                    phase: Some("indexing".to_string()),
                    active: Some(true),
                    ..Default::default()
                };
                update
                    .details
                    .insert("pending_events".to_string(), pending.to_string());
                let _ = jobs::heartbeat_job(&control.workspace, JobKind::Watcher, update);

                let workspace = control.workspace.clone();
                let changed_paths = control.take_dirty_paths();
                // Gate watcher-triggered indexing behind the same CPU semaphore
                // as client requests (#58). A multi-repo branch switch / build
                // can dirty many watched workspaces at once; without this, each
                // watcher's indexing spawn_blocking runs unbounded (saturating
                // the rayon chunking pool + the blocking pool), oversubscribing
                // CPU/memory exactly like the client burst #58 fixed.
                let permit = state.cpu_permits.clone().acquire_owned().await.ok();
                let result = tokio::task::spawn_blocking(move || {
                    let _permit = permit;
                    let hash_model = cached_hash_model();
                    let _ = if changed_paths.is_empty() {
                        index_workspace_for_watcher(&workspace, hash_model.as_ref())?
                    } else {
                        index_workspace_paths_for_watcher(
                            &workspace,
                            hash_model.as_ref(),
                            &changed_paths,
                        )?
                    };
                    Result::<(), anyhow::Error>::Ok(())
                })
                .await
                .unwrap_or_else(|join_err| Err(anyhow::anyhow!(join_err.to_string())));

                match result {
                    Ok(()) => {
                        state.clear_workspace_contexts(&control.workspace);
                        if crate::config::background_enhancement_enabled()
                            && control.workspace.needs_neural_enhancement()
                        {
                            let _ = control.workspace.trigger_background_enhancement();
                        }
                        daemon_log(&format!(
                            "watch update indexed {}",
                            control.workspace.root.display()
                        ));
                        let success = JobUpdate {
                            phase: Some(if control.dirty.load(Ordering::Relaxed) {
                                "dirty".to_string()
                            } else {
                                "idle".to_string()
                            }),
                            last_error: Some(None),
                            ..Default::default()
                        };
                        let _ = jobs::heartbeat_job(&control.workspace, JobKind::Watcher, success);
                    }
                    Err(err) => {
                        daemon_log(&format!(
                            "watch update failed for {}: {err:#}",
                            control.workspace.root.display()
                        ));
                        warn!(
                            "watch-triggered indexing failed for {}: {err:#}",
                            control.workspace.root.display()
                        );
                        let failed = JobUpdate {
                            phase: Some("error".to_string()),
                            last_error: Some(Some(format!("{err:#}"))),
                            ..Default::default()
                        };
                        let _ = jobs::heartbeat_job(&control.workspace, JobKind::Watcher, failed);
                    }
                }

                if control.dirty.load(Ordering::Relaxed) {
                    wait_for_watch_quiet(&control).await;
                }
            }

            control.indexing.store(false, Ordering::Relaxed);
            let idle = JobUpdate {
                phase: Some(if control.dirty.load(Ordering::Relaxed) {
                    "dirty".to_string()
                } else {
                    "idle".to_string()
                }),
                ..Default::default()
            };
            let _ = jobs::heartbeat_job(&control.workspace, JobKind::Watcher, idle);
        }
    });
}

async fn wait_for_watch_quiet(control: &WatchControl) {
    let started = tokio::time::Instant::now();
    let mut last_seen = control.pending_events.load(Ordering::Relaxed);
    let mut last_changed = tokio::time::Instant::now();

    loop {
        tokio::time::sleep(Duration::from_millis(250)).await;
        if !control.active.load(Ordering::Relaxed) {
            break;
        }

        let current = control.pending_events.load(Ordering::Relaxed);
        let now = tokio::time::Instant::now();
        if current != last_seen {
            last_seen = current;
            last_changed = now;
        }

        if now.duration_since(last_changed) >= WATCH_QUIET_PERIOD
            || now.duration_since(started) >= WATCH_MAX_DEBOUNCE
        {
            break;
        }
    }
}

/// Process-wide cached hash embedding model. Avoids rebuilding the alias
/// hash map on every watcher event, index request, or fallback search.
fn cached_hash_model() -> Arc<dyn EmbeddingModel> {
    static HASH_MODEL: std::sync::OnceLock<Arc<dyn EmbeddingModel>> = std::sync::OnceLock::new();
    HASH_MODEL
        .get_or_init(|| Arc::from(create_model(true)))
        .clone()
}

fn search_context_signature(
    workspace: &Workspace,
    emb_dim: Option<usize>,
    wants_neural_vectors: bool,
) -> SearchContextSignature {
    let wants_hash_vectors = emb_dim.is_some();
    let index_generation = workspace
        .read_metadata()
        .ok()
        .flatten()
        .map(|metadata| metadata.index_generation);
    let use_overlay = workspace.has_overlay() || workspace.base_ref_path().exists();

    if use_overlay {
        let base_dir = workspace
            .base_index_dir
            .clone()
            .unwrap_or_else(|| workspace.index_dir.clone());
        SearchContextSignature {
            index_generation,
            sqlite: file_stamp(&workspace.overlay_sqlite_path()),
            tantivy: dir_stamp(&workspace.overlay_tantivy_dir()),
            hash_vectors: wants_hash_vectors
                .then(|| file_stamp(&workspace.overlay_vector_path()))
                .flatten(),
            neural_vectors: None,
            base_sqlite: file_stamp(&base_dir.join("metadata.sqlite3")),
            base_tantivy: dir_stamp(&base_dir.join("tantivy")),
            base_hash_vectors: wants_hash_vectors
                .then(|| file_stamp(&base_dir.join("vectors.usearch")))
                .flatten(),
            base_neural_vectors: wants_neural_vectors
                .then(|| file_stamp(&base_dir.join("vectors_neural.usearch")))
                .flatten(),
        }
    } else {
        SearchContextSignature {
            index_generation,
            sqlite: file_stamp(&workspace.sqlite_path()),
            tantivy: dir_stamp(&workspace.tantivy_dir()),
            hash_vectors: wants_hash_vectors
                .then(|| file_stamp(&workspace.vector_path()))
                .flatten(),
            neural_vectors: wants_neural_vectors
                .then(|| file_stamp(&workspace.vector_neural_path()))
                .flatten(),
            base_sqlite: None,
            base_tantivy: None,
            base_hash_vectors: None,
            base_neural_vectors: None,
        }
    }
}

fn query_cache_key(
    workspaces: &[Workspace],
    query: &str,
    options: &SearchOptions,
    emb_dim: usize,
    wants_neural: bool,
    all_indices: bool,
) -> QueryCacheKey {
    QueryCacheKey {
        workspace_ids: workspaces
            .iter()
            .map(|workspace| workspace.id.clone())
            .collect(),
        signatures: workspaces
            .iter()
            .map(|workspace| search_context_signature(workspace, Some(emb_dim), wants_neural))
            .collect(),
        all_indices,
        query: query.to_string(),
        limit: options.limit,
        context: options.context,
        type_filter: options.type_filter.clone(),
        include_globs: options.include_globs.clone(),
        exclude_globs: options.exclude_globs.clone(),
        scope_filter: options.scope_filter.clone(),
        skip_gitignore: options.skip_gitignore,
        emb_dim,
        wants_neural,
    }
}

fn file_stamp(path: &Path) -> Option<FileStamp> {
    let metadata = std::fs::metadata(path).ok()?;
    Some(FileStamp {
        len: metadata.len(),
        modified_nanos: modified_nanos(&metadata),
    })
}

fn dir_stamp(path: &Path) -> Option<DirStamp> {
    let entries = std::fs::read_dir(path).ok()?;
    let mut files = 0u64;
    let mut len = 0u64;
    let mut newest_modified_nanos = 0u128;

    for entry in entries.flatten() {
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        files += 1;
        len = len.saturating_add(metadata.len());
        newest_modified_nanos = newest_modified_nanos.max(modified_nanos(&metadata));
    }

    Some(DirStamp {
        files,
        len,
        newest_modified_nanos,
    })
}

fn modified_nanos(metadata: &std::fs::Metadata) -> u128 {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

fn build_root_gitignore(root: &Path) -> Option<ignore::gitignore::Gitignore> {
    let mut builder = ignore::gitignore::GitignoreBuilder::new(root);
    let gitignore = root.join(".gitignore");
    if gitignore.exists() {
        let _ = builder.add(&gitignore);
    }
    let git_exclude = root.join(".git/info/exclude");
    if git_exclude.exists() {
        let _ = builder.add(&git_exclude);
    }
    builder.build().ok()
}

fn is_always_ignored_watch_path(rel: &Path) -> bool {
    rel.components().any(|component| {
        let part = component.as_os_str();
        part == ".git" || part == ".ivygrep"
    })
}

fn is_common_build_output_path(rel: &Path) -> bool {
    rel.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some(
                "target"
                    | "node_modules"
                    | ".next"
                    | ".nuxt"
                    | ".svelte-kit"
                    | ".turbo"
                    | ".cache"
                    | ".direnv"
                    | "dist"
                    | "build"
                    | "coverage"
            )
        )
    })
}

fn daemon_log(message: &str) {
    eprintln!("{} {}", daemon_timestamp(), message);
}

fn daemon_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("[{}.{:03}]", now.as_secs(), now.subsec_millis())
}

fn open_daemon_log_file() -> Result<File> {
    let log_path = config::app_home()?.join("daemon.log");
    if log_path
        .metadata()
        .map(|metadata| metadata.len() > MAX_DAEMON_LOG_BYTES)
        .unwrap_or(false)
    {
        let rotated = log_path.with_extension("log.1");
        let _ = std::fs::remove_file(&rotated);
        let _ = std::fs::rename(&log_path, rotated);
    }

    Ok(OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?)
}

fn scope_from_request(scope_path: Option<PathBuf>, scope_is_file: bool) -> Option<WorkspaceScope> {
    scope_path.map(|rel_path| WorkspaceScope {
        rel_path,
        is_file: scope_is_file,
    })
}

pub async fn request<F>(
    request: &DaemonRequest,
    autospawn: bool,
    mut progress_cb: Option<F>,
) -> Result<Option<DaemonResponse>>
where
    F: FnMut(String, usize, usize) + Send,
{
    if crate::ipc::socket_exists() && crate::ipc::connect().await.is_err() {
        crate::ipc::cleanup_socket();
    }

    // Auto-spawn the daemon if it isn't running.
    // Skip when IVYGREP_NO_AUTOSPAWN is set (for tests and CI).
    if autospawn
        && !crate::ipc::socket_exists()
        && std::env::var_os("IVYGREP_NO_AUTOSPAWN").is_none()
        && let Ok(exe) = std::env::current_exe()
    {
        let is_ig = exe
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n == "ig");
        if is_ig {
            let mut cmd = std::process::Command::new(exe);
            cmd.arg("--daemon");

            // Redirect daemon I/O to a log file to keep the CLI terminal clean.
            if let Ok(mut log_file) = open_daemon_log_file() {
                let _ = writeln!(log_file, "{} spawning daemon", daemon_timestamp());
                let log_stderr = log_file.try_clone();
                cmd.stdout(std::process::Stdio::from(log_file));
                if let Ok(stderr_file) = log_stderr {
                    cmd.stderr(std::process::Stdio::from(stderr_file));
                } else {
                    cmd.stderr(std::process::Stdio::null());
                }
            }

            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                cmd.process_group(0);
                unsafe {
                    cmd.pre_exec(|| {
                        libc::nice(5);
                        Ok(())
                    });
                }
            }

            #[cfg(not(unix))]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                cmd.creation_flags(CREATE_NO_WINDOW);
            }

            let _ = cmd.spawn();
            // Poll for socket readiness (up to 2s)
            for _ in 0..20 {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                if crate::ipc::socket_exists() {
                    break;
                }
            }
        }
    }

    if !crate::ipc::socket_exists() {
        return Ok(None);
    }

    // Timeout on connect — if the daemon is a zombie stuck in kernel sleep,
    // the connect() will hang. Don't let the CLI join the zombie pile.
    let mut stream = match tokio::time::timeout(
        std::time::Duration::from_secs(2),
        crate::ipc::connect(),
    )
    .await
    {
        Ok(Ok(stream)) => stream,
        _ => {
            // Connect timed out or failed — daemon is dead or zombie.
            // Remove the stale socket so we don't try again.
            crate::ipc::cleanup_socket();
            return Ok(None);
        }
    };

    let payload = serde_json::to_vec(&DaemonRequestEnvelope::new(request.clone()))?;
    if payload.len() > MAX_DAEMON_REQUEST_BYTES {
        anyhow::bail!("daemon request exceeds maximum of {MAX_DAEMON_REQUEST_BYTES} bytes");
    }
    // Timeout writes too — a zombie daemon may accept the connection
    // but never read from it, causing writes to eventually block.
    if tokio::time::timeout(std::time::Duration::from_secs(2), async {
        stream.write_all(&payload).await?;
        stream.write_all(b"\n").await?;
        Ok::<_, anyhow::Error>(())
    })
    .await
    .is_err()
    {
        crate::ipc::cleanup_socket();
        return Ok(None);
    }

    let mut reader = BufReader::new(stream);
    let mut line = String::new();

    // Timeout varies by request type: Index can take 30+ min on massive repos
    // (large monorepos: 270K+ files), while Status should complete in seconds.
    let timeout_secs = match request {
        DaemonRequest::Index { .. } => 1800, // 30 min for large repos
        DaemonRequest::Status | DaemonRequest::Restart => 5, // quick
        DaemonRequest::Search { .. }
        | DaemonRequest::RegexSearch { .. }
        | DaemonRequest::LiteralSearch { .. } => 120, // 2 min for search
        DaemonRequest::Remove { .. } => 30,  // cleanup
    };

    loop {
        line.clear();
        match tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            reader.read_line(&mut line),
        )
        .await
        {
            Ok(Ok(0)) => return Ok(None),
            Ok(Ok(_)) => {}
            Ok(Err(_)) | Err(_) => return Ok(None),
        }

        if line.trim().is_empty() {
            continue;
        }

        let response: DaemonResponse = serde_json::from_str(&line)?;
        match response {
            DaemonResponse::SearchProgress {
                stage,
                scanned,
                total,
            } => {
                if let Some(cb) = &mut progress_cb {
                    cb(stage, scanned, total);
                }
            }
            other => return Ok(Some(other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use serial_test::serial;
    use tempfile::tempdir;

    use crate::embedding::create_hash_model;
    use crate::indexer::index_workspace;
    use crate::search::{SearchOptions, hybrid_search};
    use crate::workspace::WorkspaceMetadata;

    fn test_state() -> DaemonState {
        DaemonState {
            lazy_model: Arc::new(std::sync::OnceLock::new()),
            model_loading: Arc::new(AtomicBool::new(false)),
            watchers: Arc::new(Mutex::new(HashMap::new())),
            search_contexts: Arc::new(Mutex::new(HashMap::new())),
            idle_search_context_count: Arc::new(AtomicUsize::new(0)),
            query_results: Arc::new(Mutex::new(QueryResultCache::default())),
            cpu_permits: Arc::new(tokio::sync::Semaphore::new(num_cpus::get().max(1))),
        }
    }

    #[test]
    fn daemon_bounds_concurrent_cpu_work() {
        // #58: heavy search/index work must be gated by a bounded semaphore so a
        // burst of clients can't spawn hundreds of simultaneous blocking tasks.
        let state = test_state();
        assert_eq!(
            state.cpu_permits.available_permits(),
            num_cpus::get().max(1),
            "cpu_permits should be sized to the CPU count"
        );
    }

    #[test]
    fn daemon_rejects_malformed_and_mismatched_protocol_requests() {
        let malformed = parse_daemon_request(b"{not-json}\n").unwrap_err();
        assert!(matches!(
            malformed,
            DaemonResponse::Error { message } if message.contains("invalid daemon request")
        ));

        let missing = parse_daemon_request(br#"{"type":"status"}"#).unwrap_err();
        assert!(matches!(
            missing,
            DaemonResponse::Error { message } if message.contains("protocol_version")
        ));

        let mismatched = serde_json::to_vec(&serde_json::json!({
            "protocol_version": DAEMON_PROTOCOL_VERSION + 1,
            "type": "status"
        }))
        .unwrap();
        let response = parse_daemon_request(&mismatched).unwrap_err();
        assert!(matches!(
            response,
            DaemonResponse::Error { message } if message.contains("unsupported daemon protocol version")
        ));

        let versioned =
            serde_json::to_vec(&DaemonRequestEnvelope::new(DaemonRequest::Status)).unwrap();
        assert!(matches!(
            serde_json::from_slice::<DaemonRequest>(&versioned).unwrap(),
            DaemonRequest::Status
        ));
    }

    #[tokio::test]
    async fn daemon_rejects_oversized_request_without_unbounded_read() {
        let mut payload = vec![b'x'; MAX_DAEMON_REQUEST_BYTES + 1];
        payload.push(b'\n');
        let mut reader = BufReader::new(payload.as_slice());

        let response = read_daemon_request(&mut reader).await.unwrap_err();
        assert!(matches!(
            response,
            DaemonResponse::Error { message } if message.contains("exceeds maximum")
        ));
    }

    #[tokio::test]
    #[serial]
    async fn restore_configured_watchers_makes_workspace_live_and_updates_search() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let repo = tempdir().unwrap();
        std::fs::write(
            repo.path().join("lib.rs"),
            "pub fn before_restart() -> bool { true }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(repo.path()).unwrap();
        let model = create_hash_model();
        index_workspace(&workspace, model.as_ref()).unwrap();

        let metadata = WorkspaceMetadata {
            id: workspace.id.clone(),
            root: workspace.root.clone(),
            created_at_unix: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            last_indexed_at_unix: Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            ),
            watch_enabled: true,
            skip_gitignore: false,
            index_generation: 0,
        };
        workspace.write_metadata(&metadata).unwrap();

        let state = test_state();

        restore_configured_watchers(&state);

        let mut watcher_live = false;
        for _ in 0..20 {
            if crate::workspace::list_workspaces()
                .unwrap()
                .into_iter()
                .find(|status| status.id == workspace.id)
                .is_some_and(|status| status.watcher_alive)
            {
                watcher_live = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        assert!(
            watcher_live,
            "restored daemon should revive configured watcher"
        );

        std::fs::write(
            repo.path().join("lib.rs"),
            "pub fn after_restart() -> bool { true }\n",
        )
        .unwrap();

        let mut updated = false;
        for _ in 0..30 {
            let hits = hybrid_search(
                &workspace,
                "after restart",
                Some(model.as_ref()),
                &SearchOptions {
                    limit: Some(5),
                    ..Default::default()
                },
            )
            .unwrap();
            if hits.iter().any(|hit| hit.preview.contains("after_restart")) {
                updated = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        assert!(
            updated,
            "restored watcher should process file changes after daemon startup"
        );

        stop_all_watchers(&state);
    }

    #[tokio::test]
    #[serial]
    async fn concurrent_register_watcher_creates_exactly_one() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let repo = tempdir().unwrap();
        std::fs::write(
            repo.path().join("lib.rs"),
            "pub fn concurrent_watcher_target() -> bool { true }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(repo.path()).unwrap();
        let model = create_hash_model();
        index_workspace(&workspace, model.as_ref()).unwrap();

        let state = test_state();

        let state_arc = Arc::new(state);
        let barrier = Arc::new(tokio::sync::Barrier::new(8));
        let repo_path = repo.path().to_path_buf();

        let handles: Vec<_> = (0..8)
            .map(|_| {
                let state = Arc::clone(&state_arc);
                let barrier = Arc::clone(&barrier);
                let path = repo_path.clone();
                tokio::spawn(async move {
                    barrier.wait().await;
                    let _ = register_watcher(&state, &path);
                })
            })
            .collect();

        for h in handles {
            h.await.expect("register_watcher task panicked");
        }

        let watcher_count = state_arc.watchers.lock().len();
        assert_eq!(
            watcher_count, 1,
            "exactly one watcher should exist after concurrent registrations, got {watcher_count}"
        );

        stop_all_watchers(&state_arc);
    }

    #[test]
    #[serial]
    fn cached_search_context_pools_concurrent_leases_until_index_changes() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let repo = tempdir().unwrap();
        std::fs::write(
            repo.path().join("lib.rs"),
            "pub fn before() -> bool { true }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(repo.path()).unwrap();
        let model = create_hash_model();
        index_workspace(&workspace, model.as_ref()).unwrap();

        let state = test_state();
        let first = state
            .cached_search_context(&workspace, Some(256), false)
            .unwrap();
        let second = state
            .cached_search_context(&workspace, Some(256), false)
            .unwrap();
        assert!(
            Arc::ptr_eq(&first.pool, &second.pool),
            "unchanged index should use the same SearchContext pool"
        );
        let first_pool = first.pool.clone();
        assert!(
            first_pool.idle.lock().is_empty(),
            "concurrent leases must own separate contexts rather than waiting for one idle context"
        );
        drop(first);
        drop(second);
        assert_eq!(
            first_pool.idle.lock().len(),
            2,
            "released concurrent contexts should be reusable"
        );

        std::fs::write(
            repo.path().join("lib.rs"),
            "pub fn after() -> bool { true }\n",
        )
        .unwrap();
        index_workspace(&workspace, model.as_ref()).unwrap();

        let third = state
            .cached_search_context(&workspace, Some(256), false)
            .unwrap();
        assert!(
            !Arc::ptr_eq(&first_pool, &third.pool),
            "index generation change should replace the SearchContext pool"
        );
    }

    #[test]
    #[serial]
    fn cached_search_context_pool_bounds_retained_idle_contexts() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let repo = tempdir().unwrap();
        std::fs::write(repo.path().join("lib.rs"), "pub fn pooled() {}\n").unwrap();

        let workspace = Workspace::resolve(repo.path()).unwrap();
        let model = create_hash_model();
        index_workspace(&workspace, model.as_ref()).unwrap();

        let state = test_state();
        let leases = (0..MAX_IDLE_SEARCH_CONTEXTS_PER_KEY + 2)
            .map(|_| {
                state
                    .cached_search_context(&workspace, Some(256), false)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let pool = leases[0].pool.clone();
        drop(leases);

        assert_eq!(
            pool.idle.lock().len(),
            MAX_IDLE_SEARCH_CONTEXTS_PER_KEY,
            "idle pool must stay bounded after a concurrent burst"
        );
        assert_eq!(
            state.idle_search_context_count.load(Ordering::Relaxed),
            MAX_IDLE_SEARCH_CONTEXTS_PER_KEY,
            "idle global accounting must track retained contexts"
        );
    }

    #[test]
    #[serial]
    fn cached_search_context_pools_share_global_idle_bound() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let repo = tempdir().unwrap();
        std::fs::write(repo.path().join("lib.rs"), "pub fn globally_bounded() {}\n").unwrap();

        let workspace = Workspace::resolve(repo.path()).unwrap();
        let model = create_hash_model();
        index_workspace(&workspace, model.as_ref()).unwrap();

        let state = test_state();
        for dimension in 0..(MAX_IDLE_SEARCH_CONTEXTS / MAX_IDLE_SEARCH_CONTEXTS_PER_KEY + 2) {
            let leases = (0..MAX_IDLE_SEARCH_CONTEXTS_PER_KEY)
                .map(|_| {
                    state
                        .cached_search_context(&workspace, Some(dimension), false)
                        .unwrap()
                })
                .collect::<Vec<_>>();
            drop(leases);
        }

        assert_eq!(
            state.idle_search_context_count.load(Ordering::Relaxed),
            MAX_IDLE_SEARCH_CONTEXTS,
            "idle contexts retained across pools must remain globally bounded"
        );
        state.search_contexts.lock().clear();
        assert_eq!(
            state.idle_search_context_count.load(Ordering::Relaxed),
            0,
            "evicted pools must release retained-context accounting"
        );
    }

    #[tokio::test]
    #[serial]
    async fn daemon_search_uses_hash_vectors_before_neural_model_loads() {
        let home = tempdir().unwrap();
        unsafe {
            std::env::set_var("IVYGREP_HOME", home.path());
            std::env::set_var("IVYGREP_NO_AUTOSPAWN", "1");
        }

        let repo = tempdir().unwrap();
        std::fs::write(
            repo.path().join("auth.rs"),
            "pub fn authenticate_user(token: &str) -> bool { !token.is_empty() }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(repo.path()).unwrap();
        let model = create_hash_model();
        index_workspace(&workspace, model.as_ref()).unwrap();
        crate::indexer::enhance_workspace_hash(&workspace, model.as_ref()).unwrap();

        let state = test_state();
        let lazy_model = state.lazy_model.clone();
        let response = handle_request(
            state,
            DaemonRequest::Search {
                path: Some(workspace.root.clone()),
                query: "authenticate user".to_string(),
                limit: Some(5),
                context: 2,
                type_filter: None,
                include_globs: Vec::new(),
                exclude_globs: Vec::new(),
                scope_path: None,
                scope_is_file: false,
                skip_gitignore: false,
            },
        )
        .await;

        match response {
            DaemonResponse::SearchResults { hits } => {
                assert!(!hits.is_empty());
                assert!(
                    hits.iter()
                        .any(|hit| hit.sources.iter().any(|source| source == "semantic")),
                    "daemon should use hash vector semantic search before neural model loads"
                );
            }
            other => panic!("expected SearchResults, got {other:?}"),
        }
        assert!(
            lazy_model.get().is_none(),
            "daemon should not block-load neural model when only hash vectors exist"
        );
        assert!(
            !workspace.has_neural_vectors(),
            "daemon search must leave neural vector generation to background work"
        );
    }

    #[tokio::test]
    #[serial]
    async fn daemon_search_caches_repeated_query_results() {
        let home = tempdir().unwrap();
        unsafe {
            std::env::set_var("IVYGREP_HOME", home.path());
            std::env::set_var("IVYGREP_NO_AUTOSPAWN", "1");
        }

        let repo = tempdir().unwrap();
        std::fs::write(
            repo.path().join("auth.rs"),
            "pub fn authenticate_user(token: &str) -> bool { !token.is_empty() }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(repo.path()).unwrap();
        let model = create_hash_model();
        index_workspace(&workspace, model.as_ref()).unwrap();

        let state = test_state();
        let request = DaemonRequest::Search {
            path: Some(workspace.root.clone()),
            query: "authenticate user".to_string(),
            limit: Some(5),
            context: 2,
            type_filter: None,
            include_globs: Vec::new(),
            exclude_globs: Vec::new(),
            scope_path: None,
            scope_is_file: false,
            skip_gitignore: false,
        };

        let first = handle_request(state.clone(), request.clone()).await;
        let first_count = match first {
            DaemonResponse::SearchResults { hits } => hits.len(),
            other => panic!("expected SearchResults, got {other:?}"),
        };
        assert!(first_count > 0);
        assert_eq!(state.query_results.lock().results.len(), 1);

        state.search_contexts.lock().clear();
        let second = handle_request(state.clone(), request).await;
        let second_count = match second {
            DaemonResponse::SearchResults { hits } => hits.len(),
            other => panic!("expected SearchResults, got {other:?}"),
        };

        assert_eq!(second_count, first_count);
        assert!(
            state.search_contexts.lock().is_empty(),
            "query cache hit should not reload SearchContext"
        );

        state.clear_workspace_contexts(&workspace);
        assert!(state.query_results.lock().results.is_empty());
    }

    #[tokio::test]
    #[serial]
    async fn daemon_query_cache_keeps_all_indices_path_mode_separate() {
        let home = tempdir().unwrap();
        unsafe {
            std::env::set_var("IVYGREP_HOME", home.path());
            std::env::set_var("IVYGREP_NO_AUTOSPAWN", "1");
        }

        let repo = tempdir().unwrap();
        std::fs::write(
            repo.path().join("auth.rs"),
            "pub fn authenticate_user(token: &str) -> bool { !token.is_empty() }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(repo.path()).unwrap();
        let model = create_hash_model();
        index_workspace(&workspace, model.as_ref()).unwrap();

        let state = test_state();
        let normal_request = DaemonRequest::Search {
            path: Some(workspace.root.clone()),
            query: "authenticate user".to_string(),
            limit: Some(5),
            context: 2,
            type_filter: None,
            include_globs: Vec::new(),
            exclude_globs: Vec::new(),
            scope_path: None,
            scope_is_file: false,
            skip_gitignore: false,
        };
        let all_request = DaemonRequest::Search {
            path: None,
            query: "authenticate user".to_string(),
            limit: Some(5),
            context: 2,
            type_filter: None,
            include_globs: Vec::new(),
            exclude_globs: Vec::new(),
            scope_path: None,
            scope_is_file: false,
            skip_gitignore: false,
        };

        let normal = handle_request(state.clone(), normal_request).await;
        match normal {
            DaemonResponse::SearchResults { hits } => {
                assert!(!hits.is_empty());
                assert!(
                    hits.iter().all(|hit| !hit.file_path.is_absolute()),
                    "single-workspace search should return workspace-relative paths"
                );
            }
            other => panic!("expected SearchResults, got {other:?}"),
        }
        assert_eq!(state.query_results.lock().results.len(), 1);

        state.search_contexts.lock().clear();
        let all = handle_request(state.clone(), all_request).await;
        match all {
            DaemonResponse::SearchResults { hits } => {
                assert!(!hits.is_empty());
                assert!(
                    hits.iter().all(|hit| hit.file_path.is_absolute()),
                    "--all search should return absolute paths even after normal query cache warmup"
                );
            }
            other => panic!("expected SearchResults, got {other:?}"),
        }
        assert_eq!(state.query_results.lock().results.len(), 2);
        assert!(
            !state.search_contexts.lock().is_empty(),
            "--all query should not reuse single-workspace cached results"
        );
    }

    #[test]
    #[serial]
    fn watch_event_filter_respects_gitignore_unless_skip_enabled() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let repo = tempdir().unwrap();
        std::fs::write(repo.path().join(".gitignore"), "target/\nsecret.txt\n").unwrap();
        std::fs::create_dir_all(repo.path().join("src")).unwrap();
        std::fs::create_dir_all(repo.path().join("target/debug")).unwrap();
        std::fs::create_dir_all(repo.path().join(".git")).unwrap();

        let workspace = Workspace::resolve(repo.path()).unwrap();
        workspace.ensure_dirs().unwrap();
        let mut metadata = WorkspaceMetadata {
            id: workspace.id.clone(),
            root: workspace.root.clone(),
            created_at_unix: 0,
            last_indexed_at_unix: None,
            watch_enabled: true,
            skip_gitignore: false,
            index_generation: 0,
        };
        workspace.write_metadata(&metadata).unwrap();

        let filter = WatchEventFilter::new(&workspace);
        assert!(filter.path_should_reindex(&repo.path().join("src/lib.rs")));
        assert!(!filter.path_should_reindex(&repo.path().join("target/debug/build.o")));
        assert!(!filter.path_should_reindex(&repo.path().join("secret.txt")));
        assert!(!filter.path_should_reindex(&repo.path().join(".git/index")));

        metadata.skip_gitignore = true;
        workspace.write_metadata(&metadata).unwrap();
        let filter = WatchEventFilter::new(&workspace);
        assert!(filter.path_should_reindex(&repo.path().join("src/lib.rs")));
        assert!(filter.path_should_reindex(&repo.path().join("target/debug/build.o")));
        assert!(filter.path_should_reindex(&repo.path().join("secret.txt")));
        assert!(!filter.path_should_reindex(&repo.path().join(".git/index")));
    }

    #[test]
    #[cfg(unix)]
    #[serial]
    fn watch_event_filter_accepts_paths_through_symlinked_root() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let repo = tempdir().unwrap();
        std::fs::create_dir_all(repo.path().join(".git")).unwrap();
        std::fs::create_dir_all(repo.path().join("src")).unwrap();

        let link = home.path().join("repo-link");
        std::os::unix::fs::symlink(repo.path(), &link).unwrap();

        let workspace = Workspace::resolve(&link).unwrap();
        workspace.ensure_dirs().unwrap();
        workspace
            .write_metadata(&WorkspaceMetadata {
                id: workspace.id.clone(),
                root: workspace.root.clone(),
                created_at_unix: 0,
                last_indexed_at_unix: None,
                watch_enabled: true,
                skip_gitignore: false,
                index_generation: 0,
            })
            .unwrap();

        let filter = WatchEventFilter::new(&workspace);
        assert!(filter.path_should_reindex(&link.join("src/lib.rs")));
    }

    #[test]
    #[serial]
    fn daemon_log_file_rotates_and_uses_timestamp_prefix() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        std::fs::create_dir_all(home.path()).unwrap();
        let log_path = home.path().join("daemon.log");
        std::fs::write(&log_path, vec![b'x'; (MAX_DAEMON_LOG_BYTES + 1) as usize]).unwrap();

        let mut log = open_daemon_log_file().unwrap();
        writeln!(log, "{} test line", daemon_timestamp()).unwrap();
        drop(log);

        assert!(home.path().join("daemon.log.1").exists());
        let current = std::fs::read_to_string(&log_path).unwrap();
        assert!(
            current.starts_with('[') && current.contains("] test line"),
            "daemon log should use a timestamp prefix, got {current:?}"
        );
    }
}
