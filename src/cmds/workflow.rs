use crate::errors::KamError;
use crate::utils::Utils;
use clap::{Args, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const VALIDATE_WORKFLOW: &str = r#"name: Validate Kam Module

on:
  workflow_dispatch:
  pull_request:
  push:
    branches:
      - main

permissions:
  contents: read

concurrency:
  group: kam-validate-${{ github.ref }}
  cancel-in-progress: true

env:
  FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true

jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout repository
        uses: actions/checkout@v6
        with:
          submodules: recursive

      - name: Setup kam
        uses: MemDeco-WG/setup-kam@v3
        with:
          github-token: ${{ github.token }}
          enable-cache: 'true'
          cache-targets: cargo,kam
          install-commitizen: 'false'
          warn-private-key: 'false'

      - name: Install lint dependencies
        run: sudo apt-get update && sudo apt-get install -y shellcheck jq python3-yaml

      - name: Validate repository
        shell: bash
        run: |
          set -euo pipefail
          kam validate
          kam check
"#;

const BUILD_WORKFLOW: &str = r#"name: Build Kam Module

on:
  workflow_dispatch:
    inputs:
      release:
        description: Create GitHub release and upload artifacts
        required: false
        type: boolean
        default: false
      prerelease:
        description: Mark GitHub release as a pre-release
        required: false
        type: boolean
        default: false
  push:
    branches:
      - main
  pull_request:

permissions:
  contents: write
  actions: read

concurrency:
  group: kam-build-${{ github.ref }}
  cancel-in-progress: true

env:
  FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true

jobs:
  build:
    runs-on: ubuntu-latest
    env:
      KAM_PRIVATE_KEY_AVAILABLE: ${{ secrets.KAM_PRIVATE_KEY != '' && '1' || '0' }}
      KAM_SIGN_ENABLED: ${{ inputs.release == true && secrets.KAM_PRIVATE_KEY != '' && '1' || '0' }}
      KAM_SIGN_REQUIRED: ${{ inputs.release == true && secrets.KAM_PRIVATE_KEY != '' && '1' || '0' }}
      KAM_CHANGELOG_ENABLED: '0'
      GH_TOKEN: ${{ github.token }}

    steps:
      - name: Checkout repository
        uses: actions/checkout@v6
        with:
          submodules: recursive
          fetch-depth: 0

      - name: Cache Cargo
        uses: actions/cache@v5
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
          key: ${{ runner.os }}-rust-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-rust-

      - name: Setup kam
        uses: MemDeco-WG/setup-kam@v3
        with:
          github-token: ${{ github.token }}
          enable-cache: 'true'
          cache-targets: cargo,kam
          install-commitizen: 'false'
          private-key: ${{ secrets.KAM_PRIVATE_KEY }}

      - name: Install artifact tools
        run: sudo apt-get update && sudo apt-get install -y unzip zip

      - name: Install Android Rust targets
        shell: bash
        run: |
          set -euo pipefail
          sysroot="$(rustc --print sysroot)"
          rm -rf \
            "${sysroot}/lib/rustlib/aarch64-linux-android" \
            "${sysroot}/lib/rustlib/x86_64-linux-android"
          rustup target add aarch64-linux-android x86_64-linux-android

      - name: Install Android cargo build tool
        run: cargo install cargo-ndk --locked

      - name: Verify tools
        shell: bash
        run: |
          set -euo pipefail
          kam --version
          gh --version
          cargo ndk --version

      - name: Build module
        shell: bash
        run: |
          set -euo pipefail
          find hooks src -type f -name '*.sh' -exec chmod +x {} + 2>/dev/null || true
          rm -rf dist
          kam build

      - name: Verify artifact contents
        shell: bash
        run: |
          set -euo pipefail
          module_id="$(sed -n 's/^id[[:space:]]*=[[:space:]]*"\(.*\)"/\1/p' kam.toml | head -n1)"
          test -n "$module_id"
          test -f "dist/${module_id}.zip"
          unzip -Z1 "dist/${module_id}.zip" | grep -Fx 'module.prop'
          ! unzip -Z1 "dist/${module_id}.zip" | grep -E '(^|/)\.git($|/)'

      - name: Create GitHub release
        if: ${{ inputs.release == true }}
        shell: bash
        run: |
          set -euo pipefail
          args=(--all-assets)
          if [ "${{ inputs.prerelease }}" = "true" ]; then
            args+=(--prerelease)
          fi
          kam publish "${args[@]}"

      - name: Upload build artifact
        if: always() && hashFiles('dist/*') != ''
        uses: actions/upload-artifact@v7
        with:
          name: kam-module-artifact
          path: dist/*
          if-no-files-found: error
"#;

const MIRROR_RELEASE_WORKFLOW: &str = r#"name: Mirror upstream release

permissions:
  contents: write

on:
  workflow_dispatch:
  schedule:
    - cron: "17 * * * *"

jobs:
  mirror:
    runs-on: ubuntu-latest
    env:
      UPSTREAM_REPO: __UPSTREAM_REPO__
      GH_TOKEN: ${{ github.token }}
    steps:
      - name: Checkout repository
        uses: actions/checkout@v6

      - name: Download latest upstream release
        shell: bash
        run: |
          set -euo pipefail

          gh api "repos/${UPSTREAM_REPO}/releases/latest" > release.json
          jq -r '.tag_name' release.json > tag.txt
          jq -r '.name // .tag_name' release.json > title.txt
          jq -r '.body // ""' release.json > release-notes.md

          mkdir -p upstream-release
          jq -r '.assets[] | @base64' release.json | while read -r asset; do
            name="$(printf '%s' "${asset}" | base64 -d | jq -r '.name')"
            url="$(printf '%s' "${asset}" | base64 -d | jq -r '.url')"
            gh api \
              -H "Accept: application/octet-stream" \
              "${url}" > "upstream-release/${name}"
          done

      - name: Publish unchanged release
        shell: bash
        run: |
          set -euo pipefail

          tag="$(cat tag.txt)"
          title="$(cat title.txt)"

          if gh release view "${tag}" >/dev/null 2>&1; then
            gh release delete "${tag}" --yes --cleanup-tag
          fi

          shopt -s nullglob
          assets=(upstream-release/*)

          if [ "${#assets[@]}" -eq 0 ]; then
            gh release create "${tag}" \
              --title "${title}" \
              --notes-file release-notes.md \
              --latest
          else
            gh release create "${tag}" "${assets[@]}" \
              --title "${title}" \
              --notes-file release-notes.md \
              --latest
          fi
"#;

/// Arguments for `kam workflow`.
#[derive(Args, Debug, Clone)]
pub struct WorkflowArgs {
    /// Workflow management command.
    #[command(subcommand)]
    pub command: WorkflowCommand,
}

/// Commands for installing generated GitHub Actions workflows.
#[derive(Subcommand, Debug, Clone)]
pub enum WorkflowCommand {
    /// Install a standard Kam build workflow or an upstream-release mirror workflow.
    Install {
        /// Module source repository address. Accepts owner/repo, GitHub URLs, SSH URLs, or a local Git checkout path.
        source_repo: String,
    },
}

fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn repo_from_git_origin(path: &Path) -> Option<String> {
    let path_arg = path.to_str()?;
    let remote = command_output("git", &["-C", path_arg, "remote", "get-url", "origin"])?;
    normalize_github_repo(&remote)
}

fn current_repo() -> Option<String> {
    if let Ok(repo) = std::env::var("GITHUB_REPOSITORY")
        && !repo.trim().is_empty()
    {
        return normalize_github_repo(&repo);
    }
    repo_from_git_origin(Path::new("."))
}

fn trim_repo_suffix(value: &str) -> &str {
    value
        .trim()
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .trim_end_matches('/')
}

fn normalize_github_repo(value: &str) -> Option<String> {
    let value = trim_repo_suffix(value);
    let path = if let Some(rest) = value.strip_prefix("git@github.com:") {
        rest
    } else if let Some(rest) = value.strip_prefix("ssh://git@github.com/") {
        rest
    } else if let Some(rest) = value.strip_prefix("https://github.com/") {
        rest
    } else if let Some(rest) = value.strip_prefix("http://github.com/") {
        rest
    } else if let Some(rest) = value.strip_prefix("github.com/") {
        rest
    } else {
        value
    };

    let parts = path
        .split('/')
        .filter(|part| !part.is_empty())
        .take(2)
        .collect::<Vec<_>>();
    if parts.len() == 2 {
        Some(format!("{}/{}", parts[0], parts[1]))
    } else {
        None
    }
}

fn normalize_source_repo(source_repo: &str) -> Result<String, KamError> {
    let source_path = PathBuf::from(source_repo);
    if source_path.exists()
        && let Some(repo) = repo_from_git_origin(&source_path)
    {
        return Ok(repo);
    }

    normalize_github_repo(source_repo).ok_or_else(|| {
        KamError::InvalidUrl(format!(
            "Could not parse GitHub repository from: {source_repo}"
        ))
    })
}

fn write_workflow(path: &Path, content: &str) -> Result<(), KamError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(KamError::Io)?;
    }
    fs::write(path, content).map_err(KamError::Io)
}

fn install_standard_workflows(workflows_dir: &Path) -> Result<(), KamError> {
    write_workflow(&workflows_dir.join("init.yml"), VALIDATE_WORKFLOW)?;
    write_workflow(&workflows_dir.join("exec.yml"), BUILD_WORKFLOW)?;
    Utils::success("Installed standard Kam validation and build workflows.");
    Ok(())
}

fn install_mirror_workflow(workflows_dir: &Path, upstream_repo: &str) -> Result<(), KamError> {
    let workflow = MIRROR_RELEASE_WORKFLOW.replace("__UPSTREAM_REPO__", upstream_repo);
    write_workflow(
        &workflows_dir.join("mirror-upstream-release.yml"),
        &workflow,
    )?;
    Utils::success(format!(
        "Installed upstream release mirror workflow for {upstream_repo}."
    ));
    Ok(())
}

fn install(source_repo: &str) -> Result<(), KamError> {
    let source_repo = normalize_source_repo(source_repo)?;
    let repo = current_repo().ok_or_else(|| {
        KamError::CommandFailed(
            "Could not determine current GitHub repository from GITHUB_REPOSITORY or git origin."
                .to_string(),
        )
    })?;
    let workflows_dir = Path::new(".github").join("workflows");

    if source_repo.eq_ignore_ascii_case(&repo) {
        install_standard_workflows(&workflows_dir)
    } else {
        install_mirror_workflow(&workflows_dir, &source_repo)
    }
}

/// Run the workflow command.
///
/// # Errors
/// Returns `KamError` if repository detection fails or files cannot be written.
pub fn run(args: &WorkflowArgs) -> Result<(), KamError> {
    match &args.command {
        WorkflowCommand::Install { source_repo } => install(source_repo),
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_github_repo;

    #[test]
    fn normalizes_common_github_repository_specs() {
        let cases = [
            ("LIghtJUNction/MagicNet", "LIghtJUNction/MagicNet"),
            (
                "https://github.com/LIghtJUNction/MagicNet.git",
                "LIghtJUNction/MagicNet",
            ),
            (
                "git@github.com:LIghtJUNction/MagicNet.git",
                "LIghtJUNction/MagicNet",
            ),
            (
                "ssh://git@github.com/LIghtJUNction/MagicNet.git",
                "LIghtJUNction/MagicNet",
            ),
            (
                "https://github.com/LIghtJUNction/MagicNet.git/",
                "LIghtJUNction/MagicNet",
            ),
        ];

        for (input, expected) in cases {
            assert_eq!(normalize_github_repo(input).as_deref(), Some(expected));
        }
    }
}
