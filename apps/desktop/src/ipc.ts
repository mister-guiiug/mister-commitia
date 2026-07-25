// Couche IPC : invoke Tauri en desktop, mock de démonstration en navigateur
// (permet de développer/vérifier l'UI sans fenêtre native). Le mock est
// clairement signalé dans l'interface via `isMock`.

import type {
  AuditEvent, BatchDeleteResult, CiAccount, CiRun, CommitGraph, Plan, PlanOp, Proposal, PurgeResult,
  PushPreview, PushResult, RepoRef, RetentionPolicy, RiskAxis, ScanResult, SimulationReport, SkillMeta,
} from "./types";

export const isTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
export const isMock = !isTauri;

/// Contrat d'erreur du cœur (CmdError côté Rust) : `code` est stable,
/// `message` est le libellé humain, `expected` porte la valeur attendue
/// des confirmations renforcées. L'UI se branche sur `code`, jamais sur
/// le texte du message.
export interface IpcError {
  code: string;
  message: string;
  expected?: string | null;
}

export function asIpcError(e: unknown): IpcError {
  if (e && typeof e === "object" && "code" in e && "message" in e) {
    return e as IpcError;
  }
  return { code: "unknown", message: String(e) };
}

export async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<T>(cmd, args);
  }
  return mockCall(cmd, args) as Promise<T>;
}

export async function pickDirectory(): Promise<string | null> {
  if (isTauri) {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const res = await open({ directory: true, multiple: false });
    return typeof res === "string" ? res : null;
  }
  return window.prompt("Chemin du dépôt Git local :", "C:\\Src\\demo-repo");
}

// ---------------------------------------------------------------------------
// Événements des opérations longues (T2/T11) — canal unique `mc://task`.
// ---------------------------------------------------------------------------

export interface TaskProgressEvent {
  task_id: string; task: string; kind: "progress";
  phase: string; current: number; total: number | null;
}
export interface TaskAiDeltaEvent {
  task_id: string; task: string; kind: "ai_delta";
  group: number; delta: string;
}
export type TaskEvent = TaskProgressEvent | TaskAiDeltaEvent;

const mockBus = new EventTarget();
const mockCancelled = new Set<string>();

export function newTaskId(): string {
  return typeof crypto.randomUUID === "function"
    ? crypto.randomUUID()
    : Math.random().toString(36).slice(2);
}

/// S'abonne aux événements de tâches (progression + fragments IA).
/// Retourne la fonction de désabonnement.
export async function onTaskEvent(cb: (e: TaskEvent) => void): Promise<() => void> {
  if (isTauri) {
    const { listen } = await import("@tauri-apps/api/event");
    return listen<TaskEvent>("mc://task", (e) => cb(e.payload));
  }
  const h = (e: Event) => cb((e as CustomEvent<TaskEvent>).detail);
  mockBus.addEventListener("mc-task", h);
  return () => mockBus.removeEventListener("mc-task", h);
}

/// Demande d'annulation coopérative — le cœur s'arrête au prochain point sûr.
export function cancelTask(taskId: string): void {
  void call("task_cancel", { taskId }).catch(() => {});
}

// ---------------------------------------------------------------------------
// Mock navigateur — données de démonstration cohérentes avec les modèles.
// ---------------------------------------------------------------------------

const gov = {
  protected_trailers: ["Signed-off-by"],
  ai_attribution_policy: "keep-required" as const,
  signature_patterns: ["Generated with Claude Code", "Co-Authored-By: Claude <noreply@anthropic.com>"],
  convention_types: ["feat", "fix", "refactor", "chore", "docs", "test", "ci", "perf", "build", "style", "revert"],
  resign_after_rewrite: false,
};

const demoRepo: RepoRef = {
  id: "repo_demo",
  name: "webapp-checkout (démo)",
  local_path: "C:\\Src\\webapp-checkout",
  remote_url: "git@ghe.example.com:shop/webapp-checkout.git",
  default_branch: "main",
  protected_branches: ["main"],
  governance: gov,
  added_at: "2026-07-24T09:00:00Z",
  last_scanned_at: null,
};

