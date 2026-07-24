// Empêche l'ouverture d'une console sous Windows en release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use mc_core::model::*;
use mc_core::Core;
use tauri::Manager;

type CmdResult<T> = std::result::Result<T, String>;

fn err(e: mc_core::CoreError) -> String {
    e.to_string()
}

// ---------------------------------------------------------------------------
// E1/E2 — dépôts & analyse
// ---------------------------------------------------------------------------

#[tauri::command]
fn repos_list(core: tauri::State<'_, Core>) -> CmdResult<Vec<RepoRef>> {
    core.repo_list().map_err(err)
}

#[tauri::command]
fn repo_declare(core: tauri::State<'_, Core>, path: String) -> CmdResult<RepoRef> {
    core.repo_declare(&path).map_err(err)
}

#[tauri::command]
fn repo_remove(core: tauri::State<'_, Core>, id: String) -> CmdResult<()> {
    core.repo_remove(&id).map_err(err)
}

#[tauri::command]
fn repo_branches(core: tauri::State<'_, Core>, id: String) -> CmdResult<Vec<BranchInfo>> {
    core.repo_branches(&id).map_err(err)
}

#[tauri::command]
fn repo_scan(
    core: tauri::State<'_, Core>,
    id: String,
    branch: Option<String>,
) -> CmdResult<mc_core::api::ScanResult> {
    core.repo_scan(&id, branch).map_err(err)
}

