#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Check resource attribution and failure handling with real child processes."""

import os
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPTS = Path(__file__).resolve().parents[1] / "scripts"
sys.path.insert(0, str(SCRIPTS))
import bench_resource_load as benchmark
import render_resource_load as renderer


@unittest.skipUnless(sys.platform == "linux", "wait4 resource units require Linux")
class ChildResourceTests(unittest.TestCase):
    def test_empty_background_series_reports_zero_completions(self):
        summary = benchmark.latency_summary([])
        self.assertEqual(summary["samples"], 0)
        self.assertEqual(summary["raw_ms"], [])
        for metric in ("p50_ms", "p95_ms", "p99_ms", "maximum_ms"):
            self.assertIsNone(summary[metric])

    def test_uneven_sample_count_fails_before_creating_artifacts(self):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory) / "work"
            result = subprocess.run(
                [sys.executable, str(SCRIPTS / "bench_resource_load.py"),
                 "--binary", "unused", "--samples", "129", "--clients", "8",
                 "--work-dir", str(work), "--output", str(work / "report.json")],
                capture_output=True, text=True)
            self.assertEqual(result.returncode, 2)
            self.assertIn("samples must be divisible by clients", result.stderr)
            self.assertFalse(work.exists())

    def test_comparison_keeps_zero_background_count_without_a_latency(self):
        report = json.loads((SCRIPTS.parent / "docs/benchmarks/resource-load-candidate.json").read_text())
        for run in report["runs"]:
            run["background_indexes"] = benchmark.latency_summary([])
        rows = renderer.comparison_rows(report, report)
        latency = next(row for row in rows if row[1] == "Background index p95")
        count = next(row for row in rows if row[1] == "Background indexes completed")
        self.assertEqual(latency[2:], ["unavailable"] * 3)
        self.assertEqual(count[2:4], ["0.00 runs (0.00 to 0.00)"] * 2)

    def test_peak_rss_is_per_child_and_disk_writes_are_measured(self):
        with tempfile.TemporaryDirectory(dir="/instance_storage" if Path("/instance_storage").is_dir() else None) as directory:
            root = Path(directory)
            large = benchmark.measured_command(
                [sys.executable, "-c", "data = bytearray(64 * 1024 * 1024)"],
                root, os.environ.copy(), root / "large.log")
            small = benchmark.measured_command(
                [sys.executable, "-c", "import os; f = open('data', 'wb'); f.write(b'x' * 1048576); f.flush(); os.fsync(f.fileno())"],
                root, os.environ.copy(), root / "small.log")
            self.assertLess(small["peak_rss_bytes"], large["peak_rss_bytes"])
            self.assertGreater(small["filesystem_write_bytes"], 0)
            self.assertGreater(large["cpu_ms"], 0)

    def test_failed_child_does_not_produce_success_metrics(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with self.assertRaisesRegex(RuntimeError, "exited with 7"):
                benchmark.measured_command([sys.executable, "-c", "raise SystemExit(7)"],
                                           root, os.environ.copy(), root / "failed.log")


if __name__ == "__main__":
    unittest.main()
