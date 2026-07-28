@echo off
call node "%~dp0install.js" hook %*
set "_epic_hook_status=%ERRORLEVEL%"
if /I "%~1"=="PreToolUse" if "%_epic_hook_status%"=="2" exit /b 0
exit /b %_epic_hook_status%
