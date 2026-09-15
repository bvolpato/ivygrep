use std::env;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

/// Upper bound on a single JSON-RPC message / header line. Prevents a
/// malformed or malicious client (or `Content-Length` header) from triggering
/// an unbounded allocation or read.
const MAX_MESSAGE_BYTES: usize = 16 * 1024 * 1024;
/// Requests, counting each batch member, that may wait for the worker. Past
/// this or `MAX_QUEUED_BYTES` the reader stops reading until the worker takes
/// the next payload, so a client that floods requests or batches cannot grow
/// memory without bound. An empty queue still takes one payload of any size.
const MAX_QUEUED_REQUESTS: usize = 64;
/// Raw payload bytes that may wait for the worker.
const MAX_QUEUED_BYTES: usize = MAX_MESSAGE_BYTES;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

use parking_lot::{Condvar, Mutex};

use crate::config;
use crate::embedding::{EmbeddingModel, create_hash_model, create_neural_model};
use crate::indexer::{index_workspace, workspace_is_indexed};
use crate::path_glob::parse_glob_csv;
use crate::protocol::{
    DaemonRequest, DaemonResponse, FileSearchResult, SearchHit, group_hits_by_file,
};
use crate::regex_search::regex_search_with_options;
use crate::search::{SearchOptions, hybrid_search, literal_search};
use crate::symbols::{SymbolSearchMode, search_symbols_with_options};
use crate::workspace::{Workspace, WorkspaceMetadata, resolve_workspace_and_scope};

const JSONRPC_VERSION: &str = "2.0";
const LEGACY_PROTOCOL_VERSION: &str = "2024-11-05";
const LATEST_PROTOCOL_VERSION: &str = "2025-11-25";
const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &[
    LATEST_PROTOCOL_VERSION,
    "2025-06-18",
    "2025-03-26",
    LEGACY_PROTOCOL_VERSION,
];
const TOOL_IG_SEARCH: &str = "ig_search";
const TOOL_IG_STATUS: &str = "ig_status";
/// Result files returned by `ig_search` in hits mode when the caller omits
/// `limit`. Unbounded hit payloads (hundreds of chunks, ~40k tokens, emitted
/// twice) were the P0 failure mode for coding agents.
const DEFAULT_HITS_FILE_LIMIT: usize = 10;
/// Hits kept per result file in hits mode unless `hits_per_file` overrides it.
const DEFAULT_HITS_PER_FILE: usize = 3;
const MAX_HITS_PER_FILE: usize = 100;
/// Hits fetched per requested result file for ranked modes (hybrid, symbol
/// definitions). Keeps the default retrieval cost equal to the CLI default.
const RANKED_HIT_OVERFETCH: usize = 5;
/// Hits fetched per requested result file for enumerating modes (literal,
/// regex, references, callers), whose matches cluster inside few files.
const ENUMERATING_HIT_OVERFETCH: usize = 20;
const MIN_ENUMERATING_HIT_BUDGET: usize = 200;

#[derive(Debug)]
struct JsonRpcRequest {
    /// `None` for a notification (no `id` member). `Some(Value::Null)` is a
    /// request with a null id and still gets a response.
    id: Option<Value>,
    method: String,
    params: Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    /// Always serialized; `null` when the request id could not be determined.
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

#[derive(Debug)]
struct DispatchError {
    code: i64,
    message: String,
}

impl DispatchError {
    fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("method not found: {method}"),
        }
    }

    fn invalid_params(error: anyhow::Error) -> Self {
        Self {
            code: -32602,
            message: error.to_string(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ToolCallParams {
    name: String,
    #[serde(default)]
    arguments: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IvygrepSearchArgs {
    query: Option<String>,
    path: Option<String>,
    output: Option<String>,
    budget_tokens: Option<usize>,
    since: Option<String>,
    limit: Option<usize>,
    hits_per_file: Option<usize>,
    context: Option<usize>,
    #[serde(rename = "type")]
    type_filter: Option<String>,
    regex: Option<bool>,
    literal: Option<bool>,
    symbol: Option<bool>,
    refs: Option<bool>,
    callers: Option<bool>,
    include: Option<String>,
    exclude: Option<String>,
    first_line_only: Option<bool>,
    file_name_only: Option<bool>,
    verbose: Option<bool>,
    skip_gitignore: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IvygrepStatusArgs {}

pub fn serve_stdio() -> Result<()> {
    config::ensure_app_dirs()?;

    serve(
        BufReader::new(io::stdin()),
        io::stdout(),
        Arc::new(dispatch),
    )
}

/// Runs one JSON-RPC method. Tests inject slow or cancellable stand-ins.
type Dispatch = dyn Fn(&str, Value, &RequestCancellation) -> std::result::Result<Value, DispatchError>
    + Send
    + Sync;

/// Serve MCP over `reader` and `writer`. A reader thread keeps reading while a
/// request runs: it answers `ping` and undecodable messages at once and
/// applies cancellations as they arrive. A worker thread runs every other
/// request in arrival order, one at a time. Replies share one writer, so
/// frames never interleave. At EOF the requests already read still run, then
/// the server returns. A reply that cannot be written ends the session at
/// once, even while stdin stays open.
fn serve<R, W>(reader: R, writer: W, dispatch: Arc<Dispatch>) -> Result<()>
where
    R: BufRead + Send + 'static,
    W: Write + Send + 'static,
{
    let writer = Arc::new(Mutex::new(BufWriter::new(writer)));
    let budget = Arc::new(QueueBudget::default());
    let (queue, queued) = mpsc::channel::<QueuedWork>();
    let (stopped, first_stopped) = mpsc::channel();
    let worker = std::thread::Builder::new()
        .name("ig-mcp-worker".to_string())
        .spawn({
            let writer = writer.clone();
            let dispatch = dispatch.clone();
            let budget = budget.clone();
            let stop = StopSignal {
                thread: ServingThread::Worker,
                stopped: stopped.clone(),
                budget: budget.clone(),
            };
            move || -> Result<()> {
                let _stop = stop;
                for QueuedWork { work, mode, cost } in queued {
                    budget.release(cost);
                    if let Some(reply) = run_work(work, dispatch.as_ref()) {
                        write_message(&mut *writer.lock(), &reply, mode)?;
                    }
                }
                Ok(())
            }
        })
        .context("failed to start the MCP worker thread")?;
    let reader = std::thread::Builder::new()
        .name("ig-mcp-reader".to_string())
        .spawn({
            let budget = budget.clone();
            let stop = StopSignal {
                thread: ServingThread::Reader,
                stopped,
                budget: budget.clone(),
            };
            move || {
                let _stop = stop;
                read_requests(reader, &writer, dispatch.as_ref(), &budget, &queue)
            }
        })
        .context("failed to start the MCP reader thread")?;

    let worker = if first_stopped.recv() == Ok(ServingThread::Worker) {
        // A reply could not be written, or the worker panicked: end the
        // session without waiting for stdin. A reader still blocked on stdin
        // ends with the process. A worker only stops cleanly after the reader
        // closed the queue.
        join_serving_thread(worker)??;
        None
    } else {
        Some(worker)
    };
    match join_serving_thread(reader)? {
        ReadOutcome::OutputFailed(err) => Err(err),
        // EOF or malformed framing: requests already read still run.
        ReadOutcome::InputEnded(result) => result.and(worker.map_or(Ok(()), |worker| {
            join_serving_thread(worker).and_then(|result| result)
        })),
    }
}

/// Why the reader thread stopped.
enum ReadOutcome {
    /// EOF (`Ok`) or malformed framing (`Err`). Requests already read still run.
    InputEnded(Result<()>),
    /// A reply written by the reader could not be written.
    OutputFailed(anyhow::Error),
}

/// Read framed payloads until EOF, a framing error, or a failed write.
fn read_requests<R: BufRead, W: Write>(
    mut reader: R,
    writer: &Mutex<BufWriter<W>>,
    dispatch: &Dispatch,
    budget: &QueueBudget,
    queue: &mpsc::Sender<QueuedWork>,
) -> ReadOutcome {
    let pending = PendingRequests::default();
    let mut mode = FramingMode::Unknown;
    loop {
        let answer = match read_message(&mut reader, &mut mode) {
            Ok(Some(Frame::Payload(payload))) => match admit(&payload, &pending) {
                Admission::Answer(work) => work,
                Admission::Queue(work) => {
                    let cost = QueueCost::of(&work, payload.len());
                    // Both fail only after the worker stopped, and `serve`
                    // returns the worker's error.
                    if !budget.reserve(cost) || queue.send(QueuedWork { work, mode, cost }).is_err()
                    {
                        return ReadOutcome::InputEnded(Ok(()));
                    }
                    continue;
                }
                Admission::Ignore => continue,
            },
            Ok(Some(Frame::Oversized)) => Work::Single(Entry::Reply(error_response(
                Value::Null,
                -32700,
                format!("parse error: message exceeds maximum of {MAX_MESSAGE_BYTES} bytes"),
            ))),
            Ok(None) => return ReadOutcome::InputEnded(Ok(())),
            Err(err) => return ReadOutcome::InputEnded(Err(err)),
        };
        if let Some(reply) = run_work(answer, dispatch)
            && let Err(err) = write_message(&mut *writer.lock(), &reply, mode)
        {
            return ReadOutcome::OutputFailed(err);
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum ServingThread {
    Reader,
    Worker,
}

/// Tells `serve` that a serving thread stopped, however it stopped, and wakes
/// a reader waiting for queue room.
struct StopSignal {
    thread: ServingThread,
    stopped: mpsc::Sender<ServingThread>,
    budget: Arc<QueueBudget>,
}

impl Drop for StopSignal {
    fn drop(&mut self) {
        self.budget.close();
        let _ = self.stopped.send(self.thread);
    }
}

/// Join a serving thread, turning a panic into an error.
fn join_serving_thread<T>(thread: std::thread::JoinHandle<T>) -> Result<T> {
    let name = thread.thread().name().unwrap_or("MCP").to_string();
    thread
        .join()
        .map_err(|_| anyhow::anyhow!("{name} thread panicked"))
}

/// A payload waiting for the worker.
struct QueuedWork {
    work: Work,
    mode: FramingMode,
    cost: QueueCost,
}

/// What one queued payload counts against the queue limits.
#[derive(Clone, Copy)]
struct QueueCost {
    /// Batch members count one each.
    requests: usize,
    bytes: usize,
}

impl QueueCost {
    fn of(work: &Work, payload_bytes: usize) -> Self {
        let requests = match work {
            Work::Single(_) => 1,
            Work::Batch(entries) => entries.len(),
        };
        Self {
            requests: requests.max(1),
            bytes: payload_bytes,
        }
    }
}

/// Payloads queued for the worker and not yet taken.
#[derive(Default)]
struct QueuedLoad {
    requests: usize,
    bytes: usize,
}

impl QueuedLoad {
    /// Add `cost` if it fits. An empty queue takes one payload of any size, so
    /// a single maximal message or batch still runs.
    fn try_add(&mut self, cost: QueueCost) -> bool {
        let fits = self.requests == 0
            || (self.requests + cost.requests <= MAX_QUEUED_REQUESTS
                && self.bytes + cost.bytes <= MAX_QUEUED_BYTES);
        if fits {
            self.requests += cost.requests;
            self.bytes += cost.bytes;
        }
        fits
    }

    fn remove(&mut self, cost: QueueCost) {
        self.requests -= cost.requests;
        self.bytes -= cost.bytes;
    }
}

/// Bounds what waits for the worker. The reader reserves a payload's cost
/// before queuing it, and the worker releases it when it takes the payload.
#[derive(Default)]
struct QueueBudget {
    state: Mutex<BudgetState>,
    changed: Condvar,
}

#[derive(Default)]
struct BudgetState {
    load: QueuedLoad,
    closed: bool,
}

impl QueueBudget {
    /// Wait until `cost` fits, then reserve it. `false` once a serving thread
    /// stopped.
    fn reserve(&self, cost: QueueCost) -> bool {
        let mut state = self.state.lock();
        loop {
            if state.closed {
                return false;
            }
            if state.load.try_add(cost) {
                return true;
            }
            self.changed.wait(&mut state);
        }
    }

    fn release(&self, cost: QueueCost) {
        self.state.lock().load.remove(cost);
        self.changed.notify_one();
    }

    fn close(&self) {
        self.state.lock().closed = true;
        self.changed.notify_one();
    }
}

/// What one framed payload sends back.
#[derive(Debug, Serialize)]
#[serde(untagged)]
enum JsonRpcReply {
    Single(JsonRpcResponse),
    Batch(Vec<JsonRpcResponse>),
}

/// Where the reader thread sends one framed payload.
enum Admission {
    /// Run on the reader thread: `ping`, parse errors, and invalid requests.
    Answer(Work),
    /// Run on the worker thread, in arrival order.
    Queue(Work),
    /// Nothing to run or send: a notification or a client's response.
    Ignore,
}

/// One payload to run: a single message, or a batch answered with one array.
enum Work {
    Single(Entry),
    Batch(Vec<Entry>),
}

/// One decoded JSON-RPC message.
enum Entry {
    /// An error decided while decoding.
    Reply(JsonRpcResponse),
    Request(QueuedRequest),
    /// A notification or a client's response.
    Ignore,
}

struct QueuedRequest {
    request: JsonRpcRequest,
    /// `None` for `initialize`, which clients must not cancel.
    registration: Option<Registration>,
}

/// Decode one framed payload: a single message or a batch array, which MCP
/// 2025-03-26 requires servers to accept.
fn admit(payload: &[u8], pending: &PendingRequests) -> Admission {
    let message: Value = match serde_json::from_slice(payload) {
        Ok(message) => message,
        Err(err) => {
            return Admission::Answer(Work::Single(Entry::Reply(error_response(
                Value::Null,
                -32700,
                format!("parse error: {err}"),
            ))));
        }
    };
    match message {
        // An empty batch is one invalid request, answered with a single object.
        Value::Array(messages) if messages.is_empty() => {
            Admission::Answer(Work::Single(Entry::Reply(error_response(
                Value::Null,
                -32600,
                "invalid request: empty batch".to_string(),
            ))))
        }
        Value::Array(messages) => Admission::Queue(Work::Batch(
            messages
                .into_iter()
                .map(|message| admit_message(message, pending))
                .collect(),
        )),
        message => match admit_message(message, pending) {
            Entry::Ignore => Admission::Ignore,
            Entry::Request(queued) if queued.request.method != "ping" => {
                Admission::Queue(Work::Single(Entry::Request(queued)))
            }
            entry => Admission::Answer(Work::Single(entry)),
        },
    }
}

/// Decode one message. Requests are registered so a later
/// `notifications/cancelled` can reach them, and cancellations apply at once.
fn admit_message(message: Value, pending: &PendingRequests) -> Entry {
    let request = match parse_request(message) {
        Ok(Some(request)) => request,
        Ok(None) => return Entry::Ignore,
        Err(response) => return Entry::Reply(response),
    };
    let Some(id) = &request.id else {
        if request.method == "notifications/cancelled" {
            pending.cancel(&request.params);
        }
        return Entry::Ignore;
    };
    let registration = (request.method != "initialize").then(|| pending.register(id));
    Entry::Request(QueuedRequest {
        request,
        registration,
    })
}

/// Run one payload. `None` means nothing is sent back.
fn run_work(work: Work, dispatch: &Dispatch) -> Option<JsonRpcReply> {
    match work {
        Work::Single(entry) => run_entry(entry, dispatch).map(JsonRpcReply::Single),
        // Notifications, client responses, and cancelled requests get no
        // entry, and a batch with nothing to answer sends nothing instead of
        // an empty array.
        Work::Batch(entries) => {
            let responses = entries
                .into_iter()
                .filter_map(|entry| run_entry(entry, dispatch))
                .collect::<Vec<_>>();
            (!responses.is_empty()).then_some(JsonRpcReply::Batch(responses))
        }
    }
}

fn run_entry(entry: Entry, dispatch: &Dispatch) -> Option<JsonRpcResponse> {
    match entry {
        Entry::Reply(response) => Some(response),
        Entry::Ignore => None,
        Entry::Request(QueuedRequest {
            request,
            registration,
        }) => {
            let cancellation = registration
                .as_ref()
                .map(|registration| registration.cancellation.clone())
                .unwrap_or_default();
            // A request cancelled while queued never starts, and a cancelled
            // request gets no response.
            if cancellation.is_cancelled() {
                return None;
            }
            handle_request(request, dispatch, &cancellation)
                .filter(|_| !cancellation.is_cancelled())
        }
    }
}

/// Queued and running requests by JSON-RPC id, so a cancellation can reach
/// them. Entries leave when their request finishes, which bounds the map by
/// the queue.
#[derive(Clone, Default)]
struct PendingRequests(Arc<Mutex<HashMap<String, RequestCancellation>>>);

impl PendingRequests {
    fn register(&self, id: &Value) -> Registration {
        let key = id.to_string();
        let cancellation = RequestCancellation::default();
        // Ids must be unique per session; a reused id replaces the older entry.
        self.0.lock().insert(key.clone(), cancellation.clone());
        Registration {
            pending: self.clone(),
            key,
            cancellation,
        }
    }

    /// Apply `notifications/cancelled`. Unknown or finished ids and malformed
    /// params are ignored.
    fn cancel(&self, params: &Value) {
        let Some(id) = params
            .get("requestId")
            .filter(|id| id.is_string() || id.is_number())
        else {
            return;
        };
        let cancellation = self.0.lock().get(&id.to_string()).cloned();
        if let Some(cancellation) = cancellation {
            let reason = params
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("no reason given");
            tracing::debug!("MCP request {id} cancelled: {reason}");
            cancellation.cancel();
        }
    }
}

/// A request's entry in [`PendingRequests`], removed when dropped.
struct Registration {
    pending: PendingRequests,
    key: String,
    cancellation: RequestCancellation,
}

impl Drop for Registration {
    fn drop(&mut self) {
        let mut requests = self.pending.0.lock();
        if requests
            .get(&self.key)
            .is_some_and(|current| Arc::ptr_eq(&current.0, &self.cancellation.0))
        {
            requests.remove(&self.key);
        }
    }
}

/// Cancellation of one MCP request. Tripping it stops local searches that
/// poll the token, cancels the daemon search the request waits on, and ends
/// the first-index wait.
#[derive(Clone, Default)]
struct RequestCancellation(Arc<CancellationState>);

#[derive(Default)]
struct CancellationState {
    token: Arc<AtomicBool>,
    /// Id of the daemon search the request is waiting on.
    daemon_search: Mutex<Option<uuid::Uuid>>,
}

impl RequestCancellation {
    fn is_cancelled(&self) -> bool {
        self.0.token.load(Ordering::SeqCst)
    }

    /// Token for [`SearchOptions::cancel_token`].
    fn token(&self) -> Arc<AtomicBool> {
        self.0.token.clone()
    }

    fn cancel(&self) {
        let daemon_search = {
            let mut daemon_search = self.0.daemon_search.lock();
            self.0.token.store(true, Ordering::SeqCst);
            daemon_search.take()
        };
        if let Some(search_id) = daemon_search {
            // Off the reader thread: the daemon answers once the search stopped.
            let _ = std::thread::Builder::new()
                .name("ig-mcp-cancel".to_string())
                .spawn(move || {
                    let request = DaemonRequest::CancelSearch { search_id };
                    if let Err(err) = crate::daemon::request_blocking(&request, false) {
                        tracing::debug!("failed to cancel MCP daemon search: {err:#}");
                    }
                });
        }
    }

    /// Run a daemon search that cancelling this request also cancels on the
    /// daemon. `Ok(None)` when the request is already cancelled.
    fn daemon_search(&self, request: &DaemonRequest) -> Result<Option<DaemonResponse>> {
        let search_id = uuid::Uuid::new_v4();
        {
            let mut daemon_search = self.0.daemon_search.lock();
            if self.is_cancelled() {
                return Ok(None);
            }
            *daemon_search = Some(search_id);
        }
        let response = crate::daemon::request_blocking_with_id(request, Some(search_id), false);
        self.0.daemon_search.lock().take();
        response
    }
}

/// Decode one JSON-RPC 2.0 message. `Ok(None)` is a client's response to a
/// server request, which needs no answer; `Err` carries the error response.
fn parse_request(message: Value) -> std::result::Result<Option<JsonRpcRequest>, JsonRpcResponse> {
    let Value::Object(mut message) = message else {
        return Err(error_response(
            Value::Null,
            -32600,
            "invalid request: expected a JSON object".to_string(),
        ));
    };
    // Only a missing `id` member makes a notification; `"id": null` is a request.
    let id = message.remove("id");
    if id
        .as_ref()
        .is_some_and(|id| !(id.is_null() || id.is_string() || id.is_number()))
    {
        return Err(error_response(
            Value::Null,
            -32600,
            "invalid request: id must be a string, number, or null".to_string(),
        ));
    }
    match message.remove("method") {
        Some(Value::String(method)) => Ok(Some(JsonRpcRequest {
            id,
            method,
            params: message.remove("params").unwrap_or(Value::Null),
        })),
        None if id.is_some()
            && (message.contains_key("result") || message.contains_key("error")) =>
        {
            Ok(None)
        }
        _ => Err(error_response(
            id.unwrap_or(Value::Null),
            -32600,
            "invalid request: method must be a string".to_string(),
        )),
    }
}

fn error_response(id: Value, code: i64, message: String) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: JSONRPC_VERSION,
        id,
        result: None,
        error: Some(JsonRpcError { code, message }),
    }
}

fn handle_request(
    request: JsonRpcRequest,
    dispatch: &Dispatch,
    cancellation: &RequestCancellation,
) -> Option<JsonRpcResponse> {
    let id = request.id?;
    let method = request.method;
    let params = request.params;

    // Isolate handler panics: a panic deep in search must not crash the
    // whole MCP session. Capture it and return a JSON-RPC error instead.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        dispatch(method.as_str(), params, cancellation)
    })) {
        Ok(Ok(result)) => Some(JsonRpcResponse {
            jsonrpc: JSONRPC_VERSION,
            id,
            result: Some(result),
            error: None,
        }),
        Ok(Err(err)) => Some(error_response(id, err.code, err.message)),
        Err(_) => Some(error_response(
            id,
            -32603,
            "internal error: request handler panicked".to_string(),
        )),
    }
}

fn dispatch(
    method: &str,
    params: Value,
    cancellation: &RequestCancellation,
) -> std::result::Result<Value, DispatchError> {
    match method {
        "initialize" => Ok(initialize_result(&params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": [search_tool_schema(), status_tool_schema()]})),
        "tools/call" => run_tool_call(params, cancellation).map_err(DispatchError::invalid_params),
        "notifications/initialized" => Ok(json!({})),
        "shutdown" => Ok(json!({})),
        other => Err(DispatchError::method_not_found(other)),
    }
}

fn initialize_result(params: &Value) -> Value {
    let requested = params.get("protocolVersion").and_then(Value::as_str);
    let protocol_version = match requested {
        Some(version) if SUPPORTED_PROTOCOL_VERSIONS.contains(&version) => version,
        Some(_) => LATEST_PROTOCOL_VERSION,
        None => LEGACY_PROTOCOL_VERSION,
    };

    json!({
        "protocolVersion": protocol_version,
        "capabilities": {
            "tools": {
                "listChanged": false
            }
        },
        "serverInfo": {
            "name": "ig",
            "title": "ivygrep",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "Local hybrid semantic and lexical code search",
            "websiteUrl": env!("CARGO_PKG_HOMEPAGE")
        },
        "instructions": "Use ig_search with an absolute path to the active workspace so searches stay scoped to the intended repository. For implementation tasks, request output=context_pack with budget_tokens=8000 to receive one bounded pack containing primary code, dependencies, dependents, definitions, callers, references, tests, configuration, documentation, and recent co-change evidence. For iterative discovery, keep output=hits, use natural-language queries for concepts, and literal=true for exact identifiers. Hits mode returns at most limit files (default 10) with at most hits_per_file hits each (default 3); check truncated, total_matches, and more_hits_in_file, then narrow the query, scope path, or raise limit instead of re-running broad queries. Start with limit=5-10 and context=2. Use ig_status when indexing health is unclear. If ig_search returns status=indexing (not an error), the first index is still building in the background: wait retry_after_secs, then call again; do not retry immediately or fall back to scanning the filesystem. Workspaces are indexed on first use and watched for incremental updates."
    })
}

fn search_tool_schema() -> Value {
    json!({
        "name": TOOL_IG_SEARCH,
        "title": "Search local code or build a task context pack",
        "description": "Hybrid semantic+lexical code search and token-budgeted task context. Auto-indexes on first query, stays local, respects .gitignore, and restricts results to the provided path scope. Use output=context_pack for implementation tasks; keep output=hits for iterative discovery. On a large repository the first call may return status=indexing with progress instead of results: the index keeps building in the background, so wait retry_after_secs and call again rather than retrying immediately.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Natural-language or keyword query. Uppercase AND, OR, and NOT between terms are Boolean operators (NOT excludes matches); write them in lowercase or wrap them in backticks to search them as words."},
                "path": {"type": "string", "description": "Workspace path, subdirectory, or file path. Defaults to current directory."},
                "output": {
                    "type": "string",
                    "enum": ["hits", "context_pack"],
                    "default": "hits",
                    "description": "Return ranked search hits or one task-ready, relationship-expanded context pack. context_pack keeps the same single MCP tool call."
                },
                "budget_tokens": {
                    "type": "integer",
                    "minimum": 256,
                    "maximum": 131072,
                    "description": "Complete context-pack budget, including metadata and snippets. Valid only with output=context_pack."
                },
                "since": {
                    "type": "string",
                    "description": "Git ref for a diff-aware context pack. Includes merge-base changes plus staged, unstaged, and untracked files. Valid only with output=context_pack."
                },
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 1000,
                    "description": "Maximum number of ranked result files in hits mode; defaults to 10 when omitted. Retrieval depth scales with it, so larger values may improve recall while adding lower-ranked files. Not a token, line, hit, or confidence limit; each file is further capped by hits_per_file. Ignored for output=context_pack, which is bounded by budget_tokens."
                },
                "hits_per_file": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 100,
                    "default": 3,
                    "description": "Maximum hits returned per result file in hits mode. Files with more matches report more_hits_in_file. Raise it, or scope path to one file, to see every match in a file."
                },
                "context": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 100,
                    "default": 2,
                    "description": "Lines before and after each focused match. Changes snippet size, not retrieval ranking."
                },
                "type": {"type": "string", "description": "Language filter - accepts names (rust, python), extensions (rs, py, md), or aliases (c++, bash, js)."},
                "regex": {"type": "boolean", "description": "Use regex mode (index-prefiltered when possible; otherwise walks raw files). Prefer 'literal' for exact matches."},
                "literal": {"type": "boolean", "description": "Fast exact-match search backed by the index. Deterministic results."},
                "symbol": {"type": "boolean", "description": "Find exact symbol definitions."},
                "refs": {"type": "boolean", "description": "Find exact references to the named symbol."},
                "callers": {"type": "boolean", "description": "Find functions or methods that call the named symbol."},
                "include": {"type": "string", "description": "Comma-separated include globs, e.g. \"*.md,src/**/*.rs\"."},
                "exclude": {"type": "string", "description": "Comma-separated exclude globs, e.g. \"target/**,*.lock\"."},
                "first_line_only": {"type": "boolean", "description": "Return only the first non-empty preview line for each hit. Ranking is unchanged."},
                "file_name_only": {"type": "boolean", "description": "Return only file paths (no hit details). Ranking is unchanged."},
                "verbose": {"type": "boolean", "description": "Include reason pointers in JSON output."},
                "skip_gitignore": {"type": "boolean", "description": "Include files ignored by .gitignore."}
            },
            "required": ["query"],
            "additionalProperties": false
        },
        "outputSchema": search_output_schema(),
        "annotations": {
            "readOnlyHint": false,
            "destructiveHint": false,
            "idempotentHint": true,
            "openWorldHint": false
        }
    })
}

