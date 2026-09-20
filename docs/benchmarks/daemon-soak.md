# Daemon mutation and restart soak

```sh
python3 scripts/soak_daemon.py --binary target/release/ig --repo . \
  --duration 1800 --restarts 2 --output benchmark-results/daemon-soak.json
```

This Linux-only check copies the corpus into a temporary repository, prepares hash
vectors, and issues concurrent requests over the real daemon protocol. There is
no CLI search fallback. It verifies exact indexed probe revisions, deletion and
recreation, then repeats after offline changes and process restarts. A stable
probe query also exercises result-cache invalidation.

Every process epoch first runs the same query/mutation workload for 30 seconds to
initialize lazy thread pools, then gets independent resource samples for its share
of the requested loaded duration. After discarding the first
20% for warmup, the medians of the first and last quarters of the remaining
samples must stay within these growth budgets: 32 MiB anonymous RSS, 96 MiB total
RSS, eight file descriptors, and four threads. Anonymous RSS is the leak signal.
Total RSS also counts mapped index segments: continuous reindexing replaces them
and the kernel reclaims their pages, so file-backed RSS moves by tens of MiB
within one epoch in both directions. Samples record total, anonymous, and
file-backed RSS. The report includes peaks and cooldown samples separately.
These are bounded-growth gates, not a proof that arbitrarily slow leaks cannot
exist. A restart cannot hide a failed epoch. Missing samples, failed queries,
stale content, insufficient samples, or a budget violation fail the run.

The JSON report and adjacent daemon log are retained on failure. Reports identify
the binary, source commit/dirty status, harness hash, per-epoch query counts,
mutations, correctness checks, and resource windows. Latency quantiles describe
the last 50,000 successful requests of each epoch; they are load diagnostics, not
isolated search benchmarks. The latency sample buffer is bounded.

The performance workflow runs 120 loaded seconds with two restarts on relevant
PRs, and 1,800 loaded seconds with two restarts on scheduled/manual runs. Each
epoch must have at least 30 loaded seconds and 20 actual samples. Use longer runs
for slow leaks; do not increase resource budgets just to turn a failure green.

## Many MCP sessions

```sh
python3 scripts/soak_mcp_sessions.py --binary target/release/ig --repo . \
  --mode short --work-dir /tmp/ivygrep-mcp-soak --output benchmark-results/mcp-session-soak.json
```

The daemon soak above drives one workspace over raw daemon RPC. Coding agents
use ivygrep differently: every Claude Code or Codex session starts its own
`ig --mcp` process, all of them share one auto-spawned daemon, and agent
worktrees under `<repo>/.claude/worktrees/` come and go all day. This
Linux-only harness starts real `ig --mcp` processes in an isolated
`IVYGREP_HOME` and runs up to six phases:

- **stampede**: N sessions start at once with no daemon. Every session must get
  search results, exactly one daemon may remain, and `daemon.pid` must name it.
- **lifecycle**: sessions end by SIGKILL, closed stdin, or closed stdout, half of
  them in the middle of a request. No session may outlive its client by more
  than 30 s, and the daemon's anonymous RSS, descriptors, and threads must stay
  within the budgets below from a warmed baseline to the end.
- **load**: N concurrent sessions issue hybrid searches with short and long
  queries, literal, regex, and symbol searches, context packs, and `ig_status`
  against several workspaces. One query in three is unique, so the 128-entry
  query cache keeps evicting. The daemon and every session are sampled from
  `/proc/<pid>/smaps_rollup`, `status`, `fd`, and `fdinfo`. The daemon soak's
  gate (first and last quarter medians after 20% warmup) applies to the daemon
  with the same budgets, plus one descriptor and one thread per client (at most
  16 threads: the blocking pool follows the requests in flight) and 16 inotify
  watches, and to the largest session with 16 MiB, four descriptors, and two
  threads.
  The report adds least-squares slopes per hour with a 95% interval from the
  slope's standard error. Samples of one process are autocorrelated, so the
  interval understates the uncertainty. In long mode the sessions also stop
  calling for two minutes after the warmup and again after the last call
  (`--settle-every` adds pauses in between), and the idle daemon is sampled
  each time (`settled_gate`). Its descriptors and threads must be within the
  budgets, without requests in flight to blur them. Its anonymous RSS, taken
  after the idle trim, is reported without gating; the next section says why.
