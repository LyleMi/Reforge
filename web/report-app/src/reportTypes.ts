export type CoverageStatus = "observed" | "partial" | "unsupported" | "not_applicable";
export type Producer = { name: string; version: string; revision?: string };
export type Target = { root: string; workspace_identity: string; source_revision?: string };
export type EntityRef = { key: string; path: string; symbol?: string };
export type Subject =
  | { kind: "repository" }
  | { kind: "directory" | "file" | "symbol"; entity: EntityRef }
  | { kind: "group"; members: EntityRef[] };
export type Location = { path: string; line?: number; symbol?: string };
export type Measurement = { name: string; value: number; threshold?: number; unit: string };
export type FlowResolution = "exact" | "modeled" | "unresolved" | "unsupported";
export type FlowEndpoint = { path: string; symbol: string; language: string; line?: number };
export type FlowStep = { path: string; symbol: string; line?: number; operation: string; resolution: FlowResolution };
export type FlowWitness = {
  source: FlowEndpoint;
  sink: FlowEndpoint;
  ordered_steps: FlowStep[];
  function_hops: number;
  module_hops: number;
  resolution: FlowResolution;
};
export type Evidence = {
  id: string;
  rule: string;
  message: string;
  measurements?: Measurement[];
  locations?: Location[];
  witness?: FlowWitness;
};
export type Issue = {
  id: string;
  kind: "advisory" | "policy";
  content_fingerprint: string;
  analysis: string;
  family: string;
  subject: Subject;
  title: string;
  guidance: string;
  evidence: Evidence[];
};
export type AnalysisCoverage = {
  status: CoverageStatus;
  scanned_files: number;
  languages?: Record<string, {
    status: CoverageStatus;
    files: number;
    functions: number;
    capabilities?: Record<string, CapabilityReceipt>;
    limitations?: CoverageLimitation[];
  }>;
  rules?: Record<string, {
    status: CoverageStatus;
    maturity: "experimental" | "preview" | "stable";
    enabled_source: "default" | "enable" | "enforce" | "disabled" | "internal";
    observations?: CoverageObservation[];
    limitations?: CoverageLimitation[];
  }>;
  limitations?: CoverageLimitation[];
};
export type CapabilityReceipt = {
  status: CoverageStatus;
  limitations?: CoverageLimitation[];
};
export type CoverageObservation = { name: string; count: number; unit: string };
export type CoverageLimitation = { code: string; count: number; message: string };
export type Report = {
  schema_version: 27;
  producer: Producer;
  target: Target;
  provenance: {
    identity_scheme: string;
    scope_digest: string;
    analyses: Record<string, { config_digest: string; policy_digest: string }>;
    rules: Record<string, { semantic_version: string; evaluation_digest: string }>;
  };
  summary: { issue_count: number; evidence_count: number; scanned_files: number };
  suppression: { evidence_count: number; by_rule: Record<string, number> };
  coverage: Record<string, AnalysisCoverage>;
  issues: Issue[];
  baseline_comparison?: {
    issues: Record<string, {
      state: "new" | "unchanged" | "updated" | "absent" | "unknown";
      reason?: string;
    }>;
  };
};
