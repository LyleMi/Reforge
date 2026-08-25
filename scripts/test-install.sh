#!/usr/bin/env sh
set -eu

repo_root=$(CDPATH= cd "$(dirname "$0")/.." && pwd -P)
version=$(awk '
    $0 == "[workspace.package]" { in_package = 1; next }
    in_package && $1 == "version" { gsub(/"/, "", $3); print $3; exit }
' "$repo_root/Cargo.toml")
test -n "$version"
release_tag="v$version"
test_root=$(mktemp -d "${TMPDIR:-/tmp}/reforge-installer-test.XXXXXX")
trap 'rm -rf "$test_root"' EXIT HUP INT TERM
release_dir="$test_root/releases/$release_tag"
package_dir="$test_root/package"
mkdir -p "$release_dir" "$package_dir/skills/reforge-analyze/agents"

printf '%s\n' '#!/usr/bin/env sh' "printf '%s\\n' 'reforge $version'" > "$package_dir/reforge"
chmod +x "$package_dir/reforge"
printf '%s\n' 'installer fixture skill' > "$package_dir/skills/reforge-analyze/SKILL.md"
printf '%s\n' 'installer fixture agent' > "$package_dir/skills/reforge-analyze/agents/openai.yaml"
tar -czf "$release_dir/reforge-linux-x86_64.tar.gz" -C "$package_dir" .
checksum=$(sha256sum "$release_dir/reforge-linux-x86_64.tar.gz" | awk '{print $1}')
printf '%s  %s\n' "$checksum" 'reforge-linux-x86_64.tar.gz' > "$release_dir/SHA256SUMS"

export REFORGE_RELEASE_BASE_URL="file://$test_root/releases"
export REFORGE_LATEST_VERSION="$release_tag"
export CODEX_HOME="$test_root/codex"
export PATH="$test_root/bin:$PATH"
"$repo_root/scripts/install.sh" --bin-dir "$test_root/bin"
test "$("$test_root/bin/reforge" --version)" = "reforge $version"
test -f "$test_root/codex/skills/reforge-analyze/SKILL.md"

"$repo_root/scripts/install.sh" --version "$release_tag" --bin-dir "$test_root/bin"
skip_root="$test_root/skip-codex"
CODEX_HOME="$skip_root" "$repo_root/scripts/install.sh" --bin-dir "$test_root/skip-bin" --skip-skill
test ! -e "$skip_root/skills/reforge-analyze/SKILL.md"

printf '%s\n' '0000000000000000000000000000000000000000000000000000000000000000  reforge-linux-x86_64.tar.gz' > "$release_dir/SHA256SUMS"
if "$repo_root/scripts/install.sh" --bin-dir "$test_root/tampered-bin" >/dev/null 2>&1; then
    printf '%s\n' 'tampered checksum unexpectedly succeeded' >&2
    exit 1
fi

printf '%s\n' 'installer tests passed'
