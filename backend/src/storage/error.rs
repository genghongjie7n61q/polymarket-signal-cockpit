use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("storage queue is full")]
    QueueFull,
    #[error("storage writer is closed")]
    WriterClosed,
    #[error("invalid storage input: {0}")]
    InvalidInput(String),
}
