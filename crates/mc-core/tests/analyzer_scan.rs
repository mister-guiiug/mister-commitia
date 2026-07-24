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
