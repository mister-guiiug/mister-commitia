mod common;

use common::*;
use git2::Repository;
use mc_core::model::*;
use mc_core::task::{CancelToken, TaskCtx, TaskEvent};

/// Réécrit (reword) puis applique un plan sur `feature/checkout`, la faisant
/// diverger de son remote-tracking. Retourne le nouveau sommet local.
fn rewrite_and_apply(core: &mc_core::Core, repo_id: &str, target: git2::Oid) -> String {
    let plan = core.plan_new(repo_id, "feature/checkout").unwrap();
    let plan = core
        .plan_set_ops(
            &plan.id,
            vec![op(
                1,
                Operation::Reword {
                    target: target.to_string(),
                    new_message: "refactor(pay): message reecrit".into(),
                },
            )],
        )
        .unwrap();
    core.plan_dry_run(&plan.id).unwrap();
    // Branche partagée (poussée) → confirmation renforcée exigée.
    let applied = core
        .plan_apply(&plan.id, Some("feature/checkout".into()))
        .unwrap();
    applied
        .mapping
        .last()
        .map(|m| m.new.clone())
        .expect("nouveau sommet")
}

/// CA-3 + CA-4 : le dry-run construit le résultat réel dans refs/mc/preview
/// sans toucher la branche ; un plan 100 % reword ne change aucun arbre.
#[test]
fn ca3_ca4_dry_run_reword_preserves_trees_and_branch() {
    let (f, shas) = feature_fixture();
    let (core, repo_id) = core_with(&f);

    let plan = core.plan_new(&repo_id, "feature/checkout").unwrap();
    let plan = core
        .plan_set_ops(
            &plan.id,
            vec![op(
                1,
                Operation::Reword {
                    target: shas[1].to_string(),
                    new_message: "refactor(pay): iterate on payment flow".into(),
                },
            )],
        )
        .unwrap();

    let tip_before = f.repo.refname_to_id("refs/heads/feature/checkout").unwrap();
    let plan = core.plan_dry_run(&plan.id).unwrap();

    // Branche et worktree intacts (CA-3).
    assert_eq!(
        f.repo.refname_to_id("refs/heads/feature/checkout").unwrap(),
        tip_before
    );
    assert_eq!(plan.status, PlanStatus::DryRunOk);

    // Arbres identiques pour chaque paire du mapping (CA-4).
    let preview = f
        .repo
        .refname_to_id(plan.preview_ref.as_ref().unwrap())
        .unwrap();
    assert_ne!(
        preview, tip_before,
        "les messages ont changé, les SHA aussi"
    );
    assert_eq!(
        f.repo.find_commit(preview).unwrap().tree_id(),
        f.repo.find_commit(tip_before).unwrap().tree_id()
    );
    for m in &plan.mapping {
        let old = f
            .repo
            .find_commit(git2::Oid::from_str(&m.old[0]).unwrap())
            .unwrap();
        let new = f
            .repo
            .find_commit(git2::Oid::from_str(&m.new).unwrap())
            .unwrap();
        assert_eq!(old.tree_id(), new.tree_id());
    }
    // Le reword est présent dans la préview.
    let reworded = plan
        .mapping
        .iter()
        .find(|m| m.old[0] == shas[1].to_string())
        .unwrap();
    let msg = f
        .repo
        .find_commit(git2::Oid::from_str(&reworded.new).unwrap())
        .unwrap()
        .message()
        .unwrap()
        .to_string();
    assert!(msg.starts_with("refactor(pay): iterate on payment flow"));
    // Les commits AVANT le premier modifié gardent leur SHA (adressage contenu).
    assert_eq!(plan.mapping[0].old[0], plan.mapping[0].new);
}

