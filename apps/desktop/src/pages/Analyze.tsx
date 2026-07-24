import { useEffect, useMemo, useState } from "react";
import {
  CheckCircle2, Download, FlaskConical, GitBranch, Play, RotateCcw, ShieldCheck, Sparkles, Undo2, XCircle,
} from "lucide-react";
import { call } from "../ipc";
import type {
  BranchInfo, CommitInfo, Plan, PlanOp, Proposal, RepoRef, RiskAxis, ScanResult,
} from "../types";
import { Badge, Button, Card, Empty, ErrorBox, inputCls, riskTone, verdictTone } from "../ui";

const flagLabels: Record<string, [string, string]> = {
  weak_message: ["message faible", "amber"],
  non_conventional: ["non conforme", "violet"],
  ai_signature: ["mention d'outil", "sky"],
  oversized_no_body: ["gros diff sans corps", "amber"],
  duplicate_message: ["doublon", "amber"],
};

export default function AnalyzePage({ repo }: { repo: RepoRef }) {
  const [scan, setScan] = useState<ScanResult | null>(null);
  const [branches, setBranches] = useState<BranchInfo[]>([]);
  const [branch, setBranch] = useState<string>("");
  const [selection, setSelection] = useState<Set<string>>(new Set());
  const [skill, setSkill] = useState("conventional-commits");
  const [proposals, setProposals] = useState<Proposal[]>([]);
  const [editing, setEditing] = useState<Record<string, string>>({});
  const [plan, setPlan] = useState<Plan | null>(null);
  const [risks, setRisks] = useState<RiskAxis[]>([]);
  const [drops, setDrops] = useState<Set<string>>(new Set());
  const [confirmShared, setConfirmShared] = useState("");
  const [consent, setConsent] = useState<{ preview: string; groups: string[][] } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const doScan = async (b?: string) => {
    setError(null); setBusy(true);
    try {
      const res = await call<ScanResult>("repo_scan", { id: repo.id, branch: b ?? null });
      setScan(res);
      setBranch(res.branch);
      setSelection(new Set());
      setProposals(await call<Proposal[]>("proposals_list", { repoId: repo.id }));
    } catch (e) { setError(String(e)); } finally { setBusy(false); }
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

  const toggle = (sha: string) =>
    setSelection((s) => {
      const n = new Set(s);
      if (n.has(sha)) n.delete(sha); else n.add(sha);
      return n;
    });

  const groupsForSkill = (): string[][] => {
    const sel = scan!.commits.filter((c) => selection.has(c.sha)).map((c) => c.sha);
    if (skill === "commit-synthesis") return sel.length >= 2 ? [sel] : [];
    return sel.map((s) => [s]);
  };

  const generate = async (consentRemote: boolean, groups?: string[][]) => {
    setError(null); setBusy(true);
    try {
      const g = groups ?? groupsForSkill();
      if (g.length === 0) {
        setError(skill === "commit-synthesis"
          ? "Sélectionner au moins deux commits pour une synthèse."
          : "Sélectionner au moins un commit.");
        return;
      }
      await call<Proposal[]>("proposals_generate", {
        repoId: repo.id, skill, groups: g, providerId: null, consentRemote,
      });
      setProposals(await call<Proposal[]>("proposals_list", { repoId: repo.id }));
      setConsent(null);
    } catch (e) {
      const msg = String(e);
      if (msg.includes("consentement")) {
        const g = groups ?? groupsForSkill();
        const preview = await call<string>("ai_preview", { repoId: repo.id, skill, shas: g[0] });
        setConsent({ preview, groups: g });
      } else setError(msg);
    } finally { setBusy(false); }
  };

  // Toujours tenter SANS consentement : si le fournisseur par défaut est
  // distant, le cœur refuse (CA-9) et on affiche alors l'aperçu de consentement.
  const propose = () => generate(false);

  const decide = async (p: Proposal, decision: "accept" | "edit" | "reject") => {
    setError(null);
    try {
      await call<Proposal>("proposal_decide", {
        proposalId: p.id, decision,
        editedMessage: decision === "edit" ? editing[p.id] ?? p.after ?? "" : null,
      });
      setProposals(await call<Proposal[]>("proposals_list", { repoId: repo.id }));
    } catch (e) { setError(String(e)); }
  };

  const buildPlan = async () => {
    setError(null); setBusy(true);
    try {
      const p = await call<Plan>("plan_new", { repoId: repo.id, branch });
      let seq = 1;
      const ops: PlanOp[] = [];
      const inSegment = new Set(scan!.commits.map((c) => c.sha));
      for (const prop of proposals) {
        if ((prop.status !== "accepted" && prop.status !== "edited") || !prop.decision) continue;
        if (!prop.targets.every((t) => inSegment.has(t))) continue;
        if (prop.targets.length === 1) {
          ops.push({ op: "reword", target: prop.targets[0], new_message: prop.decision, seq: seq++, origin: `skill:${prop.skill}@${prop.skill_version}`, risk: prop.risk, approved_by: "utilisateur", approved_at: new Date().toISOString() });
        } else {
          ops.push({ op: "squash", targets: prop.targets, new_message: prop.decision, seq: seq++, origin: `skill:${prop.skill}@${prop.skill_version}`, risk: prop.risk, approved_by: "utilisateur", approved_at: new Date().toISOString() });
        }
      }
      for (const sha of drops) {
        ops.push({ op: "drop", target: sha, reason: "abandon manuel", seq: seq++, origin: "manuel", risk: "high", approved_by: "utilisateur", approved_at: new Date().toISOString() });
      }
      if (ops.length === 0) {
        setError("Aucune opération : accepter des propositions ou marquer des abandons d'abord.");
        return;
      }
      const withOps = await call<Plan>("plan_set_ops", { planId: p.id, ops });
      setPlan(withOps);
      setRisks(await call<RiskAxis[]>("plan_risk", { planId: withOps.id }));
    } catch (e) { setError(String(e)); } finally { setBusy(false); }
  };

  const dryRun = async () => {
    if (!plan) return;
    setError(null); setBusy(true);
    try {
      setPlan(await call<Plan>("plan_dry_run", { planId: plan.id }));
      setRisks(await call<RiskAxis[]>("plan_risk", { planId: plan.id }));
    } catch (e) { setError(String(e)); } finally { setBusy(false); }
  };

  const hasShared = scan?.commits.some((c) => c.on_remote && selectionOrPlanTargets().has(c.sha)) ?? false;
  function selectionOrPlanTargets(): Set<string> {
    const s = new Set<string>();
    plan?.ops.forEach((o) => {
      if ("target" in o) s.add(o.target);
      if ("targets" in o) o.targets.forEach((t) => s.add(t));
    });
    return s.size > 0 ? s : new Set(scan?.commits.map((c) => c.sha));
  }

  const apply = async () => {
    if (!plan) return;
    setError(null); setBusy(true);
    try {
      const updated = await call<Plan>("plan_apply", {
        planId: plan.id,
        confirm: hasShared ? confirmShared : null,
      });
      setPlan(updated);
      await doScan(branch);
    } catch (e) { setError(String(e)); } finally { setBusy(false); }
  };

  const rollback = async () => {
    if (!plan) return;
    setError(null); setBusy(true);
    try {
      setPlan(await call<Plan>("plan_rollback", { planId: plan.id }));
      await doScan(branch);
    } catch (e) { setError(String(e)); } finally { setBusy(false); }
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
  };

  if (!scan) return <p className="text-sm text-slate-500">{error ?? "Analyse en cours…"}</p>;

  const statusBadge = plan && (
    <Badge tone={plan.status === "applied" ? "teal" : plan.status === "dry_run_ok" ? "sky" : plan.status === "rolled_back" ? "amber" : "slate"}>
      {plan.status === "draft" ? "brouillon" : plan.status === "dry_run_ok" ? "dry-run OK" : plan.status === "applied" ? "appliqué" : plan.status === "rolled_back" ? "restauré" : plan.status}
    </Badge>
  );

  return (
    <div className="mx-auto max-w-6xl space-y-4">
      <div className="flex items-center gap-3">
        <GitBranch size={16} className="text-teal-400" />
        <span className="font-semibold">{repo.name}</span>
        <select
          className={inputCls + " !w-auto"}
          value={branch}
          onChange={(e) => { setPlan(null); setDrops(new Set()); void doScan(e.target.value); }}
        >
          {branches.map((b) => (
            <option key={b.name} value={b.name}>
              {b.name}{b.is_head ? " (courante)" : ""}{repo.protected_branches.includes(b.name) ? " 🔒" : ""}
            </option>
          ))}
          {!branches.some((b) => b.name === branch) && <option value={branch}>{branch}</option>}
        </select>
        <div className="ml-auto flex gap-2 text-xs">
          <Badge tone="slate">{scan.report.total} commits</Badge>
          <Badge tone="teal">{scan.report.conform} conformes</Badge>
          <Badge tone="amber">{scan.report.weak} faibles</Badge>
          <Badge tone="sky">{scan.report.ai_signatures} mentions d'outils</Badge>
        </div>
      </div>
      <ErrorBox error={error} />

      <Card
        title="Commits du segment réécrivable (du plus ancien au plus récent)"
        actions={
          <>
            <select className={inputCls + " !w-auto"} value={skill} onChange={(e) => setSkill(e.target.value)}>
              <option value="conventional-commits">Skill : Conventional Commits (reword)</option>
              <option value="commit-synthesis">Skill : Synthèse de groupe (squash)</option>
              <option value="ai-signature-cleaner">Skill : Nettoyage des mentions (gouverné)</option>
            </select>
            <Button kind="primary" onClick={propose} disabled={busy || selection.size === 0}>
              <span className="flex items-center gap-1.5"><Sparkles size={14} /> Proposer ({selection.size})</span>
            </Button>
            {scan.squash_suggestions.length > 0 && (
              <Button
                onClick={() => { setSkill("commit-synthesis"); void generate(false, scan.squash_suggestions); }}
                title="Groupes suggérés par l'heuristique locale"
              >
                Suggérer des fusions ({scan.squash_suggestions.length})
              </Button>
            )}
          </>
        }
      >
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-slate-800 text-left text-xs uppercase tracking-wide text-slate-500">
              <th className="w-8 py-2"></th>
              <th className="py-2 pr-3">SHA</th>
              <th className="py-2 pr-3">Sujet</th>
              <th className="py-2 pr-3">Auteur · date</th>
              <th className="py-2 pr-3">Diff</th>
              <th className="py-2">Signaux</th>
              <th className="w-24 py-2"></th>
            </tr>
          </thead>
          <tbody className="divide-y divide-slate-800/70">
            {scan.commits.map((c) => (
              <tr key={c.sha} className={selection.has(c.sha) ? "bg-teal-950/30" : undefined}>
                <td className="py-1.5">
                  <input type="checkbox" checked={selection.has(c.sha)} onChange={() => toggle(c.sha)} />
                </td>
                <td className="py-1.5 pr-3 font-mono text-xs text-slate-400">{c.short}</td>
                <td className="py-1.5 pr-3">
                  <div className={`${drops.has(c.sha) ? "line-through opacity-50" : ""} text-slate-100`}>{c.subject}</div>
                </td>
                <td className="py-1.5 pr-3 whitespace-nowrap text-xs text-slate-500">
                  {c.author_name} · {c.date.slice(0, 10)}
                </td>
                <td className="py-1.5 pr-3 whitespace-nowrap text-xs">
                  <span className="text-teal-400">+{c.insertions}</span>{" "}
                  <span className="text-rose-400">−{c.deletions}</span>
                </td>
                <td className="py-1.5">
                  <div className="flex flex-wrap gap-1">
                    {c.on_remote && <Badge tone="rose">partagé</Badge>}
                    {c.signed && <Badge tone="violet">signé</Badge>}
                    {flagsFor(c.sha).map((f, i) => {
                      const [label, tone] = flagLabels[f.kind] ?? [f.kind, "slate"];
                      return <Badge key={i} tone={tone}><span title={f.detail}>{label}</span></Badge>;
                    })}
                  </div>
                </td>
                <td className="py-1.5 text-right">
                  <Button kind="ghost" onClick={() => setDrops((d) => { const n = new Set(d); if (n.has(c.sha)) n.delete(c.sha); else n.add(c.sha); return n; })}>
                    {drops.has(c.sha) ? "garder" : "abandonner"}
                  </Button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </Card>

      <div className="grid grid-cols-2 gap-4">
        <Card title={`Propositions (${proposals.length}) — l'IA propose, vous disposez`}>
          {proposals.length === 0 ? (
            <Empty>Sélectionner des commits puis « Proposer ». Sans fournisseur LLM configuré, l'assistant local déterministe est utilisé.</Empty>
          ) : (
            <div className="max-h-[420px] space-y-2.5 overflow-y-auto pr-1">
              {proposals.map((p) => (
                <div key={p.id} className="rounded border border-slate-800 bg-slate-950/60 p-2.5">
                  <div className="flex items-center gap-2 text-xs">
                    <Badge tone="violet">{p.skill}</Badge>
                    <Badge tone={riskTone(p.risk)}>risque {p.risk}</Badge>
                    <Badge tone={p.status === "proposed" ? "sky" : p.status === "accepted" || p.status === "edited" ? "teal" : p.status === "refused" ? "rose" : "slate"}>
                      {p.status === "proposed" ? "à décider" : p.status === "accepted" ? "acceptée" : p.status === "edited" ? "éditée" : p.status === "refused" ? "refus de la skill" : "rejetée"}
                    </Badge>
                    <span className="ml-auto font-mono text-slate-500">{p.targets.map((t) => t.slice(0, 8)).join(", ")}</span>
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
                          value={editing[p.id] ?? p.after ?? ""}
                          onChange={(e) => setEditing((m) => ({ ...m, [p.id]: e.target.value }))}
                        />
                      ) : (
                        <pre className={`whitespace-pre-wrap rounded p-2 ${p.after ? "bg-teal-950/40 text-teal-200" : "bg-rose-950/40 text-rose-200"}`}>
                          {p.decision ?? p.after ?? p.explanation}
                        </pre>
                      )}
                    </div>
                  </div>
                  <p className="mt-1.5 text-xs italic text-slate-500">{p.explanation}</p>
                  {p.status === "proposed" && (
                    <div className="mt-2 flex gap-2">
                      <Button kind="primary" onClick={() => decide(p, editing[p.id] && editing[p.id] !== p.after ? "edit" : "accept")}>
                        <span className="flex items-center gap-1"><CheckCircle2 size={14} /> {editing[p.id] && editing[p.id] !== p.after ? "Valider l'édition" : "Accepter"}</span>
                      </Button>
                      <Button onClick={() => decide(p, "reject")}>
                        <span className="flex items-center gap-1"><XCircle size={14} /> Rejeter</span>
                      </Button>
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </Card>

        <Card
          title={<span className="flex items-center gap-2">Plan de réécriture {statusBadge}</span>}
          actions={
            <>
              <Button onClick={buildPlan} disabled={busy}>Composer depuis les décisions</Button>
              {plan && plan.status === "draft" && (
                <Button kind="primary" onClick={dryRun} disabled={busy}>
                  <span className="flex items-center gap-1"><FlaskConical size={14} /> Dry-run</span>
                </Button>
              )}
              {plan?.status === "dry_run_ok" && (
                <Button kind="danger" onClick={apply} disabled={busy || (hasShared && confirmShared !== branch)}>
                  <span className="flex items-center gap-1"><Play size={14} /> Appliquer</span>
                </Button>
              )}
              {plan?.status === "applied" && (
                <Button onClick={rollback} disabled={busy}>
                  <span className="flex items-center gap-1"><Undo2 size={14} /> Rollback</span>
                </Button>
              )}
              {plan && (
                <Button kind="ghost" onClick={exportPlan} title="Exporter le plan reproductible">
                  <Download size={14} />
                </Button>
              )}
            </>
          }
        >
          {!plan ? (
            <Empty>
              Accepter/éditer des propositions (et éventuellement marquer des abandons), puis composer le plan.
              Séquence imposée : plan → dry-run → backup automatique → application → rollback possible.
            </Empty>
          ) : (
            <div className="space-y-3 text-sm">
              <ol className="space-y-1">
                {plan.ops.map((o) => (
                  <li key={o.seq} className="flex items-center gap-2 rounded border border-slate-800 bg-slate-950/50 px-2.5 py-1.5">
                    <Badge tone={o.op === "drop" ? "rose" : o.op === "squash" ? "amber" : "teal"}>{o.op}</Badge>
                    <span className="font-mono text-xs text-slate-500">
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
                  <h3 className="mb-1 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wide text-slate-400">
                    <ShieldCheck size={14} /> Panneau risques
                  </h3>
                  <ul className="space-y-1">
                    {risks.map((r) => (
                      <li key={r.axe} className="flex items-start gap-2 text-xs">
                        <Badge tone={verdictTone(r.verdict)}>{r.axe} : {r.verdict}</Badge>
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
                        <tr key={i}>
                          <td className="py-1 pr-2 font-mono text-slate-500">{m.old.map((o) => o.slice(0, 8)).join("+")}</td>
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

              {hasShared && plan.status === "dry_run_ok" && (
                <div className="rounded border border-rose-800 bg-rose-950/40 p-2.5 text-xs text-rose-200">
                  Des commits du segment sont déjà poussés (branche partagée). Confirmation renforcée :
                  saisir exactement <b>{branch}</b> pour autoriser l'application.
                  <input className={inputCls + " mt-1.5"} value={confirmShared} onChange={(e) => setConfirmShared(e.target.value)} placeholder={branch} />
                </div>
              )}

              {plan.status === "applied" && (
                <p className="flex items-center gap-1.5 text-xs text-teal-300">
                  <RotateCcw size={13} /> Backup : <code>{plan.backup_ref}</code> + tag <code>{plan.backup_tag}</code> — rollback en un clic tant que la branche n'avance pas.
                </p>
              )}
            </div>
          )}
        </Card>
      </div>

      {consent && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60" role="dialog">
          <div className="w-[640px] rounded-lg border border-sky-800 bg-slate-900 p-5">
            <h3 className="text-sm font-semibold text-sky-300">Consentement : envoi à un fournisseur IA distant</h3>
            <p className="mt-1 text-xs text-slate-400">
              Aperçu EXACT des données qui seraient transmises (messages de commit, statistiques de diff — jamais de secrets) :
            </p>
            <pre className="mt-2 max-h-64 overflow-y-auto whitespace-pre-wrap rounded bg-slate-950 p-3 text-xs text-slate-300">
              {consent.preview}
            </pre>
            <div className="mt-4 flex justify-end gap-2">
              <Button onClick={() => setConsent(null)}>Refuser</Button>
              <Button kind="primary" onClick={() => generate(true, consent.groups)}>
                J'autorise cet envoi
              </Button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
