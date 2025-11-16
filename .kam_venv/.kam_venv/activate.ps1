<# PowerShell activation script for Kam venv template #>
$PSScriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Definition
$env:KAM_OLD_PATH = $env:PATH
$env:PATH = "$PSScriptRoot\bin;$env:PATH"
$env:KAM_VENV_ACTIVE = '1'
Write-Host "Kam virtual environment activated (Kam)" -ForegroundColor Green
Write-Host "Run 'deactivate' to exit" -ForegroundColor Green

function global:deactivate {
    if (Test-Path env:KAM_OLD_PATH) {
        $env:PATH = $env:KAM_OLD_PATH
        Remove-Item env:KAM_OLD_PATH
    }
    if (Test-Path env:KAM_VENV_ACTIVE) {
        Remove-Item env:KAM_VENV_ACTIVE
    }
    Remove-Item function:deactivate
}