- **churn**: create a worktree under `.claude/worktrees`, search it through MCP
  (which indexes an overlay and registers a watcher), edit a file until the
  search returns the edit, and remove the worktree. After the run settles,
  threads, descriptors, inotify instances and watches, and index directories
  must be back at the baseline, which is taken after a warmup of half as many
  worktrees, at most 256, so that the daemon's per-workspace LRU caches (up to
  256 entries) are full and do not read as growth.
  While a worktree exists, the base must not return any of its files.
- **idle** (optional): CPU and context switches of a daemon that watches many
  workspaces while nobody calls it.
- **storm** (optional, with `--enable-enhancement`): edit many workspaces at
  once while one session keeps searching the last of them, and another session
  creates, edits, and searches a fresh agent worktree once the edits are
  visible. Reports how many enhancement workers run, wait, or pause together,
  what they hold, and when the searched workspace, the fresh worktree, and all
  workspaces have hash and neural vectors for their current index.

`--mode short` takes about two and a half minutes, 30 seconds of which warm the
daemon before the load phase samples it. `--mode long` warms for ten minutes,
runs 64 sessions for two hours, 1,000 lifecycle cycles, and 500 worktrees, and
settles for 100 seconds before each baseline and end sample so an idle daemon
has returned freed memory first. The harness only signals processes whose
environment names its own `IVYGREP_HOME`, and it passes none of the caller's
`IVYGREP_*` variables on; `--env KEY=VALUE` sets one for the sessions and the
daemon. The report records the value of `IVYGREP_*` and `MALLOC_*` settings,
which decide what was measured, and only the name of any other variable or of
one that looks like a credential. `--work-dir` must be outside `--repo`, and
`--output` outside `--work-dir`, which is removed after a successful run.
`--calls` restricts the load phase to chosen kinds of call, to see which kind
moves a resource.

### What the memory gate measures

The harness runs the daemon and the sessions with `MALLOC_ARENA_MAX=2` unless
`--malloc-arenas default` is given. glibc gives each thread its own malloc
arena, up to eight per core, and freed memory stays in the arena that freed it.
A daemon serving 64 sessions has about 125 threads, and 780 of its 787 MiB of
anonymous memory sat in 127 arenas, 0.9 MiB in the main heap. That retention
creeps for hours as arenas reach new high-water marks: in the two-hour run below
the quarter medians moved 38 MiB, past the 32 MiB budget, in a run whose last
half hour grew 5.5 MiB per hour. A budget cannot tell that from a leak. With two
arenas freed memory is reused across threads and anonymous RSS follows the
memory in use, so the same budgets catch a real leak: the three-hour run below
stayed between 273 and 299 MiB and passed. The cap is a test setting, not a
recommendation. With two arenas and 64 sessions the daemon served 43 calls per
second at a hybrid median of 1.2 s, against 151 calls per second and 0.25 s
with default arenas (four arenas at 32 sessions: 16% fewer calls and twice the
median latency). Numbers that describe production use
`--malloc-arenas default`, as the tables below do unless they say otherwise,
and with that setting no memory metric gates; descriptors, threads, and inotify
use still do.

The idle daemon after the trim is not a leak signal with default arenas either.
In one hour of 64 sessions (478,811 calls) it went from 245 to 291 MiB, while
about the same number of calls with two arenas moved RSS under load by 24 MiB
at most, and RSS under load cannot hide a leak. `malloc_trim` returns whole
free pages only, and after a busy hour more pages of a hundred arenas have
something live on them. What the allocator does matters more than any of these
gates can show; see "Allocators" below.

