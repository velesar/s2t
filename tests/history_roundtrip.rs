//! Integration test: History persistence round-trip.
//!
//! Verifies that History with various entry types can be serialized to JSON,
//! written to disk, loaded back, and all data is preserved. Also tests
//! search, cleanup, trimming, and export through the full persistence path.

use std::fs;
use std::path::PathBuf;

use chrono::{Duration, TimeZone, Utc};

use voice_dictation::domain::traits::HistoryRepository;
use voice_dictation::history::{export_to_text, History, HistoryEntry};

/// Helper: create a HistoryEntry with a specific timestamp.
fn entry_at(text: &str, timestamp: chrono::DateTime<Utc>) -> HistoryEntry {
    HistoryEntry {
        id: uuid::Uuid::new_v4().to_string(),
        text: text.to_string(),
        timestamp,
        duration_secs: 5.0,
        language: "uk".to_string(),
        recording_path: None,
        speakers: Vec::new(),
    }
}

/// Full round-trip: build History → JSON → file → load → verify all fields.
#[test]
fn history_persistence_roundtrip() {
    let dir = std::env::temp_dir().join("s2t_integ_history_rt");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("history.json");

    // Build history with diverse entry types
    let mut history = History::default();
    history.add(HistoryEntry::new(
        "Simple dictation".to_string(),
        5.0,
        "uk".to_string(),
    ));
    history.add(HistoryEntry::new_with_recording(
        "Conference notes with diarization".to_string(),
        120.0,
        "en".to_string(),
        Some("/tmp/conference_2026-01-30.wav".to_string()),
        vec![
            "Alice".to_string(),
            "Bob".to_string(),
            "Charlie".to_string(),
        ],
    ));
    history.add(HistoryEntry::new(
        "Текст українською мовою з UTF-8 символами: їжак, ґанок".to_string(),
        8.5,
        "uk".to_string(),
    ));

    // Serialize and write
    let json = serde_json::to_string_pretty(&history).expect("serialize");
    fs::write(&path, &json).expect("write");

    // Load and deserialize
    let content = fs::read_to_string(&path).expect("read");
    let loaded: History = serde_json::from_str(&content).expect("deserialize");

    // Verify entry count and ordering (newest first)
    assert_eq!(loaded.entries.len(), 3);
    assert!(loaded.entries[0].text.contains("UTF-8")); // last added = first in list
    assert!(loaded.entries[1].text.contains("Conference"));
    assert_eq!(loaded.entries[2].text, "Simple dictation");

    // Verify conference entry fields
    let conf = &loaded.entries[1];
    assert_eq!(conf.duration_secs, 120.0);
    assert_eq!(conf.language, "en");
    assert_eq!(
        conf.recording_path,
        Some("/tmp/conference_2026-01-30.wav".to_string())
    );
    assert_eq!(conf.speakers, vec!["Alice", "Bob", "Charlie"]);

    // Verify UTF-8 preservation
    assert!(loaded.entries[0].text.contains("їжак"));
    assert!(loaded.entries[0].text.contains("ґанок"));

    // Verify IDs are preserved (non-empty UUIDs)
    for entry in &loaded.entries {
        assert!(!entry.id.is_empty());
        assert!(entry.id.len() >= 32); // UUID format
    }

    let _ = fs::remove_file(&path);
    let _ = fs::remove_dir(&dir);
}

/// Search works after persistence round-trip.
#[test]
fn history_search_after_roundtrip() {
    let mut history = History::default();
    history.add(HistoryEntry::new(
        "The quick brown fox jumps".to_string(),
        3.0,
        "en".to_string(),
    ));
    history.add(HistoryEntry::new(
        "Lazy dog sleeps in the sun".to_string(),
        4.0,
        "en".to_string(),
    ));
    history.add(HistoryEntry::new(
        "FOX hunting is controversial".to_string(),
        2.0,
        "en".to_string(),
    ));

    // Serialize and deserialize
    let json = serde_json::to_string(&history).expect("serialize");
    let loaded: History = serde_json::from_str(&json).expect("deserialize");

    // Case-insensitive search
    let results = loaded.search("fox");
    assert_eq!(results.len(), 2);

    let results = loaded.search("dog");
    assert_eq!(results.len(), 1);
    assert!(results[0].text.contains("Lazy dog"));

    let results = loaded.search("nonexistent");
    assert!(results.is_empty());
}

/// Cleanup removes old entries, trim limits count.
#[test]
fn history_cleanup_and_trim_after_roundtrip() {
    let mut history = History::default();

    // Entries at different ages
    history.add(entry_at("ancient", Utc::now() - Duration::days(365)));
    history.add(entry_at("old", Utc::now() - Duration::days(100)));
    history.add(entry_at("medium", Utc::now() - Duration::days(30)));
    history.add(entry_at("recent", Utc::now() - Duration::days(1)));
    history.add(entry_at("today", Utc::now()));

    // Serialize/deserialize cycle
    let json = serde_json::to_string(&history).expect("serialize");
    let mut loaded: History = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(loaded.entries.len(), 5);

    // Cleanup entries older than 90 days (should remove "ancient" and "old")
    let removed = HistoryRepository::cleanup_old(&mut loaded, 90);
    assert_eq!(removed, 2);
    assert_eq!(loaded.entries.len(), 3);

    // Trim to 2 entries (keeps newest)
    let trimmed = HistoryRepository::trim_to_limit(&mut loaded, 2);
    assert_eq!(trimmed, 1);
    assert_eq!(loaded.entries.len(), 2);
    assert_eq!(loaded.entries[0].text, "today");
    assert_eq!(loaded.entries[1].text, "recent");
}

