#!/usr/bin/env sh
set -eu

repo_root=$(CDPATH= cd "$(dirname "$0")/.." && pwd -P)
git -C "$repo_root" config core.hooksPath .githooks
echo "Installed repository hooks. Pre-push now runs scripts/check-ci.sh."
