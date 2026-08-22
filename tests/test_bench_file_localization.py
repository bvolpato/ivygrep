#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Tests for the repository file-localization benchmark harness (no network)."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "bench_file_localization.py"
SPEC = importlib.util.spec_from_file_location("bench_file_localization", SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError(f"could not load {SCRIPT}")
benchmark = importlib.util.module_from_spec(SPEC)
sys.modules["bench_file_localization"] = benchmark
SPEC.loader.exec_module(benchmark)

PUBLIC_TASKS = ROOT / "benchmarks" / "public" / "file_localization_tasks.jsonl"


def task_line(**overrides: object) -> str:
    row = {
        "task_id": "demo-1",
        "repo": "https://example.invalid/demo.git",
        "base_commit": "0123456789abcdef0123456789abcdef01234567",
        "query": "Crash when parsing empty input",
        "gold_files": ["src/parse.rs"],
    }
    row.update(overrides)
    return json.dumps(row)


class FileLocalizationMetricsTests(unittest.TestCase):
    def test_rank_metrics_acc_recall_and_mrr(self) -> None:
        ranked = [
            "src/other.rs",
            "src/parse.rs",
            "src/parse.rs",  # duplicate file must not earn extra credit
            "src/lexer.rs",
        ] + [f"src/pad{index}.rs" for index in range(20)]

        metrics = benchmark.rank_metrics(ranked, ["src/parse.rs", "src/lexer.rs", "src/ast.rs"])

        self.assertEqual(metrics["acc_at_1"], 0.0)
        self.assertEqual(metrics["acc_at_5"], 1.0)
        self.assertEqual(metrics["acc_at_10"], 1.0)
        self.assertAlmostEqual(metrics["recall_at_10"], 2 / 3)
        self.assertAlmostEqual(metrics["recall_at_20"], 2 / 3)
        self.assertAlmostEqual(metrics["mrr"], 0.5)
        self.assertEqual(metrics["first_gold_rank"], 2.0)
        self.assertEqual(metrics["returned_files"], 23.0)

    def test_rank_metrics_recall_cutoffs_respect_rank(self) -> None:
        ranked = [f"src/pad{index}.rs" for index in range(12)] + ["src/parse.rs"]

        metrics = benchmark.rank_metrics(ranked, ["src/parse.rs"])

        self.assertEqual(metrics["acc_at_10"], 0.0)
        self.assertEqual(metrics["recall_at_10"], 0.0)
        self.assertEqual(metrics["recall_at_20"], 1.0)
        self.assertAlmostEqual(metrics["mrr"], 1 / 13)

    def test_rank_metrics_miss_scores_zero(self) -> None:
        metrics = benchmark.rank_metrics(["src/a.rs"], ["src/b.rs"])

        self.assertEqual(metrics["acc_at_1"], 0.0)
        self.assertEqual(metrics["mrr"], 0.0)
        self.assertEqual(metrics["first_gold_rank"], 0.0)
        with self.assertRaises(ValueError):
            benchmark.rank_metrics(["src/a.rs"], [])

    def test_pack_metrics_precision_recall(self) -> None:
        metrics = benchmark.pack_metrics(
            ["src/parse.rs", "src/parse.rs", "README.md", "src/cli.rs"],
            ["src/parse.rs", "src/ast.rs"],
        )

        self.assertEqual(metrics["pack_files"], 3.0)
        self.assertEqual(metrics["pack_matched"], 1.0)
        self.assertAlmostEqual(metrics["pack_recall"], 0.5)
        self.assertAlmostEqual(metrics["pack_precision"], 1 / 3)
        self.assertEqual(metrics["pack_hit"], 1.0)
        self.assertEqual(benchmark.pack_metrics([], ["src/parse.rs"])["pack_precision"], 0.0)

    def test_aggregate_skips_failed_rows_and_reports_delta(self) -> None:
        def row(task_id: str, language: str, acc1: float, status: str = "ok") -> dict:
            return {
                "task_id": task_id,
                "language": language,
                "status": status,
                "metrics": {
                    "acc_at_1": acc1,
                    "acc_at_5": 1.0,
                    "acc_at_10": 1.0,
                    "recall_at_10": 1.0,
                    "recall_at_20": 1.0,
                    "mrr": acc1 or 0.5,
                    "pack_recall": 1.0,
                    "pack_precision": 0.25,
                    "pack_hit": 1.0,
                },
                "search_latency_ms": 10.0,
                "context_latency_ms": 20.0,
                "pack_used_tokens": 4000,
            }

        primary = benchmark.aggregate(
            [row("a", "rust", 1.0), row("b", "python", 0.0), row("c", "python", 0.0, "error")]
        )
        baseline = benchmark.aggregate([row("a", "rust", 0.0), row("b", "python", 0.0)])

        self.assertEqual(primary["tasks"], 3)
        self.assertEqual(primary["scored_tasks"], 2)
        self.assertEqual(primary["failed_tasks"], 1)
        self.assertAlmostEqual(primary["acc_at_1"], 0.5)
        self.assertAlmostEqual(primary["by_language"]["rust"]["acc_at_1"], 1.0)
        self.assertEqual(primary["by_language"]["python"]["tasks"], 1)
        delta = benchmark.delta(primary, baseline)
        self.assertAlmostEqual(delta["acc_at_1"], 0.5)
        table = benchmark.markdown_table({"blended": primary, "lexical": baseline}, delta)
        self.assertIn("| blended | 2/3 |", table)
        self.assertIn("| delta | | +0.500 |", table)

    def test_gate_rejects_partially_scored_runs(self) -> None:
        report = {
            "baseline": "lexical",
            "summary": {
                "blended": {"acc_at_5": 0.90, "tasks": 30, "failed_tasks": 0},
                "lexical": {"acc_at_5": 0.50, "tasks": 30, "failed_tasks": 4},
            },
            "delta_over_baseline": {"acc_at_5": 0.40},
        }
        arguments = SimpleNamespace(mode="blended", min_acc_at_5=0.60, min_margin_over_baseline=None)
        with self.assertRaisesRegex(SystemExit, r"lexical: 4 of 30 tasks failed or timed out"):
            benchmark.check_gates(report, arguments)
        # Without gates a partial run is merely reported, not rejected.
        benchmark.check_gates(report, SimpleNamespace(mode="blended", min_acc_at_5=None, min_margin_over_baseline=None))

    def test_gate_requires_margin_over_baseline(self) -> None:
        report = {
            "baseline": "lexical",
            "summary": {
                "blended": {"acc_at_5": 0.70, "tasks": 30, "failed_tasks": 0},
                "lexical": {"acc_at_5": 0.65, "tasks": 30, "failed_tasks": 0},
            },
            "delta_over_baseline": {"acc_at_5": 0.05},
        }
        arguments = SimpleNamespace(mode="blended", min_acc_at_5=0.60, min_margin_over_baseline=0.10)

        with self.assertRaisesRegex(SystemExit, r"margin over lexical=\+0\.0500"):
            benchmark.check_gates(report, arguments)

        arguments.min_margin_over_baseline = 0.05
        benchmark.check_gates(report, arguments)
        arguments.min_acc_at_5 = 0.75
        with self.assertRaisesRegex(SystemExit, "acc_at_5=0.7000"):
            benchmark.check_gates(report, arguments)


class FileLocalizationTaskParsingTests(unittest.TestCase):
    def test_parse_tasks_normalizes_rows_and_skips_blank_lines(self) -> None:
        tasks = benchmark.parse_tasks(
            [
                "",
                "# comment",
                task_line(gold_files=["./src/parse.rs", "src/lexer.rs"]),
                task_line(task_id="demo-2", also_changed=[{"path": "tests/t.rs", "reason": "test"}]),
            ]
        )

        self.assertEqual([task["task_id"] for task in tasks], ["demo-1", "demo-2"])
        self.assertEqual(tasks[0]["gold_files"], ["src/parse.rs", "src/lexer.rs"])
        self.assertEqual(tasks[0]["also_changed"], [])
        self.assertEqual(tasks[1]["also_changed"][0]["reason"], "test")

    def test_parse_tasks_rejects_malformed_rows(self) -> None:
        cases = {
            "missing fields": task_line().replace('"gold_files"', '"gold"'),
            "non-empty list": task_line(gold_files=[]),
            "commit hash": task_line(base_commit="main"),
            "invalid JSON": "{not json",
            "non-empty string": task_line(query="   "),
        }
        for expected, line in cases.items():
            with self.subTest(expected=expected):
                with self.assertRaisesRegex(benchmark.TaskError, expected):
                    benchmark.parse_tasks([line])
        with self.assertRaisesRegex(benchmark.TaskError, "duplicate task_id"):
            benchmark.parse_tasks([task_line(), task_line()])
        with self.assertRaisesRegex(benchmark.TaskError, "no tasks"):
            benchmark.parse_tasks(["", "# only comments"])

    def test_load_tasks_reads_jsonl_file(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "tasks.jsonl"
            path.write_text(task_line() + "\n" + task_line(task_id="demo-2") + "\n")

            tasks = benchmark.load_tasks(path)

        self.assertEqual(len(tasks), 2)

    def test_truncate_query_records_truncation(self) -> None:
        text, truncated = benchmark.truncate_query("  short  ", 2000)
        self.assertEqual((text, truncated), ("short", False))
        text, truncated = benchmark.truncate_query("a" * 10 + " tail", 10)
        self.assertEqual((text, truncated), ("a" * 10, True))
        text, truncated = benchmark.truncate_query("keep all", None)
        self.assertEqual((text, truncated), ("keep all", False))

    def test_normalize_path_makes_repo_relative(self) -> None:
        repo = Path("/tmp/ivygrep-bench/repo")
        self.assertEqual(benchmark.normalize_path("/tmp/ivygrep-bench/repo/src/a.rs", repo), "src/a.rs")
        self.assertEqual(benchmark.normalize_path("./src/a.rs", repo), "src/a.rs")
        self.assertEqual(
            benchmark.search_paths(["src/a.rs", {"file_path": "src/b.rs"}, "src/a.rs"], repo),
            ["src/a.rs", "src/b.rs"],
        )
        self.assertEqual(
            benchmark.context_paths({"items": [{"file_path": "src/c.rs"}, {"file_path": "src/c.rs"}]}, repo),
            ["src/c.rs"],
        )

    def test_mode_flags_select_retrieval_route(self) -> None:
        binary = Path("/opt/ig")
        repo = Path("/tmp/repo")
        lexical = benchmark.search_command(binary, "lexical", False, 50, "q", repo)
        hashed = benchmark.search_command(binary, "hash", False, 50, "q", repo)
        blended = benchmark.search_command(binary, "blended", False, 50, "q", repo)
        neural = benchmark.search_command(binary, "blended", True, 50, "q", repo)

        self.assertIn("--lexical-only", lexical)
        self.assertIn("--hash", hashed)
        self.assertNotIn("--hash", blended)
        self.assertNotIn("--lexical-only", blended)
        self.assertIn("--force-neural", neural)
        self.assertEqual(lexical[-2:], ["q", str(repo)])
        self.assertIn("--file-name-only", blended)
        context = benchmark.context_command(binary, "lexical", 8000, "q", repo)
        self.assertEqual(context[1], "context")
        self.assertIn("--lexical-only", context)
        self.assertIn("8000", context)


class PublicTaskFileTests(unittest.TestCase):
    def test_public_task_file_is_well_formed_and_sourced(self) -> None:
        tasks = benchmark.load_tasks(PUBLIC_TASKS)

        self.assertGreaterEqual(len(tasks), 20)
        allowed_licenses = {"MIT", "BSD-3-Clause", "Apache-2.0", "Unlicense", "ISC", "BSD-2-Clause"}
        for task in tasks:
            with self.subTest(task=task["task_id"]):
                self.assertTrue(task["repo"].startswith("https://github.com/"))
                self.assertIn(task["repo_license"], allowed_licenses)
                self.assertTrue(task["source_url"].startswith("https://github.com/"))
                self.assertTrue(task["issue_url"].startswith("https://github.com/"))
                self.assertTrue(all("/" in path or "." in path for path in task["gold_files"]))
                self.assertLessEqual(len(task["query"]), 1500 + 400)
                self.assertNotRegex(task["query"], r"/(?:home|Users)/(?!user\b)[^/\s]+")
                self.assertFalse(
                    any(
                        benchmark.normalize_relative(change["path"]) in task["gold_files"]
                        for change in task["also_changed"]
                    )
                )
        languages = {task["language"] for task in tasks}
        self.assertTrue({"python", "rust", "typescript"} <= languages)


if __name__ == "__main__":
    unittest.main()
