import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "check_allocator.py"


@unittest.skipIf(os.name == "nt", "the stand-in binary is a shell script")
class CheckAllocatorTest(unittest.TestCase):
    def check(self, report: str, *expected: str) -> subprocess.CompletedProcess:
        with tempfile.TemporaryDirectory() as temp:
            binary = Path(temp) / "ig"
            binary.write_text(f"#!/bin/sh\ncat <<'JSON'\n{report}\nJSON\n")
            binary.chmod(0o755)
            return subprocess.run(
                [sys.executable, str(SCRIPT), "--binary", str(binary), *expected],
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
            )

    def test_a_jemalloc_build_for_small_pages_fails_the_aarch64_gate(self) -> None:
        built_for_64k = '{"allocator": {"name": "jemalloc", "page_size": 65536}}'
        built_for_4k = '{"allocator": {"name": "jemalloc", "page_size": 4096}}'
        aarch64_gate = ("--name", "jemalloc", "--page-size", "65536")
        self.assertEqual(self.check(built_for_64k, *aarch64_gate).returncode, 0)
        rejected = self.check(built_for_4k, *aarch64_gate)
        self.assertNotEqual(rejected.returncode, 0)
        self.assertIn("4096", rejected.stderr)

    def test_the_allocator_itself_is_checked_in_both_directions(self) -> None:
        system = '{"allocator": {"name": "system", "page_size": null}}'
        self.assertEqual(self.check(system, "--name", "system").returncode, 0)
        self.assertNotEqual(
            self.check(system, "--name", "jemalloc", "--page-size", "4096").returncode, 0
        )
        # A binary from before the field existed reports nothing.
        self.assertNotEqual(self.check("{}", "--name", "system").returncode, 0)


if __name__ == "__main__":
    unittest.main()
