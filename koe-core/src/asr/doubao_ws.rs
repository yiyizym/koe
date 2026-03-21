use crate::asr::{AsrConfig, AsrEvent, AsrProvider};
use crate::errors::{KoeError, Result};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::io::{Read, Write};
use tokio::time::{timeout, Duration};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use uuid::Uuid;

type WsStream = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

// ─── Binary Protocol Constants ──────────────────────────────────────

// Header byte 0: version (high nibble) | header_size (low nibble)
const PROTOCOL_VERSION: u8 = 0b0001;
const HEADER_SIZE: u8 = 0b0001; // 1 * 4 = 4 bytes

// Message types (high nibble of byte 1)
const MSG_FULL_CLIENT_REQUEST: u8 = 0b0001;
const MSG_AUDIO_ONLY: u8 = 0b0010;
const MSG_FULL_SERVER_RESPONSE: u8 = 0b1001;
const MSG_ERROR: u8 = 0b1111;

// Message type specific flags (low nibble of byte 1)
const FLAG_NONE: u8 = 0b0000;
#[allow(dead_code)]
const FLAG_HAS_SEQUENCE: u8 = 0b0001;
const FLAG_LAST_PACKET: u8 = 0b0010;
#[allow(dead_code)]
const FLAG_LAST_PACKET_WITH_SEQ: u8 = 0b0011;

// Serialization (high nibble of byte 2)
const SERIAL_NONE: u8 = 0b0000;
const SERIAL_JSON: u8 = 0b0001;

// Compression (low nibble of byte 2)
#[allow(dead_code)]
const COMPRESS_NONE: u8 = 0b0000;
const COMPRESS_GZIP: u8 = 0b0001;

fn build_header(msg_type: u8, flags: u8, serialization: u8, compression: u8) -> [u8; 4] {
    [
        (PROTOCOL_VERSION << 4) | HEADER_SIZE,
        (msg_type << 4) | flags,
        (serialization << 4) | compression,
        0x00, // reserved
    ]
}

fn gzip_compress(data: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(data)
        .map_err(|e| KoeError::AsrProtocol(format!("gzip compress: {e}")))?;
    encoder
        .finish()
        .map_err(|e| KoeError::AsrProtocol(format!("gzip finish: {e}")))
}

fn gzip_decompress(data: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = GzDecoder::new(data);
    let mut buf = Vec::new();
    decoder
        .read_to_end(&mut buf)
        .map_err(|e| KoeError::AsrProtocol(format!("gzip decompress: {e}")))?;
    Ok(buf)
}

/// Build a binary frame: header + payload_size (big-endian u32) + payload
fn build_frame(header: [u8; 4], payload: &[u8]) -> Vec<u8> {
    let payload_len = payload.len() as u32;
    let mut frame = Vec::with_capacity(4 + 4 + payload.len());
    frame.extend_from_slice(&header);
    frame.extend_from_slice(&payload_len.to_be_bytes());
    frame.extend_from_slice(payload);
    frame
}

// ─── Provider ───────────────────────────────────────────────────────

/// Doubao streaming ASR provider using the Volcengine binary WebSocket protocol.
///
/// Uses the "双向流式模式（优化版本）" endpoint (bigmodel_async) by default.
/// Protocol: custom binary framing over WebSocket with gzip-compressed payloads.
pub struct DoubaoWsProvider {
    ws: Option<WsStream>,
    connect_id: String,
    logid: Option<String>,
}

impl DoubaoWsProvider {
    pub fn new() -> Self {
        Self {
            ws: None,
            connect_id: Uuid::new_v4().to_string(),
            logid: None,
        }
    }

    /// Build the full client request payload (JSON, then gzip compressed).
    fn build_full_client_request(&self, config: &AsrConfig) -> Result<Vec<u8>> {
        let mut request = serde_json::json!({
            "model_name": "bigmodel",
            "enable_itn": config.enable_itn,
            "enable_punc": config.enable_punc,
            "enable_ddc": config.enable_ddc,
            "result_type": "full",
            "show_utterances": true
        });

        // Add hotwords if dictionary entries are available
        if !config.hotwords.is_empty() {
            let hotwords: Vec<serde_json::Value> = config.hotwords.iter()
                .map(|w| serde_json::json!({"word": w}))
                .collect();
            request["hotwords"] = serde_json::json!(hotwords);
            log::info!("ASR hotwords: {} entries", config.hotwords.len());
        }

        let payload_json = serde_json::json!({
            "user": {
                "uid": "koe-app"
            },
            "audio": {
                "format": "pcm",
                "codec": "raw",
                "rate": config.sample_rate_hz,
                "bits": 16,
                "channel": 1
            },
            "request": request
        });

        let json_bytes = serde_json::to_vec(&payload_json)
            .map_err(|e| KoeError::AsrProtocol(format!("serialize request: {e}")))?;

        let compressed = gzip_compress(&json_bytes)?;

        let header = build_header(MSG_FULL_CLIENT_REQUEST, FLAG_NONE, SERIAL_JSON, COMPRESS_GZIP);
        Ok(build_frame(header, &compressed))
    }

