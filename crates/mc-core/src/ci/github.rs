use serde_json::Value;

use crate::error::{CoreError, Result};
use crate::model::{CiAccount, CiRun, PrRef};
use crate::task::TaskCtx;

use super::platform_error;

/// Client GitHub Actions (GitHub.com : base https://api.github.com ;
/// GitHub Enterprise Server : https://hote/api/v3). Version d'API 2022-11-28.
pub struct GithubCi {
    base: String,
    owner: String,
    repo: String,
    token: String,
    account_id: String,
    client: reqwest::Client,
}

impl GithubCi {
    pub fn new(account: &CiAccount, token: String) -> Result<Self> {
        let owner = account
            .org
            .clone()
            .ok_or_else(|| CoreError::Invalid("owner/organisation manquant".into()))?;
        let repo = account
            .repo
            .clone()
            .ok_or_else(|| CoreError::Invalid("dépôt manquant".into()))?;
        Ok(Self {
            base: account.base_url.trim_end_matches('/').to_string(),
            owner,
            repo,
            token,
            account_id: account.id.clone(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()?,
        })
    }

    fn req(&self, method: reqwest::Method, url: String) -> reqwest::RequestBuilder {
        self.client
            .request(method, url)
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", "mister-commitia")
    }

    pub async fn validate(&self) -> Result<String> {
        let url = format!("{}/repos/{}/{}", self.base, self.owner, self.repo);
        let resp = self.req(reqwest::Method::GET, url).send().await?;
        let status = resp.status();
        let headers = resp.headers().clone();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(platform_error(status, &headers, &body, "validation GitHub"));
        }
        Ok(format!(
            "accès en lecture à {}/{} confirmé",
            self.owner, self.repo
        ))
    }

    pub async fn list_runs(&self, max: usize, ctx: &TaskCtx) -> Result<Vec<CiRun>> {
        let mut out = Vec::new();
        let mut page = 1u32;
        while out.len() < max {
            ctx.step("inventaire des runs", out.len() as u64, Some(max as u64))?;
            let url = format!(
                "{}/repos/{}/{}/actions/runs?per_page=100&page={page}",
                self.base, self.owner, self.repo
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
                    "inventaire GitHub Actions",
                ));
            }
            let v: Value = serde_json::from_str(&body)?;
            let runs = v["workflow_runs"].as_array().cloned().unwrap_or_default();
            if runs.is_empty() {
                break;
            }
            for r in &runs {
                let status_s = r["status"].as_str().unwrap_or("").to_string();
                let running = matches!(
                    status_s.as_str(),
                    "in_progress" | "queued" | "requested" | "waiting" | "pending"
                );
                out.push(CiRun {
                    account_id: self.account_id.clone(),
                    pipeline_id: r["workflow_id"]
                        .as_i64()
                        .map(|x| x.to_string())
                        .unwrap_or_default(),
                    pipeline_name: r["name"].as_str().unwrap_or("(workflow)").to_string(),
                    run_id: r["id"].as_i64().map(|x| x.to_string()).unwrap_or_default(),
                    status: status_s,
                    result: r["conclusion"].as_str().map(String::from),
                    branch: r["head_branch"].as_str().map(String::from),
                    created_at: r["created_at"].as_str().unwrap_or("").to_string(),
                    url: r["html_url"].as_str().map(String::from),
                    leased: false,
                    running,
                });
                if out.len() >= max {
                    break;
                }
            }
            if runs.len() < 100 {
                break;
            }
            page += 1;
        }
        Ok(out)
    }

    /// PR ouvertes dont la branche source est `branch` (head = owner:branch).
    pub async fn list_open_prs(&self, branch: &str) -> Result<Vec<PrRef>> {
        let url = format!("{}/repos/{}/{}/pulls", self.base, self.owner, self.repo);
        let head = format!("{}:{}", self.owner, branch);
        let resp = self
            .req(reqwest::Method::GET, url)
            .query(&[("state", "open"), ("head", head.as_str())])
            .send()
            .await?;
        let status = resp.status();
        let headers = resp.headers().clone();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(platform_error(
                status,
                &headers,
                &body,
                "liste des PR GitHub",
            ));
        }
        let v: Value = serde_json::from_str(&body)?;
        Ok(v.as_array()
            .map(|arr| {
                arr.iter()
                    .map(|p| PrRef {
                        number: p["number"].as_u64().unwrap_or(0),
                        title: p["title"].as_str().unwrap_or("").to_string(),
                        url: p["html_url"].as_str().unwrap_or("").to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    pub async fn delete_run(&self, run: &CiRun) -> Result<()> {
        let url = format!(
            "{}/repos/{}/{}/actions/runs/{}",
            self.base, self.owner, self.repo, run.run_id
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
                "suppression du run GitHub",
            ));
        }
        Ok(())
    }
}
