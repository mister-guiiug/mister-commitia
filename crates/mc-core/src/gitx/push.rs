//! Opérations git réseau du push assisté (F4) : fetch de la ref
//! remote-tracking (support du bail `--force-with-lease`) et push.
//!
//! On passe par le binaire `git` (via `run_git`) et non par git2 : le push et
//! le fetch réutilisent ainsi le credential helper et la configuration réseau
//! de l'utilisateur (proxys, jetons), au lieu de réimplémenter l'authentification.

use std::path::Path;

use crate::error::{CoreError, Result};

use super::rewrite::run_git;

/// Met à jour `refs/remotes/<remote>/<branch>` depuis le remote. Retourne
/// `Ok(false)` si la branche n'existe pas côté distant (premier push).
pub fn fetch_branch(dir: &Path, remote: &str, branch: &str) -> Result<bool> {
    let refspec = format!("+refs/heads/{branch}:refs/remotes/{remote}/{branch}");
    match run_git(dir, &["fetch", remote, &refspec], &[]) {
        Ok(_) => Ok(true),
        Err(CoreError::Git(msg))
            if msg.contains("couldn't find remote ref")
                || msg.contains("n'a pas trouvé la référence")
                || msg.contains("does not exist") =>
        {
            Ok(false)
        }
        Err(e) => Err(e),
    }
}

/// Pousse `branch` vers `remote`. Avec `lease`, l'écriture est un
/// `--force-with-lease=<branch>:<lease>` : le push ÉCHOUE (sans rien écraser) si
/// la ref distante ne vaut plus exactement `lease` — protégeant tout travail
/// distant non vu depuis le dernier fetch.
pub fn push_branch(
    dir: &Path,
    remote: &str,
    branch: &str,
    lease: Option<&str>,
    set_upstream: bool,
) -> Result<()> {
    let mut args: Vec<String> = vec!["push".into()];
    if set_upstream {
        args.push("--set-upstream".into());
    }
    if let Some(l) = lease {
        args.push(format!("--force-with-lease={branch}:{l}"));
    }
    args.push(remote.into());
    args.push(format!("{branch}:{branch}"));
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();
    run_git(dir, &argv, &[]).map(|_| ())
}
