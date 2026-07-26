pub mod push;
pub mod rewrite;
pub mod sign;
pub mod split;

use std::collections::HashSet;

use git2::{BranchType, Oid, Repository, Sort, StatusOptions};

use crate::error::{CoreError, Result};
use crate::model::{BranchInfo, CommitInfo};

pub struct GitEngine;

impl GitEngine {
    pub fn open(path: &str) -> Result<Repository> {
        Repository::open(path)
            .map_err(|e| CoreError::Git(format!("ouverture de {path} : {}", e.message())))
    }

    pub fn detect_default_branch(repo: &Repository) -> Option<String> {
        if let Ok(r) = repo.find_reference("refs/remotes/origin/HEAD") {
            if let Ok(Some(t)) = r.symbolic_target() {
                return t.strip_prefix("refs/remotes/origin/").map(String::from);
            }
        }
        for cand in ["main", "master"] {
            if repo.find_branch(cand, BranchType::Local).is_ok() {
                return Some(cand.to_string());
            }
        }
        repo.head()
            .ok()
            .and_then(|h| h.shorthand().ok().map(|s| s.to_string()))
    }

    pub fn remote_url(repo: &Repository) -> Option<String> {
        repo.find_remote("origin")
            .ok()
            .and_then(|r| r.url().ok().map(|s| s.to_string()))
    }

    pub fn head_branch(repo: &Repository) -> Option<String> {
        let head = repo.head().ok()?;
        if head.is_branch() {
            head.shorthand().ok().map(|s| s.to_string())
        } else {
            None
        }
    }

    pub fn branches(repo: &Repository) -> Result<Vec<BranchInfo>> {
        let head_name = Self::head_branch(repo);
        let mut out = Vec::new();
        for entry in repo.branches(Some(BranchType::Local))? {
            let (branch, _) = entry?;
            let name = branch.name()?.unwrap_or("").to_string();
            let tip = branch
                .get()
                .target()
                .map(|o| o.to_string())
                .unwrap_or_default();
            let upstream = branch
                .upstream()
                .ok()
                .and_then(|u| u.get().shorthand().ok().map(|s| s.to_string()));
            out.push(BranchInfo {
                is_head: head_name.as_deref() == Some(name.as_str()),
                name,
                upstream,
                tip,
            });
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    pub fn branch_tip(repo: &Repository, branch: &str) -> Result<Oid> {
        let b = repo
            .find_branch(branch, BranchType::Local)
            .map_err(|_| CoreError::NotFound(format!("branche {branch}")))?;
        b.get()
            .target()
            .ok_or_else(|| CoreError::Git(format!("branche {branch} sans cible")))
    }

    pub fn merge_base(repo: &Repository, a: Oid, b: Oid) -> Result<Oid> {
        Ok(repo.merge_base(a, b)?)
    }

    /// Résout une spécification (branche, tag, SHA, `HEAD~2`…) en l'OID du
    /// commit visé (F6 : choix explicite de la base du segment).
    pub fn resolve(repo: &Repository, spec: &str) -> Result<Oid> {
        let obj = repo.revparse_single(spec).map_err(|e| {
            CoreError::NotFound(format!(
                "référence « {spec} » introuvable : {}",
                e.message()
            ))
        })?;
        // Pèle jusqu'au commit (déréférence un tag annoté au besoin).
        let commit = obj.peel(git2::ObjectType::Commit).map_err(|e| {
            CoreError::Invalid(format!("« {spec} » n'est pas un commit : {}", e.message()))
        })?;
        Ok(commit.id())
    }

    /// SHA du segment `base..tip`, du plus ancien au plus récent.
    pub fn segment(repo: &Repository, base: Option<Oid>, tip: Oid) -> Result<Vec<Oid>> {
        let mut walk = repo.revwalk()?;
        walk.push(tip)?;
        if let Some(b) = base {
            walk.hide(b)?;
        }
        walk.set_sorting(Sort::TOPOLOGICAL | Sort::REVERSE)?;
        let mut out = Vec::new();
        for oid in walk {
            out.push(oid?);
        }
        Ok(out)
    }

    /// Commits atteignables depuis une réf remote (bornés par `base`) :
    /// sert à marquer les commits « partagés ».
    pub fn remote_reachable(repo: &Repository, base: Option<Oid>) -> HashSet<Oid> {
        let mut set = HashSet::new();
        let Ok(mut walk) = repo.revwalk() else {
            return set;
        };
        if walk.push_glob("refs/remotes/*").is_err() {
            return set;
        }
        if let Some(b) = base {
            let _ = walk.hide(b);
        }
        // Borne dure pour ne pas parcourir tout l'historique d'un gros dépôt.
        for oid in walk.take(10_000).flatten() {
            set.insert(oid);
        }
        set
    }

    pub fn parse_trailers(message: &str) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let last_para: Vec<&str> = message
            .trim_end()
            .rsplit("\n\n")
            .next()
            .unwrap_or("")
            .lines()
            .collect();
        for line in last_para {
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim();
                let ok = !key.is_empty()
                    && key
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
                if ok && !value.trim().is_empty() {
                    out.push((key.to_string(), value.trim().to_string()));
                }
            }
        }
        out
    }