fn status_tool_schema() -> Value {
    json!({
        "name": TOOL_IG_STATUS,
        "title": "Inspect ivygrep indexes",
        "description": "Returns the list of indexed projects (workspaces) and their current indexing status, detailing if they are ready to query.",
        "inputSchema": {
            "type": "object",
            "properties": {},
            "required": [],
            "additionalProperties": false
        },
        "outputSchema": status_output_schema(),
        "annotations": {
            "readOnlyHint": true,
            "destructiveHint": false,
            "idempotentHint": true,
            "openWorldHint": false
        }
    })
}

fn search_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "workspace_root": {"type": "string"},
            "scope_path": {"type": ["string", "null"]},
            "scope_is_file": {"type": "boolean"},
            "query": {"type": "string"},
            "mode": {"type": "string", "enum": ["hybrid", "literal", "regex", "symbol", "references", "callers", "context"]},
            "result_count": {"type": "integer", "minimum": 0},
            "total_matches": {
                "type": "integer",
                "minimum": 0,
                "description": "Files matched before limit was applied (hits mode). A lower bound when retrieval hit its candidate budget."
            },
            "truncated": {
                "type": "boolean",
                "description": "True when matched files were cut to limit or retrieval hit its candidate budget; narrow the query or raise limit to see more."
            },
            "include": {"type": "array", "items": {"type": "string"}},
            "exclude": {"type": "array", "items": {"type": "string"}},
            "warnings": {"type": "array", "items": {"type": "string"}},
            "results": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "file_path": {"type": "string"},
                        "total_score": {"type": "number"},
                        "hit_count": {"type": "integer", "minimum": 0, "description": "Hits matched in this file before hits_per_file was applied."},
                        "more_hits_in_file": {"type": "integer", "minimum": 1, "description": "Hits omitted from this file by hits_per_file among the retrieved matches. Absent when nothing was cut."},
                        "hit_count_is_lower_bound": {"type": "boolean", "description": "True when retrieval stopped at its candidate budget, so hit_count and more_hits_in_file are lower bounds for this file. Absent when counts are exact."},
                        "hits": {"type": "array", "items": {"type": "object"}}
                    },
                    "required": ["file_path", "total_score", "hit_count", "hits"]
                }
            },
            "file_paths": {"type": "array", "items": {"type": "string"}},
            "context_pack": context_pack_output_schema(),
            "status": {
                "type": "string",
                "enum": ["indexing"],
                "description": "Present only when the workspace's first index is still running; no results are returned. Retry after retry_after_secs."
            },
            "progress": {
                "type": "object",
                "properties": {
                    "phase": {"type": "string"},
                    "done": {"type": ["integer", "null"], "minimum": 0},
                    "total": {"type": ["integer", "null"], "minimum": 0},
                    "percent": {"type": ["number", "null"], "minimum": 0, "maximum": 100}
                },
                "required": ["phase", "done", "total", "percent"],
                "additionalProperties": false
            },
            "elapsed_secs": {"type": "integer", "minimum": 0},
            "retry_after_secs": {"type": "integer", "minimum": 1},
            "message": {"type": "string"}
        },
        "oneOf": [
            {
                "required": [
                    "workspace_root",
                    "scope_path",
                    "scope_is_file",
                    "query",
                    "mode",
                    "result_count",
                    "include",
                    "exclude"
                ],
                "oneOf": [
                    {"required": ["results"]},
                    {"required": ["file_paths"]},
                    {"required": ["context_pack"]}
                ]
            },
            {
                "required": [
                    "status",
                    "workspace_root",
                    "progress",
                    "elapsed_secs",
                    "retry_after_secs",
                    "message"
                ]
            }
        ],
        "additionalProperties": false
    })
}

fn context_pack_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "task": {"type": "string"},
            "workspace": {"type": "string"},
            "change_scope": {
                "type": "object",
                "properties": {
                    "since": {"type": "string"},
                    "base_commit": {"type": "string"},
                    "dirty_worktree": {"type": "boolean"},
                    "total_changes": {"type": "integer", "minimum": 0},
                    "changes_truncated": {"type": "boolean"},
                    "changes": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "file_path": {"type": "string"},
                                "old_path": {"type": "string"},
                                "status": {
                                    "type": "string",
                                    "enum": ["added", "modified", "deleted", "renamed", "copied", "type_changed", "unmerged", "unknown"]
                                },
                                "sources": {
                                    "type": "array",
                                    "items": {"type": "string", "enum": ["since", "staged", "worktree", "untracked"]}
                                }
                            },
                            "required": ["file_path", "status", "sources"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["dirty_worktree", "total_changes", "changes_truncated", "changes"],
                "additionalProperties": false
            },
            "referenced_paths": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "file_path": {"type": "string"},
                        "line": {"type": "integer", "minimum": 1}
                    },
                    "required": ["file_path"],
                    "additionalProperties": false
                }
            },
            "budget_tokens": {"type": "integer", "minimum": 256, "maximum": 131072},
            "used_tokens": {"type": "integer", "minimum": 0},
            "candidate_count": {"type": "integer", "minimum": 0},
            "truncated": {"type": "boolean"},
            "anchor_symbols": {"type": "array", "items": {"type": "string"}},
            "coverage": {
                "type": "object",
                "properties": {
                    "files": {"type": "integer", "minimum": 0},
                    "primary": {"type": "integer", "minimum": 0},
                    "definitions": {"type": "integer", "minimum": 0},
                    "dependencies": {"type": "integer", "minimum": 0},
                    "dependents": {"type": "integer", "minimum": 0},
                    "callers": {"type": "integer", "minimum": 0},
                    "references": {"type": "integer", "minimum": 0},
                    "tests": {"type": "integer", "minimum": 0},
                    "config": {"type": "integer", "minimum": 0},
                    "documentation": {"type": "integer", "minimum": 0}
                },
                "required": [
                    "files", "primary", "definitions", "dependencies", "dependents",
                    "callers", "references", "tests", "config", "documentation"
                ],
                "additionalProperties": false
            },
            "items": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "file_path": {"type": "string"},
                        "start_line": {"type": "integer", "minimum": 1},
                        "end_line": {"type": "integer", "minimum": 1},
                        "roles": {
                            "type": "array",
                            "items": {
                                "type": "string",
                                "enum": [
                                    "primary", "definition", "dependency", "dependent",
                                    "caller", "reference", "test", "config",
                                    "documentation", "related"
                                ]
                            }
                        },
                        "reasons": {"type": "array", "items": {"type": "string"}},
                        "sources": {"type": "array", "items": {"type": "string"}},
                        "preview": {"type": "string"},
                        "estimated_tokens": {"type": "integer", "minimum": 0}
                    },
                    "required": [
                        "file_path", "start_line", "end_line", "roles", "reasons",
                        "sources", "preview", "estimated_tokens"
                    ],
                    "additionalProperties": false
                }
            }
        },
        "required": [
            "task", "workspace", "referenced_paths", "budget_tokens", "used_tokens", "candidate_count",
            "truncated", "anchor_symbols", "coverage", "items"
        ],
        "additionalProperties": false
    })
}

