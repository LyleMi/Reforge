const STORAGE_KEY = "reforge.locale";

const copy = {
  en: {
    language: "Language", headline: "Inspect the evidence before installing", scenarios: "Choose a scenario",
    intro: "Three deliberately constructed, repository-owned examples show one Reforge rule at a time. Analysis runs during the Pages build; no code is uploaded.",
    openReport: "Open interactive report", reviewQuestion: "Review question", reasonableException: "Reasonable exception",
    source: "Complete source", configuration: "Configuration", reproduce: "Reproduce locally",
    footer: "These examples are intentionally small and do not evaluate any third-party project.", loading: "Loading source…",
  },
  "zh-CN": {
    language: "语言", headline: "安装之前，先查看真实证据", scenarios: "选择一个场景",
    intro: "三个由 Reforge 仓库自有、特意构造的示例，每次只展示一条规则。分析在 Pages 构建时完成，不会上传任何代码。",
    openReport: "打开交互报告", reviewQuestion: "Review question / 审查问题", reasonableException: "合理例外",
    source: "完整源码", configuration: "配置", reproduce: "本地复现",
    footer: "这些示例刻意保持小巧，不扫描或评价任何第三方项目。", loading: "正在载入源码…",
  },
};

const scenarios = {
  "rust-similarity": {
    language: "Rust", rule: "reforge.codebase.similar_functions", files: ["src/providers.rs"],
    en: { title: "Provider adapters that drift together", card: "Three structurally similar adapters", summary: "Atlas, Boreal, and Cascade repeat the same request adaptation shape with provider-specific names.", question: "Would a shared request builder reduce coordinated edits without hiding provider-specific behavior?", exception: "Keep the adapters separate when providers are expected to diverge soon or when explicit boundary code is easier to audit." },
    "zh-CN": { title: "共同漂移的 Provider Adapter", card: "三个结构相似的适配器", summary: "Atlas、Boreal 与 Cascade 使用不同名称重复相同的请求转换结构。", question: "共享的请求构建器能否减少同步修改，同时保留各 provider 的特有行为？", exception: "如果各 provider 即将明显分化，或显式边界代码更便于审计，保持独立可能更合理。" },
  },
  "typescript-cycle": {
    language: "TypeScript", rule: "reforge.codebase.dependency_cycle", files: ["src/checkout.ts", "src/pricing.ts", "src/promotions.ts"],
    en: { title: "A checkout dependency loop", card: "A three-file dependency cycle", summary: "Checkout calls pricing, pricing calls promotions, and promotions reaches back into checkout.", question: "Which module should own the shared decision so the checkout flow has a one-way dependency direction?", exception: "A tightly bounded mutually recursive model can be valid when the cycle is deliberate, stable, and documented." },
    "zh-CN": { title: "Checkout 依赖环", card: "三文件依赖环", summary: "Checkout 调用 pricing，pricing 调用 promotions，而 promotions 又依赖 checkout。", question: "哪个模块应拥有共享决策，才能让结账流程形成单向依赖？", exception: "如果互递归模型边界清晰、长期稳定且有文档说明，保留有意设计的环也可能合理。" },
  },
  "python-long-function": {
    language: "Python", rule: "reforge.codebase.long_function", files: ["orders.py"],
    en: { title: "An order processor with four jobs", card: "Parsing, validation, pricing, persistence", summary: "process_order parses input, validates it, calculates totals, and prepares a persistence record.", question: "Which responsibility changes for a different reason and deserves a named boundary first?", exception: "A linear orchestration function can remain long when splitting it would obscure the sequence and its steps are already simple." },
    "zh-CN": { title: "承担四项职责的订单处理函数", card: "解析、校验、定价与持久化", summary: "process_order 同时解析输入、执行校验、计算总价，并准备持久化记录。", question: "哪项职责会因不同原因变化，最应该先形成具名边界？", exception: "如果函数只是清晰的线性编排，拆分反而会掩盖执行顺序，而且每一步都很简单，那么较长也可以接受。" },
  },
};

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

const byId = id => document.getElementById(id);
const escapeHtml = value => value.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");

function updateUrl() {
  const url = new URL(location.href);
  url.searchParams.set("scenario", scenarioId);
  url.searchParams.set("lang", locale);
  history.replaceState(null, "", url);
}

async function renderDetail() {
  const scenario = scenarios[scenarioId];
  const text = scenario[locale];
  byId("scenario-rule").textContent = scenario.rule;
  byId("scenario-title").textContent = text.title;
  byId("scenario-summary").textContent = text.summary;
  byId("scenario-question").textContent = text.question;
  byId("scenario-exception").textContent = text.exception;
  byId("report-link").href = `reports/${scenarioId}/?lang=${encodeURIComponent(locale)}`;
  byId("scenario-command").textContent = `reforge analyze playground/fixtures/${scenarioId} \\\n+  --config playground/fixtures/${scenarioId}/reforge.toml \\\n+  --output html --output-file reforge-report.html --reproducible`;
  byId("source-files").textContent = copy[locale].loading;
  const root = `fixtures/${scenarioId}/`;
  const [config, ...sources] = await Promise.all(["reforge.toml", ...scenario.files].map(path => fetch(root + path).then(response => {
    if (!response.ok) throw new Error(`${response.status} ${path}`);
    return response.text();
  })));
  byId("scenario-config").textContent = config;
  byId("source-files").innerHTML = scenario.files.map((path, index) => `<div class="source-file"><h4>${path}</h4><pre><code>${escapeHtml(sources[index])}</code></pre></div>`).join("");
}

function render() {
  document.documentElement.lang = locale;
  document.title = locale === "zh-CN" ? "Reforge 在线体验" : "Reforge Playground";
  byId("locale").value = locale;
  document.querySelectorAll("[data-i18n]").forEach(node => { node.textContent = copy[locale][node.dataset.i18n]; });
  byId("scenario-cards").innerHTML = Object.entries(scenarios).map(([id, scenario]) => `<button class="scenario-card" data-scenario="${id}" aria-pressed="${id === scenarioId}"><span class="eyebrow">${scenario.language}</span><strong>${scenario[locale].title}</strong><span>${scenario[locale].card}</span></button>`).join("");
  document.querySelectorAll("[data-scenario]").forEach(button => button.addEventListener("click", () => {
    scenarioId = button.dataset.scenario;
    updateUrl();
    render();
    byId("scenario-detail").scrollIntoView({ behavior: "smooth", block: "start" });
  }));
  void renderDetail();
}

byId("locale").addEventListener("change", event => {
  locale = event.target.value;
  try { localStorage.setItem(STORAGE_KEY, locale); } catch { /* visible switch still works */ }
  updateUrl();
  render();
});

render();
