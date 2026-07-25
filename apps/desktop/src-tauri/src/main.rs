// Empêche l'ouverture d'une console sous Windows en release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use mc_core::model::*;
use mc_core::task::{CancelToken, TaskCtx};
use mc_core::Core;
use tauri::{Emitter, Manager};

/// Le cœur est partagé en Arc pour permettre aux commandes lourdes de
/// s'exécuter hors des workers IPC (spawn_blocking) sans geler l'UI.
type ArcCore = Arc<Core>;

/// Registre des tâches annulables : task_id (généré par l'UI) → jeton.
#[derive(Default)]
struct Tasks(Mutex<HashMap<String, CancelToken>>);

/// Contexte d'une commande longue : progression et fragments IA relayés vers
/// l'UI sur le canal unique `mc://task`, jeton enregistré pour `task_cancel`.
/// Sans `task_id` (appelant historique), contexte inerte.
fn task_ctx(
    app: &tauri::AppHandle,
    tasks: &Tasks,
    task: &str,
    task_id: &Option<String>,
) -> TaskCtx {
    match task_id {
        Some(id) => {
            let cancel = CancelToken::new();
            tasks.0.lock().unwrap().insert(id.clone(), cancel.clone());
            let app = app.clone();
            TaskCtx::new(task, id, cancel, move |payload| {
                let _ = app.emit("mc://task", &payload);
            })
        }
        None => TaskCtx::noop(task),
    }
}

fn task_done(tasks: &Tasks, task_id: &Option<String>) {
    if let Some(id) = task_id {
        tasks.0.lock().unwrap().remove(id);
    }
}

/// Demande d'annulation coopérative : le cœur s'interrompt au prochain point
/// d'arrêt sûr (l'opération répond alors avec le code `cancelled`).
#[tauri::command]
fn task_cancel(tasks: tauri::State<'_, Tasks>, task_id: String) -> CmdResult<()> {
    if let Some(token) = tasks.0.lock().unwrap().get(&task_id) {
        token.cancel();
    }
    Ok(())
}

/// Contrat d'erreur UI ↔ cœur : `code` est stable (l'UI s'y branche),
/// `message` est le libellé humain, `expected` porte la valeur attendue
/// des confirmations renforcées.
#[derive(serde::Serialize)]
struct CmdError {
    code: &'static str,
    message: String,
    expected: Option<String>,
}

type CmdResult<T> = std::result::Result<T, CmdError>;

fn err(e: mc_core::CoreError) -> CmdError {
    CmdError {
        code: e.code(),
        expected: e.expected().map(String::from),
        message: e.to_string(),
    }
}

// ---------------------------------------------------------------------------
// E1/E2 — dépôts & analyse
// ---------------------------------------------------------------------------

#[tauri::command]
fn repos_list(core: tauri::State<'_, ArcCore>) -> CmdResult<Vec<RepoRef>> {
    core.repo_list().map_err(err)
}

#[tauri::command]
fn repo_declare(core: tauri::State<'_, ArcCore>, path: String) -> CmdResult<RepoRef> {
    core.repo_declare(&path).map_err(err)
}

#[tauri::command]
fn repo_remove(core: tauri::State<'_, ArcCore>, id: String) -> CmdResult<()> {
    core.repo_remove(&id).map_err(err)
}

#[tauri::command]
fn repo_branches(core: tauri::State<'_, ArcCore>, id: String) -> CmdResult<Vec<BranchInfo>> {
    core.repo_branches(&id).map_err(err)
}

fn join_err(e: tauri::Error) -> CmdError {
    CmdError {
        code: "internal",
        message: format!("tâche interrompue : {e}"),
        expected: None,
    }
}

#[tauri::command]
async fn repo_scan(
    app: tauri::AppHandle,
    tasks: tauri::State<'_, Tasks>,
    core: tauri::State<'_, ArcCore>,
    id: String,
    branch: Option<String>,
    base: Option<String>,
    task_id: Option<String>,
) -> CmdResult<mc_core::api::ScanResult> {
    let ctx = task_ctx(&app, &tasks, "repo_scan", &task_id);
    let core = core.inner().clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
        core.repo_scan_base(&id, branch, base, &ctx).map_err(err)
    })
    .await
    .map_err(join_err);
    task_done(&tasks, &task_id);
    res?
}