fn status_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "workspaces": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "workspace_root": {"type": "string"},
                        "ready_to_query": {"type": "boolean"},
                        "status": {"type": "string"},
                        "chunk_count": {"type": "integer", "minimum": 0},
                        "file_count": {"type": "integer", "minimum": 0},
                        "indexing_in_progress": {"type": "boolean"},
                        "enhancing_in_progress": {"type": "boolean"},
                        "watch_enabled": {"type": "boolean"},
                        "watcher_alive": {"type": "boolean"},
                        "indexing_stalled": {"type": "boolean"},
                        "enhancing_stalled": {"type": "boolean"}
                    },
                    "required": [
                        "workspace_root",
                        "ready_to_query",
                        "status",
                        "chunk_count",
                        "file_count",
                        "indexing_in_progress",
                        "enhancing_in_progress",
                        "watch_enabled",
                        "watcher_alive",
                        "indexing_stalled",
                        "enhancing_stalled"
                    ],
                    "additionalProperties": false
                }
            }
        },
        "required": ["workspaces"],
        "additionalProperties": false
    })
}

fn run_tool_call(params: Value, cancellation: &RequestCancellation) -> Result<Value> {
    let call: ToolCallParams = serde_json::from_value(params)?;
    if call.name == TOOL_IG_SEARCH {
        let result = serde_json::from_value(call.arguments)
            .map_err(anyhow::Error::from)
            .and_then(|args| execute_ivygrep_search(args, cancellation));
        Ok(result.unwrap_or_else(tool_error_result))
    } else if call.name == TOOL_IG_STATUS {
        let arguments = if call.arguments.is_null() {
            json!({})
        } else {
            call.arguments
        };
        let result = serde_json::from_value::<IvygrepStatusArgs>(arguments)
            .map_err(anyhow::Error::from)
            .and_then(|_| execute_ivygrep_status());
        Ok(result.unwrap_or_else(tool_error_result))
    } else {
        bail!("unknown tool: {}", call.name);
    }
}

fn tool_error_result(error: anyhow::Error) -> Value {
    json!({
        "content": [
            {
                "type": "text",
                "text": error.to_string()
            }
        ],
        "isError": true
    })
}

fn tool_success_result(payload: Value, pretty: bool) -> Result<Value> {
    let text = if pretty {
        serde_json::to_string_pretty(&payload)?
    } else {
        serde_json::to_string(&payload)?
    };
    Ok(tool_success_result_with_text(payload, text))
}

/// Build a tool result whose `structuredContent` is the machine-readable
/// payload and whose text block is a separate rendering. Emitting the same JSON
/// in both places doubled every search payload on the wire.
fn tool_success_result_with_text(payload: Value, text: String) -> Value {
    json!({
        "content": [
            {
                "type": "text",
                "text": text
            }
        ],
        "structuredContent": payload,
        "isError": false
    })
}

fn execute_ivygrep_status() -> Result<Value> {
    let workspaces = crate::workspace::list_workspaces()?;

    let mut projects = Vec::new();
    for ws in workspaces {
        let ready_to_query = ws.chunk_count > 0 && ws.last_indexed_at_unix.is_some();
        // A run accepted by the daemon but still parked behind the workspace
        // lease or CPU permits has no job heartbeat yet; ask the daemon so the
        // workspace is not reported as idle while its first index is queued.
        let indexing_in_progress =
            ws.indexing_in_progress || (!ready_to_query && daemon_index_in_flight(&ws.root));
        let status_msg = if ready_to_query {
            if ws.enhancing_in_progress {
                "Ready to query (Background enhancement in progress)"
            } else if ws.enhancing_stalled {
                "Ready to query (Background enhancement stalled)"
            } else if !ws.has_neural_vectors {
                "Ready to query (Lexical only)"
            } else {
                "Ready to query"
            }
        } else if indexing_in_progress {
            "Indexing in progress (Not ready)"
        } else if ws.indexing_stalled {
            "Indexing stalled (Needs attention)"
        } else {
            "Not indexed"
        };

        projects.push(json!({
            "workspace_root": ws.root,
            "ready_to_query": ready_to_query,
            "status": status_msg,
            "chunk_count": ws.chunk_count,
            "file_count": ws.file_count,
            "indexing_in_progress": indexing_in_progress,
            "enhancing_in_progress": ws.enhancing_in_progress,
            "watch_enabled": ws.watch_enabled,
            "watcher_alive": ws.watcher_alive,
            "indexing_stalled": ws.indexing_stalled,
            "enhancing_stalled": ws.enhancing_stalled,
        }));
    }

    let payload = json!({
        "workspaces": projects
    });

    tool_success_result(payload, true)
}

/// Whether the daemon has an explicit index run queued or running for `root`.
/// Never spawns a daemon; an absent daemon means nothing is in flight.
fn daemon_index_in_flight(root: &Path) -> bool {
    if !crate::ipc::socket_exists() {
        return false;
    }
    let request = DaemonRequest::RuntimeStatus {
        path: Some(root.to_path_buf()),
    };
    matches!(
        crate::daemon::request_blocking(&request, false),
        Ok(Some(DaemonResponse::RuntimeStatus {
            workspace: Some(status),
            ..
        })) if status.index_in_flight
    )
}

/// The neural query model for this MCP process, loaded once and reused across
/// requests.
///
/// `serve_stdio` is a long-lived server, so reconstructing the Candle model
/// per request reloads the weights every search — hundreds of ms of avoidable
/// latency and memory churn. Cache it here, mirroring the daemon's
/// `DaemonState.lazy_model` / `cached_hash_model()`.
///
/// Only a *successfully initialized neural* model is cached. If neural init
/// fails (transient model download/load error, or the `neural` feature is not
/// compiled in) we return a fresh hash model for this call and leave the cache
/// empty so the next request retries neural — otherwise a single startup
/// failure would silently pin every future search to hash embeddings until the
/// process restarts. Hash-model construction is cheap (no I/O), so retrying it
/// per call costs nothing meaningful.
fn mcp_query_model() -> Arc<dyn EmbeddingModel> {
    static MODEL: OnceLock<Arc<dyn EmbeddingModel>> = OnceLock::new();
    if let Some(model) = MODEL.get() {
        return model.clone();
    }
    match create_neural_model() {
        Ok(model) => {
            let model: Arc<dyn EmbeddingModel> = Arc::from(model);
            // First successful neural init wins; MCP requests run one at a
            // time on the worker thread, so a lost race here is not a concern.
            let _ = MODEL.set(model.clone());
            model
        }
        Err(_) => Arc::from(create_hash_model()),
    }
}

fn mcp_search_model(workspace: &Workspace) -> Arc<dyn EmbeddingModel> {
    if workspace.has_neural_vectors() {
        mcp_query_model()
    } else {
        Arc::from(create_hash_model())
    }
}

/// Outcome of preparing a workspace for an MCP search.
enum WorkspaceReadiness {
    Ready,
    /// The daemon is still building the index; carries the structured
    /// `status: indexing` payload for the tool result.
    Indexing(Value),
}

const MCP_INDEX_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);
const DEFAULT_INDEX_RETRY_AFTER_SECS: u64 = 10;

/// Re-index when the caller wants ignored files but the index excludes them.
fn needs_ignored_refresh(workspace: &Workspace, include_ignored: bool) -> Result<bool> {
    let metadata = workspace.read_metadata()?;
    Ok(include_ignored
        && !metadata
            .as_ref()
            .is_some_and(|metadata| metadata.skip_gitignore))
}

fn ensure_mcp_workspace_ready(
    workspace: &Workspace,
    include_ignored: bool,
    cancellation: &RequestCancellation,
) -> Result<WorkspaceReadiness> {
    let metadata = workspace.read_metadata()?;
    let include_ignored = include_ignored
        || metadata
            .as_ref()
            .is_some_and(|metadata| metadata.skip_gitignore);

    if workspace_is_indexed(workspace)
        && workspace.is_watcher_alive()
        && !needs_ignored_refresh(workspace, include_ignored)?
    {
        return Ok(WorkspaceReadiness::Ready);
    }

    // Enqueue on the daemon and wait a bounded time. A first index of a large
    // repository can take minutes; MCP clients time out tool calls long before
    // that, so never block the call on the whole run.
    let start_request = DaemonRequest::StartIndex {
        path: workspace.root.clone(),
        watch: true,
        skip_gitignore: include_ignored,
    };
    let wait_started = std::time::Instant::now();
    match crate::daemon::request_blocking(&start_request, true)? {
        Some(DaemonResponse::IndexStarted { .. }) => {
            return wait_for_daemon_index(workspace, include_ignored, wait_started, cancellation);
        }
        Some(DaemonResponse::Error { message }) => {
            // The daemon is up and rejected the request; do not duplicate its
            // work locally. An already queryable index still serves searches,
            // unless the request needs ignored files the index never saw.
            if workspace_is_indexed(workspace)
                && !needs_ignored_refresh(workspace, include_ignored)?
            {
                tracing::warn!(
                    "MCP daemon index request rejected; searching existing index: {message}"
                );
                return Ok(WorkspaceReadiness::Ready);
            }
            bail!(
                "ivygrep daemon rejected index request for {}: {message}",
                workspace.root.display()
            );
        }
        Some(response) => {
            tracing::warn!("unexpected MCP daemon indexing response: {response:?}");
        }
        None => {}
    }

    // No response. Either the daemon is unreachable (no socket, autospawn
    // disabled, transport failure before the request was sent) or the request
    // was accepted but the reply was lost (timeout, dropped connection). Only
    // the first case may index in-process: if a daemon still answers, its
    // detached run may already be active and a local run would duplicate it.
    if crate::daemon::request_blocking(&DaemonRequest::Version, false)?.is_some() {
        tracing::warn!(
            "MCP index request for {} got no reply from a live daemon; polling its status instead of indexing locally",
            workspace.root.display()
        );
        return wait_for_daemon_index(workspace, include_ignored, wait_started, cancellation);
    }
    index_workspace_locally(workspace, include_ignored)?;
    Ok(WorkspaceReadiness::Ready)
}

/// Poll the daemon until its run for `workspace` clears or the bounded wait
/// (`IVYGREP_MCP_INDEX_WAIT_SECS`) elapses. Never indexes locally: the daemon
/// owns the run, and a second `StartIndex` joins it instead of duplicating it.
fn wait_for_daemon_index(
    workspace: &Workspace,
    include_ignored: bool,
    wait_started: std::time::Instant,
    cancellation: &RequestCancellation,
) -> Result<WorkspaceReadiness> {
    let deadline = wait_started + config::mcp_index_wait();
    let status_request = DaemonRequest::RuntimeStatus {
        path: Some(workspace.root.clone()),
    };
    let mut resubmitted = false;
    loop {
        // Only the wait stops; the daemon keeps indexing for the next call.
        if cancellation.is_cancelled() {
            bail!("request cancelled");
        }
        let in_flight = match crate::daemon::request_blocking(&status_request, false)? {
            Some(DaemonResponse::RuntimeStatus {
                workspace: Some(status),
                ..
            }) => status.index_in_flight,
            Some(DaemonResponse::RuntimeStatus { .. }) => false,
            Some(DaemonResponse::Error { message }) => {
                bail!(
                    "ivygrep daemon status failed for {}: {message}",
                    workspace.root.display()
                )
            }
            Some(response) => {
                tracing::warn!("unexpected MCP daemon status response: {response:?}");
                false
            }
            None => {
                // A queryable index only counts if it already covers what the
                // request asked for; a gitignore-respecting index cannot serve
                // a skip_gitignore search.
                if workspace_is_indexed(workspace)
                    && !needs_ignored_refresh(workspace, include_ignored)?
                {
                    return Ok(WorkspaceReadiness::Ready);
                }
                bail!(
                    "ivygrep daemon became unavailable while indexing {}; call again to resume",
                    workspace.root.display()
                );
            }
        };

        if !in_flight {
            if workspace_is_indexed(workspace)
                && !needs_ignored_refresh(workspace, include_ignored)?
            {
                return Ok(WorkspaceReadiness::Ready);
            }
            let failure = crate::jobs::job_status(
                workspace,
                crate::jobs::JobKind::Indexing,
                crate::jobs::INDEXING_HEARTBEAT_TTL_SECS,
            )
            .record
            .filter(|record| !record.active)
            .and_then(|record| record.last_error);
            if let Some(error) = failure {
                bail!(
                    "ivygrep daemon index failed for {}: {error}",
                    workspace.root.display()
                );
            }
            if resubmitted {
                bail!(
                    "ivygrep daemon index for {} finished without a queryable index; call again",
                    workspace.root.display()
                );
            }
            // The run that was in flight had different options (for example a
            // CLI `--no-watch` index) and did not satisfy this request; queue
            // ours now that it can lead.
            resubmitted = true;
            let start_request = DaemonRequest::StartIndex {
                path: workspace.root.clone(),
                watch: true,
                skip_gitignore: include_ignored,
            };
            match crate::daemon::request_blocking(&start_request, false)? {
                Some(DaemonResponse::IndexStarted { .. }) => {}
                Some(DaemonResponse::Error { message }) => {
                    bail!(
                        "ivygrep daemon rejected index request for {}: {message}",
                        workspace.root.display()
                    )
                }
                _ => bail!(
                    "ivygrep daemon became unavailable while indexing {}; call again to resume",
                    workspace.root.display()
                ),
            }
        }

        if std::time::Instant::now() >= deadline {
            return Ok(WorkspaceReadiness::Indexing(indexing_status_payload(
                workspace,
                wait_started.elapsed(),
            )));
        }
        std::thread::sleep(
            MCP_INDEX_POLL_INTERVAL
                .min(deadline.saturating_duration_since(std::time::Instant::now())),
        );
    }
}

/// In-process index used only when no daemon is reachable.
fn index_workspace_locally(workspace: &Workspace, include_ignored: bool) -> Result<()> {
    workspace.ensure_dirs()?;
    let mut metadata = workspace
        .read_metadata()?
        .unwrap_or_else(|| WorkspaceMetadata {
            id: workspace.id.clone(),
            root: workspace.root.clone(),
            created_at_unix: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            last_indexed_at_unix: None,
            watch_enabled: true,
            skip_gitignore: include_ignored,
            index_generation: 0,
        });
    metadata.watch_enabled = true;
    if include_ignored {
        metadata.skip_gitignore = true;
    }
    workspace.write_metadata(&metadata)?;

    let index_model = create_hash_model();
    index_workspace(workspace, index_model.as_ref())?;
    Ok(())
}

/// Progress of the daemon's index run, read from the same job ledger and
/// progress file that `ig --status` and the CLI first-run spinner use.
fn indexing_status_payload(workspace: &Workspace, waited: std::time::Duration) -> Value {
    let job = crate::jobs::job_status(
        workspace,
        crate::jobs::JobKind::Indexing,
        crate::jobs::INDEXING_HEARTBEAT_TTL_SECS,
    );
    let active_record = job.record.as_ref().filter(|_| job.active());
    let raw_progress = std::fs::read_to_string(workspace.indexing_progress_path())
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| active_record.and_then(|record| record.details.get("progress").cloned()))
        .or_else(|| {
            active_record
                .map(|record| record.phase.clone())
                .filter(|phase| !phase.is_empty())
        });
    let (phase, done, total) = match raw_progress.as_deref() {
        Some(progress) => match parse_file_progress(progress) {
            Some((done, total)) => ("indexing".to_string(), Some(done), Some(total)),
            None => (progress.to_string(), None, None),
        },
        None => ("queued".to_string(), None, None),
    };
    let now = crate::jobs::now_unix();
    let elapsed_secs = active_record
        .and_then(|record| record.started_at_unix)
        .map(|started| now.saturating_sub(started))
        .map_or(waited.as_secs(), |since_start| {
            since_start.max(waited.as_secs())
        });
    let percent = match (done, total) {
        (Some(done), Some(total)) if total > 0 => {
            Some(((done as f64 / total as f64) * 1000.0).round() / 10.0)
        }
        _ => None,
    };
    let retry_after_secs = estimate_retry_after_secs(done, total, elapsed_secs);
    json!({
        "status": "indexing",
        "workspace_root": workspace.root,
        "progress": {
            "phase": phase,
            "done": done,
            "total": total,
            "percent": percent,
        },
        "elapsed_secs": elapsed_secs,
        "retry_after_secs": retry_after_secs,
        "message": "Index in progress; call again later. Lexical search becomes available when the first index commits.",
    })
}

