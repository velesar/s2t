use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub text: String,
    pub timestamp: DateTime<Utc>,
    pub duration_secs: f32,
    pub language: String,
    #[serde(default)]
    pub recording_path: Option<String>,
    #[serde(default)]
    pub speakers: Vec<String>,
}

impl HistoryEntry {
    pub fn new(text: String, duration_secs: f32, language: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            text,
            timestamp: Utc::now(),
            duration_secs,
            language,
            recording_path: None,
            speakers: Vec::new(),
        }
    }

    pub fn new_with_recording(
        text: String,
        duration_secs: f32,
        language: String,
        recording_path: Option<String>,
        speakers: Vec<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            text,
            timestamp: Utc::now(),
            duration_secs,
            language,
            recording_path,
            speakers,
        }
    }

    /// Returns a preview of the text (first 80 chars, single line)
    pub fn preview(&self) -> String {
        let text = self.text.replace('\n', " ");
        let chars: Vec<char> = text.chars().collect();
        if chars.len() > 80 {
            format!("{}...", chars[..80].iter().collect::<String>())
        } else {
            text
        }
    }

    /// Returns formatted timestamp in local time (YYYY-MM-DD HH:MM)
    pub fn formatted_timestamp(&self) -> String {
        let local = self.timestamp.with_timezone(&chrono::Local);
        local.format("%Y-%m-%d %H:%M").to_string()
    }

    /// Returns formatted duration (MM:SS)
    pub fn formatted_duration(&self) -> String {
        let mins = (self.duration_secs / 60.0).floor() as u32;
        let secs = (self.duration_secs % 60.0).floor() as u32;
        format!("{:02}:{:02}", mins, secs)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct History {
    pub entries: Vec<HistoryEntry>,
}

impl History {
    pub fn add(&mut self, entry: HistoryEntry) {
        self.entries.insert(0, entry);
    }

    pub fn remove(&mut self, id: &str) {
        self.entries.retain(|e| e.id != id);
    }

    /// Trim history to max_entries, keeping newest
    pub fn trim_to_limit(&mut self, max_entries: usize) {
        if self.entries.len() > max_entries {
            self.entries.truncate(max_entries);
        }
    }

    /// Remove entries older than max_age_days
    pub fn cleanup_old_entries(&mut self, max_age_days: i64) {
        let cutoff = Utc::now() - Duration::days(max_age_days);
        self.entries.retain(|e| e.timestamp > cutoff);
    }

    /// Filter entries by date range (inclusive)
    pub fn filter_by_date_range(
        &self,
        start_date: Option<DateTime<Utc>>,
        end_date: Option<DateTime<Utc>>,
    ) -> Vec<&HistoryEntry> {
        self.entries
            .iter()
            .filter(|e| {
                if let Some(start) = start_date {
                    if e.timestamp < start {
                        return false;
                    }
                }
                if let Some(end) = end_date {
                    if e.timestamp > end {
                        return false;
                    }
                }
                true
            })
            .collect()
    }
}

// === Export ===

/// Export history entries to a text file.
///
/// This is a standalone function (not a method on History) because
/// file I/O is an infrastructure concern, not domain logic.
pub fn export_to_text(entries: &[&HistoryEntry], path: &PathBuf) -> Result<()> {
    use std::io::Write;

    let mut file = fs::File::create(path)
        .with_context(|| format!("Не вдалося створити файл: {}", path.display()))?;

    writeln!(file, "# Історія диктовок").context("Не вдалося записати заголовок")?;
    writeln!(
        file,
        "# Експортовано: {}",
        Utc::now().format("%Y-%m-%d %H:%M:%S")
    )
    .context("Не вдалося записати дату експорту")?;
    writeln!(file).context("Не вдалося записати порожній рядок")?;

    for entry in entries {
        let local_time = entry.timestamp.with_timezone(&chrono::Local);
        writeln!(file, "---").context("Не вдалося записати роздільник")?;
        writeln!(file, "Дата: {}", local_time.format("%Y-%m-%d %H:%M:%S"))
            .context("Не вдалося записати дату")?;
        writeln!(file, "Тривалість: {}", entry.formatted_duration())
            .context("Не вдалося записати тривалість")?;
        writeln!(file, "Мова: {}", entry.language).context("Не вдалося записати мову")?;
        writeln!(file).context("Не вдалося записати порожній рядок")?;
        writeln!(file, "{}", entry.text).context("Не вдалося записати текст")?;
        writeln!(file).context("Не вдалося записати порожній рядок")?;
    }

    Ok(())
}

// === Persistence ===

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

    let content = fs::read_to_string(&path)
        .with_context(|| format!("Не вдалося прочитати історію: {}", path.display()))?;

    serde_json::from_str(&content).with_context(|| "Не вдалося розпарсити історію")
}