    pub fn commit_info(
        repo: &Repository,
        oid: Oid,
        remote_set: &HashSet<Oid>,
    ) -> Result<CommitInfo> {
        let c = repo.find_commit(oid)?;
        let message = c.message().unwrap_or("").to_string();
        let subject = c.summary().ok().flatten().unwrap_or("").to_string();
        let body = message
            .split_once('\n')
            .map(|(_, rest)| rest.trim().to_string())
            .unwrap_or_default();
        let signed = repo.extract_signature(&oid, None).is_ok();

        let tree = c.tree()?;
        let parent_tree = if c.parent_count() > 0 {
            Some(c.parent(0)?.tree()?)
        } else {
            None
        };
        let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;
        let stats = diff.stats()?;
        let files: Vec<String> = diff
            .deltas()
            .take(50)
            .filter_map(|d| d.new_file().path().map(|p| p.to_string_lossy().to_string()))
            .collect();

        let author = c.author();
        let date = chrono::DateTime::from_timestamp(c.time().seconds(), 0)
            .map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
            .unwrap_or_default();

        Ok(CommitInfo {
            sha: oid.to_string(),
            short: oid.to_string()[..8.min(oid.to_string().len())].to_string(),
            parents: c.parent_ids().map(|p| p.to_string()).collect(),
            author_name: author.name().unwrap_or("").to_string(),
            author_email: author.email().unwrap_or("").to_string(),
            date,
            subject,
            trailers: Self::parse_trailers(&message),
            body,
            is_merge: c.parent_count() > 1,
            signed,
            on_remote: remote_set.contains(&oid),
            files_changed: stats.files_changed(),
            insertions: stats.insertions(),
            deletions: stats.deletions(),
            files,
        })
    }

    pub fn segment_infos(
        repo: &Repository,
        base: Option<Oid>,
        tip: Oid,
        max: usize,
    ) -> Result<Vec<CommitInfo>> {
        Self::segment_infos_cb(repo, base, tip, max, |_, _| Ok(()))
    }

    /// Variante avec point d'arrêt par commit : `on_each(i, total)` est appelé
    /// AVANT la lecture du i-ème commit (1-indexé) — progression + annulation.
    pub fn segment_infos_cb(
        repo: &Repository,
        base: Option<Oid>,
        tip: Oid,
        max: usize,
        mut on_each: impl FnMut(usize, usize) -> Result<()>,
    ) -> Result<Vec<CommitInfo>> {
        let mut oids = Self::segment(repo, base, tip)?;
        if oids.len() > max {
            oids = oids.split_off(oids.len() - max);
        }
        let remote_set = Self::remote_reachable(repo, base);
        let total = oids.len();
        let mut out = Vec::with_capacity(total);
        for (i, o) in oids.iter().enumerate() {
            on_each(i + 1, total)?;
            out.push(Self::commit_info(repo, *o, &remote_set)?);
        }
        Ok(out)
    }

    /// Patch unifié d'un commit (vs son premier parent), tronqué à `max_bytes`.
    pub fn commit_patch(repo: &Repository, oid: Oid, max_bytes: usize) -> Result<String> {
        let c = repo.find_commit(oid)?;
        let tree = c.tree()?;
        let parent_tree = if c.parent_count() > 0 {
            Some(c.parent(0)?.tree()?)
        } else {
            None
        };
        let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;
        let mut out = String::new();
        let mut truncated = false;
        let print_result = diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            if out.len() >= max_bytes {
                truncated = true;
                return false; // interrompt l'itération (erreur EUSER attendue)
            }
            match line.origin() {
                '+' | '-' | ' ' => out.push(line.origin()),
                _ => {}
            }
            out.push_str(&String::from_utf8_lossy(line.content()));
            true
        });
        if let Err(e) = print_result {
            if !truncated {
                return Err(e.into());
            }
        }
        if truncated {
            out.push_str("\n… [diff tronqué : commit volumineux]\n");
        }
        Ok(out)
    }

    /// Aucun fichier suivi modifié (les fichiers non suivis sont tolérés :
    /// un `reset --hard` ne les touche pas).
    pub fn workdir_clean(repo: &Repository) -> Result<bool> {
        let mut opts = StatusOptions::new();
        opts.include_untracked(false).include_ignored(false);
        let statuses = repo.statuses(Some(&mut opts))?;
        Ok(statuses.is_empty())
    }
}