/// Parse the indexer's `done/total` progress string.
fn parse_file_progress(progress: &str) -> Option<(u64, u64)> {
    let (done, total) = progress.split_once('/')?;
    Some((done.trim().parse().ok()?, total.trim().parse().ok()?))
}

/// Remaining-time estimate from observed throughput, clamped to 5-60 s;
/// falls back to a fixed delay when no throughput is known yet.
fn estimate_retry_after_secs(done: Option<u64>, total: Option<u64>, elapsed_secs: u64) -> u64 {
    match (done, total) {
        (Some(done), Some(total)) if done > 0 && total > done && elapsed_secs > 0 => {
            let remaining = (total - done) as f64 * elapsed_secs as f64 / done as f64;
            (remaining.ceil() as u64).clamp(5, 60)
        }
        _ => DEFAULT_INDEX_RETRY_AFTER_SECS,
    }
}

/// Non-error tool result for a workspace whose first index is still running.
/// `content[0].text` is human-readable (followed by the JSON payload) so agents
/// without `structuredContent` support still see what to do.
fn indexing_tool_result(payload: Value) -> Result<Value> {
    let progress = &payload["progress"];
    let phase = progress["phase"].as_str().unwrap_or("indexing");
    let counts = match (progress["done"].as_u64(), progress["total"].as_u64()) {
        (Some(done), Some(total)) => {
            let percent = progress["percent"].as_f64().unwrap_or(0.0);
            format!(" {done}/{total} files ({percent:.1}%)")
        }
        _ => String::new(),
    };
    let text = format!(
        "Indexing {}: {phase}{counts}, {}s elapsed. Not ready yet; call ig_search again in ~{}s. Lexical search becomes available when the first index commits.\n{}",
        payload["workspace_root"].as_str().unwrap_or("workspace"),
        payload["elapsed_secs"].as_u64().unwrap_or(0),
        payload["retry_after_secs"]
            .as_u64()
            .unwrap_or(DEFAULT_INDEX_RETRY_AFTER_SECS),
        serde_json::to_string(&payload)?
    );
    Ok(json!({
        "content": [
            {
                "type": "text",
                "text": text
            }
        ],
        "structuredContent": payload,
        "isError": false
    }))
}

fn execute_ivygrep_search(
    args: IvygrepSearchArgs,
    cancellation: &RequestCancellation,
) -> Result<Value> {
    let query = args
        .query
        .as_deref()
        .context("missing required argument: query")?;
    if query.trim().is_empty() {
        bail!("query must not be empty");
    }
    let output = args.output.as_deref().unwrap_or("hits");
    if !matches!(output, "hits" | "context_pack") {
        bail!("output must be hits or context_pack");
    }
    let wants_context_pack = output == "context_pack";
    let requested_modes = [
        args.literal.unwrap_or(false),
        args.regex.unwrap_or(false),
        args.symbol.unwrap_or(false),
        args.refs.unwrap_or(false),
        args.callers.unwrap_or(false),
    ];
    if requested_modes
        .into_iter()
        .filter(|enabled| *enabled)
        .count()
        > 1
    {
        bail!("literal, regex, symbol, refs, and callers modes are mutually exclusive");
    }
    if args.limit == Some(0) || args.limit.is_some_and(|limit| limit > 1000) {
        bail!("limit must be between 1 and 1000");
    }
    if args
        .hits_per_file
        .is_some_and(|hits| !(1..=MAX_HITS_PER_FILE).contains(&hits))
    {
        bail!("hits_per_file must be between 1 and {MAX_HITS_PER_FILE}");
    }
    if args.context.is_some_and(|context| context > 100) {
        bail!("context must be between 0 and 100");
    }
    if args
        .budget_tokens
        .is_some_and(|budget| !(256..=131_072).contains(&budget))
    {
        bail!("budget_tokens must be between 256 and 131072");
    }
    if !wants_context_pack && args.budget_tokens.is_some() {
        bail!("budget_tokens requires output=context_pack");
    }
    if !wants_context_pack && args.since.is_some() {
        bail!("since requires output=context_pack");
    }
    if wants_context_pack && args.hits_per_file.is_some() {
        bail!("hits_per_file requires output=hits");
    }
    if wants_context_pack
        && (requested_modes.into_iter().any(|enabled| enabled)
            || args.first_line_only.unwrap_or(false)
            || args.file_name_only.unwrap_or(false))
    {
        bail!(
            "output=context_pack cannot be combined with literal, regex, symbol, refs, callers, first_line_only, or file_name_only"
        );
    }

    let input_path = match args.path {
        Some(path) => PathBuf::from(path),
        None => env::current_dir()?,
    };

    let (current_workspace, scope_filter) = resolve_workspace_and_scope(Path::new(&input_path))?;

    // MCP search is intentionally scoped to one workspace. Ensure that
    // workspace is indexed and watched before searching so edits made by a
    // coding agent become searchable without restarting the MCP process.
    if let WorkspaceReadiness::Indexing(payload) = ensure_mcp_workspace_ready(
        &current_workspace,
        args.skip_gitignore.unwrap_or(false),
        cancellation,
    )? {
        return indexing_tool_result(payload);
    }
    let workspace = current_workspace.clone();
    let _ = workspace.cleanup_stale_legacy_runtime_files();

    let literal = args.literal.unwrap_or(false);
    let regex = args.regex.unwrap_or(false);
    let symbol_mode = if args.symbol.unwrap_or(false) {
        Some(SymbolSearchMode::Definitions)
    } else if args.refs.unwrap_or(false) {
        Some(SymbolSearchMode::References)
    } else if args.callers.unwrap_or(false) {
        Some(SymbolSearchMode::Callers)
    } else {
        None
    };
    // `limit` counts result files. Retrieval APIs count hits, so over-fetch a
    // bounded hit budget and group/cap afterwards.
    let file_limit = args.limit.unwrap_or(DEFAULT_HITS_FILE_LIMIT);
    let hits_per_file = args.hits_per_file.unwrap_or(DEFAULT_HITS_PER_FILE);
    let ranked_mode = !literal
        && !regex
        && !matches!(
            symbol_mode,
            Some(SymbolSearchMode::References | SymbolSearchMode::Callers)
        );
    let hit_budget = hits_mode_hit_budget(file_limit, ranked_mode);
    let search_limit = Some(hit_budget);

    let include_globs = parse_glob_csv(args.include.as_deref());
    let exclude_globs = parse_glob_csv(args.exclude.as_deref());
    let search_options = SearchOptions {
        limit: search_limit,
        context: args.context.unwrap_or(2),
        type_filter: args.type_filter.clone(),
        include_globs: include_globs.clone(),
        exclude_globs: exclude_globs.clone(),
        scope_filter: scope_filter.clone(),
        skip_gitignore: args.skip_gitignore.unwrap_or(false),
        force_neural: false,
        progress_tx: None,
        cancel_token: Some(cancellation.token()),
    };

    if wants_context_pack {
        let model = mcp_search_model(&workspace);
        let bundle = crate::context::build_context_bundle_with_options(
            &workspace,
            query,
            Some(model.as_ref()),
            &SearchOptions {
                limit: None,
                ..search_options.clone()
            },
            args.budget_tokens.unwrap_or(8_000),
            &crate::context::ContextBuildOptions {
                since: args.since.as_deref(),
            },
        )?;
        let query_uses_neural = crate::search::query_uses_neural(query, false);
        if std::env::var_os("IVYGREP_NO_AUTOSPAWN").is_none()
            && workspace.needs_search_enhancement(query_uses_neural)
        {
            let _ = workspace.trigger_background_search_enhancement(query_uses_neural);
        }
        let payload = json!({
            "workspace_root": current_workspace.root,
            "scope_path": scope_filter.as_ref().map(|scope| scope.rel_path.clone()),
            "scope_is_file": scope_filter.as_ref().is_some_and(|scope| scope.is_file),
            "query": query,
            "mode": "context",
            "result_count": bundle.items.len(),
            "include": include_globs,
            "exclude": exclude_globs,
            "context_pack": bundle,
        });
        return tool_success_result(payload, false);
    }

    let daemon_request = if symbol_mode.is_some() {
        None
    } else if literal {
        Some(DaemonRequest::LiteralSearch {
            path: Some(workspace.root.clone()),
            query: query.to_string(),
            limit: search_limit,
            context: args.context.unwrap_or(2),
            type_filter: args.type_filter.clone(),
            include_globs: include_globs.clone(),
            exclude_globs: exclude_globs.clone(),
            scope_path: scope_filter.as_ref().map(|scope| scope.rel_path.clone()),
            scope_is_file: scope_filter.as_ref().is_some_and(|scope| scope.is_file),
            skip_gitignore: args.skip_gitignore.unwrap_or(false),
        })
    } else if regex {
        Some(DaemonRequest::RegexSearch {
            path: Some(workspace.root.clone()),
            pattern: query.to_string(),
            limit: search_limit,
            context: args.context.unwrap_or(2),
            type_filter: args.type_filter.clone(),
            include_globs: include_globs.clone(),
            exclude_globs: exclude_globs.clone(),
            scope_path: scope_filter.as_ref().map(|scope| scope.rel_path.clone()),
            scope_is_file: scope_filter.as_ref().is_some_and(|scope| scope.is_file),
            skip_gitignore: args.skip_gitignore.unwrap_or(false),
        })
    } else {
        Some(DaemonRequest::Search {
            path: Some(workspace.root.clone()),
            query: query.to_string(),
            limit: search_limit,
            context: args.context.unwrap_or(2),
            type_filter: args.type_filter.clone(),
            include_globs: include_globs.clone(),
            exclude_globs: exclude_globs.clone(),
            scope_path: scope_filter.as_ref().map(|scope| scope.rel_path.clone()),
            scope_is_file: scope_filter.as_ref().is_some_and(|scope| scope.is_file),
            skip_gitignore: args.skip_gitignore.unwrap_or(false),
            force_neural: false,
            disable_memory_expansion: false,
        })
    };
    let mut search_warnings = Vec::new();
    let daemon_hits = if let Some(daemon_request) = daemon_request {
        // Tag the search so a client-side timeout or a cancelled MCP request
        // cancels it on the daemon instead of leaving the work running for a
        // caller that gave up.
        match cancellation.daemon_search(&daemon_request)? {
            Some(DaemonResponse::SearchResults { hits, warnings }) => {
                search_warnings = warnings;
                Some(hits)
            }
            Some(DaemonResponse::Error { message }) => {
                tracing::warn!("MCP daemon search unavailable, searching locally: {message}");
                None
            }
            Some(response) => {
                tracing::warn!("unexpected MCP daemon search response: {response:?}");
                None
            }
            None => None,
        }
    } else {
        None
    };
    // A cancelled daemon search must not fall back to a local search.
    if cancellation.is_cancelled() {
        bail!("request cancelled");
    }

    let mut hits = if let Some(hits) = daemon_hits {
        hits
    } else if let Some(mode) = symbol_mode {
        search_symbols_with_options(&workspace, query, mode, &search_options)?
    } else if literal {
        literal_search(&workspace, query, &search_options)?
    } else if regex {
        regex_search_with_options(&workspace, query, &search_options)?
    } else {
        // Load a neural query model only after neural vectors exist; a new
        // index returns hash results without downloading/loading model assets.
        let model = mcp_search_model(&workspace);
        let hits = hybrid_search(&workspace, query, Some(model.as_ref()), &search_options)?;
        // Exact queries build hash vectors; natural-language queries also build neural vectors.
        let query_uses_neural = crate::search::query_uses_neural(query, false);
        if std::env::var_os("IVYGREP_NO_AUTOSPAWN").is_none()
            && workspace.needs_search_enhancement(query_uses_neural)
        {
            let _ = workspace.trigger_background_search_enhancement(query_uses_neural);
        }
        hits
    };

    if !literal && !regex {
        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    let BoundedHits {
        files: mut grouped,
        total_matches,
        truncated,
    } = bound_hits_by_file(&hits, file_limit, hits_per_file, hit_budget);
    let verbose = args.verbose.unwrap_or(false);
    let first_line_only = args.first_line_only.unwrap_or(false);
    let file_name_only = args.file_name_only.unwrap_or(false);

    if !verbose {
        for file in &mut grouped {
            for hit in &mut file.hits {
                hit.reason.clear();
            }
        }
    }

    if first_line_only {
        for file in &mut grouped {
            for hit in &mut file.hits {
                hit.preview = hit
                    .preview
                    .lines()
                    .find(|line| !line.trim().is_empty())
                    .unwrap_or("")
                    .trim()
                    .to_string();
            }
        }
    }

    let mode_name = if literal {
        "literal"
    } else if regex {
        "regex"
    } else {
        match symbol_mode {
            Some(SymbolSearchMode::Definitions) => "symbol",
            Some(SymbolSearchMode::References) => "references",
            Some(SymbolSearchMode::Callers) => "callers",
            None => "hybrid",
        }
    };
    let summary = HitsSummary {
        workspace_root: &current_workspace.root,
        query,
        mode: mode_name,
        total_matches,
        truncated,
        warnings: &search_warnings,
        verbose,
    };
    let text = if file_name_only {
        render_file_paths_text(&summary, &grouped)
    } else {
        render_hits_text(&summary, &grouped)
    };
    let payload = if file_name_only {
        json!({
            "workspace_root": current_workspace.root,
            "scope_path": scope_filter.as_ref().map(|scope| scope.rel_path.clone()),
            "scope_is_file": scope_filter.as_ref().is_some_and(|scope| scope.is_file),
            "query": query,
            "mode": mode_name,
            "result_count": grouped.len(),
            "total_matches": total_matches,
            "truncated": truncated,
            "include": include_globs,
            "exclude": exclude_globs,
            "warnings": search_warnings,
            "file_paths": grouped.iter().map(|file| file.file_path.clone()).collect::<Vec<_>>(),
        })
    } else {
        json!({
            "workspace_root": current_workspace.root,
            "scope_path": scope_filter.as_ref().map(|scope| scope.rel_path.clone()),
            "scope_is_file": scope_filter.as_ref().is_some_and(|scope| scope.is_file),
            "query": query,
            "mode": mode_name,
            "result_count": grouped.len(),
            "total_matches": total_matches,
            "truncated": truncated,
            "include": include_globs,
            "exclude": exclude_globs,
            "warnings": search_warnings,
            "results": grouped,
        })
    };

    Ok(tool_success_result_with_text(payload, text))
}

/// Hit budget requested from the search layer for one hits-mode call.
///
/// Ranked modes return score-ordered chunks spread across files, so a small
/// multiple of `file_limit` fills the file list; the floor matches the CLI
/// default so the default MCP call costs the same as `ig <query>`. Enumerating
/// modes (literal, regex, references, callers) cluster many hits per file and
/// need a deeper budget to reach `file_limit` distinct files.
fn hits_mode_hit_budget(file_limit: usize, ranked_mode: bool) -> usize {
    let budget = if ranked_mode {
        file_limit
            .saturating_mul(RANKED_HIT_OVERFETCH)
            .max(crate::search::DEFAULT_SEARCH_LIMIT)
    } else {
        file_limit
            .saturating_mul(ENUMERATING_HIT_OVERFETCH)
            .max(MIN_ENUMERATING_HIT_BUDGET)
    };
    budget.min(crate::search::MAX_SEARCH_RESULT_LIMIT)
}

struct BoundedHits {
    files: Vec<FileSearchResult>,
    /// Distinct files matched before `file_limit` was applied. A lower bound
    /// when retrieval saturated `hit_budget`.
    total_matches: usize,
    /// Files were dropped, or retrieval saturated its hit budget so more files
    /// may exist.
    truncated: bool,
}

fn bound_hits_by_file(
    hits: &[SearchHit],
    file_limit: usize,
    hits_per_file: usize,
    hit_budget: usize,
) -> BoundedHits {
    let mut files = group_hits_by_file(hits, None);
    let total_matches = files.len();
    let budget_saturated = hits.len() >= hit_budget;
    let truncated = total_matches > file_limit || budget_saturated;
    files.truncate(file_limit);
    for file in &mut files {
        // With the retrieval budget exhausted, a dense file may hold matches
        // that were never retrieved; its counts are lower bounds, not totals.
        file.hit_count_is_lower_bound = budget_saturated;
        if file.hits.len() > hits_per_file {
            file.more_hits_in_file = file.hits.len() - hits_per_file;
            file.hits.truncate(hits_per_file);
        }
    }
    BoundedHits {
        files,
        total_matches,
        truncated,
    }
}

struct HitsSummary<'a> {
    workspace_root: &'a Path,
    query: &'a str,
    mode: &'a str,
    total_matches: usize,
    truncated: bool,
    warnings: &'a [String],
    verbose: bool,
}

