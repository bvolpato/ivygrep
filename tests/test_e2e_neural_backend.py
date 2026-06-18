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
        self, fake_body: str, attempts: int
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
            result = subprocess.run(
                [
                    "sh",
                    str(HELPER),
                    "--binary",
                    str(fake),
                    "--expect-backend",
                    "StaticEmbedding token mean via Rust",
                ],
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
                echo "StaticEmbedding token mean via Rust"
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


if __name__ == "__main__":
    unittest.main()
