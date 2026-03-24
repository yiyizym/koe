use crate::config::AsrConfig;
use crate::error::Result;
use crate::event::AsrEvent;
use crate::funasr::FunAsrProvider;
use crate::provider::AsrProvider;
use crate::sherpa::SherpaProvider;

/// Provider-agnostic wrapper that delegates to the selected ASR backend.
pub enum AnyProvider {
    Sherpa(SherpaProvider),
    FunAsr(FunAsrProvider),
}

impl AsrProvider for AnyProvider {
    async fn connect(&mut self, config: &AsrConfig) -> Result<()> {
        match self {
            AnyProvider::Sherpa(p) => p.connect(config).await,
            AnyProvider::FunAsr(p) => p.connect(config).await,
        }
    }

    async fn send_audio(&mut self, frame: &[u8]) -> Result<()> {
        match self {
            AnyProvider::Sherpa(p) => p.send_audio(frame).await,
            AnyProvider::FunAsr(p) => p.send_audio(frame).await,
        }
    }

    async fn finish_input(&mut self) -> Result<()> {
        match self {
            AnyProvider::Sherpa(p) => p.finish_input().await,
            AnyProvider::FunAsr(p) => p.finish_input().await,
        }
    }

    async fn next_event(&mut self) -> Result<AsrEvent> {
        match self {
            AnyProvider::Sherpa(p) => p.next_event().await,
            AnyProvider::FunAsr(p) => p.next_event().await,
        }
    }

    async fn close(&mut self) -> Result<()> {
        match self {
            AnyProvider::Sherpa(p) => p.close().await,
            AnyProvider::FunAsr(p) => p.close().await,
        }
    }
}
