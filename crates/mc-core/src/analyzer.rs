use once_cell::sync::Lazy;
use regex::Regex;

use crate::model::*;

static WEAK_VOCAB: &[&str] = &[
    "wip", "fix", "fixes", "fixed", "update", "updates", "updated", "test", "tests", "tmp",
    "temp", "oops", "typo", "again", "cleanup", "clean", "minor", "stuff", "change", "changes",
    "misc", "foo", "bar", "asdf", "cont", "continue", "save", "work", "todo", "debug", "retry",
    "revert", "ok", "done", "final", "final2", "encore", "corrections", "correction", "maj",
];

static REF_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b[A-Z][A-Z0-9]+-\d+\b").unwrap());
static URL_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"https?://\S+").unwrap());

pub fn conventional_regex(types: &[String]) -> Regex {
    let alt = types
        .iter()
        .map(|t| regex::escape(t))
        .collect::<Vec<_>>()
        .join("|");
    Regex::new(&format!(r"^(?:{alt})(?:\([\w./-]+\))?!?:\s+\S.*")).unwrap()
}

pub fn is_conventional(subject: &str, types: &[String]) -> bool {
    conventional_regex(types).is_match(subject)
}

/// Références « à conserver » d'un message (tickets, URLs).
pub fn references(message: &str) -> Vec<String> {
    let mut out: Vec<String> = REF_RE
        .find_iter(message)
        .map(|m| m.as_str().to_string())
        .collect();
    out.extend(URL_RE.find_iter(message).map(|m| m.as_str().to_string()));
    out.sort();
    out.dedup();
    out
}

pub fn weak_score(subject: &str) -> (u8, Vec<String>) {
    let s = subject.trim();
    let lower = s.to_lowercase();
    let mut score: u32 = 0;
    let mut reasons = Vec::new();
    if s.len() < 10 {
        score += 40;
        reasons.push("sujet très court".to_string());
    }
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    if !words.is_empty() && words.len() <= 3 && words.iter().all(|w| WEAK_VOCAB.contains(w)) {
        score += 50;
        reasons.push("vocabulaire vide (wip/fix/update…)".to_string());
    } else if words.first().is_some_and(|w| WEAK_VOCAB.contains(w)) && words.len() <= 2 {
        score += 30;
        reasons.push("mot faible en tête".to_string());
    }
    if s.chars().all(|c| !c.is_alphabetic()) {
        score += 30;
        reasons.push("aucune lettre".to_string());
    }
    (score.min(100) as u8, reasons)
}

pub fn detect_ai_signatures(message: &str, patterns: &[String]) -> Vec<String> {
    let lower = message.to_lowercase();
    patterns
        .iter()
        .filter(|p| lower.contains(&p.to_lowercase()))
        .cloned()
        .collect()
}

pub fn analyze_commits(
    repo_ref: &RepoRef,
    branch: &str,
    base: Option<&str>,
    commits: &[CommitInfo],
) -> AnalysisReport {
    let gov = &repo_ref.governance;
    let mut flags = Vec::new();
    let mut conform = 0usize;
    let mut weak = 0usize;
    let mut ai = 0usize;
    let mut prev_subject: Option<String> = None;

    for c in commits {
        let (score, reasons) = weak_score(&c.subject);
        if score >= 40 {
            weak += 1;
            flags.push(CommitFlag {
                sha: c.sha.clone(),
                kind: FlagKind::WeakMessage,
                score,
                detail: reasons.join(" ; "),
            });
        }
        if is_conventional(&c.subject, &gov.convention_types) {
            conform += 1;
        } else {
            flags.push(CommitFlag {
                sha: c.sha.clone(),
                kind: FlagKind::NonConventional,
                score: 50,
                detail: "sujet non conforme à la convention du dépôt".to_string(),
            });
        }
        let sigs = detect_ai_signatures(&c.full_message(), &gov.signature_patterns);
        if !sigs.is_empty() {
            ai += 1;
            flags.push(CommitFlag {
                sha: c.sha.clone(),
                kind: FlagKind::AiSignature,
                score: 60,
                detail: format!("motifs détectés : {}", sigs.join(" | ")),
            });
        }
        if c.insertions + c.deletions > 300 && c.body.trim().is_empty() {
            flags.push(CommitFlag {
                sha: c.sha.clone(),
                kind: FlagKind::OversizedNoBody,
                score: 40,
                detail: format!(
                    "{} lignes modifiées sans corps de message",
                    c.insertions + c.deletions
                ),
            });
        }
        let norm = c.subject.trim().to_lowercase();
        if prev_subject.as_deref() == Some(norm.as_str()) {
            flags.push(CommitFlag {
                sha: c.sha.clone(),
                kind: FlagKind::DuplicateMessage,
                score: 40,
                detail: "même sujet que le commit précédent".to_string(),
            });
        }
        prev_subject = Some(norm);
    }

    AnalysisReport {
        repo_id: repo_ref.id.clone(),
        branch: branch.to_string(),
        base: base.map(String::from),
        tip: commits.last().map(|c| c.sha.clone()).unwrap_or_default(),
        total: commits.len(),
        conform,
        weak,
        ai_signatures: ai,
        flags,
        generated_at: now_iso(),
    }
}

/// Heuristique locale de regroupement pour la skill squash-advisor : itérations
/// consécutives d'un même auteur, fenêtre courte ou messages faibles à la suite
/// d'un commit porteur.
pub fn suggest_squash_groups(commits: &[CommitInfo]) -> Vec<Vec<String>> {
    let mut groups: Vec<Vec<String>> = Vec::new();
    let mut current: Vec<String> = Vec::new();
    let mut current_author = String::new();

    for c in commits {
        if c.is_merge || c.on_remote {
            if current.len() > 1 {
                groups.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
            continue;
        }
        let (score, _) = weak_score(&c.subject);
        let weakish = score >= 40;
        if current.is_empty() {
            current.push(c.sha.clone());
            current_author = c.author_email.clone();
        } else if c.author_email == current_author && weakish {
            current.push(c.sha.clone());
        } else {
            if current.len() > 1 {
                groups.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
            current.push(c.sha.clone());
            current_author = c.author_email.clone();
        }
    }
    if current.len() > 1 {
        groups.push(current);
    }
    groups
}
