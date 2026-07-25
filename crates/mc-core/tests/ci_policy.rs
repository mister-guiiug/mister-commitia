mod common;

use chrono::{Duration, Utc};
use common::mockhttp::MockServer;
use mc_core::model::*;
use mc_core::task::{CancelToken, TaskCtx, TaskEvent};

fn run(id: &str, pipeline: &str, days_ago: i64, leased: bool, running: bool) -> CiRun {
    CiRun {
        account_id: "acct_test".into(),
        pipeline_id: pipeline.into(),
        pipeline_name: format!("Pipeline {pipeline}"),
        run_id: id.into(),
        status: if running { "in_progress" } else { "completed" }.into(),
        result: if running {
            None
        } else {
            Some("success".into())
        },
        branch: Some("develop".into()),
        created_at: (Utc::now() - Duration::days(days_ago)).to_rfc3339(),
        url: None,
        leased,
        running,
    }
}

fn account(base: &str, kind: CiKind) -> CiAccount {
    CiAccount {
        id: "acct_test".into(),
        kind,
        base_url: base.into(),
        org: Some("o".into()),
        project: Some("proj".into()),
        repo: Some("r".into()),
        token_ref: "test".into(),
        scopes: vec![],
        added_at: String::new(),
    }
}

/// CA-11/CA-12 : classification pure du moteur de politiques — les runs sous
/// lease ou en cours ne sont JAMAIS candidats.
#[test]
fn simulate_classifies_protected_and_candidates() {
    let policy = RetentionPolicy {
        id: "pol1".into(),
        name: "90j".into(),
        rules: RetentionRules {
            max_age_days: Some(90),
            keep_last_per_pipeline: 2,
            protect_branches: vec!["main".into()],
            protect_failed: false,
        },
        enabled: true,
    };
    let mut main_run = run("r4", "p1", 400, false, false);
    main_run.branch = Some("main".into());
    let runs = vec![
        run("r1", "p1", 10, false, false), // rang 0 → conservé (keep_last 2)
        run("r2", "p1", 200, true, false), // lease → protégé
        run("r3", "p1", 200, false, true), // en cours → protégé
        main_run,                          // branche protégée
        run("r5", "p1", 100, false, false), // rang 2, > 90 j → CANDIDAT
        run("r6", "p1", 150, false, false), // rang 3, > 90 j → CANDIDAT
        run("r7", "p1", 40, false, false), // rang 1 → conservé (keep_last 2)
    ];
    let acct = account("http://localhost", CiKind::Github);
    let report = mc_core::ci::simulate(&policy, &acct, &runs, Utc::now());

    let candidate_ids: Vec<&str> = report
        .candidates
        .iter()
        .map(|r| r.run_id.as_str())
        .collect();
    assert_eq!(candidate_ids, vec!["r5", "r6"], "rapport : {report:?}");
    assert_eq!(report.kept_recent, 2);
    let protected: Vec<(&str, &str)> = report
        .protected
        .iter()
        .map(|p| (p.run.run_id.as_str(), p.reason.as_str()))
        .collect();
    assert!(protected
        .iter()
        .any(|(id, r)| *id == "r2" && r.contains("lease")));
    assert!(protected
        .iter()
        .any(|(id, r)| *id == "r3" && r.contains("cours")));
    assert!(protected
        .iter()
        .any(|(id, r)| *id == "r4" && r.contains("main")));
    assert_eq!(report.total, 7);
}

fn github_runs_json() -> String {
    let old_date = (Utc::now() - Duration::days(200)).to_rfc3339();
    let recent_date = (Utc::now() - Duration::days(3)).to_rfc3339();
    serde_json::json!({
        "total_count": 2,
        "workflow_runs": [
            {"id": 101, "name": "CI", "workflow_id": 9, "status": "completed",
             "conclusion": "success", "head_branch": "develop",
             "created_at": old_date, "html_url": "http://x/101"},
            {"id": 102, "name": "CI", "workflow_id": 9, "status": "completed",
             "conclusion": "success", "head_branch": "develop",
             "created_at": recent_date, "html_url": "http://x/102"}
        ]
    })
    .to_string()
}

