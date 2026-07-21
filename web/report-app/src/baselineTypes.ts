export type ReportProvenance = { engine: { version: string; build_revision?: string | null }; source: { git_revision?: string | null; dirty?: boolean | null }; configuration: { effective: Record<string, unknown>; hash: string }; detector_policy_hash: string };
export type LineageCandidate = { id: string; entity: string; previous_id: string; current_id: string; confidence_percent: number; reasons: string[] };
export type DifferenceSet = { added: unknown[]; removed: unknown[]; changed: unknown[]; unchanged_count: number };
export type BaselineComparison = { baseline_path?: string | null; baseline_provenance: ReportProvenance; provenance_changed: boolean; provenance_change_dimensions: string[]; findings: DifferenceSet; issues: DifferenceSet; lineage_candidates: LineageCandidate[] };
