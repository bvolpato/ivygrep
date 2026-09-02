import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


E2E_WORKFLOW = (
    Path(__file__).resolve().parents[1]
    / ".github"
    / "workflows"
    / "e2e-cross-platform.yml"
)


class E2EWorkflowTest(unittest.TestCase):
    def test_local_e2e_uses_cargo_target_directory(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            shutil.copy2(E2E_WORKFLOW.parents[2] / "test.sh", root / "test.sh")
            (root / "scripts").mkdir()
            runner = root / "scripts/e2e_all.sh"
            runner.write_text('#!/bin/sh\nprintf "%s\\n" "$2" > "$E2E_RESULT"\n')
            runner.chmod(0o755)
            bin_dir = root / "bin"
            bin_dir.mkdir()
            cargo = bin_dir / "cargo"
            cargo.write_text(
                '#!/bin/sh\nif [ "$1" = metadata ]; then cat "$CARGO_METADATA"; fi\n'
            )
            cargo.chmod(0o755)
            target = root / "custom target"
            metadata = root / "metadata.json"
            metadata.write_text(json.dumps({"target_directory": str(target)}))
            result = root / "e2e-result"
            env = {
                **os.environ,
                "PATH": str(bin_dir) + os.pathsep + os.environ["PATH"],
                "CARGO_METADATA": str(metadata),
                "E2E_RESULT": str(result),
            }
            for profile in ("debug", "release"):
                with self.subTest(profile=profile):
                    args = ["bash", "test.sh", "--quick", "--hash-only", "--e2e"]
                    if profile == "release":
                        args.append("--release")
                    subprocess.run(args, cwd=root, env=env, check=True, capture_output=True)
                    self.assertEqual(result.read_text().strip(), str(target / profile / "ig"))

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
