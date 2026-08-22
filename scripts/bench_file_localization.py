#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Repository file-localization benchmark: issue text -> files the fix touched.

Each task pins a public repository at the commit *before* a merged fix and asks
ivygrep to locate the files that fix changed, given only the linked issue text.
This is the File Acc@k / Recall@k / MRR protocol used by LocAgent and SweRank
on SWE-bench Lite, applied to small permissively licensed repositories so the
benchmark runs in minutes without a model download.

Input is JSONL with one task per line:

    {"task_id": ..., "repo": <git url or local path>, "base_commit": ...,
     "query": <issue title + body>, "gold_files": [...], "also_changed": [...]}

For every task the harness clones the repository into ``--cache-dir``,
materializes ``base_commit`` as a clean standalone Git checkout in a temporary
directory (so Git change scope is empty and no worktree overlay is involved),
indexes it with ``ig --add --hash --no-watch``, then scores two surfaces:

* ``ig --json --file-name-only -n N "<query>"`` (ranked file list)
* ``ig context "<query>" --json --budget B`` (context pack file set)

A ``--baseline lexical`` run repeats the queries with ``--lexical-only`` so the
report shows the delta of the selected mode over BM25-style retrieval.
"""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from typing import Any, Iterable


SCHEMA_VERSION = 1
REQUIRED_FIELDS = ("task_id", "repo", "base_commit", "query", "gold_files")
SEARCH_CUTOFFS = (1, 5, 10)
RECALL_CUTOFFS = (10, 20)
MODE_FLAGS = {
    "hash": ["--hash"],
    "lexical": ["--lexical-only"],
    "blended": [],
}
# ``ig context`` has no --force-neural; blended context uses the default route.
CONTEXT_MODE_FLAGS = {
    "hash": ["--hash"],
    "lexical": ["--lexical-only"],
    "blended": [],
}


class TaskError(ValueError):
    """Raised when a JSONL task row is malformed."""


# --------------------------------------------------------------------------- #
# Task parsing
# --------------------------------------------------------------------------- #


def parse_tasks(lines: Iterable[str], source: str = "<tasks>") -> list[dict[str, Any]]:
    tasks: list[dict[str, Any]] = []
    seen: set[str] = set()
    for number, raw in enumerate(lines, start=1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        try:
            row = json.loads(line)
        except json.JSONDecodeError as error:
            raise TaskError(f"{source}:{number}: invalid JSON: {error}") from error
        if not isinstance(row, dict):
            raise TaskError(f"{source}:{number}: task must be a JSON object")
        missing = [field for field in REQUIRED_FIELDS if field not in row]
        if missing:
            raise TaskError(f"{source}:{number}: missing fields {missing}")
        if not isinstance(row["gold_files"], list) or not row["gold_files"]:
            raise TaskError(f"{source}:{number}: gold_files must be a non-empty list")
        if not all(isinstance(path, str) and path for path in row["gold_files"]):
            raise TaskError(f"{source}:{number}: gold_files must contain path strings")
        if not isinstance(row["query"], str) or not row["query"].strip():
            raise TaskError(f"{source}:{number}: query must be a non-empty string")
        if not re.fullmatch(r"[0-9a-fA-F]{7,64}", str(row["base_commit"])):
            raise TaskError(f"{source}:{number}: base_commit must be a commit hash")
        task_id = str(row["task_id"])
        if task_id in seen:
            raise TaskError(f"{source}:{number}: duplicate task_id {task_id!r}")
        seen.add(task_id)
        row["task_id"] = task_id
        row["gold_files"] = [normalize_relative(path) for path in row["gold_files"]]
        row.setdefault("also_changed", [])
        tasks.append(row)
    if not tasks:
        raise TaskError(f"{source}: no tasks found")
    return tasks


def load_tasks(path: Path) -> list[dict[str, Any]]:
    with path.open(encoding="utf-8") as handle:
        return parse_tasks(handle, str(path))


def truncate_query(query: str, max_chars: int | None) -> tuple[str, bool]:
    text = query.strip()
    if max_chars is None or max_chars <= 0 or len(text) <= max_chars:
        return text, False
    return text[:max_chars].rstrip(), True


# --------------------------------------------------------------------------- #
# Metrics
# --------------------------------------------------------------------------- #


def normalize_relative(path: str) -> str:
    return Path(path).as_posix().removeprefix("./")


def normalize_path(path: str, repo: Path) -> str:
    candidate = Path(path)
    if candidate.is_absolute():
        for root in (repo, repo.resolve()):
            try:
                return candidate.relative_to(root).as_posix()
            except ValueError:
                continue
        try:
            return candidate.resolve().relative_to(repo.resolve()).as_posix()
        except (ValueError, OSError):
            return candidate.as_posix()
    return normalize_relative(path)


def unique_in_order(paths: Iterable[str]) -> list[str]:
    return list(dict.fromkeys(paths))


def rank_metrics(ranked: list[str], gold: Iterable[str]) -> dict[str, float]:
    """File Acc@k (any gold file in top k), Recall@k, and MRR over a ranked file list."""
    unique = unique_in_order(ranked)
    gold_set = set(gold)
    if not gold_set:
        raise ValueError("gold set is empty")
    first_rank = next(
        (rank for rank, path in enumerate(unique, start=1) if path in gold_set), None
    )
    metrics: dict[str, float] = {}
    for cutoff in SEARCH_CUTOFFS:
        metrics[f"acc_at_{cutoff}"] = float(first_rank is not None and first_rank <= cutoff)
    for cutoff in RECALL_CUTOFFS:
        metrics[f"recall_at_{cutoff}"] = len(gold_set.intersection(unique[:cutoff])) / len(
            gold_set
        )
    metrics["mrr"] = 1.0 / first_rank if first_rank else 0.0
    metrics["first_gold_rank"] = float(first_rank) if first_rank else 0.0
    metrics["returned_files"] = float(len(unique))
    return metrics


def pack_metrics(files: list[str], gold: Iterable[str]) -> dict[str, float]:
    """Precision/recall of a context pack's file set against the gold files."""
    unique = unique_in_order(files)
    gold_set = set(gold)
    if not gold_set:
        raise ValueError("gold set is empty")
    matched = gold_set.intersection(unique)
    return {
        "pack_files": float(len(unique)),
        "pack_matched": float(len(matched)),
        "pack_recall": len(matched) / len(gold_set),
        "pack_precision": len(matched) / len(unique) if unique else 0.0,
        "pack_hit": float(bool(matched)),
    }


