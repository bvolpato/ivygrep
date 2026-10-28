import { readFileSync } from "node:fs";
import { expect, test } from "@playwright/test";

test("documentation typing keeps escaped markup as literal text", async ({ page }) => {
  const payload = '<img src=x onerror="window.__ivygrepDomXss=true">';
  const escapedPayload = payload.replaceAll("<", "&lt;").replaceAll(">", "&gt;");
  const html = readFileSync(new URL("../../docs/index.html", import.meta.url), "utf8")
    .replace(/(<div class="line typing">)[\s\S]*?(<\/div>)/,
      (_, start, end) => `${start}<span class="prompt">$</span> ${escapedPayload}${end}`);

  await page.route("**/*", route => route.fulfill({ body: "" }));
  await page.setContent(html, { waitUntil: "domcontentloaded" });

  const line = page.locator(".typing");
  await expect(line).toHaveText(`$ ${payload}`);
  await expect(line.locator("img")).toHaveCount(0);
  await expect(line.locator(".prompt")).toHaveText("$");
  expect(await page.evaluate("window.__ivygrepDomXss === true")).toBe(false);
});
