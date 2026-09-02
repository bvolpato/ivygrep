import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]


def load_script(name: str):
    path = ROOT / "scripts" / f"{name}.py"
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


verifier = load_script("verify_release_artifact")
sys.path.insert(0, str(ROOT / "scripts"))
readiness = load_script("check_release_readiness")


class ReleaseArtifactTest(unittest.TestCase):
    def release_fixture(self, root: Path) -> None:
        files = {
            "Cargo.toml": '[package]\nname = "ivygrep"\nversion = "1.2.13"\n',
            "Cargo.lock": '[[package]]\nname = "ivygrep"\nversion = "1.2.13"\n',
            "CHANGELOG.md": '## [1.2.13] - 2026-09-02\n\n- Tested release.\n',
            "docs/index.html": "<span>v1.2.13</span>",
            "src/lib.rs": "pub fn fixture() {}\n",
            "scripts/eval_relevance.py": "# fixture harness\n",
            "tests/fixtures/ivygrep_relevance_queries.json": '{"queries": [{"id": "one"}]}',
            **{path: '{"version": "1.2.13"}' for path in readiness.PLUGIN_MANIFESTS},
        }
        for name, content in files.items():
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content)
        quality = {
            "queries": 1, "mean_ndcg10": 1.0, "mean_mrr": 1.0,
            "mean_candidate_recall": 1.0, "no_hit_queries": 0,
            "neural_queries_executed": 1, "neural_queries_unobservable": 0,
        }
        report = {
            "binary": {"version": "ivygrep 1.2.13"},
            "source": {"sha256": readiness.relevance.source_inputs_sha256(root)},
            "fixture": {"sha256": readiness.relevance.sha256_file(root / "tests/fixtures/ivygrep_relevance_queries.json")},
            "harness": {"sha256": readiness.relevance.sha256_file(root / "scripts/eval_relevance.py")},
            "modes": {mode: quality for mode in ("foreground", "hash-enriched", "neural")},
        }
        path = root / "docs/benchmarks/current-head-relevance.json"
        path.parent.mkdir(parents=True)
        path.write_text(json.dumps(report))

    def test_release_preflight_rejects_version_drift_and_empty_notes(self) -> None:
        for file in ("Cargo.toml", "Cargo.lock", *readiness.PLUGIN_MANIFESTS, "docs/index.html", "CHANGELOG.md"):
            with self.subTest(file=file), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                self.release_fixture(root)
                self.assertEqual(readiness.validate_release(root, "v1.2.13"), [])
                path = root / file
                path.write_text(path.read_text().replace("1.2.13", "1.2.12"))
                self.assertTrue(any(file in error for error in readiness.validate_release(root, "v1.2.13")))
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.release_fixture(root)
            (root / "CHANGELOG.md").write_text("## [1.2.13]\n\n### Fixed\n")
            self.assertTrue(any("no release notes" in error for error in readiness.validate_release(root, "v1.2.13")))

    def test_release_preflight_rejects_stale_source_and_missing_neural_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.release_fixture(root)
            self.assertTrue(readiness.validate_release(root, "not-a-tag"))
            source = root / "src/lib.rs"
            source.write_text("pub fn changed() {}\n")
            self.assertTrue(any("source SHA-256" in error for error in readiness.validate_release(root, "v1.2.13")))
            source.write_text("pub fn fixture() {}\n")
            path = root / "docs/benchmarks/current-head-relevance.json"
            report = json.loads(path.read_text())
            report["modes"].pop("neural")
            path.write_text(json.dumps(report))
            self.assertTrue(any("missing neural" in error for error in readiness.validate_release(root, "v1.2.13")))

    def test_checksum_requires_matching_archive_name_and_digest(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            archive = root / "ivygrep-v1-linux.tar.gz"
            archive.write_bytes(b"archive")
            checksum = root / f"{archive.name}.sha256"
            digest = hashlib.sha256(b"archive").hexdigest()
            checksum.write_text(f"{digest}  {archive.name}\n", encoding="utf-8")
            self.assertEqual(verifier.verify_checksum(archive, checksum), digest)

            checksum.write_text(f"{digest}  another.tar.gz\n", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "does not name"):
                verifier.verify_checksum(archive, checksum)

    def test_safe_extract_rejects_parent_traversal(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            archive = root / "bad.tar.gz"
            payload = root / "payload"
            payload.write_text("bad", encoding="utf-8")
            with tarfile.open(archive, "w:gz") as handle:
                handle.add(payload, arcname="../outside")
            with self.assertRaisesRegex(ValueError, "unsafe path"):
                verifier.extract_archive(archive, root / "extract")

    def test_provenance_path_cannot_escape_extract_root(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            with self.assertRaisesRegex(ValueError, "unsafe archive path"):
                verifier.safe_child(root, "..", "ig")
            with self.assertRaisesRegex(ValueError, "unsafe archive path"):
                verifier.safe_child(root, "archive/subdir", "ig")

    def test_provenance_shape_is_path_neutral(self) -> None:
        document = {
            "schema_version": 1,
            "source": {"commit": "a" * 40},
            "build": {"target": "x86_64-unknown-linux-musl"},
            "artifact": {"name": "ivygrep-v1-linux.tar.gz"},
            "binary": {"name": "ig"},
            "sbom": {"name": "ivygrep-v1-linux.spdx.json"},
        }
        rendered = json.dumps(document)
        self.assertNotIn("/home/", rendered)
        self.assertNotIn("\\\\Users\\\\", rendered)

    def test_provenance_accepts_option_like_cargo_flags(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            archive = root / "ivygrep-v1-windows.zip"
            binary = root / "ig.exe"
            sbom = root / "ivygrep-v1-windows.spdx.json"
            output = root / "ivygrep-v1-windows.provenance.json"
            archive.write_bytes(b"archive")
            binary.write_bytes(b"binary")
            sbom.write_text('{"spdxVersion":"SPDX-2.3"}\n', encoding="utf-8")

            subprocess.run(
                [
                    sys.executable,
                    str(ROOT / "scripts" / "generate_release_provenance.py"),
                    f"--archive={archive}",
                    "--archive-root=ivygrep-v1-windows",
                    f"--binary={binary}",
                    f"--sbom={sbom}",
                    "--target=x86_64-pc-windows-msvc",
                    "--features=",
                    "--cargo-flags=--no-default-features",
                    "--version=1.0.0",
                    f"--source-commit={'a' * 40}",
                    "--source-ref=refs/tags/v1.0.0",
                    "--workflow-run-id=123",
                    "--rustc-version=rustc 1.96.0",
                    f"--output={output}",
                ],
                check=True,
            )

            document = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual(
                document["build"]["cargo_flags"],
                ["--no-default-features"],
            )


if __name__ == "__main__":
    unittest.main()
