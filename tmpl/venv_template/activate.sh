#!/bin/sh
# POSIX-compatible activation script

# Check if already activated
if [ -n "${KAM_VENV_ACTIVE:-}" ]; then
    echo "Kam virtual environment is already activated." >&2
    return 1 2>/dev/null || exit 1
fi

# Store original environment
export KAM_OLD_PATH="$PATH"
export KAM_OLD_PS1="${PS1:-}"
export KAM_OLD_PS1_SET_AT_ACTIVATION="$KAM_OLD_PS1"

# Determine venv directory
VENV_DIR="$(cd "$(dirname "$0")" && pwd)"
export KAM_VENV_DIR="$VENV_DIR"
export PATH="$VENV_DIR/bin:$PATH"
export KAM_VENV_ACTIVE=1

# Set custom prompt
export PS1="(kam-{{prop.id}}) $PS1"

echo "Kam virtual environment activated ({{prop.id}})"
echo "Venv location: $VENV_DIR"
echo "Run 'deactivate' to exit"

# Define deactivation function
deactivate() {
    # Only run if in activated state
    if [ -z "${KAM_VENV_ACTIVE:-}" ]; then
        echo "Kam virtual environment is not activated." >&2
        return 1
    fi

    # Restore PATH
    if [ -n "${KAM_OLD_PATH:-}" ]; then
        export PATH="$KAM_OLD_PATH"
        unset KAM_OLD_PATH
    fi

    # Restore PS1
    if [ -n "${KAM_OLD_PS1_SET_AT_ACTIVATION:-}" ]; then
        export PS1="$KAM_OLD_PS1_SET_AT_ACTIVATION"
        unset KAM_OLD_PS1_SET_AT_ACTIVATION
    else
        unset PS1
    fi

    # Clear environment variables
    unset KAM_VENV_ACTIVE
    unset KAM_VENV_DIR
    unset -f deactivate 2>/dev/null || true
    
    echo "Kam virtual environment deactivated."
}
