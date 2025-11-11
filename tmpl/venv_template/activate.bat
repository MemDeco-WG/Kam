@echo off
REM Kam venv activation (template)
SETLOCAL
if defined KAM_OLD_PATH (
    rem already set
) else (
    set "KAM_OLD_PATH=%PATH%"
)
set "VENV_DIR=%~dp0"
set "PATH=%VENV_DIR%bin;%PATH%"
set "KAM_VENV_ACTIVE=1"
echo Kam virtual environment activated ({{id}})
echo Run 'deactivate' to exit

:deactivate
if defined KAM_OLD_PATH (
    set "PATH=%KAM_OLD_PATH%"
    set KAM_OLD_PATH=
)
set KAM_VENV_ACTIVE=
endlocal & goto :eof
