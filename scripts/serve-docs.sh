#!/usr/bin/env sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)

cd "$repo_root"
mkdir -p docs/sample
cargo run --locked -p reforge-cli -- analyze . --config .github/pages/reforge.toml --output html --output-file docs/sample/index.html --reproducible
mdbook serve --open
