# Example pre-build hook script (PowerShell)
# This script runs before the build process starts.

$UtilsPath = Join-Path $Env:KAM_HOOKS_ROOT "lib\utils.ps1"

if (Test-Path $UtilsPath) {
    . $UtilsPath
} else {
    Write-Host "Warning: utils.ps1 not found at $UtilsPath" -ForegroundColor Yellow
    function Log-Info { param([string]$m) Write-Host "[INFO] $m" }
}

Log-Info "Running tmpl pre-build hook..."
Log-Info "Building module: $Env:KAM_MODULE_ID v$Env:KAM_MODULE_VERSION"

# Add your pre-build logic here (e.g., downloading assets, checking environment)
