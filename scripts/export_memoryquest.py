#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Export MemoryQuest as scoped Markdown note-retrieval datasets."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from urllib.request import urlopen

SOURCE_REVISION = "1ef78876a1785cfe800988f083d38e32d5ff0dc4"
SOURCE_REPOSITORY = "https://huggingface.co/datasets/harshitachopra/MemoryQuest"
SOURCE_TEMPLATE = (
    f"{SOURCE_REPOSITORY}/resolve/{SOURCE_REVISION}/data/user{{user_id}}.json"
)
USER_COUNT = 50
AMBIGUOUS_REFERENCES = {
    (
        "user23",
        "2026-08-05",
        "Planned mixed activities including long walks and evening events.",
    ): "s49",
    (
        "user23",
        "2026-10-03",
        "Noted chronic fatigue during weeks with overlapping deadlines.",
    ): "s65",
    (
        "user23",
        "2026-08-05",
        "Recognized financial stress as major factor affecting focus.",
    ): "s50",
    (
        "user23",
        "2026-10-03",
        "Found that fewer commitments led to better academic outcomes.",
    ): "s66",
}


def sha256_bytes(content: bytes) -> str:
    return hashlib.sha256(content).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def download_user(user_id: int) -> tuple[int, bytes]:
    with urlopen(SOURCE_TEMPLATE.format(user_id=user_id), timeout=60) as response:
        return user_id, response.read()


def load_users(source: Path | None) -> tuple[list[dict], dict[str, str]]:
    contents: dict[int, bytes] = {}
    if source:
        for user_id in range(USER_COUNT):
            path = source / f"user{user_id}.json"
            if not path.is_file():
                raise FileNotFoundError(path)
            contents[user_id] = path.read_bytes()
    else:
        with ThreadPoolExecutor(max_workers=8) as executor:
            for user_id, content in executor.map(download_user, range(USER_COUNT)):
                contents[user_id] = content
    checksums = {
        f"user{user_id}.json": sha256_bytes(contents[user_id])
        for user_id in range(USER_COUNT)
    }
    return [json.loads(contents[user_id]) for user_id in range(USER_COUNT)], checksums


def conversation_text(session: dict) -> str:
    lines = [f"Date: {session['date']}"]
    for turn in session["conversation"]:
        if len(turn) != 1:
            raise ValueError(f"invalid conversation turn in {session['id']}")
        role, text = next(iter(turn.items()))
        lines.append(f"{role.capitalize()}: {text}")
    return "\n\n".join(lines)


def resolve_reference(
    user_id: str,
    reference_date: str,
    reference_text: str,
    sessions_by_date: dict[str, list[dict]],
) -> str:
    candidates = [
        session
        for session in sessions_by_date.get(reference_date, [])
        if session["is_required"]
    ]
    if len(candidates) == 1:
        return str(candidates[0]["id"])
    override = AMBIGUOUS_REFERENCES.get((user_id, reference_date, reference_text))
    if override and any(str(session["id"]) == override for session in candidates):
        return override
    candidate_ids = ", ".join(str(session["id"]) for session in candidates) or "none"
    raise ValueError(
        f"cannot resolve {user_id} reference {reference_date!r} "
        f"{reference_text!r}; candidates: {candidate_ids}"
    )


def export_records(
    users: list[dict],
    query_limit: int | None = None,
) -> tuple[list[dict], list[dict], list[tuple[str, str, int]], dict]:
    corpus: list[dict] = []
    queries: list[dict] = []
    qrels: list[tuple[str, str, int]] = []
    required_references = 0
    resolved_references = 0

    for user in users:
        user_id = str(user["demographics"]["user_id"])
        sessions_by_date: dict[str, list[dict]] = {}
        session_paths_by_date: list[tuple[str, str]] = []
        for session in user["sessions"]:
            sessions_by_date.setdefault(str(session["date"]), []).append(session)
            session_id = str(session["id"])
            session_path = f"users/{user_id}/{session_id}.md"
            session_paths_by_date.append((str(session["date"]), session_path))
            corpus.append(
                {
                    "_id": f"{user_id}:{session_id}",
                    "title": f"Memory from {session['date']}",
                    "text": conversation_text(session),
                    "metadata": {
                        "path": session_path,
                        "user": user_id,
                        "session": session_id,
                        "date": session["date"],
                    },
                }
            )
        for position, query in enumerate(user["queries"]):
            query_id = f"{user_id}:q{position:03d}"
            relevant = []
            for reference_date, reference_text in query["needed_references"]:
                required_references += 1
                session_id = resolve_reference(
                    user_id,
                    str(reference_date),
                    str(reference_text),
                    sessions_by_date,
                )
                resolved_references += 1
                relevant.append(f"{user_id}:{session_id}")
            queries.append(
                {
                    "_id": query_id,
                    "text": str(query["query"]),
                    "metadata": {
                        "scope": f"users/{user_id}",
                        "user": user_id,
                        "date": query["date"],
                        "exclude_globs": [
                            path
                            for session_date, path in session_paths_by_date
                            if session_date > str(query["date"])
                        ],
                    },
                }
            )
            qrels.extend(
                (query_id, document_id, 1) for document_id in dict.fromkeys(relevant)
            )
            if query_limit is not None and len(queries) >= query_limit:
                break
        if query_limit is not None and len(queries) >= query_limit:
            break

    counts = {
        "users": len(users),
        "documents": len(corpus),
        "queries": len(queries),
        "required_references": required_references,
        "resolved_references": resolved_references,
        "qrels": len(qrels),
    }
    return corpus, queries, qrels, counts


