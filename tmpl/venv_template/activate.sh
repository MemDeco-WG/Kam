#!/bin/sh
# POSIX-compatible activation wrapper

# Check if already activated
if [ -n "$KAM_VENV_ACTIVE" ]; then
    echo "Kam virtual environment is already activated."
    return 1 2>/dev/null || exit 1
fi

VENV_DIR="$(cd "$(dirname "$0")" && pwd)"
export KAM_OLD_PATH="$PATH"
export PATH="$VENV_DIR/bin:$PATH"
export KAM_VENV_ACTIVE=1
export KAM_OLD_PS1="${PS1:-}"
export PS1="(kam-{{id}}) $PS1"

echo "Kam virtual environment activated ({{id}})"
echo "Run 'deactivate' to exit"

deactivate() {
    if [ -n "${KAM_OLD_PATH:-}" ]; then
        export PATH="$KAM_OLD_PATH"
        unset KAM_OLD_PATH
    fi
    if [ -n "${KAM_OLD_PS1:-}" ]; then
        export PS1="$KAM_OLD_PS1"
        unset KAM_OLD_PS1
    fi
    unset KAM_VENV_ACTIVE
    unset -f deactivate
    echo "Kam virtual environment deactivated."
}
