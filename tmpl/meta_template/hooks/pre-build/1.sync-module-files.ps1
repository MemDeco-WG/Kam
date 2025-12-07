# Sync kam.toml to module.prop and update.json
# This hook generates:
# - module.prop in module directory ($env:KAM_MODULE_ROOT/module.prop)
# - update.json in project root ($env:KAM_PROJECT_ROOT/update.json)

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

Log-Info "Syncing kam.toml to module.prop and update.json..."

# Check if required KAM environment variables are set
if (-not $env:KAM_MODULE_ID -or -not $env:KAM_MODULE_VERSION -or -not $env:KAM_MODULE_VERSION_CODE) {
    Log-Error "Required KAM_MODULE_* environment variables are not set"
    exit 1
}

# Skip template modules (modules with id ending in _template)
if ($env:KAM_MODULE_ID -match '_template$') {
    Log-Info "Skipping template module: $($env:KAM_MODULE_ID)"
    exit 0
}

# Determine file paths
# module.prop goes to module directory
$modulePropPath = Join-Path $env:KAM_MODULE_ROOT "module.prop"
# update.json goes to project root directory
$updateJsonPath = Join-Path $env:KAM_PROJECT_ROOT "update.json"

# Check if the module root directory exists
if (-not (Test-Path $env:KAM_MODULE_ROOT)) {
    Log-Warn "Module directory does not exist: $($env:KAM_MODULE_ROOT)"
    Log-Info "Attempting to create directory..."
    try {
        New-Item -ItemType Directory -Path $env:KAM_MODULE_ROOT -Force | Out-Null
    } catch {
        Log-Error "Failed to create directory: $($env:KAM_MODULE_ROOT)"
        Log-Error $_.Exception.Message
        exit 1
    }
}

###########################################
# Sync module.prop
###########################################
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

###########################################
# Sync update.json
###########################################
Log-Info "Generating update.json at: $updateJsonPath"

# Try to read kam.toml to extract repository and changelog info
$kamTomlPath = Join-Path $env:KAM_PROJECT_ROOT "kam.toml"
$repositoryUrl = ""
$changelogUrl = ""

if (Test-Path $kamTomlPath) {
    try {
        $tomlContent = Get-Content $kamTomlPath -Raw

        # Extract repository URL from [mmrl.repo] section
        if ($tomlContent -match '(?ms)\[mmrl\.repo\].*?repository\s*=\s*"([^"]*)"') {
            $repositoryUrl = $matches[1]
            # Remove template variables
            $repositoryUrl = $repositoryUrl -replace '\{\{[^}]*\}\}', ''
            $repositoryUrl = $repositoryUrl -replace '\{%[^%]*%\}', ''
            $repositoryUrl = $repositoryUrl.Trim()
        }

        # Try homepage if repository is not found
        if (-not $repositoryUrl -and $tomlContent -match '(?ms)\[mmrl\.repo\].*?homepage\s*=\s*"([^"]*)"') {
            $repositoryUrl = $matches[1]
            $repositoryUrl = $repositoryUrl -replace '\{\{[^}]*\}\}', ''
            $repositoryUrl = $repositoryUrl -replace '\{%[^%]*%\}', ''
            $repositoryUrl = $repositoryUrl.Trim()
        }

        # Extract changelog URL from [mmrl.repo] section
        if ($tomlContent -match '(?ms)\[mmrl\.repo\].*?changelog\s*=\s*"([^"]*)"') {
            $changelogUrl = $matches[1]
            $changelogUrl = $changelogUrl -replace '\{\{[^}]*\}\}', ''
            $changelogUrl = $changelogUrl -replace '\{%[^%]*%\}', ''
            $changelogUrl = $changelogUrl.Trim()
        }
    } catch {
        Log-Warn "Failed to parse kam.toml: $_"
    }
}

# Fallback to environment variables if available
if ($env:KAM_MODULE_REPOSITORY) {
    $repositoryUrl = $env:KAM_MODULE_REPOSITORY
}

if ($env:KAM_MODULE_CHANGELOG) {
    $changelogUrl = $env:KAM_MODULE_CHANGELOG
}

# Determine zipUrl
if ($repositoryUrl -and $repositoryUrl -ne "") {
    # If repository URL is from GitHub, construct the release URL
    if ($repositoryUrl -match 'github\.com') {
        $zipUrl = "$repositoryUrl/releases/latest/download/$($env:KAM_MODULE_ID).zip"
    } else {
        # For other platforms, use a generic pattern
        $zipUrl = "$repositoryUrl/releases/latest/download/$($env:KAM_MODULE_ID).zip"
    }
} else {
    # Default fallback
    $zipUrl = "https://github.com/user/repo/releases/latest/download/$($env:KAM_MODULE_ID).zip"
}

# Determine changelog URL
if ($changelogUrl -and $changelogUrl -ne "") {
    # Use the changelog URL from kam.toml
    $finalChangelogUrl = $changelogUrl
} elseif ($repositoryUrl -and $repositoryUrl -ne "") {
    # Try to construct changelog URL from repository
    if ($repositoryUrl -match 'github\.com') {
        # Convert https://github.com/user/repo to raw URL
        $finalChangelogUrl = "$repositoryUrl/raw/main/CHANGELOG.md"
    } else {
        # For other platforms, try a similar pattern
        $finalChangelogUrl = "$repositoryUrl/CHANGELOG.md"
    }
} else {
    # Default fallback
    $finalChangelogUrl = "https://raw.githubusercontent.com/user/repo/main/CHANGELOG.md"
}

# Generate update.json with proper JSON formatting
$updateJsonContent = @"
{
  "version": "$($env:KAM_MODULE_VERSION)",
  "versionCode": $($env:KAM_MODULE_VERSION_CODE),
  "zipUrl": "$zipUrl",
  "changelog": "$finalChangelogUrl"
}
"@

# Write the content to update.json
try {
    Set-Content -Path $updateJsonPath -Value $updateJsonContent -Encoding UTF8 -NoNewline
    # Ensure Unix line endings (LF)
    $content = Get-Content -Path $updateJsonPath -Raw
    $content = $content -replace "`r`n", "`n"
    [System.IO.File]::WriteAllText($updateJsonPath, $content)
} catch {
    Log-Error "Failed to write update.json: $_"
    exit 1
}

# Verify the file was created successfully
if (Test-Path $updateJsonPath) {
    Log-Success "update.json synced successfully"

    # Show content if debug mode is enabled
    if ($env:KAM_DEBUG -eq "1") {
        Log-Info "update.json content:"
        Get-Content $updateJsonPath | ForEach-Object {
            Write-Host "  $_" -ForegroundColor Gray
        }
    }
} else {
    Log-Error "Failed to create update.json at: $updateJsonPath"
    exit 1
}

Log-Success "kam.toml → module.prop & update.json sync completed"
