//! Re-signature des commits produits par la réécriture (B1).
//!
//! git2 ne signe pas : on recrée chaque commit du segment via
//! `commit_create_buffer` (contenu canonique sans signature), on signe ce
//! contenu avec l'outil externe configuré dans le dépôt (SSH via `ssh-keygen
//! -Y sign`, ou GPG), puis on l'écrit avec `commit_signed` (en-tête `gpgsig`).
//! Signer change forcément les SHA (ajout de la signature) : tout le segment
//! est recréé, parents remappés.

use std::collections::HashSet;
use std::io::Write;
use std::process::{Command, Stdio};

use git2::{Commit, Oid, Repository};
use std::collections::HashMap;

use crate::error::{CoreError, Result};

use super::GitEngine;

/// Outil de signature détecté depuis la configuration du dépôt.
pub enum Signer {
    /// `gpg.format = ssh` — `key` est le chemin de la clé privée SSH.
    Ssh { key: String },
    /// GPG (openpgp) — `key_id` est l'identité de signature.
    Gpg { key_id: String },
}

impl Signer {
    /// Signe `content` et renvoie la signature armée (telle que git l'attend
    /// dans l'en-tête `gpgsig`).
    pub fn sign(&self, content: &[u8]) -> Result<String> {
        let (bin, args): (&str, Vec<String>) = match self {
            Signer::Ssh { key } => (
                "ssh-keygen",
                vec![
                    "-Y".into(),
                    "sign".into(),
                    "-n".into(),
                    "git".into(),
                    "-f".into(),
                    key.clone(),
                ],
            ),
            Signer::Gpg { key_id } => (
                "gpg",
                vec![
                    "--armor".into(),
                    "--detach-sign".into(),
                    "-u".into(),
                    key_id.clone(),
                ],
            ),
        };
        let mut child = Command::new(bin)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| CoreError::Git(format!("{bin} introuvable pour signer : {e}")))?;
        child
            .stdin
            .take()
            .ok_or_else(|| CoreError::Git("stdin du signeur indisponible".into()))?
            .write_all(content)?;
        let out = child
            .wait_with_output()
            .map_err(|e| CoreError::Git(format!("attente du signeur : {e}")))?;
        if !out.status.success() {
            return Err(CoreError::Git(format!(
                "échec de la signature ({bin}) : {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }
}

/// Détecte le signeur d'après `user.signingkey` + `gpg.format` du dépôt.
/// Renvoie `None` si aucune clé n'est configurée.
pub fn signer_from_config(repo: &Repository) -> Result<Option<Signer>> {
    let cfg = repo.config()?;
    let key = match cfg.get_string("user.signingkey") {
        Ok(k) if !k.trim().is_empty() => k,
        _ => return Ok(None),
    };
    let format = cfg
        .get_string("gpg.format")
        .unwrap_or_else(|_| "openpgp".into());
    Ok(Some(if format == "ssh" {
        Signer::Ssh { key }
    } else {
        Signer::Gpg { key_id: key }
    }))
}

/// Recrée le segment `base..tip` en SIGNANT chaque commit (parents remappés,
/// arbres et messages inchangés). Retourne le nouveau sommet et la carte
/// ancien→nouveau SHA.
pub fn sign_segment(
    repo: &Repository,
    base: Oid,
    tip: Oid,
    signer: &Signer,
) -> Result<(Oid, HashMap<Oid, Oid>)> {
    let segment = GitEngine::segment(repo, Some(base), tip)?;
    let seg_set: HashSet<Oid> = segment.iter().copied().collect();
    let mut map: HashMap<Oid, Oid> = HashMap::new();
    for oid in &segment {
        let c = repo.find_commit(*oid)?;
        let parents: Vec<Commit> = c
            .parent_ids()
            .map(|pid| {
                let mapped = if seg_set.contains(&pid) {
                    *map.get(&pid).unwrap_or(&pid)
                } else {
                    pid
                };
                repo.find_commit(mapped)
            })
            .collect::<std::result::Result<_, _>>()?;
        let parent_refs: Vec<&Commit> = parents.iter().collect();
        let buf = repo.commit_create_buffer(
            &c.author(),
            &c.committer(),
            c.message().unwrap_or(""),
            &c.tree()?,
            &parent_refs,
        )?;
        let content = std::str::from_utf8(&buf)
            .map_err(|e| CoreError::Git(format!("contenu du commit non UTF-8 : {e}")))?;
        let signature = signer.sign(content.as_bytes())?;
        let new_oid = repo.commit_signed(content, &signature, None)?;
        map.insert(*oid, new_oid);
    }
    let new_tip = *map
        .get(&tip)
        .ok_or_else(|| CoreError::Invalid("segment vide".into()))?;
    Ok((new_tip, map))
}