/// T2 : un dry-run annulé n'écrit RIEN (pas de préview, statut inchangé) et
/// les phases émises portent des libellés exploitables par l'UI.
#[test]
fn t2_dry_run_cancelled_writes_nothing_and_emits_phases() {
    let (f, shas) = feature_fixture();
    let (core, repo_id) = core_with(&f);
    let plan = core.plan_new(&repo_id, "feature/checkout").unwrap();
    let plan = core
        .plan_set_ops(
            &plan.id,
            vec![op(
                1,
                Operation::Reword {
                    target: shas[1].to_string(),
                    new_message: "refactor(pay): iterate on payment flow".into(),
                },
            )],
        )
        .unwrap();

    // Jeton déjà annulé → interruption avant tout travail Git.
    let cancel = CancelToken::new();
    cancel.cancel();
    let ctx = TaskCtx::new("plan_dry_run", "t-dr", cancel, |_| {});
    let err = core.plan_dry_run_with(&plan.id, &ctx).unwrap_err();
    assert_eq!(err.code(), "cancelled");
    let after = core.plan_get(&plan.id).unwrap();
    assert_eq!(after.status, PlanStatus::Draft, "statut inchangé");
    assert!(after.preview_ref.is_none());
    assert!(
        f.repo
            .find_reference(&format!("refs/mc/preview/{}", plan.id))
            .is_err(),
        "aucune réf de préview créée"
    );

    // Exécution normale : les phases attendues sont émises, dans l'ordre.
    let phases = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let sink = phases.clone();
    let ctx = TaskCtx::new("plan_dry_run", "t-dr2", CancelToken::new(), move |p| {
        if let TaskEvent::Progress { phase, .. } = p.event {
            sink.lock().unwrap().push(phase);
        }
    });
    core.plan_dry_run_with(&plan.id, &ctx).unwrap();
    let phases = phases.lock().unwrap();
    assert_eq!(phases.first().unwrap(), "vérification de l'empreinte");
    assert!(phases.iter().any(|p| p == "réécriture des messages"));
    assert!(phases.iter().any(|p| p.contains("écriture de la préview")));
}

/// F4 : après réécriture, le preview signale la divergence et exige un push
/// forcé ; l'exécution sans confirmation est refusée, puis le force-with-lease
/// réécrit bien la branche distante.
#[tokio::test]
async fn f4_push_preview_and_forced_push_succeeds() {
    let (f, shas) = feature_fixture();
    let (core, repo_id) = core_with(&f);
    let bare = add_remote_and_push(&f, "feature/checkout");

    let new_tip = rewrite_and_apply(&core, &repo_id, shas[1]);

    let preview = core
        .push_preview(&repo_id, "feature/checkout", None)
        .await
        .unwrap();
    assert!(preview.can_push);
    assert!(preview.needs_force, "l'historique distant diverge");
    assert!(!preview.protected);
    assert!(preview.behind > 0 && preview.ahead > 0);
    assert_eq!(
        preview.remote_tip.as_deref(),
        Some(shas[3].to_string().as_str())
    );
    assert!(preview.open_prs.is_none(), "aucun accès GitHub fourni");

    // Sans confirmation → refus typé.
    let err = core
        .push_execute(&repo_id, "feature/checkout", None)
        .unwrap_err();
    assert_eq!(err.code(), "confirm_required");
    assert_eq!(err.expected(), Some("feature/checkout"));

    // Confirmation exacte → force-with-lease réussi.
    let res = core
        .push_execute(
            &repo_id,
            "feature/checkout",
            Some("feature/checkout".into()),
        )
        .unwrap();
    assert!(res.forced);
    assert_eq!(res.remote_tip, new_tip);

    // Le remote a bien été réécrit sur le nouveau sommet.
    let bare_repo = Repository::open(bare.path()).unwrap();
    let remote_head = bare_repo
        .refname_to_id("refs/heads/feature/checkout")
        .unwrap()
        .to_string();
    assert_eq!(remote_head, new_tip);

    // Journal : tentative avant résultat ok.
    let audit = core.audit_list(50).unwrap();
    let attempt = audit.iter().find(|e| e.action == "push_attempt").unwrap();
    let done = audit.iter().find(|e| e.action == "push").unwrap();
    assert!(attempt.seq < done.seq);
    assert_eq!(done.result, "ok");
}

