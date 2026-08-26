import { fetchReport, loadSources } from "./data.js";
import { lineDiff, splitLines } from "./diff.js";
import { copy, scenarios } from "./scenarios.js";

const STORAGE_KEY = "reforge.locale";

const byId = id => document.getElementById(id);
const escapeHtml = value => String(value ?? "").replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");
const humanize = value => String(value ?? "").replaceAll("_", " ").replace(/\b\w/g, character => character.toUpperCase());

function normalizedLocale(value) {
  if (!value) return undefined;
  const normalized = value.toLowerCase();
  if (normalized === "zh" || normalized === "zh-cn") return "zh-CN";
  if (normalized === "en" || normalized.startsWith("en-")) return "en";
}

function initialLocale() {
  const query = normalizedLocale(new URLSearchParams(location.search).get("lang"));
  let stored;
  try { stored = normalizedLocale(localStorage.getItem(STORAGE_KEY)); } catch { /* optional storage */ }
  return query || stored || navigator.languages.map(normalizedLocale).find(Boolean) || "en";
}

let locale = initialLocale();
let scenarioId = new URLSearchParams(location.search).get("scenario");
if (!(scenarioId in scenarios)) scenarioId = "typescript-boundary-bypass";
let activeTab = "patch";
let activeFile = scenarios[scenarioId].entry;
let renderVersion = 0;
let currentSources;
let currentIssue;

function updateUrl() {
  const url = new URL(location.href);
  url.searchParams.set("scenario", scenarioId);
  url.searchParams.set("lang", locale);
  history.replaceState(null, "", url);
}

function evidenceLines(issue, path) {
  const lines = new Set();
  for (const evidence of issue?.evidence ?? []) for (const location of evidence.locations ?? []) if (location.path === path) lines.add(location.line ?? 0);
  return lines;
}

function renderCode(path, source, issue) {
  const relevant = evidenceLines(issue, path);
  return `<div class="source-file" data-source-path="${escapeHtml(path)}"><pre><code>${splitLines(source).map((line, index) => `<span class="code-line${relevant.has(index + 1) ? " evidence-line" : ""}" data-new-line="${index + 1}"><span class="line-number">${index + 1}</span><span class="line-code">${escapeHtml(line) || " "}</span></span>`).join("")}</code></pre></div>`;
}

function renderPatch(path, sources, issue) {
  const relevant = evidenceLines(issue, path);
  const rows = lineDiff(sources.before[path], sources.after[path]);
  return `<div class="source-file diff-file" data-source-path="${escapeHtml(path)}"><div class="diff-header"><span>${sources.before[path] === null ? "/dev/null" : `before/${path}`}</span><span>→</span><span>${sources.after[path] === null ? "/dev/null" : `after/${path}`}</span></div><pre><code>${rows.map(row => `<span class="code-line diff-${row.kind}${row.newLine && relevant.has(row.newLine) ? " evidence-line" : ""}"${row.newLine ? ` data-new-line="${row.newLine}"` : ""}><span class="diff-sign">${row.kind === "addition" ? "+" : row.kind === "deletion" ? "−" : " "}</span><span class="line-number old">${row.oldLine ?? ""}</span><span class="line-number new">${row.newLine ?? ""}</span><span class="line-code">${escapeHtml(row.text) || " "}</span></span>`).join("")}</code></pre></div>`;
}

function filesForTab(scenario, sources) {
  if (activeTab === "patch") return [...new Set([...scenario.beforeFiles, ...scenario.afterFiles])].filter(path => sources.before[path] !== sources.after[path]);
  if (activeTab === "context") return scenario.contextFiles;
  if (activeTab === "source") return scenario.afterFiles;
  return [];
}

function renderCodePanel() {
  if (!currentSources) return;
  const scenario = scenarios[scenarioId];
  const files = filesForTab(scenario, currentSources);
  if (files.length && !files.includes(activeFile)) activeFile = files[0];
  byId("file-tabs").hidden = files.length === 0;
  byId("file-tabs").innerHTML = files.map(path => `<button role="tab" data-file="${escapeHtml(path)}" aria-selected="${path === activeFile}" class="${evidenceLines(currentIssue, path).size ? "has-evidence" : ""}">${escapeHtml(path)}</button>`).join("");
  document.querySelectorAll("[data-file]").forEach(button => button.addEventListener("click", () => { activeFile = button.dataset.file; renderCodePanel(); }));
  if (activeTab === "patch") byId("patch-files").innerHTML = renderPatch(activeFile, currentSources, currentIssue);
  if (activeTab === "context") byId("context-files").innerHTML = renderCode(activeFile, currentSources.before[activeFile] ?? currentSources.after[activeFile], currentIssue);
  if (activeTab === "source") byId("source-files").innerHTML = renderCode(activeFile, currentSources.after[activeFile], currentIssue);
}

