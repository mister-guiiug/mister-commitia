// Miroir TypeScript des modèles Rust (serde, champs snake_case).

export type AiAttributionPolicy = "keep-required" | "normalization-allowed";

export interface Governance {
  protected_trailers: string[];
  ai_attribution_policy: AiAttributionPolicy;
  signature_patterns: string[];
  convention_types: string[];
}

export interface RepoRef {
  id: string;
  name: string;
  local_path: string;
  remote_url: string | null;
  default_branch: string | null;
  protected_branches: string[];
  governance: Governance;
  added_at: string;
  last_scanned_at: string | null;
}

export interface BranchInfo {
  name: string;
  is_head: boolean;
  upstream: string | null;
  tip: string;
}

export interface CommitInfo {
  sha: string;
  short: string;
  parents: string[];
  author_name: string;
  author_email: string;
  date: string;
  subject: string;
  body: string;
  is_merge: boolean;
  signed: boolean;
  on_remote: boolean;
  files_changed: number;
  insertions: number;
  deletions: number;
  files: string[];
  trailers: [string, string][];
}

export type FlagKind =
  | "weak_message"
  | "non_conventional"
  | "ai_signature"
  | "oversized_no_body"
  | "duplicate_message";

export interface CommitFlag {
  sha: string;
  kind: FlagKind;
  score: number;
  detail: string;
}

export interface AnalysisReport {
  repo_id: string;
  branch: string;
  base: string | null;
  tip: string;
  total: number;
  conform: number;
  weak: number;
  ai_signatures: number;
  flags: CommitFlag[];
  generated_at: string;
}

export interface GraphParent {
  sha: string;
  lane: number;
  in_segment: boolean;
}

export interface GraphNode {
  sha: string;
  row: number;
  lane: number;
  is_merge: boolean;
  parents: GraphParent[];
}

export interface CommitGraph {
  nodes: GraphNode[];
  lanes: number;
}

export interface ScanResult {
  repo: RepoRef;
  branch: string;
  base: string | null;
  commits: CommitInfo[];
  report: AnalysisReport;
  squash_suggestions: string[][];
  graph: CommitGraph;
}

export type Risk = "low" | "medium" | "high";

export type Operation =
  | { op: "reword"; target: string; new_message: string }
  | { op: "squash"; targets: string[]; new_message: string }
  | { op: "fixup"; targets: string[] }
  | { op: "drop"; target: string; reason: string }
  | { op: "reorder"; order: string[] };

export type PlanOp = Operation & {
  seq: number;
  origin: string;
  risk: Risk;
  approved_by: string | null;
  approved_at: string | null;
};

export type PlanStatus = "draft" | "dry_run_ok" | "applied" | "rolled_back" | "invalidated";

export interface ShaMapping {
  old: string[];
  new: string;
}

export interface Plan {
  id: string;
  version: number;
  repo_id: string;
  fingerprint: { branch: string; tip: string; base: string };
  status: PlanStatus;
  ops: PlanOp[];
  dry_run_hash: string | null;
  preview_ref: string | null;
  backup_ref: string | null;
  backup_tag: string | null;
  mapping: ShaMapping[];
  created_at: string;
  dry_run_at: string | null;
  applied_at: string | null;
  error: string | null;
}

export type ProposalStatus = "proposed" | "accepted" | "edited" | "rejected" | "refused";

export interface Proposal {
  id: string;
  repo_id: string;
  skill: string;
  skill_version: string;
  targets: string[];
  before: string;
  after: string | null;
  explanation: string;
  risk: Risk;
  status: ProposalStatus;
  decision: string | null;
  created_at: string;
}

export interface RiskAxis {
  axe: string;
  verdict: "ok" | "attention" | "bloquant";
  motif: string;
}

export type CiKind = "github" | "github_enterprise" | "azure_devops" | "azure_devops_server";

export interface CiAccount {
  id: string;
  kind: CiKind;
  base_url: string;
  org: string | null;
  project: string | null;
  repo: string | null;
  token_ref: string;
  scopes: string[];
  added_at: string;
}

export interface CiRun {
  account_id: string;
  pipeline_id: string;
  pipeline_name: string;
  run_id: string;
  status: string;
  result: string | null;
  branch: string | null;
  created_at: string;
  url: string | null;
  leased: boolean;
  running: boolean;
}

export interface RetentionRules {
  max_age_days: number | null;
  keep_last_per_pipeline: number;
  protect_branches: string[];
  protect_failed: boolean;
}

export interface RetentionPolicy {
  id: string;
  name: string;
  rules: RetentionRules;
  enabled: boolean;
}

export interface SimulationReport {
  id: string;
  policy_id: string;
  account_id: string;
  generated_at: string;
  total: number;
  candidates: CiRun[];
  protected: { run: CiRun; reason: string }[];
  kept_recent: number;
  scope_hash: string;
}

export interface BatchFailure {
  run_id: string;
  reason: string;
}

export interface BatchDeleteResult {
  total: number;
  deleted: string[];
  failed: BatchFailure[];
  cancelled: boolean;
}

export type AiProviderKind = "rule_based" | "ollama" | "open_ai_compat" | "anthropic";

export interface AiProviderConfig {
  id: string;
  kind: AiProviderKind;
  base_url: string | null;
  model: string | null;
  key_ref: string | null;
  is_default: boolean;
}

export interface AuditEvent {
  seq: number;
  ts: string;
  actor: string;
  category: string;
  action: string;
  target: string;
  params: unknown;
  result: string;
}

export interface SkillMeta {
  name: string;
  version: string;
  owner: string;
  status: string;
  description: string;
  output: string;
  guardrails: string[];
  rules: string[];
  tests: number;
  local_capable: boolean;
}

export interface SkillTestResult {
  case: string;
  passed: boolean;
  detail: string;
}

export interface PrRef {
  number: number;
  title: string;
  url: string;
}

export interface PushPreview {
  remote: string | null;
  remote_url: string | null;
  branch: string;
  local_tip: string;
  remote_tip: string | null;
  ahead: number;
  behind: number;
  needs_force: boolean;
  protected: boolean;
  can_push: boolean;
  open_prs: PrRef[] | null;
  warnings: string[];
}

export interface PushResult {
  branch: string;
  forced: boolean;
  remote_tip: string;
  detail: string;
}