/// F4 : le bail --force-with-lease protège le travail distant non revu — si le
/// remote a bougé depuis le dernier fetch, le push forcé est refusé par git et
/// n'écrase RIEN.
#[tokio::test]
async fn f4_force_with_lease_aborts_when_remote_moved() {
    let (f, shas) = feature_fixture();
    let (core, repo_id) = core_with(&f);
    let bare = add_remote_and_push(&f, "feature/checkout");

    rewrite_and_apply(&core, &repo_id, shas[1]);
    // Le preview fetch et fixe le bail sur le sommet distant courant (shas[3]).
    let preview = core
        .push_preview(&repo_id, "feature/checkout", None)
        .await
        .unwrap();
    assert!(preview.needs_force);

    // Un collègue déplace la branche distante (simulé : la ref du bare bouge).
    let bare_repo = Repository::open(bare.path()).unwrap();
    bare_repo
        .reference("refs/heads/feature/checkout", shas[0], true, "collegue")
        .unwrap();

    // Le push forcé s'appuie sur le bail (shas[3]) ≠ état réel (shas[0]) → abort.
    let err = core
        .push_execute(
            &repo_id,
            "feature/checkout",
            Some("feature/checkout".into()),
        )
        .unwrap_err();
    assert_eq!(err.code(), "git", "git refuse le push (stale info) : {err}");

    // Le remote est resté sur la position du collègue : rien écrasé.
    let bare_repo = Repository::open(bare.path()).unwrap();
    assert_eq!(
        bare_repo
            .refname_to_id("refs/heads/feature/checkout")
            .unwrap(),
        shas[0]
    );
}

/// F4 : le push forcé est refusé net sur une branche protégée.
#[tokio::test]
async fn f4_force_push_refused_on_protected_branch() {
    let (f, shas) = feature_fixture();
    let (core, repo_id) = core_with(&f);
    let _bare = add_remote_and_push(&f, "feature/checkout");
    rewrite_and_apply(&core, &repo_id, shas[1]);

    // On protège explicitement la branche de travail.
    let repo = core.repo_list().unwrap().into_iter().next().unwrap();
    core.repo_update_governance(
        &repo_id,
        repo.governance.clone(),
        vec!["main".into(), "feature/checkout".into()],
    )
    .unwrap();

    let err = core
        .push_execute(
            &repo_id,
            "feature/checkout",
            Some("feature/checkout".into()),
        )
        .unwrap_err();
    assert_eq!(err.code(), "refused");
    assert!(err.to_string().contains("protégée"), "{err}");
}

/// T10 : un reword à travers un segment contenant un MERGE réussit — la
/// topologie (le merge à 2 parents) et l'arbre final sont préservés à
/// l'identique, seul le message change.
#[test]
fn t10_reword_across_merge_preserves_topology() {
    let (f, (c, _d, _e, m)) = merge_fixture();
    let (core, repo_id) = core_with(&f);

    let plan = core.plan_new(&repo_id, "feature/merge").unwrap();
    let plan = core
        .plan_set_ops(
            &plan.id,
            vec![op(
                1,
                Operation::Reword {
                    target: c.to_string(),
                    new_message: "feat: base feature (reformule)".into(),
                },
            )],
        )
        .unwrap();
    let plan = core.plan_dry_run(&plan.id).unwrap();
    assert_eq!(plan.status, PlanStatus::DryRunOk, "reword-merge accepté");

    let preview = f
        .repo
        .refname_to_id(plan.preview_ref.as_ref().unwrap())
        .unwrap();
    let new_tip = f.repo.find_commit(preview).unwrap();
    assert_eq!(new_tip.parent_count(), 2, "le merge est préservé");
    assert_eq!(
        new_tip.tree_id(),
        f.repo.find_commit(m).unwrap().tree_id(),
        "arbre final identique (reword pur)"
    );

    // Le message de C est réécrit dans la préview.
    let cnew = plan
        .mapping
        .iter()
        .find(|mm| mm.old[0] == c.to_string())
        .unwrap();
    let msg = f
        .repo
        .find_commit(git2::Oid::from_str(&cnew.new).unwrap())
        .unwrap()
        .message()
        .unwrap()
        .to_string();
    assert!(msg.contains("reformule"), "{msg}");
}

