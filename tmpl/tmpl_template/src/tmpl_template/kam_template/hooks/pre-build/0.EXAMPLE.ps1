# Example pre-build hook script (PowerShell)
# This script runs before the build process starts.

$UtilsPath = Join-Path $Env:KAM_HOOKS_ROOT "lib\utils.ps1"

if (Test-Path $UtilsPath) {
    . $UtilsPath
} else {
    Write-Host "Warning: utils.ps1 not found at $UtilsPath" -ForegroundColor Yellow
    function Log-Info { param([string]$m) Write-Host "[INFO] $m" }
    function Log-Warn { param([string]$m) Write-Host "[WARN] $m" -ForegroundColor Yellow }
    function Log-Error { param([string]$m) Write-Host "[ERROR] $m" -ForegroundColor Red }
    function Log-Success { param([string]$m) Write-Host "[SUCCESS] $m" -ForegroundColor Green }
}

Log-Info "Running tmpl pre-build hook..."
Log-Info "Building module: $Env:KAM_MODULE_ID v$Env:KAM_MODULE_VERSION"

# If KAM_DEBUG is enabled, pretty-print environment variables and update the prompt
if ($Env:KAM_DEBUG -eq '1') {
    if (Get-Command Log-Warn -ErrorAction SilentlyContinue) {
        Log-Warn "KAM_DEBUG is enabled — dumping environment variables starting with 'KAM'"
    } else {
        Log-Info "KAM_DEBUG is enabled — dumping environment variables starting with 'KAM'"
    }

    # Print KAM-prefixed environment variables sorted and nicely formatted (Name / Value)
    $kams = @(Get-ChildItem Env: | Where-Object { $_.Name -like 'KAM*' } | Sort-Object Name)
    if ($kams.Count -eq 0) {
        Log-Info "No KAM-prefixed environment variables found."
    } else {
        $kams | ForEach-Object {
            Write-Host ("  {0,-30} = {1}" -f $_.Name, $_.Value)
        }
    }

    # Preserve the original prompt if present, and augment it with KAM debug info
    $OldPrompt = $null
    if (Test-Path function:\prompt) {
        $OldPrompt = (Get-Item function:\prompt).ScriptBlock
    }

    function Prompt {
        $prefix = ""
        if ($Env:KAM_DEBUG -eq '1') {
            $prefix = "[KAM_DEBUG:$($Env:KAM_MODULE_ID)] "
        }
        if ($OldPrompt) {
            return $prefix + (& $OldPrompt)
        } else {
            return "$prefix PS> "
        }
    }
}

# Add your pre-build logic here (e.g., downloading assets, checking environment)
