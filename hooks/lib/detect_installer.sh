#!/bin/bash
set -Eeuo pipefail

info() { printf '[INSTALLER-INFO] %s\n' "$*"; }
error() { printf '[INSTALLER-ERROR] %s\n' "$*" >&2; }

# Decide whether to use sudo (if not root)
declare -a SUDO_CMD=()
if [ "$(id -u)" -ne 0 ]; then
    if command -v sudo >/dev/null 2>&1; then
        SUDO_CMD=(sudo)
    else
        error "Not running as root and sudo not available. Please run as root or install sudo."
        exit 1
    fi
fi

# Vars to be set by detection
installer=""              # string, e.g. "apt-get"
declare -a installer_cmd=()   # array, e.g. ( sudo env ... apt-get -y ... install )
declare -a update_cmd=()      # array, e.g. ( sudo apt-get update -qq )

# Configure arrays for a given manager name
setup_pkg_manager() {
    local m="$1"
    case "$m" in
        apt-get)
            installer="apt-get"
            installer_cmd=( "${SUDO_CMD[@]}" env DEBIAN_FRONTEND=noninteractive apt-get -y --no-install-recommends install -o Dpkg::Options::=--force-confdef -o Dpkg::Options::=--force-confold )
            update_cmd=( "${SUDO_CMD[@]}" apt-get update -qq )
            ;;
        apt)
            installer="apt"
            installer_cmd=( "${SUDO_CMD[@]}" env DEBIAN_FRONTEND=noninteractive apt -y install --no-install-recommends )
            update_cmd=( "${SUDO_CMD[@]}" apt update -qq )
            ;;
        dnf)
            installer="dnf"
            installer_cmd=( "${SUDO_CMD[@]}" dnf -y install )
            update_cmd=( "${SUDO_CMD[@]}" dnf makecache -q )
            ;;
        yum)
            installer="yum"
            installer_cmd=( "${SUDO_CMD[@]}" yum -y install )
            update_cmd=( "${SUDO_CMD[@]}" yum makecache -q )
            ;;
        pacman)
            installer="pacman"
            installer_cmd=( "${SUDO_CMD[@]}" pacman -S --noconfirm --needed )
            update_cmd=( "${SUDO_CMD[@]}" pacman -Sy --noconfirm )
            ;;
        apk)
            installer="apk"
            installer_cmd=( "${SUDO_CMD[@]}" apk add --no-cache )
            update_cmd=( "${SUDO_CMD[@]}" apk update )
            ;;
        zypper)
            installer="zypper"
            installer_cmd=( "${SUDO_CMD[@]}" zypper -n in )
            update_cmd=( "${SUDO_CMD[@]}" zypper refresh -s )
            ;;
        *)
            error "setup_pkg_manager: unsupported manager $m"
            return 1
            ;;
    esac
    return 0
}

# Detect the package manager
detect_installer() {
    # Optional override by the environment (PREFERRED_INSTALLER)
    if [ -n "${PREFERRED_INSTALLER:-}" ]; then
        if command -v "${PREFERRED_INSTALLER}" >/dev/null 2>&1; then
            setup_pkg_manager "${PREFERRED_INSTALLER}" && return 0
        else
            error "PREFERRED_INSTALLER (${PREFERRED_INSTALLER}) is not present; falling back to auto-detect"
        fi
    fi

    # If ANDROID_BUILD_TOP is defined, prefer apt-get if present (keeps older behaviour)
    if [ -n "${ANDROID_BUILD_TOP:-}" ]; then
        if command -v apt-get >/dev/null 2>&1; then
            setup_pkg_manager apt-get && return 0
        fi
    fi

    # Auto-detect by checking availability in a sensible order
    if command -v apt-get >/dev/null 2>&1; then
        setup_pkg_manager apt-get && return 0
    elif command -v apt >/dev/null 2>&1; then
        setup_pkg_manager apt && return 0
    elif command -v dnf >/dev/null 2>&1; then
        setup_pkg_manager dnf && return 0
    elif command -v yum >/dev/null 2>&1; then
        setup_pkg_manager yum && return 0
    elif command -v pacman >/dev/null 2>&1; then
        setup_pkg_manager pacman && return 0
    elif command -v apk >/dev/null 2>&1; then
        setup_pkg_manager apk && return 0
    elif command -v zypper >/dev/null 2>&1; then
        setup_pkg_manager zypper && return 0
    fi

    error "No supported package manager found. Supported: apt-get, apt, dnf, yum, pacman, apk, zypper"
    return 1
}

# Run detection now (or user can call detect_installer explicitly)
if ! detect_installer; then
    exit 1
fi
info "Using installer: ${installer}"

# Update package index (unless SKIP_UPDATE=true)
pkg_update() {
    if [ "${SKIP_UPDATE:-false}" = "true" ]; then
        info "Skipping package index update (SKIP_UPDATE=true)"
        return 0
    fi
    info "Updating package database using ${installer}"
    # run update command
    "${update_cmd[@]}"
}

# Install packages (pkg_install pacman|apk libs...)
# When no packages are provided, it prints a message and returns error.
pkg_install() {
    if [ $# -eq 0 ]; then
        error "pkg_install: no package name provided"
        return 1
    fi

    pkg_update
    info "Installing: $* (via ${installer})"
    "${installer_cmd[@]}" "$@"
}

# Uninstall removed packages (optional helper)
pkg_remove() {
    if [ $# -eq 0 ]; then
        error "pkg_remove: no package name provided"
        return 1
    fi

    case "$installer" in
        apt-get|apt)
            "${SUDO_CMD[@]}" apt-get -y remove --purge "$@"
            ;;
        dnf|yum)
            "${SUDO_CMD[@]}" $installer -y remove "$@"
            ;;
        pacman)
            "${SUDO_CMD[@]}" pacman -R --noconfirm "$@"
            ;;
        apk)
            "${SUDO_CMD[@]}" apk del "$@"
            ;;
        zypper)
            "${SUDO_CMD[@]}" zypper -n rm "$@"
            ;;
        *)
            error "pkg_remove: unsupported installer $installer"
            return 1
            ;;
    esac
}

info "Available packages:"
info "apt-get, apt, dnf, yum, pacman, apk, zypper"

info "Available commands:"
info "pkg_install, pkg_remove, pkg_update"

# Example: call `pkg_install curl ca-certificates` in your script as needed.
# End of script
