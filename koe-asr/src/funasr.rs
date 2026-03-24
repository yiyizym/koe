use crate::config::AsrConfig;
use crate::error::{AsrError, Result};
use crate::event::AsrEvent;
use crate::provider::AsrProvider;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::time::{timeout, Duration};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

type WsStream = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

/// FunASR streaming ASR provider using WebSocket protocol.
///
/// Supports three modes:
/// - `online`: real-time streaming only
/// - `offline`: batch recognition only
/// - `2pass`: streaming + offline correction (recommended)
pub struct FunAsrProvider {
    ws: Option<WsStream>,
    closed: bool,
}

impl FunAsrProvider {
    pub fn new() -> Self {
        Self {
            ws: None,
            closed: false,
        }
    }
}

impl Default for FunAsrProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AsrProvider for FunAsrProvider {
    async fn connect(&mut self, config: &AsrConfig) -> Result<()> {
        let connect_timeout = Duration::from_millis(config.connect_timeout_ms);

        log::info!("connecting to FunASR: {}", config.url);

        let (ws_stream, _response) = timeout(connect_timeout, async {
            connect_async(&config.url)
                .await
                .map_err(|e| AsrError::Connection(e.to_string()))
        })
        .await
        .map_err(|_| AsrError::Connection("connection timed out".into()))??;

        self.ws = Some(ws_stream);

        // Build hotwords JSON: {"word1": 20, "word2": 20, ...}
        let hotwords_str = if !config.hotwords.is_empty() {
            let hotwords_map: serde_json::Map<String, Value> = config
                .hotwords
                .iter()
                .map(|w| (w.clone(), json!(20)))
                .collect();
            serde_json::to_string(&hotwords_map).unwrap_or_default()
        } else {
            String::new()
        };

        // Send initial configuration
        let init_msg = json!({
            "mode": config.mode,
            "wav_name": "koe",
            "is_speaking": true,
            "wav_format": "pcm",
            "chunk_size": config.chunk_size,
            "audio_fs": config.sample_rate_hz,
            "itn": config.enable_itn,
            "hotwords": hotwords_str,
        });

        log::info!(
            "FunASR config: mode={}, chunk_size={:?}, itn={}, hotwords={}",
            config.mode,
            config.chunk_size,
            config.enable_itn,
            config.hotwords.len(),
        );

        if let Some(ref mut ws) = self.ws {
            ws.send(Message::Text(init_msg.to_string().into()))
                .await
                .map_err(|e| AsrError::Connection(format!("send config: {e}")))?;
        }

        log::info!("FunASR connected, config sent");
        Ok(())
    }

    async fn send_audio(&mut self, frame: &[u8]) -> Result<()> {
        if let Some(ref mut ws) = self.ws {
            ws.send(Message::Binary(frame.to_vec().into()))
                .await
                .map_err(|e| AsrError::Protocol(format!("send audio: {e}")))?;
        }
        Ok(())
    }

    async fn finish_input(&mut self) -> Result<()> {
        let end_msg = json!({"is_speaking": false});
        if let Some(ref mut ws) = self.ws {
            ws.send(Message::Text(end_msg.to_string().into()))
                .await
                .map_err(|e| AsrError::Protocol(format!("send finish: {e}")))?;
        }
        log::debug!("FunASR finish signal sent");
        Ok(())
    }

    async fn next_event(&mut self) -> Result<AsrEvent> {
        if self.closed {
            return Ok(AsrEvent::Closed);
        }

        if let Some(ref mut ws) = self.ws {
            match ws.next().await {
                Some(Ok(Message::Text(text))) => {
                    let json: Value = serde_json::from_str(&text)
                        .map_err(|e| AsrError::Protocol(format!("parse JSON: {e}")))?;

                    let mode = json.get("mode").and_then(|m| m.as_str()).unwrap_or("");
                    let text = json
                        .get("text")
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string();
                    let is_final = json
                        .get("is_final")
                        .and_then(|f| f.as_bool())
                        .unwrap_or(false);

                    log::debug!("FunASR event: mode={mode}, is_final={is_final}, text_len={}", text.len());

                    if is_final {
                        self.closed = true;
                        return Ok(AsrEvent::Final(text));
                    }

                    match mode {
                        "2pass-online" | "online" => Ok(AsrEvent::Interim(text)),
                        "2pass-offline" | "offline" => Ok(AsrEvent::Definite(text)),
                        _ => {
                            log::debug!("FunASR unknown mode: {mode}");
                            Ok(AsrEvent::Interim(text))
                        }
                    }
                }
                Some(Ok(Message::Close(_))) => {
                    self.closed = true;
                    Ok(AsrEvent::Closed)
                }
                Some(Ok(_)) => Ok(AsrEvent::Interim(String::new())),
                Some(Err(e)) => Err(AsrError::Protocol(e.to_string())),
                None => {
                    self.closed = true;
                    Ok(AsrEvent::Closed)
                }
            }
        } else {
            Err(AsrError::Connection("not connected".into()))
        }
    }

    async fn close(&mut self) -> Result<()> {
        if let Some(mut ws) = self.ws.take() {
            let _ = ws.close(None).await;
        }
        self.closed = true;
        log::debug!("FunASR connection closed");
        Ok(())
    }
}
