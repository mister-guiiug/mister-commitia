//! Secrets : coffre du système d'exploitation exclusivement
//! (Windows Credential Manager / macOS Keychain / Secret Service via D-Bus,
//! crate `keyring`). La base locale ne voit passer que des ALIAS.
//! Toute valeur lue ou écrite est enregistrée auprès du redacteur global :
//! aucun canal de sortie (logs, audit, erreurs) ne peut la refléter en clair.

use once_cell::sync::Lazy;
use std::sync::Mutex;

use crate::error::{CoreError, Result};

pub const SERVICE: &str = "mister-commitia";

static REDACT: Lazy<Mutex<Vec<String>>> = Lazy::new(|| Mutex::new(Vec::new()));

pub fn register_redaction(secret: &str) {
    if secret.len() < 6 {
        return; // trop court pour être masqué sans faux positifs massifs
    }
    let mut guard = REDACT.lock().unwrap();
    if !guard.iter().any(|s| s == secret) {
        guard.push(secret.to_string());
    }
}

/// Masque toute occurrence de secret connu dans une chaîne destinée à un
/// canal de sortie (journal, audit, message d'erreur, export).
pub fn redact(input: &str) -> String {
    let guard = REDACT.lock().unwrap();
    let mut out = input.to_string();
    for s in guard.iter() {
        if out.contains(s.as_str()) {
            out = out.replace(s.as_str(), "***");
        }
    }
    out
}

/// Backend mémoire (processus courant uniquement) : réservé aux TESTS et aux
/// environnements sans coffre (`MC_SECRETS_MODE=memory`). Jamais persisté.
static MEMORY: Lazy<Mutex<std::collections::HashMap<String, String>>> =
    Lazy::new(|| Mutex::new(Default::default()));

fn memory_mode() -> bool {
    std::env::var("MC_SECRETS_MODE").as_deref() == Ok("memory")
}

fn entry(reference: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, reference)
        .map_err(|e| CoreError::Secret(format!("coffre OS indisponible : {e}")))
}

pub fn set_secret(reference: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(CoreError::Invalid("secret vide".into()));
    }
    if memory_mode() {
        MEMORY
            .lock()
            .unwrap()
            .insert(reference.to_string(), value.to_string());
    } else {
        entry(reference)?
            .set_password(value)
            .map_err(|e| CoreError::Secret(format!("écriture au coffre : {e}")))?;
    }
    register_redaction(value);
    Ok(())
}

pub fn get_secret(reference: &str) -> Result<String> {
    let v = if memory_mode() {
        MEMORY
            .lock()
            .unwrap()
            .get(reference)
            .cloned()
            .ok_or_else(|| CoreError::NotFound(format!("aucun secret pour « {reference} »")))?
    } else {
        entry(reference)?.get_password().map_err(|e| match e {
            keyring::Error::NoEntry => {
                CoreError::NotFound(format!("aucun secret pour « {reference} » dans le coffre"))
            }
            other => CoreError::Secret(format!("lecture du coffre : {other}")),
        })?
    };
    register_redaction(&v);
    Ok(v)
}

pub fn delete_secret(reference: &str) -> Result<()> {
    if memory_mode() {
        MEMORY.lock().unwrap().remove(reference);
        return Ok(());
    }
    match entry(reference)?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(CoreError::Secret(format!("suppression au coffre : {e}"))),
    }
}

/// Tableau « fonctionnalité → droit requis » affiché AVANT l'enregistrement
/// d'un token (CA-9). Faits vérifiés le 2026-07-24 (docs officielles).
pub fn required_scopes(kind: crate::model::CiKind) -> Vec<(String, String)> {
    use crate::model::CiKind::*;
    match kind {
        Github | GithubEnterprise => vec![
            (
                "Inventaire des workflows et runs".into(),
                "PAT fine-grained : « Actions: read » (classique : repo)".into(),
            ),
            (
                "Suppression de runs / logs / artifacts".into(),
                "PAT fine-grained : « Actions: write » (classique : repo)".into(),
            ),
            (
                "Lecture de la protection de branche".into(),
                "PAT fine-grained : « Administration: read »".into(),
            ),
        ],
        AzureDevops | AzureDevopsServer => vec![
            (
                "Inventaire des builds, leases et réglages de rétention".into(),
                "Scope PAT « Build (read) » (vso.build)".into(),
            ),
            (
                "Suppression d'un build".into(),
                "Scope PAT « Build (read & execute) » (vso.build_execute) + permission objet « Delete builds » sur le pipeline".into(),
            ),
        ],
    }
}
