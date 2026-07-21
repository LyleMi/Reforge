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

"$script_dir/install-agent-workflow.sh" --agent claude --project-dir "$test_root/claude-project" --skip-cli
test -f "$test_root/claude-project/CLAUDE.md"
test -f "$test_root/claude-project/.claude/skills/reforge-scan/SKILL.md"

"$script_dir/install-agent-workflow.sh" --agent gemini --project-dir "$test_root/gemini-project" --skip-cli
test -f "$test_root/gemini-project/GEMINI.md"

"$script_dir/install-agent-workflow.sh" --agent opencode --project-dir "$test_root/opencode-project" --skip-cli
test -f "$test_root/opencode-project/AGENTS.md"
test -f "$test_root/opencode-project/.opencode/skills/reforge-plan/SKILL.md"

"$script_dir/install-agent-workflow.sh" --agent codebuddy --project-dir "$test_root/codebuddy-project" --skip-cli
test -f "$test_root/codebuddy-project/CODEBUDDY.md"
test -f "$test_root/codebuddy-project/.codebuddy/skills/reforge-verify/SKILL.md"

"$script_dir/install-agent-workflow.sh" --agent cursor --project-dir "$test_root/cursor-project" --skip-cli
test -f "$test_root/cursor-project/.cursor/rules/reforge.mdc"

"$script_dir/install-agent-workflow.sh" --agent all --project-dir "$test_root/all-project" --skip-cli
test -f "$test_root/all-project/CLAUDE.md"
test -f "$test_root/all-project/GEMINI.md"
test -f "$test_root/all-project/AGENTS.md"
test -f "$test_root/all-project/CODEBUDDY.md"
test -f "$test_root/all-project/.cursor/rules/reforge.mdc"
test -f "$test_root/all-project/.claude/skills/reforge-scan/SKILL.md"
test -f "$test_root/all-project/.opencode/skills/reforge-scan/SKILL.md"
test -f "$test_root/all-project/.codebuddy/skills/reforge-scan/SKILL.md"
test -f "$test_root/all-project/.agents/skills/reforge-scan/SKILL.md"

"$script_dir/install-agent-skill.sh" --skills-dir "$test_root/legacy" --skip-cli
test -f "$test_root/legacy/reforge-scan/SKILL.md"

"$script_dir/install-agent-skill.sh" --skills-dir "$repo_root/skills" --source "$repo_root/skills/reforge-scan" --skip-cli

if "$script_dir/install-agent-skill.sh" --skills-dir "$test_root/missing" --source "$test_root/not-a-skill" --skip-cli 2>/dev/null; then
    echo "missing source unexpectedly succeeded" >&2
    exit 1
fi

echo "installer tests passed"
