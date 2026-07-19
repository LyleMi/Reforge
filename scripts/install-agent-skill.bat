@echo off
setlocal EnableExtensions DisableDelayedExpansion

set "agent=codex"
set "skills_dir="
set "source_dir="
set "force=0"
set "install_cli=1"

:parse_args
if "%~1"=="" goto validate_args

if /I "%~1"=="--agent" (
    if "%~2"=="" goto missing_agent
    set "agent=%~2"
    shift
    shift
    goto parse_args
)
if /I "%~1"=="--skills-dir" (
    if "%~2"=="" goto missing_skills_dir
    set "skills_dir=%~2"
    shift
    shift
    goto parse_args
)
if /I "%~1"=="--source" (
    if "%~2"=="" goto missing_source
    set "source_dir=%~2"
    shift
    shift
    goto parse_args
)
if /I "%~1"=="--force" (
    set "force=1"
    shift
    goto parse_args
)
if /I "%~1"=="--install-cli" (
    set "install_cli=1"
    shift
    goto parse_args
)
if /I "%~1"=="--skip-cli" (
    set "install_cli=0"
    shift
    goto parse_args
)
if /I "%~1"=="-h" goto show_help
if /I "%~1"=="--help" goto show_help

>&2 echo Unknown option: %~1
call :usage >&2
exit /b 2

:validate_args
if /I "%agent%"=="codex" goto resolve_paths
if /I "%agent%"=="generic" goto resolve_paths
>&2 echo Unsupported agent: %agent%
exit /b 2

:resolve_paths
for %%I in ("%~dp0..") do set "repo_root=%%~fI"

if not defined source_dir set "source_dir=%repo_root%\skills\reforge-scan"
if not exist "%source_dir%\SKILL.md" (
    >&2 echo Source is not a skill folder: %source_dir%
    exit /b 1
)
for %%I in ("%source_dir%") do set "source_abs=%%~fI"

if defined skills_dir goto create_skills_dir
if /I "%agent%"=="generic" (
    >&2 echo Pass --skills-dir when --agent generic is used.
    exit /b 2
)
if defined CODEX_HOME (
    set "skills_dir=%CODEX_HOME%\skills"
) else if defined USERPROFILE (
    set "skills_dir=%USERPROFILE%\.codex\skills"
) else if defined HOME (
    set "skills_dir=%HOME%\.codex\skills"
) else (
    >&2 echo Cannot infer the home directory. Pass --skills-dir explicitly.
    exit /b 1
)

:create_skills_dir
if not exist "%skills_dir%\" (
    mkdir "%skills_dir%"
    if errorlevel 1 (
        >&2 echo Failed to create skills directory: %skills_dir%
        exit /b 1
    )
)
for %%I in ("%skills_dir%") do set "skills_abs=%%~fI"
set "target_abs=%skills_abs%\reforge-scan"

if /I "%source_abs%"=="%target_abs%" (
    echo Source and target are the same folder; leaving %target_abs% unchanged
    goto install_cli
)

if exist "%target_abs%" (
    if not "%force%"=="1" (
        >&2 echo Skill already exists at %target_abs%. Pass --force to update it.
        exit /b 1
    )
    rmdir /S /Q "%target_abs%"
    if errorlevel 1 (
        >&2 echo Failed to remove existing skill: %target_abs%
        exit /b 1
    )
)

mkdir "%target_abs%"
if errorlevel 1 (
    >&2 echo Failed to create skill directory: %target_abs%
    exit /b 1
)
xcopy "%source_abs%\*" "%target_abs%\" /E /H /I /Y >nul
if errorlevel 1 (
    >&2 echo Failed to copy the skill to %target_abs%
    exit /b 1
)
echo Installed reforge-scan skill to %target_abs%

:install_cli
if not "%install_cli%"=="1" exit /b 0
where cargo.exe >nul 2>&1
if errorlevel 1 (
    >&2 echo cargo is required to install the Reforge CLI. Pass --skip-cli to install only the skill.
    exit /b 1
)
cargo install --path "%repo_root%"
if errorlevel 1 (
    >&2 echo cargo install failed.
    exit /b 1
)
exit /b 0

:missing_agent
>&2 echo Missing value for --agent.
exit /b 2

:missing_skills_dir
>&2 echo Missing value for --skills-dir.
exit /b 2

:missing_source
>&2 echo Missing value for --source.
exit /b 2

:show_help
call :usage
exit /b 0

:usage
echo Usage: scripts\install-agent-skill.bat [options]
echo(
echo Options:
echo   --agent codex^|generic   Target agent layout. Defaults to codex.
echo   --skills-dir DIR        Directory that contains skill folders.
echo   --source DIR            Skill source folder. Defaults to skills\reforge-scan.
echo   --force                 Replace an existing reforge-scan skill.
echo   --skip-cli              Install the skill without installing the Reforge CLI.
echo   -h, --help              Show this help.
exit /b 0