fn render_hits_header(summary: &HitsSummary<'_>, shown: usize, out: &mut String) {
    use std::fmt::Write as _;
    if shown == 0 {
        let _ = writeln!(
            out,
            "No {} matches for \"{}\" in {}",
            summary.mode,
            summary.query,
            summary.workspace_root.display()
        );
    } else {
        // Retrieval that saturated its hit budget makes total_matches a lower
        // bound; say so instead of printing a misleading "3 of 3".
        let lower_bound = if summary.truncated && summary.total_matches <= shown {
            "+"
        } else {
            ""
        };
        let _ = write!(
            out,
            "{shown} of {}{lower_bound} files for \"{}\" ({}) in {}",
            summary.total_matches,
            summary.query,
            summary.mode,
            summary.workspace_root.display()
        );
        if summary.truncated {
            out.push_str("; truncated: narrow the query or scope path, or raise limit");
        }
        out.push('\n');
    }
    for warning in summary.warnings {
        let _ = writeln!(out, "warning: {warning}");
    }
}

/// Compact text rendering of grouped hits for LLM clients that read the text
/// block rather than `structuredContent`. Carries paths, line ranges, and
/// previews without JSON keys, escaping, scores, or per-hit path repetition.
fn render_hits_text(summary: &HitsSummary<'_>, files: &[FileSearchResult]) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    render_hits_header(summary, files.len(), &mut out);
    for file in files {
        let lower_bound = if file.hit_count_is_lower_bound {
            "+"
        } else {
            ""
        };
        let _ = write!(
            out,
            "\n{}  ({}{lower_bound} hit{}",
            file.file_path.display(),
            file.hit_count,
            if file.hit_count == 1 && !file.hit_count_is_lower_bound {
                ""
            } else {
                "s"
            }
        );
        if file.more_hits_in_file > 0 {
            let _ = write!(
                out,
                ", {} shown, {}{lower_bound} more",
                file.hits.len(),
                file.more_hits_in_file
            );
        }
        out.push_str(")\n");
        for hit in &file.hits {
            if hit.start_line == hit.end_line {
                let _ = writeln!(out, "  L{}", hit.start_line);
            } else {
                let _ = writeln!(out, "  L{}-{}", hit.start_line, hit.end_line);
            }
            if summary.verbose && !hit.reason.is_empty() {
                let _ = writeln!(out, "    reason: {}", hit.reason.trim());
            }
            for line in hit.preview.lines() {
                let _ = writeln!(out, "    {line}");
            }
        }
    }
    out
}

fn render_file_paths_text(summary: &HitsSummary<'_>, files: &[FileSearchResult]) -> String {
    let mut out = String::new();
    render_hits_header(summary, files.len(), &mut out);
    for file in files {
        out.push_str(&file.file_path.to_string_lossy());
        out.push('\n');
    }
    out
}

/// Detected framing mode for the stdio transport.
#[derive(Clone, Copy, PartialEq)]
enum FramingMode {
    /// Auto-detect on first line (initial state).
    Unknown,
    /// Newline-delimited JSON-RPC (mcp-cli, MCP Inspector).
    JsonLine,
    /// LSP-style Content-Length header framing.
    ContentLength,
}

/// Read a single line, bounded to MAX_MESSAGE_BYTES so a client that never
/// sends a newline can't grow memory without limit. Returns bytes read (0 = EOF).
fn read_line_capped<R: BufRead>(reader: &mut R, line: &mut String) -> Result<usize> {
    line.clear();
    let mut buf = Vec::new();
    let n = reader
        .take((MAX_MESSAGE_BYTES as u64) + 1)
        .read_until(b'\n', &mut buf)?;
    if buf.len() > MAX_MESSAGE_BYTES {
        bail!("request line exceeds maximum of {MAX_MESSAGE_BYTES} bytes");
    }
    line.push_str(&String::from_utf8_lossy(&buf));
    Ok(n)
}

/// One message read from the stdio transport.
enum Frame {
    Payload(Vec<u8>),
    /// A newline-delimited message over `MAX_MESSAGE_BYTES`. The rest of its
    /// line was skipped, so the next message can still be read.
    Oversized,
}

/// Consume input through the next newline without buffering it.
fn skip_line<R: BufRead>(reader: &mut R) -> io::Result<()> {
    loop {
        let buffer = reader.fill_buf()?;
        if buffer.is_empty() {
            return Ok(());
        }
        match buffer.iter().position(|byte| *byte == b'\n') {
            Some(newline) => {
                reader.consume(newline + 1);
                return Ok(());
            }
            None => {
                let length = buffer.len();
                reader.consume(length);
            }
        }
    }
}

fn read_message<R: BufRead>(reader: &mut R, mode: &mut FramingMode) -> Result<Option<Frame>> {
    // Read first non-empty line (skip blank lines between messages).
    let trimmed = loop {
        let mut line = Vec::new();
        let bytes = reader
            .by_ref()
            .take((MAX_MESSAGE_BYTES as u64) + 1)
            .read_until(b'\n', &mut line)?;
        if bytes == 0 {
            return Ok(None);
        }
        if line.len() > MAX_MESSAGE_BYTES {
            // A JSON line can resync at the next newline. An oversized header
            // line leaves no safe point to resume, so it ends the session.
            let json_line = match *mode {
                FramingMode::JsonLine => true,
                FramingMode::Unknown => line
                    .iter()
                    .find(|byte| !byte.is_ascii_whitespace())
                    .is_some_and(|byte| matches!(byte, b'{' | b'[')),
                FramingMode::ContentLength => false,
            };
            if !json_line {
                bail!("request line exceeds maximum of {MAX_MESSAGE_BYTES} bytes");
            }
            *mode = FramingMode::JsonLine;
            if line.last() != Some(&b'\n') {
                skip_line(reader)?;
            }
            return Ok(Some(Frame::Oversized));
        }
        let line = String::from_utf8_lossy(&line);
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            break trimmed.to_string();
        }
    };

    // Auto-detect framing: a first meaningful line starting with '{', or '[' for
    // a batch, is bare JSON.
    if *mode == FramingMode::Unknown {
        if trimmed.starts_with(['{', '[']) {
            *mode = FramingMode::JsonLine;
        } else {
            *mode = FramingMode::ContentLength;
        }
    }

    match *mode {
        FramingMode::JsonLine => {
            // The trimmed line IS the JSON payload.
            Ok(Some(Frame::Payload(trimmed.into_bytes())))
        }
        FramingMode::ContentLength => {
            // Parse header lines for Content-Length.
            let mut content_length: Option<usize> = None;
            let lower = trimmed.to_ascii_lowercase();
            if let Some(value) = lower.strip_prefix("content-length:") {
                content_length = Some(value.trim().parse::<usize>()?);
            }

            // Read remaining headers until empty line.
            loop {
                let mut line = String::new();
                let bytes = read_line_capped(reader, &mut line)?;
                if bytes == 0 {
                    return Ok(None);
                }
                let t = line.trim_end_matches(['\r', '\n']);
                if t.is_empty() {
                    break;
                }
                let lower = t.to_ascii_lowercase();
                if let Some(value) = lower.strip_prefix("content-length:") {
                    content_length = Some(value.trim().parse::<usize>()?);
                }
            }

            let len = content_length.context("missing Content-Length header")?;
            if len > MAX_MESSAGE_BYTES {
                bail!("Content-Length {len} exceeds maximum of {MAX_MESSAGE_BYTES} bytes");
            }
            let mut payload = vec![0u8; len];
            reader.read_exact(&mut payload)?;
            Ok(Some(Frame::Payload(payload)))
        }
        FramingMode::Unknown => unreachable!(),
    }
}

