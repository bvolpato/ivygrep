import "highlight.js/styles/github-dark.css";
import hljs from "highlight.js/lib/common";
import MarkdownIt from "markdown-it";
import {
  guardCurrentStream,
  LatestRequestGuard,
  searchCompletionStatus
} from "./event-source-session";
import "./styles.css";

const logoUrl = new URL("./assets/ivygrep-icon.svg", import.meta.url).href;

type BootConfig = {
  version?: string;
  query?: string | null;
  workspace?: string | null;
};

type WorkspaceStatus = {
  root: string;
  file_count?: number;
  chunk_count?: number;
  watch_enabled?: boolean;
  watcher_alive?: boolean;
  has_neural_vectors?: boolean;
  neural_coverage_percent?: number;
  enhancing_stalled?: boolean;
  enhancing_in_progress?: boolean;
  enhancing_progress_count?: number | null;
  indexing_stalled?: boolean;
  indexing_in_progress?: boolean;
  indexing_progress?: string | null;
  compaction?: { healthy?: boolean };
};

type SearchHit = {
  file_path: string;
  start_line: number;
  end_line: number;
  score: number;
  preview?: string;
  sources?: string[];
};

type SearchPayload = {
  error?: string;
  errors?: string[];
  hits?: SearchHit[];
  context_pack?: ContextBundle;
  elapsed_ms?: number;
};

type ContextChange = {
  file_path: string;
  old_path?: string;
  status: string;
  sources: string[];
};

type ContextBundle = {
  task: string;
  workspace: string;
  change_scope?: {
    since?: string;
    base_commit?: string;
    dirty_worktree: boolean;
    total_changes: number;
    changes_truncated: boolean;
    changes: ContextChange[];
  };
  referenced_paths: Array<{ file_path: string; line?: number }>;
  budget_tokens: number;
  used_tokens: number;
  candidate_count: number;
  truncated: boolean;
  anchor_symbols: string[];
  coverage: Record<string, number>;
  items: Array<{
    file_path: string;
    start_line: number;
    end_line: number;
    roles: string[];
    reasons: string[];
    sources: string[];
    preview: string;
    estimated_tokens: number;
  }>;
};

type SearchDonePayload = {
  ok?: boolean;
};

type TreeEntry = {
  name: string;
  path: string;
  is_dir: boolean;
};

type TreePayload = {
  error?: string;
  path?: string;
  entries?: TreeEntry[];
};

type FilePayload = {
  error?: string;
  path: string;
  absolute_path?: string;
  text?: string;
  line_count: number;
  truncated?: boolean;
};

type ViewerMode = "preview" | "source";
type CopyFormat = "files" | "raw" | "json";
type CopyScope = "visible" | "pinned";

type AppState = {
  workspaces: WorkspaceStatus[];
  workspace: string;
  workspaceFilter: string;
  scope: string;
  events: EventSource | null;
  searching: boolean;
  hits: SearchHit[];
  contextPack: ContextBundle | null;
  autoOpenKey: string;
  manualOpen: boolean;
  requestedWorkspace: string;
  viewerMode: ViewerMode;
  currentFilePath: string;
  currentFileAbsolutePath: string;
  currentFileText: string;
  currentFileStart: number;
  currentFileEnd: number;
  currentHitKey: string;
  pinnedHitKeys: Set<string>;
  searchErrors: string[];
};

const ALL_WORKSPACES = "__all__";
const DEFAULT_LIMIT = 50;
const LOAD_MORE_STEP = 50;
const MAX_LIMIT = 500;
const boot = readBoot();
const queryParams = new URLSearchParams(location.search);
const fileRequests = new LatestRequestGuard();
const treeRequests = new LatestRequestGuard();
const markdown = new MarkdownIt({
  html: false,
  linkify: true,
  highlight(code, language) {
    return `<pre class="md-code"><code class="hljs">${highlightText(code, language)}</code></pre>`;
  }
});
const state: AppState = {
  workspaces: [],
  workspace: ALL_WORKSPACES,
  workspaceFilter: "",
  scope: "",
  events: null,
  searching: false,
  hits: [],
  contextPack: null,
  autoOpenKey: "",
  manualOpen: false,
  requestedWorkspace: queryParams.get("workspace") || boot.workspace || "",
  viewerMode: "source",
  currentFilePath: "",
  currentFileAbsolutePath: "",
  currentFileText: "",
  currentFileStart: 1,
  currentFileEnd: 1,
  currentHitKey: "",
  pinnedHitKeys: new Set(),
  searchErrors: []
};

function readBoot(): BootConfig {
  const text = document.querySelector<HTMLMetaElement>("meta[name='ivygrep-boot']")?.content.trim();
  if (!text || text === "__IVYGREP_BOOT__") return {};
  try {
    return JSON.parse(text) as BootConfig;
  } catch {
    return {};
  }
}

