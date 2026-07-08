#!/usr/bin/env sh
set -eu

agent="codex"
skills_dir=""
source_dir=""
force=0
install_cli=0

usage() {
    cat <<'EOF'
Usage: scripts/install-agent-skill.sh [options]

Options:
  --agent codex|generic   Target agent layout. Defaults to codex.
  --skills-dir DIR        Directory that contains skill folders.
  --source DIR            Skill source folder. Defaults to skills/reforge-scan.
  --force                 Replace an existing reforge-scan skill.
  --install-cli           Run cargo install --path . after installing the skill.
  -h, --help              Show this help.
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --agent)
            agent="${2:?missing value for --agent}"
            shift 2
            ;;
        --skills-dir)
            skills_dir="${2:?missing value for --skills-dir}"
            shift 2
            ;;
        --source)
            source_dir="${2:?missing value for --source}"
            shift 2
            ;;
        --force)
            force=1
            shift
            ;;
        --install-cli)
            install_cli=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

case "$agent" in
    codex|generic) ;;
    *)
        echo "Unsupported agent: $agent" >&2
        exit 2
        ;;
esac

script_dir=$(CDPATH= cd "$(dirname "$0")" && pwd)
repo_root=$(CDPATH= cd "$script_dir/.." && pwd)

if [ -z "$source_dir" ]; then
    source_dir="$repo_root/skills/reforge-scan"
fi

if [ ! -f "$source_dir/SKILL.md" ]; then
    echo "Source is not a skill folder: $source_dir" >&2
    exit 1
fi

source_abs=$(CDPATH= cd "$source_dir" && pwd -P)

if [ -z "$skills_dir" ]; then
    if [ "$agent" = "codex" ]; then
        if [ -n "${CODEX_HOME:-}" ]; then
            codex_home="$CODEX_HOME"
        else
            codex_home="${HOME:?HOME is required when CODEX_HOME is unset}/.codex"
        fi
        skills_dir="$codex_home/skills"
    else
        echo "Pass --skills-dir when --agent generic is used." >&2
        exit 2
    fi
fi

mkdir -p "$skills_dir"
skills_abs=$(CDPATH= cd "$skills_dir" && pwd -P)
target_abs="$skills_abs/reforge-scan"

if [ "$source_abs" = "$target_abs" ]; then
    printf 'Source and target are the same folder; leaving %s unchanged\n' "$target_abs"
else
    if [ -e "$target_abs" ]; then
        if [ "$force" -ne 1 ]; then
            echo "Skill already exists at $target_abs. Pass --force to update it." >&2
            exit 1
        fi

        case "$target_abs" in
            "$skills_abs"/reforge-scan) ;;
            *)
                echo "Refusing to remove unexpected target: $target_abs" >&2
                exit 1
                ;;
        esac

        rm -rf "$target_abs"
    fi

    mkdir -p "$target_abs"
    cp -R "$source_abs"/. "$target_abs"/

    printf 'Installed reforge-scan skill to %s\n' "$target_abs"
fi

if [ "$install_cli" -eq 1 ]; then
    if ! command -v cargo >/dev/null 2>&1; then
        echo "cargo is required for --install-cli" >&2
        exit 1
    fi
    cargo install --path "$repo_root"
fi