/// CA-11 : flux complet GitHub mocké — simulation obligatoire, double
/// confirmation, journalisation avant suppression, suppression émise une fois.
#[tokio::test]
async fn ca11_github_delete_flow_requires_simulation_and_confirmation() {
    std::env::set_var("MC_SECRETS_MODE", "memory");
    let server = MockServer::start();
    server.add("GET", "/repos/o/r", 200, &[], r#"{"full_name":"o/r"}"#);
    server.add(
        "GET",
        "/repos/o/r/actions/runs",
        200,
        &[],
        &github_runs_json(),
    );
    server.add("DELETE", "/repos/o/r/actions/runs/101", 204, &[], "");

    let core = mc_core::Core::in_memory(common::skills_dir()).unwrap();
    let (acct, validation) = core
        .ci_account_add(
            CiKind::Github,
            server.base_url(),
            Some("o".into()),
            None,
            Some("r".into()),
            "token-de-test-123456".into(),
            vec!["Actions: read/write".into()],
        )
        .await
        .unwrap();
    assert!(validation.contains("o/r"));

    let policy = core
        .policy_save(
            "90 jours".into(),
            RetentionRules {
                max_age_days: Some(90),
                keep_last_per_pipeline: 1,
                protect_branches: vec![],
                protect_failed: false,
            },
        )
        .unwrap();

    let target = run("101", "9", 200, false, false);
    let del_path = "/repos/o/r/actions/runs/101";

    // 1) Suppression sans simulation préalable → refus, aucun DELETE émis.
    let err = core
        .ci_delete_run(&acct.id, &policy.id, target.clone(), "Pipeline 9".into())
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("simulation"), "{err}");
    assert_eq!(server.hits("DELETE", del_path), 0);

    // 2) Simulation : rapport, zéro suppression émise (CA-11).
    let report = core.ci_simulate(&acct.id, &policy.id, 500).await.unwrap();
    assert_eq!(report.candidates.len(), 1);
    assert_eq!(report.candidates[0].run_id, "101");
    assert_eq!(report.kept_recent, 1);
    assert_eq!(server.hits("DELETE", del_path), 0);

    let candidate = report.candidates[0].clone();

    // 3) Confirmation erronée → refus.
    let err = core
        .ci_delete_run(
            &acct.id,
            &policy.id,
            candidate.clone(),
            "mauvais nom".into(),
        )
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("saisir exactement"), "{err}");
    assert_eq!(server.hits("DELETE", del_path), 0);

    // 4) Confirmation exacte (nom du pipeline) → suppression émise UNE fois.
    core.ci_delete_run(
        &acct.id,
        &policy.id,
        candidate.clone(),
        candidate.pipeline_name.clone(),
    )
    .await
    .unwrap();
    assert_eq!(server.hits("DELETE", del_path), 1);

    // 5) Journal : tentative AVANT résultat, puis résultat ok (CA-14).
    let audit = core.audit_list(50).unwrap();
    let attempt = audit.iter().find(|e| e.action == "delete_attempt").unwrap();
    let done = audit.iter().find(|e| e.action == "delete").unwrap();
    assert!(attempt.seq < done.seq, "la tentative précède le résultat");
    assert_eq!(done.result, "ok");
    // Et le token n'apparaît nulle part dans l'export.
    let export = core.audit_export().unwrap();
    assert!(!export.contains("token-de-test-123456"));
}

/// CA-13 : une réponse 429 avec Retry-After est traduite en erreur exploitable.
#[tokio::test]
async fn ca13_rate_limited_maps_retry_after() {
    let server = MockServer::start();
    server.add(
        "DELETE",
        "/repos/o/r/actions/runs/55",
        429,
        &[("retry-after", "7")],
        "{}",
    );
    let acct = account(&server.base_url(), CiKind::Github);
    let client = mc_core::ci::CiClient::from_account(&acct, "t-secret-1".into()).unwrap();
    let err = client.delete_run(&run("55", "9", 100, false, false)).await;
    match err {
        Err(mc_core::CoreError::RateLimited { retry_after_secs }) => {
            assert_eq!(retry_after_secs, 7)
        }
        other => panic!("attendu RateLimited, obtenu {other:?}"),
    }
}

/// CA-12 : Azure DevOps — la revérification des leases bloque la suppression
/// et l'appel DELETE n'est jamais émis.
#[tokio::test]
async fn ca12_azdo_lease_recheck_blocks_delete() {
    let server = MockServer::start();
    server.add(
        "GET",
        "/proj/_apis/build/builds/55/leases",
        200,
        &[],
        r#"{"count":1,"value":[{"leaseId":1}]}"#,
    );
    server.add("DELETE", "/proj/_apis/build/builds/55", 200, &[], "{}");

    let acct = account(&server.base_url(), CiKind::AzureDevops);
    let client = mc_core::ci::CiClient::from_account(&acct, "pat-secret-1".into()).unwrap();
    let err = client
        .delete_run(&run("55", "3", 100, false, false))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("lease"), "{err}");
    assert_eq!(server.hits("DELETE", "/proj/_apis/build/builds/55"), 0);
}

