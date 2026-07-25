import { useEffect, useRef, useState } from "react";
import { ArrowUpDown, Eraser, PlugZap, ShieldAlert, Trash2 } from "lucide-react";
import { asIpcError, call } from "../ipc";
import { t, useLang } from "../i18n";
import { useTask } from "../tasks";
import type { BatchDeleteResult, CiAccount, CiKind, CiRun, PurgeResult, RetentionPolicy, SimulationReport } from "../types";
import {
  Badge, Button, Card, ConfirmTyped, Empty, ErrorBox, Field, ICON_SM, ProgressPanel,
  VerdictBadge, inputCls, trCls, useToast,
} from "../ui";

export default function CiPage() {
  useLang();
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
  const task = useTask();

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
  // Suppression EN MASSE (F7) : confirmation + point de reprise.
  const [batchConfirm, setBatchConfirm] = useState(false);
  const [batchDone, setBatchDone] = useState<string[]>([]);
  // Purge des logs/artefacts (F7, extension) : reclaim de stockage, runs conservés.
  const [purge, setPurge] = useState(false);
  const [purgeLogs, setPurgeLogs] = useState(true);
  const [purgeArtifacts, setPurgeArtifacts] = useState(true);

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
    const taskId = task.begin("Inventaire des runs");
    try {
      const list = await call<CiRun[]>("ci_inventory", { accountId: account, max: 500, taskId });
      setRuns(list);
      toast("info", `${list.length} runs inventoriés`);
    } catch (e) {
      const ie = asIpcError(e);
      if (ie.code === "cancelled") toast("info", "Inventaire annulé");
      else setError(ie.message);
    } finally {
      task.end();
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
    const taskId = task.begin("Simulation de rétention");
    try {
      const r = await call<SimulationReport>("ci_simulate", { accountId: account, policyId: policy, max: 500, taskId });
      setReport(r);
      toast("success", `Simulation terminée — ${r.candidates.length} candidat(s), ${r.protected.length} protégé(s), aucune suppression émise`);
    } catch (e) {
      const ie = asIpcError(e);
      if (ie.code === "cancelled") toast("info", "Simulation annulée — aucun rapport produit, aucune suppression émise");
      else setError(ie.message);
    } finally {
      task.end();
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

  // F7 : reste à supprimer (hors point de reprise) — c'est le nombre à confirmer.
  const pending = report ? report.candidates.filter((c) => !batchDone.includes(c.run_id)) : [];

  const doBatch = async (confirm: string) => {
    if (!report) return;
    setError(null);
    setBusy(true);
    const taskId = task.begin(`Suppression de ${pending.length} run(s)`);
    try {
      const res = await call<BatchDeleteResult>("ci_delete_batch", {
        accountId: account, policyId: policy, runs: report.candidates,
        confirm, alreadyDone: batchDone, taskId,
      });
      // Retire les runs supprimés du rapport ; conserve un point de reprise.
      setReport((r) => r && { ...r, candidates: r.candidates.filter((c) => !res.deleted.includes(c.run_id)) });
      const remaining = res.cancelled || res.failed.length > 0;
      setBatchDone(remaining ? res.deleted : []);
      setBatchConfirm(false);
      toast(
        res.failed.length > 0 ? "error" : res.cancelled ? "info" : "success",
        res.cancelled
          ? `Interrompu : ${res.deleted.length} supprimé(s), reprise possible`
          : `${res.deleted.length} run(s) supprimé(s)` + (res.failed.length ? `, ${res.failed.length} échec(s)` : "") + " — journalisé",
      );
    } catch (e) {
      const ie = asIpcError(e);
      if (ie.code === "cancelled") toast("info", "Suppression en masse annulée");
      else setError(ie.message);
    } finally {
      task.end();
      setBusy(false);
    }
  };

  // F7 (extension) : runs éligibles à la purge (les runs en cours sont exclus).
  const purgeTargets = runs.filter((r) => !r.running);

  const doPurge = async (confirm: string) => {
    if (!purgeLogs && !purgeArtifacts) {
      setError("Rien à purger : activer les logs et/ou les artefacts.");
      return;
    }
    setError(null);
    setBusy(true);
    const taskId = task.begin(`Purge de ${purgeTargets.length} run(s)`);
    try {
      const res = await call<PurgeResult>("ci_purge_assets", {
        accountId: account, runs, purgeLogs, purgeArtifacts, confirm, taskId,
      });
      setPurge(false);
      const failed = res.failed.length;
      toast(
        failed > 0 ? "error" : res.cancelled ? "info" : "success",
        res.cancelled
          ? `Purge interrompue : ${res.artifacts_deleted} artefact(s), ${res.logs_deleted} log(s)`
          : `${res.artifacts_deleted} artefact(s) + ${res.logs_deleted} log(s) purgés sur ${res.runs} run(s)`
            + (failed ? `, ${failed} échec(s)` : "") + " — runs conservés, journalisé",
      );
    } catch (e) {
      const ie = asIpcError(e);
      if (ie.code === "cancelled") toast("info", "Purge annulée");
      else setError(ie.message);
    } finally {
      task.end();
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
        <Card title={t("ci.acct.title")}>
          <div className="space-y-2.5">
            <Field label={t("ci.platform")}>
              <select className={inputCls} value={kind} onChange={(e) => setKind(e.target.value as CiKind)}>
                <option value="github">{t("ci.gh")}</option>
                <option value="github_enterprise">{t("ci.ghe")}</option>
                <option value="azure_devops">{t("ci.azdo")}</option>
                <option value="azure_devops_server">{t("ci.azdoServer")}</option>
              </select>
            </Field>
            <Field label={t("ci.apiUrl")}>
              <input className={inputCls} value={baseUrl} onChange={(e) => setBaseUrl(e.target.value)} />
            </Field>
            <div className="grid grid-cols-3 gap-2">
              <Field label={t("ci.orgOwner")}>
                <input className={inputCls} value={org} onChange={(e) => setOrg(e.target.value)} />
              </Field>
              <Field label={t("ci.projectAzdo")}>
                <input className={inputCls} value={project} onChange={(e) => setProject(e.target.value)} />
              </Field>
              <Field label={t("ci.repoGithub")}>
                <input className={inputCls} value={repo} onChange={(e) => setRepo(e.target.value)} />
              </Field>
            </div>
            <div className="rounded border border-sky-900 bg-sky-950/40 p-2.5 text-xs text-sky-200">
              <div className="mb-1 font-semibold">{t("ci.scopesTitle")}</div>
              <ul className="list-inside list-disc space-y-0.5">
                {scopes.map(([f, s]) => (
                  <li key={f}>
                    <span className="text-sky-300">{f}</span>&nbsp;: {s}
                  </li>
                ))}
              </ul>
            </div>
            <Field label={t("ci.token")}>
              <input
                type="password"
                className={inputCls}
                value={token}
                onChange={(e) => setToken(e.target.value)}
                placeholder="ghp_… / PAT"
              />
            </Field>
            <Button kind="primary" onClick={addAccount} loading={busy} disabled={!token}>
              <PlugZap size={ICON_SM} /> {t("ci.validateSave")}
            </Button>
          </div>
        </Card>

        <Card title={t("ci.policy.title")}>
          <div className="space-y-2.5">
            <Field label={t("ci.name")}>
              <input className={inputCls} value={polName} onChange={(e) => setPolName(e.target.value)} />
            </Field>
            <div className="grid grid-cols-2 gap-2">
              <Field label={t("ci.maxAge")}>
                <input type="number" className={inputCls} value={maxAge} onChange={(e) => setMaxAge(Number(e.target.value))} />
              </Field>
              <Field label={t("ci.keepLast")}>
                <input type="number" className={inputCls} value={keepLast} onChange={(e) => setKeepLast(Number(e.target.value))} />
              </Field>
            </div>
            <Field label={t("ci.protectedBranches")}>
              <input className={inputCls} value={protectBranches} onChange={(e) => setProtectBranches(e.target.value)} />
            </Field>
            <p className="text-xs text-slate-500">{t("ci.alwaysProtected")}</p>
            <Button onClick={savePolicy}>{t("ci.savePolicy")}</Button>
          </div>
        </Card>
      </div>

      <Card
        title={t("ci.invSim.title")}
        actions={
          <>
            <select
              className={inputCls + " !w-auto"}
              aria-label={t("ci.acctAria")}
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
              aria-label={t("ci.policyAria")}
              value={policy}
              onChange={(e) => setPolicy(e.target.value)}
            >
              <option value="">{t("ci.policyPlaceholder")}</option>
              {policies.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
            <Button onClick={inventory} loading={busy} disabled={!account}>
              {t("ci.inventory")}
            </Button>
            <Button kind="primary" onClick={simulate} loading={busy} disabled={!account || !policy}>
              {t("ci.simulate")}
            </Button>
          </>
        }
      >
        <ErrorBox error={error} />
        {task.running && (
          <div className="mb-3">
            <ProgressPanel
              label={task.running.label}
              phase={task.running.phase}
              current={task.running.current}
              total={task.running.total}
              onCancel={task.cancel}
            />
          </div>
        )}
        {accounts.length === 0 && (
          <Empty
            actionLabel={t("ci.addAccess")}
            onAction={() => formRef.current?.scrollIntoView({ behavior: "smooth" })}
          >
            {t("ci.noAccount")}
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
              <Badge tone="slate">{report.total} {t("ci.total")}</Badge>
              <Badge tone="teal">{report.kept_recent} {t("ci.kept")}</Badge>
              <VerdictBadge verdict="attention" label={`${report.protected.length} ${t("ci.protectedN")}`} />
              <VerdictBadge verdict="bloquant" label={`${report.candidates.length} ${t("ci.candidatesN")}`} />
              {pending.length > 0 && (
                <Button kind="danger" onClick={() => setBatchConfirm(true)} title={t("ci.batchTitle")}>
                  <Trash2 size={ICON_SM} /> {batchDone.length > 0 ? `${t("ci.resume")} (${pending.length})` : `${t("ci.deleteAll")} (${pending.length})`}
                </Button>
              )}
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div>
                <h3 className="mb-1.5 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wide text-amber-400">
                  <ShieldAlert size={ICON_SM} /> {t("ci.protectedHead")}
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
                  <span>{t("ci.candidatesHead")}</span>
                  <button
                    type="button"
                    className="inline-flex items-center gap-1 text-slate-400 hover:text-slate-200"
                    onClick={() => setSortAsc((s) => !s)}
                    title={`${t("au.sortBy")} ${t("ci.date")}`}
                  >
                    <ArrowUpDown size={ICON_SM} /> {t("ci.date")} {sortAsc ? "↑" : "↓"}
                  </button>
                </h3>
                {sortedCandidates.length === 0 ? (
                  <Empty>{t("ci.noCandidates")}</Empty>
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
                        <Button kind="danger" onClick={() => setDeleting(run)} title={t("ci.deleteConfirm")}>
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
          <Empty actionLabel={account ? t("ci.inventory") : undefined} onAction={account ? inventory : undefined}>
            {t("ci.invSimEmpty")}
          </Empty>
        )}
        {runs.length > 0 && (
          <div className="mt-3 flex flex-wrap items-center gap-2 border-t border-slate-800 pt-3">
            <span className="flex-1 text-xs text-slate-400">
              {t("ci.reclaim").replace("{n}", String(purgeTargets.length))}
            </span>
            <Button
              onClick={() => setPurge(true)}
              disabled={purgeTargets.length === 0}
              title={t("ci.purgeTitle")}
            >
              <Eraser size={ICON_SM} /> {t("ci.purgeBtn")} ({purgeTargets.length})
            </Button>
          </div>
        )}
      </Card>

      {deleting && (
        <ConfirmTyped
          title={t("ci.deleteTitle")}
          description={
            <>
              Run <b>#{deleting.run_id}</b> du pipeline <b>{deleting.pipeline_name}</b> ({deleting.branch} ·{" "}
              {deleting.created_at.slice(0, 10)}). L'action est journalisée avant l'appel à la plateforme&nbsp;;
              les runs sous lease ou en cours sont refusés par le cœur.
            </>
          }
          expected={deleting.pipeline_name}
          confirmLabel={t("ci.deleteConfirm")}
          busy={busy}
          onConfirm={(typed) => void doDelete(deleting, typed)}
          onClose={() => setDeleting(null)}
        />
      )}

      {batchConfirm && report && (
        <ConfirmTyped
          title={t("ci.batchTitle")}
          description={
            <>
              <b>{pending.length}</b> run(s) seront supprimés un par un. Chaque suppression est
              journalisée&nbsp;; les runs en cours ou sous lease restent refusés par le cœur ; le
              débit est respecté (429/Retry-After) et l'opération est <b>annulable</b> puis
              reprenable. Saisir le <b>nombre</b> de runs pour confirmer.
            </>
          }
          expected={String(pending.length)}
          confirmLabel={`Supprimer ${pending.length} run(s)`}
          busy={busy}
          onConfirm={(typed) => void doBatch(typed)}
          onClose={() => setBatchConfirm(false)}
        />
      )}

      {purge && (
        <ConfirmTyped
          title={t("ci.purgeTitle")}
          description={
            <>
              <b>{purgeTargets.length}</b> run(s) verront leurs données de stockage purgées&nbsp;;
              <b> les runs eux-mêmes sont conservés</b> (contrairement à la suppression). Opération
              irréversible et journalisée&nbsp;; les runs en cours sont ignorés. Choisir ce qui est
              purgé, puis saisir le <b>nombre</b> de runs.
              <div className="mt-2 flex gap-4 text-xs">
                <label className="flex items-center gap-1.5">
                  <input
                    type="checkbox"
                    checked={purgeArtifacts}
                    onChange={(e) => setPurgeArtifacts(e.target.checked)}
                  />
                  {t("ci.artifacts")}
                </label>
                <label className="flex items-center gap-1.5">
                  <input
                    type="checkbox"
                    checked={purgeLogs}
                    onChange={(e) => setPurgeLogs(e.target.checked)}
                  />
                  {t("ci.logs")}
                </label>
              </div>
            </>
          }
          expected={String(purgeTargets.length)}
          confirmLabel={`Purger ${purgeTargets.length} run(s)`}
          busy={busy}
          onConfirm={(typed) => void doPurge(typed)}
          onClose={() => setPurge(false)}
        />
      )}
    </div>
  );
}
