import {
  guardCurrentStream
} from "./event-source-session";
import type {
  FilePayload,
  SearchDonePayload,
  SearchPayload,
  StatusPayload,
  TreePayload
} from "./app-types";

export function apiFetch(input: RequestInfo | URL, init: RequestInit = {}): Promise<Response> {
  return fetch(input, { ...init, credentials: "same-origin" });
}

export async function fetchStatus(): Promise<StatusPayload> {
  const response = await apiFetch("/api/status");
  const payload = await response.json() as StatusPayload;
  if (!response.ok || payload.error || payload.type === "error") {
    throw new Error(payload.error || payload.message || "status failed");
  }
  return payload;
}

export async function fetchTree(workspace: string, path: string, signal: AbortSignal): Promise<TreePayload> {
  const response = await apiFetch(
    `/api/tree?workspace=${encodeURIComponent(workspace)}&path=${encodeURIComponent(path || ".")}`,
    { signal }
  );
  const payload = await response.json() as TreePayload;
  if (!response.ok || payload.error) throw new Error(payload.error || "tree failed");
  return payload;
}

export async function fetchFile(workspace: string, path: string, signal: AbortSignal): Promise<FilePayload> {
  const response = await apiFetch(
    `/api/file?workspace=${encodeURIComponent(workspace)}&path=${encodeURIComponent(path)}`,
    { signal }
  );
  const payload = await response.json() as FilePayload;
  if (!response.ok || payload.error) throw new Error(payload.error || "file failed");
  return payload;
}

export async function openPathInEditor(path: string, line: number, workspace = ""): Promise<{ program?: string }> {
  const params = new URLSearchParams({ path, line: String(Math.max(1, line || 1)) });
  if (workspace) params.set("workspace", workspace);
  const response = await apiFetch(`/api/open?${params.toString()}`, { method: "POST" });
  const payload = await response.json() as { ok?: boolean; error?: string; program?: string };
  if (!response.ok || payload.error || !payload.ok) throw new Error(payload.error || "open failed");
  return payload;
}

export type SearchStreamHandlers = {
  onStatus: () => void;
  onResults: (payload: SearchPayload) => void;
  onDone: (payload: SearchDonePayload) => void;
  onError: () => void;
};

export function connectSearchStream(
  params: URLSearchParams,
  current: () => EventSource | null,
  handlers: SearchStreamHandlers
): EventSource {
  const events = new EventSource(`/api/search/stream?${params.toString()}`, { withCredentials: true });
  events.addEventListener("status", guardCurrentStream(
    current,
    events,
    () => handlers.onStatus()
  ));
  events.addEventListener("results", guardCurrentStream(
    current,
    events,
    (event) => handlers.onResults(parseEvent<SearchPayload>(event))
  ));
  events.addEventListener("done", guardCurrentStream(
    current,
    events,
    (event) => handlers.onDone(parseEvent<SearchDonePayload>(event))
  ));
  events.onerror = guardCurrentStream(
    current,
    events,
    () => handlers.onError()
  );
  return events;
}

function parseEvent<T>(event: Event): T {
  return JSON.parse((event as MessageEvent<string>).data) as T;
}
