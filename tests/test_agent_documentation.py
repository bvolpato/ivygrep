import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


class AgentDocumentationTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.readme = (ROOT / "README.md").read_text()
        cls.site = (ROOT / "docs" / "index.html").read_text()
        cls.unix_installer = (ROOT / "install.sh").read_text()
        cls.windows_installer = (ROOT / "install.ps1").read_text()

    def test_cursor_stdio_type_is_documented_everywhere(self) -> None:
        cursor_config = '"type": "stdio", "command": "ig", "args": ["--mcp"]'
        self.assertIn(cursor_config, self.readme)
        self.assertIn(cursor_config, self.site)

    def test_current_cli_setup_commands_are_documented(self) -> None:
        commands = [
            "claude mcp add -s user ig -- ig --mcp",
            "codex mcp add ig -- ig --mcp",
            "gemini mcp add --scope user --transport stdio ig ig --mcp",
        ]
        for command in commands:
            with self.subTest(command=command):
                self.assertIn(command, self.readme)
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
                self.assertIn(command, self.site)
        self.assertIn("Manual MCP setup", self.site)
        self.assertIn("preserves existing", self.readme)

    def test_task_context_packs_are_prominent_and_consistent(self) -> None:
        search_command = 'ig "where is refresh token rotated?"'
        context_command = (
            'ig context "fix refresh-token races" --since main --budget 8000'
        )
        for document in [self.readme, self.site]:
            with self.subTest(document=document[:20]):
                self.assertIn(search_command, document)
                self.assertIn(context_command, document)
                self.assertLess(
                    document.index(search_command), document.index(context_command)
                )
                self.assertIn("definitions", document)
                self.assertIn("callers", document)
                self.assertIn("references", document)
                self.assertIn("configuration", document)
                self.assertIn("dependencies", document)
                self.assertIn("dependents", document)
                self.assertIn("output=context_pack", document)
                self.assertIn("budget_tokens", document)
        self.assertIn('id="context-packs"', self.site)
        self.assertIn("complete Markdown pack", self.readme)

    def test_quick_start_has_one_search_and_one_context_command(self) -> None:
        readme_quick_start = self.readme.split("## Try it in 30 seconds", 1)[1].split(
            "## Install", 1
        )[0]
        site_demo = self.site.split("<!-- Demo terminal -->", 1)[1].split(
            "<!-- Social proof strip -->", 1
        )[0]

        for section in [readme_quick_start, site_demo]:
            with self.subTest(section=section[:20]):
                self.assertEqual(section.count('ig "where is refresh token rotated?"'), 1)
                self.assertEqual(
                    section.count(
                        'ig context "fix refresh-token races" --since main --budget 8000'
                    ),
                    1,
                )
                self.assertNotIn("ig agent", section)
                self.assertNotIn("--mcp", section)

    def test_opencode_uses_current_local_server_shape(self) -> None:
        self.assertIn('"type": "local"', self.readme)
        self.assertIn('"command": ["ig", "--mcp"]', self.readme)
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
        self.assertIn("Pass absolute current repository", self.readme)
        self.assertIn("absolute active worktree path", self.readme)
        self.assertIn("store only divergent chunks and tombstones", self.site)


if __name__ == "__main__":
    unittest.main()
