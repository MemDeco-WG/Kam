# Kam GitHub Workflows

English README. For Chinese, see [README-CN.md](./README-CN.md).

This directory contains the standard workflow set that `kam workflow install` writes into Kam module repositories.

## `init.yml`

`init.yml` validates a Kam module repository.

It runs on:
- `workflow_dispatch`
- `pull_request`
- pushes to `main`

Main checks:
- checkout with recursive submodules
- install Kam through `MemDeco-WG/setup-kam@v3`
- run `kam validate`
- run `kam check`
- run `shellcheck` over shell files under `hooks/`, `src/`, and `kam.sh` when present

## `exec.yml`

`exec.yml` builds and optionally releases a Kam module.

It runs on:
- `workflow_dispatch`
- `pull_request`
- pushes to `main`

Manual inputs:
- `release`: create a GitHub Release with `kam publish --all-assets`
- `prerelease`: mark the release as a prerelease

Main steps:
- checkout with recursive submodules and full history
- install Kam through `MemDeco-WG/setup-kam@v3`
- run `kam build`
- verify the generated module ZIP contains required installer files
- reject accidental `.git`, `.github`, and `.gitignore` entries inside the ZIP
- upload everything under `dist/` as a workflow artifact

## `release-android.yml`

`release-android.yml` is specific to this Kam repository. It cross-compiles the Kam CLI for Android targets and can publish those binaries to a GitHub Release.

## Installing Workflows

From a Kam module repository:

```bash
kam workflow install owner/repo
```

If `owner/repo` matches the current repository, Kam installs `init.yml` and `exec.yml`.

If `owner/repo` is different from the current repository, Kam installs `mirror-upstream-release.yml` instead. That workflow mirrors the latest upstream GitHub Release without rebuilding it.