/// T10 : un changement de STRUCTURE à travers un merge reste refusé (garde-fou).
#[test]
fn t10_structure_change_across_merge_refused() {
    let (f, (c, _d, _e, _m)) = merge_fixture();
    let (core, repo_id) = core_with(&f);
    let plan = core.plan_new(&repo_id, "feature/merge").unwrap();
    let plan = core
        .plan_set_ops(
            &plan.id,
            vec![op(
                1,
                Operation::Drop {
                    target: c.to_string(),
                    reason: "x".into(),
                },
            )],
        )
        .unwrap();
    let err = core.plan_dry_run(&plan.id).unwrap_err().to_string();
    assert!(err.contains("merge"), "{err}");
    assert!(err.contains("structure"), "{err}");
}

/// CA-3 : pas d'application sans dry-run du même plan.
#[test]
fn ca3_apply_requires_fresh_dry_run() {
    let (f, shas) = feature_fixture();
    let (core, repo_id) = core_with(&f);
    let plan = core.plan_new(&repo_id, "feature/checkout").unwrap();
    let plan = core
        .plan_set_ops(
            &plan.id,
            vec![op(
                1,
                Operation::Reword {
                    target: shas[1].to_string(),
                    new_message: "chore: x".into(),
                },
            )],
        )
        .unwrap();

    // Sans dry-run → refus.
    let err = core.plan_apply(&plan.id, None).unwrap_err().to_string();
    assert!(err.contains("dry-run"), "{err}");

    // Dry-run puis modification du plan → nouveau refus.
    core.plan_dry_run(&plan.id).unwrap();
    core.plan_set_ops(
        &plan.id,
        vec![op(
            1,
            Operation::Reword {
                target: shas[1].to_string(),
                new_message: "chore: y".into(),
            },
        )],
    )
    .unwrap();
    let err = core.plan_apply(&plan.id, None).unwrap_err().to_string();
    assert!(err.contains("dry-run"), "{err}");
}

/// CA-1 + CA-8 : backup systématique avant application, rollback intégral.
#[test]
fn ca1_ca8_apply_backup_then_rollback() {
    let (f, shas) = feature_fixture();
    let (core, repo_id) = core_with(&f);
    let plan = core.plan_new(&repo_id, "feature/checkout").unwrap();
    let plan = core
        .plan_set_ops(
            &plan.id,
            vec![op(
                1,
                Operation::Reword {
                    target: shas[2].to_string(),
                    new_message: "fix(pay): stabilise payment retries".into(),
                },
            )],
        )
        .unwrap();
    let old_tip = f.repo.refname_to_id("refs/heads/feature/checkout").unwrap();
    let plan = core.plan_dry_run(&plan.id).unwrap();
    let preview = f
        .repo
        .refname_to_id(plan.preview_ref.as_ref().unwrap())
        .unwrap();

    let plan = core.plan_apply(&plan.id, None).unwrap();
    assert_eq!(plan.status, PlanStatus::Applied);

    // Backup réf + tag pointent l'ancien sommet (CA-1).
    let backup = plan.backup_ref.as_ref().unwrap();
    assert_eq!(f.repo.refname_to_id(backup).unwrap(), old_tip);
    assert_eq!(
        f.repo
            .refname_to_id(plan.backup_tag.as_ref().unwrap())
            .unwrap(),
        old_tip
    );
    // La branche pointe le résultat du dry-run.
    assert_eq!(
        f.repo.refname_to_id("refs/heads/feature/checkout").unwrap(),
        preview
    );

    // Rollback (CA-8).
    let plan = core.plan_rollback(&plan.id).unwrap();
    assert_eq!(plan.status, PlanStatus::RolledBack);
    assert_eq!(
        f.repo.refname_to_id("refs/heads/feature/checkout").unwrap(),
        old_tip
    );
}

