import "highlight.js/styles/github-dark.css";
import { connectSearchStream, fetchFile, fetchStatus, fetchTree, openPathInEditor as openPathInEditorApi } from "./api-client";
import {
  ALL_WORKSPACES,
  DEFAULT_LIMIT,
  LOAD_MORE_STEP,
  MAX_LIMIT,
  createInitialState,
  readBoot
} from "./app-types";
import type {
  ContextBundle,
  CopyFormat,
  CopyScope,
  SearchHit,
  SearchPayload,
  TreeEntry,
  TreePayload,
  WorkspaceStatus
} from "./app-types";
import { clipboardText, writeClipboard } from "./clipboard-utils";
import { LatestRequestGuard, searchCompletionStatus } from "./event-source-session";
import { renderViewerFile } from "./file-viewer";
import { languageIconForPath } from "./language-icons";
import {
  isMarkdownPath,
  renderSnippet,
  sourceBadges
} from "./render-utils";
import "./styles.css";
import {
  byId,
  countText,
  escapeHtml,
  fileExtension,
  hitKey,
  isWithin,
  parentPath,
  pathText,
  pathWithBreaks,
  relativeWithin,
  samePath,
  shortPath
} from "./ui-utils";

const logoUrl = new URL("./assets/ivygrep-icon.svg", import.meta.url).href;
const boot = readBoot();
const queryParams = new URLSearchParams(location.search);
const fileRequests = new LatestRequestGuard();
const treeRequests = new LatestRequestGuard();
const state = createInitialState(queryParams.get("workspace") || boot.workspace || "");

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

function workspaceLabel(ws: WorkspaceStatus): string {
  const root = pathText(ws.root);
  const parts = root.split(/[\\/]/).filter(Boolean);
  return parts.slice(-2).join("/") || root;
}

