/// Configuration for an ASR session.
/// Contains fields for all supported providers; each provider uses what it needs.
#[derive(Debug, Clone)]
pub struct AsrConfig {
    // ── Shared ───────────────────────────────────────────────────────
    /// Audio sample rate in Hz (default: 16000)
    pub sample_rate_hz: u32,
    /// Timeout waiting for final ASR result after finish signal (default: 5000)
    pub final_wait_timeout_ms: u64,
    /// Hotwords for improved recognition accuracy
    pub hotwords: Vec<String>,

    // ── sherpa-onnx specific ─────────────────────────────────────────
    /// Path to the sherpa-onnx model directory
    pub model_dir: String,
    /// Hotwords boosting score (default: 1.5)
    pub hotwords_score: f32,
    /// Number of threads for inference (default: 2)
    pub num_threads: i32,

    // ── FunASR specific ──────────────────────────────────────────────
    /// WebSocket endpoint URL (e.g. "ws://localhost:10096")
    pub url: String,
    /// Connection timeout in milliseconds (default: 3000)
    pub connect_timeout_ms: u64,
    /// FunASR mode: "2pass", "online", or "offline"
    pub mode: String,
    /// FunASR chunk size for streaming latency control, e.g. [5, 10, 5]
    pub chunk_size: Vec<u32>,
    /// Enable ITN (inverse text normalization)
    pub enable_itn: bool,
}

impl Default for AsrConfig {
    fn default() -> Self {
        Self {
            sample_rate_hz: 16000,
            final_wait_timeout_ms: 5000,
            hotwords: Vec::new(),
            // sherpa-onnx
            model_dir: String::new(),
            hotwords_score: 1.5,
            num_threads: 2,
            // FunASR
            url: "ws://localhost:10096".into(),
            connect_timeout_ms: 3000,
            mode: "2pass".into(),
            chunk_size: vec![5, 10, 5],
            enable_itn: true,
        }
    }
}
