use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Stdio};

use git2::{Oid, Repository};

use crate::error::{CoreError, Result};
use crate::task::{CancelToken, TaskCtx};

use super::GitEngine;

/// Groupe de la todo list du sequencer : un leader (`pick`) suivi de ses
/// `fixup`. Les commits abandonnés (`drop`) sont simplement absents.
#[derive(Debug, Clone)]
pub struct TodoGroup {
    pub leader: Oid,
    pub fixups: Vec<Oid>,
}

/// Réécrit uniquement les MESSAGES d'une chaîne linéaire `base..tip`.
/// Les arbres, auteurs et horodatages sont conservés à l'identique — les
/// commits non modifiés gardent donc exactement le même SHA (adressage par
/// contenu) jusqu'au premier commit changé.
pub fn reword_chain(
    repo: &Repository,
    base: Option<Oid>,
    tip: Oid,
    messages: &HashMap<Oid, String>,
    ctx: &TaskCtx,
) -> Result<(Oid, HashMap<Oid, Oid>)> {
    let segment = GitEngine::segment(repo, base, tip)?;
    let total = segment.len() as u64;
    let mut map = HashMap::new();
    let mut new_parent: Option<Oid> = base;
    for (i, oid) in segment.into_iter().enumerate() {
        ctx.step("réécriture des messages", i as u64 + 1, Some(total))?;
        let c = repo.find_commit(oid)?;
        if c.parent_count() > 1 {
            return Err(CoreError::Refused(
                "le segment contient un commit de merge (non supporté)".into(),
            ));
        }
        let owned;
        let msg: &str = match messages.get(&oid) {
            Some(m) => {
                owned = normalize_message(m);
                &owned
            }
            None => c.message().unwrap_or(""),
        };
        let tree = c.tree()?;
        let author = c.author();
        let committer = c.committer();
        let new_oid = match new_parent {
            Some(p) => {
                let parent = repo.find_commit(p)?;
                repo.commit(None, &author, &committer, msg, &tree, &[&parent])?
            }
            None => repo.commit(None, &author, &committer, msg, &tree, &[])?,
        };
        map.insert(oid, new_oid);
        new_parent = Some(new_oid);
    }
    new_parent
        .map(|tip| (tip, map))
        .ok_or_else(|| CoreError::Invalid("segment vide".into()))
}

fn normalize_message(m: &str) -> String {
    let mut s = m.replace("\r\n", "\n").trim_end().to_string();
    s.push('\n');
    s
}

fn posix(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

pub(crate) fn run_git(dir: &Path, args: &[&str], envs: &[(&str, String)]) -> Result<String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(dir).args(args);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let out = cmd
        .output()
        .map_err(|e| CoreError::Git(format!("exécution de git : {e}")))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(CoreError::Git(format!(
            "git {} : {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&out.stderr).trim()
        )))
    }
}

/// Variante annulable : le processus git est TUÉ si le jeton passe à annulé
/// (l'appelant nettoie ensuite — `rebase --abort` + suppression du worktree).
/// Les sorties restent bornées (messages du sequencer) : pas de risque de
/// saturation du pipe pendant l'attente.
pub(crate) fn run_git_cancellable(
    dir: &Path,
    args: &[&str],
    envs: &[(&str, String)],
    cancel: &CancelToken,
) -> Result<String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(dir)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| CoreError::Git(format!("exécution de git : {e}")))?;
    loop {
        if cancel.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(CoreError::Cancelled);
        }
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(50)),
            Err(e) => return Err(CoreError::Git(format!("attente de git : {e}"))),
        }
    }
    let out = child
        .wait_with_output()
        .map_err(|e| CoreError::Git(format!("sortie de git : {e}")))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(CoreError::Git(format!(
            "git {} : {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&out.stderr).trim()
        )))
    }
}

/// Rejoue le segment `base..tip` selon la structure donnée (ordre des groupes,
/// fixups, drops) via le sequencer natif de Git, dans un worktree temporaire
/// détaché — la branche d'origine n'est jamais touchée. Retourne le nouveau
/// sommet et la correspondance groupe → nouveau SHA.
pub fn sequencer_rebase(
    repo: &Repository,
    base: Oid,
    tip: Oid,
    groups: &[TodoGroup],
    cancel: &CancelToken,
) -> Result<(Oid, Vec<(TodoGroup, Oid)>)> {
    if groups.is_empty() {
        return Err(CoreError::Refused(
            "le plan supprimerait tous les commits du segment".into(),
        ));
    }
    let repo_dir = repo
        .workdir()
        .ok_or_else(|| CoreError::Refused("dépôt bare non supporté".into()))?
        .to_path_buf();

    let scratch = std::env::temp_dir().join(format!("mc-rebase-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&scratch)?;
    let wt = scratch.join("wt");
    let todo_src = scratch.join("todo.txt");
    let hooks_empty = scratch.join("hooks-vides");
    std::fs::create_dir_all(&hooks_empty)?;

    let mut todo = String::new();
    for g in groups {
        todo.push_str(&format!("pick {}\n", g.leader));
        for f in &g.fixups {
            todo.push_str(&format!("fixup {}\n", f));
        }
    }
    std::fs::write(&todo_src, &todo)?;

    let cleanup = |repo_dir: &Path, wt: &Path, scratch: &Path| {
        let _ = run_git(
            repo_dir,
            &["worktree", "remove", "--force", &wt.to_string_lossy()],
            &[],
        );
        let _ = run_git(repo_dir, &["worktree", "prune"], &[]);
        let _ = std::fs::remove_dir_all(scratch);
    };

    run_git(
        &repo_dir,
        &[
            "worktree",
            "add",
            "--detach",
            &wt.to_string_lossy(),
            &tip.to_string(),
        ],
        &[],
    )?;

    let envs = [
        ("GIT_SEQUENCE_EDITOR", format!("cp '{}'", posix(&todo_src))),
        ("GIT_EDITOR", "true".to_string()),
    ];
    let rebase = run_git_cancellable(
        &wt,
        &[
            "-c",
            "commit.gpgsign=false",
            "-c",
            &format!("core.hooksPath={}", posix(&hooks_empty)),
            "rebase",
            "-i",
            "--onto",
            &base.to_string(),
            &base.to_string(),
        ],
        &envs,
        cancel,
    );

    if let Err(e) = rebase {
        let _ = run_git(&wt, &["rebase", "--abort"], &[]);
        cleanup(&repo_dir, &wt, &scratch);
        return Err(match e {
            CoreError::Cancelled => CoreError::Cancelled,
            e => CoreError::Refused(format!(
                "le rejeu de la structure a échoué (conflit probable) : {e}"
            )),
        });
    }

    let head = run_git(&wt, &["rev-parse", "HEAD"], &[])?;
    cleanup(&repo_dir, &wt, &scratch);

    let new_tip = Oid::from_str(&head).map_err(|e| CoreError::Git(e.to_string()))?;
    let new_oids = GitEngine::segment(repo, Some(base), new_tip)?;
    if new_oids.len() != groups.len() {
        return Err(CoreError::Git(format!(
            "incohérence après rejeu : {} commits pour {} groupes",
            new_oids.len(),
            groups.len()
        )));
    }
    let mapping = groups
        .iter()
        .cloned()
        .zip(new_oids.iter().copied())
        .collect();
    Ok((new_tip, mapping))
}
