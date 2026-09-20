//! The daemon's queue of background enhancement requests.
//!
//! A search or a watch update asks for hash or neural vectors. The daemon used
//! to start a worker process for every such workspace at once, so twenty
//! edited workspaces ran twenty workers, each with its own model and stores.
//! Requests now wait here. The daemon starts a worker only while the lane has
//! a free place (`IVYGREP_ENHANCE_MAX_WORKERS`), and the workspace that was
//! searched or edited last goes first. Workers take their place through a
//! lock under the app home (`indexer::admit_worker`), so the limit also holds
//! for workers that a CLI or an MCP session starts without the daemon.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;
use parking_lot::Mutex;
use tracing::warn;

use crate::indexer::WorkerLane;
use crate::workspace::Workspace;

/// Workspaces that may wait at once. One more evicts the oldest request; its
/// next search or edit asks again.
const MAX_PENDING: usize = 1024;

/// How the queue looks at workers. The daemon uses `WorkerProcesses`; tests
/// use a host without processes.
pub(crate) trait WorkerHost: Send + Sync {
    /// Workers that may run at once in one lane.
    fn places(&self) -> usize;
    /// Places of `lane` that no worker of any process holds now.
    fn free_places(&self, lane: WorkerLane) -> usize;
    /// Lane of the work that `workspace` still needs, `None` when it needs none.
    fn lane(&self, workspace: &Workspace, query_uses_neural: bool) -> Option<WorkerLane>;
    /// Whether hash vectors lag the index, whatever else the workspace needs.
    fn needs_hash(&self, workspace: &Workspace) -> bool;
    /// Live worker of `workspace`, running or waiting for a place.
    fn worker(&self, workspace: &Workspace) -> Option<u32>;
    fn start(&self, workspace: &Workspace, lane: WorkerLane) -> Result<()>;
}

/// Worker processes as `Workspace::trigger_background_enhancement` starts them.
pub(crate) struct WorkerProcesses;

impl WorkerHost for WorkerProcesses {
    fn places(&self) -> usize {
        crate::config::enhance_max_workers()
    }

    fn free_places(&self, lane: WorkerLane) -> usize {
        crate::indexer::free_worker_slots(lane)
    }

    fn lane(&self, workspace: &Workspace, query_uses_neural: bool) -> Option<WorkerLane> {
        if !workspace.needs_search_enhancement(query_uses_neural) {
            return None;
        }
        Some(
            if workspace.search_enhancement_uses_neural(query_uses_neural) {
                WorkerLane::Neural
            } else {
                WorkerLane::Hash
            },
        )
    }

    fn needs_hash(&self, workspace: &Workspace) -> bool {
        workspace.needs_hash_enhancement()
    }

    fn worker(&self, workspace: &Workspace) -> Option<u32> {
        workspace.enhancement_worker_pid()
    }

    fn start(&self, workspace: &Workspace, lane: WorkerLane) -> Result<()> {
        match lane {
            WorkerLane::Hash => workspace.trigger_background_hash_enhancement(),
            WorkerLane::Neural => workspace.trigger_background_enhancement(),
        }
    }
}

struct PendingRequest {
    workspace: Workspace,
    query_uses_neural: bool,
    lane: WorkerLane,
    /// Position in the order of requests and searches; the highest goes first.
    recency: u64,
    /// A neural request that has to wait may still get its hash vectors now.
    /// Cleared once that was tried, so a long queue is not probed every pass.
    hash_first: bool,
}

struct StartedWorker {
    workspace: Workspace,
    lane: WorkerLane,
    pid: u32,
}

#[derive(Default)]
struct QueueState {
    pending: HashMap<String, PendingRequest>,
    /// Workers that this queue started and that still live. Never more than
    /// `places` per lane.
    started: HashMap<String, StartedWorker>,
    recency: u64,
}

impl QueueState {
    fn next_recency(&mut self) -> u64 {
        self.recency += 1;
        self.recency
    }
}

pub(crate) struct EnhancementQueue {
    host: Box<dyn WorkerHost>,
    state: Mutex<QueueState>,
    /// `pending.len()`, readable without the lock that `dispatch` holds.
    pending_len: AtomicUsize,
    wake: tokio::sync::Notify,
}