/// CA-8 : rollback refusé (et guidé) si la branche a avancé.
#[test]
fn ca8_rollback_guided_when_branch_moved() {
    let (f, shas) = feature_fixture();
    let (core, repo_id) = core_with(&f);
    let plan = core.plan_new(&repo_id, "feature/checkout").unwrap();
    let plan = core
        .plan_set_ops(
            &plan.id,
            vec![op(
                1,
                Operation::Reword {
                    target: shas[1].to_string(),
                    new_message: "chore: z".into(),
                },
            )],
        )
        .unwrap();
    core.plan_dry_run(&plan.id).unwrap();
    core.plan_apply(&plan.id, None).unwrap();

    commit(
        &f.repo,
        &[("src/new.rs", "// après application\n")],
        "feat: after apply",
        1_700_000_900,
    );
    let err = core.plan_rollback(&plan.id).unwrap_err().to_string();
    assert!(err.contains("avancé"), "{err}");
    assert!(
        err.contains("refs/mc/backup/"),
        "la procédure guidée cite le backup : {err}"
    );
}

/// CA-5 : un plan est refusé si le dépôt a bougé depuis son empreinte.
#[test]
fn ca5_fingerprint_drift_refused() {
    let (f, shas) = feature_fixture();
    let (core, repo_id) = core_with(&f);
    let plan = core.plan_new(&repo_id, "feature/checkout").unwrap();
    let plan = core
        .plan_set_ops(
            &plan.id,
            vec![op(
                1,
                Operation::Reword {
                    target: shas[1].to_string(),
                    new_message: "chore: v".into(),
                },
            )],
        )
        .unwrap();
    commit(
        &f.repo,
        &[("src/drift.rs", "// drift\n")],
        "feat: drift",
        1_700_000_800,
    );
    let err = core.plan_dry_run(&plan.id).unwrap_err().to_string();
    assert!(err.contains("empreinte"), "{err}");
}

/// CA-5 : reproductibilité — deux dry-runs du même plan produisent le même SHA.
#[test]
fn ca5_dry_run_is_reproducible() {
    let (f, shas) = feature_fixture();
    let (core, repo_id) = core_with(&f);
    let plan = core.plan_new(&repo_id, "feature/checkout").unwrap();
    let ops = vec![op(
        1,
        Operation::Reword {
            target: shas[2].to_string(),
            new_message: "fix(pay): stable".into(),
        },
    )];
    let plan = core.plan_set_ops(&plan.id, ops.clone()).unwrap();
    let p1 = core.plan_dry_run(&plan.id).unwrap();
    let first = p1.mapping.last().unwrap().new.clone();

    let plan = core.plan_set_ops(&p1.id, ops).unwrap();
    let p2 = core.plan_dry_run(&plan.id).unwrap();
    assert_eq!(first, p2.mapping.last().unwrap().new);
}

/// CA-6 : branche par défaut/protégée bloquée dès la création du plan.
#[test]
fn ca6_protected_branch_blocked() {
    let (f, _) = feature_fixture();
    let (core, repo_id) = core_with(&f);
    let err = core.plan_new(&repo_id, "main").unwrap_err().to_string();
    assert!(
        err.contains("protégée") || err.contains("réécrivable"),
        "{err}"
    );
}

