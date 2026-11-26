import { test, expect } from "@playwright/test";

// `ig --web` prints a URL carrying the session token, loopback included.
// Visiting `/?token=...` sets the session cookie and forwards to `/`.
function sessionEntryPath(): string {
  const printed = process.env.IVYGREP_WEB_URL;
  const token = printed ? new URL(printed).searchParams.get("token") : null;
  return token ? `/?token=${encodeURIComponent(token)}` : "/";
}

const projectRoot = new URL(process.env.IVYGREP_WEB_URL ?? "http://localhost").searchParams.get("workspace") ?? "";

test("opens through the launcher redirect file and calls the API with the session cookie", async ({ page }) => {
  // scripts/e2e_web_ui.sh runs `ig --web` with a recording opener and passes on
  // the one argument it received: the owner-only redirect file, not the token URL.
  const redirectUrl = process.env.IVYGREP_REDIRECT_URL;
  test.skip(!redirectUrl, "IVYGREP_REDIRECT_URL is set by scripts/e2e_web_ui.sh");
  const printed = new URL(process.env.IVYGREP_WEB_URL ?? "");

  // file:// page -> cross-site /?token=... -> token-free / on the server origin.
  await page.goto(redirectUrl!);
  await page.waitForURL((url) => url.origin === printed.origin && url.pathname === "/" && !url.searchParams.has("token"));

  await expect(page.locator("#workspaces .workspace").filter({ hasText: "project" })).toBeVisible();
  const status = await page.evaluate(async () => (await fetch("/api/status")).status);
  expect(status).toBe(200);
});

test("searches an indexed workspace and opens the result in the viewer", async ({ page }) => {
  await page.goto(sessionEntryPath());

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
  await page.goto(sessionEntryPath());
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

test("a URL restores its scope, and an untracked path is not searched elsewhere", async ({ page }) => {
  await page.goto(sessionEntryPath());
  await expect(page.locator("#workspaces .workspace").filter({ hasText: "project" })).toBeVisible();

  const query = "q=semantic+browser+marker";
  await page.goto(`/?${query}&workspace=${encodeURIComponent(`${projectRoot}/src/web.ts`)}`);
  await expect(page.locator("#results .file-path")).toHaveText(["src/web.ts"]);
  await expect(page.locator("#tree .tree-row.file")).toHaveText([/web\.ts/]);

  await page.locator("#workspaces .workspace").filter({ hasText: "All indices" }).click();
  await expect(page.locator("#results .file-path").first()).toContainText(`${projectRoot}/src/web.ts`);
  await page.reload();
  await expect(page.locator("#results .file-path").first()).toContainText(`${projectRoot}/src/web.ts`);
  const activeWorkspace = page.locator("#workspaces .workspace.active");
  await expect(activeWorkspace).toContainText("All indices");

  await page.goto(`/?${query}&workspace=/not/tracked`);
  await expect(page.locator("#summary")).toHaveText("/not/tracked is not tracked.");
  await expect(page.locator("#results .hit")).toHaveCount(0);

  // `ig --add` can track the path of `ig --web` after the page loads.
  const otherRoot = projectRoot.replace(/project$/, "other");
  await page.route("**/api/status", async (route) => {
    const response = await route.fetch();
    const status = await response.json() as { workspaces: { root: string }[] };
    status.workspaces = status.workspaces.filter((ws) => ws.root !== otherRoot);
    await route.fulfill({ response, json: status });
  });
  await page.goto(`/?workspace=${encodeURIComponent(otherRoot)}`);
  await expect(activeWorkspace).toContainText("All indices");
  await page.unroute("**/api/status");
  await page.getByRole("button", { name: "Refresh" }).click();
  await expect(activeWorkspace).toContainText("other");
});

test("a search that reached its deadline shows partial results", async ({ page }) => {
  await page.goto(sessionEntryPath());
  await expect(page.locator("#workspaces .workspace").filter({ hasText: "project" })).toBeVisible();
  const results = JSON.stringify({ hits: [], warnings: ["search deadline of 60s exceeded; returning partial results"] });
  await page.route("**/api/search/stream?*", (route) => route.fulfill({
    contentType: "text/event-stream",
    body: `event: results\ndata: ${results}\n\nevent: done\ndata: {"ok":true}\n\n`
  }));
  await page.locator("#query").fill("slow query");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await expect(page.locator(".search-warnings strong")).toHaveText("Partial results");
});

test("the viewer keeps source text and the results keep their workspace", async ({ page, context }) => {
  await page.goto(sessionEntryPath());
  await expect(page.locator("#tree .tree-row.folder").filter({ hasText: "docs" })).toBeVisible();
  await page.locator("#query").fill("guide link marker");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await page.locator("#results .hit-main").filter({ hasText: "guide.md" }).first().click();
  await page.locator("#file-view").getByRole("link", { name: "source" }).click();
  await expect(page.locator("#viewer-title")).toHaveText("src/web.ts");
  const selected = await page.locator("#file-view .code").evaluate((code) => {
    getSelection()?.selectAllChildren(code);
    return getSelection()?.toString() ?? "";
  });
  expect(selected).toContain("semantic browser marker");
  expect(selected).not.toContain("Line 1:");

  await page.locator("#query").fill("");
  await page.locator("#workspaces .workspace").filter({ hasText: "other" }).click();
  await expect(page.locator("#results .hit")).toHaveCount(0);

  await context.clearCookies();
  await page.locator("#query").fill("lost session");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await expect(page.locator("#summary")).toContainText("authentication required");
});