const demoCommits: import("./types").CommitInfo[] = [
  {
    sha: "a1".padEnd(40, "0"), short: "a1000000", parents: [], author_name: "Guillaume",
    author_email: "dev@example.org", date: "2026-07-22T09:00:00Z",
    subject: "feat(pay): add express payment flow", body: "", is_merge: false, signed: false,
    on_remote: true, files_changed: 3, insertions: 120, deletions: 4, files: ["src/pay.rs"], trailers: [],
  },
  {
    sha: "b2".padEnd(40, "0"), short: "b2000000", parents: [], author_name: "Guillaume",
    author_email: "dev@example.org", date: "2026-07-22T10:12:00Z",
    subject: "wip", body: "", is_merge: false, signed: false,
    on_remote: false, files_changed: 1, insertions: 22, deletions: 9, files: ["src/pay.rs"], trailers: [],
  },
  {
    sha: "c3".padEnd(40, "0"), short: "c3000000", parents: [], author_name: "Guillaume",
    author_email: "dev@example.org", date: "2026-07-22T11:40:00Z",
    subject: "fix stuff",
    body: "🤖 Generated with Claude Code\nCo-Authored-By: Claude <noreply@anthropic.com>",
    is_merge: false, signed: false, on_remote: false, files_changed: 1, insertions: 8,
    deletions: 2, files: ["src/pay.rs"], trailers: [],
  },
  {
    sha: "d4".padEnd(40, "0"), short: "d4000000", parents: [], author_name: "Jane",
    author_email: "jane@example.org", date: "2026-07-23T08:05:00Z",
    subject: "update JIRA-123", body: "Signed-off-by: Jane Doe <jane@example.org>",
    is_merge: false, signed: true, on_remote: false, files_changed: 1, insertions: 40,
    deletions: 0, files: ["docs/pay.md"], trailers: [["Signed-off-by", "Jane Doe <jane@example.org>"]],
  },
];

const mock = {
  repos: [demoRepo] as RepoRef[],
  proposals: [] as Proposal[],
  plans: [] as Plan[],
  accounts: [
    {
      id: "acct_demo", kind: "github" as const, base_url: "https://api.github.com",
      org: "mister-guiiug", project: null, repo: "mister-commitia",
      token_ref: "ci:acct_demo", scopes: ["Actions: read/write"], added_at: "2026-07-24T09:00:00Z",
    },
  ] as CiAccount[],
  policies: [] as RetentionPolicy[],
  providers: [] as { id: string; kind: string; base_url: string | null; model: string | null; key_ref: string | null; is_default: boolean }[],
  audit: [
    { seq: 1, ts: "2026-07-24T09:00:00Z", actor: "demo", category: "config", action: "repo_declare", target: "webapp-checkout", params: {}, result: "ok" },
  ] as AuditEvent[],
  simulated: false,
  // DEMO C1 : plans dont le conflit de rejeu a déjà été résolu (ne re-conflicte plus).
  resolvedConflicts: new Set<string>(),
  skillsYaml: {
    "conventional-commits": "apiVersion: mister-commitia/skill.v1\nname: conventional-commits\nversion: 1.2.0\nowner: platform-team@example.org\nstatus: published\ndescription: >\n  Reformule un message selon Conventional Commits 1.0.0.\n",
    "commit-synthesis": "apiVersion: mister-commitia/skill.v1\nname: commit-synthesis\nversion: 1.0.0\nowner: platform-team@example.org\nstatus: published\ndescription: Synthèse de groupe.\n",
    "ai-signature-cleaner": "apiVersion: mister-commitia/skill.v1\nname: ai-signature-cleaner\nversion: 1.0.0\nowner: platform-team@example.org\nstatus: published\ndescription: Normalisation gouvernée.\n",
    "squash-advisor": "apiVersion: mister-commitia/skill.v1\nname: squash-advisor\nversion: 1.0.0\nowner: platform-team@example.org\nstatus: published\ndescription: Groupes fusionnables.\n",
    "ci-cleanup-policy": "apiVersion: mister-commitia/skill.v1\nname: ci-cleanup-policy\nversion: 1.0.0\nowner: platform-team@example.org\nstatus: published\ndescription: Politique de rétention.\n",
    "risk-reviewer": "apiVersion: mister-commitia/skill.v1\nname: risk-reviewer\nversion: 1.0.0\nowner: platform-team@example.org\nstatus: published\ndescription: Risques d'un plan.\n",
  } as Record<string, string>,
};

let seq = 1;
const id = (p: string) => `${p}_${Math.random().toString(36).slice(2, 10)}`;
const now = () => new Date().toISOString();
const audit = (category: string, action: string, target: string, result = "ok") => {
  mock.audit.unshift({ seq: ++seq, ts: now(), actor: "demo", category, action, target, params: {}, result });
};

