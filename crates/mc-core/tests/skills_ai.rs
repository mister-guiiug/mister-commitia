mod common;

use std::sync::Mutex;

use common::mockhttp::MockServer;
use common::*;
use mc_core::ai::Provider;
use mc_core::model::*;
use mc_core::task::{CancelToken, TaskCtx, TaskEvent};

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
    let gov = Governance {
        ai_attribution_policy: AiAttributionPolicy::NormalizationAllowed,
        ..Governance::default()
    };
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
        p.after
            .as_ref()
            .unwrap()
            .contains("Signed-off-by: Jane Doe"),
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
    let b3 = commit(
        &f.repo,
        &[("src/api.rs", "// v3\n")],
        "fix tests",
        1_700_000_630,
    );
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

fn temp_skills() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().join("essai");
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(
        d.join("skill.yaml"),
        "apiVersion: mister-commitia/skill.v1\nname: essai\nversion: 1.0.0\nowner: t@example.org\nstatus: draft\ndescription: test\n",
    )
    .unwrap();
    dir
}

/// F8 : lecture/écriture d'une skill — validation YAML, nom immuable, audit.
#[test]
fn skill_editor_roundtrip_validates() {
    std::env::set_var("MC_SECRETS_MODE", "memory");
    let skills = temp_skills();
    let core = mc_core::Core::in_memory(skills.path().to_path_buf()).unwrap();

    let content = core.skill_read("essai").unwrap();
    assert!(content.contains("name: essai"));

    let err = core
        .skill_write("essai", "::pas du yaml::")
        .unwrap_err()
        .to_string();
    assert!(err.contains("YAML"), "{err}");

    let err = core
        .skill_write("essai", &content.replace("name: essai", "name: autre"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("name"), "{err}");

    let edited = content.replace("version: 1.0.0", "version: 1.1.0");
    core.skill_write("essai", &edited).unwrap();
    assert!(core.skill_read("essai").unwrap().contains("version: 1.1.0"));
    let audit = core.audit_list(10).unwrap();
    assert!(audit
        .iter()
        .any(|e| e.action == "edit" && e.target == "essai"));
}

/// F8 : une skill inconnue ne peut pas servir de vecteur de traversée.
#[test]
fn skill_editor_rejects_unknown_names() {
    std::env::set_var("MC_SECRETS_MODE", "memory");
    let skills = temp_skills();
    let core = mc_core::Core::in_memory(skills.path().to_path_buf()).unwrap();
    assert!(core.skill_read("../evil").is_err());
    assert!(core.skill_write("../evil", "name: x").is_err());
}

// ---------------------------------------------------------------------------
// T11 — streaming des réponses IA, retry/backoff, budget de tokens
// ---------------------------------------------------------------------------

fn openai_provider(server: &MockServer) -> Provider {
    Provider::OpenAiCompat {
        base_url: server.base_url(),
        model: "modele-test".into(),
        api_key: "cle-test".into(),
    }
}

fn collect_stream(deltas: &Mutex<Vec<String>>) -> impl Fn(&str) + Send + Sync + '_ {
    move |d: &str| deltas.lock().unwrap().push(d.to_string())
}

/// T11 : flux SSE OpenAI-compatible — un fragment par événement `data:`,
/// arrêt sur `[DONE]`, texte complet reconstitué.
#[tokio::test]
async fn t11_streaming_openai_sse() {
    let server = MockServer::start();
    let sse = |c: &str| {
        format!(
            "data: {}\n\n",
            serde_json::json!({"choices":[{"delta":{"content": c}}]})
        )
    };
    let body = format!(
        "data: {}\n\n{}{}data: [DONE]\n\n",
        r#"{"choices":[{"delta":{"role":"assistant"}}]}"#,
        sse("Bonjour"),
        sse(" monde"),
    );
    server.add("POST", "/v1/chat/completions", 200, &[], &body);

    let deltas = Mutex::new(Vec::new());
    let full = openai_provider(&server)
        .complete_streaming(
            "sys",
            "user",
            512,
            &CancelToken::new(),
            &collect_stream(&deltas),
        )
        .await
        .unwrap();
    assert_eq!(full, "Bonjour monde");
    assert_eq!(*deltas.lock().unwrap(), vec!["Bonjour", " monde"]);
}

