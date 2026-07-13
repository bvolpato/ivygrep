import importlib.util
import os
from unittest import mock
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "check_daemon_equivalence.py"
SPEC = importlib.util.spec_from_file_location("check_daemon_equivalence", SCRIPT)
assert SPEC and SPEC.loader
check_daemon_equivalence = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = check_daemon_equivalence
SPEC.loader.exec_module(check_daemon_equivalence)


class DaemonEquivalenceFixtureTest(unittest.TestCase):
    def test_subprocess_output_is_decoded_as_utf8(self) -> None:
        result = check_daemon_equivalence.run(
            [
                sys.executable,
                "-c",
                "import sys; sys.stdout.buffer.write('✓'.encode('utf-8'))",
            ],
            cwd=SCRIPT.parent,
            env=os.environ.copy(),
        )

        self.assertEqual(result.stdout, "✓")

    def test_fixture_marks_its_workspace_boundary(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp) / "fixture"
            repo.mkdir()
            check_daemon_equivalence.write_fixture(repo)

            self.assertTrue((repo / ".git").is_dir())
            self.assertTrue((repo / "src" / "auth.rs").is_file())

    def test_daemon_endpoint_matches_platform_transport(self) -> None:
        home = Path("bench-home")
        with mock.patch.object(check_daemon_equivalence.os, "name", "nt"):
            self.assertEqual(
                check_daemon_equivalence.daemon_endpoint_path(home),
                home / "daemon.port",
            )
        with mock.patch.object(check_daemon_equivalence.os, "name", "posix"):
            self.assertEqual(
                check_daemon_equivalence.daemon_endpoint_path(home),
                home / "daemon.sock",
            )


if __name__ == "__main__":
    unittest.main()
