import unittest
from pathlib import Path


RELEASE_WORKFLOW = (
    Path(__file__).resolve().parents[1] / ".github" / "workflows" / "release.yml"
)


class ReleaseWorkflowTest(unittest.TestCase):
    def test_linux_x86_release_uses_baseline_cpu(self) -> None:
        workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")
        forbidden_cpu_targets = (
            "target-cpu=native",
            "target-cpu=x86-64-v2",
            "target-cpu=x86-64-v3",
            "target-cpu=x86-64-v4",
        )

        for target in forbidden_cpu_targets:
            self.assertNotIn(target, workflow)

    def test_release_waits_for_archive_acceptance(self) -> None:
        workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")
        self.assertIn("artifact-acceptance:", workflow)
        self.assertIn("needs: artifact-acceptance", workflow)
        self.assertIn("scripts/verify_release_artifact.py", workflow)
        self.assertIn("scripts/e2e_x86_baseline.sh", workflow)

    def test_native_daemon_acceptance_uses_platform_temp_default(self) -> None:
        workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")
        native_acceptance = workflow.split(
            "- name: Run exact archive procedures", maxsplit=1
        )[1].split(
            "- name: Run exact Linux aarch64 archive under QEMU", maxsplit=1
        )[0]

        self.assertIn("scripts/check_daemon_equivalence.py", native_acceptance)
        self.assertNotIn("--bench-home", native_acceptance)
        self.assertNotIn("${RUNNER_TEMP:-$TEMP}", workflow)

    def test_linux_arm_acceptance_has_git_without_network_access(self) -> None:
        workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")
        arm_acceptance = workflow.split(
            "- name: Run exact Linux aarch64 archive under QEMU", maxsplit=1
        )[1].split("- name: Reject elevated x86 ISA requirements", maxsplit=1)[0]

        self.assertIn("--network none", arm_acceptance)
        self.assertIn("--entrypoint sh", arm_acceptance)
        self.assertIn("alpine/git@sha256:", arm_acceptance)

    def test_release_publishes_sbom_and_provenance(self) -> None:
        workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")
        self.assertIn("anchore/sbom-action@", workflow)
        self.assertIn(
            "file: target/${{ matrix.target }}/release/${{ matrix.binary_name }}",
            workflow,
        )
        self.assertNotIn(
            "path: target/${{ matrix.target }}/release/${{ matrix.binary_name }}",
            workflow,
        )
        self.assertIn("actions/attest@", workflow)
        self.assertIn('--features="$EXTRA_FEATURES"', workflow)
        self.assertIn('--cargo-flags="$CARGO_FLAGS"', workflow)
        self.assertIn("*.spdx.json", workflow)
        self.assertIn("*.provenance.json", workflow)

    def test_release_includes_shell_installer_macos_metal_archive(self) -> None:
        workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")

        self.assertIn("archive_name: macos-aarch64-metal", workflow)
        self.assertIn("extra_features: accelerate,metal", workflow)
        self.assertIn("matrix.archive_name == 'macos-aarch64-metal'", workflow)
        self.assertIn('--expect-backend "Candle Metal"', workflow)

    def test_release_includes_shell_installer_cuda_archive(self) -> None:
        workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")
        cuda_toolkit = workflow.split(
            "- name: Install CUDA toolkit", maxsplit=1
        )[1].split("- name: Expose CUDA build libraries", maxsplit=1)[0]

        self.assertIn("archive_name: linux-x86_64-cuda", workflow)
        self.assertIn("target: x86_64-unknown-linux-gnu", workflow)
        self.assertIn("extra_features: cuda", workflow)
        self.assertIn("CUDA_COMPUTE_CAP", workflow)
        self.assertIn(
            "Jimver/cuda-toolkit@3d45d157f327c09c04b50ee6ccdea2d9d017ec76",
            workflow,
        )
        self.assertIn("libcuda.so.1", workflow)
        self.assertIn('"nvrtc-dev"', cuda_toolkit)
        self.assertIn("libcublas-dev", workflow)
        self.assertIn("libcurand-dev", workflow)

    def test_windows_release_includes_neural_offline_acceptance(self) -> None:
        workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")
        windows_build = workflow.split(
            "- target: x86_64-pc-windows-msvc", maxsplit=1
        )[1].split("steps:", maxsplit=1)[0]
        windows_acceptance = workflow.split(
            "- name: Validate Windows neural backend and cached offline reuse",
            maxsplit=1,
        )[1].split("release:", maxsplit=1)[0]

        self.assertNotIn("--no-default-features", windows_build)
        self.assertIn('grep -aFq "longPathAware"', workflow)
        self.assertIn("VCRUNTIME", workflow)
        self.assertIn("MSVCP", workflow)
        self.assertIn("scripts/e2e_cached_model.sh", windows_acceptance)
        self.assertIn("StaticEmbedding token mean via Rust", windows_acceptance)
        self.assertIn("HTTP_PROXY=http://127.0.0.1:9", windows_acceptance)


if __name__ == "__main__":
    unittest.main()
