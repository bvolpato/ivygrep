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

export function isWithin(path: string, root: string): boolean {
  return path === root || path.startsWith(`${root}/`) || path.startsWith(`${root}\\`);
}

export function relativeWithin(path: string, root: string): string {
  return path.slice(root.length).replace(/^[/\\]+/, "");
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
