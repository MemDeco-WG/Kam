#!/bin/sh
# POSIX-compatible activation wrapper
VENV_DIR="$(cd "$(dirname "$0")" && pwd)"
export KAM_OLD_PATH="$PATH"
export PATH="$VENV_DIR/bin:$PATH"
export KAM_VENV_ACTIVE=1
echo "Kam virtual environment activated ({{id}})"
echo "Run 'deactivate' to exit"

deactivate() {
    if [ -n "${KAM_OLD_PATH:-}" ]; then
        export PATH="$KAM_OLD_PATH"
        unset KAM_OLD_PATH
    fi
    unset KAM_VENV_ACTIVE
    unset -f deactivate
}
