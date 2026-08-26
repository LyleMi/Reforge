const STORAGE_KEY = "reforge.locale";

const copy = {
  en: {
    language: "Language", docs: "Documentation", headline: "The agent finished. Does the repository agree?",
    intro: "Review a plausible patch against the architecture and abstractions already present in its repository. Every finding below comes from Reforge.",
    chooseLabel: "Review queue", scenarios: "Choose a failure mode", maintainerTask: "Maintainer task", agentChose: "Agent chose",
    repositoryExpects: "Repository already expects", reforgeObserved: "Reforge observed", reviewResult: "Review result",
    generatedEvidence: "Generated evidence", loadingReport: "Loading generated evidence…", evidenceShape: "Evidence shape", reportFacts: "Report facts",
    technicalDetails: "Technical details", openReport: "Open full report", agentPatch: "Agent patch", patch: "Patch",
    repositoryContext: "Repository context", fullSource: "Full source", configuration: "Config", reproduce: "Command",
    commandNote: "Run from the Reforge repository after building the CLI.", footer: "Repository-owned fixtures. Real schema 27 reports. No uploads.",
    reportError: "The generated report could not be loaded. Patch, repository context, configuration, and local reproduction remain available.",
    threshold: "threshold", coverage: "coverage", exactWitness: "Exact ordered witness", source: "Source", sink: "Sink", step: "Step",
    implementationGroup: "Implementations grouped by the report", dependencyLoop: "Reported cycle members", beforeDirection: "Before: one-way dependency", afterDirection: "After: closed loop",
  },
  "zh-CN": {
    language: "语言", docs: "文档", headline: "Agent 写完了。仓库同意吗？",
    intro: "把一份看似合理的 patch 放回仓库已有架构与抽象中审查。下方每个结论都来自 Reforge 的真实报告。",
    chooseLabel: "审查队列", scenarios: "选择失败模式", maintainerTask: "Maintainer task｜维护者任务", agentChose: "Agent chose｜Agent 的选择",
    repositoryExpects: "Repository already expects｜仓库原有约定", reforgeObserved: "Reforge observed｜Reforge 观察",
    reviewResult: "审查结论", generatedEvidence: "生成的真实证据", loadingReport: "正在载入生成的证据…", evidenceShape: "证据结构", reportFacts: "报告字段",
    technicalDetails: "技术细节", openReport: "打开完整报告", agentPatch: "Agent patch", patch: "Patch",
    repositoryContext: "Repository context", fullSource: "Full source", configuration: "Config", reproduce: "Command",
    commandNote: "构建 CLI 后，从 Reforge 仓库根目录运行。", footer: "仓库自有 fixture、真实 schema 27 报告、不上传代码。",
    reportError: "无法载入生成的报告，但 patch、仓库上下文、配置和本地复现命令仍然可用。",
    threshold: "阈值", coverage: "覆盖", exactWitness: "精确有序 witness", source: "源点", sink: "汇点", step: "步骤",
    implementationGroup: "报告归组的实现", dependencyLoop: "报告中的闭环成员", beforeDirection: "修改前：单向依赖", afterDirection: "修改后：形成闭环",
  },
};

