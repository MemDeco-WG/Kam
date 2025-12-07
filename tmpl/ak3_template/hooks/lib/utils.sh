#!/bin/sh
# Common utility functions for Kam hooks

# Colors
RED=$(printf '\033[0;31m')
GREEN=$(printf '\033[0;32m')
YELLOW=$(printf '\033[1;33m')
BLUE=$(printf '\033[0;34m')
NC=$(printf '\033[0m') # No Color

log_info() {
    printf "${BLUE}[INFO]${NC} %s\n" "$1"
}

log_success() {
    printf "${GREEN}[SUCCESS]${NC} %s\n" "$1"
}

log_warn() {
    printf "${YELLOW}[WARN]${NC} %s\n" "$1"
}

log_error() {
    printf "${RED}[ERROR]${NC} %s\n" "$1"
}

# Check if a command exists
require_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        log_error "Command '$1' is required but not found."
        exit 1
    fi
}

# Check if a variable is set
require_env() {
    var_name="$1"
    eval value=\$$var_name
    if [ -z "$value" ]; then
        log_error "Environment variable '$var_name' is not set."
        exit 1
    fi
}

# Magisk-like utility functions

ui_print() {
    printf "  ${NC}• %s${NC}\n" "$1"
}

abort() {
    printf "  ${RED}! %s${NC}\n" "$1"
    exit 1
}

set_perm() {
    target="$1"
    owner="$2"
    group="$3"
    permission="$4"
    context="$5"

    if [ -z "$context" ]; then
        context="u:object_r:system_file:s0"
    fi

    # Attempt chown/chcon but ignore failures on host build environments
    chown "$owner.$group" "$target" >/dev/null 2>&1 || true
    chmod "$permission" "$target"
    chcon "$context" "$target" >/dev/null 2>&1 || true
}

set_perm_recursive() {
    target="$1"
    owner="$2"
    group="$3"
    dpermission="$4"
    fpermission="$5"
    context="$6"

    if [ -z "$context" ]; then
        context="u:object_r:system_file:s0"
    fi

    find "$target" -type d | while read -r dir; do
        set_perm "$dir" "$owner" "$group" "$dpermission" "$context"
    done

    find "$target" -type f | while read -r file; do
        set_perm "$file" "$owner" "$group" "$fpermission" "$context"
    done
}
