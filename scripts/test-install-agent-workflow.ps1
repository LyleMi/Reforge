$ErrorActionPreference = "Stop"
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$workflowInstaller = Join-Path $scriptDir "install-agent-workflow.ps1"
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

    $projectRoot = Join-Path $testRoot "all-project"
    & $workflowInstaller -Agent all -ProjectDir $projectRoot -SkipCli
    Assert-File (Join-Path $projectRoot "CLAUDE.md")
    Assert-File (Join-Path $projectRoot "GEMINI.md")
    Assert-File (Join-Path $projectRoot "AGENTS.md")
    Assert-File (Join-Path $projectRoot "CODEBUDDY.md")
    Assert-File (Join-Path $projectRoot ".cursor\rules\reforge.mdc")
    Assert-File (Join-Path $projectRoot ".claude\skills\reforge-scan\SKILL.md")
    Assert-File (Join-Path $projectRoot ".opencode\skills\reforge-scan\SKILL.md")
    Assert-File (Join-Path $projectRoot ".codebuddy\skills\reforge-scan\SKILL.md")
    Assert-File (Join-Path $projectRoot ".agents\skills\reforge-scan\SKILL.md")

    $globalCases = @(
        [pscustomobject]@{ Agent = "claude"; Instruction = "CLAUDE.md"; Skills = "skills\reforge-scan\SKILL.md" },
        [pscustomobject]@{ Agent = "gemini"; Instruction = "GEMINI.md"; Skills = $null },
        [pscustomobject]@{ Agent = "opencode"; Instruction = "AGENTS.md"; Skills = "skills\reforge-scan\SKILL.md" },
        [pscustomobject]@{ Agent = "codebuddy"; Instruction = "CODEBUDDY.md"; Skills = "skills\reforge-scan\SKILL.md" },
        [pscustomobject]@{ Agent = "cursor"; Instruction = "rules\reforge.mdc"; Skills = $null },
        [pscustomobject]@{ Agent = "generic"; Instruction = "AGENTS.md"; Skills = "skills\reforge-scan\SKILL.md" }
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
