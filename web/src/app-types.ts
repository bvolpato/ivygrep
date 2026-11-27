export type BootConfig = {
  version?: string;
  query?: string | null;
  workspace?: string | null;
};

export type WorkspaceStatus = {
  root: string;
  statistics_unavailable?: boolean;
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

export type SearchHit = {
  file_path: string;
  start_line: number;
  end_line: number;
  score: number;
  preview?: string;
  sources?: string[];
};

export type ContextChange = {
  file_path: string;
  old_path?: string;
  status: string;
  sources: string[];
};

export type ContextBundle = {
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

export type SearchPayload = {
  error?: string;
  errors?: string[];
  warnings?: string[];
  hits?: SearchHit[];
  more_hits_in_file?: Record<string, number>;
  context_pack?: ContextBundle;
  elapsed_ms?: number;
};

export type SearchDonePayload = {
  ok?: boolean;
};

export type TreeEntry = {
  name: string;
  path: string;
  is_dir: boolean;
};

export type TreePayload = {
  error?: string;
  path?: string;
  /** The requested path is a file, and `path` is its folder. */
  is_file?: boolean;
  entries?: TreeEntry[];
  total?: number;
};

export type FilePayload = {
  error?: string;
  workspace?: string;
  path: string;
  absolute_path?: string;
  text?: string;
  line_count: number;
  truncated?: boolean;
};

export type StatusPayload = {
  type?: string;
  error?: string;
  message?: string;
  version?: string;
  workspaces?: WorkspaceStatus[];
};

export type ViewerMode = "preview" | "source";
export type CopyFormat = "files" | "raw" | "json";
export type CopyScope = "visible" | "pinned";

export type SearchMetadata = {
  query: string;
  workspace: string;
  scope: string;
  mode: string;
  limit: number;
};

export type AppState = {
  workspaces: WorkspaceStatus[];
  workspace: string;
  workspaceFilter: string;
  scope: string;
  scopeIsFile: boolean;
  events: EventSource | null;
  searching: boolean;
  /** Request that produced `hits`. The controls can change after it. */
  search: SearchMetadata | null;
  hits: SearchHit[];
  contextPack: ContextBundle | null;
  autoOpenKey: string;
  manualOpen: boolean;
  requestedWorkspace: string;
  viewerMode: ViewerMode;
  currentFileWorkspace: string;
  currentFilePath: string;
  currentFileAbsolutePath: string;
  currentFileText: string;
  currentFileStart: number;
  currentFileEnd: number;
  currentHitKey: string;
  pinnedHitKeys: Set<string>;
  searchErrors: string[];
};

export const ALL_WORKSPACES = "__all__";
export const DEFAULT_LIMIT = 50;
export const LOAD_MORE_STEP = 50;
export const MAX_LIMIT = 500;

export function createInitialState(requestedWorkspace = ""): AppState {
  return {
    workspaces: [],
    workspace: ALL_WORKSPACES,
    workspaceFilter: "",
    scope: "",
    scopeIsFile: false,
    events: null,
    searching: false,
    search: null,
    hits: [],
    contextPack: null,
    autoOpenKey: "",
    manualOpen: false,
    requestedWorkspace,
    viewerMode: "source",
    currentFileWorkspace: "",
    currentFilePath: "",
    currentFileAbsolutePath: "",
    currentFileText: "",
    currentFileStart: 1,
    currentFileEnd: 1,
    currentHitKey: "",
    pinnedHitKeys: new Set(),
    searchErrors: []
  };
}

/** Query and workspace to start with. The boot values apply only to a bare URL. */
export function initialSelection(search: string, boot: BootConfig): { query: string; workspace: string } {
  const params = new URLSearchParams(search);
  const [query, workspace] = search
    ? [params.get("q"), params.get("workspace")]
    : [boot.query, boot.workspace];
  return {
    query: query || "",
    workspace: workspace && workspace !== ALL_WORKSPACES ? workspace : ""
  };
}

export function readBoot(documentRef: Document = document): BootConfig {
  const text = documentRef.querySelector<HTMLMetaElement>("meta[name='ivygrep-boot']")?.content.trim();
  if (!text || text === "__IVYGREP_BOOT__") return {};
  try {
    return JSON.parse(text) as BootConfig;
  } catch {
    return {};
  }
}