    /// Build an audio-only frame (gzip compressed PCM data).
    fn build_audio_frame(data: &[u8], is_last: bool) -> Result<Vec<u8>> {
        let compressed = gzip_compress(data)?;
        let flags = if is_last { FLAG_LAST_PACKET } else { FLAG_NONE };
        let header = build_header(MSG_AUDIO_ONLY, flags, SERIAL_NONE, COMPRESS_GZIP);
        Ok(build_frame(header, &compressed))
    }

    /// Parse a binary server response frame.
    fn parse_server_response(data: &[u8]) -> Result<ServerMessage> {
        if data.len() < 4 {
            return Err(KoeError::AsrProtocol("frame too short".into()));
        }

        let msg_type = (data[1] >> 4) & 0x0F;
        let flags = data[1] & 0x0F;
        let serialization = (data[2] >> 4) & 0x0F;
        let compression = data[2] & 0x0F;

        match msg_type {
            MSG_FULL_SERVER_RESPONSE => {
                let has_sequence = (flags & 0b0001) != 0;
                let is_last = (flags & 0b0010) != 0;

                let header_bytes = ((data[0] & 0x0F) as usize) * 4;
                let mut offset = header_bytes;

                let _sequence = if has_sequence {
                    if data.len() < offset + 4 {
                        return Err(KoeError::AsrProtocol("missing sequence".into()));
                    }
                    let seq = i32::from_be_bytes([
                        data[offset],
                        data[offset + 1],
                        data[offset + 2],
                        data[offset + 3],
                    ]);
                    offset += 4;
                    Some(seq)
                } else {
                    None
                };

                if data.len() < offset + 4 {
                    return Err(KoeError::AsrProtocol("missing payload size".into()));
                }
                let payload_size = u32::from_be_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]) as usize;
                offset += 4;

                if data.len() < offset + payload_size {
                    return Err(KoeError::AsrProtocol("incomplete payload".into()));
                }
                let payload_bytes = &data[offset..offset + payload_size];

                let json_bytes = if compression == COMPRESS_GZIP {
                    gzip_decompress(payload_bytes)?
                } else {
                    payload_bytes.to_vec()
                };

                let json: Value = if serialization == SERIAL_JSON {
                    serde_json::from_slice(&json_bytes)
                        .map_err(|e| KoeError::AsrProtocol(format!("parse JSON: {e}")))?
                } else {
                    Value::Null
                };

                Ok(ServerMessage::Response { json, is_last })
            }
            MSG_ERROR => {
                let header_bytes = ((data[0] & 0x0F) as usize) * 4;
                let mut offset = header_bytes;

                let error_code = if data.len() >= offset + 4 {
                    let code = u32::from_be_bytes([
                        data[offset],
                        data[offset + 1],
                        data[offset + 2],
                        data[offset + 3],
                    ]);
                    offset += 4;
                    code
                } else {
                    0
                };

                let error_msg = if data.len() >= offset + 4 {
                    let msg_size = u32::from_be_bytes([
                        data[offset],
                        data[offset + 1],
                        data[offset + 2],
                        data[offset + 3],
                    ]) as usize;
                    offset += 4;
                    if data.len() >= offset + msg_size {
                        String::from_utf8_lossy(&data[offset..offset + msg_size]).to_string()
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };

                Ok(ServerMessage::Error {
                    code: error_code,
                    message: error_msg,
                })
            }
            _ => Err(KoeError::AsrProtocol(format!(
                "unknown message type: {msg_type:#06b}"
            ))),
        }
    }
}

enum ServerMessage {
    Response { json: Value, is_last: bool },
    Error { code: u32, message: String },
}

