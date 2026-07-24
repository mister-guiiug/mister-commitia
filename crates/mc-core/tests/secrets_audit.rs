mod common;

use mc_core::secrets;

/// CA-9 : aller-retour du backend mémoire + redaction systématique.
#[test]
fn memory_secret_roundtrip_and_redaction() {
    std::env::set_var("MC_SECRETS_MODE", "memory");
    secrets::set_secret("test:roundtrip", "SECRETXYZ123").unwrap();
    assert_eq!(
        secrets::get_secret("test:roundtrip").unwrap(),
        "SECRETXYZ123"
    );

    let masked = secrets::redact("Authorization: Bearer SECRETXYZ123 fin");
    assert!(!masked.contains("SECRETXYZ123"));
    assert!(masked.contains("***"));

    secrets::delete_secret("test:roundtrip").unwrap();
    assert!(secrets::get_secret("test:roundtrip").is_err());
}

/// CA-9 : le coffre RÉEL de l'OS fonctionne (Windows Credential Manager).
/// Passe directement par keyring pour ne pas dépendre du mode mémoire global.
#[cfg(windows)]
#[test]
fn windows_credential_manager_roundtrip() {
    let entry = keyring::Entry::new("mister-commitia-test", "roundtrip").unwrap();
    entry.set_password("VALEUR-TEST-9876").unwrap();
    assert_eq!(entry.get_password().unwrap(), "VALEUR-TEST-9876");
    entry.delete_credential().unwrap();
    assert!(entry.get_password().is_err());
}

/// CA-14 : journal append-only, séquence croissante, export JSONL rejouable.
#[test]
fn ca14_audit_append_only_and_export() {
    std::env::set_var("MC_SECRETS_MODE", "memory");
    let core = mc_core::Core::in_memory(common::skills_dir()).unwrap();
    for i in 0..3 {
        core.store
            .audit_append(
                "test",
                "config",
                "action",
                &format!("cible-{i}"),
                &serde_json::json!({"i": i}),
                "ok",
            )
            .unwrap();
    }
    let events = core.audit_list(10).unwrap();
    assert_eq!(events.len(), 3);
    assert!(
        events[0].seq > events[1].seq,
        "liste du plus récent au plus ancien"
    );

    let export = core.audit_export().unwrap();
    let lines: Vec<&str> = export.lines().collect();
    assert_eq!(lines.len(), 3);
    let seqs: Vec<i64> = lines
        .iter()
        .map(|l| {
            serde_json::from_str::<serde_json::Value>(l).unwrap()["seq"]
                .as_i64()
                .unwrap()
        })
        .collect();
    assert!(
        seqs.windows(2).all(|w| w[0] < w[1]),
        "export chronologique sans trous : {seqs:?}"
    );
}

/// T3 : migrations versionnées — réouverture idempotente, version stable.
#[test]
fn store_migrations_are_versioned_and_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("t.sqlite");
    {
        let store = mc_core::store::Store::open(&db).unwrap();
        assert_eq!(store.schema_version().unwrap(), 1);
        store
            .setting_set("k", &serde_json::json!({"v": 1}))
            .unwrap();
    }
    let store = mc_core::store::Store::open(&db).unwrap();
    assert_eq!(store.schema_version().unwrap(), 1);
    assert_eq!(
        store.setting_get("k").unwrap().unwrap()["v"].as_i64(),
        Some(1)
    );
}

/// CA-9 : aucune clé d'API en clair dans l'export d'audit après configuration
/// d'un fournisseur IA.
#[test]
fn ca9_no_api_key_in_audit_export() {
    std::env::set_var("MC_SECRETS_MODE", "memory");
    let core = mc_core::Core::in_memory(common::skills_dir()).unwrap();
    core.ai_provider_save(
        mc_core::model::AiProviderKind::Anthropic,
        None,
        Some("claude-sonnet-5".into()),
        Some("TOPSECRET-abcdef-0123456789".into()),
        true,
    )
    .unwrap();
    let export = core.audit_export().unwrap();
    assert!(!export.contains("TOPSECRET"));
    // La redaction du canal de sortie couvre aussi une fuite volontaire.
    assert!(!secrets::redact("x TOPSECRET-abcdef-0123456789 y").contains("TOPSECRET-abcdef"));
}
