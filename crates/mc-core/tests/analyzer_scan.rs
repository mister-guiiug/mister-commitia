mod common;

use common::*;
use mc_core::gitx::GitEngine;
use mc_core::model::FlagKind;

/// CA-2 : l'analyse détecte messages faibles, non-conformité et mentions
/// générées — sans AUCUN effet de bord sur le dépôt.
#[test]
fn ca2_analyze_flags_without_side_effects() {
    let (f, shas) = feature_fixture();
    let (core, repo_id) = core_with(&f);

    let head_before = f
        .repo
        .refname_to_id("refs/heads/feature/checkout")
        .unwrap();

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
    assert!(flag_for(&shas[1], FlagKind::WeakMessage), "« wip » doit être faible");
    assert!(flag_for(&shas[2], FlagKind::AiSignature), "mention générée détectée");
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
