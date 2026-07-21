[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot

Push-Location $repoRoot
try {
    New-Item -ItemType Directory -Force "docs/sample" | Out-Null
    cargo run "--locked" "--" scan . "--output" html "--output-file" "docs/sample/index.html" "--progress" never "--color" never
    if ($LASTEXITCODE -ne 0) {
        throw "failed to generate the sample report"
    }

    mdbook serve --open
    if ($LASTEXITCODE -ne 0) {
        throw "mdBook server failed"
    }
}
finally {
    Pop-Location
}