impl EnhancementQueue {
    pub(crate) fn new(host: Box<dyn WorkerHost>) -> Self {
        Self {
            host,
            state: Mutex::new(QueueState::default()),
            pending_len: AtomicUsize::new(0),
            wake: tokio::sync::Notify::new(),
        }
    }

    /// A search reached these workspaces: their waiting requests move to the
    /// front. Never blocks; a touch that meets a running dispatch is skipped.
    pub(crate) fn touch<'a>(&self, workspace_ids: impl Iterator<Item = &'a str>) {
        if self.pending_len.load(Ordering::Relaxed) == 0 {
            return;
        }
        let Some(mut state) = self.state.try_lock() else {
            return;
        };
        for id in workspace_ids {
            let recency = state.next_recency();
            if let Some(request) = state.pending.get_mut(id) {
                request.recency = recency;
            }
        }
    }

    /// Queue the work that `workspace` still needs. Reads the index, so call
    /// it off the async runtime.
    pub(crate) fn request(&self, workspace: Workspace, query_uses_neural: bool) {
        let Some(mut lane) = self.host.lane(&workspace, query_uses_neural) else {
            return;
        };
        let mut state = self.state.lock();
        let mut uses_neural = query_uses_neural;
        if let Some(waiting) = state.pending.get(&workspace.id)
            && waiting.query_uses_neural
        {
            // A neural request is not downgraded by a later lexical search.
            uses_neural = true;
            lane = waiting.lane;
        }
        let recency = state.next_recency();
        state.pending.insert(
            workspace.id.clone(),
            PendingRequest {
                workspace,
                query_uses_neural: uses_neural,
                lane,
                recency,
                // An edit or a search since the last try: hash vectors may lag again.
                hash_first: true,
            },
        );
        if state.pending.len() > MAX_PENDING
            && let Some(oldest) = state
                .pending
                .iter()
                .min_by_key(|(_, request)| request.recency)
                .map(|(id, _)| id.clone())
        {
            state.pending.remove(&oldest);
        }
        self.pending_len
            .store(state.pending.len(), Ordering::Relaxed);
        drop(state);
        self.wake.notify_one();
    }

    /// Start workers for the most recent requests while their lane has a free
    /// place, and forget requests whose root is gone or whose work is done.
    /// Touches the file system and starts processes: call it off the runtime.
    pub(crate) fn dispatch(&self) {
        let mut state = self.state.lock();
        let QueueState {
            pending, started, ..
        } = &mut *state;
        started.retain(|_, worker| self.host.worker(&worker.workspace) == Some(worker.pid));
        pending.retain(|_, request| !crate::workspace::root_is_gone(&request.workspace.root));

        // A started worker may not hold its place yet: it waits for the load
        // guard, or behind a worker of another process. So the daemon also
        // keeps its own workers of a lane within the limit.
        let places = self.host.places();
        let mut capacity = HashMap::new();
        for lane in [WorkerLane::Hash, WorkerLane::Neural] {
            let mine = started.values().filter(|worker| worker.lane == lane);
            let free = self.host.free_places(lane);
            capacity.insert(lane, free.min(places.saturating_sub(mine.count())));
        }

        let mut order = pending
            .iter()
            .map(|(id, request)| (request.recency, id.clone()))
            .collect::<Vec<_>>();
        order.sort_by_key(|(recency, _)| std::cmp::Reverse(*recency));
        for (_, id) in order {
            if capacity.values().all(|free| *free == 0) {
                break;
            }
            let request = pending.get_mut(&id).expect("request is pending");
            let hash_while_waiting = capacity[&request.lane] == 0
                && request.lane == WorkerLane::Neural
                && request.hash_first
                && capacity[&WorkerLane::Hash] > 0;
            if capacity[&request.lane] == 0 && !hash_while_waiting {
                continue;
            }
            // One worker per workspace: the request waits for that worker.
            if self.host.worker(&request.workspace).is_some() {
                continue;
            }
            let lane = if hash_while_waiting {
                // Neural runs are long. A workspace that waits for one gets
                // its hash vectors meanwhile, so that semantic search works,
                // and keeps its turn for the neural place.
                request.hash_first = false;
                if !self.host.needs_hash(&request.workspace) {
                    continue;
                }
                WorkerLane::Hash
            } else {
                // The index may have moved on since the request.
                let Some(lane) = self
                    .host
                    .lane(&request.workspace, request.query_uses_neural)
                else {
                    pending.remove(&id);
                    continue;
                };
                if capacity[&lane] == 0 {
                    request.lane = lane;
                    continue;
                }
                lane
            };
            let workspace = request.workspace.clone();
            if !hash_while_waiting {
                pending.remove(&id);
            }
            if let Err(error) = self.host.start(&workspace, lane) {
                warn!(
                    "failed to start background enhancement for {}: {error:#}",
                    workspace.root.display()
                );
                continue;
            }
            if let Some(pid) = self.host.worker(&workspace) {
                *capacity.get_mut(&lane).expect("both lanes have a capacity") -= 1;
                started.insert(
                    id,
                    StartedWorker {
                        workspace,
                        lane,
                        pid,
                    },
                );
            }
        }
        self.pending_len.store(pending.len(), Ordering::Relaxed);
    }

    /// Resolves once a request waits. An idle daemon does not poll the queue.
    pub(crate) async fn wait_for_requests(&self) {
        while self.pending_len.load(Ordering::Relaxed) == 0 {
            self.wake.notified().await;
        }
    }

    #[cfg(test)]
    fn pending_ids(&self) -> Vec<String> {
        let mut ids = self
            .state
            .lock()
            .pending
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        ids.sort();
        ids
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::Arc;

    use serial_test::serial;
    use tempfile::{TempDir, tempdir};

    use super::*;

    /// Workers without processes: `start` makes one, `finish` ends it.
    #[derive(Default)]
    struct FakeWorkers {
        live: Mutex<HashMap<String, (u32, WorkerLane)>>,
        neural: Mutex<HashSet<String>>,
        hash_done: Mutex<HashSet<String>>,
        done: Mutex<HashSet<String>>,
        order: Mutex<Vec<(String, WorkerLane)>>,
        /// Places that workers of other processes hold.
        foreign: Mutex<usize>,
    }

    impl FakeWorkers {
        fn finish(&self, workspace: &Workspace) {
            let (_, lane) = self.live.lock().remove(&workspace.id).unwrap();
            self.hash_done.lock().insert(workspace.id.clone());
            if lane == WorkerLane::Neural || !self.neural.lock().contains(&workspace.id) {
                self.done.lock().insert(workspace.id.clone());
            }
        }

        fn started(&self) -> Vec<(String, WorkerLane)> {
            self.order.lock().clone()
        }
    }

    impl WorkerHost for Arc<FakeWorkers> {
        fn places(&self) -> usize {
            2
        }

        fn free_places(&self, lane: WorkerLane) -> usize {
            let held = self
                .live
                .lock()
                .values()
                .filter(|(_, held)| *held == lane)
                .count();
            2usize.saturating_sub(held + *self.foreign.lock())
        }

        fn lane(&self, workspace: &Workspace, _query_uses_neural: bool) -> Option<WorkerLane> {
            if self.done.lock().contains(&workspace.id) {
                return None;
            }
            Some(if self.neural.lock().contains(&workspace.id) {
                WorkerLane::Neural
            } else {
                WorkerLane::Hash
            })
        }

        fn needs_hash(&self, workspace: &Workspace) -> bool {
            !self.hash_done.lock().contains(&workspace.id)
        }

        fn worker(&self, workspace: &Workspace) -> Option<u32> {
            self.live.lock().get(&workspace.id).map(|(pid, _)| *pid)
        }

        fn start(&self, workspace: &Workspace, lane: WorkerLane) -> Result<()> {
            let mut order = self.order.lock();
            order.push((workspace.id.clone(), lane));
            self.live
                .lock()
                .insert(workspace.id.clone(), (order.len() as u32, lane));
            Ok(())
        }
    }

    fn workspaces(count: usize) -> (Vec<TempDir>, Vec<Workspace>) {
        let roots = (0..count).map(|_| tempdir().unwrap()).collect::<Vec<_>>();
        let workspaces = roots
            .iter()
            .map(|root| Workspace::resolve(root.path()).unwrap())
            .collect();
        (roots, workspaces)
    }

    #[test]
    #[serial]
    fn queue_runs_the_latest_requests_first_within_the_limit_and_drains() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let (mut roots, workspaces) = workspaces(5);
        let host = Arc::new(FakeWorkers::default());
        let queue = EnhancementQueue::new(Box::new(host.clone()));
        let hash = |index: usize| (workspaces[index].id.clone(), WorkerLane::Hash);

        for workspace in &workspaces {
            queue.request(workspace.clone(), false);
        }
        // A search moves a waiting workspace to the front.
        queue.touch([workspaces[1].id.as_str()].into_iter());
        queue.dispatch();
        assert_eq!(
            host.started(),
            [hash(1), hash(4)],
            "two places: the searched workspace, then the latest request"
        );
        queue.dispatch();
        assert_eq!(host.started().len(), 2, "a full lane starts nothing");

        // A request for a workspace whose worker still runs waits for it.
        queue.request(workspaces[4].clone(), false);
        // The root of a waiting workspace is deleted.
        drop(roots.remove(0));
        host.finish(&workspaces[1]);
        queue.dispatch();
        assert_eq!(host.started().last(), Some(&hash(3)));
        let mut waiting = vec![workspaces[2].id.clone(), workspaces[4].id.clone()];
        waiting.sort();
        assert_eq!(
            queue.pending_ids(),
            waiting,
            "the deleted root left the queue"
        );

        // A worker of another process holds a place: the daemon leaves it alone.
        *host.foreign.lock() = 1;
        host.finish(&workspaces[3]);
        queue.dispatch();
        assert_eq!(host.started().len(), 3);
        *host.foreign.lock() = 0;

        host.finish(&workspaces[4]);
        queue.dispatch();
        assert_eq!(host.started().last(), Some(&hash(2)));
        host.finish(&workspaces[2]);
        queue.dispatch();
        assert!(queue.pending_ids().is_empty(), "the queue drains");
        assert_eq!(host.started().len(), 4, "finished work is not run again");
        assert!(queue.state.lock().started.is_empty());
    }

    #[test]
    #[serial]
    fn workspace_that_waits_for_a_neural_place_gets_hash_vectors_meanwhile() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let (_roots, workspaces) = workspaces(4);
        let host = Arc::new(FakeWorkers::default());
        let queue = EnhancementQueue::new(Box::new(host.clone()));
        for workspace in &workspaces {
            host.neural.lock().insert(workspace.id.clone());
        }
        let started = |index: usize, lane| (workspaces[index].id.clone(), lane);

        // Two long neural runs take both neural places.
        queue.request(workspaces[0].clone(), true);
        queue.request(workspaces[1].clone(), true);
        queue.dispatch();
        // A fresh worktree is searched. Its neural run has to wait, its hash
        // vectors do not. A later lexical search keeps the neural request.
        queue.request(workspaces[2].clone(), true);
        queue.request(workspaces[2].clone(), false);
        // While other processes hold the hash places too, nothing starts.
        *host.foreign.lock() = 2;
        queue.dispatch();
        assert_eq!(host.started().len(), 2);
        *host.foreign.lock() = 0;
        queue.dispatch();
        assert_eq!(
            host.started(),
            [
                started(1, WorkerLane::Neural),
                started(0, WorkerLane::Neural),
                started(2, WorkerLane::Hash)
            ]
        );
        assert_eq!(queue.pending_ids(), [workspaces[2].id.clone()]);

        // The hash worker ends; no neural place is free yet. Hash vectors that
        // are current are not built again, by this workspace or a new one.
        host.finish(&workspaces[2]);
        host.hash_done.lock().insert(workspaces[3].id.clone());
        queue.request(workspaces[3].clone(), true);
        queue.dispatch();
        queue.dispatch();
        assert_eq!(host.started().len(), 3);

        // A neural place frees: the request that was made last goes first.
        host.finish(&workspaces[0]);
        queue.dispatch();
        assert_eq!(host.started().last(), Some(&started(3, WorkerLane::Neural)));
        host.finish(&workspaces[1]);
        queue.dispatch();
        assert_eq!(host.started().last(), Some(&started(2, WorkerLane::Neural)));
        assert!(queue.pending_ids().is_empty());
    }
}
