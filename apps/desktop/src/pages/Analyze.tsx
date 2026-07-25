import { useEffect, useMemo, useRef, useState } from "react";
import {
  CheckCircle2, Download, FileText, FlaskConical, GitBranch, GitGraph as GitGraphIcon, List,
  Play, RotateCcw, ShieldCheck, Sparkles, Undo2, Upload, UploadCloud, XCircle,
} from "lucide-react";
import { asIpcError, call } from "../ipc";
import type {
  BranchInfo, CiAccount, CommitInfo, Plan, PlanOp, Proposal, PushPreview, PushResult,
  RepoRef, RiskAxis, ScanResult,
} from "../types";
import GitGraph from "../GitGraph";
import { t, useLang } from "../i18n";
import { useTask } from "../tasks";
import {
  Badge, Button, Card, ConfirmTyped, Empty, ErrorBox, ICON_SM, Modal, ProgressPanel,
  VerdictBadge, VerdictLegend, inputCls, riskTone, shaCls, thCls, trCls, useToast,
} from "../ui";

const flagLabels: Record<string, [string, string]> = {
  weak_message: ["message faible", "amber"],
  non_conventional: ["non conforme", "violet"],
  ai_signature: ["mention d'outil", "sky"],
  oversized_no_body: ["gros diff sans corps", "amber"],
  duplicate_message: ["doublon", "amber"],
};

