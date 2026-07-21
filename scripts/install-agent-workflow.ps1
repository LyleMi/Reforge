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
    [ValidateSet("codex", "claude", "gemini", "opencode", "codebuddy", "cursor", "generic", "all")][string]$Agent = "codex",
    [string]$ProjectDir,
    [string]$RootDir,
    [switch]$InstallCli,
    [Alias("h", "-help")][switch]$Help
)

function Show-Usage {
    @'
Usage: scripts\install-agent-workflow.ps1 [options]

  -Mode plugin|skills-only      Installation mode (default: plugin).
  -PluginDir DIR                Exact plugin destination.
  -SkillsDir DIR                Exact skills parent directory.
  -AgentDir DIR                 Exact custom-agent parent directory.
  -SkipAgent                    Do not install the investigator agent.
  -SkipCli                      Do not install the Reforge CLI.
  -InstallCli                   Install the Reforge CLI (default).
  -Force                        Atomically replace an existing installation.
  -OnlyScan                     Install only reforge-scan (compatibility mode).
  -Source DIR                   Custom reforge-scan source (compatibility mode).
  -Agent NAME                   Target agent: codex, claude, gemini, opencode,
                                codebuddy, cursor, generic, or all.
  -ProjectDir DIR               Install project-local files into DIR.
  -RootDir DIR                  Override the selected agent's global root/config dir.
  -Help, -h, --help             Print this help and exit.
'@
}

if ($Help) {
    Show-Usage
    exit 0
}

$ErrorActionPreference = "Stop"
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = (Resolve-Path (Join-Path $scriptDir "..")).Path
$codexRoot = if ($env:CODEX_HOME) { $env:CODEX_HOME } elseif ($HOME) { Join-Path $HOME ".codex" } else { throw "Cannot infer Codex home" }
if ($RootDir) { $codexRoot = $RootDir }
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

function Get-SelectedSkills {
    if ($OnlyScan) { @("reforge-scan") } else { @("reforge-scan","reforge-plan","reforge-apply","reforge-verify") }
}

function Get-SkillSource([string]$Name) {
    if ($Name -eq "reforge-scan" -and $OnlyScan) { $Source } else { Join-Path $repoRoot "skills\$Name" }
}

function Install-SkillSet([string]$DestinationParent) {
    foreach ($name in (Get-SelectedSkills)) {
        $targetSkill = Join-Path $DestinationParent $name
        $skillFile = Join-Path $targetSkill "SKILL.md"
        if ((Test-Path $targetSkill) -and -not $Force -and (Test-Path $skillFile) -and ((Get-Content $skillFile -Raw) -match "name:\s+$([regex]::Escape($name))")) {
            Write-Host "Skill already installed at $targetSkill"
        } else {
            Install-DirectoryAtomic (Get-SkillSource $name) $targetSkill
        }
    }
}

function Write-AgentInstructions([string]$TargetFile, [string]$Label) {
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $TargetFile) | Out-Null
    @"
# Reforge Agent Workflow

Reforge is a Rust CLI for evidence-driven refactoring scans and approval-gated refactor workflows.

Use the installed Reforge skills when the user asks to scan, inspect, plan, apply, or verify maintainability/refactoring work:

- ``reforge-scan``: run ``reforge scan <target> --progress never`` before broad cleanup or architecture recommendations.
- ``reforge-plan``: investigate selected Reforge issues and produce workflow artifacts without editing source.
- ``reforge-apply``: edit source only after an explicit approved Reforge workflow exists.
- ``reforge-verify``: run checks and compare the rescan result after approved changes.

Keep Reforge workflow artifacts durable and schema-valid. Do not add suppressions, change thresholds, install dependencies, commit, push, or open pull requests unless the user explicitly asks.

Installed for: $Label.
"@ | Set-Content -Path $TargetFile -Encoding UTF8
}

function Write-CursorRule([string]$TargetFile) {
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $TargetFile) | Out-Null
    @"
---
description: Reforge evidence-driven refactoring workflow
alwaysApply: true
---
# Reforge Agent Workflow

Use Reforge for maintainability/refactoring scans and approval-gated refactor workflows.

- Run ``reforge scan <target> --progress never`` before broad cleanup or architecture recommendations.
- Use ``reforge workflow`` artifacts for selected issues, plans, approvals, application, and verification.
- Do not edit source during planning. Edit only after an explicit approved Reforge workflow exists.
- Keep workflow artifacts durable and schema-valid.
- Do not add suppressions, change thresholds, install dependencies, commit, push, or open pull requests unless the user explicitly asks.
"@ | Set-Content -Path $TargetFile -Encoding UTF8
}