#[tauri::command]
fn commit_diff(core: tauri::State<'_, ArcCore>, repo_id: String, sha: String) -> CmdResult<String> {
    core.commit_diff(&repo_id, &sha).map_err(err)
}

#[tauri::command]
fn repo_set_governance(
    core: tauri::State<'_, ArcCore>,
    id: String,
    governance: Governance,
    protected_branches: Vec<String>,
) -> CmdResult<RepoRef> {
    core.repo_update_governance(&id, governance, protected_branches)
        .map_err(err)
}

// ---------------------------------------------------------------------------
// E3 — plans
// ---------------------------------------------------------------------------

#[tauri::command]
fn plan_new(core: tauri::State<'_, ArcCore>, repo_id: String, branch: String) -> CmdResult<Plan> {
    core.plan_new(&repo_id, &branch).map_err(err)
}

#[tauri::command]
fn plan_set_ops(
    core: tauri::State<'_, ArcCore>,
    plan_id: String,
    ops: Vec<PlanOp>,
) -> CmdResult<Plan> {
    core.plan_set_ops(&plan_id, ops).map_err(err)
}

#[tauri::command]
async fn plan_dry_run(
    app: tauri::AppHandle,
    tasks: tauri::State<'_, Tasks>,
    core: tauri::State<'_, ArcCore>,
    plan_id: String,
    task_id: Option<String>,
) -> CmdResult<Plan> {
    let ctx = task_ctx(&app, &tasks, "plan_dry_run", &task_id);
    let core = core.inner().clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
        core.plan_dry_run_with(&plan_id, &ctx).map_err(err)
    })
    .await
    .map_err(join_err);
    task_done(&tasks, &task_id);
    res?
}

#[tauri::command]
async fn plan_apply(
    app: tauri::AppHandle,
    tasks: tauri::State<'_, Tasks>,
    core: tauri::State<'_, ArcCore>,
    plan_id: String,
    confirm: Option<String>,
    task_id: Option<String>,
) -> CmdResult<Plan> {
    let ctx = task_ctx(&app, &tasks, "plan_apply", &task_id);
    let core = core.inner().clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
        core.plan_apply_with(&plan_id, confirm, &ctx).map_err(err)
    })
    .await
    .map_err(join_err);
    task_done(&tasks, &task_id);
    res?
}

/// C1 — enregistre le contenu résolu d'un fichier en conflit (puis `git add`).
#[tauri::command]
async fn plan_conflict_resolve(
    core: tauri::State<'_, ArcCore>,
    plan_id: String,
    file: String,
    content: String,
) -> CmdResult<()> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        core.plan_conflict_resolve(&plan_id, &file, &content)
            .map_err(err)
    })
    .await
    .map_err(join_err)?
}

/// C1 — reprend un dry-run mis en pause sur conflit (après résolution).
#[tauri::command]
async fn plan_conflict_continue(
    app: tauri::AppHandle,
    tasks: tauri::State<'_, Tasks>,
    core: tauri::State<'_, ArcCore>,
    plan_id: String,
    task_id: Option<String>,
) -> CmdResult<Plan> {
    let ctx = task_ctx(&app, &tasks, "plan_conflict_continue", &task_id);
    let core = core.inner().clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
        core.plan_conflict_continue_with(&plan_id, &ctx)
            .map_err(err)
    })
    .await
    .map_err(join_err);
    task_done(&tasks, &task_id);
    res?
}

/// C1 — abandonne la résolution de conflit (retour du plan en Draft).
#[tauri::command]
async fn plan_conflict_abort(core: tauri::State<'_, ArcCore>, plan_id: String) -> CmdResult<Plan> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || core.plan_conflict_abort(&plan_id).map_err(err))
        .await
        .map_err(join_err)?
}

#[tauri::command]
async fn plan_rollback(core: tauri::State<'_, ArcCore>, plan_id: String) -> CmdResult<Plan> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || core.plan_rollback(&plan_id).map_err(err))
        .await
        .map_err(join_err)?
}

#[tauri::command]
fn plan_list(core: tauri::State<'_, ArcCore>, repo_id: String) -> CmdResult<Vec<Plan>> {
    core.plan_list(&repo_id).map_err(err)
}

