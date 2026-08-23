import { test, expect } from "@playwright/test";

test("searches an indexed workspace and opens the result in the viewer", async ({ page }) => {
  await page.goto("/");

  await expect(page.locator("#status")).not.toHaveText("Starting");
  await expect(page.locator("#workspaces .workspace").filter({ hasText: "project" })).toBeVisible();
  await page.locator("#tree .tree-row.folder").filter({ hasText: "src" }).click();
  await expect(page.locator("#tree .tree-row.file").filter({ hasText: "web.ts" })).toBeVisible();

  await page.locator("#query").fill("semantic browser marker");
  await page.getByRole("button", { name: "Search", exact: true }).click();

  const firstHit = page.locator("#results .hit").first();
  await expect(firstHit).toBeVisible();
  await expect(firstHit.locator(".file-path")).toContainText("web.ts");
  await expect(page.locator("#summary")).toContainText("hit");
  await expect(page.locator("#status")).toContainText("Ready");
  await expect(page.locator("#viewer-title")).toContainText("web.ts");
  await expect(page.locator("#file-view")).toContainText("semantic browser marker");
});

test("ignores late search results after folder navigation", async ({ page }) => {
  // Streams are told apart by URL, not by creation order, so the test does not
  // depend on how quickly the runner processes the clicks:
  // - the first search (no `scope`) holds its results until the test releases
  //   them after navigation, standing in for a slow backend;
  // - the re-run after navigating into `src` (`scope=src`) answers at once.
  await page.addInitScript(() => {
    const hit = (filePath: string) => JSON.stringify({
      hits: [{ file_path: filePath, start_line: 1, end_line: 1, score: 1, preview: `${filePath} result` }],
      elapsed_ms: 1
    });
    class ScriptedEventSource extends EventTarget {
      onerror: ((event: Event) => void) | null = null;

      constructor(url: string | URL, _options?: EventSourceInit) {
        super();
        const params = new URL(String(url), window.location.origin).searchParams;
        if (params.get("q") !== "delayed result") {
          window.setTimeout(() => {
            this.dispatchEvent(new MessageEvent("done", { data: JSON.stringify({ ok: true }) }));
          }, 0);
          return;
        }
        if (params.get("scope") === "src") {
          window.setTimeout(() => {
            this.dispatchEvent(new MessageEvent("results", { data: hit("src/fresh.ts") }));
            this.dispatchEvent(new MessageEvent("done", { data: JSON.stringify({ ok: true }) }));
          }, 0);
          return;
        }
        (window as unknown as { __releaseLateResults?: () => boolean }).__releaseLateResults = () => {
          this.dispatchEvent(new MessageEvent("results", { data: hit("stale.ts") }));
          this.dispatchEvent(new MessageEvent("done", { data: JSON.stringify({ ok: true }) }));
          return true;
        };
      }

      close(): void {}
    }

    Object.defineProperty(window, "EventSource", { value: ScriptedEventSource });
  });
  await page.goto("/");
  await expect(page.locator("#tree .tree-row.folder").filter({ hasText: "src" })).toBeVisible();

  await page.locator("#query").fill("delayed result");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await expect(page.locator("#summary")).toHaveText("Searching...");

  await page.locator("#tree .tree-row.folder").filter({ hasText: "src" }).click();
  await expect(page.locator("#scope-label")).toHaveText("Scope: src");
  await expect(page.locator("#results .file-path").filter({ hasText: "fresh.ts" })).toHaveCount(1);

  // Only now does the superseded search deliver; the UI must drop it.
  const released = await page.evaluate(() =>
    (window as unknown as { __releaseLateResults?: () => boolean }).__releaseLateResults?.() ?? false
  );
  expect(released).toBe(true);
  await expect(page.locator("#results .file-path").filter({ hasText: "stale.ts" })).toHaveCount(0);
  await expect(page.locator("#results .file-path").filter({ hasText: "fresh.ts" })).toHaveCount(1);
});
