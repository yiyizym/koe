use crate::config::AsrConfig;
use crate::error::{AsrError, Result};
use crate::event::AsrEvent;
use crate::provider::AsrProvider;

use sherpa_onnx::{OnlineRecognizer, OnlineRecognizerConfig, OnlineStream};
use std::collections::VecDeque;
use std::path::Path;

/// sherpa-onnx streaming ASR provider.
///
/// Runs entirely locally using ONNX Runtime. No server or network required.
/// Supports streaming Zipformer (transducer) and Paraformer models.
pub struct SherpaProvider {
    recognizer: Option<OnlineRecognizer>,
    stream: Option<OnlineStream>,
    events: VecDeque<AsrEvent>,
    /// Text accumulated from completed utterance segments (endpoint resets).
    accumulated_text: String,
    /// Last emitted interim text, used to deduplicate events.
    last_interim: String,
    sample_rate: i32,
}

// Safety: SherpaProvider is only used within a single async task (run_session).
// The underlying C pointers are not shared across threads.
unsafe impl Send for SherpaProvider {}

impl SherpaProvider {
    pub fn new() -> Self {
        Self {
            recognizer: None,
            stream: None,
            events: VecDeque::new(),
            accumulated_text: String::new(),
            last_interim: String::new(),
            sample_rate: 16000,
        }
    }
}

impl Default for SherpaProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Discover ONNX model files in the given directory.
/// Returns (encoder, decoder, joiner, tokens, bpe_vocab).
/// Joiner is None for Paraformer models.
fn discover_model_files(
    model_dir: &Path,
) -> Result<(String, String, Option<String>, String, Option<String>)> {
    let entries: Vec<_> = std::fs::read_dir(model_dir)
        .map_err(|e| {
            AsrError::Connection(format!(
                "cannot read model dir {}: {e}",
                model_dir.display()
            ))
        })?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();

    let find_onnx = |keyword: &str| -> Option<String> {
        // Prefer int8 version for smaller size and faster inference
        let int8 = entries.iter().find(|p| {
            let name = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            name.contains(keyword) && name.ends_with(".int8.onnx")
        });
        if let Some(p) = int8 {
            return Some(p.to_string_lossy().to_string());
        }
        // Fall back to regular onnx
        entries
            .iter()
            .find(|p| {
                let name = p
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase();
                name.contains(keyword) && name.ends_with(".onnx") && !name.ends_with(".int8.onnx")
            })
            .map(|p| p.to_string_lossy().to_string())
    };

    let find_file = |filename: &str| -> Option<String> {
        entries
            .iter()
            .find(|p| p.file_name().unwrap_or_default() == filename)
            .map(|p| p.to_string_lossy().to_string())
    };

    let encoder = find_onnx("encoder").ok_or_else(|| {
        AsrError::Connection(format!(
            "no encoder model found in {}",
            model_dir.display()
        ))
    })?;
    let decoder = find_onnx("decoder").ok_or_else(|| {
        AsrError::Connection(format!(
            "no decoder model found in {}",
            model_dir.display()
        ))
    })?;
    let joiner = find_onnx("joiner"); // None for Paraformer models
    let tokens = find_file("tokens.txt").ok_or_else(|| {
        AsrError::Connection(format!("no tokens.txt found in {}", model_dir.display()))
    })?;
    let bpe_vocab = find_file("bpe.vocab");

    log::info!(
        "model files: encoder={}, decoder={}, joiner={:?}, tokens={}, bpe_vocab={:?}",
        encoder,
        decoder,
        joiner,
        tokens,
        bpe_vocab,
    );

    Ok((encoder, decoder, joiner, tokens, bpe_vocab))
}