#[tauri::command]
fn plan_risk(
    core: tauri::State<'_, ArcCore>,
    plan_id: String,
) -> CmdResult<Vec<mc_core::plan::RiskAxis>> {
    core.plan_risk(&plan_id).map_err(err)
}

#[tauri::command]
fn plan_export(core: tauri::State<'_, ArcCore>, plan_id: String) -> CmdResult<String> {
    core.plan_export(&plan_id).map_err(err)
}

#[tauri::command]
fn plan_import(core: tauri::State<'_, ArcCore>, repo_id: String, json: String) -> CmdResult<Plan> {
    core.plan_import(&repo_id, &json).map_err(err)
}

// ---------------------------------------------------------------------------
// E3b — push assisté (F4)
// ---------------------------------------------------------------------------

#[tauri::command]
async fn push_preview(
    core: tauri::State<'_, ArcCore>,
    repo_id: String,
    branch: String,
    ci_account_id: Option<String>,
) -> CmdResult<mc_core::api::PushPreview> {
    core.push_preview(&repo_id, &branch, ci_account_id)
        .await
        .map_err(err)
}

#[tauri::command]
async fn push_execute(
    core: tauri::State<'_, ArcCore>,
    repo_id: String,
    branch: String,
    confirm: Option<String>,
) -> CmdResult<mc_core::api::PushResult> {
    // Réseau + git : hors des workers IPC.
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        core.push_execute(&repo_id, &branch, confirm).map_err(err)
    })
    .await
    .map_err(join_err)?
}

// ---------------------------------------------------------------------------
// E4 — skills & IA
// ---------------------------------------------------------------------------

#[tauri::command]
fn skills_list(
    core: tauri::State<'_, ArcCore>,
) -> CmdResult<(Vec<mc_core::api::SkillMeta>, Vec<(String, String)>)> {
    core.skills_list().map_err(err)
}

#[tauri::command]
fn skill_read(core: tauri::State<'_, ArcCore>, name: String) -> CmdResult<String> {
    core.skill_read(&name).map_err(err)
}

#[tauri::command]
fn skill_write(core: tauri::State<'_, ArcCore>, name: String, content: String) -> CmdResult<()> {
    core.skill_write(&name, &content).map_err(err)
}

#[tauri::command]
fn skill_run_tests(
    core: tauri::State<'_, ArcCore>,
    name: String,
) -> CmdResult<Vec<mc_core::api::SkillTestResult>> {
    core.skill_run_tests(&name).map_err(err)
}

#[tauri::command]
fn ai_preview(
    core: tauri::State<'_, ArcCore>,
    repo_id: String,
    skill: String,
    shas: Vec<String>,
) -> CmdResult<String> {
    core.ai_preview(&repo_id, &skill, shas).map_err(err)
}

#[tauri::command]
async fn proposals_generate(
    app: tauri::AppHandle,
    tasks: tauri::State<'_, Tasks>,
    core: tauri::State<'_, ArcCore>,
    repo_id: String,
    skill: String,
    groups: Vec<Vec<String>>,
    provider_id: Option<String>,
    consent_remote: bool,
    task_id: Option<String>,
) -> CmdResult<Vec<Proposal>> {
    let ctx = task_ctx(&app, &tasks, "proposals_generate", &task_id);
    let res = core
        .proposals_generate_with(&repo_id, &skill, groups, provider_id, consent_remote, &ctx)
        .await
        .map_err(err);
    task_done(&tasks, &task_id);
    res
}

#[tauri::command]
fn proposals_list(core: tauri::State<'_, ArcCore>, repo_id: String) -> CmdResult<Vec<Proposal>> {
    core.proposals_list(&repo_id).map_err(err)
}

#[tauri::command]
fn proposal_decide(
    core: tauri::State<'_, ArcCore>,
    proposal_id: String,
    decision: String,
    edited_message: Option<String>,
) -> CmdResult<Proposal> {
    core.proposal_decide(&proposal_id, &decision, edited_message)
        .map_err(err)
}

// ---------------------------------------------------------------------------
// E5 — fournisseurs IA & scopes
// ---------------------------------------------------------------------------

#[tauri::command]
fn ai_provider_save(
    core: tauri::State<'_, ArcCore>,
    kind: AiProviderKind,
    base_url: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
    is_default: bool,
) -> CmdResult<AiProviderConfig> {
    core.ai_provider_save(kind, base_url, model, api_key, is_default)
        .map_err(err)
}

