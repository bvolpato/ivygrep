import re
import tomllib
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


class CommunityHealthTest(unittest.TestCase):
    def read(self, path: str) -> str:
        return (ROOT / path).read_text(encoding="utf-8")

    def test_required_community_files_exist_without_placeholders(self) -> None:
        required = [
            "CODE_OF_CONDUCT.md",
            "CONTRIBUTING.md",
            "GOVERNANCE.md",
            "LICENSE",
            "README.md",
            "SECURITY.md",
            "SUPPORT.md",
            ".github/pull_request_template.md",
        ]
        for path in required:
            with self.subTest(path=path):
                self.assertTrue((ROOT / path).is_file())
                text = self.read(path)
                for placeholder in ("[INSERT", "<YOUR", "YOUR_EMAIL", "TODO:"):
                    self.assertNotIn(placeholder, text.upper())

    def test_issue_forms_route_bugs_features_questions_and_security(self) -> None:
        bug = self.read(".github/ISSUE_TEMPLATE/bug_report.yml")
        feature = self.read(".github/ISSUE_TEMPLATE/feature_request.yml")
        config = self.read(".github/ISSUE_TEMPLATE/config.yml")

        for form in (bug, feature):
            self.assertRegex(form, r"(?m)^name: .+")
            self.assertRegex(form, r"(?m)^description: .+")
            self.assertIn("required: true", form)

        self.assertIn("ig --version", bug)
        self.assertIn("Minimal reproduction", bug)
        self.assertIn("local-first operation", feature)
        self.assertIn("blank_issues_enabled: false", config)
        self.assertIn("/discussions", config)
        self.assertIn("/security/advisories/new", config)

    def test_pull_request_template_requires_validation_and_evidence(self) -> None:
        template = self.read(".github/pull_request_template.md")
        for heading in ("## Why", "## What changed", "## Validation", "## Evidence", "## Risk"):
            self.assertIn(heading, template)
        self.assertIn("./test.sh --quick", template)
        self.assertIn("before/after", template)

    def test_contributor_commands_and_local_links_are_valid(self) -> None:
        guide = self.read("CONTRIBUTING.md")
        for command in ("./test.sh --quick", "./test.sh", "./bench.sh", "pnpm -C web check"):
            self.assertIn(command, guide)
        for path in (
            "AGENTS_TESTING.md",
            "ARCHITECTURE.md",
            "CODE_OF_CONDUCT.md",
            "GOVERNANCE.md",
            "LICENSE",
            "SECURITY.md",
        ):
            self.assertIn(f"]({path})", guide)
            self.assertTrue((ROOT / path).exists())

    def test_security_and_support_use_private_reporting(self) -> None:
        security = self.read("SECURITY.md")
        support = self.read("SUPPORT.md")
        private_report = "https://github.com/bvolpato/ivygrep/security/advisories/new"
        self.assertIn(private_report, security)
        self.assertIn(private_report, support)
        self.assertIn("Do not open public issue", security)

    def test_release_metadata_is_synchronized(self) -> None:
        cargo = tomllib.loads(self.read("Cargo.toml"))
        version = cargo["package"]["version"]
        lock = self.read("Cargo.lock")
        readme = self.read("README.md")
        website = self.read("docs/index.html")
        changelog = self.read("CHANGELOG.md")

        self.assertRegex(lock, rf'name = "ivygrep"\nversion = "{re.escape(version)}"')
        self.assertIn("https://github.com/bvolpato/ivygrep/releases/latest", readme)
        self.assertIn("https://img.shields.io/github/v/release/bvolpato/ivygrep", readme)
        self.assertRegex(website, rf">\s*v{re.escape(version)}\s*<")
        self.assertRegex(changelog, rf"(?m)^## \[{re.escape(version)}\] - \d{{4}}-\d{{2}}-\d{{2}}$")


if __name__ == "__main__":
    unittest.main()
