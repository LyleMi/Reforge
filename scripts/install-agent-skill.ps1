[CmdletBinding()]
param(
    [ValidateSet("codex", "generic")]
    [string]$Agent = "codex",

    [string]$SkillsDir,

    [string]$Source,

    [switch]$Force,

    [switch]$InstallCli
)

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = (Resolve-Path -LiteralPath (Join-Path $scriptDir "..")).Path

if ([string]::IsNullOrWhiteSpace($Source)) {
    $Source = Join-Path $repoRoot "skills\reforge-scan"
}

$sourcePath = (Resolve-Path -LiteralPath $Source).Path
$skillFile = Join-Path $sourcePath "SKILL.md"
if (-not (Test-Path -LiteralPath $skillFile)) {
    throw "Source is not a skill folder: $sourcePath"
}

if ([string]::IsNullOrWhiteSpace($SkillsDir)) {
    switch ($Agent) {
        "codex" {
            $codexHome = $env:CODEX_HOME
            if ([string]::IsNullOrWhiteSpace($codexHome)) {
                $homeDir = $HOME
                if ([string]::IsNullOrWhiteSpace($homeDir)) {
                    $homeDir = $env:USERPROFILE
                }
                if ([string]::IsNullOrWhiteSpace($homeDir)) {
                    throw "Cannot infer the home directory. Pass -SkillsDir explicitly."
                }
                $codexHome = Join-Path $homeDir ".codex"
            }
            $SkillsDir = Join-Path $codexHome "skills"
        }
        "generic" {
            throw "Pass -SkillsDir when -Agent generic is used."
        }
    }
}

New-Item -ItemType Directory -Force -Path $SkillsDir | Out-Null
$skillsRoot = (Resolve-Path -LiteralPath $SkillsDir).Path
$target = Join-Path $skillsRoot "reforge-scan"

$sourceComparable = $sourcePath.TrimEnd('\', '/')
$targetComparable = if (Test-Path -LiteralPath $target) {
    (Resolve-Path -LiteralPath $target).Path.TrimEnd('\', '/')
}
else {
    $target.TrimEnd('\', '/')
}

if ([string]::Equals($sourceComparable, $targetComparable, [StringComparison]::OrdinalIgnoreCase)) {
    Write-Host "Source and target are the same folder; leaving $target unchanged"
}
else {
    if (Test-Path -LiteralPath $target) {
        if (-not $Force) {
            throw "Skill already exists at $target. Pass -Force to update it."
        }

        $resolvedTarget = (Resolve-Path -LiteralPath $target).Path
        if ((Split-Path -Leaf $resolvedTarget) -ne "reforge-scan") {
            throw "Refusing to remove unexpected target: $resolvedTarget"
        }

        $rootWithSeparator = $skillsRoot.TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
        $targetWithSeparator = $resolvedTarget.TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
        if (-not $targetWithSeparator.StartsWith($rootWithSeparator, [StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing to remove target outside skills directory: $resolvedTarget"
        }

        Remove-Item -LiteralPath $resolvedTarget -Recurse -Force
    }

    New-Item -ItemType Directory -Force -Path $target | Out-Null
    Get-ChildItem -LiteralPath $sourcePath -Force | Copy-Item -Destination $target -Recurse -Force

    Write-Host "Installed reforge-scan skill to $target"
}

if ($InstallCli) {
    & cargo install --path $repoRoot
    if ($LASTEXITCODE -ne 0) {
        throw "cargo install failed with exit code $LASTEXITCODE"
    }
}
