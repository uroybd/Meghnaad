use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("storage error: {0}")]
    Store(String),
    #[error("corrupt data: {0}")]
    Corrupt(String),
    #[error("decryption failed (wrong secret, salt, or tampered object)")]
    Decrypt,
    #[error("random number generator failure")]
    Rng,
    #[error("task {0} not found")]
    NotFound(uuid::Uuid),
    #[error("could not commit after repeated conflicts with other replicas")]
    TooMuchContention,
    #[error("invalid input: {0}")]
    Invalid(String),
}
