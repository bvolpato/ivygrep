import { ALL_WORKSPACES } from "./app-types";
import type { WorkspaceStatus } from "./app-types";
import { countText } from "./ui-utils";

export function workspaceMeta(ws: WorkspaceStatus): string {
  if (ws.root === ALL_WORKSPACES) return "Search every tracked index";
  const watch = ws.watch_enabled ? (ws.watcher_alive ? "watch live" : "watch offline") : "watch off";

  if (ws.statistics_unavailable) {
    let progress = "";
    if (ws.indexing_stalled) {
      progress = "index stalled";
    } else if (ws.indexing_in_progress) {
      progress = ws.indexing_progress || "indexing";
    } else if (ws.enhancing_stalled) {
      progress = "enhancement stalled";
    } else if (ws.enhancing_in_progress) {
      progress = ws.enhancing_progress_count == null
        ? "enhancing"
        : `enhancing (${ws.enhancing_progress_count} processed)`;
    }
    const progressText = progress ? `${progress} | ` : "";
    return `${progressText}Statistics temporarily unavailable | ${watch}`;
  }

  const counts = `${countText(ws.file_count || 0, "file")}, ${countText(ws.chunk_count || 0, "chunk")}`;
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
