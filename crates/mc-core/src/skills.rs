use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::analyzer;
use crate::error::{CoreError, Result};
use crate::gitx::GitEngine;
use crate::model::{AiAttributionPolicy, Governance, Risk};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillScope {
    #[serde(default)]
    pub repos: Vec<String>,
    #[serde(default)]
    pub branches: Vec<String>,
    #[serde(default)]
    pub operations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillOutput {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub must_explain: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillRule {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Guardrail {
    pub assert: String,
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default)]
    pub condition: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillTestCase {
    pub name: String,
    #[serde(default)]
    pub given: serde_yaml::Value,
    #[serde(default)]
    pub expect: serde_yaml::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TestsField {
    Path(String),
    Cases(Vec<SkillTestCase>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDef {
    #[serde(rename = "apiVersion", default)]
    pub api_version: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub owner: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub scope: SkillScope,
    #[serde(default)]
    pub risk_default: Option<String>,
    #[serde(default)]
    pub inputs: Vec<String>,
    #[serde(default)]
    pub output: Option<SkillOutput>,
    #[serde(default)]
    pub rules: Vec<SkillRule>,
    #[serde(default)]
    pub guardrails: Vec<Guardrail>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub prompt_file: Option<String>,
    #[serde(default)]
    pub detection_patterns: Vec<String>,
    #[serde(default)]
    pub examples: serde_yaml::Value,
    #[serde(default)]
    pub tests: Option<TestsField>,
}

#[derive(Debug, Clone)]
pub struct Skill {
    pub def: SkillDef,
    pub dir: PathBuf,
    pub prompt: String,
    pub test_cases: Vec<SkillTestCase>,
}

impl Skill {
    pub fn risk_default(&self) -> Risk {
        match self.def.risk_default.as_deref() {
            Some("high") => Risk::High,
            Some("medium") => Risk::Medium,
            _ => Risk::Low,
        }
    }
}

/// Skills chargées + erreurs de chargement (nom du dossier, motif).
pub type SkillLoadResult = (Vec<Skill>, Vec<(String, String)>);

/// Charge toutes les skills d'un dossier (un sous-dossier = une skill avec
/// `skill.yaml`). Les skills invalides sont ignorées avec leur motif.
pub fn load_dir(dir: &Path) -> Result<SkillLoadResult> {
    let mut skills = Vec::new();
    let mut errors = Vec::new();
    if !dir.exists() {
        return Ok((skills, errors));
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if !entry.path().is_dir() {
            continue;
        }
        let manifest = entry.path().join("skill.yaml");
        if !manifest.exists() {
            continue;
        }
        match load_one(&entry.path(), &manifest) {
            Ok(s) => skills.push(s),
            Err(e) => errors.push((
                entry.file_name().to_string_lossy().to_string(),
                e.to_string(),
            )),
        }
    }
    skills.sort_by(|a, b| a.def.name.cmp(&b.def.name));
    Ok((skills, errors))
}

fn load_one(dir: &Path, manifest: &Path) -> Result<Skill> {
    let raw = std::fs::read_to_string(manifest)?;
    let def: SkillDef = serde_yaml::from_str(&raw)?;
    let prompt = match (&def.prompt, &def.prompt_file) {
        (Some(p), _) => p.clone(),
        (None, Some(f)) => std::fs::read_to_string(dir.join(f))?,
        (None, None) => String::new(),
    };
    let test_cases = match &def.tests {
        Some(TestsField::Cases(cases)) => cases.clone(),
        Some(TestsField::Path(p)) => {
            let raw = std::fs::read_to_string(dir.join(p))?;
            serde_yaml::from_str(&raw)?
        }
        None => Vec::new(),
    };
    Ok(Skill {
        def,
        dir: dir.to_path_buf(),
        prompt,
        test_cases,
    })
}

// ---------------------------------------------------------------------------
// Garde-fous : post-conditions vérifiées PAR L'APPLICATION sur toute
// proposition, quelle que soit sa provenance (LLM ou heuristique locale).
// Un prompt n'est jamais un mécanisme de sécurité à lui seul.
// ---------------------------------------------------------------------------

static BREAKING_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^BREAKING CHANGE\b.*$").unwrap());

/// Issue d'une génération de skill, avant conversion en proposition.
#[derive(Debug, Clone)]
pub enum GenOutcome {
    Proposal {
        message: String,
        explanation: String,
        risk: Risk,
        removed: Vec<String>,
    },
    Refusal {
        explanation: String,
    },
}

pub fn validate_outcome(
    skill: &Skill,
    gov: &Governance,
    before: &str,
    outcome: &GenOutcome,
) -> Result<()> {
    for g in &skill.def.guardrails {
        match g.assert.as_str() {
            "must_refuse_when" => {
                let cond_met = matches!(
                    g.condition.as_deref(),
                    Some("repo.governance.ai_attribution_policy == 'keep-required'")
                ) && gov.ai_attribution_policy == AiAttributionPolicy::KeepRequired;
                if cond_met && matches!(outcome, GenOutcome::Proposal { .. }) {
                    return Err(CoreError::Refused(
                        "gouvernance du dépôt : la traçabilité IA est exigée (keep-required), la normalisation est refusée".into(),
                    ));
                }
            }
            "preserves_references" => {
                if let GenOutcome::Proposal { message, .. } = outcome {
                    for r in analyzer::references(before) {
                        if !message.contains(&r) {
                            return Err(CoreError::Refused(format!(
                                "garde-fou : la référence « {r} » du message d'origine serait perdue"
                            )));
                        }
                    }
                }
            }
            "preserves_protected_trailers" => {
                if let GenOutcome::Proposal { message, .. } = outcome {
                    for (key, value) in GitEngine::parse_trailers(before) {
                        if gov
                            .protected_trailers
                            .iter()
                            .any(|t| t.eq_ignore_ascii_case(&key))
                        {
                            let line = format!("{key}: {value}");
                            if !message.contains(&line) {
                                return Err(CoreError::Refused(format!(
                                    "garde-fou : le trailer protégé « {key} » serait supprimé"
                                )));
                            }
                        }
                    }
                }
            }
            "preserves_breaking_changes" => {
                if let GenOutcome::Proposal { message, .. } = outcome {
                    for m in BREAKING_RE.find_iter(before) {
                        if !message.contains(m.as_str()) {
                            return Err(CoreError::Refused(
                                "garde-fou : une mention BREAKING CHANGE serait perdue".into(),
                            ));
                        }
                    }
                }
            }
            "subject_matches" => {
                if let (GenOutcome::Proposal { message, .. }, Some(pat)) = (outcome, &g.pattern) {
                    let re = Regex::new(pat)
                        .map_err(|e| CoreError::Invalid(format!("pattern de garde-fou : {e}")))?;
                    let subject = message.lines().next().unwrap_or("");
                    if !re.is_match(subject) {
                        return Err(CoreError::Refused(format!(
                            "garde-fou : le sujet proposé ne respecte pas le format attendu ({subject})"
                        )));
                    }
                }
            }
            "only_removes_matched_patterns" => {
                if let GenOutcome::Proposal { message, .. } = outcome {
                    let patterns = if skill.def.detection_patterns.is_empty() {
                        &gov.signature_patterns
                    } else {
                        &skill.def.detection_patterns
                    };
                    let before_lines: Vec<&str> = before.lines().map(str::trim_end).collect();
                    for line in message.lines().map(str::trim_end) {
                        if !line.is_empty() && !before_lines.contains(&line) {
                            return Err(CoreError::Refused(format!(
                                "garde-fou : ligne ajoutée hors périmètre de nettoyage (« {line} »)"
                            )));
                        }
                    }
                    let after_lines: Vec<&str> = message.lines().map(str::trim_end).collect();
                    for line in &before_lines {
                        if !line.is_empty() && !after_lines.contains(line) {
                            let lower = line.to_lowercase();
                            let matched =
                                patterns.iter().any(|p| lower.contains(&p.to_lowercase()));
                            if !matched {
                                return Err(CoreError::Refused(format!(
                                    "garde-fou : ligne retirée sans motif détecté (« {line} »)"
                                )));
                            }
                        }
                    }
                }
            }
            // Garde-fous structurels, appliqués par le flux (jamais d'auto-apply,
            // rapports sans action…) ou par le plan-engine : rien à vérifier ici.
            "no_auto_apply"
            | "report_only"
            | "removed_content_journaled"
            | "leased_runs_always_protected"
            | "can_require_enhanced_confirmation"
            | "can_block"
            | "blocking_when"
            | "groups_within_segment"
            | "no_merge_commits_in_groups"
            | "shared_commits_flagged_high" => {}
            _ => {}
        }
    }
    Ok(())
}
