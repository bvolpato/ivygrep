import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


class AgentDocumentationTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.site = (ROOT / "docs" / "index.html").read_text()
        cls.unix_installer = (ROOT / "install.sh").read_text()
        cls.windows_installer = (ROOT / "install.ps1").read_text()

    def test_cursor_stdio_type_is_documented_on_site(self) -> None:
        cursor_config = '"type": "stdio", "command": "ig", "args": ["--mcp"]'
        self.assertIn(cursor_config, self.site)

    def test_current_cli_setup_commands_are_documented(self) -> None:
        commands = [
            "claude mcp add -s user ig -- ig --mcp",
            "codex mcp add ig -- ig --mcp",
            "gemini mcp add --scope user --transport stdio ig ig --mcp",
        ]
        for command in commands:
            with self.subTest(command=command):
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
                self.assertIn(command, self.site)
        self.assertIn("Manual MCP setup", self.site)

    def test_task_context_packs_are_prominent_on_site(self) -> None:
        context_command = (
            'ig context "fix refresh-token races" --since main --budget 8000'
        )
        search_command = 'ig "what did we decide about cache invalidation?" ~/notes'
        self.assertIn(search_command, self.site)
        self.assertIn(context_command, self.site)
        self.assertLess(
            self.site.index(search_command), self.site.index(context_command)
        )
        for term in (
            "definitions",
            "callers",
            "references",
            "configuration",
            "dependencies",
            "dependents",
            "output=context_pack",
            "budget_tokens",
        ):
            self.assertIn(term, self.site)
        self.assertIn('id="context-packs"', self.site)

    def test_quick_start_has_one_search_and_one_context_command(self) -> None:
        site_demo = self.site.split("<!-- Demo terminal -->", 1)[1].split(
            "<!-- Social proof strip -->", 1
        )[0]

        search_command = 'ig "what did we decide about cache invalidation?" ~/notes'
        context_command = (
            'ig context "fix refresh-token races" --since main --budget 8000'
        )
        self.assertEqual(site_demo.count(search_command), 1)
        self.assertEqual(site_demo.count(context_command), 1)
        self.assertNotIn("ig agent", site_demo)
        self.assertNotIn("--mcp", site_demo)

    def test_opencode_uses_current_local_server_shape(self) -> None:
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
        self.assertIn("brew install bvolpato/tap/ivygrep", self.site)
        self.assertIn(unix_command, self.site)
        self.assertIn(windows_command, self.site)

    def test_installers_verify_release_checksums_and_configure_path(self) -> None:
        self.assertIn(".sha256", self.unix_installer)
        self.assertIn("sha256sum -c", self.unix_installer)
        self.assertIn("shasum -a 256 -c", self.unix_installer)
        self.assertIn("IVYGREP_INSTALL_DIR", self.unix_installer)
        self.assertIn("IVYGREP_ACCELERATOR", self.unix_installer)
        self.assertIn("linux-x86_64-cuda", self.unix_installer)
        self.assertIn("macos-aarch64-metal", self.unix_installer)
        self.assertIn(".sha256", self.windows_installer)
        self.assertIn("Get-FileHash", self.windows_installer)
        self.assertIn("ivygrep-$tag-windows-x86_64.zip", self.windows_installer)
        self.assertIn('SetEnvironmentVariable("Path"', self.windows_installer)

    def test_agent_guidance_requires_explicit_worktree_scope(self) -> None:
        self.assertIn("store only divergent chunks and tombstones", self.site)


if __name__ == "__main__":
    unittest.main()
