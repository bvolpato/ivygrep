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
  await page.addInitScript(() => {
    let streamCount = 0;
    class LateEventSource extends EventTarget {
      onerror: ((event: Event) => void) | null = null;

      constructor(_url: string | URL, _options?: EventSourceInit) {
        super();
        streamCount += 1;
        if (streamCount === 1) {
          window.setTimeout(() => {
            this.dispatchEvent(new MessageEvent("done", { data: JSON.stringify({ ok: true }) }));
          }, 0);
          return;
        }
        window.setTimeout(() => {
          this.dispatchEvent(new MessageEvent("results", {
            data: JSON.stringify({
              hits: [{
                file_path: "stale.ts",
                start_line: 1,
                end_line: 1,
                score: 1,
                preview: "stale result"
              }],
              elapsed_ms: 1
            })
          }));
        }, 100);
      }

      close(): void {}
    }

    Object.defineProperty(window, "EventSource", { value: LateEventSource });
  });
  await page.goto("/");
  await expect(page.locator("#tree .tree-row.folder").filter({ hasText: "src" })).toBeVisible();
  await page.route("**/api/tree**", async (route) => {
    const url = new URL(route.request().url());
    if (url.searchParams.get("path") === "src") {
      await new Promise((resolve) => setTimeout(resolve, 500));
    }
    await route.continue();
  });

  await page.locator("#query").fill("delayed result");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await page.locator("#tree .tree-row.folder").filter({ hasText: "src" }).click();

  await page.waitForTimeout(200);
  await expect(page.locator("#results .file-path").filter({ hasText: "stale.ts" })).toHaveCount(0);
});
