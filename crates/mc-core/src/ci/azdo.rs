use base64::Engine as _;
use serde_json::Value;

use crate::error::{CoreError, Result};
use crate::model::{CiAccount, CiRun};
use crate::task::TaskCtx;

use super::platform_error;

/// Client Azure DevOps (cloud : https://dev.azure.com/{org} ; Server :
/// https://serveur/{collection}). API Builds.
///
/// L'api-version est NÉGOCIÉE (F12) : on tente 7.1 (cloud et Server récents) et
/// on se rabat une fois sur 7.0 si le serveur la déclare hors plage — cas d'un
/// Azure DevOps Server on-prem plus ancien. Le choix est mémorisé pour les
/// appels suivants du même client.
///
/// La suppression d'un run passe par Builds–Delete (l'API Pipelines Runs n'a
/// pas d'opération DELETE, cf. docs/08-apis-plateformes.md).
pub struct AzDoCi {
    base: String,
    project: String,
    token: String,
    account_id: String,
    client: reqwest::Client,
    /// api-version effective, négociée à la volée (7.1 → 7.0).
    api_version: std::sync::Mutex<String>,
}

impl AzDoCi {
    pub fn new(account: &CiAccount, token: String) -> Result<Self> {
        let project = account
            .project
            .clone()
            .ok_or_else(|| CoreError::Invalid("projet Azure DevOps manquant".into()))?;
        Ok(Self {
            base: account.base_url.trim_end_matches('/').to_string(),
            project,
            token,
            account_id: account.id.clone(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()?,
            api_version: std::sync::Mutex::new("7.1".to_string()),
        })
    }

    fn auth(&self) -> String {
        let raw = format!(":{}", self.token);
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(raw)
        )
    }

    fn req(&self, method: reqwest::Method, url: String) -> reqwest::RequestBuilder {
        self.client
            .request(method, url)
            .header("Authorization", self.auth())
            .header("Accept", "application/json")
    }

    fn ver(&self) -> String {
        self.api_version.lock().unwrap().clone()
    }

    fn downgrade(&self) {
        *self.api_version.lock().unwrap() = "7.0".to_string();
    }

    /// Détecte une erreur « api-version hors plage » d'Azure DevOps Server : un
    /// HTTP 400 dont le message cite une version non supportée. C'est un rejet
    /// franc (rien n'a été exécuté côté serveur) — rejouer la requête, même un
    /// DELETE, avec une version inférieure est donc sûr (aucune double action).
    fn is_version_error(status: reqwest::StatusCode, body: &str) -> bool {
        if status.as_u16() != 400 {
            return false;
        }
        let b = body.to_lowercase();
        b.contains("version") && (b.contains("range") || b.contains("supported"))
    }

    /// Envoie une requête en négociant l'api-version. `suffix` est la partie
    /// d'URL après `{base}/{project}` et contient le jeton `{VER}` à la place de
    /// la valeur d'api-version. Sur erreur de version, se rabat une fois sur 7.0
    /// et mémorise le choix.
    async fn send(
        &self,
        method: reqwest::Method,
        suffix: &str,
    ) -> Result<(reqwest::StatusCode, reqwest::header::HeaderMap, String)> {
        let url = |ver: &str| {
            format!(
                "{}/{}{}",
                self.base,
                self.project,
                suffix.replace("{VER}", ver)
            )
        };
        let ver = self.ver();
        let resp = self.req(method.clone(), url(&ver)).send().await?;
        let status = resp.status();
        let headers = resp.headers().clone();
        let body = resp.text().await.unwrap_or_default();
        if ver != "7.0" && Self::is_version_error(status, &body) {
            self.downgrade();
            let resp = self.req(method, url("7.0")).send().await?;
            let status = resp.status();
            let headers = resp.headers().clone();
            let body = resp.text().await.unwrap_or_default();
            return Ok((status, headers, body));
        }
        Ok((status, headers, body))
    }

    pub async fn validate(&self) -> Result<String> {
        let (status, headers, body) = self
            .send(
                reqwest::Method::GET,
                "/_apis/build/builds?$top=1&api-version={VER}",
            )
            .await?;
        if !status.is_success() {
            return Err(platform_error(
                status,
                &headers,
                &body,
                "validation Azure DevOps",
            ));
        }
        Ok(format!(
            "accès en lecture au projet {} confirmé",
            self.project
        ))
    }

