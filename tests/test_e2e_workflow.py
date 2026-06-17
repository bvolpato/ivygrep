import unittest
from pathlib import Path


E2E_WORKFLOW = (
    Path(__file__).resolve().parents[1]
    / ".github"
    / "workflows"
    / "e2e-cross-platform.yml"
)


class E2EWorkflowTest(unittest.TestCase):
    def test_aarch64_procedure_checks_do_not_require_python(self) -> None:
        workflow = E2E_WORKFLOW.read_text(encoding="utf-8")

        self.assertIn("alpine:latest sh -c", workflow)
        procedure = workflow.index('sh scripts/e2e_procedures.sh --binary "$IG"')
        python_install = workflow.index("apk add --no-cache python3")
        self.assertLess(procedure, python_install)

    def test_windows_runs_neural_and_unicode_path_acceptance(self) -> None:
        workflow = E2E_WORKFLOW.read_text(encoding="utf-8")
        windows = workflow.split("native-windows-x86_64:", maxsplit=1)[1].split(
            "# ── Summary", maxsplit=1
        )[0]

        self.assertIn("cargo build --locked --release", windows)
        self.assertIn("cargo test --locked --lib --bins --tests", windows)
        self.assertNotIn("--no-default-features", windows)
        self.assertIn("scripts/e2e_neural_backend.sh", windows)
        self.assertIn("StaticEmbedding token mean via Rust", windows)
        self.assertIn("ivygrep-数据-é", windows)
        self.assertIn("segment_08_abcdefghijklmnopqrstuvwxyz", windows)


if __name__ == "__main__":
    unittest.main()