// --- Simulation des tâches longues (progression, streaming, annulation) ----

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const cancelledError = (): IpcError => ({
  code: "cancelled",
  message: "opération annulée par l'utilisateur",
});
const emitTask = (e: TaskEvent) =>
  mockBus.dispatchEvent(new CustomEvent("mc-task", { detail: e }));

/// Rejoue des phases [libellé, current, total, durée ms] comme le ferait le
/// cœur : émission AVANT la phase, annulation vérifiée à chaque point sûr.
async function playPhases(
  taskId: unknown,
  task: string,
  phases: [string, number, number | null, number][],
): Promise<void> {
  const tid = typeof taskId === "string" && taskId ? taskId : null;
  for (const [phase, current, total, ms] of phases) {
    if (tid) {
      if (mockCancelled.has(tid)) throw cancelledError();
      emitTask({ task_id: tid, task, kind: "progress", phase, current, total });
    }
    await sleep(ms);
    if (tid && mockCancelled.has(tid)) throw cancelledError();
  }
}

/// Streaming IA simulé : le texte arrive mot à mot (kind ai_delta).
async function streamAiText(taskId: unknown, group: number, text: string): Promise<void> {
  const tid = typeof taskId === "string" && taskId ? taskId : null;
  if (!tid) return;
  for (const word of text.match(/\S+\s*/g) ?? []) {
    if (mockCancelled.has(tid)) throw cancelledError();
    emitTask({ task_id: tid, task: "proposals_generate", kind: "ai_delta", group, delta: word });
    await sleep(45);
  }
}

const demoRuns: CiRun[] = Array.from({ length: 14 }, (_, i) => ({
  account_id: "acct_demo",
  pipeline_id: i % 2 === 0 ? "9" : "12",
  pipeline_name: i % 2 === 0 ? "CI" : "Deploy",
  run_id: String(1000 + i),
  status: i === 0 ? "in_progress" : "completed",
  result: i === 0 ? null : i % 5 === 3 ? "failure" : "success",
  branch: i % 3 === 0 ? "main" : "develop",
  created_at: new Date(Date.now() - i * 20 * 86400_000).toISOString(),
  url: null,
  leased: i === 5,
  running: i === 0,
}));

