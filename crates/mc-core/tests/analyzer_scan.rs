mod common;

use std::sync::{Arc, Mutex};

use common::*;
use mc_core::gitx::GitEngine;
use mc_core::model::FlagKind;
use mc_core::task::{CancelToken, TaskCtx, TaskEvent, TaskPayload};

/// CA-2 : l'analyse détecte messages faibles, non-conformité et mentions
/// générées — sans AUCUN effet de bord sur le dépôt.
#[test]
fn ca2_analyze_flags_without_side_effects() {
    let (f, shas) = feature_fixture();
    let (core, repo_id) = core_with(&f);

    let head_before = f.repo.refname_to_id("refs/heads/feature/checkout").unwrap();

    let scan = core
        .repo_scan(&repo_id, Some("feature/checkout".into()))
        .unwrap();

    assert_eq!(scan.commits.len(), 4, "segment merge-base..tip attendu");
    assert_eq!(scan.branch, "feature/checkout");

    let flag_for = |sha: &git2::Oid, kind: FlagKind| {
        scan.report
            .flags
            .iter()
            .any(|fl| fl.sha == sha.to_string() && fl.kind == kind)
    };
    assert!(
        flag_for(&shas[1], FlagKind::WeakMessage),
        "« wip » doit être faible"
    );
    assert!(
        flag_for(&shas[2], FlagKind::AiSignature),
        "mention générée détectée"
    );
    assert!(
        flag_for(&shas[3], FlagKind::NonConventional),
        "« update JIRA-123 » n'est pas conforme"
    );
    assert_eq!(scan.report.ai_signatures, 1);
    assert!(scan.report.conform >= 1, "c1 est conforme");

    // Aucun effet de bord : réfs intactes, worktree propre.
    assert_eq!(
        f.repo.refname_to_id("refs/heads/feature/checkout").unwrap(),
        head_before
    );
    assert!(GitEngine::workdir_clean(&f.repo).unwrap());
    assert!(f.repo.find_reference("refs/mc/preview").is_err());
}

/// F6 : choix explicite de la base du segment. La base forcée (branche/tag/SHA)
/// remplace le merge-base auto ; elle doit être un ancêtre STRICT du sommet.
#[test]
fn f6_base_override_selects_segment() {
    let (f, shas) = feature_fixture();
    let (core, repo_id) = core_with(&f);
    let ctx = TaskCtx::noop("scan");

    // Base auto (merge-base main..feature) : 4 commits.
    let full = core
        .repo_scan(&repo_id, Some("feature/checkout".into()))
        .unwrap();
    assert_eq!(full.commits.len(), 4);

    // Base forcée sur c2 (shas[1]) → segment réduit à {c3, c4} (base exclue).
    let scoped = core
        .repo_scan_base(
            &repo_id,
            Some("feature/checkout".into()),
            Some(shas[1].to_string()),
            &ctx,
        )
        .unwrap();
    assert_eq!(scoped.commits.len(), 2);
    assert_eq!(scoped.base.as_deref(), Some(shas[1].to_string().as_str()));
    assert_eq!(scoped.commits[0].sha, shas[2].to_string());
    assert_eq!(scoped.commits[1].sha, shas[3].to_string());

    // Base == sommet → segment vide, refusé.
    let err = core
        .repo_scan_base(
            &repo_id,
            Some("feature/checkout".into()),
            Some(shas[3].to_string()),
            &ctx,
        )
        .unwrap_err()
        .to_string();
    assert!(err.contains("segment"), "{err}");

    // Base non ancêtre (commit de la feature vu depuis main) → refusé.
    let err = core
        .repo_scan_base(
            &repo_id,
            Some("main".into()),
            Some(shas[3].to_string()),
            &ctx,
        )
        .unwrap_err()
        .to_string();
    assert!(err.contains("ancêtre"), "{err}");

    // Référence inconnue → introuvable.
    let err = core
        .repo_scan_base(
            &repo_id,
            Some("feature/checkout".into()),
            Some("pas-une-ref".into()),
            &ctx,
        )
        .unwrap_err()
        .to_string();
    assert!(err.contains("introuvable"), "{err}");
}