function svgIcon(name: "folder" | "file" | "up"): string {
  if (name === "folder") {
    return `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 7.5A2.5 2.5 0 0 1 5.5 5H10l2 2h6.5A2.5 2.5 0 0 1 21 9.5v7A2.5 2.5 0 0 1 18.5 19h-13A2.5 2.5 0 0 1 3 16.5v-9Z" fill="currentColor"/></svg>`;
  }
  if (name === "up") {
    return `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M11 6 5 12l6 6M5 12h14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
  }
  return `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 3h7l5 5v13H7V3Z" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"/><path d="M14 3v6h5" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"/></svg>`;
}

function renderShell(): void {
  byId("app").innerHTML = `
    <div class="app-shell">
      <header class="topbar">
        <div class="brand">
          <div class="mark"><img src="${logoUrl}" alt="" /></div>
          <div>ivygrep web</div>
        </div>
        <div class="searchbar" role="search">
          <label class="sr-only" for="query">Search query</label>
          <textarea id="query" rows="2" placeholder="Search code, paste issue, or paste stack trace"></textarea>
          <select id="mode" aria-label="Search mode">
            <option value="hybrid">Hybrid</option>
            <option value="context">Context pack</option>
            <option value="literal">Literal</option>
            <option value="regex">Regex</option>
          </select>
          <button class="btn" id="search" type="button">Search</button>
          <button class="btn secondary" id="refresh" type="button">Refresh</button>
        </div>
        <div class="statusline" role="status" aria-live="polite" aria-atomic="true"><span class="dot" aria-hidden="true"></span><span id="status">Starting</span></div>
      </header>
      <main class="layout">
        <aside class="sidebar">
          <div class="section-title">Workspaces</div>
          <label class="sr-only" for="workspace-filter">Filter workspaces</label>
          <input id="workspace-filter" class="workspace-filter" type="search" placeholder="Filter workspaces" />
          <div id="workspaces"></div>
          <div class="section-title">Filters</div>
          <div class="filters">
            <input id="type" placeholder="type" aria-label="Language or file type" />
            <input id="limit" type="number" min="1" max="${MAX_LIMIT}" value="${DEFAULT_LIMIT}" aria-label="Result limit" />
            <input id="include" placeholder="include globs" aria-label="Include path globs" />
            <input id="exclude" placeholder="exclude globs" aria-label="Exclude path globs" />
            <label id="since-field" class="filter-field"><span>Git base</span><input id="since" placeholder="main" aria-label="Context Git base" /></label>
            <label id="budget-field" class="filter-field"><span>Token budget</span><input id="budget" type="number" min="256" max="131072" value="8000" aria-label="Context token budget" /></label>
          </div>
          <div class="scopebar">
            <span id="scope-label">Scope: all folders</span>
            <button class="linkbtn" id="clear-scope" type="button">Global</button>
          </div>
          <div class="section-title">Explorer</div>
          <div id="tree" class="tree"><div class="empty">Select a workspace to browse.</div></div>
        </aside>
        <section class="results">
          <div class="results-head">
            <div class="summary" id="summary" aria-live="polite" aria-atomic="true">No search yet.</div>
            <div class="copy-tools">
              <select id="copy-scope" aria-label="Copy scope">
                <option value="visible">Visible</option>
                <option value="pinned">Pinned</option>
              </select>
              <select id="copy-format" aria-label="Copy format">
                <option value="files">File names</option>
                <option value="raw">Raw output</option>
                <option value="json">JSON</option>
              </select>
              <button class="btn secondary copy-btn" id="copy-results" type="button" disabled>Copy</button>
            </div>
          </div>
          <div id="results"><div class="empty">Pick a workspace or all indices, then search.</div></div>
        </section>
        <section class="viewer">
          <div class="viewer-head">
            <div class="viewer-head-row">
              <div>
                <div class="viewer-title" id="viewer-title">No file selected</div>
                <div class="viewer-meta" id="viewer-meta">Search result previews open here.</div>
              </div>
              <div class="viewer-controls">
                <button class="toggle" id="open-current" type="button" disabled>Open</button>
                <div class="viewer-actions" id="viewer-actions" hidden>
                  <button class="toggle active" id="preview-mode" type="button">Preview</button>
                  <button class="toggle" id="source-mode" type="button">Source</button>
                </div>
              </div>
            </div>
          </div>
          <div class="file-view" id="file-view"></div>
        </section>
      </main>
    </div>`;
}

function installFavicon(): void {
  const existing = document.querySelector<HTMLLinkElement>("link[rel='icon']");
  const favicon = existing || document.createElement("link");
  favicon.rel = "icon";
  favicon.type = "image/svg+xml";
  favicon.href = logoUrl;
  if (!existing) document.head.appendChild(favicon);
}

function byId<T extends HTMLElement = HTMLElement>(id: string): T {
  const element = document.getElementById(id);
  if (!element) throw new Error(`missing #${id}`);
  return element as T;
}

function enc(value: string): string {
  return encodeURIComponent(value || "");
}

function apiFetch(input: RequestInfo | URL, init: RequestInit = {}): Promise<Response> {
  return fetch(input, { ...init, credentials: "same-origin" });
}

function setStatus(text: string): void {
  byId("status").textContent = text;
}

function setSearching(searching: boolean): void {
  state.searching = searching;
  byId("search").toggleAttribute("disabled", searching);
  document.querySelector<HTMLButtonElement>("#load-more")?.toggleAttribute("disabled", searching);
  document.querySelector(".statusline")?.classList.toggle("searching", searching);
  byId("results").setAttribute("aria-busy", String(searching));
  updateCopyAvailability();
}

function pathText(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function workspaceLabel(ws: WorkspaceStatus): string {
  const root = pathText(ws.root);
  const parts = root.split(/[\\/]/).filter(Boolean);
  return parts.slice(-2).join("/") || root;
}

function workspaceMeta(ws: WorkspaceStatus): string {
  if (ws.root === ALL_WORKSPACES) return "Search every tracked index";
  const counts = `${ws.file_count || 0} files, ${ws.chunk_count || 0} chunks`;
  const watch = ws.watch_enabled ? (ws.watcher_alive ? "watch live" : "watch offline") : "watch off";
  let search = "hash";
  if (ws.enhancing_stalled) {
    search = "neural stalled";
  } else if (ws.enhancing_in_progress) {
    const progress = ws.enhancing_progress_count ? ` ${ws.enhancing_progress_count}/${ws.chunk_count || 0}` : "";
    search = `enhancing${progress}`;
  } else if (ws.has_neural_vectors) {
    search = `neural ${Number(ws.neural_coverage_percent || 0).toFixed(0)}%`;
  }
  let health = "healthy";
  if (ws.indexing_stalled) {
    health = "index stalled";
  } else if (ws.indexing_in_progress) {
    health = ws.indexing_progress || "indexing";
  } else if (ws.compaction?.healthy === false) {
    health = "compaction due";
  }
  return `${counts} | ${watch} | ${search} | ${health}`;
}

async function loadStatus(): Promise<void> {
  const response = await apiFetch("/api/status");
  const payload = await response.json() as { type?: string; error?: string; message?: string; version?: string; workspaces?: WorkspaceStatus[] };
  if (!response.ok || payload.error || payload.type === "error") {
    throw new Error(payload.error || payload.message || "status failed");
  }
  state.workspaces = payload.workspaces || [];
  resolveRequestedWorkspace();
  renderWorkspaces();
  updateScopeLabel();
  await loadTree(state.scope || ".");
  setStatus(`${state.workspaces.length} workspace(s), v${payload.version || boot.version || "dev"}`);
}

function resolveRequestedWorkspace(): void {
  if (!state.requestedWorkspace) return;
  const requested = state.requestedWorkspace;
  const match = state.workspaces.find((ws) => isWithin(requested, ws.root));
  if (!match) return;
  state.workspace = match.root;
  state.scope = relativeWithin(requested, match.root);
  state.requestedWorkspace = "";
}

function isWithin(path: string, root: string): boolean {
  return path === root || path.startsWith(`${root}/`) || path.startsWith(`${root}\\`);
}

function relativeWithin(path: string, root: string): string {
  return path.slice(root.length).replace(/^[/\\]+/, "");
}

function renderWorkspaces(): void {
  const root = byId("workspaces");
  const filter = state.workspaceFilter.trim().toLowerCase();
  root.innerHTML = "";
  root.appendChild(workspaceButton({ root: ALL_WORKSPACES, file_count: 0, chunk_count: 0 }, "All indices"));
  for (const ws of state.workspaces) {
    const label = workspaceLabel(ws);
    const haystack = `${label} ${ws.root} ${workspaceMeta(ws)}`.toLowerCase();
    if (!filter || haystack.includes(filter)) root.appendChild(workspaceButton(ws, label));
  }
  for (const button of root.querySelectorAll<HTMLButtonElement>(".workspace")) {
    const active = button.dataset.workspace === state.workspace;
    button.classList.toggle("active", active);
    button.setAttribute("aria-pressed", String(active));
  }
}

function workspaceButton(ws: WorkspaceStatus, label: string): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.className = "workspace";
  button.dataset.workspace = ws.root;
  button.setAttribute("aria-pressed", String(ws.root === state.workspace));
  button.innerHTML = `<div class="path">${escapeHtml(label)}</div><div class="meta">${escapeHtml(workspaceMeta(ws))}</div>`;
  button.addEventListener("click", () => {
    fileRequests.cancel();
    state.workspace = ws.root;
    state.scope = "";
    syncUrlState();
    renderWorkspaces();
    updateScopeLabel();
    void loadTree(".")
      .then(refreshSearchIfQuery)
      .catch((err: Error) => setStatus(err.message));
  });
  return button;
}

function shortPath(path: string): string {
  const parts = pathText(path).split(/[\\/]/).filter(Boolean);
  return parts.slice(-2).join("/") || path;
}

async function loadTree(path = "."): Promise<void> {
  const tree = byId("tree");
  if (state.workspace === ALL_WORKSPACES) {
    treeRequests.cancel();
    tree.innerHTML = '<div class="empty">Select one workspace to browse files.</div>';
    return;
  }
  const signal = treeRequests.start();
  try {
    const response = await apiFetch(
      `/api/tree?workspace=${enc(state.workspace)}&path=${enc(path || ".")}`,
      { signal }
    );
    const payload = await response.json() as TreePayload;
    if (!treeRequests.isCurrent(signal)) return;
    if (payload.error) throw new Error(payload.error);
    renderTree(payload);
  } catch (error) {
    if (treeRequests.isCurrent(signal)) throw error;
  }
}

function renderTree(payload: TreePayload): void {
  const tree = byId("tree");
  const current = pathText(payload.path);
  const rows: HTMLButtonElement[] = [];
  if (current) {
    rows.push(treeButton({ name: "Parent folder", path: parentPath(current), is_dir: true }, "parent"));
  }
  for (const entry of payload.entries || []) {
    const active = entry.is_dir ? entry.path === state.scope : samePath(entry.path, state.currentFilePath);
    rows.push(treeButton(entry, entry.is_dir ? "folder" : "file", active));
  }
  tree.innerHTML = "";
  if (!rows.length) {
    tree.innerHTML = '<div class="empty">Empty folder.</div>';
    return;
  }
  for (const row of rows) tree.appendChild(row);
}

function treeButton(entry: TreeEntry, kind: "parent" | "folder" | "file", active = false): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.className = `tree-row ${kind}${active ? " active" : ""}`;
  button.dataset.kind = kind;
  button.dataset.path = entry.path;
  if (active) button.setAttribute("aria-current", "true");
  const icon = kind === "parent" ? "up" : kind === "folder" ? "folder" : "file";
  const meta = kind === "folder" ? "folder" : kind === "parent" ? "up" : fileExtension(entry.name);
  button.innerHTML = `<span class="tree-icon">${svgIcon(icon)}</span><span class="tree-name">${escapeHtml(entry.name)}</span><span class="tree-meta">${escapeHtml(meta)}</span>`;
  button.addEventListener("click", () => {
    if (entry.is_dir) {
      fileRequests.cancel();
      state.scope = pathText(entry.path);
      updateScopeLabel();
      syncUrlState();
      void loadTree(state.scope || ".")
        .then(refreshSearchIfQuery)
        .catch((err: Error) => setStatus(err.message));
    } else {
      state.manualOpen = true;
      void openFilePath(entry.path, 1, 1, state.workspace, "").catch((err: Error) => setStatus(err.message));
    }
  });
  return button;
}

