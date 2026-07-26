use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::{Command, Stdio};

use git2::{Commit, Oid, Repository};

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

/// Réécrit UNIQUEMENT les messages d'un segment `base..tip` pouvant contenir des
/// MERGES (T10). Généralise `reword_chain` à un DAG : chaque commit est recréé
/// avec ses parents remappés (ceux hors segment gardent leur SHA) et son arbre
/// INCHANGÉ — la topologie (y compris les merges) et le contenu sont donc
/// préservés à l'identique ; seuls les messages (et donc les SHA) changent, et
/// les commits inchangés gardent leur SHA par adressage de contenu.
pub fn reword_dag(
    repo: &Repository,
    base: Oid,
    tip: Oid,
    messages: &HashMap<Oid, String>,
    ctx: &TaskCtx,
) -> Result<(Oid, HashMap<Oid, Oid>)> {
    // Ordre topologique du plus ancien au plus récent : un parent du segment
    // est toujours traité avant ses enfants.
    let segment = GitEngine::segment(repo, Some(base), tip)?;
    let seg_set: HashSet<Oid> = segment.iter().copied().collect();
    let total = segment.len() as u64;
    let mut map: HashMap<Oid, Oid> = HashMap::new();
    for (i, oid) in segment.iter().enumerate() {
        ctx.step("réécriture des messages", i as u64 + 1, Some(total))?;
        let c = repo.find_commit(*oid)?;
        let parents: Vec<Commit> = c
            .parent_ids()
            .map(|pid| {
                let mapped = if seg_set.contains(&pid) {
                    *map.get(&pid).unwrap_or(&pid)
                } else {
                    pid
                };
                repo.find_commit(mapped)
            })
            .collect::<std::result::Result<_, _>>()?;
        let parent_refs: Vec<&Commit> = parents.iter().collect();
        let owned;
        let msg: &str = match messages.get(oid) {
            Some(m) => {
                owned = normalize_message(m);
                &owned
            }
            None => c.message().unwrap_or(""),
        };
        let new_oid = repo.commit(
            None,
            &c.author(),
            &c.committer(),
            msg,
            &c.tree()?,
            &parent_refs,
        )?;
        map.insert(*oid, new_oid);
    }
    let new_tip = *map
        .get(&tip)
        .ok_or_else(|| CoreError::Invalid("segment vide".into()))?;
    Ok((new_tip, map))
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
        // Rapport de conflits par fichier (T10) avant d'abandonner.
        let conflicts =
            run_git(&wt, &["diff", "--name-only", "--diff-filter=U"], &[]).unwrap_or_default();
        let _ = run_git(&wt, &["rebase", "--abort"], &[]);
        cleanup(&repo_dir, &wt, &scratch);
        return Err(match e {
            CoreError::Cancelled => CoreError::Cancelled,
            e => {
                let files = if conflicts.trim().is_empty() {
                    String::new()
                } else {
                    format!(
                        " Fichiers en conflit : {}.",
                        conflicts.lines().collect::<Vec<_>>().join(", ")
                    )
                };
                CoreError::Refused(format!(
                    "le rejeu de la structure a échoué (conflit probable) : {e}.{files}"
                ))
            }
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

/// Rejoue un segment `base..tip` CONTENANT DES MERGES en appliquant les
/// changements de structure du plan (drop, fixup/squash de commits NON-merge)
/// tout en PRÉSERVANT la topologie des merges (T10 complet, `--rebase-merges`).
///
/// Stratégie sûre : on laisse git générer sa todo `--rebase-merges` (qui encode
/// la topologie via `label`/`reset`/`merge`), on la CAPTURE, on ne transforme
/// QUE les lignes `pick` selon le plan (fixup absorbé → `fixup`, commit
/// abandonné → ligne retirée ; les lignes `merge`/`label`/`reset` restent
/// intactes), puis on rejoue. C'est git qui valide le placement des `fixup` :
/// une todo invalide fait échouer le rebase qui est ABANDONNÉ sans rien écrire.
/// Un hook `post-rewrite` capture la correspondance ancien→nouveau SHA.
/// Retourne le nouveau sommet et cette carte (pour la passe de messages).
pub fn sequencer_rebase_merges(
    repo: &Repository,
    base: Oid,
    tip: Oid,
    groups: &[TodoGroup],
    cancel: &CancelToken,
) -> Result<(Oid, HashMap<Oid, Oid>)> {
    let repo_dir = repo
        .workdir()
        .ok_or_else(|| CoreError::Refused("dépôt bare non supporté".into()))?
        .to_path_buf();

    let scratch =
        std::env::temp_dir().join(format!("mc-rebasem-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&scratch)?;
    let wt = scratch.join("wt");
    let captured = scratch.join("todo-git.txt");
    let transformed = scratch.join("todo-mc.txt");
    let map_file = scratch.join("rewritten.txt");
    let hooks = scratch.join("hooks");
    std::fs::create_dir_all(&hooks)?;

    // Hook post-rewrite : reçoit sur stdin les paires « <ancien> <nouveau> ».
    let hook = hooks.join("post-rewrite");
    std::fs::write(&hook, format!("#!/bin/sh\ncat >> '{}'\n", posix(&map_file)))?;
    // Éditeur de PHASE 1 : copie la todo générée par git puis échoue → git
    // abandonne le rebase avant tout travail (on récupère juste la todo).
    let cap_editor = scratch.join("capture.sh");
    std::fs::write(
        &cap_editor,
        format!("#!/bin/sh\ncp \"$1\" '{}'\nexit 1\n", posix(&captured)),
    )?;
    // Sur Unix, git n'exécute un hook (post-rewrite) que s'il est exécutable :
    // sans le bit +x le hook est ignoré silencieusement et la carte des SHA
    // reste vide. (Sur Windows, git exécute les scripts shell via sa propre
    // sh, indépendamment des permissions.)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for p in [&hook, &cap_editor] {
            if let Ok(meta) = std::fs::metadata(p) {
                let mut perm = meta.permissions();
                perm.set_mode(0o755);
                let _ = std::fs::set_permissions(p, perm);
            }
        }
    }

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

    let hooks_cfg = format!("core.hooksPath={}", posix(&hooks));

    // -- Phase 1 : capturer la todo `--rebase-merges` de git (puis abandon). --
    let envs1 = [
        (
            "GIT_SEQUENCE_EDITOR",
            format!("sh '{}'", posix(&cap_editor)),
        ),
        ("GIT_EDITOR", "true".to_string()),
    ];
    let _ = run_git(
        &wt,
        &[
            "-c",
            "commit.gpgsign=false",
            "-c",
            &hooks_cfg,
            "-c",
            "core.abbrev=40",
            "rebase",
            "-i",
            "--rebase-merges",
            "--onto",
            &base.to_string(),
            &base.to_string(),
        ],
        &envs1,
    );
    let _ = run_git(&wt, &["rebase", "--abort"], &[]);
    let todo = std::fs::read_to_string(&captured).map_err(|_| {
        cleanup(&repo_dir, &wt, &scratch);
        CoreError::Git("capture de la todo --rebase-merges impossible".into())
    });
    let todo = match todo {
        Ok(t) => t,
        Err(e) => {
            cleanup(&repo_dir, &wt, &scratch);
            return Err(e);
        }
    };

    // -- Transformation : uniquement les lignes `pick`. --
    let leaders: HashSet<Oid> = groups.iter().map(|g| g.leader).collect();
    let fixups: HashSet<Oid> = groups
        .iter()
        .flat_map(|g| g.fixups.iter().copied())
        .collect();
    let match_oid =
        |set: &HashSet<Oid>, sha: &str| set.iter().any(|o| o.to_string().starts_with(sha));
    let mut out = String::new();
    for line in todo.lines() {
        let t = line.trim_start();
        let cmd = t.split_whitespace().next().unwrap_or("");
        if cmd == "pick" || cmd == "p" {
            let sha = t.split_whitespace().nth(1).unwrap_or("");
            if match_oid(&fixups, sha) {
                out.push_str(&format!("fixup {sha}\n"));
            } else if match_oid(&leaders, sha) {
                out.push_str(line);
                out.push('\n');
            }
            // sinon : commit abandonné (hors groupes) → ligne retirée.
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    std::fs::write(&transformed, &out)?;

    // -- Phase 2 : rejouer la todo transformée (hook post-rewrite actif). --
    let envs2 = [
        (
            "GIT_SEQUENCE_EDITOR",
            format!("cp '{}'", posix(&transformed)),
        ),
        ("GIT_EDITOR", "true".to_string()),
    ];
    let rebase = run_git_cancellable(
        &wt,
        &[
            "-c",
            "commit.gpgsign=false",
            "-c",
            &hooks_cfg,
            "-c",
            "core.abbrev=40",
            "rebase",
            "-i",
            "--rebase-merges",
            "--onto",
            &base.to_string(),
            &base.to_string(),
        ],
        &envs2,
        cancel,
    );
    if let Err(e) = rebase {
        let conflicts =
            run_git(&wt, &["diff", "--name-only", "--diff-filter=U"], &[]).unwrap_or_default();
        let _ = run_git(&wt, &["rebase", "--abort"], &[]);
        cleanup(&repo_dir, &wt, &scratch);
        return Err(match e {
            CoreError::Cancelled => CoreError::Cancelled,
            e => {
                let files = if conflicts.trim().is_empty() {
                    String::new()
                } else {
                    format!(
                        " Fichiers en conflit : {}.",
                        conflicts.lines().collect::<Vec<_>>().join(", ")
                    )
                };
                CoreError::Refused(format!(
                    "le rejeu de la structure à travers un merge a échoué (conflit probable) : {e}.{files}"
                ))
            }
        });
    }

    let head = run_git(&wt, &["rev-parse", "HEAD"], &[])?;
    let new_tip = Oid::from_str(&head).map_err(|e| CoreError::Git(e.to_string()))?;

    let mut map: HashMap<Oid, Oid> = HashMap::new();
    if let Ok(txt) = std::fs::read_to_string(&map_file) {
        for line in txt.lines() {
            let mut it = line.split_whitespace();
            if let (Some(o), Some(n)) = (it.next(), it.next()) {
                if let (Ok(oo), Ok(nn)) = (Oid::from_str(o), Oid::from_str(n)) {
                    map.insert(oo, nn);
                }
            }
        }
    }
    cleanup(&repo_dir, &wt, &scratch);
    Ok((new_tip, map))
}

/// Résultat d'un pas de rejeu INTERACTIF (C1) : soit terminé (nouveau sommet),
/// soit EN PAUSE sur un conflit (le worktree est conservé pour résolution).
pub enum RebaseStep {
    Done(Oid),
    /// Fichiers en conflit : (chemin relatif, contenu AVEC les marqueurs).
    Conflict(Vec<(String, String)>),
}

/// Nettoie le worktree lié et le dossier de session. On supprime le physique
/// AVANT de `prune` : si `worktree remove` échoue, le prune final réclame quand
/// même l'entrée admin (le dossier ayant alors disparu).
fn cleanup_session(repo_dir: &Path, session_dir: &Path) {
    let wt = session_dir.join("wt");
    let _ = run_git(
        repo_dir,
        &["worktree", "remove", "--force", &wt.to_string_lossy()],
        &[],
    );
    let _ = std::fs::remove_dir_all(session_dir);
    let _ = run_git(repo_dir, &["worktree", "prune"], &[]);
}

/// Lit les fichiers en conflit du worktree (chemin + contenu à marqueurs). Un
/// fichier BINAIRE (contenu non-UTF-8) ne peut pas être résolu comme du texte :
/// on renvoie une sentinelle porteuse de marqueurs (la reprise reste bloquée tant
/// qu'ils subsistent) plutôt qu'un contenu vide qui écraserait le binaire.
fn conflicted_files(wt: &Path) -> Vec<(String, String)> {
    run_git(wt, &["diff", "--name-only", "--diff-filter=U"], &[])
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|p| {
            let content = match std::fs::read(wt.join(p)) {
                Ok(bytes) => String::from_utf8(bytes).unwrap_or_else(|_| {
                    "<<<<<<< CONFLIT BINAIRE\n(contenu non textuel — résoudre hors de l'outil puis \
                     `git add`)\n>>>>>>>\n"
                        .to_string()
                }),
                Err(_) => String::new(),
            };
            (p.to_string(), content)
        })
        .collect()
}

/// Interprète le résultat d'un `git rebase` (start ou --continue) : succès →
/// Done (worktree nettoyé) ; conflit (fichiers non fusionnés) → Conflict
/// (worktree CONSERVÉ) ; annulation ou autre échec → abort + nettoyage + erreur.
fn step_from_result(
    repo_dir: &Path,
    session_dir: &Path,
    res: Result<String>,
) -> Result<RebaseStep> {
    let wt = session_dir.join("wt");
    match res {
        Ok(_) => {
            let head = run_git(&wt, &["rev-parse", "HEAD"], &[])?;
            let new_tip = Oid::from_str(&head).map_err(|e| CoreError::Git(e.to_string()))?;
            cleanup_session(repo_dir, session_dir);
            Ok(RebaseStep::Done(new_tip))
        }
        Err(CoreError::Cancelled) => {
            let _ = run_git(&wt, &["rebase", "--abort"], &[]);
            cleanup_session(repo_dir, session_dir);
            Err(CoreError::Cancelled)
        }
        Err(e) => {
            let files = conflicted_files(&wt);
            if files.is_empty() {
                let _ = run_git(&wt, &["rebase", "--abort"], &[]);
                cleanup_session(repo_dir, session_dir);
                Err(CoreError::Refused(format!(
                    "le rejeu a échoué hors conflit résoluble (une résolution a-t-elle vidé un \
                     commit ? git ne peut alors pas poursuivre) : {e}"
                )))
            } else {
                // Conflit : on LAISSE le worktree en pause pour résolution.
                Ok(RebaseStep::Conflict(files))
            }
        }
    }
}

/// Démarre un rejeu de structure LINÉAIRE dans `session_dir` SANS abandonner sur
/// conflit (C1). Comme `sequencer_rebase`, mais renvoie `RebaseStep::Conflict`
/// en laissant le worktree prêt pour `sequencer_continue`/`sequencer_abort`.
pub fn sequencer_start(
    repo: &Repository,
    session_dir: &Path,
    base: Oid,
    tip: Oid,
    groups: &[TodoGroup],
    cancel: &CancelToken,
) -> Result<RebaseStep> {
    if groups.is_empty() {
        return Err(CoreError::Refused(
            "le plan supprimerait tous les commits du segment".into(),
        ));
    }
    let repo_dir = repo
        .workdir()
        .ok_or_else(|| CoreError::Refused("dépôt bare non supporté".into()))?
        .to_path_buf();
    let wt = session_dir.join("wt");
    let todo_src = session_dir.join("todo.txt");
    let hooks_empty = session_dir.join("hooks-vides");
    std::fs::create_dir_all(&hooks_empty)?;

    let mut todo = String::new();
    for g in groups {
        todo.push_str(&format!("pick {}\n", g.leader));
        for f in &g.fixups {
            todo.push_str(&format!("fixup {}\n", f));
        }
    }
    std::fs::write(&todo_src, &todo)?;

    // Réclame les worktrees périmés AVANT l'ajout : après un redémarrage de l'app,
    // le registre de sessions (en mémoire) est perdu mais l'entrée admin du dépôt
    // pour ce chemin déterministe persiste ; sans ce prune, `worktree add` sur le
    // même chemin échouerait (« missing but already registered ») et bloquerait le
    // plan. `prune` ne réclame que les entrées dont le dossier a disparu.
    let _ = run_git(&repo_dir, &["worktree", "prune"], &[]);
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
    let res = run_git_cancellable(
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
    step_from_result(&repo_dir, session_dir, res)
}

/// Poursuit un rejeu en pause après résolution (`git rebase --continue`).
pub fn sequencer_continue(
    repo: &Repository,
    session_dir: &Path,
    cancel: &CancelToken,
) -> Result<RebaseStep> {
    let repo_dir = repo
        .workdir()
        .ok_or_else(|| CoreError::Refused("dépôt bare non supporté".into()))?
        .to_path_buf();
    let wt = session_dir.join("wt");
    let hooks_empty = session_dir.join("hooks-vides");
    let envs = [("GIT_EDITOR", "true".to_string())];
    let res = run_git_cancellable(
        &wt,
        &[
            "-c",
            "commit.gpgsign=false",
            "-c",
            &format!("core.hooksPath={}", posix(&hooks_empty)),
            "rebase",
            "--continue",
        ],
        &envs,
        cancel,
    );
    step_from_result(&repo_dir, session_dir, res)
}

/// Abandonne un rejeu en pause (`git rebase --abort`) et nettoie la session.
pub fn sequencer_abort(repo: &Repository, session_dir: &Path) {
    if let Some(repo_dir) = repo.workdir() {
        let wt = session_dir.join("wt");
        let _ = run_git(&wt, &["rebase", "--abort"], &[]);
        cleanup_session(repo_dir, session_dir);
    }
}

/// Écrit le contenu résolu d'un fichier en conflit puis le stage (`git add`).
///
/// SÉCURITÉ : `file` DOIT être un chemin relatif au worktree, sans remontée. On
/// rejette l'absolu (qui remplacerait la cible) et tout composant `..`/racine/
/// préfixe (qui sortirait du worktree) AVANT d'écrire — sinon un appelant (ou du
/// contenu de dépôt non fiable rendu dans la webview) pourrait écraser un fichier
/// arbitraire du poste via `Path::join`.
pub fn conflict_resolve(session_dir: &Path, file: &str, content: &str) -> Result<()> {
    use std::path::Component;
    let rel = Path::new(file);
    let unsafe_path = rel.components().any(|c| {
        matches!(
            c,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    });
    if file.is_empty() || rel.is_absolute() || unsafe_path {
        return Err(CoreError::Refused(format!(
            "chemin de résolution invalide (absolu ou remontée « .. » interdite) : {file}"
        )));
    }
    let wt = session_dir.join("wt");
    let target = wt.join(file);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&target, content)?;
    run_git(&wt, &["add", "--", file], &[])?;
    Ok(())
}
