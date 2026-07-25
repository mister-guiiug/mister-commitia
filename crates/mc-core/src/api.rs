//! Orchestrateur : l'API de haut niveau consommée par l'UI desktop (et par un
//! futur CLI). Chaque garde-fou vit ICI ou plus bas — jamais dans l'UI.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ai::{self, Provider, SkillContext};
use crate::analyzer;
use crate::ci::{self, CiClient};
use crate::error::{CoreError, Result};
use crate::gitx::GitEngine;
use crate::model::*;
use crate::plan::{plan_hash, PlanEngine, RiskAxis};
use crate::secrets;
use crate::skills::{self, GenOutcome, Skill};
use crate::store::Store;
use crate::task::{sleep_cancellable, TaskCtx};

pub struct Core {
    pub store: Store,
    pub skills_dir: PathBuf,
    pub actor: String,
    /// Cache d'analyse par SHA (T13) : parties SHA-invariantes d'un `CommitInfo`
    /// (diff, fichiers, trailers…). Le champ `on_remote`, dépendant du contexte,
    /// est TOUJOURS recalculé à chaque scan et n'est jamais servi depuis le cache.
    analysis_cache: Mutex<HashMap<String, CommitInfo>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub repo: RepoRef,
    pub branch: String,
    pub base: Option<String>,
    pub commits: Vec<CommitInfo>,
    pub report: AnalysisReport,
    pub squash_suggestions: Vec<Vec<String>>,
    /// Disposition en lanes de l'historique réel (vue graphe, F1). Reflète la
    /// topologie git courante, indépendamment d'un réordonnancement proposé.
    pub graph: crate::graph::CommitGraph,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMeta {
    pub name: String,
    pub version: String,
    pub owner: String,
    pub status: String,
    pub description: String,
    pub output: String,
    pub guardrails: Vec<String>,
    pub rules: Vec<String>,
    pub tests: usize,
    pub local_capable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillTestResult {
    pub case: String,
    pub passed: bool,
    pub detail: String,
}

/// Métadonnées de skills + erreurs de chargement (nom, motif).
pub type SkillsListing = (Vec<SkillMeta>, Vec<(String, String)>);

/// Aperçu du push assisté (F4) : état de divergence, bail de force-with-lease,
/// PR ouvertes détectées, avertissements de coordination. Aucun effet de bord.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushPreview {
    pub remote: Option<String>,
    pub remote_url: Option<String>,
    pub branch: String,
    pub local_tip: String,
    /// SHA de la ref remote-tracking (le bail de `--force-with-lease`).
    pub remote_tip: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    /// Vrai si le push n'est pas un fast-forward (réécriture distante).
    pub needs_force: bool,
    pub protected: bool,
    pub can_push: bool,
    /// PR ouvertes sur cette branche ; `None` = non vérifié (pas d'accès GitHub).
    pub open_prs: Option<Vec<PrRef>>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushResult {
    pub branch: String,
    pub forced: bool,
    pub remote_tip: String,
    pub detail: String,
}

/// Échec unitaire lors d'une suppression en masse (F7).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchFailure {
    pub run_id: String,
    pub reason: String,
}

/// Résultat d'un nettoyage CI en masse (F7). `deleted` inclut le point de
/// reprise fourni : renvoyé tel quel, il permet de RELANCER pour terminer un
/// lot annulé ou partiellement en échec sans re-supprimer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchDeleteResult {
    pub total: usize,
    pub deleted: Vec<String>,
    pub failed: Vec<BatchFailure>,
    /// Vrai si interrompu par l'utilisateur avant la fin (reprise possible).
    pub cancelled: bool,
}

/// Résultat d'une purge de logs/artefacts (F7, extension) : reclaim de stockage
/// qui CONSERVE les runs. Les runs en cours sont ignorés ; les échecs par run
/// sont collectés sans interrompre le reste du lot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurgeResult {
    pub runs: usize,
    pub artifacts_deleted: usize,
    pub logs_deleted: usize,
    pub failed: Vec<BatchFailure>,
    pub cancelled: bool,
}

impl Core {
    pub fn new(db_path: &Path, skills_dir: PathBuf) -> Result<Self> {
        Ok(Self {
            store: Store::open(db_path)?,
            skills_dir,
            actor: std::env::var("USERNAME")
                .or_else(|_| std::env::var("USER"))
                .unwrap_or_else(|_| "local".into()),
            analysis_cache: Mutex::new(HashMap::new()),
        })
    }

    pub fn in_memory(skills_dir: PathBuf) -> Result<Self> {
        Ok(Self {
            store: Store::open_in_memory()?,
            skills_dir,
            actor: "test".into(),
            analysis_cache: Mutex::new(HashMap::new()),
        })
    }

    /// Nombre d'entrées du cache d'analyse (T13) — utilitaire de test/diagnostic.
    pub fn analysis_cache_len(&self) -> usize {
        self.analysis_cache.lock().unwrap().len()
    }

    /// Lit un `CommitInfo` via le cache par SHA (parties SHA-invariantes), puis
    /// recalcule `on_remote` depuis le contexte courant (jamais mis en cache).
    fn commit_info_cached(
        &self,
        repo: &git2::Repository,
        oid: git2::Oid,
        remote_set: &HashSet<git2::Oid>,
    ) -> Result<CommitInfo> {
        let key = oid.to_string();
        let cached = self.analysis_cache.lock().unwrap().get(&key).cloned();
        let mut info = match cached {
            Some(ci) => ci,
            None => {
                // Calcul SHA-invariant : remote_set vide → on_remote = false.
                let fresh = GitEngine::commit_info(repo, oid, &HashSet::new())?;
                self.analysis_cache
                    .lock()
                    .unwrap()
                    .insert(key, fresh.clone());
                fresh
            }
        };
        info.on_remote = remote_set.contains(&oid);
        Ok(info)
    }

    fn audit(
        &self,
        category: &str,
        action: &str,
        target: &str,
        params: serde_json::Value,
        result: &str,
    ) {
        let redacted: serde_json::Value =
            serde_json::from_str(&secrets::redact(&params.to_string()))
                .unwrap_or(serde_json::Value::Null);
        // Journal structuré (T4) — le writer du souscripteur redacte aussi.
        tracing::info!(
            target: "mc::audit",
            category,
            action,
            cible = target,
            resultat = %secrets::redact(result),
            "audit"
        );
        let _ = self.store.audit_append(
            &self.actor,
            category,
            action,
            target,
            &redacted,
            &secrets::redact(result),
        );
    }

    // -- E1 : dépôts ---------------------------------------------------------

