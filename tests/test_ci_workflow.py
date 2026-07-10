import unittest
from pathlib import Path


CI_WORKFLOW = (
    Path(__file__).resolve().parents[1] / ".github" / "workflows" / "ci.yml"
)


class CIWorkflowTest(unittest.TestCase):
    def test_ci_gate_requires_every_matrix(self) -> None:
        workflow = CI_WORKFLOW.read_text(encoding="utf-8")
        gate = workflow.split("  ci-gate:\n", maxsplit=1)[1]

        self.assertIn("name: CI gate", gate)
        self.assertIn("if: always()", gate)
        self.assertIn("needs: [check, test, build]", gate)
        self.assertIn('test "$CHECK_RESULT" = success', gate)
        self.assertIn('test "$TEST_RESULT" = success', gate)
        self.assertIn('test "$BUILD_RESULT" = success', gate)


if __name__ == "__main__":
    unittest.main()
