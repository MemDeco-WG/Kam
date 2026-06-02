#!/usr/bin/env bash
set -euo pipefail

APP_NAME="kam"
REPO_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
CARGO_BIN="$CARGO_HOME/bin"
RUSTUP_URL="https://sh.rustup.rs"

log() {
  printf '[kam-install] %s\n' "$*"
}

warn() {
  printf '[kam-install] WARN: %s\n' "$*" >&2
}

die() {
  printf '[kam-install] ERROR: %s\n' "$*" >&2
  exit 1
}

has_cmd() {
  command -v "$1" >/dev/null 2>&1
}

detect_os() {
  case "${OSTYPE:-}" in
    darwin*) printf '%s\n' "macos" ;;
    msys* | cygwin* | win32*) printf '%s\n' "windows" ;;
    linux-android*) printf '%s\n' "termux" ;;
    linux*)
      if [ -n "${TERMUX_VERSION:-}" ] || [ -d "/data/data/com.termux/files/usr" ]; then
        printf '%s\n' "termux"
      else
        printf '%s\n' "linux"
      fi
      ;;
    *) printf '%s\n' "unknown" ;;
  esac
}

ensure_path() {
  case ":$PATH:" in
    *":$CARGO_BIN:"*) ;;
    *)
      export PATH="$CARGO_BIN:$PATH"
      warn "$CARGO_BIN was not in PATH for this shell; exported it for this install run."
      ;;
  esac
}

profile_file() {
  if [ "${OS_KIND:-}" = "windows" ]; then
    printf '%s\n' "$HOME/.bashrc"
  elif [ -n "${ZSH_VERSION:-}" ]; then
    printf '%s\n' "$HOME/.zshrc"
  else
    printf '%s\n' "$HOME/.profile"
  fi
}

persist_path_hint() {
  profile="$(profile_file)"
  marker="export PATH=\"\$HOME/.cargo/bin:\$PATH\""
  if [ -f "$profile" ] && grep -F "$marker" "$profile" >/dev/null 2>&1; then
    return 0
  fi
  warn "Add Cargo to PATH persistently if your next shell cannot find kam:"
  warn "  echo '$marker' >> $profile"
}

ensure_termux_packages() {
  has_cmd pkg || die "Termux detected but pkg was not found."
  missing=""
  for cmd in curl cc git make pkg-config perl; do
    has_cmd "$cmd" || missing="$missing $cmd"
  done
  if [ -n "$missing" ]; then
    log "Installing Termux build dependencies:$missing"
    pkg update
    pkg install -y curl git clang make pkg-config openssl perl
  fi
}

ensure_macos_packages() {
  if ! has_cmd curl; then
    die "curl is required. Install Xcode Command Line Tools or Homebrew curl first."
  fi
  if ! has_cmd cc; then
    log "Installing Xcode Command Line Tools."
    xcode-select --install || true
    die "Re-run this installer after Xcode Command Line Tools finishes installing."
  fi
}

ensure_windows_packages() {
  has_cmd curl || die "curl is required. Run from Git Bash/MSYS2 with curl available."
  has_cmd cc || warn "C compiler not found. If cargo build fails, install MSYS2 mingw-w64 toolchain or Visual Studio Build Tools."
}

ensure_common_tools() {
  has_cmd tar || warn "tar not found; cargo/rustup may need it on some platforms."
  has_cmd git || warn "git not found; installing from this working tree can still work, but some cargo metadata may be limited."
}

install_rustup() {
  has_cmd rustup && has_cmd cargo && return 0
  has_cmd curl || die "curl is required to install rustup."

  log "Installing Rust toolchain with rustup."
  curl --proto '=https' --tlsv1.2 -fsSL "$RUSTUP_URL" | sh -s -- -y --profile minimal
  # shellcheck source=/dev/null
  [ -f "$CARGO_HOME/env" ] && . "$CARGO_HOME/env"
}

ensure_rust() {
  ensure_path
  install_rustup
  ensure_path
  has_cmd cargo || die "cargo is still unavailable after rustup installation."
  has_cmd rustc || die "rustc is still unavailable after rustup installation."
  rustup default stable >/dev/null 2>&1 || true
  log "Rust: $(rustc --version)"
  log "Cargo: $(cargo --version)"
}

install_kam() {
  cd "$REPO_DIR"
  if [ "${KAM_INSTALL_DRY_RUN:-0}" = "1" ]; then
    log "Dry run: skipping cargo install."
    return 0
  fi
  log "Installing kam from $REPO_DIR"
  if [ -f Cargo.lock ]; then
    cargo install --path . --locked
  else
    cargo install --path .
  fi
}

verify_install() {
  ensure_path
  if [ "${KAM_INSTALL_DRY_RUN:-0}" = "1" ]; then
    log "Dry run: skipping kam --version verification."
    persist_path_hint
    return 0
  fi
  has_cmd "$APP_NAME" || die "kam was installed but is not on PATH. Expected $CARGO_BIN in PATH."
  log "Installed: $(kam --version)"
  persist_path_hint
}

main() {
  OS_KIND="$(detect_os)"
  export OS_KIND
  log "Detected platform: $OS_KIND"

  case "$OS_KIND" in
    termux) ensure_termux_packages ;;
    macos) ensure_macos_packages ;;
    windows) ensure_windows_packages ;;
    linux) warn "Linux detected. This installer is focused on macOS, Windows shell environments, and Termux; continuing with generic checks." ;;
    *) die "Unsupported platform: ${OSTYPE:-unknown}" ;;
  esac

  ensure_common_tools
  ensure_rust
  install_kam
  verify_install
}

main "$@"
