import hashlib
import importlib.util
import io
import json
import tarfile
import tempfile
import tomllib
import unittest
import zipfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PACKAGE_SCRIPT = ROOT / "scripts" / "package_mcpb.py"


def load_package_module():
    spec = importlib.util.spec_from_file_location("package_mcpb", PACKAGE_SCRIPT)
    if spec is None or spec.loader is None:
        raise RuntimeError("could not load package_mcpb.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class DistributionPackagesTest(unittest.TestCase):
    def test_agent_marketplaces_and_plugins_match_release_version(self) -> None:
        package = tomllib.loads((ROOT / "Cargo.toml").read_text())
        version = package["package"]["version"]
        codex_marketplace = json.loads(
            (ROOT / ".agents" / "plugins" / "marketplace.json").read_text()
        )
        claude_marketplace = json.loads(
            (ROOT / ".claude-plugin" / "marketplace.json").read_text()
        )

        self.assertEqual(codex_marketplace["plugins"][0]["name"], "ivygrep")
        self.assertEqual(claude_marketplace["plugins"][0]["name"], "ivygrep")
        for manifest in [
            ROOT / "plugins" / "ivygrep" / ".codex-plugin" / "plugin.json",
            ROOT / "plugins" / "ivygrep" / ".claude-plugin" / "plugin.json",
        ]:
            self.assertEqual(json.loads(manifest.read_text())["version"], version)

        mcp = json.loads((ROOT / "plugins" / "ivygrep" / ".mcp.json").read_text())[
            "mcpServers"
        ]["ivygrep"]
        self.assertEqual(mcp, {"command": "ig", "args": ["--mcp"]})

    def test_crates_use_publishable_versioned_forks(self) -> None:
        root = tomllib.loads((ROOT / "Cargo.toml").read_text())
        self.assertNotIn("patch", root)
        expected = {
            "candle_embed": ("ivygrep-candle-embed", "=0.1.4-ivygrep.1"),
            "hf-hub": ("ivygrep-hf-hub", "=0.3.2-ivygrep.1"),
            "usearch": ("ivygrep-usearch", "=2.24.0-ivygrep.1"),
            "tree-sitter-haskell": (
                "ivygrep-tree-sitter-haskell",
                "=0.23.1-ivygrep.1",
            ),
        }
        for dependency, (package, version) in expected.items():
            with self.subTest(dependency=dependency):
                config = root["dependencies"][dependency]
                self.assertEqual(config["package"], package)
                self.assertEqual(config["version"], version)
                self.assertTrue((ROOT / config["path"]).is_dir())

    def test_mcpb_builder_packages_each_target_and_pins_bundle_hash(self) -> None:
        package_mcpb = load_package_module()
        version = "9.8.7"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            artifacts = root / "artifacts"
            artifacts.mkdir()
            for suffix, (_, binary_name) in package_mcpb.TARGETS.items():
                archive_root = f"ivygrep-v{version}-{suffix}"
                payload = f"{suffix}-binary".encode()
                if suffix.startswith("windows-"):
                    archive = artifacts / f"{archive_root}.zip"
                    with zipfile.ZipFile(archive, "w") as bundle:
                        bundle.writestr(f"{archive_root}/{binary_name}", payload)
                else:
                    archive = artifacts / f"{archive_root}.tar.gz"
                    info = tarfile.TarInfo(f"{archive_root}/{binary_name}")
                    info.size = len(payload)
                    with tarfile.open(archive, "w:gz") as bundle:
                        bundle.addfile(info, io.BytesIO(payload))

            output = root / "ivygrep.mcpb"
            server_json = root / "server.json"
            package_mcpb.build(artifacts, version, output, server_json)

            with zipfile.ZipFile(output) as bundle:
                names = set(bundle.namelist())
                manifest = json.loads(bundle.read("manifest.json"))
            self.assertEqual(manifest["version"], version)
            self.assertIn("server/launcher.cjs", names)
            for target_dir, binary_name in package_mcpb.TARGETS.values():
                self.assertIn(f"server/bin/{target_dir}/{binary_name}", names)

            registry = json.loads(server_json.read_text())
            self.assertEqual(registry["version"], version)
            self.assertEqual(
                registry["packages"][0]["fileSha256"],
                hashlib.sha256(output.read_bytes()).hexdigest(),
            )

    def test_winget_manifest_targets_verified_portable_archive(self) -> None:
        path = (
            ROOT
            / "packaging"
            / "winget"
            / "BrunoVolpato.ivygrep"
            / "1.2.6"
            / "BrunoVolpato.ivygrep.installer.yaml"
        )
        manifest = path.read_text()
        self.assertIn("PackageVersion: 1.2.6", manifest)
        self.assertIn("InstallerType: zip", manifest)
        self.assertIn("NestedInstallerType: portable", manifest)
        self.assertIn("PortableCommandAlias: ig", manifest)
        self.assertIn(
            "InstallerSha256: 3B83FAFAA40A5221C0140D2D6C041AA7DAD8BF3ACAE5D1C9F01006D6DF86FF8A",
            manifest,
        )
        self.assertFalse(
            (ROOT / "packaging" / "winget" / "BrunoVolpato.ivygrep" / "1.2.7").exists(),
            "do not advertise a WinGet version before registry publication",
        )

    def test_each_supported_agent_has_a_setup_page(self) -> None:
        pages = {
            "codex.html": "codex plugin add ivygrep@ivygrep",
            "claude-code.html": "claude plugin install ivygrep@ivygrep",
            "cursor.html": "ig agent install cursor",
            "gemini-cli.html": "gemini mcp add",
            "opencode.html": '"type": "local"',
        }
        for page, command in pages.items():
            with self.subTest(page=page):
                document = (ROOT / "docs" / "integrations" / page).read_text()
                self.assertIn(command, document)
                self.assertIn("mcp.html", document)

    def test_release_workflows_publish_mcp_and_crates(self) -> None:
        release = (ROOT / ".github" / "workflows" / "release.yml").read_text()
        crates = (ROOT / ".github" / "workflows" / "publish-crates.yml").read_text()
        self.assertIn("scripts/package_mcpb.py", release)
        self.assertIn("mcp-publisher login github-oidc", release)
        self.assertIn("id-token: write", release)
        order = [
            "ivygrep-hf-hub",
            "ivygrep-candle-embed",
            "ivygrep-usearch",
            "ivygrep-tree-sitter-haskell",
            'publish Cargo.toml ivygrep "$MAIN_VERSION"',
        ]
        positions = [crates.index(package) for package in order]
        self.assertEqual(positions, sorted(positions))


if __name__ == "__main__":
    unittest.main()
