import { describe, expect, it } from "vitest";
import { searchQuery, uniqueHits, workspaceForPath } from "./ui-utils";

describe("search input", () => {
  it("keeps spaces only in patterns", () => {
    expect(searchQuery("regex", "\n foo \r\n")).toBe(" foo ");
    expect(searchQuery("literal", "\n foo \r\n")).toBe(" foo ");
    expect(searchQuery("hybrid", " foo \n")).toBe("foo");
  });
});

describe("search hits", () => {
  it("keeps one hit for each file range", () => {
    const first = { file_path: "a.txt", start_line: 1, end_line: 3 };
    const other = { file_path: "b.txt", start_line: 1, end_line: 3 };
    expect(uniqueHits([first, { ...first }, other])).toEqual([first, other]);
  });
});

describe("workspace selection", () => {
  const workspaces = [{ root: "/repo" }, { root: "/repo/inner" }, { root: "/other" }];

  it("selects the innermost workspace that contains the path", () => {
    expect(workspaceForPath(workspaces, "/repo/inner/src")).toEqual({ root: "/repo/inner", scope: "src" });
    expect(workspaceForPath(workspaces, "/repo/innermost/a.rs")).toEqual({ root: "/repo", scope: "innermost/a.rs" });
  });

  it("selects no workspace for an untracked path", () => {
    expect(workspaceForPath(workspaces, "/repository")).toBeUndefined();
  });
});