AGGREGATE_KEYS = (
    "acc_at_1",
    "acc_at_5",
    "acc_at_10",
    "recall_at_10",
    "recall_at_20",
    "mrr",
    "pack_recall",
    "pack_precision",
    "pack_hit",
)


def aggregate(rows: list[dict[str, Any]]) -> dict[str, Any]:
    scored = [row for row in rows if row.get("status") == "ok"]
    summary: dict[str, Any] = {
        "tasks": len(rows),
        "scored_tasks": len(scored),
        "failed_tasks": len(rows) - len(scored),
    }
    for key in AGGREGATE_KEYS:
        values = [float(row["metrics"][key]) for row in scored if key in row["metrics"]]
        summary[key] = statistics.fmean(values) if values else 0.0
    search_latencies = [row["search_latency_ms"] for row in scored]
    context_latencies = [row["context_latency_ms"] for row in scored]
    summary["search_latency_p50_ms"] = statistics.median(search_latencies) if search_latencies else 0.0
    summary["context_latency_p50_ms"] = (
        statistics.median(context_latencies) if context_latencies else 0.0
    )
    summary["mean_pack_tokens"] = (
        statistics.fmean(row["pack_used_tokens"] for row in scored) if scored else 0.0
    )
    by_language: dict[str, list[dict[str, Any]]] = {}
    for row in scored:
        by_language.setdefault(str(row.get("language") or "unknown"), []).append(row)
    summary["by_language"] = {
        language: {
            "tasks": len(group),
            **{
                key: statistics.fmean(float(item["metrics"][key]) for item in group)
                for key in AGGREGATE_KEYS
            },
        }
        for language, group in sorted(by_language.items())
    }
    return summary


def delta(primary: dict[str, Any], baseline: dict[str, Any]) -> dict[str, float]:
    return {key: float(primary[key]) - float(baseline[key]) for key in AGGREGATE_KEYS}


