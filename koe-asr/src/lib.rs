//! # koe-asr
//!
//! Streaming ASR (Automatic Speech Recognition) client using FunASR.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use koe_asr::{AsrConfig, AsrEvent, AsrProvider, FunAsrProvider, TranscriptAggregator};
//!
//! # async fn example() -> Result<(), koe_asr::AsrError> {
//! let config = AsrConfig::default(); // connects to ws://localhost:10096
//!
//! let mut asr = FunAsrProvider::new();
//! asr.connect(&config).await?;
//!
//! // Push audio frames...
//! // asr.send_audio(&pcm_data).await?;
//! asr.finish_input().await?;
//!
//! let mut aggregator = TranscriptAggregator::new();
//! loop {
//!     match asr.next_event().await? {
//!         AsrEvent::Interim(text) => aggregator.update_interim(&text),
//!         AsrEvent::Definite(text) => aggregator.update_definite(&text),
//!         AsrEvent::Final(text) => { aggregator.update_final(&text); break; }
//!         AsrEvent::Closed => break,
//!         _ => {}
//!     }
//! }
//!
//! println!("{}", aggregator.best_text());
//! asr.close().await?;
//! # Ok(())
//! # }
//! ```

pub mod config;
pub mod error;
pub mod event;
pub mod funasr;
pub mod provider;
pub mod transcript;

pub use config::AsrConfig;
pub use error::AsrError;
pub use event::AsrEvent;
pub use funasr::FunAsrProvider;
pub use provider::AsrProvider;
pub use transcript::TranscriptAggregator;
