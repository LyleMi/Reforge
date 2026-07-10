export type Severity = "info" | "warning" | "critical" | string;

export type Percentiles = {
  p50?: number;
  p75?: number;
  p90?: number;
  p95?: number;
  max?: number;
};

export type ChurnFileMetric = {
  commits_touched?: number;
  lines_added?: number;
  lines_deleted?: number;
  authors_count?: number;
  recent_weighted_churn?: number;
};

export type FileRawMetric = {
  path: string;
  loc?: number;
  imports?: number;
  public_items?: number;
  directory_source_files?: number;
  is_test?: boolean;
  churn?: ChurnFileMetric;
};

export type FindingMetric = {
  name: string;
  value?: number;
  threshold?: number | null;
  unit?: string;
  normalized?: number | null;
  percentile?: number | null;
};

export type RelatedLocation = {
  path: string;
  line?: number;
  name?: string | null;
};

export type Finding = {
  id?: string;
  kind: string;
  severity: Severity;
  path: string;
  line?: number | null;
  metrics?: FindingMetric[];
  construct?: string;
  mechanism?: string;
  issue_cluster_id?: string | null;
  priority?: number;
  confidence?: number;
  priority_factors?: Record<string, number>;
  rank_explanation?: string;
  message?: string;
  recommendation?: string;
  related_locations?: RelatedLocation[];
};

export type IssueCluster = {
  id: string;
  construct: string;
  mechanism: string;
  action: string;
  path: string;
  line?: number | null;
  primary_finding_id: string;
  finding_ids: string[];
  kinds: string[];
  priority: number;
  severity: Severity;
};

export type DetectorRelation = {
  kind: string;
  relation: "facet_of" | "alternative_evidence" | string;
};

export type DetectorManifestEntry = {
  kind: string;
  construct: string;
  mechanism: string;
  action: string;
  entity_scope: string;
  approach: string;
  supported_languages: string[];
  precision_risk: string;
  parent_kind?: string | null;
  relations: DetectorRelation[];
};

export type RawMetricManifestEntry = {
  name: string;
  entity_scope: string;
  unit: string;
  scale: "boolean" | "count" | string;
  direction: "higher_is_more_pressure" | "context_only" | string;
  description: string;
};

export type Hotspot = {
  level: string;
  path: string;
  line?: number | null;
  name?: string | null;
  priority?: number;
  severity?: Severity;
  static_risk?: number;
  churn_risk?: number;
  reason?: string;
};

export type DependencyGraphNode = {
  path: string;
  fan_in?: number;
  fan_out?: number;
};

export type DependencyGraphEdge = {
  from: string;
  to: string;
};

export type ChurnSummary = {
  mode?: string;
  enabled?: boolean;
  status?: string;
  reason?: string | null;
  window_days?: number;
  max_commit_lines?: number;
};

export type ScanSummary = {
  scanned_files?: number;
  finding_count?: number;
  issue_count?: number;
  hotspot_count?: number;
  similar_function_group_count?: number;
  duration_ms?: number;
  hotspot_model?: string;
  churn?: ChurnSummary;
};

export type ScanStats = {
  source_files_scanned?: number;
  directories_scanned?: number;
  function_candidates?: number;
};

export type SuppressionSummary = {
  suppressed_count?: number;
  suppressed_by_kind?: Record<string, number>;
  suppressed_by_severity?: Record<string, number>;
  highest_suppressed_priority?: number | null;
};

type MetricScope = "files" | "functions" | "types" | "churn";
type LocatedRawMetric = Record<string, unknown> & { path: string };

export type MetricsSummaryShape = Partial<Record<MetricScope, Record<string, Percentiles>>>;

export type RawMetrics = {
  files?: FileRawMetric[];
  functions?: LocatedRawMetric[];
  types?: LocatedRawMetric[];
};

export type DependencyGraph = {
  nodes?: DependencyGraphNode[];
  edges?: DependencyGraphEdge[];
};

export type ScanReport = {
  schema_version: number;
  summary?: ScanSummary;
  stats?: ScanStats;
  metrics_summary?: MetricsSummaryShape;
  raw_metrics?: RawMetrics;
  raw_metric_manifest?: RawMetricManifestEntry[];
  dependency_graph?: DependencyGraph;
  hotspots?: Hotspot[];
  suppression_summary?: SuppressionSummary;
  issue_clusters?: IssueCluster[];
  detector_manifest?: DetectorManifestEntry[];
  findings?: Finding[];
};
