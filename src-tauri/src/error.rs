use std::io;

use serde::Serialize;
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("project is not open")]
    ProjectNotOpen,
    #[error("invalid project: {0}")]
    InvalidProject(String),
    #[error("entity not found: {0}")]
    NotFound(String),
    #[error("duplicate id detected: {0}")]
    DuplicateId(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("request failed: {0}")]
    Request(String),
    #[error("websocket failed: {0}")]
    WebSocket(String),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