### Linux x86_64 measured run

Host: 16 vCPUs, 61 GiB, Linux 6.8, glibc 2.35, shared with another evaluation
workload and with this audit's own builds and test suites, so the load average
was 40 to 65 for most of the two-hour run. Treat latencies as loaded-host
numbers. Builds: `main` is `db7a225`; `fixed` is `b3184f6` plus the watcher
release, nested-checkout, index collection, log rotation, MCP runtime,
daemon context pack, and idle trim changes listed under Unreleased in the
changelog; the storm and the settled runs also have the enhancement worker
limit. Corpus: this repository, 33 MB per full workspace, neural profile
`static-retrieval-v1`, the default before `potion-code-16m-v2`. Reports: [two-hour load](mcp-session-soak-linux-x86_64-2h.json),
[three-hour load with two arenas](mcp-session-soak-linux-x86_64-3h-two-arenas.json),
[churn on main](mcp-session-soak-linux-x86_64-churn-main.json),
[churn on fixed](mcp-session-soak-linux-x86_64-churn-fixed.json),
[churn gate run with two arenas](mcp-session-soak-linux-x86_64-churn-gate.json),
[two-hour load with settled samples](mcp-session-soak-linux-x86_64-2h-settled-series.json),
the allocator comparison (linked in its section),
storm [before](mcp-session-soak-linux-x86_64-storm-before.json) and
[after](mcp-session-soak-linux-x86_64-storm-after.json) the enhancement worker
limit. The load reports keep one sample a minute (`published_sample_stride`);
their gates and slopes were computed from the 10-second samples. The runs were
made while the harness was being written, so `harness_sha256` in a report names
the revision that produced it, not the committed script; the workload was the
same throughout.

One `ig --mcp` process, fixed build (main in parentheses where it differs):

| State | Anonymous RSS | PSS | Threads | Descriptors |
| --- | --- | --- | --- | --- |
| after `initialize` | 1.4 MiB (1.6) | 2.4 MiB | 3 (19) | 9 |
| after 10,000 hybrid searches | 1.9 MiB (2.2) | 3.5 MiB | 3 (19) | 9 |
| after 501 context packs | 2.3 MiB (73.7) | 4.1 MiB (79.4) | 3 (44) | 9 |
| local fallback, one search | 46 MiB | 50 MiB | 28 (44) | 9 |
| local fallback, literal, regex, and 50 context packs | 88 MiB (86) | 92 MiB | 28 (44) | 9 |

Fifty sessions that all request context packs for two minutes: on main the
sessions held 3,588 MiB of anonymous memory and 2,200 threads and the daemon
237 MiB; on the fixed build the sessions held 92 MiB and 196 threads and the
daemon 660 MiB, which the idle trim takes to about 230 MiB. The daemon bounds
the builds with its CPU permits, so that burst ran 19% fewer packs per second
(17.2 against 21.3) at a higher median (2.8 s against 2.0 s) and a lower p95
(3.5 s against 3.9 s).

Stampede: thirteen rounds of 32 sessions started together with no daemon, under
host load. Each round spawned one to seven daemon processes, exactly one
remained after two seconds, `daemon.pid` named it, all 32 sessions got search
results within 2.3 s, and no session fell back to in-process work.

Lifecycle: 1,000 sessions ended by SIGKILL, closed stdin, or closed stdout, half
of them while the daemon built a context pack for them. No session outlived its
client, one daemon remained, and between the settled baseline and the settled
end the daemon's anonymous RSS grew 3.1 MiB, descriptors 0, threads 0.

Load, fixed build, default arenas, mixed calls over eight workspaces:

