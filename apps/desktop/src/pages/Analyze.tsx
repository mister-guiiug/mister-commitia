import { useEffect, useMemo, useRef, useState } from "react";
import {
  CheckCircle2, Download, FlaskConical, GitBranch, Play, RotateCcw, ShieldCheck, Sparkles, Undo2, Upload, XCircle,
} from "lucide-react";
import { asIpcError, call } from "../ipc";
import type {
  BranchInfo, CommitInfo, Plan, PlanOp, Proposal, RepoRef, RiskAxis, ScanResult,
} from "../types";
import {
  Badge, Button, Card, ConfirmTyped, Empty, ErrorBox, ICON_SM, Modal, VerdictBadge, VerdictLegend,
  inputCls, riskTone, shaCls, thCls, trCls, useToast,
} from "../ui";

const flagLabels: Record<string, [string, string]> = {
  weak_message: ["message faible", "amber"],
  non_conventional: ["non conforme", "violet"],
  ai_signature: ["mention d'outil", "sky"],
  oversized_no_body: ["gros diff sans corps", "amber"],
  duplicate_message: ["doublon", "amber"],
};

export default function AnalyzePage({ repo }: { repo: RepoRef }) {
  const toast = useToast();
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
  const [consent, setConsent] = useState<{ preview: string; groups: string[][] } | null>(null);
  const [confirmReq, setConfirmReq] = useState<{ expected: string; message: string } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const importRef = useRef<HTMLInputElement>(null);

  const doScan = async (b?: string) => {
    setError(null);
    setBusy(true);
    try {
      const res = await call<ScanResult>("repo_scan", { id: repo.id, branch: b ?? null });
      setScan(res);
      setBranch(res.branch);
      setSelection(new Set());
      setProposals(await call<Proposal[]>("proposals_list", { repoId: repo.id }));
    } catch (e) {
      setError(asIpcError(e).message);
    } finally {
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
    setBusy(true);
    try {
      const g = groups ?? groupsForSkill();
      if (g.length === 0) {
        setError(
          skill === "commit-synthesis"
            ? "Sélectionner au moins deux commits pour une synthèse."
            : "Sélectionner au moins un commit.",
        );
        return;
      }
      const generated = await call<Proposal[]>("proposals_generate", {
        repoId: repo.id, skill, groups: g, providerId: null, consentRemote,
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
        const g = groups ?? groupsForSkill();
        const preview = await call<string>("ai_preview", { repoId: repo.id, skill, shas: g[0] });
        setConsent({ preview, groups: g });
      } else {
        setError(ie.message);
      }
    } finally {
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
    try {
      setPlan(await call<Plan>("plan_dry_run", { planId: plan.id }));
      setRisks(await call<RiskAxis[]>("plan_risk", { planId: plan.id }));
      toast("success", "Dry-run réussi — résultat réel construit dans la préview, branche intacte");
    } catch (e) {
      setError(asIpcError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const apply = async (confirm?: string) => {
    if (!plan) return;
    setError(null);
    setBusy(true);
    try {
      const updated = await call<Plan>("plan_apply", { planId: plan.id, confirm: confirm ?? null });
      setPlan(updated);
      setConfirmReq(null);
      toast("success", `Plan appliqué — backup ${updated.backup_ref ?? ""}`);
      await doScan(branch);
    } catch (e) {
      const ie = asIpcError(e);
      if (ie.code === "confirm_required") {
        setConfirmReq({ expected: ie.expected ?? branch, message: ie.message });
      } else {
        setError(ie.message);
      }
    } finally {
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

  if (!scan) return <p className="text-sm text-slate-500">{error ?? "Analyse en cours…"}</p>;

  const statusBadge = plan && (
    <Badge
      tone={
        plan.status === "applied" ? "teal"
        : plan.status === "dry_run_ok" ? "sky"
        : plan.status === "rolled_back" ? "amber"
        : "slate"
      }
    >
      {plan.status === "draft" ? "brouillon"
        : plan.status === "dry_run_ok" ? "dry-run OK"
        : plan.status === "applied" ? "appliqué"
        : plan.status === "rolled_back" ? "restauré"
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
          aria-label="Branche analysée"
          value={branch}
          onChange={(e) => {
            setPlan(null);
            setDrops(new Set());
            void doScan(e.target.value);
          }}
        >
          {branches.map((b) => (
            <option key={b.name} value={b.name}>
              {b.name}
              {b.is_head ? " (courante)" : ""}
              {repo.protected_branches.includes(b.name) ? " 🔒" : ""}
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
            <select
              className={inputCls + " !w-auto"}
              aria-label="Skill à utiliser"
              value={skill}
              onChange={(e) => setSkill(e.target.value)}
            >
              <option value="conventional-commits">Skill&nbsp;: Conventional Commits (reword)</option>
              <option value="commit-synthesis">Skill&nbsp;: Synthèse de groupe (squash)</option>
              <option value="ai-signature-cleaner">Skill&nbsp;: Nettoyage des mentions (gouverné)</option>
            </select>
            <Button kind="primary" onClick={() => void generate(false)} loading={busy} disabled={selection.size === 0}>
              <Sparkles size={ICON_SM} /> Proposer ({selection.size})
            </Button>
            {scan.squash_suggestions.length > 0 && (
              <Button
                onClick={() => {
                  setSkill("commit-synthesis");
                  void generate(false, scan.squash_suggestions);
                }}
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
            <tr className="border-b border-slate-800">
              <th className={thCls + " w-8"}></th>
              <th className={thCls}>SHA</th>
              <th className={thCls}>Sujet</th>
              <th className={thCls}>Auteur · date</th>
              <th className={thCls}>Diff</th>
              <th className={thCls}>Signaux</th>
              <th className={thCls + " w-24"}></th>
            </tr>
          </thead>
          <tbody className="divide-y divide-slate-800/70">
            {scan.commits.map((c) => (
              <tr key={c.sha} className={`${trCls} ${selection.has(c.sha) ? "bg-teal-950/30" : ""}`}>
                <td className="py-1.5">
                  <input
                    type="checkbox"
                    aria-label={`Sélectionner le commit ${c.short} — ${c.subject}`}
                    checked={selection.has(c.sha)}
                    onChange={() => toggle(c.sha)}
                  />
                </td>
                <td className={"py-1.5 pr-3 " + shaCls}>{c.short}</td>
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
                    {c.on_remote && <Badge tone="rose">partagé</Badge>}
                    {c.signed && <Badge tone="violet">signé</Badge>}
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
          title={<span className="flex items-center gap-2">Plan de réécriture {statusBadge}</span>}
          actions={
            <>
              <Button onClick={buildPlan} loading={busy}>Composer depuis les décisions</Button>
              {plan && plan.status === "draft" && (
                <Button kind="primary" onClick={dryRun} loading={busy}>
                  <FlaskConical size={ICON_SM} /> Dry-run
                </Button>
              )}
              {plan?.status === "dry_run_ok" && (
                <Button kind="danger" onClick={() => void apply()} loading={busy}>
                  <Play size={ICON_SM} /> Appliquer
                </Button>
              )}
              {plan?.status === "applied" && (
                <Button onClick={rollback} loading={busy}>
                  <Undo2 size={ICON_SM} /> Rollback
                </Button>
              )}
              {plan && (
                <Button kind="ghost" onClick={exportPlan} title="Exporter le plan reproductible">
                  <Download size={ICON_SM} />
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
            <Empty>
              Accepter/éditer des propositions (et éventuellement marquer des abandons), puis composer le plan.
              Séquence imposée&nbsp;: plan → dry-run → backup automatique → application → rollback possible.
            </Empty>
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
