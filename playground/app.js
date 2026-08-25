const STORAGE_KEY = "reforge.locale";

const copy = {
  en: {
    language: "Language", docs: "Documentation", headline: "Read the evidence.\nShape the code.", scenarios: "Choose a scenario",
    intro: "Explore three focused findings generated from real source. Nothing runs in your browser and no code leaves your machine.",
    stepOne: "01 / Choose evidence", workspace: "02 / Inspect source", stepThree: "03 / Review finding",
    openReport: "Open full report", reviewQuestion: "Review question", source: "Source", configuration: "Config", reproduce: "Command",
    evidence: "Evidence", footer: "Small, deliberate examples. Real Reforge output.", loading: "Loading source…",
    loadingReport: "Loading generated evidence…", reportError: "The generated report could not be loaded. Source and local reproduction remain available.",
    commandNote: "Run from the Reforge repository after building the CLI.", threshold: "threshold", fileLevel: "file-level evidence",
  },
  "zh-CN": {
    language: "语言", docs: "文档", headline: "读懂证据，\n重塑代码。", scenarios: "选择分析场景",
    intro: "通过三个真实源码生成的聚焦问题，直接理解 Reforge 的分析结果。浏览器不会运行分析，代码也不会离开本机。",
    stepOne: "01 / 选择证据", workspace: "02 / 检查源码", stepThree: "03 / 审查问题",
    openReport: "打开完整报告", reviewQuestion: "审查问题", source: "源码", configuration: "配置", reproduce: "命令",
    evidence: "证据", footer: "小而明确的示例，真实的 Reforge 输出。", loading: "正在载入源码…",
    loadingReport: "正在载入分析证据…", reportError: "无法载入生成的报告，但仍可查看源码并在本地复现。",
    commandNote: "构建 CLI 后，从 Reforge 仓库根目录运行。", threshold: "阈值", fileLevel: "文件级证据",
  },
};

const scenarios = {
  "rust-similarity": {
    language: "Rust", rule: "reforge.codebase.similar_functions", files: ["src/providers.rs"],
    en: { title: "Provider adapters that drift together", card: "Three structurally similar adapters", question: "Would a shared request builder reduce coordinated edits without hiding provider-specific behavior?" },
    "zh-CN": { title: "共同漂移的 Provider Adapter", card: "三个结构相似的适配器", question: "共享的请求构建器能否减少同步修改，同时保留各 provider 的特有行为？" },
  },
  "typescript-cycle": {
    language: "TypeScript", rule: "reforge.codebase.dependency_cycle", files: ["src/checkout.ts", "src/pricing.ts", "src/promotions.ts"],
    en: { title: "A checkout dependency loop", card: "A three-file dependency cycle", question: "Which module should own the shared decision so the checkout flow has a one-way dependency direction?" },
    "zh-CN": { title: "Checkout 依赖环", card: "三文件依赖环", question: "哪个模块应拥有共享决策，才能让结账流程形成单向依赖？" },
  },
  "python-long-function": {
    language: "Python", rule: "reforge.codebase.long_function", files: ["orders.py"],
    en: { title: "An order processor with four jobs", card: "Parsing, validation, pricing, persistence", question: "Which responsibility changes for a different reason and deserves a named boundary first?" },
    "zh-CN": { title: "承担四项职责的订单处理函数", card: "解析、校验、定价与持久化", question: "哪项职责会因不同原因变化，最应该先形成具名边界？" },
  },
};

const reportCache = new Map();
const sourceCache = new Map();
const byId = id => document.getElementById(id);
const escapeHtml = value => value.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");
const label = value => value.replaceAll("_", " ").replace(/\b\w/g, character => character.toUpperCase());

function normalizedLocale(value) {
  if (!value) return undefined;
  const normalized = value.toLowerCase();
  if (normalized === "zh" || normalized === "zh-cn") return "zh-CN";
  if (normalized === "en" || normalized.startsWith("en-")) return "en";
}

function initialLocale() {
  const query = normalizedLocale(new URLSearchParams(location.search).get("lang"));
  let stored;
  try { stored = normalizedLocale(localStorage.getItem(STORAGE_KEY)); } catch { /* storage can be unavailable */ }
  return query || stored || navigator.languages.map(normalizedLocale).find(Boolean) || "en";
}