fn github_page_json(count: usize, first_id: i64) -> String {
    let runs: Vec<serde_json::Value> = (0..count)
        .map(|i| {
            serde_json::json!({
                "id": first_id + i as i64, "name": "CI", "workflow_id": 9,
                "status": "completed", "conclusion": "success",
                "head_branch": "develop", "created_at": "2026-01-01T00:00:00Z",
                "html_url": "http://x"
            })
        })
        .collect();
    serde_json::json!({"total_count": count, "workflow_runs": runs}).to_string()
}

/// T2 : l'inventaire paginé émet une progression PAR PAGE et l'annulation
/// entre deux pages interrompt sans émettre la requête suivante.
#[tokio::test]
async fn t2_inventory_progress_per_page_and_cancel_between_pages() {
    let server = MockServer::start();
    let path = "/repos/o/r/actions/runs";
    // Page 1 : 100 runs (pleine → une page suit) ; page 2 : 2 runs (fin).
    server.add_seq(
        "GET",
        path,
        &[
            (200, &[], &github_page_json(100, 1000)),
            (200, &[], &github_page_json(2, 2000)),
        ],
    );
    let acct = account(&server.base_url(), CiKind::Github);
    let client = mc_core::ci::CiClient::from_account(&acct, "t-secret-2".into()).unwrap();

    let progress = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u64>::new()));
    let sink = progress.clone();
    let ctx = TaskCtx::new("ci_inventory", "t-inv", CancelToken::new(), move |p| {
        if let TaskEvent::Progress { current, .. } = p.event {
            sink.lock().unwrap().push(current);
        }
    });
    let runs = client.list_runs_with(500, &ctx).await.unwrap();
    assert_eq!(runs.len(), 102);
    assert_eq!(*progress.lock().unwrap(), vec![0, 100]);
    assert_eq!(server.hits("GET", path), 2);

    // Annulation pendant la première page (déclenchée par l'émission qui la
    // précède) : le point d'arrêt AVANT la page 2 interrompt — la seconde
    // requête n'est JAMAIS émise.
    let server2 = MockServer::start();
    server2.add_seq(
        "GET",
        path,
        &[
            (200, &[], &github_page_json(100, 1000)),
            (200, &[], &github_page_json(2, 2000)),
        ],
    );
    let acct2 = account(&server2.base_url(), CiKind::Github);
    let client2 = mc_core::ci::CiClient::from_account(&acct2, "t-secret-3".into()).unwrap();
    let cancel = CancelToken::new();
    let trigger = cancel.clone();
    let ctx = TaskCtx::new("ci_inventory", "t-inv2", cancel, move |p| {
        if let TaskEvent::Progress { current, .. } = p.event {
            if current == 0 {
                trigger.cancel();
            }
        }
    });
    let err = client2.list_runs_with(500, &ctx).await.unwrap_err();
    assert_eq!(err.code(), "cancelled");
    assert_eq!(
        server2.hits("GET", path),
        1,
        "aucune requête après l'annulation"
    );
}