/// T11 : flux SSE Anthropic — `content_block_delta` → fragments, `message_stop`
/// → fin ; les autres événements sont ignorés.
#[tokio::test]
async fn t11_streaming_anthropic_sse() {
    let server = MockServer::start();
    let ev = |t: &str| {
        format!(
            "event: content_block_delta\ndata: {}\n\n",
            serde_json::json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text": t}})
        )
    };
    let body = format!(
        "event: message_start\ndata: {}\n\n{}{}event: message_stop\ndata: {}\n\n",
        r#"{"type":"message_start"}"#,
        ev("Bon"),
        ev("jour"),
        r#"{"type":"message_stop"}"#,
    );
    server.add("POST", "/v1/messages", 200, &[], &body);

    let provider = Provider::Anthropic {
        base_url: server.base_url(),
        model: "modele-test".into(),
        api_key: "cle-test".into(),
    };
    let deltas = Mutex::new(Vec::new());
    let full = provider
        .complete_streaming(
            "sys",
            "user",
            512,
            &CancelToken::new(),
            &collect_stream(&deltas),
        )
        .await
        .unwrap();
    assert_eq!(full, "Bonjour");
    assert_eq!(deltas.lock().unwrap().len(), 2);
}

/// T11 : flux NDJSON Ollama — une ligne JSON par fragment, `done: true` → fin.
#[tokio::test]
async fn t11_streaming_ollama_ndjson() {
    let server = MockServer::start();
    let body = concat!(
        r#"{"message":{"role":"assistant","content":"Bon"},"done":false}"#,
        "\n",
        r#"{"message":{"role":"assistant","content":"jour"},"done":false}"#,
        "\n",
        r#"{"message":{"role":"assistant","content":""},"done":true}"#,
        "\n",
    );
    server.add("POST", "/api/chat", 200, &[], body);

    let provider = Provider::Ollama {
        base_url: server.base_url(),
        model: "modele-test".into(),
    };
    let deltas = Mutex::new(Vec::new());
    let full = provider
        .complete_streaming(
            "sys",
            "user",
            512,
            &CancelToken::new(),
            &collect_stream(&deltas),
        )
        .await
        .unwrap();
    assert_eq!(full, "Bonjour");
    assert_eq!(*deltas.lock().unwrap(), vec!["Bon", "jour"]);
}

/// T11 : l'annulation en COURS de flux interrompt à la ligne suivante.
#[tokio::test]
async fn t11_streaming_cancel_mid_stream() {
    let server = MockServer::start();
    let sse = |c: &str| {
        format!(
            "data: {}\n\n",
            serde_json::json!({"choices":[{"delta":{"content": c}}]})
        )
    };
    let body = format!(
        "{}{}{}data: [DONE]\n\n",
        sse("un"),
        sse("deux"),
        sse("trois")
    );
    server.add("POST", "/v1/chat/completions", 200, &[], &body);

    let cancel = CancelToken::new();
    let trigger = cancel.clone();
    let deltas = Mutex::new(Vec::new());
    let on_delta = move |d: &str| {
        deltas.lock().unwrap().push(d.to_string());
        trigger.cancel(); // annulation dès le premier fragment reçu
    };
    let err = openai_provider(&server)
        .complete_streaming("sys", "user", 512, &cancel, &on_delta)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "cancelled");
}

/// T11 : réessai automatique — 429 (Retry-After) puis 200 → succès en 2 appels.
#[tokio::test]
async fn t11_retry_429_then_success() {
    let server = MockServer::start();
    server.add_seq(
        "POST",
        "/v1/chat/completions",
        &[
            (429, &[("retry-after", "0")], "{}"),
            (
                200,
                &[],
                r#"{"choices":[{"message":{"content":"ok apres retry"}}]}"#,
            ),
        ],
    );
    let out = openai_provider(&server)
        .complete("sys", "user")
        .await
        .unwrap();
    assert_eq!(out, "ok apres retry");
    assert_eq!(server.hits("POST", "/v1/chat/completions"), 2);
}

