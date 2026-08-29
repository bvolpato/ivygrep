use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Weak};
use std::time::{Duration, UNIX_EPOCH};

use anyhow::Result;
use lru::LruCache;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use parking_lot::{Condvar, Mutex};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Notify;
use tracing::{error, info, warn};

use crate::config;
use crate::embedding::{EmbeddingModel, create_model};
use crate::indexer::{
    index_workspace, index_workspace_for_watcher, index_workspace_paths_for_watcher,
    indexed_skip_gitignore, reconcile_worktree_overlay, remove_workspace_index,
    workspace_index_matches_skip_gitignore,
};
use crate::jobs::{self, JobKind, JobUpdate};
use crate::protocol::{
    BUILD_VERSION, DAEMON_PROTOCOL_VERSION, DaemonRequest, DaemonRequestEnvelope, DaemonResponse,
    SearchHit, WorkspaceRuntimeStatus, group_hits_by_file,
};
use crate::regex_search::regex_search_with_options;
use crate::search::{
    DEFAULT_SEARCH_LIMIT, NeuralQueryVectorJob, SearchContext, SearchOptions,
    hybrid_search_with_context_and_neural_job, literal_search_with_context, query_uses_neural,
    workspace_neural_model_identity,
};
use crate::search_service::{
    HitOrdering, SearchBatch, SearchOutcome, SearchWorkspaceSet, select_all_indexed_workspaces,
};
use crate::workspace::{Workspace, WorkspaceIndexState, WorkspaceScope, list_workspaces};

const WATCH_SINGLE_EVENT_QUIET_PERIOD: Duration = Duration::from_millis(250);
const WATCH_BURST_QUIET_PERIOD: Duration = Duration::from_millis(750);
const WATCH_MAX_DEBOUNCE: Duration = Duration::from_secs(30);
const WATCH_RETRY_INITIAL_DELAY: Duration = Duration::from_millis(250);
const WATCH_RETRY_MAX_DELAY: Duration = Duration::from_secs(30);
const DAEMON_CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const DAEMON_WRITE_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_DAEMON_LOG_BYTES: u64 = 10 * 1024 * 1024;
const MAX_DAEMON_REQUEST_BYTES: usize = 1024 * 1024;
const MAX_QUERY_CACHE_ENTRIES: usize = 128;
const MAX_NEURAL_QUERY_CACHE_ENTRIES: usize = 128;
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
/// Workspace resolution canonicalizes paths and inspects Git metadata. Cache
/// exact absolute roots so repeated daemon searches avoid reconstructing the
/// same immutable path-derived workspace descriptor.
const MAX_RESOLVED_WORKSPACES: usize = 128;
const MAX_NEURAL_STATUSES: usize = 128;
/// Bound on remembered enhancement-trigger attempts (one per workspace/mode).
const MAX_ENHANCEMENT_TRIGGERS: usize = 256;
/// Minimum spacing between background enhancement trigger attempts for one
/// workspace and mode. Each attempt reads the job ledger, probes the worker
/// process, and may spawn a child; doing that on every query is wasted work
/// while a worker is already running or paused.
const ENHANCEMENT_TRIGGER_INTERVAL: Duration = Duration::from_secs(10);
const MAX_READY_WORKSPACES: usize = 256;
const MAX_WATCH_POLICIES: usize = 256;
const MAX_SEARCH_CANCELLATION_TOMBSTONES: usize = 256;
const MAX_MEMORY_PROBE_LIMIT: usize = 80;
const MAX_MEMORY_PROBE_QUERY_CHARS: usize = 512;
const MEMORY_ORIGINAL_RRF_WEIGHT: f32 = 1.25;
/// Don't cache result sets larger than this (each hit carries preview/reason
/// strings; large `--no-limit` results would bloat the query cache).
const MAX_CACHEABLE_HITS: usize = 2_000;

fn finish_daemon_search_batch(
    batch: SearchBatch,
    options: &SearchOptions,
    ordering: HitOrdering,
) -> Result<SearchOutcome> {
    batch.finish(options.bounded_limit(), ordering)
}

fn truncate_daemon_search_hits(hits: &mut Vec<SearchHit>, options: &SearchOptions) {
    if let Some(limit) = options.bounded_limit() {
        hits.truncate(limit);
    }
}

fn cancelled_search_response() -> DaemonResponse {
    DaemonResponse::Error {
        message: "search cancelled".to_string(),
    }
}

/// Warning attached to results when the daemon's per-request deadline
/// cancelled a search; `None` for client-driven cancellation.
fn deadline_warning(cancellation: Option<&SearchCancellation>) -> Option<String> {
    cancellation
        .filter(|cancellation| cancellation.deadline_expired())
        .map(|_| {
            let secs = config::search_deadline()
                .map_or(config::DEFAULT_SEARCH_DEADLINE_SECS, |deadline| {
                    deadline.as_secs()
                });
            format!("search deadline of {secs}s exceeded; returning partial results")
        })
}

/// Response for a search cancelled before it produced anything: deadline
/// expiry yields empty results plus the warning, client cancellation the error.
fn cancelled_search_outcome(
    cancellation: Option<&SearchCancellation>,
    mut warnings: Vec<String>,
) -> DaemonResponse {
    match deadline_warning(cancellation) {
        Some(warning) => {
            warnings.push(warning);
            DaemonResponse::SearchResults {
                hits: Vec::new(),
                warnings,
            }
        }
        None => cancelled_search_response(),
    }
}

/// Map a regex/literal task outcome to a response, honouring cancellation:
/// deadline expiry returns the partial outcome plus a warning, client
/// cancellation keeps the explicit error.
fn finish_cancellable_search(
    result: std::result::Result<SearchOutcome, String>,
    cancellation: Option<&SearchCancellation>,
) -> DaemonResponse {
    match result {
        Ok(mut outcome) => {
            if cancellation.is_some_and(SearchCancellation::is_cancelled) {
                match deadline_warning(cancellation) {
                    Some(warning) => outcome.warnings.push(warning),
                    None => return cancelled_search_response(),
                }
            }
            DaemonResponse::SearchResults {
                hits: outcome.hits,
                warnings: outcome.warnings,
            }
        }
        Err(message) => DaemonResponse::Error { message },
    }
}

fn should_start_model_load(has_neural_vectors: bool, query: &str, force_neural: bool) -> bool {
    has_neural_vectors && query_uses_neural(query, force_neural)
}

fn is_note_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            ["md", "mdx", "txt", "rst", "adoc", "org"]
                .iter()
                .any(|candidate| extension.eq_ignore_ascii_case(candidate))
        })
}

fn should_expand_memory_query(query: &str, hits: &[SearchHit], limit: Option<usize>) -> bool {
    if hits.first().is_none_or(|hit| !is_note_path(&hit.file_path))
        || limit == Some(usize::MAX)
        || query.split_whitespace().count() < 5
        || !query_uses_neural(query, false)
    {
        return false;
    }
    let files = group_hits_by_file(hits, Some(5));
    files.len() >= 3
        && files
            .iter()
            .filter(|result| is_note_path(&result.file_path))
            .count()
            * 5
            >= files.len() * 4
}

fn memory_query_variants(query: &str) -> [String; 2] {
    let query = query
        .char_indices()
        .nth(MAX_MEMORY_PROBE_QUERY_CHARS)
        .map_or(query, |(end, _)| &query[..end]);
    [
        format!(
            "Personal context, prior preferences, constraints, and commitments relevant to: {query}"
        ),
        format!("Information needed before deciding, responding, or acting on: {query}"),
    ]
}

fn search_request_with_query(request: &DaemonRequest, query: String) -> DaemonRequest {
    let mut request = request.clone();
    let DaemonRequest::Search {
        query: request_query,
        limit,
        disable_memory_expansion,
        ..
    } = &mut request
    else {
        unreachable!("memory expansion requires a hybrid search request");
    };
    *request_query = query;
    if let Some(limit) = limit {
        *limit = limit
            .saturating_mul(4)
            .min(MAX_MEMORY_PROBE_LIMIT)
            .max(*limit);
    }
    *disable_memory_expansion = true;
    request
}

fn fuse_memory_probe_hits(
    original: Vec<SearchHit>,
    probes: Vec<Vec<SearchHit>>,
    limit: Option<usize>,
) -> Vec<SearchHit> {
    if probes.is_empty() {
        return original;
    }

    let mut scores = HashMap::<PathBuf, f32>::new();
    let mut selected = HashMap::<PathBuf, SearchHit>::new();
    for (output_index, output) in std::iter::once(original).chain(probes).enumerate() {
        let weight = if output_index == 0 {
            MEMORY_ORIGINAL_RRF_WEIGHT
        } else {
            1.0
        };
        for (rank, file) in group_hits_by_file(&output, None).into_iter().enumerate() {
            *scores.entry(file.file_path.clone()).or_default() += weight / (61 + rank) as f32;
            let Some(candidate) = file.hits.into_iter().next() else {
                continue;
            };
            selected
                .entry(file.file_path)
                .and_modify(|current| {
                    if candidate.score.total_cmp(&current.score).is_gt() {
                        current.clone_from(&candidate);
                    }
                })
                .or_insert(candidate);
        }
    }
    let mut ranked = scores.into_iter().collect::<Vec<_>>();
    ranked.sort_by(|(path_a, score_a), (path_b, score_b)| {
        score_b.total_cmp(score_a).then_with(|| path_a.cmp(path_b))
    });
    ranked
        .into_iter()
        .take(limit.unwrap_or(DEFAULT_SEARCH_LIMIT))
        .filter_map(|(path, score)| {
            let mut hit = selected.remove(&path)?;
            hit.score = score;
            if !hit.sources.iter().any(|source| source == "memory") {
                hit.sources.push("memory".to_string());
            }
            Some(hit)
        })
        .collect()
}

struct WatchRegistration {
    watcher: RecommendedWatcher,
    control: Arc<WatchControl>,
    event_filter: Arc<Mutex<WatchEventFilter>>,
    external_git_common_dir: Option<PathBuf>,
    external_git_watch: Option<PathBuf>,
}

#[derive(Clone)]
struct WatchEventFilter {
    workspace: Workspace,
    skip_gitignore: bool,
    git_exclude_path: Option<PathBuf>,
    root_gitignore: Option<ignore::gitignore::Gitignore>,
}

#[derive(Debug, Default)]
enum WatchChange {
    #[default]
    None,
    Paths(HashSet<PathBuf>),
    FullReconciliation,
}

#[derive(Debug, Default)]
struct PendingWatchWork {
    change: WatchChange,
    backend_error: Option<String>,
}

#[derive(Clone)]
enum WatchReadiness {
    Reconciling,
    Ready,
    Failed(String),
    Stopped,
}

struct WatchControl {
    workspace: Workspace,
    notify: Notify,
    shutdown: Notify,
    dirty: AtomicBool,
    indexing: AtomicBool,
    retrying: AtomicBool,
    active: AtomicBool,
    initial_scan_required: AtomicBool,
    initial_reconciliation_pending: AtomicBool,
    readiness: tokio::sync::watch::Sender<WatchReadiness>,
    pending_events: AtomicU64,
    coalesced_events: AtomicU64,
    pending_work: Mutex<PendingWatchWork>,
    /// Nonce of this watcher's job-ledger record. Shared so the heartbeat can
    /// re-create the record (and rotate the nonce) when the ledger is wiped
    /// by an index rebuild while the watcher keeps running.
    job_nonce: Mutex<Option<String>>,
}

impl WatchControl {
    fn new(workspace: Workspace) -> Self {
        let (readiness, _) = tokio::sync::watch::channel(WatchReadiness::Reconciling);
        Self {
            workspace,
            notify: Notify::new(),
            shutdown: Notify::new(),
            dirty: AtomicBool::new(false),
            indexing: AtomicBool::new(false),
            retrying: AtomicBool::new(false),
            active: AtomicBool::new(true),
            initial_scan_required: AtomicBool::new(true),
            initial_reconciliation_pending: AtomicBool::new(true),
            readiness,
            pending_events: AtomicU64::new(0),
            coalesced_events: AtomicU64::new(0),
            pending_work: Mutex::new(PendingWatchWork::default()),
            job_nonce: Mutex::new(None),
        }
    }

    fn job_nonce(&self) -> Option<String> {
        self.job_nonce.lock().clone()
    }

    fn set_job_nonce(&self, nonce: Option<String>) {
        *self.job_nonce.lock() = nonce;
    }

    fn mark_paths_dirty(&self, paths: impl IntoIterator<Item = PathBuf>) {
        let paths = paths.into_iter().collect::<HashSet<_>>();
        if paths.is_empty() {
            return;
        }

        let mut pending = self.pending_work.lock();
        match &mut pending.change {
            WatchChange::None => pending.change = WatchChange::Paths(paths),
            WatchChange::Paths(existing) => existing.extend(paths),
            WatchChange::FullReconciliation => {}
        }
        self.dirty.store(true, Ordering::Relaxed);
        self.pending_events.fetch_add(1, Ordering::Relaxed);
        drop(pending);
        self.notify.notify_one();
    }

    fn mark_full_reconciliation(&self, backend_error: Option<String>) {
        let mut pending = self.pending_work.lock();
        pending.change = WatchChange::FullReconciliation;
        if backend_error.is_some() {
            pending.backend_error = backend_error;
        }
        self.dirty.store(true, Ordering::Relaxed);
        self.pending_events.fetch_add(1, Ordering::Relaxed);
        drop(pending);
        self.notify.notify_one();
    }

    fn take_pending_work(&self) -> Option<PendingWatchWork> {
        let mut pending = self.pending_work.lock();
        if matches!(pending.change, WatchChange::None) {
            return None;
        }
        // Readiness checks the queue under this same lock. Claimed work must
        // remain visible even before the worker acquires its workspace lease.
        self.indexing.store(true, Ordering::Relaxed);
        let work = std::mem::take(&mut *pending);
        self.dirty.store(false, Ordering::Relaxed);
        Some(work)
    }

    fn requeue_failed_index(&self, error: String) {
        self.retrying.store(true, Ordering::Relaxed);
        self.mark_full_reconciliation(Some(error));
    }

    fn snapshot_phase(&self) -> (&'static str, bool, bool, u64, u64) {
        let indexing = self.indexing.load(Ordering::Relaxed);
        let dirty = self.dirty.load(Ordering::Relaxed);
        let pending_events = self.pending_events.load(Ordering::Relaxed);
        let coalesced_events = self.coalesced_events.load(Ordering::Relaxed);
        let phase = if self.retrying.load(Ordering::Relaxed) {
            "error"
        } else if indexing {
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
    neural_model: Option<FileStamp>,
    base_sqlite: Option<FileStamp>,
    base_tantivy: Option<DirStamp>,
    base_hash_vectors: Option<FileStamp>,
    base_neural_vectors: Option<FileStamp>,
    base_neural_model: Option<FileStamp>,
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
struct FileStamp {
    len: u64,
    modified_nanos: u128,
}

#[derive(Clone, Copy)]
struct CachedWatchPolicy {
    metadata: FileStamp,
    requires_watcher: bool,
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

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct NeuralStatusSignature {
    model: Option<FileStamp>,
    vectors: Option<FileStamp>,
    base_model: Option<FileStamp>,
    base_vectors: Option<FileStamp>,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct WorkspaceReadinessCacheKey {
    workspace_id: String,
    skip_gitignore: bool,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct WorkspaceReadinessSignature {
    metadata: Option<FileStamp>,
    indexed_skip_gitignore: Option<bool>,
    index_format: Option<FileStamp>,
    sqlite: Option<FileStamp>,
    tantivy: Option<DirStamp>,
    hash_vectors: Option<FileStamp>,
    overlay_sqlite: Option<FileStamp>,
    overlay_tantivy: Option<DirStamp>,
    overlay_hash_vectors: Option<FileStamp>,
    base_ref: Option<FileStamp>,
    base_metadata: Option<FileStamp>,
    base_index_format: Option<FileStamp>,
    merkle: Option<FileStamp>,
    indexing_pid: Option<FileStamp>,
}

#[derive(Clone)]
struct CachedNeuralStatus {
    signature: NeuralStatusSignature,
    identity: Option<crate::embedding::NeuralModelIdentity>,
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
    force_neural: bool,
    reranker: String,
}

struct QueryResultCache {
    results: LruCache<QueryCacheKey, Vec<crate::protocol::SearchHit>>,
}

struct NeuralQueryCache {
    vectors: LruCache<String, Vec<f32>>,
}

fn bounded_lru<K: std::hash::Hash + Eq, V>(capacity: usize) -> LruCache<K, V> {
    LruCache::new(NonZeroUsize::new(capacity).expect("cache capacity must be nonzero"))
}

impl Default for QueryResultCache {
    fn default() -> Self {
        Self {
            results: bounded_lru(MAX_QUERY_CACHE_ENTRIES),
        }
    }
}

impl Default for NeuralQueryCache {
    fn default() -> Self {
        Self {
            vectors: bounded_lru(MAX_NEURAL_QUERY_CACHE_ENTRIES),
        }
    }
}

impl NeuralQueryCache {
    fn get(&mut self, query: &str) -> Option<Vec<f32>> {
        self.vectors.get(query.trim()).cloned()
    }

    fn insert(&mut self, query: String, vector: Vec<f32>) {
        self.vectors.put(query.trim().to_string(), vector);
    }
}

impl QueryResultCache {
    fn get(&mut self, key: &QueryCacheKey) -> Option<Vec<crate::protocol::SearchHit>> {
        self.results.get(key).cloned()
    }

    fn insert(&mut self, key: QueryCacheKey, hits: Vec<crate::protocol::SearchHit>) {
        self.results.put(key, hits);
    }

    fn remove_workspace(&mut self, workspace_id: &str) {
        let keys = self
            .results
            .iter()
            .filter(|(key, _)| key.workspace_ids.iter().any(|id| id == workspace_id))
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        for key in keys {
            self.results.pop(&key);
        }
    }
}

impl WatchEventFilter {
    fn new(workspace: &Workspace) -> Self {
        let mut filter = Self {
            workspace: workspace.clone(),
            skip_gitignore: false,
            git_exclude_path: None,
            root_gitignore: None,
        };
        filter.refresh();
        filter
    }

    fn refresh(&mut self) {
        self.skip_gitignore = self
            .workspace
            .read_metadata()
            .ok()
            .flatten()
            .is_some_and(|metadata| metadata.skip_gitignore);
        self.git_exclude_path = crate::workspace::git_common_dir(&self.workspace.root)
            .map(|common_dir| common_dir.join("info/exclude"));
        self.root_gitignore = (!self.skip_gitignore)
            .then(|| build_root_gitignore(&self.workspace.root, self.git_exclude_path.as_deref()))
            .flatten();
    }

    fn change_for_event(&mut self, event: &notify::Event) -> WatchChange {
        if matches!(event.kind, notify::EventKind::Access(_)) {
            return WatchChange::None;
        }
        if event.paths.iter().any(|path| {
            !self
                .normalize_watch_path(path)
                .is_some_and(|(normalized, _)| {
                    crate::walker::is_ivygrep_owned_path(&self.workspace.root, &normalized)
                })
                && self.is_ignore_configuration_path(path)
        }) {
            self.refresh();
            return WatchChange::FullReconciliation;
        }

        let paths = self
            .paths_to_reindex(event)
            .into_iter()
            .collect::<HashSet<_>>();
        if paths.is_empty() {
            WatchChange::None
        } else {
            WatchChange::Paths(paths)
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
        if rel.as_os_str().is_empty()
            || is_always_ignored_watch_path(&rel)
            || crate::walker::is_ivygrep_owned_path(&self.workspace.root, &normalized_path)
        {
            return false;
        }

        if !self.skip_gitignore
            && let Some(gitignore) = &self.root_gitignore
            && gitignore
                .matched_path_or_any_parents(&normalized_path, normalized_path.is_dir())
                .is_ignore()
        {
            return false;
        }

        true
    }

    fn normalize_watch_path(&self, path: &Path) -> Option<(PathBuf, PathBuf)> {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.workspace.root.join(path)
        };

        if let Ok(rel) = absolute.strip_prefix(&self.workspace.root) {
            return Some((absolute.clone(), rel.to_path_buf()));
        }

        let normalized = canonicalize_existing_prefix(&absolute)?;
        let rel = normalized
            .strip_prefix(&self.workspace.root)
            .ok()?
            .to_path_buf();
        Some((normalized, rel))
    }

    fn is_ignore_configuration_path(&self, path: &Path) -> bool {
        if self.git_exclude_path.as_deref().is_some_and(|exclude| {
            watch_paths_match(path, exclude)
                || exclude
                    .parent()
                    .is_some_and(|parent| watch_paths_match(path, parent))
        }) {
            return true;
        }

        self.normalize_watch_path(path).is_some_and(|(_, rel)| {
            rel.file_name()
                .is_some_and(|name| name == ".gitignore" || name == ".ignore")
        })
    }
}

fn watch_paths_match(left: &Path, right: &Path) -> bool {
    left == right
        || canonicalize_existing_prefix(left)
            .zip(canonicalize_existing_prefix(right))
            .is_some_and(|(left, right)| left == right)
}

fn handle_watch_result(
    control: &WatchControl,
    event_filter: &Mutex<WatchEventFilter>,
    result: notify::Result<notify::Event>,
) {
    match result {
        Ok(event) if event.need_rescan() => {
            event_filter.lock().refresh();
            warn!(
                "watch backend requested a full rescan for {}",
                control.workspace.root.display()
            );
            daemon_log(&format!(
                "watch backend requested a full rescan for {}; scheduling full reconciliation",
                control.workspace.root.display()
            ));
            control.mark_full_reconciliation(None);
        }
        Ok(event) => match event_filter.lock().change_for_event(&event) {
            WatchChange::None => {}
            WatchChange::Paths(paths) => control.mark_paths_dirty(paths),
            WatchChange::FullReconciliation => control.mark_full_reconciliation(None),
        },
        Err(err) => {
            event_filter.lock().refresh();
            let error = format!("{err:#}");
            warn!(
                "watch backend error for {}: {error}",
                control.workspace.root.display()
            );
            daemon_log(&format!(
                "watch backend error for {}: {error}; scheduling full reconciliation",
                control.workspace.root.display()
            ));
            control.mark_full_reconciliation(Some(error));
        }
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
struct SearchCancellation {
    flag: Arc<AtomicBool>,
    /// Set when the daemon's own per-request deadline fired. Deadline-cancelled
    /// searches return the hits gathered so far plus a warning instead of the
    /// client-cancel error.
    deadline_expired: Arc<AtomicBool>,
    signal: tokio::sync::watch::Sender<bool>,
    finished_signal: tokio::sync::watch::Sender<bool>,
}

impl SearchCancellation {
    fn new(cancelled: bool) -> Self {
        let (signal, _) = tokio::sync::watch::channel(cancelled);
        let (finished_signal, _) = tokio::sync::watch::channel(false);
        Self {
            flag: Arc::new(AtomicBool::new(cancelled)),
            deadline_expired: Arc::new(AtomicBool::new(false)),
            signal,
            finished_signal,
        }
    }

    fn cancel(&self) {
        self.flag.store(true, Ordering::Relaxed);
        self.signal.send_replace(true);
    }

    fn cancel_for_deadline(&self) {
        self.deadline_expired.store(true, Ordering::Relaxed);
        self.cancel();
    }

    fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
    }

    fn deadline_expired(&self) -> bool {
        self.deadline_expired.load(Ordering::Relaxed)
    }

    async fn cancelled(&self) {
        let mut receiver = self.signal.subscribe();
        while !*receiver.borrow() {
            if receiver.changed().await.is_err() {
                break;
            }
        }
    }

    fn finish(&self) {
        self.finished_signal.send_replace(true);
    }

    async fn finished(&self) {
        let mut receiver = self.finished_signal.subscribe();
        while !*receiver.borrow() {
            if receiver.changed().await.is_err() {
                break;
            }
        }
    }
}

enum SearchCancellationEntry {
    Active(SearchCancellation),
    Tombstone(SearchCancellation),
}

#[derive(Default)]
struct SearchCancellationRegistry {
    entries: HashMap<uuid::Uuid, SearchCancellationEntry>,
    tombstones: VecDeque<uuid::Uuid>,
}

struct ActiveSearchRegistration {
    request_id: uuid::Uuid,
    cancellation: SearchCancellation,
    registry: Arc<Mutex<SearchCancellationRegistry>>,
}

impl Drop for ActiveSearchRegistration {
    fn drop(&mut self) {
        let mut registry = self.registry.lock();
        let owns_entry = matches!(
            registry.entries.get(&self.request_id),
            Some(SearchCancellationEntry::Active(cancellation))
                if Arc::ptr_eq(&cancellation.flag, &self.cancellation.flag)
        );
        if owns_entry {
            registry.entries.remove(&self.request_id);
        }
        drop(registry);
        self.cancellation.finish();
    }
}

#[derive(Default)]
struct WorkspaceModeCoordinatorState {
    active_mode: Option<bool>,
    active_leases: usize,
    next_mode: Option<bool>,
    next_mode_generation: u64,
    next_mode_waiters: usize,
    exclusive_active: bool,
    exclusive_waiters: usize,
}

#[derive(Default)]
struct WorkspaceModeCoordinator {
    state: Mutex<WorkspaceModeCoordinatorState>,
    changed: Condvar,
}

impl WorkspaceModeCoordinator {
    fn acquire_shared(
        self: &Arc<Self>,
        requested_mode: bool,
        cancellation: Option<&AtomicBool>,
    ) -> Option<WorkspaceModeLease> {
        let mut state = self.state.lock();
        let mut reserved_next_mode_generation = None;
        loop {
            if cancellation.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
                if state.next_mode == Some(requested_mode)
                    && reserved_next_mode_generation == Some(state.next_mode_generation)
                {
                    state.next_mode_waiters = state.next_mode_waiters.saturating_sub(1);
                    if state.next_mode_waiters == 0 {
                        state.next_mode = None;
                    }
                    self.changed.notify_all();
                }
                return None;
            }
            if !state.exclusive_active
                && state.exclusive_waiters == 0
                && state.active_leases == 0
                && state.next_mode.is_none_or(|next| next == requested_mode)
            {
                state.active_mode = Some(requested_mode);
                state.active_leases = 1;
                state.next_mode = None;
                state.next_mode_waiters = 0;
                return Some(WorkspaceModeLease {
                    coordinator: self.clone(),
                    exclusive: false,
                });
            }
            if !state.exclusive_active
                && state.exclusive_waiters == 0
                && state.active_mode == Some(requested_mode)
                && state.next_mode.is_none()
            {
                state.active_leases += 1;
                return Some(WorkspaceModeLease {
                    coordinator: self.clone(),
                    exclusive: false,
                });
            }
            if !state.exclusive_active
                && state.exclusive_waiters == 0
                && state.active_mode != Some(requested_mode)
            {
                if state.next_mode.is_none() {
                    state.next_mode = Some(requested_mode);
                    state.next_mode_generation = state.next_mode_generation.wrapping_add(1);
                    state.next_mode_waiters = 1;
                    reserved_next_mode_generation = Some(state.next_mode_generation);
                } else if state.next_mode == Some(requested_mode)
                    && reserved_next_mode_generation != Some(state.next_mode_generation)
                {
                    state.next_mode_waiters += 1;
                    reserved_next_mode_generation = Some(state.next_mode_generation);
                }
            }
            if cancellation.is_some() {
                self.changed.wait_for(&mut state, Duration::from_millis(10));
            } else {
                self.changed.wait(&mut state);
            }
        }
    }

    /// Grant a shared lease only when it is available right now: no exclusive
    /// holder or waiter, and the requested mode is active or nothing is
    /// active. Never waits or reserves the next mode, so it is safe to call
    /// inline from the async runtime.
    fn try_acquire_shared(self: &Arc<Self>, requested_mode: bool) -> Option<WorkspaceModeLease> {
        let mut state = self.state.lock();
        if state.exclusive_active || state.exclusive_waiters > 0 {
            return None;
        }
        if state.active_leases == 0 && state.next_mode.is_none_or(|next| next == requested_mode) {
            state.active_mode = Some(requested_mode);
            state.active_leases = 1;
            state.next_mode = None;
            state.next_mode_waiters = 0;
            return Some(WorkspaceModeLease {
                coordinator: self.clone(),
                exclusive: false,
            });
        }
        if state.active_mode == Some(requested_mode) && state.next_mode.is_none() {
            state.active_leases += 1;
            return Some(WorkspaceModeLease {
                coordinator: self.clone(),
                exclusive: false,
            });
        }
        None
    }

    fn acquire_exclusive(
        self: &Arc<Self>,
        cancellation: Option<&AtomicBool>,
    ) -> Option<WorkspaceModeLease> {
        let mut state = self.state.lock();
        if cancellation.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            return None;
        }
        state.exclusive_waiters += 1;
        while state.exclusive_active || state.active_leases > 0 {
            if cancellation.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
                state.exclusive_waiters -= 1;
                self.changed.notify_all();
                return None;
            }
            if cancellation.is_some() {
                self.changed.wait_for(&mut state, Duration::from_millis(10));
            } else {
                self.changed.wait(&mut state);
            }
        }
        if cancellation.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            state.exclusive_waiters -= 1;
            self.changed.notify_all();
            return None;
        }
        state.exclusive_waiters -= 1;
        state.exclusive_active = true;
        Some(WorkspaceModeLease {
            coordinator: self.clone(),
            exclusive: true,
        })
    }
}

struct WorkspaceModeLease {
    coordinator: Arc<WorkspaceModeCoordinator>,
    exclusive: bool,
}

impl Drop for WorkspaceModeLease {
    fn drop(&mut self) {
        let mut state = self.coordinator.state.lock();
        if self.exclusive {
            state.exclusive_active = false;
            self.coordinator.changed.notify_all();
            return;
        }
        state.active_leases = state.active_leases.saturating_sub(1);
        if state.active_leases == 0 {
            self.coordinator.changed.notify_all();
        }
    }
}

#[derive(Clone)]
struct InflightIndex {
    watch: bool,
    skip_gitignore: bool,
    outcome: tokio::sync::watch::Receiver<Option<DaemonResponse>>,
}

/// Leader-side handle for a coalesced `Index` request. Publishes the final
/// response to followers and removes the in-flight entry on drop so an aborted
/// leader never strands followers or blocks later requests.
struct InflightIndexLead {
    workspace_id: String,
    outcome: tokio::sync::watch::Sender<Option<DaemonResponse>>,
    registry: Arc<Mutex<HashMap<String, InflightIndex>>>,
}

impl InflightIndexLead {
    fn publish(&self, response: &DaemonResponse) {
        self.registry.lock().remove(&self.workspace_id);
        self.outcome.send_replace(Some(response.clone()));
    }
}

impl Drop for InflightIndexLead {
    fn drop(&mut self) {
        self.registry.lock().remove(&self.workspace_id);
        if self.outcome.borrow().is_none() {
            self.outcome.send_replace(Some(DaemonResponse::Error {
                message: "index request aborted before completion".to_string(),
            }));
        }
    }
}

enum InflightIndexSlot {
    Lead(InflightIndexLead),
    Follow(tokio::sync::watch::Receiver<Option<DaemonResponse>>),
}

struct ModeLeasedEmbeddingModel {
    inner: Arc<dyn EmbeddingModel>,
    _leases: Vec<WorkspaceModeLease>,
}

impl EmbeddingModel for ModeLeasedEmbeddingModel {
    fn dimensions(&self) -> usize {
        self.inner.dimensions()
    }

    fn embed(&self, text: &str) -> Vec<f32> {
        self.inner.embed(text)
    }

    fn embed_batch(&self, texts: &[&str]) -> Vec<Vec<f32>> {
        self.inner.embed_batch(texts)
    }

    fn warm_for_search(&self) {
        self.inner.warm_for_search();
    }

    fn backend_info(&self) -> Option<&'static str> {
        self.inner.backend_info()
    }

    fn profile_info(&self) -> Option<&'static str> {
        self.inner.profile_info()
    }

    fn model_identity(&self) -> Option<&crate::embedding::NeuralModelIdentity> {
        self.inner.model_identity()
    }

    fn respects_system_constraints(&self) -> bool {
        self.inner.respects_system_constraints()
    }
}

#[derive(Clone)]
pub(crate) struct DaemonState {
    lazy_model: Arc<std::sync::OnceLock<Arc<dyn EmbeddingModel>>>,
    model_loading: Arc<AtomicBool>,
    watchers: Arc<Mutex<HashMap<String, WatchRegistration>>>,
    watch_policies: Arc<Mutex<LruCache<String, CachedWatchPolicy>>>,
    resolved_workspaces: Arc<Mutex<LruCache<PathBuf, Workspace>>>,
    neural_statuses: Arc<Mutex<LruCache<String, CachedNeuralStatus>>>,
    /// Last background enhancement trigger attempt per (workspace, mode).
    enhancement_triggers: Arc<Mutex<LruCache<EnhancementTriggerKey, std::time::Instant>>>,
    ready_workspaces: Arc<Mutex<LruCache<WorkspaceReadinessCacheKey, WorkspaceReadinessSignature>>>,
    search_contexts: Arc<Mutex<LruCache<SearchContextCacheKey, CachedSearchContext>>>,
    idle_search_context_count: Arc<AtomicUsize>,
    query_results: Arc<Mutex<QueryResultCache>>,
    neural_queries: Arc<Mutex<NeuralQueryCache>>,
    search_cancellations: Arc<Mutex<SearchCancellationRegistry>>,
    workspace_modes: Arc<Mutex<HashMap<String, Weak<WorkspaceModeCoordinator>>>>,
    /// Explicit `Index` requests in flight, keyed by workspace id. Concurrent
    /// requests with the same options await the leader's outcome instead of
    /// queuing another full rescan behind the exclusive workspace lease.
    inflight_indexes: Arc<Mutex<HashMap<String, InflightIndex>>>,
    /// When the daemon last started a full index walk per workspace. A waiting
    /// Index request may only skip its own rescan when a full walk started
    /// after the request arrived; anything earlier could have scanned files
    /// before edits the request was meant to pick up.
    full_index_run_starts: Arc<Mutex<HashMap<String, std::time::Instant>>>,
    query_result_cache_enabled: bool,
    /// Bounds concurrent CPU-heavy work (hybrid/literal/regex search + index).
    /// Without this, a burst of clients each spawn a `spawn_blocking` task on
    /// Tokio's blocking pool (default cap 512), oversubscribing CPU and memory
    /// with no backpressure. See #58.
    cpu_permits: Arc<tokio::sync::Semaphore>,
    web_server: Arc<Mutex<Option<WebServerRuntime>>>,
    /// Watcher registrations that failed, per workspace id, with the retry
    /// backoff that keeps a broken watcher from being retried on every
    /// client request.
    watcher_recovery: Arc<Mutex<HashMap<String, WatcherRecovery>>>,
}

/// First retry delay after a failed watcher registration; doubles per
/// consecutive failure up to [`WATCHER_RETRY_MAX`].
const WATCHER_RETRY_BASE: Duration = Duration::from_secs(30);
const WATCHER_RETRY_MAX: Duration = Duration::from_secs(15 * 60);
/// How often the daemon looks for enabled, indexed workspaces without a live
/// watcher (startup failures, roots that reappeared, raised inotify limits).
const WATCHER_SUPERVISOR_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
struct WatcherRecovery {
    failures: u32,
    next_attempt_at: std::time::Instant,
    last_error: String,
}

fn watcher_retry_delay(failures: u32) -> Duration {
    let exponent = failures.saturating_sub(1).min(16);
    WATCHER_RETRY_BASE
        .checked_mul(1u32 << exponent)
        .map_or(WATCHER_RETRY_MAX, |delay| delay.min(WATCHER_RETRY_MAX))
}

struct WebServerRuntime {
    local_addr: SocketAddr,
    alive: Arc<AtomicBool>,
}

fn create_search_model() -> Arc<dyn EmbeddingModel> {
    let model = create_model(false);
    let started = std::time::Instant::now();
    let is_neural = model.model_identity().is_some();
    model.warm_for_search();
    if is_neural {
        tracing::trace!("daemon_model_warmup={:?}", started.elapsed());
    }
    Arc::from(model)
}

impl DaemonState {
    fn watcher_registered(&self, workspace_id: &str) -> bool {
        self.watchers.lock().contains_key(workspace_id)
    }

