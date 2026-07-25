use std::collections::HashMap;

use git2::{Oid, Repository};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{CoreError, Result};
use crate::gitx::rewrite::{self, TodoGroup};
use crate::gitx::GitEngine;
use crate::model::*;
use crate::task::TaskCtx;

pub fn plan_hash(fingerprint: &Fingerprint, ops: &[PlanOp]) -> String {
    let payload = serde_json::json!({ "fingerprint": fingerprint, "ops": ops });
    let mut h = Sha256::new();
    h.update(payload.to_string().as_bytes());
    format!("{:x}", h.finalize())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAxis {
    pub axe: String,
    pub verdict: String, // ok | attention | bloquant
    pub motif: String,
}

struct GroupState {
    leader: Oid,
    fixups: Vec<Oid>,
    message: Option<String>,
    dropped: bool,
}

struct Compiled {
    groups: Vec<TodoGroup>,
    /// Messages finaux par leader (SHA d'origine).
    messages: HashMap<Oid, String>,
    structure_changed: bool,
    has_drop: bool,
}

fn idx_of(groups: &[GroupState], oid: Oid) -> Result<usize> {
    groups
        .iter()
        .position(|g| g.leader == oid && !g.dropped)
        .ok_or_else(|| {
            CoreError::Invalid(format!(
                "cible {oid} déjà consommée par une autre opération"
            ))
        })
}

/// Fusionne `targets` (leaders contigus) dans le premier ; `with_message`
/// impose le message final (Squash), sinon le message du premier est conservé (Fixup).
fn fold(
    groups: &mut [GroupState],
    segment: &[Oid],
    targets: &[String],
    with_message: Option<String>,
) -> Result<()> {
    if targets.len() < 2 {
        return Err(CoreError::Invalid(
            "une fusion requiert au moins deux commits".into(),
        ));
    }
    let mut idxs: Vec<usize> = Vec::new();
    for t in targets {
        idxs.push(idx_of(groups, resolve(segment, t)?)?);
    }
    let mut sorted = idxs.clone();
    sorted.sort_unstable();
    sorted.dedup();
    if sorted.len() != targets.len() {
        return Err(CoreError::Invalid(
            "doublon dans les cibles de fusion".into(),
        ));
    }
    let alive: Vec<usize> = groups
        .iter()
        .enumerate()
        .filter(|(_, g)| !g.dropped)
        .map(|(i, _)| i)
        .collect();
    let positions: Vec<usize> = sorted
        .iter()
        .map(|i| alive.iter().position(|a| a == i).unwrap())
        .collect();
    for w in positions.windows(2) {
        if w[1] != w[0] + 1 {
            return Err(CoreError::Refused(
                "les commits à fusionner doivent être contigus (réordonner d'abord)".into(),
            ));
        }
    }
    let first = sorted[0];
    let mut absorbed = Vec::new();
    for &i in sorted.iter().skip(1) {
        absorbed.push(groups[i].leader);
        let mut extra = std::mem::take(&mut groups[i].fixups);
        absorbed.append(&mut extra);
        groups[i].dropped = true; // absorbé (pas un drop utilisateur)
    }
    groups[first].fixups.extend(absorbed);
    if let Some(m) = with_message {
        groups[first].message = Some(m);
    }
    Ok(())
}

fn resolve(segment: &[Oid], sha: &str) -> Result<Oid> {
    let matches: Vec<&Oid> = segment
        .iter()
        .filter(|o| o.to_string().starts_with(sha))
        .collect();
    match matches.len() {
        1 => Ok(*matches[0]),
        0 => Err(CoreError::Invalid(format!(
            "cible {sha} hors du segment réécrivable"
        ))),
        _ => Err(CoreError::Invalid(format!("cible {sha} ambiguë"))),
    }
}

fn compile(segment: &[Oid], ops: &[PlanOp]) -> Result<Compiled> {
    if segment.is_empty() {
        return Err(CoreError::Invalid("segment vide".into()));
    }
    let mut groups: Vec<GroupState> = segment
        .iter()
        .map(|o| GroupState {
            leader: *o,
            fixups: Vec::new(),
            message: None,
            dropped: false,
        })
        .collect();
    let mut structure_changed = false;
    let mut has_drop = false;

    let mut sorted_ops: Vec<&PlanOp> = ops.iter().collect();
    sorted_ops.sort_by_key(|o| o.seq);

    for pop in sorted_ops {
        match &pop.op {
            Operation::Reword {
                target,
                new_message,
            } => {
                let oid = resolve(segment, target)?;
                let i = idx_of(&groups, oid)?;
                if new_message.trim().is_empty() {
                    return Err(CoreError::Invalid("message vide".into()));
                }
                groups[i].message = Some(new_message.clone());
            }
            Operation::Squash {
                targets,
                new_message,
            } => {
                fold(&mut groups, segment, targets, Some(new_message.clone()))?;
                structure_changed = true;
            }
            Operation::Fixup { targets } => {
                fold(&mut groups, segment, targets, None)?;
                structure_changed = true;
            }
            Operation::Drop { target, .. } => {
                let oid = resolve(segment, target)?;
                let i = idx_of(&groups, oid)?;
                if !groups[i].fixups.is_empty() {
                    return Err(CoreError::Invalid(
                        "impossible d'abandonner un commit cible d'une fusion".into(),
                    ));
                }
                groups[i].dropped = true;
                structure_changed = true;
                has_drop = true;
            }
            Operation::Reorder { order } => {
                let alive: Vec<Oid> = groups
                    .iter()
                    .filter(|g| !g.dropped)
                    .map(|g| g.leader)
                    .collect();
                let mut wanted = Vec::new();
                for sha in order {
                    let oid = resolve(segment, sha)?;
                    if !alive.contains(&oid) {
                        return Err(CoreError::Invalid(format!(
                            "réordonnancement : {sha} n'est pas un commit actif"
                        )));
                    }
                    wanted.push(oid);
                }
                if wanted.len() != alive.len() {
                    return Err(CoreError::Invalid(format!(
                        "réordonnancement : {} commits fournis, {} attendus",
                        wanted.len(),
                        alive.len()
                    )));
                }
                let mut seen = std::collections::HashSet::new();
                if !wanted.iter().all(|o| seen.insert(*o)) {
                    return Err(CoreError::Invalid("réordonnancement : doublon".into()));
                }
                let mut by_leader: HashMap<Oid, GroupState> =
                    groups.drain(..).map(|g| (g.leader, g)).collect();
                let mut rebuilt = Vec::new();
                for oid in &wanted {
                    rebuilt.push(by_leader.remove(oid).unwrap());
                }
                // les groupes abandonnés sont conservés en fin de liste (inertes)
                rebuilt.extend(by_leader.into_values());
                groups = rebuilt;
                if wanted != alive {
                    structure_changed = true;
                }
            }
        }
    }

    let final_groups: Vec<TodoGroup> = groups
        .iter()
        .filter(|g| !g.dropped)
        .map(|g| TodoGroup {
            leader: g.leader,
            fixups: g.fixups.clone(),
        })
        .collect();
    if final_groups.is_empty() {
        return Err(CoreError::Refused(
            "le plan supprimerait tous les commits du segment".into(),
        ));
    }
    let messages = groups
        .iter()
        .filter(|g| !g.dropped)
        .filter_map(|g| g.message.clone().map(|m| (g.leader, m)))
        .collect();

    Ok(Compiled {
        groups: final_groups,
        messages,
        structure_changed,
        has_drop,
    })
}

pub struct PlanEngine;

impl PlanEngine {
    /// Crée un plan vide sur `branch`, avec empreinte de l'état courant.
    pub fn new_plan(repo_ref: &RepoRef, repo: &Repository, branch: &str) -> Result<Plan> {
        Self::ensure_not_protected(repo_ref, branch)?;
        let tip = GitEngine::branch_tip(repo, branch)?;
        let default = repo_ref
            .default_branch
            .clone()
            .or_else(|| GitEngine::detect_default_branch(repo))
            .ok_or_else(|| CoreError::Invalid("branche par défaut inconnue".into()))?;
        if default == branch {
            return Err(CoreError::Refused(format!(
                "la branche par défaut « {branch} » n'est pas réécrivable"
            )));
        }
        let default_tip = GitEngine::branch_tip(repo, &default)?;
        let base = GitEngine::merge_base(repo, tip, default_tip)?;
        if base == tip {
            return Err(CoreError::Invalid(
                "aucun commit propre à cette branche (segment vide)".into(),
            ));
        }
        Ok(Plan {
            id: new_id("pln"),
            version: 1,
            repo_id: repo_ref.id.clone(),
            fingerprint: Fingerprint {
                branch: branch.to_string(),
                tip: tip.to_string(),
                base: base.to_string(),
            },
            status: PlanStatus::Draft,
            ops: Vec::new(),
            dry_run_hash: None,
            preview_ref: None,
            backup_ref: None,
            backup_tag: None,
            mapping: Vec::new(),
            created_at: now_iso(),
            dry_run_at: None,
            applied_at: None,
            error: None,
        })
    }

    fn ensure_not_protected(repo_ref: &RepoRef, branch: &str) -> Result<()> {
        let protected = repo_ref.protected_branches.iter().any(|b| b == branch)
            || repo_ref.default_branch.as_deref() == Some(branch);
        if protected {
            return Err(CoreError::Refused(format!(
                "la branche « {branch} » est protégée : réécriture bloquée"
            )));
        }
        Ok(())
    }

    fn check_fingerprint(repo: &Repository, plan: &Plan) -> Result<(Oid, Oid)> {
        let tip = GitEngine::branch_tip(repo, &plan.fingerprint.branch)?;
        if tip.to_string() != plan.fingerprint.tip {
            return Err(CoreError::Refused(format!(
                "empreinte invalide : la branche « {} » a bougé depuis la création du plan (attendu {}, trouvé {})",
                plan.fingerprint.branch,
                &plan.fingerprint.tip[..8],
                &tip.to_string()[..8]
            )));
        }
        let base =
            Oid::from_str(&plan.fingerprint.base).map_err(|e| CoreError::Invalid(e.to_string()))?;
        Ok((base, tip))
    }

    /// Construit RÉELLEMENT le résultat du plan dans `refs/mc/preview/<id>`
    /// sans toucher la branche, vérifie les invariants, calcule le mapping.
    pub fn dry_run(
        repo_ref: &RepoRef,
        repo: &Repository,
        plan: &mut Plan,
        ctx: &TaskCtx,
    ) -> Result<()> {
        Self::ensure_not_protected(repo_ref, &plan.fingerprint.branch)?;
        if plan.ops.is_empty() {
            return Err(CoreError::Invalid("plan sans opération".into()));
        }
        ctx.step("vérification de l'empreinte", 0, None)?;
        let (base, tip) = Self::check_fingerprint(repo, plan)?;
        let segment = GitEngine::segment(repo, Some(base), tip)?;
        // T10 complet : un segment contenant un merge accepte les changements de
        // STRUCTURE (squash/fixup/drop de commits non-merge) via `--rebase-merges`,
        // git préservant la topologie. Restent refusés : le réordonnancement
        // (sémantique ambiguë à travers un merge) et toute opération ciblant un
        // commit de merge (seule la reformulation de messages seuls le permet,
        // via reword_dag).
        let merge_oids: std::collections::HashSet<Oid> = segment
            .iter()
            .filter(|o| {
                repo.find_commit(**o)
                    .map(|c| c.parent_count() > 1)
                    .unwrap_or(false)
            })
            .copied()
            .collect();
        let has_merge = !merge_oids.is_empty();
        let only_reword = plan
            .ops
            .iter()
            .all(|o| matches!(o.op, Operation::Reword { .. }));
        if has_merge && !only_reword {
            if plan
                .ops
                .iter()
                .any(|o| matches!(o.op, Operation::Reorder { .. }))
            {
                return Err(CoreError::Refused(
                    "le réordonnancement à travers un merge n'est pas supporté (topologie ambiguë)"
                        .into(),
                ));
            }
            for op in &plan.ops {
                let targets: Vec<&String> = match &op.op {
                    Operation::Reword { target, .. } | Operation::Drop { target, .. } => {
                        vec![target]
                    }
                    Operation::Squash { targets, .. } | Operation::Fixup { targets } => {
                        targets.iter().collect()
                    }
                    Operation::Reorder { .. } => vec![],
                };
                for t in targets {
                    if let Ok(oid) = resolve(&segment, t) {
                        if merge_oids.contains(&oid) {
                            return Err(CoreError::Refused(
                                "un commit de merge ne peut être ni fusionné, ni abandonné, ni \
                                 reformulé au sein d'un changement de structure ; seule la \
                                 reformulation de messages seuls (sans autre opération) le permet"
                                    .into(),
                            ));
                        }
                    }
                }
            }
        }
        ctx.step("compilation du plan", 0, None)?;
        let compiled = compile(&segment, &plan.ops)?;

        let (final_tip, mapping) = if !compiled.structure_changed {
            let (new_tip, map) = if has_merge {
                rewrite::reword_dag(repo, base, tip, &compiled.messages, ctx)?
            } else {
                rewrite::reword_chain(repo, Some(base), tip, &compiled.messages, ctx)?
            };
            // Invariant CA-4 : un reword ne change jamais les arbres.
            for (old, new) in &map {
                let t_old = repo.find_commit(*old)?.tree_id();
                let t_new = repo.find_commit(*new)?.tree_id();
                if t_old != t_new {
                    return Err(CoreError::Git(
                        "invariant violé : arbre modifié par un reword".into(),
                    ));
                }
            }
            let mapping = segment
                .iter()
                .map(|o| ShaMapping {
                    old: vec![o.to_string()],
                    new: map[o].to_string(),
                })
                .collect::<Vec<_>>();
            (new_tip, mapping)
        } else {
            ctx.step("rejeu de la structure (sequencer git)", 0, None)?;
            if has_merge {
                // T10 complet : structure à travers un merge — git préserve la
                // topologie (`--rebase-merges`), les messages suivent via reword_dag.
                let (h1, oldnew) = rewrite::sequencer_rebase_merges(
                    repo,
                    base,
                    tip,
                    &compiled.groups,
                    &ctx.cancel,
                )?;
                let mut msg_by_new: HashMap<Oid, String> = HashMap::new();
                for (leader, m) in &compiled.messages {
                    if let Some(nw) = oldnew.get(leader) {
                        msg_by_new.insert(*nw, m.clone());
                    }
                }
                let (h2, rwmap) = rewrite::reword_dag(repo, base, h1, &msg_by_new, ctx)?;
                if !compiled.has_drop {
                    let t_old = repo.find_commit(tip)?.tree_id();
                    let t_new = repo.find_commit(h2)?.tree_id();
                    if t_old != t_new {
                        return Err(CoreError::Git(
                            "invariant violé : l'arbre final diffère sans drop".into(),
                        ));
                    }
                }
                let mapping = compiled
                    .groups
                    .iter()
                    .filter_map(|g| {
                        let n1 = *oldnew.get(&g.leader)?;
                        let nf = *rwmap.get(&n1).unwrap_or(&n1);
                        let mut old = vec![g.leader.to_string()];
                        old.extend(g.fixups.iter().map(|o| o.to_string()));
                        Some(ShaMapping {
                            old,
                            new: nf.to_string(),
                        })
                    })
                    .collect::<Vec<_>>();
                (h2, mapping)
            } else {
                let (h1, groupmap) =
                    rewrite::sequencer_rebase(repo, base, tip, &compiled.groups, &ctx.cancel)?;
                let mut msg_by_new: HashMap<Oid, String> = HashMap::new();
                for (group, new_oid) in &groupmap {
                    if let Some(m) = compiled.messages.get(&group.leader) {
                        msg_by_new.insert(*new_oid, m.clone());
                    }
                }
                let (h2, map2) = rewrite::reword_chain(repo, Some(base), h1, &msg_by_new, ctx)?;
                if !compiled.has_drop {
                    let t_old = repo.find_commit(tip)?.tree_id();
                    let t_new = repo.find_commit(h2)?.tree_id();
                    if t_old != t_new {
                        return Err(CoreError::Git(
                            "invariant violé : l'arbre final diffère sans drop".into(),
                        ));
                    }
                }
                let mapping = groupmap
                    .iter()
                    .map(|(g, new_oid)| {
                        let mut old = vec![g.leader.to_string()];
                        old.extend(g.fixups.iter().map(|o| o.to_string()));
                        ShaMapping {
                            old,
                            new: map2.get(new_oid).copied().unwrap_or(*new_oid).to_string(),
                        }
                    })
                    .collect::<Vec<_>>();
                (h2, mapping)
            }
        };

        ctx.step("contrôle des invariants et écriture de la préview", 0, None)?;
        let preview = format!("refs/mc/preview/{}", plan.id);
        repo.reference(&preview, final_tip, true, "mister-commitia dry-run")?;
        plan.preview_ref = Some(preview);
        plan.mapping = mapping;
        plan.dry_run_at = Some(now_iso());
        plan.dry_run_hash = Some(plan_hash(&plan.fingerprint, &plan.ops));
        plan.status = PlanStatus::DryRunOk;
        plan.error = None;
        Ok(())
    }

    /// Applique un plan : exige un dry-run réussi du MÊME plan, crée le backup
    /// (réf + tag) puis bascule la réf de branche sur le résultat du preview.
    pub fn apply(
        repo_ref: &RepoRef,
        repo: &Repository,
        plan: &mut Plan,
        confirm: Option<&str>,
        ctx: &TaskCtx,
    ) -> Result<()> {
        ctx.step(
            "contrôles préalables (empreinte, préview, partage)",
            0,
            None,
        )?;
        Self::ensure_not_protected(repo_ref, &plan.fingerprint.branch)?;
        if plan.status != PlanStatus::DryRunOk {
            return Err(CoreError::Refused(
                "dry-run requis avant application (aucun dry-run réussi pour ce plan)".into(),
            ));
        }
        let current = plan_hash(&plan.fingerprint, &plan.ops);
        if plan.dry_run_hash.as_deref() != Some(current.as_str()) {
            plan.status = PlanStatus::Draft;
            return Err(CoreError::Refused(
                "le plan a été modifié depuis son dry-run : relancer le dry-run".into(),
            ));
        }
        let (base, tip) = Self::check_fingerprint(repo, plan)?;
        let preview_name = plan
            .preview_ref
            .clone()
            .ok_or_else(|| CoreError::Refused("préview absente".into()))?;
        let final_tip = repo
            .find_reference(&preview_name)?
            .target()
            .ok_or_else(|| CoreError::Git("réf de préview sans cible".into()))?;

        // Branche partagée (commits présents sur un remote) → confirmation renforcée.
        let remote_set = GitEngine::remote_reachable(repo, Some(base));
        let segment = GitEngine::segment(repo, Some(base), tip)?;
        let shared = segment.iter().any(|o| remote_set.contains(o));
        if shared && confirm != Some(plan.fingerprint.branch.as_str()) {
            return Err(CoreError::ConfirmRequired {
                expected: plan.fingerprint.branch.clone(),
                message: format!(
                    "branche partagée : saisir exactement « {} » pour confirmer la réécriture",
                    plan.fingerprint.branch
                ),
            });
        }

        let branch = plan.fingerprint.branch.clone();
        let checked_out = GitEngine::head_branch(repo).as_deref() == Some(branch.as_str());
        if checked_out && !GitEngine::workdir_clean(repo)? {
            return Err(CoreError::Refused(
                "des fichiers suivis sont modifiés : commiter ou remiser avant d'appliquer".into(),
            ));
        }

        // Dernier point d'annulation : au-delà, backup puis bascule vont au
        // bout — jamais d'état intermédiaire.
        ctx.step("création du backup", 0, None)?;

        // Backup obligatoire (CA-1) — son échec annule tout.
        let ts = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
        let short = plan.id.chars().rev().take(6).collect::<String>();
        let backup_ref = format!("refs/mc/backup/{branch}/{ts}-{short}");
        let backup_tag = format!("refs/tags/mc-backup-{}", plan.id);
        repo.reference(&backup_ref, tip, false, "mister-commitia backup")?;
        repo.reference(&backup_tag, tip, false, "mister-commitia backup tag")?;
        if repo.find_reference(&backup_ref)?.target() != Some(tip) {
            return Err(CoreError::Git(
                "échec de création du backup : application annulée".into(),
            ));
        }

        ctx.emit("bascule de la branche (non annulable)", 0, None);
        if checked_out {
            let dir = repo
                .workdir()
                .ok_or_else(|| CoreError::Refused("dépôt bare".into()))?;
            rewrite::run_git(dir, &["reset", "--hard", &final_tip.to_string()], &[])?;
        } else {
            repo.reference(
                &format!("refs/heads/{branch}"),
                final_tip,
                true,
                "mister-commitia apply",
            )?;
        }

        plan.backup_ref = Some(backup_ref);
        plan.backup_tag = Some(backup_tag);
        plan.applied_at = Some(now_iso());
        plan.status = PlanStatus::Applied;
        Ok(())
    }

    /// Restaure la branche depuis le backup tant qu'aucun commit n'a été ajouté.
    pub fn rollback(repo_ref: &RepoRef, repo: &Repository, plan: &mut Plan) -> Result<()> {
        let _ = repo_ref;
        if plan.status != PlanStatus::Applied {
            return Err(CoreError::Refused(
                "seul un plan appliqué se restaure".into(),
            ));
        }
        let branch = plan.fingerprint.branch.clone();
        let tip = GitEngine::branch_tip(repo, &branch)?;
        let expected = plan
            .mapping
            .last()
            .map(|m| m.new.clone())
            .ok_or_else(|| CoreError::Invalid("mapping absent".into()))?;
        if tip.to_string() != expected {
            return Err(CoreError::Refused(format!(
                "la branche « {branch} » a avancé depuis l'application : rollback automatique impossible. \
                 Procédure guidée : créer une branche de secours sur le sommet actuel, puis réinitialiser \
                 « {branch} » sur {} (réf de backup).",
                plan.backup_ref.as_deref().unwrap_or("?")
            )));
        }
        let backup = plan
            .backup_ref
            .clone()
            .ok_or_else(|| CoreError::Invalid("réf de backup absente".into()))?;
        let old_tip = repo
            .find_reference(&backup)?
            .target()
            .ok_or_else(|| CoreError::Git("backup sans cible".into()))?;

        let checked_out = GitEngine::head_branch(repo).as_deref() == Some(branch.as_str());
        if checked_out {
            if !GitEngine::workdir_clean(repo)? {
                return Err(CoreError::Refused(
                    "des fichiers suivis sont modifiés : impossible de restaurer".into(),
                ));
            }
            let dir = repo
                .workdir()
                .ok_or_else(|| CoreError::Refused("dépôt bare".into()))?;
            rewrite::run_git(dir, &["reset", "--hard", &old_tip.to_string()], &[])?;
        } else {
            repo.reference(
                &format!("refs/heads/{branch}"),
                old_tip,
                true,
                "mister-commitia rollback",
            )?;
        }
        plan.status = PlanStatus::RolledBack;
        Ok(())
    }

    /// Rapport de risques (skill risk-reviewer, version programmatique).
    pub fn risk_report(repo_ref: &RepoRef, repo: &Repository, plan: &Plan) -> Vec<RiskAxis> {
        let mut axes = Vec::new();
        let branch = &plan.fingerprint.branch;
        let protected = repo_ref.protected_branches.iter().any(|b| b == branch)
            || repo_ref.default_branch.as_deref() == Some(branch.as_str());
        axes.push(RiskAxis {
            axe: "branche".into(),
            verdict: if protected { "bloquant" } else { "ok" }.into(),
            motif: if protected {
                format!("« {branch} » est protégée")
            } else {
                format!("« {branch} » n'est pas protégée")
            },
        });

        let base = Oid::from_str(&plan.fingerprint.base).ok();
        let tip = Oid::from_str(&plan.fingerprint.tip).ok();
        if let (Some(base), Some(tip)) = (base, tip) {
            let remote_set = GitEngine::remote_reachable(repo, Some(base));
            let segment = GitEngine::segment(repo, Some(base), tip).unwrap_or_default();
            let shared = segment.iter().filter(|o| remote_set.contains(o)).count();
            axes.push(RiskAxis {
                axe: "partage".into(),
                verdict: if shared > 0 { "attention" } else { "ok" }.into(),
                motif: if shared > 0 {
                    format!("{shared} commit(s) déjà poussé(s) : confirmation renforcée exigée, force-push à coordonner")
                } else {
                    "aucun commit du segment n'est présent sur un remote".into()
                },
            });
            let signed = segment
                .iter()
                .filter(|o| repo.extract_signature(o, None).is_ok())
                .count();
            axes.push(RiskAxis {
                axe: "signatures".into(),
                verdict: if signed > 0 { "attention" } else { "ok" }.into(),
                motif: if signed > 0 {
                    format!("{signed} commit(s) signé(s) perdront leur signature")
                } else {
                    "aucun commit signé dans le segment".into()
                },
            });
        }
        let drops = plan
            .ops
            .iter()
            .filter(|o| matches!(o.op, Operation::Drop { .. }))
            .count();
        axes.push(RiskAxis {
            axe: "pertes".into(),
            verdict: if drops > 0 { "attention" } else { "ok" }.into(),
            motif: if drops > 0 {
                format!("{drops} commit(s) abandonné(s) : contenu retiré de la branche (récupérable via backup)")
            } else {
                "aucune suppression de contenu".into()
            },
        });
        axes.push(RiskAxis {
            axe: "réversibilité".into(),
            verdict: "ok".into(),
            motif: "backup branche + tag créés avant application ; rollback en un clic tant que la branche n'avance pas".into(),
        });
        axes
    }
}

/// T6 : propriétés de `compile()` sur des séquences d'opérations aléatoires.
/// Complète les tests exemple-par-exemple : quelle que soit l'entrée, `compile`
/// ne PANIQUE jamais et, en cas de succès, respecte les invariants structurels.
#[cfg(test)]
mod prop_tests {
    use super::*;
    use proptest::prelude::*;

    fn oid(i: usize) -> Oid {
        Oid::from_str(&format!("{:040x}", i + 1)).unwrap()
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(400))]
        #[test]
        fn compile_never_panics_and_preserves_commits(
            n in 1usize..=8,
            descriptors in prop::collection::vec(
                (0u8..5, any::<usize>(), any::<usize>(), any::<usize>()),
                0..7,
            ),
        ) {
            let segment: Vec<Oid> = (0..n).map(oid).collect();
            let seg_set: std::collections::HashSet<Oid> = segment.iter().copied().collect();

            let mut ops = Vec::new();
            for (k, (kind, a, b, c)) in descriptors.iter().enumerate() {
                let (ai, bi, ci) = (a % n, b % n, c % n);
                let operation = match kind % 5 {
                    0 => Operation::Reword {
                        target: segment[ai].to_string(),
                        new_message: "msg".into(),
                    },
                    1 => Operation::Squash {
                        targets: vec![segment[ai].to_string(), segment[bi].to_string()],
                        new_message: "sq".into(),
                    },
                    2 => Operation::Fixup {
                        targets: vec![segment[ai].to_string(), segment[bi].to_string()],
                    },
                    3 => Operation::Drop {
                        target: segment[ai].to_string(),
                        reason: "d".into(),
                    },
                    _ => {
                        // Rotation du segment complet = permutation valide.
                        let order: Vec<String> = segment
                            .iter()
                            .cycle()
                            .skip(ci)
                            .take(n)
                            .map(|o| o.to_string())
                            .collect();
                        Operation::Reorder { order }
                    }
                };
                ops.push(PlanOp {
                    seq: k as u32,
                    op: operation,
                    origin: String::new(),
                    risk: Risk::Low,
                    approved_by: None,
                    approved_at: None,
                });
            }

            // Ne doit jamais paniquer ; peut échouer (Err) sur une combinaison invalide.
            if let Ok(compiled) = compile(&segment, &ops) {
                prop_assert!(!compiled.groups.is_empty());
                // Chaque commit du segment apparaît AU PLUS une fois (leader ou fixup),
                // et jamais hors du segment : ni perte silencieuse ni duplication.
                let mut seen = std::collections::HashSet::new();
                for g in &compiled.groups {
                    prop_assert!(seg_set.contains(&g.leader));
                    prop_assert!(seen.insert(g.leader), "leader vu deux fois");
                    for f in &g.fixups {
                        prop_assert!(seg_set.contains(f));
                        prop_assert!(seen.insert(*f), "commit dans deux groupes");
                    }
                }
                // Reword-only ⇒ aucune modification de structure, mêmes leaders dans l'ordre.
                let only_reword = ops.iter().all(|o| matches!(o.op, Operation::Reword { .. }));
                if only_reword {
                    prop_assert!(!compiled.structure_changed);
                    let leaders: Vec<Oid> = compiled.groups.iter().map(|g| g.leader).collect();
                    prop_assert_eq!(leaders, segment.clone());
                }
            }
        }
    }
}