def markdown_table(
    summaries: dict[str, dict[str, Any]], delta_row: dict[str, float] | None = None
) -> str:
    columns = [
        ("acc_at_1", "Acc@1"),
        ("acc_at_5", "Acc@5"),
        ("acc_at_10", "Acc@10"),
        ("recall_at_10", "R@10"),
        ("recall_at_20", "R@20"),
        ("mrr", "MRR"),
        ("pack_recall", "Pack R"),
        ("pack_precision", "Pack P"),
        ("pack_hit", "Pack hit"),
    ]
    lines = [
        "| Mode | Tasks | " + " | ".join(label for _, label in columns) + " |",
        "| --- | ---: | " + " | ".join("---:" for _ in columns) + " |",
    ]
    for mode, summary in summaries.items():
        lines.append(
            f"| {mode} | {summary['scored_tasks']}/{summary['tasks']} | "
            + " | ".join(f"{float(summary[key]):.3f}" for key, _ in columns)
            + " |"
        )
    if delta_row is not None:
        lines.append(
            "| delta | | "
            + " | ".join(f"{delta_row[key]:+.3f}" for key, _ in columns)
            + " |"
        )
    return "\n".join(lines)


# --------------------------------------------------------------------------- #
# Repository materialization
# --------------------------------------------------------------------------- #


def run(command: list[str], cwd: Path | None = None, env: dict[str, str] | None = None,
        timeout: float | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        cwd=cwd,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout,
    )


def git(repo: Path, *args: str) -> str:
    completed = run(["git", *args], cwd=repo)
    if completed.returncode != 0:
        raise RuntimeError(
            f"git {' '.join(args)} failed in {repo}: {completed.stderr.strip()}"
        )
    return completed.stdout


def cache_name(repo: str) -> str:
    slug = re.sub(r"[^A-Za-z0-9._-]+", "_", repo.rstrip("/").removesuffix(".git"))
    return slug.strip("_")[-100:] or "repo"


def ensure_clone(repo: str, cache_dir: Path, base_commit: str) -> Path:
    source = repo
    local = Path(repo).expanduser()
    if local.exists():
        source = str(local.resolve())
    clone = cache_dir / cache_name(source)
    if not clone.exists():
        cache_dir.mkdir(parents=True, exist_ok=True)
        completed = run(
            ["git", "clone", "--quiet", "--filter=blob:none", source, str(clone)]
        )
        if completed.returncode != 0:
            shutil.rmtree(clone, ignore_errors=True)
            raise RuntimeError(f"git clone {source} failed: {completed.stderr.strip()}")
    if run(["git", "cat-file", "-e", f"{base_commit}^{{commit}}"], cwd=clone).returncode != 0:
        fetch = run(["git", "fetch", "--quiet", "origin", base_commit], cwd=clone)
        if fetch.returncode != 0:
            fetch = run(["git", "fetch", "--quiet", "--all"], cwd=clone)
        if run(["git", "cat-file", "-e", f"{base_commit}^{{commit}}"], cwd=clone).returncode != 0:
            raise RuntimeError(f"{base_commit} not found in {source}: {fetch.stderr.strip()}")
    return clone