    /// The error to report instead of attempting another registration while
    /// the workspace's watcher is inside its retry backoff window.
    fn watcher_backoff_error(&self, workspace_id: &str) -> Option<String> {
        let recovery = self.watcher_recovery.lock();
        let entry = recovery.get(workspace_id)?;
        let remaining = entry
            .next_attempt_at
            .checked_duration_since(std::time::Instant::now())?;
        Some(format!(
            "{} (next watcher retry in {}s)",
            entry.last_error,
            remaining.as_secs().max(1)
        ))
    }

    fn watcher_last_error(&self, workspace_id: &str) -> Option<String> {
        self.watcher_recovery
            .lock()
            .get(workspace_id)
            .map(|entry| entry.last_error.clone())
    }

    fn record_watcher_failure(&self, workspace_id: &str, error: String) -> u32 {
        let mut recovery = self.watcher_recovery.lock();
        let failures = recovery
            .get(workspace_id)
            .map_or(1, |entry| entry.failures.saturating_add(1));
        recovery.insert(
            workspace_id.to_string(),
            WatcherRecovery {
                failures,
                next_attempt_at: std::time::Instant::now() + watcher_retry_delay(failures),
                last_error: error,
            },
        );
        failures
    }

    fn clear_watcher_failure(&self, workspace_id: &str) {
        self.watcher_recovery.lock().remove(workspace_id);
    }

    pub(crate) async fn acquire_cpu_permit(&self) -> Option<tokio::sync::OwnedSemaphorePermit> {
        self.cpu_permits.clone().acquire_owned().await.ok()
    }

    fn workspace_mode_coordinator(&self, workspace_id: &str) -> Arc<WorkspaceModeCoordinator> {
        let mut coordinators = self.workspace_modes.lock();
        if let Some(coordinator) = coordinators.get(workspace_id).and_then(Weak::upgrade) {
            return coordinator;
        }
        coordinators.retain(|_, coordinator| coordinator.strong_count() > 0);
        let coordinator = Arc::new(WorkspaceModeCoordinator::default());
        coordinators.insert(workspace_id.to_string(), Arc::downgrade(&coordinator));
        coordinator
    }

    #[cfg(test)]
    fn acquire_workspace_mode(
        &self,
        workspace: &Workspace,
        skip_gitignore: bool,
    ) -> WorkspaceModeLease {
        self.workspace_mode_coordinator(&workspace.id)
            .acquire_shared(skip_gitignore, None)
            .expect("uncancelled workspace mode acquisition")
    }

    fn acquire_workspace_modes(
        &self,
        workspaces: &[Workspace],
        skip_gitignore: bool,
    ) -> Vec<WorkspaceModeLease> {
        self.acquire_workspace_leases(workspaces, skip_gitignore, false, None)
            .expect("uncancelled workspace mode acquisition")
    }

    fn acquire_workspace_modes_cancellable(
        &self,
        workspaces: &[Workspace],
        skip_gitignore: bool,
        cancellation: Option<&AtomicBool>,
    ) -> Option<Vec<WorkspaceModeLease>> {
        self.acquire_workspace_leases(workspaces, skip_gitignore, false, cancellation)
    }

    fn acquire_workspace_mutations(&self, workspaces: &[Workspace]) -> Vec<WorkspaceModeLease> {
        self.acquire_workspace_leases(workspaces, false, true, None)
            .expect("uncancelled workspace mutation acquisition")
    }

    fn acquire_workspace_leases(
        &self,
        workspaces: &[Workspace],
        skip_gitignore: bool,
        direct_exclusive: bool,
        cancellation: Option<&AtomicBool>,
    ) -> Option<Vec<WorkspaceModeLease>> {
        let mut requirements = HashMap::new();
        for workspace in workspaces {
            requirements
                .entry(workspace.id.clone())
                .and_modify(|exclusive| *exclusive |= direct_exclusive)
                .or_insert(direct_exclusive);
            if workspace.is_worktree()
                && let Some(main_root) = workspace.main_worktree_root()
                && let Ok(base_workspace) = Workspace::resolve(&main_root)
            {
                let base_requires_mutation = direct_exclusive
                    || !self.workspace_is_ready(
                        workspace,
                        skip_gitignore,
                        &workspace_readiness_signature(workspace),
                    );
                requirements
                    .entry(base_workspace.id)
                    .and_modify(|exclusive| *exclusive |= base_requires_mutation)
                    .or_insert(base_requires_mutation);
            }
        }
        let shared_base_workspaces = if direct_exclusive {
            Vec::new()
        } else {
            workspaces
                .iter()
                .filter(|workspace| {
                    workspace.is_worktree()
                        && workspace
                            .main_worktree_root()
                            .and_then(|root| Workspace::resolve(&root).ok())
                            .and_then(|base| requirements.get(&base.id))
                            .is_some_and(|exclusive| !exclusive)
                })
                .collect::<Vec<_>>()
        };
        let mut requirements = requirements.into_iter().collect::<Vec<_>>();
        requirements.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        let mut leases = Vec::with_capacity(requirements.len());
        for (workspace_id, exclusive) in requirements {
            let coordinator = self.workspace_mode_coordinator(&workspace_id);
            let lease = if exclusive {
                coordinator.acquire_exclusive(cancellation)
            } else {
                coordinator.acquire_shared(skip_gitignore, cancellation)
            }?;
            leases.push(lease);
        }
        if shared_base_workspaces.into_iter().any(|workspace| {
            !self.workspace_is_ready(
                workspace,
                skip_gitignore,
                &workspace_readiness_signature(workspace),
            )
        }) {
            drop(leases);
            return self.acquire_workspace_leases(workspaces, skip_gitignore, true, cancellation);
        }
        Some(leases)
    }

    async fn acquire_search_permit(
        &self,
        cancellation: Option<&SearchCancellation>,
    ) -> Option<tokio::sync::OwnedSemaphorePermit> {
        let Some(cancellation) = cancellation else {
            return self.acquire_cpu_permit().await;
        };
        if cancellation.is_cancelled() {
            return None;
        }
        tokio::select! {
            biased;
            () = cancellation.cancelled() => None,
            permit = self.cpu_permits.clone().acquire_owned() => permit.ok(),
        }
    }

    /// Shared search leases for plain (non-worktree) workspaces when every
    /// coordinator can grant immediately. Returns `None` as soon as one is
    /// contended, releasing anything already taken, so callers fall back to
    /// the blocking wait. This keeps the common uncontended query on one
    /// blocking hop instead of two.
    fn try_acquire_search_leases_inline(
        &self,
        workspaces: &[Workspace],
        skip_gitignore: bool,
    ) -> Option<Vec<WorkspaceModeLease>> {
        if workspaces.iter().any(Workspace::is_worktree) {
            return None;
        }
        let mut ids = workspaces
            .iter()
            .map(|workspace| workspace.id.as_str())
            .collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();
        let mut leases = Vec::with_capacity(ids.len());
        for workspace_id in ids {
            let lease = self
                .workspace_mode_coordinator(workspace_id)
                .try_acquire_shared(skip_gitignore)?;
            leases.push(lease);
        }
        Some(leases)
    }

    /// Take the workspace leases for a search before any CPU permit is held.
    /// Uncontended leases are granted inline; otherwise the wait runs on the
    /// blocking pool. A search parked behind an exclusive index lease must not
    /// pin CPU capacity that unrelated workspaces could use.
    async fn acquire_search_leases(
        &self,
        workspaces: &[Workspace],
        skip_gitignore: bool,
        cancellation: Option<&SearchCancellation>,
    ) -> std::result::Result<Option<Vec<WorkspaceModeLease>>, DaemonResponse> {
        // A live watch only covers changes since registration. Catch up with
        // the persisted snapshot before taking search leases or CPU capacity.
        for workspace in workspaces {
            let control = self
                .watchers
                .lock()
                .get(&workspace.id)
                .map(|watch| watch.control.clone());
            let control = match control {
                Some(control) => Some(control),
                None if self.workspace_requires_watcher(workspace) => {
                    let state = self.clone();
                    let workspace = workspace.clone();
                    let registration = tokio::task::spawn_blocking(move || {
                        ensure_watcher(&state, &workspace)?;
                        let control = state
                            .watchers
                            .lock()
                            .get(&workspace.id)
                            .map(|watch| watch.control.clone());
                        Ok::<_, anyhow::Error>(control)
                    })
                    .await
                    .map_err(|err| DaemonResponse::Error {
                        message: format!("watcher readiness task failed: {err}"),
                    })?;
                    // The per-workspace preparation path reports a recorded
                    // registration failure, preserving partial all-index results.
                    registration.ok().flatten()
                }
                None => None,
            };
            if let Some(control) = control {
                let mut readiness = control.readiness.subscribe();
                loop {
                    let current = readiness.borrow().clone();
                    match current {
                        WatchReadiness::Ready => break,
                        WatchReadiness::Failed(_) => break,
                        WatchReadiness::Stopped => {
                            return Err(DaemonResponse::Error {
                                message: format!(
                                    "watcher stopped while reconciling {}",
                                    workspace.root.display()
                                ),
                            });
                        }
                        WatchReadiness::Reconciling => {}
                    }
                    if let Some(cancellation) = cancellation {
                        tokio::select! {
                            biased;
                            () = cancellation.cancelled() => return Ok(None),
                            changed = readiness.changed() => if changed.is_err() { return Ok(None); },
                        }
                    } else if readiness.changed().await.is_err() {
                        return Ok(None);
                    }
                }
            }
        }
        if !cancellation.is_some_and(SearchCancellation::is_cancelled)
            && let Some(leases) = self.try_acquire_search_leases_inline(workspaces, skip_gitignore)
        {
            return Ok(Some(leases));
        }
        let lease_state = self.clone();
        let lease_workspaces = workspaces.to_vec();
        let cancel_flag = cancellation.map(|cancellation| cancellation.flag.clone());
        tokio::task::spawn_blocking(move || {
            lease_state.acquire_workspace_modes_cancellable(
                &lease_workspaces,
                skip_gitignore,
                cancel_flag.as_deref(),
            )
        })
        .await
        .map_err(|join_err| DaemonResponse::Error {
            message: format!("workspace lease task panicked: {join_err:#}"),
        })
    }

    /// Take the exclusive mutation lease for an index run on the blocking
    /// pool, before any CPU permit is held (see `acquire_search_leases`).
    async fn acquire_index_lease(
        &self,
        workspace: &Workspace,
    ) -> std::result::Result<Vec<WorkspaceModeLease>, DaemonResponse> {
        let lease_state = self.clone();
        let lease_workspace = workspace.clone();
        tokio::task::spawn_blocking(move || {
            lease_state.acquire_workspace_mutations(std::slice::from_ref(&lease_workspace))
        })
        .await
        .map_err(|join_err| DaemonResponse::Error {
            message: format!("workspace lease task panicked: {join_err:#}"),
        })
    }

    /// Whether an explicit `Index`/`StartIndex` run is queued or running.
    fn index_in_flight(&self, workspace_id: &str) -> bool {
        self.inflight_indexes.lock().contains_key(workspace_id)
    }

    /// Coalesce explicit `Index` requests per workspace: the first request
    /// leads; later requests with identical options follow the leader's
    /// outcome. Requests with different options fall through to the normal
    /// path and rely on the post-lease generation check to avoid a rescan.
    fn join_or_lead_index(
        &self,
        workspace_id: &str,
        watch: bool,
        skip_gitignore: bool,
    ) -> Option<InflightIndexSlot> {
        let mut inflight = self.inflight_indexes.lock();
        if let Some(existing) = inflight.get(workspace_id) {
            if existing.watch == watch && existing.skip_gitignore == skip_gitignore {
                return Some(InflightIndexSlot::Follow(existing.outcome.clone()));
            }
            return None;
        }
        let (outcome_tx, outcome_rx) = tokio::sync::watch::channel(None);
        inflight.insert(
            workspace_id.to_string(),
            InflightIndex {
                watch,
                skip_gitignore,
                outcome: outcome_rx,
            },
        );
        Some(InflightIndexSlot::Lead(InflightIndexLead {
            workspace_id: workspace_id.to_string(),
            outcome: outcome_tx,
            registry: self.inflight_indexes.clone(),
        }))
    }

    /// Record that a full index walk of `workspace_id` starts now.
    fn note_full_index_run_start(&self, workspace_id: &str) {
        self.full_index_run_starts
            .lock()
            .insert(workspace_id.to_string(), std::time::Instant::now());
    }

    /// Whether a full index walk of `workspace_id` started after `arrived_at`,
    /// i.e. observed a filesystem at least as new as a request arriving then.
    fn full_index_run_started_after(
        &self,
        workspace_id: &str,
        arrived_at: std::time::Instant,
    ) -> bool {
        self.full_index_run_starts
            .lock()
            .get(workspace_id)
            .is_some_and(|started| *started > arrived_at)
    }

    fn register_search(
        &self,
        request_id: Option<uuid::Uuid>,
    ) -> Result<Option<ActiveSearchRegistration>> {
        let Some(request_id) = request_id else {
            return Ok(None);
        };
        let mut registry = self.search_cancellations.lock();
        if matches!(
            registry.entries.get(&request_id),
            Some(SearchCancellationEntry::Active(_))
        ) {
            anyhow::bail!("search request {request_id} is already active");
        }
        let cancellation = match registry.entries.remove(&request_id) {
            Some(SearchCancellationEntry::Tombstone(cancellation)) => {
                registry.tombstones.retain(|id| id != &request_id);
                cancellation
            }
            None => SearchCancellation::new(false),
            Some(SearchCancellationEntry::Active(_)) => unreachable!(),
        };
        registry.entries.insert(
            request_id,
            SearchCancellationEntry::Active(cancellation.clone()),
        );
        drop(registry);
        Ok(Some(ActiveSearchRegistration {
            request_id,
            cancellation,
            registry: self.search_cancellations.clone(),
        }))
    }

    fn cancel_search(&self, request_id: uuid::Uuid) -> Option<SearchCancellation> {
        let mut registry = self.search_cancellations.lock();
        if let Some(entry) = registry.entries.get(&request_id) {
            match entry {
                SearchCancellationEntry::Active(cancellation) => {
                    cancellation.cancel();
                    return Some(cancellation.clone());
                }
                SearchCancellationEntry::Tombstone(cancellation) => cancellation.cancel(),
            }
            return None;
        }

        let cancellation = SearchCancellation::new(true);
        registry
            .entries
            .insert(request_id, SearchCancellationEntry::Tombstone(cancellation));
        registry.tombstones.push_back(request_id);
        while registry.tombstones.len() > MAX_SEARCH_CANCELLATION_TOMBSTONES {
            let Some(expired) = registry.tombstones.pop_front() else {
                break;
            };
            if matches!(
                registry.entries.get(&expired),
                Some(SearchCancellationEntry::Tombstone(_))
            ) {
                registry.entries.remove(&expired);
            }
        }
        None
    }

    pub(crate) fn prepare_context_model(
        &self,
        workspace: &Workspace,
        skip_gitignore: bool,
    ) -> Result<Arc<dyn EmbeddingModel>> {
        if workspace
            .read_metadata()?
            .is_some_and(|metadata| metadata.watch_enabled)
        {
            if !self.watcher_registered(&workspace.id) {
                ensure_watcher(self, workspace)?;
            }
            self.check_watcher_reconciliation(workspace)?;
        }
        let leases = self.acquire_workspace_modes(std::slice::from_ref(workspace), skip_gitignore);
        self.prepare_workspace_for_hybrid_query(workspace, skip_gitignore)?;
        let inner = if self.cached_neural_identity(workspace).is_none() {
            cached_hash_model()
        } else {
            self.maybe_start_model_load();
            self.get_model_for_search(false)?
        };
        Ok(Arc::new(ModeLeasedEmbeddingModel {
            inner,
            _leases: leases,
        }))
    }

    fn resolve_workspace(&self, path: &Path) -> Result<Workspace> {
        if path.is_absolute()
            && let Some(workspace) = self.resolved_workspaces.lock().get(path).cloned()
        {
            return Ok(workspace);
        }

        let workspace = Workspace::resolve(path)?;
        if path == workspace.root {
            self.resolved_workspaces
                .lock()
                .put(path.to_path_buf(), workspace.clone());
        }
        Ok(workspace)
    }

    /// Kick background hash/neural enhancement for `workspaces` without
    /// holding up the search response. The check of whether enhancement is
    /// needed (SQLite counts, vector store sizes, job ledger, worker probe)
    /// and the trigger itself run on a blocking task after the hits are
    /// returned, and each workspace/mode is re-checked at most once per
    /// `ENHANCEMENT_TRIGGER_INTERVAL`.
    fn schedule_search_enhancement(&self, workspaces: Vec<Workspace>, query_uses_neural: bool) {
        if workspaces.is_empty() || !crate::config::background_enhancement_enabled() {
            return;
        }
        let due = self.due_enhancement_workspaces(workspaces, query_uses_neural);
        if due.is_empty() {
            return;
        }
        let trigger = move || {
            for workspace in due {
                if workspace.needs_search_enhancement(query_uses_neural)
                    && let Err(err) =
                        workspace.trigger_background_search_enhancement(query_uses_neural)
                {
                    warn!(
                        "failed to trigger background enhancement for {}: {err:#}",
                        workspace.root.display()
                    );
                }
            }
        };
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                handle.spawn_blocking(trigger);
            }
            Err(_) => trigger(),
        }
    }

    /// Workspaces whose enhancement trigger is due: not attempted for this
    /// mode within `ENHANCEMENT_TRIGGER_INTERVAL`. Records the attempt.
    fn due_enhancement_workspaces(
        &self,
        workspaces: Vec<Workspace>,
        query_uses_neural: bool,
    ) -> Vec<Workspace> {
        let now = std::time::Instant::now();
        let mut triggers = self.enhancement_triggers.lock();
        workspaces
            .into_iter()
            .filter(|workspace| {
                let key = EnhancementTriggerKey {
                    workspace_id: workspace.id.clone(),
                    query_uses_neural,
                };
                let recent = triggers
                    .get(&key)
                    .is_some_and(|last| now.duration_since(*last) < ENHANCEMENT_TRIGGER_INTERVAL);
                if recent {
                    return false;
                }
                triggers.put(key, now);
                true
            })
            .collect()
    }

    fn cached_neural_identity(
        &self,
        workspace: &Workspace,
    ) -> Option<crate::embedding::NeuralModelIdentity> {
        let signature = neural_status_signature(workspace);
        if let Some(status) = self.neural_statuses.lock().get(&workspace.id)
            && status.signature == signature
        {
            return status.identity.clone();
        }

        let identity = workspace_neural_model_identity(workspace);
        self.neural_statuses.lock().put(
            workspace.id.clone(),
            CachedNeuralStatus {
                signature,
                identity: identity.clone(),
            },
        );
        identity
    }

    fn can_precompute_neural_query(
        &self,
        workspaces: &[Workspace],
        model: &dyn EmbeddingModel,
        query: &str,
        force_neural: bool,
    ) -> bool {
        // Automatic neural retrieval is decided after lexical scoring. Eager
        // embedding would waste work for queries that clear the confidence gate.
        force_neural
            && workspaces.len() == 1
            && query_uses_neural(query, force_neural)
            && model.model_identity().is_some_and(|active_identity| {
                self.cached_neural_identity(&workspaces[0]).as_ref() == Some(active_identity)
            })
    }

    fn validate_forced_neural_workspaces(
        &self,
        workspaces: &[Workspace],
        identities: &[Option<crate::embedding::NeuralModelIdentity>],
        force_neural: bool,
    ) -> Result<()> {
        if !force_neural {
            return Ok(());
        }

        let missing = workspaces
            .iter()
            .zip(identities)
            .filter(|(_, identity)| identity.is_none())
            .map(|(workspace, _)| workspace.root.display().to_string())
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            anyhow::bail!(
                "neural search was required, but these workspaces have no neural vectors: {}",
                missing.join(", ")
            );
        }

        let expected = crate::embedding::configured_neural_model_identity();
        let incompatible = workspaces
            .iter()
            .zip(identities)
            .filter(|(_, identity)| identity.as_ref() != Some(&expected))
            .map(|(workspace, _)| workspace.root.display().to_string())
            .collect::<Vec<_>>();
        if !incompatible.is_empty() {
            anyhow::bail!(
                "neural search was required, but these workspaces use an incompatible neural model: {}",
                incompatible.join(", ")
            );
        }
        Ok(())
    }

    /// Try to get the neural model without blocking. If it is not loaded yet,
    /// return a fast hash-based model so searches don't stall during startup.
    fn get_model_or_fallback(&self) -> Arc<dyn EmbeddingModel> {
        match self.lazy_model.get() {
            Some(model) => model.clone(),
            None => cached_hash_model(),
        }
    }

    fn get_model_for_search(&self, force_neural: bool) -> Result<Arc<dyn EmbeddingModel>> {
        if !force_neural {
            return Ok(self.get_model_or_fallback());
        }

        let model = self.lazy_model.get_or_init(create_search_model).clone();
        if model.model_identity().is_none() {
            anyhow::bail!("neural search was required, but the neural model is unavailable");
        }
        Ok(model)
    }

    fn maybe_start_model_load(&self) {
        if self.lazy_model.get().is_some() || self.model_loading.swap(true, Ordering::Relaxed) {
            return;
        }

        let lazy = self.lazy_model.clone();
        let loading = self.model_loading.clone();
        std::thread::spawn(move || {
            daemon_log("loading embedding model...");
            lazy.get_or_init(create_search_model);
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
        let reconciliation_model = cached_hash_model();
        if reconcile_worktree_overlay(workspace, reconciliation_model.as_ref())? {
            self.clear_workspace_contexts(workspace);
        }
        let signature = search_context_signature(workspace, emb_dim, wants_neural);
        self.cached_search_context_for_signature(workspace, emb_dim, wants_neural, signature)
    }

    fn cached_search_context_for_signature(
        &self,
        workspace: &Workspace,
        emb_dim: Option<usize>,
        wants_neural: bool,
        signature: SearchContextSignature,
    ) -> Result<SearchContextLease> {
        let key = SearchContextCacheKey {
            workspace_id: workspace.id.clone(),
            emb_dim,
            wants_neural,
        };

        let pool = {
            let mut cache = self.search_contexts.lock();
            if let Some(entry) = cache.get(&key)
                && entry.signature == signature
            {
                entry.pool.clone()
            } else {
                let pool = Arc::new(SearchContextPool {
                    idle: Mutex::new(Vec::new()),
                    idle_context_count: self.idle_search_context_count.clone(),
                });
                cache.put(
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

    fn prepare_workspace_for_hybrid_query(
        &self,
        workspace: &Workspace,
        skip_gitignore: bool,
    ) -> Result<bool> {
        self.check_watcher_reconciliation(workspace)?;
        let signature = workspace_readiness_signature(workspace);
        if self.workspace_is_ready(workspace, skip_gitignore, &signature) {
            return Ok(false);
        }

        let mut changed = ensure_queryable_workspace(self, workspace, skip_gitignore)?;
        let reconciliation_model = cached_hash_model();
        changed |= reconcile_worktree_overlay(workspace, reconciliation_model.as_ref())?;
        if changed {
            self.clear_workspace_contexts(workspace);
            self.refresh_workspace_watcher(workspace)?;
        }

        self.store_workspace_ready(
            workspace,
            skip_gitignore,
            workspace_readiness_signature(workspace),
        );
        Ok(changed)
    }

    fn check_watcher_reconciliation(&self, workspace: &Workspace) -> Result<()> {
        let control = self
            .watchers
            .lock()
            .get(&workspace.id)
            .map(|watch| watch.control.clone());
        if let Some(control) = control {
            let readiness = control.readiness.borrow().clone();
            match readiness {
                WatchReadiness::Ready => {}
                WatchReadiness::Failed(message) => {
                    anyhow::bail!("watcher reconciliation failed: {message}")
                }
                WatchReadiness::Reconciling => anyhow::bail!(
                    "workspace watcher is reconciling offline changes; retry when indexing completes"
                ),
                WatchReadiness::Stopped => {
                    anyhow::bail!("workspace watcher stopped during reconciliation")
                }
            }
        } else if let Some(error) = self.watcher_last_error(&workspace.id)
            && workspace
                .read_metadata()?
                .is_some_and(|metadata| metadata.watch_enabled)
        {
            anyhow::bail!("watcher unavailable: {error}");
        }
        Ok(())
    }

    fn workspace_requires_watcher(&self, workspace: &Workspace) -> bool {
        let path = workspace.metadata_path();
        let mut before = file_stamp(&path);
        if let Some(stamp) = before
            && let Some(policy) = self.watch_policies.lock().get(&workspace.id)
            && policy.metadata == stamp
        {
            return policy.requires_watcher;
        }
        self.watch_policies.lock().pop(&workspace.id);

        // Policy changes and the first completed index must be visible before
        // a cached query can return. Cache only valid metadata read between
        // matching stamps, never an absent, corrupt, or raced negative result.
        let mut retried = false;
        loop {
            let metadata = workspace.read_metadata().ok().flatten();
            let requires_watcher = metadata.as_ref().is_some_and(|metadata| {
                metadata.watch_enabled && metadata.last_indexed_at_unix.is_some()
            });
            let after = file_stamp(&path);
            if before == after {
                if let Some(stamp) = after
                    && metadata.is_some()
                {
                    self.watch_policies.lock().put(
                        workspace.id.clone(),
                        CachedWatchPolicy {
                            metadata: stamp,
                            requires_watcher,
                        },
                    );
                }
                return requires_watcher;
            }
            if retried {
                // Keep the existing uncached read behavior if another writer
                // keeps replacing metadata; do not retain that raced policy.
                return requires_watcher;
            }
            before = after;
            retried = true;
        }
    }

    fn workspace_is_ready(
        &self,
        workspace: &Workspace,
        skip_gitignore: bool,
        signature: &WorkspaceReadinessSignature,
    ) -> bool {
        let key = workspace_readiness_key(workspace, skip_gitignore);
        self.ready_workspaces
            .lock()
            .get(&key)
            .is_some_and(|cached| cached == signature)
    }

    fn store_workspace_ready(
        &self,
        workspace: &Workspace,
        skip_gitignore: bool,
        signature: WorkspaceReadinessSignature,
    ) {
        let key = workspace_readiness_key(workspace, skip_gitignore);
        self.ready_workspaces.lock().put(key, signature);
    }

    fn clear_workspace_contexts(&self, workspace: &Workspace) {
        {
            let mut contexts = self.search_contexts.lock();
            let keys = contexts
                .iter()
                .filter(|(key, _)| key.workspace_id == workspace.id)
                .map(|(key, _)| key.clone())
                .collect::<Vec<_>>();
            for key in keys {
                contexts.pop(&key);
            }
        }
        self.neural_statuses.lock().pop(&workspace.id);
        self.watch_policies.lock().pop(&workspace.id);
        {
            let mut ready = self.ready_workspaces.lock();
            let keys = ready
                .iter()
                .filter(|(key, _)| key.workspace_id == workspace.id)
                .map(|(key, _)| key.clone())
                .collect::<Vec<_>>();
            for key in keys {
                ready.pop(&key);
            }
        }
        self.query_results.lock().remove_workspace(&workspace.id);
    }

    fn refresh_workspace_watcher(&self, workspace: &Workspace) -> Result<()> {
        let mut watchers = self.watchers.lock();
        if let Some(registration) = watchers.get_mut(&workspace.id) {
            refresh_watch_registration(workspace, registration)?;
        }
        Ok(())
    }

    fn cached_query_results(&self, key: &QueryCacheKey) -> Option<Vec<crate::protocol::SearchHit>> {
        if !self.query_result_cache_enabled {
            return None;
        }
        self.query_results.lock().get(key)
    }

    fn store_query_results(&self, key: QueryCacheKey, hits: &[crate::protocol::SearchHit]) {
        if !self.query_result_cache_enabled {
            return;
        }
        // Don't cache very large result sets (e.g. --no-limit / file_name_only
        // on a big repo): with up to MAX_QUERY_CACHE_ENTRIES of them, each
        // carrying preview/reason strings, this would bloat daemon memory.
        if hits.len() > MAX_CACHEABLE_HITS {
            return;
        }
        self.query_results.lock().insert(key, hits.to_vec());
    }

    fn cached_neural_query(&self, query: &str) -> Option<Vec<f32>> {
        self.neural_queries.lock().get(query)
    }

    fn store_neural_query(&self, query: String, vector: Vec<f32>) {
        self.neural_queries.lock().insert(query, vector);
    }

    fn store_completed_neural_query(
        &self,
        query: String,
        completed: &std::sync::OnceLock<Vec<f32>>,
        options: &SearchOptions,
    ) {
        if options.is_cancelled() {
            return;
        }
        if let Some(vector) = completed.get() {
            self.store_neural_query(query, vector.clone());
        }
    }
}

fn create_daemon_state() -> DaemonState {
    // Defer model creation; model artifact download happens on first neural use.
    let lazy_model: Arc<std::sync::OnceLock<Arc<dyn EmbeddingModel>>> =
        Arc::new(std::sync::OnceLock::new());

    DaemonState {
        lazy_model: lazy_model.clone(),
        model_loading: Arc::new(AtomicBool::new(false)),
        watchers: Arc::new(Mutex::new(HashMap::new())),
        watch_policies: Arc::new(Mutex::new(bounded_lru(MAX_WATCH_POLICIES))),
        resolved_workspaces: Arc::new(Mutex::new(bounded_lru(MAX_RESOLVED_WORKSPACES))),
        neural_statuses: Arc::new(Mutex::new(bounded_lru(MAX_NEURAL_STATUSES))),
        enhancement_triggers: Arc::new(Mutex::new(bounded_lru(MAX_ENHANCEMENT_TRIGGERS))),
        ready_workspaces: Arc::new(Mutex::new(bounded_lru(MAX_READY_WORKSPACES))),
        search_contexts: Arc::new(Mutex::new(bounded_lru(MAX_SEARCH_CONTEXTS))),
        idle_search_context_count: Arc::new(AtomicUsize::new(0)),
        query_results: Arc::new(Mutex::new(QueryResultCache::default())),
        neural_queries: Arc::new(Mutex::new(NeuralQueryCache::default())),
        search_cancellations: Arc::new(Mutex::new(SearchCancellationRegistry::default())),
        workspace_modes: Arc::new(Mutex::new(HashMap::new())),
        inflight_indexes: Arc::new(Mutex::new(HashMap::new())),
        full_index_run_starts: Arc::new(Mutex::new(HashMap::new())),
        query_result_cache_enabled: config::query_result_cache_enabled(),
        cpu_permits: Arc::new(tokio::sync::Semaphore::new(num_cpus::get().max(1))),
        web_server: Arc::new(Mutex::new(None)),
        watcher_recovery: Arc::new(Mutex::new(HashMap::new())),
    }
}

pub async fn run_daemon() -> Result<()> {
    run_daemon_inner().await
}

async fn run_daemon_inner() -> Result<()> {
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
    let _daemon_pid = crate::ipc::write_daemon_pid()?;

    let (listener, socket_path) = crate::ipc::bind().await?;
    daemon_log(&format!(
        "ivygrep daemon listening on {}",
        socket_path.display()
    ));

    let state = create_daemon_state();
    if std::env::var_os("IVYGREP_SKIP_WATCHER_RESTORE").is_none() {
        let restore_state = state.clone();
        tokio::task::spawn_blocking(move || restore_configured_watchers(&restore_state));
        spawn_watcher_supervisor(state.clone());
    }

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
            crate::ipc::cleanup_daemon_pid();
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

pub(crate) async fn handle_web_request(
    state: DaemonState,
    request: DaemonRequest,
) -> DaemonResponse {
    handle_request(state, request).await
}

fn start_web_server(state: &DaemonState, web_config: crate::web::WebConfig) -> Result<String> {
    if let Some(local_addr) = active_web_addr(state) {
        return Ok(crate::web::initial_url(&web_config, local_addr));
    }

    let bind_addr = crate::web::bind_addr(&web_config.host, web_config.port)?;
    let std_listener = std::net::TcpListener::bind(bind_addr)?;
    std_listener.set_nonblocking(true)?;
    let web_listener = tokio::net::TcpListener::from_std(std_listener)?;
    let local_addr = web_listener.local_addr()?;
    let url = crate::web::initial_url(&web_config, local_addr);
    let web_state = state.clone();
    let server_config = web_config.clone();
    let alive = Arc::new(AtomicBool::new(true));
    let server_alive = alive.clone();
    tokio::spawn(async move {
        if let Err(err) = crate::web::serve(web_listener, web_state, server_config).await {
            error!("web server error: {err:#}");
        }
        server_alive.store(false, Ordering::Relaxed);
    });
    *state.web_server.lock() = Some(WebServerRuntime { local_addr, alive });
    Ok(url)
}

fn active_web_addr(state: &DaemonState) -> Option<SocketAddr> {
    let mut guard = state.web_server.lock();
    if guard
        .as_ref()
        .is_some_and(|runtime| !runtime.alive.load(Ordering::Relaxed))
    {
        *guard = None;
    }
    guard.as_ref().map(|runtime| runtime.local_addr)
}

fn restore_configured_watchers(state: &DaemonState) {
    supervise_watchers(state);
}

/// Registers a watcher for every enabled, indexed workspace that has none,
/// honoring each workspace's retry backoff. Runs at startup and then every
/// [`WATCHER_SUPERVISOR_INTERVAL`], so a watcher that failed to start (inotify
/// limits, a root that was temporarily missing) comes back without anyone
/// restarting the daemon.
fn supervise_watchers(state: &DaemonState) {
    let workspaces = match crate::workspace::list_workspace_metadata() {
        Ok(workspaces) => workspaces,
        Err(err) => {
            warn!("failed to enumerate workspaces for watcher supervision: {err:#}");
            return;
        }
    };

    for (index_dir, metadata) in workspaces {
        if !metadata.watch_enabled
            || metadata.last_indexed_at_unix.is_none()
            || state.watcher_registered(&metadata.id)
            || state.watcher_backoff_error(&metadata.id).is_some()
        {
            continue;
        }
        let workspace = match Workspace::resolve(&metadata.root) {
            Ok(workspace) => workspace,
            Err(err) => {
                // No resolved workspace (the root is missing or unreadable),
                // but the ledger lives under the index directory: record the
                // failure where `ig --status` and the next daemon read it.
                let message = format!("{err:#}");
                let ledger_workspace = Workspace::ledger_only(index_dir, &metadata);
                let _ = jobs::finish_job(
                    &ledger_workspace,
                    JobKind::Watcher,
                    "failed",
                    Some(message.clone()),
                );
                state.record_watcher_failure(&metadata.id, message.clone());
                warn!(
                    "failed to restore watcher for {}: {message}",
                    metadata.root.display()
                );
                continue;
            }
        };
        if let Err(err) = ensure_watcher(state, &workspace) {
            warn!(
                "failed to restore watcher for {}: {err:#}",
                workspace.root.display()
            );
        }
    }
}

fn spawn_watcher_supervisor(state: DaemonState) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(WATCHER_SUPERVISOR_INTERVAL).await;
            let state = state.clone();
            let _ = tokio::task::spawn_blocking(move || supervise_watchers(&state)).await;
        }
    });
}

/// Registers (or refreshes) the watcher for `workspace`, recording a failure
/// in the job ledger and the in-memory backoff so the next attempt waits.
/// Callers already inside the backoff window get the last error back without
/// touching the filesystem.
fn ensure_watcher(state: &DaemonState, workspace: &Workspace) -> Result<()> {
    if let Some(error) = state.watcher_backoff_error(&workspace.id) {
        anyhow::bail!("{error}");
    }
    match register_watcher(state, &workspace.root) {
        Ok(()) => {
            state.clear_watcher_failure(&workspace.id);
            Ok(())
        }
        Err(err) => {
            let message = format!("{err:#}");
            let failures = state.record_watcher_failure(&workspace.id, message.clone());
            let _ = jobs::finish_job(workspace, JobKind::Watcher, "failed", Some(message));
            Err(err.context(format!(
                "watcher registration failed ({failures} consecutive attempt{})",
                if failures == 1 { "" } else { "s" }
            )))
        }
    }
}

fn stop_watcher(workspace: &Workspace, registration: WatchRegistration) {
    // Serialize stopping with the initial readiness/liveness publication.
    let _pending = registration.control.pending_work.lock();
    registration.control.active.store(false, Ordering::Relaxed);
    registration
        .control
        .readiness
        .send_replace(WatchReadiness::Stopped);
    registration.control.notify.notify_waiters();
    registration.control.shutdown.notify_one();
    if let Some(nonce) = registration.control.job_nonce() {
        let _ = jobs::finish_job_if_current(workspace, JobKind::Watcher, &nonce, "stopped", None);
    } else {
        let _ = jobs::finish_job(workspace, JobKind::Watcher, "stopped", None);
    }
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
    loop {
        let (response, keep_alive) = match read_daemon_request(&mut reader).await {
            Ok(Some(envelope)) => {
                match handle_client_request(state.clone(), envelope, &mut reader).await {
                    ClientRequestOutcome::Respond(response) => (response, true),
                    // The client went away mid-request; its search was
                    // cancelled and there is nobody left to answer.
                    ClientRequestOutcome::Disconnected => return Ok(()),
                }
            }
            Ok(None) => return Ok(()),
            Err(response) => (response, false),
        };

        let payload = serde_json::to_vec(&response)?;
        reader.get_mut().write_all(&payload).await?;
        reader.get_mut().write_all(b"\n").await?;
        if !keep_alive {
            return Ok(());
        }
    }
}

enum ClientRequestOutcome {
    Respond(DaemonResponse),
    Disconnected,
}

/// Run one request for a connected client. Searches are raced against the
/// client disconnecting (EOF/error on the idle stream) and against the
/// daemon-side deadline; either one trips the request's cancellation token so
/// abandoned work stops holding leases and CPU permits.
async fn handle_client_request<R>(
    state: DaemonState,
    envelope: DaemonRequestEnvelope,
    reader: &mut R,
) -> ClientRequestOutcome
where
    R: AsyncBufRead + Unpin,
{
    let (registration, cancellation) = match register_request_cancellation(&state, &envelope) {
        Ok(prepared) => prepared,
        Err(response) => return ClientRequestOutcome::Respond(response),
    };
    let handler = handle_request_with_cancellation(state, envelope.request, cancellation.clone());
    tokio::pin!(handler);
    let Some(cancellation) = cancellation else {
        let response = handler.await;
        drop(registration);
        return ClientRequestOutcome::Respond(response);
    };

    let deadline = config::search_deadline();
    let deadline_timer = async move {
        match deadline {
            Some(deadline) => tokio::time::sleep(deadline).await,
            None => std::future::pending::<()>().await,
        }
    };
    tokio::pin!(deadline_timer);
    let mut deadline_armed = deadline.is_some();
    let mut watch_client = true;
    let mut client_disconnected = false;
    loop {
        tokio::select! {
            biased;
            response = &mut handler => {
                drop(registration);
                return if client_disconnected {
                    ClientRequestOutcome::Disconnected
                } else {
                    ClientRequestOutcome::Respond(response)
                };
            }
            peek = reader.fill_buf(), if watch_client => {
                watch_client = false;
                match peek {
                    // Pipelined bytes: the client is alive and waiting.
                    Ok(buffered) if !buffered.is_empty() => {}
                    Ok(_) | Err(_) => {
                        client_disconnected = true;
                        tracing::debug!("client disconnected mid-search; cancelling");
                        cancellation.cancel();
                    }
                }
            }
            () = &mut deadline_timer, if deadline_armed => {
                deadline_armed = false;
                warn!(
                    "search exceeded the {}s daemon deadline; returning partial results",
                    deadline.map_or(0, |deadline| deadline.as_secs())
                );
                cancellation.cancel_for_deadline();
            }
        }
    }
}

async fn read_daemon_request<R>(
    reader: &mut R,
) -> std::result::Result<Option<DaemonRequestEnvelope>, DaemonResponse>
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

fn parse_daemon_request(line: &[u8]) -> std::result::Result<DaemonRequestEnvelope, DaemonResponse> {
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
    Ok(envelope)
}

/// Await the outcome a coalesced `Index` leader publishes for its followers.
async fn await_coalesced_index(
    mut outcome: tokio::sync::watch::Receiver<Option<DaemonResponse>>,
) -> DaemonResponse {
    match outcome.wait_for(Option::is_some).await {
        Ok(response) => response.clone().unwrap_or_else(|| DaemonResponse::Error {
            message: "coalesced index request produced no response".to_string(),
        }),
        Err(_) => DaemonResponse::Error {
            message: "coalesced index request aborted".to_string(),
        },
    }
}

/// Make a workspace visible to `ig --status`/`ig_status` as soon as an index
/// run is accepted, so a run parked behind the lease or CPU permits is not
/// reported as absent. Existing metadata is left untouched; the run rewrites
/// it once it holds the lease.
fn register_workspace_for_index(workspace: &Workspace, watch: bool, skip_gitignore: bool) {
    if workspace.ensure_dirs().is_err() {
        return;
    }
    if matches!(workspace.read_metadata(), Ok(None)) {
        let _ = workspace.write_metadata(&crate::workspace::WorkspaceMetadata {
            id: workspace.id.clone(),
            root: workspace.root.clone(),
            created_at_unix: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            last_indexed_at_unix: None,
            watch_enabled: watch,
            skip_gitignore,
            index_generation: 0,
        });
    }
}

/// Run one explicit index request end to end: exclusive workspace lease, CPU
/// permit, index, watcher registration. Shared by the blocking `Index` request
/// and the detached `StartIndex` run; `lead` publishes the outcome to
/// coalesced followers and clears the in-flight entry.
async fn run_index_request(
    state: DaemonState,
    workspace: Workspace,
    watch: bool,
    skip_gitignore: bool,
    lead: Option<InflightIndexLead>,
    arrived_at: std::time::Instant,
) -> DaemonResponse {
    // Generation observed on arrival: if a full walk that started after this
    // request arrived advances it while the request waits for the exclusive
    // lease, the rescan is redundant. Earlier walks cannot vouch for edits made
    // after they scanned.
    let generation_on_arrival = workspace
        .read_metadata()
        .ok()
        .flatten()
        .map(|metadata| metadata.index_generation);

    // Take the exclusive workspace lease before a CPU permit so requests
    // parked behind an in-flight index never pin CPU capacity.
    let mode_leases = match state.acquire_index_lease(&workspace).await {
        Ok(leases) => leases,
        Err(response) => return response,
    };
    // Bound concurrent heavy index work (see #58).
    let permit = state.cpu_permits.clone().acquire_owned().await.ok();
    let index_workspace_target = workspace.clone();
    let index_state = state.clone();
    let index_result = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let _mode_leases = mode_leases;
        index_workspace_target.ensure_dirs()?;
        let mut metadata = index_workspace_target.read_metadata()?.unwrap_or_else(|| {
            crate::workspace::WorkspaceMetadata {
                id: index_workspace_target.id.clone(),
                root: index_workspace_target.root.clone(),
                created_at_unix: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                last_indexed_at_unix: None,
                watch_enabled: watch,
                skip_gitignore,
                index_generation: 0,
            }
        });
        let already_current = metadata.last_indexed_at_unix.is_some()
            && metadata.skip_gitignore == skip_gitignore
            && generation_on_arrival.is_some_and(|arrival| metadata.index_generation > arrival)
            && index_state.full_index_run_started_after(&index_workspace_target.id, arrived_at);
        metadata.skip_gitignore = skip_gitignore;
        metadata.watch_enabled = watch;
        index_workspace_target.write_metadata(&metadata)?;
        if !watch
            && let Some(registration) = index_state
                .watchers
                .lock()
                .remove(&index_workspace_target.id)
        {
            stop_watcher(&index_workspace_target, registration);
        }
        index_state.refresh_workspace_watcher(&index_workspace_target)?;
        // Register before the scan so filesystem edits made during it remain
        // queued. A newly registered watcher cannot certify the old index.
        let watcher_error = watch
            .then(|| register_watcher(&index_state, &index_workspace_target.root))
            .and_then(Result::err)
            .map(|err| format!("indexed but failed to watch: {err:#}"));
        let control = index_state
            .watchers
            .lock()
            .get(&index_workspace_target.id)
            .map(|watch| watch.control.clone());
        let reconcile_startup = control
            .as_ref()
            .is_some_and(|control| control.initial_scan_required.load(Ordering::Relaxed));
        if already_current && !reconcile_startup {
            daemon_log(&format!(
                "index already current for {} (generation {}); skipping redundant rescan",
                index_workspace_target.root.display(),
                metadata.index_generation
            ));
            return Result::<_, anyhow::Error>::Ok((None, watcher_error));
        }
        let hash_model = cached_hash_model();
        index_state.note_full_index_run_start(&index_workspace_target.id);
        let summary = if reconcile_startup {
            index_workspace_for_watcher(&index_workspace_target, hash_model.as_ref())?
        } else {
            index_workspace(&index_workspace_target, hash_model.as_ref())?
        };
        if summary.indexed_files > 0 || summary.deleted_files > 0 {
            index_state.clear_workspace_contexts(&index_workspace_target);
        }
        if let Some(control) = control {
            complete_initial_watch_reconciliation(&control);
        }
        Result::<_, anyhow::Error>::Ok((Some(summary), watcher_error))
    })
    .await
    .unwrap_or_else(|join_err| Err(anyhow::anyhow!(join_err.to_string())));

    let response = match index_result {
        Ok((summary, watcher_error)) => {
            let watcher_result = if watch {
                watcher_error.map_or(Ok(()), Err)
            } else {
                Ok(())
            };
            match (watcher_result, summary) {
                (Err(message), _) => DaemonResponse::Error { message },
                (Ok(()), Some(summary)) => DaemonResponse::Ack {
                    message: format!(
                        "indexed {} files ({} chunks)",
                        summary.indexed_files, summary.total_chunks
                    ),
                },
                (Ok(()), None) => DaemonResponse::Ack {
                    message: "index already current; skipped redundant rescan".to_string(),
                },
            }
        }
        Err(err) => DaemonResponse::Error {
            message: err.to_string(),
        },
    };
    if let Some(lead) = lead {
        lead.publish(&response);
    }
    response
}

