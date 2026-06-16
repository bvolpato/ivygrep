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

    def test_release_publishes_sbom_and_provenance(self) -> None:
        workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")
        self.assertIn("anchore/sbom-action@", workflow)
        self.assertIn("actions/attest@", workflow)
        self.assertIn('--cargo-flags "$CARGO_FLAGS"', workflow)
        self.assertIn("*.spdx.json", workflow)
        self.assertIn("*.provenance.json", workflow)


if __name__ == "__main__":
    unittest.main()
