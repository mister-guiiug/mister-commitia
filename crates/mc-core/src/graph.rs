//! Disposition en « lanes » d'un segment de commits pour la vue graphe (F1).
//!
//! Fonction PURE sur la liste déjà lue par l'analyse (ordre du plus ancien au
//! plus récent). On assigne à chaque commit une colonne (lane) et on expose la
//! position de chaque parent, de sorte que l'UI puisse tracer nœuds et arêtes
//! en SVG sans re-parcourir le dépôt. Les merges rendent le graphe non linéaire
//! (plusieurs lanes) ; les parents hors du segment visible sont marqués comme
//! « bornes » (l'arête sort du cadre — typiquement la base du segment).

use serde::{Deserialize, Serialize};

use crate::model::CommitInfo;

/// Arête d'un commit vers l'un de ses parents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphParent {
    pub sha: String,
    /// Colonne où l'arête aboutit. Pour un parent dans le segment, c'est la
    /// lane où ce parent sera dessiné ; pour une borne, une colonne de sortie.
    pub lane: usize,
    /// Faux si le parent est hors du segment visible (arête tronquée / borne).
    pub in_segment: bool,
}

/// Un commit positionné : ligne (0 = le plus récent), colonne, parents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub sha: String,
    pub row: usize,
    pub lane: usize,
    pub is_merge: bool,
    pub parents: Vec<GraphParent>,
}

/// Graphe positionné : nœuds du plus RÉCENT au plus ancien + largeur (nb lanes).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitGraph {
    pub nodes: Vec<GraphNode>,
    pub lanes: usize,
}

/// Construit la disposition. `commits` est fourni du plus ancien au plus récent
/// (comme `ScanResult.commits`) ; le graphe est produit du plus récent au plus
/// ancien (sens de lecture d'un `git log --graph`).
pub fn build_graph(commits: &[CommitInfo]) -> CommitGraph {
    // Du plus récent au plus ancien.
    let order: Vec<&CommitInfo> = commits.iter().rev().collect();
    let in_segment: std::collections::HashSet<&str> =
        order.iter().map(|c| c.sha.as_str()).collect();

    // Chaque lane active « attend » le SHA d'un commit encore à venir (plus bas).
    let mut lanes: Vec<Option<String>> = Vec::new();
    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut max_lanes = 0usize;

    for (row, c) in order.iter().enumerate() {
        // Lane du commit : la première qui l'attend. Les autres lanes qui
        // l'attendent (plusieurs enfants) fusionnent et sont libérées.
        let mut my_lane: Option<usize> = None;
        for slot in lanes.iter_mut() {
            if slot.as_deref() == Some(c.sha.as_str()) {
                match my_lane {
                    None => my_lane = Some(0), // marque « trouvé » ; index fixé après
                    Some(_) => *slot = None,
                }
            }
        }
        // Récupère l'index réel de la première lane qui l'attendait.
        let my_lane = match my_lane {
            Some(_) => lanes
                .iter()
                .position(|s| s.as_deref() == Some(c.sha.as_str()))
                .unwrap(),
            None => {
                // Tip (aucun enfant dans le segment) : nouvelle lane.
                lanes.push(None);
                lanes.len() - 1
            }
        };

        // Assigne les parents : le premier reprend la lane du commit, les
        // suivants (merge) prennent une lane libre. Les parents hors segment
        // deviennent des bornes (pas de lane réservée qui continuerait à vide).
        let mut parents = Vec::with_capacity(c.parents.len());
        let mut first_in_lane_consumed = false;
        for p in &c.parents {
            let seg = in_segment.contains(p.as_str());
            if !first_in_lane_consumed {
                first_in_lane_consumed = true;
                lanes[my_lane] = if seg { Some(p.clone()) } else { None };
                parents.push(GraphParent {
                    sha: p.clone(),
                    lane: my_lane,
                    in_segment: seg,
                });
            } else if seg {
                let lane = free_lane(&mut lanes, p.clone());
                parents.push(GraphParent {
                    sha: p.clone(),
                    lane,
                    in_segment: true,
                });
            } else {
                // Merge d'un parent hors segment (ex. main fusionnée) : borne
                // sur une colonne de sortie à droite, sans réserver de lane.
                parents.push(GraphParent {
                    sha: p.clone(),
                    lane: lanes.len(),
                    in_segment: false,
                });
            }
        }
        if c.parents.is_empty() {
            lanes[my_lane] = None; // racine : la lane s'arrête
        }

        max_lanes = max_lanes.max(lanes.len());
        nodes.push(GraphNode {
            sha: c.sha.clone(),
            row,
            lane: my_lane,
            is_merge: c.parents.len() > 1,
            parents,
        });
    }

    CommitGraph {
        nodes,
        lanes: max_lanes.max(1),
    }
}

/// Réutilise la première lane libre (None) ou en crée une, et l'oriente vers
/// `target`. Retourne son index.
fn free_lane(lanes: &mut Vec<Option<String>>, target: String) -> usize {
    if let Some(i) = lanes.iter().position(|s| s.is_none()) {
        lanes[i] = Some(target);
        i
    } else {
        lanes.push(Some(target));
        lanes.len() - 1
    }
}
