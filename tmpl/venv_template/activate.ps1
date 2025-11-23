<# PowerShell activation script for Kam venv template #>

# Check if already activated
if ($env:KAM_VENV_ACTIVE) {
    Write-Host "Kam virtual environment is already activated." -ForegroundColor Red
    return
}

# Store original environment
if (-not $env:KAM_OLD_PATH) {
    $env:KAM_OLD_PATH = $env:PATH
}
if (-not $env:KAM_OLD_PROMPT) {
    $env:KAM_OLD_PROMPT = $Host.UI.RawUI.WindowTitle
}

# Determine venv directory
$PSScriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Definition
$env:KAM_VENV_DIR = $PSScriptRoot
$env:PATH = "$PSScriptRoot\bin;$env:PATH"
$env:KAM_VENV_ACTIVE = '1'

# Set custom window title to indicate activation
$Host.UI.RawUI.WindowTitle = "(kam-{{prop.id}}) $($Host.UI.RawUI.WindowTitle)"

Write-Host "Kam virtual environment activated ({{prop.id}})" -ForegroundColor Green
Write-Host "Venv location: $PSScriptRoot" -ForegroundColor Cyan
Write-Host "Run 'deactivate' to exit" -ForegroundColor Green

function global:deactivate {
    # Check if environment is activated
    if (-not $env:KAM_VENV_ACTIVE) {
        Write-Host "Kam virtual environment is not activated." -ForegroundColor Red
        return
    }
    
    # Restore PATH
    if (Test-Path env:KAM_OLD_PATH) {
        $env:PATH = $env:KAM_OLD_PATH
        Remove-Item env:KAM_OLD_PATH
    }
    
    # Restore window title
    if (Test-Path env:KAM_OLD_PROMPT) {
        $Host.UI.RawUI.WindowTitle = $env:KAM_OLD_PROMPT
        Remove-Item env:KAM_OLD_PROMPT
    }
    
    # Clear environment variables
    if (Test-Path env:KAM_VENV_ACTIVE) {
        Remove-Item env:KAM_VENV_ACTIVE
    }
    if (Test-Path env:KAM_VENV_DIR) {
        Remove-Item env:KAM_VENV_DIR
    }
    
    # Remove the deactivate function
    Remove-Item function:deactivate -ErrorAction SilentlyContinue
    
    Write-Host "Kam virtual environment deactivated." -ForegroundColor Green
}
