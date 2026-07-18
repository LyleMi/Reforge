# HTML Report App

Reforge's HTML report is implemented by the React + TypeScript report app. The
CLI still emits one self-contained offline `.html` file, with the interactive
UI provided by the frontend bundle.

## User Flow

Generate an HTML report explicitly:

```powershell
cargo run -- scan . --output html --output-file reforge-report.html --progress never
```

Or let the output file extension select HTML:

```powershell
cargo run -- scan . --output-file reforge-report.html --progress never
```

The resulting file contains the scan data, HTML shell, CSS, and JavaScript app
bundle. It can be opened directly in a browser without a local server and
without network access. Reforge creates missing parent directories in the
output path before writing the report.

## Architecture

The report path is:

1. Rust scanner collects source metrics and findings.
2. Scanner assembles a schema 20 `ScanReport`.
3. HTML output serializes the `ScanReport` as JSON.
4. Reforge writes an HTML shell containing that JSON payload.
5. The shell inlines the compiled React bundle and stylesheet.
6. React renders the visualization from the embedded report data.

The frontend must treat `ScanReport` as its data contract. When fields are
added, removed, or renamed, update `docs/report-schema.md` and the report app
together.

## Source and Build Flow

Frontend source lives in `web/report-app`. Use that package for React
components, TypeScript types, styling, and visualization behavior.

Frontend development requires Node.js `^20.19.0` or `>=22.12.0` and npm; CI
uses Node.js 22. The project uses its pinned Vite 8 dependency from
`package-lock.json`; install it with `npm ci` and run it through the package
scripts rather than relying on a global Vite installation.

Build the app after changing report UI code:

```powershell
cd web\report-app
npm ci
npm run build
```

The build is expected to refresh these checked-in assets:

- `assets/report-app.js`
- `assets/report-app.css`

Rust embeds those generated assets into the single-file HTML report. Commit the
frontend source changes and the regenerated assets together so `--output html`
uses the current React app. These two bundles are the repository's intentional
exception to the rule against committing generated build output.
