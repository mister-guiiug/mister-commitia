//! Contexte d'exécution des opérations longues : progression + annulation (T2).
//!
//! L'annulation est COOPÉRATIVE : le cœur vérifie le jeton à chaque point
//! d'arrêt sûr (entre deux commits lus, entre deux pages d'API, entre deux
//! groupes de génération…). Une opération annulée ne laisse jamais d'état
//! incohérent : les points de non-retour (backup + bascule de branche) ne
//! comportent aucun point d'annulation.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::Serialize;

use crate::error::{CoreError, Result};

/// Jeton d'annulation partageable entre threads (UI → cœur).
#[derive(Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    /// Point d'arrêt sûr : erreur `cancelled` si l'utilisateur a annulé.
    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(CoreError::Cancelled)
        } else {
            Ok(())
        }
    }
}

/// Événement émis vers l'UI pendant une opération longue.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskEvent {
    /// Avancement d'une phase ; `total` absent = indéterminé.
    Progress {
        phase: String,
        current: u64,
        total: Option<u64>,
    },
    /// Fragment de texte produit par un fournisseur IA (streaming, T11).
    AiDelta { group: u64, delta: String },
}

/// Enveloppe transportée jusqu'à l'UI (un seul canal, filtré par `task_id`).
#[derive(Debug, Clone, Serialize)]
pub struct TaskPayload {
    pub task_id: String,
    pub task: String,
    #[serde(flatten)]
    pub event: TaskEvent,
}

type EventFn = dyn Fn(TaskPayload) + Send + Sync;

/// Contexte passé aux opérations longues : identité de la tâche, jeton
/// d'annulation, émetteur d'événements (optionnel — `noop` pour les appels
/// synchrones historiques et les tests qui ne s'y intéressent pas).
pub struct TaskCtx {
    pub task: String,
    pub task_id: String,
    pub cancel: CancelToken,
    on_event: Option<Box<EventFn>>,
}

impl TaskCtx {
    /// Contexte inerte : aucune émission, jamais annulé.
    pub fn noop(task: &str) -> Self {
        Self {
            task: task.to_string(),
            task_id: String::new(),
            cancel: CancelToken::new(),
            on_event: None,
        }
    }

    pub fn new(
        task: &str,
        task_id: &str,
        cancel: CancelToken,
        on_event: impl Fn(TaskPayload) + Send + Sync + 'static,
    ) -> Self {
        Self {
            task: task.to_string(),
            task_id: task_id.to_string(),
            cancel,
            on_event: Some(Box::new(on_event)),
        }
    }

    fn send(&self, event: TaskEvent) {
        if let Some(f) = &self.on_event {
            f(TaskPayload {
                task_id: self.task_id.clone(),
                task: self.task.clone(),
                event,
            });
        }
    }

    /// Émet la progression SANS vérifier l'annulation — réservé aux phases
    /// au-delà du point de non-retour (« non annulable »).
    pub fn emit(&self, phase: &str, current: u64, total: Option<u64>) {
        self.send(TaskEvent::Progress {
            phase: phase.to_string(),
            current,
            total,
        });
    }

    /// Point d'arrêt sûr : vérifie l'annulation PUIS émet la progression.
    pub fn step(&self, phase: &str, current: u64, total: Option<u64>) -> Result<()> {
        self.cancel.check()?;
        self.emit(phase, current, total);
        Ok(())
    }

    /// Fragment IA (streaming) pour le groupe `group`.
    pub fn ai_delta(&self, group: u64, delta: &str) {
        self.send(TaskEvent::AiDelta {
            group,
            delta: delta.to_string(),
        });
    }
}
