#!/usr/bin/env sh
set -eu

repo_root=$(CDPATH= cd "$(dirname "$0")/.." && pwd -P)
cd "$repo_root"

echo "==> Rust formatting"
cargo fmt --all -- --check

echo "==> Rust lint"
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings

echo "==> Rust tests"
cargo test --locked --workspace --all-targets --all-features

echo "==> Rust workspace build"
cargo build --locked --workspace

echo "==> Installer distribution"
sh scripts/test-install-agent-workflow.sh
sh scripts/test-install.sh

echo "==> Report app"
(
    cd web/report-app
    npm ci
    npm test
    npm run build
    npm run test:e2e
)
cmp assets/report-app.js crates/reforge-output/assets/report-app.js
cmp assets/report-app.css crates/reforge-output/assets/report-app.css

echo "==> Release analyzer and full-rule self audit"
cargo build --locked --release -p reforge-cli
scripts/check-self-audit.sh target/release/reforge target/self-audit

echo "==> Package contents"
cargo package --locked --allow-dirty -p reforge-schema
cargo package --locked --allow-dirty --list -p reforge-output > target/reforge-output-package-files.txt
cargo package --locked --allow-dirty --list -p reforge-engine > target/reforge-engine-package-files.txt
cargo package --locked --allow-dirty --list -p reforge-cli > target/reforge-cli-package-files.txt
grep -Fx assets/report-app.css target/reforge-output-package-files.txt
grep -Fx assets/report-app.js target/reforge-output-package-files.txt
grep -Fx build.rs target/reforge-engine-package-files.txt
grep -Fx src/main.rs target/reforge-cli-package-files.txt

echo "==> Documentation"
if ! command -v mdbook >/dev/null 2>&1; then
    echo "error: mdbook 0.5.4 is required; install it with: cargo install mdbook --version 0.5.4 --locked" >&2
    exit 1
fi
test "$(mdbook --version)" = "mdbook v0.5.4"
sh scripts/build-docs.sh target/docs-site

echo "Local CI gate passed."
