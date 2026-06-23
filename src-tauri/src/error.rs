use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("MIDI error: {0}")]
    Midi(String),

    #[error("Audio error: {0}")]
    Audio(String),

    #[error("Profile error: {0}")]
    Profile(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("Not connected")]
    NotConnected,
}

/// Serializes to a plain error string for Tauri command responses.
impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        Self::Serialization(e.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
