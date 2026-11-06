# Clean Git index reuse

Measured on November 6, 2026, on a Ryzen 9 9950X3D with 32 logical CPUs,
121 GiB RAM, Linux 6.17, and Rust 1.98.1. Builds and measurements ran separately.
These are shared-workstation measurements; filesystem caches were not cleared.

## Change and source certification

Clean Git rechecks previously allocated a normalized `BTreeSet<String>` for
every tracked path, even when an existing certified index could be reused.
Capture now retains the Git output bytes and builds the same membership set
only when certifying an index after indexing.

Capture still validates every record's flags and UTF-8 immediately and discovers
tracked `.ignore` controls. Certification still requires an unchanged checkout,
tracked membership for every indexed input, and matching source metadata.
Transient or deleted untracked inputs, hidden edits, and changed ignore controls
retain their existing invalidation behavior. Marker contents and formats stay
the same. Actual indexing retains the raw Git output until certification and
parses those records again when building the set.

## Identity and method

Baseline source `0e3457743c42f887b7ac835d0b06ae0bea4234db` has the same tree as
merged main `8877bfd1fb10d26d8cc78d6ace7c04bbd5bcda52`. Both binaries were built
from the same checkout path with matching release/ThinLTO settings,
`CARGO_PROFILE_RELEASE_DEBUG=1`, and `CARGO_PROFILE_RELEASE_STRIP=none`.

| Identity | SHA-256 |
| --- | --- |
| Baseline binary | `5bd1a5809a0ef0c33eb769f23083049fd76874f3051f3bfcabb4db278d5c343c` |
| Candidate binary | `04ef7d35479a4c17cab445d3019d47b2662e8346411c7aff514fe5e992f0c5a2` |
| Measured `git diff <baseline> -- src/indexer/git_state.rs` | `804b42ac047e1bd74c7a1ecef00cf6f247bc91a8a012bd859f9dc3cb549120b3` |
| 10,000-file corpus | `e50e1175f78b488ad57448f9e89a0fe252b1eb5509cd363eed9f375309b81f23` |
| 100,000-file corpus | `ee38ca315827e14e9cd87ca7f914629632fc3d95b5f953393d7f3d69ed643017` |

The committed fixtures contain simple Rust files in 64 and 256 directories.
Each file has a unique function; the first has an extra constant. Indexing
produces 10,001 and 100,001 chunks. Both variants use the same source repository.
Fresh trials use new app homes. Reuse trials share one preindexed app home.

The driver invokes `ig --add <corpus> --hash --no-watch --json` with daemon
autospawn and background enhancement disabled. JSON output verifies synchronous
completion, expected indexed file count, nonzero chunk count, and zero deletions.
Reuse also preserves the preindexed chunk count. Every reuse call checks that
the certification marker's inode, size, mtime, and ctime remain unchanged.
Fixture Git status and commit are checked before and after the run.

Fresh indexing has three paired repetitions per fixture. Reuse has five paired
blocks of 20 invocations per binary, alternating execution order between blocks.
The table reports pooled per-invocation medians, not medians of block statistics.
Each invocation starts a new CLI process; these are not daemon-only timings.

Wall time covers the invocation and small output-file overhead. CPU and maximum
RSS come from Linux `wait4` resource usage, including reaped Git descendants.
CPU can exceed elapsed time because work runs in parallel. RSS is the reported
maximum for an invocation, not a simultaneous sum across its process tree.

## Results

| Workload | Files | Wall before / after | CPU before / after | Peak RSS before / after |
| --- | ---: | ---: | ---: | ---: |
| Clean reuse | 10,000 | 30.69 / 26.27 ms | 56.92 / 50.48 ms | 17.90 / 17.13 MiB |
| Clean reuse | 100,000 | 160.66 / 151.39 ms | 364.65 / 350.86 ms | 36.50 / 27.71 MiB |
| Fresh index | 10,000 | 378.20 / 309.14 ms | 1,124.74 / 1,074.16 ms | 60.36 / 61.05 MiB |
| Fresh index | 100,000 | 1,775.01 / 1,562.58 ms | 9,433.14 / 9,071.97 ms | 226.28 / 222.07 MiB |

Clean reuse is about 14% faster at 10k files and 6% faster at 100k files, with
about 11% and 4% less CPU respectively. At 100k files, reported peak RSS falls
about 24%, or 8.8 MiB. One 100k block has a slower candidate wall median, so the
improvement is not uniform across blocks. Git subprocess work and CLI startup
remain material costs.

Fresh indexing shows lower median time and CPU in these trials, with overlapping
CPU ranges: 1,063–1,211 / 1,021–1,129 ms at 10k files and
9,203–9,977 / 8,965–9,515 ms at 100k files. Fresh 10k peak RSS rises slightly.
The small fresh-trial sample does not establish a general fresh-index speedup.
These fixtures do not measure complex syntax, submodule workloads, cold disk
access, or neural enhancement. Source integrity tests cover the certification
contracts separately.

## Reproduce

This diagnostic used a temporary driver, not a packaged benchmark. The existing
Criterion `incremental_reindex_no_change` fixture has an empty `.git` directory;
it does not exercise certified clean-Git reuse. Use an actual committed fixture.
The following generator reproduces the file bytes for the 10k fixture. Use
`files=100000` and `directories=256` for the larger fixture. Use a new fixture
path and configure a Git identity and signing key before the signed commit.

```sh
uv run --script - <<'PY'
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
from pathlib import Path
root = Path('/tmp/ig-clean-git-corpus')
root.mkdir(parents=True)
files, directories = 10000, 64
for i in range(files):
    folder = root / 'src' / f'group_{i % directories:02d}'
    folder.mkdir(parents=True, exist_ok=True)
    body = [f'// deterministic fixture file {i:05d}',
            f'pub fn sample_{i:05d}() -> usize {{ {i} }}']
    if i == 0:
        body.append('pub const SEARCH_NEEDLE: &str = "needle_quartz_rare_0001";')
    (folder / f'file_{i:05d}.rs').write_text('\n'.join(body) + '\n')
PY
git -C /tmp/ig-clean-git-corpus init --initial-branch=main
git -C /tmp/ig-clean-git-corpus add .
git -C /tmp/ig-clean-git-corpus commit -S -m 'Prepare committed index fixture'
IVYGREP_HOME=/tmp/ig-clean-git-home IVYGREP_NO_AUTOSPAWN=1 \
  IVYGREP_DISABLE_BACKGROUND_ENHANCEMENT=1 \
  /tmp/ig-candidate --add /tmp/ig-clean-git-corpus --hash --no-watch --json
```

Use new app homes for fresh trials. Repeat against the same certified home for
reuse trials, alternating binary order and checking output counts and unchanged
certification markers. Keep fixture generation, builds, and measurements separate.
