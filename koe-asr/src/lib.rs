//! # koe-asr
//!
//! Streaming ASR (Automatic Speech Recognition) with multiple provider support.
//!
//! Providers:
//! - **sherpa-onnx**: local, offline, runs entirely on device
//! - **FunASR**: server-based via WebSocket, supports 2pass mode

pub mod any;
pub mod config;
pub mod error;
pub mod event;
pub mod funasr;
pub mod provider;
pub mod sensevoice;
pub mod sherpa;
pub mod transcript;

pub use any::AnyProvider;
pub use config::AsrConfig;
pub use error::AsrError;
pub use event::AsrEvent;
pub use funasr::FunAsrProvider;
pub use provider::AsrProvider;
pub use sensevoice::SenseVoiceProvider;
pub use sherpa::SherpaProvider;
pub use transcript::TranscriptAggregator;
