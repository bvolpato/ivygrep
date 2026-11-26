import { describe, expect, it } from "vitest";
import { workspaceMeta } from "./workspace-status";

describe("workspace status", () => {
  it("does not render placeholder statistics when index stats are unavailable", () => {
    const meta = workspaceMeta({
      root: "/workspace/repo",
      statistics_unavailable: true,
      file_count: 0,
      chunk_count: 0,
      has_neural_vectors: false,
      neural_coverage_percent: 0,
      compaction: { healthy: true },
      watch_enabled: true,
      watcher_alive: true,
      indexing_in_progress: true,
      indexing_progress: "120/300"
    });

    expect(meta).toContain("120/300");
    expect(meta).toContain("Statistics temporarily unavailable");
    expect(meta).toContain("watch live");
    expect(meta).not.toContain("0 files");
    expect(meta).not.toContain("0 chunks");
    expect(meta).not.toContain("neural 0%");
  });

  it("keeps zero counts for older status payloads without the optional flag", () => {
    const meta = workspaceMeta({ root: "/workspace/repo", file_count: 0, chunk_count: 0 });

    expect(meta).toContain("0 files, 0 chunks");
    expect(meta).not.toContain("Statistics temporarily unavailable");
  });
});
