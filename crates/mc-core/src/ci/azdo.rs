use base64::Engine as _;
use serde_json::Value;

use crate::error::{CoreError, Result};
use crate::model::{CiAccount, CiRun};
use crate::task::TaskCtx;

use super::platform_error;

/// Client Azure DevOps (cloud : https://dev.azure.com/{org} ; Server :
/// https://serveur/{collection}). API Builds, api-version 7.1.
/// La suppression d'un run passe par Builds–Delete (l'API Pipelines Runs n'a
/// pas d'opération DELETE, cf. docs/08-apis-plateformes.md).
pub struct AzDoCi {
    base: String,
    project: String,
    token: String,
    account_id: String,
    client: reqwest::Client,
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

    pub async fn validate(&self) -> Result<String> {
        let url = format!(
            "{}/{}/_apis/build/builds?$top=1&api-version=7.1",
            self.base, self.project
        );
        let resp = self.req(reqwest::Method::GET, url).send().await?;
        let status = resp.status();
        let headers = resp.headers().clone();
        let body = resp.text().await.unwrap_or_default();
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
            let mut url = format!(
                "{}/{}/_apis/build/builds?$top=100&queryOrder=queueTimeDescending&api-version=7.1",
                self.base, self.project
            );
            if let Some(c) = &continuation {
                url.push_str(&format!("&continuationToken={c}"));
            }
            let resp = self.req(reqwest::Method::GET, url).send().await?;
            let status = resp.status();
            let headers = resp.headers().clone();
            let body = resp.text().await.unwrap_or_default();
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
        let url = format!(
            "{}/{}/_apis/build/builds/{}/leases?api-version=7.1",
            self.base, self.project, run_id
        );
        let resp = self.req(reqwest::Method::GET, url).send().await?;
        let status = resp.status();
        let headers = resp.headers().clone();
        let body = resp.text().await.unwrap_or_default();
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
        let url = format!(
            "{}/{}/_apis/build/builds/{}?api-version=7.1",
            self.base, self.project, run.run_id
        );
        let resp = self.req(reqwest::Method::DELETE, url).send().await?;
        let status = resp.status();
        let headers = resp.headers().clone();
        let body = resp.text().await.unwrap_or_default();
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