function fileExtension(name: string): string {
  const index = name.lastIndexOf(".");
  return index > 0 ? name.slice(index + 1) : "file";
}

function parentPath(path: string): string {
  const parts = pathText(path).split(/[\\/]/).filter(Boolean);
  parts.pop();
  return parts.join("/");
}

function updateScopeLabel(): void {
  const label = state.workspace === ALL_WORKSPACES || !state.scope ? "Scope: all folders" : `Scope: ${state.scope}`;
  byId("scope-label").textContent = label;
}

function runSearch(options: { preserveSelection?: boolean } = {}): void {
  const q = byId<HTMLTextAreaElement>("query").value.trim();
  if (!options.preserveSelection) fileRequests.cancel();
  if (state.events) {
    state.events.close();
    state.events = null;
  }
  if (!q) {
    state.hits = [];
    state.contextPack = null;
    state.currentHitKey = "";
    state.pinnedHitKeys.clear();
    state.searchErrors = [];
    setSearching(false);
    updateCopyAvailability();
    byId("summary").textContent = "Enter a query.";
    byId("results").innerHTML = '<div class="empty">Enter a query to search tracked workspaces.</div>';
    return;
  }
  if (byId<HTMLSelectElement>("mode").value === "context" && state.workspace === ALL_WORKSPACES) {
    state.hits = [];
    state.contextPack = null;
    state.pinnedHitKeys.clear();
    setSearching(false);
    updateCopyAvailability();
    byId("summary").textContent = "Select one workspace for a context pack.";
    byId("results").innerHTML = '<div class="empty">Context graphs and Git diffs belong to one workspace. Select it on the left.</div>';
    return;
  }
  syncUrlState();
  state.searchErrors = [];
  if (options.preserveSelection) {
    state.manualOpen = true;
    state.autoOpenKey = state.currentHitKey;
  } else {
    state.autoOpenKey = "";
    state.manualOpen = false;
    state.hits = [];
    state.contextPack = null;
    state.currentHitKey = "";
    state.pinnedHitKeys.clear();
    byId("results").innerHTML = "";
  }
  setSearching(true);
  byId("summary").textContent = "Searching...";
  const params = new URLSearchParams({
    q,
    workspace: state.workspace,
    mode: byId<HTMLSelectElement>("mode").value,
    limit: String(currentLimit()),
    type: byId<HTMLInputElement>("type").value || "",
    include: byId<HTMLInputElement>("include").value || "",
    exclude: byId<HTMLInputElement>("exclude").value || ""
  });
  if (byId<HTMLSelectElement>("mode").value === "context") {
    params.set("since", byId<HTMLInputElement>("since").value || "");
    params.set("budget_tokens", byId<HTMLInputElement>("budget").value || "8000");
  }
  if (state.workspace !== ALL_WORKSPACES && state.scope) params.set("scope", state.scope);
  const events = new EventSource(`/api/search/stream?${params.toString()}`, { withCredentials: true });
  state.events = events;
  events.addEventListener("status", guardCurrentStream(
    () => state.events,
    events,
    () => setStatus("Searching")
  ));
  events.addEventListener("results", guardCurrentStream(
    () => state.events,
    events,
    (event) => renderResults(JSON.parse((event as MessageEvent<string>).data) as SearchPayload)
  ));
  events.addEventListener("done", guardCurrentStream(
    () => state.events,
    events,
    (event) => {
      const payload = JSON.parse((event as MessageEvent<string>).data) as SearchDonePayload;
      events.close();
      state.events = null;
      setSearching(false);
      setStatus(searchCompletionStatus(payload.ok, state.searchErrors));
    }
  ));
  events.onerror = guardCurrentStream(
    () => state.events,
    events,
    () => {
      events.close();
      state.events = null;
      setSearching(false);
      setStatus("Search connection closed");
    }
  );
}

function refreshSearchIfQuery(): void {
  if (byId<HTMLTextAreaElement>("query").value.trim()) runSearch();
}

