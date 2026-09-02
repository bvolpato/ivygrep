import importlib.util
import io
import json
import os
from types import SimpleNamespace
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
    def test_windows_rpc_authenticates_before_sending_query(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp)
            token = bytes(range(16)).hex()
            endpoint = home / "daemon.port"
            endpoint.write_bytes(f"43123\r\n{token}\r\n".encode())
            response = {"type": "search_results", "hits": []}
            connection = mock.MagicMock()
            connection.makefile.return_value = io.BytesIO(json.dumps(response).encode() + b"\n")
            with (
                mock.patch.object(check_daemon_equivalence, "os", SimpleNamespace(name="nt")),
                mock.patch.object(check_daemon_equivalence.socket, "create_connection", return_value=connection) as connect,
            ):
                self.assertEqual(check_daemon_equivalence.daemon_request(home, {"type": "status"}), response)
                connect.assert_called_once_with(("127.0.0.1", 43123), timeout=30)
                handshake, request = [call.args[0] for call in connection.sendall.call_args_list]
                self.assertEqual(handshake, token.encode() + b"\n")
                self.assertEqual(json.loads(request)["type"], "status")
                for invalid in ("", "z" * 32, "a" * 31):
                    endpoint.write_text(f"43123\n{invalid}\n")
                    connect.reset_mock()
                    with self.assertRaisesRegex(ValueError, "authentication token"):
                        check_daemon_equivalence.daemon_request(home, {"type": "status"})
                    connect.assert_not_called()

    def test_worktree_comparison_preserves_content_spans_and_duplicates(self) -> None:
        groups = [{"file_path": "src/shared.rs", "hits": [
            {"start_line": 2, "end_line": 4, "preview": "branch content"}
        ]}]
        expected = check_daemon_equivalence.canonical_content(groups)
        groups[0]["hits"][0]["preview"] = "stale base content"
        self.assertNotEqual(check_daemon_equivalence.canonical_content(groups), expected)
        groups[0]["hits"][0]["preview"] = "branch content"
        groups[0]["hits"][0]["start_line"] = 1
        self.assertNotEqual(check_daemon_equivalence.canonical_content(groups), expected)
        groups[0]["hits"][0]["start_line"] = 2
        groups[0]["hits"].append(dict(groups[0]["hits"][0]))
        self.assertEqual(check_daemon_equivalence.canonical_content(groups), expected * 2)

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
