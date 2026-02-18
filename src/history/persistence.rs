use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

use super::History;

pub fn history_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("voice-dictation")
        .join("history.json")
}

pub fn load_history() -> Result<History> {
    let path = history_path();

    if !path.exists() {
        return Ok(History::default());
    }

    let content =
        fs::read_to_string(&path).with_context(|| format!("Не вдалося прочитати історію: {}", path.display()))?;

    serde_json::from_str(&content).with_context(|| "Не вдалося розпарсити історію")
}

pub fn save_history(history: &History) -> Result<()> {
    let path = history_path();
    let dir = path.parent().unwrap();

    fs::create_dir_all(dir).with_context(|| format!("Не вдалося створити директорію: {}", dir.display()))?;

    let content = serde_json::to_string_pretty(history).context("Не вдалося серіалізувати історію")?;

    fs::write(&path, &content).with_context(|| format!("Не вдалося записати історію: {}", path.display()))?;

    crate::app::config::set_owner_only_permissions(&path)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::HistoryEntry;

    #[test]
    fn test_history_path_contains_voice_dictation() {
        let path = history_path();
        assert!(path.to_string_lossy().contains("voice-dictation"));
        assert!(path.to_string_lossy().ends_with("history.json"));
    }

    #[test]
    fn test_history_serialization() {
        let mut history = History::default();
        history
            .entries
            .insert(0, HistoryEntry::new("Test".to_string(), 5.0, "uk".to_string()));

        let json = serde_json::to_string(&history).unwrap();
        let parsed: History = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].text, "Test");
    }

    /// Integration test: full persistence round-trip (serialize → file → deserialize).
    #[test]
    fn test_history_persistence_round_trip() {
        let dir = std::env::temp_dir().join("s2t_test_persistence");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("test_history.json");

        // Create history with varied entries
        let mut history = History::default();
        history.entries.insert(
            0,
            HistoryEntry::new("Simple dictation".to_string(), 5.0, "uk".to_string()),
        );
        history.entries.insert(
            0,
            HistoryEntry::new_with_recording(
                "Conference notes".to_string(),
                120.0,
                "en".to_string(),
                Some("/tmp/rec.wav".to_string()),
                vec!["Alice".to_string(), "Bob".to_string()],
            ),
        );

        // Save (mirrors save_history implementation)
        let content = serde_json::to_string_pretty(&history).unwrap();
        fs::write(&path, &content).unwrap();

        // Load (mirrors load_history implementation)
        let loaded_content = fs::read_to_string(&path).unwrap();
        let loaded: History = serde_json::from_str(&loaded_content).unwrap();

        // Verify round-trip preserves all data
        assert_eq!(loaded.entries.len(), 2);
        assert_eq!(loaded.entries[0].text, "Conference notes");
        assert_eq!(loaded.entries[0].language, "en");
        assert_eq!(loaded.entries[0].duration_secs, 120.0);
        assert_eq!(loaded.entries[0].recording_path, Some("/tmp/rec.wav".to_string()));
        assert_eq!(loaded.entries[0].speakers, vec!["Alice", "Bob"]);
        assert_eq!(loaded.entries[1].text, "Simple dictation");
        assert_eq!(loaded.entries[1].recording_path, None);
        assert!(loaded.entries[1].speakers.is_empty());

        // Cleanup
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&dir);
    }
}
