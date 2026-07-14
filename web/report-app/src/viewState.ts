import type { ScanReport } from "./reportTypes";

export type TabId = "overview" | "issues" | "map" | "metrics" | "coverage";
export type IssueSort = "priority" | "severity" | "path" | "kind";
export type MetricScope = "files" | "functions" | "types" | "churn";
export type MapLayer = "priority" | "severity" | "churn" | "findings";

export type ViewState = {
  tab: TabId;
  query: string;
  severity: "" | "critical" | "warning" | "info";
  kind: string;
  sort: IssueSort;
  file: string | null;
  layer: MapLayer;
  scope: MetricScope;
};

export const defaultViewState: ViewState = {
  tab: "overview", query: "", severity: "", kind: "", sort: "priority",
  file: null, layer: "priority", scope: "files",
};

const tabs = new Set<TabId>(["overview", "issues", "map", "metrics", "coverage"]);
const severities = new Set(["critical", "warning", "info"]);
const sorts = new Set<IssueSort>(["priority", "severity", "path", "kind"]);
const layers = new Set<MapLayer>(["priority", "severity", "churn", "findings"]);
const scopes = new Set<MetricScope>(["files", "functions", "types", "churn"]);

export function parseViewState(hash: string, report?: ScanReport): ViewState {
  const raw = hash.replace(/^#/, "");
  const [page = "overview", query = ""] = raw.split("?", 2);
  const params = new URLSearchParams(query);
  const tab = tabs.has(page as TabId) ? page as TabId : "overview";
  const severity = params.get("severity") ?? "";
  const sort = params.get("sort") ?? "priority";
  const layer = params.get("layer") ?? "priority";
  const scope = params.get("scope") ?? "files";
  const requestedFile = params.get("file");
  const paths = report ? new Set([
    ...(report.raw_metrics?.files ?? []).map((item) => item.path),
    ...(report.findings ?? []).map((item) => item.path),
    ...(report.dependency_graph?.nodes ?? []).map((item) => item.path),
  ]) : null;
  return {
    tab,
    query: params.get("query") ?? "",
    severity: severities.has(severity) ? severity as ViewState["severity"] : "",
    kind: params.get("kind") ?? "",
    sort: sorts.has(sort as IssueSort) ? sort as IssueSort : "priority",
    file: requestedFile && (!paths || paths.has(requestedFile)) ? requestedFile : null,
    layer: layers.has(layer as MapLayer) ? layer as MapLayer : "priority",
    scope: scopes.has(scope as MetricScope) ? scope as MetricScope : "files",
  };
}

export function serializeViewState(state: ViewState): string {
  const params = new URLSearchParams();
  const append = (key: string, value: string | null, defaultValue = "") => {
    if (value && value !== defaultValue) params.set(key, value);
  };
  const serializers: Partial<Record<TabId, () => void>> = {
    issues: () => {
      append("query", state.query);
      append("severity", state.severity);
      append("kind", state.kind);
      append("sort", state.sort, "priority");
    },
    map: () => {
      append("file", state.file);
      append("layer", state.layer, "priority");
    },
    metrics: () => append("scope", state.scope, "files"),
  };
  serializers[state.tab]?.();
  const query = params.toString();
  return `#${state.tab}${query ? `?${query}` : ""}`;
}
