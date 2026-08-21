#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Tests for clean, frozen context-retrieval evaluation."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "bench_context_retrieval.py"
SPEC = importlib.util.spec_from_file_location("bench_context_retrieval", SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError(f"could not load {SCRIPT}")
benchmark = importlib.util.module_from_spec(SPEC)
sys.modules["bench_context_retrieval"] = benchmark
SPEC.loader.exec_module(benchmark)


def git(repo: Path, *args: str) -> str:
    return subprocess.run(
        ["git", "-c", "commit.gpgSign=false", *args],
        cwd=repo,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()


class ContextRetrievalBenchmarkTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.repo = Path(self.temporary.name) / "repo"
        self.repo.mkdir()
        git(self.repo, "init", "-b", "main")
        git(self.repo, "config", "user.name", "Context Benchmark Test")
        git(self.repo, "config", "user.email", "context-benchmark@example.com")
        (self.repo / "src").mkdir()
        (self.repo / "src" / "old.rs").write_text("fn old() {}\n")
        git(self.repo, "add", "src/old.rs")
        git(self.repo, "commit", "-m", "create initial source")
        self.base_commit = git(self.repo, "rev-parse", "HEAD")

        (self.repo / "src" / "old.rs").write_text("fn improved() {}\n")
        (self.repo / "src" / "new.rs").write_text("fn added() {}\n")
        git(self.repo, "add", "src")
        git(self.repo, "commit", "-m", "improve old source handling")
        (self.repo / "retrieval-results.json").write_text("{}\n")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_historical_tasks_use_parent_tree_and_retrievable_paths(self) -> None:
        tasks = benchmark.historical_tasks(self.repo, 1)

        self.assertEqual(tasks[0]["base_commit"], self.base_commit)
        self.assertEqual(tasks[0]["expected_paths"], ["src/old.rs"])

    def test_clean_task_worktree_excludes_current_dirty_files(self) -> None:
        destination = Path(self.temporary.name) / "task-worktree"

        with benchmark.clean_task_worktree(
            self.repo, self.base_commit, destination
        ) as worktree:
            self.assertEqual((worktree / "src" / "old.rs").read_text(), "fn old() {}\n")
            self.assertFalse((worktree / "src" / "new.rs").exists())
            self.assertFalse((worktree / "retrieval-results.json").exists())
            self.assertEqual(git(worktree, "status", "--porcelain"), "")

        self.assertFalse(destination.exists())

    def test_curated_tasks_allow_relevant_unchanged_paths(self) -> None:
        fixture = Path(self.temporary.name) / "curated.json"
        fixture.write_text(
            json.dumps(
                {
                    "tasks": [
                        {
                            "id": "curated-task",
                            "task": "find old source behavior",
                            "base_commit": self.base_commit,
                            "expected_paths": ["src/old.rs"],
                            "label_source": "curated",
                        }
                    ]
                }
            )
        )

        tasks = benchmark.load_tasks(self.repo, fixture)

        self.assertEqual(tasks[0]["label_source"], "curated")
        self.assertNotIn("source_commit", tasks[0])

    def test_metrics_report_role_recall_without_duplicate_file_credit(self) -> None:
        metrics = benchmark.retrieval_metrics(
            ["src/search.rs", "src/search.rs", "tests/search.rs"],
            ["src/search.rs", "src/indexer.rs", "tests/search.rs"],
        )

        self.assertEqual(metrics["matched_files"], 2)
        self.assertAlmostEqual(metrics["recall"], 2 / 3)
        self.assertEqual(metrics["role_recall"], {"primary": 0.5, "test": 1.0})

    def test_context_gates_reject_quality_below_floor(self) -> None:
        result = {
            "mode": "context",
            "mean_recall": 0.75,
            "zero_recall_rate": 0.08,
            "mean_recall_per_1k_tokens": 0.12,
            "mean_covered_roles": 7.0,
            "mean_role_recall": {"primary": 0.82},
        }
        arguments = SimpleNamespace(
            min_context_recall=0.76,
            min_context_primary_recall=0.75,
            min_context_test_recall=None,
            max_context_zero_recall_rate=0.17,
            min_context_covered_roles=7.0,
            min_context_recall_per_1k_tokens=0.11,
        )

        with self.assertRaisesRegex(SystemExit, "mean_recall=0.750000"):
            benchmark.check_context_gates([result], arguments)

    def test_constant_topk_baseline_scores_most_frequent_gold_paths(self) -> None:
        tasks = [
            {"id": "a", "expected_paths": ["src/search.rs", "src/indexer.rs"]},
            {"id": "b", "expected_paths": ["src/search.rs", "src/daemon.rs"]},
            {"id": "c", "expected_paths": ["src/search.rs", "tests/web.rs"]},
            {"id": "d", "expected_paths": ["src/web.rs"]},
        ]

        baseline = benchmark.constant_topk_baseline(tasks, 2)

        self.assertEqual(baseline["mode"], "constant-topk")
        self.assertEqual(baseline["k"], 2)
        # Ties break on path so the baseline is deterministic.
        self.assertEqual(baseline["paths"], ["src/search.rs", "src/daemon.rs"])
        self.assertAlmostEqual(baseline["mean_recall"], (0.5 + 1.0 + 0.5 + 0.0) / 4)
        self.assertAlmostEqual(baseline["zero_recall_rate"], 0.25)
        self.assertEqual(baseline["mean_role_recall"]["test"], 0.0)

    def test_context_gates_require_margin_over_constant_baseline(self) -> None:
        result = {
            "mode": "context",
            "mean_recall": 0.80,
            "zero_recall_rate": 0.08,
            "mean_recall_per_1k_tokens": 0.12,
            "mean_covered_roles": 7.0,
            "mean_role_recall": {"primary": 0.82},
        }
        baseline = {"mode": "constant-topk", "k": 5, "mean_recall": 0.78}
        arguments = SimpleNamespace(
            min_context_recall=0.70,
            min_context_primary_recall=None,
            min_context_test_recall=None,
            max_context_zero_recall_rate=None,
            min_context_covered_roles=None,
            min_context_recall_per_1k_tokens=None,
            min_margin_over_baseline=0.10,
        )

        with self.assertRaisesRegex(SystemExit, r"margin over constant-topk\(k=5\)=0\.020000"):
            benchmark.check_context_gates([result], arguments, baseline)

        result["mean_recall"] = 0.89
        benchmark.check_context_gates([result], arguments, baseline)
        benchmark.check_context_gates([result], arguments, None)

    def test_context_gates_reject_role_coverage_loss(self) -> None:
        result = {
            "mode": "context",
            "mean_recall": 0.75,
            "zero_recall_rate": 0.08,
            "mean_recall_per_1k_tokens": 0.12,
            "mean_covered_roles": 6.9,
            "mean_role_recall": {"primary": 0.82},
        }
        arguments = SimpleNamespace(
            min_context_recall=0.70,
            min_context_primary_recall=0.75,
            min_context_test_recall=None,
            max_context_zero_recall_rate=0.17,
            min_context_covered_roles=7.0,
            min_context_recall_per_1k_tokens=0.11,
        )

        with self.assertRaisesRegex(SystemExit, "mean_covered_roles=6.900000"):
            benchmark.check_context_gates([result], arguments)

    def test_context_gates_reject_test_recall_loss(self) -> None:
        result = {
            "mode": "context",
            "mean_recall": 0.82,
            "zero_recall_rate": 0.08,
            "mean_recall_per_1k_tokens": 0.17,
            "mean_covered_roles": 7.2,
            "mean_role_recall": {"primary": 0.82, "test": 0.5},
        }
        arguments = SimpleNamespace(
            min_context_recall=0.80,
            min_context_primary_recall=0.80,
            min_context_test_recall=1.0,
            max_context_zero_recall_rate=0.17,
            min_context_covered_roles=7.0,
            min_context_recall_per_1k_tokens=0.16,
        )

        with self.assertRaisesRegex(SystemExit, "mean_role_recall.test=0.500000"):
            benchmark.check_context_gates([result], arguments)


if __name__ == "__main__":
    unittest.main()