function renderTabs() {
  document.querySelectorAll("[data-tab]").forEach(button => button.setAttribute("aria-selected", String(button.dataset.tab === activeTab)));
  for (const tab of ["patch", "context", "source", "config", "command"]) byId(`${tab}-panel`).hidden = tab !== activeTab;
  renderCodePanel();
}

function endpoint(endpoint, role) {
  return `<div class="flow-endpoint"><span>${copy[locale][role]}</span><strong>${escapeHtml(endpoint.symbol.split("::").at(-1))}</strong><small>${escapeHtml(endpoint.path)}:${endpoint.line}</small></div>`;
}

function renderSpecialEvidence(issue, evidence) {
  const scenario = scenarios[scenarioId];
  if (scenario.visualization === "witness") {
    const witness = evidence.witness;
    byId("special-evidence").innerHTML = `<div class="witness"><p><strong>${copy[locale].exactWitness}</strong><span>${escapeHtml(witness.resolution)}</span></p>${endpoint(witness.source, "source")}<ol>${witness.ordered_steps.map((step, index) => `<li><i>${index + 1}</i><div><strong>${escapeHtml(step.operation)}</strong><small>${escapeHtml(step.path)}:${step.line} · ${escapeHtml(step.symbol)} · ${escapeHtml(step.resolution)}</small></div></li>`).join("")}</ol>${endpoint(witness.sink, "sink")}</div>`;
    return;
  }
  const members = issue.subject?.members ?? [];
  if (scenario.visualization === "group") {
    byId("special-evidence").innerHTML = `<div class="implementation-group"><p>${copy[locale].implementationGroup}</p><div>${members.map((member, index) => `<button data-evidence-path="${escapeHtml(member.path)}" data-evidence-line="1"><span>0${index + 1}</span><strong>${escapeHtml(member.path)}</strong><small>${escapeHtml(member.symbol)}</small></button>`).join("")}</div></div>`;
    return;
  }
  const paths = members.map(member => member.path);
  byId("special-evidence").innerHTML = `<div class="cycle-view"><p>${copy[locale].dependencyLoop}</p><div class="cycle-ring" aria-hidden="true">${paths.map((path, index) => `<span style="--index:${index}">${escapeHtml(path.split("/").at(-1))}</span>`).join("")}<i>↻</i></div><ol><li class="before-edge">${copy[locale].beforeDirection}: checkout.ts → pricing.ts → promotions.ts</li><li>${copy[locale].afterDirection}: ${scenario.relationships.map(([from, to]) => `${from.split("/").at(-1)} → ${to.split("/").at(-1)}`).join("; ")}</li></ol></div>`;
}

function renderReport(report) {
  const scenario = scenarios[scenarioId];
  const issue = report.issues.find(candidate => candidate.evidence?.some(item => item.rule === scenario.rule));
  if (!issue) throw new Error(`Missing ${scenario.rule}`);
  const evidence = issue.evidence.find(item => item.rule === scenario.rule);
  currentIssue = issue;
  byId("scenario-rule").textContent = evidence.rule;
  byId("issue-title").textContent = issue.title;
  byId("issue-guidance").textContent = issue.guidance;
  byId("evidence-message").textContent = evidence.message;
  const execution = report.coverage?.[scenario.analysis.toLowerCase()]?.rules?.[scenario.rule];
  const coverage = report.coverage?.[scenario.analysis.toLowerCase()]?.status;
  byId("issue-maturity").textContent = humanize(execution?.maturity ?? "unknown");
  byId("coverage-status").textContent = `${copy[locale].coverage}: ${humanize(coverage)}`;
  byId("measurements").innerHTML = (evidence.measurements ?? []).map(item => `<div><dt>${escapeHtml(item.name)}</dt><dd><strong>${item.value}</strong> ${escapeHtml(item.unit)}${item.threshold === undefined ? "" : `<small>${copy[locale].threshold} ${item.threshold}</small>`}</dd></div>`).join("");
  byId("evidence-locations").innerHTML = (evidence.locations ?? []).map(location => `<button data-evidence-path="${escapeHtml(location.path)}" data-evidence-line="${location.line ?? 0}">${escapeHtml(location.path)}${location.line ? `:${location.line}` : ""}${location.symbol ? `<span>${escapeHtml(location.symbol)}</span>` : ""}</button>`).join("");
  renderSpecialEvidence(issue, evidence);
  bindEvidenceLocations();
  byId("result-loading").hidden = true;
  byId("result-error").hidden = true;
  byId("result-content").hidden = false;
  renderCodePanel();
}

