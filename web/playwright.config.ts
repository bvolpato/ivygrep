import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  testMatch: "**/*.pw.ts",
  timeout: 30_000,
  expect: { timeout: 10_000 },
  fullyParallel: true,
  reporter: "line",
  use: {
    baseURL: process.env.IVYGREP_WEB_URL || "http://127.0.0.1:4747",
    browserName: "chromium",
    headless: true,
    trace: "retain-on-failure"
  }
});