/// Remove by ID works after round-trip.
#[test]
fn history_remove_by_id_after_roundtrip() {
    let mut history = History::default();
    let entry1 = HistoryEntry::new("keep me".to_string(), 5.0, "uk".to_string());
    let entry2 = HistoryEntry::new("delete me".to_string(), 5.0, "uk".to_string());
    let delete_id = entry2.id.clone();

    history.add(entry1);
    history.add(entry2);

    let json = serde_json::to_string(&history).expect("serialize");
    let mut loaded: History = serde_json::from_str(&json).expect("deserialize");

    loaded.remove(&delete_id);
    assert_eq!(loaded.entries.len(), 1);
    assert_eq!(loaded.entries[0].text, "keep me");
}

/// Date range filtering works with specific timestamps.
#[test]
fn history_filter_by_date_range() {
    let mut history = History::default();

    let jan = Utc.with_ymd_and_hms(2025, 1, 15, 12, 0, 0).unwrap();
    let mar = Utc.with_ymd_and_hms(2025, 3, 15, 12, 0, 0).unwrap();
    let jun = Utc.with_ymd_and_hms(2025, 6, 15, 12, 0, 0).unwrap();
    let sep = Utc.with_ymd_and_hms(2025, 9, 15, 12, 0, 0).unwrap();

    history.add(entry_at("January", jan));
    history.add(entry_at("March", mar));
    history.add(entry_at("June", jun));
    history.add(entry_at("September", sep));

    // Filter Q1-Q2 (Feb to Jul)
    let from = Utc.with_ymd_and_hms(2025, 2, 1, 0, 0, 0).unwrap();
    let to = Utc.with_ymd_and_hms(2025, 7, 1, 0, 0, 0).unwrap();
    let results = history.filter_by_date_range(Some(from), Some(to));
    assert_eq!(results.len(), 2);

    let texts: Vec<&str> = results.iter().map(|e| e.text.as_str()).collect();
    assert!(texts.contains(&"March"));
    assert!(texts.contains(&"June"));
}

/// Export to text produces readable output from persisted history.
#[test]
fn history_export_to_text_after_roundtrip() {
    let dir = std::env::temp_dir().join("s2t_integ_history_export");
    let _ = fs::create_dir_all(&dir);
    let json_path = dir.join("history.json");
    let export_path = dir.join("export.txt");

    // Build and persist
    let mut history = History::default();
    history.add(HistoryEntry::new(
        "First transcription".to_string(),
        10.0,
        "uk".to_string(),
    ));
    history.add(HistoryEntry::new(
        "Second transcription".to_string(),
        20.0,
        "en".to_string(),
    ));

    let json = serde_json::to_string_pretty(&history).expect("serialize");
    fs::write(&json_path, &json).expect("write json");

    // Load from file
    let content = fs::read_to_string(&json_path).expect("read json");
    let loaded: History = serde_json::from_str(&content).expect("deserialize");

    // Export to text
    let entries: Vec<&HistoryEntry> = loaded.entries.iter().collect();
    export_to_text(&entries, &PathBuf::from(&export_path)).expect("export");

    // Verify export content
    let exported = fs::read_to_string(&export_path).expect("read export");
    assert!(exported.contains("Історія диктовок"));
    assert!(exported.contains("First transcription"));
    assert!(exported.contains("Second transcription"));
    assert!(exported.contains("---"));
    assert!(exported.contains("Мова: uk"));
    assert!(exported.contains("Мова: en"));

    let _ = fs::remove_file(&json_path);
    let _ = fs::remove_file(&export_path);
    let _ = fs::remove_dir(&dir);
}

/// Empty history round-trips correctly.
#[test]
fn history_empty_roundtrip() {
    let history = History::default();
    let json = serde_json::to_string(&history).expect("serialize empty");
    let loaded: History = serde_json::from_str(&json).expect("deserialize empty");
    assert!(loaded.entries.is_empty());
}

/// Large history (1000 entries) round-trips without data loss.
#[test]
fn history_large_roundtrip() {
    let mut history = History::default();
    for i in 0..1000 {
        history.add(HistoryEntry::new(
            format!("Entry number {}", i),
            (i as f32) * 0.5,
            if i % 2 == 0 { "uk" } else { "en" }.to_string(),
        ));
    }

    let json = serde_json::to_string(&history).expect("serialize large");
    let loaded: History = serde_json::from_str(&json).expect("deserialize large");

    assert_eq!(loaded.entries.len(), 1000);
    // Newest first: entry 999 is at index 0
    assert_eq!(loaded.entries[0].text, "Entry number 999");
    assert_eq!(loaded.entries[999].text, "Entry number 0");
}
