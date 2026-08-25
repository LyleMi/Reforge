#!/usr/bin/env sh
set -eu

script_dir=$(CDPATH= cd "$(dirname "$0")" && pwd -P)
repo_root=$(CDPATH= cd "$script_dir/.." && pwd -P)
preview_root=$(mktemp -d "${TMPDIR:-/tmp}/reforge-p-limit-preview.XXXXXX")
trap 'rm -rf "$preview_root"' EXIT HUP INT TERM

git clone --quiet https://github.com/sindresorhus/p-limit.git "$preview_root/source"
git -C "$preview_root/source" checkout --quiet --detach df476048d023ff868cd45b35ee47f5fb0ca2b25a
cargo build --locked --release -p reforge-cli --manifest-path "$repo_root/Cargo.toml"
"$repo_root/target/release/reforge" analyze "$preview_root/source" \
    --config "$repo_root/calibration/reforge.toml" \
    --analysis codebase \
    --analysis dataflow \
    --output html \
    --output-file "$preview_root/report.html" \
    --reproducible

(
    cd "$repo_root/web/report-app"
    REPORT_URL="file://$preview_root/report.html" node --input-type=module -e '
        import { chromium } from "playwright";
        const browser = await chromium.launch({ headless: true });
        const page = await browser.newPage({ viewport: { width: 1440, height: 1100 }, colorScheme: "light" });
        await page.goto(process.env.REPORT_URL);
        await page.locator(".issue").first().waitFor();
        await page.evaluate(() => document.querySelectorAll(".coverage").forEach(section =>
            [...section.children].slice(3).forEach(child => child.remove())));
        await page.screenshot({ path: "../../assets/report-preview.png" });
        await browser.close();
    '
)

printf '%s\n' "Generated assets/report-preview.png from sindresorhus/p-limit@df476048d023ff868cd45b35ee47f5fb0ca2b25a"
