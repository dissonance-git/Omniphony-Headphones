#!/usr/bin/env python3
"""Emit a disposable GitHub reasoning-agent bootstrap from current Omniphony repository truth."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import subprocess


SCHEMA_VERSION = "omniphony-github-agent-bootstrap-001.0"
ROOT = Path(__file__).resolve().parents[1]
SKILLS_ROOT = ROOT / ".agents" / "skills"


def git_value(*args: str) -> str:
    completed = subprocess.run(
        ["git", *args],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return completed.stdout.strip()


def live_skills() -> list[dict[str, str]]:
    skills: list[dict[str, str]] = []
    for path in sorted(SKILLS_ROOT.glob("*/SKILL.md")):
        text = path.read_text(encoding="utf-8")
        name = path.parent.name
        declared = next(
            (line.split(":", 1)[1].strip() for line in text.splitlines() if line.startswith("name: ")),
            "",
        )
        if declared != name:
            raise RuntimeError(
                f"skill name/path mismatch: {path.relative_to(ROOT)} -> {declared!r}"
            )

        description = ""
        lines = text.splitlines()
        for index, line in enumerate(lines):
            if not line.startswith("description:"):
                continue
            tail = line.split(":", 1)[1].strip()
            if tail and tail != ">":
                description = tail
            elif tail == ">":
                parts: list[str] = []
                for follow in lines[index + 1 :]:
                    if not follow.startswith(" "):
                        break
                    parts.append(follow.strip())
                description = " ".join(parts)
            break

        skills.append(
            {
                "name": name,
                "path": path.relative_to(ROOT).as_posix(),
                "sha256": hashlib.sha256(text.encode("utf-8")).hexdigest(),
                "description": description,
            }
        )
    return skills


def catalog_fingerprint(skills: list[dict[str, str]]) -> str:
    payload = json.dumps(
        [{"name": item["name"], "path": item["path"], "sha256": item["sha256"]} for item in skills],
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    return hashlib.sha256(payload).hexdigest()


def build_bootstrap() -> dict[str, object]:
    skills = live_skills()
    return {
        "schema_version": SCHEMA_VERSION,
        "system": "Omniphony",
        "transport": "github-connector",
        "semantic_mode": "reasoning-agent-workspace",
        "head_sha": git_value("rev-parse", "HEAD"),
        "tree_sha": git_value("rev-parse", "HEAD^{tree}"),
        "authority_path": "AGENTS.md",
        "identity_path": "README.md",
        "roadmap_path": "ROADMAP.md",
        "skill_preflight_path": ".agents/skills/skill-preflight/SKILL.md",
        "skill_catalog": {
            "count": len(skills),
            "sha256": catalog_fingerprint(skills),
            "canonical_truth": ".agents/skills/*/SKILL.md",
            "skills": skills,
        },
        "bootstrap_sequence": [
            "identify-omniphony",
            "freeze-github-head",
            "read-agents",
            "read-smallest-relevant-readme-surface",
            "enumerate-live-skills",
            "run-skill-preflight",
            "state-exact-obligation",
            "assess-github-capabilities",
            "acquire-smallest-sufficient-context",
            "act",
            "verify",
        ],
        "canonical_truth": False,
        "writable_state": False,
        "note": (
            "Disposable projection only. Current AGENTS.md, README.md, live skill bytes, "
            "exact Git state, code/tests, and living contracts outrank this packet."
        ),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    packet = build_bootstrap()
    if args.json:
        print(json.dumps(packet, indent=2, sort_keys=True))
    else:
        catalog = packet["skill_catalog"]
        print(
            "Omniphony GitHub agent bootstrap\n"
            f"HEAD: {packet['head_sha']}\n"
            f"skills: {catalog['count']}\n"
            f"skill catalog sha256: {catalog['sha256']}\n"
            "next: read AGENTS.md, run live skill preflight, then state the exact obligation"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