const scenarios = {
  "typescript-boundary-bypass": {
    language: "TypeScript", analysis: "Dataflow", rule: "reforge.dataflow.adapter_flow_bypass", visualization: "witness",
    beforeFiles: ["src/application_checkout.ts", "src/payment_gateway.ts", "src/transport.ts"],
    afterFiles: ["src/application_checkout.ts", "src/application_refunds.ts", "src/payment_gateway.ts", "src/transport.ts"],
    contextFiles: ["src/application_checkout.ts", "src/payment_gateway.ts", "src/transport.ts"], entry: "src/application_refunds.ts",
    en: {
      title: "Bypasses an existing boundary", card: "A refund calls transport directly", change: "Add refund handling",
      task: "Add refund handling to the payment flow.", chose: "Called sendPayment directly from the new refund module.",
      expects: "Payment operations cross the payment_gateway adapter before reaching transport.",
      observed: "An exact value path reaches transport without crossing that adapter.",
      explanation: "The refund is locally straightforward, but it creates a second route to payment transport and bypasses the repository's existing gateway boundary.",
      context: "Repository context (not a report field): the existing charge path enters transport through payment_gateway.",
    },
    "zh-CN": {
      title: "绕过既有边界", card: "退款路径直接调用 transport", change: "增加退款处理",
      task: "为支付流程增加退款处理。", chose: "在新的退款模块中直接调用 sendPayment。",
      expects: "支付操作先经过 payment_gateway adapter，再到 transport。",
      observed: "一条精确值流路径未经过该 adapter，直接到达 transport。",
      explanation: "退款实现局部看起来很直接，却为支付 transport 创建了第二条入口，绕开了仓库已有的 gateway 边界。",
      context: "仓库上下文（并非报告字段）：已有扣款路径通过 payment_gateway 进入 transport。",
    },
  },
  "python-shadowed-abstraction": {
    language: "Python", analysis: "Codebase", rule: "reforge.codebase.shadowed_abstraction", visualization: "group",
    beforeFiles: ["providers/legacy_webhook_helper.py", "shared/event_normalizer.py"],
    afterFiles: ["providers/legacy_webhook_helper.py", "providers/orbit_webhook_helper.py", "shared/event_normalizer.py"],
    contextFiles: ["shared/event_normalizer.py", "providers/legacy_webhook_helper.py"], entry: "providers/orbit_webhook_helper.py",
    en: {
      title: "Duplicates repository capability", card: "A third event normalizer appears", change: "Add an Orbit webhook provider",
      task: "Add a webhook provider for Orbit.", chose: "Copied the neighboring provider's local normalization helper.",
      expects: "Webhook events use the shared event normalizer; one legacy provider still has a local copy.",
      observed: "The shared implementation and two local helpers form one shadowed-abstraction group.",
      explanation: "The agent followed nearby code, but repository-wide evidence shows that this creates a third implementation of a capability already owned by shared/event_normalizer.py.",
      context: "Repository context: a shared normalizer already exists alongside one legacy local implementation; two copies remain below the detector threshold.",
    },
    "zh-CN": {
      title: "复制仓库已有能力", card: "出现第三份 event normalizer", change: "增加 Orbit webhook provider",
      task: "增加 Orbit webhook provider。", chose: "照抄了邻近 provider 的局部 normalization helper。",
      expects: "Webhook event 使用共享 normalizer；只有一个遗留 provider 仍保留局部实现。",
      observed: "共享实现和两个局部 helper 被归入同一个 shadowed-abstraction 分组。",
      explanation: "Agent 跟随了邻近代码，但仓库级证据表明，它制造了第三份已有能力；该能力本应由 shared/event_normalizer.py 负责。",
      context: "仓库上下文：共享 normalizer 与一个遗留局部实现已经并存；两份实现尚未达到检测阈值。",
    },
  },
  "typescript-cycle": {
    language: "TypeScript", analysis: "Codebase", rule: "reforge.codebase.dependency_cycle", visualization: "cycle",
    beforeFiles: ["src/checkout.ts", "src/pricing.ts", "src/promotions.ts"], afterFiles: ["src/checkout.ts", "src/pricing.ts", "src/promotions.ts"],
    contextFiles: ["src/checkout.ts", "src/pricing.ts", "src/promotions.ts"], entry: "src/promotions.ts",
    relationships: [["src/checkout.ts", "src/pricing.ts"], ["src/pricing.ts", "src/promotions.ts"], ["src/promotions.ts", "src/checkout.ts"]],
    en: {
      title: "Fixes locally, closes a global loop", card: "A promotion imports back into checkout", change: "Add a first-order discount",
      task: "Add a first-order discount to promotions.", chose: "Imported checkout's order-history query helper into promotions.",
      expects: "Dependencies flow in one direction: checkout → pricing → promotions.",
      observed: "The reverse import closes a three-file cycle with three internal edges.",
      explanation: "Reusing the query helper avoids local duplication, but its ownership is on the wrong side of the dependency direction and closes a repository-level loop.",
      context: "Repository context: before the patch, checkout depends on pricing, which depends on promotions; promotions does not import back.",
    },
    "zh-CN": {
      title: "修复局部需求，制造全局闭环", card: "promotions 反向 import checkout", change: "增加首单优惠",
      task: "在 promotions 中增加首单优惠。", chose: "从 checkout 导入订单历史查询 helper。",
      expects: "依赖保持单向：checkout → pricing → promotions。",
      observed: "反向 import 形成包含三个文件、三条内部边的依赖闭环。",
      explanation: "复用查询 helper 避免了局部重复，但 helper 的归属位于错误的依赖方向上，最终闭合了仓库级循环。",
      context: "仓库上下文：修改前 checkout 依赖 pricing，pricing 依赖 promotions，而 promotions 不反向导入。",
    },
  },
};

const byId = id => document.getElementById(id);
const escapeHtml = value => String(value ?? "").replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");
const humanize = value => String(value ?? "").replaceAll("_", " ").replace(/\b\w/g, character => character.toUpperCase());
const reportCache = new Map();
const sourceCache = new Map();

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

async function fetchReport(id) {
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

async function loadSources(id) {
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

function splitLines(source) {
  if (source === null) return [];
  return source.replace(/\n$/, "").split("\n");
}

function lineDiff(before, after) {
  const left = splitLines(before);
  const right = splitLines(after);
  const table = Array.from({ length: left.length + 1 }, () => Array(right.length + 1).fill(0));
  for (let i = left.length - 1; i >= 0; i--) for (let j = right.length - 1; j >= 0; j--) table[i][j] = left[i] === right[j] ? table[i + 1][j + 1] + 1 : Math.max(table[i + 1][j], table[i][j + 1]);
  const rows = [];
  let i = 0; let j = 0;
  while (i < left.length || j < right.length) {
    if (i < left.length && j < right.length && left[i] === right[j]) rows.push({ kind: "context", text: left[i++], oldLine: i, newLine: ++j });
    else if (j < right.length && (i === left.length || table[i][j + 1] >= table[i + 1][j])) rows.push({ kind: "addition", text: right[j++], oldLine: null, newLine: j });
    else rows.push({ kind: "deletion", text: left[i++], oldLine: i, newLine: null });
  }
  return rows;
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
