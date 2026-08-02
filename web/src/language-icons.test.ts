import { describe, expect, it } from "vitest";
import { languageIconForPath } from "./language-icons";

describe("language icons", () => {
  it("renders accessible labels for common source files", () => {
    expect(languageIconForPath("src/main.ts")).toContain('title="TypeScript"');
    expect(languageIconForPath("README.md")).toContain('title="Markdown"');
    expect(languageIconForPath("unknown.bin")).toContain('title="File"');
  });
});
