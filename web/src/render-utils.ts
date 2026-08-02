import hljs from "highlight.js/lib/common";
import MarkdownIt from "markdown-it";
import type { SearchHit } from "./app-types";
import { escapeHtml } from "./ui-utils";

const markdown = new MarkdownIt({
  html: false,
  linkify: true,
  highlight(code, language) {
    return `<pre class="md-code"><code class="hljs">${highlightText(code, language)}</code></pre>`;
  }
});

export function renderMarkdown(text: string): string {
  return markdown.render(text);
}

export function renderCode(text: string, start: number, end: number, path: string): string {
  const language = languageForPath(path);
  const lines = text.split(/\r?\n/);
  return `<pre class="code">${lines.map((line, index) => {
    const number = index + 1;
    const focus = number >= start && number <= end ? " focus" : "";
    return `<span class="line${focus}"><span class="line-number" aria-hidden="true">${number}</span><span><span class="sr-only">Line ${number}: </span>${highlightLine(line, language) || " "}</span></span>`;
  }).join("")}</pre>`;
}

export function renderSnippet(hit: SearchHit, query: string): string {
  const language = languageForPath(hit.file_path);
  return (hit.preview || "").split(/\r?\n/).map((line, index) => {
    const number = hit.start_line + index;
    return `<span class="snippet-line"><span class="snippet-line-number" aria-hidden="true">${number}</span><span><span class="sr-only">Line ${number}: </span>${markQueryTerms(highlightLine(line, language) || " ", query)}</span></span>`;
  }).join("");
}

export function markQueryTerms(html: string, query: string): string {
  const terms = queryTerms(query);
  if (!terms.length) return html;
  const pattern = new RegExp(`(${terms.map(escapeRegExp).join("|")})`, "gi");
  return html
    .split(/(<[^>]+>)/g)
    .map((part) => part.startsWith("<") ? part : part.replace(pattern, '<mark class="query-mark">$1</mark>'))
    .join("");
}

export function queryTerms(query: string): string[] {
  const trimmed = query.trim();
  if (!trimmed) return [];
  const terms = trimmed.match(/[A-Za-z0-9_.$:/-]{2,}/g) || (trimmed.length <= 32 ? [trimmed] : []);
  return Array.from(new Set(terms.map((term) => term.toLowerCase()))).slice(0, 8);
}

export function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

export function sourceBadges(sources: string[]): string {
  return sources.map((source) => `<span class="source-badge">${escapeHtml(source)}</span>`).join("");
}

export function highlightLine(line: string, language?: string): string {
  return language ? highlightText(line, language) : escapeHtml(line);
}

export function highlightText(text: string, language?: string): string {
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

export function isMarkdownPath(path: string): boolean {
  return /\.(md|markdown|mdown|mkd)$/i.test(path);
}

export function languageForPath(path: string): string | undefined {
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

export function normalizeLanguage(language?: string): string | undefined {
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
