@echo off
REM Kam venv activation script

REM Check if already activated
if defined KAM_VENV_ACTIVE (
    echo Kam virtual environment is already activated.
    goto :eof
)

REM Store original environment
if not defined KAM_OLD_PATH set "KAM_OLD_PATH=%PATH%"
if not defined KAM_OLD_PROMPT set "KAM_OLD_PROMPT=%PROMPT%"

REM Determine venv directory
set "VENV_DIR=%~dp0"
set "VENV_DIR=%VENV_DIR:~0,-1%" REM Remove trailing slash

REM Activate environment
set "PATH=%VENV_DIR%\bin;%PATH%"
set "PROMPT=(kam-{{prop.id}}) $P$G "
set "KAM_VENV_ACTIVE=1"
set "KAM_VENV_DIR=%VENV_DIR%"

echo Kam virtual environment activated ({{prop.id}})
echo Venv location: %VENV_DIR%
echo Run 'deactivate' to exit

REM Define deactivate function using doskey macro (for command line usage)
doskey /macro deactivation=if defined KAM_VENV_ACTIVE (set "PATH=%KAM_OLD_PATH%" & set "PROMPT=%KAM_OLD_PROMPT%" & set KAM_VENV_ACTIVE= & set KAM_VENV_DIR= & echo Kam virtual environment deactivated. & doskey deactivation= & goto :eof) else (echo Kam virtual environment is not activated. & goto :eof)

echo To deactivate, run: deactivation
goto :eof

REM For batch file usage, define a separate label
:deactivate_internal
if not defined KAM_VENV_ACTIVE (
    echo Kam virtual environment is not activated.
    exit /b 1
)

REM Restore original environment
set "PATH=%KAM_OLD_PATH%"
set "PROMPT=%KAM_OLD_PROMPT%"
set KAM_VENV_ACTIVE=
set KAM_VENV_DIR=

REM Remove the macro
doskey deactivation=

echo Kam virtual environment deactivated.
goto :eof

REM Public deactivate command that can be called from outside
:deactivate
goto deactivate_internal
