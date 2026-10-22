---
name: setup-kam-workflow
description: Use this skill when writing, installing, reviewing, or debugging GitHub Actions workflows that use MemDeco-WG/setup-kam for Kam module repositories. Applies to kam workflow install, .github/workflows/init.yml, exec.yml, mirror-upstream-release.yml, setup-kam inputs, Kam validation/build/release CI, GitHub release publishing, workflow artifacts, submodules, and cache.
---

# Setup Kam Workflow

Use this skill for GitHub Actions workflow work around Kam module repositories
and the `MemDeco-WG/setup-kam@2a0e9193a7e84dc7da78e99af586f8a4bb823bcb` action.

## Fast Path

Prefer Kam's generator when the repository is a normal Kam module repository:

```bash
kam workflow install owner/repo
```

- If `owner/repo` matches the current repository, Kam writes standard
  `.github/workflows/init.yml` and `.github/workflows/exec.yml`.
- If `owner/repo` points to a different source repository, Kam writes
  `.github/workflows/mirror-upstream-release.yml` to mirror upstream releases
  without rebuilding.

When editing Kam itself, the source of truth is:

- `.github/workflows/init.yml`
- `.github/workflows/exec.yml`
- `.github/workflows/README.md`
- `src/cmds/workflow.rs`

## Setup Action

Use the current setup action version:

```yaml
- name: Setup kam
  uses: MemDeco-WG/setup-kam@2a0e9193a7e84dc7da78e99af586f8a4bb823bcb
  with:
    github-token: ${{ github.token }}
    enable-cache: 'true'
    cache-targets: cargo,kam
    install-commitizen: 'false'
```

Setup and release publishing do not require a KernelSU developer key.

## Standard Validate Workflow

The validation workflow should:

1. Use `actions/checkout@v6` with `submodules: recursive`.
2. Use `MemDeco-WG/setup-kam@2a0e9193a7e84dc7da78e99af586f8a4bb823bcb`.
3. Install lint dependencies when shell/YAML/JSON checks are needed:
   `shellcheck jq python3-yaml`.
4. Run:

```bash
kam validate
kam check
```

5. Run `shellcheck` over shell files under `hooks/`, `src/`, and top-level
   `kam.sh` when present.

## Standard Build Workflow

The build workflow should:

1. Use `actions/checkout@v6` with `submodules: recursive` and `fetch-depth: 0`.
2. Cache Cargo/Rustup when Rust dependencies may be built.
3. Use `MemDeco-WG/setup-kam@2a0e9193a7e84dc7da78e99af586f8a4bb823bcb`.
4. Install artifact tools: `unzip zip`.
5. Install Android Rust targets used by KamHooks:

```bash
rustup target add aarch64-linux-android x86_64-linux-android
```

6. Verify tools:

```bash
kam --version
gh --version
```

7. Build:

```bash
find hooks src -type f -name '*.sh' -exec chmod +x {} + 2>/dev/null || true
rm -rf dist
kam build
```

8. Verify ZIP contents:
   - `module.prop` exists at ZIP root.
   - `META-INF/com/google/android/update-binary` exists.
   - `.git`, `.github`, and `.gitignore` are not packaged.
9. Upload `dist/*` with `actions/upload-artifact@v7`.

## Releases

Use manual workflow inputs for release publishing:

```yaml
workflow_dispatch:
  inputs:
    release:
      type: boolean
      default: false
    prerelease:
      type: boolean
      default: false
```

Use `permissions.contents: write` for release workflows and set:

```yaml
env:
  GH_TOKEN: ${{ github.token }}
```

Publish with:

```bash
args=(--all-assets)
if [ "${{ inputs.prerelease }}" = "true" ]; then
  args+=(--prerelease)
fi
kam publish "${args[@]}"
```

## Mirror-Only Repositories

For KernelSU Modules Repo metadata-only repositories where source code lives
elsewhere, do not rebuild. Install the mirror workflow through:

```bash
kam workflow install upstream-owner/upstream-repo
```

The mirror workflow should use:

- `permissions.contents: write`
- `GH_TOKEN: ${{ github.token }}`
- `gh api repos/${UPSTREAM_REPO}/releases/latest`
- `gh release create` after deleting the local matching tag/release if needed

## Review Checklist

- Do not hand-roll workflows when `kam workflow install` can regenerate them.
- Keep action versions aligned with the repository source of truth.
- Use recursive submodule checkout for Kam module repos.
- Keep release jobs manual unless the repository explicitly wants tag-based
  publishing.
- Never print secrets.
- For docs changes only, `git diff --check` is enough; for workflow generator
  changes, run `cargo test --workspace` and compare generated workflow content.
