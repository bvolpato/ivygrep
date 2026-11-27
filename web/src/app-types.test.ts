import { describe, expect, it } from "vitest";
import { ALL_WORKSPACES, initialSelection } from "./app-types";

const boot = { query: "boot query", workspace: "/boot" };

describe("initial selection", () => {
  it("restores the URL state instead of the boot values", () => {
    expect(initialSelection(`?q=x&workspace=${ALL_WORKSPACES}`, boot)).toEqual({ query: "x", workspace: "" });
    expect(initialSelection("?workspace=/repo/src", boot)).toEqual({ query: "", workspace: "/repo/src" });
  });

  it("uses the boot values for a bare URL", () => {
    expect(initialSelection("", boot)).toEqual({ query: "boot query", workspace: "/boot" });
    expect(initialSelection("", {})).toEqual({ query: "", workspace: "" });
  });
});
