# Common utility functions for Kam hooks (PowerShell)

# Colors
$ColorRed = "Red"
$ColorGreen = "Green"
$ColorYellow = "Yellow"
$ColorBlue = "Blue"

function Log-Info {
    param([string]$Message)
    Write-Host "[INFO] $Message" -ForegroundColor $ColorBlue
}

function Log-Success {
    param([string]$Message)
    Write-Host "[SUCCESS] $Message" -ForegroundColor $ColorGreen
}

function Log-Warn {
    param([string]$Message)
    Write-Host "[WARN] $Message" -ForegroundColor $ColorYellow
}

function Log-Error {
    param([string]$Message)
    Write-Host "[ERROR] $Message" -ForegroundColor $ColorRed
}

function Fail {
    param([string]$Message)
    Log-Error $Message
    exit 1
}

# Check if a command exists
function Require-Command {
    param([string]$CommandName)
    if (-not (Get-Command $CommandName -ErrorAction SilentlyContinue)) {
        Log-Error "Command '$CommandName' is required but not found."
        exit 1
    }
}

# Check if a variable is set
function Require-Env {
    param([string]$VarName)
    if (-not (Test-Path "Env:\$VarName")) {
        Log-Error "Environment variable '$VarName' is not set."
        exit 1
    }
    $val = Get-Content "Env:\$VarName"
    if ([string]::IsNullOrWhiteSpace($val)) {
        Log-Error "Environment variable '$VarName' is empty."
        exit 1
    }
}

# Magisk-like utility functions (Simulated for Windows/Cross-platform context)

function Ui-Print {
    param([string]$Message)
    Write-Host "  • $Message"
}

function Abort {
    param([string]$Message)
    Write-Host "  ! $Message" -ForegroundColor $ColorRed
    exit 1
}

function Set-Perm {
    param(
        [string]$Target,
        [string]$Owner,
        [string]$Group,
        [string]$Permission,
        [string]$Context = "u:object_r:system_file:s0"
    )
    # On Windows, we can't really set Linux permissions/contexts directly.
    # This is mostly a stub for compatibility or if running in a specific environment.
    # We could log it for debugging.
    # Write-Host "Set-Perm: $Target $Owner $Group $Permission $Context" -ForegroundColor DarkGray
}

function Set-Perm-Recursive {
    param(
        [string]$Target,
        [string]$Owner,
        [string]$Group,
        [string]$DirPermission,
        [string]$FilePermission,
        [string]$Context = "u:object_r:system_file:s0"
    )

    if (Test-Path $Target) {
        Get-ChildItem -Path $Target -Recurse | ForEach-Object {
            if ($_.PSIsContainer) {
                Set-Perm -Target $_.FullName -Owner $Owner -Group $Group -Permission $DirPermission -Context $Context
            } else {
                Set-Perm -Target $_.FullName -Owner $Owner -Group $Group -Permission $FilePermission -Context $Context
            }
        }
    }
}