| Sessions | Time | Calls | Daemon anonymous RSS | Daemon threads | Descriptors | Hybrid p50 / p95 | Context pack p50 / p95 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 10 min | 7,648 | 286 to 290 MiB | 68 | 84 | 2.5 / 10 ms | 0.25 / 0.76 s |
| 8 | 10 min | 50,516 | 446 to 464 MiB | 95 | 127 to 136 | 4 / 21 ms | 0.35 / 1.1 s |
| 32 | 10 min | 88,634 | 676 to 726 MiB | 120 | 186 to 198 | 34 / 124 ms | 0.68 / 1.7 s |
| 64 | 2 h | 1,087,117 | 789 to 827 MiB | 125 | 233 to 237 | 253 / 465 ms | 0.99 / 2.1 s |

No call failed in any run. The ten-minute stages are still filling arenas, so
their growth says nothing about leaks. In the two-hour run the daemon went from
297 MiB to 762 MiB in seven minutes, crept to 800 MiB by minute 74, stepped to
822 MiB within seven minutes while a 50-session pack storm on main ran beside
it, and ended at 826 MiB. Least-squares slope after warmup: +32.6 ± 0.9 MiB per
hour over 1.6 hours; over the last 30 minutes +5.5 ± 1.0 MiB per hour. Threads
+0.2 ± 0.7 per hour, descriptors +2.4 ± 1.8 per hour. The 64 sessions together
held 179 MiB of anonymous memory (2.8 MiB each, the largest 4.3 MiB, slope 0)
and 880 descriptors. Search latencies of the last quarter were at or below the
first quarter's. `ig_status` and symbol lookups, which run in the MCP process,
slowed in the last quarter (p50 411 to 703 ms and 3.7 to 8.7 ms) while a
release build and a test suite ran on the same cores; the daemon-side calls,
which queue for the same 16 CPU permits all the time, did not.

Load with two arenas (`MALLOC_ARENA_MAX=2`), 64 sessions, three hours, build
with the idle trim: 465,528 calls, none failed, every gate passed. After warmup
the daemon's anonymous RSS stayed between 273 and 299 MiB and moved in steps,
down as well as up (-11.6 MiB at minute 36, -10.5 at minute 97, +12.7 at minute
153). Hourly medians: 275, 287, and 288 MiB. Quarter medians: +15.0 MiB against
the 32 MiB budget. Least-squares slope after warmup: +7.1 ± 0.6 MiB per hour
over 2.4 hours, most of it the last step. Taken as a leak that is at most
185 MiB per day at this load; three hours of a staircase cannot tell a leak of
that size from allocator high-water marks. It is still the tightest bound on
memory in use that these runs give: at most 24 MiB over 465,000 calls, staircase
included, which is 52 bytes per call. Descriptors grew
3.1 ± 0.3 per hour (243 to 249) and threads 0.6 ± 0.2 per hour (117 to
118), both inside their budgets. A census of the daemon's descriptors during
the one-hour run below shows what moves: sockets of requests in flight (55 to 64
under load, 4 when idle) and the SQLite handles of cached search contexts,
which went from 79 to 95 in 20 minutes and stayed there; the idle daemon had 165
descriptors before that hour and 170 after it. The 64 sessions held 178 MiB
together, the largest 4.1 MiB, slope 0.01 MiB per hour. No latency drifted
between the first and the last quarter.

Idle daemon after busy hours, default arenas, build with the enhancement worker
limit. The load pauses for two minutes, the daemon trims, and its anonymous RSS
is sampled. One hour (478,811 calls): 245 MiB before, 291 MiB after. Two hours
with a pause every twelve minutes (912,306 calls, none failed): 259, 287, 330,
340, 359, 373, 388, 382, 399, and 399 MiB, which is +140 MiB, 56 ± 14 MiB per
hour after the first interval, about 150 bytes per call, and not levelling off
within two hours. Threads stayed at 82 and descriptors went from 161 to 168.
Set against the two-arena bound of 52 bytes per call, at most a third of that
growth can be memory in use; the rest sits on partly used pages of about 127
arenas, where `malloc_trim` cannot return it. It is the number to plan with
for a daemon that serves saturating load for hours with glibc defaults.

