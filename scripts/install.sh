#!/usr/bin/env sh
set -eu

repository=${REFORGE_REPOSITORY:-LyleMi/Reforge}
release_base=${REFORGE_RELEASE_BASE_URL:-https://github.com/$repository/releases/download}
version=""
bin_dir=${REFORGE_INSTALL_DIR:-"$HOME/.local/bin"}
skip_skill=0

usage() {
    printf '%s\n' "Usage: install.sh [--version TAG] [--bin-dir PATH] [--skip-skill]"
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --version)
            [ "$#" -ge 2 ] || { printf '%s\n' "error: --version requires a tag" >&2; exit 2; }
            version=$2
            shift 2
            ;;
        --bin-dir)
            [ "$#" -ge 2 ] || { printf '%s\n' "error: --bin-dir requires a path" >&2; exit 2; }
            bin_dir=$2
            shift 2
            ;;
        --skip-skill)
            skip_skill=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            printf '%s\n' "error: unknown option: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

command -v curl >/dev/null 2>&1 || { printf '%s\n' "error: curl is required" >&2; exit 1; }
command -v tar >/dev/null 2>&1 || { printf '%s\n' "error: tar is required" >&2; exit 1; }

if [ -z "$version" ]; then
    if [ -n "${REFORGE_LATEST_VERSION:-}" ]; then
        version=$REFORGE_LATEST_VERSION
    else
        latest_url=$(curl -fsSL -o /dev/null -w '%{url_effective}' "https://github.com/$repository/releases/latest")
        version=${latest_url##*/}
    fi
fi
case "$version" in
    v[0-9]*) ;;
    *) printf '%s\n' "error: invalid release tag: $version" >&2; exit 1 ;;
esac

case "$(uname -s)" in
    Linux) platform=linux ;;
    Darwin) platform=macos ;;
    *) printf '%s\n' "error: unsupported operating system: $(uname -s)" >&2; exit 1 ;;
esac
case "$(uname -m)" in
    x86_64|amd64) architecture=x86_64 ;;
    arm64|aarch64) architecture=aarch64 ;;
    *) printf '%s\n' "error: unsupported CPU architecture: $(uname -m)" >&2; exit 1 ;;
esac
if [ "$platform" = linux ] && [ "$architecture" != x86_64 ]; then
    printf '%s\n' "error: Linux releases currently support x86_64 only" >&2
    exit 1
fi

asset="reforge-$platform-$architecture.tar.gz"
tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/reforge-install.XXXXXX")
trap 'rm -rf "$tmp_dir"' EXIT HUP INT TERM

curl -fsSL "$release_base/$version/$asset" -o "$tmp_dir/$asset"
curl -fsSL "$release_base/$version/SHA256SUMS" -o "$tmp_dir/SHA256SUMS"
expected=$(awk -v name="$asset" '$2 == name || $2 == "*" name { print $1; exit }' "$tmp_dir/SHA256SUMS")
[ -n "$expected" ] || { printf '%s\n' "error: checksum missing for $asset" >&2; exit 1; }
if command -v sha256sum >/dev/null 2>&1; then
    actual=$(sha256sum "$tmp_dir/$asset" | awk '{print $1}')
else
    actual=$(shasum -a 256 "$tmp_dir/$asset" | awk '{print $1}')
fi
[ "$actual" = "$expected" ] || { printf '%s\n' "error: SHA-256 verification failed for $asset" >&2; exit 1; }

mkdir "$tmp_dir/unpacked"
tar -xzf "$tmp_dir/$asset" -C "$tmp_dir/unpacked"
[ -f "$tmp_dir/unpacked/reforge" ] || { printf '%s\n' "error: release archive does not contain reforge" >&2; exit 1; }
chmod +x "$tmp_dir/unpacked/reforge"
expected_version=${version#v}
actual_version=$($tmp_dir/unpacked/reforge --version)
[ "$actual_version" = "reforge $expected_version" ] || {
    printf '%s\n' "error: downloaded binary reports '$actual_version', expected 'reforge $expected_version'" >&2
    exit 1
}

mkdir -p "$bin_dir"
binary_stage="$bin_dir/.reforge.$$.tmp"
cp "$tmp_dir/unpacked/reforge" "$binary_stage"
chmod +x "$binary_stage"
mv -f "$binary_stage" "$bin_dir/reforge"

if [ "$skip_skill" -eq 0 ]; then
    skill_source="$tmp_dir/unpacked/skills/reforge-analyze"
    [ -f "$skill_source/SKILL.md" ] || { printf '%s\n' "error: release archive does not contain reforge-analyze" >&2; exit 1; }
    codex_root=${CODEX_HOME:-"$HOME/.codex"}
    skill_root="$codex_root/skills/reforge-analyze"
    mkdir -p "$skill_root"
    skill_stage="$skill_root/.SKILL.md.$$.tmp"
    cp "$skill_source/SKILL.md" "$skill_stage"
    mv -f "$skill_stage" "$skill_root/SKILL.md"
    if [ -d "$skill_source/agents" ]; then
        mkdir -p "$skill_root/agents"
        for source in "$skill_source"/agents/*; do
            [ -f "$source" ] || continue
            name=${source##*/}
            agent_stage="$skill_root/agents/.$name.$$.tmp"
            cp "$source" "$agent_stage"
            mv -f "$agent_stage" "$skill_root/agents/$name"
        done
    fi
fi

printf '%s\n' "Installed reforge $expected_version to $bin_dir/reforge"
case ":$PATH:" in
    *":$bin_dir:"*) ;;
    *)
        printf '%s\n' "$bin_dir is not on PATH. Add this line to your shell profile:"
        printf '%s\n' "export PATH=\"$bin_dir:\$PATH\""
        ;;
esac

