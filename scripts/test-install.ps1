$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$testRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("reforge-installer-test-" + [guid]::NewGuid().ToString("N"))
$releaseDir = Join-Path $testRoot "releases\v0.2.0"
$packageDir = Join-Path $testRoot "package"
$server = $null
try {
    New-Item -ItemType Directory -Force $releaseDir, (Join-Path $packageDir "skills\reforge-analyze\agents") | Out-Null
    Copy-Item (Join-Path $repoRoot "target\debug\reforge.exe") (Join-Path $packageDir "reforge.exe")
    Set-Content (Join-Path $packageDir "skills\reforge-analyze\SKILL.md") "installer fixture skill"
    Set-Content (Join-Path $packageDir "skills\reforge-analyze\agents\openai.yaml") "installer fixture agent"
    $asset = Join-Path $releaseDir "reforge-windows-x86_64.zip"
    Compress-Archive -Path (Join-Path $packageDir "*") -DestinationPath $asset
    $checksum = (Get-FileHash -Algorithm SHA256 $asset).Hash.ToLowerInvariant()
    Set-Content (Join-Path $releaseDir "SHA256SUMS") "$checksum  reforge-windows-x86_64.zip"

    $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
    $listener.Start()
    $port = ([System.Net.IPEndPoint]$listener.LocalEndpoint).Port
    $listener.Stop()
    $server = Start-Process python -ArgumentList "-m", "http.server", "$port", "--bind", "127.0.0.1", "--directory", $testRoot -PassThru -WindowStyle Hidden
    Start-Sleep -Milliseconds 750

    $env:REFORGE_RELEASE_BASE_URL = "http://127.0.0.1:$port/releases"
    $env:REFORGE_LATEST_VERSION = "v0.2.0"
    $env:CODEX_HOME = Join-Path $testRoot "codex"
    $binDir = Join-Path $testRoot "bin"
    & (Join-Path $repoRoot "scripts\install.ps1") -BinDir $binDir
    if ((& (Join-Path $binDir "reforge.exe") --version) -ne "reforge 0.2.0") { throw "installed binary version mismatch" }
    if (-not (Test-Path (Join-Path $env:CODEX_HOME "skills\reforge-analyze\SKILL.md"))) { throw "skill was not installed" }

    & (Join-Path $repoRoot "scripts\install.ps1") -Version v0.2.0 -BinDir $binDir
    $skipRoot = Join-Path $testRoot "skip-codex"
    $env:CODEX_HOME = $skipRoot
    & (Join-Path $repoRoot "scripts\install.ps1") -BinDir (Join-Path $testRoot "skip-bin") -SkipSkill
    if (Test-Path (Join-Path $skipRoot "skills\reforge-analyze\SKILL.md")) { throw "-SkipSkill installed a skill" }

    Set-Content (Join-Path $releaseDir "SHA256SUMS") (('0' * 64) + "  reforge-windows-x86_64.zip")
    $failed = $false
    try {
        & (Join-Path $repoRoot "scripts\install.ps1") -BinDir (Join-Path $testRoot "tampered-bin")
    } catch {
        $failed = $_.Exception.Message -match "SHA-256 verification failed"
    }
    if (-not $failed) { throw "tampered checksum unexpectedly succeeded" }
    Write-Output "installer tests passed"
} finally {
    if ($server -and -not $server.HasExited) { Stop-Process -Id $server.Id -Force }
    Remove-Item Env:REFORGE_RELEASE_BASE_URL -ErrorAction SilentlyContinue
    Remove-Item Env:REFORGE_LATEST_VERSION -ErrorAction SilentlyContinue
    if (Test-Path $testRoot) { Remove-Item -Recurse -Force $testRoot }
}

