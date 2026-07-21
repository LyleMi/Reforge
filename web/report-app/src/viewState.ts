import type { ScanReport } from "./reportTypes";

export type ViewName = "overview" | "evidence" | "map" | "metrics" | "coverage";
export type MapLayer = "findings" | "issues" | "churn" | "coverage";
export type EvidenceSort = "counts" | "path" | "churn";
export type ViewState = { view: ViewName; query: string; kind: string; sort: EvidenceSort; layer: MapLayer; file: string | null };
export const defaultViewState: ViewState = { view: "overview", query: "", kind: "", sort: "counts", layer: "findings", file: null };

export function parseViewState(hash: string, report?: ScanReport): ViewState {
  const raw = hash.replace(/^#/, "");
  const [viewRaw, queryRaw = ""] = raw.split("?");
  const views: ViewName[] = ["overview", "evidence", "map", "metrics", "coverage"];
  const view = views.includes(viewRaw as ViewName) ? viewRaw as ViewName : defaultViewState.view;
  const params = new URLSearchParams(queryRaw);
  const sorts: EvidenceSort[] = ["counts", "path", "churn"];
  const layers: MapLayer[] = ["findings", "issues", "churn", "coverage"];
  const sort = sorts.includes(params.get("sort") as EvidenceSort) ? params.get("sort") as EvidenceSort : defaultViewState.sort;
  const layer = layers.includes(params.get("layer") as MapLayer) ? params.get("layer") as MapLayer : defaultViewState.layer;
  const requestedFile = params.get("file");
  const known = new Set([
    ...(report?.raw_metrics?.files ?? []).map(item => item.path),
    ...(report?.findings ?? []).map(item => item.path),
    ...(report?.issues ?? []).map(item => item.path),
  ]);
  return { view, query: params.get("query") ?? "", kind: params.get("kind") ?? "", sort, layer, file: requestedFile && (!report || known.has(requestedFile)) ? requestedFile : null };
}

export function serializeViewState(state: ViewState): string {
  const params = new URLSearchParams();
  if (state.query) params.set("query", state.query);
  if (state.kind) params.set("kind", state.kind);
  if (state.sort !== defaultViewState.sort) params.set("sort", state.sort);
  if (state.layer !== defaultViewState.layer) params.set("layer", state.layer);
  if (state.file) params.set("file", state.file);
  const query = params.toString();
  return `#${state.view}${query ? `?${query}` : ""}`;
}
