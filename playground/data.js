import { scenarios } from "./scenarios.js";

const reportCache = new Map();
const sourceCache = new Map();

export async function fetchReport(id) {
  if (!reportCache.has(id)) reportCache.set(id, fetch(`reports/${id}/index.html`).then(async response => {
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const parsed = new DOMParser().parseFromString(await response.text(), "text/html");
    const data = parsed.getElementById("reforge-report-data")?.textContent;
    if (!data) throw new Error("Missing #reforge-report-data");
    const report = JSON.parse(data);
    if (report.schema_version !== 27 || !Array.isArray(report.issues)) throw new Error("Unsupported report schema");
    return report;
  }));
  return reportCache.get(id);
}

async function fetchText(path) {
  const response = await fetch(path);
  if (!response.ok) throw new Error(`${response.status} ${path}`);
  return response.text();
}

export async function loadSources(id) {
  if (!sourceCache.has(id)) {
    const scenario = scenarios[id];
    const paths = [...new Set([...scenario.beforeFiles, ...scenario.afterFiles])];
    sourceCache.set(id, Promise.all([
      fetchText(`fixtures/${id}/reforge.toml`).then(value => ["config", value]),
      ...scenario.beforeFiles.map(path => fetchText(`fixtures/${id}/before/${path}`).then(value => [`before:${path}`, value])),
      ...scenario.afterFiles.map(path => fetchText(`fixtures/${id}/after/${path}`).then(value => [`after:${path}`, value])),
    ]).then(entries => {
      const values = Object.fromEntries(entries);
      return { config: values.config, before: Object.fromEntries(paths.map(path => [path, values[`before:${path}`] ?? null])), after: Object.fromEntries(paths.map(path => [path, values[`after:${path}`] ?? null])) };
    }));
  }
  return sourceCache.get(id);
}
