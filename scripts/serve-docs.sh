#!/usr/bin/env sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)

cd "$repo_root"
mkdir -p docs/sample
cargo run --locked -- scan . --output html --output-file docs/sample/index.html --progress never --color never
mdbook serve --open
