import re
import subprocess
import textwrap
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
WORKFLOWS = ROOT / ".github" / "workflows"


def workflow_text(name: str) -> str:
    return (WORKFLOWS / name).read_text(encoding="utf-8")


def job_block(workflow: str, job: str) -> str:
    lines = workflow.splitlines(keepends=True)
    marker = f"  {job}:\n"
    start = lines.index(marker) + 1
    end = start
    while end < len(lines):
        line = lines[end]
        if line.startswith("  ") and not line.startswith("    ") and line.strip():
            break
        end += 1
    return "".join(lines[start:end])


def run_scripts(workflow: str) -> dict[str, str]:
    """Extract multiline Bash steps without depending on a YAML package."""
    lines = workflow.splitlines(keepends=True)
    scripts: dict[str, str] = {}
    step_indices = [
        index
        for index, line in enumerate(lines)
        if re.match(r"^      - name: .+\n$", line)
    ]
    for step_number, start in enumerate(step_indices):
        end = step_indices[step_number + 1] if step_number + 1 < len(step_indices) else len(lines)
        match = re.match(r"^      - name: (.+)\n$", lines[start])
        assert match is not None
        run_line = next(
            (index for index in range(start + 1, end) if lines[index] == "        run: |\n"),
            None,
        )
        if run_line is None:
            continue
        name = match.group(1)
        index = run_line + 1
        body: list[str] = []
        while index < end:
            line = lines[index]
            body.append(line)
            index += 1
        scripts[name] = textwrap.dedent("".join(body)).strip() + "\n"
    return scripts


class WorkflowHardeningTest(unittest.TestCase):
    def test_dashboard_validation_fetches_publication_history(self) -> None:
        for workflow_name, job_name in (
            ("ci.yml", "check"),
            ("public-retrieval.yml", "validate"),
        ):
            job = job_block(workflow_text(workflow_name), job_name)
            self.assertIn("fetch-depth: 0", job, workflow_name)
            self.assertIn("scripts/render_evidence_dashboard.py", job, workflow_name)

    def test_benchmark_publication_permissions_are_split(self) -> None:
        workflow = workflow_text("benchmarks.yml")
        top_level = workflow.split("jobs:\n", maxsplit=1)[0]
        benchmark = job_block(workflow, "benchmark")
        publisher = job_block(workflow, "publish-history")

        self.assertRegex(top_level, r"permissions:\n  contents: read\n")
        self.assertNotRegex(benchmark, r"contents: write|pull-requests: write|issues: write")
        self.assertRegex(publisher, r"permissions:\n      actions: read\n      contents: write\n")
        self.assertIn("needs: benchmark", publisher)
        self.assertIn("needs.benchmark.outputs.required == 'true'", publisher)
        self.assertEqual(workflow.count("name: benchmark-history-input"), 2)
        self.assertIn("cargo install cargo-criterion --version 1.1.0 --locked", workflow)

    def test_crates_publish_requires_exact_release_identity(self) -> None:
        workflow = workflow_text("publish-crates.yml")
        release_job = job_block(workflow, "publish")
        self.assertRegex(
            workflow,
            r"release_tag:\n\s+description: .*\n\s+required: true\n\s+type: string",
        )
        self.assertIn("ref: ${{ inputs.release_tag }}", release_job)
        self.assertIn("fetch-depth: 0", release_job)

        release_script = run_scripts(workflow)["Validate release identity and package manifests"]
        for contract in (
            "refs/heads/main",
            "refs/tags/$RELEASE_TAG",
            "refs/tags/$RELEASE_TAG^{commit}",
            '"$tag_commit" != "$head_commit"',
            '"$tag_commit" != "$WORKFLOW_SHA"',
            'main_version="$(package_version Cargo.toml ivygrep)"',
            'echo "version=$main_version" >> "$GITHUB_OUTPUT"',
        ):
            self.assertIn(contract, release_script)

        publish_script = run_scripts(workflow)["Publish packages in dependency order"]
        for contract in (
            "cargo package --locked --no-verify",
            'artifact="$CARGO_TARGET_DIR/package/${package}-${version}.crate"',
            'artifact_root="$(tar -tzf "$artifact"',
            "check_remote_artifact",
            "sha256sum",
            "cargo publish --locked --manifest-path",
        ):
            self.assertIn(contract, publish_script)

    def test_hardened_workflow_scripts_have_valid_bash_syntax(self) -> None:
        for name in ("benchmarks.yml", "publish-crates.yml", "security.yml"):
            scripts = run_scripts(workflow_text(name))
            self.assertTrue(scripts, name)
            for step, script in scripts.items():
                result = subprocess.run(
                    ["bash", "-n"],
                    input=script,
                    text=True,
                    capture_output=True,
                    check=False,
                )
                self.assertEqual(
                    result.returncode,
                    0,
                    f"{name} step {step!r} has invalid Bash syntax:\n{result.stderr}",
                )


if __name__ == "__main__":
    unittest.main()
