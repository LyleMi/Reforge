[CmdletBinding()]
param(
    [string]$Destination = "target/docs-site",
    [switch]$SkipSample
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot

Push-Location $repoRoot
try {
    if (-not $SkipSample) {
        New-Item -ItemType Directory -Force "docs/sample" | Out-Null
        cargo run "--locked" "--" scan . "--output" html "--output-file" "docs/sample/index.html" "--progress" never "--color" never
        if ($LASTEXITCODE -ne 0) {
            throw "failed to generate the sample report"
        }
    }

    mdbook build --dest-dir $Destination
    if ($LASTEXITCODE -ne 0) {
        throw "mdBook build failed"
    }
}
finally {
    Pop-Location
}
