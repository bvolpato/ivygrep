import "highlight.js/styles/github-dark.css";
import hljs from "highlight.js/lib/common";
import MarkdownIt from "markdown-it";
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
  hits?: SearchHit[];
  elapsed_ms?: number;
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
  text?: string;
  line_count: number;
  truncated?: boolean;
};

type ViewerMode = "preview" | "source";

type AppState = {
  workspaces: WorkspaceStatus[];
  workspace: string;
  scope: string;
  events: EventSource | null;
  autoOpenKey: string;
  manualOpen: boolean;
  requestedWorkspace: string;
  viewerMode: ViewerMode;
  currentFilePath: string;
  currentFileText: string;
  currentFileStart: number;
  currentFileEnd: number;
  currentHitKey: string;
};

const ALL_WORKSPACES = "__all__";
const boot = readBoot();
const queryParams = new URLSearchParams(location.search);
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
  scope: "",
  events: null,
  autoOpenKey: "",
  manualOpen: false,
  requestedWorkspace: queryParams.get("workspace") || boot.workspace || "",
  viewerMode: "source",
  currentFilePath: "",
  currentFileText: "",
  currentFileStart: 1,
  currentFileEnd: 1,
  currentHitKey: ""
};

function readBoot(): BootConfig {
  const text = document.getElementById("ivygrep-boot")?.textContent?.trim();
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
        <div class="searchbar">
          <input id="query" type="search" placeholder="Search code, symbols, paths" />
          <select id="mode" title="Search mode">
            <option value="hybrid">Hybrid</option>
            <option value="literal">Literal</option>
            <option value="regex">Regex</option>
          </select>
          <button class="btn" id="search" type="button">Search</button>
          <button class="btn secondary" id="refresh" type="button">Refresh</button>
        </div>
        <div class="statusline"><span class="dot"></span><span id="status">Starting</span></div>
      </header>
      <main class="layout">
        <aside class="sidebar">
          <div class="section-title">Workspaces</div>
          <div id="workspaces"></div>
          <div class="section-title">Filters</div>
          <div class="filters">
            <input id="type" placeholder="type" />
            <input id="limit" type="number" min="1" max="500" value="20" title="Limit" />
            <input id="include" placeholder="include globs" />
            <input id="exclude" placeholder="exclude globs" />
          </div>
          <div class="scopebar">
            <span id="scope-label">Scope: all folders</span>
            <button class="linkbtn" id="clear-scope" type="button">Global</button>
          </div>
          <div class="section-title">Explorer</div>
          <div id="tree" class="tree"><div class="empty">Select a workspace to browse.</div></div>
        </aside>
        <section class="results">
          <div class="summary" id="summary">No search yet.</div>
          <div id="results"><div class="empty">Pick a workspace or all indices, then search.</div></div>
        </section>
        <section class="viewer">
          <div class="viewer-head">
            <div class="viewer-head-row">
              <div>
                <div class="viewer-title" id="viewer-title">No file selected</div>
                <div class="viewer-meta" id="viewer-meta">Search result previews open here.</div>
              </div>
              <div class="viewer-actions" id="viewer-actions" hidden>
                <button class="toggle active" id="preview-mode" type="button">Preview</button>
                <button class="toggle" id="source-mode" type="button">Source</button>
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

function setStatus(text: string): void {
  byId("status").textContent = text;
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
  const response = await fetch("/api/status");
  const payload = await response.json() as { type?: string; message?: string; version?: string; workspaces?: WorkspaceStatus[] };
  if (payload.type === "error") throw new Error(payload.message || "status failed");
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
  root.innerHTML = "";
  root.appendChild(workspaceButton({ root: ALL_WORKSPACES, file_count: 0, chunk_count: 0 }, "All indices"));
  for (const ws of state.workspaces) root.appendChild(workspaceButton(ws, workspaceLabel(ws)));
  for (const button of root.querySelectorAll<HTMLButtonElement>(".workspace")) {
    button.classList.toggle("active", button.dataset.workspace === state.workspace);
  }
}

function workspaceButton(ws: WorkspaceStatus, label: string): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.className = "workspace";
  button.dataset.workspace = ws.root;
  button.innerHTML = `<div class="path">${escapeHtml(label)}</div><div class="meta">${escapeHtml(workspaceMeta(ws))}</div>`;
  button.addEventListener("click", () => {
    state.workspace = ws.root;
    state.scope = "";
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
    tree.innerHTML = '<div class="empty">Select one workspace to browse files.</div>';
    return;
  }
  const response = await fetch(`/api/tree?workspace=${enc(state.workspace)}&path=${enc(path || ".")}`);
  const payload = await response.json() as TreePayload;
  if (payload.error) throw new Error(payload.error);
  renderTree(payload);
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
  const icon = kind === "parent" ? "up" : kind === "folder" ? "folder" : "file";
  const meta = kind === "folder" ? "folder" : kind === "parent" ? "up" : fileExtension(entry.name);
  button.innerHTML = `<span class="tree-icon">${svgIcon(icon)}</span><span class="tree-name">${escapeHtml(entry.name)}</span><span class="tree-meta">${escapeHtml(meta)}</span>`;
  button.addEventListener("click", () => {
    if (entry.is_dir) {
      state.scope = pathText(entry.path);
      updateScopeLabel();
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

function runSearch(): void {
  const q = byId<HTMLInputElement>("query").value.trim();
  if (!q) {
    byId("summary").textContent = "Enter a query.";
    return;
  }
  if (state.events) state.events.close();
  state.autoOpenKey = "";
  state.manualOpen = false;
  byId("results").innerHTML = "";
  byId("summary").textContent = "Searching...";
  const params = new URLSearchParams({
    q,
    workspace: state.workspace,
    mode: byId<HTMLSelectElement>("mode").value,
    limit: byId<HTMLInputElement>("limit").value || "20",
    type: byId<HTMLInputElement>("type").value || "",
    include: byId<HTMLInputElement>("include").value || "",
    exclude: byId<HTMLInputElement>("exclude").value || ""
  });
  if (state.workspace !== ALL_WORKSPACES && state.scope) params.set("scope", state.scope);
  state.events = new EventSource(`/api/search/stream?${params.toString()}`);
  state.events.addEventListener("status", () => setStatus("Searching"));
  state.events.addEventListener("results", (event) => {
    renderResults(JSON.parse((event as MessageEvent<string>).data) as SearchPayload);
  });
  state.events.addEventListener("done", () => {
    state.events?.close();
    state.events = null;
    setStatus("Ready");
  });
  state.events.onerror = () => {
    state.events?.close();
    state.events = null;
    setStatus("Search connection closed");
  };
}

function refreshSearchIfQuery(): void {
  if (byId<HTMLInputElement>("query").value.trim()) runSearch();
}

function renderResults(payload: SearchPayload): void {
  if (payload.error) {
    byId("summary").textContent = payload.error;
    byId("results").innerHTML = `<div class="empty">${escapeHtml(payload.error)}</div>`;
    return;
  }
  const hits = payload.hits || [];
  byId("summary").textContent = `${hits.length} hit(s) in ${Number(payload.elapsed_ms || 0).toFixed(1)} ms`;
  if (!hits.length) {
    byId("results").innerHTML = '<div class="empty">No hits.</div>';
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
    item.innerHTML = `<button type="button"><div class="file">${languageIconForPath(hit.file_path)}<span class="file-path">${escapeHtml(hit.file_path)}</span></div><div class="score">score ${Number(hit.score).toFixed(3)} ${escapeHtml((hit.sources || []).join(", "))}</div></button><pre class="snippet hljs">${renderSnippet(hit)}</pre>`;
    item.querySelector("button")?.addEventListener("click", () => selectHit(hit));
    root.appendChild(item);
  }
  const first = hits[0];
  const firstKey = `${first.file_path}:${first.start_line}:${first.end_line}`;
  if (!state.manualOpen && state.autoOpenKey !== firstKey) {
    state.autoOpenKey = firstKey;
    void openHit(first).catch((err: Error) => setStatus(err.message));
  }
}

function selectHit(hit: SearchHit): void {
  state.manualOpen = true;
  void openHit(hit).catch((err: Error) => setStatus(err.message));
}

async function openHit(hit: SearchHit): Promise<void> {
  const workspace = state.workspace === ALL_WORKSPACES ? "" : state.workspace;
  await openFilePath(hit.file_path, hit.start_line, hit.end_line, workspace, hitKey(hit));
}

async function openFilePath(path: string, start = 1, end = 1, workspace = state.workspace, hitKeyValue = ""): Promise<void> {
  const response = await fetch(`/api/file?workspace=${enc(workspace)}&path=${enc(path)}`);
  const payload = await response.json() as FilePayload;
  if (payload.error) throw new Error(payload.error);
  state.currentFilePath = pathText(payload.path);
  state.currentFileText = payload.text || "";
  state.currentFileStart = start;
  state.currentFileEnd = end;
  state.currentHitKey = hitKeyValue;
  state.viewerMode = isMarkdownPath(state.currentFilePath) ? "preview" : "source";
  byId("viewer-title").textContent = payload.path;
  byId("viewer-meta").textContent = `${payload.line_count} lines${payload.truncated ? ", truncated" : ""}`;
  renderViewerFile();
  updateSelectedRows();
}

function renderViewerFile(): void {
  const actions = byId<HTMLDivElement>("viewer-actions");
  const markdownFile = isMarkdownPath(state.currentFilePath);
  actions.hidden = !markdownFile;
  byId("preview-mode").classList.toggle("active", markdownFile && state.viewerMode === "preview");
  byId("source-mode").classList.toggle("active", !markdownFile || state.viewerMode === "source");
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
    return `<span class="line${focus}"><span class="line-number">${number}</span><span>${highlightLine(line, language) || " "}</span></span>`;
  }).join("")}</pre>`;
}

function renderSnippet(hit: SearchHit): string {
  const language = languageForPath(hit.file_path);
  return (hit.preview || "").split(/\r?\n/).map((line, index) => {
    const number = hit.start_line + index;
    return `<span class="snippet-line"><span class="snippet-line-number">${number}</span><span>${highlightLine(line, language) || " "}</span></span>`;
  }).join("");
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

function updateSelectedRows(): void {
  for (const hit of document.querySelectorAll<HTMLElement>(".hit")) {
    hit.classList.toggle("active", hit.dataset.hitKey === state.currentHitKey);
  }
  for (const row of document.querySelectorAll<HTMLButtonElement>(".tree-row")) {
    const kind = row.dataset.kind;
    const path = row.dataset.path || "";
    const active = kind === "file" ? samePath(path, state.currentFilePath) : kind === "folder" && path === state.scope;
    row.classList.toggle("active", active);
  }
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

function escapeHtml(value: unknown): string {
  return String(value).replace(/[&<>"']/g, (ch) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[ch] || ch);
}

function attachEvents(): void {
  byId("search").addEventListener("click", runSearch);
  byId("refresh").addEventListener("click", () => void loadStatus().catch((err: Error) => setStatus(err.message)));
  byId("clear-scope").addEventListener("click", () => {
    state.scope = "";
    updateScopeLabel();
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
  byId<HTMLInputElement>("query").addEventListener("keydown", (event) => {
    if (event.key === "Enter") runSearch();
  });
  const debouncedRefresh = debounce(refreshSearchIfQuery, 220);
  for (const id of ["type", "limit", "include", "exclude"]) {
    byId<HTMLInputElement>(id).addEventListener("input", debouncedRefresh);
  }
  byId<HTMLSelectElement>("mode").addEventListener("change", refreshSearchIfQuery);
}

installFavicon();
renderShell();
attachEvents();
byId<HTMLInputElement>("query").value = queryParams.get("q") || boot.query || "";
void loadStatus()
  .then(() => {
    if (byId<HTMLInputElement>("query").value.trim()) runSearch();
  })
  .catch((err: Error) => setStatus(err.message));
