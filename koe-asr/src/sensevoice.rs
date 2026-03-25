use crate::config::AsrConfig;
use crate::error::{AsrError, Result};
use crate::event::AsrEvent;
use crate::provider::AsrProvider;

use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig, OfflineSenseVoiceModelConfig};
use std::collections::VecDeque;
use std::path::Path;

/// SenseVoice offline ASR provider.
///
/// Buffers all audio during recording, then runs recognition in one shot
/// when input finishes. Higher accuracy than streaming models but with
/// additional latency after the user stops speaking.
pub struct SenseVoiceProvider {
    recognizer: Option<OfflineRecognizer>,
    audio_buffer: Vec<f32>,
    events: VecDeque<AsrEvent>,
    sample_rate: i32,
}

// Safety: SenseVoiceProvider is only used within a single async task.
unsafe impl Send for SenseVoiceProvider {}

impl SenseVoiceProvider {
    pub fn new() -> Self {
        Self {
            recognizer: None,
            audio_buffer: Vec::new(),
            events: VecDeque::new(),
            sample_rate: 16000,
        }
    }
}

impl Default for SenseVoiceProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Find the SenseVoice model file in the given directory.
/// Returns (model_onnx, tokens) paths.
fn find_model_files(model_dir: &Path) -> Result<(String, String)> {
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

    // Prefer int8 model
    let model = entries
        .iter()
        .find(|p| {
            let name = p.file_name().unwrap_or_default().to_string_lossy();
            name.ends_with(".int8.onnx")
        })
        .or_else(|| {
            entries.iter().find(|p| {
                let name = p.file_name().unwrap_or_default().to_string_lossy();
                name.ends_with(".onnx")
            })
        })
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| {
            AsrError::Connection(format!("no .onnx model found in {}", model_dir.display()))
        })?;

    let tokens = entries
        .iter()
        .find(|p| p.file_name().unwrap_or_default() == "tokens.txt")
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| {
            AsrError::Connection(format!("no tokens.txt found in {}", model_dir.display()))
        })?;

    log::info!("SenseVoice model: {model}, tokens: {tokens}");
    Ok((model, tokens))
}

impl AsrProvider for SenseVoiceProvider {
    async fn connect(&mut self, config: &AsrConfig) -> Result<()> {
        let model_dir = Path::new(&config.model_dir);
        if !model_dir.exists() {
            return Err(AsrError::Connection(format!(
                "model directory not found: {} — download from \
                 https://github.com/k2-fsa/sherpa-onnx/releases/tag/asr-models",
                model_dir.display()
            )));
        }

        let (model, tokens) = find_model_files(model_dir)?;

        let mut rc = OfflineRecognizerConfig::default();
        rc.model_config.sense_voice = OfflineSenseVoiceModelConfig {
            model: Some(model),
            language: Some("auto".into()),
            use_itn: true,
        };
        rc.model_config.tokens = Some(tokens);
        rc.model_config.num_threads = config.num_threads;
        rc.model_config.provider = Some("cpu".into());

        log::info!(
            "creating SenseVoice recognizer: model_dir={}, threads={}",
            config.model_dir,
            config.num_threads,
        );

        let recognizer = OfflineRecognizer::create(&rc)
            .ok_or_else(|| AsrError::Connection("failed to create SenseVoice recognizer".into()))?;

        self.recognizer = Some(recognizer);
        self.sample_rate = config.sample_rate_hz as i32;
        self.audio_buffer.clear();

        log::info!("SenseVoice recognizer ready");
        Ok(())
    }

    async fn send_audio(&mut self, frame: &[u8]) -> Result<()> {
        // Convert int16 PCM to f32 and accumulate
        let samples: Vec<f32> = frame
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
            .collect();
        self.audio_buffer.extend_from_slice(&samples);
        Ok(())
    }

    async fn finish_input(&mut self) -> Result<()> {
        let recognizer = self
            .recognizer
            .as_ref()
            .ok_or_else(|| AsrError::Connection("not connected".into()))?;

        if self.audio_buffer.is_empty() {
            self.events.push_back(AsrEvent::Closed);
            return Ok(());
        }

        log::info!(
            "SenseVoice decoding {} samples ({:.1}s)",
            self.audio_buffer.len(),
            self.audio_buffer.len() as f32 / self.sample_rate as f32,
        );

        let stream = recognizer.create_stream();
        stream.accept_waveform(self.sample_rate, &self.audio_buffer);
        recognizer.decode(&stream);

        if let Some(result) = stream.get_result() {
            let text = result.text.trim().to_string();
            if !text.is_empty() {
                self.events.push_back(AsrEvent::Final(text));
            } else {
                self.events.push_back(AsrEvent::Closed);
            }
        } else {
            self.events.push_back(AsrEvent::Closed);
        }

        self.audio_buffer.clear();
        Ok(())
    }

    async fn next_event(&mut self) -> Result<AsrEvent> {
        if let Some(event) = self.events.pop_front() {
            return Ok(event);
        }
        // No events — offline model only produces events in finish_input().
        std::future::pending().await
    }

    async fn close(&mut self) -> Result<()> {
        self.recognizer = None;
        self.audio_buffer.clear();
        log::debug!("SenseVoice provider closed");
        Ok(())
    }
}