/// F3 : le diff d'un commit est un patch unifié exploitable.
#[test]
fn commit_diff_returns_unified_patch() {
    let (f, shas) = feature_fixture();
    let (core, repo_id) = core_with(&f);
    let patch = core.commit_diff(&repo_id, &shas[0].to_string()).unwrap();
    assert!(patch.contains("diff --git"), "{patch}");
    assert!(patch.contains("src/pay.rs"));
    assert!(patch.contains("+pub fn pay()"));
}

/// T2 : le scan émet une progression par commit (task_id propagé) et un jeton
/// déjà annulé interrompt AVANT tout travail (code stable `cancelled`).
#[test]
fn t2_scan_progress_events_and_precancelled_token() {
    let (f, _shas) = feature_fixture();
    let (core, repo_id) = core_with(&f);

    let events: Arc<Mutex<Vec<TaskPayload>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    let ctx = TaskCtx::new("repo_scan", "t-scan", CancelToken::new(), move |p| {
        sink.lock().unwrap().push(p)
    });
    core.repo_scan_with(&repo_id, Some("feature/checkout".into()), &ctx)
        .unwrap();

    let events = events.lock().unwrap();
    assert!(events.iter().all(|p| p.task_id == "t-scan"));
    let reads: Vec<(u64, Option<u64>)> = events
        .iter()
        .filter_map(|p| match &p.event {
            TaskEvent::Progress {
                phase,
                current,
                total,
            } if phase == "lecture des commits" => Some((*current, *total)),
            _ => None,
        })
        .collect();
    assert_eq!(reads.len(), 4, "une émission par commit du segment");
    assert_eq!(reads.last().unwrap(), &(4, Some(4)));

    let cancel = CancelToken::new();
    cancel.cancel();
    let ctx = TaskCtx::new("repo_scan", "t-annule", cancel, |_| {});
    let err = core
        .repo_scan_with(&repo_id, Some("feature/checkout".into()), &ctx)
        .unwrap_err();
    assert_eq!(err.code(), "cancelled");
}

/// T2 : l'annulation en COURS de scan interrompt au point d'arrêt suivant.
#[test]
fn t2_scan_cancel_mid_flight_stops_at_next_checkpoint() {
    let (f, _shas) = feature_fixture();
    let (core, repo_id) = core_with(&f);

    let cancel = CancelToken::new();
    let trigger = cancel.clone();
    let ctx = TaskCtx::new("repo_scan", "t-vol", cancel, move |p| {
        if let TaskEvent::Progress { phase, current, .. } = &p.event {
            if phase == "lecture des commits" && *current == 2 {
                trigger.cancel(); // annulation déclenchée pendant la lecture
            }
        }
    });
    let err = core
        .repo_scan_with(&repo_id, Some("feature/checkout".into()), &ctx)
        .unwrap_err();
    assert_eq!(err.code(), "cancelled");
}

/// F1 : segment linéaire → une seule lane, et le parent du plus ancien commit
/// (la base) est hors segment (arête-borne).
#[test]
fn f1_graph_linear_single_lane_with_boundary_base() {
    let (f, shas) = feature_fixture();
    let (core, repo_id) = core_with(&f);
    let scan = core
        .repo_scan(&repo_id, Some("feature/checkout".into()))
        .unwrap();
    let g = &scan.graph;
    assert_eq!(g.lanes, 1, "chaîne linéaire : une lane");
    assert_eq!(g.nodes.len(), 4);
    assert!(g.nodes.iter().all(|n| n.lane == 0));
    // Le plus récent est en tête (row 0), le plus ancien en dernier.
    assert_eq!(g.nodes[0].sha, shas[3].to_string());
    assert_eq!(g.nodes[3].sha, shas[0].to_string());
    // Le plus ancien pointe vers la base, hors segment.
    let oldest = &g.nodes[3];
    assert_eq!(oldest.parents.len(), 1);
    assert!(!oldest.parents[0].in_segment, "la base est hors segment");
    // Les autres parents sont dans le segment.
    for n in &g.nodes[..3] {
        assert!(n.parents.iter().all(|p| p.in_segment));
        assert!(!n.is_merge);
    }
}

