import { describe, expect, it } from "vitest";
import { searchQuery, uniqueHits } from "./ui-utils";

describe("search input", () => {
  it("keeps spaces only in regex patterns", () => {
    expect(searchQuery("regex", "\n foo \r\n")).toBe(" foo ");
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
