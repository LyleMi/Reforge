@echo off
setlocal EnableExtensions DisableDelayedExpansion
set "mode=plugin"
set "force=0"
set "skip_cli=0"
set "skip_agent=0"
set "only_scan=0"
set "plugin_dir="
set "skills_dir="
set "agent_dir="
set "scan_source="
set "agent=codex"
set "project_dir="
set "root_dir="
:parse
if "%~1"=="" goto resolve
if /I "%~1"=="--plugin" set "mode=plugin"&shift&goto parse
if /I "%~1"=="--skills-only" set "mode=skills"&shift&goto parse
if /I "%~1"=="--force" set "force=1"&shift&goto parse
if /I "%~1"=="--skip-cli" set "skip_cli=1"&shift&goto parse
if /I "%~1"=="--install-cli" set "skip_cli=0"&shift&goto parse
if /I "%~1"=="--skip-agent" set "skip_agent=1"&shift&goto parse
if /I "%~1"=="--only-scan" set "only_scan=1"&shift&goto parse
if /I "%~1"=="--plugin-dir" set "plugin_dir=%~2"&shift&shift&goto parse
if /I "%~1"=="--skills-dir" set "skills_dir=%~2"&shift&shift&goto parse
if /I "%~1"=="--agent-dir" set "agent_dir=%~2"&shift&shift&goto parse
if /I "%~1"=="--source" set "scan_source=%~2"&shift&shift&goto parse
if /I "%~1"=="--agent" set "agent=%~2"&shift&shift&goto parse
if /I "%~1"=="--project-dir" set "project_dir=%~2"&shift&shift&goto parse
if /I "%~1"=="--root-dir" set "root_dir=%~2"&shift&shift&goto parse
if /I "%~1"=="--help" goto help
if /I "%~1"=="-h" goto help
>&2 echo Unknown option: %~1
exit /b 2
:resolve
if exist "%~dp0Cargo.toml" (
    pushd "%~dp0." >nul || exit /b 1
) else (
    pushd "%~dp0.." >nul || exit /b 1
)
set "repo_root=%CD%"
popd >nul
if defined CODEX_HOME (set "codex_root=%CODEX_HOME%") else if defined USERPROFILE (set "codex_root=%USERPROFILE%\.codex") else (>&2 echo Cannot infer Codex home&exit /b 1)
if not defined plugin_dir set "plugin_dir=%codex_root%\plugins\reforge"
if not defined skills_dir set "skills_dir=%codex_root%\skills"
if not defined agent_dir set "agent_dir=%codex_root%\agents"
if not defined scan_source set "scan_source=%repo_root%\skills\reforge-scan"
if not exist "%scan_source%\SKILL.md" (>&2 echo Invalid scan skill source&exit /b 1)
if /I not "%agent%"=="codex" goto portable
if defined project_dir goto portable
if not exist "%repo_root%\.codex-plugin\plugin.json" (>&2 echo Missing plugin manifest&exit /b 1)
if /I "%mode%"=="plugin" goto plugin
if "%only_scan%"=="1" call :install_dir "%scan_source%" "%skills_dir%\reforge-scan" || exit /b 1
if "%only_scan%"=="0" for %%S in (reforge-scan reforge-plan reforge-apply reforge-verify) do call :install_dir "%repo_root%\skills\%%S" "%skills_dir%\%%S" || exit /b 1
if "%skip_agent%"=="0" (if not exist "%agent_dir%" mkdir "%agent_dir%"&if exist "%agent_dir%\reforge-investigator.toml" if not "%force%"=="1" (>&2 echo Agent exists; pass --force&exit /b 1)&copy /Y "%repo_root%\.codex\agents\reforge-investigator.toml" "%agent_dir%\reforge-investigator.toml" >nul)
goto cli
:plugin
set "stage_source=%TEMP%\reforge-plugin-source-%RANDOM%"
mkdir "%stage_source%\.codex-plugin" "%stage_source%\skills" "%stage_source%\.codex\agents" || exit /b 1
copy /Y "%repo_root%\.codex-plugin\plugin.json" "%stage_source%\.codex-plugin\plugin.json" >nul
if "%only_scan%"=="1" xcopy "%scan_source%\*" "%stage_source%\skills\reforge-scan\" /E /H /I /Y >nul
if "%only_scan%"=="0" for %%S in (reforge-scan reforge-plan reforge-apply reforge-verify) do xcopy "%repo_root%\skills\%%S\*" "%stage_source%\skills\%%S\" /E /H /I /Y >nul
if "%skip_agent%"=="0" copy /Y "%repo_root%\.codex\agents\reforge-investigator.toml" "%stage_source%\.codex\agents\" >nul
call :install_dir "%stage_source%" "%plugin_dir%" || exit /b 1
rmdir /S /Q "%stage_source%"
:cli
if "%skip_cli%"=="1" exit /b 0
where cargo.exe >nul 2>&1 || (>&2 echo cargo is required; pass --skip-cli&exit /b 1)
cargo install --path "%repo_root%"
exit /b %ERRORLEVEL%
:install_dir
set "source_path=%~1"
set "target_path=%~2"
for %%I in ("%target_path%") do set "target_parent=%%~dpI"&set "target_name=%%~nxI"
if not exist "%target_parent%" mkdir "%target_parent%" || exit /b 1
if exist "%target_path%" if not "%force%"=="1" (>&2 echo Installation exists at %target_path%. Pass --force.&exit /b 1)
set "stage=%target_parent%.%target_name%.stage.%RANDOM%"
set "backup=%target_parent%.%target_name%.backup.%RANDOM%"
xcopy "%source_path%\*" "%stage%\" /E /H /I /Y >nul || exit /b 1
if exist "%target_path%" move "%target_path%" "%backup%" >nul || exit /b 1
move "%stage%" "%target_path%" >nul || (if exist "%backup%" move "%backup%" "%target_path%" >nul&exit /b 1)
if exist "%backup%" rmdir /S /Q "%backup%"
echo Installed %target_path%
exit /b 0
:help
echo Usage: scripts\install-agent-workflow.bat [options]
echo.
echo   --plugin                    Install the standard plugin ^(default^).
echo   --skills-only               Install skills without the plugin manifest.
echo   --plugin-dir DIR            Exact plugin destination.
echo   --skills-dir DIR            Exact skills parent directory.
echo   --agent-dir DIR             Exact custom-agent parent directory.
echo   --skip-agent                Do not install the investigator agent.
echo   --skip-cli                  Do not install the Reforge CLI.
echo   --install-cli               Install the Reforge CLI ^(default^).
echo   --force                     Atomically replace an existing installation.
echo   --only-scan                 Install only reforge-scan ^(compatibility mode^).
echo   --source DIR                Custom reforge-scan source ^(compatibility mode^).
echo   --agent NAME                Target agent: codex, claude, gemini, opencode,
echo                               codebuddy, cursor, generic, or all.
echo   --project-dir DIR           Install project-local files into DIR.
echo   --root-dir DIR              Override the selected agent's global root/config dir.
echo   -h, --help                  Print this help and exit.
exit /b 0
:portable
set "ps=powershell.exe"
where pwsh.exe >nul 2>&1 && set "ps=pwsh.exe"
set "ps_mode=plugin"
if /I "%mode%"=="skills" set "ps_mode=skills-only"
set "ps_args=-NoProfile -ExecutionPolicy Bypass -File "%repo_root%\scripts\install-agent-workflow.ps1" -Mode %ps_mode% -Agent %agent% -Source "%scan_source%""
if "%force%"=="1" set "ps_args=%ps_args% -Force"
if "%skip_cli%"=="1" set "ps_args=%ps_args% -SkipCli"
if "%skip_agent%"=="1" set "ps_args=%ps_args% -SkipAgent"
if "%only_scan%"=="1" set "ps_args=%ps_args% -OnlyScan"
if defined plugin_dir set "ps_args=%ps_args% -PluginDir "%plugin_dir%""
if defined skills_dir set "ps_args=%ps_args% -SkillsDir "%skills_dir%""
if defined agent_dir set "ps_args=%ps_args% -AgentDir "%agent_dir%""
if defined project_dir set "ps_args=%ps_args% -ProjectDir "%project_dir%""
if defined root_dir set "ps_args=%ps_args% -RootDir "%root_dir%""
%ps% %ps_args%
exit /b %ERRORLEVEL%