function renderResults(payload: SearchPayload): void {
  if (payload.error) {
    state.searchErrors = [payload.error];
    state.hits = [];
    state.contextPack = null;
    state.currentHitKey = "";
    setSearching(false);
    byId("summary").textContent = payload.error;
    byId("results").innerHTML = `<div class="empty">${escapeHtml(payload.error)}</div>`;
    return;
  }
  state.searchErrors = payload.errors || [];
  if (payload.context_pack) {
    renderContextPack(payload.context_pack, Number(payload.elapsed_ms || 0));
    return;
  }
  const hits = payload.hits || [];
  state.contextPack = null;
  state.hits = hits;
  pruneResultKeySets();
  updateCopyAvailability();
  const errorSummary = state.searchErrors.length
    ? `, ${state.searchErrors.length} workspace error(s): ${state.searchErrors.join("; ")}`
    : "";
  byId("summary").textContent = `${hits.length} hit(s) in ${Number(payload.elapsed_ms || 0).toFixed(1)} ms${errorSummary}`;
  if (!hits.length) {
    state.currentHitKey = "";
    byId("results").innerHTML = '<div class="empty">No hits. Try a broader query, fewer filters, or a different search mode.</div>';
    updateResultRows();
    return;
  }
  const root = byId("results");
  root.innerHTML = "";
  for (const hit of hits) {
    const item = document.createElement("article");
    const key = hitKey(hit);
    item.className = "hit";
    item.dataset.hitKey = key;
    item.classList.toggle("active", key === state.currentHitKey);
    item.classList.toggle("pinned", state.pinnedHitKeys.has(key));
    item.innerHTML = `
      <div class="hit-head">
        <button class="hit-main" type="button">
          <div class="file">${languageIconForPath(hit.file_path)}<span class="file-path">${escapeHtml(hit.file_path)}</span></div>
          <div class="score">score ${Number(hit.score).toFixed(3)}${sourceBadges(hit.sources || [])}</div>
        </button>
        <div class="hit-actions">
          <button class="small-action pin-hit${state.pinnedHitKeys.has(key) ? " active" : ""}" type="button" aria-pressed="${state.pinnedHitKeys.has(key)}">${state.pinnedHitKeys.has(key) ? "Pinned" : "Pin"}</button>
          <button class="small-action open-hit" type="button">Open</button>
        </div>
      </div>
      <pre class="snippet hljs">${renderSnippet(hit)}</pre>`;
    const hitMain = item.querySelector<HTMLButtonElement>(".hit-main");
    if (key === state.currentHitKey) hitMain?.setAttribute("aria-current", "true");
    hitMain?.addEventListener("click", () => selectHit(hit));
    item.querySelector(".pin-hit")?.addEventListener("click", () => togglePinnedHit(key));
    item.querySelector(".open-hit")?.addEventListener("click", () => {
      void openHitInEditor(hit).catch((err: Error) => setStatus(err.message));
    });
    root.appendChild(item);
  }
  if (canLoadMore(hits.length)) root.appendChild(loadMoreButton(hits.length));
  const first = hits[0];
  const firstKey = hitKey(first);
  if (!state.manualOpen && state.autoOpenKey !== firstKey) {
    state.autoOpenKey = firstKey;
    void openHit(first).catch((err: Error) => setStatus(err.message));
  }
}

function renderContextPack(bundle: ContextBundle, elapsedMs: number): void {
  const hits = bundle.items.map((item) => ({
    file_path: item.file_path,
    start_line: item.start_line,
    end_line: item.end_line,
    score: 0,
    preview: item.preview,
    sources: item.sources
  }));
  state.hits = hits;
  state.contextPack = bundle;
  pruneResultKeySets();
  updateCopyAvailability();
  const coverage = bundle.coverage;
  const changeText = bundle.change_scope
    ? `, ${bundle.change_scope.total_changes} changed${bundle.change_scope.dirty_worktree ? " including worktree" : ""}`
    : "";
  byId("summary").textContent = `${coverage.files} files, ${bundle.used_tokens}/${bundle.budget_tokens} tokens${changeText}, ${elapsedMs.toFixed(1)} ms`;
  const root = byId("results");
  root.innerHTML = "";
  const overview = document.createElement("section");
  overview.className = "context-overview";
  const coverageText = [
    ["primary", coverage.primary],
    ["dependencies", coverage.dependencies],
    ["dependents", coverage.dependents],
    ["callers", coverage.callers],
    ["tests", coverage.tests],
    ["config", coverage.config],
    ["docs", coverage.documentation]
  ].filter(([, count]) => Number(count) > 0).map(([label, count]) => `${count} ${label}`).join(" | ");
  const changes = bundle.change_scope?.changes.slice(0, 12).map((change) =>
    `<li><strong>${escapeHtml(change.status)}</strong> ${escapeHtml(change.file_path)} <span>${escapeHtml(change.sources.join(", "))}</span></li>`
  ).join("") || "";
  overview.innerHTML = `
    <div class="context-title">Structured context pack</div>
    <div>${escapeHtml(coverageText || "No relationship coverage")}${bundle.truncated ? " | truncated to budget" : ""}</div>
    ${bundle.anchor_symbols.length ? `<div>Anchors: ${escapeHtml(bundle.anchor_symbols.join(", "))}</div>` : ""}
    ${changes ? `<ul class="context-changes">${changes}</ul>` : ""}`;
  root.appendChild(overview);

  for (let index = 0; index < bundle.items.length; index += 1) {
    const item = bundle.items[index];
    const hit = hits[index];
    const key = hitKey(hit);
    const article = document.createElement("article");
    article.className = "hit context-item";
    article.dataset.hitKey = key;
    article.innerHTML = `
      <div class="hit-head">
        <button class="hit-main" type="button">
          <div class="file">${languageIconForPath(item.file_path)}<span class="file-path">${pathWithBreaks(item.file_path)}<span class="context-location">:${item.start_line}-${item.end_line}</span></span></div>
          <div class="context-roles">${item.roles.map((role) => `<span>${escapeHtml(role)}</span>`).join("")}</div>
        </button>
        <div class="hit-actions">
          <button class="small-action pin-hit${state.pinnedHitKeys.has(key) ? " active" : ""}" type="button" aria-pressed="${state.pinnedHitKeys.has(key)}">${state.pinnedHitKeys.has(key) ? "Pinned" : "Pin"}</button>
          <button class="small-action open-hit" type="button">Open</button>
        </div>
      </div>
      ${item.reasons.length ? `<div class="context-why"><strong>Why:</strong> ${escapeHtml(item.reasons.join("; "))}</div>` : ""}
      <pre class="snippet hljs">${renderSnippet(hit)}</pre>`;
    article.querySelector(".hit-main")?.addEventListener("click", () => selectHit(hit));
    article.querySelector(".pin-hit")?.addEventListener("click", () => togglePinnedHit(key));
    article.querySelector(".open-hit")?.addEventListener("click", () => {
      void openHitInEditor(hit).catch((err: Error) => setStatus(err.message));
    });
    root.appendChild(article);
  }
  if (!hits.length) {
    root.insertAdjacentHTML("beforeend", '<div class="empty">No context evidence found.</div>');
  } else if (!state.manualOpen) {
    void openHit(hits[0]).catch((err: Error) => setStatus(err.message));
  }
}

function selectHit(hit: SearchHit): void {
  state.manualOpen = true;
  void openHit(hit).catch((err: Error) => setStatus(err.message));
}

function selectHitByIndex(index: number): void {
  if (!state.hits.length) return;
  const clamped = Math.max(0, Math.min(index, state.hits.length - 1));
  state.manualOpen = true;
  void openHit(state.hits[clamped]).catch((err: Error) => setStatus(err.message));
}

function loadMoreResults(): void {
  const input = byId<HTMLInputElement>("limit");
  input.value = String(Math.min(currentLimit() + LOAD_MORE_STEP, MAX_LIMIT));
  syncUrlState();
  runSearch({ preserveSelection: true });
}

