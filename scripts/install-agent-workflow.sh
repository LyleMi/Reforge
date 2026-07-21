#!/usr/bin/env sh
set -eu

mode=plugin
force=0
install_cli=1
install_agent=1
only_scan=0
plugin_dir=""
skills_dir=""
agent_dir=""
scan_source=""
agent=codex
project_dir=""
root_dir=""

usage() {
    cat <<'EOF'
Usage: scripts/install-agent-workflow.sh [options]

  --plugin                    Install the standard plugin (default).
  --skills-only               Install skills without the plugin manifest.
  --plugin-dir DIR            Exact plugin destination.
  --skills-dir DIR            Exact skills parent directory.
  --agent-dir DIR             Exact custom-agent parent directory.
  --skip-agent                Do not install the investigator agent.
  --skip-cli                  Do not install the Reforge CLI.
  --force                     Atomically replace an existing installation.
  --only-scan                 Install only reforge-scan (compatibility mode).
  --source DIR                Custom reforge-scan source (compatibility mode).
  --agent NAME                Target agent: codex, claude, gemini, opencode,
                              codebuddy, cursor, generic, or all.
  --project-dir DIR           Install project-local files into DIR.
  --root-dir DIR              Override the selected agent's global root/config dir.
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --plugin) mode=plugin; shift ;;
        --skills-only) mode=skills; shift ;;
        --plugin-dir) plugin_dir="${2:?missing --plugin-dir value}"; shift 2 ;;
        --skills-dir) skills_dir="${2:?missing --skills-dir value}"; shift 2 ;;
        --agent-dir) agent_dir="${2:?missing --agent-dir value}"; shift 2 ;;
        --skip-agent) install_agent=0; shift ;;
        --skip-cli) install_cli=0; shift ;;
        --install-cli) install_cli=1; shift ;;
        --force) force=1; shift ;;
        --only-scan) only_scan=1; shift ;;
        --source) scan_source="${2:?missing --source value}"; shift 2 ;;
        --agent) agent="${2:?missing --agent value}"; shift 2 ;;
        --project-dir) project_dir="${2:?missing --project-dir value}"; shift 2 ;;
        --root-dir) root_dir="${2:?missing --root-dir value}"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "Unknown option: $1" >&2; usage >&2; exit 2 ;;
    esac
done

case "$agent" in codex|claude|gemini|opencode|codebuddy|cursor|generic|all) ;; *) echo "Unsupported agent: $agent" >&2; exit 2;; esac
requested_agent=$agent
script_dir=$(CDPATH= cd "$(dirname "$0")" && pwd -P)
repo_root=$(CDPATH= cd "$script_dir/.." && pwd -P)
codex_root=${CODEX_HOME:-${HOME:?HOME is required}/.codex}
[ -n "$root_dir" ] && codex_root="$root_dir"
[ -n "$skills_dir" ] || skills_dir="$codex_root/skills"
[ -n "$agent_dir" ] || agent_dir="$codex_root/agents"
[ -n "$plugin_dir" ] || plugin_dir="$codex_root/plugins/reforge"
[ -n "$scan_source" ] || scan_source="$repo_root/skills/reforge-scan"

for required in "$repo_root/.codex-plugin/plugin.json" "$scan_source/SKILL.md"; do
    [ -f "$required" ] || { echo "Missing workflow source: $required" >&2; exit 1; }
done

atomic_install_dir() {
    source_path=$1
    target_path=$2
    target_parent=$(dirname "$target_path")
    target_name=$(basename "$target_path")
    mkdir -p "$target_parent"
    parent_abs=$(CDPATH= cd "$target_parent" && pwd -P)
    target_abs="$parent_abs/$target_name"
    source_abs=$(CDPATH= cd "$source_path" && pwd -P)
    if [ "$source_abs" = "$target_abs" ]; then
        printf 'Source and target are the same folder; leaving %s unchanged\n' "$target_abs"
        return
    fi
    if [ -e "$target_abs" ] && [ "$force" -ne 1 ]; then
        echo "Installation already exists at $target_abs. Pass --force to update it." >&2
        exit 1
    fi
    stage="$parent_abs/.$target_name.stage.$$"
    backup="$parent_abs/.$target_name.backup.$$"
    trap 'rm -rf "$stage" "$backup"' EXIT HUP INT TERM
    mkdir "$stage"
    cp -R "$source_abs"/. "$stage"/
    if [ -e "$target_abs" ]; then mv "$target_abs" "$backup"; fi
    if mv "$stage" "$target_abs"; then
        [ ! -e "$backup" ] || rm -rf "$backup"
    else
        [ ! -e "$backup" ] || mv "$backup" "$target_abs"
        exit 1
    fi
    trap - EXIT HUP INT TERM
    printf 'Installed %s\n' "$target_abs"
}

