[CmdletBinding()]
param(
    [ValidateSet("plugin", "skills-only")][string]$Mode = "plugin",
    [string]$PluginDir,
    [string]$SkillsDir,
    [string]$AgentDir,
    [switch]$SkipAgent,
    [switch]$SkipCli,
    [switch]$Force,
    [switch]$OnlyScan,
    [string]$Source,
    [ValidateSet("codex", "generic")][string]$Agent = "codex",
    [switch]$InstallCli
)
$ErrorActionPreference = "Stop"
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = (Resolve-Path (Join-Path $scriptDir "..")).Path
$codexRoot = if ($env:CODEX_HOME) { $env:CODEX_HOME } elseif ($HOME) { Join-Path $HOME ".codex" } else { throw "Cannot infer Codex home" }
if (-not $PluginDir) { $PluginDir = Join-Path $codexRoot "plugins\reforge" }
if (-not $SkillsDir) { $SkillsDir = Join-Path $codexRoot "skills" }
if (-not $AgentDir) { $AgentDir = Join-Path $codexRoot "agents" }
if (-not $Source) { $Source = Join-Path $repoRoot "skills\reforge-scan" }
if (-not (Test-Path (Join-Path $repoRoot ".codex-plugin\plugin.json"))) { throw "Missing plugin manifest" }
if (-not (Test-Path (Join-Path $Source "SKILL.md"))) { throw "Source is not a skill folder: $Source" }

function Install-DirectoryAtomic([string]$SourcePath, [string]$TargetPath) {
    $sourceResolved = (Resolve-Path $SourcePath).Path.TrimEnd('\','/')
    $parent = Split-Path -Parent $TargetPath
    $name = Split-Path -Leaf $TargetPath
    New-Item -ItemType Directory -Force -Path $parent | Out-Null
    $targetFull = [IO.Path]::GetFullPath($TargetPath).TrimEnd('\','/')
    if ([string]::Equals($sourceResolved,$targetFull,[StringComparison]::OrdinalIgnoreCase)) { Write-Host "Source and target are the same folder; leaving $targetFull unchanged"; return }
    if ((Test-Path $targetFull) -and -not $Force) { throw "Installation already exists at $targetFull. Pass -Force to update it." }
    $stage = Join-Path $parent ".$name.stage.$PID"
    $backup = Join-Path $parent ".$name.backup.$PID"
    try {
        New-Item -ItemType Directory -Path $stage | Out-Null
        Get-ChildItem -Force $sourceResolved | Copy-Item -Destination $stage -Recurse -Force
        if (Test-Path $targetFull) { Move-Item $targetFull $backup }
        Move-Item $stage $targetFull
        if (Test-Path $backup) { Remove-Item $backup -Recurse -Force }
    } catch {
        if ((Test-Path $backup) -and -not (Test-Path $targetFull)) { Move-Item $backup $targetFull }
        throw
    } finally {
        if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
    }
    Write-Host "Installed $targetFull"
}

if ($Mode -eq "plugin") {
    $stageSource = Join-Path ([IO.Path]::GetTempPath()) "reforge-plugin-source-$PID"
    New-Item -ItemType Directory -Force -Path (Join-Path $stageSource ".codex-plugin"),(Join-Path $stageSource "skills"),(Join-Path $stageSource ".codex\agents") | Out-Null
    Copy-Item (Join-Path $repoRoot ".codex-plugin\plugin.json") (Join-Path $stageSource ".codex-plugin\plugin.json")
    $names = if ($OnlyScan) { @("reforge-scan") } else { @("reforge-scan","reforge-plan","reforge-apply","reforge-verify") }
    foreach ($name in $names) { $skillSource = if ($name -eq "reforge-scan" -and $OnlyScan) { $Source } else { Join-Path $repoRoot "skills\$name" }; Copy-Item $skillSource (Join-Path $stageSource "skills\$name") -Recurse }
    if (-not $SkipAgent) { Copy-Item (Join-Path $repoRoot ".codex\agents\reforge-investigator.toml") (Join-Path $stageSource ".codex\agents\") }
    try { Install-DirectoryAtomic $stageSource $PluginDir } finally { if (Test-Path $stageSource) { Remove-Item $stageSource -Recurse -Force } }
} else {
    $names = if ($OnlyScan) { @("reforge-scan") } else { @("reforge-scan","reforge-plan","reforge-apply","reforge-verify") }
    foreach ($name in $names) { $skillSource = if ($name -eq "reforge-scan" -and $OnlyScan) { $Source } else { Join-Path $repoRoot "skills\$name" }; Install-DirectoryAtomic $skillSource (Join-Path $SkillsDir $name) }
    if (-not $SkipAgent) { New-Item -ItemType Directory -Force -Path $AgentDir | Out-Null; $target=Join-Path $AgentDir "reforge-investigator.toml"; if((Test-Path $target)-and -not $Force){throw "Agent already exists at $target. Pass -Force to update it."}; Copy-Item (Join-Path $repoRoot ".codex\agents\reforge-investigator.toml") "$target.stage" -Force; Move-Item "$target.stage" $target -Force }
}
if (-not $SkipCli) { if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw "cargo is required; pass -SkipCli to omit the CLI" }; & cargo install --path $repoRoot; if($LASTEXITCODE-ne 0){throw "cargo install failed"} }