/// F1 : un merge interne rend le graphe non linéaire (≥ 2 lanes) ; le nœud de
/// merge a deux parents du segment qui convergent vers un même ancêtre.
#[test]
fn f1_graph_merge_uses_multiple_lanes() {
    let (f, (c, d, e, m)) = merge_fixture();
    let (core, repo_id) = core_with(&f);
    let scan = core
        .repo_scan(&repo_id, Some("feature/merge".into()))
        .unwrap();
    let g = &scan.graph;
    assert_eq!(g.nodes.len(), 4, "segment C,D,E,M");
    assert!(
        g.lanes >= 2,
        "un merge occupe au moins deux lanes : {}",
        g.lanes
    );

    let node = |oid: git2::Oid| g.nodes.iter().find(|n| n.sha == oid.to_string()).unwrap();
    let mn = node(m);
    assert!(mn.is_merge && mn.parents.len() == 2);
    assert!(
        mn.parents.iter().all(|p| p.in_segment),
        "D et E sont dans le segment"
    );
    // D et E vivent en parallèle → lanes distinctes.
    assert_ne!(
        node(d).lane,
        node(e).lane,
        "les deux lignes sont sur des colonnes différentes"
    );
    // C (le plus ancien) borne le segment vers la base (A, hors segment).
    let cn = node(c);
    assert_eq!(cn.row, 3);
    assert!(cn.parents.iter().all(|p| !p.in_segment));
    // Toutes les colonnes référencées restent bornées par la largeur annoncée.
    for n in &g.nodes {
        assert!(n.lane < g.lanes);
    }
}

/// T13 : le cache d'analyse par SHA est réutilisé entre scans (pas de
/// recomputation) ; `on_remote`, dépendant du contexte, est recalculé hors cache.
#[test]
fn t13_analysis_cache_reused_and_on_remote_recomputed() {
    let (f, _shas) = feature_fixture();
    let (core, repo_id) = core_with(&f);

    let s1 = core
        .repo_scan(&repo_id, Some("feature/checkout".into()))
        .unwrap();
    assert!(
        s1.commits.iter().all(|c| !c.on_remote),
        "rien sur le remote"
    );
    assert_eq!(core.analysis_cache_len(), 4);

    // Deuxième scan : cache stable (aucune nouvelle entrée), résultats identiques.
    let s2 = core
        .repo_scan(&repo_id, Some("feature/checkout".into()))
        .unwrap();
    assert_eq!(
        core.analysis_cache_len(),
        4,
        "cache réutilisé, pas de croissance"
    );
    assert_eq!(s1.commits[0].sha, s2.commits[0].sha);
    assert_eq!(s1.commits[1].files, s2.commits[1].files);

    // Après push, on_remote doit repasser à true alors que le cache sert encore.
    let _bare = add_remote_and_push(&f, "feature/checkout");
    let s3 = core
        .repo_scan(&repo_id, Some("feature/checkout".into()))
        .unwrap();
    assert!(
        s3.commits.iter().all(|c| c.on_remote),
        "on_remote recalculé hors cache après push"
    );
    assert_eq!(
        core.analysis_cache_len(),
        4,
        "toujours pas de recomputation"
    );
}

#[test]
fn commit_infos_carry_files_signatures_and_trailers() {
    let (f, shas) = feature_fixture();
    let (core, repo_id) = core_with(&f);
    let scan = core
        .repo_scan(&repo_id, Some("feature/checkout".into()))
        .unwrap();

    let c4 = scan
        .commits
        .iter()
        .find(|c| c.sha == shas[3].to_string())
        .unwrap();
    assert_eq!(c4.files, vec!["docs/pay.md".to_string()]);
    assert!(c4
        .trailers
        .iter()
        .any(|(k, v)| k == "Signed-off-by" && v.contains("Jane")));
    assert!(!c4.is_merge);
    assert!(!c4.on_remote, "pas de remote déclaré");
}
