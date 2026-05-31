---
name: kam-template-development
description: Use this skill when creating, refactoring, validating, packaging, or debugging Kam project templates. Applies to tmpl/ and templates/ directories, template kam.toml files, Tera variables such as {{prop.id}}, init flows using kam init -t/--tmpl, template include/exclude rules, raw-copy areas like lib/hooks/.github, and kam tmpl import/export/package/list workflows.
---

# Kam Template Development

Use this skill for Kam template work inside this repository or inside a Kam
project generated from a template.

## Workflow

1. Inspect the template source first:
   - Built-in templates usually live under `tmpl/`.
   - Project-local templates may live under `tmpl/` or `templates/`.
   - The main template metadata is normally `kam.toml`.
2. Identify variable surfaces before editing:
   - Path variables such as `src/{{prop.id}}`.
   - Tera content variables such as `{{prop.name}}`.
   - `kam.tmpl.variables` defaults and user-provided `--var key=value`.
3. Keep raw-copy directories raw:
   - Do not rely on Tera rendering inside `lib/`, `hooks/`, or `.github/`.
   - These files often contain shell `${var}`, GitHub `${{ }}`, or other template
     syntax that conflicts with Tera.
   - If values are needed there, pass them through generated env files or runtime
     scripts instead of embedding Tera placeholders in file content.
4. Preserve path rendering:
   - File and directory names may still use placeholders, for example
     `src/{{prop.id}}/`.
   - Avoid literal `{{...}}` examples in rendered Markdown unless the file is in
     a raw-copy area or the braces are rewritten as prose.
5. Validate on the real init path:
   - Prefer a clean temp project:
     `rm -rf /tmp/kam-template-smoke`
     `cargo run -- init /tmp/kam-template-smoke -t <template-name> --force`
   - Then run:
     `cargo run -- check /tmp/kam-template-smoke`
6. For template packaging or cache work, use Kam's template commands:
   - `cargo run -- tmpl list`
   - `cargo run -- tmpl export <name> <output>`
   - `cargo run -- tmpl import <archive-or-dir>`

## Quality Rules

- Do not hide template render failures by silently copying text files.
- Prefer explicit errors when a template cannot render.
- Keep `kam.toml` in generated projects valid after variable substitution.
- Default generated module versions should satisfy Kam validators, for example
  `v1.0.0` rather than `1.0.0`.
- When changing template behavior, run Rust gates from the repo root:
  `cargo fmt --check`
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  `cargo check`

## Common Failure Modes

- A stale global template cache shadows the project-local `tmpl/` template.
  Prefer explicit path, built-in, then project-local templates before cache when
  fixing resolver behavior.
- Markdown examples like literal `{{prop.id}}` can break Tera rendering.
  Rewrite as `<module-id>` or put the example in a raw-copy location.
- Shell snippets using `${#var}` or GitHub Actions using `${{ ... }}` can be
  parsed as Tera unless the file is raw-copied.
- `kam init . --tmpl --force` is a project gate; if it fails, fix the template
  or explain the exact failing render path.
