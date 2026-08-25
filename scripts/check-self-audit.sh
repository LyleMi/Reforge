#!/usr/bin/env sh
set -eu

repo_root=$(CDPATH= cd "$(dirname "$0")/.." && pwd -P)
analyzer=${1:-"$repo_root/target/release/reforge"}
audit_dir=${2:-"$repo_root/target/self-audit"}

if ! command -v jq >/dev/null 2>&1; then
    echo "error: jq is required for the self-audit gate" >&2
    exit 1
fi

if [ ! -x "$analyzer" ]; then
    echo "error: analyzer is not executable: $analyzer" >&2
    echo "build it with: cargo build --locked --release -p reforge-cli" >&2
    exit 1
fi

mkdir -p "$audit_dir"

run_analysis() {
    "$analyzer" analyze "$repo_root" "$@" --reproducible
}

run_analysis --analysis codebase --output json --output-file "$audit_dir/codebase.json" --metrics-output "$audit_dir/metrics.json"
run_analysis --analysis codebase --output json --output-file "$audit_dir/codebase-repeat.json" --metrics-output "$audit_dir/metrics-repeat.json"
run_analysis --analysis dataflow --output json --output-file "$audit_dir/dataflow.json" --flow-ir-output "$audit_dir/flow-ir.json"
run_analysis --analysis dataflow --output json --output-file "$audit_dir/dataflow-repeat.json" --flow-ir-output "$audit_dir/flow-ir-repeat.json"
run_analysis --analysis codebase --analysis dataflow --output json --output-file "$audit_dir/combined.json"
run_analysis --analysis codebase --analysis dataflow --output json --output-file "$audit_dir/combined-repeat.json"

cmp "$audit_dir/codebase.json" "$audit_dir/codebase-repeat.json"
cmp "$audit_dir/dataflow.json" "$audit_dir/dataflow-repeat.json"
cmp "$audit_dir/combined.json" "$audit_dir/combined-repeat.json"
cmp "$audit_dir/metrics.json" "$audit_dir/metrics-repeat.json"
cmp "$audit_dir/flow-ir.json" "$audit_dir/flow-ir-repeat.json"

test "$(jq '.schema_version' "$audit_dir/codebase.json")" = "27"

issue_count=$(jq '.summary.issue_count' "$audit_dir/codebase.json")
if [ "$issue_count" != "0" ]; then
    echo "error: Codebase self-audit reported $issue_count issue(s):" >&2
    jq -r '.issues[] | "  - [\(.evidence[0].rule)] \(.title)"' "$audit_dir/codebase.json" >&2
    exit 1
fi

jq -S '[.issues[] | {id, content_fingerprint}] | sort_by(.id)' "$audit_dir/codebase.json" > "$audit_dir/codebase-issues.json"
jq -S '[.issues[] | {id, content_fingerprint}] | sort_by(.id)' "$audit_dir/dataflow.json" > "$audit_dir/dataflow-issues.json"
jq -S -s 'add | sort_by(.id)' "$audit_dir/codebase-issues.json" "$audit_dir/dataflow-issues.json" > "$audit_dir/isolated-union.json"
jq -S '[.issues[] | {id, content_fingerprint}] | sort_by(.id)' "$audit_dir/combined.json" > "$audit_dir/combined-issues.json"
cmp "$audit_dir/isolated-union.json" "$audit_dir/combined-issues.json"
jq -s -e '.[0].coverage.codebase == .[1].coverage.codebase' "$audit_dir/codebase.json" "$audit_dir/combined.json" >/dev/null
jq -s -e '.[0].coverage.dataflow == .[1].coverage.dataflow' "$audit_dir/dataflow.json" "$audit_dir/combined.json" >/dev/null
jq -e '
  .coverage.dataflow.status as $status
  | ($status == "observed" or $status == "partial")
  and (if $status == "partial" then (.coverage.dataflow.limitations | length) > 0 else true end)
  and all(
    .coverage.dataflow.languages[];
    if .status == "unsupported" then (.limitations | length) > 0 else true end
  )
' "$audit_dir/dataflow.json" >/dev/null

echo "Self-audit gate passed: deterministic schema 27 reports, zero Codebase issues, isolated/combined parity."
