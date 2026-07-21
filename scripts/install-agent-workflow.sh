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
legacy_agent=codex

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
        --agent) legacy_agent="${2:?missing --agent value}"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "Unknown option: $1" >&2; usage >&2; exit 2 ;;
    esac
done

case "$legacy_agent" in codex|generic) ;; *) echo "Unsupported agent: $legacy_agent" >&2; exit 2;; esac
script_dir=$(CDPATH= cd "$(dirname "$0")" && pwd -P)
repo_root=$(CDPATH= cd "$script_dir/.." && pwd -P)
codex_root=${CODEX_HOME:-${HOME:?HOME is required}/.codex}
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

if [ "$install_cli" -eq 1 ]; then
    command -v cargo >/dev/null 2>&1 || { echo "cargo is required; pass --skip-cli to omit the CLI" >&2; exit 1; }
    cargo install --path "$repo_root"
fi
