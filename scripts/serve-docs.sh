#!/usr/bin/env sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)

cd "$repo_root"
mkdir -p docs/sample
cargo run --locked -p reforge -- analyze . --output html --output-file docs/sample/index.html --reproducible
mdbook serve --open
