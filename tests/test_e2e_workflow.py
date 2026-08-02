import unittest
from pathlib import Path


E2E_WORKFLOW = (
    Path(__file__).resolve().parents[1]
    / ".github"
    / "workflows"
    / "e2e-cross-platform.yml"
)


class E2EWorkflowTest(unittest.TestCase):
    def test_aarch64_smoke_uses_git_and_python_images(self) -> None:
        workflow = E2E_WORKFLOW.read_text(encoding="utf-8")
        arm = workflow.split("cross-linux-aarch64:", maxsplit=1)[1].split(
            "# ── Hash-only mode", maxsplit=1
        )[0]

        self.assertIn("--network none", arm)
        self.assertIn("--entrypoint sh", arm)
        self.assertIn("alpine/git@sha256:", arm)
        self.assertIn("python:3.13-alpine", arm)
        self.assertNotIn("alpine:latest", arm)
        self.assertNotIn("apk add", arm)
        self.assertLess(
            arm.index("alpine/git@sha256:"),
            arm.index('sh scripts/e2e_procedures.sh --binary "$IG"'),
        )
        self.assertLess(
            arm.index("python:3.13-alpine"),
            arm.index("python3 scripts/check_daemon_equivalence.py"),
        )

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

    def test_ci_gates_semantic_neural_and_browser_acceptance(self) -> None:
        ci = (E2E_WORKFLOW.parent / "ci.yml").read_text(encoding="utf-8")
        self.assertIn("--expect-file \"src/lib.rs\"", ci)
        self.assertIn("Install Playwright Chromium for Web UI E2E", ci)
        self.assertIn("scripts/e2e_web_ui.sh --binary", ci)


if __name__ == "__main__":
    unittest.main()
