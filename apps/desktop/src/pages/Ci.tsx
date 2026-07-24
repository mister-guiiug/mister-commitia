import { useEffect, useState } from "react";
import { PlugZap, ShieldAlert, Trash2 } from "lucide-react";
import { call } from "../ipc";
import type { CiAccount, CiKind, CiRun, RetentionPolicy, SimulationReport } from "../types";
import { Badge, Button, Card, Empty, ErrorBox, Field, inputCls } from "../ui";

export default function CiPage() {
  const [accounts, setAccounts] = useState<CiAccount[]>([]);
  const [policies, setPolicies] = useState<RetentionPolicy[]>([]);
  const [account, setAccount] = useState<string>("");
  const [policy, setPolicy] = useState<string>("");
  const [runs, setRuns] = useState<CiRun[]>([]);
  const [report, setReport] = useState<SimulationReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // Formulaire d'ajout de compte
  const [kind, setKind] = useState<CiKind>("github");
  const [baseUrl, setBaseUrl] = useState("https://api.github.com");
  const [org, setOrg] = useState("");
  const [project, setProject] = useState("");
  const [repo, setRepo] = useState("");
  const [token, setToken] = useState("");
  const [scopes, setScopes] = useState<[string, string][]>([]);
  const [validation, setValidation] = useState<string | null>(null);

  // Formulaire politique
  const [polName, setPolName] = useState("Rétention 180 jours");
  const [maxAge, setMaxAge] = useState(180);
  const [keepLast, setKeepLast] = useState(10);
  const [protectBranches, setProtectBranches] = useState("main");

  // Suppression confirmée
  const [deleting, setDeleting] = useState<CiRun | null>(null);
  const [confirmText, setConfirmText] = useState("");

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

  useEffect(() => { void refresh(); /* eslint-disable-line */ }, []);
  useEffect(() => {
    call<[string, string][]>("required_scopes", { kind }).then(setScopes).catch(() => setScopes([]));
    setBaseUrl(kind === "github" ? "https://api.github.com"
      : kind === "github_enterprise" ? "https://ghe.example.com/api/v3"
      : "https://dev.azure.com/mon-org");
  }, [kind]);

  const addAccount = async () => {
    setError(null); setValidation(null); setBusy(true);
    try {
      const [acct, msg] = await call<[CiAccount, string]>("ci_account_add", {
        kind, baseUrl, org: org || null, project: project || null, repo: repo || null,
        token, scopes: scopes.map(([f, s]) => `${f} → ${s}`),
      });
      setValidation(msg);
      setToken("");
      setAccount(acct.id);
      await refresh();
    } catch (e) { setError(String(e)); } finally { setBusy(false); }
  };

  const inventory = async () => {
    setError(null); setBusy(true);
    try { setRuns(await call<CiRun[]>("ci_inventory", { accountId: account, max: 500 })); }
    catch (e) { setError(String(e)); } finally { setBusy(false); }
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
    } catch (e) { setError(String(e)); }
  };

  const simulate = async () => {
    setError(null); setBusy(true); setReport(null);
    try { setReport(await call<SimulationReport>("ci_simulate", { accountId: account, policyId: policy, max: 500 })); }
    catch (e) { setError(String(e)); } finally { setBusy(false); }
  };

  const doDelete = async () => {
    if (!deleting) return;
    setError(null); setBusy(true);
    try {
      await call("ci_delete_run", { accountId: account, policyId: policy, run: deleting, confirm: confirmText });
      setReport((r) => r && { ...r, candidates: r.candidates.filter((c) => c.run_id !== deleting.run_id) });
      setDeleting(null); setConfirmText("");
    } catch (e) { setError(String(e)); } finally { setBusy(false); }
  };

  return (
    <div className="mx-auto max-w-6xl space-y-4">
      <div className="grid grid-cols-2 gap-4">
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
            <Field label="URL de base de l'API"><input className={inputCls} value={baseUrl} onChange={(e) => setBaseUrl(e.target.value)} /></Field>
            <div className="grid grid-cols-3 gap-2">
              <Field label="Organisation / owner"><input className={inputCls} value={org} onChange={(e) => setOrg(e.target.value)} /></Field>
              <Field label="Projet (AzDO)"><input className={inputCls} value={project} onChange={(e) => setProject(e.target.value)} /></Field>
              <Field label="Dépôt (GitHub)"><input className={inputCls} value={repo} onChange={(e) => setRepo(e.target.value)} /></Field>
            </div>
            <div className="rounded border border-sky-900 bg-sky-950/40 p-2.5 text-xs text-sky-200">
              <div className="mb-1 font-semibold">Droits requis (créer un token minimal) — affichés avant l'enregistrement :</div>
              <ul className="list-inside list-disc space-y-0.5">
                {scopes.map(([f, s]) => <li key={f}><span className="text-sky-300">{f}</span> : {s}</li>)}
              </ul>
            </div>
            <Field label="Token (stocké au coffre du système, jamais en clair)">
              <input type="password" className={inputCls} value={token} onChange={(e) => setToken(e.target.value)} placeholder="ghp_… / PAT" />
            </Field>
            <Button kind="primary" onClick={addAccount} disabled={busy || !token}>
              <span className="flex items-center gap-1.5"><PlugZap size={15} /> Valider & enregistrer</span>
            </Button>
            {validation && <p className="text-xs text-teal-400">{validation}</p>}
          </div>
        </Card>

        <Card title="Politique de rétention">
          <div className="space-y-2.5">
            <Field label="Nom"><input className={inputCls} value={polName} onChange={(e) => setPolName(e.target.value)} /></Field>
            <div className="grid grid-cols-2 gap-2">
              <Field label="Âge max (jours)"><input type="number" className={inputCls} value={maxAge} onChange={(e) => setMaxAge(Number(e.target.value))} /></Field>
              <Field label="Conserver les N derniers / pipeline"><input type="number" className={inputCls} value={keepLast} onChange={(e) => setKeepLast(Number(e.target.value))} /></Field>
            </div>
            <Field label="Branches protégées (séparées par des virgules)">
              <input className={inputCls} value={protectBranches} onChange={(e) => setProtectBranches(e.target.value)} />
            </Field>
            <p className="text-xs text-slate-500">
              Toujours protégés, non désactivable : runs en cours, runs sous retention lease (Azure DevOps).
            </p>
            <Button onClick={savePolicy}>Enregistrer la politique</Button>
          </div>
        </Card>
      </div>

      <Card
        title="Inventaire & simulation"
        actions={
          <>
            <select className={inputCls + " !w-auto"} value={account} onChange={(e) => setAccount(e.target.value)}>
              {accounts.map((a) => <option key={a.id} value={a.id}>{a.kind} · {a.org ?? a.project ?? a.base_url}</option>)}
            </select>
            <select className={inputCls + " !w-auto"} value={policy} onChange={(e) => setPolicy(e.target.value)}>
              <option value="">— politique —</option>
              {policies.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
            </select>
            <Button onClick={inventory} disabled={!account || busy}>Inventorier</Button>
            <Button kind="primary" onClick={simulate} disabled={!account || !policy || busy}>
              Simuler (aucune suppression)
            </Button>
          </>
        }
      >
        <ErrorBox error={error} />
        {runs.length > 0 && !report && (
          <p className="mb-2 text-xs text-slate-400">{runs.length} runs inventoriés ({runs.filter((r) => r.leased).length} retenus par rétention, {runs.filter((r) => r.running).length} en cours).</p>
        )}
        {report && (
          <div className="space-y-3">
            <div className="flex gap-3 text-sm">
              <Badge tone="slate">{report.total} runs au total</Badge>
              <Badge tone="teal">{report.kept_recent} conservés (règles d'âge / N derniers)</Badge>
              <Badge tone="amber">{report.protected.length} protégés</Badge>
              <Badge tone="rose">{report.candidates.length} candidats à suppression</Badge>
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div>
                <h3 className="mb-1.5 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wide text-amber-400">
                  <ShieldAlert size={14} /> Protégés (jamais supprimés)
                </h3>
                <ul className="space-y-1 text-sm">
                  {report.protected.map((p) => (
                    <li key={p.run.run_id} className="rounded border border-slate-800 bg-slate-950/50 px-2.5 py-1.5">
                      <span className="text-slate-200">{p.run.pipeline_name} #{p.run.run_id}</span>{" "}
                      <span className="text-xs text-amber-300">— {p.reason}</span>
                    </li>
                  ))}
                </ul>
              </div>
              <div>
                <h3 className="mb-1.5 text-xs font-semibold uppercase tracking-wide text-rose-400">
                  Candidats (suppression unitaire, double confirmation)
                </h3>
                {report.candidates.length === 0 ? (
                  <Empty>Aucun candidat selon cette politique.</Empty>
                ) : (
                  <ul className="max-h-72 space-y-1 overflow-y-auto pr-1 text-sm">
                    {report.candidates.map((run) => (
                      <li key={run.run_id} className="flex items-center gap-2 rounded border border-slate-800 bg-slate-950/50 px-2.5 py-1.5">
                        <span className="flex-1 text-slate-200">
                          {run.pipeline_name} #{run.run_id}
                          <span className="ml-2 text-xs text-slate-500">{run.branch} · {run.created_at.slice(0, 10)} · {run.result ?? run.status}</span>
                        </span>
                        <Button kind="danger" onClick={() => { setDeleting(run); setConfirmText(""); }}>
                          <Trash2 size={13} />
                        </Button>
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            </div>
          </div>
        )}
        {!report && runs.length === 0 && <Empty>Inventorier puis simuler : le rapport distingue candidats et protégés avec motifs.</Empty>}
      </Card>

      {deleting && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60" role="dialog">
          <div className="w-[480px] rounded-lg border border-rose-800 bg-slate-900 p-5">
            <h3 className="text-sm font-semibold text-rose-300">Suppression définitive d'un run</h3>
            <p className="mt-2 text-sm text-slate-300">
              Run <b>#{deleting.run_id}</b> du pipeline <b>{deleting.pipeline_name}</b>. Cette action est
              journalisée. Pour confirmer, saisir exactement le nom du pipeline :
            </p>
            <input
              className={inputCls + " mt-3"}
              value={confirmText}
              onChange={(e) => setConfirmText(e.target.value)}
              placeholder={deleting.pipeline_name}
              autoFocus
            />
            <div className="mt-4 flex justify-end gap-2">
              <Button onClick={() => setDeleting(null)}>Annuler</Button>
              <Button kind="danger" onClick={doDelete} disabled={busy || confirmText !== deleting.pipeline_name}>
                Supprimer ce run
              </Button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
