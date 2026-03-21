use std::fmt;

#[derive(Debug)]
pub enum KoeError {
    Config(String),
    AsrConnection(String),
    AsrTimeout,
    AsrProtocol(String),
    LlmFailed(String),
    LlmTimeout,
    SessionInvalidState { from: String, action: String },
    PermissionDenied(String),
    PasteFailed(String),
    AudioBuffer(String),
    Internal(String),
}

impl fmt::Display for KoeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KoeError::Config(msg) => write!(f, "config error: {msg}"),
            KoeError::AsrConnection(msg) => write!(f, "ASR connection failed: {msg}"),
            KoeError::AsrTimeout => write!(f, "ASR final result timed out"),
            KoeError::AsrProtocol(msg) => write!(f, "ASR protocol error: {msg}"),
            KoeError::LlmFailed(msg) => write!(f, "LLM correction failed: {msg}"),
            KoeError::LlmTimeout => write!(f, "LLM correction timed out"),
            KoeError::SessionInvalidState { from, action } => {
                write!(f, "invalid state transition: {action} from {from}")
            }
            KoeError::PermissionDenied(msg) => write!(f, "permission denied: {msg}"),
            KoeError::PasteFailed(msg) => write!(f, "paste failed: {msg}"),
            KoeError::AudioBuffer(msg) => write!(f, "audio buffer error: {msg}"),
            KoeError::Internal(msg) => write!(f, "internal error: {msg}"),
        }
    }
}

impl std::error::Error for KoeError {}

pub type Result<T> = std::result::Result<T, KoeError>;