export default function AnalyzePage({ repo }: { repo: RepoRef }) {
  useLang();
  const toast = useToast();
  const [scan, setScan] = useState<ScanResult | null>(null);
  const [branches, setBranches] = useState<BranchInfo[]>([]);
  const [branch, setBranch] = useState<string>("");
  const [baseRef, setBaseRef] = useState<string>("");
  const [selection, setSelection] = useState<Set<string>>(new Set());
  const [skill, setSkill] = useState("conventional-commits");
  const [proposals, setProposals] = useState<Proposal[]>([]);
  const [editing, setEditing] = useState<Record<string, string>>({});
  const [plan, setPlan] = useState<Plan | null>(null);
  const [risks, setRisks] = useState<RiskAxis[]>([]);
  const [drops, setDrops] = useState<Set<string>>(new Set());
  const [consent, setConsent] = useState<{ preview: string; groups: string[][] } | null>(null);
  const [confirmReq, setConfirmReq] = useState<{ expected: string; message: string } | null>(null);
  const [diffView, setDiffView] = useState<{ sha: string; subject: string; patch: string } | null>(null);
  const [order, setOrder] = useState<string[]>([]);
  const [view, setView] = useState<"list" | "graph">("list");
  const [pushPreview, setPushPreview] = useState<PushPreview | null>(null);
  const [pushTyped, setPushTyped] = useState("");
  const [pushing, setPushing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // T11 : texte streamé par groupe pendant la génération (affiché en direct).
  const [streams, setStreams] = useState<Record<number, string>>({});
  const importRef = useRef<HTMLInputElement>(null);
  const dragSha = useRef<string | null>(null);
  const task = useTask((group, delta) =>
    setStreams((s) => ({ ...s, [group]: (s[group] ?? "") + delta })),
  );

  // F6 : base explicite du segment (branche/tag/SHA). Vide = merge-base auto.
  // `base` explicite (null pour forcer l'auto) prime sur l'état `baseRef`.
  const doScan = async (b?: string, base?: string | null) => {
    setError(null);
    setBusy(true);
    const taskId = task.begin("Analyse de la branche");
    try {
      const res = await call<ScanResult>("repo_scan", {
        id: repo.id,
        branch: b ?? null,
        base: base === undefined ? baseRef.trim() || null : base,
        taskId,
      });
      setScan(res);
      setBranch(res.branch);
      setSelection(new Set());
      setOrder(res.commits.map((c) => c.sha));
      setProposals(await call<Proposal[]>("proposals_list", { repoId: repo.id }));
    } catch (e) {
      const ie = asIpcError(e);
      if (ie.code === "cancelled") toast("info", "Analyse annulée — aucun effet de bord");
      else setError(ie.message);
    } finally {
      task.end();
      setBusy(false);
    }
  };

  useEffect(() => {
    void doScan();
    call<BranchInfo[]>("repo_branches", { id: repo.id }).then(setBranches).catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [repo.id]);

  const commitsBySha = useMemo(() => {
    const m = new Map<string, CommitInfo>();
    scan?.commits.forEach((c) => m.set(c.sha, c));
    return m;
  }, [scan]);

  const flagsFor = (sha: string) => scan?.report.flags.filter((f) => f.sha === sha) ?? [];

  // Ordre d'affichage = ordre du futur plan (F2). Dérivé de `order`.
  const displayCommits: CommitInfo[] = order
    .map((sha) => commitsBySha.get(sha))
    .filter((c): c is CommitInfo => Boolean(c));
  const reordered = scan ? order.join() !== scan.commits.map((c) => c.sha).join() : false;

  const moveInOrder = (sha: string, delta: number) =>
    setOrder((o) => {
      const i = o.indexOf(sha);
      const j = i + delta;
      if (i < 0 || j < 0 || j >= o.length) return o;
      const n = [...o];
      [n[i], n[j]] = [n[j], n[i]];
      return n;
    });

  const dropOn = (targetSha: string) => {
    const from = dragSha.current;
    dragSha.current = null;
    if (!from || from === targetSha) return;
    setOrder((o) => {
      const n = o.filter((s) => s !== from);
      n.splice(n.indexOf(targetSha), 0, from);
      return n;
    });
  };

  const showDiff = async (c: CommitInfo) => {
    setError(null);
    try {
      const patch = await call<string>("commit_diff", { repoId: repo.id, sha: c.sha });
      setDiffView({ sha: c.short, subject: c.subject, patch });
    } catch (e) {
      setError(asIpcError(e).message);
    }
  };

  const toggle = (sha: string) =>
    setSelection((s) => {
      const n = new Set(s);
      if (n.has(sha)) n.delete(sha);
      else n.add(sha);
      return n;
    });

  const groupsForSkill = (): string[][] => {
    const sel = scan!.commits.filter((c) => selection.has(c.sha)).map((c) => c.sha);
    if (skill === "commit-synthesis") return sel.length >= 2 ? [sel] : [];
    return sel.map((s) => [s]);
  };

  const generate = async (consentRemote: boolean, groups?: string[][]) => {
    setError(null);
    const g = groups ?? groupsForSkill();
    if (g.length === 0) {
      setError(
        skill === "commit-synthesis"
          ? "Sélectionner au moins deux commits pour une synthèse."
          : "Sélectionner au moins un commit.",
      );
      return;
    }
    setBusy(true);
    setStreams({});
    const taskId = task.begin("Génération des propositions");
    try {
      const generated = await call<Proposal[]>("proposals_generate", {
        repoId: repo.id, skill, groups: g, providerId: null, consentRemote, taskId,
      });
      setProposals(await call<Proposal[]>("proposals_list", { repoId: repo.id }));
      setConsent(null);
      const refused = generated.filter((p) => p.status === "refused").length;
      toast(
        refused === generated.length ? "info" : "success",
        refused > 0
          ? `${generated.length} proposition(s), dont ${refused} refus de gouvernance`
          : `${generated.length} proposition(s) générée(s) — à vous de décider`,
      );
    } catch (e) {
      const ie = asIpcError(e);
      if (ie.code === "consent_required") {
        const preview = await call<string>("ai_preview", { repoId: repo.id, skill, shas: g[0] });
        setConsent({ preview, groups: g });
      } else if (ie.code === "cancelled") {
        setProposals(await call<Proposal[]>("proposals_list", { repoId: repo.id }));
        toast("info", "Génération annulée — les propositions déjà produites sont conservées");
      } else {
        setError(ie.message);
      }
    } finally {
      task.end();
      setStreams({});
      setBusy(false);
    }
  };

  const decide = async (p: Proposal, decision: "accept" | "edit" | "reject") => {
    setError(null);
    try {
      await call<Proposal>("proposal_decide", {
        proposalId: p.id,
        decision,
        editedMessage: decision === "edit" ? (editing[p.id] ?? p.after ?? "") : null,
      });
      setProposals(await call<Proposal[]>("proposals_list", { repoId: repo.id }));
      if (decision !== "reject") toast("success", decision === "edit" ? "Édition validée par les garde-fous" : "Proposition acceptée");
    } catch (e) {
      setError(asIpcError(e).message);
    }
  };

  const buildPlan = async () => {
    setError(null);
    setBusy(true);
    try {
      const p = await call<Plan>("plan_new", { repoId: repo.id, branch });
      let seq = 1;
      const ops: PlanOp[] = [];
      const inSegment = new Set(scan!.commits.map((c) => c.sha));
      // F2 : le réordonnancement visuel devient une opération du plan.
      if (reordered) {
        ops.push({
          op: "reorder",
          order: order.filter((sha) => !drops.has(sha)),
          seq: seq++,
          origin: "manuel",
          risk: "medium",
          approved_by: "utilisateur",
          approved_at: new Date().toISOString(),
        });
      }
      for (const prop of proposals) {
        if ((prop.status !== "accepted" && prop.status !== "edited") || !prop.decision) continue;
        if (!prop.targets.every((t) => inSegment.has(t))) continue;
        const base = {
          seq: seq++,
          origin: `skill:${prop.skill}@${prop.skill_version}`,
          risk: prop.risk,
          approved_by: "utilisateur",
          approved_at: new Date().toISOString(),
        };
        if (prop.targets.length === 1) {
          ops.push({ op: "reword", target: prop.targets[0], new_message: prop.decision, ...base });
        } else {
          ops.push({ op: "squash", targets: prop.targets, new_message: prop.decision, ...base });
        }
      }
      for (const sha of drops) {
        ops.push({
          op: "drop", target: sha, reason: "abandon manuel",
          seq: seq++, origin: "manuel", risk: "high",
          approved_by: "utilisateur", approved_at: new Date().toISOString(),
        });
      }
      if (ops.length === 0) {
        setError("Aucune opération : accepter des propositions ou marquer des abandons d'abord.");
        return;
      }
      const withOps = await call<Plan>("plan_set_ops", { planId: p.id, ops });
      setPlan(withOps);
      setRisks(await call<RiskAxis[]>("plan_risk", { planId: withOps.id }));
      toast("info", `Plan composé (${ops.length} opération(s)) — dry-run requis avant application`);
    } catch (e) {
      setError(asIpcError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const dryRun = async () => {
    if (!plan) return;
    setError(null);
    setBusy(true);
    const taskId = task.begin("Dry-run du plan");
    try {
      setPlan(await call<Plan>("plan_dry_run", { planId: plan.id, taskId }));
      setRisks(await call<RiskAxis[]>("plan_risk", { planId: plan.id }));
      toast("success", "Dry-run réussi — résultat réel construit dans la préview, branche intacte");
    } catch (e) {
      const ie = asIpcError(e);
      if (ie.code === "cancelled") toast("info", "Dry-run annulé — branche et préview intactes");
      else setError(ie.message);
    } finally {
      task.end();
      setBusy(false);
    }
  };

  const apply = async (confirm?: string) => {
    if (!plan) return;
    setError(null);
    setBusy(true);
    const taskId = task.begin("Application du plan");
    try {
      const updated = await call<Plan>("plan_apply", { planId: plan.id, confirm: confirm ?? null, taskId });
      setPlan(updated);
      setConfirmReq(null);
      task.end();
      toast("success", `Plan appliqué — backup ${updated.backup_ref ?? ""}`);
      await doScan(branch);
    } catch (e) {
      const ie = asIpcError(e);
      if (ie.code === "confirm_required") {
        setConfirmReq({ expected: ie.expected ?? branch, message: ie.message });
      } else if (ie.code === "cancelled") {
        toast("info", "Application annulée avant le backup — rien n'a été écrit");
      } else {
        setError(ie.message);
      }
    } finally {
      task.end();
      setBusy(false);
    }
  };

  const rollback = async () => {
    if (!plan) return;
    setError(null);
    setBusy(true);
    try {
      setPlan(await call<Plan>("plan_rollback", { planId: plan.id }));
      toast("success", "Branche restaurée depuis le backup");
      await doScan(branch);
    } catch (e) {
      setError(asIpcError(e).message);
    } finally {
      setBusy(false);
    }
  };

  // F4 : push assisté — aperçu (divergence, PR ouvertes) puis force-with-lease guidé.
  const openPush = async () => {
    setError(null);
    setPushTyped("");
    setBusy(true);
    try {
      const accounts = await call<CiAccount[]>("ci_account_list");
      const gh = accounts.find((a) => a.kind === "github" || a.kind === "github_enterprise");
      const pv = await call<PushPreview>("push_preview", {
        repoId: repo.id, branch, ciAccountId: gh?.id ?? null,
      });
      setPushPreview(pv);
    } catch (e) {
      setError(asIpcError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const doPush = async () => {
    if (!pushPreview) return;
    setPushing(true);
    try {
      const res = await call<PushResult>("push_execute", {
        repoId: repo.id, branch, confirm: pushPreview.needs_force ? pushTyped : null,
      });
      setPushPreview(null);
      toast(
        "success",
        res.forced
          ? "Push forcé effectué (force-with-lease) — historique distant réécrit"
          : "Commits poussés vers le remote",
      );
      await doScan(branch);
    } catch (e) {
      setError(asIpcError(e).message);
    } finally {
      setPushing(false);
    }
  };

  const exportPlan = async () => {
    if (!plan) return;
    const content = await call<string>("plan_export", { planId: plan.id });
    const blob = new Blob([content], { type: "application/json" });
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = `${plan.id}.json`;
    a.click();
    URL.revokeObjectURL(a.href);
    toast("success", "Plan exporté (JSON reproductible)");
  };

  // F9 : rapport HTML autonome (revue d'équipe hors outil).
  const esc = (s: string) =>
    s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  const exportHtmlReport = () => {
    if (!plan || !scan) return;
    const opRows = plan.ops
      .map((o) => {
        const cible = "target" in o ? o.target.slice(0, 8) : "targets" in o ? o.targets.map((t) => t.slice(0, 8)).join(" + ") : "order" in o ? o.order.map((t) => t.slice(0, 8)).join(" → ") : "";
        const detail = "new_message" in o ? o.new_message.split("\n")[0] : "reason" in o ? o.reason : "";
        return `<tr><td>${o.seq}</td><td>${o.op}</td><td><code>${esc(cible)}</code></td><td>${esc(detail)}</td><td>${o.risk}</td></tr>`;
      })
      .join("");
    const riskRows = risks
      .map((r) => `<li><b class="${r.verdict}">${esc(r.axe)} : ${r.verdict}</b> — ${esc(r.motif)}</li>`)
      .join("");
    const mapRows = plan.mapping
      .map((m) => {
        const subjects = m.old.map((o) => commitsBySha.get(o)?.subject ?? "").filter(Boolean).join(" + ");
        return `<tr><td><code>${m.old.map((o) => o.slice(0, 8)).join("+")}</code></td><td><code>${m.new.slice(0, 8)}</code></td><td>${esc(subjects)}</td></tr>`;
      })
      .join("");
    const html = `<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>Plan ${plan.id} — ${esc(repo.name)}</title>
<style>body{font-family:system-ui;margin:2rem auto;max-width:60rem;color:#1e293b}table{border-collapse:collapse;width:100%;margin:.5rem 0}td,th{border:1px solid #cbd5e1;padding:.3rem .5rem;text-align:left;font-size:.9rem}code{font-family:ui-monospace,monospace;background:#f1f5f9;padding:0 .2rem}.ok{color:#0f766e}.attention{color:#b45309}.bloquant{color:#be123c}h1{font-size:1.3rem}h2{font-size:1rem;margin-top:1.5rem}footer{margin-top:2rem;font-size:.8rem;color:#64748b}</style></head><body>
<h1>Plan de réécriture — ${esc(repo.name)} · ${esc(plan.fingerprint.branch)}</h1>
<p>Statut : <b>${plan.status}</b> · généré le ${new Date().toLocaleString("fr-FR")} · plan <code>${plan.id}</code></p>
<h2>Opérations (${plan.ops.length})</h2><table><tr><th>#</th><th>Opération</th><th>Cible(s)</th><th>Détail</th><th>Risque</th></tr>${opRows}</table>
<h2>Risques</h2><ul>${riskRows}</ul>
${plan.mapping.length > 0 ? `<h2>Avant / après (dry-run réel — ${esc(plan.preview_ref ?? "")})</h2><table><tr><th>Anciens SHA</th><th>Nouveau</th><th>Sujets</th></tr>${mapRows}</table>` : ""}
${plan.backup_ref ? `<p>Backup : <code>${esc(plan.backup_ref)}</code> · tag <code>${esc(plan.backup_tag ?? "")}</code></p>` : ""}
<footer>Généré par mister-commitia — garde-fous : dry-run obligatoire, backup automatique, branches protégées bloquées, journal d'audit local.</footer>
</body></html>`;
    const blob = new Blob([html], { type: "text/html" });
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = `rapport-${plan.id}.html`;
    a.click();
    URL.revokeObjectURL(a.href);
    toast("success", "Rapport HTML exporté");
  };

  const importPlan = async (file: File) => {
    setError(null);
    try {
      const json = await file.text();
      const imported = await call<Plan>("plan_import", { repoId: repo.id, json });
      setPlan(imported);
      setRisks(await call<RiskAxis[]>("plan_risk", { planId: imported.id }));
      toast("success", "Plan importé — statut brouillon, dry-run requis");
    } catch (e) {
      setError(asIpcError(e).message);
    }
  };

  if (!scan) return <p className="text-sm text-slate-500">{error ?? t("an.analyzing")}</p>;

  const statusBadge = plan && (
    <Badge
      tone={
        plan.status === "applied" ? "teal"
        : plan.status === "dry_run_ok" ? "sky"
        : plan.status === "rolled_back" ? "amber"
        : "slate"
      }
    >
      {plan.status === "draft" ? t("an.st.draft")
        : plan.status === "dry_run_ok" ? t("an.st.dryRunOk")
        : plan.status === "applied" ? t("an.st.applied")
        : plan.status === "rolled_back" ? t("an.st.rolledBack")
        : plan.status}
    </Badge>
  );

  return (
    <div className="mx-auto max-w-6xl space-y-4">
      <div className="flex items-center gap-3">
        <GitBranch size={ICON_SM} className="text-teal-400" />
        <span className="font-semibold">{repo.name}</span>
        <select
          className={inputCls + " !w-auto"}
          aria-label={t("an.branchAria")}
          value={branch}
          onChange={(e) => {
            setPlan(null);
            setDrops(new Set());
            setBaseRef("");
            void doScan(e.target.value, null);
          }}
        >
          {branches.map((b) => (
            <option key={b.name} value={b.name}>
              {b.name}
              {b.is_head ? ` ${t("an.current")}` : ""}
              {repo.protected_branches.includes(b.name) ? " 🔒" : ""}
            </option>
          ))}
          {!branches.some((b) => b.name === branch) && <option value={branch}>{branch}</option>}
        </select>
        {/* F6 : base explicite du segment (branche/tag/SHA), utile pour branches empilées. */}
        <div className="flex items-center gap-1 text-xs text-slate-400">
          <label htmlFor="baseref">base&nbsp;:</label>
          <input
            id="baseref"
            className={inputCls + " !w-44"}
            placeholder="auto (merge-base)"
            value={baseRef}
            onChange={(e) => setBaseRef(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void doScan(branch);
            }}
            aria-label="Base du segment : branche, tag ou SHA (F6)"
            title="Forcer la base du segment (branche, tag ou SHA) — utile pour les branches empilées"
          />
          <Button kind="ghost" onClick={() => void doScan(branch)} disabled={busy}>
            appliquer
          </Button>
          {baseRef && (
            <Button
              kind="ghost"
              onClick={() => {
                setBaseRef("");
                void doScan(branch, null);
              }}
              disabled={busy}
            >
              auto
            </Button>
          )}
        </div>
        <div className="ml-auto flex gap-2 text-xs">
          <Badge tone="slate">{scan.report.total} {t("an.commits")}</Badge>
          <Badge tone="teal">{scan.report.conform} {t("an.conform")}</Badge>
          <Badge tone="amber">{scan.report.weak} {t("an.weak")}</Badge>
          <Badge tone="sky">{scan.report.ai_signatures} {t("an.aiMentions")}</Badge>
        </div>
      </div>
      <ErrorBox error={error} />
      {task.running && (
        <ProgressPanel
          label={task.running.label}
          phase={task.running.phase}
          current={task.running.current}
          total={task.running.total}
          onCancel={task.cancel}
        />
      )}

      <Card
        title={
          <span className="flex items-center gap-2">
            {t("an.segTitle")}
            {reordered && (
              <>
                <Badge tone="amber">{t("an.orderChanged")}</Badge>
                <Button kind="ghost" onClick={() => setOrder(scan.commits.map((c) => c.sha))}>
                  {t("an.resetOrder")}
                </Button>
              </>
            )}
          </span>
        }
        actions={
          <>
            <div className="mr-1 inline-flex overflow-hidden rounded border border-slate-700" role="group" aria-label={t("an.viewMode")}>
              <button
                type="button"
                aria-pressed={view === "list"}
                onClick={() => setView("list")}
                className={`inline-flex items-center gap-1 px-2 py-1 text-xs ${view === "list" ? "bg-slate-700 text-slate-100" : "text-slate-400 hover:bg-slate-800"}`}
              >
                <List size={ICON_SM} /> {t("an.list")}
              </button>
              <button
                type="button"
                aria-pressed={view === "graph"}
                onClick={() => setView("graph")}
                className={`inline-flex items-center gap-1 px-2 py-1 text-xs ${view === "graph" ? "bg-slate-700 text-slate-100" : "text-slate-400 hover:bg-slate-800"}`}
              >
                <GitGraphIcon size={ICON_SM} /> {t("an.graph")}
              </button>
            </div>
            <select
              className={inputCls + " !w-auto"}
              aria-label={t("an.skillAria")}
              value={skill}
              onChange={(e) => setSkill(e.target.value)}
            >
              <option value="conventional-commits">{t("an.skillConv")}</option>
              <option value="commit-synthesis">{t("an.skillSynth")}</option>
              <option value="ai-signature-cleaner">{t("an.skillClean")}</option>
            </select>
            <Button kind="primary" onClick={() => void generate(false)} loading={busy} disabled={selection.size === 0}>
              <Sparkles size={ICON_SM} /> {t("an.propose")} ({selection.size})
            </Button>
            {scan.squash_suggestions.length > 0 && (
              <Button
                onClick={() => {
                  setSkill("commit-synthesis");
                  void generate(false, scan.squash_suggestions);
                }}
                title={t("an.suggestTitle")}
              >
                {t("an.suggestMerges")} ({scan.squash_suggestions.length})
              </Button>
            )}
          </>
        }
      >
        {view === "graph" ? (
          <GitGraph graph={scan.graph} commits={scan.commits} onSelect={showDiff} />
        ) : (
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-slate-800">
              <th className={thCls + " w-8"}></th>
              <th className={thCls}>SHA</th>
              <th className={thCls}>{t("an.col.subject")}</th>
              <th className={thCls}>{t("an.col.author")}</th>
              <th className={thCls}>{t("an.col.diff")}</th>
              <th className={thCls}>{t("an.col.signals")}</th>
              <th className={thCls + " w-24"}></th>
            </tr>
          </thead>
          <tbody className="divide-y divide-slate-800/70">
            {displayCommits.map((c, idx) => (
              <tr
                key={c.sha}
                draggable
                onDragStart={() => {
                  dragSha.current = c.sha;
                }}
                onDragOver={(e) => e.preventDefault()}
                onDrop={() => dropOn(c.sha)}
                className={`${trCls} ${selection.has(c.sha) ? "bg-teal-950/30" : ""} cursor-grab`}
              >
                <td className="py-1.5">
                  <input
                    type="checkbox"
                    aria-label={`Sélectionner le commit ${c.short} — ${c.subject}`}
                    checked={selection.has(c.sha)}
                    onChange={() => toggle(c.sha)}
                  />
                </td>
                <td className="py-1.5 pr-3">
                  <button
                    type="button"
                    className={shaCls + " underline decoration-slate-700 underline-offset-2 hover:text-teal-300"}
                    title="Voir le diff de ce commit"
                    onClick={() => void showDiff(c)}
                  >
                    {c.short}
                  </button>
                </td>
                <td className="py-1.5 pr-3">
                  <div className={`${drops.has(c.sha) ? "line-through opacity-50" : ""} truncate text-slate-100`} title={c.subject}>
                    {c.subject}
                  </div>
                </td>
                <td className="whitespace-nowrap py-1.5 pr-3 text-xs text-slate-500">
                  {c.author_name} · {c.date.slice(0, 10)}
                </td>
                <td className="whitespace-nowrap py-1.5 pr-3 text-xs">
                  <span className="text-teal-400">+{c.insertions}</span>{" "}
                  <span className="text-rose-400">−{c.deletions}</span>
                </td>
                <td className="py-1.5">
                  <div className="flex flex-wrap gap-1">
                    {c.on_remote && <Badge tone="rose">{t("an.shared")}</Badge>}
                    {c.signed && <Badge tone="violet">{t("an.signed")}</Badge>}
                    {flagsFor(c.sha).map((f, i) => {
                      const [label, tone] = flagLabels[f.kind] ?? [f.kind, "slate"];
                      return (
                        <Badge key={i} tone={tone} title={f.detail}>
                          {label}
                        </Badge>
                      );
                    })}
                  </div>
                </td>
                <td className="py-1.5 text-right">
                  <span className="inline-flex items-center gap-0.5">
                    <button
                      type="button"
                      aria-label={`Monter ${c.short}`}
                      disabled={idx === 0}
                      onClick={() => moveInOrder(c.sha, -1)}
                      className="rounded px-1 text-slate-500 hover:text-slate-200 disabled:opacity-30"
                    >
                      ↑
                    </button>
                    <button
                      type="button"
                      aria-label={`Descendre ${c.short}`}
                      disabled={idx === displayCommits.length - 1}
                      onClick={() => moveInOrder(c.sha, 1)}
                      className="rounded px-1 text-slate-500 hover:text-slate-200 disabled:opacity-30"
                    >
                      ↓
                    </button>
                    <Button
                      kind="ghost"
                      onClick={() =>
                        setDrops((d) => {
                          const n = new Set(d);
                          if (n.has(c.sha)) n.delete(c.sha);
                          else n.add(c.sha);
                          return n;
                        })
                      }
                    >
                      {drops.has(c.sha) ? t("an.keep") : t("an.drop")}
                    </Button>
                  </span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        )}
      </Card>

      <div className="grid grid-cols-2 gap-4">
        <Card title={t("an.propTitle").replace("{n}", String(proposals.length))}>
          {Object.keys(streams).length > 0 && (
            <div className="mb-2.5 space-y-1.5">
              {Object.entries(streams).map(([g, text]) => (
                <div key={g} className="rounded border border-violet-800 bg-violet-950/30 p-2 text-xs">
                  <span className="mb-1 flex items-center gap-1.5 text-violet-300">
                    <Sparkles size={12} className="animate-pulse" /> flux du fournisseur — groupe{" "}
                    {Number(g) + 1} (brut, en cours)
                  </span>
                  <pre className="max-h-24 overflow-y-auto whitespace-pre-wrap font-mono text-slate-300">{text}</pre>
                </div>
              ))}
            </div>
          )}
          {proposals.length === 0 ? (
            <Empty
              actionLabel={selection.size > 0 ? `Proposer (${selection.size})` : undefined}
              onAction={selection.size > 0 ? () => void generate(false) : undefined}
            >
              Sélectionner des commits puis « Proposer ». Sans fournisseur LLM configuré,
              l'assistant local déterministe est utilisé (100&nbsp;% hors-ligne).
            </Empty>
          ) : (
            <div className="max-h-[420px] space-y-2.5 overflow-y-auto pr-1">
              {proposals.map((p) => (
                <div key={p.id} className="rounded border border-slate-800 bg-slate-950/60 p-2.5">
                  <div className="flex items-center gap-2 text-xs">
                    <Badge tone="violet">{p.skill}</Badge>
                    <Badge tone={riskTone(p.risk)}>risque {p.risk}</Badge>
                    <Badge
                      tone={
                        p.status === "proposed" ? "sky"
                        : p.status === "accepted" || p.status === "edited" ? "teal"
                        : p.status === "refused" ? "rose"
                        : "slate"
                      }
                    >
                      {p.status === "proposed" ? "à décider"
                        : p.status === "accepted" ? "acceptée"
                        : p.status === "edited" ? "éditée"
                        : p.status === "refused" ? "refus de la skill"
                        : "rejetée"}
                    </Badge>
                    <span className={"ml-auto " + shaCls}>{p.targets.map((t) => t.slice(0, 8)).join(", ")}</span>
                  </div>
                  <div className="mt-2 grid grid-cols-2 gap-2 text-xs">
                    <div>
                      <div className="mb-0.5 text-slate-500">Avant</div>
                      <pre className="whitespace-pre-wrap rounded bg-slate-900 p-2 text-slate-400">{p.before}</pre>
                    </div>
                    <div>
                      <div className="mb-0.5 text-slate-500">{p.after ? "Après (proposé)" : "Refus motivé"}</div>
                      {p.status === "proposed" ? (
                        <textarea
                          className={inputCls + " h-full min-h-20 font-mono"}
                          aria-label="Message proposé (éditable)"
                          value={editing[p.id] ?? p.after ?? ""}
                          onChange={(e) => setEditing((m) => ({ ...m, [p.id]: e.target.value }))}
                        />
                      ) : (
                        <pre
                          className={`whitespace-pre-wrap rounded p-2 ${p.after ? "bg-teal-950/40 text-teal-200" : "bg-rose-950/40 text-rose-200"}`}
                        >
                          {p.decision ?? p.after ?? p.explanation}
                        </pre>
                      )}
                    </div>
                  </div>
                  <p className="mt-1.5 text-xs italic text-slate-500">{p.explanation}</p>
                  {p.status === "proposed" && (
                    <div className="mt-2 flex gap-2">
                      <Button
                        kind="primary"
                        onClick={() => decide(p, editing[p.id] && editing[p.id] !== p.after ? "edit" : "accept")}
                      >
                        <CheckCircle2 size={ICON_SM} />{" "}
                        {editing[p.id] && editing[p.id] !== p.after ? "Valider l'édition" : "Accepter"}
                      </Button>
                      <Button onClick={() => decide(p, "reject")}>
                        <XCircle size={ICON_SM} /> Rejeter
                      </Button>
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </Card>

        <Card
          title={<span className="flex items-center gap-2">{t("an.planTitle")} {statusBadge}</span>}
          actions={
            <>
              <Button onClick={buildPlan} loading={busy}>{t("an.composeFrom")}</Button>
              {plan && plan.status === "draft" && (
                <Button kind="primary" onClick={dryRun} loading={busy}>
                  <FlaskConical size={ICON_SM} /> {t("an.dryRun")}
                </Button>
              )}
              {plan?.status === "dry_run_ok" && (
                <Button kind="danger" onClick={() => void apply()} loading={busy}>
                  <Play size={ICON_SM} /> {t("an.apply")}
                </Button>
              )}
              {plan?.status === "applied" && (
                <Button onClick={rollback} loading={busy}>
                  <Undo2 size={ICON_SM} /> {t("an.rollback")}
                </Button>
              )}
              {plan?.status === "applied" && (
                <Button
                  kind="primary"
                  onClick={openPush}
                  loading={busy}
                  title={t("an.push")}
                >
                  <UploadCloud size={ICON_SM} /> {t("an.push")}
                </Button>
              )}
              {plan && (
                <Button kind="ghost" onClick={exportPlan} title="Exporter le plan reproductible (JSON)">
                  <Download size={ICON_SM} />
                </Button>
              )}
              {plan && (
                <Button kind="ghost" onClick={exportHtmlReport} title="Exporter le rapport HTML (revue d'équipe)">
                  <FileText size={ICON_SM} />
                </Button>
              )}
              <Button kind="ghost" onClick={() => importRef.current?.click()} title="Importer un plan (JSON)">
                <Upload size={ICON_SM} />
              </Button>
              <input
                ref={importRef}
                type="file"
                accept="application/json"
                className="hidden"
                aria-label="Importer un plan JSON"
                onChange={(e) => {
                  const f = e.target.files?.[0];
                  if (f) void importPlan(f);
                  e.target.value = "";
                }}
              />
            </>
          }
        >
          {!plan ? (
            <Empty>{t("an.planEmpty")}</Empty>
          ) : (
            <div className="space-y-3 text-sm">
              <ol className="space-y-1">
                {plan.ops.map((o) => (
                  <li key={o.seq} className="flex items-center gap-2 rounded border border-slate-800 bg-slate-950/50 px-2.5 py-1.5">
                    <Badge tone={o.op === "drop" ? "rose" : o.op === "squash" ? "amber" : "teal"}>{o.op}</Badge>
                    <span className={shaCls}>
                      {"target" in o ? o.target.slice(0, 8) : "targets" in o ? o.targets.map((t) => t.slice(0, 8)).join("+") : ""}
                    </span>
                    <span className="truncate text-slate-300">
                      {"new_message" in o ? o.new_message.split("\n")[0] : "reason" in o ? o.reason : ""}
                    </span>
                    <Badge tone={riskTone(o.risk)}>{o.risk}</Badge>
                  </li>
                ))}
              </ol>

              {risks.length > 0 && (
                <div>
                  <h3 className="mb-1 flex items-center justify-between text-xs font-semibold uppercase tracking-wide text-slate-400">
                    <span className="flex items-center gap-1.5">
                      <ShieldCheck size={ICON_SM} /> Panneau risques
                    </span>
                    <VerdictLegend />
                  </h3>
                  <ul className="space-y-1">
                    {risks.map((r) => (
                      <li key={r.axe} className="flex items-start gap-2 text-xs">
                        <VerdictBadge verdict={r.verdict} label={`${r.axe} : ${r.verdict}`} />
                        <span className="text-slate-400">{r.motif}</span>
                      </li>
                    ))}
                  </ul>
                </div>
              )}

              {plan.status !== "draft" && plan.mapping.length > 0 && (
                <div>
                  <h3 className="mb-1 text-xs font-semibold uppercase tracking-wide text-slate-400">
                    Avant / après (résultat réel du dry-run — réf {plan.preview_ref})
                  </h3>
                  <table className="w-full text-xs">
                    <tbody className="divide-y divide-slate-800/60">
                      {plan.mapping.map((m, i) => (
                        <tr key={i} className={trCls}>
                          <td className={"py-1 pr-2 " + shaCls}>{m.old.map((o) => o.slice(0, 8)).join("+")}</td>
                          <td className="py-1 pr-2 text-slate-600">→</td>
                          <td className="py-1 pr-2 font-mono text-teal-400">{m.new.slice(0, 8)}</td>
                          <td className="py-1 text-slate-400">
                            {m.old.map((o) => commitsBySha.get(o)?.subject).filter(Boolean).join(" + ")}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}

              {plan.status === "applied" && (
                <p className="flex items-center gap-1.5 text-xs text-teal-300">
                  <RotateCcw size={ICON_SM} /> Backup&nbsp;: <code>{plan.backup_ref}</code> + tag{" "}
                  <code>{plan.backup_tag}</code> — rollback en un clic tant que la branche n'avance pas.
                </p>
              )}
            </div>
          )}
        </Card>
      </div>

      {consent && (
        <Modal
          title="Consentement — envoi à un fournisseur IA distant"
          tone="sky"
          width={640}
          onClose={() => setConsent(null)}
          footer={
            <>
              <Button onClick={() => setConsent(null)}>Refuser</Button>
              <Button kind="primary" loading={busy} onClick={() => void generate(true, consent.groups)}>
                J'autorise cet envoi
              </Button>
            </>
          }
        >
          <p className="text-xs text-slate-400">
            Aperçu EXACT des données qui seraient transmises (messages de commit, statistiques de diff — jamais de
            secrets)&nbsp;:
          </p>
          <pre className="mt-2 max-h-64 overflow-y-auto whitespace-pre-wrap rounded bg-slate-950 p-3 text-xs text-slate-300">
            {consent.preview}
          </pre>
        </Modal>
      )}

      {diffView && (
        <Modal
          title={`Diff — ${diffView.sha} · ${diffView.subject}`}
          width={760}
          onClose={() => setDiffView(null)}
          footer={<Button onClick={() => setDiffView(null)} autoFocus>Fermer</Button>}
        >
          <pre className="max-h-[60vh] overflow-auto rounded bg-slate-950 p-3 text-xs leading-relaxed">
            {diffView.patch.split("\n").map((line, i) => (
              <div
                key={i}
                className={
                  line.startsWith("+") && !line.startsWith("+++") ? "text-teal-300"
                  : line.startsWith("-") && !line.startsWith("---") ? "text-rose-300"
                  : line.startsWith("@@") ? "text-sky-400"
                  : "text-slate-400"
                }
              >
                {line || " "}
              </div>
            ))}
          </pre>
        </Modal>
      )}

      {pushPreview && (
        <Modal
          title="Pousser vers le remote"
          tone={pushPreview.needs_force ? "rose" : "sky"}
          width={620}
          onClose={() => setPushPreview(null)}
          footer={
            <>
              <Button onClick={() => setPushPreview(null)}>Annuler</Button>
              <Button
                kind={pushPreview.needs_force ? "danger" : "primary"}
                loading={pushing}
                disabled={
                  !pushPreview.can_push ||
                  (pushPreview.needs_force && pushPreview.protected) ||
                  (pushPreview.needs_force && pushTyped !== branch)
                }
                onClick={doPush}
              >
                {pushPreview.needs_force ? "Réécrire le remote (force-with-lease)" : "Pousser"}
              </Button>
            </>
          }
        >
          <div className="space-y-2.5 text-sm">
            <div className="flex flex-wrap items-center gap-2 text-xs">
              <Badge tone="slate">{pushPreview.remote ?? "aucun remote"}</Badge>
              <span className="truncate text-slate-400">{pushPreview.remote_url}</span>
            </div>
            <div className="flex flex-wrap gap-2 text-xs">
              <Badge tone="teal">{pushPreview.ahead} commit(s) en avance</Badge>
              <Badge tone={pushPreview.behind > 0 ? "amber" : "slate"}>
                {pushPreview.behind} en retard
              </Badge>
              {pushPreview.needs_force ? (
                <Badge tone="rose">push forcé requis (historique distant réécrit)</Badge>
              ) : (
                <Badge tone="teal">fast-forward</Badge>
              )}
              {pushPreview.protected && <Badge tone="rose">branche protégée</Badge>}
            </div>
            {pushPreview.warnings.length > 0 && (
              <ul className="list-inside list-disc space-y-1 rounded border border-amber-900 bg-amber-950/30 p-2 text-xs text-amber-200">
                {pushPreview.warnings.map((w, i) => (
                  <li key={i}>{w}</li>
                ))}
              </ul>
            )}
            {pushPreview.open_prs && pushPreview.open_prs.length > 0 && (
              <div className="text-xs">
                <div className="mb-1 font-semibold text-slate-300">
                  PR ouvertes sur cette branche (seront mises à jour) :
                </div>
                <ul className="space-y-0.5">
                  {pushPreview.open_prs.map((pr) => (
                    <li key={pr.number} className="flex items-center gap-1.5">
                      <Badge tone="sky">#{pr.number}</Badge>
                      <span className="truncate text-slate-300">{pr.title}</span>
                    </li>
                  ))}
                </ul>
              </div>
            )}
            <div className="rounded border border-slate-800 bg-slate-950/50 p-2 text-xs text-slate-400">
              Checklist coordination : prévenir l'équipe&nbsp;; s'assurer qu'aucun collègue n'a de
              travail non poussé sur cette branche&nbsp;; le <code>--force-with-lease</code> échoue
              sans rien écraser si le remote a bougé depuis le dernier fetch.
            </div>
            {pushPreview.protected && pushPreview.needs_force ? (
              <ErrorBox error="Branche protégée : le push forcé est refusé par le cœur. Retirer la protection ou pousser une nouvelle branche." />
            ) : pushPreview.needs_force ? (
              <>
                <p className="text-xs text-slate-400">
                  Pour confirmer la réécriture distante, saisir exactement&nbsp;:{" "}
                  <code className="text-rose-300">{branch}</code>
                </p>
                <input
                  autoFocus
                  className={inputCls}
                  value={pushTyped}
                  onChange={(e) => setPushTyped(e.target.value)}
                  placeholder={branch}
                  aria-label={`Saisir ${branch} pour confirmer le push forcé`}
                />
              </>
            ) : null}
          </div>
        </Modal>
      )}

      {confirmReq && plan && (
        <ConfirmTyped
          title="Application sur branche partagée"
          description={
            <>
              {confirmReq.message} L'historique déjà poussé sera réécrit&nbsp;; un backup (branche + tag) est créé
              avant toute écriture et le push restera à coordonner avec l'équipe.
            </>
          }
          expected={confirmReq.expected}
          confirmLabel="Réécrire la branche"
          busy={busy}
          onConfirm={(typed) => void apply(typed)}
          onClose={() => setConfirmReq(null)}
        />
      )}
    </div>
  );
}
