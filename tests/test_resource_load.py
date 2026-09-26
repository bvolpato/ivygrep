#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Check resource attribution and failure handling with real child processes."""

import os
from pathlib import Path
import sys
import tempfile
import unittest

SCRIPTS = Path(__file__).resolve().parents[1] / "scripts"
sys.path.insert(0, str(SCRIPTS))
import bench_resource_load as benchmark


@unittest.skipUnless(sys.platform == "linux", "wait4 resource units require Linux")
class ChildResourceTests(unittest.TestCase):
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