/// F7 : nettoyage CI EN MASSE — supprime le lot des candidats, résiste au
/// throttling (429 → attente → reprise), confirmation par le nombre de runs,
/// et un point de reprise évite de re-supprimer.
#[tokio::test]
async fn f7_batch_delete_throttles_and_resumes() {
    std::env::set_var("MC_SECRETS_MODE", "memory");
    let server = MockServer::start();
    server.add("GET", "/repos/o/r", 200, &[], r#"{"full_name":"o/r"}"#);
    server.add(
        "GET",
        "/repos/o/r/actions/runs",
        200,
        &[],
        &github_runs_json(),
    );
    // Run 101 : throttlé (429, Retry-After 0) puis supprimé ; run 102 : direct.
    server.add_seq(
        "DELETE",
        "/repos/o/r/actions/runs/101",
        &[(429, &[("retry-after", "0")], "{}"), (204, &[], "")],
    );
    server.add("DELETE", "/repos/o/r/actions/runs/102", 204, &[], "");

    let core = mc_core::Core::in_memory(common::skills_dir()).unwrap();
    let (acct, _) = core
        .ci_account_add(
            CiKind::Github,
            server.base_url(),
            Some("o".into()),
            None,
            Some("r".into()),
            "token-de-test-batch".into(),
            vec![],
        )
        .await
        .unwrap();
    let policy = core
        .policy_save(
            "tout".into(),
            RetentionRules {
                max_age_days: None,
                keep_last_per_pipeline: 0,
                protect_branches: vec![],
                protect_failed: false,
            },
        )
        .unwrap();
    let report = core.ci_simulate(&acct.id, &policy.id, 500).await.unwrap();
    assert_eq!(report.candidates.len(), 2, "les deux runs sont candidats");

    // Confirmation invalide (mauvais nombre) → refus typé.
    let err = core
        .ci_delete_batch(
            &acct.id,
            &policy.id,
            report.candidates.clone(),
            "1".into(),
            vec![],
            &TaskCtx::noop("batch"),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "confirm_required");
    assert_eq!(err.expected(), Some("2"));

    // Confirmation exacte (« 2 ») → tout supprimé, avec throttling sur 101.
    let res = core
        .ci_delete_batch(
            &acct.id,
            &policy.id,
            report.candidates.clone(),
            "2".into(),
            vec![],
            &TaskCtx::noop("batch"),
        )
        .await
        .unwrap();
    assert_eq!(res.deleted.len(), 2);
    assert!(res.failed.is_empty());
    assert!(!res.cancelled);
    assert_eq!(
        server.hits("DELETE", "/repos/o/r/actions/runs/101"),
        2,
        "429 puis succès"
    );
    assert_eq!(server.hits("DELETE", "/repos/o/r/actions/runs/102"), 1);

    // Reprise : avec 101 déjà fait, seul 102 reste (confirm « 1 ») — 101 non re-supprimé.
    let res2 = core
        .ci_delete_batch(
            &acct.id,
            &policy.id,
            report.candidates.clone(),
            "1".into(),
            vec!["101".into()],
            &TaskCtx::noop("batch"),
        )
        .await
        .unwrap();
    assert!(res2.deleted.contains(&"101".to_string()) && res2.deleted.contains(&"102".to_string()));
    assert_eq!(
        server.hits("DELETE", "/repos/o/r/actions/runs/101"),
        2,
        "101 n'est pas re-supprimé lors de la reprise"
    );
}

/// F4 : détection des PR ouvertes via l'API GitHub (push assisté) ; Azure
/// DevOps n'est pas couvert par ce chemin.
#[tokio::test]
async fn f4_github_list_open_prs() {
    let server = MockServer::start();
    server.add(
        "GET",
        "/repos/o/r/pulls",
        200,
        &[],
        r#"[{"number":42,"title":"feat: express payment","html_url":"http://x/42"},
            {"number":43,"title":"wip cleanup","html_url":"http://x/43"}]"#,
    );
    let acct = account(&server.base_url(), CiKind::Github);
    let client = mc_core::ci::CiClient::from_account(&acct, "t-secret-pr".into()).unwrap();
    let prs = client.list_open_prs("feature/x").await.unwrap();
    assert_eq!(prs.len(), 2);
    assert_eq!(prs[0].number, 42);
    assert!(prs[0].title.contains("express"));
    assert_eq!(prs[1].url, "http://x/43");

    let az = account(&server.base_url(), CiKind::AzureDevops);
    let azc = mc_core::ci::CiClient::from_account(&az, "t".into()).unwrap();
    assert!(azc.list_open_prs("b").await.is_err(), "AzDO non couvert");
}

/// Inventaire Azure DevOps : keepForever/retainedByRelease ⇒ marqué retenu.
#[tokio::test]
async fn azdo_inventory_marks_retained_runs() {
    let server = MockServer::start();
    server.add(
        "GET",
        "/proj/_apis/build/builds",
        200,
        &[],
        &serde_json::json!({
            "count": 2,
            "value": [
                {"id": 77, "buildNumber": "77", "status": "completed", "result": "succeeded",
                 "sourceBranch": "refs/heads/main", "queueTime": "2026-01-01T00:00:00Z",
                 "definition": {"id": 3, "name": "Pipe"}, "keepForever": true,
                 "_links": {"web": {"href": "http://x/77"}}},
                {"id": 78, "buildNumber": "78", "status": "completed", "result": "failed",
                 "sourceBranch": "refs/heads/dev", "queueTime": "2026-02-01T00:00:00Z",
                 "definition": {"id": 3, "name": "Pipe"}}
            ]
        })
        .to_string(),
    );
    let acct = account(&server.base_url(), CiKind::AzureDevops);
    let client = mc_core::ci::CiClient::from_account(&acct, "pat-secret-2".into()).unwrap();
    let runs = client.list_runs(100).await.unwrap();
    assert_eq!(runs.len(), 2);
    assert!(runs[0].leased);
    assert_eq!(runs[0].branch.as_deref(), Some("main"));
    assert!(!runs[1].leased);
}
