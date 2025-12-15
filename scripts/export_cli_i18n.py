# /// script
# requires-python = ">=3.12"
# dependencies = [
#     "toml>=0.10.2",
# ]
# ///

"""
export_cli_i18n.py

Export CLI i18n TOML (src/i18n/{en,zh}.toml) into WEB UI JSON files
(KamWEBUI/src/data/cli/en.json and .../zh.json).

This script:
 - Loads `cli` section from each TOML locale file
 - Recursively maps `cli.commands.*` into a JSON structure:
     { "flags": {...}, "commands": { "<cmd>": { summary, description, flags, subcommands: {...} } } }
 - Writes stable, pretty-printed JSON files into the WEB UI data folder
 - Optionally reports missing keys between locales and can exit non-zero if asked

Usage:
  python3 scripts/export_cli_i18n.py
  python3 scripts/export_cli_i18n.py --fail-on-missing

Notes:
 - Prefer Python 3.11+ (uses stdlib tomllib). If not available, the script will
   try to use the third-party `toml` package (pip install toml).
 - Run from repository (no special CWD required) - script locates files relative
   to its location.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any, Dict, Tuple


def parse_toml_string(s: str) -> Dict[str, Any]:
    """
    Parse TOML using stdlib tomllib (py3.11+) if available,
    otherwise fall back to third-party 'toml' package.
    """
    try:
        import tomllib  # py3.11+

        return tomllib.loads(s)
    except Exception:
        try:
            import toml as toml_pkg
        except Exception:
            print(
                "ERROR: Could not find a TOML parser. Use Python 3.11+ (tomllib) "
                "or install 'toml' (pip install toml).",
                file=sys.stderr,
            )
            sys.exit(2)
        # third-party toml package matches loads signature
        return toml_pkg.loads(s)


def read_toml_file(p: Path) -> Dict[str, Any]:
    if not p.exists():
        raise FileNotFoundError(f"TOML file not found: {p}")
    txt = p.read_text(encoding="utf-8")
    return parse_toml_string(txt)


def process_command_node(node: Dict[str, Any]) -> Dict[str, Any]:
    """
    Convert a nested TOML table (for a command/subcommand) into our JSON-friendly format.

    Result keys:
      - summary (if present)
      - description (if present)
      - flags (mapping of flag_id -> string) (if present)
      - subcommands (mapping of subcommand_name -> processed node) (if any)
    """
    out: Dict[str, Any] = {}

    # Basic fields
    if isinstance(node.get("summary"), str):
        out["summary"] = node["summary"]
    if isinstance(node.get("description"), str):
        out["description"] = node["description"]

    # Flags: expect a table mapping flag id -> description
    if isinstance(node.get("flags"), dict):
        flags = {}
        for k, v in node["flags"].items():
            # Robustly coerce to string
            flags[k] = str(v) if v is not None else ""
        out["flags"] = flags

    # Recurse into other dicts -> treat as subcommands
    subcommands = {}
    for k, v in node.items():
        if k in ("summary", "description", "flags"):
            continue
        if isinstance(v, dict):
            # If this dict is just a 'flags' table (already handled), skip; otherwise recurse
            processed = process_command_node(v)
            if processed:
                subcommands[k] = processed

    if subcommands:
        out["subcommands"] = subcommands

    return out


def extract_cli_from_toml(parsed: Dict[str, Any]) -> Dict[str, Any]:
    """
    Extract the `cli` subtree from a parsed TOML file and convert to JSON structure.
    """
    cli = parsed.get("cli", {})
    out: Dict[str, Any] = {}

    # Top-level 'about' / 'long_about' (optional)
    about = {}
    if isinstance(cli.get("about"), str):
        about["about"] = cli["about"]
    if isinstance(cli.get("long_about"), str):
        about["long_about"] = cli["long_about"]
    if about:
        out["about"] = about

    # Top-level flags (global flags)
    if isinstance(cli.get("flags"), dict):
        flags = {k: str(v) for k, v in cli["flags"].items()}
        out["flags"] = flags
    else:
        out["flags"] = {}

    # Commands and nested subcommands
    commands = {}
    cli_commands = cli.get("commands", {})
    if isinstance(cli_commands, dict):
        for cmd_name, cmd_node in cli_commands.items():
            if isinstance(cmd_node, dict):
                commands[cmd_name] = process_command_node(cmd_node)
    out["commands"] = commands

    return out


def write_json_file(data: Dict[str, Any], out_path: Path) -> None:
    out_path.parent.mkdir(parents=True, exist_ok=True)
    # Stable output for easier diffs
    txt = json.dumps(data, ensure_ascii=False, indent=2, sort_keys=True)
    out_path.write_text(txt, encoding="utf-8")


def compare_locales(en: Dict[str, Any], zh: Dict[str, Any]) -> Tuple[int, int]:
    """
    Compare top-level command & flag keys between en and zh.
    Returns (missing_in_zh_count, missing_in_en_count)
    """
    missing_in_zh = 0
    missing_in_en = 0

    en_cmds = set(en.get("commands", {}).keys())
    zh_cmds = set(zh.get("commands", {}).keys())

    for c in sorted(en_cmds - zh_cmds):
        print(f"WARNING: command '{c}' present in en but missing in zh")
        missing_in_zh += 1
    for c in sorted(zh_cmds - en_cmds):
        print(f"INFO: command '{c}' present in zh but missing in en")
        missing_in_en += 1

    # Flags
    en_flags = set(en.get("flags", {}).keys())
    zh_flags = set(zh.get("flags", {}).keys())
    for f in sorted(en_flags - zh_flags):
        print(f"WARNING: global flag '{f}' present in en but missing in zh")
        missing_in_zh += 1
    for f in sorted(zh_flags - en_flags):
        print(f"INFO: global flag '{f}' present in zh but missing in en")
        missing_in_en += 1

    # Per-command flags (shallow check)
    common_cmds = en_cmds & zh_cmds
    for c in sorted(common_cmds):
        eflags = set(en["commands"].get(c, {}).get("flags", {}).keys())
        zflags = set(zh["commands"].get(c, {}).get("flags", {}).keys())
        for f in sorted(eflags - zflags):
            print(
                f"WARNING: flag '{f}' for command '{c}' present in en but missing in zh"
            )
            missing_in_zh += 1
        for f in sorted(zflags - eflags):
            print(f"INFO: flag '{f}' for command '{c}' present in zh but missing in en")
            missing_in_en += 1

    return missing_in_zh, missing_in_en


def main() -> int:
    p = Path(__file__).resolve()
    # repo root is parent of 'scripts' dir (i.e., one up)
    kam_root = p.parents[1]

    parser = argparse.ArgumentParser(
        description="Export CLI i18n TOML to WEBUI JSON (en/zh)"
    )
    parser.add_argument(
        "--src",
        type=Path,
        default=kam_root / "src" / "i18n",
        help="Source i18n directory (contains en.toml, zh.toml)",
    )
    parser.add_argument(
        "--dest",
        type=Path,
        default=kam_root / "KamWEBUI" / "src" / "data" / "cli",
        help="Destination folder inside WEB UI (will write en.json / zh.json)",
    )
    parser.add_argument(
        "--locales",
        type=str,
        default="en,zh",
        help="Comma-separated locales to export (default: en,zh)",
    )
    parser.add_argument(
        "--fail-on-missing",
        action="store_true",
        help="Exit with non-zero status if there are missing translations between locales (warnings will be printed)",
    )
    args = parser.parse_args()

    locales = [l.strip() for l in args.locales.split(",") if l.strip()]
    if not locales:
        print("No locales specified", file=sys.stderr)
        return 2

    results = {}
    for loc in locales:
        toml_file = args.src / f"{loc}.toml"
        if not toml_file.exists():
            print(
                f"ERROR: missing TOML for locale '{loc}' at {toml_file}",
                file=sys.stderr,
            )
            return 2
        parsed = read_toml_file(toml_file)
        cli_data = extract_cli_from_toml(parsed)
        results[loc] = cli_data

        out_file = args.dest / f"{loc}.json"
        write_json_file(cli_data, out_file)
        print(
            f"Wrote {out_file} (commands: {len(cli_data.get('commands', {}))}, flags: {len(cli_data.get('flags', {}))})"
        )

    # Basic cross-locale checks (if both en & zh present)
    if "en" in results and "zh" in results:
        missing_in_zh, missing_in_en = compare_locales(results["en"], results["zh"])
        total_missing = missing_in_zh + missing_in_en
        print(
            f"Locale comparison: missing_in_zh={missing_in_zh}, missing_in_en={missing_in_en}"
        )
        if total_missing > 0 and args.fail_on_missing:
            print(
                "Failing due to missing translations (--fail-on-missing specified)",
                file=sys.stderr,
            )
            return 3

    print("Export complete.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