#### Allocators: the shipped musl build, glibc, and one arena per core

Everything above is a glibc build. The Linux archives that `install.sh` and the
Homebrew formula install are static musl builds, whose allocator has no arenas
and no `malloc_trim`, and the idle trim is compiled out there. So the same tree
(main `b811aa8` plus this series) was built for `x86_64-unknown-linux-musl` the
way the release workflow builds it (cross, same revision) and for glibc, and
three daemons ran at the same time, each under 64 sessions for an hour with a
pause every twelve minutes: musl, glibc, and glibc with `MALLOC_ARENA_MAX=16`,
one arena per core. They shared the host with each other, with another load
run, and with two test suites (load average up to 135), so the latencies are
far above the two-hour run's; the comparison holds because all three ran at
once. Reports: [musl](mcp-session-soak-linux-x86_64-musl-1h.json),
[glibc](mcp-session-soak-linux-x86_64-glibc-1h.json),
[glibc with 16 arenas](mcp-session-soak-linux-x86_64-glibc-16-arenas-1h.json).

| 64 sessions, one hour, at the same time | musl (shipped) | glibc | glibc, 16 arenas |
| --- | --- | --- | --- |
| calls served, none failed | 61,326 | 153,393 | 137,817 |
| hybrid short, p50 / p95 | 2.2 / 4.1 s | 0.21 / 0.77 s | 0.20 / 0.79 s |
| literal, p50 / p95 | 2.4 / 4.4 s | 0.34 / 1.1 s | 0.35 / 1.2 s |
| context pack, p50 / p95 | 7.4 / 16 s | 2.0 / 4.3 s | 2.2 / 4.5 s |
| anonymous RSS under load, median / peak | 199 / 238 MiB | 597 / 633 MiB | 487 / 528 MiB |
| idle daemon at the six pauses, MiB | 176 175 180 174 184 185 | 211 230 256 261 290 289 | 151 147 161 174 178 170 |
| idle slope after the first interval | +10 ± 12 MiB/h | +75 ± 19 MiB/h | +32 ± 21 MiB/h |
| threads idle / peak, descriptors idle | 82 / 128, 155 to 163 | 82 / 115, 146 to 151 | 82 / 119, 147 to 154 |

With 8 sessions per daemon instead of 64, again at the same time: musl served
10,854 calls in ten minutes and glibc 32,475; hybrid short p50 20 against 11 ms,
literal 139 against 34 ms, context packs 2.7 against 0.62 s; anonymous RSS
under load 129 to 162 MiB against 469 to 474 MiB.

What this says:

- **musl** keeps the daemon small without any trim: it gives memory back as it
  is freed, the idle daemon stayed between 174 and 185 MiB for the hour, and
  under load it used a third of what glibc did. It pays for that in the
  allocator's lock: every thread allocates through it, and with 8 busy sessions
  the daemon already served a third of the calls at two to four times the
  latency. A musl daemon that is idle after a burst simply is small; nothing
  needs to run.
- **glibc** is fast and keeps memory: see the idle trim and the arena numbers
  above. One arena per core costs about 10% of the calls at unchanged median
  latency, lowers memory under load by about a fifth, and halves the growth of
  the idle daemon; two or four arenas cost far more (above).
- The macOS allocator was not measured: the harness reads `/proc`, and no macOS
  host was available. Nothing here says how a macOS daemon behaves.

An explicit allocator is the obvious next question, so one was tried, outside
this series: the same tree with mimalloc as the global allocator, against the
unchanged builds, four daemons at the same time, 16 sessions each for fifteen
minutes, idle samples two minutes before and after the load, on the same
overloaded host:

