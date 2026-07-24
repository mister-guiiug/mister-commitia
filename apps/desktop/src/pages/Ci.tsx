import { useEffect, useRef, useState } from "react";
import { ArrowUpDown, PlugZap, ShieldAlert, Trash2 } from "lucide-react";
import { asIpcError, call } from "../ipc";
import type { CiAccount, CiKind, CiRun, RetentionPolicy, SimulationReport } from "../types";
import {
  Badge, Button, Card, ConfirmTyped, Empty, ErrorBox, Field, ICON_SM, VerdictBadge,
  inputCls, trCls, useToast,
} from "../ui";

export default function CiPage() {
  const toast = useToast();
  const [accounts, setAccounts] = useState<CiAccount[]>([]);
  const [policies, setPolicies] = useState<RetentionPolicy[]>([]);
  const [account, setAccount] = useState<string>("");
  const [policy, setPolicy] = useState<string>("");
  const [runs, setRuns] = useState<CiRun[]>([]);
  const [report, setReport] = useState<SimulationReport | null>(null);
  const [sortAsc, setSortAsc] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const formRef = useRef<HTMLDivElement>(null);

  // Formulaire d'ajout de compte
  const [kind, setKind] = useState<CiKind>("github");
  const [baseUrl, setBaseUrl] = useState("https://api.github.com");
  const [org, setOrg] = useState("");
  const [project, setProject] = useState("");
  const [repo, setRepo] = useState("");
  const [token, setToken] = useState("");
  const [scopes, setScopes] = useState<[string, string][]>([]);

  // Formulaire politique
  const [polName, setPolName] = useState("Rétention 180 jours");
  const [maxAge, setMaxAge] = useState(180);
  const [keepLast, setKeepLast] = useState(10);
  const [protectBranches, setProtectBranches] = useState("main");

  // Suppression confirmée (composant unifié)
  const [deleting, setDeleting] = useState<CiRun | null>(null);

  const refresh = async () => {
    const [a, p] = await Promise.all([
      call<CiAccount[]>("ci_account_list"),
      call<RetentionPolicy[]>("policy_list"),
    ]);
    setAccounts(a);
    setPolicies(p);
    if (!account && a.length > 0) setAccount(a[0].id);
    if (!policy && p.length > 0) setPolicy(p[0].id);
  };

  useEffect(() => {
    void refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    call<[string, string][]>("required_scopes", { kind }).then(setScopes).catch(() => setScopes([]));
    setBaseUrl(
      kind === "github" ? "https://api.github.com"
      : kind === "github_enterprise" ? "https://ghe.example.com/api/v3"
      : "https://dev.azure.com/mon-org",
    );
  }, [kind]);

  const addAccount = async () => {
    setError(null);
    setBusy(true);
    try {
      const [acct, msg] = await call<[CiAccount, string]>("ci_account_add", {
        kind, baseUrl, org: org || null, project: project || null, repo: repo || null,
        token, scopes: scopes.map(([f, s]) => `${f} → ${s}`),
      });
      setToken("");
      setAccount(acct.id);
      await refresh();
      toast("success", `Accès validé — ${msg}. Token envoyé au coffre du système.`);
    } catch (e) {
      setError(asIpcError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const inventory = async () => {
    setError(null);
    setBusy(true);
    try {
      const list = await call<CiRun[]>("ci_inventory", { accountId: account, max: 500 });
      setRuns(list);
      toast("info", `${list.length} runs inventoriés`);
    } catch (e) {
      setError(asIpcError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const savePolicy = async () => {
    setError(null);
    try {
      const p = await call<RetentionPolicy>("policy_save", {
        name: polName,
        rules: {
          max_age_days: maxAge || null,
          keep_last_per_pipeline: keepLast,
          protect_branches: protectBranches.split(",").map((s) => s.trim()).filter(Boolean),
          protect_failed: false,
        },
      });
      setPolicy(p.id);
      await refresh();
      toast("success", `Politique « ${p.name} » enregistrée`);
    } catch (e) {
      setError(asIpcError(e).message);
    }
  };

  const simulate = async () => {
    setError(null);
    setBusy(true);
    setReport(null);
    try {
      const r = await call<SimulationReport>("ci_simulate", { accountId: account, policyId: policy, max: 500 });
      setReport(r);
      toast("success", `Simulation terminée — ${r.candidates.length} candidat(s), ${r.protected.length} protégé(s), aucune suppression émise`);
    } catch (e) {
      setError(asIpcError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const doDelete = async (run: CiRun, confirm: string) => {
    setError(null);
    setBusy(true);
    try {
      await call("ci_delete_run", { accountId: account, policyId: policy, run, confirm });
      setReport((r) => r && { ...r, candidates: r.candidates.filter((c) => c.run_id !== run.run_id) });
      setDeleting(null);
      toast("success", `Run ${run.run_id} supprimé — action journalisée`);
    } catch (e) {
      setError(asIpcError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const sortedCandidates = report
    ? [...report.candidates].sort((a, b) =>
        sortAsc ? a.created_at.localeCompare(b.created_at) : b.created_at.localeCompare(a.created_at),
      )
    : [];

  return (
    <div className="mx-auto max-w-6xl space-y-4">
      <div className="grid grid-cols-2 gap-4" ref={formRef}>
        <Card title="Ajouter un accès plateforme">
          <div className="space-y-2.5">
            <Field label="Plateforme">
              <select className={inputCls} value={kind} onChange={(e) => setKind(e.target.value as CiKind)}>
                <option value="github">GitHub.com</option>
                <option value="github_enterprise">GitHub Enterprise Server</option>
                <option value="azure_devops">Azure DevOps Services</option>
                <option value="azure_devops_server">Azure DevOps Server</option>
              </select>
            </Field>
            <Field label="URL de base de l'API">
              <input className={inputCls} value={baseUrl} onChange={(e) => setBaseUrl(e.target.value)} />
            </Field>
            <div className="grid grid-cols-3 gap-2">
              <Field label="Organisation / owner">
                <input className={inputCls} value={org} onChange={(e) => setOrg(e.target.value)} />
              </Field>
              <Field label="Projet (AzDO)">
                <input className={inputCls} value={project} onChange={(e) => setProject(e.target.value)} />
              </Field>
              <Field label="Dépôt (GitHub)">
                <input className={inputCls} value={repo} onChange={(e) => setRepo(e.target.value)} />
              </Field>
            </div>
            <div className="rounded border border-sky-900 bg-sky-950/40 p-2.5 text-xs text-sky-200">
              <div className="mb-1 font-semibold">
                Droits requis (créer un token minimal) — affichés avant l'enregistrement&nbsp;:
              </div>
              <ul className="list-inside list-disc space-y-0.5">
                {scopes.map(([f, s]) => (
                  <li key={f}>
                    <span className="text-sky-300">{f}</span>&nbsp;: {s}
                  </li>
                ))}
              </ul>
            </div>
            <Field label="Token (stocké au coffre du système, jamais en clair)">
              <input
                type="password"
                className={inputCls}
                value={token}
                onChange={(e) => setToken(e.target.value)}
                placeholder="ghp_… / PAT"
              />
            </Field>
            <Button kind="primary" onClick={addAccount} loading={busy} disabled={!token}>
              <PlugZap size={ICON_SM} /> Valider &amp; enregistrer
            </Button>
          </div>
        </Card>

        <Card title="Politique de rétention">
          <div className="space-y-2.5">
            <Field label="Nom">
              <input className={inputCls} value={polName} onChange={(e) => setPolName(e.target.value)} />
            </Field>
            <div className="grid grid-cols-2 gap-2">
              <Field label="Âge max (jours)">
                <input type="number" className={inputCls} value={maxAge} onChange={(e) => setMaxAge(Number(e.target.value))} />
              </Field>
              <Field label="Conserver les N derniers / pipeline">
                <input type="number" className={inputCls} value={keepLast} onChange={(e) => setKeepLast(Number(e.target.value))} />
              </Field>
            </div>
            <Field label="Branches protégées (séparées par des virgules)">
              <input className={inputCls} value={protectBranches} onChange={(e) => setProtectBranches(e.target.value)} />
            </Field>
            <p className="text-xs text-slate-500">
              Toujours protégés, non désactivable&nbsp;: runs en cours, runs sous retention lease (Azure DevOps).
            </p>
            <Button onClick={savePolicy}>Enregistrer la politique</Button>
          </div>
        </Card>
      </div>

      <Card
        title="Inventaire &amp; simulation"
        actions={
          <>
            <select
              className={inputCls + " !w-auto"}
              aria-label="Compte plateforme"
              value={account}
              onChange={(e) => setAccount(e.target.value)}
            >
              {accounts.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.kind} · {a.org ?? a.project ?? a.base_url}
                </option>
              ))}
            </select>
            <select
              className={inputCls + " !w-auto"}
              aria-label="Politique de rétention"
              value={policy}
              onChange={(e) => setPolicy(e.target.value)}
            >
              <option value="">— politique —</option>
              {policies.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
            <Button onClick={inventory} loading={busy} disabled={!account}>
              Inventorier
            </Button>
            <Button kind="primary" onClick={simulate} loading={busy} disabled={!account || !policy}>
              Simuler (aucune suppression)
            </Button>
          </>
        }
      >
        <ErrorBox error={error} />
        {accounts.length === 0 && (
          <Empty
            actionLabel="Ajouter un accès"
            onAction={() => formRef.current?.scrollIntoView({ behavior: "smooth" })}
          >
            Aucun compte plateforme déclaré.
          </Empty>
        )}
        {accounts.length > 0 && runs.length > 0 && !report && (
          <p className="mb-2 text-xs text-slate-400">
            {runs.length} runs inventoriés ({runs.filter((r) => r.leased).length} retenus par rétention,{" "}
            {runs.filter((r) => r.running).length} en cours).
          </p>
        )}
        {report && (
          <div className="space-y-3">
            <div className="flex flex-wrap items-center gap-3 text-sm">
              <Badge tone="slate">{report.total} runs au total</Badge>
              <Badge tone="teal">{report.kept_recent} conservés (âge / N derniers)</Badge>
              <VerdictBadge verdict="attention" label={`${report.protected.length} protégés`} />
              <VerdictBadge verdict="bloquant" label={`${report.candidates.length} candidats à suppression`} />
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div>
                <h3 className="mb-1.5 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wide text-amber-400">
                  <ShieldAlert size={ICON_SM} /> Protégés (jamais supprimés)
                </h3>
                <ul className="space-y-1 text-sm">
                  {report.protected.map((p) => (
                    <li key={p.run.run_id} className={`rounded border border-slate-800 bg-slate-950/50 px-2.5 py-1.5 ${trCls}`}>
                      <span className="text-slate-200">
                        {p.run.pipeline_name} #{p.run.run_id}
                      </span>{" "}
                      <span className="text-xs text-amber-300">— {p.reason}</span>
                    </li>
                  ))}
                </ul>
              </div>
              <div>
                <h3 className="mb-1.5 flex items-center justify-between text-xs font-semibold uppercase tracking-wide text-rose-400">
                  <span>Candidats (suppression unitaire, confirmation renforcée)</span>
                  <button
                    type="button"
                    className="inline-flex items-center gap-1 text-slate-400 hover:text-slate-200"
                    onClick={() => setSortAsc((s) => !s)}
                    title="Trier par date"
                  >
                    <ArrowUpDown size={ICON_SM} /> date {sortAsc ? "↑" : "↓"}
                  </button>
                </h3>
                {sortedCandidates.length === 0 ? (
                  <Empty>Aucun candidat selon cette politique.</Empty>
                ) : (
                  <ul className="max-h-72 space-y-1 overflow-y-auto pr-1 text-sm">
                    {sortedCandidates.map((run) => (
                      <li
                        key={run.run_id}
                        className={`flex items-center gap-2 rounded border border-slate-800 bg-slate-950/50 px-2.5 py-1.5 ${trCls}`}
                      >
                        <span className="flex-1 truncate text-slate-200" title={`${run.pipeline_name} #${run.run_id}`}>
                          {run.pipeline_name} #{run.run_id}
                          <span className="ml-2 text-xs text-slate-500">
                            {run.branch} · {run.created_at.slice(0, 10)} · {run.result ?? run.status}
                          </span>
                        </span>
                        <Button kind="danger" onClick={() => setDeleting(run)} title="Supprimer ce run (confirmation renforcée)">
                          <Trash2 size={ICON_SM} />
                        </Button>
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            </div>
          </div>
        )}
        {accounts.length > 0 && !report && runs.length === 0 && (
          <Empty actionLabel={account ? "Inventorier" : undefined} onAction={account ? inventory : undefined}>
            Inventorier puis simuler&nbsp;: le rapport distingue candidats et protégés avec motifs.
          </Empty>
        )}
      </Card>

      {deleting && (
        <ConfirmTyped
          title="Suppression définitive d'un run"
          description={
            <>
              Run <b>#{deleting.run_id}</b> du pipeline <b>{deleting.pipeline_name}</b> ({deleting.branch} ·{" "}
              {deleting.created_at.slice(0, 10)}). L'action est journalisée avant l'appel à la plateforme&nbsp;;
              les runs sous lease ou en cours sont refusés par le cœur.
            </>
          }
          expected={deleting.pipeline_name}
          confirmLabel="Supprimer ce run"
          busy={busy}
          onConfirm={(typed) => void doDelete(deleting, typed)}
          onClose={() => setDeleting(null)}
        />
      )}
    </div>
  );
}
