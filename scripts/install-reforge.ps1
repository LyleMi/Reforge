$ErrorActionPreference = "Stop"

if ($args -contains "-Help" -or $args -contains "--help" -or $args -contains "-h") {
    Write-Output "Usage: scripts/install-reforge.ps1 [cargo-install-options]"
    Write-Output "Installs the reforge binary and reforge-analyze skill."
    exit 0
}

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent $scriptDir

& cargo install --path (Join-Path $repoRoot "tools/reforge") --locked @args
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$codexRoot = if ($env:CODEX_HOME) { $env:CODEX_HOME } else { Join-Path $HOME ".codex" }
$skillRoot = Join-Path $codexRoot "skills/reforge-analyze"
New-Item -ItemType Directory -Force $skillRoot | Out-Null
Copy-Item (Join-Path $repoRoot "skills/reforge-analyze/SKILL.md") (Join-Path $skillRoot "SKILL.md") -Force
$agents = Join-Path $repoRoot "skills/reforge-analyze/agents"
if (Test-Path $agents) {
    New-Item -ItemType Directory -Force (Join-Path $skillRoot "agents") | Out-Null
    Copy-Item (Join-Path $agents "*") (Join-Path $skillRoot "agents") -Force
}
