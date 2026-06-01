---
name: setup-kam-workflow
description: Use this skill when writing, installing, reviewing, or debugging GitHub Actions workflows that use MemDeco-WG/setup-kam for Kam module repositories. Applies to kam workflow install, .github/workflows/init.yml, exec.yml, mirror-upstream-release.yml, setup-kam@v3 inputs, Kam validation/build/release CI, GitHub release publishing, workflow artifacts, submodules, cache, and KAM_PRIVATE_KEY signing secrets.
---

# Setup Kam Workflow

Use this skill for GitHub Actions workflow work around Kam module repositories
and the `MemDeco-WG/setup-kam@v3` action.

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
  uses: MemDeco-WG/setup-kam@v3
  with:
    github-token: ${{ github.token }}
    enable-cache: 'true'
    cache-targets: cargo,kam
    install-commitizen: 'false'
```

For release signing, pass the private key secret:

```yaml
- name: Setup kam
  uses: MemDeco-WG/setup-kam@v3
  with:
    github-token: ${{ github.token }}
    enable-cache: 'true'
    cache-targets: cargo,kam
    install-commitizen: 'false'
    private-key: ${{ secrets.KAM_PRIVATE_KEY }}
```

For validation-only workflows that should not warn about missing signing keys:

```yaml
warn-private-key: 'false'
```

## Standard Validate Workflow

The validation workflow should:

1. Use `actions/checkout@v6` with `submodules: recursive`.
2. Use `MemDeco-WG/setup-kam@v3`.
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
3. Use `MemDeco-WG/setup-kam@v3`.
4. Install artifact tools: `unzip zip`.
5. Verify tools:

```bash
kam --version
gh --version
```

6. Build:

```bash
find hooks src -type f -name '*.sh' -exec chmod +x {} + 2>/dev/null || true
rm -rf dist
kam build
```

7. Verify ZIP contents:
   - `module.prop` exists at ZIP root.
   - `META-INF/com/google/android/update-binary` exists.
   - `.git`, `.github`, and `.gitignore` are not packaged.
8. Upload `dist/*` with `actions/upload-artifact@v7`.

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

Signing convention:

```yaml
KAM_PRIVATE_KEY_AVAILABLE: ${{ secrets.KAM_PRIVATE_KEY != '' && '1' || '0' }}
KAM_SIGN_ENABLED: ${{ inputs.release == true && secrets.KAM_PRIVATE_KEY != '' && '1' || '0' }}
KAM_SIGN_REQUIRED: ${{ inputs.release == true && secrets.KAM_PRIVATE_KEY != '' && '1' || '0' }}
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
- Never print `KAM_PRIVATE_KEY` or other secrets.
- For docs changes only, `git diff --check` is enough; for workflow generator
  changes, run `cargo test --workspace` and compare generated workflow content.
