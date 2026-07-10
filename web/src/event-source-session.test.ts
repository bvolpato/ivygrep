import { describe, expect, it, vi } from "vitest";
import {
  guardCurrentStream,
  LatestRequestGuard,
  searchCompletionStatus
} from "./event-source-session";

describe("guardCurrentStream", () => {
  it("ignores callbacks from a replaced stream", () => {
    const oldStream = {};
    const newStream = {};
    let current: object | null = newStream;
    const handler = vi.fn();

    guardCurrentStream(() => current, oldStream, handler)(new Event("done"));

    expect(handler).not.toHaveBeenCalled();
    current = null;
    guardCurrentStream(() => current, oldStream, handler)(new Event("done"));
    expect(handler).not.toHaveBeenCalled();
    current = oldStream;
    guardCurrentStream(() => current, oldStream, handler)(new Event("done"));
    expect(handler).toHaveBeenCalledOnce();
  });
});

describe("LatestRequestGuard", () => {
  it("aborts replaced requests and accepts only latest response", () => {
    const requests = new LatestRequestGuard();
    const first = requests.start();
    const second = requests.start();

    expect(first.aborted).toBe(true);
    expect(requests.isCurrent(first)).toBe(false);
    expect(requests.isCurrent(second)).toBe(true);

    requests.cancel();
    expect(second.aborted).toBe(true);
    expect(requests.isCurrent(second)).toBe(false);
  });
});

describe("searchCompletionStatus", () => {
  it("distinguishes success, partial errors, and failed searches", () => {
    expect(searchCompletionStatus(true, [])).toBe("Ready");
    expect(searchCompletionStatus(false, [])).toBe("Search failed");
    expect(searchCompletionStatus(false, ["repo: index unavailable"])).toBe(
      "Search incomplete: repo: index unavailable"
    );
  });
});
