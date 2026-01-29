use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

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
    fn test_formatted_timestamp_contains_date() {
        let entry = HistoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            text: "test".to_string(),
            timestamp: Utc.with_ymd_and_hms(2025, 3, 15, 10, 30, 0).unwrap(),
            duration_secs: 5.0,
            language: "uk".to_string(),
            recording_path: None,
            speakers: Vec::new(),
        };
        let formatted = entry.formatted_timestamp();
        // The exact output depends on local timezone, but should contain a date pattern
        assert!(formatted.contains("2025"));
        assert!(formatted.contains("03") || formatted.contains("3"));
    }
}
