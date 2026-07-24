use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("git : {0}")]
    Git(String),
    #[error("stockage : {0}")]
    Db(String),
    #[error("http : {0}")]
    Http(String),
    /// Refus par un garde-fou (branche protégée, gouvernance, dry-run manquant…).
    #[error("refusé : {0}")]
    Refused(String),
    #[error("invalide : {0}")]
    Invalid(String),
    #[error("introuvable : {0}")]
    NotFound(String),
    #[error("io : {0}")]
    Io(String),
    #[error("secret : {0}")]
    Secret(String),
    #[error("limite de débit atteinte, réessayer dans {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },
}

pub type Result<T> = std::result::Result<T, CoreError>;

impl From<git2::Error> for CoreError {
    fn from(e: git2::Error) -> Self {
        CoreError::Git(e.message().to_string())
    }
}
impl From<rusqlite::Error> for CoreError {
    fn from(e: rusqlite::Error) -> Self {
        CoreError::Db(e.to_string())
    }
}
impl From<std::io::Error> for CoreError {
    fn from(e: std::io::Error) -> Self {
        CoreError::Io(e.to_string())
    }
}
impl From<serde_json::Error> for CoreError {
    fn from(e: serde_json::Error) -> Self {
        CoreError::Invalid(format!("json : {e}"))
    }
}
impl From<serde_yaml::Error> for CoreError {
    fn from(e: serde_yaml::Error) -> Self {
        CoreError::Invalid(format!("yaml : {e}"))
    }
}
impl From<reqwest::Error> for CoreError {
    fn from(e: reqwest::Error) -> Self {
        CoreError::Http(e.to_string())
    }
}