function bindEvidenceLocations() {
  document.querySelectorAll("[data-evidence-path]").forEach(button => button.addEventListener("click", () => {
    const path = button.dataset.evidencePath;
    const scenario = scenarios[scenarioId];
    const changed = currentSources && currentSources.before[path] !== currentSources.after[path];
    activeTab = changed ? "patch" : "source";
    activeFile = path;
    renderTabs();
    const line = Number(button.dataset.evidenceLine);
    const target = line ? document.querySelector(`[data-source-path="${CSS.escape(path)}"] [data-new-line="${line}"]`) : document.querySelector(`[data-source-path="${CSS.escape(path)}"]`);
    target?.scrollIntoView({ behavior: "smooth", block: "center" });
  }));
}

async function renderDetail() {
  const version = ++renderVersion;
  const scenario = scenarios[scenarioId];
  const text = scenario[locale];
  currentSources = undefined;
  currentIssue = undefined;
  for (const [id, value] of [["scenario-title", text.title], ["change-title", text.change], ["scenario-language", `${scenario.language} · ${scenario.analysis}`], ["maintainer-task", text.task], ["agent-chose", text.chose], ["repository-expects", text.expects], ["reforge-observed", text.observed], ["plain-explanation", text.explanation], ["context-note", text.context]]) byId(id).textContent = value;
  byId("report-link").href = `reports/${scenarioId}/?lang=${encodeURIComponent(locale)}`;
  byId("scenario-command").textContent = `reforge analyze "$PWD/playground/fixtures/${scenarioId}/after" \\\n  --config playground/fixtures/${scenarioId}/reforge.toml \\\n  --output html --output-file reforge-report.html --reproducible`;
  byId("result-loading").hidden = false;
  byId("result-error").hidden = true;
  byId("result-content").hidden = true;
  const [sourcesResult, reportResult] = await Promise.allSettled([loadSources(scenarioId), fetchReport(scenarioId)]);
  if (version !== renderVersion) return;
  if (sourcesResult.status === "fulfilled") {
    currentSources = sourcesResult.value;
    byId("scenario-config").textContent = currentSources.config;
    renderTabs();
  } else {
    byId("patch-files").textContent = sourcesResult.reason;
  }
  if (reportResult.status === "fulfilled") {
    try { renderReport(reportResult.value); } catch (error) { showReportError(error); }
  } else showReportError(reportResult.reason);
}

function showReportError(error) {
  byId("result-loading").hidden = true;
  byId("result-error").hidden = false;
  byId("result-error").textContent = `${copy[locale].reportError} (${error instanceof Error ? error.message : String(error)})`;
}

function render() {
  document.documentElement.lang = locale;
  document.title = locale === "zh-CN" ? "Reforge Agent 变更审查台" : "Reforge Agent Change Review";
  byId("locale").value = locale;
  document.querySelectorAll("[data-i18n]").forEach(node => { node.textContent = copy[locale][node.dataset.i18n]; });
  byId("scenario-cards").innerHTML = Object.entries(scenarios).map(([id, scenario], index) => `<button class="scenario-card" data-scenario="${id}" aria-pressed="${id === scenarioId}"><span class="scenario-number">0${index + 1}</span><strong>${escapeHtml(scenario[locale].title)}</strong><span>${escapeHtml(scenario[locale].card)}</span><small>${scenario.language} · ${scenario.analysis}<br>${escapeHtml(scenario.rule)}</small><i aria-hidden="true">→</i></button>`).join("");
  document.querySelectorAll("[data-scenario]").forEach(button => button.addEventListener("click", () => {
    scenarioId = button.dataset.scenario;
    activeTab = "patch";
    activeFile = scenarios[scenarioId].entry;
    currentSources = undefined;
    currentIssue = undefined;
    updateUrl();
    render();
    if (matchMedia("(max-width: 760px)").matches) byId("scenario-detail").scrollIntoView({ behavior: "smooth", block: "start" });
  }));
  renderTabs();
  void renderDetail();
}

document.querySelectorAll("[data-tab]").forEach(button => button.addEventListener("click", () => { activeTab = button.dataset.tab; renderTabs(); }));
byId("locale").addEventListener("change", event => {
  locale = event.target.value;
  try { localStorage.setItem(STORAGE_KEY, locale); } catch { /* visible switch still works */ }
  updateUrl();
  render();
});

render();
