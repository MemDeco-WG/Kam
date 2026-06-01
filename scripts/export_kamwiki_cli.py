#!/usr/bin/env python3
"""Export the current Kam CLI help surface into KamWiki data files."""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
KAMWIKI = ROOT / "KamWiki"
GENERATED_JSON = KAMWIKI / "src" / "data" / "generated-cli.json"
CLI_HELP_MD = KAMWIKI / "docs" / "cli-help.md"
KAM_BIN = ROOT / "target" / "debug" / "kam"

ANSI_RE = re.compile(r"\x1b\[[0-9;]*m")
COMMAND_RE = re.compile(r"^\s{2,}([a-z][a-z0-9_-]*)\s{2,}(.+?)\s*$")
ITEM_RE = re.compile(r"^\s{2,}((?:-\S|\[[A-Z_][A-Z0-9_]*(?:\.\.\.)?\]).*?)(?:\s{2,}(.+?))?\s*$")


def ensure_kam_binary() -> None:
    proc = subprocess.run(
        ["cargo", "build", "--quiet"],
        cwd=ROOT,
        text=True,
        check=False,
    )
    if proc.returncode != 0:
        raise SystemExit(f"cargo build failed with exit code {proc.returncode}")


def run_kam(args: list[str], locale: str) -> str:
    env = os.environ.copy()
    env["KAM_UI_LANGUAGE"] = locale
    env.setdefault("CARGO_TERM_COLOR", "never")
    proc = subprocess.run(
        [str(KAM_BIN), *args],
        cwd=ROOT,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if proc.returncode != 0:
        sys.stderr.write(proc.stderr)
        raise SystemExit(f"kam {' '.join(args)} failed with exit code {proc.returncode}")
    return ANSI_RE.sub("", proc.stdout).strip() + "\n"


def run_version() -> str:
    output = run_kam(["--version"], "en")
    return output.strip()


def parse_usage(help_text: str) -> str:
    for line in help_text.splitlines():
        if line.startswith("Usage:"):
            return line.removeprefix("Usage:").strip()
    return ""


def parse_summary(help_text: str) -> str:
    for line in help_text.splitlines():
        stripped = line.strip()
        if not stripped:
            continue
        if stripped.startswith("Usage:"):
            return ""
        if " — " in stripped:
            return stripped.split(" — ", 1)[1].strip()
        return stripped
    return ""


def section_lines(help_text: str, heading: str) -> list[str]:
    lines = help_text.splitlines()
    start = None
    for idx, line in enumerate(lines):
        if line.strip() == heading:
            start = idx + 1
            break
    if start is None:
        return []

    out: list[str] = []
    for line in lines[start:]:
        stripped = line.strip()
        if stripped.endswith(":") and not line.startswith(" "):
            break
        out.append(line)
    return out


def parse_commands(help_text: str) -> list[dict[str, Any]]:
    commands: list[dict[str, Any]] = []
    for line in section_lines(help_text, "Commands:"):
        match = COMMAND_RE.match(line)
        if not match:
            continue
        name, summary = match.groups()
        aliases: list[str] = []
        alias_match = re.search(r"\s+\[aliases:\s*([^\]]+)\]\s*$", summary)
        if alias_match:
            aliases = [item.strip() for item in alias_match.group(1).split(",")]
            summary = summary[: alias_match.start()].rstrip()
        commands.append({"name": name, "summary": summary, "aliases": aliases})
    return commands


def parse_item_section(help_text: str, heading: str) -> list[dict[str, str]]:
    items: list[dict[str, str]] = []
    current: dict[str, str] | None = None
    description_parts: list[str] = []

    def finish_current() -> None:
        nonlocal current, description_parts
        if current is None:
            return
        description = " ".join(part.strip() for part in description_parts if part.strip())
        if description:
            current["description"] = description
        items.append(current)
        current = None
        description_parts = []

    for line in section_lines(help_text, heading):
        stripped = line.strip()
        if not stripped:
            continue

        item_match = ITEM_RE.match(line)
        if item_match:
            finish_current()
            current = {"flag": item_match.group(1).strip()}
            if item_match.group(2):
                description_parts.append(item_match.group(2))
            continue

        if current is not None:
            description_parts.append(stripped)

    finish_current()
    return items


def parse_flags(help_text: str) -> list[dict[str, str]]:
    return parse_item_section(help_text, "Arguments:") + parse_item_section(help_text, "Options:")


def command_doc(name: str, help_by_locale: dict[str, str]) -> dict[str, Any]:
    en_help = help_by_locale["en"]
    zh_help = help_by_locale["zh"]
    usage = parse_usage(en_help)
    command = {
        "name": name,
        "summary": parse_summary(en_help),
        "usage": usage,
        "description": parse_summary(en_help),
        "flags": parse_flags(en_help),
        "examples": [usage] if usage else [],
        "localized": {
            "en": {
                "summary": parse_summary(en_help),
                "description": parse_summary(en_help),
                "flags": parse_flags(en_help),
            },
            "zh": {
                "summary": parse_summary(zh_help),
                "description": parse_summary(zh_help),
                "flags": parse_flags(zh_help),
            },
        },
    }
    return command


def export_data() -> dict[str, Any]:
    top_help = {
        "en": run_kam(["--help"], "en"),
        "zh": run_kam(["--help"], "zh"),
    }
    top_commands = parse_commands(top_help["en"])
    zh_summaries = {item["name"]: item["summary"] for item in parse_commands(top_help["zh"])}

    commands: list[dict[str, Any]] = []
    raw_help: dict[str, dict[str, str]] = {"en": {"kam": top_help["en"]}, "zh": {"kam": top_help["zh"]}}

    for item in top_commands:
        name = item["name"]
        help_by_locale = {
            "en": run_kam([name, "--help"], "en"),
            "zh": run_kam([name, "--help"], "zh"),
        }
        raw_help["en"][name] = help_by_locale["en"]
        raw_help["zh"][name] = help_by_locale["zh"]
        doc = command_doc(name, help_by_locale)
        doc["aliases"] = item["aliases"]
        if item["summary"]:
            doc["summary"] = item["summary"]
            doc["localized"]["en"]["summary"] = item["summary"]
        if zh_summaries.get(name):
            doc["localized"]["zh"]["summary"] = zh_summaries[name]
        commands.append(doc)

    return {
        "schemaVersion": 1,
        "source": "cargo build --quiet, then target/debug/kam <command> --help",
        "kamVersion": run_version(),
        "globalFlags": parse_flags(top_help["en"]),
        "localizedGlobalFlags": {
            "en": parse_flags(top_help["en"]),
            "zh": parse_flags(top_help["zh"]),
        },
        "commands": commands,
        "rawHelp": raw_help,
    }


def write_markdown(data: dict[str, Any]) -> None:
    lines = [
        "# kam CLI Help Reference",
        "",
        "This file is generated from the current Kam CLI help output.",
        "",
        f"- Source: `{data['source']}`",
        f"- Version: `{data['kamVersion']}`",
        "",
        "## Top Level",
        "",
        "```text",
        data["rawHelp"]["en"]["kam"].rstrip(),
        "```",
    ]

    for command in data["commands"]:
        name = command["name"]
        lines.extend(
            [
                "",
                f"## `{name}`",
                "",
                "```text",
                data["rawHelp"]["en"][name].rstrip(),
                "```",
            ]
        )

    CLI_HELP_MD.parent.mkdir(parents=True, exist_ok=True)
    CLI_HELP_MD.write_text("\n".join(lines) + "\n", encoding="utf-8")


def write_json(data: dict[str, Any]) -> None:
    GENERATED_JSON.parent.mkdir(parents=True, exist_ok=True)
    GENERATED_JSON.write_text(
        json.dumps(data, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def check_clean(paths: list[Path]) -> None:
    rel_paths = [str(path.relative_to(ROOT)) for path in paths]
    proc = subprocess.run(
        ["git", "diff", "--exit-code", "--", *rel_paths],
        cwd=ROOT,
        text=True,
        check=False,
    )
    if proc.returncode != 0:
        raise SystemExit("KamWiki CLI data is out of date. Re-run scripts/export_kamwiki_cli.py.")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="fail if generated files differ")
    args = parser.parse_args()

    ensure_kam_binary()
    data = export_data()
    write_json(data)
    write_markdown(data)
    if args.check:
        check_clean([GENERATED_JSON, CLI_HELP_MD])


if __name__ == "__main__":
    main()