let locale = initialLocale();
let scenarioId = new URLSearchParams(location.search).get("scenario");
if (!(scenarioId in scenarios)) scenarioId = "rust-similarity";
let activeTab = "source";
let activeFile = scenarios[scenarioId].files[0];
let renderVersion = 0;

function updateUrl() {
  const url = new URL(location.href);
  url.searchParams.set("scenario", scenarioId);
  url.searchParams.set("lang", locale);
  history.replaceState(null, "", url);
}

async function loadReport(id) {
  if (!reportCache.has(id)) reportCache.set(id, fetch(`reports/${id}/index.html`).then(async response => {
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const document = new DOMParser().parseFromString(await response.text(), "text/html");
    const data = document.getElementById("reforge-report-data")?.textContent;
    if (!data) throw new Error("Missing #reforge-report-data");
    const report = JSON.parse(data);
    if (report.schema_version !== 27 || !Array.isArray(report.issues)) throw new Error("Unsupported report schema");
    return report;
  }));
  return reportCache.get(id);
}

async function loadSources(id) {
  if (!sourceCache.has(id)) {
    const scenario = scenarios[id];
    sourceCache.set(id, Promise.all(["reforge.toml", ...scenario.files].map(async path => {
      const response = await fetch(`fixtures/${id}/${path}`);
      if (!response.ok) throw new Error(`${response.status} ${path}`);
      return [path, await response.text()];
    })).then(entries => Object.fromEntries(entries)));
  }
  return sourceCache.get(id);
}

function evidenceLines(issue, path) {
  const matches = [];
  for (const evidence of issue?.evidence ?? []) for (const location of evidence.locations ?? []) {
    if (location.path === path || location.path.endsWith(`/${path}`) || path.endsWith(`/${location.path}`)) matches.push(location.line ?? 0);
  }
  return new Set(matches);
}

function renderSource(path, source, issue) {
  const relevant = evidenceLines(issue, path);
  const fileLevel = relevant.has(0);
  const lines = source.replace(/\n$/, "").split("\n");
  byId("source-files").innerHTML = `<div class="source-file${fileLevel ? " file-evidence" : ""}" data-file="${escapeHtml(path)}">${fileLevel ? `<div class="file-evidence-label">${copy[locale].fileLevel}</div>` : ""}<pre><code>${lines.map((line, index) => `<span class="code-line${relevant.has(index + 1) ? " evidence-line" : ""}"><span class="line-number">${index + 1}</span><span class="line-code">${escapeHtml(line) || " "}</span></span>`).join("")}</code></pre></div>`;
}

function renderFileTabs(files, issue, sources) {
  byId("file-tabs").innerHTML = files.map(path => `<button role="tab" data-file-tab="${escapeHtml(path)}" aria-selected="${path === activeFile}" class="${evidenceLines(issue, path).size ? "has-evidence" : ""}">${escapeHtml(path)}</button>`).join("");
  document.querySelectorAll("[data-file-tab]").forEach(button => button.addEventListener("click", () => {
    activeFile = button.dataset.fileTab;
    renderFileTabs(files, issue, sources);
    renderSource(activeFile, sources[activeFile], issue);
  }));
}

function renderResult(report) {
  const scenario = scenarios[scenarioId];
  const issue = report.issues.find(candidate => candidate.evidence?.some(evidence => evidence.rule === scenario.rule)) ?? report.issues[0];
  if (!issue) throw new Error("Generated report contains no issue");
  const evidence = issue.evidence.find(candidate => candidate.rule === scenario.rule) ?? issue.evidence[0];
  byId("issue-kind").textContent = label(issue.kind);
  byId("issue-kind").className = issue.kind;
  byId("issue-analysis").textContent = label(issue.analysis);
  byId("scenario-rule").textContent = evidence.rule;
  byId("issue-title").textContent = issue.title;
  byId("issue-guidance").textContent = issue.guidance;
  byId("evidence-message").textContent = evidence.message;
  byId("evidence-locations").innerHTML = (evidence.locations ?? []).map(item => `<button data-evidence-path="${escapeHtml(item.path)}" data-evidence-line="${item.line ?? 0}">${escapeHtml(item.path)}${item.line ? `:${item.line}` : ""}${item.symbol ? `<span>${escapeHtml(item.symbol)}</span>` : ""}</button>`).join("");
  byId("measurements").innerHTML = (evidence.measurements ?? []).map(item => `<div><dt>${escapeHtml(label(item.name))}</dt><dd><strong>${item.value}</strong> ${escapeHtml(label(item.unit))}${item.threshold === undefined ? "" : `<small>${copy[locale].threshold} ${item.threshold}</small>`}</dd></div>`).join("");
  byId("result-loading").hidden = true;
  byId("result-error").hidden = true;
  byId("result-content").hidden = false;
  return issue;
}

