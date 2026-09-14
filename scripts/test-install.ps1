$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$metadata = cargo metadata --manifest-path (Join-Path $repoRoot "Cargo.toml") --locked --no-deps --format-version 1 | ConvertFrom-Json
$version = ($metadata.packages | Where-Object name -eq "reforge-cli").version
$releaseTag = "v$version"
$testRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("reforge-installer-test-" + [guid]::NewGuid().ToString("N"))
$releaseDir = Join-Path $testRoot "releases\$releaseTag"
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

    $python = Get-Command python -ErrorAction SilentlyContinue
    if (-not $python) { throw "Python is required to serve release fixtures for the installer test" }
    $serverOut = Join-Path $testRoot "http-server.out.log"
    $serverErr = Join-Path $testRoot "http-server.err.log"
    $server = Start-Process $python.Source -ArgumentList "-m", "http.server", "$port", "--bind", "127.0.0.1", "--directory", $testRoot -PassThru -WindowStyle Hidden -RedirectStandardOutput $serverOut -RedirectStandardError $serverErr

    $deadline = (Get-Date).AddSeconds(30)
    while ($true) {
        if ($server.HasExited) {
            throw "Release fixture server exited with code $($server.ExitCode). stderr: $(Get-Content $serverErr -Raw -ErrorAction SilentlyContinue)"
        }
        $ready = $false
        $probe = [System.Net.Sockets.TcpClient]::new()
        try {
            $ready = $probe.ConnectAsync("127.0.0.1", $port).Wait(250) -and $probe.Connected
        } catch {
            $ready = $false
        } finally {
            $probe.Dispose()
        }
        if ($ready) { break }
        if ((Get-Date) -ge $deadline) {
            throw "Release fixture server did not accept connections on 127.0.0.1:$port within 30s. stderr: $(Get-Content $serverErr -Raw -ErrorAction SilentlyContinue)"
        }
        Start-Sleep -Milliseconds 100
    }

    $env:REFORGE_RELEASE_BASE_URL = "http://127.0.0.1:$port/releases"
    $env:REFORGE_LATEST_VERSION = $releaseTag
    $env:CODEX_HOME = Join-Path $testRoot "codex"
    $binDir = Join-Path $testRoot "bin"
    & (Join-Path $repoRoot "scripts\install.ps1") -BinDir $binDir
    if ((& (Join-Path $binDir "reforge.exe") --version) -ne "reforge $version") { throw "installed binary version mismatch" }
    if (-not (Test-Path (Join-Path $env:CODEX_HOME "skills\reforge-analyze\SKILL.md"))) { throw "skill was not installed" }

    & (Join-Path $repoRoot "scripts\install.ps1") -Version $releaseTag -BinDir $binDir
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
