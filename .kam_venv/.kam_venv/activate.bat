@echo off
REM Kam venv activation (template)

REM Check if already activated
if defined KAM_VENV_ACTIVE (
    echo Kam virtual environment is already activated.
    goto :eof
)

SETLOCAL
if defined KAM_OLD_PATH (
    rem already set
) else (
    set "KAM_OLD_PATH=%PATH%"
)
if defined KAM_OLD_PROMPT (
    rem already set
) else (
    set "KAM_OLD_PROMPT=%PROMPT%"
)
set "VENV_DIR=%~dp0"
set "PATH=%VENV_DIR%bin;%PATH%"
set "PROMPT=(kam-Kam) %PROMPT%"
set "KAM_VENV_ACTIVE=1"
echo Kam virtual environment activated (Kam)
echo Run 'deactivate' to exit

:deactivate
if defined KAM_OLD_PATH (
    set "PATH=%KAM_OLD_PATH%"
    set KAM_OLD_PATH=
)
if defined KAM_OLD_PROMPT (
    set "PROMPT=%KAM_OLD_PROMPT%"
    set KAM_OLD_PROMPT=
)
set KAM_VENV_ACTIVE=
echo Kam virtual environment deactivated.
endlocal & goto :eof
