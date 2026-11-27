import { readFileSync } from "node:fs";
import { expect, test } from "@playwright/test";

test("documentation typing keeps escaped markup as literal text", async ({ page }) => {
  const payload = '<img src=x onerror="window.__ivygrepDomXss=true">';
  const escapedPayload = payload.replaceAll("<", "&lt;").replaceAll(">", "&gt;");
  const docs = (name: string) => readFileSync(new URL(`../../docs/${name}`, import.meta.url), "utf8");
  const html = docs("index.html")
    .replace(/(<button role="tab" id="q1"[^>]*>)[\s\S]*?(<\/button>)/,
      (_, start, end) => `${start}${escapedPayload}${end}`);

  await page.route("**/*", route => {
    const path = new URL(route.request().url()).pathname;
    if (path === "/") return route.fulfill({ contentType: "text/html", body: html });
    if (path === "/site.js") return route.fulfill({ contentType: "text/javascript", body: docs("site.js") });
    return route.fulfill({ body: "" });
  });
  await page.goto("http://docs.invalid/");

  const tab = page.locator("#q1");
  await expect(tab).toHaveText(payload);
  await tab.click();

  const question = page.locator("#a1 .ask q");
  await expect(question).toHaveText(payload);
  await expect(question.locator("img")).toHaveCount(0);
  expect(await page.evaluate("window.__ivygrepDomXss === true")).toBe(false);
});
