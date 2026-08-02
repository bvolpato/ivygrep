import { escapeHtml } from "./ui-utils";

export function languageIconForPath(path: string): string {
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
