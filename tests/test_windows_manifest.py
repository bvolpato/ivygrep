import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


class WindowsManifestTest(unittest.TestCase):
    def test_msvc_binary_embeds_long_path_manifest(self) -> None:
        build_script = (ROOT / "build.rs").read_text(encoding="utf-8")
        manifest = (
            ROOT / "assets" / "windows" / "ivygrep.manifest"
        ).read_text(encoding="utf-8")

        self.assertIn("CARGO_CFG_TARGET_OS", build_script)
        self.assertIn("CARGO_CFG_TARGET_ENV", build_script)
        self.assertIn("CARGO_MANIFEST_DIR", build_script)
        self.assertIn("/MANIFEST:EMBED", build_script)
        self.assertIn("/MANIFESTINPUT:", build_script)
        self.assertIn("<longPathAware", manifest)
        self.assertIn(">true</longPathAware>", manifest)

    def test_windows_target_and_usearch_use_static_crt(self) -> None:
        cargo_config = (ROOT / ".cargo" / "config.toml").read_text(encoding="utf-8")
        usearch_build = (ROOT / "vendor" / "usearch" / "build.rs").read_text(
            encoding="utf-8"
        )

        self.assertIn("[target.x86_64-pc-windows-msvc]", cargo_config)
        self.assertIn("target-feature=+crt-static", cargo_config)
        self.assertIn(".static_crt(true)", usearch_build)
        self.assertNotIn('.flag_if_supported("/MD")', usearch_build)


if __name__ == "__main__":
    unittest.main()