function canLoadMore(hitCount: number): boolean {
  const limit = currentLimit();
  return hitCount >= limit && limit < MAX_LIMIT;
}

function loadMoreButton(hitCount: number): HTMLDivElement {
  const footer = document.createElement("div");
  footer.className = "load-more-wrap";
  const button = document.createElement("button");
  button.id = "load-more";
  button.className = "btn secondary load-more";
  button.type = "button";
  button.textContent = `Load ${Math.min(LOAD_MORE_STEP, MAX_LIMIT - currentLimit())} more`;
  button.title = `Showing ${hitCount} snippets`;
  button.addEventListener("click", loadMoreResults);
  footer.appendChild(button);
  return footer;
}

function pruneResultKeySets(): void {
  const keys = new Set(state.hits.map(hitKey));
  state.pinnedHitKeys = new Set([...state.pinnedHitKeys].filter((key) => keys.has(key)));
}

function togglePinnedHit(key: string): void {
  if (state.pinnedHitKeys.has(key)) {
    state.pinnedHitKeys.delete(key);
  } else {
    state.pinnedHitKeys.add(key);
  }
  updateResultRows();
  updateCopyAvailability();
}

function updateCopyAvailability(): void {
  const button = document.querySelector<HTMLButtonElement>("#copy-results");
  if (button) button.disabled = state.searching || copyHits(currentCopyScope()).length === 0;
}

async function copyVisibleResults(): Promise<void> {
  const scope = currentCopyScope();
  const hits = copyHits(scope);
  const text = clipboardText(byId<HTMLSelectElement>("copy-format").value as CopyFormat, hits);
  if (!text) {
    setStatus(`No ${scope} results to copy`);
    return;
  }
  await writeClipboard(text);
  setStatus(`Copied ${hits.length} result(s)`);
}

function currentCopyScope(): CopyScope {
  const value = document.querySelector<HTMLSelectElement>("#copy-scope")?.value;
  return value === "pinned" ? "pinned" : "visible";
}

function copyHits(scope: CopyScope): SearchHit[] {
  if (scope === "pinned") return state.hits.filter((hit) => state.pinnedHitKeys.has(hitKey(hit)));
  return state.hits;
}

function clipboardText(format: CopyFormat, hits: SearchHit[]): string {
  if (!hits.length) return "";
  if (format === "files") return uniqueFilePaths(hits).join("\n");
  if (format === "json" && state.contextPack && currentCopyScope() === "visible") {
    return JSON.stringify(state.contextPack, null, 2);
  }
  if (format === "json") return JSON.stringify(searchExport(hits), null, 2);
  return hits.map(rawHitText).join("\n\n");
}

function uniqueFilePaths(hits: SearchHit[]): string[] {
  return Array.from(new Set(hits.map((hit) => hit.file_path)));
}

function searchExport(hits: SearchHit[]): Record<string, unknown> {
  return {
    query: byId<HTMLInputElement>("query").value.trim(),
    workspace: state.workspace,
    scope: state.scope || null,
    mode: byId<HTMLSelectElement>("mode").value,
    limit: currentLimit(),
    count: hits.length,
    hits: hits.map((hit) => ({
      file_path: hit.file_path,
      start_line: hit.start_line,
      end_line: hit.end_line,
      score: hit.score,
      sources: hit.sources || [],
      preview: hit.preview || ""
    }))
  };
}

function rawHitText(hit: SearchHit): string {
  const range = hit.start_line === hit.end_line ? `${hit.start_line}` : `${hit.start_line}-${hit.end_line}`;
  const sources = hit.sources?.length ? ` [${hit.sources.join(", ")}]` : "";
  const preview = (hit.preview || "").trimEnd();
  return `${hit.file_path}:${range}\nscore ${Number(hit.score).toFixed(3)}${sources}${preview ? `\n${preview}` : ""}`;
}

async function writeClipboard(text: string): Promise<void> {
  if (navigator.clipboard?.writeText && window.isSecureContext) {
    await navigator.clipboard.writeText(text);
    return;
  }
  const textArea = document.createElement("textarea");
  textArea.value = text;
  textArea.setAttribute("readonly", "");
  textArea.style.position = "fixed";
  textArea.style.opacity = "0";
  document.body.appendChild(textArea);
  textArea.select();
  const copied = document.execCommand("copy");
  textArea.remove();
  if (!copied) throw new Error("clipboard write failed");
}

async function openHit(hit: SearchHit): Promise<void> {
  const workspace = state.workspace === ALL_WORKSPACES ? "" : state.workspace;
  await openFilePath(hit.file_path, hit.start_line, hit.end_line, workspace, hitKey(hit));
}

async function openHitInEditor(hit: SearchHit): Promise<void> {
  const workspace = state.workspace === ALL_WORKSPACES ? "" : state.workspace;
  await openPathInEditor(hit.file_path, hit.start_line, workspace);
}

async function openCurrentInEditor(): Promise<void> {
  const path = state.currentFileAbsolutePath || state.currentFilePath;
  if (!path) {
    setStatus("No file selected");
    return;
  }
  await openPathInEditor(path, state.currentFileStart, "");
}

async function openPathInEditor(path: string, line: number, workspace: string): Promise<void> {
  const params = new URLSearchParams({ path, line: String(Math.max(1, line || 1)) });
  if (workspace && workspace !== ALL_WORKSPACES) params.set("workspace", workspace);
  const response = await apiFetch(`/api/open?${params.toString()}`, { method: "POST" });
  const payload = await response.json() as { ok?: boolean; error?: string; program?: string };
  if (payload.error || !payload.ok) throw new Error(payload.error || "open failed");
  setStatus(`Opened ${shortPath(path)}`);
}

async function openFilePath(path: string, start = 1, end = 1, workspace = state.workspace, hitKeyValue = ""): Promise<void> {
  const signal = fileRequests.start();
  try {
    const response = await apiFetch(`/api/file?workspace=${enc(workspace)}&path=${enc(path)}`, { signal });
    const payload = await response.json() as FilePayload;
    if (!fileRequests.isCurrent(signal)) return;
    if (payload.error) throw new Error(payload.error);
    state.currentFilePath = pathText(payload.path);
    state.currentFileAbsolutePath = pathText(payload.absolute_path);
    state.currentFileText = payload.text || "";
    state.currentFileStart = start;
    state.currentFileEnd = end;
    state.currentHitKey = hitKeyValue;
    state.viewerMode = isMarkdownPath(state.currentFilePath) ? "preview" : "source";
    byId("viewer-title").textContent = payload.path;
    byId("viewer-meta").textContent = `${payload.line_count} lines${payload.truncated ? ", truncated" : ""}`;
    renderViewerFile();
    updateResultRows();
  } catch (error) {
    if (fileRequests.isCurrent(signal)) throw error;
  }
}

