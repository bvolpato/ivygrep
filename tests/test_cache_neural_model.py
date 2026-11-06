import hashlib
import importlib.util
import os
import sys
import tempfile
import unittest
from pathlib import Path
from types import ModuleType
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "cache_neural_model.py"
SPEC = importlib.util.spec_from_file_location("cache_neural_model", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class CacheNeuralModelTest(unittest.TestCase):
    def test_default_profile_matches_pinned_runtime_model(self) -> None:
        potion = MODULE.PROFILES["potion-code-v2"]
        embedding = (ROOT / "src" / "embedding.rs").read_text(encoding="utf-8")

        self.assertIn("pub const DEFAULT: Self = Self::PotionCodeV2;", embedding)
        self.assertEqual(potion.repo_id, "minishlab/potion-code-16M-v2")
        self.assertEqual(potion.assets, ("tokenizer.json", "model.safetensors"))
        for pinned in (potion.repo_id, potion.revision, potion.weights_sha256):
            self.assertIn(f'"{pinned}"', embedding)

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

    def test_cache_profile_selects_patched_tls_bundle_without_user_override(self) -> None:
        profile = MODULE.PROFILES["potion-code-v2"]
        patched_bundle = "/patched/certifi.pem"
        cases = (
            ({}, patched_bundle, None),
            ({"SSL_CERT_FILE": ""}, patched_bundle, None),
            ({"SSL_CERT_DIR": ""}, patched_bundle, ""),
            ({"SSL_CERT_FILE": "/user/custom.pem"}, "/user/custom.pem", None),
            ({"SSL_CERT_DIR": "/user/custom-certs"}, None, "/user/custom-certs"),
        )

        for environment, expected_file, expected_dir in cases:
            with self.subTest(environment=environment):
                with tempfile.TemporaryDirectory() as tmp:
                    cache = Path(tmp)
                    fake_hub = ModuleType("huggingface_hub")

                    def download_asset(
                        repo_id: str,
                        filename: str,
                        revision: str,
                        cache_dir: str,
                    ) -> str:
                        repo_name = f"models--{repo_id.replace('/', '--')}"
                        repo_dir = Path(cache_dir) / repo_name
                        path = repo_dir / "snapshots" / revision / filename
                        path.parent.mkdir(parents=True, exist_ok=True)
                        path.write_bytes(b"test asset")
                        return str(path)

                    fake_hub.hf_hub_download = download_asset
                    fake_certifi = ModuleType("certifi")
                    fake_certifi.where = lambda: patched_bundle

                    with mock.patch.dict(os.environ, environment, clear=True):
                        with mock.patch.dict(
                            sys.modules,
                            {"certifi": fake_certifi, "huggingface_hub": fake_hub},
                        ):
                            with mock.patch.object(
                                MODULE,
                                "file_sha256",
                                return_value=profile.weights_sha256,
                            ):
                                with mock.patch("builtins.print"):
                                    MODULE.cache_profile("potion-code-v2", cache)

                        self.assertEqual(
                            os.environ.get("SSL_CERT_FILE"), expected_file
                        )
                        self.assertEqual(os.environ.get("SSL_CERT_DIR"), expected_dir)


if __name__ == "__main__":
    unittest.main()