/// T11 : réessais épuisés sur 5xx → erreur typée après exactement 3 essais.
#[tokio::test]
async fn t11_retry_exhausted_on_5xx() {
    let server = MockServer::start();
    server.add(
        "POST",
        "/v1/chat/completions",
        500,
        &[("retry-after", "0")],
        r#"{"error":"boom"}"#,
    );
    let err = openai_provider(&server)
        .complete("sys", "user")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "http");
    assert!(err.to_string().contains("HTTP 500"), "{err}");
    assert_eq!(server.hits("POST", "/v1/chat/completions"), 3);
}

/// T11 : le budget de tokens d'un lot se répartit entre les groupes, borné
/// [256, 1024] — un gros lot ne peut pas dériver en coût.
#[test]
fn t11_batch_token_budget_is_split_and_clamped() {
    assert_eq!(mc_core::ai::batch_max_tokens(1), 1024);
    assert_eq!(mc_core::ai::batch_max_tokens(16), 1024);
    assert_eq!(mc_core::ai::batch_max_tokens(32), 512);
    assert_eq!(mc_core::ai::batch_max_tokens(64), 256);
    assert_eq!(mc_core::ai::batch_max_tokens(500), 256);
}

/// T11 bout-en-bout : génération via fournisseur distant mocké en SSE —
/// fragments relayés par `ai_delta`, progression par groupe, garde-fous
/// appliqués sur le texte reconstitué, proposition enregistrée.
#[tokio::test]
async fn t11_proposals_generate_streams_via_task_events() {
    let (f, shas) = feature_fixture();
    let (core, repo_id) = core_with(&f);
    let server = MockServer::start();
    let outcome = r#"{"decision":"propose","message":"fix(pay): stabilize payment flow","explication":"via SSE","risque":"low"}"#;
    let (part1, part2) = outcome.split_at(40);
    let sse = |c: &str| {
        format!(
            "data: {}\n\n",
            serde_json::json!({"choices":[{"delta":{"content": c}}]})
        )
    };
    let body = format!("{}{}data: [DONE]\n\n", sse(part1), sse(part2));
    server.add("POST", "/v1/chat/completions", 200, &[], &body);

    core.ai_provider_save(
        AiProviderKind::OpenAiCompat,
        Some(server.base_url()),
        Some("modele-test".into()),
        Some("cle-secrete-abcdef".into()),
        true,
    )
    .unwrap();

    let events = std::sync::Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    let ctx = TaskCtx::new(
        "proposals_generate",
        "t-gen",
        CancelToken::new(),
        move |p| sink.lock().unwrap().push(p.event),
    );
    // Consentement explicite donné (CA-9) — l'aperçu a été montré côté UI.
    let proposals = core
        .proposals_generate_with(
            &repo_id,
            "conventional-commits",
            vec![vec![shas[1].to_string()]],
            None,
            true,
            &ctx,
        )
        .await
        .unwrap();

    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].status, ProposalStatus::Proposed);
    assert_eq!(
        proposals[0].after.as_deref(),
        Some("fix(pay): stabilize payment flow")
    );

    let events = events.lock().unwrap();
    let streamed: String = events
        .iter()
        .filter_map(|e| match e {
            TaskEvent::AiDelta { group: 0, delta } => Some(delta.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(streamed, outcome, "fragments relayés dans l'ordre");
    assert!(events.iter().any(|e| matches!(
        e,
        TaskEvent::Progress { phase, current: 1, total: Some(1) } if phase == "génération des propositions"
    )));
}

/// Le runner de self-tests des skills passe en mode déterministe local.
#[test]
fn skill_selftests_pass_for_local_capable_skills() {
    let f = init_repo();
    commit(&f.repo, &[("a.txt", "a\n")], "chore: seed", 1_700_000_000);
    let (core, _) = core_with(&f);
    for name in [
        "conventional-commits",
        "commit-synthesis",
        "ai-signature-cleaner",
    ] {
        let results = core.skill_run_tests(name).unwrap();
        assert!(!results.is_empty(), "{name} sans cas de test");
        for r in &results {
            assert!(r.passed, "{name}/{} : {}", r.case, r.detail);
        }
    }
}
