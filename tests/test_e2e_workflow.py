import unittest
from pathlib import Path


E2E_WORKFLOW = (
    Path(__file__).resolve().parents[1]
    / ".github"
    / "workflows"
    / "e2e-cross-platform.yml"
)


class E2EWorkflowTest(unittest.TestCase):
    def test_aarch64_alpine_installs_python_for_procedure_checks(self) -> None:
        workflow = E2E_WORKFLOW.read_text(encoding="utf-8")

        self.assertIn("alpine:latest sh -c", workflow)
        self.assertIn("apk add --no-cache python3", workflow)
        self.assertLess(
            workflow.index("apk add --no-cache python3"),
            workflow.index('sh scripts/e2e_procedures.sh --binary "$IG"'),
        )


if __name__ == "__main__":
    unittest.main()
