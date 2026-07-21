#!/usr/bin/env sh
set -eu
script_dir=$(CDPATH= cd "$(dirname "$0")" && pwd -P)
repo_root=$(CDPATH= cd "$script_dir/.." && pwd -P)
test_root=$(mktemp -d)
trap 'rm -rf "$test_root"' EXIT HUP INT TERM

"$script_dir/install-agent-workflow.sh" --skills-only --skills-dir "$test_root/skills" --agent-dir "$test_root/agents" --skip-cli
for skill in reforge-scan reforge-plan reforge-apply reforge-verify; do test -f "$test_root/skills/$skill/SKILL.md"; done
test -f "$test_root/agents/reforge-investigator.toml"

if "$script_dir/install-agent-workflow.sh" --skills-only --skills-dir "$test_root/skills" --agent-dir "$test_root/agents" --skip-cli 2>/dev/null; then
    echo "update without --force unexpectedly succeeded" >&2
    exit 1
fi
"$script_dir/install-agent-workflow.sh" --skills-only --skills-dir "$test_root/skills" --agent-dir "$test_root/agents" --skip-cli --force

"$script_dir/install-agent-workflow.sh" --plugin --plugin-dir "$test_root/plugin" --skip-agent --skip-cli
test -f "$test_root/plugin/.codex-plugin/plugin.json"
test ! -e "$test_root/plugin/.codex/agents/reforge-investigator.toml"

"$script_dir/install-agent-skill.sh" --skills-dir "$test_root/legacy" --skip-cli
test -f "$test_root/legacy/reforge-scan/SKILL.md"

"$script_dir/install-agent-skill.sh" --skills-dir "$repo_root/skills" --source "$repo_root/skills/reforge-scan" --skip-cli

if "$script_dir/install-agent-skill.sh" --skills-dir "$test_root/missing" --source "$test_root/not-a-skill" --skip-cli 2>/dev/null; then
    echo "missing source unexpectedly succeeded" >&2
    exit 1
fi

echo "installer tests passed"