    pub fn repo_declare(&self, path: &str) -> Result<RepoRef> {
        let repo = GitEngine::open(path)?;
        let name = Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string());
        let default_branch = GitEngine::detect_default_branch(&repo);
        let mut protected = Vec::new();
        if let Some(d) = &default_branch {
            protected.push(d.clone());
        }
        let r = RepoRef {
            id: new_id("repo"),
            name,
            local_path: path.to_string(),
            remote_url: GitEngine::remote_url(&repo),
            default_branch,
            protected_branches: protected,
            governance: Governance::default(),
            added_at: now_iso(),
            last_scanned_at: None,
        };
        self.store.repo_add(&r)?;
        self.audit(
            "config",
            "repo_declare",
            &r.name,
            json!({"path": path}),
            "ok",
        );
        Ok(r)
    }

    pub fn repo_list(&self) -> Result<Vec<RepoRef>> {
        self.store.repo_list()
    }

    pub fn repo_remove(&self, id: &str) -> Result<()> {
        let r = self.store.repo_get(id)?;
        self.store.repo_remove(id)?;
        self.audit("config", "repo_remove", &r.name, json!({}), "ok");
        Ok(())
    }

    pub fn repo_update_governance(
        &self,
        id: &str,
        governance: Governance,
        protected_branches: Vec<String>,
    ) -> Result<RepoRef> {
        let mut r = self.store.repo_get(id)?;
        r.governance = governance;
        r.protected_branches = protected_branches;
        self.store.repo_update(&r)?;
        self.audit("config", "governance_update", &r.name, json!({}), "ok");
        Ok(r)
    }

    pub fn repo_branches(&self, id: &str) -> Result<Vec<BranchInfo>> {
        let r = self.store.repo_get(id)?;
        let repo = GitEngine::open(&r.local_path)?;
        GitEngine::branches(&repo)
    }

    // -- E2 : analyse --------------------------------------------------------

    pub fn repo_scan(&self, id: &str, branch: Option<String>) -> Result<ScanResult> {
        self.repo_scan_with(id, branch, &TaskCtx::noop("repo_scan"))
    }

    /// Analyse avec progression (par commit lu) et annulation coopérative.
    pub fn repo_scan_with(
        &self,
        id: &str,
        branch: Option<String>,
        ctx: &TaskCtx,
    ) -> Result<ScanResult> {
        self.repo_scan_base(id, branch, None, ctx)
    }

    /// Comme `repo_scan_with`, mais permet de FORCER la base du segment (F6) :
    /// `base_ref` (branche, tag ou SHA) remplace le merge-base automatique —
    /// utile pour les branches empilées où l'on veut analyser au-delà du point
    /// de divergence par défaut. La base doit être un ancêtre STRICT du sommet.
    pub fn repo_scan_base(
        &self,
        id: &str,
        branch: Option<String>,
        base_ref: Option<String>,
        ctx: &TaskCtx,
    ) -> Result<ScanResult> {
        let mut r = self.store.repo_get(id)?;
        ctx.step("ouverture du dépôt", 0, None)?;
        let repo = GitEngine::open(&r.local_path)?;
        let branch = branch
            .or_else(|| GitEngine::head_branch(&repo))
            .or_else(|| r.default_branch.clone())
            .ok_or_else(|| CoreError::Invalid("aucune branche à analyser".into()))?;
        let tip = GitEngine::branch_tip(&repo, &branch)?;
        let base = match base_ref {
            // F6 : base explicite (branche/tag/SHA). Validée : commit existant,
            // distinct du sommet, et ancêtre du sommet (sinon segment absurde).
            Some(spec) => {
                let b = GitEngine::resolve(&repo, spec.trim())?;
                if b == tip {
                    return Err(CoreError::Invalid(
                        "la base choisie est le sommet lui-même : le segment serait vide".into(),
                    ));
                }
                if !repo.graph_descendant_of(tip, b).unwrap_or(false) {
                    return Err(CoreError::Invalid(
                        "la base choisie n'est pas un ancêtre du sommet de la branche".into(),
                    ));
                }
                Some(b)
            }
            None => match (&r.default_branch, branch.as_str()) {
                (Some(d), b) if d != b => {
                    let dt = GitEngine::branch_tip(&repo, d)?;
                    let mb = GitEngine::merge_base(&repo, tip, dt)?;
                    if mb == tip {
                        None
                    } else {
                        Some(mb)
                    }
                }
                _ => None,
            },
        };
        // Lecture des commits avec cache par SHA (T13) + progression/annulation.
        let mut oids = GitEngine::segment(&repo, base, tip)?;
        if oids.len() > 500 {
            oids = oids.split_off(oids.len() - 500);
        }
        let remote_set = GitEngine::remote_reachable(&repo, base);
        let total = oids.len() as u64;
        let mut commits = Vec::with_capacity(oids.len());
        for (i, o) in oids.iter().enumerate() {
            ctx.step("lecture des commits", i as u64 + 1, Some(total))?;
            commits.push(self.commit_info_cached(&repo, *o, &remote_set)?);
        }
        ctx.step("analyse des messages", 0, None)?;
        let report = analyzer::analyze_commits(
            &r,
            &branch,
            base.map(|o| o.to_string()).as_deref(),
            &commits,
        );
        let squash_suggestions = analyzer::suggest_squash_groups(&commits);
        let graph = crate::graph::build_graph(&commits);
        r.last_scanned_at = Some(now_iso());
        self.store.repo_update(&r)?;
        Ok(ScanResult {
            repo: r,
            branch,
            base: base.map(|o| o.to_string()),
            commits,
            report,
            squash_suggestions,
            graph,
        })
    }

    /// Patch unifié d'un commit (visionneuse de diff, F3).
    pub fn commit_diff(&self, repo_id: &str, sha: &str) -> Result<String> {
        let r = self.store.repo_get(repo_id)?;
        let repo = GitEngine::open(&r.local_path)?;
        let oid =
            git2::Oid::from_str(sha).map_err(|e| CoreError::Invalid(format!("sha {sha} : {e}")))?;
        GitEngine::commit_patch(&repo, oid, 200_000)
    }

    // -- E3 : plans ----------------------------------------------------------

    pub fn plan_new(&self, repo_id: &str, branch: &str) -> Result<Plan> {
        let r = self.store.repo_get(repo_id)?;
        let repo = GitEngine::open(&r.local_path)?;
        let plan = PlanEngine::new_plan(&r, &repo, branch)?;
        self.store.plan_save(&plan)?;
        Ok(plan)
    }

    pub fn plan_get(&self, id: &str) -> Result<Plan> {
        self.store.plan_get(id)
    }

    pub fn plan_list(&self, repo_id: &str) -> Result<Vec<Plan>> {
        self.store.plan_list(repo_id)
    }

    pub fn plan_set_ops(&self, plan_id: &str, ops: Vec<PlanOp>) -> Result<Plan> {
        let mut plan = self.store.plan_get(plan_id)?;
        if matches!(plan.status, PlanStatus::Applied | PlanStatus::RolledBack) {
            return Err(CoreError::Refused(
                "un plan appliqué est immuable : créer un nouveau plan".into(),
            ));
        }
        plan.ops = ops;
        plan.status = PlanStatus::Draft;
        plan.dry_run_hash = None;
        plan.dry_run_at = None;
        plan.mapping.clear();
        self.store.plan_save(&plan)?;
        Ok(plan)
    }

    pub fn plan_dry_run(&self, plan_id: &str) -> Result<Plan> {
        self.plan_dry_run_with(plan_id, &TaskCtx::noop("plan_dry_run"))
    }

    pub fn plan_dry_run_with(&self, plan_id: &str, ctx: &TaskCtx) -> Result<Plan> {
        let mut plan = self.store.plan_get(plan_id)?;
        let r = self.store.repo_get(&plan.repo_id)?;
        let repo = GitEngine::open(&r.local_path)?;
        let outcome = PlanEngine::dry_run(&r, &repo, &mut plan, ctx);
        match &outcome {
            Ok(()) => self.audit(
                "git_rewrite",
                "dry_run",
                &format!("{}:{}", r.name, plan.fingerprint.branch),
                json!({"plan": plan.id, "ops": plan.ops.len()}),
                "ok",
            ),
            Err(e) => self.audit(
                "git_rewrite",
                "dry_run",
                &format!("{}:{}", r.name, plan.fingerprint.branch),
                json!({"plan": plan.id}),
                &format!("erreur : {e}"),
            ),
        }
        self.store.plan_save(&plan)?;
        outcome.map(|_| plan)
    }

    pub fn plan_apply(&self, plan_id: &str, confirm: Option<String>) -> Result<Plan> {
        self.plan_apply_with(plan_id, confirm, &TaskCtx::noop("plan_apply"))
    }

    pub fn plan_apply_with(
        &self,
        plan_id: &str,
        confirm: Option<String>,
        ctx: &TaskCtx,
    ) -> Result<Plan> {
        let mut plan = self.store.plan_get(plan_id)?;
        let r = self.store.repo_get(&plan.repo_id)?;
        let repo = GitEngine::open(&r.local_path)?;
        let outcome = PlanEngine::apply(&r, &repo, &mut plan, confirm.as_deref(), ctx);
        let result = match &outcome {
            Ok(()) => "ok".to_string(),
            Err(e) => format!("erreur : {e}"),
        };
        self.audit(
            "git_rewrite",
            "apply",
            &format!("{}:{}", r.name, plan.fingerprint.branch),
            json!({"plan": plan.id, "backup_ref": plan.backup_ref, "mapping": plan.mapping.len()}),
            &result,
        );
        self.store.plan_save(&plan)?;
        outcome.map(|_| plan)
    }

    pub fn plan_rollback(&self, plan_id: &str) -> Result<Plan> {
        let mut plan = self.store.plan_get(plan_id)?;
        let r = self.store.repo_get(&plan.repo_id)?;
        let repo = GitEngine::open(&r.local_path)?;
        let outcome = PlanEngine::rollback(&r, &repo, &mut plan);
        let result = match &outcome {
            Ok(()) => "ok".to_string(),
            Err(e) => format!("erreur : {e}"),
        };
        self.audit(
            "git_rewrite",
            "rollback",
            &format!("{}:{}", r.name, plan.fingerprint.branch),
            json!({"plan": plan.id, "backup_ref": plan.backup_ref}),
            &result,
        );
        self.store.plan_save(&plan)?;
        outcome.map(|_| plan)
    }

    pub fn plan_risk(&self, plan_id: &str) -> Result<Vec<RiskAxis>> {
        let plan = self.store.plan_get(plan_id)?;
        let r = self.store.repo_get(&plan.repo_id)?;
        let repo = GitEngine::open(&r.local_path)?;
        Ok(PlanEngine::risk_report(&r, &repo, &plan))
    }

    pub fn plan_export(&self, plan_id: &str) -> Result<String> {
        let plan = self.store.plan_get(plan_id)?;
        Ok(serde_json::to_string_pretty(&plan)?)
    }

    pub fn plan_import(&self, repo_id: &str, json_str: &str) -> Result<Plan> {
        let mut plan: Plan = serde_json::from_str(json_str)?;
        if plan.version != 1 {
            return Err(CoreError::Invalid(format!(
                "version de plan non supportée : {}",
                plan.version
            )));
        }
        plan.repo_id = repo_id.to_string();
        plan.id = new_id("pln");
        plan.status = PlanStatus::Draft;
        plan.dry_run_hash = None;
        plan.dry_run_at = None;
        plan.applied_at = None;
        plan.preview_ref = None;
        plan.backup_ref = None;
        plan.backup_tag = None;
        plan.mapping.clear();
        self.store.plan_save(&plan)?;
        Ok(plan)
    }

    // -- E3b : push assisté (F4) --------------------------------------------

    /// Divergence entre la branche locale et le remote-tracking. Rafraîchit
    /// d'abord la ref remote-tracking (fetch) pour un état exact et un bail sûr.
    async fn push_facts(
        &self,
        repo_ref: &RepoRef,
        branch: &str,
        do_fetch: bool,
    ) -> Result<(bool, Option<String>, usize, usize)> {
        let repo = GitEngine::open(&repo_ref.local_path)?;
        let has_remote = repo.find_remote("origin").is_ok();
        if !has_remote {
            return Ok((false, None, 0, 0));
        }
        let local_tip = GitEngine::branch_tip(&repo, branch)?;
        if do_fetch {
            let dir = repo
                .workdir()
                .ok_or_else(|| CoreError::Refused("dépôt bare".into()))?
                .to_path_buf();
            let _ = crate::gitx::push::fetch_branch(&dir, "origin", branch);
        }
        let remote_tip = repo
            .refname_to_id(&format!("refs/remotes/origin/{branch}"))
            .ok();
        let (ahead, behind) = match remote_tip {
            Some(rt) => repo.graph_ahead_behind(local_tip, rt).unwrap_or((0, 0)),
            None => (0, 0),
        };
        Ok((true, remote_tip.map(|o| o.to_string()), ahead, behind))
    }

    /// PR ouvertes pour `branch` via un accès GitHub explicite (best-effort).
    async fn detect_open_prs(
        &self,
        branch: &str,
        ci_account_id: Option<&str>,
        warnings: &mut Vec<String>,
    ) -> Option<Vec<PrRef>> {
        let account = self.store.ci_account_get(ci_account_id?).ok()?;
        if !matches!(account.kind, CiKind::Github | CiKind::GithubEnterprise) {
            warnings.push("détection des PR : uniquement pour un accès GitHub.".into());
            return None;
        }
        let token = secrets::get_secret(&account.token_ref).ok()?;
        let client = CiClient::from_account(&account, token).ok()?;
        match client.list_open_prs(branch).await {
            Ok(prs) => Some(prs),
            Err(e) => {
                warnings.push(format!("PR non vérifiées : {e}"));
                None
            }
        }
    }

    pub async fn push_preview(
        &self,
        repo_id: &str,
        branch: &str,
        ci_account_id: Option<String>,
    ) -> Result<PushPreview> {
        let r = self.store.repo_get(repo_id)?;
        let repo = GitEngine::open(&r.local_path)?;
        let local_tip = GitEngine::branch_tip(&repo, branch)?;
        let remote_url = GitEngine::remote_url(&repo);
        let protected = r.protected_branches.iter().any(|b| b == branch)
            || r.default_branch.as_deref() == Some(branch);
        let mut warnings = Vec::new();

        let (has_remote, remote_tip, ahead, behind) = self.push_facts(&r, branch, true).await?;
        if !has_remote {
            warnings.push("aucun remote « origin » configuré : push impossible.".into());
            return Ok(PushPreview {
                remote: None,
                remote_url,
                branch: branch.to_string(),
                local_tip: local_tip.to_string(),
                remote_tip: None,
                ahead: 0,
                behind: 0,
                needs_force: false,
                protected,
                can_push: false,
                open_prs: None,
                warnings,
            });
        }
        let needs_force = behind > 0;
        if remote_tip.is_none() {
            warnings.push("nouvelle branche : premier push (aucune version distante).".into());
        }
        if needs_force {
            warnings.push(format!(
                "réécriture de l'historique distant : {behind} commit(s) distant(s) seront remplacés. \
                 À coordonner (les collègues devront réaligner leur copie) ; push forcé sécurisé par --force-with-lease."
            ));
        }
        if protected && needs_force {
            warnings.push(format!(
                "« {branch} » est protégée : le push forcé sera refusé."
            ));
        }
        let open_prs = self
            .detect_open_prs(branch, ci_account_id.as_deref(), &mut warnings)
            .await;
        if let Some(prs) = &open_prs {
            if !prs.is_empty() && needs_force {
                warnings.push(format!(
                    "{} PR ouverte(s) sur cette branche : le push forcé mettra à jour leur contenu.",
                    prs.len()
                ));
            }
        }
        Ok(PushPreview {
            remote: Some("origin".into()),
            remote_url,
            branch: branch.to_string(),
            local_tip: local_tip.to_string(),
            remote_tip,
            ahead,
            behind,
            needs_force,
            protected,
            can_push: true,
            open_prs,
            warnings,
        })
    }

    /// Pousse la branche. Force-with-lease requis quand l'historique distant
    /// diverge : refusé sur branche protégée, confirmation renforcée exigée,
    /// journalisé avant/après. N'effectue PAS de fetch : le bail s'appuie sur le
    /// remote-tracking déjà vu (protège tout travail distant non revu).
    pub fn push_execute(
        &self,
        repo_id: &str,
        branch: &str,
        confirm: Option<String>,
    ) -> Result<PushResult> {
        let r = self.store.repo_get(repo_id)?;
        let repo = GitEngine::open(&r.local_path)?;
        if repo.find_remote("origin").is_err() {
            return Err(CoreError::Refused(
                "aucun remote « origin » configuré".into(),
            ));
        }
        let dir = repo
            .workdir()
            .ok_or_else(|| CoreError::Refused("dépôt bare".into()))?
            .to_path_buf();
        let local_tip = GitEngine::branch_tip(&repo, branch)?;
        let remote_tip = repo
            .refname_to_id(&format!("refs/remotes/origin/{branch}"))
            .ok();
        let behind = match remote_tip {
            Some(rt) => repo.graph_ahead_behind(local_tip, rt).unwrap_or((0, 0)).1,
            None => 0,
        };
        let needs_force = behind > 0;
        let protected = r.protected_branches.iter().any(|b| b == branch)
            || r.default_branch.as_deref() == Some(branch);

        if needs_force && protected {
            return Err(CoreError::Refused(format!(
                "« {branch} » est protégée : push forcé refusé (réécriture d'historique distant interdite)"
            )));
        }
        if needs_force && confirm.as_deref() != Some(branch) {
            return Err(CoreError::ConfirmRequired {
                expected: branch.to_string(),
                message: format!(
                    "push forcé (--force-with-lease) : saisir exactement « {branch} » pour confirmer la réécriture de l'historique distant"
                ),
            });
        }

        self.audit(
            "git_push",
            "push_attempt",
            &format!("{}:{}", r.name, branch),
            json!({
                "force": needs_force,
                "remote_tip": remote_tip.map(|o| o.to_string()),
                "local_tip": local_tip.to_string(),
            }),
            "tentative",
        );
        let lease = if needs_force {
            remote_tip.map(|o| o.to_string())
        } else {
            None
        };
        let set_upstream = remote_tip.is_none();
        let outcome =
            crate::gitx::push::push_branch(&dir, "origin", branch, lease.as_deref(), set_upstream);
        let result = match &outcome {
            Ok(()) => "ok".to_string(),
            Err(e) => format!("erreur : {e}"),
        };
        self.audit(
            "git_push",
            "push",
            &format!("{}:{}", r.name, branch),
            json!({"force": needs_force, "lease": lease}),
            &result,
        );
        outcome?;
        Ok(PushResult {
            branch: branch.to_string(),
            forced: needs_force,
            remote_tip: local_tip.to_string(),
            detail: if needs_force {
                "historique distant réécrit (force-with-lease)".into()
            } else {
                "commits poussés".into()
            },
        })
    }

    // -- E4 : skills & IA ----------------------------------------------------

    pub fn skills_load(&self) -> Result<skills::SkillLoadResult> {
        skills::load_dir(&self.skills_dir)
    }

    pub fn skills_list(&self) -> Result<SkillsListing> {
        let (loaded, errors) = self.skills_load()?;
        let metas = loaded
            .iter()
            .map(|s| SkillMeta {
                name: s.def.name.clone(),
                version: s.def.version.clone(),
                owner: s.def.owner.clone(),
                status: s.def.status.clone(),
                description: s.def.description.trim().to_string(),
                output: s
                    .def
                    .output
                    .as_ref()
                    .map(|o| o.kind.clone())
                    .unwrap_or_default(),
                guardrails: s.def.guardrails.iter().map(|g| g.assert.clone()).collect(),
                rules: s
                    .def
                    .rules
                    .iter()
                    .map(|r| r.text.trim().to_string())
                    .collect(),
                tests: s.test_cases.len(),
                local_capable: matches!(
                    s.def.name.as_str(),
                    "conventional-commits" | "commit-synthesis" | "ai-signature-cleaner"
                ),
            })
            .collect();
        Ok((metas, errors))
    }

    fn skill_by_name(&self, name: &str) -> Result<Skill> {
        let (loaded, _) = self.skills_load()?;
        loaded
            .into_iter()
            .find(|s| s.def.name == name)
            .ok_or_else(|| CoreError::NotFound(format!("skill {name}")))
    }

    fn provider_from_config(&self, cfg: &AiProviderConfig) -> Result<Provider> {
        Ok(match cfg.kind {
            AiProviderKind::RuleBased => Provider::RuleBased,
            AiProviderKind::Ollama => Provider::Ollama {
                base_url: cfg
                    .base_url
                    .clone()
                    .unwrap_or_else(|| "http://127.0.0.1:11434".into()),
                model: cfg.model.clone().unwrap_or_else(|| "qwen2.5-coder".into()),
            },
            AiProviderKind::OpenAiCompat => Provider::OpenAiCompat {
                base_url: cfg
                    .base_url
                    .clone()
                    .ok_or_else(|| CoreError::Invalid("base_url requis".into()))?,
                model: cfg.model.clone().unwrap_or_default(),
                api_key: match &cfg.key_ref {
                    Some(r) => secrets::get_secret(r)?,
                    None => String::new(),
                },
            },
            AiProviderKind::Anthropic => {
                Provider::Anthropic {
                    base_url: cfg
                        .base_url
                        .clone()
                        .unwrap_or_else(|| "https://api.anthropic.com".into()),
                    model: cfg
                        .model
                        .clone()
                        .unwrap_or_else(|| "claude-sonnet-5".into()),
                    api_key: secrets::get_secret(cfg.key_ref.as_deref().ok_or_else(|| {
                        CoreError::Invalid("clé d'API absente du coffre".into())
                    })?)?,
                }
            }
        })
    }

    fn resolve_provider(&self, provider_id: Option<&str>) -> Result<(Provider, String)> {
        let configs = self.store.ai_provider_list()?;
        let cfg = match provider_id {
            Some(id) => configs.into_iter().find(|c| c.id == id),
            None => configs.into_iter().find(|c| c.is_default),
        };
        match cfg {
            Some(c) => Ok((self.provider_from_config(&c)?, format!("{:?}", c.kind))),
            None => Ok((Provider::RuleBased, "RuleBased".into())),
        }
    }

    fn commits_by_shas(&self, repo_ref: &RepoRef, shas: &[String]) -> Result<Vec<CommitInfo>> {
        let repo = GitEngine::open(&repo_ref.local_path)?;
        let empty = std::collections::HashSet::new();
        shas.iter()
            .map(|s| {
                let oid = git2::Oid::from_str(s)
                    .map_err(|e| CoreError::Invalid(format!("sha {s} : {e}")))?;
                GitEngine::commit_info(&repo, oid, &empty)
            })
            .collect()
    }

    /// Aperçu exact de ce qui serait transmis au fournisseur (consentement CA-9).
    pub fn ai_preview(&self, repo_id: &str, skill_name: &str, shas: Vec<String>) -> Result<String> {
        let r = self.store.repo_get(repo_id)?;
        let skill = self.skill_by_name(skill_name)?;
        let commits = self.commits_by_shas(&r, &shas)?;
        let ctx = SkillContext {
            skill: &skill,
            governance: &r.governance,
            commits: &commits,
        };
        Ok(ai::preview_payload(&ctx))
    }

    /// Génère des propositions pour des groupes de commits. `consent_remote`
    /// doit être vrai pour un fournisseur distant (aperçu montré à l'utilisateur).
    pub async fn proposals_generate(
        &self,
        repo_id: &str,
        skill_name: &str,
        groups: Vec<Vec<String>>,
        provider_id: Option<String>,
        consent_remote: bool,
    ) -> Result<Vec<Proposal>> {
        self.proposals_generate_with(
            repo_id,
            skill_name,
            groups,
            provider_id,
            consent_remote,
            &TaskCtx::noop("proposals_generate"),
        )
        .await
    }

    /// Variante événementielle (T11) : progression par groupe, fragments IA
    /// streamés via `ctx.ai_delta`, budget de tokens réparti sur le lot,
    /// annulation entre deux groupes (les propositions déjà générées restent
    /// enregistrées et journalisées).
    pub async fn proposals_generate_with(
        &self,
        repo_id: &str,
        skill_name: &str,
        groups: Vec<Vec<String>>,
        provider_id: Option<String>,
        consent_remote: bool,
        task: &TaskCtx,
    ) -> Result<Vec<Proposal>> {
        let r = self.store.repo_get(repo_id)?;
        let skill = self.skill_by_name(skill_name)?;
        let (provider, provider_label) = self.resolve_provider(provider_id.as_deref())?;
        if provider.is_remote() && !consent_remote {
            return Err(CoreError::ConsentRequired(
                "envoi à un fournisseur IA distant : accord explicite requis, aperçu des données à l'appui".into(),
            ));
        }
        let total = groups.len() as u64;
        let max_tokens = ai::batch_max_tokens(groups.len());
        let mut out = Vec::new();
        for (gi, shas) in groups.into_iter().enumerate() {
            task.step("génération des propositions", gi as u64 + 1, Some(total))?;
            let commits = self.commits_by_shas(&r, &shas)?;
            let before = commits
                .iter()
                .map(|c| c.full_message())
                .collect::<Vec<_>>()
                .join("\n---\n");
            let ctx = SkillContext {
                skill: &skill,
                governance: &r.governance,
                commits: &commits,
            };
            let on_delta = |d: &str| task.ai_delta(gi as u64, d);
            let generated =
                ai::generate_with(&provider, &ctx, max_tokens, &task.cancel, Some(&on_delta)).await;
            let outcome = match generated {
                Ok(o) => o,
                Err(CoreError::Refused(msg)) => GenOutcome::Refusal { explanation: msg },
                Err(e) => return Err(e),
            };
            // Post-conditions applicatives — un prompt n'est pas une sécurité.
            let outcome = match skills::validate_outcome(&skill, &r.governance, &before, &outcome) {
                Ok(()) => outcome,
                Err(CoreError::Refused(msg)) => GenOutcome::Refusal { explanation: msg },
                Err(e) => return Err(e),
            };
            let (after, explanation, risk, status, removed) = match outcome {
                GenOutcome::Proposal {
                    message,
                    explanation,
                    risk,
                    removed,
                } => (
                    Some(message),
                    explanation,
                    risk,
                    ProposalStatus::Proposed,
                    removed,
                ),
                GenOutcome::Refusal { explanation } => (
                    None,
                    explanation,
                    skill.risk_default(),
                    ProposalStatus::Refused,
                    Vec::new(),
                ),
            };
            let p = Proposal {
                id: new_id("prp"),
                repo_id: repo_id.to_string(),
                skill: skill.def.name.clone(),
                skill_version: skill.def.version.clone(),
                targets: shas.clone(),
                before,
                after,
                explanation,
                risk,
                status,
                decision: None,
                created_at: now_iso(),
            };
            self.store.proposal_save(&p)?;
            self.audit(
                "skill",
                "proposal",
                &format!("{}:{}", r.name, skill.def.name),
                json!({
                    "proposal": p.id,
                    "provider": provider_label,
                    "targets": p.targets,
                    "status": p.status,
                    "retire": removed,
                }),
                "ok",
            );
            out.push(p);
        }
        Ok(out)
    }

    pub fn proposals_list(&self, repo_id: &str) -> Result<Vec<Proposal>> {
        self.store.proposal_list(repo_id)
    }

    /// Décision humaine sur une proposition. Un message édité repasse par les
    /// garde-fous de la skill (CA-7).
    pub fn proposal_decide(
        &self,
        proposal_id: &str,
        decision: &str,
        edited_message: Option<String>,
    ) -> Result<Proposal> {
        let mut all: Vec<Proposal> = Vec::new();
        for r in self.store.repo_list()? {
            all.extend(self.store.proposal_list(&r.id)?);
        }
        let mut p = all
            .into_iter()
            .find(|p| p.id == proposal_id)
            .ok_or_else(|| CoreError::NotFound(format!("proposition {proposal_id}")))?;
        let repo = self.store.repo_get(&p.repo_id)?;
        match decision {
            "accept" => {
                if p.after.is_none() {
                    return Err(CoreError::Refused(
                        "rien à accepter : la skill a refusé".into(),
                    ));
                }
                p.status = ProposalStatus::Accepted;
                p.decision = p.after.clone();
            }
            "edit" => {
                let msg = edited_message
                    .ok_or_else(|| CoreError::Invalid("message édité manquant".into()))?;
                let skill = self.skill_by_name(&p.skill)?;
                let outcome = GenOutcome::Proposal {
                    message: msg.clone(),
                    explanation: String::new(),
                    risk: p.risk,
                    removed: Vec::new(),
                };
                skills::validate_outcome(&skill, &repo.governance, &p.before, &outcome)?;
                p.status = ProposalStatus::Edited;
                p.decision = Some(msg);
            }
            "reject" => {
                p.status = ProposalStatus::Rejected;
                p.decision = None;
            }
            other => return Err(CoreError::Invalid(format!("décision inconnue : {other}"))),
        }
        self.store.proposal_save(&p)?;
        self.audit(
            "skill",
            "decision",
            &format!("{}:{}", repo.name, p.skill),
            json!({"proposal": p.id, "decision": decision}),
            "ok",
        );
        Ok(p)
    }

    /// Contenu YAML d'une skill (éditeur intégré, F8). Le chemin est résolu
    /// depuis le registre chargé — jamais depuis un chemin fourni par l'UI.
    pub fn skill_read(&self, name: &str) -> Result<String> {
        let skill = self.skill_by_name(name)?;
        Ok(std::fs::read_to_string(skill.dir.join("skill.yaml"))?)
    }

    /// Écrit une skill existante après validation : YAML parsable, `name`
    /// inchangé. Toute édition est journalisée.
    pub fn skill_write(&self, name: &str, content: &str) -> Result<()> {
        let skill = self.skill_by_name(name)?;
        let def: crate::skills::SkillDef = serde_yaml::from_str(content)
            .map_err(|e| CoreError::Invalid(format!("YAML invalide : {e}")))?;
        if def.name != name {
            return Err(CoreError::Invalid(format!(
                "le champ name (« {} ») doit rester « {name} » — renommer = créer une nouvelle skill",
                def.name
            )));
        }
        std::fs::write(skill.dir.join("skill.yaml"), content)?;
        self.audit(
            "skill",
            "edit",
            name,
            json!({"version": def.version, "status": def.status}),
            "ok",
        );
        Ok(())
    }

    /// Runner de tests de skills en mode déterministe (assistant local).
    pub fn skill_run_tests(&self, name: &str) -> Result<Vec<SkillTestResult>> {
        let skill = self.skill_by_name(name)?;
        let mut results = Vec::new();
        for case in &skill.test_cases {
            results.push(run_skill_case(&skill, case));
        }
        Ok(results)
    }

    // -- E5 : secrets & fournisseurs IA -------------------------------------

    pub fn required_scopes(&self, kind: CiKind) -> Vec<(String, String)> {
        secrets::required_scopes(kind)
    }

    pub fn ai_provider_save(
        &self,
        kind: AiProviderKind,
        base_url: Option<String>,
        model: Option<String>,
        api_key: Option<String>,
        is_default: bool,
    ) -> Result<AiProviderConfig> {
        let id = new_id("ai");
        let key_ref = match api_key {
            Some(k) if !k.trim().is_empty() => {
                let reference = format!("ai:{id}");
                secrets::set_secret(&reference, &k)?;
                Some(reference)
            }
            _ => None,
        };
        let cfg = AiProviderConfig {
            id,
            kind,
            base_url,
            model,
            key_ref,
            is_default,
        };
        if is_default {
            for mut other in self.store.ai_provider_list()? {
                if other.is_default {
                    other.is_default = false;
                    self.store.ai_provider_save(&other)?;
                }
            }
        }
        self.store.ai_provider_save(&cfg)?;
        self.audit(
            "secret",
            "ai_provider_save",
            &format!("{:?}", cfg.kind),
            json!({"id": cfg.id}),
            "ok",
        );
        Ok(cfg)
    }

    pub fn ai_provider_list(&self) -> Result<Vec<AiProviderConfig>> {
        self.store.ai_provider_list()
    }

    pub fn ai_provider_remove(&self, id: &str) -> Result<()> {
        if let Some(cfg) = self
            .store
            .ai_provider_list()?
            .into_iter()
            .find(|c| c.id == id)
        {
            if let Some(r) = &cfg.key_ref {
                secrets::delete_secret(r)?;
            }
        }
        self.store.ai_provider_remove(id)?;
        self.audit("secret", "ai_provider_remove", id, json!({}), "ok");
        Ok(())
    }

    // -- E6 : CI/CD ----------------------------------------------------------

    // Signature dictée par le formulaire IPC : chaque champ est distinct.
    #[allow(clippy::too_many_arguments)]
    pub async fn ci_account_add(
        &self,
        kind: CiKind,
        base_url: String,
        org: Option<String>,
        project: Option<String>,
        repo: Option<String>,
        token: String,
        scopes: Vec<String>,
    ) -> Result<(CiAccount, String)> {
        let id = new_id("acct");
        let token_ref = format!("ci:{id}");
        let account = CiAccount {
            id,
            kind,
            base_url,
            org,
            project,
            repo,
            token_ref: token_ref.clone(),
            scopes,
            added_at: now_iso(),
        };
        let client = CiClient::from_account(&account, token.clone())?;
        let validation = client.validate().await?;
        secrets::set_secret(&token_ref, &token)?;
        self.store.ci_account_save(&account)?;
        self.audit(
            "secret",
            "ci_account_add",
            &format!("{:?}:{}", account.kind, account.base_url),
            json!({"id": account.id, "scopes": account.scopes}),
            "ok",
        );
        Ok((account, validation))
    }

    pub fn ci_account_list(&self) -> Result<Vec<CiAccount>> {
        self.store.ci_account_list()
    }

    pub fn ci_account_remove(&self, id: &str) -> Result<()> {
        let a = self.store.ci_account_get(id)?;
        secrets::delete_secret(&a.token_ref)?;
        self.store.ci_account_remove(id)?;
        self.audit("secret", "ci_account_remove", &a.base_url, json!({}), "ok");
        Ok(())
    }

    fn client_for(&self, account_id: &str) -> Result<(CiAccount, CiClient)> {
        let account = self.store.ci_account_get(account_id)?;
        let token = secrets::get_secret(&account.token_ref)?;
        let client = CiClient::from_account(&account, token)?;
        Ok((account, client))
    }

    pub async fn ci_inventory(&self, account_id: &str, max: usize) -> Result<Vec<CiRun>> {
        self.ci_inventory_with(account_id, max, &TaskCtx::noop("ci_inventory"))
            .await
    }

    pub async fn ci_inventory_with(
        &self,
        account_id: &str,
        max: usize,
        ctx: &TaskCtx,
    ) -> Result<Vec<CiRun>> {
        let (_, client) = self.client_for(account_id)?;
        client.list_runs_with(max, ctx).await
    }

    pub fn policy_save(&self, name: String, rules: RetentionRules) -> Result<RetentionPolicy> {
        let p = RetentionPolicy {
            id: new_id("pol"),
            name,
            rules,
            enabled: true,
        };
        self.store.policy_save(&p)?;
        Ok(p)
    }

    pub fn policy_list(&self) -> Result<Vec<RetentionPolicy>> {
        self.store.policy_list()
    }

    /// Simulation : AUCUNE suppression, un rapport détaillé, journalisé.
    pub async fn ci_simulate(
        &self,
        account_id: &str,
        policy_id: &str,
        max: usize,
    ) -> Result<SimulationReport> {
        self.ci_simulate_with(account_id, policy_id, max, &TaskCtx::noop("ci_simulate"))
            .await
    }

    pub async fn ci_simulate_with(
        &self,
        account_id: &str,
        policy_id: &str,
        max: usize,
        ctx: &TaskCtx,
    ) -> Result<SimulationReport> {
        let (account, client) = self.client_for(account_id)?;
        let policy = self.store.policy_get(policy_id)?;
        let runs = client.list_runs_with(max, ctx).await?;
        ctx.step("application de la politique de rétention", 0, None)?;
        let report = ci::simulate(&policy, &account, &runs, chrono::Utc::now());
        let job = CleanupJob {
            id: new_id("job"),
            policy_id: policy.id.clone(),
            account_id: account.id.clone(),
            mode: JobMode::Simulation,
            status: "ok".into(),
            report: serde_json::to_value(&report)?,
            created_at: now_iso(),
            finished_at: Some(now_iso()),
        };
        self.store.job_save(&job)?;
        self.audit(
            "ci_cleanup",
            "simulate",
            &account.base_url,
            json!({
                "policy": policy.name,
                "total": report.total,
                "candidats": report.candidates.len(),
                "proteges": report.protected.len(),
            }),
            "ok",
        );
        Ok(report)
    }

    /// Suppression unitaire (CA-11/CA-12) : exige une simulation préalable du
    /// même périmètre contenant ce run, et la saisie du nom du pipeline.
    pub async fn ci_delete_run(
        &self,
        account_id: &str,
        policy_id: &str,
        run: CiRun,
        confirm: String,
    ) -> Result<()> {
        let (account, client) = self.client_for(account_id)?;
        let policy = self.store.policy_get(policy_id)?;
        let scope = ci::scope_hash(&account, &policy);
        let sim = self.store.last_simulation(&scope)?.ok_or_else(|| {
            CoreError::Refused(
                "aucune simulation préalable pour ce périmètre : exécuter la simulation d'abord"
                    .into(),
            )
        })?;
        if !sim.candidates.iter().any(|c| c.run_id == run.run_id) {
            return Err(CoreError::Refused(
                "ce run n'est pas dans les candidats du rapport de simulation".into(),
            ));
        }
        if confirm != run.pipeline_name {
            return Err(CoreError::ConfirmRequired {
                expected: run.pipeline_name.clone(),
                message: format!(
                    "confirmation invalide : saisir exactement « {} »",
                    run.pipeline_name
                ),
            });
        }
        // Journalisation AVANT l'appel de suppression (CA-11).
        self.audit(
            "ci_cleanup",
            "delete_attempt",
            &account.base_url,
            json!({"run": run.run_id, "pipeline": run.pipeline_name, "policy": policy.name}),
            "tentative",
        );
        let outcome = client.delete_run(&run).await;
        let result = match &outcome {
            Ok(()) => "ok".to_string(),
            Err(e) => format!("erreur : {e}"),
        };
        self.audit(
            "ci_cleanup",
            "delete",
            &account.base_url,
            json!({"run": run.run_id, "pipeline": run.pipeline_name}),
            &result,
        );
        outcome
    }

    /// Nettoyage CI EN MASSE (F7) : supprime chaque run du lot avec les mêmes
    /// garde-fous que la suppression unitaire (simulation préalable requise ;
    /// runs en cours / sous lease refusés côté client). Résiste au throttling
    /// (429/Retry-After → attente annulable puis reprise), journalise chaque
    /// run, émet la progression, et s'interrompt proprement sur annulation en
    /// renvoyant un point de reprise (`deleted`) pour relancer sans doublon.
    /// `confirm` doit valoir le NOMBRE de runs à supprimer (friction délibérée).
    pub async fn ci_delete_batch(
        &self,
        account_id: &str,
        policy_id: &str,
        runs: Vec<CiRun>,
        confirm: String,
        already_done: Vec<String>,
        ctx: &TaskCtx,
    ) -> Result<BatchDeleteResult> {
        let (account, client) = self.client_for(account_id)?;
        let policy = self.store.policy_get(policy_id)?;
        let scope = ci::scope_hash(&account, &policy);
        let sim = self.store.last_simulation(&scope)?.ok_or_else(|| {
            CoreError::Refused(
                "aucune simulation préalable pour ce périmètre : exécuter la simulation d'abord"
                    .into(),
            )
        })?;

        // B3 : le point de reprise est PERSISTÉ (survit à un crash/redémarrage) —
        // on repart du checkpoint en base fusionné avec ce que fournit l'appelant.
        let mut deleted: std::collections::HashSet<String> = already_done.into_iter().collect();
        deleted.extend(self.store.checkpoint_list(&scope)?);
        let pending: Vec<CiRun> = runs
            .into_iter()
            .filter(|r| !deleted.contains(&r.run_id))
            .collect();

        // Confirmation renforcée : saisir le nombre exact de runs restants.
        let expected = pending.len().to_string();
        if confirm != expected {
            return Err(CoreError::ConfirmRequired {
                expected: expected.clone(),
                message: format!(
                    "suppression en masse : saisir exactement « {expected} » (nombre de runs) pour confirmer"
                ),
            });
        }

        self.audit(
            "ci_cleanup",
            "batch_start",
            &account.base_url,
            json!({"policy": policy.name, "a_supprimer": pending.len(), "reprise": deleted.len()}),
            "début",
        );

        let total = pending.len() as u64;
        let mut failed: Vec<BatchFailure> = Vec::new();
        let mut cancelled = false;

        'lot: for (i, run) in pending.iter().enumerate() {
            if ctx.cancel.is_cancelled() {
                cancelled = true;
                break;
            }
            ctx.emit("suppression des runs", i as u64, Some(total));

            if !sim.candidates.iter().any(|c| c.run_id == run.run_id) {
                failed.push(BatchFailure {
                    run_id: run.run_id.clone(),
                    reason: "hors des candidats de la simulation".into(),
                });
                continue;
            }
            self.audit(
                "ci_cleanup",
                "delete_attempt",
                &account.base_url,
                json!({"run": run.run_id, "pipeline": run.pipeline_name, "lot": true}),
                "tentative",
            );

            // Throttling : jusqu'à 5 attentes sur 429/Retry-After.
            let mut throttles = 0u32;
            loop {
                match client.delete_run(run).await {
                    Ok(()) => {
                        deleted.insert(run.run_id.clone());
                        let _ = self.store.checkpoint_add(&scope, &run.run_id);
                        self.audit(
                            "ci_cleanup",
                            "delete",
                            &account.base_url,
                            json!({"run": run.run_id, "lot": true}),
                            "ok",
                        );
                        break;
                    }
                    Err(CoreError::RateLimited { retry_after_secs }) if throttles < 5 => {
                        throttles += 1;
                        ctx.emit(
                            &format!("throttling (429) — attente {retry_after_secs}s"),
                            i as u64,
                            Some(total),
                        );
                        if sleep_cancellable(retry_after_secs, &ctx.cancel)
                            .await
                            .is_err()
                        {
                            cancelled = true;
                            break 'lot;
                        }
                    }
                    Err(e) => {
                        self.audit(
                            "ci_cleanup",
                            "delete",
                            &account.base_url,
                            json!({"run": run.run_id, "lot": true}),
                            &format!("erreur : {e}"),
                        );
                        failed.push(BatchFailure {
                            run_id: run.run_id.clone(),
                            reason: e.to_string(),
                        });
                        break;
                    }
                }
            }
        }

        // B3 : lot ENTIÈREMENT traité (ni annulé ni échec) → efface le point de
        // reprise persistant. Sinon on le CONSERVE pour reprendre après coup.
        if !cancelled && failed.is_empty() {
            let _ = self.store.checkpoint_clear(&scope);
        }

        self.audit(
            "ci_cleanup",
            "batch_end",
            &account.base_url,
            json!({"supprimes": deleted.len(), "echecs": failed.len(), "annule": cancelled}),
            if cancelled { "annulé" } else { "ok" },
        );

        Ok(BatchDeleteResult {
            total: total as usize,
            deleted: deleted.into_iter().collect(),
            failed,
            cancelled,
        })
    }

    /// Purge des logs/artefacts CI (F7, extension) : reclaim de STOCKAGE qui
    /// CONSERVE les runs (contrairement à la suppression). Un run en cours est
    /// toujours ignoré. Confirmation par le NOMBRE de runs ciblés ; progression
    /// et annulation coopérative (résultat partiel). Les échecs par run sont
    /// collectés sans interrompre le lot. GitHub uniquement (AzDO renvoie une
    /// erreur par run, collectée comme échec).
    pub async fn ci_purge_assets(
        &self,
        account_id: &str,
        runs: Vec<CiRun>,
        purge_logs: bool,
        purge_artifacts: bool,
        confirm: String,
        ctx: &TaskCtx,
    ) -> Result<PurgeResult> {
        if !purge_logs && !purge_artifacts {
            return Err(CoreError::Invalid(
                "rien à purger : activer les logs et/ou les artefacts".into(),
            ));
        }
        let (account, client) = self.client_for(account_id)?;

        // Un run en cours d'exécution n'est jamais purgé.
        let targets: Vec<CiRun> = runs.into_iter().filter(|r| !r.running).collect();
        let expected = targets.len().to_string();
        if confirm != expected {
            return Err(CoreError::ConfirmRequired {
                expected: expected.clone(),
                message: format!(
                    "purge des logs/artefacts : saisir exactement « {expected} » (nombre de runs) pour confirmer"
                ),
            });
        }

        self.audit(
            "ci_cleanup",
            "purge_start",
            &account.base_url,
            json!({"runs": targets.len(), "logs": purge_logs, "artefacts": purge_artifacts}),
            "début",
        );

        let total = targets.len() as u64;
        let mut res = PurgeResult {
            runs: 0,
            artifacts_deleted: 0,
            logs_deleted: 0,
            failed: Vec::new(),
            cancelled: false,
        };

        'lot: for (i, run) in targets.iter().enumerate() {
            if ctx.cancel.is_cancelled() {
                res.cancelled = true;
                break;
            }
            ctx.emit("purge des logs/artefacts", i as u64, Some(total));
            self.audit(
                "ci_cleanup",
                "purge_attempt",
                &account.base_url,
                json!({"run": run.run_id, "logs": purge_logs, "artefacts": purge_artifacts}),
                "tentative",
            );

            let mut run_errors: Vec<String> = Vec::new();
            let mut run_arts = 0usize;
            let mut run_logs = 0usize;

            if purge_artifacts {
                match client.run_artifacts(run).await {
                    Ok(arts) => {
                        for a in arts {
                            // Un essai + une reprise sur 429 (attente annulable).
                            for attempt in 0..2 {
                                match client.delete_artifact(run, &a.id).await {
                                    Ok(()) => {
                                        run_arts += 1;
                                        break;
                                    }
                                    Err(CoreError::RateLimited { retry_after_secs })
                                        if attempt == 0 =>
                                    {
                                        ctx.emit(
                                            &format!(
                                                "throttling (429) — attente {retry_after_secs}s"
                                            ),
                                            i as u64,
                                            Some(total),
                                        );
                                        if sleep_cancellable(retry_after_secs, &ctx.cancel)
                                            .await
                                            .is_err()
                                        {
                                            res.cancelled = true;
                                            break 'lot;
                                        }
                                    }
                                    Err(e) => {
                                        run_errors.push(format!("artefact {} : {e}", a.name));
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => run_errors.push(format!("liste des artefacts : {e}")),
                }
            }

            if purge_logs {
                for attempt in 0..2 {
                    match client.delete_run_logs(run).await {
                        Ok(()) => {
                            run_logs += 1;
                            break;
                        }
                        Err(CoreError::RateLimited { retry_after_secs }) if attempt == 0 => {
                            ctx.emit(
                                &format!("throttling (429) — attente {retry_after_secs}s"),
                                i as u64,
                                Some(total),
                            );
                            if sleep_cancellable(retry_after_secs, &ctx.cancel)
                                .await
                                .is_err()
                            {
                                res.cancelled = true;
                                break 'lot;
                            }
                        }
                        Err(e) => {
                            run_errors.push(format!("logs : {e}"));
                            break;
                        }
                    }
                }
            }

            res.artifacts_deleted += run_arts;
            res.logs_deleted += run_logs;
            res.runs += 1;
            let outcome = if run_errors.is_empty() {
                "ok".to_string()
            } else {
                res.failed.push(BatchFailure {
                    run_id: run.run_id.clone(),
                    reason: run_errors.join(" ; "),
                });
                "partiel".to_string()
            };
            self.audit(
                "ci_cleanup",
                "purge",
                &account.base_url,
                json!({"run": run.run_id, "artefacts": run_arts, "logs": run_logs}),
                &outcome,
            );
        }

        self.audit(
            "ci_cleanup",
            "purge_end",
            &account.base_url,
            json!({
                "runs": res.runs,
                "artefacts": res.artifacts_deleted,
                "logs": res.logs_deleted,
                "echecs": res.failed.len(),
                "annule": res.cancelled,
            }),
            if res.cancelled { "annulé" } else { "ok" },
        );

        Ok(res)
    }

    pub fn job_list(&self) -> Result<Vec<CleanupJob>> {
        self.store.job_list()
    }

    // -- E7 : audit ----------------------------------------------------------

    pub fn audit_list(&self, limit: u32) -> Result<Vec<AuditEvent>> {
        self.store.audit_list(limit)
    }

    pub fn audit_export(&self) -> Result<String> {
        self.store.audit_export_jsonl()
    }

    // utilitaire pour l'UI : hash courant d'un plan (détection de dérive)
    pub fn plan_current_hash(&self, plan_id: &str) -> Result<String> {
        let p = self.store.plan_get(plan_id)?;
        Ok(plan_hash(&p.fingerprint, &p.ops))
    }
}

/// Exécute un cas de test de skill en mode déterministe (assistant local).
fn run_skill_case(skill: &Skill, case: &skills::SkillTestCase) -> SkillTestResult {
    use serde_yaml::Value as Y;

    let g = &case.given;
    let mut governance = Governance::default();
    if let Some(gov) = g.get("governance") {
        if let Some(p) = gov.get("ai_attribution_policy").and_then(Y::as_str) {
            governance.ai_attribution_policy = if p == "keep-required" {
                AiAttributionPolicy::KeepRequired
            } else {
                AiAttributionPolicy::NormalizationAllowed
            };
        }
    }
    let given_files: Vec<String> = g
        .get("files")
        .and_then(Y::as_sequence)
        .map(|s| {
            s.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let mk_commit = |subject: String, body: String, files: Vec<String>| CommitInfo {
        sha: "0000000000000000000000000000000000000000".into(),
        short: "00000000".into(),
        parents: vec![],
        author_name: "test".into(),
        author_email: "test@example.org".into(),
        date: String::new(),
        trailers: GitEngine::parse_trailers(&format!("{subject}\n\n{body}")),
        subject,
        body,
        is_merge: false,
        signed: false,
        on_remote: false,
        files_changed: files.len().max(1),
        insertions: 10,
        deletions: 2,
        files,
    };
    let commits: Vec<CommitInfo> = if let Some(msgs) = g.get("messages").and_then(Y::as_sequence) {
        msgs.iter()
            .map(|m| {
                let full = match m {
                    Y::String(s) => s.clone(),
                    other => serde_yaml::to_string(other).unwrap_or_default(),
                };
                let (subject, body) = full.split_once('\n').unwrap_or((full.as_str(), ""));
                mk_commit(
                    subject.trim().to_string(),
                    body.trim().to_string(),
                    Vec::new(),
                )
            })
            .collect()
    } else {
        let subject = g
            .get("subject")
            .and_then(Y::as_str)
            .unwrap_or("")
            .to_string();
        let body = g
            .get("body")
            .and_then(Y::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        vec![mk_commit(subject, body, given_files)]
    };

    let ctx = SkillContext {
        skill,
        governance: &governance,
        commits: &commits,
    };
    let outcome = match crate::ai::rule_based::generate(&ctx) {
        Ok(o) => o,
        Err(e) => {
            return SkillTestResult {
                case: case.name.clone(),
                passed: false,
                detail: format!("génération impossible : {e}"),
            }
        }
    };
    let before = ctx.before();
    let outcome = match skills::validate_outcome(skill, &governance, &before, &outcome) {
        Ok(()) => outcome,
        Err(CoreError::Refused(msg)) => GenOutcome::Refusal { explanation: msg },
        Err(e) => {
            return SkillTestResult {
                case: case.name.clone(),
                passed: false,
                detail: e.to_string(),
            }
        }
    };

    let e = &case.expect;
    let message = match &outcome {
        GenOutcome::Proposal { message, .. } => Some(message.clone()),
        GenOutcome::Refusal { .. } => None,
    };
    let mut failures: Vec<String> = Vec::new();
    let mut checked = 0;
    if let Some(want) = e.get("must_refuse").and_then(Y::as_bool) {
        checked += 1;
        if want != message.is_none() {
            failures.push(format!("must_refuse={want} non satisfait"));
        }
    }
    if let Some(sub) = e.get("contains").and_then(Y::as_str) {
        checked += 1;
        if !message.as_deref().unwrap_or("").contains(sub) {
            failures.push(format!("contains « {sub} » absent"));
        }
    }
    if let Some(sub) = e.get("not_contains").and_then(Y::as_str) {
        checked += 1;
        if message.as_deref().unwrap_or("").contains(sub) {
            failures.push(format!("not_contains « {sub} » présent"));
        }
    }
    if let Some(pat) = e.get("matches").and_then(Y::as_str) {
        checked += 1;
        match regex::Regex::new(pat) {
            Ok(re) => {
                let subject = message
                    .as_deref()
                    .unwrap_or("")
                    .lines()
                    .next()
                    .unwrap_or("");
                if !re.is_match(subject) {
                    failures.push(format!("matches « {pat} » non satisfait ({subject})"));
                }
            }
            Err(err) => failures.push(format!("pattern invalide : {err}")),
        }
    }
    if checked == 0 {
        return SkillTestResult {
            case: case.name.clone(),
            passed: true,
            detail: "aucune assertion exécutable (cas informatif)".into(),
        };
    }
    SkillTestResult {
        case: case.name.clone(),
        passed: failures.is_empty(),
        detail: if failures.is_empty() {
            "ok".into()
        } else {
            failures.join(" ; ")
        },
    }
}
