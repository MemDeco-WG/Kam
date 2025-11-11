#!/bin/sh
# Kam Virtual Environment Activation Script
# Type: Runtime

# Store old PATH
KAM_OLD_PATH="$PATH"
export KAM_OLD_PATH

# Store old prompt
KAM_OLD_PS1="$PS1"
export KAM_OLD_PS1

# Add venv bin to PATH
PATH=".\.kam-venv\bin:$PATH"
export PATH

# Update prompt
PS1="(kam-venv) $PS1"
export PS1

# Set environment marker
KAM_VENV_ACTIVE="1"
export KAM_VENV_ACTIVE

# Define deactivate function
deactivate() {
    # Restore PATH
    if [ -n "${KAM_OLD_PATH:-}" ]; then
        PATH="$KAM_OLD_PATH"
        export PATH
        unset KAM_OLD_PATH
    fi
    
    # Restore prompt
    if [ -n "${KAM_OLD_PS1:-}" ]; then
        PS1="$KAM_OLD_PS1"
        export PS1
        unset KAM_OLD_PS1
    fi
    
    # Unset environment marker
    unset KAM_VENV_ACTIVE
    
    # Remove deactivate function
    unset -f deactivate
}

echo "Kam virtual environment activated (runtime mode)"
echo "Run 'deactivate' to exit"
