$ErrorActionPreference = "Stop"
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$workflowInstaller = Join-Path $scriptDir "install-agent-workflow.ps1"
$canonicalInstaller = Join-Path $scriptDir "install-reforge.ps1"
$testRoot = Join-Path ([IO.Path]::GetTempPath()) "reforge-agent-workflow-test-$PID"

function Assert-File([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "Expected file was not installed: $Path"
    }
}

function Assert-Missing([string]$Path) {
    if (Test-Path -LiteralPath $Path) {
        throw "Unexpected path was installed: $Path"
    }
}

New-Item -ItemType Directory -Path $testRoot | Out-Null
try {
    $helpOutput = & $workflowInstaller -Help | Out-String
    if ($helpOutput -notmatch "Usage: scripts\\install-agent-workflow\.ps1 \[options\]") {
        throw "Installer help output is missing the usage line"
    }
    & $canonicalInstaller -Help | Out-Null

    $skills = Join-Path $testRoot "skills"
    $agents = Join-Path $testRoot "agents"
    & $workflowInstaller -Mode skills-only -SkillsDir $skills -AgentDir $agents -SkipCli
    Assert-File (Join-Path $skills "reforge-analyze\SKILL.md")
    Assert-Missing (Join-Path $skills "reforge-plan")
    Assert-File (Join-Path $agents "reforge-investigator.toml")

    $guardSkills = Join-Path $testRoot "guard-skills"
    $guardAgents = Join-Path $testRoot "guard-agents"
    & $workflowInstaller -Mode skills-only -SkillsDir $guardSkills -AgentDir $guardAgents -SkipCli -WithGuard
    foreach ($skill in @("reforge-analyze", "reforge-plan", "reforge-apply", "reforge-verify")) {
        Assert-File (Join-Path $guardSkills "$skill\SKILL.md")
    }

    $plugin = Join-Path $testRoot "plugin"
    & $workflowInstaller -Mode plugin -PluginDir $plugin -SkipAgent -SkipCli
    Assert-File (Join-Path $plugin ".codex-plugin\plugin.json")
    Assert-File (Join-Path $plugin "skills\reforge-analyze\SKILL.md")
    Assert-Missing (Join-Path $plugin ".codex\agents\reforge-investigator.toml")

    $projectRoot = Join-Path $testRoot "all-project"
    & $workflowInstaller -Agent all -ProjectDir $projectRoot -SkipCli
    Assert-File (Join-Path $projectRoot "CLAUDE.md")
    Assert-File (Join-Path $projectRoot "GEMINI.md")
    Assert-File (Join-Path $projectRoot "AGENTS.md")
    Assert-File (Join-Path $projectRoot "CODEBUDDY.md")
    Assert-File (Join-Path $projectRoot ".cursor\rules\reforge.mdc")
    Assert-File (Join-Path $projectRoot ".claude\skills\reforge-analyze\SKILL.md")
    Assert-File (Join-Path $projectRoot ".opencode\skills\reforge-analyze\SKILL.md")
    Assert-File (Join-Path $projectRoot ".codebuddy\skills\reforge-analyze\SKILL.md")
    Assert-File (Join-Path $projectRoot ".agents\skills\reforge-analyze\SKILL.md")

    $globalCases = @(
        [pscustomobject]@{ Agent = "claude"; Instruction = "CLAUDE.md"; Skills = "skills\reforge-analyze\SKILL.md" },
        [pscustomobject]@{ Agent = "gemini"; Instruction = "GEMINI.md"; Skills = $null },
        [pscustomobject]@{ Agent = "opencode"; Instruction = "AGENTS.md"; Skills = "skills\reforge-analyze\SKILL.md" },
        [pscustomobject]@{ Agent = "codebuddy"; Instruction = "CODEBUDDY.md"; Skills = "skills\reforge-analyze\SKILL.md" },
        [pscustomobject]@{ Agent = "cursor"; Instruction = "rules\reforge.mdc"; Skills = $null },
        [pscustomobject]@{ Agent = "generic"; Instruction = "AGENTS.md"; Skills = "skills\reforge-analyze\SKILL.md" }
    )
    foreach ($case in $globalCases) {
        $root = Join-Path $testRoot "$($case.Agent)-global"
        & $workflowInstaller -Agent $case.Agent -RootDir $root -SkipCli
        Assert-File (Join-Path $root $case.Instruction)
        if ($case.Skills) {
            Assert-File (Join-Path $root $case.Skills)
        } else {
            Assert-Missing (Join-Path $root "skills")
        }
    }

    Write-Host "PowerShell installer tests passed"
} finally {
    $resolvedTestRoot = [IO.Path]::GetFullPath($testRoot)
    $resolvedTempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
    if ($resolvedTestRoot.StartsWith($resolvedTempRoot, [StringComparison]::OrdinalIgnoreCase) -and (Test-Path -LiteralPath $resolvedTestRoot)) {
        Remove-Item -LiteralPath $resolvedTestRoot -Recurse -Force
    }
}
