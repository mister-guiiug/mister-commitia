pub mod azdo;
pub mod github;

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};

use crate::error::{CoreError, Result};
use crate::model::*;
use crate::task::TaskCtx;

pub enum CiClient {
    Github(github::GithubCi),
    AzDo(azdo::AzDoCi),
}

impl CiClient {
    pub fn from_account(account: &CiAccount, token: String) -> Result<Self> {
        match account.kind {
            CiKind::Github | CiKind::GithubEnterprise => {
                Ok(CiClient::Github(github::GithubCi::new(account, token)?))
            }
            CiKind::AzureDevops | CiKind::AzureDevopsServer => {
                Ok(CiClient::AzDo(azdo::AzDoCi::new(account, token)?))
            }
        }
    }

    pub async fn validate(&self) -> Result<String> {
        match self {
            CiClient::Github(c) => c.validate().await,
            CiClient::AzDo(c) => c.validate().await,
        }
    }

    pub async fn list_runs(&self, max: usize) -> Result<Vec<CiRun>> {
        self.list_runs_with(max, &TaskCtx::noop("ci_inventory"))
            .await
    }

    /// Inventaire paginé avec progression (une page = un point d'arrêt sûr).
    pub async fn list_runs_with(&self, max: usize, ctx: &TaskCtx) -> Result<Vec<CiRun>> {
        match self {
            CiClient::Github(c) => c.list_runs(max, ctx).await,
            CiClient::AzDo(c) => c.list_runs(max, ctx).await,
        }
    }

    /// Pull requests ouvertes dont la source est `branch` (push assisté, F4).
    /// Azure DevOps n'est pas couvert ici (API distincte).
    pub async fn list_open_prs(&self, branch: &str) -> Result<Vec<PrRef>> {
        match self {
            CiClient::Github(c) => c.list_open_prs(branch).await,
            CiClient::AzDo(_) => Err(CoreError::Invalid(
                "détection des PR non implémentée pour Azure DevOps".into(),
            )),
        }
    }

    /// Suppression UNITAIRE. Refuse en amont les runs protégés ; vérifie les
    /// leases côté Azure DevOps immédiatement avant l'appel.
    pub async fn delete_run(&self, run: &CiRun) -> Result<()> {
        if run.running {
            return Err(CoreError::Refused(
                "run en cours d'exécution : suppression refusée".into(),
            ));
        }
        if run.leased {
            return Err(CoreError::Refused(
                "run retenu par une rétention (lease) : suppression refusée".into(),
            ));
        }
        match self {
            CiClient::Github(c) => c.delete_run(run).await,
            CiClient::AzDo(c) => c.delete_run(run).await,
        }
    }

    /// Artefacts d'un run (F7). Azure DevOps n'est pas couvert par ce chemin.
    pub async fn run_artifacts(&self, run: &CiRun) -> Result<Vec<CiArtifact>> {
        match self {
            CiClient::Github(c) => c.run_artifacts(&run.run_id).await,
            CiClient::AzDo(_) => Err(CoreError::Invalid(
                "purge des artefacts non implémentée pour Azure DevOps".into(),
            )),
        }
    }

    /// Supprime un artefact par id (F7).
    pub async fn delete_artifact(&self, artifact_id: &str) -> Result<()> {
        match self {
            CiClient::Github(c) => c.delete_artifact(artifact_id).await,
            CiClient::AzDo(_) => Err(CoreError::Invalid(
                "purge des artefacts non implémentée pour Azure DevOps".into(),
            )),
        }
    }

    /// Supprime les logs d'un run (F7).
    pub async fn delete_run_logs(&self, run: &CiRun) -> Result<()> {
        match self {
            CiClient::Github(c) => c.delete_run_logs(&run.run_id).await,
            CiClient::AzDo(_) => Err(CoreError::Invalid(
                "purge des logs non implémentée pour Azure DevOps".into(),
            )),
        }
    }
}

