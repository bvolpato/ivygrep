import os
import stat
import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
HELPER = ROOT / "scripts" / "e2e_cached_model.sh"


class CachedModelE2ETest(unittest.TestCase):
    def test_cached_model_flow_checks_semantic_retrieval(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            fake = root / "ig"
            cache = root / "cache"
            fake.write_text(
                textwrap.dedent(
                    """
                    #!/bin/sh
                    set -eu
                    case "$1" in
                      --add|--enhance-internal)
                        exit 0
                        ;;
                      --status)
                        echo '{"chunk_count":1,"has_neural_vectors":true,"neural_model": {}}'
                        exit 0
                        ;;
                      --json)
                        [ "$2" = "--force-neural" ] && [ "$3" = "--limit" ] && [ "$4" = "1" ] || exit 7
                        echo '{"results":[{"file_path":"lib.rs","hits":[{"file_path":"lib.rs","neural_executed":true}]}]}'
                        exit 0
                        ;;
                    esac
                    exit 2
                    """
                ),
                encoding="utf-8",
            )
            fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            env = os.environ.copy()
            env["IVYGREP_E2E_SEMANTIC_QUERY"] = "portable neural cache search"
            result = subprocess.run(
                [
                    "sh",
                    str(HELPER),
                    "--binary",
                    str(fake),
                    "--cache",
                    str(cache),
                    "--expect-file",
                    "lib.rs",
                ],
                cwd=ROOT,
                env=env,
                text=True,
                capture_output=True,
                check=False,
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("cached semantic retrieval passed", result.stdout)

    def test_cached_model_flow_requires_neural_execution(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            fake = root / "ig"
            cache = root / "cache"
            fake.write_text(
                textwrap.dedent(
                    """
                    #!/bin/sh
                    set -eu
                    case "$1" in
                      --add|--enhance-internal)
                        exit 0
                        ;;
                      --status)
                        echo '{"chunk_count":1,"has_neural_vectors":true,"neural_model": {}}'
                        exit 0
                        ;;
                      --json)
                        [ "$2" = "--force-neural" ] || exit 7
                        echo '{"results":[{"file_path":"lib.rs","hits":[{"file_path":"lib.rs"}]}]}'
                        exit 0
                        ;;
                    esac
                    exit 2
                    """
                ),
                encoding="utf-8",
            )
            fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            result = subprocess.run(
                [
                    "sh",
                    str(HELPER),
                    "--binary",
                    str(fake),
                    "--cache",
                    str(cache),
                    "--expect-file",
                    "lib.rs",
                ],
                cwd=ROOT,
                text=True,
                capture_output=True,
                check=False,
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("did not execute neural retrieval", result.stderr)


if __name__ == "__main__":
    unittest.main()