/// CA-6 : branche partagée (présente sur un remote) → confirmation renforcée
/// par saisie exacte du nom de branche.
#[test]
fn ca6_shared_branch_requires_typed_confirmation() {
    let (f, shas) = feature_fixture();
    let _bare = add_remote_and_push(&f, "feature/checkout");
    let (core, repo_id) = core_with(&f);
    let plan = core.plan_new(&repo_id, "feature/checkout").unwrap();
    let plan = core
        .plan_set_ops(
            &plan.id,
            vec![op(
                1,
                Operation::Reword {
                    target: shas[1].to_string(),
                    new_message: "chore: shared".into(),
                },
            )],
        )
        .unwrap();
    core.plan_dry_run(&plan.id).unwrap();

    let err = core.plan_apply(&plan.id, None).unwrap_err().to_string();
    assert!(err.contains("partagée"), "{err}");
    let err = core
        .plan_apply(&plan.id, Some("mauvaise-saisie".into()))
        .unwrap_err()
        .to_string();
    assert!(err.contains("partagée"), "{err}");

    let plan = core
        .plan_apply(&plan.id, Some("feature/checkout".into()))
        .unwrap();
    assert_eq!(plan.status, PlanStatus::Applied);
}

/// Squash + drop via le sequencer natif : structure, messages, contenu.
#[test]
fn squash_and_drop_full_pipeline() {
    let (f, shas) = feature_fixture();
    let (core, repo_id) = core_with(&f);
    let plan = core.plan_new(&repo_id, "feature/checkout").unwrap();
    let plan = core
        .plan_set_ops(
            &plan.id,
            vec![
                op(
                    1,
                    Operation::Squash {
                        targets: vec![shas[1].to_string(), shas[2].to_string()],
                        new_message: "fix(pay): stabilize payment flow".into(),
                    },
                ),
                op(
                    2,
                    Operation::Drop {
                        target: shas[3].to_string(),
                        reason: "doc provisoire".into(),
                    },
                ),
            ],
        )
        .unwrap();

    let plan = core.plan_dry_run(&plan.id).unwrap();
    let preview = f
        .repo
        .refname_to_id(plan.preview_ref.as_ref().unwrap())
        .unwrap();

    // Structure attendue : base → c1 → (c2+c3 squashés). c4 abandonné.
    // c1 étant intact ET premier, il garde son SHA.
    assert_eq!(plan.mapping.len(), 2);
    assert_eq!(plan.mapping[0].old, vec![shas[0].to_string()]);
    assert_eq!(
        plan.mapping[1].old,
        vec![shas[1].to_string(), shas[2].to_string()]
    );
    let squashed = f
        .repo
        .find_commit(git2::Oid::from_str(&plan.mapping[1].new).unwrap())
        .unwrap();
    assert!(squashed
        .message()
        .unwrap()
        .starts_with("fix(pay): stabilize payment flow"));
    assert_eq!(preview, squashed.id());

    // Le drop retire docs/pay.md de l'arbre final.
    let tree = squashed.tree().unwrap();
    assert!(tree.get_path(std::path::Path::new("docs/pay.md")).is_err());
    // Mais le contenu squashé (src/pay.rs v3) est conservé.
    assert!(tree.get_path(std::path::Path::new("src/pay.rs")).is_ok());

    // Application : le worktree suit (reset --hard) puisque HEAD est dessus.
    core.plan_apply(&plan.id, None).unwrap();
    assert!(!f.dir.path().join("docs/pay.md").exists());

    // Rollback : le fichier revient.
    core.plan_rollback(&plan.id).unwrap();
    assert!(f.dir.path().join("docs/pay.md").exists());
}

