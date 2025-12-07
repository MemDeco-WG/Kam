# WebUI Build Hook (PowerShell)

# Source common utilities
$UtilsPath = Join-Path $Env:KAM_HOOKS_ROOT "lib\utils.ps1"
if (Test-Path $UtilsPath) {
    . $UtilsPath
} else {
    Write-Host "Warning: utils.ps1 not found at $UtilsPath" -ForegroundColor Yellow
    function Log-Info { param([string]$m) Write-Host "[INFO] $m" -ForegroundColor Blue }
    function Log-Error { param([string]$m) Write-Host "[ERROR] $m" -ForegroundColor Red }
    function Log-Success { param([string]$m) Write-Host "[SUCCESS] $m" -ForegroundColor Green }
}

Log-Info "Building WebUI for module: $Env:KAM_MODULE_ID"

$WebUiDir = Join-Path $Env:KAM_PROJECT_ROOT "ModuleWebUI"
$BuildScript = Join-Path $WebUiDir "build.sh"

if (-not (Test-Path $WebUiDir)) {
    Log-Error "ModuleWebUI directory not found at $WebUiDir"
    exit 1
}

# Check if build.ps1 exists, prefer that on Windows if available
$BuildScriptPs1 = Join-Path $WebUiDir "build.ps1"

if (Test-Path $BuildScriptPs1) {
    Log-Info "Found build.ps1, executing..."
    Push-Location $WebUiDir
    try {
        # Execute build.ps1 with Module ID argument
        & $BuildScriptPs1 $Env:KAM_MODULE_ID
        if ($LASTEXITCODE -ne 0) { throw "Build failed with exit code $LASTEXITCODE" }
    } catch {
        Pop-Location
        Log-Error "WebUI build failed: $_"
        exit 1
    }
    Pop-Location
} elseif (Test-Path $BuildScript) {
    Log-Info "Executing build.sh via bash..."
    Push-Location $WebUiDir
    try {
        # Try to find bash
        $Bash = Get-Command "bash" -ErrorAction SilentlyContinue
        if ($Bash) {
            # Use bash to run the script
            & $Bash.Source -c "./build.sh $Env:KAM_MODULE_ID"
            if ($LASTEXITCODE -ne 0) { throw "Build failed with exit code $LASTEXITCODE" }
        } else {
            # Try sh
            $Sh = Get-Command "sh" -ErrorAction SilentlyContinue
            if ($Sh) {
                 & $Sh.Source -c "./build.sh $Env:KAM_MODULE_ID"
                 if ($LASTEXITCODE -ne 0) { throw "Build failed with exit code $LASTEXITCODE" }
            } else {
                throw "bash/sh not found, cannot execute build.sh on Windows. Please install Git Bash or WSL."
            }
        }
    } catch {
        Pop-Location
        Log-Error "WebUI build failed: $_"
        exit 1
    }
    Pop-Location
} else {
    Log-Error "No build script found (checked build.ps1 and build.sh)"
    exit 1
}

# Move dist to webroot
$DistDir = Join-Path $WebUiDir "dist"
$TargetWebroot = $Env:KAM_WEB_ROOT

if (-not (Test-Path $DistDir)) {
    Log-Error "Dist directory not found at $DistDir after build"
    exit 1
}

Log-Info "Installing WebUI to $TargetWebroot"

# Remove existing webroot if it exists
if (Test-Path $TargetWebroot) {
    Remove-Item -Path $TargetWebroot -Recurse -Force
}

# Ensure parent directory exists
$ParentDir = Split-Path -Parent $TargetWebroot
if (-not (Test-Path $ParentDir)) {
    New-Item -Path $ParentDir -ItemType Directory -Force | Out-Null
}

# Move dist to webroot
try {
    Move-Item -Path $DistDir -Destination $TargetWebroot -Force
    Log-Success "WebUI built and installed successfully"
} catch {
    Log-Error "Failed to move WebUI artifacts: $_"
    exit 1
}
