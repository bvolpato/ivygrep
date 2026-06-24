import importlib.util
from pathlib import Path
import sys
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "eval_relevance.py"
SPEC = importlib.util.spec_from_file_location("eval_relevance", SCRIPT)
eval_relevance = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = eval_relevance
SPEC.loader.exec_module(eval_relevance)


class RelevanceEvaluationTest(unittest.TestCase):
    def test_neural_execution_is_unobservable_without_hits(self):
        self.assertIsNone(eval_relevance.neural_execution_status([]))
        self.assertTrue(
            eval_relevance.neural_execution_status([{"neural_executed": True}])
        )
        self.assertFalse(
            eval_relevance.neural_execution_status([{"neural_executed": False}])
        )


if __name__ == "__main__":
    unittest.main()