| 16 sessions, at the same time | musl | musl + mimalloc | glibc | glibc + mimalloc |
| --- | --- | --- | --- | --- |
| calls served | 14,708 | 19,767 | 29,519 | 31,937 |
| literal p50 / context pack p50 | 230 ms / 4.5 s | 98 ms / 2.6 s | 83 ms / 1.3 s | 77 ms / 1.3 s |
| anonymous RSS under load, median | 160 MiB | 483 MiB | 547 MiB | 783 MiB |
| idle daemon before / after the load | 138 / 156 MiB | 389 / 407 MiB | 217 / 271 MiB | 451 / 490 MiB |

With its defaults mimalloc makes the musl build a third faster and three times
larger, does little for glibc, and keeps more memory when idle than either
system allocator; the idle trim does not reach it. An allocator switch is a
trade to tune and measure on its own, not a free fix.

Where the memory of a busy hour goes, measured on the glibc build with a
preloaded shim that logs malloc's own count of bytes in use (`mallinfo2`,
allocated chunks plus mmapped chunks over all arenas) every five seconds. Under
the mixed 64-session load (319,198 calls) the daemon had 83 to 197 MiB in use,
median 127 MiB, while its arenas held 750 to 854 MiB: more than 600 MiB of the
anonymous RSS was free space inside arenas. One kind of request at a time, 64
sessions, with a 20-second pause every 150 seconds to read the heap with
nothing in flight:

| Only these calls | Calls | Idle live heap at the pauses, MiB | From the second pause to the last |
| --- | --- | --- | --- |
| hybrid, short and long queries | 206,170 | 76.8 74.7 74.7 77.2 77.6 77.8 77.7 | +3.0 MiB over 165,736 calls; +0.5 MiB over the last 85,000 |
| hybrid scoped to a directory or file | 705,834 | 75.3 70.4 79.6 77.1 78.2 76.2 72.1 | +1.7 MiB over 567,098 calls, inside the scatter |
| literal | 92,542 | 37.5 36.8 35.8 37.4 37.0 38.4 38.0 | +1.2 MiB over 74,431 calls, inside the scatter |
| regex | 71,554 | 15.8 16.0 16.8 17.1 17.5 17.6 17.7 | +1.7 MiB over 57,642 calls; +0.1 MiB over the last 16,000 |
| context packs | 8,959 | 42.8 45.9 46.3 48.9 48.8 47.8 44.6 | -1.3 MiB; too few calls to bound anything under about 700 bytes per call |
| `ig_status` (runs in the MCP process) | 43,286 | 11.77 11.80 11.82 11.82 11.82 11.82 11.82 | the daemon is not involved; sessions flat |

The idle heap moves by one to three MiB in the first tens of thousands of calls
and then stops: caches filling, not a leak. Context packs are the weak spot of
this table, because they are slow; the mixed runs above contain one pack in ten
calls, 32,000 in the run with the shim, with memory in use flat. Every structure in the daemon that
is keyed by a query, a request, or a workspace is an LRU table or has a cap, and
nothing in these runs contradicts that.

Idle trim: after 64 sessions stopped calling, the daemon's `malloc_trim` call
took 77 ms and took anonymous RSS from 661 MiB to 226 MiB; with 32 sessions,
697 MiB to 228 MiB. A daemon without the trim kept 668 MiB for as long as it
was idle.

Worktree churn, 500 worktrees, default arenas:

| | main | fixed |
| --- | --- | --- |
| daemon anonymous RSS, start to end | 104 to 790 MiB | 72 to 114 MiB, both after the idle trim |
| threads | 44 to 551 | 44 to 44 |
| descriptors | 18 to 1,514 | 21 to 21 |
| inotify instances | 1 to 501 | 1 to 1 |
| index directories, bytes | 1 to 501, 13 to 180 MB | 1 to 1, 13 to 13 MB |
| `daemon.log` | 5.0 MB in 30 minutes | 0.5 MB |
| seconds per worktree | 3.5 | 2.2 |
| base results from a nested worktree | every search | none |

