# Kam Virtual Environment Activation Script (PowerShell)
# Type: Runtime

# Store old PATH
$env:KAM_OLD_PATH = $env:PATH

# Add venv bin to PATH
$env:PATH = ".\.kam-venv\bin;$env:PATH"

# Update prompt
function global:_OLD_KAM_PROMPT {""}
$function:_OLD_KAM_PROMPT = $function:prompt
function global:prompt {
    Write-Host "(kam-venv) " -NoNewline
    & $function:_OLD_KAM_PROMPT
}

# Set environment marker
$env:KAM_VENV_ACTIVE = "1"

# Define deactivate function
function global:deactivate {
    # Restore PATH
    if (Test-Path env:KAM_OLD_PATH) {
        $env:PATH = $env:KAM_OLD_PATH
        Remove-Item env:KAM_OLD_PATH
    }
    
    # Restore prompt
    if (Test-Path function:_OLD_KAM_PROMPT) {
        $function:prompt = $function:_OLD_KAM_PROMPT
        Remove-Item function:_OLD_KAM_PROMPT
    }
    
    # Unset environment marker
    Remove-Item env:KAM_VENV_ACTIVE
    
    # Remove deactivate function
    Remove-Item function:deactivate
}

Write-Host "Kam virtual environment activated (runtime mode)" -ForegroundColor Green
Write-Host "Run 'deactivate' to exit" -ForegroundColor Green
