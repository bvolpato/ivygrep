import { describe, expect, it } from "vitest";
import {
  isMarkdownPath,
  languageForPath,
  queryTerms,
  renderCode,
  renderMarkdown,
  renderSnippet
} from "./render-utils";

describe("render-utils", () => {
  it("keeps path language mapping and markdown detection centralized", () => {
    expect(languageForPath("src/main.ts")).toBe("typescript");
    expect(languageForPath("Dockerfile")).toBe("dockerfile");
    expect(isMarkdownPath("docs/README.md")).toBe(true);
    expect(isMarkdownPath("src/main.rs")).toBe(false);
  });

  it("marks query terms without injecting raw HTML", () => {
    const hit = {
      file_path: "src/auth.ts",
      start_line: 12,
      end_line: 12,
      score: 0.9,
      preview: "const oauthToken = '<unsafe>';"
    };
    const snippet = renderSnippet(hit, "oauthToken");

    expect(snippet).toContain("query-mark");
    expect(snippet).toContain("&lt;unsafe&gt;");
    expect(snippet).not.toContain("<unsafe>");
  });

  it("renders focused source lines and safe markdown", () => {
    const code = renderCode("one\ntwo", 2, 2, "example.ts");
    expect(code).toContain('class="line focus"');
    expect(code).toContain("Line 2:");

    const markdown = renderMarkdown("# Heading\n\n<script>alert(1)</script>");
    expect(markdown).toContain("<h1>Heading</h1>");
    expect(markdown).toContain("&lt;script&gt;");
    expect(markdown).not.toContain("<script>alert(1)</script>");
  });

  it("extracts stable terms from free-form queries", () => {
    expect(queryTerms("fix OAuth token refresh")).toEqual(["fix", "oauth", "token", "refresh"]);
  });
});
