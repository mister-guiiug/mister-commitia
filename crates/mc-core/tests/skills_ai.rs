mod common;

use common::*;
use mc_core::model::*;

/// Les six skills du dépôt se chargent sans erreur.
#[test]
fn skills_load_all_six() {
    let (skills, errors) = mc_core::skills::load_dir(&skills_dir()).unwrap();
    assert!(errors.is_empty(), "erreurs de chargement : {errors:?}");
    let names: Vec<&str> = skills.iter().map(|s| s.def.name.as_str()).collect();
    for expected in [
        "ai-signature-cleaner",
        "ci-cleanup-policy",
        "commit-synthesis",
        "conventional-commits",
        "risk-reviewer",
        "squash-advisor",
    ] {
        assert!(names.contains(&expected), "skill absente : {expected}");
    }
    // La skill de référence a bien son prompt externe et ses tests.
    let cc = skills
        .iter()
        .find(|s| s.def.name == "conventional-commits")
        .unwrap();
    assert!(!cc.prompt.is_empty());
    assert!(cc.test_cases.len() >= 3);
}

/// CA-7 : par défaut (keep-required), la skill de nettoyage REFUSE — et rien
/// n'est jamais appliqué automatiquement.
#[tokio::test]
async fn ca7_cleaner_refuses_under_keep_required() {
    let (f, shas) = feature_fixture();
    let (core, repo_id) = core_with(&f);

    let proposals = core
        .proposals_generate(
            &repo_id,
            "ai-signature-cleaner",
            vec![vec![shas[2].to_string()]],
            None,
            false,
        )
        .await
        .unwrap();
    assert_eq!(proposals.len(), 1);
    let p = &proposals[0];
    assert_eq!(p.status, ProposalStatus::Refused);
    assert!(p.after.is_none());
    assert!(p.explanation.contains("keep-required"), "{}", p.explanation);

    // Aucune proposition n'a touché le dépôt.
    assert!(mc_core::gitx::GitEngine::workdir_clean(&f.repo).unwrap());
}

/// CA-7 : politique « normalization-allowed » → nettoyage proposé, mentions
/// retirées, trailer protégé intact ; l'accepter reste une décision humaine.
#[tokio::test]
async fn ca7_cleaner_cleans_when_allowed_and_protects_trailers() {
    let (f, _) = feature_fixture();
    // Commit mêlant mention générée ET trailer protégé.
    let mixed = commit(
        &f.repo,
        &[("src/mix.rs", "// mix\n")],
        "feat(mix): add mixed commit\n\nDetail utile.\n\nSigned-off-by: Jane Doe <jane@example.org>\n🤖 Generated with Claude Code\nCo-Authored-By: Claude <noreply@anthropic.com>",
        1_700_000_600,
    );
    let (core, repo_id) = core_with(&f);
    let mut gov = Governance::default();
    gov.ai_attribution_policy = AiAttributionPolicy::NormalizationAllowed;
    core.repo_update_governance(&repo_id, gov, vec!["main".into()])
        .unwrap();

    let proposals = core
        .proposals_generate(
            &repo_id,
            "ai-signature-cleaner",
            vec![vec![mixed.to_string()]],
            None,
            false,
        )
        .await
        .unwrap();
    let p = &proposals[0];
    assert_eq!(p.status, ProposalStatus::Proposed, "{}", p.explanation);
    let after = p.after.as_ref().unwrap();
    assert!(!after.contains("Generated with Claude Code"));
    assert!(!after.contains("Co-Authored-By: Claude"));
    assert!(after.contains("Detail utile."));
    assert!(after.contains("Signed-off-by: Jane Doe <jane@example.org>"));

    // Décision humaine : accepter.
    let decided = core.proposal_decide(&p.id, "accept", None).unwrap();
    assert_eq!(decided.status, ProposalStatus::Accepted);
    assert_eq!(decided.decision.as_deref(), p.after.as_deref());
}