function renderViewerFile(): void {
  const actions = byId<HTMLDivElement>("viewer-actions");
  byId<HTMLButtonElement>("open-current").disabled = !state.currentFilePath;
  const markdownFile = isMarkdownPath(state.currentFilePath);
  actions.hidden = !markdownFile;
  byId("preview-mode").classList.toggle("active", markdownFile && state.viewerMode === "preview");
  byId("source-mode").classList.toggle("active", !markdownFile || state.viewerMode === "source");
  byId("preview-mode").setAttribute("aria-pressed", String(markdownFile && state.viewerMode === "preview"));
  byId("source-mode").setAttribute("aria-pressed", String(!markdownFile || state.viewerMode === "source"));
  if (markdownFile && state.viewerMode === "preview") {
    byId("file-view").innerHTML = `<article class="markdown-preview">${markdown.render(state.currentFileText)}</article>`;
    return;
  }
  renderCode(state.currentFileText, state.currentFileStart, state.currentFileEnd, state.currentFilePath);
}

function renderCode(text: string, start: number, end: number, path: string): void {
  const language = languageForPath(path);
  const lines = text.split(/\r?\n/);
  byId("file-view").innerHTML = `<pre class="code">${lines.map((line, index) => {
    const number = index + 1;
    const focus = number >= start && number <= end ? " focus" : "";
    return `<span class="line${focus}"><span class="line-number" aria-hidden="true">${number}</span><span><span class="sr-only">Line ${number}: </span>${highlightLine(line, language) || " "}</span></span>`;
  }).join("")}</pre>`;
  scrollFocusedLine();
}

function renderSnippet(hit: SearchHit): string {
  const language = languageForPath(hit.file_path);
  return (hit.preview || "").split(/\r?\n/).map((line, index) => {
    const number = hit.start_line + index;
    return `<span class="snippet-line"><span class="snippet-line-number" aria-hidden="true">${number}</span><span><span class="sr-only">Line ${number}: </span>${markQueryTerms(highlightLine(line, language) || " ")}</span></span>`;
  }).join("");
}

function markQueryTerms(html: string): string {
  const terms = queryTerms();
  if (!terms.length) return html;
  const pattern = new RegExp(`(${terms.map(escapeRegExp).join("|")})`, "gi");
  return html
    .split(/(<[^>]+>)/g)
    .map((part) => part.startsWith("<") ? part : part.replace(pattern, '<mark class="query-mark">$1</mark>'))
    .join("");
}