function bindEvidenceLocations(sources, issue) {
  document.querySelectorAll("[data-evidence-path]").forEach(button => button.addEventListener("click", () => {
    const matched = scenarios[scenarioId].files.find(path => button.dataset.evidencePath === path || button.dataset.evidencePath.endsWith(`/${path}`));
    if (!matched) return;
    activeTab = "source";
    activeFile = matched;
    renderTabs();
    renderFileTabs(scenarios[scenarioId].files, issue, sources);
    renderSource(activeFile, sources[activeFile], issue);
    const line = Number(button.dataset.evidenceLine);
    const target = line ? document.querySelectorAll(".code-line")[line - 1] : byId("source-files");
    target?.scrollIntoView({ behavior: "smooth", block: "center" });
  }));
}

function renderTabs() {
  document.querySelectorAll("[data-tab]").forEach(button => button.setAttribute("aria-selected", String(button.dataset.tab === activeTab)));
  for (const tab of ["source", "config", "command"]) byId(`${tab}-panel`).hidden = tab !== activeTab;
}

async function renderDetail() {
  const version = ++renderVersion;
  const scenario = scenarios[scenarioId];
  const text = scenario[locale];
  byId("scenario-title").textContent = text.title;
  byId("scenario-language").textContent = scenario.language;
  byId("scenario-question").textContent = text.question;
  byId("report-link").href = `reports/${scenarioId}/?lang=${encodeURIComponent(locale)}`;
  byId("scenario-command").textContent = `reforge analyze playground/fixtures/${scenarioId} \\\n  --config playground/fixtures/${scenarioId}/reforge.toml \\\n  --output html --output-file reforge-report.html --reproducible`;
  byId("source-files").textContent = copy[locale].loading;
  byId("result-loading").hidden = false;
  byId("result-error").hidden = true;
  byId("result-content").hidden = true;
  try {
    const [sources, report] = await Promise.all([loadSources(scenarioId), loadReport(scenarioId)]);
    if (version !== renderVersion) return;
    byId("scenario-config").textContent = sources["reforge.toml"];
    const issue = renderResult(report);
    renderFileTabs(scenario.files, issue, sources);
    renderSource(activeFile, sources[activeFile], issue);
    bindEvidenceLocations(sources, issue);
  } catch (error) {
    if (version !== renderVersion) return;
    let sources;
    try { sources = await loadSources(scenarioId); } catch { /* retain the source loading state */ }
    if (sources) {
      byId("scenario-config").textContent = sources["reforge.toml"];
      renderFileTabs(scenario.files, undefined, sources);
      renderSource(activeFile, sources[activeFile]);
    }
    byId("result-loading").hidden = true;
    byId("result-error").hidden = false;
    byId("result-error").textContent = `${copy[locale].reportError} (${error instanceof Error ? error.message : String(error)})`;
  }
}

function render() {
  document.documentElement.lang = locale;
  document.title = locale === "zh-CN" ? "Reforge 在线体验" : "Reforge Playground";
  byId("locale").value = locale;
  document.querySelectorAll("[data-i18n]").forEach(node => { node.textContent = copy[locale][node.dataset.i18n]; });
  byId("scenario-cards").innerHTML = Object.entries(scenarios).map(([id, scenario], index) => `<button class="scenario-card" data-scenario="${id}" aria-pressed="${id === scenarioId}"><span class="scenario-number">0${index + 1}</span><span class="eyebrow">${scenario.language}</span><strong>${scenario[locale].title}</strong><span>${scenario[locale].card}</span><i aria-hidden="true">→</i></button>`).join("");
  document.querySelectorAll("[data-scenario]").forEach(button => button.addEventListener("click", () => {
    scenarioId = button.dataset.scenario;
    activeFile = scenarios[scenarioId].files[0];
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