#[tauri::command]
fn ai_provider_list(core: tauri::State<'_, ArcCore>) -> CmdResult<Vec<AiProviderConfig>> {
    core.ai_provider_list().map_err(err)
}

#[tauri::command]
fn ai_provider_remove(core: tauri::State<'_, ArcCore>, id: String) -> CmdResult<()> {
    core.ai_provider_remove(&id).map_err(err)
}

#[tauri::command]
fn required_scopes(
    core: tauri::State<'_, ArcCore>,
    kind: CiKind,
) -> CmdResult<Vec<(String, String)>> {
    Ok(core.required_scopes(kind))
}

// ---------------------------------------------------------------------------
// E6 — CI/CD
// ---------------------------------------------------------------------------

#[tauri::command]
async fn ci_account_add(
    core: tauri::State<'_, ArcCore>,
    kind: CiKind,
    base_url: String,
    org: Option<String>,
    project: Option<String>,
    repo: Option<String>,
    token: String,
    scopes: Vec<String>,
) -> CmdResult<(CiAccount, String)> {
    core.ci_account_add(kind, base_url, org, project, repo, token, scopes)
        .await
        .map_err(err)
}

#[tauri::command]
fn ci_account_list(core: tauri::State<'_, ArcCore>) -> CmdResult<Vec<CiAccount>> {
    core.ci_account_list().map_err(err)
}

#[tauri::command]
fn ci_account_remove(core: tauri::State<'_, ArcCore>, id: String) -> CmdResult<()> {
    core.ci_account_remove(&id).map_err(err)
}

#[tauri::command]
async fn ci_inventory(
    app: tauri::AppHandle,
    tasks: tauri::State<'_, Tasks>,
    core: tauri::State<'_, ArcCore>,
    account_id: String,
    max: usize,
    task_id: Option<String>,
) -> CmdResult<Vec<CiRun>> {
    let ctx = task_ctx(&app, &tasks, "ci_inventory", &task_id);
    let res = core
        .ci_inventory_with(&account_id, max, &ctx)
        .await
        .map_err(err);
    task_done(&tasks, &task_id);
    res
}

#[tauri::command]
fn policy_save(
    core: tauri::State<'_, ArcCore>,
    name: String,
    rules: RetentionRules,
) -> CmdResult<RetentionPolicy> {
    core.policy_save(name, rules).map_err(err)
}

#[tauri::command]
fn policy_list(core: tauri::State<'_, ArcCore>) -> CmdResult<Vec<RetentionPolicy>> {
    core.policy_list().map_err(err)
}

#[tauri::command]
async fn ci_simulate(
    app: tauri::AppHandle,
    tasks: tauri::State<'_, Tasks>,
    core: tauri::State<'_, ArcCore>,
    account_id: String,
    policy_id: String,
    max: usize,
    task_id: Option<String>,
) -> CmdResult<SimulationReport> {
    let ctx = task_ctx(&app, &tasks, "ci_simulate", &task_id);
    let res = core
        .ci_simulate_with(&account_id, &policy_id, max, &ctx)
        .await
        .map_err(err);
    task_done(&tasks, &task_id);
    res
}

#[tauri::command]
async fn ci_delete_run(
    core: tauri::State<'_, ArcCore>,
    account_id: String,
    policy_id: String,
    run: CiRun,
    confirm: String,
) -> CmdResult<()> {
    core.ci_delete_run(&account_id, &policy_id, run, confirm)
        .await
        .map_err(err)
}

#[tauri::command]
async fn ci_delete_batch(
    app: tauri::AppHandle,
    tasks: tauri::State<'_, Tasks>,
    core: tauri::State<'_, ArcCore>,
    account_id: String,
    policy_id: String,
    runs: Vec<CiRun>,
    confirm: String,
    already_done: Vec<String>,
    task_id: Option<String>,
) -> CmdResult<mc_core::api::BatchDeleteResult> {
    let ctx = task_ctx(&app, &tasks, "ci_delete_batch", &task_id);
    let res = core
        .ci_delete_batch(&account_id, &policy_id, runs, confirm, already_done, &ctx)
        .await
        .map_err(err);
    task_done(&tasks, &task_id);
    res
}

