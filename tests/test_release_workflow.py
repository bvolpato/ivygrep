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
        builds = workflow.split("  build:\n", maxsplit=1)[1].split(
            "  artifact-acceptance:\n", maxsplit=1
        )[0]
        acceptance = workflow.split("  artifact-acceptance:\n", maxsplit=1)[1].split(
            "  release:\n", maxsplit=1
        )[0]

        self.assertIn("artifact-acceptance:", workflow)
        self.assertIn("needs: artifact-acceptance", workflow)
        self.assertIn("scripts/verify_release_artifact.py", workflow)
        self.assertIn("scripts/e2e_x86_baseline.sh", workflow)
        self.assertNotIn("scripts/cache_neural_model.py", builds)
        self.assertIn("scripts/cache_neural_model.py", acceptance)
        self.assertLess(
            acceptance.index("Install uv for neural model caching"),
            acceptance.index("Cache pinned neural model through Xet"),
        )
        uv_setup = acceptance.split(
            "- name: Install uv for neural model caching", maxsplit=1
        )[1].split("- name: Cache pinned neural model through Xet", maxsplit=1)[0]
        self.assertIn("enable-cache: false", uv_setup)

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

    def test_primed_model_acceptance_blocks_network_on_first_load(self) -> None:
        workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")
        metal = workflow.split("- name: Validate macOS Metal backend", maxsplit=1)[
            1
        ].split("- name: Run exact Linux aarch64 archive under QEMU", maxsplit=1)[0]
        linux = workflow.split(
            "- name: Prime and import cached neural model without network", maxsplit=1
        )[1].split(
            "- name: Validate Windows neural backend and cached offline reuse",
            maxsplit=1,
        )[0]
        windows = workflow.split(
            "- name: Validate Windows neural backend and cached offline reuse",
            maxsplit=1,
        )[1].split("  release:", maxsplit=1)[0]

        self.assertIn("HTTP_PROXY: http://127.0.0.1:9", metal)
        self.assertIn('HF_HOME="$IVYGREP_RELEASE_HF_CACHE"', metal)
        self.assertNotIn("HF_HOME: ${{ env.IVYGREP_RELEASE_HF_CACHE }}", metal)
        self.assertIn("HTTP_PROXY=http://127.0.0.1:9", linux)
        self.assertEqual(windows.count("HTTP_PROXY=http://127.0.0.1:9"), 2)

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

    def test_homebrew_uses_metal_archive_on_apple_silicon(self) -> None:
        workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")
        formula = workflow.split(
            "- name: Generate formula", maxsplit=1
        )[1].split("- name: Commit and push formula", maxsplit=1)[0]

        self.assertIn("SHA_MACOS_ARM64_METAL", formula)
        self.assertIn("macos-aarch64-metal.tar.gz", formula)
        self.assertNotIn(
            'ivygrep-${FORMULA_TAG}-macos-aarch64.tar.gz"', formula
        )

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

    def test_cuda_raw_binary_has_dedicated_bounded_size_budget(self) -> None:
        workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")
        binary_budget = workflow.split(
            "- name: Enforce release size budget", maxsplit=1
        )[1].split("- name: Build-output sanity check", maxsplit=1)[0]
        archive_budget = workflow.split(
            "- name: Enforce archive size budget", maxsplit=1
        )[1].split("- name: Compute checksum and provenance", maxsplit=1)[0]

        self.assertIn("MAX_BINARY_MIB=80", binary_budget)
        self.assertIn('[[ "$EXTRA_FEATURES" == "cuda" ]]', binary_budget)
        self.assertIn("MAX_BINARY_MIB=81", binary_budget)
        self.assertIn('--max-mib "$MAX_BINARY_MIB"', binary_budget)
        self.assertIn('--max-mib 80', archive_budget)

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
        self.assertIn("IVYGREP_RELEASE_HF_CACHE", windows_acceptance)
        self.assertIn("StaticEmbedding token mean via Rust", windows_acceptance)
        self.assertIn("HTTP_PROXY=http://127.0.0.1:9", windows_acceptance)

    def test_release_accepts_exact_installer_inputs(self) -> None:
        workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")
        self.assertIn("IVYGREP_INSTALL_ARCHIVE", workflow)
        self.assertIn("IVYGREP_INSTALL_CHECKSUM", workflow)
        self.assertIn("Validate exact Unix installer artifact", workflow)
        self.assertIn("Validate exact PowerShell installer artifact", workflow)
        self.assertIn("echo \"archive=$ARCHIVE\"", workflow)
        self.assertIn("echo \"checksum=$CHECKSUM\"", workflow)

    def test_cuda_release_acceptance_checks_backend_or_explicit_cpu_fallback(self) -> None:
        workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")
        cuda = workflow.split(
            "- name: Validate Linux CUDA backend and semantic retrieval", maxsplit=1
        )[1].split("- name: Run exact Linux aarch64 archive under QEMU", maxsplit=1)[0]
        self.assertIn('if: matrix.cuda', cuda)
        self.assertIn("BERT embedding via Candle CUDA", cuda)
        self.assertIn("BERT embedding via Candle CPU", cuda)
        self.assertIn('--expect-file "src/lib.rs"', cuda)


if __name__ == "__main__":
    unittest.main()
