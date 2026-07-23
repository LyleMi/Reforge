# HTML report

The offline React app renders the compact schema 26 report: Issues, nested
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
`assets/report-app.css`; the HTML renderer embeds both assets and requires no
server or network.