async fn handle_request(state: DaemonState, request: DaemonRequest) -> DaemonResponse {
    handle_request_with_cancellation(state, request, None).await
}

fn is_search_request(request: &DaemonRequest) -> bool {
    matches!(
        request,
        DaemonRequest::Search { .. }
            | DaemonRequest::RegexSearch { .. }
            | DaemonRequest::LiteralSearch { .. }
    )
}

/// Every search gets a cancellation token so disconnects and deadlines can
/// stop it. Only requests carrying a `request_id` are registered for explicit
/// `CancelSearch`; the registration keeps the existing tombstone semantics.
fn register_request_cancellation(
    state: &DaemonState,
    envelope: &DaemonRequestEnvelope,
) -> std::result::Result<
    (Option<ActiveSearchRegistration>, Option<SearchCancellation>),
    DaemonResponse,
> {
    if !is_search_request(&envelope.request) {
        return Ok((None, None));
    }
    let registration = state
        .register_search(envelope.request_id)
        .map_err(|error| DaemonResponse::Error {
            message: error.to_string(),
        })?;
    let cancellation = registration
        .as_ref()
        .map(|registration| registration.cancellation.clone())
        .unwrap_or_else(|| SearchCancellation::new(false));
    Ok((registration, Some(cancellation)))
}

#[cfg(test)]
async fn handle_enveloped_request(
    state: DaemonState,
    envelope: DaemonRequestEnvelope,
) -> DaemonResponse {
    let (registration, cancellation) = match register_request_cancellation(&state, &envelope) {
        Ok(prepared) => prepared,
        Err(response) => return response,
    };
    let response = handle_request_with_cancellation(state, envelope.request, cancellation).await;
    drop(registration);
    response
}

