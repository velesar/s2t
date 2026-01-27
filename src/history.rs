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
}

impl HistoryEntry {
    pub fn new(text: String, duration_secs: f32, language: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            text,
            timestamp: Utc::now(),
            duration_secs,
            language,
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

    /// Search entries by text (case-insensitive)
    pub fn search(&self, query: &str) -> Vec<&HistoryEntry> {
        let query_lower = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| e.text.to_lowercase().contains(&query_lower))
            .collect()
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
    pub fn filter_by_date_range(&self, start_date: Option<DateTime<Utc>>, end_date: Option<DateTime<Utc>>) -> Vec<&HistoryEntry> {
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

    /// Export filtered entries to text file
    pub fn export_to_text(&self, entries: &[&HistoryEntry], path: &PathBuf) -> Result<()> {
        use std::io::Write;
        
        let mut file = fs::File::create(path)
            .with_context(|| format!("Не вдалося створити файл: {}", path.display()))?;

        writeln!(file, "# Історія диктовок")
            .context("Не вдалося записати заголовок")?;
        writeln!(file, "# Експортовано: {}", Utc::now().format("%Y-%m-%d %H:%M:%S"))
            .context("Не вдалося записати дату експорту")?;
        writeln!(file, "")
            .context("Не вдалося записати порожній рядок")?;

        for entry in entries {
            let local_time = entry.timestamp.with_timezone(&chrono::Local);
            writeln!(file, "---")
                .context("Не вдалося записати роздільник")?;
            writeln!(file, "Дата: {}", local_time.format("%Y-%m-%d %H:%M:%S"))
                .context("Не вдалося записати дату")?;
            writeln!(file, "Тривалість: {}", entry.formatted_duration())
                .context("Не вдалося записати тривалість")?;
            writeln!(file, "Мова: {}", entry.language)
                .context("Не вдалося записати мову")?;
            writeln!(file, "")
                .context("Не вдалося записати порожній рядок")?;
            writeln!(file, "{}", entry.text)
                .context("Не вдалося записати текст")?;
            writeln!(file, "")
                .context("Не вдалося записати порожній рядок")?;
        }

        Ok(())
    }
}

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

    let content = serde_json::to_string_pretty(history).context("Не вдалося серіалізувати історію")?;

    fs::write(&path, content)
        .with_context(|| format!("Не вдалося записати історію: {}", path.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        history.add(HistoryEntry::new("First".to_string(), 5.0, "uk".to_string()));
        history.add(HistoryEntry::new("Second".to_string(), 5.0, "uk".to_string()));

        assert_eq!(history.entries[0].text, "Second");
        assert_eq!(history.entries[1].text, "First");
    }

    #[test]
    fn test_history_search() {
        let mut history = History::default();
        history.add(HistoryEntry::new("Hello world".to_string(), 5.0, "uk".to_string()));
        history.add(HistoryEntry::new("Goodbye world".to_string(), 5.0, "uk".to_string()));
        history.add(HistoryEntry::new("Something else".to_string(), 5.0, "uk".to_string()));

        let results = history.search("world");
        assert_eq!(results.len(), 2);

        let results = history.search("WORLD"); // case-insensitive
        assert_eq!(results.len(), 2);

        let results = history.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_history_trim_to_limit() {
        let mut history = History::default();
        for i in 0..10 {
            history.add(HistoryEntry::new(format!("Entry {}", i), 5.0, "uk".to_string()));
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
}