def write_jsonl(path: Path, records: list[dict]) -> None:
    with path.open("w", encoding="utf-8") as handle:
        for record in records:
            handle.write(json.dumps(record, ensure_ascii=False, sort_keys=True) + "\n")


def validate_replaceable_output(output: Path) -> None:
    if not output.exists():
        return
    if not output.is_dir():
        raise ValueError(f"output exists and is not a directory: {output}")
    provenance_path = output / "provenance.json"
    try:
        provenance = json.loads(provenance_path.read_text(encoding="utf-8"))
    except (FileNotFoundError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError(
            f"refusing to replace unrecognized output directory: {output}"
        ) from error
    if provenance.get("dataset") != "MemoryQuest":
        raise ValueError(f"refusing to replace non-MemoryQuest output: {output}")


def export_dataset(
    output: Path,
    source: Path | None = None,
    query_limit: int | None = None,
) -> dict:
    users, source_checksums = load_users(source)
    corpus, queries, qrels, counts = export_records(users, query_limit)
    if not queries:
        raise ValueError("export produced no queries")

    validate_replaceable_output(output)
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = Path(tempfile.mkdtemp(prefix=f".{output.name}-", dir=output.parent))
    try:
        write_jsonl(temporary / "corpus.jsonl", corpus)
        write_jsonl(temporary / "queries.jsonl", queries)
        (temporary / "qrels.tsv").write_text(
            "query-id\tcorpus-id\tscore\n"
            + "".join(
                f"{query_id}\t{document_id}\t{score}\n"
                for query_id, document_id, score in qrels
            ),
            encoding="utf-8",
        )
        checksums = {
            name: sha256_file(temporary / name)
            for name in ("corpus.jsonl", "queries.jsonl", "qrels.tsv")
        }
        provenance = {
            "schema_version": 1,
            "dataset": "MemoryQuest",
            "dataset_description": (
                "Synthetic multi-session personal-assistant conversations"
            ),
            "source_repository": SOURCE_REPOSITORY,
            "source_revision": SOURCE_REVISION,
            "source_files_sha256": source_checksums,
            "license": "CC BY 4.0",
            "license_source": f"{SOURCE_REPOSITORY}/blob/{SOURCE_REVISION}/README.md",
            "paper": (
                "https://www.microsoft.com/en-us/research/publication/"
                "thinking-ahead-prospection-guided-retrieval-of-memory-with-language-models/"
            ),
            "index_contents": (
                "session date plus raw user and assistant turns; construction metadata excluded"
            ),
            "scope": (
                "queries search only their user's sessions at or before query date"
            ),
            "counts": counts,
            "checksums": checksums,
            "query_limit": query_limit,
        }
        (temporary / "provenance.json").write_text(
            json.dumps(provenance, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        if output.exists():
            shutil.rmtree(output)
        temporary.rename(output)
    except BaseException:
        shutil.rmtree(temporary, ignore_errors=True)
        raise
    return provenance


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--source", type=Path)
    parser.add_argument("--limit-queries", type=int)
    args = parser.parse_args()
    if args.limit_queries is not None and args.limit_queries < 1:
        raise ValueError("--limit-queries must be positive")
    provenance = export_dataset(args.output, args.source, args.limit_queries)
    print(json.dumps(provenance, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