async fn handle_request_with_cancellation(
    state: DaemonState,
    request: DaemonRequest,
    cancellation: Option<SearchCancellation>,
) -> DaemonResponse {
    match request {
        DaemonRequest::Version => DaemonResponse::Version {
            version: Some(BUILD_VERSION.to_string()),
        },
        DaemonRequest::RuntimeStatus { path } => {
            let workspace = match path {
                Some(path) => match Workspace::resolve(&path) {
                    Ok(workspace) => {
                        let watch_enabled = workspace
                            .read_metadata()
                            .ok()
                            .flatten()
                            .is_some_and(|metadata| metadata.watch_enabled);
                        let watcher_alive = workspace.is_watcher_alive();
                        let watcher_error = (!watcher_alive)
                            .then(|| {
                                state.watcher_last_error(&workspace.id).or_else(|| {
                                    jobs::job_status(
                                        &workspace,
                                        JobKind::Watcher,
                                        jobs::WATCHER_HEARTBEAT_TTL_SECS,
                                    )
                                    .record
                                    .and_then(|record| record.last_error)
                                })
                            })
                            .flatten();
                        Some(WorkspaceRuntimeStatus {
                            id: workspace.id.clone(),
                            watch_enabled,
                            watcher_alive,
                            watcher_error,
                            index_in_flight: state.index_in_flight(&workspace.id),
                        })
                    }
                    Err(err) => {
                        return DaemonResponse::Error {
                            message: err.to_string(),
                        };
                    }
                },
                None => None,
            };
            DaemonResponse::RuntimeStatus {
                version: Some(BUILD_VERSION.to_string()),
                workspace,
            }
        }
        DaemonRequest::Status => match list_workspaces() {
            Ok(workspaces) => DaemonResponse::Status {
                workspaces,
                version: Some(BUILD_VERSION.to_string()),
            },
            Err(err) => DaemonResponse::Error {
                message: err.to_string(),
            },
        },
        DaemonRequest::ServeWeb {
            host,
            port,
            initial_query,
            initial_path,
        } => {
            match start_web_server(
                &state,
                crate::web::WebConfig {
                    host,
                    port,
                    initial_query,
                    initial_path,
                },
            ) {
                Ok(url) => DaemonResponse::WebStarted { url },
                Err(err) => DaemonResponse::Error {
                    message: err.to_string(),
                },
            }
        }
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

            let arrived_at = std::time::Instant::now();
            // Coalesce concurrent explicit index requests for one workspace:
            // followers with identical options await the leader's response,
            // and may reuse it only when the leader's walk started after they
            // arrived. A follower that arrived mid-walk may have edits the
            // leader scanned past, so it falls through to its own (incremental)
            // rescan once the leader releases the lease.
            let lead = match state.join_or_lead_index(&workspace.id, watch, skip_gitignore) {
                Some(InflightIndexSlot::Follow(outcome)) => {
                    let response = await_coalesced_index(outcome).await;
                    if state.full_index_run_started_after(&workspace.id, arrived_at)
                        || matches!(response, DaemonResponse::Error { .. })
                    {
                        return response;
                    }
                    None
                }
                Some(InflightIndexSlot::Lead(lead)) => Some(lead),
                None => None,
            };
            register_workspace_for_index(&workspace, watch, skip_gitignore);
            run_index_request(state, workspace, watch, skip_gitignore, lead, arrived_at).await
        }
        DaemonRequest::StartIndex {
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
            let generation = workspace
                .read_metadata()
                .ok()
                .flatten()
                .map(|metadata| metadata.index_generation);
            // Only a leader spawns work; followers (and requests whose options
            // differ from the in-flight run) report the existing run and let
            // the client poll `RuntimeStatus` until it clears.
            let already_running =
                match state.join_or_lead_index(&workspace.id, watch, skip_gitignore) {
                    Some(InflightIndexSlot::Lead(lead)) => {
                        register_workspace_for_index(&workspace, watch, skip_gitignore);
                        let root = workspace.root.clone();
                        let arrived_at = std::time::Instant::now();
                        tokio::spawn(async move {
                            let response = run_index_request(
                                state,
                                workspace,
                                watch,
                                skip_gitignore,
                                Some(lead),
                                arrived_at,
                            )
                            .await;
                            if let DaemonResponse::Error { message } = response {
                                daemon_log(&format!(
                                    "background index for {} failed: {message}",
                                    root.display()
                                ));
                            }
                        });
                        false
                    }
                    Some(InflightIndexSlot::Follow(_)) | None => true,
                };
            DaemonResponse::IndexStarted {
                accepted: true,
                already_running,
                generation,
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
            force_neural,
            disable_memory_expansion,
        } => {
            if cancellation
                .as_ref()
                .is_some_and(SearchCancellation::is_cancelled)
            {
                return cancelled_search_response();
            }
            let request_started = std::time::Instant::now();
            let state_clone = state.clone();
            let all_indices = path.is_none();

            let workspace_set = if let Some(ref p) = path {
                match state_clone.resolve_workspace(p) {
                    Ok(workspace) => crate::search_service::SearchWorkspaceSet {
                        workspaces: vec![workspace],
                        warnings: Vec::new(),
                    },
                    Err(err) => {
                        return DaemonResponse::Error {
                            message: err.to_string(),
                        };
                    }
                }
            } else {
                match select_all_indexed_workspaces(|root| state_clone.resolve_workspace(root)) {
                    Ok(workspaces) => workspaces,
                    Err(err) => {
                        return DaemonResponse::Error {
                            message: err.to_string(),
                        };
                    }
                }
            };
            let workspaces = workspace_set.workspaces;
            let workspace_warnings = workspace_set.warnings;

            let options = SearchOptions {
                limit,
                context: context.min(crate::search::MAX_SEARCH_CONTEXT_LINES),
                type_filter,
                include_globs,
                exclude_globs,
                scope_filter: (!all_indices)
                    .then(|| scope_from_request(scope_path, scope_is_file))
                    .flatten(),
                skip_gitignore,
                force_neural,
                progress_tx: None,
                cancel_token: cancellation
                    .as_ref()
                    .map(|cancellation| cancellation.flag.clone()),
            };
            tracing::trace!(
                "daemon_search_resolve={:?} workspaces={}",
                request_started.elapsed(),
                workspaces.len()
            );

            let neural_identities = workspaces
                .iter()
                .map(|workspace| state_clone.cached_neural_identity(workspace))
                .collect::<Vec<_>>();
            if let Err(err) = state_clone.validate_forced_neural_workspaces(
                &workspaces,
                &neural_identities,
                force_neural,
            ) {
                return DaemonResponse::Error {
                    message: err.to_string(),
                };
            }
            let has_neural_vectors = neural_identities.iter().any(Option::is_some);
            if !options.is_cancelled()
                && should_start_model_load(has_neural_vectors, &query, force_neural)
            {
                state_clone.maybe_start_model_load();
            }
            tracing::trace!(
                "daemon_search_validate={:?} neural={}",
                request_started.elapsed(),
                has_neural_vectors
            );

            // Workspace leases come before the CPU permit: a search parked
            // behind an exclusive index lease must not pin CPU capacity.
            let mode_leases = match state_clone
                .acquire_search_leases(&workspaces, options.skip_gitignore, cancellation.as_ref())
                .await
            {
                Ok(Some(leases)) => leases,
                Ok(None) => {
                    return cancelled_search_outcome(cancellation.as_ref(), workspace_warnings);
                }
                Err(response) => return response,
            };
            tracing::trace!("daemon_search_lease={:?}", request_started.elapsed());
            // Bound concurrent heavy search work (see #58). The permit is held
            // for the whole blocking task and released when it completes.
            let Some(permit) = state_clone
                .acquire_search_permit(cancellation.as_ref())
                .await
            else {
                return cancelled_search_outcome(cancellation.as_ref(), workspace_warnings);
            };
            tracing::trace!("daemon_search_permit={:?}", request_started.elapsed());
            let result = tokio::task::spawn_blocking(move || {
                let task_started = std::time::Instant::now();
                let _permit = permit;
                let _mode_leases = mode_leases;
                if options.is_cancelled() {
                    return (Vec::new(), workspace_warnings, 0, query, options, true);
                }
                let model = match state_clone.get_model_for_search(force_neural) {
                    Ok(model) => model,
                    Err(err) => {
                        let mut warnings = workspace_warnings;
                        warnings.push(err.to_string());
                        return (Vec::new(), warnings, 0, query, options, false);
                    }
                };
                tracing::trace!("daemon_search_model={:?}", task_started.elapsed());
                let mut all_hits = Vec::new();
                let mut all_errors = workspace_warnings;
                let mut successful_workspaces = 0usize;
                let mut prepared_workspaces = Vec::with_capacity(workspaces.len());
                for workspace in workspaces {
                    if options.is_cancelled() {
                        break;
                    }
                    match state_clone
                        .prepare_workspace_for_hybrid_query(&workspace, options.skip_gitignore)
                    {
                        Ok(_) => {
                            prepared_workspaces.push(workspace);
                        }
                        Err(err) => {
                            warn!(
                                "failed to prepare index for {}: {err:#}",
                                workspace.root.display()
                            );
                            all_errors.push(format!("{}: {err:#}", workspace.root.display()));
                        }
                    }
                }
                let workspaces = prepared_workspaces;
                tracing::trace!("daemon_search_prepare={:?}", task_started.elapsed());
                if options.is_cancelled() {
                    return (Vec::new(), all_errors, 0, query, options, true);
                }
                let query_uses_neural = query_uses_neural(&query, options.force_neural);
                let workspace_signatures = workspaces
                    .iter()
                    .map(|workspace| {
                        search_context_signature(
                            workspace,
                            Some(model.dimensions()),
                            model.model_identity().is_some(),
                        )
                    })
                    .collect::<Vec<_>>();
                tracing::trace!("daemon_search_signature={:?}", task_started.elapsed());

                let cache_key = query_cache_key(
                    &workspaces,
                    workspace_signatures.clone(),
                    &query,
                    &options,
                    model.dimensions(),
                    model.model_identity().is_some(),
                    all_indices,
                );
                if let Some(cached_hits) = state_clone.cached_query_results(&cache_key) {
                    let cancelled = options.is_cancelled();
                    if !cancelled {
                        state_clone
                            .schedule_search_enhancement(workspaces.clone(), query_uses_neural);
                    }
                    return (
                        cached_hits,
                        all_errors,
                        workspaces.len(),
                        query,
                        options,
                        cancelled,
                    );
                }

                let mut neural_query_cache_write = None;
                let mut neural_query_vector_job = if !options.is_cancelled()
                    && state_clone.can_precompute_neural_query(
                        &workspaces,
                        model.as_ref(),
                        &query,
                        options.force_neural,
                    ) {
                    let neural_query = query.trim().to_string();
                    if let Some(vector) = state_clone.cached_neural_query(&neural_query) {
                        Some(NeuralQueryVectorJob::Ready(vector))
                    } else {
                        let model = model.clone();
                        let completed = Arc::new(std::sync::OnceLock::new());
                        let worker_completed = completed.clone();
                        neural_query_cache_write = Some((neural_query.clone(), completed));
                        Some(NeuralQueryVectorJob::pending(std::thread::spawn(
                            move || {
                                let vector = model.embed(&neural_query);
                                let _ = worker_completed.set(vector.clone());
                                vector
                            },
                        )))
                    }
                } else {
                    None
                };

                for (workspace, signature) in workspaces.iter().zip(workspace_signatures) {
                    if options.is_cancelled() {
                        break;
                    }
                    let context = match state_clone.cached_search_context_for_signature(
                        workspace,
                        Some(model.dimensions()),
                        model.model_identity().is_some(),
                        signature,
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
                    if options.is_cancelled() {
                        break;
                    }
                    tracing::trace!("daemon_search_context={:?}", task_started.elapsed());
                    match hybrid_search_with_context_and_neural_job(
                        &context,
                        workspace,
                        &query,
                        Some(model.as_ref()),
                        &options,
                        neural_query_vector_job.take(),
                    ) {
                        Ok(mut hits) => {
                            successful_workspaces += 1;
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
                    tracing::trace!("daemon_search_hybrid={:?}", task_started.elapsed());
                }
                all_hits.sort_by(|a, b| {
                    b.score
                        .partial_cmp(&a.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                truncate_daemon_search_hits(&mut all_hits, &options);
                drop(neural_query_vector_job.take());
                let cancelled = options.is_cancelled();
                if !cancelled && let Some((query, completed)) = neural_query_cache_write {
                    state_clone.store_completed_neural_query(query, &completed, &options);
                }
                if !cancelled && all_errors.is_empty() {
                    state_clone.store_query_results(cache_key, &all_hits);
                }
                // Background hash and neural enhancement runs off the response path.
                if !cancelled {
                    state_clone.schedule_search_enhancement(workspaces.clone(), query_uses_neural);
                }
                tracing::trace!("daemon_search_task_total={:?}", task_started.elapsed());
                (
                    all_hits,
                    all_errors,
                    successful_workspaces,
                    query,
                    options,
                    cancelled,
                )
            })
            .await;
            let result = match result {
                Ok(result) => result,
                Err(join_err) => {
                    warn!("search task panicked: {join_err:#}");
                    return DaemonResponse::Error {
                        message: format!("search task panicked: {join_err:#}"),
                    };
                }
            };
            tracing::trace!("daemon_search_total={:?}", request_started.elapsed());

            let (
                mut hits,
                mut errors,
                successful_workspaces,
                expansion_query,
                expansion_options,
                cancelled,
            ) = result;
            if cancelled
                || cancellation
                    .as_ref()
                    .is_some_and(SearchCancellation::is_cancelled)
            {
                // The daemon's own deadline returns whatever completed before
                // it fired; client cancellation keeps the explicit error.
                return match deadline_warning(cancellation.as_ref()) {
                    Some(warning) => {
                        errors.push(warning);
                        DaemonResponse::SearchResults {
                            hits,
                            warnings: errors,
                        }
                    }
                    None => cancelled_search_response(),
                };
            }
            if successful_workspaces == 0 && !errors.is_empty() {
                DaemonResponse::Error {
                    message: format!("search failed: {}", errors.join("; ")),
                }
            } else {
                if !disable_memory_expansion
                    && !force_neural
                    && should_expand_memory_query(&expansion_query, &hits, limit)
                {
                    let scope_is_file = expansion_options
                        .scope_filter
                        .as_ref()
                        .is_some_and(|scope| scope.is_file);
                    let expansion_request = DaemonRequest::Search {
                        path,
                        query: String::new(),
                        limit: expansion_options.limit,
                        context: expansion_options.context,
                        type_filter: expansion_options.type_filter,
                        include_globs: expansion_options.include_globs,
                        exclude_globs: expansion_options.exclude_globs,
                        scope_path: expansion_options.scope_filter.map(|scope| scope.rel_path),
                        scope_is_file,
                        skip_gitignore: expansion_options.skip_gitignore,
                        force_neural: expansion_options.force_neural,
                        disable_memory_expansion: true,
                    };
                    let variants = memory_query_variants(&expansion_query);
                    let requests = variants
                        .map(|variant| search_request_with_query(&expansion_request, variant));
                    let (first, second) = tokio::join!(
                        Box::pin(handle_request_with_cancellation(
                            state.clone(),
                            requests[0].clone(),
                            cancellation.clone(),
                        )),
                        Box::pin(handle_request_with_cancellation(
                            state.clone(),
                            requests[1].clone(),
                            cancellation.clone(),
                        )),
                    );
                    if cancellation
                        .as_ref()
                        .is_some_and(SearchCancellation::is_cancelled)
                    {
                        return match deadline_warning(cancellation.as_ref()) {
                            Some(warning) => {
                                errors.push(warning);
                                DaemonResponse::SearchResults {
                                    hits,
                                    warnings: errors,
                                }
                            }
                            None => cancelled_search_response(),
                        };
                    }
                    let mut probe_outputs = Vec::new();
                    for response in [first, second] {
                        match response {
                            DaemonResponse::SearchResults {
                                hits: expanded_hits,
                                ..
                            } if !expanded_hits.is_empty() => probe_outputs.push(expanded_hits),
                            DaemonResponse::SearchResults { .. } => {}
                            DaemonResponse::Error { message } => {
                                warn!("memory query expansion failed: {message}");
                            }
                            other => {
                                warn!("memory query expansion unavailable: {other:?}");
                            }
                        }
                    }
                    hits = fuse_memory_probe_hits(hits, probe_outputs, limit);
                }
                DaemonResponse::SearchResults {
                    hits,
                    warnings: errors,
                }
            }
        }
        DaemonRequest::RegexSearch {
            path,
            pattern,
            limit,
            context,
            type_filter,
            include_globs,
            exclude_globs,
            scope_path,
            scope_is_file,
            skip_gitignore,
        } => {
            if cancellation
                .as_ref()
                .is_some_and(SearchCancellation::is_cancelled)
            {
                return cancelled_search_response();
            }
            let workspace_set = if let Some(ref p) = path {
                match Workspace::resolve(p) {
                    Ok(workspace) => SearchWorkspaceSet {
                        workspaces: vec![workspace],
                        warnings: Vec::new(),
                    },
                    Err(err) => {
                        return DaemonResponse::Error {
                            message: err.to_string(),
                        };
                    }
                }
            } else {
                match select_all_indexed_workspaces(Workspace::resolve) {
                    Ok(workspaces) => workspaces,
                    Err(err) => {
                        return DaemonResponse::Error {
                            message: err.to_string(),
                        };
                    }
                }
            };
            let workspaces = workspace_set.workspaces;
            let workspace_warnings = workspace_set.warnings;

            let all_indices = path.is_none();
            let options = SearchOptions {
                limit,
                context: context.min(crate::search::MAX_SEARCH_CONTEXT_LINES),
                type_filter,
                include_globs,
                exclude_globs,
                scope_filter: (!all_indices)
                    .then(|| scope_from_request(scope_path, scope_is_file))
                    .flatten(),
                skip_gitignore,
                force_neural: false,
                progress_tx: None,
                cancel_token: cancellation
                    .as_ref()
                    .map(|cancellation| cancellation.flag.clone()),
            };
            // Workspace leases come before the CPU permit (see Search).
            let mode_leases = match state
                .acquire_search_leases(&workspaces, skip_gitignore, cancellation.as_ref())
                .await
            {
                Ok(Some(leases)) => leases,
                Ok(None) => {
                    return cancelled_search_outcome(cancellation.as_ref(), workspace_warnings);
                }
                Err(response) => return response,
            };
            // Bound concurrent heavy regex work (see #58).
            let Some(permit) = state.acquire_search_permit(cancellation.as_ref()).await else {
                return cancelled_search_outcome(cancellation.as_ref(), workspace_warnings);
            };
            let state_clone = state.clone();
            let result = tokio::task::spawn_blocking(move || {
                let _permit = permit;
                let _mode_leases = mode_leases;
                let mut batch = SearchBatch::new(workspace_warnings);
                if options.is_cancelled() {
                    return Ok(batch.finish_partial(options.bounded_limit()));
                }
                let mut workspace_options = options.clone();
                if all_indices {
                    workspace_options.context = 0;
                }
                for workspace in &workspaces {
                    if options.is_cancelled() {
                        return Ok(batch.finish_partial(options.bounded_limit()));
                    }
                    let result = (|| {
                        if ensure_queryable_workspace(&state_clone, workspace, skip_gitignore)? {
                            state_clone.refresh_workspace_watcher(workspace)?;
                        }
                        regex_search_with_options(workspace, &pattern, &workspace_options)
                    })();
                    batch.record(&workspace.root, all_indices, result);
                }
                if options.is_cancelled() {
                    return Ok(batch.finish_partial(options.bounded_limit()));
                }
                let mut outcome =
                    finish_daemon_search_batch(batch, &options, HitOrdering::Preserve)
                        .map_err(|err| err.to_string())?;
                if all_indices && !options.is_cancelled() {
                    crate::regex_search::expand_regex_context_absolute_with_options(
                        &mut outcome.hits,
                        options.bounded_context(),
                        &options,
                    );
                }
                Ok(outcome)
            })
            .await
            .unwrap_or_else(|join_err| {
                warn!("regex search task panicked: {join_err:#}");
                Err(join_err.to_string())
            });

            finish_cancellable_search(result, cancellation.as_ref())
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
            if cancellation
                .as_ref()
                .is_some_and(SearchCancellation::is_cancelled)
            {
                return cancelled_search_response();
            }
            let workspace_set = if let Some(ref p) = path {
                match Workspace::resolve(p) {
                    Ok(workspace) => SearchWorkspaceSet {
                        workspaces: vec![workspace],
                        warnings: Vec::new(),
                    },
                    Err(err) => {
                        return DaemonResponse::Error {
                            message: err.to_string(),
                        };
                    }
                }
            } else {
                match select_all_indexed_workspaces(Workspace::resolve) {
                    Ok(workspaces) => workspaces,
                    Err(err) => {
                        return DaemonResponse::Error {
                            message: err.to_string(),
                        };
                    }
                }
            };
            let workspaces = workspace_set.workspaces;
            let workspace_warnings = workspace_set.warnings;

            let all_indices = path.is_none();
            let options = SearchOptions {
                limit,
                context: context.min(crate::search::MAX_SEARCH_CONTEXT_LINES),
                type_filter,
                include_globs,
                exclude_globs,
                scope_filter: (!all_indices)
                    .then(|| scope_from_request(scope_path, scope_is_file))
                    .flatten(),
                skip_gitignore,
                force_neural: false,
                progress_tx: None,
                cancel_token: cancellation
                    .as_ref()
                    .map(|cancellation| cancellation.flag.clone()),
            };

            let state_clone = state.clone();
            // Workspace leases come before the CPU permit (see Search).
            let mode_leases = match state_clone
                .acquire_search_leases(&workspaces, options.skip_gitignore, cancellation.as_ref())
                .await
            {
                Ok(Some(leases)) => leases,
                Ok(None) => {
                    return cancelled_search_outcome(cancellation.as_ref(), workspace_warnings);
                }
                Err(response) => return response,
            };
            // Bound concurrent heavy literal work (see #58).
            let Some(permit) = state_clone
                .acquire_search_permit(cancellation.as_ref())
                .await
            else {
                return cancelled_search_outcome(cancellation.as_ref(), workspace_warnings);
            };
            let result = tokio::task::spawn_blocking(move || {
                let _permit = permit;
                let _mode_leases = mode_leases;
                let mut batch = SearchBatch::new(workspace_warnings);
                if options.is_cancelled() {
                    return Ok(batch.finish_partial(options.bounded_limit()));
                }
                for workspace in &workspaces {
                    if options.is_cancelled() {
                        return Ok(batch.finish_partial(options.bounded_limit()));
                    }
                    let result = (|| {
                        if ensure_queryable_workspace(
                            &state_clone,
                            workspace,
                            options.skip_gitignore,
                        )? {
                            state_clone.clear_workspace_contexts(workspace);
                            state_clone.refresh_workspace_watcher(workspace)?;
                        }
                        let context = state_clone.cached_search_context(workspace, None, false)?;
                        literal_search_with_context(&context, workspace, &query, &options)
                    })();
                    batch.record(&workspace.root, all_indices, result);
                }
                if options.is_cancelled() {
                    return Ok(batch.finish_partial(options.bounded_limit()));
                }
                finish_daemon_search_batch(batch, &options, HitOrdering::Preserve)
                    .map_err(|err| err.to_string())
            })
            .await
            .unwrap_or_else(|join_err| {
                warn!("literal search task panicked: {join_err:#}");
                Err(join_err.to_string())
            });

            finish_cancellable_search(result, cancellation.as_ref())
        }
        DaemonRequest::CancelSearch { search_id } => {
            if let Some(cancellation) = state.cancel_search(search_id) {
                cancellation.finished().await;
            }
            DaemonResponse::Ack {
                message: format!("cancellation requested for search {search_id}"),
            }
        }
        DaemonRequest::Remove { path } => match Workspace::resolve(&path) {
            Ok(workspace) => {
                let workspace_for_cache = workspace.clone();
                let remove_state = state.clone();
                match tokio::task::spawn_blocking(move || {
                    let leases =
                        remove_state.acquire_workspace_mutations(std::slice::from_ref(&workspace));
                    if let Some(registration) = remove_state.watchers.lock().remove(&workspace.id) {
                        stop_watcher(&workspace, registration);
                    }
                    if let Ok(Some(mut metadata)) = workspace.read_metadata() {
                        metadata.watch_enabled = false;
                        let _ = workspace.write_metadata(&metadata);
                    }
                    remove_workspace_index(&workspace)?;
                    Result::<_, anyhow::Error>::Ok(leases)
                })
                .await
                .unwrap_or_else(|join_err| Err(anyhow::anyhow!(join_err.to_string())))
                {
                    Ok(_leases) => {
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
        DaemonRequest::EnsureWatcher { path } => match Workspace::resolve(&path) {
            Ok(workspace) => {
                if state.watcher_registered(&workspace.id) && workspace.is_watcher_alive() {
                    return DaemonResponse::Ack {
                        message: format!("already watching {}", workspace.root.display()),
                    };
                }
                if let Some(error) = state.watcher_backoff_error(&workspace.id) {
                    return DaemonResponse::Error { message: error };
                }
                // Registration walks the tree on inotify platforms; answer now
                // and let the job ledger carry the outcome.
                let ensure_state = state.clone();
                let root = workspace.root.clone();
                tokio::task::spawn_blocking(move || {
                    if let Err(err) = ensure_watcher(&ensure_state, &workspace) {
                        warn!("failed to restart watcher for {}: {err:#}", root.display());
                    }
                });
                DaemonResponse::Ack {
                    message: format!("restarting watcher for {}", path.display()),
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
            crate::ipc::cleanup_daemon_pid();
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
    if let Some(registration) = watchers.get_mut(&workspace.id) {
        refresh_watch_registration(&workspace, registration)?;
        return Ok(());
    }

    let control = Arc::new(WatchControl::new(workspace.clone()));
    let callback_control = control.clone();
    let event_filter = Arc::new(Mutex::new(WatchEventFilter::new(&workspace)));
    let callback_filter = event_filter.clone();
    let external_git_common_dir = {
        let filter = event_filter.lock();
        (!filter.skip_gitignore)
            .then(|| {
                filter
                    .git_exclude_path
                    .as_deref()
                    .and_then(Path::parent)
                    .and_then(Path::parent)
                    .filter(|path| !path.starts_with(&workspace.root))
                    .map(Path::to_path_buf)
            })
            .flatten()
    };
    let external_git_watch = external_git_common_dir
        .as_deref()
        .and_then(external_git_watch_target);

    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        handle_watch_result(&callback_control, &callback_filter, event);
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
    if let Some(git_watch) = &external_git_watch {
        watcher
            .watch(git_watch, RecursiveMode::NonRecursive)
            .map_err(|err| {
                anyhow::anyhow!(
                    "failed watching external Git metadata directory {}: {err:#}",
                    git_watch.display()
                )
            })?;
    }
    let job_nonce = jobs::start_job(&workspace, JobKind::Watcher, "reconciling", 1)
        .ok()
        .and_then(|record| record.nonce);
    control.set_job_nonce(job_nonce);
    update_watcher_job(&control, JobUpdate::default());
    watchers.insert(
        workspace.id.clone(),
        WatchRegistration {
            watcher,
            control: control.clone(),
            event_filter,
            external_git_common_dir,
            external_git_watch,
        },
    );
    drop(watchers);

    spawn_watch_heartbeat(control.clone());
    spawn_watch_worker(state.clone(), control.clone());
    // Separate startup work from actual events: an explicit Index can satisfy
    // this scan without consuming events delivered while its scan was running.
    control.notify.notify_one();

    if let Ok(Some(mut metadata)) = workspace.read_metadata()
        && !metadata.watch_enabled
    {
        metadata.watch_enabled = true;
        let _ = workspace.write_metadata(&metadata);
    }

    daemon_log(&format!("watching {}", workspace.root.display()));

    Ok(())
}

fn refresh_watch_registration(
    workspace: &Workspace,
    registration: &mut WatchRegistration,
) -> Result<()> {
    let desired_filter = WatchEventFilter::new(workspace);
    let desired_common_dir = (!desired_filter.skip_gitignore)
        .then(|| {
            desired_filter
                .git_exclude_path
                .as_deref()
                .and_then(Path::parent)
                .and_then(Path::parent)
                .filter(|path| !path.starts_with(&workspace.root))
                .map(Path::to_path_buf)
        })
        .flatten();
    let desired_watch = desired_common_dir
        .as_deref()
        .and_then(external_git_watch_target);

    if registration.external_git_watch != desired_watch {
        if let Some(desired) = &desired_watch {
            registration
                .watcher
                .watch(desired, RecursiveMode::NonRecursive)
                .map_err(|error| {
                    anyhow::anyhow!(
                        "failed watching external Git metadata directory {}: {error:#}",
                        desired.display()
                    )
                })?;
        }

        if let Some(current) = &registration.external_git_watch
            && let Err(error) = registration.watcher.unwatch(current)
            && !is_missing_watch_error(&error)
        {
            if let Some(desired) = &desired_watch {
                let _ = registration.watcher.unwatch(desired);
            }
            return Err(anyhow::anyhow!(
                "failed removing external Git metadata watch {}: {error:#}",
                current.display()
            ));
        }
    }

    registration.external_git_common_dir = desired_common_dir;
    registration.external_git_watch = desired_watch;
    *registration.event_filter.lock() = desired_filter;
    Ok(())
}

fn external_git_watch_target(common_dir: &Path) -> Option<PathBuf> {
    let info = common_dir.join("info");
    if info.is_dir() {
        Some(info)
    } else if common_dir.is_dir() {
        Some(common_dir.to_path_buf())
    } else {
        None
    }
}

fn is_missing_watch_error(error: &notify::Error) -> bool {
    matches!(
        error.kind,
        notify::ErrorKind::PathNotFound | notify::ErrorKind::WatchNotFound
    )
}

fn log_external_git_watch_error(
    workspace: &Workspace,
    action: &str,
    path: &Path,
    error: &notify::Error,
) {
    warn!(
        "failed {action} external Git metadata watch {} for {}: {error:#}",
        path.display(),
        workspace.root.display()
    );
    daemon_log(&format!(
        "failed {action} external Git metadata watch {} for {}: {error:#}",
        path.display(),
        workspace.root.display()
    ));
}

fn reconcile_external_git_watch(state: &DaemonState, workspace: &Workspace) {
    let mut watchers = state.watchers.lock();
    let Some(registration) = watchers.get_mut(&workspace.id) else {
        return;
    };
    let Some(common_dir) = registration.external_git_common_dir.clone() else {
        return;
    };
    let desired = external_git_watch_target(&common_dir);
    if registration.external_git_watch == desired {
        return;
    }

    let Some(target) = desired else {
        if let Some(current) = registration.external_git_watch.as_ref()
            && let Err(error) = registration.watcher.unwatch(current)
            && !is_missing_watch_error(&error)
        {
            log_external_git_watch_error(workspace, "removing", current, &error);
            return;
        }
        registration.external_git_watch = None;
        return;
    };
    if let Err(error) = registration
        .watcher
        .watch(&target, RecursiveMode::NonRecursive)
    {
        log_external_git_watch_error(workspace, "adding", &target, &error);
        return;
    }

    let Some(current) = registration.external_git_watch.as_ref() else {
        registration.external_git_watch = Some(target);
        return;
    };
    if let Err(error) = registration.watcher.unwatch(current)
        && !is_missing_watch_error(&error)
    {
        log_external_git_watch_error(workspace, "removing", current, &error);
        if let Err(rollback_error) = registration.watcher.unwatch(&target)
            && !is_missing_watch_error(&rollback_error)
        {
            log_external_git_watch_error(workspace, "rolling back", &target, &rollback_error);
        }
        return;
    }
    registration.external_git_watch = Some(target);
}

fn complete_initial_watch_reconciliation(control: &WatchControl) {
    if !control.active.load(Ordering::Relaxed)
        || !control
            .initial_reconciliation_pending
            .load(Ordering::Relaxed)
    {
        return;
    }
    // A successful full scan satisfies only the scan itself. Events delivered
    // after it read their paths still need the ordinary targeted Merkle update.
    control
        .initial_scan_required
        .store(false, Ordering::Relaxed);
    publish_initial_watch_readiness_if_caught_up(control);
}

fn publish_initial_watch_readiness_if_caught_up(control: &WatchControl) {
    if !control
        .initial_reconciliation_pending
        .load(Ordering::Relaxed)
    {
        return;
    }
    let pending = control.pending_work.lock();
    if !control.active.load(Ordering::Relaxed)
        || control.initial_scan_required.load(Ordering::Relaxed)
        || !matches!(pending.change, WatchChange::None)
        || control.indexing.load(Ordering::Relaxed)
        || !control
            .initial_reconciliation_pending
            .swap(false, Ordering::Relaxed)
    {
        return;
    }
    // Keep the queue lock through publication. An event is either accounted for
    // above, including already-claimed work, or arrives after the startup barrier
    // and follows steady-state debounce. The callback queues directly here.
    update_watcher_job(
        control,
        JobUpdate {
            phase: Some("idle".to_string()),
            last_error: Some(None),
            ..Default::default()
        },
    );
    let _ = std::fs::write(
        control.workspace.watcher_pid_path(),
        std::process::id().to_string(),
    );
    control.readiness.send_if_modified(|readiness| {
        if matches!(readiness, WatchReadiness::Stopped) {
            return false;
        }
        *readiness = WatchReadiness::Ready;
        true
    });
}

fn update_watcher_job(control: &WatchControl, mut update: JobUpdate) {
    let reconciling = control
        .initial_reconciliation_pending
        .load(Ordering::Relaxed);
    update.active = Some(control.active.load(Ordering::Relaxed) && !reconciling);
    if reconciling && update.phase.as_deref() != Some("error") {
        update.phase = Some("reconciling".to_string());
    }
    let refreshed = control.job_nonce().is_some_and(|nonce| {
        jobs::heartbeat_job_if_current(&control.workspace, JobKind::Watcher, &nonce, update.clone())
            .ok()
            .flatten()
            .is_some()
    });
    if refreshed || !control.active.load(Ordering::Relaxed) {
        return;
    }
    // The ledger lost this watcher's record (an index rebuild wipes the
    // index directory, including `job.json`) while the watcher kept running.
    // Re-create it under a fresh nonce so status and clients see the watcher
    // as alive instead of treating it as crashed on every query.
    // start_job publishes an active record before the follow-up heartbeat can
    // make it inactive. Keep startup non-live even if that second write fails.
    let phase = if reconciling {
        "reconciling".to_string()
    } else {
        update.phase.clone().unwrap_or_else(|| "idle".to_string())
    };
    if let Ok(record) = jobs::start_job(&control.workspace, JobKind::Watcher, phase, 1) {
        if let Some(nonce) = record.nonce.as_deref() {
            let _ =
                jobs::heartbeat_job_if_current(&control.workspace, JobKind::Watcher, nonce, update);
        }
        control.set_job_nonce(record.nonce);
    }
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
            update_watcher_job(&control, update);
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    });
}

fn spawn_watch_worker(state: DaemonState, control: Arc<WatchControl>) {
    tokio::spawn(async move {
        let mut consecutive_failures = 0u32;
        loop {
            control.notify.notified().await;
            if !control.active.load(Ordering::Relaxed) {
                break;
            }

            if !control.initial_scan_required.load(Ordering::Relaxed) {
                wait_for_watch_quiet(&control).await;
            }
            if !control.active.load(Ordering::Relaxed) {
                break;
            }

            if control.indexing.swap(true, Ordering::Relaxed) {
                continue;
            }

            while control.active.load(Ordering::Relaxed) {
                let Some(pending_work) = control.take_pending_work().or_else(|| {
                    control
                        .initial_scan_required
                        .load(Ordering::Relaxed)
                        .then(PendingWatchWork::default)
                }) else {
                    break;
                };
                control.retrying.store(false, Ordering::Relaxed);
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
                if let Some(error) = &pending_work.backend_error {
                    update.last_error = Some(Some(error.clone()));
                }
                update_watcher_job(&control, update);

                let workspace = control.workspace.clone();
                if matches!(&pending_work.change, WatchChange::FullReconciliation)
                    || control.initial_scan_required.load(Ordering::Relaxed)
                {
                    reconcile_external_git_watch(&state, &workspace);
                }
                let startup_only = matches!(&pending_work.change, WatchChange::None);
                let changed_paths = match pending_work.change {
                    WatchChange::Paths(paths) => paths.into_iter().collect(),
                    WatchChange::FullReconciliation => Vec::new(),
                    WatchChange::None => Vec::new(),
                };
                // Gate watcher-triggered indexing behind the same CPU semaphore
                // as client requests (#58). A multi-repo branch switch / build
                // can dirty many watched workspaces at once; without this, each
                // watcher's indexing spawn_blocking runs unbounded (saturating
                // the rayon chunking pool + the blocking pool), oversubscribing
                // CPU/memory exactly like the client burst #58 fixed.
                // Workspace lease first, CPU permit second: a watcher parked
                // behind an explicit index must not pin CPU capacity.
                let lease_state = state.clone();
                let lease_workspace = workspace.clone();
                let lease_control = control.clone();
                let mode_leases = tokio::task::spawn_blocking(move || {
                    if lease_control.initial_scan_required.load(Ordering::Relaxed) {
                        return Ok(lease_state
                            .acquire_workspace_mutations(std::slice::from_ref(&lease_workspace)));
                    }
                    let skip_gitignore = lease_workspace
                        .read_metadata()?
                        .is_some_and(|metadata| metadata.skip_gitignore);
                    Result::<_, anyhow::Error>::Ok(lease_state.acquire_workspace_modes(
                        std::slice::from_ref(&lease_workspace),
                        skip_gitignore,
                    ))
                })
                .await
                .unwrap_or_else(|join_err| Err(anyhow::anyhow!(join_err.to_string())));
                let result = match mode_leases {
                    Err(err) => Err(err),
                    Ok(mode_leases) => {
                        let permit = state.cpu_permits.clone().acquire_owned().await.ok();
                        let watcher_state = state.clone();
                        let index_control = control.clone();
                        tokio::task::spawn_blocking(move || {
                            let _permit = permit;
                            let _mode_leases = mode_leases;
                            if !index_control.active.load(Ordering::Relaxed)
                                || (startup_only
                                    && !index_control.initial_scan_required.load(Ordering::Relaxed))
                            {
                                return Ok(false);
                            }
                            let hash_model = cached_hash_model();
                            let summary = if changed_paths.is_empty()
                                || index_control.initial_scan_required.load(Ordering::Relaxed)
                            {
                                watcher_state.note_full_index_run_start(&workspace.id);
                                index_workspace_for_watcher(&workspace, hash_model.as_ref())?
                            } else {
                                index_workspace_paths_for_watcher(
                                    &workspace,
                                    hash_model.as_ref(),
                                    &changed_paths,
                                )?
                            };
                            Result::<bool, anyhow::Error>::Ok(
                                summary.indexed_files > 0 || summary.deleted_files > 0,
                            )
                        })
                        .await
                        .unwrap_or_else(|join_err| Err(anyhow::anyhow!(join_err.to_string())))
                    }
                };

                match result {
                    Ok(changed) => {
                        consecutive_failures = 0;
                        if changed {
                            state.clear_workspace_contexts(&control.workspace);
                        }
                        complete_initial_watch_reconciliation(&control);
                        if crate::config::background_enhancement_enabled()
                            && control.workspace.needs_search_enhancement(false)
                        {
                            let _ = control
                                .workspace
                                .trigger_background_search_enhancement(false);
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
                        update_watcher_job(&control, success);
                    }
                    Err(err) => {
                        let error = format!("{err:#}");
                        consecutive_failures = consecutive_failures.saturating_add(1);
                        daemon_log(&format!(
                            "watch update failed for {}: {error}",
                            control.workspace.root.display()
                        ));
                        warn!(
                            "watch-triggered indexing failed for {}: {err:#}",
                            control.workspace.root.display()
                        );
                        let failed = JobUpdate {
                            phase: Some("error".to_string()),
                            last_error: Some(Some(error.clone())),
                            ..Default::default()
                        };
                        update_watcher_job(&control, failed);
                        if control
                            .initial_reconciliation_pending
                            .load(Ordering::Relaxed)
                        {
                            control.readiness.send_if_modified(|readiness| {
                                if matches!(readiness, WatchReadiness::Stopped) {
                                    return false;
                                }
                                *readiness = WatchReadiness::Failed(error.clone());
                                true
                            });
                        }
                        control.requeue_failed_index(error);
                        tokio::select! {
                            () = tokio::time::sleep(watch_retry_delay(consecutive_failures)) => {},
                            () = control.shutdown.notified() => {},
                        }
                    }
                }

                if control.active.load(Ordering::Relaxed) && control.dirty.load(Ordering::Relaxed) {
                    wait_for_watch_quiet(&control).await;
                }
            }

            control.indexing.store(false, Ordering::Relaxed);
            publish_initial_watch_readiness_if_caught_up(&control);
            if !control.active.load(Ordering::Relaxed) {
                break;
            }
            let idle = JobUpdate {
                phase: Some(if control.dirty.load(Ordering::Relaxed) {
                    "dirty".to_string()
                } else {
                    "idle".to_string()
                }),
                ..Default::default()
            };
            update_watcher_job(&control, idle);
        }
    });
}

fn watch_retry_delay(consecutive_failures: u32) -> Duration {
    let multiplier = 1u32 << consecutive_failures.saturating_sub(1).min(16);
    WATCH_RETRY_INITIAL_DELAY
        .saturating_mul(multiplier)
        .min(WATCH_RETRY_MAX_DELAY)
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

        let quiet_period = watch_quiet_period(current);
        if now.duration_since(last_changed) >= quiet_period
            || now.duration_since(started) >= WATCH_MAX_DEBOUNCE
        {
            break;
        }
    }
}

fn watch_quiet_period(pending_events: u64) -> Duration {
    if pending_events <= 1 {
        WATCH_SINGLE_EVENT_QUIET_PERIOD
    } else {
        WATCH_BURST_QUIET_PERIOD
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

fn ensure_queryable_workspace(
    state: &DaemonState,
    workspace: &Workspace,
    skip_gitignore: bool,
) -> Result<bool> {
    state.check_watcher_reconciliation(workspace)?;
    let indexed_filter_is_current =
        workspace_index_matches_skip_gitignore(workspace, skip_gitignore);
    if let Some(mut metadata) = workspace.read_metadata()?
        && (!indexed_filter_is_current || metadata.skip_gitignore != skip_gitignore)
    {
        if metadata.skip_gitignore != skip_gitignore {
            metadata.skip_gitignore = skip_gitignore;
            workspace.write_metadata(&metadata)?;
        }
        state.refresh_workspace_watcher(workspace)?;
    }

    if workspace.quick_index_health().is_queryable() && indexed_filter_is_current {
        return Ok(false);
    }

    let health = workspace.index_health();
    if health.is_queryable() && indexed_filter_is_current {
        return Ok(false);
    }

    if health.is_queryable() {
        let model = cached_hash_model();
        index_workspace(workspace, model.as_ref())?;
        return Ok(true);
    }

    let should_rebuild = match health.state {
        WorkspaceIndexState::Unhealthy => true,
        WorkspaceIndexState::NotIndexed => health.has_indexable_files,
        WorkspaceIndexState::Healthy | WorkspaceIndexState::HealthyEmpty => false,
    };
    if !should_rebuild {
        return Ok(false);
    }

    let mut metadata =
        workspace
            .read_metadata()?
            .unwrap_or_else(|| crate::workspace::WorkspaceMetadata {
                id: workspace.id.clone(),
                root: workspace.root.clone(),
                created_at_unix: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                last_indexed_at_unix: None,
                watch_enabled: false,
                skip_gitignore,
                index_generation: 0,
            });
    metadata.skip_gitignore = skip_gitignore;

    remove_workspace_index(workspace)?;
    workspace.ensure_dirs()?;
    workspace.write_metadata(&metadata)?;
    let model = cached_hash_model();
    index_workspace(workspace, model.as_ref())?;
    Ok(true)
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
            neural_vectors: wants_neural_vectors
                .then(|| file_stamp(&workspace.vector_neural_path()))
                .flatten(),
            neural_model: wants_neural_vectors
                .then(|| file_stamp(&workspace.neural_model_path()))
                .flatten(),
            base_sqlite: file_stamp(&base_dir.join("metadata.sqlite3")),
            base_tantivy: dir_stamp(&base_dir.join("tantivy")),
            base_hash_vectors: wants_hash_vectors
                .then(|| file_stamp(&base_dir.join("vectors.usearch")))
                .flatten(),
            base_neural_vectors: wants_neural_vectors
                .then(|| file_stamp(&base_dir.join("vectors_neural.usearch")))
                .flatten(),
            base_neural_model: wants_neural_vectors
                .then(|| file_stamp(&base_dir.join("neural_model.json")))
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
            neural_model: wants_neural_vectors
                .then(|| file_stamp(&workspace.neural_model_path()))
                .flatten(),
            base_sqlite: None,
            base_tantivy: None,
            base_hash_vectors: None,
            base_neural_vectors: None,
            base_neural_model: None,
        }
    }
}

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
struct EnhancementTriggerKey {
    workspace_id: String,
    query_uses_neural: bool,
}

fn neural_status_signature(workspace: &Workspace) -> NeuralStatusSignature {
    let base_dir = workspace.base_index_dir.as_ref();
    NeuralStatusSignature {
        model: file_stamp(&workspace.neural_model_path()),
        vectors: file_stamp(&workspace.vector_neural_path()),
        base_model: base_dir.and_then(|dir| file_stamp(&dir.join("neural_model.json"))),
        base_vectors: base_dir.and_then(|dir| file_stamp(&dir.join("vectors_neural.usearch"))),
    }
}

fn workspace_readiness_key(
    workspace: &Workspace,
    skip_gitignore: bool,
) -> WorkspaceReadinessCacheKey {
    WorkspaceReadinessCacheKey {
        workspace_id: workspace.id.clone(),
        skip_gitignore,
    }
}

fn workspace_readiness_signature(workspace: &Workspace) -> WorkspaceReadinessSignature {
    let base_dir = workspace.base_index_dir.as_ref();
    WorkspaceReadinessSignature {
        metadata: file_stamp(&workspace.metadata_path()),
        indexed_skip_gitignore: indexed_skip_gitignore(workspace),
        index_format: file_stamp(&workspace.index_format_version_path()),
        sqlite: file_stamp(&workspace.sqlite_path()),
        tantivy: dir_stamp(&workspace.tantivy_dir()),
        hash_vectors: file_stamp(&workspace.vector_path()),
        overlay_sqlite: file_stamp(&workspace.overlay_sqlite_path()),
        overlay_tantivy: dir_stamp(&workspace.overlay_tantivy_dir()),
        overlay_hash_vectors: file_stamp(&workspace.overlay_vector_path()),
        base_ref: file_stamp(&workspace.base_ref_path()),
        base_metadata: base_dir.and_then(|dir| file_stamp(&dir.join("workspace.json"))),
        base_index_format: base_dir.and_then(|dir| file_stamp(&dir.join("index_format_version"))),
        merkle: file_stamp(&workspace.merkle_snapshot_path()),
        indexing_pid: file_stamp(&workspace.indexing_pid_path()),
    }
}

fn query_cache_key(
    workspaces: &[Workspace],
    signatures: Vec<SearchContextSignature>,
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
        signatures,
        all_indices,
        query: query.trim().to_string(),
        limit: options.limit,
        context: options.context,
        type_filter: options.type_filter.clone(),
        include_globs: options.include_globs.clone(),
        exclude_globs: options.exclude_globs.clone(),
        scope_filter: options.scope_filter.clone(),
        skip_gitignore: options.skip_gitignore,
        emb_dim,
        wants_neural,
        force_neural: options.force_neural,
        reranker: crate::reranker::cache_identity(),
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
    if let Some(manifest) = file_stamp(&path.join("meta.json")) {
        return Some(DirStamp {
            files: 1,
            len: manifest.len,
            newest_modified_nanos: manifest.modified_nanos,
        });
    }

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

fn build_root_gitignore(
    root: &Path,
    git_exclude_path: Option<&Path>,
) -> Option<ignore::gitignore::Gitignore> {
    let mut builder = ignore::gitignore::GitignoreBuilder::new(root);
    if let Some(git_exclude) = git_exclude_path.filter(|path| path.exists()) {
        let _ = builder.add(git_exclude);
    }
    let gitignore = root.join(".gitignore");
    if gitignore.exists() {
        let _ = builder.add(&gitignore);
    }
    builder.build().ok()
}

fn is_always_ignored_watch_path(rel: &Path) -> bool {
    rel.components().any(|component| {
        let part = component.as_os_str();
        part == ".git" || part == ".ivygrep"
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

fn is_ig_executable(path: &Path) -> bool {
    path.file_stem()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("ig"))
}

pub fn request_blocking(
    daemon_request: &DaemonRequest,
    autospawn: bool,
) -> Result<Option<DaemonResponse>> {
    request_blocking_with_id(daemon_request, None, autospawn)
}

/// Blocking variant that tags the request with a cancellation id. Search
/// callers should pass a fresh id so a client-side timeout (or an abandoned
/// future) sends `CancelSearch` instead of leaving daemon work running.
pub fn request_blocking_with_id(
    daemon_request: &DaemonRequest,
    request_id: Option<uuid::Uuid>,
    autospawn: bool,
) -> Result<Option<DaemonResponse>> {
    let daemon_request = daemon_request.clone();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        runtime.block_on(async {
            match request_id {
                Some(request_id) => {
                    request_with_id::<fn(String, usize, usize)>(
                        &daemon_request,
                        request_id,
                        autospawn,
                        None,
                    )
                    .await
                }
                None => request::<fn(String, usize, usize)>(&daemon_request, autospawn, None).await,
            }
        })
    })
    .join()
    .map_err(|_| anyhow::anyhow!("daemon request thread panicked"))?
}

pub async fn request<F>(
    request: &DaemonRequest,
    autospawn: bool,
    progress_cb: Option<F>,
) -> Result<Option<DaemonResponse>>
where
    F: FnMut(String, usize, usize) + Send,
{
    if !matches!(
        request,
        DaemonRequest::Version
            | DaemonRequest::RuntimeStatus { .. }
            | DaemonRequest::Status
            | DaemonRequest::Restart
    ) {
        ensure_compatible_daemon().await;
    }

    request_unchecked(request, autospawn, progress_cb).await
}

pub async fn request_with_id<F>(
    request: &DaemonRequest,
    request_id: uuid::Uuid,
    autospawn: bool,
    progress_cb: Option<F>,
) -> Result<Option<DaemonResponse>>
where
    F: FnMut(String, usize, usize) + Send,
{
    ensure_compatible_daemon().await;
    request_unchecked_with_id(request, Some(request_id), autospawn, progress_cb).await
}

async fn ensure_compatible_daemon() {
    if !crate::ipc::socket_exists() {
        return;
    }

    match request_unchecked::<fn(String, usize, usize)>(&DaemonRequest::Version, false, None).await
    {
        Ok(Some(DaemonResponse::Version { version }))
            if version.as_deref() == Some(BUILD_VERSION) => {}
        Ok(Some(_)) => restart_daemon_process().await,
        // A bounded transport probe can fail while a live daemon is overloaded.
        // A response decoding failure means the endpoint speaks an incompatible
        // protocol and must follow the existing restart path.
        Ok(None) => {}
        Err(err) => {
            warn!("daemon compatibility response was invalid: {err:#}");
            restart_daemon_process().await;
        }
    }
}

pub(crate) async fn restart_daemon_process() {
    let restarted =
        request_unchecked::<fn(String, usize, usize)>(&DaemonRequest::Restart, false, None).await;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    if !matches!(restarted, Ok(Some(DaemonResponse::Ack { .. }))) {
        let _ = crate::ipc::terminate_recorded_daemon(std::time::Duration::from_secs(2));
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    if crate::ipc::socket_exists()
        && let Err(err) = crate::ipc::cleanup_stale_socket()
    {
        warn!("failed to inspect stale daemon endpoint after restart: {err:#}");
    }
}

enum DaemonConnectFailure {
    TimedOut,
    Io(std::io::Error),
}

async fn connect_with_timeout<F>(
    connect: F,
    timeout: Duration,
) -> std::result::Result<crate::ipc::IpcStream, DaemonConnectFailure>
where
    F: std::future::Future<Output = std::io::Result<crate::ipc::IpcStream>>,
{
    match tokio::time::timeout(timeout, connect).await {
        Ok(Ok(stream)) => Ok(stream),
        Ok(Err(err)) => Err(DaemonConnectFailure::Io(err)),
        Err(_) => Err(DaemonConnectFailure::TimedOut),
    }
}

fn recover_stale_daemon_endpoint() -> bool {
    match crate::ipc::cleanup_stale_socket() {
        Ok(cleaned) => cleaned,
        Err(err) => {
            warn!("failed to inspect stale daemon endpoint: {err:#}");
            false
        }
    }
}

async fn spawn_daemon_if_missing(request: &DaemonRequest, autospawn: bool) {
    if !autospawn
        || crate::ipc::socket_exists()
        || std::env::var_os("IVYGREP_NO_AUTOSPAWN").is_some()
    {
        return;
    }

    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    if !is_ig_executable(&exe) {
        return;
    }

    let mut cmd = std::process::Command::new(exe);
    cmd.arg("--daemon");
    if matches!(request, DaemonRequest::ServeWeb { .. }) {
        cmd.env("IVYGREP_SKIP_WATCHER_RESTORE", "1");
    }

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
    // Poll for socket readiness (up to 2s).
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if crate::ipc::socket_exists() {
            break;
        }
    }
}

async fn connect_to_daemon(
    request: &DaemonRequest,
    autospawn: bool,
) -> Option<crate::ipc::IpcStream> {
    // At most one stale-endpoint recovery and one retry. Every connection
    // attempt has the same bound, including the first probe.
    for attempt in 0..2 {
        spawn_daemon_if_missing(request, autospawn).await;
        if !crate::ipc::socket_exists() {
            return None;
        }

        match connect_with_timeout(crate::ipc::connect(), DAEMON_CONNECT_TIMEOUT).await {
            Ok(stream) => return Some(stream),
            Err(DaemonConnectFailure::Io(err)) => {
                warn!("daemon connection failed: {err}");
            }
            Err(DaemonConnectFailure::TimedOut) => {
                warn!("daemon connection timed out");
            }
        }

        if attempt > 0 || !recover_stale_daemon_endpoint() {
            return None;
        }
    }

    None
}

async fn request_unchecked<F>(
    request: &DaemonRequest,
    autospawn: bool,
    progress_cb: Option<F>,
) -> Result<Option<DaemonResponse>>
where
    F: FnMut(String, usize, usize) + Send,
{
    request_unchecked_with_id(request, None, autospawn, progress_cb).await
}

async fn request_unchecked_with_id<F>(
    request: &DaemonRequest,
    request_id: Option<uuid::Uuid>,
    autospawn: bool,
    mut progress_cb: Option<F>,
) -> Result<Option<DaemonResponse>>
where
    F: FnMut(String, usize, usize) + Send,
{
    let Some(mut stream) = connect_to_daemon(request, autospawn).await else {
        return Ok(None);
    };

    let envelope = request_id.map_or_else(
        || DaemonRequestEnvelope::new(request.clone()),
        |request_id| DaemonRequestEnvelope::with_request_id(request.clone(), request_id),
    );
    let payload = serde_json::to_vec(&envelope)?;
    if payload.len() > MAX_DAEMON_REQUEST_BYTES {
        anyhow::bail!("daemon request exceeds maximum of {MAX_DAEMON_REQUEST_BYTES} bytes");
    }
    // Timeout writes too — a zombie daemon may accept the connection
    // but never read from it, causing writes to eventually block.
    match tokio::time::timeout(DAEMON_WRITE_TIMEOUT, async {
        stream.write_all(&payload).await?;
        stream.write_all(b"\n").await?;
        Ok::<_, anyhow::Error>(())
    })
    .await
    {
        Ok(Ok(())) => {}
        Ok(Err(err)) => {
            warn!("daemon request write failed: {err:#}");
            recover_stale_daemon_endpoint();
            return Ok(None);
        }
        Err(_) => {
            warn!("daemon request write timed out");
            recover_stale_daemon_endpoint();
            return Ok(None);
        }
    }

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    // Searches tagged with an id are cancelled on the daemon when this client
    // gives up (timeout, transport error) or its future is dropped mid-flight.
    let mut cancel_guard = request_id
        .filter(|_| is_search_request(request))
        .map(SearchCancelOnDrop::new);

    // Timeout varies by request type: Index can take 30+ min on massive repos
    // (large monorepos: 270K+ files), while Status should complete in seconds.
    let timeout_secs = match request {
        DaemonRequest::Index { .. } => 1800,    // 30 min for large repos
        DaemonRequest::StartIndex { .. } => 30, // enqueue only; generous for a loaded daemon
        DaemonRequest::Version
        | DaemonRequest::RuntimeStatus { .. }
        | DaemonRequest::Status
        | DaemonRequest::ServeWeb { .. }
        | DaemonRequest::EnsureWatcher { .. }
        | DaemonRequest::Restart => 5, // quick
        DaemonRequest::Search { .. }
        | DaemonRequest::RegexSearch { .. }
        | DaemonRequest::LiteralSearch { .. }
        | DaemonRequest::CancelSearch { .. } => 120, // wait for active search shutdown
        DaemonRequest::Remove { .. } => 30,     // cleanup
    };

    loop {
        line.clear();
        match tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            reader.read_line(&mut line),
        )
        .await
        {
            Ok(Ok(0)) => {
                // Daemon closed the stream: nothing left to cancel.
                if let Some(guard) = cancel_guard.as_mut() {
                    guard.disarm();
                }
                return Ok(None);
            }
            Ok(Ok(_)) => {}
            Ok(Err(_)) | Err(_) => {
                if let Some(guard) = cancel_guard.take() {
                    warn!("daemon search timed out; requesting cancellation");
                    guard.cancel_now().await;
                }
                return Ok(None);
            }
        }

        if line.trim().is_empty() {
            continue;
        }

        let response: DaemonResponse = match serde_json::from_str(&line) {
            Ok(response) => response,
            Err(err) => {
                // A final line arrived, so the daemon-side search is over.
                if let Some(guard) = cancel_guard.as_mut() {
                    guard.disarm();
                }
                return Err(err.into());
            }
        };
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
            other => {
                if let Some(guard) = cancel_guard.as_mut() {
                    guard.disarm();
                }
                return Ok(Some(other));
            }
        }
    }
}

const DAEMON_CANCEL_TIMEOUT: Duration = Duration::from_secs(3);

/// Client-side guard for an in-flight daemon search. Dropping it before the
/// response arrived (caller aborted, future cancelled) sends a best-effort
/// `CancelSearch`; explicit timeouts call `cancel_now` and wait briefly.
struct SearchCancelOnDrop {
    request_id: uuid::Uuid,
    armed: bool,
}

impl SearchCancelOnDrop {
    fn new(request_id: uuid::Uuid) -> Self {
        Self {
            request_id,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }

    async fn cancel_now(mut self) {
        self.armed = false;
        cancel_daemon_search(self.request_id).await;
    }
}

impl Drop for SearchCancelOnDrop {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let request_id = self.request_id;
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(cancel_daemon_search(request_id));
        }
    }
}

/// Best-effort `CancelSearch` for an abandoned request. Bounded so a wedged
/// daemon cannot hold the client; the daemon also cancels on disconnect.
async fn cancel_daemon_search(request_id: uuid::Uuid) {
    let request = DaemonRequest::CancelSearch {
        search_id: request_id,
    };
    // Boxed: this runs from inside the request path it calls back into.
    let _ = tokio::time::timeout(
        DAEMON_CANCEL_TIMEOUT,
        Box::pin(request_unchecked::<fn(String, usize, usize)>(
            &request, false, None,
        )),
    )
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    use serial_test::serial;
    use tempfile::tempdir;

    use crate::embedding::create_hash_model;
    use crate::indexer::{
        index_workspace, index_workspace_for_watcher, index_workspace_paths_for_watcher,
        open_sqlite_readonly, workspace_is_indexed,
    };
    use crate::search::{
        SearchOptions, hybrid_search, literal_search_with_context,
        validate_forced_neural_workspaces,
    };
    use crate::workspace::WorkspaceMetadata;

    fn test_state() -> DaemonState {
        DaemonState {
            lazy_model: Arc::new(std::sync::OnceLock::new()),
            model_loading: Arc::new(AtomicBool::new(false)),
            watchers: Arc::new(Mutex::new(HashMap::new())),
            watch_policies: Arc::new(Mutex::new(bounded_lru(MAX_WATCH_POLICIES))),
            resolved_workspaces: Arc::new(Mutex::new(bounded_lru(MAX_RESOLVED_WORKSPACES))),
            neural_statuses: Arc::new(Mutex::new(bounded_lru(MAX_NEURAL_STATUSES))),
            enhancement_triggers: Arc::new(Mutex::new(bounded_lru(MAX_ENHANCEMENT_TRIGGERS))),
            ready_workspaces: Arc::new(Mutex::new(bounded_lru(MAX_READY_WORKSPACES))),
            search_contexts: Arc::new(Mutex::new(bounded_lru(MAX_SEARCH_CONTEXTS))),
            idle_search_context_count: Arc::new(AtomicUsize::new(0)),
            query_results: Arc::new(Mutex::new(QueryResultCache::default())),
            neural_queries: Arc::new(Mutex::new(NeuralQueryCache::default())),
            search_cancellations: Arc::new(Mutex::new(SearchCancellationRegistry::default())),
            workspace_modes: Arc::new(Mutex::new(HashMap::new())),
            inflight_indexes: Arc::new(Mutex::new(HashMap::new())),
            full_index_run_starts: Arc::new(Mutex::new(HashMap::new())),
            query_result_cache_enabled: true,
            cpu_permits: Arc::new(tokio::sync::Semaphore::new(num_cpus::get().max(1))),
            web_server: Arc::new(Mutex::new(None)),
            watcher_recovery: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[test]
    fn search_cancellation_tombstone_closes_registration_race() {
        let state = test_state();
        let request_id = uuid::Uuid::new_v4();

        let _ = state.cancel_search(request_id);
        let registration = state.register_search(Some(request_id)).unwrap().unwrap();

        assert!(registration.cancellation.is_cancelled());
        assert!(state.search_cancellations.lock().tombstones.is_empty());
        drop(registration);
        assert!(state.search_cancellations.lock().entries.is_empty());
    }

    #[test]
    fn search_cancellation_registry_rejects_duplicates_and_bounds_tombstones() {
        let state = test_state();
        let active_id = uuid::Uuid::from_u128(1);
        let registration = state.register_search(Some(active_id)).unwrap().unwrap();
        assert!(state.register_search(Some(active_id)).is_err());

        let _ = state.cancel_search(active_id);
        assert!(registration.cancellation.is_cancelled());
        drop(registration);

        for value in 2..=(MAX_SEARCH_CANCELLATION_TOMBSTONES as u128 + 2) {
            let _ = state.cancel_search(uuid::Uuid::from_u128(value));
        }
        let registry = state.search_cancellations.lock();
        assert_eq!(
            registry.tombstones.len(),
            MAX_SEARCH_CANCELLATION_TOMBSTONES
        );
        assert_eq!(registry.entries.len(), MAX_SEARCH_CANCELLATION_TOMBSTONES);
        assert!(!registry.entries.contains_key(&uuid::Uuid::from_u128(2)));
    }

    #[tokio::test]
    async fn pre_cancelled_search_envelope_does_not_search_or_cache() {
        let state = test_state();
        let request_id = uuid::Uuid::new_v4();
        let _ = state.cancel_search(request_id);
        let request = DaemonRequest::Search {
            path: Some(PathBuf::from("/path/that/must/not/be-resolved")),
            query: "needle".to_string(),
            limit: Some(10),
            context: 0,
            type_filter: None,
            include_globs: Vec::new(),
            exclude_globs: Vec::new(),
            scope_path: None,
            scope_is_file: false,
            skip_gitignore: false,
            force_neural: false,
            disable_memory_expansion: true,
        };

        let response = handle_enveloped_request(
            state.clone(),
            DaemonRequestEnvelope::with_request_id(request, request_id),
        )
        .await;

        assert!(matches!(
            response,
            DaemonResponse::Error { message } if message == "search cancelled"
        ));
        assert!(state.query_results.lock().results.is_empty());
        assert!(state.search_cancellations.lock().entries.is_empty());
    }

    #[tokio::test]
    async fn cancelled_searches_leave_semaphore_queue_for_latest_request() {
        let mut state = test_state();
        state.cpu_permits = Arc::new(tokio::sync::Semaphore::new(0));
        let mut registrations = Vec::new();
        let mut waiters = Vec::new();

        for value in 1..=8 {
            let request_id = uuid::Uuid::from_u128(value);
            let registration = state.register_search(Some(request_id)).unwrap().unwrap();
            let cancellation = registration.cancellation.clone();
            let waiter_state = state.clone();
            waiters.push(tokio::spawn(async move {
                waiter_state
                    .acquire_search_permit(Some(&cancellation))
                    .await
                    .is_none()
            }));
            registrations.push(registration);
            let _ = state.cancel_search(request_id);
        }

        for waiter in waiters {
            assert!(
                tokio::time::timeout(Duration::from_secs(1), waiter)
                    .await
                    .unwrap()
                    .unwrap()
            );
        }

        let latest_id = uuid::Uuid::from_u128(9);
        let latest = state.register_search(Some(latest_id)).unwrap().unwrap();
        let latest_cancellation = latest.cancellation.clone();
        let latest_state = state.clone();
        let latest_waiter = tokio::spawn(async move {
            latest_state
                .acquire_search_permit(Some(&latest_cancellation))
                .await
        });
        state.cpu_permits.add_permits(1);
        assert!(
            tokio::time::timeout(Duration::from_secs(1), latest_waiter)
                .await
                .unwrap()
                .unwrap()
                .is_some()
        );

        drop(latest);
        drop(registrations);
        assert!(state.search_cancellations.lock().entries.is_empty());
    }

    #[tokio::test]
    async fn cancel_request_stops_enveloped_search_waiting_for_cpu() {
        let mut state = test_state();
        state.cpu_permits = Arc::new(tokio::sync::Semaphore::new(0));
        let root = tempdir().unwrap();
        let request_id = uuid::Uuid::new_v4();
        let request = DaemonRequest::Search {
            path: Some(root.path().to_path_buf()),
            query: "needle".to_string(),
            limit: Some(10),
            context: 0,
            type_filter: None,
            include_globs: Vec::new(),
            exclude_globs: Vec::new(),
            scope_path: None,
            scope_is_file: false,
            skip_gitignore: false,
            force_neural: false,
            disable_memory_expansion: true,
        };
        let search_state = state.clone();
        let search = tokio::spawn(async move {
            handle_enveloped_request(
                search_state,
                DaemonRequestEnvelope::with_request_id(request, request_id),
            )
            .await
        });

        for _ in 0..100 {
            if state
                .search_cancellations
                .lock()
                .entries
                .contains_key(&request_id)
            {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(
            state
                .search_cancellations
                .lock()
                .entries
                .contains_key(&request_id)
        );

        let cancel = handle_request(
            state.clone(),
            DaemonRequest::CancelSearch {
                search_id: request_id,
            },
        )
        .await;
        assert!(matches!(cancel, DaemonResponse::Ack { .. }));
        let response = tokio::time::timeout(Duration::from_secs(1), search)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            response,
            DaemonResponse::Error { message } if message == "search cancelled"
        ));
        assert!(state.search_cancellations.lock().entries.is_empty());
    }

    fn git(root: &Path, args: &[&str]) {
        let output = std::process::Command::new("git")
            .args(["-c", "commit.gpgSign=false"])
            .args(args)
            .current_dir(root)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn indexed_file_contains(workspace: &Workspace, path: &str, needle: &str) -> bool {
        let conn = open_sqlite_readonly(&workspace.sqlite_path()).unwrap();
        let mut stmt = conn
            .prepare("SELECT text FROM chunks WHERE file_path = ?1")
            .unwrap();
        stmt.query_map([path], |row| row.get::<_, Vec<u8>>(0))
            .unwrap()
            .any(|row| crate::indexer::decompress_text(row.unwrap()).contains(needle))
    }

    fn indexed_literal_visible(workspace: &Workspace, needle: &str) -> Option<bool> {
        let context = SearchContext::load(workspace, None, false).ok()?;
        let hits = literal_search_with_context(
            &context,
            workspace,
            needle,
            &SearchOptions {
                limit: Some(5),
                ..Default::default()
            },
        )
        .ok()?;
        Some(!hits.is_empty())
    }

    async fn wait_for_literal_visibility(
        workspace: &Workspace,
        needle: &str,
        expected: bool,
    ) -> bool {
        for _ in 0..60 {
            if indexed_literal_visible(workspace, needle) == Some(expected) {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        false
    }

    fn test_hit(path: &str, score: f32) -> SearchHit {
        test_hit_at(path, score, 1, path)
    }

    fn test_hit_at(path: &str, score: f32, start_line: usize, preview: &str) -> SearchHit {
        SearchHit {
            file_path: PathBuf::from(path),
            start_line,
            end_line: start_line + 1,
            preview: preview.to_string(),
            reason: String::new(),
            score,
            sources: vec!["test".to_string()],
            neural_requested: false,
            neural_executed: false,
        }
    }

    #[test]
    fn daemon_search_aggregation_applies_global_result_cap() {
        let mut batch = SearchBatch::new(Vec::new());
        for workspace in ["/one", "/two"] {
            let hits = (0..600)
                .map(|index| test_hit(&format!("src/{index}.rs"), 1.0))
                .collect();
            batch.record(Path::new(workspace), true, Ok(hits));
        }
        let options = SearchOptions {
            limit: Some(crate::search::MAX_SEARCH_RESULT_LIMIT + 500),
            ..Default::default()
        };

        let outcome = finish_daemon_search_batch(batch, &options, HitOrdering::Preserve).unwrap();

        assert_eq!(outcome.hits.len(), crate::search::MAX_SEARCH_RESULT_LIMIT);

        let mut hybrid_hits = (0..1_200)
            .map(|index| test_hit(&format!("src/{index}.rs"), 1.0))
            .collect();
        truncate_daemon_search_hits(&mut hybrid_hits, &options);
        assert_eq!(hybrid_hits.len(), crate::search::MAX_SEARCH_RESULT_LIMIT);
    }

    #[test]
    fn memory_expansion_requires_natural_query_and_bounded_note_results() {
        let query = "What should I remember before planning this weekend trip?";
        let note_hits = (0..5)
            .map(|index| test_hit(&format!("notes/{index}.md"), 10.0 - index as f32))
            .collect::<Vec<_>>();
        assert!(should_expand_memory_query(query, &note_hits, Some(20)));
        assert!(!should_expand_memory_query(
            "weekend trip",
            &note_hits,
            Some(20)
        ));
        assert!(!should_expand_memory_query(
            query,
            &note_hits,
            Some(usize::MAX)
        ));

        let mut mixed_hits = note_hits;
        mixed_hits[3] = test_hit("src/planner.rs", 7.0);
        mixed_hits[4] = test_hit("src/calendar.rs", 6.0);
        assert!(!should_expand_memory_query(query, &mixed_hits, Some(20)));

        mixed_hits[3] = test_hit("notes/3.md", 7.0);
        mixed_hits.swap(0, 4);
        assert!(!should_expand_memory_query(query, &mixed_hits, Some(20)));
    }

    #[test]
    fn memory_probe_fusion_rewards_files_found_by_multiple_probes() {
        let fused = fuse_memory_probe_hits(
            vec![test_hit("a.md", 2.0), test_hit("b.md", 1.0)],
            vec![vec![test_hit("b.md", 2.0), test_hit("c.md", 1.0)]],
            Some(20),
        );
        assert_eq!(fused[0].file_path, PathBuf::from("b.md"));
        assert!(fused[0].sources.iter().any(|source| source == "memory"));
    }

    #[test]
    fn memory_probe_fusion_keeps_the_best_matching_snippet() {
        let fused = fuse_memory_probe_hits(
            vec![test_hit_at("a.md", 0.2, 1, "original passage")],
            vec![
                vec![test_hit_at("a.md", 0.9, 20, "matching memory")],
                vec![test_hit_at("a.md", 0.7, 30, "other memory")],
            ],
            Some(20),
        );
        assert_eq!(fused.len(), 1);
        assert_eq!(fused[0].start_line, 20);
        assert_eq!(fused[0].preview, "matching memory");
    }

    #[test]
    fn memory_probe_fusion_anchors_the_original_ranking() {
        let fused = fuse_memory_probe_hits(
            vec![test_hit("z-original.md", 1.0)],
            vec![vec![test_hit("a-probe.md", 1.0)]],
            Some(20),
        );
        assert_eq!(fused[0].file_path, PathBuf::from("z-original.md"));
    }

    #[test]
    fn memory_probe_fusion_preserves_original_when_all_probes_fail() {
        let original = vec![test_hit("a.md", 2.0), test_hit("b.md", 1.0)];
        let fused = fuse_memory_probe_hits(original, Vec::new(), Some(20));
        assert_eq!(fused.len(), 2);
        assert_eq!(fused[0].score, 2.0);
        assert_eq!(fused[0].sources, vec!["test"]);
    }

    #[test]
    fn memory_probe_fusion_respects_requested_file_limit() {
        let fused = fuse_memory_probe_hits(
            vec![test_hit("a.md", 2.0), test_hit("b.md", 1.0)],
            vec![vec![test_hit("c.md", 2.0), test_hit("d.md", 1.0)]],
            Some(2),
        );
        assert_eq!(group_hits_by_file(&fused, None).len(), 2);
    }

    #[test]
    fn memory_probe_fusion_respects_default_file_limit() {
        let original = (0..60)
            .map(|index| test_hit(&format!("original-{index}.md"), 100.0 - index as f32))
            .collect();
        let probe = (0..60)
            .map(|index| test_hit(&format!("probe-{index}.md"), 100.0 - index as f32))
            .collect();
        let fused = fuse_memory_probe_hits(original, vec![probe], None);
        assert_eq!(fused.len(), DEFAULT_SEARCH_LIMIT);
        assert_eq!(group_hits_by_file(&fused, None).len(), DEFAULT_SEARCH_LIMIT);
    }

    #[test]
    fn memory_probes_overfetch_with_a_bounded_limit() {
        let request = DaemonRequest::Search {
            path: None,
            query: "original".to_string(),
            limit: Some(20),
            context: 2,
            type_filter: None,
            include_globs: Vec::new(),
            exclude_globs: Vec::new(),
            scope_path: None,
            scope_is_file: false,
            skip_gitignore: false,
            force_neural: false,
            disable_memory_expansion: false,
        };
        let DaemonRequest::Search {
            query,
            limit,
            disable_memory_expansion,
            ..
        } = search_request_with_query(&request, "expanded".to_string())
        else {
            unreachable!();
        };
        assert_eq!(query, "expanded");
        assert_eq!(limit, Some(MAX_MEMORY_PROBE_LIMIT));
        assert!(disable_memory_expansion);
    }

    #[test]
    fn memory_probe_queries_have_a_unicode_safe_length_bound() {
        let query = "é".repeat(MAX_MEMORY_PROBE_QUERY_CHARS + 1);
        let variants = memory_query_variants(&query);
        assert_eq!(
            variants[0].matches('é').count(),
            MAX_MEMORY_PROBE_QUERY_CHARS
        );
    }

    #[tokio::test]
    #[serial]
    async fn daemon_search_applies_default_memory_expansion() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let repo = tempdir().unwrap();
        for index in 0..5 {
            std::fs::write(
                repo.path().join(format!("memory-{index}.md")),
                format!(
                    "# Weekend trip note\nRemember planning preference {index} for quiet travel."
                ),
            )
            .unwrap();
        }
        let workspace = Workspace::resolve(repo.path()).unwrap();
        let model = create_hash_model();
        index_workspace(&workspace, model.as_ref()).unwrap();

        let request = DaemonRequest::Search {
            path: Some(workspace.root.clone()),
            query: "What should I remember before planning my weekend trip?".to_string(),
            limit: Some(5),
            context: 2,
            type_filter: None,
            include_globs: Vec::new(),
            exclude_globs: Vec::new(),
            scope_path: None,
            scope_is_file: false,
            skip_gitignore: false,
            force_neural: false,
            disable_memory_expansion: false,
        };
        let response = handle_request(test_state(), request).await;
        let DaemonResponse::SearchResults { hits, .. } = response else {
            panic!("expected search results");
        };
        assert!(
            hits.iter()
                .any(|hit| hit.sources.iter().any(|source| source == "memory")),
            "daemon-backed clients should receive default memory expansion"
        );
    }

    #[test]
    fn neural_query_cache_normalizes_and_bounds_entries() {
        let mut cache = NeuralQueryCache::default();
        cache.insert("  same query  ".to_string(), vec![1.0, 2.0]);
        assert_eq!(cache.get("same query"), Some(vec![1.0, 2.0]));

        for index in 0..=MAX_NEURAL_QUERY_CACHE_ENTRIES {
            cache.insert(format!("query {index}"), vec![index as f32]);
        }

        assert_eq!(cache.vectors.len(), MAX_NEURAL_QUERY_CACHE_ENTRIES);
        assert!(cache.get("same query").is_none());
        assert!(cache.get("query 0").is_none());
        assert_eq!(
            cache.get(&format!("query {MAX_NEURAL_QUERY_CACHE_ENTRIES}")),
            Some(vec![MAX_NEURAL_QUERY_CACHE_ENTRIES as f32])
        );
    }

    #[test]
    fn frequently_accessed_queries_survive_cache_churn() {
        let mut neural = NeuralQueryCache::default();
        neural.insert("hot query".to_string(), vec![1.0]);
        let mut results = QueryResultCache::default();
        let options = SearchOptions::default();
        let key =
            |query: &str| query_cache_key(&[], Vec::new(), query, &options, 256, false, false);
        let hot = key("hot query");
        results.insert(hot.clone(), Vec::new());

        for index in 0..MAX_QUERY_CACHE_ENTRIES {
            neural.insert(format!("cold query {index}"), vec![index as f32]);
            results.insert(key(&format!("cold query {index}")), Vec::new());
            assert_eq!(neural.get("hot query"), Some(vec![1.0]));
            assert!(results.get(&hot).is_some());
        }
    }

    #[test]
    fn readiness_cache_evicts_cold_workspaces_without_global_flushes() {
        let state = test_state();
        let workspace = Workspace {
            id: "hot-workspace".to_string(),
            root: PathBuf::from("/nonexistent/hot-workspace"),
            index_dir: PathBuf::from("/nonexistent/hot-index"),
            repo_id: None,
            base_index_dir: None,
        };
        let signature = workspace_readiness_signature(&workspace);
        state.store_workspace_ready(&workspace, false, signature.clone());

        for index in 0..MAX_READY_WORKSPACES {
            let mut cold = workspace.clone();
            cold.id = format!("cold-workspace-{index}");
            state.store_workspace_ready(&cold, false, signature.clone());
            assert!(state.workspace_is_ready(&workspace, false, &signature));
        }

        assert_eq!(state.ready_workspaces.lock().len(), MAX_READY_WORKSPACES);
    }

    #[test]
    #[serial]
    fn readiness_cache_invalidates_when_index_artifacts_change() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let repo = tempdir().unwrap();
        let workspace = Workspace::resolve(repo.path()).unwrap();
        workspace.ensure_dirs().unwrap();

        let state = test_state();
        let signature = workspace_readiness_signature(&workspace);
        assert!(!state.workspace_is_ready(&workspace, false, &signature));
        state.store_workspace_ready(&workspace, false, signature.clone());
        assert!(state.workspace_is_ready(&workspace, false, &signature));

        workspace.write_index_format_version().unwrap();
        let changed = workspace_readiness_signature(&workspace);
        assert!(!state.workspace_is_ready(&workspace, false, &changed));
    }

    #[test]
    fn neural_model_load_follows_query_routing() {
        assert!(!should_start_model_load(true, "SearchContext", false));
        assert!(should_start_model_load(
            true,
            "where is search context loaded",
            false
        ));
        assert!(should_start_model_load(true, "SearchContext", true));
        assert!(!should_start_model_load(
            false,
            "where is search context loaded",
            false
        ));
    }

    #[test]
    #[serial]
    fn clearing_workspace_contexts_clears_readiness_cache() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let repo = tempdir().unwrap();
        let workspace = Workspace::resolve(repo.path()).unwrap();
        workspace.ensure_dirs().unwrap();

        let state = test_state();
        let signature = workspace_readiness_signature(&workspace);
        state.store_workspace_ready(&workspace, false, signature.clone());
        assert!(state.workspace_is_ready(&workspace, false, &signature));

        state.clear_workspace_contexts(&workspace);
        assert!(!state.workspace_is_ready(&workspace, false, &signature));
    }

    #[test]
    #[serial]
    fn clearing_workspace_contexts_preserves_other_query_results() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let first_repo = tempdir().unwrap();
        let second_repo = tempdir().unwrap();
        let first = Workspace::resolve(first_repo.path()).unwrap();
        let second = Workspace::resolve(second_repo.path()).unwrap();
        let state = test_state();
        let options = SearchOptions::default();
        let first_key = query_cache_key(
            std::slice::from_ref(&first),
            Vec::new(),
            "first",
            &options,
            256,
            false,
            false,
        );
        let second_key = query_cache_key(
            std::slice::from_ref(&second),
            Vec::new(),
            "second",
            &options,
            256,
            false,
            false,
        );
        let combined_key = query_cache_key(
            &[first.clone(), second],
            Vec::new(),
            "both",
            &options,
            256,
            false,
            true,
        );
        state.store_query_results(first_key.clone(), &[]);
        state.store_query_results(second_key.clone(), &[]);
        state.store_query_results(combined_key.clone(), &[]);

        state.clear_workspace_contexts(&first);

        let cache = state.query_results.lock();
        assert!(!cache.results.contains(&first_key));
        assert!(!cache.results.contains(&combined_key));
        assert!(cache.results.contains(&second_key));
        assert_eq!(cache.results.len(), 1);
    }

    fn write_broken_completed_index_metadata(workspace: &Workspace, skip_gitignore: bool) {
        workspace.ensure_dirs().unwrap();
        workspace
            .write_metadata(&WorkspaceMetadata {
                id: workspace.id.clone(),
                root: workspace.root.clone(),
                created_at_unix: 0,
                last_indexed_at_unix: Some(1),
                watch_enabled: false,
                skip_gitignore,
                index_generation: 0,
            })
            .unwrap();
    }

    struct TestNeuralModel;

    impl EmbeddingModel for TestNeuralModel {
        fn dimensions(&self) -> usize {
            self.model_identity().unwrap().dimensions
        }

        fn embed(&self, _text: &str) -> Vec<f32> {
            let mut vector = vec![0.0; self.dimensions()];
            vector[0] = 1.0;
            vector
        }

        fn model_identity(&self) -> Option<&crate::embedding::NeuralModelIdentity> {
            static IDENTITY: std::sync::OnceLock<crate::embedding::NeuralModelIdentity> =
                std::sync::OnceLock::new();
            Some(IDENTITY.get_or_init(crate::embedding::configured_neural_model_identity))
        }
    }

    struct BlockingNeuralModel {
        active: Arc<AtomicUsize>,
        release: Mutex<std::sync::mpsc::Receiver<()>>,
    }

    impl EmbeddingModel for BlockingNeuralModel {
        fn dimensions(&self) -> usize {
            TestNeuralModel.dimensions()
        }

        fn embed(&self, _text: &str) -> Vec<f32> {
            self.active.fetch_add(1, Ordering::SeqCst);
            self.release.lock().recv().unwrap();
            self.active.fetch_sub(1, Ordering::SeqCst);
            let mut vector = vec![0.0; self.dimensions()];
            vector[0] = 1.0;
            vector
        }

        fn model_identity(&self) -> Option<&crate::embedding::NeuralModelIdentity> {
            TestNeuralModel.model_identity()
        }
    }

    #[tokio::test]
    #[serial]
    async fn forced_neural_cancellation_keeps_precompute_bounded_and_uncached() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let repo = tempdir().unwrap();
        std::fs::write(
            repo.path().join("search.rs"),
            "pub fn forced_neural_cancellation() {}\n",
        )
        .unwrap();
        let workspace = Workspace::resolve(repo.path()).unwrap();
        let hash_model = create_hash_model();
        index_workspace(&workspace, hash_model.as_ref()).unwrap();
        crate::indexer::enhance_workspace_neural(&workspace, &TestNeuralModel).unwrap();

        let active = Arc::new(AtomicUsize::new(0));
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let model: Arc<dyn EmbeddingModel> = Arc::new(BlockingNeuralModel {
            active: active.clone(),
            release: Mutex::new(release_rx),
        });
        let lazy_model = std::sync::OnceLock::new();
        assert!(lazy_model.set(model).is_ok());
        let mut state = test_state();
        state.lazy_model = Arc::new(lazy_model);
        state.cpu_permits = Arc::new(tokio::sync::Semaphore::new(1));

        let request_id = uuid::Uuid::new_v4();
        let query = "forced neural cancellation".to_string();
        let request = DaemonRequest::Search {
            path: Some(workspace.root.clone()),
            query: query.clone(),
            limit: Some(10),
            context: 0,
            type_filter: None,
            include_globs: Vec::new(),
            exclude_globs: Vec::new(),
            scope_path: None,
            scope_is_file: false,
            skip_gitignore: false,
            force_neural: true,
            disable_memory_expansion: true,
        };
        let search_state = state.clone();
        let search = tokio::spawn(async move {
            handle_enveloped_request(
                search_state,
                DaemonRequestEnvelope::with_request_id(request, request_id),
            )
            .await
        });

        tokio::time::timeout(Duration::from_secs(5), async {
            while active.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let cancel_state = state.clone();
        let cancel = tokio::spawn(async move {
            handle_request(
                cancel_state,
                DaemonRequest::CancelSearch {
                    search_id: request_id,
                },
            )
            .await
        });
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let cancelled = state
                    .search_cancellations
                    .lock()
                    .entries
                    .get(&request_id)
                    .is_some_and(|entry| match entry {
                        SearchCancellationEntry::Active(cancellation)
                        | SearchCancellationEntry::Tombstone(cancellation) => {
                            cancellation.is_cancelled()
                        }
                    });
                if cancelled {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(!cancel.is_finished());
        assert_eq!(active.load(Ordering::SeqCst), 1);
        assert_eq!(state.cpu_permits.available_permits(), 0);
        assert!(state.cached_neural_query(&query).is_none());

        release_tx.send(()).unwrap();
        let response = tokio::time::timeout(Duration::from_secs(5), search)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            response,
            DaemonResponse::Error { message } if message == "search cancelled"
        ));
        let cancel = tokio::time::timeout(Duration::from_secs(5), cancel)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(cancel, DaemonResponse::Ack { .. }));
        assert_eq!(active.load(Ordering::SeqCst), 0);
        assert_eq!(state.cpu_permits.available_permits(), 1);
        assert!(state.cached_neural_query(&query).is_none());
    }

    #[test]
    #[serial]
    fn daemon_caches_only_exact_absolute_workspace_roots() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let repo = tempdir().unwrap();
        let git_dir = repo.path().join(".git");
        std::fs::create_dir_all(git_dir.join("objects")).unwrap();
        std::fs::create_dir(git_dir.join("refs")).unwrap();
        std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        let repo_root = repo.path().canonicalize().unwrap();
        let nested = repo_root.join("src");
        std::fs::create_dir(&nested).unwrap();

        let state = test_state();
        let workspace = state.resolve_workspace(&repo_root).unwrap();
        assert_eq!(workspace.root, repo_root);
        assert_eq!(state.resolved_workspaces.lock().len(), 1);

        state.resolved_workspaces.lock().clear();
        let nested_workspace = state.resolve_workspace(&nested).unwrap();
        assert_eq!(nested_workspace.root, repo_root);
        assert!(
            state.resolved_workspaces.lock().is_empty(),
            "subpaths must still perform full workspace resolution"
        );
    }

    #[test]
    #[serial]
    fn enhancement_triggers_are_rate_limited_per_workspace_and_mode() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let repo_a = tempdir().unwrap();
        let repo_b = tempdir().unwrap();
        let a = Workspace::resolve(repo_a.path()).unwrap();
        let b = Workspace::resolve(repo_b.path()).unwrap();
        let state = test_state();

        let due = state.due_enhancement_workspaces(vec![a.clone(), b.clone()], false);
        assert_eq!(
            due.iter().map(|ws| ws.id.clone()).collect::<Vec<_>>(),
            vec![a.id.clone(), b.id.clone()],
            "first attempt is due for every workspace"
        );
        assert!(
            state
                .due_enhancement_workspaces(vec![a.clone(), b.clone()], false)
                .is_empty(),
            "a second query within the interval does not re-probe or re-spawn"
        );
        let neural_due = state.due_enhancement_workspaces(vec![a.clone()], true);
        assert_eq!(neural_due.len(), 1, "neural mode is tracked separately");

        state.enhancement_triggers.lock().put(
            EnhancementTriggerKey {
                workspace_id: a.id.clone(),
                query_uses_neural: false,
            },
            std::time::Instant::now() - ENHANCEMENT_TRIGGER_INTERVAL,
        );
        assert_eq!(
            state.due_enhancement_workspaces(vec![a, b], false).len(),
            1,
            "only the workspace whose interval elapsed is due again"
        );
    }

    #[test]
    #[serial]
    fn daemon_neural_status_cache_tracks_vector_store_changes() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let repo = tempdir().unwrap();
        std::fs::write(
            repo.path().join("search.rs"),
            "pub fn cached_neural_search() {}\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(repo.path()).unwrap();
        let hash_model = create_hash_model();
        index_workspace(&workspace, hash_model.as_ref()).unwrap();

        let state = test_state();
        assert!(
            !state.can_precompute_neural_query(
                std::slice::from_ref(&workspace),
                &TestNeuralModel,
                "cached neural search",
                false,
            ),
            "hash-only workspaces must not start neural query embedding"
        );

        crate::indexer::enhance_workspace_neural(&workspace, &TestNeuralModel).unwrap();

        assert!(state.cached_neural_identity(&workspace).is_some());
        assert!(!state.can_precompute_neural_query(
            std::slice::from_ref(&workspace),
            &TestNeuralModel,
            "cached neural search",
            false,
        ));
        assert!(state.can_precompute_neural_query(
            std::slice::from_ref(&workspace),
            &TestNeuralModel,
            "cached neural search",
            true,
        ));
        assert_eq!(state.neural_statuses.lock().len(), 1);

        std::fs::remove_file(workspace.vector_neural_path()).unwrap();
        assert!(
            state.cached_neural_identity(&workspace).is_none(),
            "vector-store deletion must invalidate cached neural readiness"
        );
        assert!(!state.can_precompute_neural_query(
            std::slice::from_ref(&workspace),
            &TestNeuralModel,
            "cached neural search",
            true,
        ));
    }

    #[test]
    #[serial]
    fn neural_identity_changes_invalidate_search_context_and_query_caches() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let repo = tempdir().unwrap();
        std::fs::write(repo.path().join("lib.rs"), "pub fn model_identity() {}\n").unwrap();

        let workspace = Workspace::resolve(repo.path()).unwrap();
        let hash_model = create_hash_model();
        index_workspace(&workspace, hash_model.as_ref()).unwrap();
        crate::indexer::enhance_workspace_neural(&workspace, &TestNeuralModel).unwrap();

        let state = test_state();
        let original_signature = search_context_signature(&workspace, Some(256), true);
        let original_context = state
            .cached_search_context(&workspace, Some(256), true)
            .unwrap();
        let original_pool = original_context.pool.clone();
        drop(original_context);

        let options = SearchOptions::default();
        let original_key = query_cache_key(
            std::slice::from_ref(&workspace),
            vec![original_signature.clone()],
            "model identity",
            &options,
            256,
            true,
            false,
        );
        state.store_query_results(original_key, &[]);

        std::fs::remove_file(workspace.neural_model_path()).unwrap();
        let changed_signature = search_context_signature(&workspace, Some(256), true);
        assert_ne!(
            original_signature, changed_signature,
            "removing neural model metadata must change the search signature"
        );

        let changed_key = query_cache_key(
            std::slice::from_ref(&workspace),
            vec![changed_signature],
            "model identity",
            &options,
            256,
            true,
            false,
        );
        assert!(state.cached_query_results(&changed_key).is_none());

        let changed_context = state
            .cached_search_context(&workspace, Some(256), true)
            .unwrap();
        assert!(!Arc::ptr_eq(&original_pool, &changed_context.pool));
        assert!(changed_context.neural_model.is_none());
    }

    #[test]
    #[serial]
    fn worktree_search_signature_tracks_base_neural_model_identity() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let repositories = tempdir().unwrap();
        let main = repositories.path().join("main");
        let linked = repositories.path().join("linked");
        std::fs::create_dir(&main).unwrap();
        git(&main, &["init", "-b", "main"]);
        std::fs::write(main.join("lib.rs"), "pub fn base_identity() {}\n").unwrap();
        git(&main, &["add", "lib.rs"]);
        git(&main, &["commit", "-m", "seed base identity"]);

        let base = Workspace::resolve(&main).unwrap();
        let hash_model = create_hash_model();
        index_workspace(&base, hash_model.as_ref()).unwrap();
        crate::indexer::enhance_workspace_neural(&base, &TestNeuralModel).unwrap();

        git(
            &main,
            &[
                "worktree",
                "add",
                "--detach",
                linked.to_str().unwrap(),
                "HEAD",
            ],
        );
        std::fs::write(linked.join("local.rs"), "pub fn branch_identity() {}\n").unwrap();
        let overlay = Workspace::resolve(&linked).unwrap();
        index_workspace(&overlay, hash_model.as_ref()).unwrap();

        let original = search_context_signature(&overlay, Some(256), true);
        std::fs::remove_file(base.neural_model_path()).unwrap();
        let changed = search_context_signature(&overlay, Some(256), true);
        assert_ne!(
            original, changed,
            "base neural identity changes must invalidate worktree search caches"
        );
    }

    #[test]
    fn cancelled_neural_precompute_joins_without_populating_cache() {
        let state = test_state();
        let cancellation = Arc::new(AtomicBool::new(false));
        let options = SearchOptions {
            cancel_token: Some(cancellation.clone()),
            ..SearchOptions::default()
        };
        let completed = Arc::new(std::sync::OnceLock::new());
        let worker_completed = completed.clone();
        let active = Arc::new(AtomicUsize::new(0));
        let worker_active = active.clone();
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let job = NeuralQueryVectorJob::pending(std::thread::spawn(move || {
            worker_active.fetch_add(1, Ordering::SeqCst);
            started_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            let vector = vec![1.0; 3];
            let _ = worker_completed.set(vector.clone());
            worker_active.fetch_sub(1, Ordering::SeqCst);
            vector
        }));
        started_rx.recv().unwrap();

        cancellation.store(true, Ordering::SeqCst);
        release_tx.send(()).unwrap();
        drop(job);
        state.store_completed_neural_query("cancelled".to_string(), &completed, &options);

        assert_eq!(active.load(Ordering::SeqCst), 0);
        assert!(state.cached_neural_query("cancelled").is_none());
    }

    #[test]
    fn force_neural_waits_for_an_inflight_model_load() {
        let state = test_state();
        let lazy_model = state.lazy_model.clone();
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let loader_barrier = barrier.clone();
        let loader = std::thread::spawn(move || {
            lazy_model.get_or_init(|| {
                loader_barrier.wait();
                std::thread::sleep(Duration::from_millis(50));
                Arc::new(TestNeuralModel) as Arc<dyn EmbeddingModel>
            });
        });

        barrier.wait();
        let started = std::time::Instant::now();
        let model = state.get_model_for_search(true).unwrap();
        loader.join().unwrap();

        assert!(started.elapsed() >= Duration::from_millis(40));
        assert!(model.model_identity().is_some());
    }

    #[test]
    fn force_neural_rejects_a_hash_model() {
        let state = test_state();
        assert!(state.lazy_model.set(cached_hash_model()).is_ok());
        assert!(state.get_model_for_search(true).is_err());
    }

    #[test]
    #[serial]
    fn context_model_keeps_hash_only_workspaces_on_hash_vectors() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let repo = tempdir().unwrap();
        std::fs::write(repo.path().join("lib.rs"), "pub fn context() {}\n").unwrap();
        let workspace = Workspace::resolve(repo.path()).unwrap();
        let hash_model = create_hash_model();
        index_workspace(&workspace, hash_model.as_ref()).unwrap();

        let state = test_state();
        assert!(
            state
                .lazy_model
                .set(Arc::new(TestNeuralModel) as Arc<dyn EmbeddingModel>)
                .is_ok()
        );
        let model = state.prepare_context_model(&workspace, false).unwrap();
        assert!(model.model_identity().is_none());
    }

    #[test]
    #[serial]
    fn force_neural_requires_vectors_in_every_workspace() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let neural_repo = tempdir().unwrap();
        std::fs::write(
            neural_repo.path().join("neural.rs"),
            "pub fn neural_ready() {}\n",
        )
        .unwrap();
        let neural_workspace = Workspace::resolve(neural_repo.path()).unwrap();
        let hash_model = create_hash_model();
        index_workspace(&neural_workspace, hash_model.as_ref()).unwrap();
        crate::indexer::enhance_workspace_neural(&neural_workspace, &TestNeuralModel).unwrap();
        assert!(neural_workspace.has_neural_vectors());

        let hash_repo = tempdir().unwrap();
        std::fs::write(hash_repo.path().join("hash.rs"), "pub fn hash_only() {}\n").unwrap();
        let hash_workspace = Workspace::resolve(hash_repo.path()).unwrap();
        index_workspace(&hash_workspace, hash_model.as_ref()).unwrap();
        assert!(!hash_workspace.has_neural_vectors());

        assert!(
            validate_forced_neural_workspaces(std::slice::from_ref(&neural_workspace), true)
                .is_ok()
        );
        let err = validate_forced_neural_workspaces(
            &[neural_workspace.clone(), hash_workspace.clone()],
            true,
        )
        .unwrap_err();
        assert!(
            err.to_string()
                .contains(&hash_workspace.root.display().to_string())
        );

        crate::indexer::enhance_workspace_neural(&hash_workspace, &TestNeuralModel).unwrap();
        let mut incompatible_identity = hash_workspace.neural_model_identity().unwrap();
        incompatible_identity.model_id = "incompatible/test-model".to_string();
        std::fs::write(
            hash_workspace.neural_model_path(),
            serde_json::to_vec_pretty(&incompatible_identity).unwrap(),
        )
        .unwrap();
        assert!(hash_workspace.has_neural_vectors());

        let err =
            validate_forced_neural_workspaces(&[neural_workspace, hash_workspace.clone()], true)
                .unwrap_err();
        assert!(err.to_string().contains("incompatible neural model"));
        assert!(
            err.to_string()
                .contains(&hash_workspace.root.display().to_string())
        );

        let overlay_root = tempdir().unwrap();
        let overlay_index = tempdir().unwrap();
        std::fs::write(
            overlay_index.path().join("neural_model.json"),
            serde_json::to_vec_pretty(&crate::embedding::configured_neural_model_identity())
                .unwrap(),
        )
        .unwrap();
        let overlay_workspace = Workspace {
            id: "overlay".to_string(),
            root: overlay_root.path().to_path_buf(),
            index_dir: overlay_index.path().to_path_buf(),
            repo_id: None,
            base_index_dir: Some(hash_workspace.index_dir.clone()),
        };
        let err = validate_forced_neural_workspaces(std::slice::from_ref(&overlay_workspace), true)
            .unwrap_err();
        assert!(err.to_string().contains("incompatible neural model"));
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
    fn daemon_autospawn_recognizes_unix_and_windows_binary_names() {
        assert!(is_ig_executable(Path::new("/usr/local/bin/ig")));
        assert!(is_ig_executable(Path::new("ig.exe")));
        assert!(!is_ig_executable(Path::new("/usr/local/bin/ivygrep")));
    }

    #[tokio::test]
    #[serial]
    async fn daemon_connection_failure_preserves_endpoint_owned_by_live_daemon() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        crate::config::ensure_app_dirs().unwrap();

        let daemon_lock = crate::ipc::acquire_daemon_lock()
            .unwrap()
            .expect("test should acquire daemon lock");
        let endpoint = crate::ipc::socket_path().unwrap();
        std::fs::write(&endpoint, b"not a daemon endpoint").unwrap();

        let response =
            request_unchecked::<fn(String, usize, usize)>(&DaemonRequest::Status, false, None)
                .await
                .unwrap();

        assert!(response.is_none());
        assert!(
            endpoint.exists(),
            "client must not unlink an endpoint while daemon lock is held"
        );

        drop(daemon_lock);
        crate::ipc::cleanup_socket();
    }

    #[tokio::test]
    async fn daemon_connect_attempt_is_bounded() {
        let started = std::time::Instant::now();
        let result = connect_with_timeout(
            std::future::pending::<std::io::Result<crate::ipc::IpcStream>>(),
            Duration::from_millis(20),
        )
        .await;

        assert!(matches!(result, Err(DaemonConnectFailure::TimedOut)));
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "stalled connection must respect timeout"
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
            serde_json::to_vec(&DaemonRequestEnvelope::new(DaemonRequest::Version)).unwrap();
        assert!(matches!(
            serde_json::from_slice::<DaemonRequest>(&versioned).unwrap(),
            DaemonRequest::Version
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
    async fn lightweight_runtime_status_reports_version_without_full_status() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let repo = tempdir().unwrap();
        let workspace = Workspace::resolve(repo.path()).unwrap();
        workspace.ensure_dirs().unwrap();
        workspace
            .write_metadata(&WorkspaceMetadata {
                id: workspace.id.clone(),
                root: workspace.root.clone(),
                created_at_unix: 1,
                last_indexed_at_unix: Some(1),
                watch_enabled: true,
                skip_gitignore: false,
                index_generation: 1,
            })
            .unwrap();

        let version = handle_request(test_state(), DaemonRequest::Version).await;
        assert!(matches!(
            version,
            DaemonResponse::Version { version, .. } if version.as_deref() == Some(BUILD_VERSION)
        ));

        let runtime = handle_request(
            test_state(),
            DaemonRequest::RuntimeStatus {
                path: Some(workspace.root.clone()),
            },
        )
        .await;
        let DaemonResponse::RuntimeStatus {
            version,
            workspace: Some(runtime_workspace),
        } = runtime
        else {
            panic!("expected runtime status, got {runtime:?}");
        };
        assert_eq!(version.as_deref(), Some(BUILD_VERSION));
        assert_eq!(runtime_workspace.id, workspace.id);
        assert!(runtime_workspace.watch_enabled);
        assert!(!runtime_workspace.watcher_alive);
    }

    #[tokio::test]
    async fn daemon_reads_multiple_requests_from_one_connection() {
        let request =
            serde_json::to_string(&DaemonRequestEnvelope::new(DaemonRequest::Status)).unwrap();
        let payload = format!("{request}\n{request}\n");
        let mut reader = BufReader::new(payload.as_bytes());

        assert!(matches!(
            read_daemon_request(&mut reader).await.unwrap(),
            Some(DaemonRequestEnvelope {
                request: DaemonRequest::Status,
                ..
            })
        ));
        assert!(matches!(
            read_daemon_request(&mut reader).await.unwrap(),
            Some(DaemonRequestEnvelope {
                request: DaemonRequest::Status,
                ..
            })
        ));
        assert!(read_daemon_request(&mut reader).await.unwrap().is_none());
    }

    #[tokio::test]
    #[serial]
    async fn non_status_request_restarts_outdated_daemon_before_dispatch() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        crate::config::ensure_app_dirs().unwrap();

        let (listener, _) = crate::ipc::bind().await.unwrap();
        let server = tokio::spawn(async move {
            let mut request_types = Vec::new();
            let responses = [
                serde_json::json!({
                    "type": "version",
                    "version": "0.10.1"
                }),
                serde_json::json!({
                    "type": "ack",
                    "message": "restarting"
                }),
            ];
            while request_types.len() < responses.len() {
                let (stream, _) = listener.accept().await.unwrap();
                let mut reader = BufReader::new(stream);
                let mut line = String::new();
                reader.read_line(&mut line).await.unwrap();
                if line.is_empty() {
                    continue;
                }
                request_types.push(
                    serde_json::from_str::<serde_json::Value>(&line).unwrap()["type"]
                        .as_str()
                        .unwrap()
                        .to_string(),
                );
                reader
                    .get_mut()
                    .write_all(format!("{}\n", responses[request_types.len() - 1]).as_bytes())
                    .await
                    .unwrap();
            }
            crate::ipc::cleanup_socket();
            request_types
        });

        let response = request::<fn(String, usize, usize)>(
            &DaemonRequest::Remove {
                path: home.path().join("workspace"),
            },
            false,
            None,
        )
        .await
        .unwrap();

        assert!(response.is_none());
        assert_eq!(server.await.unwrap(), ["version", "restart"]);
    }

    #[tokio::test]
    #[serial]
    async fn non_status_request_restarts_daemon_after_invalid_version_response() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        crate::config::ensure_app_dirs().unwrap();

        let (listener, _) = crate::ipc::bind().await.unwrap();
        let server = tokio::spawn(async move {
            let mut request_types = Vec::new();
            let responses = [
                "{\"type\":\"version\",\"version\":42}\n",
                "{\"type\":\"ack\",\"message\":\"restarting\"}\n",
            ];
            while request_types.len() < responses.len() {
                let (stream, _) = listener.accept().await.unwrap();
                let mut reader = BufReader::new(stream);
                let mut line = String::new();
                reader.read_line(&mut line).await.unwrap();
                if line.is_empty() {
                    continue;
                }
                request_types.push(
                    serde_json::from_str::<serde_json::Value>(&line).unwrap()["type"]
                        .as_str()
                        .unwrap()
                        .to_string(),
                );
                reader
                    .get_mut()
                    .write_all(responses[request_types.len() - 1].as_bytes())
                    .await
                    .unwrap();
            }
            crate::ipc::cleanup_socket();
            request_types
        });

        let response = request::<fn(String, usize, usize)>(
            &DaemonRequest::Remove {
                path: home.path().join("workspace"),
            },
            false,
            None,
        )
        .await
        .unwrap();

        assert!(response.is_none());
        assert_eq!(server.await.unwrap(), ["version", "restart"]);
    }

    #[test]
    fn watcher_retry_delay_doubles_and_caps() {
        assert_eq!(watcher_retry_delay(0), WATCHER_RETRY_BASE);
        assert_eq!(watcher_retry_delay(1), WATCHER_RETRY_BASE);
        assert_eq!(watcher_retry_delay(2), WATCHER_RETRY_BASE * 2);
        assert_eq!(watcher_retry_delay(4), WATCHER_RETRY_BASE * 8);
        assert_eq!(watcher_retry_delay(6), WATCHER_RETRY_MAX);
        assert_eq!(watcher_retry_delay(40), WATCHER_RETRY_MAX);
    }

    #[tokio::test]
    #[serial]
    async fn ensure_watcher_records_failure_and_backs_off() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let repo = tempdir().unwrap();
        std::fs::write(repo.path().join("lib.rs"), "pub fn gone() {}\n").unwrap();
        let workspace = Workspace::resolve(repo.path()).unwrap();
        let metadata = WorkspaceMetadata {
            id: workspace.id.clone(),
            root: workspace.root.clone(),
            created_at_unix: 1,
            last_indexed_at_unix: Some(1),
            watch_enabled: true,
            skip_gitignore: false,
            index_generation: 0,
        };
        workspace.ensure_dirs().unwrap();
        workspace.write_metadata(&metadata).unwrap();
        // A root that vanished cannot be watched; registration must fail.
        drop(repo);

        let state = test_state();
        let first = ensure_watcher(&state, &workspace).unwrap_err().to_string();
        assert!(first.contains("1 consecutive attempt"), "{first}");
        assert!(!workspace.is_watcher_alive());
        assert!(!state.watcher_registered(&workspace.id));

        let ledger = jobs::job_status(&workspace, JobKind::Watcher, 15)
            .record
            .expect("failure recorded in the job ledger");
        assert_eq!(ledger.phase, "failed");
        assert!(ledger.last_error.is_some());
        let status = crate::workspace::list_workspaces()
            .unwrap()
            .into_iter()
            .find(|status| status.id == workspace.id)
            .expect("workspace listed");
        assert!(!status.watcher_alive);
        assert_eq!(status.watcher_error, ledger.last_error);

        // Inside the backoff window nothing is retried: the failure count
        // stays at one and the caller gets the pending retry instead.
        let second = ensure_watcher(&state, &workspace).unwrap_err().to_string();
        assert!(second.contains("next watcher retry in"), "{second}");
        assert_eq!(
            state
                .watcher_recovery
                .lock()
                .get(&workspace.id)
                .map(|entry| entry.failures),
            Some(1)
        );
        // The supervisor pass also skips it without touching the count.
        supervise_watchers(&state);
        assert_eq!(
            state
                .watcher_recovery
                .lock()
                .get(&workspace.id)
                .map(|entry| entry.failures),
            Some(1)
        );
    }

    #[tokio::test]
    #[serial]
    async fn supervisor_records_unresolvable_root_in_the_ledger() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let repo = tempdir().unwrap();
        std::fs::write(repo.path().join("lib.rs"), "pub fn gone() {}\n").unwrap();
        let workspace = Workspace::resolve(repo.path()).unwrap();
        let metadata = WorkspaceMetadata {
            id: workspace.id.clone(),
            root: workspace.root.clone(),
            created_at_unix: 1,
            last_indexed_at_unix: Some(1),
            watch_enabled: true,
            skip_gitignore: false,
            index_generation: 0,
        };
        workspace.ensure_dirs().unwrap();
        workspace.write_metadata(&metadata).unwrap();
        drop(repo);

        let state = test_state();
        supervise_watchers(&state);

        let record = jobs::job_status(&workspace, JobKind::Watcher, 15)
            .record
            .expect("missing root recorded in the job ledger");
        assert_eq!(record.phase, "failed");
        assert!(record.last_error.is_some());
        assert!(!workspace.is_watcher_alive());
        assert!(state.watcher_backoff_error(&workspace.id).is_some());
        let status = crate::workspace::list_workspaces()
            .unwrap()
            .into_iter()
            .find(|status| status.id == workspace.id)
            .expect("workspace listed");
        assert_eq!(status.watcher_error, record.last_error);
    }

    #[tokio::test]
    #[serial]
    async fn watcher_heartbeat_recreates_ledger_record_after_index_wipe() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let repo = tempdir().unwrap();
        std::fs::write(repo.path().join("lib.rs"), "pub fn watched() {}\n").unwrap();
        let workspace = Workspace::resolve(repo.path()).unwrap();
        workspace.ensure_dirs().unwrap();

        let control = Arc::new(WatchControl::new(workspace.clone()));
        let nonce = jobs::start_job(&workspace, JobKind::Watcher, "idle", 1)
            .unwrap()
            .nonce;
        control.set_job_nonce(nonce.clone());
        complete_initial_watch_reconciliation(&control);
        assert!(workspace.is_watcher_alive());

        // An index rebuild removes both liveness artifacts while the watcher
        // keeps running. Its next heartbeat must recreate the job record.
        std::fs::remove_file(workspace.job_ledger_path()).unwrap();
        std::fs::remove_file(workspace.watcher_pid_path()).unwrap();
        assert!(!workspace.is_watcher_alive());

        update_watcher_job(
            &control,
            JobUpdate {
                phase: Some("idle".to_string()),
                active: Some(true),
                ..Default::default()
            },
        );
        assert!(
            workspace.is_watcher_alive(),
            "heartbeat must re-create the record"
        );
        assert_ne!(
            control.job_nonce(),
            nonce,
            "the record carries a fresh nonce"
        );

        // A stopped watcher never resurrects its record.
        control.active.store(false, Ordering::Relaxed);
        std::fs::remove_file(workspace.job_ledger_path()).unwrap();
        update_watcher_job(
            &control,
            JobUpdate {
                phase: Some("idle".to_string()),
                active: Some(true),
                ..Default::default()
            },
        );
        assert!(!workspace.is_watcher_alive());
    }

    #[test]
    #[serial]
    fn recreated_startup_error_stays_non_live_when_heartbeat_fails() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let repo = tempdir().unwrap();
        let workspace = Workspace::resolve(repo.path()).unwrap();
        workspace.ensure_dirs().unwrap();
        workspace
            .write_metadata(&WorkspaceMetadata {
                id: workspace.id.clone(),
                root: workspace.root.clone(),
                created_at_unix: 1,
                last_indexed_at_unix: Some(1),
                watch_enabled: true,
                skip_gitignore: false,
                index_generation: 1,
            })
            .unwrap();
        let control = WatchControl::new(workspace.clone());
        // The scan completed, but startup catch-up is still pending.
        control
            .initial_scan_required
            .store(false, Ordering::Relaxed);
        let failed = JobUpdate {
            phase: Some("error".to_string()),
            last_error: Some(Some("startup reconciliation failed".to_string())),
            ..Default::default()
        };

        // There is no nonce to refresh. The fault hits the follow-up heartbeat
        // after start_job has persisted its initially active replacement record.
        jobs::fail_next_watcher_heartbeat();
        update_watcher_job(&control, failed.clone());
        let recreated = jobs::job_status(&workspace, JobKind::Watcher, 15)
            .record
            .expect("the replacement record must have been persisted");
        assert!(
            recreated.active,
            "the injected heartbeat must leave the initial record untouched"
        );
        assert!(
            !workspace.is_watcher_alive(),
            "an error during startup must not publish watcher liveness when the inactive heartbeat fails"
        );
        let listed = crate::workspace::list_workspaces()
            .unwrap()
            .into_iter()
            .find(|status| status.id == workspace.id)
            .unwrap();
        assert!(
            !listed.watcher_alive,
            "workspace status must not publish startup liveness"
        );

        // A later successful heartbeat retains the error without certifying the
        // index. Only completing reconciliation may make the watcher live.
        update_watcher_job(&control, failed);
        let recovered = jobs::job_status(&workspace, JobKind::Watcher, 15)
            .record
            .unwrap();
        assert_eq!(
            recovered.last_error.as_deref(),
            Some("startup reconciliation failed")
        );
        assert!(!recovered.active);
        assert!(!workspace.is_watcher_alive());
        complete_initial_watch_reconciliation(&control);
        assert!(workspace.is_watcher_alive());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[serial]
    async fn enabling_watch_policy_reconciles_before_cached_search() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let repo = tempdir().unwrap();
        let source = repo.path().join("policy.rs");
        std::fs::write(&source, "pub fn policy_previous_marker() {}\n").unwrap();
        let workspace = Workspace::resolve(repo.path()).unwrap();
        index_workspace(&workspace, create_hash_model().as_ref()).unwrap();
        let state = test_state();
        let request = DaemonRequest::Search {
            path: Some(workspace.root.clone()),
            query: "policy".to_string(),
            limit: Some(5),
            context: 0,
            type_filter: None,
            include_globs: Vec::new(),
            exclude_globs: Vec::new(),
            scope_path: None,
            scope_is_file: false,
            skip_gitignore: false,
            force_neural: false,
            disable_memory_expansion: true,
        };
        let before = handle_request(state.clone(), request.clone()).await;
        assert!(matches!(before, DaemonResponse::SearchResults { hits, .. }
            if hits.iter().any(|hit| hit.preview.contains("policy_previous_marker"))));
        assert!(!state.watcher_registered(&workspace.id));

        // Change policy without clearing any daemon cache. The repeated query
        // must register and reconcile before it can reuse its old cached hits.
        let held = state.acquire_workspace_mutations(std::slice::from_ref(&workspace));
        std::fs::write(&source, "pub fn policy_current_marker() {}\n").unwrap();
        let mut metadata = workspace.read_metadata().unwrap().unwrap();
        metadata.watch_enabled = true;
        workspace.write_metadata(&metadata).unwrap();
        let mut searching = tokio::spawn(handle_request(state.clone(), request));
        tokio::time::timeout(Duration::from_secs(5), async {
            while !state.watcher_registered(&workspace.id) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("enabling watch policy did not register the watcher");
        assert!(!workspace.is_watcher_alive());
        assert!(
            tokio::time::timeout(Duration::from_millis(50), &mut searching)
                .await
                .is_err()
        );
        drop(held);
        let after = tokio::time::timeout(Duration::from_secs(10), searching)
            .await
            .unwrap()
            .unwrap();
        let alive = workspace.is_watcher_alive();
        stop_all_watchers(&state);
        assert!(
            matches!(after, DaemonResponse::SearchResults { hits, warnings }
            if warnings.is_empty()
                && hits.iter().any(|hit| hit.preview.contains("policy_current_marker"))
                && hits.iter().all(|hit| !hit.preview.contains("policy_previous_marker")))
        );
        assert!(alive);
        assert!(indexed_file_contains(
            &workspace,
            "policy.rs",
            "policy_current_marker"
        ));
        assert!(!indexed_file_contains(
            &workspace,
            "policy.rs",
            "policy_previous_marker"
        ));
    }

    async fn acquire_search_lease_without_watcher(state: &DaemonState, workspace: &Workspace) {
        let leases = tokio::time::timeout(
            Duration::from_secs(5),
            state.acquire_search_leases(std::slice::from_ref(workspace), false, None),
        )
        .await
        .unwrap()
        .unwrap()
        .unwrap();
        drop(leases);
        assert!(!state.watcher_registered(&workspace.id));
        assert!(!workspace.is_watcher_alive());
    }

    #[tokio::test]
    #[serial]
    async fn completed_first_index_rechecks_cached_watch_policy() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let repo = tempdir().unwrap();
        std::fs::write(
            repo.path().join("policy.rs"),
            "pub fn first_policy_index() {}\n",
        )
        .unwrap();
        let workspace = Workspace::resolve(repo.path()).unwrap();
        workspace.ensure_dirs().unwrap();
        workspace
            .write_metadata(&WorkspaceMetadata {
                id: workspace.id.clone(),
                root: workspace.root.clone(),
                created_at_unix: 1,
                last_indexed_at_unix: None,
                watch_enabled: true,
                skip_gitignore: false,
                index_generation: 0,
            })
            .unwrap();
        let state = test_state();
        acquire_search_lease_without_watcher(&state, &workspace).await;

        index_workspace_for_watcher(&workspace, create_hash_model().as_ref()).unwrap();
        let response = tokio::time::timeout(
            Duration::from_secs(10),
            handle_request(
                state.clone(),
                literal_request_for(&workspace, "first_policy_index"),
            ),
        )
        .await
        .unwrap();
        let alive = workspace.is_watcher_alive();
        stop_all_watchers(&state);
        assert!(
            matches!(response, DaemonResponse::SearchResults { hits, warnings }
            if hits.len() == 1 && warnings.is_empty())
        );
        assert!(
            alive,
            "completing the first index must enable startup reconciliation"
        );
    }

    #[tokio::test]
    #[serial]
    async fn repaired_metadata_rechecks_cached_watch_policy() {
        for corrupt in [false, true] {
            let home = tempdir().unwrap();
            unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
            let repo = tempdir().unwrap();
            std::fs::write(
                repo.path().join("policy.rs"),
                "pub fn repaired_policy_marker() {}\n",
            )
            .unwrap();
            let workspace = Workspace::resolve(repo.path()).unwrap();
            index_workspace(&workspace, create_hash_model().as_ref()).unwrap();
            let mut metadata = workspace.read_metadata().unwrap().unwrap();
            let state = test_state();
            acquire_search_lease_without_watcher(&state, &workspace).await;

            if corrupt {
                std::fs::write(workspace.metadata_path(), "invalid metadata").unwrap();
            } else {
                std::fs::remove_file(workspace.metadata_path()).unwrap();
            }
            acquire_search_lease_without_watcher(&state, &workspace).await;
            metadata.watch_enabled = true;
            workspace.write_metadata(&metadata).unwrap();
            let response = tokio::time::timeout(
                Duration::from_secs(10),
                handle_request(
                    state.clone(),
                    literal_request_for(&workspace, "repaired_policy_marker"),
                ),
            )
            .await
            .unwrap();
            let alive = workspace.is_watcher_alive();
            stop_all_watchers(&state);
            assert!(
                matches!(response, DaemonResponse::SearchResults { hits, warnings }
                if hits.len() == 1 && warnings.is_empty())
            );
            assert!(
                alive,
                "repaired watch policy must not inherit a missing/corrupt result"
            );
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[serial]
    async fn restored_watcher_reconciles_offline_changes_before_search() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let repo = tempdir().unwrap();
        for (name, marker) in [
            ("changed.rs", "offline_original_marker"),
            ("deleted.rs", "offline_deleted_marker"),
            ("stable.rs", "offline_stable_marker"),
        ] {
            std::fs::write(repo.path().join(name), format!("pub fn {marker}() {{}}\n")).unwrap();
        }
        let workspace = Workspace::resolve(repo.path()).unwrap();
        index_workspace(&workspace, create_hash_model().as_ref()).unwrap();
        let before =
            crate::merkle::MerkleSnapshot::load(&workspace.merkle_snapshot_path()).unwrap();
        let mut metadata = workspace.read_metadata().unwrap().unwrap();
        metadata.watch_enabled = true;
        workspace.write_metadata(&metadata).unwrap();

        // Keep the same daemon state so the first post-restore request also
        // exercises a preexisting query-result cache, not just a cold restart.
        let state = test_state();
        let cached_request = DaemonRequest::Search {
            path: Some(workspace.root.clone()),
            query: "offline_added_marker".to_string(),
            limit: Some(5),
            context: 0,
            type_filter: None,
            include_globs: Vec::new(),
            exclude_globs: Vec::new(),
            scope_path: None,
            scope_is_file: false,
            skip_gitignore: false,
            force_neural: false,
            disable_memory_expansion: true,
        };
        let before_search = handle_request(state.clone(), cached_request.clone()).await;
        assert!(
            matches!(before_search, DaemonResponse::SearchResults { hits, .. } if hits.iter().all(|hit| hit.file_path != Path::new("added.rs")))
        );
        assert!(!state.query_results.lock().results.is_empty());
        stop_all_watchers(&state);

        std::fs::write(
            repo.path().join("changed.rs"),
            "pub fn offline_updated_marker() {}\n",
        )
        .unwrap();
        std::fs::write(
            repo.path().join("added.rs"),
            "pub fn offline_added_marker() {}\n",
        )
        .unwrap();
        std::fs::remove_file(repo.path().join("deleted.rs")).unwrap();

        let held = state.acquire_workspace_mutations(std::slice::from_ref(&workspace));
        restore_configured_watchers(&state);
        assert!(state.watcher_registered(&workspace.id));
        assert!(
            !workspace.is_watcher_alive(),
            "registration is not proof that offline changes were reconciled"
        );
        let control = state
            .watchers
            .lock()
            .get(&workspace.id)
            .unwrap()
            .control
            .clone();

        // Events observed after registration must survive startup bookkeeping.
        std::fs::write(
            repo.path().join("queued.rs"),
            "pub fn queued_startup_marker() {}\n",
        )
        .unwrap();
        control.mark_paths_dirty([PathBuf::from("queued.rs")]);
        let mut searching = tokio::spawn(handle_request(state.clone(), cached_request));
        assert!(
            tokio::time::timeout(Duration::from_millis(50), &mut searching)
                .await
                .is_err()
        );
        drop(held);
        let response = tokio::time::timeout(Duration::from_secs(10), searching)
            .await
            .unwrap()
            .unwrap();
        assert!(
            matches!(response, DaemonResponse::SearchResults { hits, warnings } if hits.iter().any(|hit| hit.file_path == Path::new("added.rs")) && warnings.is_empty())
        );
        for marker in [
            "offline_updated_marker",
            "offline_added_marker",
            "offline_stable_marker",
            "queued_startup_marker",
        ] {
            let response =
                handle_request(state.clone(), literal_request_for(&workspace, marker)).await;
            assert!(
                matches!(response, DaemonResponse::SearchResults { hits, .. } if hits.len() == 1),
                "missing {marker}"
            );
        }
        assert!(!indexed_file_contains(
            &workspace,
            "changed.rs",
            "offline_original_marker"
        ));
        assert!(!indexed_file_contains(
            &workspace,
            "deleted.rs",
            "offline_deleted_marker"
        ));
        let after = crate::merkle::MerkleSnapshot::load(&workspace.merkle_snapshot_path()).unwrap();
        assert_eq!(before.files["stable.rs"], after.files["stable.rs"]);
        assert!(!after.files.contains_key("deleted.rs"));
        assert!(after.files.contains_key("added.rs"));
        assert!(after.files.contains_key("queued.rs"));
        assert!(workspace.is_watcher_alive());
        stop_all_watchers(&state);

        // A second restart without source edits is a real no-op, not a fresh
        // rebuild or an unconditional recomputation of the unchanged files.
        let generation = workspace.read_metadata().unwrap().unwrap().index_generation;
        let snapshot = std::fs::read(workspace.merkle_snapshot_path()).unwrap();
        let state = test_state();
        restore_configured_watchers(&state);
        let response = tokio::time::timeout(
            Duration::from_secs(10),
            handle_request(
                state.clone(),
                literal_request_for(&workspace, "offline_stable_marker"),
            ),
        )
        .await
        .unwrap();
        assert!(matches!(response, DaemonResponse::SearchResults { hits, .. } if hits.len() == 1));
        assert_eq!(
            workspace.read_metadata().unwrap().unwrap().index_generation,
            generation
        );
        assert_eq!(
            std::fs::read(workspace.merkle_snapshot_path()).unwrap(),
            snapshot
        );
        stop_all_watchers(&state);
    }

    #[tokio::test]
    #[serial]
    async fn explicit_index_satisfies_initial_watcher_reconciliation() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let repo = tempdir().unwrap();
        std::fs::write(
            repo.path().join("lib.rs"),
            "pub fn initial_index_marker() {}\n",
        )
        .unwrap();
        let workspace = Workspace::resolve(repo.path()).unwrap();
        let state = test_state();
        let response = handle_request(
            state.clone(),
            DaemonRequest::Index {
                path: workspace.root.clone(),
                watch: true,
                skip_gitignore: false,
            },
        )
        .await;
        assert!(
            matches!(response, DaemonResponse::Ack { .. }),
            "{response:?}"
        );
        let control = state
            .watchers
            .lock()
            .get(&workspace.id)
            .unwrap()
            .control
            .clone();
        assert!(!control.initial_scan_required.load(Ordering::Relaxed));
        // Wake the startup worker even if it had not yet observed its notify.
        control.notify.notify_one();
        let response = tokio::time::timeout(
            Duration::from_secs(10),
            handle_request(
                state.clone(),
                literal_request_for(&workspace, "initial_index_marker"),
            ),
        )
        .await
        .unwrap();
        assert!(matches!(response, DaemonResponse::SearchResults { hits, .. } if hits.len() == 1));
        assert!(workspace.is_watcher_alive());
        assert_eq!(
            jobs::read_job_ledger(&workspace)
                .jobs
                .into_iter()
                .find(|job| job.kind == JobKind::Indexing)
                .unwrap()
                .generation,
            1
        );
        stop_all_watchers(&state);
    }

    fn queued_startup_watch_change() -> (tempfile::TempDir, tempfile::TempDir, Arc<WatchControl>) {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let repo = tempdir().unwrap();
        let source = repo.path().join("edited.rs");
        std::fs::write(&source, "pub fn before_startup_marker() {}\n").unwrap();
        std::fs::write(
            repo.path().join("stable.rs"),
            "pub fn stable_startup_marker() {}\n",
        )
        .unwrap();
        let workspace = Workspace::resolve(repo.path()).unwrap();
        index_workspace_for_watcher(&workspace, create_hash_model().as_ref()).unwrap();
        let control = Arc::new(WatchControl::new(workspace.clone()));
        let filter = Mutex::new(WatchEventFilter::new(&workspace));

        // The scan has already read this file. Deliver the later edit through
        // the real notify callback path before that scan publishes readiness.
        std::fs::write(&source, "pub fn after_startup_marker() {}\n").unwrap();
        handle_watch_result(
            &control,
            &filter,
            Ok(
                notify::Event::new(notify::EventKind::Modify(notify::event::ModifyKind::Data(
                    notify::event::DataChange::Content,
                )))
                .add_path(source),
            ),
        );
        (home, repo, control)
    }

    #[tokio::test]
    #[serial]
    async fn initial_reconciliation_waits_for_queued_startup_change() {
        let (_home, _repo, control) = queued_startup_watch_change();
        let workspace = &control.workspace;
        let before =
            crate::merkle::MerkleSnapshot::load(&workspace.merkle_snapshot_path()).unwrap();
        let mut readiness = control.readiness.subscribe();
        complete_initial_watch_reconciliation(&control);
        assert!(
            matches!(*readiness.borrow(), WatchReadiness::Reconciling),
            "a successful scan must not publish Ready before its queued edit is indexed"
        );
        assert!(!workspace.is_watcher_alive());
        assert!(!control.initial_scan_required.load(Ordering::Relaxed));
        assert!(indexed_file_contains(
            workspace,
            "edited.rs",
            "before_startup_marker"
        ));

        spawn_watch_worker(test_state(), control.clone());
        let outcome = tokio::time::timeout(
            Duration::from_secs(10),
            readiness.wait_for(|value| !matches!(value, WatchReadiness::Reconciling)),
        )
        .await
        .expect("queued startup change did not finish")
        .unwrap()
        .clone();
        let alive_when_ready = workspace.is_watcher_alive();
        control.active.store(false, Ordering::Relaxed);
        control.notify.notify_one();
        assert!(matches!(outcome, WatchReadiness::Ready));
        assert!(!control.indexing.load(Ordering::Relaxed));
        assert!(indexed_file_contains(
            workspace,
            "edited.rs",
            "after_startup_marker"
        ));
        assert!(!indexed_file_contains(
            workspace,
            "edited.rs",
            "before_startup_marker"
        ));
        let after = crate::merkle::MerkleSnapshot::load(&workspace.merkle_snapshot_path()).unwrap();
        assert_eq!(before.files["stable.rs"], after.files["stable.rs"]);
        assert!(alive_when_ready);
    }

    #[test]
    #[serial]
    fn initial_reconciliation_waits_for_claimed_startup_change() {
        let (_home, _repo, control) = queued_startup_watch_change();
        let workspace = &control.workspace;
        let pending = control.take_pending_work().unwrap();
        assert!(control.take_pending_work().is_none());

        // An explicit Index may finish while the worker has removed the event
        // from the queue but is still waiting for that Index's workspace lease.
        complete_initial_watch_reconciliation(&control);
        assert!(
            matches!(*control.readiness.borrow(), WatchReadiness::Reconciling),
            "an empty queue does not certify work already claimed by the worker"
        );
        assert!(!workspace.is_watcher_alive());
        let WatchChange::Paths(paths) = pending.change else {
            panic!("expected a targeted startup change");
        };
        let summary = index_workspace_paths_for_watcher(
            workspace,
            create_hash_model().as_ref(),
            &paths.into_iter().collect::<Vec<_>>(),
        )
        .unwrap();
        assert_eq!(summary.indexed_files, 1);
        complete_initial_watch_reconciliation(&control);
        assert!(matches!(
            *control.readiness.borrow(),
            WatchReadiness::Reconciling
        ));

        control.indexing.store(false, Ordering::Relaxed);
        complete_initial_watch_reconciliation(&control);
        assert!(matches!(*control.readiness.borrow(), WatchReadiness::Ready));
        assert!(indexed_file_contains(
            workspace,
            "edited.rs",
            "after_startup_marker"
        ));
        assert!(!indexed_file_contains(
            workspace,
            "edited.rs",
            "before_startup_marker"
        ));
        assert!(workspace.is_watcher_alive());
    }

    #[tokio::test]
    #[serial]
    async fn initial_reconciliation_reports_startup_catchup_failure_and_recovers() {
        let (_home, _repo, control) = queued_startup_watch_change();
        let workspace = &control.workspace;
        // The full scan succeeded, but its queued delta has not. A later error
        // must still release readiness waiters, rather than leaving them hung.
        control
            .initial_scan_required
            .store(false, Ordering::Relaxed);
        let metadata = std::fs::read(workspace.metadata_path()).unwrap();
        std::fs::write(workspace.metadata_path(), "invalid metadata").unwrap();
        let mut readiness = control.readiness.subscribe();
        spawn_watch_worker(test_state(), control.clone());
        let failed = tokio::time::timeout(
            Duration::from_secs(5),
            readiness.wait_for(|value| !matches!(value, WatchReadiness::Reconciling)),
        )
        .await
        .ok()
        .and_then(Result::ok)
        .map(|value| value.clone());
        let alive_after_failure = workspace.is_watcher_alive();
        std::fs::write(workspace.metadata_path(), metadata).unwrap();
        let recovered = if matches!(failed, Some(WatchReadiness::Failed(_))) {
            tokio::time::timeout(
                Duration::from_secs(10),
                readiness.wait_for(|value| matches!(value, WatchReadiness::Ready)),
            )
            .await
            .ok()
            .and_then(Result::ok)
            .is_some()
        } else {
            false
        };
        control.active.store(false, Ordering::Relaxed);
        control.notify.notify_one();
        control.shutdown.notify_one();
        tokio::time::timeout(Duration::from_secs(5), async {
            while control.indexing.load(Ordering::Relaxed) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("startup worker failed to stop");

        assert!(matches!(failed, Some(WatchReadiness::Failed(message)) if !message.is_empty()));
        assert!(!alive_after_failure);
        assert!(
            recovered,
            "startup catch-up did not recover after the failure cleared"
        );
        assert!(indexed_file_contains(
            workspace,
            "edited.rs",
            "after_startup_marker"
        ));
        assert!(!indexed_file_contains(
            workspace,
            "edited.rs",
            "before_startup_marker"
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
                force_neural: false,
                disable_memory_expansion: true,
            },
        )
        .await;

        match response {
            DaemonResponse::SearchResults { hits, .. } => {
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
            force_neural: false,
            disable_memory_expansion: true,
        };

        let first = handle_request(state.clone(), request.clone()).await;
        let first_count = match first {
            DaemonResponse::SearchResults { hits, .. } => hits.len(),
            other => panic!("expected SearchResults, got {other:?}"),
        };
        assert!(first_count > 0);
        assert_eq!(state.query_results.lock().results.len(), 1);

        let indexed = handle_request(
            state.clone(),
            DaemonRequest::Index {
                path: workspace.root.clone(),
                watch: false,
                skip_gitignore: false,
            },
        )
        .await;
        assert!(matches!(indexed, DaemonResponse::Ack { .. }));
        assert_eq!(
            state.query_results.lock().results.len(),
            1,
            "no-op indexing should preserve valid query results"
        );

        state.search_contexts.lock().clear();
        let mut equivalent_request = request;
        let DaemonRequest::Search { query, .. } = &mut equivalent_request else {
            unreachable!("test request is a search")
        };
        query.push_str("  ");
        let second = handle_request(state.clone(), equivalent_request).await;
        let second_count = match second {
            DaemonResponse::SearchResults { hits, .. } => hits.len(),
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
    async fn daemon_search_repairs_broken_selected_workspace_index() {
        let home = tempdir().unwrap();
        unsafe {
            std::env::set_var("IVYGREP_HOME", home.path());
            std::env::set_var("IVYGREP_NO_AUTOSPAWN", "1");
        }

        let repo = tempdir().unwrap();
        std::fs::write(
            repo.path().join("recover.rs"),
            "pub fn recoverable_marker() -> usize { 42 }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(repo.path()).unwrap();
        write_broken_completed_index_metadata(&workspace, false);
        assert_eq!(
            workspace.index_health().state,
            WorkspaceIndexState::Unhealthy
        );

        let response = handle_request(
            test_state(),
            DaemonRequest::Search {
                path: Some(workspace.root.clone()),
                query: "recoverable_marker".to_string(),
                limit: Some(10),
                context: 1,
                type_filter: None,
                include_globs: Vec::new(),
                exclude_globs: Vec::new(),
                scope_path: None,
                scope_is_file: false,
                skip_gitignore: false,
                force_neural: false,
                disable_memory_expansion: true,
            },
        )
        .await;

        match response {
            DaemonResponse::SearchResults { hits, .. } => {
                assert!(
                    hits.iter()
                        .any(|hit| hit.file_path.to_string_lossy().ends_with("recover.rs")),
                    "repaired search should return recover.rs, got {hits:?}"
                );
            }
            other => panic!("expected SearchResults, got {other:?}"),
        }
        assert!(workspace.index_health().is_queryable());
    }

    #[tokio::test]
    #[serial]
    async fn daemon_all_indices_search_repairs_broken_workspace_index() {
        let home = tempdir().unwrap();
        unsafe {
            std::env::set_var("IVYGREP_HOME", home.path());
            std::env::set_var("IVYGREP_NO_AUTOSPAWN", "1");
        }

        let broken_repo = tempdir().unwrap();
        std::fs::write(
            broken_repo.path().join("recover.rs"),
            "pub fn all_indices_repair_marker() -> usize { 42 }\n",
        )
        .unwrap();
        let broken_workspace = Workspace::resolve(broken_repo.path()).unwrap();
        write_broken_completed_index_metadata(&broken_workspace, false);

        let healthy_repo = tempdir().unwrap();
        std::fs::write(
            healthy_repo.path().join("other.rs"),
            "pub fn unrelated_marker() -> usize { 7 }\n",
        )
        .unwrap();
        let healthy_workspace = Workspace::resolve(healthy_repo.path()).unwrap();
        let model = create_hash_model();
        index_workspace(&healthy_workspace, model.as_ref()).unwrap();

        let response = handle_request(
            test_state(),
            DaemonRequest::Search {
                path: None,
                query: "all_indices_repair_marker".to_string(),
                limit: Some(10),
                context: 1,
                type_filter: None,
                include_globs: Vec::new(),
                exclude_globs: Vec::new(),
                scope_path: None,
                scope_is_file: false,
                skip_gitignore: false,
                force_neural: false,
                disable_memory_expansion: true,
            },
        )
        .await;

        match response {
            DaemonResponse::SearchResults { hits, .. } => {
                assert!(
                    hits.iter().any(|hit| {
                        hit.file_path.is_absolute()
                            && hit.file_path.to_string_lossy().ends_with("recover.rs")
                    }),
                    "all-index search should repair and return absolute recover.rs, got {hits:?}"
                );
            }
            other => panic!("expected SearchResults, got {other:?}"),
        }
        assert!(broken_workspace.index_health().is_queryable());
    }

    #[tokio::test]
    #[serial]
    async fn daemon_all_indices_clears_scope_for_every_search_mode() {
        let home = tempdir().unwrap();
        unsafe {
            std::env::set_var("IVYGREP_HOME", home.path());
            std::env::set_var("IVYGREP_NO_AUTOSPAWN", "1");
        }
        let first = tempdir().unwrap();
        let second = tempdir().unwrap();
        std::fs::create_dir_all(first.path().join("src")).unwrap();
        std::fs::create_dir_all(second.path().join("other")).unwrap();
        std::fs::write(
            first.path().join("src/first.rs"),
            "before\npub fn daemon_cross_workspace_marker() {}\nafter\n",
        )
        .unwrap();
        std::fs::write(
            second.path().join("other/second.rs"),
            "before\npub fn daemon_cross_workspace_marker() {}\nafter\n",
        )
        .unwrap();
        std::fs::write(
            second.path().join("other/decoy.md"),
            "before\ndaemon_cross_workspace_marker\nafter\n",
        )
        .unwrap();

        let first_workspace = Workspace::resolve(first.path()).unwrap();
        let second_workspace = Workspace::resolve(second.path()).unwrap();
        let model = create_hash_model();
        index_workspace(&first_workspace, model.as_ref()).unwrap();
        index_workspace(&second_workspace, model.as_ref()).unwrap();

        let common_scope = Some(PathBuf::from("src"));
        let include = vec!["**/*.rs".to_string()];
        let requests = [
            DaemonRequest::LiteralSearch {
                path: None,
                query: "daemon_cross_workspace_marker".to_string(),
                limit: Some(10),
                context: 1,
                type_filter: Some("rust".to_string()),
                include_globs: include.clone(),
                exclude_globs: Vec::new(),
                scope_path: common_scope.clone(),
                scope_is_file: false,
                skip_gitignore: false,
            },
            DaemonRequest::RegexSearch {
                path: None,
                pattern: "daemon_cross_workspace_marker".to_string(),
                limit: Some(10),
                context: 1,
                type_filter: Some("rust".to_string()),
                include_globs: include.clone(),
                exclude_globs: Vec::new(),
                scope_path: common_scope.clone(),
                scope_is_file: false,
                skip_gitignore: false,
            },
            DaemonRequest::Search {
                path: None,
                query: "daemon_cross_workspace_marker".to_string(),
                limit: Some(10),
                context: 1,
                type_filter: Some("rust".to_string()),
                include_globs: include,
                exclude_globs: Vec::new(),
                scope_path: common_scope,
                scope_is_file: false,
                skip_gitignore: false,
                force_neural: false,
                disable_memory_expansion: true,
            },
        ];

        for request in requests {
            let response = handle_request(test_state(), request).await;
            let DaemonResponse::SearchResults { hits, .. } = response else {
                panic!("expected search results, got {response:?}");
            };
            let paths = hits
                .iter()
                .map(|hit| hit.file_path.clone())
                .collect::<std::collections::HashSet<_>>();
            let first_path = first.path().join("src/first.rs").canonicalize().unwrap();
            let second_path = second
                .path()
                .join("other/second.rs")
                .canonicalize()
                .unwrap();
            let decoy_path = second.path().join("other/decoy.md").canonicalize().unwrap();
            assert!(paths.contains(&first_path), "{paths:?}");
            assert!(paths.contains(&second_path), "{paths:?}");
            assert!(!paths.contains(&decoy_path));
        }
    }

    #[test]
    fn daemon_query_cache_can_be_disabled() {
        let mut state = test_state();
        state.query_result_cache_enabled = false;
        let key = QueryCacheKey {
            workspace_ids: Vec::new(),
            signatures: Vec::new(),
            all_indices: false,
            query: "needle".to_string(),
            limit: Some(10),
            context: 2,
            type_filter: None,
            include_globs: Vec::new(),
            exclude_globs: Vec::new(),
            scope_filter: None,
            skip_gitignore: false,
            emb_dim: 256,
            wants_neural: true,
            force_neural: true,
            reranker: crate::reranker::cache_identity(),
        };
        state.store_query_results(key.clone(), &[]);
        assert!(state.cached_query_results(&key).is_none());
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
            force_neural: false,
            disable_memory_expansion: true,
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
            force_neural: false,
            disable_memory_expansion: true,
        };

        let normal = handle_request(state.clone(), normal_request).await;
        match normal {
            DaemonResponse::SearchResults { hits, .. } => {
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
            DaemonResponse::SearchResults { hits, .. } => {
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
    #[serial]
    fn watch_event_filter_ignores_nested_ivygrep_storage() {
        let repo = tempdir().unwrap();
        let home = repo.path().join("local-state");
        std::fs::create_dir_all(&home).unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", &home) };

        let workspace = Workspace::resolve(repo.path()).unwrap();
        workspace.ensure_dirs().unwrap();
        let filter = WatchEventFilter::new(&workspace);

        assert!(filter.path_should_reindex(&workspace.root.join("src/lib.rs")));
        assert!(!filter.path_should_reindex(&workspace.index_dir.join("job.json")));
        assert!(!filter.path_should_reindex(&home.join("daemon.log")));
    }

    #[tokio::test]
    #[serial]
    async fn query_preparation_reconciles_healthy_index_filter_transitions() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let repo = tempdir().unwrap();
        std::fs::write(repo.path().join(".gitignore"), "ignored.rs\n").unwrap();
        std::fs::write(
            repo.path().join("visible.rs"),
            "pub fn visible_transition_marker() {}\n",
        )
        .unwrap();
        std::fs::write(
            repo.path().join("ignored.rs"),
            "pub fn ignored_transition_marker() {}\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(repo.path()).unwrap();
        let model = create_hash_model();
        index_workspace(&workspace, model.as_ref()).unwrap();
        let state = test_state();
        register_watcher(&state, repo.path()).unwrap();
        drop(
            state
                .acquire_search_leases(std::slice::from_ref(&workspace), false, None)
                .await
                .unwrap()
                .unwrap(),
        );
        assert!(!indexed_file_contains(
            &workspace,
            "ignored.rs",
            "ignored_transition_marker"
        ));

        assert!(
            state
                .prepare_workspace_for_hybrid_query(&workspace, true)
                .unwrap()
        );
        assert!(indexed_file_contains(
            &workspace,
            "ignored.rs",
            "ignored_transition_marker"
        ));
        assert!(
            state
                .watchers
                .lock()
                .get(&workspace.id)
                .unwrap()
                .event_filter
                .lock()
                .skip_gitignore
        );

        assert!(
            state
                .prepare_workspace_for_hybrid_query(&workspace, false)
                .unwrap()
        );
        assert!(!indexed_file_contains(
            &workspace,
            "ignored.rs",
            "ignored_transition_marker"
        ));
        assert!(
            !state
                .watchers
                .lock()
                .get(&workspace.id)
                .unwrap()
                .event_filter
                .lock()
                .skip_gitignore
        );

        let mut metadata = workspace.read_metadata().unwrap().unwrap();
        metadata.skip_gitignore = true;
        workspace.write_metadata(&metadata).unwrap();
        register_watcher(&state, repo.path()).unwrap();
        assert!(
            state
                .watchers
                .lock()
                .get(&workspace.id)
                .unwrap()
                .event_filter
                .lock()
                .skip_gitignore
        );
        assert!(
            !state
                .prepare_workspace_for_hybrid_query(&workspace, false)
                .unwrap()
        );
        assert!(!workspace.read_metadata().unwrap().unwrap().skip_gitignore);
        assert!(
            !state
                .watchers
                .lock()
                .get(&workspace.id)
                .unwrap()
                .event_filter
                .lock()
                .skip_gitignore
        );

        stop_all_watchers(&state);
    }

    #[tokio::test]
    #[serial]
    async fn index_request_reconciles_filter_and_refreshes_live_watcher() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let repo = tempdir().unwrap();
        std::fs::write(repo.path().join(".gitignore"), "ignored.rs\n").unwrap();
        std::fs::write(
            repo.path().join("visible.rs"),
            "pub fn visible_watcher_transition() {}\n",
        )
        .unwrap();
        std::fs::write(
            repo.path().join("ignored.rs"),
            "pub fn ignored_watcher_transition() {}\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(repo.path()).unwrap();
        let model = create_hash_model();
        index_workspace(&workspace, model.as_ref()).unwrap();
        let state = test_state();
        register_watcher(&state, repo.path()).unwrap();

        for skip_gitignore in [true, false] {
            let response = handle_request(
                state.clone(),
                DaemonRequest::Index {
                    path: repo.path().to_path_buf(),
                    watch: true,
                    skip_gitignore,
                },
            )
            .await;
            assert!(matches!(response, DaemonResponse::Ack { .. }));

            let registration_filter = state
                .watchers
                .lock()
                .get(&workspace.id)
                .unwrap()
                .event_filter
                .clone();
            assert_eq!(registration_filter.lock().skip_gitignore, skip_gitignore);
            assert_eq!(
                indexed_file_contains(&workspace, "ignored.rs", "ignored_watcher_transition"),
                skip_gitignore
            );
        }

        stop_all_watchers(&state);
    }

    #[tokio::test]
    #[serial]
    async fn opposite_filter_requests_wait_for_active_mode_through_search_and_index() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let repo = tempdir().unwrap();
        std::fs::write(repo.path().join(".gitignore"), "ignored.rs\n").unwrap();
        std::fs::write(
            repo.path().join("visible.rs"),
            "pub fn visible_concurrency_marker() {}\n",
        )
        .unwrap();
        std::fs::write(
            repo.path().join("ignored.rs"),
            "pub fn ignored_concurrency_marker() {}\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(repo.path()).unwrap();
        let model = create_hash_model();
        index_workspace(&workspace, model.as_ref()).unwrap();
        let mut state = test_state();
        state.cpu_permits = Arc::new(tokio::sync::Semaphore::new(4));

        let literal_request = |skip_gitignore| DaemonRequest::LiteralSearch {
            path: Some(workspace.root.clone()),
            query: "ignored_concurrency_marker".to_string(),
            limit: Some(5),
            context: 0,
            type_filter: None,
            include_globs: Vec::new(),
            exclude_globs: Vec::new(),
            scope_path: None,
            scope_is_file: false,
            skip_gitignore,
        };
        let hybrid_request = |skip_gitignore| DaemonRequest::Search {
            path: Some(workspace.root.clone()),
            query: "ignored_concurrency_marker".to_string(),
            limit: Some(5),
            context: 0,
            type_filter: None,
            include_globs: Vec::new(),
            exclude_globs: Vec::new(),
            scope_path: None,
            scope_is_file: false,
            skip_gitignore,
            force_neural: false,
            disable_memory_expansion: true,
        };
        let response_has_ignored_file = |response: DaemonResponse| match response {
            DaemonResponse::SearchResults { hits, .. } => hits
                .iter()
                .any(|hit| hit.file_path.as_path() == Path::new("ignored.rs")),
            other => panic!("expected search results, got {other:?}"),
        };

        let false_lease = state.acquire_workspace_mode(&workspace, false);
        let same_mode = tokio::spawn(handle_request(state.clone(), literal_request(false)));
        let same_mode = tokio::time::timeout(Duration::from_secs(2), same_mode)
            .await
            .expect("same-mode search should remain concurrent")
            .unwrap();
        assert!(!response_has_ignored_file(same_mode));

        let opposite = tokio::spawn(handle_request(state.clone(), literal_request(true)));
        let coordinator = state.workspace_mode_coordinator(&workspace.id);
        for _ in 0..100 {
            if coordinator.state.lock().next_mode == Some(true) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(coordinator.state.lock().next_mode, Some(true));
        assert!(!opposite.is_finished());
        drop(false_lease);
        assert!(response_has_ignored_file(opposite.await.unwrap()));
        assert!(response_has_ignored_file(
            handle_request(state.clone(), hybrid_request(true)).await
        ));
        assert_eq!(state.query_results.lock().results.len(), 1);

        let true_lease = state.acquire_workspace_mode(&workspace, true);
        let index = tokio::spawn(handle_request(
            state.clone(),
            DaemonRequest::Index {
                path: workspace.root.clone(),
                watch: false,
                skip_gitignore: false,
            },
        ));
        for _ in 0..100 {
            if coordinator.state.lock().exclusive_waiters > 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(coordinator.state.lock().exclusive_waiters > 0);
        assert!(!index.is_finished());
        drop(true_lease);
        assert!(matches!(index.await.unwrap(), DaemonResponse::Ack { .. }));
        assert!(state.query_results.lock().results.is_empty());
        assert!(!response_has_ignored_file(
            handle_request(state.clone(), hybrid_request(false)).await
        ));
        assert_eq!(state.query_results.lock().results.len(), 1);
    }

    #[test]
    #[serial]
    fn linked_worktree_readers_share_base_until_base_mutation() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let repositories = tempdir().unwrap();
        let main = repositories.path().join("main");
        let first_path = repositories.path().join("first");
        let second_path = repositories.path().join("second");
        std::fs::create_dir(&main).unwrap();
        git(&main, &["init", "-b", "main"]);
        std::fs::write(main.join("lib.rs"), "pub fn shared_worktree_base() {}\n").unwrap();
        git(&main, &["add", "lib.rs"]);
        git(&main, &["commit", "-m", "seed shared base"]);
        for path in [&first_path, &second_path] {
            git(
                &main,
                &[
                    "worktree",
                    "add",
                    "--detach",
                    path.to_str().unwrap(),
                    "HEAD",
                ],
            );
        }

        let base = Workspace::resolve(&main).unwrap();
        let first = Workspace::resolve(&first_path).unwrap();
        let second = Workspace::resolve(&second_path).unwrap();
        let model = create_hash_model();
        index_workspace(&base, model.as_ref()).unwrap();
        index_workspace(&first, model.as_ref()).unwrap();
        index_workspace(&second, model.as_ref()).unwrap();
        let state = test_state();
        let coordinator = state.workspace_mode_coordinator(&base.id);
        let preparation_leases = state.acquire_workspace_modes(std::slice::from_ref(&first), false);
        assert!(
            coordinator.state.lock().exclusive_active,
            "unprepared worktree searches must exclusively lock the base during reconciliation"
        );
        drop(preparation_leases);
        for workspace in [&first, &second] {
            state.store_workspace_ready(workspace, false, workspace_readiness_signature(workspace));
        }

        let first_leases = state.acquire_workspace_modes(std::slice::from_ref(&first), false);
        assert!(
            !coordinator.state.lock().exclusive_active,
            "read-only worktree search must not exclusively lock its shared base"
        );

        let second_leases = state.acquire_workspace_modes(std::slice::from_ref(&second), false);
        assert_eq!(coordinator.state.lock().active_leases, 2);

        let mutation_state = state.clone();
        let mutation_base = base.clone();
        let (started, acquired) = std::sync::mpsc::channel();
        let writer = std::thread::spawn(move || {
            let leases = mutation_state.acquire_workspace_mutations(&[mutation_base]);
            started.send(()).unwrap();
            leases
        });
        assert!(
            acquired.recv_timeout(Duration::from_millis(100)).is_err(),
            "base mutation must wait for active worktree readers"
        );
        drop(first_leases);
        drop(second_leases);
        acquired.recv_timeout(Duration::from_secs(2)).unwrap();
        drop(writer.join().unwrap());
    }

    async fn wait_for_initial_watch_reconciliation(state: &DaemonState, workspace: &Workspace) {
        let skip_gitignore = workspace.read_metadata().unwrap().unwrap().skip_gitignore;
        let leases = tokio::time::timeout(
            Duration::from_secs(10),
            state.acquire_search_leases(std::slice::from_ref(workspace), skip_gitignore, None),
        )
        .await
        .expect("initial watcher reconciliation timed out")
        .expect("initial watcher reconciliation failed")
        .expect("initial watcher reconciliation was cancelled");
        drop(leases);
    }

    fn literal_request_for(workspace: &Workspace, query: &str) -> DaemonRequest {
        DaemonRequest::LiteralSearch {
            path: Some(workspace.root.clone()),
            query: query.to_string(),
            limit: Some(5),
            context: 0,
            type_filter: None,
            include_globs: Vec::new(),
            exclude_globs: Vec::new(),
            scope_path: None,
            scope_is_file: false,
            skip_gitignore: false,
        }
    }

    fn index_request_for(workspace: &Workspace, skip_gitignore: bool) -> DaemonRequest {
        DaemonRequest::Index {
            path: workspace.root.clone(),
            watch: false,
            skip_gitignore,
        }
    }

    fn indexed_workspace(marker: &str) -> (tempfile::TempDir, Workspace) {
        let repo = tempdir().unwrap();
        std::fs::write(
            repo.path().join("lib.rs"),
            format!("pub fn {marker}() -> bool {{ true }}\n"),
        )
        .unwrap();
        let workspace = Workspace::resolve(repo.path()).unwrap();
        index_workspace(&workspace, create_hash_model().as_ref()).unwrap();
        (repo, workspace)
    }

    #[test]
    #[serial]
    fn uncontended_search_leases_are_granted_inline_and_contended_ones_are_not() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let repo = tempdir().unwrap();
        let workspace = Workspace::resolve(repo.path()).unwrap();
        let state = test_state();

        let first = state
            .try_acquire_search_leases_inline(std::slice::from_ref(&workspace), false)
            .expect("idle workspace grants a shared lease inline");
        let second = state
            .try_acquire_search_leases_inline(std::slice::from_ref(&workspace), false)
            .expect("shared leases stack in the same mode");
        assert!(
            state
                .try_acquire_search_leases_inline(std::slice::from_ref(&workspace), true)
                .is_none(),
            "a different ignore mode must wait for the active readers"
        );
        drop(first);
        drop(second);

        let exclusive = state.acquire_workspace_mutations(std::slice::from_ref(&workspace));
        assert!(
            state
                .try_acquire_search_leases_inline(std::slice::from_ref(&workspace), false)
                .is_none(),
            "an exclusive holder makes the inline path decline instead of waiting"
        );
        drop(exclusive);
        assert!(
            state
                .try_acquire_search_leases_inline(std::slice::from_ref(&workspace), false)
                .is_some()
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[serial]
    async fn requests_parked_on_a_workspace_lease_do_not_hold_cpu_permits() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let (_repo_a, workspace_a) = indexed_workspace("lease_parked_marker");
        let (_repo_b, workspace_b) = indexed_workspace("other_workspace_marker");

        let mut state = test_state();
        // One CPU permit: any parked request that still held a permit would
        // starve every other workspace.
        state.cpu_permits = Arc::new(tokio::sync::Semaphore::new(1));
        // Simulate a long-running index holding A's exclusive lease.
        let held = state.acquire_workspace_mutations(std::slice::from_ref(&workspace_a));

        let mut parked = Vec::new();
        for _ in 0..2 {
            parked.push(tokio::spawn(handle_request(
                state.clone(),
                literal_request_for(&workspace_a, "lease_parked_marker"),
            )));
        }
        parked.push(tokio::spawn(handle_request(
            state.clone(),
            DaemonRequest::Search {
                path: Some(workspace_a.root.clone()),
                query: "lease parked marker".to_string(),
                limit: Some(5),
                context: 0,
                type_filter: None,
                include_globs: Vec::new(),
                exclude_globs: Vec::new(),
                scope_path: None,
                scope_is_file: false,
                skip_gitignore: false,
                force_neural: false,
                disable_memory_expansion: true,
            },
        )));
        parked.push(tokio::spawn(handle_request(
            state.clone(),
            index_request_for(&workspace_a, false),
        )));

        let coordinator = state.workspace_mode_coordinator(&workspace_a.id);
        tokio::time::timeout(Duration::from_secs(5), async {
            while coordinator.state.lock().exclusive_waiters == 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("index request should queue for A's exclusive lease");
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(parked.iter().all(|task| !task.is_finished()));
        assert_eq!(
            state.cpu_permits.available_permits(),
            1,
            "requests waiting for a workspace lease must not hold CPU permits"
        );

        let other = tokio::time::timeout(
            Duration::from_secs(5),
            handle_request(
                state.clone(),
                literal_request_for(&workspace_b, "other_workspace_marker"),
            ),
        )
        .await
        .expect("search on an idle workspace must not wait for A's lease");
        match other {
            DaemonResponse::SearchResults { hits, .. } => assert!(!hits.is_empty()),
            other => panic!("expected search results, got {other:?}"),
        }
        assert_eq!(state.cpu_permits.available_permits(), 1);

        drop(held);
        for task in parked {
            let response = tokio::time::timeout(Duration::from_secs(30), task)
                .await
                .unwrap()
                .unwrap();
            assert!(
                matches!(
                    response,
                    DaemonResponse::SearchResults { .. } | DaemonResponse::Ack { .. }
                ),
                "parked request should complete once the lease frees: {response:?}"
            );
        }
        assert_eq!(state.cpu_permits.available_permits(), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[serial]
    async fn concurrent_index_requests_coalesce_into_one_run() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let repo = tempdir().unwrap();
        for index in 0..3 {
            std::fs::write(
                repo.path().join(format!("module_{index}.rs")),
                format!("pub fn coalesced_marker_{index}() -> usize {{ {index} }}\n"),
            )
            .unwrap();
        }
        let workspace = Workspace::resolve(repo.path()).unwrap();
        let state = test_state();
        let held = state.acquire_workspace_mutations(std::slice::from_ref(&workspace));

        let requests = (0..5)
            .map(|_| {
                tokio::spawn(handle_request(
                    state.clone(),
                    index_request_for(&workspace, false),
                ))
            })
            .collect::<Vec<_>>();
        let coordinator = state.workspace_mode_coordinator(&workspace.id);
        tokio::time::timeout(Duration::from_secs(5), async {
            while coordinator.state.lock().exclusive_waiters == 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("leader should queue for the exclusive lease");
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(
            coordinator.state.lock().exclusive_waiters,
            1,
            "only the leader waits for the lease; followers await its outcome"
        );
        assert!(state.inflight_indexes.lock().contains_key(&workspace.id));

        drop(held);
        let mut messages = Vec::new();
        for request in requests {
            match tokio::time::timeout(Duration::from_secs(30), request)
                .await
                .unwrap()
                .unwrap()
            {
                DaemonResponse::Ack { message } => messages.push(message),
                other => panic!("expected ack, got {other:?}"),
            }
        }
        assert!(
            messages[0].starts_with("indexed 3 files"),
            "leader should index every file: {}",
            messages[0]
        );
        assert!(
            messages.iter().all(|message| message == &messages[0]),
            "followers must share the leader's outcome: {messages:?}"
        );
        assert!(state.inflight_indexes.lock().is_empty());
        let generation = workspace.read_metadata().unwrap().unwrap().index_generation;
        assert_eq!(generation, 1, "exactly one index run should have committed");

        // A request that waited for the lease while a full walk that STARTED
        // AFTER it arrived advanced the generation skips the redundant rescan.
        let held = state.acquire_workspace_mutations(std::slice::from_ref(&workspace));
        let late = tokio::spawn(handle_request(
            state.clone(),
            index_request_for(&workspace, false),
        ));
        tokio::time::timeout(Duration::from_secs(5), async {
            while coordinator.state.lock().exclusive_waiters == 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        std::fs::write(
            repo.path().join("module_0.rs"),
            "pub fn coalesced_marker_0() -> usize { 100 }\n",
        )
        .unwrap();
        state.note_full_index_run_start(&workspace.id);
        index_workspace(&workspace, create_hash_model().as_ref()).unwrap();
        assert_eq!(
            workspace.read_metadata().unwrap().unwrap().index_generation,
            2
        );
        drop(held);
        match tokio::time::timeout(Duration::from_secs(30), late)
            .await
            .unwrap()
            .unwrap()
        {
            DaemonResponse::Ack { message } => {
                assert!(message.contains("already current"), "{message}");
            }
            other => panic!("expected ack, got {other:?}"),
        }
        assert_eq!(
            workspace.read_metadata().unwrap().unwrap().index_generation,
            2,
            "skipped rescan must not touch the index generation"
        );

        // A walk that started BEFORE the request arrived cannot vouch for edits
        // made afterwards: the request must rescan even though the generation
        // advanced while it waited.
        state.note_full_index_run_start(&workspace.id);
        tokio::time::sleep(Duration::from_millis(20)).await;
        let held = state.acquire_workspace_mutations(std::slice::from_ref(&workspace));
        let late = tokio::spawn(handle_request(
            state.clone(),
            index_request_for(&workspace, false),
        ));
        tokio::time::timeout(Duration::from_secs(5), async {
            while coordinator.state.lock().exclusive_waiters == 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        // The earlier walk commits without this edit, then the edit lands.
        index_workspace(&workspace, create_hash_model().as_ref()).unwrap();
        std::fs::write(
            repo.path().join("late_edit.rs"),
            "pub fn late_edit_marker() -> usize { 7 }\n",
        )
        .unwrap();
        let generation_before_late = workspace.read_metadata().unwrap().unwrap().index_generation;
        drop(held);
        match tokio::time::timeout(Duration::from_secs(30), late)
            .await
            .unwrap()
            .unwrap()
        {
            DaemonResponse::Ack { message } => {
                assert!(
                    !message.contains("already current"),
                    "a walk that predates the request must not satisfy it: {message}"
                );
                assert!(message.contains("indexed 1 file"), "{message}");
            }
            other => panic!("expected ack, got {other:?}"),
        }
        assert!(
            workspace.read_metadata().unwrap().unwrap().index_generation > generation_before_late,
            "the late edit must be picked up by the request's own rescan"
        );
    }

    /// `StartIndex` returns before the run, registers the workspace for status
    /// immediately, joins an in-flight run instead of queuing another, and
    /// reports `index_in_flight` through `RuntimeStatus` until the run commits.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[serial]
    async fn start_index_enqueues_and_reports_in_flight_until_commit() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let repo = tempdir().unwrap();
        for index in 0..3 {
            std::fs::write(
                repo.path().join(format!("module_{index}.rs")),
                format!("pub fn start_index_marker_{index}() -> usize {{ {index} }}\n"),
            )
            .unwrap();
        }
        let workspace = Workspace::resolve(repo.path()).unwrap();
        let state = test_state();
        // Park the run behind the exclusive lease so the request must return
        // while the index is still pending.
        let held = state.acquire_workspace_mutations(std::slice::from_ref(&workspace));

        let start_request = DaemonRequest::StartIndex {
            path: workspace.root.clone(),
            watch: false,
            skip_gitignore: false,
        };
        let started = std::time::Instant::now();
        let first = tokio::time::timeout(
            Duration::from_secs(5),
            handle_request(state.clone(), start_request.clone()),
        )
        .await
        .expect("StartIndex must not wait for the run");
        assert!(started.elapsed() < Duration::from_secs(5));
        match first {
            DaemonResponse::IndexStarted {
                accepted,
                already_running,
                generation,
            } => {
                assert!(accepted);
                assert!(!already_running, "first request leads the run");
                assert_eq!(generation, None, "no index existed before the run");
            }
            other => panic!("expected IndexStarted, got {other:?}"),
        }
        assert!(
            workspace.read_metadata().unwrap().is_some(),
            "accepted run registers the workspace before it holds the lease"
        );
        assert!(state.index_in_flight(&workspace.id));

        // A second request with the same options follows the queued run.
        match handle_request(state.clone(), start_request.clone()).await {
            DaemonResponse::IndexStarted {
                already_running, ..
            } => assert!(already_running),
            other => panic!("expected IndexStarted, got {other:?}"),
        }
        // A blocking `Index` also joins the same run instead of queuing.
        let follower = tokio::spawn(handle_request(
            state.clone(),
            index_request_for(&workspace, false),
        ));
        tokio::time::sleep(Duration::from_millis(100)).await;
        let coordinator = state.workspace_mode_coordinator(&workspace.id);
        assert_eq!(
            coordinator.state.lock().exclusive_waiters,
            1,
            "only the StartIndex leader waits for the lease"
        );

        match handle_request(
            state.clone(),
            DaemonRequest::RuntimeStatus {
                path: Some(workspace.root.clone()),
            },
        )
        .await
        {
            DaemonResponse::RuntimeStatus {
                workspace: Some(status),
                ..
            } => assert!(status.index_in_flight),
            other => panic!("expected RuntimeStatus, got {other:?}"),
        }

        drop(held);
        match tokio::time::timeout(Duration::from_secs(30), follower)
            .await
            .unwrap()
            .unwrap()
        {
            DaemonResponse::Ack { message } => {
                assert!(message.starts_with("indexed 3 files"), "{message}");
            }
            other => panic!("expected Ack, got {other:?}"),
        }
        tokio::time::timeout(Duration::from_secs(5), async {
            while state.index_in_flight(&workspace.id) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("in-flight entry clears once the run publishes");
        match handle_request(
            state.clone(),
            DaemonRequest::RuntimeStatus {
                path: Some(workspace.root.clone()),
            },
        )
        .await
        {
            DaemonResponse::RuntimeStatus {
                workspace: Some(status),
                ..
            } => assert!(!status.index_in_flight),
            other => panic!("expected RuntimeStatus, got {other:?}"),
        }
        assert!(workspace_is_indexed(&workspace));
        assert_eq!(
            workspace.read_metadata().unwrap().unwrap().index_generation,
            1,
            "exactly one run committed for three requests"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[serial]
    async fn client_disconnect_cancels_parked_search() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let (_repo, workspace) = indexed_workspace("disconnect_marker");
        let state = test_state();
        let held = state.acquire_workspace_mutations(std::slice::from_ref(&workspace));

        let request_id = uuid::Uuid::new_v4();
        let envelope = DaemonRequestEnvelope::with_request_id(
            literal_request_for(&workspace, "disconnect_marker"),
            request_id,
        );
        let (client, server) = tokio::io::duplex(1024);
        let task_state = state.clone();
        let task = tokio::spawn(async move {
            let mut reader = BufReader::new(server);
            handle_client_request(task_state, envelope, &mut reader).await
        });
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(!task.is_finished(), "search should be parked on the lease");
        assert!(matches!(
            state.search_cancellations.lock().entries.get(&request_id),
            Some(SearchCancellationEntry::Active(_))
        ));

        drop(client);
        let outcome = tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("disconnect must cancel the parked search")
            .unwrap();
        assert!(matches!(outcome, ClientRequestOutcome::Disconnected));
        assert!(
            state.search_cancellations.lock().entries.is_empty(),
            "cancelled registration must be released"
        );
        drop(held);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[serial]
    async fn search_deadline_returns_partial_results_with_warning() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        unsafe { std::env::set_var("IVYGREP_SEARCH_DEADLINE_SECS", "1") };
        let (_repo, workspace) = indexed_workspace("deadline_marker");
        let state = test_state();
        let held = state.acquire_workspace_mutations(std::slice::from_ref(&workspace));

        let envelope =
            DaemonRequestEnvelope::new(literal_request_for(&workspace, "deadline_marker"));
        let (_client, server) = tokio::io::duplex(1024);
        let task_state = state.clone();
        let started = std::time::Instant::now();
        let task = tokio::spawn(async move {
            let mut reader = BufReader::new(server);
            handle_client_request(task_state, envelope, &mut reader).await
        });
        let outcome = tokio::time::timeout(Duration::from_secs(10), task)
            .await
            .expect("deadline must release the parked search")
            .unwrap();
        unsafe { std::env::remove_var("IVYGREP_SEARCH_DEADLINE_SECS") };
        assert!(started.elapsed() >= Duration::from_secs(1));
        match outcome {
            ClientRequestOutcome::Respond(DaemonResponse::SearchResults { hits, warnings }) => {
                assert!(hits.is_empty());
                assert_eq!(warnings.len(), 1, "{warnings:?}");
                assert!(
                    warnings[0].contains("deadline of 1s exceeded"),
                    "{warnings:?}"
                );
            }
            ClientRequestOutcome::Respond(other) => {
                panic!("deadline should return partial results, got {other:?}")
            }
            ClientRequestOutcome::Disconnected => panic!("client stayed connected"),
        }
        drop(held);

        // Once the lease is free the same request completes normally.
        let response = tokio::time::timeout(
            Duration::from_secs(10),
            handle_request(
                state.clone(),
                literal_request_for(&workspace, "deadline_marker"),
            ),
        )
        .await
        .unwrap();
        match response {
            DaemonResponse::SearchResults { hits, warnings } => {
                assert!(!hits.is_empty());
                assert!(warnings.is_empty());
            }
            other => panic!("expected search results, got {other:?}"),
        }
    }

    #[tokio::test]
    #[serial]
    async fn cancel_request_stops_search_waiting_for_opposite_workspace_mode() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let repo = tempdir().unwrap();
        let workspace = Workspace::resolve(repo.path()).unwrap();
        let state = test_state();
        let active_lease = state.acquire_workspace_mode(&workspace, false);
        let request_id = uuid::Uuid::new_v4();
        let request = DaemonRequest::LiteralSearch {
            path: Some(workspace.root.clone()),
            query: "needle".to_string(),
            limit: Some(5),
            context: 0,
            type_filter: None,
            include_globs: Vec::new(),
            exclude_globs: Vec::new(),
            scope_path: None,
            scope_is_file: false,
            skip_gitignore: true,
        };
        let search_state = state.clone();
        let search = tokio::spawn(async move {
            handle_enveloped_request(
                search_state,
                DaemonRequestEnvelope::with_request_id(request, request_id),
            )
            .await
        });

        let coordinator = state.workspace_mode_coordinator(&workspace.id);
        tokio::time::timeout(Duration::from_secs(2), async {
            while coordinator.state.lock().next_mode != Some(true) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("opposite-mode search should enter the workspace-mode queue");

        let cancel = tokio::time::timeout(
            Duration::from_secs(1),
            handle_request(
                state.clone(),
                DaemonRequest::CancelSearch {
                    search_id: request_id,
                },
            ),
        )
        .await
        .expect("cancellation should not wait for the active workspace lease");
        assert!(matches!(cancel, DaemonResponse::Ack { .. }));
        let response = tokio::time::timeout(Duration::from_secs(1), search)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            response,
            DaemonResponse::Error { message } if message == "search cancelled"
        ));
        assert_eq!(coordinator.state.lock().next_mode, None);
        assert!(state.search_cancellations.lock().entries.is_empty());

        drop(active_lease);
    }

    #[test]
    fn cancelled_waiter_preserves_other_workspace_mode_reservations() {
        let coordinator = Arc::new(WorkspaceModeCoordinator::default());
        let active_lease = coordinator.acquire_shared(false, None).unwrap();
        let owner_cancellation = Arc::new(AtomicBool::new(false));
        let owner_coordinator = coordinator.clone();
        let owner_cancellation_thread = owner_cancellation.clone();
        let owner = std::thread::spawn(move || {
            owner_coordinator.acquire_shared(true, Some(&owner_cancellation_thread))
        });

        for _ in 0..100 {
            if coordinator.state.lock().next_mode_waiters == 1 {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(coordinator.state.lock().next_mode_waiters, 1);

        let waiter_cancellation = Arc::new(AtomicBool::new(false));
        let waiter_coordinator = coordinator.clone();
        let waiter_cancellation_thread = waiter_cancellation.clone();
        let waiter = std::thread::spawn(move || {
            waiter_coordinator.acquire_shared(true, Some(&waiter_cancellation_thread))
        });
        for _ in 0..100 {
            if coordinator.state.lock().next_mode_waiters == 2 {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(coordinator.state.lock().next_mode_waiters, 2);

        let cancelled = AtomicBool::new(true);
        assert!(coordinator.acquire_shared(true, Some(&cancelled)).is_none());
        assert_eq!(coordinator.state.lock().next_mode, Some(true));
        assert_eq!(coordinator.state.lock().next_mode_waiters, 2);

        owner_cancellation.store(true, Ordering::Relaxed);
        assert!(owner.join().unwrap().is_none());
        assert_eq!(coordinator.state.lock().next_mode, Some(true));
        assert_eq!(coordinator.state.lock().next_mode_waiters, 1);

        drop(active_lease);
        drop(waiter.join().unwrap());
    }

    #[test]
    #[serial]
    fn watcher_updates_tracked_and_unignored_sources_in_build_named_directories() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let repo = tempdir().unwrap();
        git(repo.path(), &["init", "-b", "main"]);
        std::fs::create_dir_all(repo.path().join("target")).unwrap();
        std::fs::create_dir_all(repo.path().join("dist")).unwrap();
        std::fs::write(
            repo.path().join("target/generated.rs"),
            "pub fn tracked_build_old_marker() {}\n",
        )
        .unwrap();
        std::fs::write(
            repo.path().join("dist/kept.rs"),
            "pub fn unignored_build_old_marker() {}\n",
        )
        .unwrap();
        std::fs::write(repo.path().join(".gitignore"), "dist/*\n!dist/kept.rs\n").unwrap();
        git(repo.path(), &["add", ".gitignore", "target/generated.rs"]);
        git(repo.path(), &["commit", "-m", "seed build paths"]);

        let workspace = Workspace::resolve(repo.path()).unwrap();
        let model = create_hash_model();
        index_workspace(&workspace, model.as_ref()).unwrap();
        assert!(indexed_file_contains(
            &workspace,
            "target/generated.rs",
            "tracked_build_old_marker"
        ));
        assert!(indexed_file_contains(
            &workspace,
            "dist/kept.rs",
            "unignored_build_old_marker"
        ));

        std::fs::write(
            repo.path().join("target/generated.rs"),
            "pub fn tracked_build_new_marker() {}\n",
        )
        .unwrap();
        std::fs::write(
            repo.path().join("dist/kept.rs"),
            "pub fn unignored_build_new_marker() {}\n",
        )
        .unwrap();
        let mut filter = WatchEventFilter::new(&workspace);
        let change = filter.change_for_event(
            &notify::Event::new(notify::EventKind::Any)
                .add_path(repo.path().join("target/generated.rs"))
                .add_path(repo.path().join("dist/kept.rs")),
        );
        let WatchChange::Paths(paths) = change else {
            panic!("valid build paths should produce a targeted watcher change");
        };
        assert_eq!(paths.len(), 2);
        let paths = paths.into_iter().collect::<Vec<_>>();
        index_workspace_paths_for_watcher(&workspace, model.as_ref(), &paths).unwrap();

        assert!(indexed_file_contains(
            &workspace,
            "target/generated.rs",
            "tracked_build_new_marker"
        ));
        assert!(indexed_file_contains(
            &workspace,
            "dist/kept.rs",
            "unignored_build_new_marker"
        ));
    }

    #[test]
    #[serial]
    fn repository_exclude_events_refresh_filter_and_force_complete_reconciliation() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let repo = tempdir().unwrap();
        git(repo.path(), &["init", "-b", "main"]);
        std::fs::write(
            repo.path().join("visible.rs"),
            "pub fn repository_exclude_marker() {}\n",
        )
        .unwrap();
        git(repo.path(), &["add", "visible.rs"]);
        git(repo.path(), &["commit", "-m", "seed visible source"]);

        let workspace = Workspace::resolve(repo.path()).unwrap();
        let model = create_hash_model();
        index_workspace(&workspace, model.as_ref()).unwrap();
        let source = repo.path().join("visible.rs");
        let exclude = crate::workspace::git_common_dir(repo.path())
            .unwrap()
            .join("info/exclude");
        let mut filter = WatchEventFilter::new(&workspace);
        assert!(filter.path_should_reindex(&source));

        std::fs::write(&exclude, "visible.rs\n").unwrap();
        assert!(matches!(
            filter.change_for_event(
                &notify::Event::new(notify::EventKind::Any).add_path(exclude.clone())
            ),
            WatchChange::FullReconciliation
        ));
        assert!(!filter.path_should_reindex(&source));
        index_workspace_for_watcher(&workspace, model.as_ref()).unwrap();
        assert!(!indexed_file_contains(
            &workspace,
            "visible.rs",
            "repository_exclude_marker"
        ));

        std::fs::write(&exclude, "").unwrap();
        assert!(matches!(
            filter.change_for_event(
                &notify::Event::new(notify::EventKind::Any).add_path(exclude.clone())
            ),
            WatchChange::FullReconciliation
        ));
        assert!(filter.path_should_reindex(&source));
        index_workspace_for_watcher(&workspace, model.as_ref()).unwrap();
        assert!(indexed_file_contains(
            &workspace,
            "visible.rs",
            "repository_exclude_marker"
        ));
    }

    #[test]
    #[serial]
    fn ignore_configuration_inside_ivygrep_storage_does_not_reconcile_workspace() {
        let repo = tempdir().unwrap();
        let home = repo.path().join("local-state");
        std::fs::create_dir_all(&home).unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", &home) };

        let workspace = Workspace::resolve(repo.path()).unwrap();
        let mut filter = WatchEventFilter::new(&workspace);
        for name in [".gitignore", ".ignore"] {
            let ignore = home.join(name);
            std::fs::write(&ignore, "*.rs\n").unwrap();
            assert!(matches!(
                filter
                    .change_for_event(&notify::Event::new(notify::EventKind::Any).add_path(ignore)),
                WatchChange::None
            ));
        }
    }

    #[test]
    #[serial]
    fn watcher_backend_error_supersedes_coalesced_paths_with_full_reconciliation() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let repo = tempdir().unwrap();
        let workspace = Workspace::resolve(repo.path()).unwrap();
        let control = WatchControl::new(workspace.clone());
        let filter = Mutex::new(WatchEventFilter::new(&workspace));

        let source = repo.path().join("recovered.rs");
        std::fs::write(&source, "pub fn recovered() {}\n").unwrap();
        std::fs::write(repo.path().join(".gitignore"), "recovered.rs\n").unwrap();
        filter.lock().refresh();
        assert!(!filter.lock().path_should_reindex(&source));

        control.mark_paths_dirty([PathBuf::from("before.rs")]);
        std::fs::write(repo.path().join(".gitignore"), "").unwrap();
        handle_watch_result(
            &control,
            &filter,
            Err(notify::Error::generic("injected watcher overflow")),
        );
        control.mark_paths_dirty([PathBuf::from("after.rs")]);

        let pending = control.take_pending_work().unwrap();
        assert!(matches!(pending.change, WatchChange::FullReconciliation));
        assert!(
            pending
                .backend_error
                .as_deref()
                .is_some_and(|error| error.contains("injected watcher overflow"))
        );
        assert!(filter.lock().path_should_reindex(&source));
        assert!(control.take_pending_work().is_none());

        std::fs::write(repo.path().join(".gitignore"), "recovered.rs\n").unwrap();
        handle_watch_result(
            &control,
            &filter,
            Ok(notify::Event::new(notify::EventKind::Other).set_flag(notify::event::Flag::Rescan)),
        );
        assert!(matches!(
            control.take_pending_work().unwrap().change,
            WatchChange::FullReconciliation
        ));
        assert!(!filter.lock().path_should_reindex(&source));
    }

    #[test]
    #[serial]
    fn failed_watch_index_requeues_full_reconciliation_and_preserves_error_phase() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let repo = tempdir().unwrap();
        let workspace = Workspace::resolve(repo.path()).unwrap();
        let control = WatchControl::new(workspace);

        control.mark_paths_dirty([PathBuf::from("changed.rs")]);
        assert!(control.take_pending_work().is_some());
        assert!(!control.dirty.load(Ordering::Relaxed));

        control.requeue_failed_index("injected temporary indexing failure".to_string());

        assert!(control.dirty.load(Ordering::Relaxed));
        assert_eq!(control.snapshot_phase().0, "error");
        let retry = control.take_pending_work().unwrap();
        assert!(matches!(retry.change, WatchChange::FullReconciliation));
        assert_eq!(
            retry.backend_error.as_deref(),
            Some("injected temporary indexing failure")
        );
    }

    #[test]
    fn watch_index_retry_backoff_is_exponential_and_bounded() {
        assert_eq!(watch_retry_delay(1), Duration::from_millis(250));
        assert_eq!(watch_retry_delay(2), Duration::from_millis(500));
        assert_eq!(watch_retry_delay(3), Duration::from_secs(1));
        assert_eq!(watch_retry_delay(100), WATCH_RETRY_MAX_DELAY);
    }

    #[tokio::test]
    #[serial]
    async fn failed_watch_index_recovers_without_another_repository_event() {
        assert_failed_watch_index_recovers(false).await;
    }

    #[tokio::test]
    #[serial]
    async fn failed_tantivy_publication_watch_update_recovers_without_new_event() {
        assert_failed_watch_index_recovers(true).await;
    }

    async fn assert_failed_watch_index_recovers(fail_tantivy_publication: bool) {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let repo = tempdir().unwrap();
        let source = repo.path().join("lib.rs");
        std::fs::write(&source, "pub fn original_watch_marker() {}\n").unwrap();

        let workspace = Workspace::resolve(repo.path()).unwrap();
        let model = create_hash_model();
        index_workspace(&workspace, model.as_ref()).unwrap();
        let state = test_state();
        register_watcher(&state, repo.path()).unwrap();
        wait_for_initial_watch_reconciliation(&state, &workspace).await;
        let metadata = std::fs::read(workspace.metadata_path()).unwrap();
        let generation = workspace.read_metadata().unwrap().unwrap().index_generation;
        let control = state
            .watchers
            .lock()
            .get(&workspace.id)
            .unwrap()
            .control
            .clone();

        let publication_failure = if fail_tantivy_publication {
            Some(crate::indexer::fail_tantivy_commits(
                &workspace.tantivy_dir(),
            ))
        } else {
            std::fs::write(workspace.metadata_path(), "invalid metadata").unwrap();
            None
        };
        std::fs::write(&source, "pub fn recovered_watch_marker() {}\n").unwrap();
        control.mark_paths_dirty([PathBuf::from("lib.rs")]);

        for _ in 0..60 {
            if control.retrying.load(Ordering::Relaxed) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(control.retrying.load(Ordering::Relaxed));
        assert_eq!(control.snapshot_phase().0, "error");
        if fail_tantivy_publication {
            assert!(
                control
                    .pending_work
                    .lock()
                    .backend_error
                    .as_ref()
                    .is_some_and(|error| {
                        error.contains("injected Tantivy metadata publication failure")
                    })
            );
        }

        drop(publication_failure);
        std::fs::write(workspace.metadata_path(), metadata).unwrap();
        let mut recovered = false;
        for _ in 0..60 {
            if workspace.read_metadata().unwrap().unwrap().index_generation > generation
                && indexed_literal_visible(&workspace, "recovered_watch_marker") == Some(true)
            {
                recovered = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        stop_all_watchers(&state);

        assert!(recovered, "failed watcher update did not recover");
    }

    #[tokio::test]
    #[serial]
    async fn removing_workspace_cancels_pending_watcher_retry() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let repo = tempdir().unwrap();
        std::fs::write(repo.path().join("lib.rs"), "pub fn pending_retry() {}\n").unwrap();

        let workspace = Workspace::resolve(repo.path()).unwrap();
        let model = create_hash_model();
        index_workspace(&workspace, model.as_ref()).unwrap();
        let state = test_state();
        register_watcher(&state, repo.path()).unwrap();
        wait_for_initial_watch_reconciliation(&state, &workspace).await;
        let control = state
            .watchers
            .lock()
            .get(&workspace.id)
            .unwrap()
            .control
            .clone();

        std::fs::write(workspace.metadata_path(), "invalid metadata").unwrap();
        control.mark_paths_dirty([PathBuf::from("lib.rs")]);
        for _ in 0..60 {
            if control.retrying.load(Ordering::Relaxed) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        assert!(control.retrying.load(Ordering::Relaxed));

        let response = handle_request(
            state,
            DaemonRequest::Remove {
                path: repo.path().to_path_buf(),
            },
        )
        .await;
        assert!(matches!(response, DaemonResponse::Ack { .. }));
        assert!(!workspace.sqlite_path().exists());
        assert!(!workspace.metadata_path().exists());

        tokio::time::sleep(WATCH_RETRY_INITIAL_DELAY.saturating_mul(2)).await;
        assert!(
            !workspace.sqlite_path().exists() && !workspace.metadata_path().exists(),
            "a stopped retry recreated the index"
        );
        assert!(!control.indexing.load(Ordering::Relaxed));
    }

    #[tokio::test]
    #[serial]
    async fn linked_worktree_watcher_reconciles_external_common_git_exclude_toggles() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let repositories = tempdir().unwrap();
        let main = repositories.path().join("main");
        let linked = repositories.path().join("linked");
        std::fs::create_dir(&main).unwrap();
        git(&main, &["init", "-b", "main"]);
        std::fs::write(
            main.join("shared.rs"),
            "pub fn external_common_git_marker() {}\n",
        )
        .unwrap();
        git(&main, &["add", "shared.rs"]);
        git(&main, &["commit", "-m", "seed linked source"]);
        git(
            &main,
            &["worktree", "add", "--detach", linked.to_str().unwrap()],
        );

        let workspace = Workspace::resolve(&linked).unwrap();
        let model = create_hash_model();
        index_workspace(&workspace, model.as_ref()).unwrap();
        assert_eq!(
            indexed_literal_visible(&workspace, "external_common_git_marker"),
            Some(true)
        );

        let exclude = crate::workspace::git_common_dir(&linked)
            .unwrap()
            .join("info/exclude");
        assert!(!exclude.starts_with(&linked));
        let state = test_state();
        register_watcher(&state, &linked).unwrap();

        std::fs::write(&exclude, "shared.rs\n").unwrap();
        let disappeared =
            wait_for_literal_visibility(&workspace, "external_common_git_marker", false).await;
        std::fs::write(&exclude, "").unwrap();
        let reappeared =
            wait_for_literal_visibility(&workspace, "external_common_git_marker", true).await;
        stop_all_watchers(&state);

        assert!(
            disappeared,
            "external common-dir exclude update did not remove linked-worktree result"
        );
        assert!(
            reappeared,
            "external common-dir exclude update did not restore linked-worktree result"
        );
    }

    #[tokio::test]
    #[serial]
    async fn linked_worktree_watcher_bootstraps_missing_external_git_info_directory() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let repositories = tempdir().unwrap();
        let main = repositories.path().join("main");
        let linked = repositories.path().join("linked");
        std::fs::create_dir(&main).unwrap();
        git(&main, &["init", "-b", "main"]);
        std::fs::write(
            main.join("shared.rs"),
            "pub fn missing_external_git_info_marker() {}\n",
        )
        .unwrap();
        git(&main, &["add", "shared.rs"]);
        git(&main, &["commit", "-m", "seed linked source"]);
        git(
            &main,
            &["worktree", "add", "--detach", linked.to_str().unwrap()],
        );

        let common_dir = crate::workspace::git_common_dir(&linked).unwrap();
        let info = common_dir.join("info");
        std::fs::remove_dir_all(&info).unwrap();
        assert!(!info.exists());

        let workspace = Workspace::resolve(&linked).unwrap();
        let model = create_hash_model();
        index_workspace(&workspace, model.as_ref()).unwrap();
        assert_eq!(
            indexed_literal_visible(&workspace, "missing_external_git_info_marker"),
            Some(true)
        );

        let state = test_state();
        register_watcher(&state, &linked).unwrap();
        assert_eq!(
            state
                .watchers
                .lock()
                .get(&workspace.id)
                .and_then(|registration| registration.external_git_watch.as_deref()),
            Some(common_dir.as_path())
        );

        std::fs::create_dir(&info).unwrap();
        let exclude = info.join("exclude");
        std::fs::write(&exclude, "shared.rs\n").unwrap();
        assert!(
            wait_for_literal_visibility(&workspace, "missing_external_git_info_marker", false)
                .await,
            "creating external info/exclude did not remove linked-worktree result"
        );
        assert_eq!(
            state
                .watchers
                .lock()
                .get(&workspace.id)
                .and_then(|registration| registration.external_git_watch.as_deref()),
            Some(info.as_path())
        );

        std::fs::remove_dir_all(&info).unwrap();
        assert!(
            wait_for_literal_visibility(&workspace, "missing_external_git_info_marker", true).await,
            "removing external info directory did not restore linked-worktree result"
        );
        assert_eq!(
            state
                .watchers
                .lock()
                .get(&workspace.id)
                .and_then(|registration| registration.external_git_watch.as_deref()),
            Some(common_dir.as_path())
        );

        std::fs::create_dir(&info).unwrap();
        std::fs::write(&exclude, "shared.rs\n").unwrap();
        assert!(
            wait_for_literal_visibility(&workspace, "missing_external_git_info_marker", false)
                .await,
            "recreating external info/exclude did not remove linked-worktree result"
        );
        assert_eq!(
            state
                .watchers
                .lock()
                .get(&workspace.id)
                .and_then(|registration| registration.external_git_watch.as_deref()),
            Some(info.as_path())
        );
        std::fs::write(&exclude, "").unwrap();
        assert!(
            wait_for_literal_visibility(&workspace, "missing_external_git_info_marker", true).await,
            "toggling recreated external info/exclude did not restore linked-worktree result"
        );
        stop_all_watchers(&state);
    }

    #[test]
    fn watcher_debounce_adapts_to_single_events_and_bursts() {
        assert_eq!(watch_quiet_period(1), WATCH_SINGLE_EVENT_QUIET_PERIOD);
        assert_eq!(watch_quiet_period(2), WATCH_BURST_QUIET_PERIOD);
        assert_eq!(watch_quiet_period(1_000), WATCH_BURST_QUIET_PERIOD);
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