/// CA-7 : un message ÉDITÉ repasse par les garde-fous — supprimer un trailer
/// protégé à la main est refusé.
#[tokio::test]
async fn ca7_edited_message_is_revalidated() {
    let (f, shas) = feature_fixture();
    let (core, repo_id) = core_with(&f);

    // c4 porte un Signed-off-by (protégé par défaut).
    let proposals = core
        .proposals_generate(
            &repo_id,
            "conventional-commits",
            vec![vec![shas[3].to_string()]],
            None,
            false,
        )
        .await
        .unwrap();
    let p = &proposals[0];
    assert_eq!(p.status, ProposalStatus::Proposed, "{}", p.explanation);
    assert!(
        p.after.as_ref().unwrap().contains("Signed-off-by: Jane Doe"),
        "l'assistant conserve le trailer : {:?}",
        p.after
    );
    assert!(p.after.as_ref().unwrap().contains("JIRA-123"));

    let err = core
        .proposal_decide(
            &p.id,
            "edit",
            Some("docs: réécrit sans le trailer\n\nRefs: JIRA-123".into()),
        )
        .unwrap_err()
        .to_string();
    assert!(err.contains("Signed-off-by"), "{err}");

    // Une édition qui conserve trailer et référence passe.
    let ok = core
        .proposal_decide(
            &p.id,
            "edit",
            Some(
                "docs(pay): document payment flow\n\nRefs: JIRA-123\n\nSigned-off-by: Jane Doe <jane@example.org>"
                    .into(),
            ),
        )
        .unwrap();
    assert_eq!(ok.status, ProposalStatus::Edited);
}

/// CA-9 : fournisseur distant sans consentement explicite → refus AVANT tout
/// appel réseau (l'endpoint bidon n'est jamais contacté).
#[tokio::test]
async fn ca9_remote_provider_requires_consent() {
    let (f, shas) = feature_fixture();
    let (core, repo_id) = core_with(&f);
    core.ai_provider_save(
        AiProviderKind::OpenAiCompat,
        Some("http://127.0.0.1:1".into()),
        Some("model-x".into()),
        Some("cle-secrete-123456".into()),
        true,
    )
    .unwrap();

    let err = core
        .proposals_generate(
            &repo_id,
            "conventional-commits",
            vec![vec![shas[1].to_string()]],
            None,
            false,
        )
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("consentement"), "{err}");

    // L'aperçu de consentement est disponible et contient bien le message.
    let preview = core
        .ai_preview(&repo_id, "conventional-commits", vec![shas[1].to_string()])
        .unwrap();
    assert!(preview.contains("[system]"));
    assert!(preview.contains("wip"));
}

/// Synthèse de groupe : références et BREAKING CHANGE conservés.
#[tokio::test]
async fn synthesis_preserves_refs_and_breaking() {
    let (f, _) = feature_fixture();
    let b1 = commit(
        &f.repo,
        &[("src/api.rs", "// v1\n")],
        "feat(api): switch to v2 auth\n\nBREAKING CHANGE: v1 tokens rejected\n\nRefs: JIRA-77",
        1_700_000_610,
    );
    let b2 = commit(&f.repo, &[("src/api.rs", "// v2\n")], "wip", 1_700_000_620);
    let b3 = commit(&f.repo, &[("src/api.rs", "// v3\n")], "fix tests", 1_700_000_630);
    let (core, repo_id) = core_with(&f);

    let proposals = core
        .proposals_generate(
            &repo_id,
            "commit-synthesis",
            vec![vec![b1.to_string(), b2.to_string(), b3.to_string()]],
            None,
            false,
        )
        .await
        .unwrap();
    let p = &proposals[0];
    assert_eq!(p.status, ProposalStatus::Proposed, "{}", p.explanation);
    let after = p.after.as_ref().unwrap();
    assert!(after.contains("BREAKING CHANGE: v1 tokens rejected"));
    assert!(after.contains("JIRA-77"));
    assert!(after.lines().next().unwrap().starts_with("feat"));
}

/// Le runner de self-tests des skills passe en mode déterministe local.
#[test]
fn skill_selftests_pass_for_local_capable_skills() {
    let f = init_repo();
    commit(&f.repo, &[("a.txt", "a\n")], "chore: seed", 1_700_000_000);
    let (core, _) = core_with(&f);
    for name in ["conventional-commits", "commit-synthesis", "ai-signature-cleaner"] {
        let results = core.skill_run_tests(name).unwrap();
        assert!(!results.is_empty(), "{name} sans cas de test");
        for r in &results {
            assert!(r.passed, "{name}/{} : {}", r.case, r.detail);
        }
    }
}