function queryTerms(): string[] {
  const query = byId<HTMLTextAreaElement>("query").value.trim();
  if (!query) return [];
  const terms = query.match(/[A-Za-z0-9_.$:/-]{2,}/g) || (query.length <= 32 ? [query] : []);
  return Array.from(new Set(terms.map((term) => term.toLowerCase()))).slice(0, 8);
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function sourceBadges(sources: string[]): string {
  return sources.map((source) => `<span class="source-badge">${escapeHtml(source)}</span>`).join("");
}

function hitKey(hit: SearchHit): string {
  return `${normalizePath(hit.file_path)}:${hit.start_line}:${hit.end_line}`;
}

function normalizePath(path: string): string {
  return pathText(path).replace(/\\/g, "/").replace(/^\.\//, "");
}

function samePath(left: string, right: string): boolean {
  return Boolean(left && right) && normalizePath(left) === normalizePath(right);
}

function updateResultRows(): void {
  for (const hit of document.querySelectorAll<HTMLElement>(".hit")) {
    const key = hit.dataset.hitKey || "";
    const pinned = state.pinnedHitKeys.has(key);
    hit.classList.toggle("active", key === state.currentHitKey);
    hit.classList.toggle("pinned", pinned);
    const pin = hit.querySelector<HTMLButtonElement>(".pin-hit");
    if (pin) {
      pin.classList.toggle("active", pinned);
      pin.textContent = pinned ? "Pinned" : "Pin";
      pin.setAttribute("aria-pressed", String(pinned));
    }
    const hitMain = hit.querySelector<HTMLButtonElement>(".hit-main");
    if (key === state.currentHitKey) {
      hitMain?.setAttribute("aria-current", "true");
    } else {
      hitMain?.removeAttribute("aria-current");
    }
  }
  for (const row of document.querySelectorAll<HTMLButtonElement>(".tree-row")) {
    const kind = row.dataset.kind;
    const path = row.dataset.path || "";
    const active = kind === "file" ? samePath(path, state.currentFilePath) : kind === "folder" && path === state.scope;
    row.classList.toggle("active", active);
    if (active) {
      row.setAttribute("aria-current", "true");
    } else {
      row.removeAttribute("aria-current");
    }
  }
  document.querySelector(".hit.active")?.scrollIntoView({ block: "nearest" });
}

function scrollFocusedLine(): void {
  requestAnimationFrame(() => {
    document.querySelector(".line.focus")?.scrollIntoView({ block: "center", inline: "nearest" });
  });
}

function highlightLine(line: string, language?: string): string {
  return language ? highlightText(line, language) : escapeHtml(line);
}

function highlightText(text: string, language?: string): string {
  const normalized = normalizeLanguage(language);
  if (normalized && hljs.getLanguage(normalized)) {
    try {
      return hljs.highlight(text, { language: normalized, ignoreIllegals: true }).value;
    } catch {
      return escapeHtml(text);
    }
  }
  return escapeHtml(text);
}

function isMarkdownPath(path: string): boolean {
  return /\.(md|markdown|mdown|mkd)$/i.test(path);
}

function languageIconForPath(path: string): string {
  const icon = languageIconSpec(path);
  return `<span class="language-icon" title="${escapeHtml(icon.title)}" aria-hidden="true">${icon.svg}</span>`;
}

function languageIconSpec(path: string): { title: string; svg: string } {
  const name = path.split(/[\\/]/).pop()?.toLowerCase() || "";
  const ext = name.split(".").pop() || "";
  if (name === "dockerfile" || name.endsWith(".dockerfile")) return { title: "Dockerfile", svg: squareIcon("D", "#0db7ed", "#ffffff") };
  if (name === "makefile") return { title: "Makefile", svg: terminalIcon("#64748b") };
  switch (ext) {
    case "html":
    case "htm":
      return { title: "HTML", svg: shieldIcon("5", "#e34f26") };
    case "css":
      return { title: "CSS", svg: shieldIcon("3", "#1572b6") };
    case "js":
    case "jsx":
    case "mjs":
      return { title: "JavaScript", svg: squareIcon("JS", "#f7df1e", "#111827") };
    case "ts":
    case "tsx":
      return { title: "TypeScript", svg: squareIcon("TS", "#3178c6", "#ffffff") };
    case "java":
      return { title: "Java", svg: javaIcon() };
    case "py":
      return { title: "Python", svg: pythonIcon() };
    case "rs":
      return { title: "Rust", svg: circleIcon("R", "#b7410e", "#ffffff") };
    case "go":
      return { title: "Go", svg: circleIcon("Go", "#00add8", "#ffffff") };
    case "rb":
      return { title: "Ruby", svg: diamondIcon("#cc342d") };
    case "swift":
      return { title: "Swift", svg: birdIcon("#f05138") };
    case "kt":
      return { title: "Kotlin", svg: squareIcon("K", "#7f52ff", "#ffffff") };
    case "c":
    case "h":
      return { title: "C", svg: hexIcon("C", "#5c6bc0") };
    case "cc":
    case "cpp":
    case "cxx":
    case "hpp":
    case "hh":
    case "hxx":
      return { title: "C++", svg: hexIcon("C++", "#00599c") };
    case "cs":
      return { title: "C#", svg: hexIcon("C#", "#68217a") };
    case "json":
      return { title: "JSON", svg: bracesIcon("#f59e0b") };
    case "md":
    case "markdown":
    case "mdown":
    case "mkd":
      return { title: "Markdown", svg: markdownIcon() };
    case "sh":
    case "bash":
    case "zsh":
      return { title: "Shell", svg: terminalIcon("#4b5563") };
    case "sql":
      return { title: "SQL", svg: databaseIcon("#336791") };
    case "yaml":
    case "yml":
      return { title: "YAML", svg: squareIcon("Y", "#cb171e", "#ffffff") };
    case "toml":
      return { title: "TOML", svg: squareIcon("T", "#9c4221", "#ffffff") };
    case "xml":
      return { title: "XML", svg: angleIcon("#f97316") };
    case "lua":
      return { title: "Lua", svg: circleIcon("Lua", "#000080", "#ffffff") };
    default:
      return { title: "File", svg: fileIcon() };
  }
}

function squareIcon(label: string, fill: string, color: string): string {
  return `<svg viewBox="0 0 24 24" role="img"><rect x="3" y="3" width="18" height="18" rx="2.5" fill="${fill}"/><text x="12" y="15.6" text-anchor="middle" font-size="${label.length > 2 ? 6 : 7}" font-weight="800" fill="${color}">${escapeHtml(label)}</text></svg>`;
}

function circleIcon(label: string, fill: string, color: string): string {
  return `<svg viewBox="0 0 24 24" role="img"><circle cx="12" cy="12" r="9" fill="${fill}"/><text x="12" y="15.2" text-anchor="middle" font-size="${label.length > 2 ? 5.5 : 7}" font-weight="800" fill="${color}">${escapeHtml(label)}</text></svg>`;
}

function shieldIcon(label: string, fill: string): string {
  return `<svg viewBox="0 0 24 24" role="img"><path d="M5 3h14l-1.3 15.2L12 21l-5.7-2.8L5 3Z" fill="${fill}"/><text x="12" y="15.4" text-anchor="middle" font-size="8" font-weight="900" fill="#fff">${escapeHtml(label)}</text></svg>`;
}

function hexIcon(label: string, fill: string): string {
  return `<svg viewBox="0 0 24 24" role="img"><path d="M12 2.5 20.2 7v10L12 21.5 3.8 17V7L12 2.5Z" fill="${fill}"/><text x="12" y="15.2" text-anchor="middle" font-size="${label.length > 1 ? 6 : 8}" font-weight="800" fill="#fff">${escapeHtml(label)}</text></svg>`;
}

function javaIcon(): string {
  return `<svg viewBox="0 0 24 24" role="img"><path d="M9 17h7c0 2-1.6 3.5-3.5 3.5S9 19 9 17Z" fill="#e76f00"/><path d="M8 14h9v2H8z" fill="#5382a1"/><path d="M10 4c2 1.7-2 2.6.4 4.6M14 3c2.2 2-2.6 3.2.2 5.6" fill="none" stroke="#e76f00" stroke-width="1.6" stroke-linecap="round"/></svg>`;
}

function pythonIcon(): string {
  return `<svg viewBox="0 0 24 24" role="img"><path d="M12 3h4a3 3 0 0 1 3 3v4h-7a3 3 0 0 0-3 3v1H5V8a3 3 0 0 1 3-3h4V3Z" fill="#3776ab"/><path d="M12 21H8a3 3 0 0 1-3-3v-4h7a3 3 0 0 0 3-3v-1h4v6a3 3 0 0 1-3 3h-4v2Z" fill="#ffd43b"/><circle cx="9" cy="7" r="1" fill="#fff"/><circle cx="15" cy="17" r="1" fill="#111827"/></svg>`;
}

function diamondIcon(fill: string): string {
  return `<svg viewBox="0 0 24 24" role="img"><path d="M12 3 21 9l-9 12L3 9l9-6Z" fill="${fill}"/><path d="M7 9h10l-5 7-5-7Z" fill="#fff" opacity=".35"/></svg>`;
}

function birdIcon(fill: string): string {
  return `<svg viewBox="0 0 24 24" role="img"><path d="M4 5c5.5 5.3 9.2 8.2 14 9.7-2.2.9-4.6 1-7 .2 1.8 1.6 3.7 2.9 6.5 4.1-5.6.5-10-2-13.5-7.5 3.1 1.7 5.4 2.2 7.2 1.8C8.5 10.7 6.2 8.2 4 5Z" fill="${fill}"/></svg>`;
}

function bracesIcon(fill: string): string {
  return `<svg viewBox="0 0 24 24" role="img"><rect x="3" y="3" width="18" height="18" rx="3" fill="${fill}"/><text x="12" y="15.4" text-anchor="middle" font-size="8" font-weight="900" fill="#111827">{}</text></svg>`;
}

function markdownIcon(): string {
  return `<svg viewBox="0 0 24 24" role="img"><rect x="3" y="5" width="18" height="14" rx="2" fill="#083344"/><path d="M6 15V9l2.7 3L11.4 9v6M15 9v6m-2.2-2.2L15 15l2.2-2.2" fill="none" stroke="#fff" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
}

function terminalIcon(fill: string): string {
  return `<svg viewBox="0 0 24 24" role="img"><rect x="3" y="4" width="18" height="16" rx="3" fill="${fill}"/><path d="m7 9 3 3-3 3M12 15h5" fill="none" stroke="#fff" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
}

function databaseIcon(fill: string): string {
  return `<svg viewBox="0 0 24 24" role="img"><ellipse cx="12" cy="6" rx="7" ry="3" fill="${fill}"/><path d="M5 6v10c0 1.7 3.1 3 7 3s7-1.3 7-3V6" fill="${fill}"/><path d="M5 11c0 1.7 3.1 3 7 3s7-1.3 7-3" fill="none" stroke="#fff" stroke-opacity=".55" stroke-width="1.4"/></svg>`;
}

function angleIcon(fill: string): string {
  return `<svg viewBox="0 0 24 24" role="img"><rect x="3" y="3" width="18" height="18" rx="3" fill="${fill}"/><path d="m10 8-4 4 4 4M14 8l4 4-4 4" fill="none" stroke="#fff" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
}

function fileIcon(): string {
  return `<svg viewBox="0 0 24 24" role="img"><path d="M7 3h7l5 5v13H7V3Z" fill="none" stroke="#64748b" stroke-width="2" stroke-linejoin="round"/><path d="M14 3v6h5" fill="none" stroke="#64748b" stroke-width="2" stroke-linejoin="round"/></svg>`;
}

function languageForPath(path: string): string | undefined {
  const name = path.split(/[\\/]/).pop()?.toLowerCase() || "";
  if (name === "dockerfile" || name.endsWith(".dockerfile")) return "dockerfile";
  if (name === "makefile") return "makefile";
  const ext = name.split(".").pop() || "";
  const languages: Record<string, string> = {
    bash: "bash",
    c: "c",
    cc: "cpp",
    cpp: "cpp",
    cs: "csharp",
    css: "css",
    go: "go",
    h: "c",
    hpp: "cpp",
    html: "xml",
    java: "java",
    js: "javascript",
    json: "json",
    jsx: "javascript",
    kt: "kotlin",
    lua: "lua",
    md: "markdown",
    mjs: "javascript",
    py: "python",
    rb: "ruby",
    rs: "rust",
    sh: "bash",
    sql: "sql",
    swift: "swift",
    toml: "ini",
    ts: "typescript",
    tsx: "typescript",
    xml: "xml",
    yaml: "yaml",
    yml: "yaml"
  };
  return languages[ext];
}

function normalizeLanguage(language?: string): string | undefined {
  if (!language) return undefined;
  const cleaned = language.trim().toLowerCase();
  const aliases: Record<string, string> = {
    cplusplus: "cpp",
    csharp: "csharp",
    js: "javascript",
    md: "markdown",
    py: "python",
    rs: "rust",
    shell: "bash",
    ts: "typescript"
  };
  return aliases[cleaned] || cleaned;
}

function debounce(fn: () => void, delayMs: number): () => void {
  let timer: number | undefined;
  return () => {
    window.clearTimeout(timer);
    timer = window.setTimeout(fn, delayMs);
  };
}

function applyInitialParams(): void {
  byId<HTMLTextAreaElement>("query").value = queryParams.get("q") || boot.query || "";
  byId<HTMLSelectElement>("mode").value = queryParams.get("mode") || "hybrid";
  byId<HTMLInputElement>("limit").value = queryParams.get("limit") || String(DEFAULT_LIMIT);
  byId<HTMLInputElement>("type").value = queryParams.get("type") || "";
  byId<HTMLInputElement>("include").value = queryParams.get("include") || "";
  byId<HTMLInputElement>("exclude").value = queryParams.get("exclude") || "";
  byId<HTMLInputElement>("since").value = queryParams.get("since") || "";
  byId<HTMLInputElement>("budget").value = queryParams.get("budget") || "8000";
  updateContextControls();
}

function updateContextControls(): void {
  const contextMode = byId<HTMLSelectElement>("mode").value === "context";
  byId("since-field").hidden = !contextMode;
  byId("budget-field").hidden = !contextMode;
  byId<HTMLTextAreaElement>("query").placeholder = contextMode
    ? "Describe task, paste issue, or paste stack trace"
    : "Search code, symbols, paths";
}

function currentLimit(): number {
  const parsed = Number.parseInt(byId<HTMLInputElement>("limit").value, 10);
  if (!Number.isFinite(parsed)) return DEFAULT_LIMIT;
  return Math.max(1, Math.min(parsed, MAX_LIMIT));
}

function syncUrlState(): void {
  const params = new URLSearchParams();
  const query = byId<HTMLTextAreaElement>("query").value.trim();
  const mode = byId<HTMLSelectElement>("mode").value;
  const limit = currentLimit();
  const type = byId<HTMLInputElement>("type").value.trim();
  const include = byId<HTMLInputElement>("include").value.trim();
  const exclude = byId<HTMLInputElement>("exclude").value.trim();
  const since = byId<HTMLInputElement>("since").value.trim();
  const budget = byId<HTMLInputElement>("budget").value.trim();
  if (query) params.set("q", query);
  if (state.workspace !== ALL_WORKSPACES) params.set("workspace", scopedWorkspacePath());
  if (mode !== "hybrid") params.set("mode", mode);
  if (limit !== DEFAULT_LIMIT) params.set("limit", String(limit));
  if (type) params.set("type", type);
  if (include) params.set("include", include);
  if (exclude) params.set("exclude", exclude);
  if (mode === "context" && since) params.set("since", since);
  if (mode === "context" && budget !== "8000") params.set("budget", budget);
  const search = params.toString();
  history.replaceState(null, "", `${location.pathname}${search ? `?${search}` : ""}`);
}

function scopedWorkspacePath(): string {
  if (!state.scope) return state.workspace;
  return `${state.workspace.replace(/[/\\]+$/, "")}/${state.scope.replace(/^[/\\]+/, "")}`;
}

function handleGlobalKeyDown(event: KeyboardEvent): void {
  const target = event.target;
  const editing = target instanceof Element && Boolean(target.closest("input, textarea, select, button, a, [contenteditable='true']"));
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
    event.preventDefault();
    const query = byId<HTMLTextAreaElement>("query");
    query.focus();
    query.select();
    return;
  }
  if (!editing && event.key === "/") {
    event.preventDefault();
    byId<HTMLTextAreaElement>("query").focus();
    return;
  }
  if (editing || event.altKey || event.metaKey || event.ctrlKey) return;
  const activeIndex = state.hits.findIndex((hit) => hitKey(hit) === state.currentHitKey);
  if (event.key === "ArrowDown" || event.key.toLowerCase() === "j") {
    event.preventDefault();
    selectHitByIndex(activeIndex < 0 ? 0 : activeIndex + 1);
  } else if (event.key === "ArrowUp" || event.key.toLowerCase() === "k") {
    event.preventDefault();
    selectHitByIndex(activeIndex < 0 ? 0 : activeIndex - 1);
  }
}

function escapeHtml(value: unknown): string {
  return String(value).replace(/[&<>"']/g, (ch) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[ch] || ch);
}

function pathWithBreaks(value: string): string {
  return escapeHtml(value).replaceAll("/", "/<wbr>").replaceAll("\\", "\\<wbr>");
}

function attachEvents(): void {
  byId("search").addEventListener("click", () => runSearch());
  byId("copy-results").addEventListener("click", () => {
    void copyVisibleResults().catch((err: Error) => setStatus(err.message));
  });
  byId("copy-scope").addEventListener("change", updateCopyAvailability);
  byId("refresh").addEventListener("click", () => void loadStatus().catch((err: Error) => setStatus(err.message)));
  byId("open-current").addEventListener("click", () => {
    void openCurrentInEditor().catch((err: Error) => setStatus(err.message));
  });
  byId("clear-scope").addEventListener("click", () => {
    fileRequests.cancel();
    state.scope = "";
    updateScopeLabel();
    syncUrlState();
    void loadTree(".")
      .then(refreshSearchIfQuery)
      .catch((err: Error) => setStatus(err.message));
  });
  byId("preview-mode").addEventListener("click", () => {
    state.viewerMode = "preview";
    renderViewerFile();
  });
  byId("source-mode").addEventListener("click", () => {
    state.viewerMode = "source";
    renderViewerFile();
  });
  byId<HTMLTextAreaElement>("query").addEventListener("keydown", (event) => {
    const contextMode = byId<HTMLSelectElement>("mode").value === "context";
    if (event.key === "Enter" && (!contextMode || event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      runSearch();
    }
  });
  byId<HTMLInputElement>("workspace-filter").addEventListener("input", (event) => {
    state.workspaceFilter = (event.target as HTMLInputElement).value;
    renderWorkspaces();
  });
  const debouncedRefresh = debounce(refreshSearchIfQuery, 220);
  for (const id of ["type", "limit", "include", "exclude", "since", "budget"]) {
    byId<HTMLInputElement>(id).addEventListener("input", () => {
      syncUrlState();
      debouncedRefresh();
    });
  }
  byId<HTMLSelectElement>("mode").addEventListener("change", () => {
    updateContextControls();
    syncUrlState();
    refreshSearchIfQuery();
  });
  document.addEventListener("keydown", handleGlobalKeyDown);
}

installFavicon();
renderShell();
attachEvents();
applyInitialParams();
void loadStatus()
  .then(() => {
    if (byId<HTMLTextAreaElement>("query").value.trim()) runSearch();
  })
  .catch((err: Error) => setStatus(err.message));