pub fn save_history(history: &History) -> Result<()> {
    let path = history_path();
    let dir = path.parent().unwrap();

    fs::create_dir_all(dir)
        .with_context(|| format!("Не вдалося створити директорію: {}", dir.display()))?;

    let content =
        serde_json::to_string_pretty(history).context("Не вдалося серіалізувати історію")?;

    fs::write(&path, content)
        .with_context(|| format!("Не вдалося записати історію: {}", path.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    /// Helper to create an entry with a specific timestamp.
    fn entry_at(text: &str, timestamp: DateTime<Utc>) -> HistoryEntry {
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

    #[test]
    fn test_history_entry_new() {
        let entry = HistoryEntry::new("Test text".to_string(), 10.5, "uk".to_string());
        assert!(!entry.id.is_empty());
        assert_eq!(entry.text, "Test text");
        assert_eq!(entry.duration_secs, 10.5);
        assert_eq!(entry.language, "uk");
    }

    #[test]
    fn test_history_entry_preview() {
        let short_entry = HistoryEntry::new("Short text".to_string(), 5.0, "uk".to_string());
        assert_eq!(short_entry.preview(), "Short text");

        let long_text = "a".repeat(100);
        let long_entry = HistoryEntry::new(long_text, 5.0, "uk".to_string());
        assert!(long_entry.preview().ends_with("..."));
        assert_eq!(long_entry.preview().len(), 83); // 80 + "..."
    }

    #[test]
    fn test_history_entry_preview_multiline() {
        let entry = HistoryEntry::new("Line 1\nLine 2\nLine 3".to_string(), 5.0, "uk".to_string());
        assert!(!entry.preview().contains('\n'));
        assert_eq!(entry.preview(), "Line 1 Line 2 Line 3");
    }

    #[test]
    fn test_history_entry_formatted_duration() {
        let entry = HistoryEntry::new("Test".to_string(), 65.5, "uk".to_string());
        assert_eq!(entry.formatted_duration(), "01:05");

        let entry2 = HistoryEntry::new("Test".to_string(), 5.0, "uk".to_string());
        assert_eq!(entry2.formatted_duration(), "00:05");
    }

    #[test]
    fn test_history_add_and_remove() {
        let mut history = History::default();
        let entry = HistoryEntry::new("Test".to_string(), 5.0, "uk".to_string());
        let id = entry.id.clone();

        history.add(entry);
        assert_eq!(history.entries.len(), 1);

        history.remove(&id);
        assert!(history.entries.is_empty());
    }

    #[test]
    fn test_history_add_inserts_at_front() {
        let mut history = History::default();
        history.add(HistoryEntry::new(
            "First".to_string(),
            5.0,
            "uk".to_string(),
        ));
        history.add(HistoryEntry::new(
            "Second".to_string(),
            5.0,
            "uk".to_string(),
        ));

        assert_eq!(history.entries[0].text, "Second");
        assert_eq!(history.entries[1].text, "First");
    }

    #[test]
    fn test_history_trim_to_limit() {
        let mut history = History::default();
        for i in 0..10 {
            history.add(HistoryEntry::new(
                format!("Entry {}", i),
                5.0,
                "uk".to_string(),
            ));
        }

        history.trim_to_limit(5);
        assert_eq!(history.entries.len(), 5);
        // Newest entries should be kept
        assert_eq!(history.entries[0].text, "Entry 9");
    }

    #[test]
    fn test_history_path_contains_voice_dictation() {
        let path = history_path();
        assert!(path.to_string_lossy().contains("voice-dictation"));
        assert!(path.to_string_lossy().ends_with("history.json"));
    }

    #[test]
    fn test_history_serialization() {
        let mut history = History::default();
        history.add(HistoryEntry::new("Test".to_string(), 5.0, "uk".to_string()));

        let json = serde_json::to_string(&history).unwrap();
        let parsed: History = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].text, "Test");
    }

    #[test]
    fn test_new_with_recording() {
        let entry = HistoryEntry::new_with_recording(
            "Conference text".to_string(),
            120.0,
            "en".to_string(),
            Some("/tmp/recording.wav".to_string()),
            vec!["Speaker A".to_string(), "Speaker B".to_string()],
        );
        assert_eq!(entry.recording_path, Some("/tmp/recording.wav".to_string()));
        assert_eq!(entry.speakers.len(), 2);
        assert_eq!(entry.duration_secs, 120.0);
    }

    #[test]
    fn test_filter_by_date_range_both_bounds() {
        let mut history = History::default();
        let jan1 = Utc.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap();
        let jan15 = Utc.with_ymd_and_hms(2025, 1, 15, 12, 0, 0).unwrap();
        let feb1 = Utc.with_ymd_and_hms(2025, 2, 1, 12, 0, 0).unwrap();

        history.add(entry_at(
            "before",
            Utc.with_ymd_and_hms(2024, 12, 15, 0, 0, 0).unwrap(),
        ));
        history.add(entry_at("inside", jan15));
        history.add(entry_at(
            "after",
            Utc.with_ymd_and_hms(2025, 3, 1, 0, 0, 0).unwrap(),
        ));

        let result = history.filter_by_date_range(Some(jan1), Some(feb1));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text, "inside");
    }

    #[test]
    fn test_filter_by_date_range_start_only() {
        let mut history = History::default();
        let jan1 = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();

        history.add(entry_at(
            "old",
            Utc.with_ymd_and_hms(2024, 6, 1, 0, 0, 0).unwrap(),
        ));
        history.add(entry_at(
            "new",
            Utc.with_ymd_and_hms(2025, 6, 1, 0, 0, 0).unwrap(),
        ));

        let result = history.filter_by_date_range(Some(jan1), None);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text, "new");
    }

    #[test]
    fn test_filter_by_date_range_end_only() {
        let mut history = History::default();
        let jan1 = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();

        history.add(entry_at(
            "old",
            Utc.with_ymd_and_hms(2024, 6, 1, 0, 0, 0).unwrap(),
        ));
        history.add(entry_at(
            "new",
            Utc.with_ymd_and_hms(2025, 6, 1, 0, 0, 0).unwrap(),
        ));

        let result = history.filter_by_date_range(None, Some(jan1));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text, "old");
    }

    #[test]
    fn test_filter_by_date_range_no_bounds() {
        let mut history = History::default();
        history.add(entry_at(
            "a",
            Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(),
        ));
        history.add(entry_at(
            "b",
            Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
        ));

        let result = history.filter_by_date_range(None, None);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_cleanup_old_entries() {
        let mut history = History::default();
        // Add an entry from 100 days ago
        history.add(entry_at("old", Utc::now() - Duration::days(100)));
        // Add a recent entry
        history.add(entry_at("recent", Utc::now() - Duration::days(1)));

        history.cleanup_old_entries(30);
        assert_eq!(history.entries.len(), 1);
        assert_eq!(history.entries[0].text, "recent");
    }

    #[test]
    fn test_cleanup_old_entries_keeps_all_when_recent() {
        let mut history = History::default();
        history.add(entry_at("a", Utc::now() - Duration::days(5)));
        history.add(entry_at("b", Utc::now() - Duration::days(10)));

        history.cleanup_old_entries(30);
        assert_eq!(history.entries.len(), 2);
    }

    #[test]
    fn test_export_to_text() {
        let entry1 = entry_at(
            "First dictation",
            Utc.with_ymd_and_hms(2025, 1, 15, 10, 30, 0).unwrap(),
        );
        let entry2 = entry_at(
            "Second dictation",
            Utc.with_ymd_and_hms(2025, 1, 16, 14, 0, 0).unwrap(),
        );
        let entries: Vec<&HistoryEntry> = vec![&entry1, &entry2];

        let dir = std::env::temp_dir().join("s2t_test_export");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("test_export.txt");

        export_to_text(&entries, &path).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("Історія диктовок"));
        assert!(content.contains("First dictation"));
        assert!(content.contains("Second dictation"));
        assert!(content.contains("---"));

        // Cleanup
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn test_trim_to_limit_no_op_when_under() {
        let mut history = History::default();
        history.add(HistoryEntry::new("One".to_string(), 5.0, "uk".to_string()));
        history.add(HistoryEntry::new("Two".to_string(), 5.0, "uk".to_string()));

        history.trim_to_limit(10);
        assert_eq!(history.entries.len(), 2);
    }

    #[test]
    fn test_formatted_timestamp_contains_date() {
        let entry = entry_at(
            "test",
            Utc.with_ymd_and_hms(2025, 3, 15, 10, 30, 0).unwrap(),
        );
        let formatted = entry.formatted_timestamp();
        // The exact output depends on local timezone, but should contain a date pattern
        assert!(formatted.contains("2025"));
        assert!(formatted.contains("03") || formatted.contains("3"));
    }

    #[test]
    fn test_remove_nonexistent_id_is_noop() {
        let mut history = History::default();
        history.add(HistoryEntry::new("Test".to_string(), 5.0, "uk".to_string()));

        history.remove("nonexistent-id");
        assert_eq!(history.entries.len(), 1);
    }

    /// Integration test: full persistence round-trip (serialize → file → deserialize).
    #[test]
    fn test_history_persistence_round_trip() {
        let dir = std::env::temp_dir().join("s2t_test_persistence");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("test_history.json");

        // Create history with varied entries
        let mut history = History::default();
        history.add(HistoryEntry::new(
            "Simple dictation".to_string(),
            5.0,
            "uk".to_string(),
        ));
        history.add(HistoryEntry::new_with_recording(
            "Conference notes".to_string(),
            120.0,
            "en".to_string(),
            Some("/tmp/rec.wav".to_string()),
            vec!["Alice".to_string(), "Bob".to_string()],
        ));

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
        assert_eq!(
            loaded.entries[0].recording_path,
            Some("/tmp/rec.wav".to_string())
        );
        assert_eq!(loaded.entries[0].speakers, vec!["Alice", "Bob"]);
        assert_eq!(loaded.entries[1].text, "Simple dictation");
        assert_eq!(loaded.entries[1].recording_path, None);
        assert!(loaded.entries[1].speakers.is_empty());

        // Cleanup
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&dir);
    }

    /// Integration test: history operations chain (add → filter → cleanup → trim).
    #[test]
    fn test_history_operations_chain() {
        let mut history = History::default();

        // Add entries spanning different dates
        history.add(entry_at("old", Utc::now() - Duration::days(100)));
        history.add(entry_at("medium", Utc::now() - Duration::days(20)));
        history.add(entry_at("recent1", Utc::now() - Duration::days(1)));
        history.add(entry_at("recent2", Utc::now()));
        assert_eq!(history.entries.len(), 4);

        // Filter shows all within range
        let recent = history.filter_by_date_range(Some(Utc::now() - Duration::days(30)), None);
        assert_eq!(recent.len(), 3); // medium, recent1, recent2

        // Cleanup removes old entries
        history.cleanup_old_entries(30);
        assert_eq!(history.entries.len(), 3);

        // Trim to limit
        history.trim_to_limit(2);
        assert_eq!(history.entries.len(), 2);
        // Newest entries kept (they're at front because add() inserts at 0)
        assert_eq!(history.entries[0].text, "recent2");
        assert_eq!(history.entries[1].text, "recent1");
    }
}
