use koe_asr::{AsrConfig, AsrEvent, AsrProvider, FunAsrProvider, TranscriptAggregator};

#[test]
fn test_default_config() {
    let config = AsrConfig::default();
    assert_eq!(config.sample_rate_hz, 16000);
    assert_eq!(config.url, "ws://localhost:10096");
    assert_eq!(config.mode, "2pass");
    assert_eq!(config.chunk_size, vec![5, 10, 5]);
    assert!(config.enable_itn);
    assert!(config.hotwords.is_empty());
}

#[test]
fn test_custom_config() {
    let config = AsrConfig {
        url: "ws://localhost:9090".into(),
        hotwords: vec!["Rust".into(), "Tokio".into()],
        mode: "online".into(),
        ..Default::default()
    };
    assert_eq!(config.url, "ws://localhost:9090");
    assert_eq!(config.hotwords.len(), 2);
    assert_eq!(config.mode, "online");
}

#[test]
fn test_provider_creation() {
    let _provider = FunAsrProvider::new();
    // Provider should be constructable without panicking
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
    // Ensure all variants can be constructed and debug-printed
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

/// Integration test: requires a running FunASR server at ws://localhost:10096.
/// Run with: cargo test --test api_test test_funasr_connect -- --ignored
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
async fn test_funasr_connect_fails_without_server() {
    let config = AsrConfig {
        url: "ws://localhost:19999".into(), // unlikely to have a server here
        connect_timeout_ms: 1000,
        ..Default::default()
    };
    let mut provider = FunAsrProvider::new();
    let result = provider.connect(&config).await;
    assert!(result.is_err());
}