selected_skills() {
    if [ "$only_scan" -eq 1 ]; then
        printf '%s\n' reforge-scan
    else
        printf '%s\n' reforge-scan reforge-plan reforge-apply reforge-verify
    fi
}

skill_source_for() {
    if [ "$1" = reforge-scan ] && [ "$only_scan" -eq 1 ]; then
        printf '%s\n' "$scan_source"
    else
        printf '%s\n' "$repo_root/skills/$1"
    fi
}

write_agent_instructions() {
    target_file=$1
    label=$2
    mkdir -p "$(dirname "$target_file")"
    cat > "$target_file" <<EOF
# Reforge Agent Workflow

Reforge is a Rust CLI for evidence-driven refactoring scans and approval-gated refactor workflows.

Use the installed Reforge skills when the user asks to scan, inspect, plan, apply, or verify maintainability/refactoring work:

- \`reforge-scan\`: run \`reforge scan <target> --progress never\` before broad cleanup or architecture recommendations.
- \`reforge-plan\`: investigate selected Reforge issues and produce workflow artifacts without editing source.
- \`reforge-apply\`: edit source only after an explicit approved Reforge workflow exists.
- \`reforge-verify\`: run checks and compare the rescan result after approved changes.

Keep Reforge workflow artifacts durable and schema-valid. Do not add suppressions, change thresholds, install dependencies, commit, push, or open pull requests unless the user explicitly asks.

Installed for: $label.
EOF
}

write_cursor_rule() {
    target_file=$1
    mkdir -p "$(dirname "$target_file")"
    cat > "$target_file" <<'EOF'
---
description: Reforge evidence-driven refactoring workflow
alwaysApply: true
---
# Reforge Agent Workflow

Use Reforge for maintainability/refactoring scans and approval-gated refactor workflows.

- Run `reforge scan <target> --progress never` before broad cleanup or architecture recommendations.
- Use `reforge workflow` artifacts for selected issues, plans, approvals, application, and verification.
- Do not edit source during planning. Edit only after an explicit approved Reforge workflow exists.
- Keep workflow artifacts durable and schema-valid.
- Do not add suppressions, change thresholds, install dependencies, commit, push, or open pull requests unless the user explicitly asks.
EOF
}

install_skill_set() {
    dest_parent=$1
    for skill in $(selected_skills); do
        target_skill="$dest_parent/$skill"
        if [ -e "$target_skill" ] && [ "$force" -ne 1 ] && grep -q "name: $skill" "$target_skill/SKILL.md" 2>/dev/null; then
            printf 'Skill already installed at %s\n' "$target_skill"
        else
            atomic_install_dir "$(skill_source_for "$skill")" "$target_skill"
        fi
    done
}

install_portable_agent() {
    target_agent=$1
    if [ -n "$project_dir" ]; then
        base=$project_dir
    else
        case "$target_agent" in
            claude) base="${root_dir:-${HOME:?HOME is required}/.claude}" ;;
            gemini) base="${root_dir:-${HOME:?HOME is required}/.gemini}" ;;
            opencode) base="${root_dir:-${XDG_CONFIG_HOME:-${HOME:?HOME is required}/.config}/opencode}" ;;
            codebuddy) base="${root_dir:-${HOME:?HOME is required}/.codebuddy}" ;;
            cursor) base="${root_dir:-${HOME:?HOME is required}/.cursor}" ;;
            generic) base="${root_dir:-${HOME:?HOME is required}/.agents}" ;;
            codex) base="${root_dir:-$codex_root}" ;;
        esac
    fi

    case "$target_agent:$project_dir" in
        claude:) instructions="$base/CLAUDE.md"; skills="$base/skills" ;;
        claude:*) instructions="$base/CLAUDE.md"; skills="$base/.claude/skills" ;;
        gemini:) instructions="$base/GEMINI.md"; skills="" ;;
        gemini:*) instructions="$base/GEMINI.md"; skills="" ;;
        opencode:) instructions="$base/AGENTS.md"; skills="$base/skills" ;;
        opencode:*) instructions="$base/AGENTS.md"; skills="$base/.opencode/skills" ;;
        codebuddy:) instructions="$base/CODEBUDDY.md"; skills="$base/skills" ;;
        codebuddy:*) instructions="$base/CODEBUDDY.md"; skills="$base/.codebuddy/skills" ;;
        cursor:) instructions="$base/rules/reforge.mdc"; skills="" ;;
        cursor:*) instructions="$base/.cursor/rules/reforge.mdc"; skills="" ;;
        generic:) instructions="$base/AGENTS.md"; skills="$base/skills" ;;
        generic:*) instructions="$base/AGENTS.md"; skills="$base/.agents/skills" ;;
        codex:) instructions="$base/AGENTS.md"; skills="$base/skills" ;;
        codex:*) instructions="$base/AGENTS.md"; skills="$base/.agents/skills" ;;
    esac

    if [ -e "$instructions" ] && [ "$force" -ne 1 ]; then
        if grep -q "Reforge Agent Workflow" "$instructions" 2>/dev/null; then
            printf 'Instructions already installed at %s\n' "$instructions"
            [ -z "$skills" ] || install_skill_set "$skills"
            return
        fi
        echo "Instructions already exist at $instructions. Pass --force to update it." >&2
        exit 1
    fi
    instruction_tmp="$(dirname "$instructions")/.$(basename "$instructions").stage.$$"
    if [ "$target_agent" = cursor ]; then
        write_cursor_rule "$instruction_tmp"
    else
        write_agent_instructions "$instruction_tmp" "$target_agent"
    fi
    mv "$instruction_tmp" "$instructions"
    printf 'Installed %s\n' "$instructions"
    [ -z "$skills" ] || install_skill_set "$skills"
}

if [ "$agent" = all ]; then
    for portable_agent in claude codex gemini opencode codebuddy cursor generic; do
        agent=$portable_agent root_dir="" install_portable_agent "$portable_agent"
    done
elif [ "$agent" != codex ] || [ -n "$project_dir" ]; then
    install_portable_agent "$agent"
else
if [ "$mode" = plugin ]; then
    stage_source=$(mktemp -d)
    mkdir -p "$stage_source/.codex-plugin" "$stage_source/skills" "$stage_source/.codex/agents"
    cp "$repo_root/.codex-plugin/plugin.json" "$stage_source/.codex-plugin/plugin.json"
    if [ "$only_scan" -eq 1 ]; then
        cp -R "$scan_source" "$stage_source/skills/reforge-scan"
    else
        for skill in reforge-scan reforge-plan reforge-apply reforge-verify; do cp -R "$repo_root/skills/$skill" "$stage_source/skills/$skill"; done
    fi
    if [ "$install_agent" -eq 1 ]; then cp "$repo_root/.codex/agents/reforge-investigator.toml" "$stage_source/.codex/agents/"; fi
    atomic_install_dir "$stage_source" "$plugin_dir"
    rm -rf "$stage_source"
else
    if [ "$only_scan" -eq 1 ]; then
        atomic_install_dir "$scan_source" "$skills_dir/reforge-scan"
    else
        for skill in reforge-scan reforge-plan reforge-apply reforge-verify; do atomic_install_dir "$repo_root/skills/$skill" "$skills_dir/$skill"; done
    fi
    if [ "$install_agent" -eq 1 ]; then
        mkdir -p "$agent_dir"
        agent_target="$agent_dir/reforge-investigator.toml"
        if [ -e "$agent_target" ] && [ "$force" -ne 1 ]; then echo "Agent already exists at $agent_target. Pass --force to update it." >&2; exit 1; fi
        agent_stage="$agent_dir/.reforge-investigator.toml.stage.$$"
        cp "$repo_root/.codex/agents/reforge-investigator.toml" "$agent_stage"
        mv "$agent_stage" "$agent_target"
    fi
fi
fi

if [ "$install_cli" -eq 1 ]; then
    command -v cargo >/dev/null 2>&1 || { echo "cargo is required; pass --skip-cli to omit the CLI" >&2; exit 1; }
    cargo install --path "$repo_root"
fi
