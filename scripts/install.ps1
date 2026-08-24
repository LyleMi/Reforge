[CmdletBinding()]
param(
    [string]$Version,
    [string]$BinDir,
    [switch]$SkipSkill
)

$ErrorActionPreference = "Stop"
$repository = if ($env:REFORGE_REPOSITORY) { $env:REFORGE_REPOSITORY } else { "LyleMi/Reforge" }
$releaseBase = if ($env:REFORGE_RELEASE_BASE_URL) { $env:REFORGE_RELEASE_BASE_URL.TrimEnd('/') } else { "https://github.com/$repository/releases/download" }
if (-not $BinDir) {
    $BinDir = if ($env:REFORGE_INSTALL_DIR) { $env:REFORGE_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "Reforge\bin" }
}
if (-not $Version) {
    if ($env:REFORGE_LATEST_VERSION) {
        $Version = $env:REFORGE_LATEST_VERSION
    } else {
        $headers = @{ "User-Agent" = "reforge-installer"; "Accept" = "application/vnd.github+json" }
        $Version = (Invoke-RestMethod -Headers $headers -Uri "https://api.github.com/repos/$repository/releases/latest").tag_name
    }
}
if ($Version -notmatch '^v[0-9]') { throw "Invalid release tag: $Version" }

if (-not [System.Environment]::Is64BitOperatingSystem) { throw "Windows releases require x86_64" }
$asset = "reforge-windows-x86_64.zip"
$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("reforge-install-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $tempRoot | Out-Null
try {
    $archive = Join-Path $tempRoot $asset
    $sums = Join-Path $tempRoot "SHA256SUMS"
    Invoke-WebRequest -UseBasicParsing -Uri "$releaseBase/$Version/$asset" -OutFile $archive
    Invoke-WebRequest -UseBasicParsing -Uri "$releaseBase/$Version/SHA256SUMS" -OutFile $sums
    $checksumLine = Get-Content $sums | Where-Object { $_ -match ("\s\*?" + [regex]::Escape($asset) + "$") } | Select-Object -First 1
    if (-not $checksumLine) { throw "Checksum missing for $asset" }
    $expected = ($checksumLine -split '\s+')[0].ToLowerInvariant()
    $actual = (Get-FileHash -Algorithm SHA256 $archive).Hash.ToLowerInvariant()
    if ($actual -ne $expected) { throw "SHA-256 verification failed for $asset" }

    $unpacked = Join-Path $tempRoot "unpacked"
    Expand-Archive -Path $archive -DestinationPath $unpacked
    $binary = Join-Path $unpacked "reforge.exe"
    if (-not (Test-Path $binary -PathType Leaf)) { throw "Release archive does not contain reforge.exe" }
    $expectedVersion = $Version.Substring(1)
    $actualVersion = (& $binary --version | Out-String).Trim()
    if ($LASTEXITCODE -ne 0 -or $actualVersion -ne "reforge $expectedVersion") {
        throw "Downloaded binary reports '$actualVersion', expected 'reforge $expectedVersion'"
    }

    New-Item -ItemType Directory -Force -Path $BinDir | Out-Null
    $binaryStage = Join-Path $BinDir (".reforge." + [guid]::NewGuid().ToString("N") + ".tmp")
    Copy-Item $binary $binaryStage
    Move-Item -Force $binaryStage (Join-Path $BinDir "reforge.exe")

    if (-not $SkipSkill) {
        $skillSource = Join-Path $unpacked "skills\reforge-analyze"
        if (-not (Test-Path (Join-Path $skillSource "SKILL.md") -PathType Leaf)) { throw "Release archive does not contain reforge-analyze" }
        $codexRoot = if ($env:CODEX_HOME) { $env:CODEX_HOME } else { Join-Path $HOME ".codex" }
        $skillRoot = Join-Path $codexRoot "skills\reforge-analyze"
        New-Item -ItemType Directory -Force -Path $skillRoot | Out-Null
        $skillStage = Join-Path $skillRoot (".SKILL.md." + [guid]::NewGuid().ToString("N") + ".tmp")
        Copy-Item (Join-Path $skillSource "SKILL.md") $skillStage
        Move-Item -Force $skillStage (Join-Path $skillRoot "SKILL.md")
        $agents = Join-Path $skillSource "agents"
        if (Test-Path $agents -PathType Container) {
            $agentTarget = Join-Path $skillRoot "agents"
            New-Item -ItemType Directory -Force -Path $agentTarget | Out-Null
            Get-ChildItem $agents -File | ForEach-Object {
                $stage = Join-Path $agentTarget ("." + $_.Name + "." + [guid]::NewGuid().ToString("N") + ".tmp")
                Copy-Item $_.FullName $stage
                Move-Item -Force $stage (Join-Path $agentTarget $_.Name)
            }
        }
    }

    Write-Output "Installed reforge $expectedVersion to $(Join-Path $BinDir 'reforge.exe')"
    $pathEntries = $env:PATH -split [IO.Path]::PathSeparator
    if ($pathEntries -notcontains $BinDir) {
        Write-Output "$BinDir is not on PATH. Add it for your user account with:"
        Write-Output ('[Environment]::SetEnvironmentVariable("Path", "{0};" + [Environment]::GetEnvironmentVariable("Path", "User"), "User")' -f $BinDir)
    }
} finally {
    if (Test-Path $tempRoot) { Remove-Item -Recurse -Force $tempRoot }
}