function Install-InstructionFile([string]$TargetFile, [string]$TargetAgent) {
    if ((Test-Path $TargetFile) -and -not $Force) {
        if ((Get-Content $TargetFile -Raw) -match "Reforge Agent Workflow") {
            Write-Host "Instructions already installed at $TargetFile"
            return
        }
        throw "Instructions already exist at $TargetFile. Pass -Force to update it."
    }
    $tmp = "$TargetFile.stage.$PID"
    if ($TargetAgent -eq "cursor") { Write-CursorRule $tmp } else { Write-AgentInstructions $tmp $TargetAgent }
    Move-Item $tmp $TargetFile -Force
    Write-Host "Installed $TargetFile"
}

function Install-PortableAgent([string]$TargetAgent) {
    if ($ProjectDir) {
        $base = $ProjectDir
    } else {
        switch ($TargetAgent) {
            "claude" { $base = if ($RootDir) { $RootDir } else { Join-Path $HOME ".claude" } }
            "gemini" { $base = if ($RootDir) { $RootDir } else { Join-Path $HOME ".gemini" } }
            "opencode" { $base = if ($RootDir) { $RootDir } elseif ($env:XDG_CONFIG_HOME) { Join-Path $env:XDG_CONFIG_HOME "opencode" } else { Join-Path $HOME ".config\opencode" } }
            "codebuddy" { $base = if ($RootDir) { $RootDir } else { Join-Path $HOME ".codebuddy" } }
            "cursor" { $base = if ($RootDir) { $RootDir } else { Join-Path $HOME ".cursor" } }
            "generic" { $base = if ($RootDir) { $RootDir } else { Join-Path $HOME ".agents" } }
            "codex" { $base = $codexRoot }
        }
    }

    switch ("$TargetAgent|$([bool]$ProjectDir)") {
        "claude|False" { $instructions = Join-Path $base "CLAUDE.md"; $skills = Join-Path $base "skills" }
        "claude|True" { $instructions = Join-Path $base "CLAUDE.md"; $skills = Join-Path $base ".claude\skills" }
        "gemini|False" { $instructions = Join-Path $base "GEMINI.md"; $skills = $null }
        "gemini|True" { $instructions = Join-Path $base "GEMINI.md"; $skills = $null }
        "opencode|False" { $instructions = Join-Path $base "AGENTS.md"; $skills = Join-Path $base "skills" }
        "opencode|True" { $instructions = Join-Path $base "AGENTS.md"; $skills = Join-Path $base ".opencode\skills" }
        "codebuddy|False" { $instructions = Join-Path $base "CODEBUDDY.md"; $skills = Join-Path $base "skills" }
        "codebuddy|True" { $instructions = Join-Path $base "CODEBUDDY.md"; $skills = Join-Path $base ".codebuddy\skills" }
        "cursor|False" { $instructions = Join-Path $base "rules\reforge.mdc"; $skills = $null }
        "cursor|True" { $instructions = Join-Path $base ".cursor\rules\reforge.mdc"; $skills = $null }
        "generic|False" { $instructions = Join-Path $base "AGENTS.md"; $skills = Join-Path $base "skills" }
        "generic|True" { $instructions = Join-Path $base "AGENTS.md"; $skills = Join-Path $base ".agents\skills" }
        "codex|False" { $instructions = Join-Path $base "AGENTS.md"; $skills = Join-Path $base "skills" }
        "codex|True" { $instructions = Join-Path $base "AGENTS.md"; $skills = Join-Path $base ".agents\skills" }
    }

    Install-InstructionFile $instructions $TargetAgent
    if ($skills) { Install-SkillSet $skills }
}

if ($Agent -eq "all") {
    foreach ($targetAgent in @("claude","codex","gemini","opencode","codebuddy","cursor","generic")) { Install-PortableAgent $targetAgent }
} elseif ($Agent -ne "codex" -or $ProjectDir) {
    Install-PortableAgent $Agent
} else {
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
}
if (-not $SkipCli) { if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw "cargo is required; pass -SkipCli to omit the CLI" }; & cargo install --path $repoRoot; if($LASTEXITCODE-ne 0){throw "cargo install failed"} }