The fixed daemon's 42 MiB are not state per worktree. Its LRU caches per
workspace hold up to 256 entries and fill until that many distinct worktrees
were seen, and memory freed in between stays scattered over the arenas. With two
arenas the same daemon stayed between 119.5 and 120.6 MiB from the 100th to the
300th worktree. The gate run warms with 250 worktrees and compares samples
taken after the idle trim: 22.5 MiB before and 35.7 MiB after the 500 measured
worktrees (+13.2 MiB against a budget of 32), with threads, descriptors,
inotify instances and watches, and index directories back at the baseline.
While it ran, the fixed build held 10 to 14 index directories: deleted
worktrees waiting out the 20-second test grace period.

Idle: a daemon watching 40 workspaces used 1.7% of one core and woke 102 times
per second (each watcher writes a two-second heartbeat), with 114 threads, 259
descriptors, 40 inotify instances, and 2,972 watches.

Background storm, 20 workspaces edited at once, same neural profile, load
guard off (`IVYGREP_ENHANCE_MAX_LOAD_RATIO=0`), before and after the limit of
two workers per lane (`IVYGREP_ENHANCE_MAX_WORKERS`). Both builds ran back to
back, twice; the first pair ran while the host's load average was 80 to 95, the
second at 37 to 50:

| | before, loaded | after, loaded | before | after |
| --- | --- | --- | --- | --- |
| enhancement workers at once | 12 | 4, one more waiting | 20 | 4, one more waiting |
| memory of all workers at the peak | 440 MiB | 198 MiB | 1,278 MiB | 237 MiB |
| edits searchable in all 20 workspaces | 44 s | 23 s | 5.1 s | 2.7 s |
| searched workspace: hash vectors current | 44 s | 22 s | 3.7 s | 3.2 s |
| searched workspace: neural vectors current | 57 s | 38 s | 12.8 s | 12.2 s |
| all 20 workspaces: neural vectors current | 60 s | 72 s | 17.8 s | 53.5 s |
| fresh worktree: first answer | 15.7 s | 6.6 s | 0.7 s | 0.6 s |
| fresh worktree: hash vectors current | 21 s | 12 s | 2.2 s | 1.9 s |

The limit trades the time until the last workspace is done (three times longer
on a host with idle cores) for a peak that no longer grows with the number of
edited workspaces. The workspace that is being searched and a fresh worktree
are served as fast as before or faster, because the queue puts them first and
because fewer workers compete with index updates. With the load guard holding
every worker back (`IVYGREP_ENHANCE_MAX_LOAD_RATIO=0.01`), 21 paused workers
held 363 MiB before the change; after it four workers waited with 41 MiB and
nothing loaded. Before the change, workers that the guard paused in the middle
of a neural run kept their models: 20 of them held 1.2 GiB for six minutes on a
host above 2.0 load per core.

These runs bound what was observed. Two or three hours do not prove a week: the
supported claim is the slope and interval above at that load, not the absence
of slower growth.

## Linux ARM64 acceptance run

The [short-run report](daemon-soak-linux-arm64-short.json) records 79,753 successful
RPC queries, 51 content checks, and two restarts against main `7413229` on
2026-09-02. All three process epochs passed the unchanged budgets; maximum
steady-window RSS growth was 7.94 MiB, FD growth was zero or negative, and thread
growth was zero or negative. This refreshed report uses the exact committed
harness, including platform and CPU-affinity metadata. This is short-run
acceptance, not long-soak evidence.

The [30-minute report](daemon-soak-linux-arm64-30m.json) also passed, with eight-core
affinity and the same budgets. Across 1,800 loaded seconds plus per-process
warmup/cooldown, it completed 716,956 RPC queries, 6,050 mutations, 465 exact-content
checks, and two restarts, with zero query errors. Maximum steady-window RSS growth
was 7.73 MiB; FD growth was nonpositive and thread growth was zero in every epoch.
The source was clean main `7413229`, and the report's harness checksum matches this
script. These windows bound observed growth; they do not prove absence of leaks
slower than the thresholds.
