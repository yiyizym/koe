use koe_asr::{AsrConfig, AsrEvent, AsrProvider, FunAsrProvider, SherpaProvider, TranscriptAggregator};

#[test]
fn test_default_config() {
    let config = AsrConfig::default();
    assert_eq!(config.sample_rate_hz, 16000);
    assert!(config.hotwords.is_empty());
    // sherpa defaults
    assert!(config.model_dir.is_empty());
    assert_eq!(config.hotwords_score, 1.5);
    assert_eq!(config.num_threads, 2);
    // funasr defaults
    assert_eq!(config.url, "ws://localhost:10096");
    assert_eq!(config.mode, "2pass");
}

#[test]
fn test_provider_creation() {
    let _sherpa = SherpaProvider::new();
    let _funasr = FunAsrProvider::new();
}

#[test]
fn test_transcript_aggregator_interim() {
    let mut agg = TranscriptAggregator::new();
    assert!(!agg.has_any_text());
    assert!(!agg.has_final_result());

    agg.update_interim("hello");
    assert!(agg.has_any_text());
    assert_eq!(agg.best_text(), "hello");

    agg.update_interim("hello world");
    assert_eq!(agg.best_text(), "hello world");
    assert_eq!(agg.interim_history(10).len(), 2);
}

#[test]
fn test_transcript_aggregator_definite_overrides_interim() {
    let mut agg = TranscriptAggregator::new();
    agg.update_interim("interim text");
    agg.update_definite("definite text");
    assert_eq!(agg.best_text(), "definite text");
}

#[test]
fn test_transcript_aggregator_final_overrides_all() {
    let mut agg = TranscriptAggregator::new();
    agg.update_interim("interim");
    agg.update_definite("definite");
    agg.update_final("final result");
    assert!(agg.has_final_result());
    assert_eq!(agg.best_text(), "final result");
}

#[test]
fn test_transcript_aggregator_history_limit() {
    let mut agg = TranscriptAggregator::new();
    for i in 0..20 {
        agg.update_interim(&format!("revision {i}"));
    }
    let history = agg.interim_history(5);
    assert_eq!(history.len(), 5);
    assert_eq!(history[0], "revision 15");
    assert_eq!(history[4], "revision 19");
}

#[test]
fn test_transcript_aggregator_dedup_consecutive() {
    let mut agg = TranscriptAggregator::new();
    agg.update_interim("same text");
    agg.update_interim("same text");
    agg.update_interim("same text");
    assert_eq!(agg.interim_history(10).len(), 1);
}

#[test]
fn test_asr_event_variants() {
    let events = vec![
        AsrEvent::Connected,
        AsrEvent::Interim("partial".into()),
        AsrEvent::Definite("confirmed".into()),
        AsrEvent::Final("done".into()),
        AsrEvent::Error("oops".into()),
        AsrEvent::Closed,
    ];
    for event in &events {
        let _ = format!("{:?}", event);
    }
    assert_eq!(events.len(), 6);
}

/// Integration test: requires a downloaded sherpa-onnx model.
#[tokio::test]
#[ignore]
async fn test_sherpa_connect() {
    let config = AsrConfig {
        model_dir: format!(
            "{}/.koe/models/sherpa-onnx-streaming-paraformer-bilingual-zh-en",
            std::env::var("HOME").unwrap()
        ),
        ..Default::default()
    };
    let mut provider = SherpaProvider::new();
    let result = provider.connect(&config).await;
    assert!(result.is_ok(), "failed to connect: {:?}", result.err());
    let _ = provider.close().await;
}

/// Integration test: requires a running FunASR server at ws://localhost:10096.
#[tokio::test]
#[ignore]
async fn test_funasr_connect() {
    let config = AsrConfig::default();
    let mut provider = FunAsrProvider::new();
    let result = provider.connect(&config).await;
    assert!(result.is_ok(), "failed to connect: {:?}", result.err());
    let _ = provider.close().await;
}

#[tokio::test]
async fn test_sherpa_connect_fails_without_model() {
    let config = AsrConfig {
        model_dir: "/nonexistent/model/path".into(),
        ..Default::default()
    };
    let mut provider = SherpaProvider::new();
    let result = provider.connect(&config).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_funasr_connect_fails_without_server() {
    let config = AsrConfig {
        url: "ws://localhost:19999".into(),
        connect_timeout_ms: 1000,
        ..Default::default()
    };
    let mut provider = FunAsrProvider::new();
    let result = provider.connect(&config).await;
    assert!(result.is_err());
}
