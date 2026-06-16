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


if __name__ == "__main__":
    unittest.main()