#[tauri::command]
fn repo_set_governance(
    core: tauri::State<'_, Core>,
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
fn plan_new(core: tauri::State<'_, Core>, repo_id: String, branch: String) -> CmdResult<Plan> {
    core.plan_new(&repo_id, &branch).map_err(err)
}

#[tauri::command]
fn plan_set_ops(core: tauri::State<'_, Core>, plan_id: String, ops: Vec<PlanOp>) -> CmdResult<Plan> {
    core.plan_set_ops(&plan_id, ops).map_err(err)
}

#[tauri::command]
fn plan_dry_run(core: tauri::State<'_, Core>, plan_id: String) -> CmdResult<Plan> {
    core.plan_dry_run(&plan_id).map_err(err)
}

#[tauri::command]
fn plan_apply(
    core: tauri::State<'_, Core>,
    plan_id: String,
    confirm: Option<String>,
) -> CmdResult<Plan> {
    core.plan_apply(&plan_id, confirm).map_err(err)
}

#[tauri::command]
fn plan_rollback(core: tauri::State<'_, Core>, plan_id: String) -> CmdResult<Plan> {
    core.plan_rollback(&plan_id).map_err(err)
}

#[tauri::command]
fn plan_list(core: tauri::State<'_, Core>, repo_id: String) -> CmdResult<Vec<Plan>> {
    core.plan_list(&repo_id).map_err(err)
}

#[tauri::command]
fn plan_risk(
    core: tauri::State<'_, Core>,
    plan_id: String,
) -> CmdResult<Vec<mc_core::plan::RiskAxis>> {
    core.plan_risk(&plan_id).map_err(err)
}

#[tauri::command]
fn plan_export(core: tauri::State<'_, Core>, plan_id: String) -> CmdResult<String> {
    core.plan_export(&plan_id).map_err(err)
}

#[tauri::command]
fn plan_import(core: tauri::State<'_, Core>, repo_id: String, json: String) -> CmdResult<Plan> {
    core.plan_import(&repo_id, &json).map_err(err)
}

// ---------------------------------------------------------------------------
// E4 — skills & IA
// ---------------------------------------------------------------------------

#[tauri::command]
fn skills_list(
    core: tauri::State<'_, Core>,
) -> CmdResult<(Vec<mc_core::api::SkillMeta>, Vec<(String, String)>)> {
    core.skills_list().map_err(err)
}

#[tauri::command]
fn skill_run_tests(
    core: tauri::State<'_, Core>,
    name: String,
) -> CmdResult<Vec<mc_core::api::SkillTestResult>> {
    core.skill_run_tests(&name).map_err(err)
}

#[tauri::command]
fn ai_preview(
    core: tauri::State<'_, Core>,
    repo_id: String,
    skill: String,
    shas: Vec<String>,
) -> CmdResult<String> {
    core.ai_preview(&repo_id, &skill, shas).map_err(err)
}

#[tauri::command]
async fn proposals_generate(
    core: tauri::State<'_, Core>,
    repo_id: String,
    skill: String,
    groups: Vec<Vec<String>>,
    provider_id: Option<String>,
    consent_remote: bool,
) -> CmdResult<Vec<Proposal>> {
    core.proposals_generate(&repo_id, &skill, groups, provider_id, consent_remote)
        .await
        .map_err(err)
}

#[tauri::command]
fn proposals_list(core: tauri::State<'_, Core>, repo_id: String) -> CmdResult<Vec<Proposal>> {
    core.proposals_list(&repo_id).map_err(err)
}

#[tauri::command]
fn proposal_decide(
    core: tauri::State<'_, Core>,
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
    core: tauri::State<'_, Core>,
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
fn ai_provider_list(core: tauri::State<'_, Core>) -> CmdResult<Vec<AiProviderConfig>> {
    core.ai_provider_list().map_err(err)
}

#[tauri::command]
fn ai_provider_remove(core: tauri::State<'_, Core>, id: String) -> CmdResult<()> {
    core.ai_provider_remove(&id).map_err(err)
}

#[tauri::command]
fn required_scopes(core: tauri::State<'_, Core>, kind: CiKind) -> CmdResult<Vec<(String, String)>> {
    Ok(core.required_scopes(kind))
}

// ---------------------------------------------------------------------------
// E6 — CI/CD
// ---------------------------------------------------------------------------

#[tauri::command]
async fn ci_account_add(
    core: tauri::State<'_, Core>,
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
fn ci_account_list(core: tauri::State<'_, Core>) -> CmdResult<Vec<CiAccount>> {
    core.ci_account_list().map_err(err)
}

#[tauri::command]
fn ci_account_remove(core: tauri::State<'_, Core>, id: String) -> CmdResult<()> {
    core.ci_account_remove(&id).map_err(err)
}

#[tauri::command]
async fn ci_inventory(
    core: tauri::State<'_, Core>,
    account_id: String,
    max: usize,
) -> CmdResult<Vec<CiRun>> {
    core.ci_inventory(&account_id, max).await.map_err(err)
}

#[tauri::command]
fn policy_save(
    core: tauri::State<'_, Core>,
    name: String,
    rules: RetentionRules,
) -> CmdResult<RetentionPolicy> {
    core.policy_save(name, rules).map_err(err)
}

#[tauri::command]
fn policy_list(core: tauri::State<'_, Core>) -> CmdResult<Vec<RetentionPolicy>> {
    core.policy_list().map_err(err)
}

#[tauri::command]
async fn ci_simulate(
    core: tauri::State<'_, Core>,
    account_id: String,
    policy_id: String,
    max: usize,
) -> CmdResult<SimulationReport> {
    core.ci_simulate(&account_id, &policy_id, max).await.map_err(err)
}

#[tauri::command]
async fn ci_delete_run(
    core: tauri::State<'_, Core>,
    account_id: String,
    policy_id: String,
    run: CiRun,
    confirm: String,
) -> CmdResult<()> {
    core.ci_delete_run(&account_id, &policy_id, run, confirm)
        .await
        .map_err(err)
}

// ---------------------------------------------------------------------------
// E7 — audit
// ---------------------------------------------------------------------------

#[tauri::command]
fn audit_list(core: tauri::State<'_, Core>, limit: u32) -> CmdResult<Vec<AuditEvent>> {
    core.audit_list(limit).map_err(err)
}

#[tauri::command]
fn audit_export(core: tauri::State<'_, Core>) -> CmdResult<String> {
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

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("répertoire de données inaccessible");
            let db_path = data_dir.join("mister-commitia.sqlite");
            let skills_dir = resolve_skills_dir(app);
            let core = Core::new(&db_path, skills_dir)
                .map_err(|e| format!("initialisation du cœur : {e}"))?;
            app.manage(core);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            repos_list,
            repo_declare,
            repo_remove,
            repo_branches,
            repo_scan,
            repo_set_governance,
            plan_new,
            plan_set_ops,
            plan_dry_run,
            plan_apply,
            plan_rollback,
            plan_list,
            plan_risk,
            plan_export,
            plan_import,
            skills_list,
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
            audit_list,
            audit_export
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de mister-commitia");
}
