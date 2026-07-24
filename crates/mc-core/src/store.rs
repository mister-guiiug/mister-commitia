use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::{CoreError, Result};
use crate::model::*;

/// Stockage local (SQLite). Ne contient jamais de secret : uniquement des
/// alias (`token_ref`, `key_ref`) vers le coffre du système d'exploitation.
/// La connexion est sous mutex pour rendre le Store `Sync` (commandes async).
pub struct Store {
    conn: Mutex<Connection>,
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS repos (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    local_path TEXT NOT NULL,
    remote_url TEXT,
    default_branch TEXT,
    protected_branches TEXT NOT NULL DEFAULT '[]',
    governance TEXT NOT NULL DEFAULT '{}',
    added_at TEXT NOT NULL,
    last_scanned_at TEXT
);
CREATE TABLE IF NOT EXISTS plans (
    id TEXT PRIMARY KEY,
    repo_id TEXT NOT NULL,
    data TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS proposals (
    id TEXT PRIMARY KEY,
    repo_id TEXT NOT NULL,
    data TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS ci_accounts (
    id TEXT PRIMARY KEY,
    data TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS policies (
    id TEXT PRIMARY KEY,
    data TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS cleanup_jobs (
    id TEXT PRIMARY KEY,
    policy_id TEXT NOT NULL,
    account_id TEXT NOT NULL,
    mode TEXT NOT NULL,
    data TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS audit (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    ts TEXT NOT NULL,
    actor TEXT NOT NULL,
    category TEXT NOT NULL,
    action TEXT NOT NULL,
    target TEXT NOT NULL,
    params TEXT NOT NULL,
    result TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS ai_providers (
    id TEXT PRIMARY KEY,
    data TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL").ok();
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().expect("verrou SQLite empoisonné")
    }

    // -- repos ---------------------------------------------------------------

    pub fn repo_add(&self, r: &RepoRef) -> Result<()> {
        self.conn().execute(
            "INSERT INTO repos (id, name, local_path, remote_url, default_branch, protected_branches, governance, added_at, last_scanned_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                r.id,
                r.name,
                r.local_path,
                r.remote_url,
                r.default_branch,
                serde_json::to_string(&r.protected_branches)?,
                serde_json::to_string(&r.governance)?,
                r.added_at,
                r.last_scanned_at
            ],
        )?;
        Ok(())
    }

    pub fn repo_update(&self, r: &RepoRef) -> Result<()> {
        self.conn().execute(
            "UPDATE repos SET name=?2, local_path=?3, remote_url=?4, default_branch=?5, protected_branches=?6, governance=?7, last_scanned_at=?8 WHERE id=?1",
            params![
                r.id,
                r.name,
                r.local_path,
                r.remote_url,
                r.default_branch,
                serde_json::to_string(&r.protected_branches)?,
                serde_json::to_string(&r.governance)?,
                r.last_scanned_at
            ],
        )?;
        Ok(())
    }

    pub fn repo_remove(&self, id: &str) -> Result<()> {
        self.conn().execute("DELETE FROM repos WHERE id=?1", params![id])?;
        Ok(())
    }

    fn row_to_repo(row: &rusqlite::Row<'_>) -> rusqlite::Result<RepoRef> {
        let protected: String = row.get(5)?;
        let governance: String = row.get(6)?;
        Ok(RepoRef {
            id: row.get(0)?,
            name: row.get(1)?,
            local_path: row.get(2)?,
            remote_url: row.get(3)?,
            default_branch: row.get(4)?,
            protected_branches: serde_json::from_str(&protected).unwrap_or_default(),
            governance: serde_json::from_str(&governance).unwrap_or_default(),
            added_at: row.get(7)?,
            last_scanned_at: row.get(8)?,
        })
    }

    pub fn repo_list(&self) -> Result<Vec<RepoRef>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, name, local_path, remote_url, default_branch, protected_branches, governance, added_at, last_scanned_at FROM repos ORDER BY added_at",
        )?;
        let rows = stmt.query_map([], Self::row_to_repo)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn repo_get(&self, id: &str) -> Result<RepoRef> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, name, local_path, remote_url, default_branch, protected_branches, governance, added_at, last_scanned_at FROM repos WHERE id=?1",
        )?;
        stmt.query_row(params![id], Self::row_to_repo)
            .optional()?
            .ok_or_else(|| CoreError::NotFound(format!("dépôt {id}")))
    }

    // -- plans / propositions ------------------------------------------------

    pub fn plan_save(&self, p: &Plan) -> Result<()> {
        self.conn().execute(
            "INSERT INTO plans (id, repo_id, data, status, created_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET data=?3, status=?4",
            params![
                p.id,
                p.repo_id,
                serde_json::to_string(p)?,
                serde_json::to_string(&p.status)?,
                p.created_at
            ],
        )?;
        Ok(())
    }

    pub fn plan_get(&self, id: &str) -> Result<Plan> {
        let data: Option<String> = self
            .conn()
            .query_row("SELECT data FROM plans WHERE id=?1", params![id], |r| r.get(0))
            .optional()?;
        let data = data.ok_or_else(|| CoreError::NotFound(format!("plan {id}")))?;
        Ok(serde_json::from_str(&data)?)
    }

    pub fn plan_list(&self, repo_id: &str) -> Result<Vec<Plan>> {
        let conn = self.conn();
        let mut stmt =
            conn.prepare("SELECT data FROM plans WHERE repo_id=?1 ORDER BY created_at DESC")?;
        let rows = stmt.query_map(params![repo_id], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(serde_json::from_str(&r?)?);
        }
        Ok(out)
    }

    pub fn proposal_save(&self, p: &Proposal) -> Result<()> {
        self.conn().execute(
            "INSERT INTO proposals (id, repo_id, data, status, created_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET data=?3, status=?4",
            params![
                p.id,
                p.repo_id,
                serde_json::to_string(p)?,
                serde_json::to_string(&p.status)?,
                p.created_at
            ],
        )?;
        Ok(())
    }

    pub fn proposal_list(&self, repo_id: &str) -> Result<Vec<Proposal>> {
        let conn = self.conn();
        let mut stmt =
            conn.prepare("SELECT data FROM proposals WHERE repo_id=?1 ORDER BY created_at DESC")?;
        let rows = stmt.query_map(params![repo_id], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(serde_json::from_str(&r?)?);
        }
        Ok(out)
    }

    // -- CI ------------------------------------------------------------------

    pub fn ci_account_save(&self, a: &CiAccount) -> Result<()> {
        self.conn().execute(
            "INSERT INTO ci_accounts (id, data) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET data=?2",
            params![a.id, serde_json::to_string(a)?],
        )?;
        Ok(())
    }

    pub fn ci_account_list(&self) -> Result<Vec<CiAccount>> {
        self.list_json("ci_accounts")
    }

    pub fn ci_account_get(&self, id: &str) -> Result<CiAccount> {
        self.get_json("ci_accounts", id)
            .and_then(|o| o.ok_or_else(|| CoreError::NotFound(format!("compte {id}"))))
    }

    pub fn ci_account_remove(&self, id: &str) -> Result<()> {
        self.conn().execute("DELETE FROM ci_accounts WHERE id=?1", params![id])?;
        Ok(())
    }

    pub fn policy_save(&self, p: &RetentionPolicy) -> Result<()> {
        self.conn().execute(
            "INSERT INTO policies (id, data) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET data=?2",
            params![p.id, serde_json::to_string(p)?],
        )?;
        Ok(())
    }

    pub fn policy_list(&self) -> Result<Vec<RetentionPolicy>> {
        self.list_json("policies")
    }

    pub fn policy_get(&self, id: &str) -> Result<RetentionPolicy> {
        self.get_json("policies", id)
            .and_then(|o| o.ok_or_else(|| CoreError::NotFound(format!("politique {id}"))))
    }

    pub fn job_save(&self, j: &CleanupJob) -> Result<()> {
        self.conn().execute(
            "INSERT INTO cleanup_jobs (id, policy_id, account_id, mode, data, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET data=?5",
            params![
                j.id,
                j.policy_id,
                j.account_id,
                serde_json::to_string(&j.mode)?,
                serde_json::to_string(j)?,
                j.created_at
            ],
        )?;
        Ok(())
    }

    pub fn job_list(&self) -> Result<Vec<CleanupJob>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT data FROM cleanup_jobs ORDER BY created_at DESC")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(serde_json::from_str(&r?)?);
        }
        Ok(out)
    }

