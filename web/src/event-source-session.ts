export function guardCurrentStream<TStream, TEvent>(
  current: () => TStream | null,
  expected: TStream,
  handler: (event: TEvent) => void
): (event: TEvent) => void {
  return (event) => {
    if (current() !== expected) return;
    handler(event);
  };
}

export class LatestRequestGuard {
  private controller: AbortController | null = null;

  start(): AbortSignal {
    this.controller?.abort();
    this.controller = new AbortController();
    return this.controller.signal;
  }

  cancel(): void {
    this.controller?.abort();
    this.controller = null;
  }

  isCurrent(signal: AbortSignal): boolean {
    return this.controller?.signal === signal && !signal.aborted;
  }
}

export function searchCompletionStatus(ok: boolean | undefined, errors: string[]): string {
  if (ok !== false && errors.length === 0) return "Ready";
  return errors.length ? `Search incomplete: ${errors.join("; ")}` : "Search failed";
}
