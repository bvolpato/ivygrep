#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Tests for the git-preserving Criterion benchmark guard."""

from __future__ import annotations

import importlib.util
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "benchmark_guard.py"
SPEC = importlib.util.spec_from_file_location("benchmark_guard", SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError(f"could not load {SCRIPT}")
benchmark_guard = importlib.util.module_from_spec(SPEC)
sys.modules["benchmark_guard"] = benchmark_guard
SPEC.loader.exec_module(benchmark_guard)


def git(repo: Path, *args: str) -> str:
    return subprocess.run(
        ["git", *args],
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


if __name__ == "__main__":
    unittest.main()
