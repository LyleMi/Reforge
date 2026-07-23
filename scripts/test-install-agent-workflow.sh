#!/usr/bin/env sh
set -eu
script_dir=$(CDPATH= cd "$(dirname "$0")" && pwd -P)
repo_root=$(CDPATH= cd "$script_dir/.." && pwd -P)
workflow_installer="$script_dir/install-agent-workflow.sh"
canonical_installer="$script_dir/install-reforge.sh"
test_root=$(mktemp -d)
trap 'rm -rf "$test_root"' EXIT HUP INT TERM

run_workflow_installer() {
    "$workflow_installer" "$@"
}

help_output=$(run_workflow_installer --help)
printf '%s\n' "$help_output" | grep -q '^Usage: scripts/install-agent-workflow.sh \[options\]$'
printf '%s\n' "$help_output" | grep -q -- '-h, --help'
run_workflow_installer -h >/dev/null
"$canonical_installer" --help >/dev/null

run_workflow_installer --skills-only --skills-dir "$test_root/skills" --agent-dir "$test_root/agents" --skip-cli
test -f "$test_root/skills/reforge-analyze/SKILL.md"
test ! -e "$test_root/skills/reforge-plan"
test -f "$test_root/agents/reforge-investigator.toml"
grep -q 'report schema `26`' "$test_root/skills/reforge-analyze/SKILL.md"
grep -q 'artifact schema 5' "$test_root/agents/reforge-investigator.toml"
if grep -Eqi 'legacy issue envelope|schema-v3 InvestigationArtifact' "$test_root/skills/reforge-analyze/SKILL.md" "$test_root/agents/reforge-investigator.toml"; then
    echo "installed workflow contains a stale schema contract" >&2
    exit 1
fi

if run_workflow_installer --skills-only --skills-dir "$test_root/skills" --agent-dir "$test_root/agents" --skip-cli 2>/dev/null; then
    echo "update without --force unexpectedly succeeded" >&2
    exit 1
fi
run_workflow_installer --skills-only --skills-dir "$test_root/skills" --agent-dir "$test_root/agents" --skip-cli --force

run_workflow_installer --plugin --plugin-dir "$test_root/plugin" --skip-agent --skip-cli
test -f "$test_root/plugin/.codex-plugin/plugin.json"
test -f "$test_root/plugin/.codex-plugin/bundle.json"
test ! -e "$test_root/plugin/.codex/agents/reforge-investigator.toml"

run_workflow_installer --agent claude --project-dir "$test_root/claude-project" --skip-cli
test -f "$test_root/claude-project/CLAUDE.md"
test -f "$test_root/claude-project/.claude/skills/reforge-analyze/SKILL.md"

run_workflow_installer --agent gemini --project-dir "$test_root/gemini-project" --skip-cli
test -f "$test_root/gemini-project/GEMINI.md"

run_workflow_installer --agent opencode --project-dir "$test_root/opencode-project" --skip-cli
test -f "$test_root/opencode-project/AGENTS.md"
test -f "$test_root/opencode-project/.opencode/skills/reforge-analyze/SKILL.md"

run_workflow_installer --agent codebuddy --project-dir "$test_root/codebuddy-project" --skip-cli
test -f "$test_root/codebuddy-project/CODEBUDDY.md"
test -f "$test_root/codebuddy-project/.codebuddy/skills/reforge-analyze/SKILL.md"

run_workflow_installer --skills-only --skills-dir "$test_root/guard-skills" --agent-dir "$test_root/guard-agents" --skip-cli --with-guard
test -f "$test_root/guard-skills/reforge-plan/SKILL.md"
test -f "$test_root/guard-skills/reforge-apply/SKILL.md"
test -f "$test_root/guard-skills/reforge-verify/SKILL.md"

run_workflow_installer --agent cursor --project-dir "$test_root/cursor-project" --skip-cli
test -f "$test_root/cursor-project/.cursor/rules/reforge.mdc"

run_workflow_installer --agent all --project-dir "$test_root/all-project" --skip-cli
test -f "$test_root/all-project/CLAUDE.md"
test -f "$test_root/all-project/GEMINI.md"
test -f "$test_root/all-project/AGENTS.md"
test -f "$test_root/all-project/CODEBUDDY.md"
test -f "$test_root/all-project/.cursor/rules/reforge.mdc"
test -f "$test_root/all-project/.claude/skills/reforge-analyze/SKILL.md"
test -f "$test_root/all-project/.opencode/skills/reforge-analyze/SKILL.md"
test -f "$test_root/all-project/.codebuddy/skills/reforge-analyze/SKILL.md"
test -f "$test_root/all-project/.agents/skills/reforge-analyze/SKILL.md"
grep -q 'report schema `26`' "$test_root/all-project/.agents/skills/reforge-analyze/SKILL.md"

for portable_agent in claude gemini opencode codebuddy cursor generic; do
    portable_root="$test_root/$portable_agent-global"
    run_workflow_installer --agent "$portable_agent" --root-dir "$portable_root" --skip-cli
done
test -f "$test_root/claude-global/CLAUDE.md"
test -f "$test_root/claude-global/skills/reforge-analyze/SKILL.md"
test -f "$test_root/gemini-global/GEMINI.md"
test ! -e "$test_root/gemini-global/skills"
test -f "$test_root/opencode-global/AGENTS.md"
test -f "$test_root/opencode-global/skills/reforge-analyze/SKILL.md"
test -f "$test_root/codebuddy-global/CODEBUDDY.md"
test -f "$test_root/codebuddy-global/skills/reforge-analyze/SKILL.md"
test -f "$test_root/cursor-global/rules/reforge.mdc"
test ! -e "$test_root/cursor-global/skills"
test -f "$test_root/generic-global/AGENTS.md"
test -f "$test_root/generic-global/skills/reforge-analyze/SKILL.md"

echo "installer tests passed"
