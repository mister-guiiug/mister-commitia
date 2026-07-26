//! Découpe d'UN commit en plusieurs (C3), PAR FICHIER, sur un segment LINÉAIRE.
//!
//! Principe : la cible `T` (parent `P`) modifie un ensemble de fichiers. On les
//! partitionne en parts ordonnées ; chaque part devient un commit qui adopte
//! CUMULATIVEMENT la version cible de ses fichiers. La dernière part reconstitue
//! donc EXACTEMENT l'arbre de `T` — d'où deux propriétés clés :
//!   - aucune perte de contenu (arbre final identique au sommet d'origine) ;
//!   - la queue (`commits après T`) se rejoue SANS conflit : son parent voit le
//!     même arbre qu'avant, donc chaque commit conserve son arbre, seul le parent
//!     change.

use std::collections::HashSet;
use std::path::Path;

use git2::{Commit, IndexEntry, IndexTime, Oid, Repository, Tree};

use crate::error::{CoreError, Result};
use crate::gitx::GitEngine;
use crate::model::SplitPart;

/// Carte d'une découpe : par commit produit, `(anciens_oids, nouvel_oid)`.
pub type SplitMapping = Vec<(Vec<Oid>, Oid)>;

/// Fichiers ajoutés/modifiés/supprimés par `commit` vs son parent (chemins), sans
/// détection de renommage (un renommage = suppression + ajout, traités par chemin).
pub fn changed_files(repo: &Repository, commit: &Commit) -> Result<Vec<String>> {
    let tree = commit.tree()?;
    let parent_tree = if commit.parent_count() > 0 {
        Some(commit.parent(0)?.tree()?)
    } else {
        None
    };
    let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;
    let mut files = Vec::new();
    let mut seen = HashSet::new();
    for delta in diff.deltas() {
        let p = delta.new_file().path().or_else(|| delta.old_file().path());
        if let Some(p) = p {
            let s = p.to_string_lossy().to_string();
            if seen.insert(s.clone()) {
                files.push(s);
            }
        }
    }
    Ok(files)
}

/// IndexEntry minimal pour (chemin, mode, blob). `flags` porte la longueur du
/// chemin (masque de nom libgit2), stage 0.
fn idx_entry(path: &str, mode: u32, id: Oid) -> IndexEntry {
    IndexEntry {
        ctime: IndexTime::new(0, 0),
        mtime: IndexTime::new(0, 0),
        dev: 0,
        ino: 0,
        mode,
        uid: 0,
        gid: 0,
        file_size: 0,
        id,
        flags: (path.len().min(0x0FFF)) as u16,
        flags_extended: 0,
        path: path.as_bytes().to_vec(),
    }
}

/// Arbre = `base_tree` où, pour chaque fichier de `files`, on adopte la version de
/// `target_tree` (ajout/modif) ou on la SUPPRIME (si absente de la cible).
fn overlay_tree(
    repo: &Repository,
    base_tree: &Tree,
    target_tree: &Tree,
    files: &[String],
) -> Result<Oid> {
    let mut index = git2::Index::new()?;
    index.read_tree(base_tree)?;
    for f in files {
        match target_tree.get_path(Path::new(f)) {
            Ok(entry) => index.add(&idx_entry(f, entry.filemode() as u32, entry.id()))?,
            Err(_) => {
                let _ = index.remove_path(Path::new(f));
            }
        }
    }
    Ok(index.write_tree_to(repo)?)
}

/// Découpe `target` (dans le segment linéaire `base..tip`, parent unique) en autant
/// de commits que de `parts`. Renvoie `(nouveau_sommet, mapping)` où chaque entrée
/// du mapping est `(anciens_oids, nouvel_oid)` : chaque part porte `target` comme
/// origine, chaque commit (avant/queue) se cartographie sur lui-même/son rejeu.
pub fn split_segment(
    repo: &Repository,
    base: Oid,
    tip: Oid,
    target: Oid,
    parts: &[SplitPart],
) -> Result<(Oid, SplitMapping)> {
    let segment = GitEngine::segment(repo, Some(base), tip)?;
    let k = segment.iter().position(|o| *o == target).ok_or_else(|| {
        CoreError::Invalid("le commit à découper n'est pas dans le segment".into())
    })?;

    let target_commit = repo.find_commit(target)?;
    let target_tree = target_commit.tree()?;
    let author = target_commit.author();
    let committer = target_commit.committer();

    let mut mapping: SplitMapping = Vec::new();

    // 1) Commits AVANT la cible : inchangés (mêmes OID).
    let mut running_tip = base;
    for oid in &segment[..k] {
        mapping.push((vec![*oid], *oid));
        running_tip = *oid;
    }

    // 2) La cible → parts (adoption cumulative des fichiers depuis la version cible).
    let mut prev_tree = repo.find_commit(running_tip)?.tree()?;
    for part in parts {
        let new_tree =
            repo.find_tree(overlay_tree(repo, &prev_tree, &target_tree, &part.files)?)?;
        let parent = repo.find_commit(running_tip)?;
        let new_oid = repo.commit(
            None,
            &author,
            &committer,
            &part.message,
            &new_tree,
            &[&parent],
        )?;
        mapping.push((vec![target], new_oid));
        running_tip = new_oid;
        prev_tree = new_tree;
    }
    if prev_tree.id() != target_tree.id() {
        return Err(CoreError::Git(
            "découpe incohérente : la réunion des parts ne reconstitue pas l'arbre du commit"
                .into(),
        ));
    }

    // 3) Commits APRÈS la cible : rejoués sur le nouveau sommet. L'arbre à
    //    `running_tip` == arbre cible, donc chaque commit garde son arbre (contenu
    //    inchangé) ; seul le parent change → aucun conflit possible.
    for oid in &segment[k + 1..] {
        let c = repo.find_commit(*oid)?;
        let tree = c.tree()?;
        let parent = repo.find_commit(running_tip)?;
        let new_oid = repo.commit(
            None,
            &c.author(),
            &c.committer(),
            c.message().unwrap_or(""),
            &tree,
            &[&parent],
        )?;
        mapping.push((vec![*oid], new_oid));
        running_tip = new_oid;
    }

    Ok((running_tip, mapping))
}
