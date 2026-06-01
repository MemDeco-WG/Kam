---
name: kam-template-development
description: Use this skill when creating, refactoring, validating, packaging, installing, or debugging Kam project templates and KernelSU module repository init flows. Applies to tmpl/ and templates/ directories, template kam.toml files, Tera variables such as {{prop.id}}, kam init -t/--template/--tmpl, --repo-mode full|reference, --source-url, --metamodule, template include/exclude rules, raw-copy areas like lib/hooks/.github, and kam tmpl import/export/package/list workflows.
---

# Kam Template Development

Use this skill for Kam template work inside this repository or inside a Kam
project generated from a template.

## Install This Skill

For other agents or module developers, install this repo-local skill before
asking the agent to develop Kam templates:

```bash
npx skills add https://github.com/MemDeco-WG/Kam --path .agents/skills/kam-template-development
```

If the installer in use expects a skill name instead of a path, use:

```bash
npx skills add https://github.com/MemDeco-WG/Kam --skill kam-template-development
```

## Workflow

1. Inspect the template source first:
   - Built-in templates usually live under `tmpl/`.
   - Project-local templates may live under `tmpl/` or `templates/`.
   - The main template metadata is normally `kam.toml`.
2. Identify variable surfaces before editing:
   - Path variables such as `src/{{prop.id}}`.
   - Tera content variables such as `{{prop.name}}`.
   - `kam.tmpl.variables` defaults and user-provided `--var key=value`.
   - Init metadata variables also include `source_url` and `metamodule` when
     `--source-url` or `--metamodule` is used.
3. Decide which init mode the template must support:
   - `--repo-mode full` creates a complete Kam project with module source.
   - `--repo-mode reference` creates a KernelSU Modules Repo metadata-only
     repository with `README.md` and `module.json`.
   - Use `--source-url` for reference repositories that point to an external
     source tree.
   - Use `--metamodule` only when the generated metadata should mark the module
     as a KernelSU metamodule.
4. Keep raw-copy directories raw:
   - Do not rely on Tera rendering inside `lib/`, `hooks/`, or `.github/`.
   - These files often contain shell `${var}`, GitHub `${{ }}`, or other template
     syntax that conflicts with Tera.
   - If values are needed there, pass them through generated env files or runtime
     scripts instead of embedding Tera placeholders in file content.
5. Preserve path rendering:
   - File and directory names may still use placeholders, for example
     `src/{{prop.id}}/`.
   - Avoid literal `{{...}}` examples in rendered Markdown unless the file is in
     a raw-copy area or the braces are rewritten as prose.
6. Validate on the real init path:
   - Template project smoke:
     `cargo run -- init /tmp/kam-template-smoke --tmpl --force`
   - Built-in or custom template smoke:
     `cargo run -- init /tmp/kam-module-smoke -t <template-name> --force`
   - KernelSU full repository smoke:
     `cargo run -- init /tmp/kam-full-smoke --repo-mode full --id org.example.module --project-name "Example Module" --description "Example" --force`
   - KernelSU reference repository smoke:
     `cargo run -- init /tmp/kam-reference-smoke --repo-mode reference --source-url https://github.com/owner/source --id org.example.module --project-name "Example Module" --description "Example" --force`
   - Then run `cargo run -- check <smoke-dir>` when the output contains a full
     Kam project.
7. For template packaging or cache work, use Kam's template commands:
   - `cargo run -- tmpl list`
   - `cargo run -- tmpl export <name> <output>`
   - `cargo run -- tmpl import <archive-or-dir>`

## Template Resolution

Kam resolves a template spec in this order:

1. Direct local path, relative to the current working directory.
2. Built-in asset such as `kam_template.tar.gz`.
3. Project-local `tmpl/` or `templates/` directory or archive.
4. Global template cache.

For simple names that do not look like a path or archive and do not already end
with `_template`, Kam also retries with `<name>_template`.

Prefer an explicit path when debugging cache shadowing.

## KernelSU Modules Repo Rules

- Repository name must match `module.prop` `id`.
- Valid IDs match `^[a-zA-Z][a-zA-Z0-9._-]+$`.
- Reserved names include `.github`, `submission`, `developers`, `modules`,
  `org.kernelsu.example`, and `module_release`.
- Reference mode intentionally writes only `README.md` and `module.json`.
- Full mode should produce the normal Kam project files plus compatible
  KernelSU metadata.

## Quality Rules

- Do not hide template render failures by silently copying text files.
- Prefer explicit errors when a template cannot render.
- Keep `kam.toml` in generated projects valid after variable substitution.
- Default generated module versions should satisfy Kam validators, for example
  `v1.0.0` rather than `1.0.0`.
- When changing template behavior, run Rust gates from the repo root:
  `cargo fmt --check`
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  `cargo test --workspace`

## Common Failure Modes

- A stale global template cache shadows the project-local `tmpl/` template.
  Prefer explicit paths when reproducing resolver behavior.
- Markdown examples like literal `{{prop.id}}` can break Tera rendering.
  Rewrite as `<module-id>` or put the example in a raw-copy location.
- Shell snippets using `${#var}` or GitHub Actions using `${{ ... }}` can be
  parsed as Tera unless the file is raw-copied.
- `kam init . --tmpl --force` is a project gate; if it fails, fix the template
  or explain the exact failing render path.
- `--repo-mode reference` does not generate a full project, so do not run
  full-project checks against that output.