impl AsrProvider for DoubaoWsProvider {
    async fn connect(&mut self, config: &AsrConfig) -> Result<()> {
        let connect_timeout = Duration::from_millis(config.connect_timeout_ms);

        log::info!(
            "connecting to ASR: {} (connect_id={})",
            config.url,
            self.connect_id
        );

        // Build request with auth headers
        let mut request = config
            .url
            .as_str()
            .into_client_request()
            .map_err(|e| KoeError::AsrConnection(format!("invalid URL: {e}")))?;

        let headers = request.headers_mut();
        headers.insert(
            "X-Api-App-Key",
            config
                .app_key
                .parse()
                .map_err(|_| KoeError::AsrConnection("invalid app_key".into()))?,
        );
        headers.insert(
            "X-Api-Access-Key",
            config
                .access_key
                .parse()
                .map_err(|_| KoeError::AsrConnection("invalid access_key".into()))?,
        );
        headers.insert(
            "X-Api-Resource-Id",
            config
                .resource_id
                .parse()
                .map_err(|_| KoeError::AsrConnection("invalid resource_id".into()))?,
        );
        headers.insert(
            "X-Api-Connect-Id",
            self.connect_id
                .parse()
                .map_err(|_| KoeError::AsrConnection("invalid connect_id".into()))?,
        );

        // Connect WebSocket
        let (ws_stream, response) = timeout(connect_timeout, async {
            connect_async(request)
                .await
                .map_err(|e| KoeError::AsrConnection(e.to_string()))
        })
        .await
        .map_err(|_| KoeError::AsrConnection("connection timed out".into()))??;

        // Capture logid from response headers
        if let Some(logid) = response.headers().get("X-Tt-Logid") {
            if let Ok(s) = logid.to_str() {
                self.logid = Some(s.to_string());
                log::info!("ASR logid: {s}");
            }
        }

        self.ws = Some(ws_stream);

        // Send full client request
        let full_request = self.build_full_client_request(config)?;
        if let Some(ref mut ws) = self.ws {
            ws.send(Message::Binary(full_request.into()))
                .await
                .map_err(|e| KoeError::AsrConnection(format!("send full request: {e}")))?;
        }

        log::info!("ASR connected, full client request sent");
        Ok(())
    }

    async fn send_audio(&mut self, frame: &[u8]) -> Result<()> {
        let binary_frame = Self::build_audio_frame(frame, false)?;
        if let Some(ref mut ws) = self.ws {
            ws.send(Message::Binary(binary_frame.into()))
                .await
                .map_err(|e| KoeError::AsrProtocol(format!("send audio: {e}")))?;
        }
        Ok(())
    }

    async fn finish_input(&mut self) -> Result<()> {
        // Send last audio packet (empty payload with last-packet flag)
        let last_frame = Self::build_audio_frame(&[], true)?;
        if let Some(ref mut ws) = self.ws {
            ws.send(Message::Binary(last_frame.into()))
                .await
                .map_err(|e| KoeError::AsrProtocol(format!("send finish: {e}")))?;
        }
        log::debug!("ASR finish signal sent (last packet)");
        Ok(())
    }

    async fn next_event(&mut self) -> Result<AsrEvent> {
        if let Some(ref mut ws) = self.ws {
            match ws.next().await {
                Some(Ok(Message::Binary(data))) => {
                    match Self::parse_server_response(&data)? {
                        ServerMessage::Response { json, is_last } => {
                            // Extract text from result
                            let text = json
                                .get("result")
                                .and_then(|r| r.get("text"))
                                .and_then(|t| t.as_str())
                                .unwrap_or("")
                                .to_string();

                            // Check if any utterance has definite=true
                            let has_definite = json
                                .get("result")
                                .and_then(|r| r.get("utterances"))
                                .and_then(|u| u.as_array())
                                .map(|utterances| {
                                    utterances.iter().any(|u| {
                                        u.get("definite")
                                            .and_then(|d| d.as_bool())
                                            .unwrap_or(false)
                                    })
                                })
                                .unwrap_or(false);

                            if is_last {
                                Ok(AsrEvent::Final(text))
                            } else if has_definite {
                                // Definite utterances in streaming mode
                                // are intermediate "confirmed" segments
                                Ok(AsrEvent::Interim(text))
                            } else {
                                Ok(AsrEvent::Interim(text))
                            }
                        }
                        ServerMessage::Error { code, message } => {
                            log::error!(
                                "ASR error: code={code}, message={message}, logid={:?}",
                                self.logid
                            );
                            Err(KoeError::AsrProtocol(format!(
                                "server error {code}: {message}"
                            )))
                        }
                    }
                }
                Some(Ok(Message::Close(_))) => Ok(AsrEvent::Closed),
                Some(Ok(_)) => {
                    // Skip text/ping/pong frames
                    Ok(AsrEvent::Interim(String::new()))
                }
                Some(Err(e)) => Err(KoeError::AsrProtocol(e.to_string())),
                None => Ok(AsrEvent::Closed),
            }
        } else {
            Err(KoeError::AsrConnection("not connected".into()))
        }
    }

    async fn close(&mut self) -> Result<()> {
        if let Some(mut ws) = self.ws.take() {
            let _ = ws.close(None).await;
        }
        log::debug!(
            "ASR connection closed (connect_id={}, logid={:?})",
            self.connect_id,
            self.logid
        );
        Ok(())
    }
}
