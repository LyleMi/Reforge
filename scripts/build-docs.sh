#!/usr/bin/env sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
destination=${1:-target/docs-site}

cd "$repo_root"
mkdir -p docs/sample
cargo run --locked -p reforge -- analyze . --output html --output-file docs/sample/index.html --reproducible
mdbook build --dest-dir "$destination"