def materialize_checkout(clone: Path, base_commit: str, destination: Path) -> None:
    """Extract ``base_commit`` into ``destination`` as a clean standalone Git repo.

    A ``git worktree`` would make ivygrep build a worktree overlay on top of the
    cache clone, and a bare ``git init`` would mark every file as untracked in
    ``ig context`` change scope. A fresh repository with one commit avoids both.
    """
    destination.mkdir(parents=True, exist_ok=False)
    archive = subprocess.Popen(
        ["git", "archive", "--format=tar", base_commit],
        cwd=clone,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    extract = subprocess.run(
        ["tar", "-x", "-C", str(destination)],
        stdin=archive.stdout,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    assert archive.stdout is not None
    archive.stdout.close()
    _, archive_stderr = archive.communicate()
    if archive.returncode != 0 or extract.returncode != 0:
        raise RuntimeError(
            f"git archive {base_commit} failed: "
            f"{archive_stderr.decode(errors='replace').strip()} "
            f"{extract.stderr.decode(errors='replace').strip()}"
        )
    git(destination, "init", "--quiet")
    git(destination, "config", "user.email", "bench@ivygrep.invalid")
    git(destination, "config", "user.name", "ivygrep benchmark")
    git(destination, "config", "commit.gpgsign", "false")
    git(destination, "add", "-A", "--force")
    git(destination, "commit", "--quiet", "--allow-empty", "-m", f"snapshot {base_commit}")


# --------------------------------------------------------------------------- #
# ivygrep invocation
# --------------------------------------------------------------------------- #


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def tool_identity(binary: Path) -> dict[str, Any]:
    version = run([str(binary), "--version"])
    if version.returncode != 0:
        raise RuntimeError(f"{binary} --version failed: {version.stderr.strip()}")
    repo_root = Path(__file__).resolve().parents[1]
    commit = run(["git", "rev-parse", "HEAD"], cwd=repo_root)
    dirty = run(["git", "status", "--porcelain"], cwd=repo_root)
    return {
        "binary": binary.name,
        "binary_version": version.stdout.strip(),
        "binary_sha256": sha256_file(binary),
        "harness_sha256": sha256_file(Path(__file__)),
        "ivygrep_commit": commit.stdout.strip() if commit.returncode == 0 else None,
        "ivygrep_dirty": bool(dirty.stdout.strip()) if dirty.returncode == 0 else None,
    }


def ivygrep_env(home: Path) -> dict[str, str]:
    env = os.environ.copy()
    env["IVYGREP_HOME"] = str(home)
    env["IVYGREP_NO_AUTOSPAWN"] = "1"
    env["IVYGREP_DISABLE_BACKGROUND_ENHANCEMENT"] = "1"
    env["IVYGREP_ENHANCE_MAX_LOAD_RATIO"] = "0"
    env.pop("IVYGREP_FORCE_NEURAL", None)
    return env


def run_json(command: list[str], cwd: Path, env: dict[str, str], timeout: float) -> tuple[Any, float]:
    started = time.perf_counter()
    completed = run(command, cwd=cwd, env=env, timeout=timeout)
    elapsed_ms = (time.perf_counter() - started) * 1000.0
    if completed.returncode != 0:
        raise RuntimeError(
            f"command failed ({completed.returncode}): {' '.join(command[:6])} ...\n"
            + completed.stderr.strip()[-2000:]
        )
    try:
        return json.loads(completed.stdout), elapsed_ms
    except json.JSONDecodeError as error:
        raise RuntimeError(
            f"non-JSON output from {' '.join(command[:4])}: {completed.stdout[:500]!r}"
        ) from error


def search_command(binary: Path, mode: str, force_neural: bool, limit: int, query: str, repo: Path) -> list[str]:
    flags = list(MODE_FLAGS[mode])
    if force_neural and mode == "blended":
        flags = ["--force-neural"]
    return [
        str(binary),
        "--json",
        "--file-name-only",
        "-n",
        str(limit),
        "--no-watch",
        *flags,
        "--",
        query,
        str(repo),
    ]


def context_command(binary: Path, mode: str, budget: int, query: str, repo: Path) -> list[str]:
    return [
        str(binary),
        "context",
        "--json",
        "--no-watch",
        "--budget",
        str(budget),
        *CONTEXT_MODE_FLAGS[mode],
        "--",
        query,
        str(repo),
    ]


def search_paths(payload: Any, repo: Path) -> list[str]:
    if isinstance(payload, dict):
        payload = payload.get("files") or payload.get("hits") or []
    paths: list[str] = []
    for item in payload:
        if isinstance(item, str):
            paths.append(normalize_path(item, repo))
        elif isinstance(item, dict) and "file_path" in item:
            paths.append(normalize_path(str(item["file_path"]), repo))
    return unique_in_order(paths)


def context_paths(payload: dict[str, Any], repo: Path) -> list[str]:
    return unique_in_order(
        normalize_path(str(item["file_path"]), repo) for item in payload.get("items", [])
    )


def index_repo(binary: Path, repo: Path, env: dict[str, str], timeout: float, enhance_neural: bool) -> dict[str, float]:
    timings: dict[str, float] = {}
    started = time.perf_counter()
    completed = run(
        [str(binary), "--add", str(repo), "--force", "--hash", "--no-watch"],
        cwd=repo,
        env=env,
        timeout=timeout,
    )
    if completed.returncode != 0:
        raise RuntimeError(f"ig --add failed: {completed.stderr.strip()[-2000:]}")
    timings["index_ms"] = (time.perf_counter() - started) * 1000.0
    started = time.perf_counter()
    completed = run(
        [str(binary), "--enhance-hash-internal", str(repo)], cwd=repo, env=env, timeout=timeout
    )
    if completed.returncode != 0:
        raise RuntimeError(f"hash enhancement failed: {completed.stderr.strip()[-2000:]}")
    timings["hash_enhance_ms"] = (time.perf_counter() - started) * 1000.0
    if enhance_neural:
        started = time.perf_counter()
        completed = run(
            [str(binary), "--enhance-internal", str(repo)], cwd=repo, env=env, timeout=timeout
        )
        if completed.returncode != 0:
            raise RuntimeError(f"neural enhancement failed: {completed.stderr.strip()[-2000:]}")
        timings["neural_enhance_ms"] = (time.perf_counter() - started) * 1000.0
    return timings


def evaluate_mode(
    binary: Path,
    repo: Path,
    env: dict[str, str],
    mode: str,
    force_neural: bool,
    query: str,
    gold: list[str],
    args: argparse.Namespace,
) -> dict[str, Any]:
    row: dict[str, Any] = {"mode": mode}
    try:
        payload, search_ms = run_json(
            search_command(binary, mode, force_neural, args.search_limit, query, repo),
            repo,
            env,
            args.timeout_secs,
        )
        ranked = search_paths(payload, repo)
        pack, context_ms = run_json(
            context_command(binary, mode, args.budget, query, repo),
            repo,
            env,
            args.timeout_secs,
        )
    except subprocess.TimeoutExpired as error:
        row.update(status="timeout", error=f"timed out after {args.timeout_secs}s: {error.cmd[:3]}")
        return row
    except RuntimeError as error:
        row.update(status="error", error=str(error))
        return row
    pack_files = context_paths(pack, repo)
    row.update(
        status="ok",
        metrics={**rank_metrics(ranked, gold), **pack_metrics(pack_files, gold)},
        search_latency_ms=search_ms,
        context_latency_ms=context_ms,
        pack_used_tokens=int(pack.get("used_tokens") or 0),
        pack_candidate_count=int(pack.get("candidate_count") or 0),
        ranked_files=ranked[:args.search_limit],
        pack_files=pack_files,
    )
    return row


def evaluate_tasks(tasks: list[dict[str, Any]], modes: list[str], args: argparse.Namespace) -> dict[str, list[dict[str, Any]]]:
    binary = args.binary.resolve()
    cache_dir = args.cache_dir.resolve()
    rows: dict[str, list[dict[str, Any]]] = {mode: [] for mode in modes}
    for position, task in enumerate(tasks, start=1):
        query, truncated = truncate_query(task["query"], args.max_query_chars)
        base: dict[str, Any] = {
            "task_id": task["task_id"],
            "repo": task["repo"],
            "language": task.get("language"),
            "base_commit": task["base_commit"],
            "source_url": task.get("source_url"),
            "gold_files": task["gold_files"],
            "query_chars": len(query),
            "query_truncated": truncated,
        }
        print(f"[{position}/{len(tasks)}] {task['task_id']}", file=sys.stderr, flush=True)
        with tempfile.TemporaryDirectory(prefix="ivygrep-file-loc-") as temporary:
            root = Path(temporary)
            checkout = root / "repo"
            home = root / "home"
            home.mkdir()
            env = ivygrep_env(home)
            try:
                clone = ensure_clone(task["repo"], cache_dir, task["base_commit"])
                materialize_checkout(clone, task["base_commit"], checkout)
                missing_gold = [path for path in task["gold_files"] if not (checkout / path).is_file()]
                if missing_gold:
                    raise RuntimeError(f"gold files absent at base_commit: {missing_gold}")
                timings = index_repo(binary, checkout, env, args.timeout_secs * 4, args.enhance_neural)
            except (RuntimeError, subprocess.TimeoutExpired, OSError) as error:
                for mode in modes:
                    rows[mode].append({**base, "mode": mode, "status": "error", "error": str(error)[:2000]})
                continue
            for mode in modes:
                result = evaluate_mode(
                    binary, checkout, env, mode, args.force_neural, query, task["gold_files"], args
                )
                rows[mode].append({**base, **timings, **result})
    return rows


# --------------------------------------------------------------------------- #
# CLI
# --------------------------------------------------------------------------- #


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--tasks", type=Path, default=Path("benchmarks/public/file_localization_tasks.jsonl"))
    parser.add_argument("--binary", type=Path, default=Path("target/release/ig"))
    parser.add_argument("--cache-dir", type=Path, default=Path(tempfile.gettempdir()) / "ivygrep-file-localization-cache")
    parser.add_argument("--mode", choices=sorted(MODE_FLAGS), default="blended")
    parser.add_argument("--force-neural", action="store_true", help="Force the neural route for blended search queries.")
    parser.add_argument("--enhance-neural", action="store_true", help="Build neural vectors after indexing (requires a neural-capable binary).")
    parser.add_argument("--baseline", choices=["none", "lexical", "hash"], default="none")
    parser.add_argument("--limit", type=int, help="Evaluate only the first N tasks.")
    parser.add_argument("--timeout-secs", type=float, default=120.0, help="Per ig query timeout; indexing gets 4x.")
    parser.add_argument("--max-query-chars", type=int, default=2000)
    parser.add_argument("--search-limit", type=int, default=50)
    parser.add_argument("--budget", type=int, default=8000)
    parser.add_argument("--output", type=Path, help="Write the full JSON report here.")
    parser.add_argument("--min-acc-at-5", type=float, help="Fail when the selected mode's File Acc@5 is below this floor.")
    parser.add_argument("--min-margin-over-baseline", type=float, help="Fail when selected-mode Acc@5 does not beat the baseline by this margin.")
    return parser.parse_args(argv)


def build_report(tasks: list[dict[str, Any]], modes: list[str], rows: dict[str, list[dict[str, Any]]], args: argparse.Namespace, identity: dict[str, Any]) -> dict[str, Any]:
    summaries = {mode: aggregate(rows[mode]) for mode in modes}
    report: dict[str, Any] = {
        "schema_version": SCHEMA_VERSION,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "benchmark": "file-localization",
        "tool": identity,
        "tasks_file": str(args.tasks),
        "tasks_sha256": sha256_file(args.tasks),
        "task_count": len(tasks),
        "mode": args.mode,
        "baseline": None if args.baseline == "none" else args.baseline,
        "force_neural": args.force_neural,
        "enhance_neural": args.enhance_neural,
        "search_limit": args.search_limit,
        "budget": args.budget,
        "max_query_chars": args.max_query_chars,
        "timeout_secs": args.timeout_secs,
        "materialization": "git-archive snapshot committed into a fresh repository",
        "retrieval_note": (
            "neural vectors built with --enhance-neural"
            if args.enhance_neural
            else "no neural vectors built: blended routes to hash vectors + lexical (hash-only build semantics)"
        ),
        "summary": summaries,
        "rows": rows,
    }
    if args.baseline != "none" and args.baseline != args.mode:
        report["delta_over_baseline"] = delta(summaries[args.mode], summaries[args.baseline])
    return report


def check_gates(report: dict[str, Any], args: argparse.Namespace) -> None:
    failures = []
    primary = report["summary"][args.mode]
    gated = args.min_acc_at_5 is not None or args.min_margin_over_baseline is not None
    if gated:
        # A gate over a partial run is meaningless: tasks that errored or timed
        # out drop out of the mean, so a run where only easy tasks completed
        # could pass. Every selected mode must have scored every task.
        for mode, summary in report["summary"].items():
            if summary["failed_tasks"]:
                failures.append(
                    f"{mode}: {summary['failed_tasks']} of {summary['tasks']} tasks failed or timed out; "
                    "gates require a fully scored run"
                )
    if args.min_acc_at_5 is not None and primary["acc_at_5"] < args.min_acc_at_5:
        failures.append(f"acc_at_5={primary['acc_at_5']:.4f} < {args.min_acc_at_5:.4f}")
    if args.min_margin_over_baseline is not None:
        if "delta_over_baseline" not in report:
            failures.append("--min-margin-over-baseline requires --baseline different from --mode")
        elif report["delta_over_baseline"]["acc_at_5"] < args.min_margin_over_baseline:
            failures.append(
                f"acc_at_5 margin over {report['baseline']}="
                f"{report['delta_over_baseline']['acc_at_5']:+.4f} < {args.min_margin_over_baseline:.4f}"
            )
    if failures:
        raise SystemExit("file localization gate failed: " + "; ".join(failures))


def main(argv: list[str] | None = None) -> None:
    args = parse_args(argv)
    tasks = load_tasks(args.tasks)
    if args.limit is not None:
        tasks = tasks[: args.limit]
    modes = [args.mode]
    if args.baseline != "none" and args.baseline != args.mode:
        modes.append(args.baseline)
    identity = tool_identity(args.binary.resolve())
    rows = evaluate_tasks(tasks, modes, args)
    report = build_report(tasks, modes, rows, args, identity)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(markdown_table(report["summary"], report.get("delta_over_baseline")))
    failed = [row["task_id"] for row in rows[args.mode] if row.get("status") != "ok"]
    if failed:
        print(f"\n{len(failed)} task(s) not scored: {', '.join(failed)}", file=sys.stderr)
    check_gates(report, args)


if __name__ == "__main__":
    main()