/// F2 : réordonnancement via le sequencer — l'ordre final suit l'op Reorder,
/// l'arbre final reste identique (aucun drop).
#[test]
fn reorder_via_plan_changes_order_keeps_final_tree() {
    let (f, shas) = feature_fixture();
    let (core, repo_id) = core_with(&f);
    let plan = core.plan_new(&repo_id, "feature/checkout").unwrap();
    // c4 (docs seul, fichiers indépendants) remonte en 2e position.
    let order = vec![
        shas[0].to_string(),
        shas[3].to_string(),
        shas[1].to_string(),
        shas[2].to_string(),
    ];
    let plan = core
        .plan_set_ops(&plan.id, vec![op(1, Operation::Reorder { order })])
        .unwrap();
    let plan = core.plan_dry_run(&plan.id).unwrap();

    let old_order: Vec<String> = plan.mapping.iter().map(|m| m.old[0].clone()).collect();
    assert_eq!(
        old_order,
        vec![
            shas[0].to_string(),
            shas[3].to_string(),
            shas[1].to_string(),
            shas[2].to_string()
        ]
    );
    // Arbre final préservé (mêmes changements, autre ordre).
    let preview = f
        .repo
        .refname_to_id(plan.preview_ref.as_ref().unwrap())
        .unwrap();
    let tip = f.repo.refname_to_id("refs/heads/feature/checkout").unwrap();
    assert_eq!(
        f.repo.find_commit(preview).unwrap().tree_id(),
        f.repo.find_commit(tip).unwrap().tree_id()
    );
}

/// Un plan appliqué est immuable.
#[test]
fn applied_plan_is_immutable() {
    let (f, shas) = feature_fixture();
    let (core, repo_id) = core_with(&f);
    let plan = core.plan_new(&repo_id, "feature/checkout").unwrap();
    let plan = core
        .plan_set_ops(
            &plan.id,
            vec![op(
                1,
                Operation::Reword {
                    target: shas[1].to_string(),
                    new_message: "chore: immuable".into(),
                },
            )],
        )
        .unwrap();
    core.plan_dry_run(&plan.id).unwrap();
    core.plan_apply(&plan.id, None).unwrap();
    let err = core.plan_set_ops(&plan.id, vec![]).unwrap_err().to_string();
    assert!(err.contains("immuable"), "{err}");
}

/// Export/import : le plan rejoué depuis JSON redonne le même résultat.
#[test]
fn plan_export_import_reproduces() {
    let (f, shas) = feature_fixture();
    let (core, repo_id) = core_with(&f);
    let plan = core.plan_new(&repo_id, "feature/checkout").unwrap();
    let plan = core
        .plan_set_ops(
            &plan.id,
            vec![op(
                1,
                Operation::Reword {
                    target: shas[2].to_string(),
                    new_message: "fix(pay): exported".into(),
                },
            )],
        )
        .unwrap();
    let p1 = core.plan_dry_run(&plan.id).unwrap();
    let exported = core.plan_export(&p1.id).unwrap();

    let imported = core.plan_import(&repo_id, &exported).unwrap();
    assert_eq!(imported.status, PlanStatus::Draft);
    let p2 = core.plan_dry_run(&imported.id).unwrap();
    assert_eq!(
        p1.mapping.last().unwrap().new,
        p2.mapping.last().unwrap().new
    );
}

/// Panneau risques : partage et pertes remontent en « attention ».
#[test]
fn risk_report_flags_shared_and_drops() {
    let (f, shas) = feature_fixture();
    let _bare = add_remote_and_push(&f, "feature/checkout");
    let (core, repo_id) = core_with(&f);
    let plan = core.plan_new(&repo_id, "feature/checkout").unwrap();
    let plan = core
        .plan_set_ops(
            &plan.id,
            vec![op(
                1,
                Operation::Drop {
                    target: shas[3].to_string(),
                    reason: "test".into(),
                },
            )],
        )
        .unwrap();
    let axes = core.plan_risk(&plan.id).unwrap();
    let get = |name: &str| axes.iter().find(|a| a.axe == name).unwrap();
    assert_eq!(get("partage").verdict, "attention");
    assert_eq!(get("pertes").verdict, "attention");
    assert_eq!(get("branche").verdict, "ok");
}