#[tauri::command]
async fn ci_purge_assets(
    app: tauri::AppHandle,
    tasks: tauri::State<'_, Tasks>,
    core: tauri::State<'_, ArcCore>,
    account_id: String,
    runs: Vec<CiRun>,
    purge_logs: bool,
    purge_artifacts: bool,
    confirm: String,
    task_id: Option<String>,
) -> CmdResult<mc_core::api::PurgeResult> {
    let ctx = task_ctx(&app, &tasks, "ci_purge_assets", &task_id);
    let res = core
        .ci_purge_assets(
            &account_id,
            runs,
            purge_logs,
            purge_artifacts,
            confirm,
            &ctx,
        )
        .await
        .map_err(err);
    task_done(&tasks, &task_id);
    res
}

// ---------------------------------------------------------------------------
// E7 — audit
// ---------------------------------------------------------------------------

#[tauri::command]
fn audit_list(core: tauri::State<'_, ArcCore>, limit: u32) -> CmdResult<Vec<AuditEvent>> {
    core.audit_list(limit).map_err(err)
}

#[tauri::command]
fn audit_export(core: tauri::State<'_, ArcCore>) -> CmdResult<String> {
    core.audit_export().map_err(err)
}

// ---------------------------------------------------------------------------

fn resolve_skills_dir(app: &tauri::App) -> PathBuf {
    // 1. Ressource embarquée (bundle) ; 2. variable d'env ; 3. arbre de dev.
    if let Ok(p) = app
        .path()
        .resolve("skills", tauri::path::BaseDirectory::Resource)
    {
        if p.exists() {
            return p;
        }
    }
    if let Ok(p) = std::env::var("MC_SKILLS_DIR") {
        return PathBuf::from(p);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../skills")
}

/// Writer du journal structuré : chaque ligne passe par la redaction des
/// secrets AVANT d'atteindre le disque (T4).
#[derive(Clone)]
struct RedactingFile(PathBuf);

impl std::io::Write for RedactingFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        use std::io::Write as _;
        let line = mc_core::secrets::redact(&String::from_utf8_lossy(buf));
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.0)?;
        f.write_all(line.as_bytes())?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for RedactingFile {
    type Writer = RedactingFile;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // MC_DATA_DIR permet un usage portable (données à côté de l'exe) ;
            // par défaut : répertoire de données utilisateur (%APPDATA%).
            let data_dir = match std::env::var("MC_DATA_DIR") {
                Ok(p) if !p.trim().is_empty() => PathBuf::from(p),
                _ => app
                    .path()
                    .app_data_dir()
                    .expect("répertoire de données inaccessible"),
            };

            // Journal structuré redacté (niveau via MC_LOG, défaut info).
            let log_dir = data_dir.join("logs");
            std::fs::create_dir_all(&log_dir).ok();
            let filter = tracing_subscriber::EnvFilter::try_from_env("MC_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
            let _ = tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_ansi(false)
                .with_writer(RedactingFile(log_dir.join("mister-commitia.log")))
                .try_init();

            let db_path = data_dir.join("mister-commitia.sqlite");
            let skills_dir = resolve_skills_dir(app);
            let core = Core::new(&db_path, skills_dir)
                .map_err(|e| format!("initialisation du cœur : {e}"))?;
            app.manage(Arc::new(core));
            app.manage(Tasks::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            repos_list,
            repo_declare,
            repo_remove,
            repo_branches,
            repo_scan,
            repo_set_governance,
            commit_diff,
            plan_new,
            plan_set_ops,
            plan_dry_run,
            plan_apply,
            plan_conflict_resolve,
            plan_conflict_continue,
            plan_conflict_abort,
            plan_rollback,
            plan_list,
            plan_risk,
            plan_export,
            plan_import,
            push_preview,
            push_execute,
            skills_list,
            skill_read,
            skill_write,
            skill_run_tests,
            ai_preview,
            proposals_generate,
            proposals_list,
            proposal_decide,
            ai_provider_save,
            ai_provider_list,
            ai_provider_remove,
            required_scopes,
            ci_account_add,
            ci_account_list,
            ci_account_remove,
            ci_inventory,
            policy_save,
            policy_list,
            ci_simulate,
            ci_delete_run,
            ci_delete_batch,
            ci_purge_assets,
            audit_list,
            audit_export,
            task_cancel
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de mister-commitia");
}
