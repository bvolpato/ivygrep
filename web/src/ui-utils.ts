export function byId<T extends HTMLElement = HTMLElement>(id: string): T {
  const element = document.getElementById(id);
  if (!element) throw new Error(`missing #${id}`);
  return element as T;
}

export function countText(count: number, singular: string, plural = `${singular}s`): string {
  return `${count} ${count === 1 ? singular : plural}`;
}

export function enc(value: string): string {
  return encodeURIComponent(value || "");
}

export function pathText(value: unknown): string {
  return typeof value === "string" ? value : "";
}

export function escapeHtml(value: unknown): string {
  return String(value).replace(/[&<>"']/g, (ch) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[ch] || ch);
}

export function pathWithBreaks(value: string): string {
  return escapeHtml(value).replaceAll("/", "/<wbr>").replaceAll("\\", "\\<wbr>");
}

export function normalizePath(path: string): string {
  return pathText(path).replace(/\\/g, "/").replace(/^\.\//, "");
}

export function samePath(left: string, right: string): boolean {
  return Boolean(left && right) && normalizePath(left) === normalizePath(right);
}

export function hitKey(hit: { file_path: string; start_line: number; end_line: number }): string {
  return `${normalizePath(hit.file_path)}:${hit.start_line}:${hit.end_line}`;
}

/** Keeps the first hit of each file range, so every row has its own key. */
export function uniqueHits<T extends { file_path: string; start_line: number; end_line: number }>(hits: T[]): T[] {
  const keys = new Set<string>();
  return hits.filter((hit) => {
    const key = hitKey(hit);
    if (keys.has(key)) return false;
    keys.add(key);
    return true;
  });
}

/** Spaces are part of a literal or regex pattern. Pasted line breaks at its ends are not. */
export function searchQuery(mode: string, value: string): string {
  const isPattern = mode === "literal" || mode === "regex";
  return isPattern ? value.replace(/^[\r\n]+|[\r\n]+$/g, "") : value.trim();
}

export function isWithin(path: string, root: string): boolean {
  return path === root || path.startsWith(`${root}/`) || path.startsWith(`${root}\\`);
}

export function relativeWithin(path: string, root: string): string {
  return path.slice(root.length).replace(/^[/\\]+/, "");
}

/**
 * Workspace and scope for a requested path. A workspace can be nested in
 * another one, and only its own index holds its files.
 */
export function workspaceForPath(
  workspaces: { root: string }[],
  path: string
): { root: string; scope: string } | undefined {
  const roots = workspaces.map((ws) => ws.root).filter((root) => isWithin(path, root));
  if (!roots.length) return undefined;
  const root = roots.reduce((longest, candidate) => candidate.length > longest.length ? candidate : longest);
  return { root, scope: relativeWithin(path, root) };
}

export function shortPath(path: string): string {
  const parts = pathText(path).split(/[\\/]/).filter(Boolean);
  return parts.slice(-2).join("/") || path;
}

export function fileExtension(name: string): string {
  const index = name.lastIndexOf(".");
  return index > 0 ? name.slice(index + 1) : "file";
}

export function parentPath(path: string): string {
  const parts = pathText(path).split(/[\\/]/).filter(Boolean);
  parts.pop();
  return parts.join("/");
}
