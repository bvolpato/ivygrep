import hashlib
import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "cache_neural_model.py"
SPEC = importlib.util.spec_from_file_location("cache_neural_model", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class CacheNeuralModelTest(unittest.TestCase):
    def test_profiles_match_pinned_runtime_models(self) -> None:
        static = MODULE.PROFILES["static"]
        general = MODULE.PROFILES["general"]

        self.assertEqual(
            static.repo_id, "sentence-transformers/static-retrieval-mrl-en-v1"
        )
        self.assertEqual(static.revision, "f60985c706f192d45d218078e49e5a8b6f15283a")
        self.assertEqual(
            static.weights_sha256,
            "164fc63ee9f9267be7378fcbd7df99d09788a2f45244c92aa99ae5a574925716",
        )
        self.assertEqual(general.repo_id, "sentence-transformers/all-MiniLM-L6-v2")
        self.assertEqual(
            general.revision, "1110a243fdf4706b3f48f1d95db1a4f5529b4d41"
        )
        self.assertEqual(
            general.weights_sha256,
            "53aa51172d142c89d9012cce15ae4d6cc0ca6895895114379cacb4fab128d9db",
        )

    def test_file_sha256_streams_exact_bytes(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "weights.safetensors"
            content = b"pinned-neural-model" * 100_000
            path.write_bytes(content)

            self.assertEqual(
                MODULE.file_sha256(path), hashlib.sha256(content).hexdigest()
            )


if __name__ == "__main__":
    unittest.main()
