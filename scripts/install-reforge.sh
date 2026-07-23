#!/usr/bin/env sh
set -eu

if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
    printf '%s\n' "Usage: scripts/install-reforge.sh [cargo-install-options]"
    printf '%s\n' "Installs the reforge binary and reforge-analyze skill."
    exit 0
fi

script_dir=$(CDPATH= cd "$(dirname "$0")" && pwd -P)
repo_root=$(CDPATH= cd "$script_dir/.." && pwd -P)

cargo install --path "$repo_root/tools/reforge" --locked "$@"

codex_root=${CODEX_HOME:-"$HOME/.codex"}
skill_root="$codex_root/skills/reforge-analyze"
mkdir -p "$skill_root"
cp "$repo_root/skills/reforge-analyze/SKILL.md" "$skill_root/SKILL.md"
if [ -d "$repo_root/skills/reforge-analyze/agents" ]; then
  mkdir -p "$skill_root/agents"
  cp "$repo_root/skills/reforge-analyze/agents/"* "$skill_root/agents/"
fi
