export type Locale = "en" | "zh-CN";

const messages = {
  en: {
    reportEyebrow: "Evidence workbench", title: "Reforge Evidence Workbench", issue: "issue", issues: "issues",
    evidence: "evidence", item: "item", items: "items", files: "files", issuesCard: "Issues",
    evidenceCard: "Evidence", suppressed: "Suppressed", baseline: "Baseline", list: "Issues and evidence",
    filter: "Filter issues", search: "Search issues and evidence", analysis: "Analysis", allAnalyses: "All analyses",
    kind: "Kind", allKinds: "Advisory and policy", policy: "Policy", advisory: "Advisory", maturity: "Maturity",
    allMaturities: "All maturities", baselineState: "Baseline state", allBaseline: "All baseline states",
    noIssues: "No issues reported.", coverage: "Coverage", scannedFiles: "scanned files", threshold: "threshold",
    functionHops: "function hops", moduleHops: "module hops", reportUnavailable: "Report unavailable",
    absence: "absence is meaningful only for observed analyses.", language: "Report language", limitations: "Limitations",
    errorLead: "The embedded report could not be loaded.", details: "Technical details", workspace: "Workspace",
    results: "results", conclusion: "Finding", guidance: "Guidance", subject: "Subject", locations: "Locations",
    measurements: "Measurements", witness: "Flow witness", selectIssue: "Select an issue to inspect its evidence.",
    noMatchingIssues: "No issues match these filters.", noReportedIssues: "No issues were reported.",
    clearFilters: "Clear filters", baselineUnavailable: "No baseline comparison", coveragePartial: "Coverage is partial",
    previousIssue: "Previous issue", nextIssue: "Next issue", issueList: "Issue list", technical: "Technical details",
    issueId: "Issue ID", evidenceId: "Evidence ID", fingerprint: "Fingerprint", summary: "Scan summary",
  },
  "zh-CN": {
    reportEyebrow: "证据工作台", title: "Reforge Evidence Workbench", issue: "个问题", issues: "个问题",
    evidence: "条证据", item: "项", items: "项", files: "个文件", issuesCard: "问题",
    evidenceCard: "证据", suppressed: "已抑制", baseline: "基线", list: "问题与证据",
    filter: "筛选问题", search: "搜索问题与证据", analysis: "分析类型", allAnalyses: "全部分析",
    kind: "类型", allKinds: "建议与策略", policy: "策略", advisory: "建议", maturity: "成熟度",
    allMaturities: "全部成熟度", baselineState: "基线状态", allBaseline: "全部基线状态",
    noIssues: "没有符合条件的问题。", coverage: "覆盖情况", scannedFiles: "个已扫描文件", threshold: "阈值",
    functionHops: "次函数跳转", moduleHops: "次模块跳转", reportUnavailable: "报告不可用",
    absence: "仅当分析状态为“已观测”时，没有发现问题才具有明确含义。", language: "报告语言", limitations: "分析限制",
    errorLead: "无法载入嵌入的报告。", details: "技术详情", workspace: "工作区",
    results: "条结果", conclusion: "问题结论", guidance: "改进建议", subject: "分析对象", locations: "位置",
    measurements: "测量值", witness: "数据流见证", selectIssue: "选择一个问题以查看证据。",
    noMatchingIssues: "没有问题符合当前筛选条件。", noReportedIssues: "本次分析未报告问题。",
    clearFilters: "清除筛选", baselineUnavailable: "无基线对比", coveragePartial: "覆盖范围不完整",
    previousIssue: "上一个问题", nextIssue: "下一个问题", issueList: "问题列表", technical: "技术详情",
    issueId: "问题 ID", evidenceId: "证据 ID", fingerprint: "指纹", summary: "扫描摘要",
  },
} as const;

export type MessageKey = keyof typeof messages.en;

const enumTranslations: Record<Locale, Record<string, string>> = {
  en: {},
  "zh-CN": {
    codebase: "代码库", dataflow: "数据流", observed: "已观测", partial: "部分覆盖",
    partially_observed: "部分观测", unsupported: "不支持", not_applicable: "不适用", exact: "精确",
    modeled: "建模", unresolved: "未解析", stable: "稳定", preview: "预览", experimental: "实验性",
    enable: "显式启用", enforce: "强制执行", default: "默认", disabled: "已禁用", internal: "内部",
    new: "新增", updated: "已更新", unknown: "未知", unchanged: "未变化", absent: "已消失",
    policy: "策略", advisory: "建议", functions: "函数", files: "文件",
  },
};

const formatLabel = (value: string) => value.replace(/[._]/g, " ").replace(/\b\w/g, character => character.toUpperCase());

export const t = (locale: Locale, key: MessageKey) => messages[locale][key];

export const translatedLabel = (locale: Locale, value: string) => enumTranslations[locale][value] ?? formatLabel(value);

export type LocaleInputs = {
  search: string;
  stored?: string | null;
  browserLanguages?: readonly string[];
};

export function normalizeLocale(value?: string | null): Locale | undefined {
  if (!value) return undefined;
  const normalized = value.trim().toLowerCase();
  if (normalized === "zh" || normalized === "zh-cn") return "zh-CN";
  if (normalized === "en" || normalized.startsWith("en-")) return "en";
  return undefined;
}

export function resolveLocale({ search, stored, browserLanguages = [] }: LocaleInputs): Locale {
  const query = normalizeLocale(new URLSearchParams(search).get("lang"));
  if (query) return query;
  const persisted = normalizeLocale(stored);
  if (persisted) return persisted;
  for (const language of browserLanguages) {
    const browser = normalizeLocale(language);
    if (browser) return browser;
  }
  return "en";
}

export function initialLocale(): Locale {
  let stored: string | null = null;
  try {
    stored = window.localStorage.getItem("reforge.locale");
  } catch {
    // Reports opened from restricted file URLs can deny storage access.
  }
  return resolveLocale({
    search: window.location.search,
    stored,
    browserLanguages: navigator.languages?.length ? navigator.languages : [navigator.language],
  });
}

export function persistLocale(locale: Locale): void {
  document.documentElement.lang = locale;
  const url = new URL(window.location.href);
  url.searchParams.set("lang", locale);
  window.history.replaceState(null, "", url);
  try {
    window.localStorage.setItem("reforge.locale", locale);
  } catch {
    // The visible locale still changes when storage is unavailable.
  }
}
