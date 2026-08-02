import type { AppState } from "./app-types";
import { byId } from "./ui-utils";
import { isMarkdownPath, renderCode, renderMarkdown } from "./render-utils";

export function renderViewerFile(state: AppState): void {
  const actions = byId<HTMLDivElement>("viewer-actions");
  byId<HTMLButtonElement>("open-current").disabled = !state.currentFilePath;
  const markdownFile = isMarkdownPath(state.currentFilePath);
  actions.hidden = !markdownFile;
  byId("preview-mode").classList.toggle("active", markdownFile && state.viewerMode === "preview");
  byId("source-mode").classList.toggle("active", !markdownFile || state.viewerMode === "source");
  byId("preview-mode").setAttribute("aria-pressed", String(markdownFile && state.viewerMode === "preview"));
  byId("source-mode").setAttribute("aria-pressed", String(!markdownFile || state.viewerMode === "source"));
  if (markdownFile && state.viewerMode === "preview") {
    byId("file-view").innerHTML = `<article class="markdown-preview">${renderMarkdown(state.currentFileText)}</article>`;
    return;
  }
  byId("file-view").innerHTML = renderCode(
    state.currentFileText,
    state.currentFileStart,
    state.currentFileEnd,
    state.currentFilePath
  );
  scrollFocusedLine();
}

function scrollFocusedLine(): void {
  requestAnimationFrame(() => {
    document.querySelector(".line.focus")?.scrollIntoView({ block: "center", inline: "nearest" });
  });
}
