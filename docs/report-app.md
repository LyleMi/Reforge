# HTML report

The offline React app renders a Reforge report: Issues, nested
Evidence and measurements, typed Dataflow witnesses, per-analysis coverage,
suppression totals, and optional baseline comparison. It does not render raw
metrics, Flow IR, arbitrary JSON extensions, or internal ontology fields.

After frontend changes run:

```sh
cd web/report-app
npm ci
npm test
npm run test:e2e
npm run build
```

Commit the source together with regenerated `assets/report-app.js` and
`assets/report-app.css` plus their synchronized `crates/reforge-output/assets`
copies; the HTML renderer embeds the package-local assets and requires no
server or network.
