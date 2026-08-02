import type { ContextBundle, CopyFormat, CopyScope, SearchHit } from "./app-types";

export type SearchExportMeta = {
  query: string;
  workspace: string;
  scope: string;
  mode: string;
  limit: number;
};

export function clipboardText(
  format: CopyFormat,
  hits: SearchHit[],
  contextPack: ContextBundle | null,
  scope: CopyScope,
  metadata: SearchExportMeta
): string {
  if (!hits.length) return "";
  if (format === "files") return uniqueFilePaths(hits).join("\n");
  if (format === "json" && contextPack && scope === "visible") {
    return JSON.stringify(contextPack, null, 2);
  }
  if (format === "json") return JSON.stringify(searchExport(hits, metadata), null, 2);
  return hits.map(rawHitText).join("\n\n");
}

export function uniqueFilePaths(hits: SearchHit[]): string[] {
  return Array.from(new Set(hits.map((hit) => hit.file_path)));
}

export function searchExport(hits: SearchHit[], metadata: SearchExportMeta): Record<string, unknown> {
  return {
    ...metadata,
    scope: metadata.scope || null,
    count: hits.length,
    hits: hits.map((hit) => ({
      file_path: hit.file_path,
      start_line: hit.start_line,
      end_line: hit.end_line,
      score: hit.score,
      sources: hit.sources || [],
      preview: hit.preview || ""
    }))
  };
}

export function rawHitText(hit: SearchHit): string {
  const range = hit.start_line === hit.end_line ? `${hit.start_line}` : `${hit.start_line}-${hit.end_line}`;
  const sources = hit.sources?.length ? ` [${hit.sources.join(", ")}]` : "";
  const preview = (hit.preview || "").trimEnd();
  return `${hit.file_path}:${range}\nscore ${Number(hit.score).toFixed(3)}${sources}${preview ? `\n${preview}` : ""}`;
}

export async function writeClipboard(text: string): Promise<void> {
  if (navigator.clipboard?.writeText && window.isSecureContext) {
    await navigator.clipboard.writeText(text);
    return;
  }
  const textArea = document.createElement("textarea");
  textArea.value = text;
  textArea.setAttribute("readonly", "");
  textArea.style.position = "fixed";
  textArea.style.opacity = "0";
  document.body.appendChild(textArea);
  textArea.select();
  const copied = document.execCommand("copy");
  textArea.remove();
  if (!copied) throw new Error("clipboard write failed");
}
