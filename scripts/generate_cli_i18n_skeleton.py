#!/usr/bin/env python3
# /// script
# requires-python = ">=3.12"
# dependencies = [
#     "toml>=0.10.2",
# ]
# ///
#

"""
Generate skeleton i18n keys from Rust CLI definitions.

This script scans `src/cmds/**` for command definitions (Args structs and
Subcommand enums) and extracts:

 - command summaries/description (if present as doc comments)
 - flags (field names + doc comments)
 - nested subcommands and their flags

It then compares the derived set of i18n keys to the existing
`src/i18n/en.toml` and `src/i18n/zh.toml`, reports missing keys, and can
optionally emit TOML skeleton files containing the missing keys and
reasonable English suggestions (based on the Rust doc comments).

Example usage:
  python3 Kam/scripts/generate_cli_i18n_skeleton.py            # show report
  python3 Kam/scripts/generate_cli_i18n_skeleton.py --check  # exit non-zero if missing
  python3 Kam/scripts/generate_cli_i18n_skeleton.py --write  # write skeleton files to scripts/

Notes:
 - The script uses Python 3.11+ tomllib if available; otherwise falls back to the
   third-party `toml` package (pip install toml).
 - It intentionally does NOT modify the existing en/zh TOML files. Use the
   generated skeleton to review and manually integrate translations.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Dict, List, Optional, Tuple

# --- Utilities ----------------------------------------------------------------


def camel_to_kebab(name: str) -> str:
    """Convert CamelCase or PascalCase to kebab-case (ExportPub -> export-pub)."""
    s1 = re.sub("(.)([A-Z][a-z]+)", r"\1-\2", name)
    s2 = re.sub("([a-z0-9])([A-Z])", r"\1-\2", s1)
    return s2.replace("_", "-").lower()


def load_toml_file(p: Path) -> dict:
    """Parse TOML file into a Python dict, using tomllib (py3.11+) or toml."""
    if not p.exists():
        return {}
    try:
        import tomllib

        with p.open("rb") as fh:
            return tomllib.load(fh)
    except Exception:
        try:
            import toml

            return toml.load(str(p))
        except Exception:
            print(
                "ERROR: Could not parse TOML. Install python >=3.11 or the 'toml' package.",
                file=sys.stderr,
            )
            sys.exit(3)


def flatten_dict(d: dict, prefix: str = "") -> Dict[str, object]:
    """Flatten nested dict into dotted keys mapping to leaf values (non-dicts)."""
    out: Dict[str, object] = {}
    for k, v in d.items():
        key = f"{prefix}.{k}" if prefix else k
        if isinstance(v, dict):
            out.update(flatten_dict(v, key))
        else:
            out[key] = v
    return out


def toml_str(s: Optional[str]) -> str:
    """Return a TOML-safe quoted string (simple form)."""
    if s is None:
        s = ""
    # Use json.dumps to escape properly (works for TOML string literals)
    return json.dumps(s, ensure_ascii=False)


# --- Rust parsing ------------------------------------------------------------


def extract_brace_block(lines: List[str], start_index: int) -> Tuple[List[str], int]:
    """
    Extract lines inside the first brace block starting at or after start_index.
    Returns (block_lines, end_index) where block_lines are the *inner* lines and
    end_index is the index of the line containing the matching closing brace.
    """
    # Find the opening brace
    i = start_index
    while i < len(lines) and "{" not in lines[i]:
        i += 1
    if i >= len(lines):
        return [], start_index
    depth = 0
    block_lines: List[str] = []
    started = False
    # Walk until matching brace
    while i < len(lines):
        line = lines[i]
        # Count braces - naive but works for typical struct/enum blocks
        opens = line.count("{")
        closes = line.count("}")
        if "{" in line and not started:
            started = True
            depth += opens - closes
            # Do not include the opening line itself
        else:
            if started:
                block_lines.append(line)
                depth += opens - closes
        if started and depth <= 0:
            return block_lines, i
        i += 1
    return block_lines, i


def strip_doc_lines(doc_lines: List[str]) -> str:
    """Concatenate a list of rust doc lines (/// ...) into a single string."""
    s = " ".join(dl.strip().lstrip("/").strip() for dl in doc_lines)
    return re.sub(r"\s+", " ", s).strip()


def parse_struct_body(
    body_lines: List[str],
) -> Tuple[Dict[str, Optional[str]], Optional[str]]:
    """
    Parse a Rust struct body and extract public fields and their doc comments.

    Returns (fields_map, struct_summary)
      - fields_map: name -> docstring (or None)
      - struct_summary: doc comment found immediately preceding struct (if any)
    """
    fields: Dict[str, Optional[str]] = {}
    docblock: List[str] = []
    # We don't have easy access to the struct's top doc comments here;
    # callers may pass them separately if needed. So we only parse fields.
    i = 0
    while i < len(body_lines):
        line = body_lines[i].rstrip()
        stripped = line.strip()
        if stripped.startswith("///"):
            docblock.append(stripped)
            i += 1
            continue
        # Skip attribute lines like #[arg(...)]
        if stripped.startswith("#["):
            # If attribute is multi-line, skip until end of attribute (unlikely here)
            i += 1
            continue
        # Field line: match `pub <name>:` or `<name>:` (could be private)
        m = re.match(r"^\s*(?:pub\s+)?([A-Za-z0-9_]+)\s*:\s*([^,]+),?", line)
        if m:
            name = m.group(1)
            # Save docblock if any; combine and clear
            doc = strip_doc_lines(docblock) if docblock else None
            fields[name] = doc
            docblock = []
        else:
            # reset docblock if we hit unrelated lines (impls, comments)
            if stripped and not stripped.startswith("//"):
                docblock = []
        i += 1
    return fields, None


def parse_subcommand_enum(body_lines: List[str]) -> Dict[str, Dict]:
    """
    Parse a Subcommand enum block and extract variants with docs and nested fields.

    Returns:
      { subcmd_kebab_name: { 'summary': str | None, 'flags': {name: doc|None} } }
    """
    subcommands: Dict[str, Dict] = {}
    docblock: List[str] = []
    i = 0
    while i < len(body_lines):
        line = body_lines[i].rstrip()
        stripped = line.strip()
        if stripped.startswith("///"):
            docblock.append(stripped)
            i += 1
            continue
        # Variant with inline block: e.g., Add { ... },
        m_block = re.match(r"^\s*([A-Za-z0-9_]+)\s*\{\s*$", stripped)
        if m_block:
            variant = m_block.group(1)
            # Extract the inner block until the closing `}` of the variant
            sub_body, end_idx = extract_brace_block(body_lines, i)
            fields, _ = parse_struct_body(sub_body)
            summary = strip_doc_lines(docblock) if docblock else None
            subcommands[camel_to_kebab(variant)] = {"summary": summary, "flags": fields}
            docblock = []
            i = end_idx + 1
            # Skip trailing comma line if present
            if i < len(body_lines) and body_lines[i].strip().startswith(","):
                i += 1
            continue
        # Simple variant: `List,`
        m_simple = re.match(r"^\s*([A-Za-z0-9_]+)\s*,\s*$", stripped)
        if m_simple:
            variant = m_simple.group(1)
            summary = strip_doc_lines(docblock) if docblock else None
            subcommands[camel_to_kebab(variant)] = {"summary": summary, "flags": {}}
            docblock = []
            i += 1
            continue
        # If nothing matched, skip line
        docblock = []
        i += 1
    return subcommands


def parse_rust_file_for_command(path: Path) -> Dict:
    """
    Parse a rust file for Args structs and Subcommand enums and return a
    structure:
      {
        'summary': Optional[str],   # if a top-level docblock exists before Args
        'flags': { name -> docstring|None, ... },
        'subcommands': { subname -> { 'summary': .., 'flags': {...} }, ... }
      }
    """
    text = path.read_text(encoding="utf-8")
    lines = text.splitlines()
    result = {"summary": None, "flags": {}, "subcommands": {}}

    # We'll scan for derive(Args) and derive(Subcommand)
    i = 0
    pending_doc: List[str] = []
    while i < len(lines):
        raw = lines[i]
        s = raw.strip()
        if s.startswith("///"):
            pending_doc.append(s)
            i += 1
            continue
        # handle derive(Args)
        if s.startswith("#[derive") and "Args" in s:
            # find the pub struct line
            j = i + 1
            while j < len(lines) and "pub struct" not in lines[j]:
                # keep collecting doc in case struct docblock follows derive
                if lines[j].strip().startswith("///"):
                    pending_doc.append(lines[j].strip())
                j += 1
            if j < len(lines) and "pub struct" in lines[j]:
                # parse struct body
                body_lines, end_idx = extract_brace_block(lines, j)
                fields, _ = parse_struct_body(body_lines)
                # merge into flags
                for k, v in fields.items():
                    result["flags"][k] = v
                # if pending_doc exists and result['summary'] is None, set it
                if pending_doc and not result["summary"]:
                    result["summary"] = strip_doc_lines(pending_doc)
                pending_doc = []
                i = end_idx + 1
                continue
        # handle derive(Subcommand) - parse enum variants
        if s.startswith("#[derive") and "Subcommand" in s:
            # find pub enum
            j = i + 1
            while j < len(lines) and "pub enum" not in lines[j]:
                if lines[j].strip().startswith("///"):
                    pending_doc.append(lines[j].strip())
                j += 1
            if j < len(lines) and "pub enum" in lines[j]:
                enum_body, end_idx = extract_brace_block(lines, j)
                subcmds = parse_subcommand_enum(enum_body)
                # merge
                for k, v in subcmds.items():
                    # If existing subcommand already seen, merge flags
                    if k in result["subcommands"]:
                        existing = result["subcommands"][k]
                        existing_flags = existing.get("flags", {})
                        existing_flags.update(v.get("flags", {}))
                        if v.get("summary") and not existing.get("summary"):
                            existing["summary"] = v.get("summary")
                    else:
                        result["subcommands"][k] = v
                pending_doc = []
                i = end_idx + 1
                continue
        # If attribute line indicates a subcommand field in a struct, record the type name so we can try to locate enum elsewhere
        if "command(subcommand)" in s:
            # Example: #[command(subcommand)]\n    # pub command: CacheCommands,
            # Find the following field line to get enum type name
            k = i + 1
            while k < len(lines):
                m = re.search(r"pub\s+([a-zA-Z0-9_]+)\s*:\s*([A-Za-z0-9_]+)", lines[k])
                if m:
                    # type name in m.group(2) - search for enum with that name in the file
                    enum_type = m.group(2)
                    # try to find pub enum <enum_type> in file and parse it
                    enum_pattern = re.compile(
                        rf"pub\s+enum\s+{re.escape(enum_type)}\s*\{{"
                    )
                    for p in range(0, len(lines)):
                        if enum_pattern.search(lines[p]):
                            enum_body, end_idx = extract_brace_block(lines, p)
                            subcmds = parse_subcommand_enum(enum_body)
                            for kk, vv in subcmds.items():
                                if kk in result["subcommands"]:
                                    ex = result["subcommands"][kk]
                                    ex.get("flags", {}).update(vv.get("flags", {}))
                                else:
                                    result["subcommands"][kk] = vv
                            break
                    break
                k += 1
        # no match - reset pending doc and continue
        pending_doc = []
        i += 1

    return result


# --- Skeleton generation & reporting ----------------------------------------


def collect_commands_from_src(cmds_dir: Path) -> Dict[str, Dict]:
    """
    Walk src/cmds and parse each command directory/file to collect pairs:

      commands[cmd_name] = {
          'summary': Optional[str],
          'flags': { name: doc|None },
          'subcommands': { name: { 'summary':..., 'flags': {...} } }
      }
    """
    commands: Dict[str, Dict] = {}
    if not cmds_dir.exists():
        print(f"ERROR: commands directory not found: {cmds_dir}", file=sys.stderr)
        return commands

    # First, handle files directly under src/cmds (e.g., install.rs could exist)
    for p in sorted(cmds_dir.iterdir()):
        if p.is_file() and p.suffix == ".rs":
            cmd_name = p.stem
            parsed = parse_rust_file_for_command(p)
            commands[cmd_name] = parsed

    # Now, handle directories under src/cmds
    for d in sorted(cmds_dir.iterdir()):
        if d.is_dir():
            cmd_name = d.name
            # Look for args.rs or lib.rs or mod.rs or <cmd>.rs inside
            parsed_accum = {"summary": None, "flags": {}, "subcommands": {}}
            files_to_scan = []
            # Prioritize args.rs
            candidate = d / "args.rs"
            if candidate.exists():
                files_to_scan.append(candidate)
            # Also take any .rs files
            for r in sorted(d.glob("*.rs")):
                if r not in files_to_scan:
                    files_to_scan.append(r)
            # Parse each file and merge results
            for f in files_to_scan:
                parsed = parse_rust_file_for_command(f)
                if parsed.get("summary") and not parsed_accum.get("summary"):
                    parsed_accum["summary"] = parsed.get("summary")
                parsed_accum["flags"].update(parsed.get("flags", {}))
                # merge subcommands
                for sc, scv in parsed.get("subcommands", {}).items():
                    if sc in parsed_accum["subcommands"]:
                        parsed_accum["subcommands"][sc]["flags"].update(
                            scv.get("flags", {})
                        )
                        if scv.get("summary") and not parsed_accum["subcommands"][
                            sc
                        ].get("summary"):
                            parsed_accum["subcommands"][sc]["summary"] = scv.get(
                                "summary"
                            )
                    else:
                        parsed_accum["subcommands"][sc] = scv
            commands[cmd_name] = parsed_accum
    return commands


def build_expected_keys(commands: Dict[str, Dict]) -> Dict[str, Optional[str]]:
    """
    From the parsed commands dictionary, build expected i18n keys and a suggested
    English value (if doc comment present), returning a mapping:

       expected[key] = suggested_value_or_None
    """
    expected: Dict[str, Optional[str]] = {}
    for cmd, info in commands.items():
        cmd_prefix = f"cli.commands.{cmd}"
        # summary
        expected[f"{cmd_prefix}.summary"] = info.get("summary")
        if info.get("summary"):
            # also suggest a long description if present
            expected[f"{cmd_prefix}.description"] = info.get("summary")
        # flags
        flags = info.get("flags", {})
        if flags:
            for fname, doc in flags.items():
                expected[f"{cmd_prefix}.flags.{fname}"] = doc
        # subcommands
        subcmds = info.get("subcommands", {})
        for sc, scinfo in subcmds.items():
            sc_prefix = f"{cmd_prefix}.{sc}"
            expected[f"{sc_prefix}.summary"] = scinfo.get("summary")
            flags2 = scinfo.get("flags", {})
            for fname, doc in flags2.items():
                expected[f"{sc_prefix}.flags.{fname}"] = doc
    return expected


def render_toml_skeleton(
    suggested_by_key: Dict[str, Optional[str]], keys: List[str]
) -> str:
    """
    Render a TOML snippet for the provided keys using suggestions when available.

    The function groups keys into logical [cli.commands.<cmd>] blocks and
    prints [cli.commands.<cmd>.flags] as needed.
    """
    # Group keys by prefix: e.g., cli.commands.build -> { 'summary', 'description', 'flags': { ... } }
    grouped = {}
    for key in keys:
        parts = key.split(".")
        # Expect keys to start with cli.commands...
        if not key.startswith("cli.commands."):
            # leave other keys as-is
            top = ".".join(parts[:2])
            grouped.setdefault(top, []).append(key)
            continue
        # Remove 'cli.commands.' prefix
        rest = parts[2:]
        cmd = rest[0]
        rest_after_cmd = rest[1:]
        grouped.setdefault(cmd, []).append(rest_after_cmd)

    lines: List[str] = []
    lines.append(
        "# Generated skeleton (review & merge into src/i18n/*.toml as appropriate)"
    )
    lines.append(
        "# Keys with English suggestions (if available) are filled; translate for zh."
    )
    lines.append("")

    for cmd, entries in sorted(grouped.items()):
        # entries is a list of rest_after_cmd lists
        # Find command-level properties
        cmd_summary_key = f"cli.commands.{cmd}.summary"
        if (
            any(
                isinstance(e, list) and len(e) == 1 and e[0] == "summary"
                for e in entries
            )
            or cmd_summary_key in suggested_by_key
        ):
            lines.append(f"[cli.commands.{cmd}]")
            if cmd_summary_key in suggested_by_key:
                lines.append(
                    f"summary = {toml_str(suggested_by_key.get(cmd_summary_key))}"
                )
            else:
                lines.append('summary = ""')
            # also description (if missing)
            cmd_desc_key = f"cli.commands.{cmd}.description"
            if cmd_desc_key in suggested_by_key:
                lines.append(
                    f"description = {toml_str(suggested_by_key.get(cmd_desc_key))}"
                )
            lines.append("")  # blank line

        # Flags for the command
        flag_entries = [
            e
            for e in entries
            if isinstance(e, list) and len(e) >= 3 and e[0] == "flags"
        ]
        if flag_entries:
            lines.append(f"[cli.commands.{cmd}.flags]")
            for entry in flag_entries:
                # entry looks like ['flags', '<name>']
                if len(entry) >= 2:
                    fname = entry[1]
                    key = f"cli.commands.{cmd}.flags.{fname}"
                    suggestion = suggested_by_key.get(key)
                    if suggestion:
                        lines.append(f"{fname} = {toml_str(suggestion)}")
                    else:
                        lines.append(f'{fname} = ""')
            lines.append("")

        # Subcommands
        subcmd_entries = [
            e
            for e in entries
            if isinstance(e, list)
            and len(e) >= 1
            and e[0] != "flags"
            and e[0] != "summary"
            and e[0] != "description"
        ]
        # We need to discover unique subcommand names from entries:
        subcmds = set()
        for e in entries:
            if isinstance(e, list) and len(e) >= 1:
                subcmds.add(e[0])
        for sc in sorted(
            k for k in subcmds if k not in ("flags", "summary", "description")
        ):
            sc_summary_key = f"cli.commands.{cmd}.{sc}.summary"
            lines.append(f"[cli.commands.{cmd}.{sc}]")
            if sc_summary_key in suggested_by_key and suggested_by_key[sc_summary_key]:
                lines.append(f"summary = {toml_str(suggested_by_key[sc_summary_key])}")
            else:
                lines.append('summary = ""')
            lines.append("")  # blank line
            # subcommand flags
            # find all flag keys with prefix cli.commands.cmd.sc.flags.*
            sc_flag_keys = [
                k
                for k in suggested_by_key.keys()
                if k.startswith(f"cli.commands.{cmd}.{sc}.flags.")
            ]
            if sc_flag_keys:
                lines.append(f"[cli.commands.{cmd}.{sc}.flags]")
                for key in sorted(sc_flag_keys):
                    fname = key.split(".")[-1]
                    val = suggested_by_key.get(key)
                    if val:
                        lines.append(f"{fname} = {toml_str(val)}")
                    else:
                        lines.append(f'{fname} = ""')
                lines.append("")
    return "\n".join(lines)


# --- Main --------------------------------------------------------------------


def main(argv: Optional[List[str]] = None) -> int:
    argv = argv or sys.argv[1:]
    parser = argparse.ArgumentParser(
        description="Scan src/cmds and generate/print missing i18n skeleton keys."
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Exit non-zero if missing keys are detected",
    )
    parser.add_argument(
        "--write",
        action="store_true",
        help="Write skeleton files to --out-dir (scripts/ by default)",
    )
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=Path("Kam/scripts"),
        help="Directory to write skeleton TOML files",
    )
    parser.add_argument(
        "--locales",
        type=str,
        default="en,zh",
        help="Locales to check / generate (comma-separated)",
    )
    args = parser.parse_args(argv)

    repo_root = Path(__file__).resolve().parents[1]
    cmds_dir = repo_root / "src" / "cmds"
    i18n_dir = repo_root / "src" / "i18n"
    en_toml = i18n_dir / "en.toml"
    zh_toml = i18n_dir / "zh.toml"

    print(f"Scanning commands in: {cmds_dir}")
    commands = collect_commands_from_src(cmds_dir)
    print(
        f"Discovered {len(commands)} commands (sample: {', '.join(sorted(list(commands)[:6]))})"
    )

    expected = build_expected_keys(commands)

    # Load existing i18n keys
    en = load_toml_file(en_toml)
    zh = load_toml_file(zh_toml)
    en_flat = set(flatten_dict(en).keys())
    zh_flat = set(flatten_dict(zh).keys())

    missing_en = {}
    missing_zh = {}

    for key, suggestion in expected.items():
        if key not in en_flat:
            missing_en[key] = suggestion
        if key not in zh_flat:
            missing_zh[key] = (
                suggestion  # suggestion here is English; translator will use as hint
            )

    # Report
    if missing_en:
        print("=== Missing in en.toml ===")
        for k, v in sorted(missing_en.items()):
            print(f"- {k}")
            if v:
                print(f"    suggestion: {v}")
        print(f"Total missing in en.toml: {len(missing_en)}")
    else:
        print("No missing CLI keys detected in en.toml.")

    if missing_zh:
        print("=== Missing in zh.toml ===")
        for k, v in sorted(missing_zh.items()):
            print(f"- {k}")
            if v:
                print(f"    (en suggestion: {v})")
        print(f"Total missing in zh.toml: {len(missing_zh)}")
    else:
        print("No missing CLI keys detected in zh.toml.")

    # Optionally write skeleton TOML files
    if args.write:
        out_dir = args.out_dir
        out_dir.mkdir(parents=True, exist_ok=True)
        # For en, use English suggestions; for zh, leave blank or include en suggestion as comment
        en_skeleton = render_toml_skeleton(missing_en, sorted(missing_en.keys()))
        zh_skeleton = render_toml_skeleton(
            {k: "" for k in missing_zh.keys()}, sorted(missing_zh.keys())
        )
        en_out = out_dir / "cli-skeleton.en.toml"
        zh_out = out_dir / "cli-skeleton.zh.toml"
        en_out.write_text(en_skeleton + "\n", encoding="utf-8")
        zh_out.write_text(zh_skeleton + "\n", encoding="utf-8")
        print(f"Wrote skeletons to: {en_out} and {zh_out}")

    # Exit code for --check
    if args.check:
        total_missing = len(missing_en) + len(missing_zh)
        if total_missing > 0:
            print(
                f"Missing i18n keys detected: {total_missing} (en: {len(missing_en)}, zh: {len(missing_zh)})"
            )
            return 1
        else:
            print("No missing i18n keys detected.")
            return 0

    # Otherwise print summary and exit 0
    print("Done.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
