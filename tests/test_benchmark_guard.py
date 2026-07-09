#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Tests for the git-preserving Criterion benchmark guard."""

from __future__ import annotations

import importlib.util
import io
import json
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest import mock


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "benchmark_guard.py"
SPEC = importlib.util.spec_from_file_location("benchmark_guard", SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError(f"could not load {SCRIPT}")
benchmark_guard = importlib.util.module_from_spec(SPEC)
sys.modules["benchmark_guard"] = benchmark_guard
SPEC.loader.exec_module(benchmark_guard)


def git(repo: Path, *args: str) -> str:
    return subprocess.run(
        ["git", "-c", "commit.gpgSign=false", *args],
        cwd=repo,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()


class BenchmarkGuardCheckoutTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.repo = Path(self.tmp.name)
        git(self.repo, "init", "-b", "main")
        git(self.repo, "config", "user.name", "Benchmark Guard Test")
        git(self.repo, "config", "user.email", "benchmark-guard@example.com")
        (self.repo / "fixture.txt").write_text("clean\n", encoding="utf-8")
        git(self.repo, "add", "fixture.txt")
        git(self.repo, "commit", "-m", "initial")

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def test_restore_checkout_returns_to_branch(self) -> None:
        original = benchmark_guard.current_checkout(self.repo)
        self.assertEqual(original, benchmark_guard.Checkout("main", detached=False))

        git(self.repo, "checkout", "--detach", "HEAD")
        self.assertTrue(benchmark_guard.current_checkout(self.repo).detached)

        benchmark_guard.restore_checkout(self.repo, original)
        self.assertEqual(git(self.repo, "branch", "--show-current"), "main")

    def test_restore_checkout_preserves_detached_head(self) -> None:
        git(self.repo, "checkout", "--detach", "HEAD")
        original = benchmark_guard.current_checkout(self.repo)
        git(self.repo, "checkout", "-b", "temporary")

        benchmark_guard.restore_checkout(self.repo, original)

        self.assertEqual(git(self.repo, "rev-parse", "HEAD"), original.ref)
        self.assertEqual(git(self.repo, "branch", "--show-current"), "")

    def test_clean_worktree_guard_rejects_uncommitted_changes(self) -> None:
        benchmark_guard.ensure_clean_worktree(self.repo)
        (self.repo / "fixture.txt").write_text("dirty\n", encoding="utf-8")

        with self.assertRaisesRegex(SystemExit, "requires a clean worktree"):
            benchmark_guard.ensure_clean_worktree(self.repo)

    def test_ratio_handles_zero_baseline(self) -> None:
        self.assertEqual(benchmark_guard.ratio(10.0, 5.0), 2.0)
        self.assertEqual(benchmark_guard.ratio(10.0, 0.0), float("inf"))

    def run_guard(
        self, measurements: list[float], output_path: Path | None = None
    ) -> tuple[int, mock.Mock]:
        measure = mock.Mock(side_effect=measurements)
        argv = [
            "benchmark_guard.py",
            "--baseline-ref",
            "baseline",
            "--threshold",
            "1.15",
        ]
        if output_path is not None:
            argv.extend(["--output", str(output_path)])
        with (
            mock.patch.object(sys, "argv", argv),
            mock.patch.object(benchmark_guard, "ensure_clean_worktree"),
            mock.patch.object(
                benchmark_guard,
                "current_checkout",
                return_value=benchmark_guard.Checkout("main", detached=False),
            ),
            mock.patch.object(benchmark_guard, "output", return_value="current"),
            mock.patch.object(benchmark_guard, "measure", measure),
            mock.patch.object(benchmark_guard, "restore_checkout"),
            redirect_stdout(io.StringIO()),
            redirect_stderr(io.StringIO()),
        ):
            return benchmark_guard.main(), measure

    def test_unconfirmed_regression_passes_after_reverse_order_retry(self) -> None:
        result, measure = self.run_guard([20.0, 10.0, 10.0, 11.0])

        self.assertEqual(result, 0)
        self.assertEqual(
            [call.args[1] for call in measure.call_args_list],
            ["current", "baseline", "baseline", "current"],
        )

    def test_confirmed_regression_fails(self) -> None:
        result, _ = self.run_guard([20.0, 10.0, 10.0, 20.0])

        self.assertEqual(result, 1)

    def test_writes_machine_readable_result(self) -> None:
        output_path = self.repo / "artifacts" / "guard.json"

        result, _ = self.run_guard([11.0, 10.0], output_path)

        self.assertEqual(result, 0)
        payload = json.loads(output_path.read_text(encoding="utf-8"))
        self.assertEqual(payload["bench"], "indexer/incremental_reindex_no_change")
        self.assertEqual(payload["ratio"], 1.1)
        self.assertEqual(payload["threshold"], 1.15)


if __name__ == "__main__":
    unittest.main()
