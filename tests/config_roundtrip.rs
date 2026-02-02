//! Integration test: Config serialization round-trip.
//!
//! Verifies that Config can be serialized to TOML, written to a file,
//! read back, and deserialized with all fields preserved. Also tests
//! serde default behavior for partial configs.

use std::fs;

use voice_dictation::app::config::Config;

/// Full round-trip: default Config → TOML → file → TOML → Config.
#[test]
fn config_save_load_roundtrip() {
    let dir = std::env::temp_dir().join("s2t_integ_config_roundtrip");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("config.toml");

    let original = Config::default();
    let toml_str = toml::to_string_pretty(&original).expect("serialize");
    fs::write(&path, &toml_str).expect("write");

    let content = fs::read_to_string(&path).expect("read");
    let loaded: Config = toml::from_str(&content).expect("deserialize");

    assert_eq!(loaded.default_model, original.default_model);
    assert_eq!(loaded.language, original.language);
    assert_eq!(loaded.history_max_entries, original.history_max_entries);
    assert_eq!(loaded.history_max_age_days, original.history_max_age_days);
    assert_eq!(loaded.auto_copy, original.auto_copy);
    assert_eq!(loaded.hotkey_enabled, original.hotkey_enabled);
    assert_eq!(loaded.hotkey, original.hotkey);
    assert_eq!(loaded.auto_paste, original.auto_paste);
    assert_eq!(loaded.recording_mode, original.recording_mode);
    assert_eq!(loaded.diarization_method, original.diarization_method);
    assert_eq!(loaded.sortformer_model_path, original.sortformer_model_path);
    assert_eq!(loaded.continuous_mode, original.continuous_mode);
    assert_eq!(loaded.segment_interval_secs, original.segment_interval_secs);
    assert_eq!(loaded.use_vad, original.use_vad);
    assert_eq!(
        loaded.vad_silence_threshold_ms,
        original.vad_silence_threshold_ms
    );
    assert_eq!(loaded.vad_min_speech_ms, original.vad_min_speech_ms);
    assert_eq!(loaded.denoise_enabled, original.denoise_enabled);
    assert_eq!(loaded.vad_engine, original.vad_engine);
    assert_eq!(loaded.silero_threshold, original.silero_threshold);
    assert_eq!(loaded.stt_backend, original.stt_backend);
    assert_eq!(loaded.tdt_model_path, original.tdt_model_path);
    assert_eq!(loaded.max_segment_secs, original.max_segment_secs);

    let _ = fs::remove_file(&path);
    let _ = fs::remove_dir(&dir);
}

/// Custom config preserves non-default values through round-trip.
#[test]
fn config_custom_values_roundtrip() {
    let original = Config {
        default_model: "ggml-large-v3.bin".to_string(),
        language: "en".to_string(),
        history_max_entries: 100,
        history_max_age_days: 30,
        auto_copy: true,
        hotkey_enabled: true,
        hotkey: "Alt+R".to_string(),
        auto_paste: true,
        recording_mode: "conference".to_string(),
        diarization_method: "sortformer".to_string(),
        sortformer_model_path: Some("/models/sortformer.onnx".to_string()),
        continuous_mode: true,
        segment_interval_secs: 10,
        use_vad: false,
        vad_silence_threshold_ms: 2000,
        vad_min_speech_ms: 1000,
        denoise_enabled: true,
        vad_engine: "silero".to_string(),
        silero_threshold: 0.8,
        stt_backend: "tdt".to_string(),
        tdt_model_path: Some("/models/tdt".to_string()),
        max_segment_secs: 600,
    };

    let toml_str = toml::to_string_pretty(&original).expect("serialize");
    let loaded: Config = toml::from_str(&toml_str).expect("deserialize");

    assert_eq!(loaded.default_model, "ggml-large-v3.bin");
    assert_eq!(loaded.language, "en");
    assert_eq!(loaded.history_max_entries, 100);
    assert!(loaded.auto_copy);
    assert!(loaded.hotkey_enabled);
    assert_eq!(loaded.hotkey, "Alt+R");
    assert!(loaded.auto_paste);
    assert_eq!(loaded.recording_mode, "conference");
    assert_eq!(loaded.diarization_method, "sortformer");
    assert_eq!(
        loaded.sortformer_model_path,
        Some("/models/sortformer.onnx".to_string())
    );
    assert!(loaded.continuous_mode);
    assert_eq!(loaded.segment_interval_secs, 10);
    assert!(!loaded.use_vad);
    assert_eq!(loaded.vad_silence_threshold_ms, 2000);
    assert!(loaded.denoise_enabled);
    assert_eq!(loaded.vad_engine, "silero");
    assert_eq!(loaded.silero_threshold, 0.8);
    assert_eq!(loaded.stt_backend, "tdt");
    assert_eq!(loaded.tdt_model_path, Some("/models/tdt".to_string()));
}

/// Partial TOML config fills missing fields with serde defaults.
#[test]
fn config_partial_toml_uses_defaults() {
    let partial_toml = r#"
default_model = "ggml-tiny.bin"
language = "de"
"#;

    let loaded: Config = toml::from_str(partial_toml).expect("deserialize partial");

    // Explicit fields preserved
    assert_eq!(loaded.default_model, "ggml-tiny.bin");
    assert_eq!(loaded.language, "de");

    // Missing fields get defaults
    let defaults = Config::default();
    assert_eq!(loaded.history_max_entries, defaults.history_max_entries);
    assert_eq!(loaded.auto_copy, defaults.auto_copy);
    assert_eq!(loaded.hotkey_enabled, defaults.hotkey_enabled);
    assert_eq!(loaded.recording_mode, defaults.recording_mode);
    assert_eq!(loaded.stt_backend, defaults.stt_backend);
    assert_eq!(loaded.vad_engine, defaults.vad_engine);
    assert_eq!(loaded.denoise_enabled, defaults.denoise_enabled);
}

/// TOML with unknown fields is silently ignored (forward compatibility).
/// This is intentional: older binaries can read configs saved by newer versions.
#[test]
fn config_unknown_fields_are_ignored() {
    let toml_with_extra = r#"
default_model = "ggml-base.bin"
language = "uk"
nonexistent_field = "value"
future_option = true
"#;

    let loaded: Config = toml::from_str(toml_with_extra).expect("should ignore unknown fields");
    assert_eq!(loaded.default_model, "ggml-base.bin");
    assert_eq!(loaded.language, "uk");
}

/// Empty TOML string fails (required fields missing).
#[test]
fn config_empty_toml_fails() {
    let result: Result<Config, _> = toml::from_str("");
    assert!(
        result.is_err(),
        "Empty TOML should fail due to missing required fields"
    );
}

/// Config can be cloned without data loss.
#[test]
fn config_clone_preserves_all_fields() {
    let original = Config {
        default_model: "test.bin".to_string(),
        language: "fr".to_string(),
        history_max_entries: 42,
        auto_copy: true,
        sortformer_model_path: Some("/path".to_string()),
        ..Config::default()
    };

    let cloned = original.clone();

    let orig_toml = toml::to_string(&original).unwrap();
    let clone_toml = toml::to_string(&cloned).unwrap();
    assert_eq!(orig_toml, clone_toml);
}
