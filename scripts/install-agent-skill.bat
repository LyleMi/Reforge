@echo off
call "%~dp0install-agent-workflow.bat" --skills-only --only-scan --skip-agent %*
exit /b %ERRORLEVEL%
