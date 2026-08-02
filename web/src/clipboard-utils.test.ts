import { describe, expect, it } from "vitest";
import { clipboardText } from "./clipboard-utils";

const metadata = {
  query: "oauth",
  workspace: "/repo",
  scope: "src",
  mode: "hybrid",
  limit: 50
};

const hits = [
  {
    file_path: "src/auth.ts",
    start_line: 4,
    end_line: 6,
    score: 0.91,
    sources: ["neural"],
    preview: "refreshToken();"
  },
  {
    file_path: "src/auth.ts",
    start_line: 20,
    end_line: 20,
    score: 0.72,
    preview: "return token;"
  }
];

describe("clipboard-utils", () => {
  it("exports unique files and readable raw snippets", () => {
    expect(clipboardText("files", hits, null, "visible", metadata)).toBe("src/auth.ts");
    const raw = clipboardText("raw", hits, null, "visible", metadata);
    expect(raw).toContain("src/auth.ts:4-6");
    expect(raw).toContain("score 0.910 [neural]");
  });

  it("exports context packs as JSON for visible results", () => {
    const contextPack = {
      task: "find auth",
      workspace: "/repo",
      referenced_paths: [],
      budget_tokens: 100,
      used_tokens: 20,
      candidate_count: 1,
      truncated: false,
      anchor_symbols: [],
      coverage: {},
      items: []
    };
    expect(JSON.parse(clipboardText("json", hits, contextPack, "visible", metadata))).toEqual(contextPack);
    const pinned = JSON.parse(clipboardText("json", hits.slice(0, 1), contextPack, "pinned", metadata));
    expect(pinned.count).toBe(1);
    expect(pinned.hits[0].file_path).toBe("src/auth.ts");
  });
});
