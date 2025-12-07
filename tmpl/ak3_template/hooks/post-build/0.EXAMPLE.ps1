# Example post-build hook script (PowerShell)
# This script runs after the build process completes.

# Source common utilities
$UtilsPath = Join-Path $Env:KAM_HOOKS_ROOT "lib\utils.ps1"

if (Test-Path $UtilsPath) {
    . $UtilsPath
} else {
    Write-Host "Warning: utils.ps1 not found at $UtilsPath" -ForegroundColor Yellow
    function Log-Info { param([string]$m) Write-Host "[INFO] $m" }
}

Log-Info "Running tmpl post-build hook..."
Log-Info "Module built successfully."

# Add your post-build logic here (e.g., uploading artifacts, notifying services)
