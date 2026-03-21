pub mod doubao_ws;

use crate::errors::Result;

#[derive(Debug, Clone)]
pub enum AsrEvent {
    Connected,
    Interim(String),
    Final(String),
    Error(String),
    Closed,
}

/// Configuration for an ASR session
pub struct AsrConfig {
    /// WebSocket endpoint URL
    pub url: String,
    /// X-Api-App-Key (App ID from Volcengine console)
    pub app_key: String,
    /// X-Api-Access-Key (Access Token from Volcengine console)
    pub access_key: String,
    /// X-Api-Resource-Id (e.g. "volc.bigasr.sauc.duration")
    pub resource_id: String,
    /// Audio sample rate in Hz
    pub sample_rate_hz: u32,
    /// Connection timeout in milliseconds
    pub connect_timeout_ms: u64,
    /// Timeout waiting for final ASR result after finish signal
    pub final_wait_timeout_ms: u64,
    /// Enable DDC (disfluency removal / smoothing)
    pub enable_ddc: bool,
    /// Enable ITN (inverse text normalization)
    pub enable_itn: bool,
    /// Enable automatic punctuation
    pub enable_punc: bool,
    /// Hotwords for improved recognition accuracy
    pub hotwords: Vec<String>,
}

/// Trait for streaming ASR providers.
/// Each session creates a new provider instance.
#[allow(async_fn_in_trait)]
pub trait AsrProvider: Send {
    async fn connect(&mut self, config: &AsrConfig) -> Result<()>;
    async fn send_audio(&mut self, frame: &[u8]) -> Result<()>;
    async fn finish_input(&mut self) -> Result<()>;
    async fn next_event(&mut self) -> Result<AsrEvent>;
    async fn close(&mut self) -> Result<()>;
}
