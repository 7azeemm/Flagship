@echo off
rem Run "start.bat reset" or choose Reset database in the menu to recreate the local database.
setlocal
cd /d "%~dp0"
if exist "%~dp0state\node22\node.exe" (
    set "flagship_node=%~dp0state\node22\node.exe"
    goto run_launcher
)
where node >nul 2>nul
if errorlevel 1 (
    echo Install Node.js 22 LTS first, then reopen this launcher.
    pause
    exit /b 1
)
set "flagship_node=node"
:run_launcher
"%flagship_node%" "%~dp0deploy\local.mjs" %*
set "launcher_exit=%errorlevel%"
pause
exit /b %launcher_exit%
