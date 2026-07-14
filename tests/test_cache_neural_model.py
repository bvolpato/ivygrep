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

    def test_writes_revision_ref_for_rust_cache_reader(self) -> None:
        profile = MODULE.PROFILES["static"]
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp) / "hub" / "models--sentence-transformers--static"
            snapshot = repo / "snapshots" / profile.revision
            downloaded = {
                asset: snapshot / asset
                for asset in profile.assets
            }
            for path in downloaded.values():
                path.parent.mkdir(parents=True, exist_ok=True)
                path.touch()

            ref = MODULE.write_rust_revision_ref(profile, downloaded)

            self.assertEqual(ref, repo / "refs" / profile.revision)
            self.assertEqual(ref.read_text(encoding="utf-8"), profile.revision)

    def test_rejects_snapshot_that_does_not_match_pinned_revision(self) -> None:
        profile = MODULE.PROFILES["general"]
        with tempfile.TemporaryDirectory() as tmp:
            snapshot = Path(tmp) / "hub" / "model" / "snapshots" / ("0" * 40)
            downloaded = {asset: snapshot / asset for asset in profile.assets}

            with self.assertRaisesRegex(SystemExit, "unexpected model snapshot"):
                MODULE.write_rust_revision_ref(profile, downloaded)


if __name__ == "__main__":
    unittest.main()
