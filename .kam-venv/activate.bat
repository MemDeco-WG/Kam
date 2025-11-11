@echo off
REM Kam Virtual Environment Activation Script (Windows)
REM Type: Runtime

REM Store old PATH
set "KAM_OLD_PATH=%PATH%"

REM Add venv bin to PATH
set "PATH=.\.kam-venv\bin;%PATH%"

REM Set environment marker
set "KAM_VENV_ACTIVE=1"

REM Update prompt
set "PROMPT=(kam-venv) %PROMPT%"

echo Kam virtual environment activated (runtime mode)
echo Run 'deactivate' to exit
