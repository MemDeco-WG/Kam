# Sync kam.toml [prop] section to module.prop
# This hook generates module.prop from kam.toml before the build process starts.

# Source common utilities
$utilsPath = Join-Path $env:KAM_HOOKS_ROOT "lib" "utils.ps1"
if (Test-Path $utilsPath) {
    . $utilsPath
} else {
    Write-Host "Warning: utils.ps1 not found at $utilsPath" -ForegroundColor Yellow
    function Log-Info($msg) { Write-Host "[INFO] $msg" -ForegroundColor Cyan }
    function Log-Warn($msg) { Write-Host "[WARN] $msg" -ForegroundColor Yellow }
    function Log-Error($msg) { Write-Host "[ERROR] $msg" -ForegroundColor Red }
    function Log-Success($msg) { Write-Host "[SUCCESS] $msg" -ForegroundColor Green }
}

Log-Info "Syncing kam.toml [prop] section to module.prop..."

# Check if required KAM environment variables are set
if (-not $env:KAM_MODULE_ID -or -not $env:KAM_MODULE_VERSION -or -not $env:KAM_MODULE_VERSION_CODE) {
    Log-Error "Required KAM_MODULE_* environment variables are not set"
    exit 1
}

# Determine module.prop location
# For standard Magisk/KernelSU modules, it should be in src/<module_id>/module.prop
$modulePropPath = Join-Path $env:KAM_PROJECT_ROOT "src" $env:KAM_MODULE_ID "module.prop"

# Check if the directory exists
$moduleDir = Split-Path -Parent $modulePropPath
if (-not (Test-Path $moduleDir)) {
    Log-Warn "Module directory does not exist: $moduleDir"
    Log-Info "Attempting to create directory..."
    try {
        New-Item -ItemType Directory -Path $moduleDir -Force | Out-Null
    } catch {
        Log-Error "Failed to create directory: $moduleDir"
        Log-Error $_.Exception.Message
        exit 1
    }
}

# Generate module.prop content from KAM environment variables
Log-Info "Generating module.prop at: $modulePropPath"

$propContent = @"
id=$($env:KAM_MODULE_ID)
name=$($env:KAM_MODULE_NAME)
version=$($env:KAM_MODULE_VERSION)
versionCode=$($env:KAM_MODULE_VERSION_CODE)
author=$($env:KAM_MODULE_AUTHOR)
description=$($env:KAM_MODULE_DESCRIPTION)
"@

# Add updateJson if set (optional field)
if ($env:KAM_MODULE_UPDATE_JSON) {
    $propContent += "`nupdateJson=$($env:KAM_MODULE_UPDATE_JSON)"
}

# Write the content to module.prop
try {
    Set-Content -Path $modulePropPath -Value $propContent -Encoding UTF8 -NoNewline
    # Ensure Unix line endings (LF)
    $content = Get-Content -Path $modulePropPath -Raw
    $content = $content -replace "`r`n", "`n"
    [System.IO.File]::WriteAllText($modulePropPath, $content)
} catch {
    Log-Error "Failed to write module.prop: $_"
    exit 1
}

# Verify the file was created successfully
if (Test-Path $modulePropPath) {
    Log-Success "module.prop synced successfully"

    # Show content if debug mode is enabled
    if ($env:KAM_DEBUG -eq "1") {
        Log-Info "module.prop content:"
        Get-Content $modulePropPath | ForEach-Object {
            Write-Host "  $_" -ForegroundColor Gray
        }
    }
} else {
    Log-Error "Failed to create module.prop at: $modulePropPath"
    exit 1
}

Log-Info "kam.toml → module.prop sync completed"