impl AsrProvider for SherpaProvider {
    async fn connect(&mut self, config: &AsrConfig) -> Result<()> {
        let model_dir = Path::new(&config.model_dir);
        if !model_dir.exists() {
            return Err(AsrError::Connection(format!(
                "model directory not found: {} — download a model from \
                 https://github.com/k2-fsa/sherpa-onnx/releases/tag/asr-models",
                model_dir.display()
            )));
        }

        let (encoder, decoder, joiner, tokens, bpe_vocab) = discover_model_files(model_dir)?;

        let mut rc = OnlineRecognizerConfig::default();

        // Configure model based on type (transducer vs paraformer)
        if let Some(ref joiner_path) = joiner {
            // Transducer model (Zipformer)
            rc.model_config.transducer.encoder = Some(encoder);
            rc.model_config.transducer.decoder = Some(decoder);
            rc.model_config.transducer.joiner = Some(joiner_path.clone());
        } else {
            // Paraformer model
            rc.model_config.paraformer.encoder = Some(encoder);
            rc.model_config.paraformer.decoder = Some(decoder);
        }

        rc.model_config.tokens = Some(tokens);
        rc.model_config.num_threads = config.num_threads;
        rc.model_config.provider = Some("cpu".into());
        rc.enable_endpoint = true;

        // Hotwords require modified_beam_search and bpe_vocab (transducer only)
        if !config.hotwords.is_empty() && joiner.is_some() {
            rc.decoding_method = Some("modified_beam_search".into());
            rc.hotwords_score = config.hotwords_score;
            if let Some(bpe) = bpe_vocab {
                rc.model_config.bpe_vocab = Some(bpe);
                rc.model_config.modeling_unit = Some("cjkchar+bpe".into());
            }
        } else {
            rc.decoding_method = Some("greedy_search".into());
        }

        log::info!(
            "creating sherpa-onnx recognizer: model_dir={}, threads={}, hotwords={}",
            config.model_dir,
            config.num_threads,
            config.hotwords.len(),
        );

        let recognizer = OnlineRecognizer::create(&rc)
            .ok_or_else(|| AsrError::Connection("failed to create recognizer".into()))?;

        // Create stream with per-session hotwords
        let stream = if !config.hotwords.is_empty() && joiner.is_some() {
            let hotwords_str: String = config
                .hotwords
                .iter()
                .map(|w| format!("{} :{}", w, config.hotwords_score))
                .collect::<Vec<_>>()
                .join("\n");
            recognizer.create_stream_with_hotwords(&hotwords_str)
        } else {
            recognizer.create_stream()
        };

        self.recognizer = Some(recognizer);
        self.stream = Some(stream);
        self.sample_rate = config.sample_rate_hz as i32;

        log::info!("sherpa-onnx recognizer ready");
        Ok(())
    }

    async fn send_audio(&mut self, frame: &[u8]) -> Result<()> {
        let recognizer = self
            .recognizer
            .as_ref()
            .ok_or_else(|| AsrError::Connection("not connected".into()))?;
        let stream = self
            .stream
            .as_mut()
            .ok_or_else(|| AsrError::Connection("no stream".into()))?;

        // Convert int16 PCM to f32 samples normalized to [-1.0, 1.0]
        let samples: Vec<f32> = frame
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
            .collect();

        stream.accept_waveform(self.sample_rate, &samples);

        while recognizer.is_ready(stream) {
            recognizer.decode(stream);
        }

        // Check for endpoint (end of utterance segment)
        if recognizer.is_endpoint(stream) {
            if let Some(result) = recognizer.get_result(stream) {
                let text = result.text.trim().to_string();
                if !text.is_empty() {
                    if !self.accumulated_text.is_empty() {
                        self.accumulated_text.push(' ');
                    }
                    self.accumulated_text.push_str(&text);
                    self.events
                        .push_back(AsrEvent::Definite(self.accumulated_text.clone()));
                    log::debug!("endpoint: accumulated={}", self.accumulated_text);
                }
            }
            recognizer.reset(stream);
            self.last_interim.clear();
        } else {
            // Emit interim result if text changed
            let partial = recognizer
                .get_result(stream)
                .map(|r| r.text.trim().to_string())
                .unwrap_or_default();
            let full_interim = if self.accumulated_text.is_empty() {
                partial.clone()
            } else if partial.is_empty() {
                self.accumulated_text.clone()
            } else {
                format!("{} {}", self.accumulated_text, partial)
            };

            if !full_interim.is_empty() && full_interim != self.last_interim {
                self.last_interim = full_interim.clone();
                self.events.push_back(AsrEvent::Interim(full_interim));
            }
        }

        Ok(())
    }

    async fn finish_input(&mut self) -> Result<()> {
        let recognizer = self
            .recognizer
            .as_ref()
            .ok_or_else(|| AsrError::Connection("not connected".into()))?;
        let stream = self
            .stream
            .as_mut()
            .ok_or_else(|| AsrError::Connection("no stream".into()))?;

        stream.input_finished();

        while recognizer.is_ready(stream) {
            recognizer.decode(stream);
        }

        let text = recognizer
            .get_result(stream)
            .map(|r| r.text.trim().to_string())
            .unwrap_or_default();
        if !text.is_empty() {
            if !self.accumulated_text.is_empty() {
                self.accumulated_text.push(' ');
            }
            self.accumulated_text.push_str(&text);
        }

        if !self.accumulated_text.is_empty() {
            self.events
                .push_back(AsrEvent::Final(self.accumulated_text.clone()));
        } else {
            self.events.push_back(AsrEvent::Closed);
        }

        log::debug!("sherpa-onnx finish: final_text={}", self.accumulated_text);
        Ok(())
    }

    async fn next_event(&mut self) -> Result<AsrEvent> {
        if let Some(event) = self.events.pop_front() {
            return Ok(event);
        }
        // No events pending. Yield forever so that tokio::select! picks
        // the audio branch instead.
        std::future::pending().await
    }

    async fn close(&mut self) -> Result<()> {
        self.stream = None;
        self.recognizer = None;
        log::debug!("sherpa-onnx provider closed");
        Ok(())
    }
}
