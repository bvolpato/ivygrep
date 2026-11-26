import { describe, expect, it } from "vitest";
import {
  isMarkdownPath,
  languageForPath,
  markQueryTerms,
  queryTerms,
  renderCode,
  renderMarkdown,
  renderSnippet,
  workspaceLink
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

  it("renders files and code fences named like object members", () => {
    for (const name of ["constructor", "__proto__"]) {
      const hit = { file_path: `docs/${name}`, start_line: 1, end_line: 1, score: 1, preview: "plain text" };
      expect(renderSnippet(hit, "")).toContain("plain text");
      expect(renderMarkdown(`\`\`\`${name}\nplain text\n\`\`\``)).toContain("plain text");
    }
  });

  it("does not mark query terms inside escaped characters", () => {
    expect(markQueryTerms("a &lt; b &amp;&amp; it&#x27;s", "lt amp x27")).toBe("a &lt; b &amp;&amp; it&#x27;s");
    expect(markQueryTerms("lt &lt; b", "lt")).toBe('<mark class="query-mark">lt</mark> &lt; b');
  });

  it("resolves Markdown links between workspace files", () => {
    expect(workspaceLink("docs/a/guide.md", "../lib%20x.md?raw=1#L12")).toEqual({ path: "docs/lib x.md", line: 12 });
    expect(workspaceLink("docs/guide.md", "/src/lib.rs")).toEqual({ path: "src/lib.rs", line: 1 });
    expect(workspaceLink("guide.md", "../../outside.md")).toEqual({ path: "../../outside.md", line: 1 });
    expect(workspaceLink("docs/guide.md", "#usage")).toEqual({ path: "docs/guide.md", line: 1 });
    for (const external of ["https://example.com/a", "//example.com/a", "mailto:a@example.com"]) {
      expect(workspaceLink("docs/guide.md", external)).toBeUndefined();
    }
    expect(renderMarkdown("[x](https://example.com)")).toContain('target="_blank" rel="noopener noreferrer"');
    expect(renderMarkdown("[x](lib.md)")).not.toContain("target=");
  });
});
