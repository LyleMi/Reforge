[CmdletBinding()]
param(
    [ValidateSet("plugin", "skills-only")][string]$Mode = "plugin",
    [string]$PluginDir,
    [string]$SkillsDir,
    [string]$AgentDir,
    [switch]$SkipAgent,
    [switch]$SkipCli,
    [switch]$Force,
    [switch]$WithUnity,
    [switch]$WithGuard,
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
  -WithUnity                    Also install the optional Unity analyzer CLI.
  -WithGuard                    Also install the workflow CLI and plan/apply/verify skills.
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
if (-not (Test-Path (Join-Path $repoRoot ".codex-plugin\plugin.json"))) { throw "Missing plugin manifest" }
if (-not (Test-Path (Join-Path $repoRoot "skills\reforge-analyze\SKILL.md"))) { throw "Missing analyze skill source" }

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
    $selected = @("reforge-analyze")
    if ($WithGuard) { $selected += @("reforge-plan","reforge-apply","reforge-verify") }
    $selected
}

function Get-SkillSource([string]$Name) {
    if ($Name -eq "reforge-analyze") {
        Join-Path $repoRoot "skills\reforge-analyze"
    } else {
        Join-Path $repoRoot "tools\reforge-workflow\skills\$Name"
    }
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
# Reforge Analysis

Reforge analyzes refactoring evidence through Codebase and Dataflow analyses. Unity is an optional analyzer and the approval workflow is an optional guard.

Use the installed Reforge skills when the user asks to scan, inspect, plan, apply, or verify maintainability/refactoring work:

- ``reforge-analyze``: run ``reforge analyze <target> --output json`` before broad cleanup or architecture recommendations.
- If the optional guard was explicitly installed, ``reforge-plan``, ``reforge-apply``, and ``reforge-verify`` manage its approval artifacts. They are not prerequisites for analysis.

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
# Reforge Analysis

Use Reforge for maintainability/refactoring scans and approval-gated refactor workflows.

- Run ``reforge analyze <target> --output json`` before broad cleanup or architecture recommendations.
- Only when the optional guard was explicitly installed, use ``reforge-workflow`` artifact v5 commands for approval-gated changes.
- Do not edit source during planning. Edit only after an explicit approved Reforge workflow exists.
- Keep workflow artifacts durable and schema-valid.
- Do not add suppressions, change thresholds, install dependencies, commit, push, or open pull requests unless the user explicitly asks.
"@ | Set-Content -Path $TargetFile -Encoding UTF8
}

function Install-InstructionFile([string]$TargetFile, [string]$TargetAgent) {
    if ((Test-Path $TargetFile) -and -not $Force) {
        if ((Get-Content $TargetFile -Raw) -match "Reforge (Agent Workflow|Analysis)") {
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

function Get-PortableAgentBase([string]$TargetAgent) {
    if ($ProjectDir) {
        return $ProjectDir
    }
    if ($RootDir) {
        return $RootDir
    }

    switch ($TargetAgent) {
        "claude" { Join-Path $HOME ".claude" }
        "gemini" { Join-Path $HOME ".gemini" }
        "opencode" { if ($env:XDG_CONFIG_HOME) { Join-Path $env:XDG_CONFIG_HOME "opencode" } else { Join-Path $HOME ".config\opencode" } }
        "codebuddy" { Join-Path $HOME ".codebuddy" }
        "cursor" { Join-Path $HOME ".cursor" }
        "generic" { Join-Path $HOME ".agents" }
        "codex" { $codexRoot }
    }
}

function Get-PortableAgentLayout([string]$TargetAgent, [string]$Base) {
    $scope = if ($ProjectDir) { "project" } else { "global" }
    $instructionPaths = @{
        "claude" = "CLAUDE.md"
        "gemini" = "GEMINI.md"
        "opencode" = "AGENTS.md"
        "codebuddy" = "CODEBUDDY.md"
        "cursor|global" = "rules\reforge.mdc"
        "cursor|project" = ".cursor\rules\reforge.mdc"
        "generic" = "AGENTS.md"
        "codex" = "AGENTS.md"
    }
    $skillPaths = @{
        "claude|global" = "skills"
        "claude|project" = ".claude\skills"
        "opencode|global" = "skills"
        "opencode|project" = ".opencode\skills"
        "codebuddy|global" = "skills"
        "codebuddy|project" = ".codebuddy\skills"
        "generic|global" = "skills"
        "generic|project" = ".agents\skills"
        "codex|global" = "skills"
        "codex|project" = ".agents\skills"
    }
    $instructionKey = if ($TargetAgent -eq "cursor") { "$TargetAgent|$scope" } else { $TargetAgent }
    $skillKey = "$TargetAgent|$scope"
    [pscustomobject]@{
        Instructions = Join-Path $Base $instructionPaths[$instructionKey]
        Skills = if ($skillPaths.ContainsKey($skillKey)) { Join-Path $Base $skillPaths[$skillKey] } else { $null }
    }
}

function Install-PortableAgent([string]$TargetAgent) {
    $base = Get-PortableAgentBase $TargetAgent
    $layout = Get-PortableAgentLayout $TargetAgent $base

    Install-InstructionFile $layout.Instructions $TargetAgent
    if ($layout.Skills) { Install-SkillSet $layout.Skills }
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
    Copy-Item (Join-Path $repoRoot ".codex-plugin\bundle.json") (Join-Path $stageSource ".codex-plugin\bundle.json")
    foreach ($name in (Get-SelectedSkills)) { Copy-Item (Get-SkillSource $name) (Join-Path $stageSource "skills\$name") -Recurse }
    if (-not $SkipAgent) { Copy-Item (Join-Path $repoRoot "tools\reforge-workflow\agents\reforge-investigator.toml") (Join-Path $stageSource ".codex\agents\") }
    try { Install-DirectoryAtomic $stageSource $PluginDir } finally { if (Test-Path $stageSource) { Remove-Item $stageSource -Recurse -Force } }
} else {
    foreach ($name in (Get-SelectedSkills)) { Install-DirectoryAtomic (Get-SkillSource $name) (Join-Path $SkillsDir $name) }
    if (-not $SkipAgent) { New-Item -ItemType Directory -Force -Path $AgentDir | Out-Null; $target=Join-Path $AgentDir "reforge-investigator.toml"; if((Test-Path $target)-and -not $Force){throw "Agent already exists at $target. Pass -Force to update it."}; Copy-Item (Join-Path $repoRoot "tools\reforge-workflow\agents\reforge-investigator.toml") "$target.stage" -Force; Move-Item "$target.stage" $target -Force }
}
}
if (-not $SkipCli) {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw "cargo is required; pass -SkipCli to omit the CLI" }
    $packages = @("reforge")
    if ($WithUnity) { $packages += "reforge-unity" }
    if ($WithGuard) { $packages += "reforge-workflow" }
    foreach ($package in $packages) {
        & cargo install --path (Join-Path $repoRoot "tools/$package") --locked
        if ($LASTEXITCODE -ne 0) { throw "cargo install failed for $package" }
    }
}
