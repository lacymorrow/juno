use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Voice controller not initialized")]
    NotInitialized,

    #[error("Initialization error: {0}")]
    InitializationError(String),

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Audio file not found: {0}")]
    AudioFileNotFound(String),

    #[error("Already dictating")]
    AlreadyDictating,

    #[error("Not dictating")]
    NotDictating,

    #[error("Audio device error: {0}")]
    AudioDevice(String),

    #[error("Resampling error: {0}")]
    Resampling(String),

    #[error("Whisper error: {0}")]
    Whisper(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("Tauri error: {0}")]
    Tauri(String),

    #[error("JSON error: {0}")]
    Json(String),

    #[error("Other error: {0}")]
    Other(String),

    #[error("Model error: {0}")]
    ModelError(String),

    #[error("Audio capture error: {0}")]
    AudioError(String),

    #[error("Transcription error: {0}")]
    TranscriptionError(String),

    #[error("Lock error: {0}")]
    LockError(String),

    #[error("Event error: {0}")]
    EventError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Control error: {0}")]
    ControlError(String),

    #[error("Microphone permission denied. Please grant microphone access in System Settings > Privacy & Security > Microphone")]
    MicrophonePermissionDenied,
}

impl serde::Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err.to_string())
    }
}

impl From<tauri::Error> for Error {
    fn from(err: tauri::Error) -> Self {
        Error::Tauri(err.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Json(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
