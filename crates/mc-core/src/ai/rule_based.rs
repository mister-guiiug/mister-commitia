//! Assistant déterministe local : aucune dépendance réseau, sorties
//! reproductibles. C'est le repli quand aucun fournisseur LLM n'est configuré,
//! et le véhicule des tests de garde-fous.

use crate::analyzer;
use crate::error::{CoreError, Result};
use crate::gitx::GitEngine;
use crate::model::{AiAttributionPolicy, CommitInfo, Risk};
use crate::skills::GenOutcome;

use super::SkillContext;

pub fn generate(ctx: &SkillContext<'_>) -> Result<GenOutcome> {
    match ctx.skill.def.name.as_str() {
        "conventional-commits" => conventional(ctx),
        "commit-synthesis" => synthesis(ctx),
        "ai-signature-cleaner" => cleaner(ctx),
        other => Err(CoreError::Invalid(format!(
            "la skill « {other} » n'est pas couverte par l'assistant local — configurer un fournisseur LLM"
        ))),
    }
}

fn infer_type(c: &CommitInfo, types: &[String]) -> String {
    let hay = format!("{} {}", c.subject, c.body).to_lowercase();
    let pick = |t: &str| -> Option<String> {
        types.iter().find(|x| x.as_str() == t).cloned()
    };
    // Les fichiers touchés priment sur les mots-clés du message.
    if !c.files.is_empty() {
        let lower: Vec<String> = c.files.iter().map(|f| f.to_lowercase()).collect();
        if lower.iter().all(|f| f.ends_with(".md") || f.starts_with("docs/")) {
            if let Some(t) = pick("docs") {
                return t;
            }
        }
        if lower
            .iter()
            .all(|f| f.starts_with(".github/workflows") || f.contains("azure-pipelines"))
        {
            if let Some(t) = pick("ci") {
                return t;
            }
        }
        if lower.iter().all(|f| f.contains("test") || f.contains("spec")) {
            if let Some(t) = pick("test") {
                return t;
            }
        }
    }
    let candidates: [(&str, &[&str]); 7] = [
        ("fix", &["fix", "bug", "corrig", "répar", "repar", "hotfix", "crash"]),
        ("docs", &["doc", "readme", "documentation"]),
        ("test", &["test", "spec", "coverage"]),
        ("ci", &["ci", "workflow", "pipeline", "action", "deploy"]),
        ("perf", &["perf", "optim", "speed", "lent"]),
        ("refactor", &["refactor", "rename", "renomm", "clean", "nettoy", "restructur"]),
        ("build", &["build", "dependenc", "dépendanc", "bump", "upgrade", "package"]),
    ];
    for (t, keys) in candidates {
        if keys.iter().any(|k| hay.contains(k)) {
            if let Some(t) = pick(t) {
                return t;
            }
        }
    }
    if c.insertions >= c.deletions {
        pick("feat").unwrap_or_else(|| "feat".into())
    } else {
        pick("refactor").unwrap_or_else(|| "refactor".into())
    }
}

fn conventional(ctx: &SkillContext<'_>) -> Result<GenOutcome> {
    let c = ctx
        .commits
        .first()
        .ok_or_else(|| CoreError::Invalid("aucun commit fourni".into()))?;
    let types = &ctx.governance.convention_types;

    // Si déjà conforme, conserver le type déclaré ; sinon inférer du contenu.
    let existing = types
        .iter()
        .find(|t| {
            c.subject.starts_with(&format!("{t}:")) || c.subject.starts_with(&format!("{t}("))
        })
        .cloned();
    let ty = existing.clone().unwrap_or_else(|| infer_type(c, types));

    let mut cleaned = c.subject.clone();
    if let Some(t) = &existing {
        if let Some(pos) = cleaned.find(':') {
            let _ = t;
            cleaned = cleaned[pos + 1..].trim().to_string();
        }
    }
    let (weakness, _) = analyzer::weak_score(&cleaned);
    if weakness >= 40 {
        cleaned = format!(
            "préciser l'intention ({} fichier(s), +{} -{})",
            c.files_changed, c.insertions, c.deletions
        );
    }
    if let Some(first) = cleaned.chars().next() {
        cleaned = first.to_lowercase().collect::<String>() + &cleaned[first.len_utf8()..];
    }
    let max = 72usize.saturating_sub(ty.len() + 2);
    if cleaned.len() > max {
        let mut cut = max.saturating_sub(1);
        while cut > 0 && !cleaned.is_char_boundary(cut) {
            cut -= 1;
        }
        cleaned = format!("{}…", &cleaned[..cut]);
    }

    let mut message = format!("{ty}: {cleaned}");
    if !c.body.trim().is_empty() {
        message.push_str("\n\n");
        message.push_str(c.body.trim());
    }
    // Références présentes dans le sujet d'origine mais absentes du nouveau
    // message → repoussées en pied (règle « pas de perte »).
    let missing: Vec<String> = analyzer::references(&c.full_message())
        .into_iter()
        .filter(|r| !message.contains(r.as_str()))
        .collect();
    if !missing.is_empty() {
        message.push_str(&format!("\n\nRefs: {}", missing.join(", ")));
    }

    Ok(GenOutcome::Proposal {
        message,
        explanation: format!(
            "Heuristique locale (sans LLM) : type « {ty} » {} ; sujet normalisé (impératif, ≤ 72 caractères) ; corps et références conservés.",
            if existing.is_some() { "conservé du message d'origine" } else { "inféré des mots-clés et du diff" }
        ),
        risk: Risk::Low,
        removed: Vec::new(),
    })
}

