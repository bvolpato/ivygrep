import os
import shlex
import stat
import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
HELPER = ROOT / "scripts" / "e2e_neural_backend.sh"


class NeuralBackendE2ETest(unittest.TestCase):
    def run_helper(
        self, fake_body: str, attempts: int, extra_args: list[str] | None = None
    ) -> tuple[subprocess.CompletedProcess[str], int]:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            fake = tmp_path / "ig"
            state = tmp_path / "attempts"
            fake.write_text(
                "#!/bin/sh\n"
                "set -eu\n"
                f"state={shlex.quote(str(state))}\n"
                + textwrap.dedent(fake_body),
                encoding="utf-8",
            )
            fake.chmod(fake.stat().st_mode | stat.S_IXUSR)

            env = os.environ.copy()
            env["IVYGREP_E2E_DOWNLOAD_ATTEMPTS"] = str(attempts)
            env["IVYGREP_E2E_RETRY_DELAY_SECONDS"] = "0"
            command = [
                "sh",
                str(HELPER),
                "--binary",
                str(fake),
                "--expect-backend",
                "StaticEmbedding token mean via Rust",
            ]
            if extra_args:
                command.extend(extra_args)

            result = subprocess.run(
                command,
                cwd=ROOT,
                env=env,
                text=True,
                capture_output=True,
                check=False,
            )
            count = int(state.read_text(encoding="utf-8")) if state.exists() else 0
            return result, count

    def test_retries_transient_model_host_failures(self) -> None:
        result, attempts = self.run_helper(
            """
            case "$1" in
              --add)
                exit 0
                ;;
              --enhance-internal)
                count=0
                [ ! -f "$state" ] || count=$(cat "$state")
                count=$((count + 1))
                echo "$count" > "$state"
                if [ "$count" -lt 3 ]; then
                  echo "request error: status code 429" >&2
                  exit 1
                fi
                exit 0
                ;;
              --status)
                echo '{"has_neural_vectors":true,"neural_backend":"StaticEmbedding token mean via Rust"}'
                exit 0
                ;;
            esac
            exit 2
            """,
            attempts=3,
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(attempts, 3)
        self.assertIn("Transient model download failure", result.stderr)

    def test_model_profile_option_sets_environment(self) -> None:
        result, attempts = self.run_helper(
            """
            case "$1" in
              --add)
                [ "${IVYGREP_MODEL_PROFILE:-}" = "general" ] || exit 7
                exit 0
                ;;
              --enhance-internal)
                echo 1 > "$state"
                [ "${IVYGREP_MODEL_PROFILE:-}" = "general" ] || exit 7
                exit 0
                ;;
              --status)
                echo '{"has_neural_vectors":true,"neural_backend":"StaticEmbedding token mean via Rust"}'
                exit 0
                ;;
            esac
            exit 2
            """,
            attempts=1,
            extra_args=["--model-profile", "general"],
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(attempts, 1)

    def test_retries_xet_cas_signed_url_forbidden(self) -> None:
        result, attempts = self.run_helper(
            """
            case "$1" in
              --add)
                exit 0
                ;;
              --enhance-internal)
                count=0
                [ ! -f "$state" ] || count=$(cat "$state")
                count=$((count + 1))
                echo "$count" > "$state"
                if [ "$count" -lt 2 ]; then
                  echo "request error: https://cas-bridge.xethub.hf.co/xet/model: status code 403" >&2
                  exit 1
                fi
                exit 0
                ;;
              --status)
                echo '{"has_neural_vectors":true,"neural_backend":"StaticEmbedding token mean via Rust"}'
                exit 0
                ;;
            esac
            exit 2
            """,
            attempts=2,
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(attempts, 2)

    def test_does_not_retry_generic_forbidden(self) -> None:
        result, attempts = self.run_helper(
            """
            case "$1" in
              --add)
                exit 0
                ;;
              --enhance-internal)
                count=0
                [ ! -f "$state" ] || count=$(cat "$state")
                echo $((count + 1)) > "$state"
                echo "request error: https://example.com/private: status code 403" >&2
                exit 1
                ;;
            esac
            exit 2
            """,
            attempts=5,
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(attempts, 1)

    def test_does_not_retry_permanent_model_failures(self) -> None:
        result, attempts = self.run_helper(
            """
            case "$1" in
              --add)
                exit 0
                ;;
              --enhance-internal)
                count=0
                [ ! -f "$state" ] || count=$(cat "$state")
                echo $((count + 1)) > "$state"
                echo "model schema mismatch" >&2
                exit 1
                ;;
            esac
            exit 2
            """,
            attempts=5,
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(attempts, 1)
        self.assertIn("neural enhancement failed on attempt 1", result.stderr)

    def test_semantic_query_assertion_requires_expected_file(self) -> None:
        result, attempts = self.run_helper(
            """
            case "$1" in
              --add)
                exit 0
                ;;
              --enhance-internal)
                echo 1 > "$state"
                exit 0
                ;;
              --status)
                echo '{"has_neural_vectors":true,"neural_backend":"StaticEmbedding token mean via Rust"}'
                exit 0
                ;;
              --json)
                [ "$2" = "--force-neural" ] || exit 7
                echo '{"hits":[{"file_path":"src/lib.rs","neural_executed":true}]}'
                exit 0
                ;;
            esac
            exit 2
            """,
            attempts=1,
            extra_args=["--expect-file", "src/lib.rs"],
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(attempts, 1)
        self.assertIn("Semantic retrieval procedure passed", result.stdout)

    def test_semantic_query_requires_neural_execution(self) -> None:
        result, attempts = self.run_helper(
            """
            case "$1" in
              --add)
                exit 0
                ;;
              --enhance-internal)
                echo 1 > "$state"
                exit 0
                ;;
              --status)
                echo '{"has_neural_vectors":true,"neural_backend":"StaticEmbedding token mean via Rust"}'
                exit 0
                ;;
              --json)
                [ "$2" = "--force-neural" ] || exit 7
                echo '{"hits":[{"file_path":"src/lib.rs"}]}'
                exit 0
                ;;
            esac
            exit 2
            """,
            attempts=1,
            extra_args=["--expect-file", "src/lib.rs"],
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(attempts, 1)
        self.assertIn("did not execute neural retrieval", result.stderr)

    def test_worktree_option_checks_branch_local_neural_retrieval(self) -> None:
        result, attempts = self.run_helper(
            """
            case "$1" in
              --add)
                exit 0
                ;;
              --enhance-internal)
                echo 1 > "$state"
                exit 0
                ;;
              --status)
                echo '{"has_neural_vectors":true,"neural_backend":"StaticEmbedding token mean via Rust"}'
                exit 0
                ;;
              --json)
                [ "$2" = "--force-neural" ] || exit 7
                echo '{"hits":[{"file_path":"src/branch_local.rs","neural_executed":true}]}'
                exit 0
                ;;
            esac
            exit 2
            """,
            attempts=1,
            extra_args=["--check-worktree"],
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(attempts, 1)
        self.assertIn("Worktree neural retrieval procedure passed", result.stdout)


if __name__ == "__main__":
    unittest.main()
