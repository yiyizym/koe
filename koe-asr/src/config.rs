/// Configuration for an ASR session.
#[derive(Debug, Clone)]
pub struct AsrConfig {
    /// WebSocket endpoint URL (e.g. "ws://localhost:10096")
    pub url: String,
    /// Audio sample rate in Hz (default: 16000)
    pub sample_rate_hz: u32,
    /// Connection timeout in milliseconds (default: 3000)
    pub connect_timeout_ms: u64,
    /// Timeout waiting for final ASR result after finish signal (default: 5000)
    pub final_wait_timeout_ms: u64,
    /// Enable ITN (inverse text normalization)
    pub enable_itn: bool,
    /// Hotwords for improved recognition accuracy
    pub hotwords: Vec<String>,
    /// FunASR mode: "2pass", "online", or "offline"
    pub mode: String,
    /// FunASR chunk size for streaming latency control, e.g. [5, 10, 5]
    pub chunk_size: Vec<u32>,
}

impl Default for AsrConfig {
    fn default() -> Self {
        Self {
            url: "ws://localhost:10096".into(),
            sample_rate_hz: 16000,
            connect_timeout_ms: 3000,
            final_wait_timeout_ms: 5000,
            enable_itn: true,
            hotwords: Vec::new(),
            mode: "2pass".into(),
            chunk_size: vec![5, 10, 5],
        }
    }
}