fn synthesis(ctx: &SkillContext<'_>) -> Result<GenOutcome> {
    if ctx.commits.len() < 2 {
        return Err(CoreError::Invalid("la synthèse requiert plusieurs commits".into()));
    }
    let types = &ctx.governance.convention_types;

    // Sujet porteur : le moins « faible », le plus long en cas d'égalité.
    let primary = ctx
        .commits
        .iter()
        .min_by_key(|c| {
            let (w, _) = analyzer::weak_score(&c.subject);
            (w, usize::MAX - c.subject.len())
        })
        .unwrap();

    let ty = types
        .iter()
        .find(|t| {
            primary.subject.starts_with(&format!("{t}:"))
                || primary.subject.starts_with(&format!("{t}("))
        })
        .cloned()
        .unwrap_or_else(|| infer_type(primary, types));
    let subject_core = primary
        .subject
        .split_once(':')
        .map(|(_, r)| r.trim().to_string())
        .filter(|_| primary.subject.starts_with(ty.as_str()))
        .unwrap_or_else(|| primary.subject.clone());

    let mut kept = Vec::new();
    let mut abandoned = Vec::new();
    let mut breaking = Vec::new();
    let mut trailers = Vec::new();
    for c in ctx.commits {
        let (w, _) = analyzer::weak_score(&c.subject);
        if w >= 40 {
            abandoned.push(c.subject.clone());
        } else if c.sha != primary.sha {
            kept.push(c.subject.clone());
        }
        for line in c.full_message().lines() {
            if line.starts_with("BREAKING CHANGE") && !breaking.contains(&line.to_string()) {
                breaking.push(line.to_string());
            }
        }
        for (k, v) in GitEngine::parse_trailers(&c.full_message()) {
            if ctx
                .governance
                .protected_trailers
                .iter()
                .any(|t| t.eq_ignore_ascii_case(&k))
            {
                let line = format!("{k}: {v}");
                if !trailers.contains(&line) {
                    trailers.push(line);
                }
            }
        }
    }
    let refs = analyzer::references(&ctx.before());

    let mut message = format!("{ty}: {subject_core}");
    let mut body = Vec::new();
    if !kept.is_empty() {
        body.push(format!("Inclut également : {}.", kept.join(" ; ")));
    }
    body.push(format!(
        "Synthèse de {} commits d'itération.",
        ctx.commits.len()
    ));
    if !breaking.is_empty() {
        body.push(breaking.join("\n"));
    }
    if !refs.is_empty() {
        body.push(format!("Refs: {}", refs.join(", ")));
    }
    if !trailers.is_empty() {
        body.push(trailers.join("\n"));
    }
    message.push_str("\n\n");
    message.push_str(&body.join("\n\n"));

    Ok(GenOutcome::Proposal {
        message,
        explanation: format!(
            "Heuristique locale : sujet porteur « {} » retenu, {} sujet(s) de bruit absorbé(s) ({}), références et trailers protégés reportés.",
            primary.subject,
            abandoned.len(),
            if abandoned.is_empty() { "aucun".to_string() } else { abandoned.join(", ") }
        ),
        risk: Risk::Medium,
        removed: abandoned,
    })
}

fn cleaner(ctx: &SkillContext<'_>) -> Result<GenOutcome> {
    if ctx.governance.ai_attribution_policy == AiAttributionPolicy::KeepRequired {
        return Ok(GenOutcome::Refusal {
            explanation: "Politique du dépôt : traçabilité IA exigée (keep-required).".into(),
        });
    }
    let c = ctx
        .commits
        .first()
        .ok_or_else(|| CoreError::Invalid("aucun commit fourni".into()))?;
    let patterns: Vec<String> = if ctx.skill.def.detection_patterns.is_empty() {
        ctx.governance.signature_patterns.clone()
    } else {
        ctx.skill.def.detection_patterns.clone()
    };
    let protected = &ctx.governance.protected_trailers;

    let before = c.full_message();
    let mut removed = Vec::new();
    let mut lines_out: Vec<String> = Vec::new();
    for line in before.lines() {
        let lower = line.to_lowercase();
        let is_protected = protected.iter().any(|t| {
            lower.starts_with(&format!("{}:", t.to_lowercase()))
        });
        let matched = !is_protected && patterns.iter().any(|p| lower.contains(&p.to_lowercase()));
        if matched {
            removed.push(line.to_string());
        } else {
            lines_out.push(line.to_string());
        }
    }
    if removed.is_empty() {
        return Ok(GenOutcome::Refusal {
            explanation: "Aucune mention automatique détectée dans ce message.".into(),
        });
    }
    // Compacter les lignes vides consécutives laissées par le retrait.
    let mut compact: Vec<String> = Vec::new();
    for l in lines_out {
        if l.trim().is_empty() && compact.last().is_some_and(|p| p.trim().is_empty()) {
            continue;
        }
        compact.push(l);
    }
    while compact.last().is_some_and(|l| l.trim().is_empty()) {
        compact.pop();
    }
    let message = compact.join("\n");

    Ok(GenOutcome::Proposal {
        message,
        explanation: format!(
            "Normalisation autorisée par la politique du dépôt (normalization-allowed) : {} ligne(s) de mention automatique retirée(s). Le contenu retiré est conservé au journal d'audit.",
            removed.len()
        ),
        risk: Risk::Low,
        removed,
    })
}