    /// Dernière simulation réussie pour un périmètre donné (empreinte).
    pub fn last_simulation(&self, scope_hash: &str) -> Result<Option<SimulationReport>> {
        for j in self.job_list()? {
            if j.mode == JobMode::Simulation && j.status == "ok" {
                if let Ok(rep) = serde_json::from_value::<SimulationReport>(j.report.clone()) {
                    if rep.scope_hash == scope_hash {
                        return Ok(Some(rep));
                    }
                }
            }
        }
        Ok(None)
    }

    // -- IA / réglages -------------------------------------------------------

    pub fn ai_provider_save(&self, p: &AiProviderConfig) -> Result<()> {
        self.conn().execute(
            "INSERT INTO ai_providers (id, data) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET data=?2",
            params![p.id, serde_json::to_string(p)?],
        )?;
        Ok(())
    }

    pub fn ai_provider_list(&self) -> Result<Vec<AiProviderConfig>> {
        self.list_json("ai_providers")
    }

    pub fn ai_provider_remove(&self, id: &str) -> Result<()> {
        self.conn().execute("DELETE FROM ai_providers WHERE id=?1", params![id])?;
        Ok(())
    }

    pub fn setting_set(&self, key: &str, value: &serde_json::Value) -> Result<()> {
        self.conn().execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value=?2",
            params![key, serde_json::to_string(value)?],
        )?;
        Ok(())
    }

    pub fn setting_get(&self, key: &str) -> Result<Option<serde_json::Value>> {
        let v: Option<String> = self
            .conn()
            .query_row("SELECT value FROM settings WHERE key=?1", params![key], |r| r.get(0))
            .optional()?;
        Ok(match v {
            Some(s) => Some(serde_json::from_str(&s)?),
            None => None,
        })
    }

    // -- audit (append-only) -------------------------------------------------

    pub fn audit_append(
        &self,
        actor: &str,
        category: &str,
        action: &str,
        target: &str,
        params_json: &serde_json::Value,
        result: &str,
    ) -> Result<i64> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO audit (ts, actor, category, action, target, params, result) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                now_iso(),
                actor,
                category,
                action,
                target,
                serde_json::to_string(params_json)?,
                result
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn audit_list(&self, limit: u32) -> Result<Vec<AuditEvent>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT seq, ts, actor, category, action, target, params, result FROM audit ORDER BY seq DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |r| {
            let params_str: String = r.get(6)?;
            Ok(AuditEvent {
                seq: r.get(0)?,
                ts: r.get(1)?,
                actor: r.get(2)?,
                category: r.get(3)?,
                action: r.get(4)?,
                target: r.get(5)?,
                params: serde_json::from_str(&params_str).unwrap_or(serde_json::Value::Null),
                result: r.get(7)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn audit_export_jsonl(&self) -> Result<String> {
        let mut events = self.audit_list(u32::MAX)?;
        events.reverse();
        let mut out = String::new();
        for e in events {
            out.push_str(&serde_json::to_string(&e)?);
            out.push('\n');
        }
        Ok(out)
    }

    // -- helpers -------------------------------------------------------------

    fn list_json<T: serde::de::DeserializeOwned>(&self, table: &str) -> Result<Vec<T>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!("SELECT data FROM {table} ORDER BY id"))?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(serde_json::from_str(&r?)?);
        }
        Ok(out)
    }

    fn get_json<T: serde::de::DeserializeOwned>(&self, table: &str, id: &str) -> Result<Option<T>> {
        let v: Option<String> = self
            .conn()
            .query_row(&format!("SELECT data FROM {table} WHERE id=?1"), params![id], |r| r.get(0))
            .optional()?;
        Ok(match v {
            Some(s) => Some(serde_json::from_str(&s)?),
            None => None,
        })
    }
}
