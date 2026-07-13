import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


class AgentDocumentationTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.readme = (ROOT / "README.md").read_text()
        cls.guide = (ROOT / "AGENT_INTEGRATION.md").read_text()
        cls.site = (ROOT / "docs" / "index.html").read_text()
        cls.unix_installer = (ROOT / "install.sh").read_text()
        cls.windows_installer = (ROOT / "install.ps1").read_text()

    def test_cursor_stdio_type_is_documented_everywhere(self) -> None:
        cursor_config = '"type": "stdio", "command": "ig", "args": ["--mcp"]'
        self.assertIn(cursor_config, self.readme)
        self.assertIn(cursor_config, self.site)
        self.assertIn('"type": "stdio"', self.guide)
        self.assertIn('"args": ["--mcp"]', self.guide)

    def test_current_cli_setup_commands_are_documented(self) -> None:
        commands = [
            "claude mcp add -s user ig -- ig --mcp",
            "codex mcp add ig -- ig --mcp",
            "gemini mcp add --scope user --transport stdio ig ig --mcp",
        ]
        for command in commands:
            with self.subTest(command=command):
                self.assertIn(command, self.readme)
                self.assertIn(command, self.guide)
                self.assertIn(command, self.site)

    def test_one_command_agent_setup_is_documented(self) -> None:
        commands = [
            "ig agent install claude",
            "ig agent install codex",
            "ig agent install cursor",
            "ig agent doctor",
        ]
        for command in commands:
            with self.subTest(command=command):
                self.assertIn(command, self.readme)
                self.assertIn(command, self.guide)
                self.assertIn(command, self.site)
        self.assertIn("Manual MCP setup", self.site)
        self.assertIn("preserves existing", self.readme)

    def test_task_context_packs_are_prominent_and_consistent(self) -> None:
        for document in [self.readme, self.guide, self.site]:
            with self.subTest(document=document[:20]):
                self.assertIn('ig context "fix refresh-token races" --budget 8000', document)
                self.assertIn("definitions", document)
                self.assertIn("callers", document)
                self.assertIn("references", document)
                self.assertIn("configuration", document)
        self.assertIn('id="context-packs"', self.site)
        self.assertIn("complete Markdown pack", self.readme)
        self.assertIn("complete-pack", self.guide)

    def test_opencode_uses_current_local_server_shape(self) -> None:
        self.assertIn('"type": "local"', self.readme)
        self.assertIn('"command": ["ig", "--mcp"]', self.readme)
        self.assertIn('"type": "local"', self.guide)
        self.assertIn('"command": ["ig", "--mcp"]', self.guide)
        self.assertIn('"type": "local"', self.site)

    def test_install_examples_are_one_command(self) -> None:
        unix_command = (
            "curl -fsSL "
            "https://raw.githubusercontent.com/bvolpato/ivygrep/main/install.sh | sh"
        )
        windows_command = (
            "irm "
            "https://raw.githubusercontent.com/bvolpato/ivygrep/main/install.ps1 | iex"
        )
        for document in [self.readme, self.site]:
            with self.subTest(document=document[:20]):
                self.assertIn("brew install bvolpato/tap/ivygrep", document)
                self.assertIn(unix_command, document)
                self.assertIn(windows_command, document)

    def test_installers_verify_release_checksums_and_configure_path(self) -> None:
        self.assertIn(".sha256", self.unix_installer)
        self.assertIn("sha256sum -c", self.unix_installer)
        self.assertIn("shasum -a 256 -c", self.unix_installer)
        self.assertIn("IVYGREP_INSTALL_DIR", self.unix_installer)
        self.assertIn("IVYGREP_ACCELERATOR", self.unix_installer)
        self.assertIn("linux-x86_64-cuda", self.unix_installer)
        self.assertIn("macos-aarch64-metal", self.unix_installer)
        self.assertIn("Linux x86_64 CUDA", self.readme)
        self.assertIn(".sha256", self.windows_installer)
        self.assertIn("Get-FileHash", self.windows_installer)
        self.assertIn("ivygrep-$tag-windows-x86_64.zip", self.windows_installer)
        self.assertIn('SetEnvironmentVariable("Path"', self.windows_installer)

    def test_agent_guidance_requires_explicit_worktree_scope(self) -> None:
        self.assertIn("Pass the absolute current repository or worktree path", self.readme)
        self.assertIn("Always pass `path`", self.guide)
        self.assertIn("Pass the active worktree root, not the main checkout", self.guide)
        self.assertIn("store only divergent chunks and tombstones", self.site)


if __name__ == "__main__":
    unittest.main()
