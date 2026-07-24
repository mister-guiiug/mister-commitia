//! mc-core — cœur de mister-commitia.
//!
//! Garde-fous non négociables (docs/11-risques-garde-fous.md §11.1) : dry-run
//! obligatoire, backup automatique, jamais d'action IA automatique, branches
//! protégées bloquées, trailers protégés intouchables, simulation avant toute
//! suppression CI, secrets au coffre OS, journal d'audit append-only.

pub mod ai;
pub mod analyzer;
pub mod api;
pub mod ci;
pub mod error;
pub mod gitx;
pub mod model;
pub mod plan;
pub mod secrets;
pub mod skills;
pub mod store;
pub mod task;

pub use api::Core;
pub use error::{CoreError, Result};