async function mockCall(cmd: string, args: Record<string, unknown> = {}): Promise<unknown> {
  await new Promise((r) => setTimeout(r, 120));
  switch (cmd) {
    case "repos_list": return mock.repos;
    case "repo_declare": {
      const r: RepoRef = { ...demoRepo, id: id("repo"), name: String(args.path).split(/[\\/]/).pop() ?? "repo", local_path: String(args.path) };
      mock.repos.push(r); audit("config", "repo_declare", r.name); return r;
    }
    case "repo_remove": mock.repos = mock.repos.filter((r) => r.id !== args.id); return null;
    case "repo_branches": return [
      { name: "main", is_head: false, upstream: "origin/main", tip: "e5".padEnd(40, "0") },
      { name: "feature/express-payment", is_head: true, upstream: null, tip: "d4".padEnd(40, "0") },
    ];
    case "repo_set_governance": {
      const r = mock.repos.find((x) => x.id === args.id)!;
      r.governance = args.governance as RepoRef["governance"];
      r.protected_branches = args.protectedBranches as string[];
      audit("config", "governance_update", r.name);
      return r;
    }
    case "task_cancel":
      mockCancelled.add(String(args.taskId));
      return null;
    case "repo_scan": {
      await playPhases(args.taskId, "repo_scan", [
        ["ouverture du dépôt", 0, null, 140],
        ["lecture des commits", 1, 4, 160],
        ["lecture des commits", 2, 4, 160],
        ["lecture des commits", 3, 4, 160],
        ["lecture des commits", 4, 4, 160],
        ["analyse des messages", 0, null, 140],
      ]);
      const repo = mock.repos.find((x) => x.id === args.id)!;
      // Graphe linéaire de démo (du plus récent au plus ancien), la base est
      // hors segment. Le cœur calcule les lanes ; ici on les fournit à la main.
      const graph: CommitGraph = {
        lanes: 1,
        nodes: [3, 2, 1, 0].map((ci, row) => ({
          sha: demoCommits[ci].sha, row, lane: 0, is_merge: false,
          parents: ci > 0
            ? [{ sha: demoCommits[ci - 1].sha, lane: 0, in_segment: true }]
            : [{ sha: "00".padEnd(40, "9"), lane: 0, in_segment: false }],
        })),
      };
      const res: ScanResult = {
        repo, branch: "feature/express-payment", base: "00".padEnd(40, "9"),
        commits: demoCommits, graph,
        report: {
          repo_id: repo.id, branch: "feature/express-payment", base: null,
          tip: demoCommits[3].sha, total: 4, conform: 1, weak: 2, ai_signatures: 1,
          flags: [
            { sha: demoCommits[1].sha, kind: "weak_message", score: 90, detail: "vocabulaire vide (wip)" },
            { sha: demoCommits[1].sha, kind: "non_conventional", score: 50, detail: "sujet non conforme" },
            { sha: demoCommits[2].sha, kind: "weak_message", score: 70, detail: "vocabulaire vide" },
            { sha: demoCommits[2].sha, kind: "ai_signature", score: 60, detail: "motifs : Generated with Claude Code" },
            { sha: demoCommits[3].sha, kind: "non_conventional", score: 50, detail: "sujet non conforme" },
          ],
          generated_at: now(),
        },
        squash_suggestions: [[demoCommits[1].sha, demoCommits[2].sha]],
      };
      return res;
    }
    case "skills_list": return [[
      { name: "conventional-commits", version: "1.2.0", owner: "platform-team@example.org", status: "published", description: "Reformule un message selon Conventional Commits 1.0.0.", output: "message-proposal", guardrails: ["preserves_references", "preserves_protected_trailers", "subject_matches", "no_auto_apply"], rules: ["Type déduit du diff", "Sujet ≤ 72 caractères", "Références conservées"], tests: 4, local_capable: true },
      { name: "commit-synthesis", version: "1.0.0", owner: "platform-team@example.org", status: "published", description: "Synthétise plusieurs commits en une intention.", output: "message-proposal", guardrails: ["preserves_references", "preserves_breaking_changes", "no_auto_apply"], rules: ["Intention fonctionnelle", "Inventaire conservation"], tests: 2, local_capable: true },
      { name: "ai-signature-cleaner", version: "1.0.0", owner: "platform-team@example.org", status: "published", description: "Normalisation gouvernée des mentions d'outils.", output: "message-proposal", guardrails: ["must_refuse_when", "preserves_protected_trailers", "only_removes_matched_patterns"], rules: ["Gouvernance d'abord", "Périmètre strict"], tests: 3, local_capable: true },
      { name: "squash-advisor", version: "1.0.0", owner: "platform-team@example.org", status: "published", description: "Groupes de commits fusionnables.", output: "group-proposal", guardrails: ["groups_within_segment"], rules: ["Signaux de groupe"], tests: 1, local_capable: false },
      { name: "ci-cleanup-policy", version: "1.0.0", owner: "platform-team@example.org", status: "published", description: "Propose une politique de rétention.", output: "report", guardrails: ["report_only"], rules: ["Protéger par défaut"], tests: 1, local_capable: false },
      { name: "risk-reviewer", version: "1.0.0", owner: "platform-team@example.org", status: "published", description: "Évalue les risques d'un plan.", output: "report", guardrails: ["report_only", "can_block"], rules: ["Axes d'analyse"], tests: 2, local_capable: false },
    ] as SkillMeta[], []];
    case "commit_diff": {
      const c = demoCommits.find((x) => x.sha === args.sha);
      return `diff --git a/src/pay.rs b/src/pay.rs\nindex 0000001..0000002 100644\n--- a/src/pay.rs\n+++ b/src/pay.rs\n@@ -1,3 +1,6 @@\n pub fn pay() {\n-    // ancien comportement\n+    // ${c?.subject ?? "changement"}\n+    validate_cart();\n+    charge_customer();\n }\n`;
    }
    case "skill_read":
      return mock.skillsYaml[String(args.name)] ?? (() => { throw { code: "not_found", message: `introuvable : skill ${args.name}` } satisfies IpcError; })();
    case "skill_write": {
      const name = String(args.name);
      const content = String(args.content);
      if (!mock.skillsYaml[name]) throw { code: "not_found", message: `introuvable : skill ${name}` } satisfies IpcError;
      if (content.includes("::")) throw { code: "invalid", message: "invalide : YAML invalide" } satisfies IpcError;
      if (!content.includes(`name: ${name}`)) throw { code: "invalid", message: `invalide : le champ name doit rester « ${name} »` } satisfies IpcError;
      mock.skillsYaml[name] = content;
      audit("skill", "edit", name);
      return null;
    }
    case "skill_run_tests": return [
      { case: "conserve-le-ticket", passed: true, detail: "ok" },
      { case: "trailer-signed-off-intact", passed: true, detail: "ok" },
    ];
    case "ai_preview": return "[system]\nTu assistes à la normalisation de messages…\n\n[user]\nMessage actuel :\nwip\n…";
    case "proposals_generate": {
      const groups = args.groups as string[][];
      const skill = String(args.skill);
      const remoteDefault = mock.providers.find(
        (p) => p.is_default && (p.kind === "open_ai_compat" || p.kind === "anthropic"),
      );
      if (remoteDefault && !args.consentRemote) {
        throw {
          code: "consent_required",
          message:
            "consentement requis : envoi à un fournisseur IA distant : accord explicite requis, aperçu des données à l'appui",
        } satisfies IpcError;
      }
      const out: Proposal[] = [];
      for (let gi = 0; gi < groups.length; gi++) {
        await playPhases(args.taskId, "proposals_generate", [
          ["génération des propositions", gi + 1, groups.length, 130],
        ]);
        const g = groups[gi];
        const c = demoCommits.find((x) => x.sha === g[0]);
        const refused = skill === "ai-signature-cleaner" && (mock.repos[0].governance.ai_attribution_policy === "keep-required");
        const after = refused ? null : skill === "commit-synthesis"
          ? "fix(pay): stabilize payment flow\n\nSynthèse de 2 commits d'itération."
          : `fix(pay): stabilise payment retries`;
        // T11 : le texte arrive en streaming avant la proposition finale.
        if (after) await streamAiText(args.taskId, gi, after);
        const p: Proposal = {
          id: id("prp"), repo_id: String(args.repoId), skill, skill_version: "1.0.0", targets: g,
          before: c ? `${c.subject}\n\n${c.body}`.trim() : "…",
          after,
          explanation: refused
            ? "La politique du dépôt exige la conservation de la traçabilité IA (keep-required) : normalisation refusée."
            : "Heuristique locale (sans LLM) : type « fix » inféré ; références conservées.",
          risk: "low", status: refused ? "refused" : "proposed", decision: null, created_at: now(),
        };
        // Comme le cœur : chaque proposition est enregistrée dès sa génération
        // (une annulation conserve celles déjà produites).
        mock.proposals.unshift(p);
        audit("skill", "proposal", skill);
        out.push(p);
      }
      return out;
    }
    case "proposals_list": return mock.proposals.filter((p) => p.repo_id === args.repoId);
    case "proposal_decide": {
      const p = mock.proposals.find((x) => x.id === args.proposalId)!;
      const d = String(args.decision);
      if (d === "accept") { p.status = "accepted"; p.decision = p.after; }
      else if (d === "edit") { p.status = "edited"; p.decision = String(args.editedMessage); }
      else { p.status = "rejected"; p.decision = null; }
      audit("skill", "decision", p.skill);
      return p;
    }
    case "plan_new": {
      const p: Plan = {
        id: id("pln"), version: 1, repo_id: String(args.repoId),
        fingerprint: { branch: String(args.branch), tip: demoCommits[3].sha, base: "00".padEnd(40, "9") },
        status: "draft", ops: [], dry_run_hash: null, preview_ref: null, backup_ref: null,
        backup_tag: null, mapping: [], created_at: now(), dry_run_at: null, applied_at: null, error: null,
        conflict: null,
      };
      mock.plans.unshift(p); return p;
    }
    case "plan_set_ops": {
      const p = mock.plans.find((x) => x.id === args.planId)!;
      mock.resolvedConflicts.delete(p.id); // rééditer les ops ré-arme le conflit de démo
      p.ops = args.ops as PlanOp[]; p.status = "draft"; p.mapping = []; return p;
    }
    case "plan_dry_run": {
      const p = mock.plans.find((x) => x.id === args.planId)!;
      // DEMO C1 : un réordonnancement des commits de démo (qui touchent le même
      // fichier) simule un conflit de rejeu à résoudre — sauf s'il l'a déjà été.
      const hasReorder = p.ops.some((o) => o.op === "reorder");
      if (hasReorder && !mock.resolvedConflicts.has(p.id)) {
        await playPhases(args.taskId, "plan_dry_run", [
          ["vérification de l'empreinte", 0, null, 200],
          ["rejeu de la structure (sequencer git)", 0, null, 450],
        ]);
        p.status = "conflict"; p.dry_run_at = null; p.preview_ref = null; p.mapping = [];
        p.conflict = {
          files: [{
            path: "src/pay.rs",
            content:
              "pub fn pay() {\n<<<<<<< (reordonné) HEAD\n    charge_express_v3();\n=======\n    charge_express_v2();\n>>>>>>> commit deplace\n}\n",
          }],
        };
        audit("git_rewrite", "dry_run", p.fingerprint.branch);
        return p;
      }
      await playPhases(args.taskId, "plan_dry_run", [
        ["vérification de l'empreinte", 0, null, 250],
        ["rejeu de la structure (sequencer git)", 0, null, 550],
        ["réécriture des messages", 1, 3, 180],
        ["réécriture des messages", 2, 3, 180],
        ["réécriture des messages", 3, 3, 180],
        ["contrôle des invariants et écriture de la préview", 0, null, 250],
      ]);
      p.status = "dry_run_ok"; p.dry_run_at = now(); p.conflict = null;
      p.preview_ref = `refs/mc/preview/${p.id}`;
      p.mapping = demoCommits.slice(0, 3).map((c, i) => ({ old: [c.sha], new: `f${i}`.padEnd(40, "1") }));
      audit("git_rewrite", "dry_run", p.fingerprint.branch);
      return p;
    }
    case "plan_conflict_resolve": {
      // Mock : la résolution est portée par l'UI (contenu édité) ; rien à stocker.
      return null;
    }
    case "plan_conflict_continue": {
      const p = mock.plans.find((x) => x.id === args.planId)!;
      await playPhases(args.taskId, "plan_conflict_continue", [
        ["rejeu de la structure (sequencer git)", 0, null, 350],
        ["réécriture des messages", 1, 2, 160],
        ["contrôle des invariants et écriture de la préview", 0, null, 220],
      ]);
      mock.resolvedConflicts.add(p.id);
      p.status = "dry_run_ok"; p.dry_run_at = now(); p.conflict = null;
      p.preview_ref = `refs/mc/preview/${p.id}`;
      p.mapping = demoCommits.slice(0, 3).map((c, i) => ({ old: [c.sha], new: `f${i}`.padEnd(40, "1") }));
      audit("git_rewrite", "dry_run_continue", p.fingerprint.branch);
      return p;
    }
    case "plan_conflict_abort": {
      const p = mock.plans.find((x) => x.id === args.planId)!;
      mock.resolvedConflicts.delete(p.id);
      p.status = "draft"; p.conflict = null; p.mapping = []; p.preview_ref = null;
      audit("git_rewrite", "dry_run_abort", p.fingerprint.branch);
      return p;
    }
    case "plan_apply": {
      const p = mock.plans.find((x) => x.id === args.planId)!;
      if (p.status !== "dry_run_ok") {
        throw {
          code: "refused",
          message: "refusé : dry-run requis avant application (aucun dry-run réussi pour ce plan)",
        } satisfies IpcError;
      }
      await playPhases(args.taskId, "plan_apply", [
        ["contrôles préalables (empreinte, préview, partage)", 0, null, 300],
      ]);
      // Le commit a1 de la démo est « partagé » → confirmation renforcée.
      if (args.confirm !== p.fingerprint.branch) {
        throw {
          code: "confirm_required",
          expected: p.fingerprint.branch,
          message: `confirmation requise : branche partagée : saisir exactement « ${p.fingerprint.branch} » pour confirmer la réécriture`,
        } satisfies IpcError;
      }
      await playPhases(args.taskId, "plan_apply", [
        ["création du backup", 0, null, 300],
        ["bascule de la branche (non annulable)", 0, null, 250],
      ]);
      p.status = "applied"; p.applied_at = now();
      p.backup_ref = `refs/mc/backup/${p.fingerprint.branch}/20260724T100000Z`;
      p.backup_tag = `refs/tags/mc-backup-${p.id}`;
      audit("git_rewrite", "apply", p.fingerprint.branch);
      return p;
    }
    case "plan_rollback": {
      const p = mock.plans.find((x) => x.id === args.planId)!;
      p.status = "rolled_back"; audit("git_rewrite", "rollback", p.fingerprint.branch); return p;
    }
    case "plan_list": return mock.plans.filter((p) => p.repo_id === args.repoId);
    case "push_preview": {
      const branch = String(args.branch);
      const forced = !demoRepo.protected_branches.includes(branch);
      const prs = args.ciAccountId
        ? [{ number: 42, title: "feat(pay): express payment flow", url: "https://github.com/mister-guiiug/mister-commitia/pull/42" }]
        : null;
      return {
        remote: "origin", remote_url: demoRepo.remote_url, branch,
        local_tip: "f0".padEnd(40, "1"), remote_tip: demoCommits[3].sha,
        ahead: 3, behind: forced ? 3 : 0, needs_force: forced,
        protected: demoRepo.protected_branches.includes(branch), can_push: true,
        open_prs: prs,
        warnings: [
          ...(forced ? ["réécriture de l'historique distant : 3 commit(s) distant(s) seront remplacés. À coordonner (les collègues devront réaligner leur copie) ; push forcé sécurisé par --force-with-lease."] : []),
          ...(prs && forced ? ["1 PR ouverte(s) sur cette branche : le push forcé mettra à jour leur contenu."] : []),
        ],
      } satisfies PushPreview;
    }
    case "push_execute": {
      const branch = String(args.branch);
      if (demoRepo.protected_branches.includes(branch)) {
        throw { code: "refused", message: `refusé : « ${branch} » est protégée : push forcé refusé (réécriture d'historique distant interdite)` } satisfies IpcError;
      }
      if (args.confirm !== branch) {
        throw {
          code: "confirm_required", expected: branch,
          message: `confirmation requise : push forcé (--force-with-lease) : saisir exactement « ${branch} » pour confirmer la réécriture de l'historique distant`,
        } satisfies IpcError;
      }
      audit("git_push", "push", branch);
      return { branch, forced: true, remote_tip: "f0".padEnd(40, "1"), detail: "historique distant réécrit (force-with-lease)" } satisfies PushResult;
    }
    case "plan_export": return JSON.stringify(mock.plans.find((x) => x.id === args.planId), null, 2);
    case "plan_risk": return [
      { axe: "branche", verdict: "ok", motif: "« feature/express-payment » n'est pas protégée" },
      { axe: "partage", verdict: "attention", motif: "1 commit déjà poussé : confirmation renforcée exigée" },
      { axe: "signatures", verdict: "attention", motif: "1 commit signé perdra sa signature" },
      { axe: "pertes", verdict: "ok", motif: "aucune suppression de contenu" },
      { axe: "réversibilité", verdict: "ok", motif: "backup branche + tag avant application" },
    ] as RiskAxis[];
    case "required_scopes": return String(args.kind).startsWith("azure")
      ? [["Inventaire des builds et leases", "Scope PAT « Build (read) » (vso.build)"], ["Suppression d'un build", "Scope « Build (read & execute) » + permission « Delete builds »"]]
      : [["Inventaire des workflows et runs", "PAT fine-grained : « Actions: read »"], ["Suppression de runs / logs / artifacts", "PAT fine-grained : « Actions: write »"]];
    case "ci_account_add": {
      const a: CiAccount = { id: id("acct"), kind: args.kind as CiAccount["kind"], base_url: String(args.baseUrl), org: (args.org as string) ?? null, project: (args.project as string) ?? null, repo: (args.repo as string) ?? null, token_ref: "ci:x", scopes: args.scopes as string[], added_at: now() };
      mock.accounts.push(a); audit("secret", "ci_account_add", a.base_url);
      return [a, "accès en lecture confirmé (démo)"];
    }
    case "ci_account_list": return mock.accounts;
    case "ci_account_remove": mock.accounts = mock.accounts.filter((a) => a.id !== args.id); return null;
    case "ci_inventory":
      await playPhases(args.taskId, "ci_inventory", [
        ["inventaire des runs", 0, 500, 350],
        ["inventaire des runs", 100, 500, 350],
        ["inventaire des runs", 200, 500, 350],
      ]);
      return demoRuns;
    case "policy_save": {
      const p: RetentionPolicy = { id: id("pol"), name: String(args.name), rules: args.rules as RetentionPolicy["rules"], enabled: true };
      mock.policies.push(p); return p;
    }
    case "policy_list": return mock.policies;
    case "ci_simulate": {
      await playPhases(args.taskId, "ci_simulate", [
        ["inventaire des runs", 0, 500, 350],
        ["inventaire des runs", 100, 500, 350],
        ["application de la politique de rétention", 0, null, 300],
      ]);
      mock.simulated = true;
      const report: SimulationReport = {
        id: id("sim"), policy_id: String(args.policyId), account_id: String(args.accountId),
        generated_at: now(), total: demoRuns.length,
        candidates: demoRuns.filter((r) => !r.leased && !r.running && r.branch !== "main").slice(4),
        protected: [
          { run: demoRuns[0], reason: "en cours d'exécution" },
          { run: demoRuns[5], reason: "retenu par une rétention (lease/keep-forever)" },
          { run: demoRuns[3], reason: "branche protégée (main)" },
        ],
        kept_recent: 5, scope_hash: "demo",
      };
      audit("ci_cleanup", "simulate", "démo");
      return report;
    }
    case "ci_delete_run": {
      if (!mock.simulated) {
        throw {
          code: "refused",
          message: "refusé : aucune simulation préalable pour ce périmètre : exécuter la simulation d'abord",
        } satisfies IpcError;
      }
      const run = args.run as CiRun;
      if (args.confirm !== run.pipeline_name) {
        throw {
          code: "confirm_required",
          expected: run.pipeline_name,
          message: `confirmation requise : confirmation invalide : saisir exactement « ${run.pipeline_name} »`,
        } satisfies IpcError;
      }
      audit("ci_cleanup", "delete", `run ${run.run_id}`);
      return null;
    }
    case "ci_delete_batch": {
      if (!mock.simulated) {
        throw { code: "refused", message: "refusé : aucune simulation préalable pour ce périmètre : exécuter la simulation d'abord" } satisfies IpcError;
      }
      const bruns = (args.runs as CiRun[]) ?? [];
      const done = new Set<string>((args.alreadyDone as string[]) ?? []);
      const pendingRuns = bruns.filter((r) => !done.has(r.run_id));
      const expected = String(pendingRuns.length);
      if (args.confirm !== expected) {
        throw { code: "confirm_required", expected, message: `confirmation requise : suppression en masse : saisir exactement « ${expected} » (nombre de runs)` } satisfies IpcError;
      }
      const tid = typeof args.taskId === "string" && args.taskId ? args.taskId : null;
      const deleted = new Set<string>(done);
      let cancelled = false;
      for (let i = 0; i < pendingRuns.length; i++) {
        if (tid && mockCancelled.has(tid)) { cancelled = true; break; }
        if (tid) emitTask({ task_id: tid, task: "ci_delete_batch", kind: "progress", phase: "suppression des runs", current: i, total: pendingRuns.length });
        await sleep(200);
        if (tid && mockCancelled.has(tid)) { cancelled = true; break; }
        deleted.add(pendingRuns[i].run_id);
        audit("ci_cleanup", "delete", `run ${pendingRuns[i].run_id}`);
      }
      return { total: pendingRuns.length, deleted: [...deleted], failed: [], cancelled } satisfies BatchDeleteResult;
    }
    case "ci_purge_assets": {
      const pruns = ((args.runs as CiRun[]) ?? []).filter((r) => !r.running);
      const expected = String(pruns.length);
      if (args.confirm !== expected) {
        throw { code: "confirm_required", expected, message: `confirmation requise : purge : saisir exactement « ${expected} » (nombre de runs)` } satisfies IpcError;
      }
      if (!args.purgeLogs && !args.purgeArtifacts) {
        throw { code: "invalid", message: "invalide : rien à purger (activer logs et/ou artefacts)" } satisfies IpcError;
      }
      const tid = typeof args.taskId === "string" && args.taskId ? args.taskId : null;
      let arts = 0, logs = 0, cancelled = false;
      for (let i = 0; i < pruns.length; i++) {
        if (tid && mockCancelled.has(tid)) { cancelled = true; break; }
        if (tid) emitTask({ task_id: tid, task: "ci_purge_assets", kind: "progress", phase: "purge des logs/artefacts", current: i, total: pruns.length });
        await sleep(180);
        if (tid && mockCancelled.has(tid)) { cancelled = true; break; }
        if (args.purgeArtifacts) arts += 2;
        if (args.purgeLogs) logs += 1;
        audit("ci_cleanup", "purge", `run ${pruns[i].run_id}`);
      }
      return { runs: cancelled ? 0 : pruns.length, artifacts_deleted: arts, logs_deleted: logs, failed: [], cancelled } satisfies PurgeResult;
    }
    case "ai_provider_save": {
      const p = { id: id("ai"), kind: String(args.kind), base_url: (args.baseUrl as string) ?? null, model: (args.model as string) ?? null, key_ref: args.apiKey ? "ai:x" : null, is_default: Boolean(args.isDefault) };
      if (p.is_default) mock.providers.forEach((x) => (x.is_default = false));
      mock.providers.push(p); audit("secret", "ai_provider_save", p.kind); return p;
    }
    case "ai_provider_list": return mock.providers;
    case "ai_provider_remove": mock.providers = mock.providers.filter((p) => p.id !== args.id); return null;
    case "audit_list": return mock.audit;
    case "audit_export": return mock.audit.map((e) => JSON.stringify(e)).join("\n");
    default:
      throw { code: "invalid", message: `commande mock inconnue : ${cmd}` } satisfies IpcError;
  }
}
