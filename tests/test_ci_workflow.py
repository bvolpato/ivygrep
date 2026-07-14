import unittest
from pathlib import Path


CI_WORKFLOW = (
    Path(__file__).resolve().parents[1] / ".github" / "workflows" / "ci.yml"
)


class CIWorkflowTest(unittest.TestCase):
    def test_neural_builds_prime_models_with_xet(self) -> None:
        workflow = CI_WORKFLOW.read_text(encoding="utf-8")
        tests = workflow.split("  test:\n", maxsplit=1)[1].split(
            "  build:\n", maxsplit=1
        )[0]
        builds = workflow.split("  build:\n", maxsplit=1)[1].split(
            "  ci-gate:\n", maxsplit=1
        )[0]

        self.assertNotIn("Install uv for neural model caching", tests)
        self.assertIn("Install uv for neural model caching", builds)
        self.assertIn("uv run scripts/cache_neural_model.py", builds)
        neural_validation = builds.split(
            "- name: Validate local neural execution", maxsplit=1
        )[1]
        priming, offline_load = neural_validation.split(
            "HTTP_PROXY=http://127.0.0.1:9", maxsplit=1
        )
        self.assertIn("uv run scripts/cache_neural_model.py", priming)
        self.assertNotIn("HTTP_PROXY", priming)
        self.assertIn("NO_PROXY=''", offline_load)
        self.assertIn("./scripts/e2e_neural_backend.sh", offline_load)
        self.assertLess(
            builds.index("Install uv for neural model caching"),
            builds.index("Validate local neural execution"),
        )

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
