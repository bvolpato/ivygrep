#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Record the homepage demo from real `ig` and `rg` output.

The homepage shows recorded output, never hand-written results. Run this script
against a checkout of the demo repository after its index is complete:

    git clone --depth 1 --branch 16.0.0 https://github.com/BurntSushi/ripgrep
    export IVYGREP_HOME="$(mktemp -d)"
    ig --add ripgrep --wait-for-enhancement
    uv run scripts/record_site_demo.py --ig "$(command -v ig)" --repo ripgrep

The script replaces the two marked blocks in `docs/index.html`.
"""

import argparse
import html
import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

# Each question is paired with the keyword a reader would probably grep first.
QUESTIONS = [
    ("which thread count does it pick by default", "thread"),
    ("ignore rules from the global git config", "ignore"),
    ("how does it know a file is binary", "binary"),
    ("what happens on a broken pipe", "pipe"),
    ("parse human readable sizes like 10M", "size"),
    ("walk directories in parallel with work stealing", "parallel"),
]
RESULT_FILES = 3
PREVIEW_CONTEXT = 4

DEFAULT_BUDGET = 2000
TASK = "expand a tilde in the global core.excludesFile path"
BUDGETS = [500, 1000, 2000, 4000, 8000]


def run(command: list[str], cwd: Path) -> str:
    return subprocess.run(
        command,
        cwd=cwd,
        check=True,
        capture_output=True,
        text=True,
        stdin=subprocess.DEVNULL,
    ).stdout


def keyword_count(keyword: str, repo: Path) -> dict[str, int]:
    rows = run(["rg", "-i", "-c", keyword, "."], repo).splitlines()
    return {
        "lines": sum(int(row.rsplit(":", 1)[1]) for row in rows),
        "files": len(rows),
    }


def search(ig: str, question: str, repo: Path) -> list[dict]:
    command = [ig, "--json", "-n", str(RESULT_FILES), "-C", str(PREVIEW_CONTEXT), question]
    results = []
    for entry in json.loads(run(command, repo)):
        hit = entry["hits"][0]
        results.append(
            {
                "path": hit["file_path"],
                "start": hit["start_line"],
                "lines": hit["preview"].split("\n"),
            }
        )
    return results


def pack(ig: str, budget: int, repo: Path) -> dict:
    return json.loads(
        run([ig, "context", TASK, "--budget", str(budget), "--json"], repo)
    )


def esc(text: str) -> str:
    return html.escape(text, quote=False)


def render_source(result: dict) -> str:
    rows = []
    for offset, line in enumerate(result["lines"]):
        classes = []
        if line.lstrip().startswith("//"):
            classes.append("c")
        # ig centers the preview on the matched line.
        row = ' class="m"' if offset == min(PREVIEW_CONTEXT, result["start"] - 1) else ""
        code = f' class="{" ".join(classes)}"' if classes else ""
        rows.append(
            f"<span{row}><i>{result['start'] + offset}</i>"
            f"<code{code}>{esc(line) or ' '}</code></span>"
        )
    return f'<pre class="src">{"".join(rows)}</pre>'


def render_searches(data: dict) -> str:
    tabs, panels = [], []
    for index, entry in enumerate(data["searches"]):
        chosen = index == 0
        question = esc(entry["question"])
        tabs.append(
            f'<button role="tab" id="q{index}" aria-controls="a{index}" '
            f'aria-selected="{str(chosen).lower()}"'
            f'{"" if chosen else " tabindex=\"-1\""}>{question}</button>'
        )
        first, *rest = entry["results"]
        others = "".join(
            f"<li>then {esc(hit['path'])}:{hit['start'] + PREVIEW_CONTEXT}</li>" for hit in rest
        )
        grep = entry["grep"]
        panels.append(
            f'<div role="tabpanel" id="a{index}" aria-labelledby="q{index}"'
            f'{"" if chosen else " hidden"}>\n'
            f'<p class="ask mono"><b>$</b> ig "<q>{question}</q>"</p>\n'
            f'<div class="hit"><p class="where mono">{esc(first["path"])}'
            "<span>top result</span></p>"
            f"{render_source(first)}</div>\n"
            f'<div class="more mono"><ul>{others}</ul></div>\n'
            f'<p class="versus"><code>rg -i {esc(entry["keyword"])}</code> in the same tree: '
            f"<b>{grep['lines']:,} lines in {grep['files']:,} files</b></p>\n"
            "</div>"
        )
    return (
        '<div class="console">\n'
        '<div class="questions"><p>We asked ripgrep’s own source code:</p>'
        f'<div role="tablist" aria-label="Questions" aria-orientation="vertical">{"".join(tabs)}</div></div>\n'
        f'<div class="answer">{"".join(panels)}</div>\n'
        "</div>\n"
        f'<p class="caption">Recorded, unedited output from ig {data["ig"]} on '
        f'<a href="https://github.com/{data["repo"]}/tree/{data["commit"]}">{data["repo"]} {esc(data["tag"])}</a>. '
        "The keyword line is what ripgrep finds for the word you would probably try first. "
        "Use each tool for what it is good at.</p>"
    )


LEAF = (
    '<svg viewBox="0 0 30 24" aria-hidden="true">'
    '<path d="M1 23C2 8 13 1 29 2 28 16 17 24 1 23Z"/></svg>'
)


def render_items(current: dict, previous: set[tuple]) -> str:
    rows = []
    for item in current["items"]:
        key = (item["file_path"], item["roles"][0], item["start_line"] // 200)
        classes = ["primary"] if item["roles"][0] == "primary" else []
        if key not in previous:
            classes.append("new")
        span = f"{item['start_line']}-{item['end_line']}"
        rows.append(
            f'<li class="{" ".join(classes)}">{LEAF}'
            f'<div><div class="at mono">{esc(item["file_path"])}<span>:{span}</span></div>'
            f'<div class="role">{esc(", ".join(item["roles"]))}</div></div>'
            f'<div class="why">{esc(item["reasons"][0])}</div>'
            f'<span class="tok">{item["estimated_tokens"]} tokens</span></li>'
        )
    return "".join(rows)


def render_packs(data: dict) -> str:
    previous: set[tuple] = set()
    packs = []
    for current in data["packs"]:
        budget = current["budget_tokens"]
        meter = "".join(
            f'<span class="{item["roles"][0]}" style="width:{100 * item["estimated_tokens"] / budget:.2f}%"></span>'
            for item in current["items"]
        )
        hidden = "" if budget == DEFAULT_BUDGET else " hidden"
        files = len({item["file_path"] for item in current["items"]})
        packs.append(
            f'<div data-budget="{budget}"{hidden}>\n'
            f'<div class="meter" aria-hidden="true">{meter}</div>\n'
            f'<p class="pack-sum">{current["used_tokens"]:,} of {budget:,} estimated tokens. '
            f"{len(current['items'])} items from {files} files.</p>\n"
            f'<ul class="vine">{render_items(current, previous)}</ul>\n</div>'
        )
        previous = {
            (item["file_path"], item["roles"][0], item["start_line"] // 200)
            for item in current["items"]
        }
    ticks = "".join(f"<span>{budget:,}</span>" for budget in BUDGETS)
    return (
        '<div class="pack-controls">\n'
        f'<div class="cmd"><pre>ig context "{esc(TASK)}" '
        f'<span class="nb">--budget <span id="budget-command">{DEFAULT_BUDGET}</span></span></pre><button class="copy">Copy</button></div>\n'
        '<div class="slider"><label for="budget">Token budget '
        f'<output id="budget-value" for="budget">{DEFAULT_BUDGET:,} tokens</output></label>'
        f'<input id="budget" type="range" min="0" max="{len(BUDGETS) - 1}" step="1" '
        f'value="{BUDGETS.index(DEFAULT_BUDGET)}">'
        f'<div class="ticks" aria-hidden="true">{ticks}</div></div>\n</div>\n'
        + "\n".join(packs)
        + f'\n<p class="caption">Recorded with ig {data["ig"]} on the same ripgrep source. '
        "Rose marks primary evidence. Green marks code reached through a relationship. "
        "The budget covers the rendered pack, so headings count too.</p>"
    )


def replace_block(page: str, name: str, body: str) -> str:
    pattern = re.compile(
        rf"(<!-- demo:{name}:start -->).*?(<!-- demo:{name}:end -->)", re.DOTALL
    )
    if len(pattern.findall(page)) != 1:
        raise SystemExit(f"docs/index.html must contain one demo:{name} block")
    return pattern.sub(lambda match: f"{match[1]}\n{body}\n{match[2]}", page)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ig", required=True)
    parser.add_argument("--repo", required=True, type=Path)
    parser.add_argument("--page", type=Path, default=ROOT / "docs" / "index.html")
    args = parser.parse_args()

    repo = args.repo.resolve()
    data = {
        "ig": run([args.ig, "--version"], repo).split()[1],
        "repo": "BurntSushi/ripgrep",
        "tag": run(["git", "describe", "--tags", "--always"], repo).strip(),
        "commit": run(["git", "rev-parse", "HEAD"], repo).strip(),
        "searches": [
            {
                "question": question,
                "keyword": keyword,
                "grep": keyword_count(keyword, repo),
                "results": search(args.ig, question, repo),
            }
            for question, keyword in QUESTIONS
        ],
        "packs": [pack(args.ig, budget, repo) for budget in BUDGETS],
    }
    page = args.page.read_text(encoding="utf-8")
    page = replace_block(page, "searches", render_searches(data))
    page = replace_block(page, "packs", render_packs(data))
    args.page.write_text(page, encoding="utf-8")
    print(f"updated {args.page}")


if __name__ == "__main__":
    main()