fn write_message<W: Write, T: Serialize>(
    writer: &mut W,
    response: &T,
    mode: FramingMode,
) -> Result<()> {
    let payload = serde_json::to_vec(response)?;
    match mode {
        FramingMode::JsonLine | FramingMode::Unknown => {
            writer.write_all(&payload)?;
            writer.write_all(b"\n")?;
        }
        FramingMode::ContentLength => {
            write!(writer, "Content-Length: {}\r\n\r\n", payload.len())?;
            writer.write_all(&payload)?;
        }
    }
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use serial_test::serial;
    use std::time::{Duration, Instant};

    use super::*;

    /// The production dispatcher without cancellation, as most tests call it.
    fn dispatch(method: &str, params: Value) -> std::result::Result<Value, DispatchError> {
        super::dispatch(method, params, &RequestCancellation::default())
    }

    fn execute_ivygrep_search(args: IvygrepSearchArgs) -> Result<Value> {
        super::execute_ivygrep_search(args, &RequestCancellation::default())
    }

    #[test]
    fn read_message_rejects_oversized_content_length() {
        // A huge Content-Length must be rejected, not allocated.
        let msg = format!("Content-Length: {}\r\n\r\n", MAX_MESSAGE_BYTES as u64 + 1);
        let mut reader = std::io::BufReader::new(msg.as_bytes());
        let mut mode = FramingMode::Unknown;
        let result = read_message(&mut reader, &mut mode);
        assert!(result.is_err(), "oversized Content-Length must be rejected");
    }

    #[test]
    fn read_message_accepts_normal_content_length() {
        let body = "{\"jsonrpc\":\"2.0\"}";
        let msg = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
        let mut reader = std::io::BufReader::new(msg.as_bytes());
        let mut mode = FramingMode::Unknown;
        let Some(Frame::Payload(payload)) = read_message(&mut reader, &mut mode).unwrap() else {
            panic!("expected a payload");
        };
        assert_eq!(payload, body.as_bytes());
    }

    fn payload_response(payload: &str) -> Option<Value> {
        let work = match admit(payload.as_bytes(), &PendingRequests::default()) {
            Admission::Answer(work) | Admission::Queue(work) => work,
            Admission::Ignore => return None,
        };
        run_work(work, &super::dispatch).map(|reply| serde_json::to_value(reply).unwrap())
    }

    #[test]
    fn jsonrpc_parse_error_reports_null_id() {
        let response = payload_response("{not json").unwrap();
        assert_eq!(response["error"]["code"], -32700);
        assert_eq!(response.get("id"), Some(&Value::Null));
    }

    #[test]
    fn jsonrpc_request_without_method_is_invalid_request_with_its_id() {
        let response = payload_response(r#"{"jsonrpc":"2.0","id":7,"params":{}}"#).unwrap();
        assert_eq!(response["error"]["code"], -32600);
        assert_eq!(response["id"], 7);

        let response = payload_response("[]").unwrap();
        assert_eq!(response["error"]["code"], -32600);
        assert_eq!(response.get("id"), Some(&Value::Null));

        // A client's reply to a server request has no method and needs no answer.
        assert!(payload_response(r#"{"jsonrpc":"2.0","id":3,"result":{}}"#).is_none());
    }

    #[test]
    fn jsonrpc_null_id_is_a_request_not_a_notification() {
        let response = payload_response(r#"{"jsonrpc":"2.0","id":null,"method":"ping"}"#).unwrap();
        assert_eq!(response.get("id"), Some(&Value::Null));
        assert_eq!(response["result"], json!({}));

        assert!(
            payload_response(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none()
        );
    }

    #[test]
    fn jsonrpc_batch_answers_requests_in_one_array() {
        let response = payload_response(
            r#"[{"jsonrpc":"2.0","id":1,"method":"ping"},{"jsonrpc":"2.0","method":"notifications/initialized"},{"jsonrpc":"2.0","id":"two","method":"missing/method"},42]"#,
        )
        .unwrap();
        let responses = response.as_array().expect("batch reply must be an array");
        assert_eq!(responses.len(), 3, "{response}");
        assert_eq!(responses[0]["id"], 1);
        assert_eq!(responses[0]["result"], json!({}));
        assert_eq!(responses[1]["id"], "two");
        assert_eq!(responses[1]["error"]["code"], -32601);
        assert_eq!(responses[2].get("id"), Some(&Value::Null));
        assert_eq!(responses[2]["error"]["code"], -32600);
    }

    #[test]
    fn jsonrpc_batch_without_requests_sends_nothing_and_empty_batch_is_invalid() {
        assert!(
            payload_response(
                r#"[{"jsonrpc":"2.0","method":"notifications/initialized"},{"jsonrpc":"2.0","id":9,"result":{}}]"#
            )
            .is_none()
        );

        let empty = payload_response("[]").unwrap();
        assert!(empty.is_object(), "{empty}");
        assert_eq!(empty["error"]["code"], -32600);
        assert_eq!(empty.get("id"), Some(&Value::Null));
    }

    #[test]
    fn read_message_detects_line_framing_from_a_batch() {
        let batch = "[{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}]\n";
        let mut reader = std::io::BufReader::new(batch.as_bytes());
        let mut mode = FramingMode::Unknown;
        let Some(Frame::Payload(payload)) = read_message(&mut reader, &mut mode).unwrap() else {
            panic!("expected a payload");
        };
        assert_eq!(payload, batch.trim_end().as_bytes());
        assert!(mode == FramingMode::JsonLine);
    }

    /// Stdin stand-in: bytes arrive as the test sends them, and EOF once the
    /// test drops the sender.
    struct ChannelReader {
        chunks: mpsc::Receiver<Vec<u8>>,
        chunk: Vec<u8>,
        offset: usize,
    }

    impl Read for ChannelReader {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            while self.offset == self.chunk.len() {
                match self.chunks.recv() {
                    Ok(chunk) => (self.chunk, self.offset) = (chunk, 0),
                    Err(_) => return Ok(0),
                }
            }
            let length = out.len().min(self.chunk.len() - self.offset);
            out[..length].copy_from_slice(&self.chunk[self.offset..self.offset + length]);
            self.offset += length;
            Ok(length)
        }
    }

    /// Stdout stand-in that forwards every write to the test.
    struct ChannelWriter(mpsc::Sender<Vec<u8>>);

    impl Write for ChannelWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            let _ = self.0.send(bytes.to_vec());
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// Stdout stand-in whose writes fail, like a client that closed its end.
    struct BrokenPipeWriter;

    impl Write for BrokenPipeWriter {
        fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
    }

    /// `serve` on its own thread over in-memory stdin and stdout.
    struct TestServer {
        input: Option<mpsc::Sender<Vec<u8>>>,
        output: mpsc::Receiver<Vec<u8>>,
        buffered: Vec<u8>,
        served: mpsc::Receiver<Result<()>>,
    }

    impl TestServer {
        fn start<D>(dispatch: D) -> Self
        where
            D: Fn(&str, Value, &RequestCancellation) -> std::result::Result<Value, DispatchError>
                + Send
                + Sync
                + 'static,
        {
            let (written, output) = mpsc::channel();
            Self::start_with_writer(dispatch, ChannelWriter(written), output)
        }

        fn start_with_writer<D>(
            dispatch: D,
            writer: impl Write + Send + 'static,
            output: mpsc::Receiver<Vec<u8>>,
        ) -> Self
        where
            D: Fn(&str, Value, &RequestCancellation) -> std::result::Result<Value, DispatchError>
                + Send
                + Sync
                + 'static,
        {
            let (input, chunks) = mpsc::channel();
            let (served_tx, served) = mpsc::channel();
            let reader = BufReader::new(ChannelReader {
                chunks,
                chunk: Vec::new(),
                offset: 0,
            });
            std::thread::spawn(move || {
                let _ = served_tx.send(serve(reader, writer, Arc::new(dispatch)));
            });
            Self {
                input: Some(input),
                output,
                buffered: Vec::new(),
                served,
            }
        }

        fn send(&self, message: Value) {
            self.send_raw(format!("{message}\n").into_bytes());
        }

        fn send_raw(&self, bytes: Vec<u8>) {
            self.input.as_ref().unwrap().send(bytes).unwrap();
        }

        /// The next reply, failing the test after five seconds.
        fn recv(&mut self) -> Value {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                if let Some(newline) = self.buffered.iter().position(|byte| *byte == b'\n') {
                    let line = self.buffered.drain(..=newline).collect::<Vec<_>>();
                    return serde_json::from_slice(&line).unwrap();
                }
                match self
                    .output
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                {
                    Ok(bytes) => self.buffered.extend(bytes),
                    Err(err) => panic!("no MCP reply within 5 s: {err}"),
                }
            }
        }

        /// What `serve` returned, failing the test if it runs past five seconds.
        fn result(&self) -> Result<()> {
            self.served
                .recv_timeout(Duration::from_secs(5))
                .expect("serve did not return within 5 s")
        }

        /// Close stdin and return what `serve` returned.
        fn finish(mut self) -> Result<()> {
            self.input.take();
            self.result()
        }
    }

    #[test]
    fn mcp_server_answers_ping_while_a_request_runs() {
        let (started, started_rx) = mpsc::channel();
        let (release, release_rx) = mpsc::channel::<()>();
        let release_rx = Mutex::new(release_rx);
        let mut server = TestServer::start(move |method, params, cancellation| {
            if method != "tools/call" {
                return super::dispatch(method, params, cancellation);
            }
            started.send(()).unwrap();
            release_rx
                .lock()
                .recv_timeout(Duration::from_secs(10))
                .expect("the test never released the request");
            Ok(json!({"released": true}))
        });

        server.send(json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {}}));
        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        server.send(json!({"jsonrpc": "2.0", "id": 2, "method": "ping"}));
        let ping = server.recv();
        assert_eq!(ping["id"], 2, "{ping}");
        assert_eq!(ping["result"], json!({}));

        release.send(()).unwrap();
        let call = server.recv();
        assert_eq!(call["id"], 1, "{call}");
        assert_eq!(call["result"]["released"], true);
        server.finish().unwrap();
    }

    #[test]
    fn mcp_server_sends_no_response_for_cancelled_requests() {
        let (events, events_rx) = mpsc::channel();
        let mut server = TestServer::start(move |method, params, cancellation| {
            if method != "tools/call" {
                return super::dispatch(method, params, cancellation);
            }
            let name = params["name"].as_str().unwrap_or_default().to_string();
            events.send(format!("{name} started")).unwrap();
            let token = cancellation.token();
            let deadline = Instant::now() + Duration::from_secs(10);
            while !token.load(Ordering::SeqCst) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(5));
            }
            events
                .send(format!(
                    "{name} token tripped: {}",
                    token.load(Ordering::SeqCst)
                ))
                .unwrap();
            Ok(json!({"finished": name}))
        });
        let call = |id: &str| json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": id}});
        let cancel = |id: &str| json!({"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": id, "reason": "test"}});
        let next_event = || events_rx.recv_timeout(Duration::from_secs(5)).unwrap();

        server.send(call("running"));
        assert_eq!(next_event(), "running started");
        // Queued behind the running request and cancelled before it starts.
        server.send(call("queued"));
        server.send(cancel("queued"));
        server.send(cancel("unknown"));
        server.send(cancel("running"));
        assert_eq!(next_event(), "running token tripped: true");

        // The worker answers in arrival order, so a response for either
        // cancelled request would arrive before this one.
        server.send(json!({"jsonrpc": "2.0", "id": "after", "method": "tools/list"}));
        let reply = server.recv();
        assert_eq!(reply["id"], "after", "{reply}");
        assert!(reply["result"]["tools"].is_array(), "{reply}");
        assert!(
            events_rx.try_recv().is_err(),
            "the request cancelled while queued started"
        );
        server.finish().unwrap();
    }

    #[test]
    fn mcp_server_recovers_from_an_oversized_json_line() {
        let mut server = TestServer::start(super::dispatch);
        let mut oversized = br#"{"jsonrpc":"2.0","id":1,"method":"ping","params":""#.to_vec();
        oversized.resize(MAX_MESSAGE_BYTES + 64, b'x');
        oversized.extend_from_slice(b"\"}\n");
        server.send_raw(oversized);
        server.send(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}));

        let error = server.recv();
        assert_eq!(error["error"]["code"], -32700, "{error}");
        assert_eq!(error.get("id"), Some(&Value::Null));
        let tools = server.recv();
        assert_eq!(tools["id"], 2, "{tools}");
        assert!(tools["result"]["tools"].is_array(), "{tools}");
        server.finish().unwrap();
    }

    #[test]
    fn mcp_server_returns_when_stdout_breaks_while_stdin_stays_open() {
        let broken_pipe = |result: Result<()>| {
            let err = result.expect_err("serve must report the failed write");
            assert!(
                err.downcast_ref::<io::Error>()
                    .is_some_and(|err| err.kind() == io::ErrorKind::BrokenPipe),
                "{err:#}"
            );
        };

        // The worker fails to write a queued reply while the reader waits for
        // more input.
        let server =
            TestServer::start_with_writer(super::dispatch, BrokenPipeWriter, mpsc::channel().1);
        server.send(json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}));
        broken_pipe(server.result());

        // The reader fails to write a ping reply while the worker runs a request.
        let (started, started_rx) = mpsc::channel();
        let (release, release_rx) = mpsc::channel::<()>();
        let release_rx = Mutex::new(release_rx);
        let server = TestServer::start_with_writer(
            move |method, params, cancellation| {
                if method == "tools/call" {
                    started.send(()).unwrap();
                    let _ = release_rx.lock().recv_timeout(Duration::from_secs(10));
                }
                super::dispatch(method, params, cancellation)
            },
            BrokenPipeWriter,
            mpsc::channel().1,
        );
        server.send(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {}}));
        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        server.send(json!({"jsonrpc": "2.0", "id": 3, "method": "ping"}));
        broken_pipe(server.result());
        drop(release);
    }

    #[test]
    fn mcp_queue_budget_counts_batch_members_and_payload_bytes() {
        let members = (0..=MAX_QUEUED_REQUESTS)
            .map(|id| json!({"jsonrpc": "2.0", "id": id, "method": "tools/list"}).to_string())
            .collect::<Vec<_>>();
        let batch = format!("[{}]", members.join(","));
        let Admission::Queue(work) = admit(batch.as_bytes(), &PendingRequests::default()) else {
            panic!("a batch goes to the worker");
        };
        let batch_cost = QueueCost::of(&work, batch.len());
        assert_eq!(batch_cost.requests, MAX_QUEUED_REQUESTS + 1);

        let mut load = QueuedLoad::default();
        let small = QueueCost {
            requests: 1,
            bytes: 64,
        };
        // An empty queue takes one payload of any size.
        assert!(load.try_add(batch_cost));
        assert!(
            !load.try_add(small),
            "batch members count against the request limit"
        );
        load.remove(batch_cost);

        for _ in 0..MAX_QUEUED_REQUESTS {
            assert!(load.try_add(small));
        }
        assert!(
            !load.try_add(small),
            "at most {MAX_QUEUED_REQUESTS} requests wait"
        );
        load.remove(small);
        let large = QueueCost {
            requests: 1,
            bytes: MAX_QUEUED_BYTES,
        };
        assert!(
            !load.try_add(large),
            "payload bytes count against the byte limit"
        );
    }

    #[test]
    #[serial]
    fn mcp_search_auto_indexes_and_respects_scope() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        let scoped = root.join("scoped");
        let other = root.join("other");
        std::fs::create_dir_all(&root).unwrap();
        let status = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(&root)
            .status()
            .unwrap();
        assert!(status.success());
        std::fs::create_dir_all(&scoped).unwrap();
        std::fs::create_dir_all(&other).unwrap();

        std::fs::write(
            scoped.join("match.rs"),
            "pub fn applyFilter(values: &[i32]) -> Vec<i32> { values.to_vec() }\n",
        )
        .unwrap();
        std::fs::write(
            other.join("match.rs"),
            "pub fn applyFilter(values: &[i32]) -> Vec<i32> { values.to_vec() }\n",
        )
        .unwrap();

        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let response = execute_ivygrep_search(IvygrepSearchArgs {
            query: Some("applyFilter".to_string()),
            path: Some(scoped.to_string_lossy().to_string()),
            output: None,
            budget_tokens: None,
            since: None,
            limit: None,
            hits_per_file: None,
            context: Some(2),
            type_filter: None,
            regex: Some(false),
            literal: None,
            symbol: None,
            refs: None,
            callers: None,
            include: None,
            exclude: None,
            first_line_only: Some(false),
            file_name_only: Some(false),
            verbose: Some(false),
            skip_gitignore: None,
        })
        .unwrap();

        let result = tool_json_payload(&response);
        let files = result
            .get("results")
            .and_then(|v| v.as_array())
            .unwrap()
            .iter()
            .filter_map(|entry| entry.get("file_path").and_then(|v| v.as_str()))
            .collect::<Vec<_>>();

        assert!(!files.is_empty());
        assert!(files.iter().all(|path| path.starts_with("scoped/")));
    }

    #[test]
    #[serial]
    fn mcp_raw_type_alias_matches_canonical_semantic_filter() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(
            root.join("lib.rs"),
            r#"pub struct AccountManager;

impl AccountManager {
    /// Performs durable credential refresh with retry and backoff after an expired session.
    pub fn refresh_credentials(&self, token: &str) -> Result<(), String> {
        let _ = token;
        Ok(())
    }
}
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("decoy.md"),
            "request validation configuration is documented here\n",
        )
        .unwrap();

        let home = tempfile::tempdir().unwrap();
        unsafe {
            std::env::set_var("IVYGREP_HOME", home.path());
            std::env::set_var("IVYGREP_NO_AUTOSPAWN", "1");
        }
        let workspace = Workspace::resolve(&root).unwrap();
        let hash_model = create_hash_model();
        index_workspace(&workspace, hash_model.as_ref()).unwrap();
        crate::indexer::enhance_workspace_hash(&workspace, hash_model.as_ref()).unwrap();
        std::fs::write(workspace.watcher_pid_path(), std::process::id().to_string()).unwrap();

        let search = |type_filter: &str| {
            execute_ivygrep_search(IvygrepSearchArgs {
                query: Some("secure account renewal strategy".to_string()),
                path: Some(root.to_string_lossy().to_string()),
                output: None,
                budget_tokens: None,
                since: None,
                limit: Some(10),
                hits_per_file: None,
                context: Some(2),
                type_filter: Some(type_filter.to_string()),
                regex: None,
                literal: None,
                symbol: None,
                refs: None,
                callers: None,
                include: None,
                exclude: None,
                first_line_only: None,
                file_name_only: Some(false),
                verbose: None,
                skip_gitignore: None,
            })
            .map(|response| {
                let payload = tool_json_payload(&response);
                let results = payload["results"].as_array().unwrap();
                let paths = results
                    .iter()
                    .filter_map(|result| result["file_path"].as_str())
                    .map(ToString::to_string)
                    .collect::<Vec<_>>();
                let sources = results
                    .iter()
                    .flat_map(|result| result["hits"].as_array().unwrap())
                    .flat_map(|hit| hit["sources"].as_array().unwrap())
                    .filter_map(serde_json::Value::as_str)
                    .map(ToString::to_string)
                    .collect::<std::collections::BTreeSet<_>>();
                (paths, sources)
            })
            .unwrap()
        };

        let (alias_paths, alias_sources) = search("rs");
        let (canonical_paths, canonical_sources) = search("rust");
        assert_eq!(alias_paths, canonical_paths);
        assert_eq!(alias_paths, vec!["lib.rs"]);
        assert_eq!(alias_sources, canonical_sources);
        assert!(alias_sources.contains("semantic"));
        assert!(alias_sources.contains("hash"));
    }

    #[test]
    #[serial]
    fn mcp_search_returns_budgeted_context_graph_pack() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("tests")).unwrap();
        let status = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(&root)
            .status()
            .unwrap();
        assert!(status.success());
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"context-fixture\"\nversion = \"0.1.0\"\ndescription = \"refresh token expiration fixture\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("src/auth.rs"),
            "use crate::clock::now;\npub fn rotate_refresh_token() { now(); }\n",
        )
        .unwrap();
        std::fs::write(root.join("src/clock.rs"), "pub fn now() -> u64 { 42 }\n").unwrap();
        std::fs::write(
            root.join("src/session.rs"),
            "use crate::auth::rotate_refresh_token;\npub fn refresh() { rotate_refresh_token(); }\n",
        )
        .unwrap();
        std::fs::write(
            root.join("tests/auth_test.rs"),
            "#[test]\nfn regression_case() { assert!(true); }\n",
        )
        .unwrap();
        std::fs::write(
            root.join("README.md"),
            "Refresh token rotation is implemented in [auth](src/auth.rs).\n",
        )
        .unwrap();
        for args in [
            vec!["config", "user.email", "test@example.com"],
            vec!["config", "user.name", "Test"],
            vec!["config", "commit.gpgsign", "false"],
            vec!["add", "."],
            vec!["commit", "-qm", "base"],
            vec!["branch", "-M", "main"],
            vec!["switch", "-qc", "feature"],
        ] {
            assert!(
                std::process::Command::new("git")
                    .args(args)
                    .current_dir(&root)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        std::fs::write(
            root.join("src/auth.rs"),
            "use crate::clock::now;\npub fn rotate_refresh_token() { now(); /* branch fix */ }\n",
        )
        .unwrap();

        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let response = execute_ivygrep_search(IvygrepSearchArgs {
            query: Some("rotate refresh token expiration".to_string()),
            path: Some(root.to_string_lossy().to_string()),
            output: Some("context_pack".to_string()),
            budget_tokens: Some(4_000),
            since: Some("main".to_string()),
            limit: None,
            hits_per_file: None,
            context: Some(2),
            type_filter: None,
            regex: None,
            literal: None,
            symbol: None,
            refs: None,
            callers: None,
            include: None,
            exclude: None,
            first_line_only: None,
            file_name_only: None,
            verbose: None,
            skip_gitignore: None,
        })
        .unwrap();

        let payload = tool_json_payload(&response);
        assert_eq!(payload["mode"], "context");
        assert_eq!(payload["context_pack"]["budget_tokens"], 4_000);
        assert_eq!(payload["context_pack"]["change_scope"]["since"], "main");
        assert_eq!(
            payload["context_pack"]["change_scope"]["dirty_worktree"],
            true
        );
        assert!(
            payload["context_pack"]["used_tokens"].as_u64().unwrap() <= 4_000,
            "{payload:#}"
        );
        let coverage = &payload["context_pack"]["coverage"];
        assert!(
            coverage["dependencies"].as_u64().unwrap() >= 1,
            "{payload:#}"
        );
        assert!(coverage["dependents"].as_u64().unwrap() >= 1, "{payload:#}");
        assert!(coverage["tests"].as_u64().unwrap() >= 1, "{payload:#}");
        assert!(coverage["config"].as_u64().unwrap() >= 1, "{payload:#}");
        assert!(
            coverage["documentation"].as_u64().unwrap() >= 1,
            "{payload:#}"
        );
        assert_eq!(response["structuredContent"], payload);

        let filtered = dispatch(
            "tools/call",
            json!({
                "name": "ig_search",
                "arguments": {
                    "query": "rotate refresh token expiration",
                    "path": root,
                    "output": "context_pack",
                    "budget_tokens": 4000,
                    "include": "src/**"
                }
            }),
        )
        .unwrap();
        let filtered = tool_json_payload(&filtered);
        assert!(
            filtered["context_pack"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .all(|item| item["file_path"].as_str().unwrap().starts_with("src/")),
            "{filtered:#}"
        );
        assert_eq!(filtered["context_pack"]["coverage"]["tests"], 0);
        assert_eq!(filtered["context_pack"]["coverage"]["config"], 0);
        assert_eq!(filtered["context_pack"]["coverage"]["documentation"], 0);
    }

    #[test]
    #[serial]
    fn mcp_context_pack_respects_gitignore_after_all_files_index() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        std::fs::create_dir_all(root.join("src")).unwrap();
        let status = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(&root)
            .status()
            .unwrap();
        assert!(status.success());
        std::fs::write(root.join(".gitignore"), "src/secret.rs\n").unwrap();
        std::fs::write(
            root.join("src/auth.rs"),
            "use crate::secret::load_seed;\npub fn rotate_refresh_token() { load_seed(); }\n",
        )
        .unwrap();
        std::fs::write(
            root.join("src/secret.rs"),
            "pub fn load_seed() -> &'static str { \"private\" }\n",
        )
        .unwrap();

        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let indexed = execute_ivygrep_search(IvygrepSearchArgs {
            query: Some("load_seed".to_string()),
            path: Some(root.to_string_lossy().to_string()),
            output: None,
            budget_tokens: None,
            since: None,
            limit: Some(10),
            hits_per_file: None,
            context: Some(2),
            type_filter: None,
            regex: None,
            literal: None,
            symbol: None,
            refs: None,
            callers: None,
            include: None,
            exclude: None,
            first_line_only: None,
            file_name_only: None,
            verbose: None,
            skip_gitignore: Some(true),
        })
        .unwrap();
        let indexed = tool_json_payload(&indexed);
        assert!(
            indexed["results"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["file_path"] == "src/secret.rs"),
            "{indexed:#}"
        );

        for (skip_gitignore, expects_secret) in [(None, false), (Some(true), true)] {
            let response = execute_ivygrep_search(IvygrepSearchArgs {
                query: Some("rotate refresh token".to_string()),
                path: Some(root.to_string_lossy().to_string()),
                output: Some("context_pack".to_string()),
                budget_tokens: Some(4_000),
                since: None,
                limit: None,
                hits_per_file: None,
                context: Some(2),
                type_filter: None,
                regex: None,
                literal: None,
                symbol: None,
                refs: None,
                callers: None,
                include: None,
                exclude: None,
                first_line_only: None,
                file_name_only: None,
                verbose: None,
                skip_gitignore,
            })
            .unwrap();
            let payload = tool_json_payload(&response);
            let contains_secret = payload["context_pack"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["file_path"] == "src/secret.rs");
            assert_eq!(contains_secret, expects_secret, "{payload:#}");
        }
    }

    #[test]
    #[serial]
    fn mcp_search_omits_reason_by_default() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(
            root.join("match.rs"),
            "pub fn applyFilter(values: &[i32]) -> Vec<i32> { values.to_vec() }\n",
        )
        .unwrap();

        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let response = execute_ivygrep_search(IvygrepSearchArgs {
            query: Some("applyFilter".to_string()),
            path: Some(root.to_string_lossy().to_string()),
            output: None,
            budget_tokens: None,
            since: None,
            limit: Some(5),
            hits_per_file: None,
            context: Some(2),
            type_filter: None,
            regex: Some(false),
            literal: None,
            symbol: None,
            refs: None,
            callers: None,
            include: None,
            exclude: None,
            first_line_only: Some(false),
            file_name_only: Some(false),
            verbose: Some(false),
            skip_gitignore: None,
        })
        .unwrap();

        let result = tool_json_payload(&response);
        let hits = result
            .get("results")
            .and_then(|v| v.as_array())
            .and_then(|files| files.first())
            .and_then(|file| file.get("hits"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        assert!(!hits.is_empty());
        assert!(hits.iter().all(|hit| hit.get("reason").is_none()));
    }

    #[test]
    #[serial]
    fn mcp_search_respects_include_exclude_globs() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(
            root.join("match.rs"),
            "pub fn applyFilter(values: &[i32]) -> Vec<i32> { values.to_vec() }\n",
        )
        .unwrap();
        std::fs::write(
            root.join("match.md"),
            "pub fn applyFilter(values: &[i32]) -> Vec<i32> { values.to_vec() }\n",
        )
        .unwrap();

        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let include_only = execute_ivygrep_search(IvygrepSearchArgs {
            query: Some("applyFilter".to_string()),
            path: Some(root.to_string_lossy().to_string()),
            output: None,
            budget_tokens: None,
            since: None,
            limit: Some(5),
            hits_per_file: None,
            context: Some(2),
            type_filter: None,
            regex: Some(false),
            literal: None,
            symbol: None,
            refs: None,
            callers: None,
            include: Some("*.md".to_string()),
            exclude: None,
            first_line_only: Some(false),
            file_name_only: Some(true),
            verbose: Some(false),
            skip_gitignore: None,
        })
        .unwrap();

        let include_payload = tool_json_payload(&include_only);
        let file_paths = include_payload
            .get("file_paths")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert_eq!(
            file_paths,
            vec![Value::String("match.md".to_string())],
            "include glob should keep only markdown results"
        );

        let brace_filtered = execute_ivygrep_search(IvygrepSearchArgs {
            query: Some("applyFilter".to_string()),
            path: Some(root.to_string_lossy().to_string()),
            output: None,
            budget_tokens: None,
            since: None,
            limit: Some(5),
            hits_per_file: None,
            context: Some(2),
            type_filter: None,
            regex: Some(false),
            literal: None,
            symbol: None,
            refs: None,
            callers: None,
            include: Some("*.{rs,md},*.txt".to_string()),
            exclude: Some("*.rs".to_string()),
            first_line_only: Some(false),
            file_name_only: Some(true),
            verbose: Some(false),
            skip_gitignore: None,
        })
        .unwrap();
        assert_eq!(
            tool_json_payload(&brace_filtered)["file_paths"],
            serde_json::json!(["match.md"])
        );

        let include_and_exclude = execute_ivygrep_search(IvygrepSearchArgs {
            query: Some("applyFilter".to_string()),
            path: Some(root.to_string_lossy().to_string()),
            output: None,
            budget_tokens: None,
            since: None,
            limit: Some(5),
            hits_per_file: None,
            context: Some(2),
            type_filter: None,
            regex: Some(false),
            literal: None,
            symbol: None,
            refs: None,
            callers: None,
            include: Some("*.md".to_string()),
            exclude: Some("match.md".to_string()),
            first_line_only: Some(false),
            file_name_only: Some(true),
            verbose: Some(false),
            skip_gitignore: None,
        })
        .unwrap();

        let excluded_payload = tool_json_payload(&include_and_exclude);
        assert_eq!(
            excluded_payload
                .get("file_paths")
                .and_then(|v| v.as_array())
                .map(|arr| arr.len())
                .unwrap_or_default(),
            0
        );
    }

    #[test]
    fn mcp_initialize_returns_protocol_version_and_capabilities() {
        let result = dispatch("initialize", json!({})).unwrap();
        assert_eq!(result["protocolVersion"], LEGACY_PROTOCOL_VERSION);
        assert!(result["capabilities"]["tools"].is_object());
        assert_eq!(result["serverInfo"]["name"], "ig");
        let version = result["serverInfo"]["version"].as_str().unwrap();
        assert!(!version.is_empty());
    }

    #[test]
    fn mcp_initialize_negotiates_current_protocol_version() {
        let result = dispatch(
            "initialize",
            json!({"protocolVersion": LATEST_PROTOCOL_VERSION}),
        )
        .unwrap();
        assert_eq!(result["protocolVersion"], LATEST_PROTOCOL_VERSION);

        let fallback = dispatch(
            "initialize",
            json!({"protocolVersion": "unsupported-version"}),
        )
        .unwrap();
        assert_eq!(fallback["protocolVersion"], LATEST_PROTOCOL_VERSION);
    }

    #[test]
    fn mcp_tools_list_returns_ig_search() {
        let result = dispatch("tools/list", json!({})).unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0]["name"], "ig_search");
        let schema = &tools[0]["inputSchema"];
        assert!(schema["properties"]["query"].is_object());
        assert!(schema["properties"]["regex"].is_object());
        assert!(schema["properties"]["symbol"].is_object());
        assert!(schema["properties"]["refs"].is_object());
        assert!(schema["properties"]["callers"].is_object());
        assert_eq!(schema["properties"]["output"]["default"], "hits");
        assert_eq!(
            schema["properties"]["output"]["enum"],
            json!(["hits", "context_pack"])
        );
        assert_eq!(schema["properties"]["budget_tokens"]["minimum"], 256);
        assert_eq!(schema["properties"]["budget_tokens"]["maximum"], 131_072);
        assert!(
            schema["properties"]["budget_tokens"]
                .get("default")
                .is_none()
        );
        assert_eq!(schema["properties"]["limit"]["minimum"], 1);
        assert_eq!(schema["properties"]["limit"]["maximum"], 1000);
        assert!(
            schema["properties"]["limit"]["description"]
                .as_str()
                .unwrap()
                .contains("defaults to 10")
        );
        assert_eq!(schema["properties"]["hits_per_file"]["minimum"], 1);
        assert_eq!(schema["properties"]["hits_per_file"]["maximum"], 100);
        assert_eq!(schema["properties"]["hits_per_file"]["default"], 3);
        assert_eq!(schema["properties"]["context"]["minimum"], 0);
        assert_eq!(schema["properties"]["context"]["maximum"], 100);
        assert_eq!(schema["properties"]["context"]["default"], 2);
        assert_eq!(schema["additionalProperties"], false);
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("query")));
        assert!(tools[0]["outputSchema"].is_object());
        assert_eq!(
            tools[0]["outputSchema"]["properties"]["context_pack"]["additionalProperties"],
            false
        );
        let output_properties = &tools[0]["outputSchema"]["properties"];
        assert_eq!(output_properties["total_matches"]["type"], "integer");
        assert_eq!(output_properties["truncated"]["type"], "boolean");
        assert_eq!(
            output_properties["results"]["items"]["properties"]["more_hits_in_file"]["type"],
            "integer"
        );
        assert_eq!(tools[0]["annotations"]["readOnlyHint"], false);
        assert_eq!(tools[1]["name"], "ig_status");
        assert!(tools[1]["outputSchema"].is_object());
        assert_eq!(tools[1]["inputSchema"]["additionalProperties"], false);
        assert_eq!(tools[1]["annotations"]["readOnlyHint"], true);
    }

    #[test]
    fn mcp_known_tool_errors_are_recoverable_tool_results() {
        let response = dispatch(
            "tools/call",
            json!({
                "name": "ig_search",
                "arguments": {}
            }),
        )
        .unwrap();
        assert_eq!(response["isError"], true);
        assert!(
            response["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("missing required argument: query")
        );

        let conflicting_modes = dispatch(
            "tools/call",
            json!({
                "name": "ig_search",
                "arguments": {
                    "query": "needle",
                    "literal": true,
                    "regex": true
                }
            }),
        )
        .unwrap();
        assert_eq!(conflicting_modes["isError"], true);
        assert!(
            conflicting_modes["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("mutually exclusive")
        );

        for (arguments, expected) in [
            (
                json!({"query": "needle", "output": "unknown"}),
                "output must be hits or context_pack",
            ),
            (
                json!({"query": "needle", "budget_tokens": 8000}),
                "budget_tokens requires output=context_pack",
            ),
            (
                json!({"query": "needle", "output": "context_pack", "budget_tokens": 255}),
                "budget_tokens must be between 256 and 131072",
            ),
            (
                json!({"query": "needle", "output": "context_pack", "literal": true}),
                "output=context_pack cannot be combined",
            ),
        ] {
            let response = dispatch(
                "tools/call",
                json!({"name": "ig_search", "arguments": arguments}),
            )
            .unwrap();
            assert_eq!(response["isError"], true);
            assert!(
                response["content"][0]["text"]
                    .as_str()
                    .unwrap()
                    .contains(expected),
                "{response:#}"
            );
        }

        let unknown_argument = dispatch(
            "tools/call",
            json!({
                "name": "ig_search",
                "arguments": {
                    "query": "needle",
                    "limt": 5
                }
            }),
        )
        .unwrap();
        assert_eq!(unknown_argument["isError"], true);
        assert!(
            unknown_argument["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("unknown field `limt`")
        );

        let unknown_status_argument = dispatch(
            "tools/call",
            json!({
                "name": "ig_status",
                "arguments": {"unexpected": true}
            }),
        )
        .unwrap();
        assert_eq!(unknown_status_argument["isError"], true);
    }

    #[test]
    fn mcp_returns_standard_json_rpc_error_codes() {
        let unknown_method = handle_request(
            JsonRpcRequest {
                id: Some(json!(1)),
                method: "tools/nonexistent".to_string(),
                params: json!({}),
            },
            &super::dispatch,
            &RequestCancellation::default(),
        )
        .unwrap();
        let error = unknown_method.error.unwrap();
        assert_eq!(error.code, -32601);
        assert!(error.message.contains("method not found"));

        for params in [json!({}), json!({"name": "unknown_tool", "arguments": {}})] {
            let invalid_params = handle_request(
                JsonRpcRequest {
                    id: Some(json!(2)),
                    method: "tools/call".to_string(),
                    params,
                },
                &super::dispatch,
                &RequestCancellation::default(),
            )
            .unwrap();
            assert_eq!(invalid_params.error.unwrap().code, -32602);
        }
    }

    #[test]
    #[serial]
    fn mcp_search_regex_mode() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(
            root.join("match.md"),
            "before\ncalculate_tax amount\nafter\n",
        )
        .unwrap();
        std::fs::write(
            root.join("match.rs"),
            "before\npub fn calculate_tax() {}\nafter\n",
        )
        .unwrap();

        let home = tempfile::tempdir().unwrap();
        unsafe {
            std::env::set_var("IVYGREP_HOME", home.path());
            std::env::set_var("IVYGREP_NO_AUTOSPAWN", "1");
        }

        let response = execute_ivygrep_search(IvygrepSearchArgs {
            query: Some(r"calculate_\w+".to_string()),
            path: Some(root.to_string_lossy().to_string()),
            output: None,
            budget_tokens: None,
            since: None,
            limit: Some(5),
            hits_per_file: None,
            context: Some(2),
            type_filter: Some("markdown".to_string()),
            regex: Some(true),
            literal: None,
            symbol: None,
            refs: None,
            callers: None,
            include: None,
            exclude: None,
            first_line_only: Some(false),
            file_name_only: Some(false),
            verbose: Some(false),
            skip_gitignore: None,
        })
        .unwrap();

        let result = tool_json_payload(&response);
        assert_eq!(result["mode"], "regex");
        assert_eq!(result["result_count"], 1);
        let file = &result["results"][0];
        assert_eq!(file["file_path"], "match.md");
        assert_eq!(file["hits"][0]["start_line"], 1);
        assert_eq!(file["hits"][0]["end_line"], 3);
        assert_eq!(
            file["hits"][0]["preview"],
            "before\ncalculate_tax amount\nafter"
        );
    }

    #[test]
    #[serial]
    fn mcp_search_literal_mode() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(
            root.join("match.rs"),
            "pub fn calculate_tax(amount: f64) -> f64 { amount * 0.2 }\n",
        )
        .unwrap();

        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let response = execute_ivygrep_search(IvygrepSearchArgs {
            query: Some("calculate_tax".to_string()),
            path: Some(root.to_string_lossy().to_string()),
            output: None,
            budget_tokens: None,
            since: None,
            limit: Some(5),
            hits_per_file: None,
            context: Some(2),
            type_filter: None,
            regex: None,
            literal: Some(true),
            symbol: None,
            refs: None,
            callers: None,
            include: None,
            exclude: None,
            first_line_only: Some(false),
            file_name_only: Some(false),
            verbose: Some(false),
            skip_gitignore: None,
        })
        .unwrap();

        let result = tool_json_payload(&response);
        assert_eq!(result["mode"], "literal");
        let count = result["result_count"].as_u64().unwrap();
        assert!(count > 0, "literal search should find results");
    }

    #[test]
    #[serial]
    fn mcp_search_symbol_modes() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(
            root.join("match.rs"),
            "pub fn calculate_tax(amount: f64) -> f64 { amount * 0.2 }\n\
             pub fn checkout() -> f64 { calculate_tax(10.0) }\n\
             pub fn applyFilter(value: bool) -> bool { value }\n\
             pub fn render() -> bool { applyFilter(true) }\n",
        )
        .unwrap();

        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        for (symbol, refs, callers, expected_mode) in [
            (true, false, false, "symbol"),
            (false, true, false, "references"),
            (false, false, true, "callers"),
        ] {
            let response = execute_ivygrep_search(IvygrepSearchArgs {
                query: Some("calculate_tax".to_string()),
                path: Some(root.to_string_lossy().to_string()),
                output: None,
                budget_tokens: None,
                since: None,
                limit: Some(5),
                hits_per_file: None,
                context: Some(2),
                type_filter: None,
                regex: None,
                literal: None,
                symbol: Some(symbol),
                refs: Some(refs),
                callers: Some(callers),
                include: None,
                exclude: None,
                first_line_only: Some(false),
                file_name_only: Some(false),
                verbose: Some(false),
                skip_gitignore: None,
            })
            .unwrap();
            let result = tool_json_payload(&response);
            assert_eq!(result["mode"], expected_mode);
            assert!(result["result_count"].as_u64().unwrap() > 0);
        }

        for query in ["applyFilter", "applyFilter()"] {
            for (refs, callers, expected_mode) in
                [(true, false, "references"), (false, true, "callers")]
            {
                let response = execute_ivygrep_search(IvygrepSearchArgs {
                    query: Some(query.to_string()),
                    path: Some(root.to_string_lossy().to_string()),
                    output: None,
                    budget_tokens: None,
                    since: None,
                    limit: Some(5),
                    hits_per_file: None,
                    context: Some(2),
                    type_filter: None,
                    regex: None,
                    literal: None,
                    symbol: None,
                    refs: Some(refs),
                    callers: Some(callers),
                    include: None,
                    exclude: None,
                    first_line_only: Some(false),
                    file_name_only: Some(false),
                    verbose: Some(false),
                    skip_gitignore: None,
                })
                .unwrap();
                let result = tool_json_payload(&response);
                assert_eq!(result["mode"], expected_mode);
                assert!(result["result_count"].as_u64().unwrap() > 0);
            }
        }
    }

    #[test]
    fn mcp_search_rejects_conflicting_modes() {
        let error = execute_ivygrep_search(IvygrepSearchArgs {
            query: Some("calculate_tax".to_string()),
            path: None,
            output: None,
            budget_tokens: None,
            since: None,
            limit: Some(5),
            hits_per_file: None,
            context: Some(2),
            type_filter: None,
            regex: None,
            literal: Some(true),
            symbol: None,
            refs: Some(true),
            callers: None,
            include: None,
            exclude: None,
            first_line_only: Some(false),
            file_name_only: Some(false),
            verbose: Some(false),
            skip_gitignore: None,
        })
        .unwrap_err();
        assert!(error.to_string().contains("mutually exclusive"));
    }

    #[test]
    #[serial]
    fn mcp_auto_index_defers_vector_enrichment() {
        // MCP auto-index must commit lexical stores without building ANN
        // vectors inline. Multi-million chunk hash HNSW construction takes
        // minutes and must run in background enhancement.
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        for i in 0..6 {
            std::fs::write(
                root.join(format!("file_{i}.rs")),
                format!("pub fn calculate_tax_{i}(amount: f64) -> f64 {{ amount * 0.2 }}\n"),
            )
            .unwrap();
        }

        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        // Don't spawn the background neural enhancement subprocess during tests.
        unsafe { std::env::set_var("IVYGREP_NO_AUTOSPAWN", "1") };

        let _ = execute_ivygrep_search(IvygrepSearchArgs {
            query: Some("calculate tax".to_string()),
            path: Some(root.to_string_lossy().to_string()),
            output: None,
            budget_tokens: None,
            since: None,
            limit: Some(5),
            hits_per_file: None,
            context: Some(2),
            type_filter: None,
            regex: Some(false),
            literal: None,
            symbol: None,
            refs: None,
            callers: None,
            include: None,
            exclude: None,
            first_line_only: Some(false),
            file_name_only: Some(false),
            verbose: Some(false),
            skip_gitignore: None,
        })
        .unwrap();

        let workspace = crate::workspace::Workspace::resolve(&root).unwrap();
        let store = crate::vector_store::VectorStore::open_readonly(
            &workspace.vector_path(),
            256,
            crate::vector_store::ScalarKind::F16,
            crate::vector_store::VectorTier::Hash,
        )
        .expect("hash vector store (vectors.usearch) should open at 256 dims");
        assert_eq!(
            store.dimensions(),
            256,
            "MCP auto-index must initialize hash store at 256 dimensions"
        );
        assert_eq!(store.size(), 0, "MCP auto-index must defer hash ANN build");

        unsafe { std::env::remove_var("IVYGREP_NO_AUTOSPAWN") };
    }

    #[test]
    fn mcp_query_model_caches_neural_but_not_hash_fallback() {
        // #57: a successfully-loaded neural model is cached once per process.
        // The hash fallback is intentionally NOT cached, so a transient neural
        // failure can't pin the process to hash embeddings forever.
        let a = mcp_query_model();
        let b = mcp_query_model();
        if a.dimensions() == 384 {
            // Neural model available — must be the same cached instance.
            assert!(
                Arc::ptr_eq(&a, &b),
                "neural query model should be cached, not reloaded per call"
            );
        } else {
            // Neural unavailable (not compiled in / load failed): hash fallback
            // is rebuilt each call so neural can be retried later. Just ensure a
            // usable model comes back.
            assert_eq!(a.dimensions(), b.dimensions());
        }
    }

    #[test]
    #[serial]
    fn mcp_search_uses_hash_model_until_neural_vectors_exist() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        std::fs::write(tmp.path().join("lib.rs"), "pub fn marker() {}\n").unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let index_model = create_hash_model();
        index_workspace(&workspace, index_model.as_ref()).unwrap();

        assert!(workspace.needs_neural_enhancement());
        assert_eq!(mcp_search_model(&workspace).dimensions(), 256);
    }

    fn synthetic_hit(file: &str, line: usize, score: f32) -> SearchHit {
        SearchHit {
            file_path: PathBuf::from(file),
            start_line: line,
            end_line: line + 2,
            preview: format!("line {line}\nbody {line}\nend {line}"),
            reason: String::new(),
            score,
            sources: vec!["literal".to_string()],
            neural_requested: false,
            neural_executed: false,
        }
    }

    #[test]
    fn hits_mode_hit_budget_scales_with_files_and_mode() {
        assert_eq!(
            hits_mode_hit_budget(DEFAULT_HITS_FILE_LIMIT, true),
            crate::search::DEFAULT_SEARCH_LIMIT
        );
        assert_eq!(
            hits_mode_hit_budget(1, true),
            crate::search::DEFAULT_SEARCH_LIMIT
        );
        assert_eq!(hits_mode_hit_budget(40, true), 200);
        assert_eq!(
            hits_mode_hit_budget(DEFAULT_HITS_FILE_LIMIT, false),
            MIN_ENUMERATING_HIT_BUDGET
        );
        assert_eq!(hits_mode_hit_budget(25, false), 500);
        assert_eq!(
            hits_mode_hit_budget(1000, false),
            crate::search::MAX_SEARCH_RESULT_LIMIT
        );
        assert_eq!(
            hits_mode_hit_budget(1000, true),
            crate::search::MAX_SEARCH_RESULT_LIMIT
        );
    }

    #[test]
    fn bound_hits_by_file_applies_file_limit_and_per_file_cap() {
        let mut hits = (1..=6)
            .map(|line| synthetic_hit("busy.rs", line * 10, 1.0))
            .collect::<Vec<_>>();
        hits.push(synthetic_hit("quiet.rs", 1, 1.0));
        hits.push(synthetic_hit("other.rs", 1, 1.0));

        let saturated = bound_hits_by_file(&hits, 2, 3, hits.len());
        assert!(saturated.truncated);
        assert!(
            saturated
                .files
                .iter()
                .all(|file| file.hit_count_is_lower_bound),
            "an exhausted retrieval budget makes every per-file count a lower bound"
        );
        let serialized = serde_json::to_value(&saturated.files).unwrap();
        assert_eq!(serialized[0]["hit_count_is_lower_bound"], true);

        let bounded = bound_hits_by_file(&hits, 2, 3, 1000);
        assert_eq!(bounded.total_matches, 3);
        assert!(bounded.truncated, "third file was dropped");
        assert!(
            bounded
                .files
                .iter()
                .all(|file| !file.hit_count_is_lower_bound)
        );
        assert_eq!(bounded.files.len(), 2);
        assert_eq!(bounded.files[0].file_path, PathBuf::from("busy.rs"));
        assert_eq!(bounded.files[0].hit_count, 6);
        assert_eq!(bounded.files[0].hits.len(), 3);
        assert_eq!(bounded.files[0].more_hits_in_file, 3);
        assert_eq!(bounded.files[1].more_hits_in_file, 0);
        let serialized = serde_json::to_value(&bounded.files).unwrap();
        assert_eq!(serialized[0]["more_hits_in_file"], 3);
        assert!(
            serialized[1].get("more_hits_in_file").is_none(),
            "uncut files omit more_hits_in_file: {serialized:#}"
        );
        assert!(
            serialized[0].get("hit_count_is_lower_bound").is_none(),
            "exact counts omit the lower-bound flag: {serialized:#}"
        );

        let complete = bound_hits_by_file(&hits, 10, 10, 1000);
        assert_eq!(complete.total_matches, 3);
        assert!(!complete.truncated);
        assert_eq!(complete.files[0].hits.len(), 6);
        assert_eq!(complete.files[0].more_hits_in_file, 0);

        // Retrieval that saturated its hit budget may hide more files.
        let saturated = bound_hits_by_file(&hits, 10, 10, hits.len());
        assert_eq!(saturated.total_matches, 3);
        assert!(saturated.truncated);
    }

    #[test]
    fn render_hits_text_is_compact_and_self_contained() {
        let mut hits = (1..=4)
            .map(|line| synthetic_hit("src/busy.rs", line * 10, 1.0))
            .collect::<Vec<_>>();
        hits.push(synthetic_hit("src/quiet.rs", 7, 1.0));
        let bounded = bound_hits_by_file(&hits, 10, 3, 1000);
        let summary = HitsSummary {
            workspace_root: Path::new("/repo"),
            query: "needle",
            mode: "literal",
            total_matches: bounded.total_matches,
            truncated: bounded.truncated,
            warnings: &["one workspace failed".to_string()],
            verbose: false,
        };
        let text = render_hits_text(&summary, &bounded.files);
        assert!(
            text.starts_with("2 of 2 files for \"needle\" (literal) in /repo\n"),
            "{text}"
        );
        assert!(text.contains("warning: one workspace failed\n"), "{text}");
        assert!(
            text.contains("src/busy.rs  (4 hits, 3 shown, 1 more)\n"),
            "{text}"
        );
        assert!(text.contains("src/quiet.rs  (1 hit)\n"), "{text}");
        assert!(
            text.contains("  L10-12\n    line 10\n    body 10\n"),
            "{text}"
        );
        assert!(!text.contains("\"file_path\""), "{text}");
        let json = serde_json::to_string(&bounded.files).unwrap();
        assert!(
            text.len() * 4 < json.len() * 3,
            "text ({}) should be materially smaller than JSON ({})",
            text.len(),
            json.len()
        );

        let saturated = render_hits_text(
            &HitsSummary {
                truncated: true,
                warnings: &[],
                ..summary
            },
            &bounded.files,
        );
        assert!(
            saturated.starts_with("2 of 2+ files for \"needle\" (literal) in /repo; truncated"),
            "{saturated}"
        );

        let empty = render_hits_text(
            &HitsSummary {
                total_matches: 0,
                truncated: false,
                warnings: &[],
                ..summary
            },
            &[],
        );
        assert_eq!(empty, "No literal matches for \"needle\" in /repo\n");
    }

    #[test]
    #[serial]
    fn mcp_hits_mode_bounds_files_and_hits_per_file() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        // One busy file with six matching lines plus fourteen single-match files.
        let busy = (0..6)
            .map(|i| format!("pub fn needle_token_{i}() -> u32 {{ {i} }}\n"))
            .collect::<String>();
        std::fs::write(root.join("busy.rs"), busy).unwrap();
        for i in 0..14 {
            std::fs::write(
                root.join(format!("file_{i:02}.rs")),
                format!("pub fn other_{i}() -> u32 {{ needle_token_{i}() }}\n"),
            )
            .unwrap();
        }
        std::fs::write(
            root.join("unique.rs"),
            "pub fn lonely_marker_fn() -> u32 { 7 }\n",
        )
        .unwrap();

        let home = tempfile::tempdir().unwrap();
        unsafe {
            std::env::set_var("IVYGREP_HOME", home.path());
            std::env::set_var("IVYGREP_NO_AUTOSPAWN", "1");
        }

        let search = |query: &str, limit: Option<usize>, hits_per_file: Option<usize>| {
            execute_ivygrep_search(IvygrepSearchArgs {
                query: Some(query.to_string()),
                path: Some(root.to_string_lossy().to_string()),
                output: None,
                budget_tokens: None,
                since: None,
                limit,
                hits_per_file,
                context: Some(0),
                type_filter: None,
                regex: None,
                literal: Some(true),
                symbol: None,
                refs: None,
                callers: None,
                include: None,
                exclude: None,
                first_line_only: None,
                file_name_only: None,
                verbose: None,
                skip_gitignore: None,
            })
            .unwrap()
        };

        // (a) omitted limit: server-side default of 10 files, not 500 hits.
        let response = search("needle_token", None, None);
        let payload = tool_json_payload(&response);
        let results = payload["results"].as_array().unwrap();
        assert_eq!(results.len(), DEFAULT_HITS_FILE_LIMIT, "{payload:#}");
        assert_eq!(payload["result_count"], DEFAULT_HITS_FILE_LIMIT);
        // (c) truncation signals.
        assert_eq!(payload["total_matches"], 15, "{payload:#}");
        assert_eq!(payload["truncated"], true);
        // (b) per-file cap on the busiest file.
        let busy = &results[0];
        assert_eq!(busy["file_path"], "busy.rs", "{payload:#}");
        assert_eq!(busy["hit_count"], 6);
        assert_eq!(
            busy["hits"].as_array().unwrap().len(),
            DEFAULT_HITS_PER_FILE
        );
        assert_eq!(busy["more_hits_in_file"], 3);
        assert!(
            results[1..]
                .iter()
                .all(|file| file.get("more_hits_in_file").is_none()),
            "{payload:#}"
        );
        // (d) text block is a compact rendering, not the JSON payload again.
        let text = tool_text(&response);
        let structured_len = serde_json::to_string(&response["structuredContent"])
            .unwrap()
            .len();
        assert!(!text.trim_start().starts_with('{'), "{text}");
        assert!(!text.contains("\"file_path\""), "{text}");
        assert!(!text.contains("structuredContent"), "{text}");
        assert!(
            text.contains("busy.rs  (6 hits, 3 shown, 3 more)"),
            "{text}"
        );
        assert!(text.contains("truncated"), "{text}");
        assert!(
            text.len() * 2 < structured_len,
            "text {} bytes vs structuredContent {structured_len} bytes",
            text.len()
        );

        // limit counts files, not hits: two files, busy.rs still capped.
        let payload = tool_json_payload(&search("needle_token", Some(2), None));
        let paths = payload["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|file| file["file_path"].as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(paths.len(), 2, "{payload:#}");
        assert_eq!(payload["results"][0]["hits"].as_array().unwrap().len(), 3);
        assert_eq!(payload["truncated"], true);

        // hits_per_file lifts the cap.
        let payload = tool_json_payload(&search("needle_token", Some(1), Some(10)));
        assert_eq!(payload["results"][0]["file_path"], "busy.rs");
        assert_eq!(payload["results"][0]["hits"].as_array().unwrap().len(), 6);
        assert!(payload["results"][0].get("more_hits_in_file").is_none());

        // Nothing cut: truncated is false and total_matches equals result_count.
        let payload = tool_json_payload(&search("lonely_marker_fn", None, None));
        assert_eq!(payload["result_count"], 1);
        assert_eq!(payload["total_matches"], 1);
        assert_eq!(payload["truncated"], false);

        // hits_per_file is validated and hits-mode only.
        let error = dispatch(
            "tools/call",
            json!({
                "name": "ig_search",
                "arguments": {"query": "needle_token", "path": root, "hits_per_file": 0}
            }),
        )
        .unwrap();
        assert_eq!(error["isError"], true);
        assert!(
            error["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("hits_per_file must be between 1 and 100"),
            "{error:#}"
        );
        let error = dispatch(
            "tools/call",
            json!({
                "name": "ig_search",
                "arguments": {
                    "query": "needle_token",
                    "path": root,
                    "output": "context_pack",
                    "hits_per_file": 3
                }
            }),
        )
        .unwrap();
        assert_eq!(error["isError"], true);
        assert!(
            error["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("hits_per_file requires output=hits"),
            "{error:#}"
        );

        unsafe { std::env::remove_var("IVYGREP_NO_AUTOSPAWN") };
    }

    fn tool_text(response: &Value) -> &str {
        response
            .get("content")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("text"))
            .and_then(|v| v.as_str())
            .expect("tool response content text")
    }

    fn tool_json_payload(response: &Value) -> Value {
        let content = tool_text(response);
        if let Some(payload) = response.get("structuredContent") {
            assert!(payload.is_object(), "structuredContent must be an object");
            assert!(!content.trim().is_empty(), "text content must not be empty");
            return payload.clone();
        }
        serde_json::from_str(content).expect("valid JSON payload")
    }

    #[test]
    fn indexing_status_result_is_non_error_with_progress_estimate() {
        assert_eq!(parse_file_progress("1200/4000"), Some((1200, 4000)));
        assert_eq!(parse_file_progress("scanning"), None);
        // 1200 files in 30 s leaves 2800 files at 40/s: 70 s, clamped to 60.
        assert_eq!(estimate_retry_after_secs(Some(1200), Some(4000), 30), 60);
        assert_eq!(estimate_retry_after_secs(Some(3900), Some(4000), 30), 5);
        assert_eq!(
            estimate_retry_after_secs(None, None, 30),
            DEFAULT_INDEX_RETRY_AFTER_SECS
        );

        let result = indexing_tool_result(json!({
            "status": "indexing",
            "workspace_root": "/tmp/repo",
            "progress": {"phase": "indexing", "done": 1200, "total": 4000, "percent": 30.0},
            "elapsed_secs": 30,
            "retry_after_secs": 60,
            "message": "Index in progress; call again later."
        }))
        .unwrap();
        assert_eq!(result["isError"], false);
        assert_eq!(result["structuredContent"]["status"], "indexing");
        let text = result["content"][0]["text"].as_str().unwrap();
        let (summary, payload) = text.split_once('\n').unwrap();
        assert!(summary.contains("1200/4000 files (30.0%)"), "{summary}");
        assert!(summary.contains("again in ~60s"), "{summary}");
        let payload: Value = serde_json::from_str(payload).unwrap();
        assert_eq!(payload, result["structuredContent"]);
    }

    #[test]
    fn search_output_schema_accepts_indexing_status_branch() {
        let schema = search_output_schema();
        let branches = schema["oneOf"].as_array().unwrap();
        assert_eq!(branches.len(), 2);
        assert!(
            branches[1]["required"]
                .as_array()
                .unwrap()
                .contains(&json!("status"))
        );
        assert_eq!(schema["properties"]["status"]["enum"], json!(["indexing"]));
        assert_eq!(schema["additionalProperties"], false);
    }

    #[test]
    fn mcp_status_returns_projects() {
        let response = dispatch(
            "tools/call",
            json!({
                "name": "ig_status",
                "arguments": {}
            }),
        )
        .unwrap();
        let payload = tool_json_payload(&response);
        let workspaces = payload
            .get("workspaces")
            .and_then(|v| v.as_array())
            .unwrap();
        let _ = workspaces.len();
        assert_eq!(response["isError"], false);
        assert!(response["structuredContent"].is_object());
    }
}