/// Interprète une réponse HTTP de plateforme en erreur exploitable :
/// limite de débit (Retry-After / x-ratelimit-*), permission, absence.
pub(crate) fn platform_error(
    status: reqwest::StatusCode,
    headers: &reqwest::header::HeaderMap,
    body: &str,
    what: &str,
) -> CoreError {
    let retry_after = headers
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    let remaining_zero = headers
        .get("x-ratelimit-remaining")
        .and_then(|v| v.to_str().ok())
        .map(|v| v == "0")
        .unwrap_or(false);
    if status.as_u16() == 429 || (status.as_u16() == 403 && remaining_zero) {
        let secs = retry_after.or_else(|| {
            headers
                .get("x-ratelimit-reset")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<i64>().ok())
                .map(|reset| (reset - Utc::now().timestamp()).max(1) as u64)
        });
        return CoreError::RateLimited {
            retry_after_secs: secs.unwrap_or(60),
        };
    }
    match status.as_u16() {
        401 => CoreError::Refused(format!(
            "{what} : jeton invalide ou expiré (401) — vérifier/renouveler le token"
        )),
        403 => CoreError::Refused(format!(
            "{what} : permission insuffisante (403) — vérifier le scope du token (GitHub : « Actions: write » ; Azure DevOps : scope Build read & execute + permission « Delete builds ») : {body}"
        )),
        404 => CoreError::NotFound(format!("{what} : ressource absente (déjà supprimée ?)")),
        _ => CoreError::Http(format!("{what} : HTTP {status} — {body}")),
    }
}

pub fn scope_hash(account: &CiAccount, policy: &RetentionPolicy) -> String {
    let mut h = Sha256::new();
    h.update(account.id.as_bytes());
    h.update(
        serde_json::to_string(&policy.rules)
            .unwrap_or_default()
            .as_bytes(),
    );
    format!("{:x}", h.finalize())
}

/// Moteur de politiques : PURE FONCTION → simulation testable sans réseau.
/// Aucune suppression n'est jamais décidée ici, seulement classée.
pub fn simulate(
    policy: &RetentionPolicy,
    account: &CiAccount,
    runs: &[CiRun],
    now: DateTime<Utc>,
) -> SimulationReport {
    let mut protected: Vec<ProtectedRun> = Vec::new();
    let mut candidates: Vec<CiRun> = Vec::new();
    let mut kept_recent = 0usize;

    // Rang de fraîcheur par pipeline (0 = plus récent).
    let mut by_pipeline: std::collections::HashMap<&str, Vec<&CiRun>> = Default::default();
    for r in runs {
        by_pipeline
            .entry(r.pipeline_id.as_str())
            .or_default()
            .push(r);
    }
    for list in by_pipeline.values_mut() {
        list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    }
    let rank = |r: &CiRun| -> usize {
        by_pipeline
            .get(r.pipeline_id.as_str())
            .and_then(|l| l.iter().position(|x| x.run_id == r.run_id))
            .unwrap_or(0)
    };

    for r in runs {
        if r.running {
            protected.push(ProtectedRun {
                run: r.clone(),
                reason: "en cours d'exécution".into(),
            });
            continue;
        }
        if r.leased {
            protected.push(ProtectedRun {
                run: r.clone(),
                reason: "retenu par une rétention (lease/keep-forever)".into(),
            });
            continue;
        }
        if let Some(b) = &r.branch {
            if policy.rules.protect_branches.iter().any(|p| p == b) {
                protected.push(ProtectedRun {
                    run: r.clone(),
                    reason: format!("branche protégée ({b})"),
                });
                continue;
            }
        }
        if policy.rules.protect_failed && r.result.as_deref() == Some("failure") {
            protected.push(ProtectedRun {
                run: r.clone(),
                reason: "échec conservé pour analyse".into(),
            });
            continue;
        }
        if rank(r) < policy.rules.keep_last_per_pipeline as usize {
            kept_recent += 1;
            continue;
        }
        if let Some(max_age) = policy.rules.max_age_days {
            let created = DateTime::parse_from_rfc3339(&r.created_at)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or(now);
            let age_days = (now - created).num_days();
            if age_days <= max_age as i64 {
                kept_recent += 1;
                continue;
            }
        } else {
            // Sans règle d'âge, seuls les runs au-delà du quota « derniers N » sont candidats.
        }
        candidates.push(r.clone());
    }

    SimulationReport {
        id: new_id("sim"),
        policy_id: policy.id.clone(),
        account_id: account.id.clone(),
        generated_at: now_iso(),
        total: runs.len(),
        candidates,
        protected,
        kept_recent,
        scope_hash: scope_hash(account, policy),
    }
}
