#!/usr/bin/env python3
"""Tests for the Linux kernel benchmark harness."""

from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "bench_linux_kernel.py"
SPEC = importlib.util.spec_from_file_location("bench_linux_kernel", SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError(f"could not load {SCRIPT}")
bench_linux_kernel = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(bench_linux_kernel)


class BenchHomeGuardTests(unittest.TestCase):
    def test_accepts_tmp_child(self) -> None:
        bench_home = Path("/tmp/ivygrep-bench-test")
        self.assertEqual(
            bench_linux_kernel.ensure_bench_home_under_tmp(bench_home),
            bench_home.resolve(),
        )

    def test_rejects_tmp_root(self) -> None:
        with self.assertRaises(SystemExit):
            bench_linux_kernel.ensure_bench_home_under_tmp(Path("/tmp"))

    def test_rejects_path_outside_tmp(self) -> None:
        with self.assertRaises(SystemExit):
            bench_linux_kernel.ensure_bench_home_under_tmp(Path.home())

    def test_rejects_tmp_symlink_to_outside_tmp(self) -> None:
        with tempfile.TemporaryDirectory(dir="/tmp") as tmp_dir:
            link = Path(tmp_dir) / "outside"
            link.symlink_to(Path.home(), target_is_directory=True)
            with self.assertRaises(SystemExit):
                bench_linux_kernel.ensure_bench_home_under_tmp(link)


if __name__ == "__main__":
    unittest.main()