function workspaceMeta(ws: WorkspaceStatus): string {
  if (ws.root === ALL_WORKSPACES) return "Search every tracked index";
  const counts = `${countText(ws.file_count || 0, "file")}, ${countText(ws.chunk_count || 0, "chunk")}`;
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
  const payload = await fetchStatus();
  state.workspaces = payload.workspaces || [];
  resolveRequestedWorkspace();
  renderWorkspaces();
  updateScopeLabel();
  await loadTree(state.scope || ".");
  setStatus(`${countText(state.workspaces.length, "workspace")}, v${payload.version || boot.version || "dev"}`);
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
    cancelSearch();
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

async function loadTree(path = "."): Promise<void> {
  const tree = byId("tree");
  if (state.workspace === ALL_WORKSPACES) {
    treeRequests.cancel();
    tree.innerHTML = '<div class="empty">Select one workspace to browse files.</div>';
    return;
  }
  const signal = treeRequests.start();
  try {
    const payload = await fetchTree(state.workspace, path, signal);
    if (!treeRequests.isCurrent(signal)) return;
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
      cancelSearch();
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

function updateScopeLabel(): void {
  const label = state.workspace === ALL_WORKSPACES || !state.scope ? "Scope: all folders" : `Scope: ${state.scope}`;
  byId("scope-label").textContent = label;
}

function cancelSearch(): void {
  state.events?.close();
  state.events = null;
  setSearching(false);
}

function runSearch(options: { preserveSelection?: boolean } = {}): void {
  const q = byId<HTMLTextAreaElement>("query").value.trim();
  if (!options.preserveSelection) fileRequests.cancel();
  cancelSearch();
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
    byId("results").innerHTML = '<div class="empty">Select one workspace to build context from its files, Git changes, and relationships.</div>';
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
  const events = connectSearchStream(params, () => state.events, {
    onStatus: () => setStatus("Searching"),
    onResults: renderResults,
    onDone: (payload) => {
      state.events?.close();
      state.events = null;
      setSearching(false);
      setStatus(searchCompletionStatus(payload.ok, state.searchErrors));
    },
    onError: () => {
      state.events?.close();
      state.events = null;
      setSearching(false);
      setStatus("Search connection closed");
    }
  });
  state.events = events;
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
    byId("results").innerHTML = `${warningMarkup(payload.warnings || [])}<div class="empty">${escapeHtml(payload.error)}</div>`;
    return;
  }
  state.searchErrors = payload.errors || [];
  if (payload.context_pack) {
    renderContextPack(payload.context_pack, Number(payload.elapsed_ms || 0), payload.warnings || []);
    return;
  }
  const hits = payload.hits || [];
  state.contextPack = null;
  state.hits = hits;
  pruneResultKeySets();
  updateCopyAvailability();
  const errorSummary = state.searchErrors.length
    ? `, ${countText(state.searchErrors.length, "workspace error")}: ${state.searchErrors.join("; ")}`
    : "";
  const warningSummary = payload.warnings?.length
    ? `, ${countText(payload.warnings.length, "warning")}: ${payload.warnings.join("; ")}`
    : "";
  byId("summary").textContent = `${countText(hits.length, "hit")} in ${Number(payload.elapsed_ms || 0).toFixed(1)} ms${errorSummary}${warningSummary}`;
  if (!hits.length) {
    state.currentHitKey = "";
    byId("results").innerHTML = `${warningMarkup(payload.warnings || [])}<div class="empty">No hits. Try a broader query, fewer filters, or a different search mode.</div>`;
    updateResultRows();
    return;
  }
  const root = byId("results");
  root.innerHTML = warningMarkup(payload.warnings || []);
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
      <pre class="snippet hljs">${renderSnippet(hit, byId<HTMLTextAreaElement>("query").value)}</pre>`;
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

function renderContextPack(bundle: ContextBundle, elapsedMs: number, warnings: string[] = []): void {
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
    ? `, ${countText(bundle.change_scope.total_changes, "changed file")}${bundle.change_scope.dirty_worktree ? " including worktree" : ""}`
    : "";
  const warningSummary = warnings.length ? `, ${countText(warnings.length, "warning")}: ${warnings.join("; ")}` : "";
  byId("summary").textContent = `${countText(coverage.files, "file")}, ${bundle.used_tokens}/${bundle.budget_tokens} tokens${changeText}, ${elapsedMs.toFixed(1)} ms${warningSummary}`;
  const root = byId("results");
  root.innerHTML = warningMarkup(warnings);
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
    <div class="context-title">Context pack</div>
    <div>${escapeHtml(coverageText || "No linked definitions, callers, tests, or dependencies")}${bundle.truncated ? " | truncated to budget" : ""}</div>
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
      <pre class="snippet hljs">${renderSnippet(hit, byId<HTMLTextAreaElement>("query").value)}</pre>`;
    article.querySelector(".hit-main")?.addEventListener("click", () => selectHit(hit));
    article.querySelector(".pin-hit")?.addEventListener("click", () => togglePinnedHit(key));
    article.querySelector(".open-hit")?.addEventListener("click", () => {
      void openHitInEditor(hit).catch((err: Error) => setStatus(err.message));
    });
    root.appendChild(article);
  }
  if (!hits.length) {
    root.insertAdjacentHTML("beforeend", '<div class="empty">No relevant context found.</div>');
  } else if (!state.manualOpen) {
    void openHit(hits[0]).catch((err: Error) => setStatus(err.message));
  }
}

function warningMarkup(warnings: string[]): string {
  if (!warnings.length) return "";
  const details = warnings.map((warning) => `<li>${escapeHtml(warning)}</li>`).join("");
  return `<aside class="search-warnings" role="status" aria-live="polite"><strong>Partial results</strong><ul>${details}</ul></aside>`;
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
  const text = clipboardText(
    byId<HTMLSelectElement>("copy-format").value as CopyFormat,
    hits,
    state.contextPack,
    scope,
    {
      query: byId<HTMLTextAreaElement>("query").value.trim(),
      workspace: state.workspace,
      scope: state.scope,
      mode: byId<HTMLSelectElement>("mode").value,
      limit: currentLimit()
    }
  );
  if (!text) {
    setStatus(`No ${scope} results to copy`);
    return;
  }
  await writeClipboard(text);
  setStatus(`Copied ${countText(hits.length, "result")}`);
}

function currentCopyScope(): CopyScope {
  const value = document.querySelector<HTMLSelectElement>("#copy-scope")?.value;
  return value === "pinned" ? "pinned" : "visible";
}

function copyHits(scope: CopyScope): SearchHit[] {
  if (scope === "pinned") return state.hits.filter((hit) => state.pinnedHitKeys.has(hitKey(hit)));
  return state.hits;
}

async function openHit(hit: SearchHit): Promise<void> {
  const workspace = state.workspace === ALL_WORKSPACES ? "" : state.workspace;
  await openFilePath(hit.file_path, hit.start_line, hit.end_line, workspace, hitKey(hit));
}

async function openHitInEditor(hit: SearchHit): Promise<void> {
  const workspace = state.workspace === ALL_WORKSPACES ? "" : state.workspace;
  await openPathInEditorApi(hit.file_path, hit.start_line, workspace);
  setStatus(`Opened ${shortPath(hit.file_path)}`);
}

async function openCurrentInEditor(): Promise<void> {
  const path = state.currentFileAbsolutePath || state.currentFilePath;
  if (!path) {
    setStatus("No file selected");
    return;
  }
  await openPathInEditorApi(path, state.currentFileStart, "");
  setStatus(`Opened ${shortPath(path)}`);
}

async function openFilePath(path: string, start = 1, end = 1, workspace = state.workspace, hitKeyValue = ""): Promise<void> {
  const signal = fileRequests.start();
  try {
    const payload = await fetchFile(workspace, path, signal);
    if (!fileRequests.isCurrent(signal)) return;
    state.currentFilePath = pathText(payload.path);
    state.currentFileAbsolutePath = pathText(payload.absolute_path);
    state.currentFileText = payload.text || "";
    state.currentFileStart = start;
    state.currentFileEnd = end;
    state.currentHitKey = hitKeyValue;
    state.viewerMode = isMarkdownPath(state.currentFilePath) ? "preview" : "source";
    byId("viewer-title").textContent = payload.path;
    byId("viewer-meta").textContent = `${payload.line_count} lines${payload.truncated ? ", truncated" : ""}`;
    renderViewerFile(state);
    updateResultRows();
  } catch (error) {
    if (fileRequests.isCurrent(signal)) throw error;
  }
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

function attachEvents(): void {
  byId("search").addEventListener("click", () => runSearch());
  byId("copy-results").addEventListener("click", () => {
    void copyVisibleResults().catch((err: Error) => setStatus(err.message));
  });
  byId("copy-scope").addEventListener("change", updateCopyAvailability);
  byId("refresh").addEventListener("click", () => {
    cancelSearch();
    void loadStatus().catch((err: Error) => setStatus(err.message));
  });
  byId("open-current").addEventListener("click", () => {
    void openCurrentInEditor().catch((err: Error) => setStatus(err.message));
  });
  byId("clear-scope").addEventListener("click", () => {
    cancelSearch();
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
    renderViewerFile(state);
  });
  byId("source-mode").addEventListener("click", () => {
    state.viewerMode = "source";
    renderViewerFile(state);
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
      cancelSearch();
      syncUrlState();
      debouncedRefresh();
    });
  }
  byId<HTMLSelectElement>("mode").addEventListener("change", () => {
    cancelSearch();
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
