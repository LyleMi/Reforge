#!/usr/bin/env sh
set -eu
script_dir=$(CDPATH= cd "$(dirname "$0")" && pwd -P)
exec "$script_dir/install-agent-workflow.sh" --skills-only --only-scan --skip-agent "$@"