    pub async fn list_runs(&self, max: usize, ctx: &TaskCtx) -> Result<Vec<CiRun>> {
        let mut out = Vec::new();
        let mut continuation: Option<String> = None;
        loop {
            ctx.step("inventaire des builds", out.len() as u64, Some(max as u64))?;
            let mut suffix =
                "/_apis/build/builds?$top=100&queryOrder=queueTimeDescending&api-version={VER}"
                    .to_string();
            if let Some(c) = &continuation {
                suffix.push_str(&format!("&continuationToken={c}"));
            }
            let (status, headers, body) = self.send(reqwest::Method::GET, &suffix).await?;
            if !status.is_success() {
                return Err(platform_error(
                    status,
                    &headers,
                    &body,
                    "inventaire Azure DevOps",
                ));
            }
            let next = headers
                .get("x-ms-continuationtoken")
                .and_then(|v| v.to_str().ok())
                .map(String::from);
            let v: Value = serde_json::from_str(&body)?;
            let builds = v["value"].as_array().cloned().unwrap_or_default();
            if builds.is_empty() {
                break;
            }
            for b in &builds {
                let status_s = b["status"].as_str().unwrap_or("").to_string();
                let running = matches!(
                    status_s.as_str(),
                    "inProgress" | "notStarted" | "postponed" | "cancelling"
                );
                // `keepForever` / `retainedByRelease` signalent une rétention
                // côté inventaire ; les leases exactes sont revérifiées avant
                // toute suppression (delete_run).
                let leased = b["keepForever"].as_bool().unwrap_or(false)
                    || b["retainedByRelease"].as_bool().unwrap_or(false);
                out.push(CiRun {
                    account_id: self.account_id.clone(),
                    pipeline_id: b["definition"]["id"]
                        .as_i64()
                        .map(|x| x.to_string())
                        .unwrap_or_default(),
                    pipeline_name: b["definition"]["name"]
                        .as_str()
                        .unwrap_or("(pipeline)")
                        .to_string(),
                    run_id: b["id"].as_i64().map(|x| x.to_string()).unwrap_or_default(),
                    status: status_s,
                    result: b["result"].as_str().map(String::from),
                    branch: b["sourceBranch"]
                        .as_str()
                        .map(|s| s.trim_start_matches("refs/heads/").to_string()),
                    created_at: b["queueTime"].as_str().unwrap_or("").to_string(),
                    url: b["_links"]["web"]["href"].as_str().map(String::from),
                    leased,
                    running,
                });
                if out.len() >= max {
                    return Ok(out);
                }
            }
            match next {
                Some(c) => continuation = Some(c),
                None => break,
            }
        }
        Ok(out)
    }

    /// Leases actives d'un run — revérifiées juste avant suppression (CA-12).
    pub async fn run_leases(&self, run_id: &str) -> Result<usize> {
        let suffix = format!("/_apis/build/builds/{run_id}/leases?api-version={{VER}}");
        let (status, headers, body) = self.send(reqwest::Method::GET, &suffix).await?;
        if !status.is_success() {
            return Err(platform_error(
                status,
                &headers,
                &body,
                "lecture des leases",
            ));
        }
        let v: Value = serde_json::from_str(&body)?;
        Ok(v["value"].as_array().map(|a| a.len()).unwrap_or(0))
    }

    pub async fn delete_run(&self, run: &CiRun) -> Result<()> {
        let leases = self.run_leases(&run.run_id).await?;
        if leases > 0 {
            return Err(CoreError::Refused(format!(
                "run {} retenu par {} lease(s) de rétention : suppression refusée (jamais de libération implicite)",
                run.run_id, leases
            )));
        }
        let suffix = format!("/_apis/build/builds/{}?api-version={{VER}}", run.run_id);
        let (status, headers, body) = self.send(reqwest::Method::DELETE, &suffix).await?;
        if !status.is_success() {
            return Err(platform_error(
                status,
                &headers,
                &body,
                "suppression du build Azure DevOps",
            ));
        }
        Ok(())
    }
}
