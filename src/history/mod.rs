mod entry;
mod export;
mod persistence;

pub use entry::HistoryEntry;
pub use export::export_to_text;
pub use persistence::{load_history, save_history};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::traits::HistoryRepository;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct History {
    pub entries: Vec<HistoryEntry>,
}

impl History {
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
}

impl HistoryRepository for History {
    type Entry = HistoryEntry;

    fn add(&mut self, entry: HistoryEntry) {
        self.entries.insert(0, entry);
    }

    fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    fn search(&self, query: &str) -> Vec<&HistoryEntry> {
        let query_lower = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| e.text.to_lowercase().contains(&query_lower))
            .collect()
    }

    fn cleanup_old(&mut self, max_age_days: u32) -> usize {
        let before = self.entries.len();
        self.cleanup_old_entries(max_age_days as i64);
        before - self.entries.len()
    }

    fn trim_to_limit(&mut self, max_entries: usize) -> usize {
        if self.entries.len() <= max_entries {
            return 0;
        }
        let removed = self.entries.len() - max_entries;
        self.entries.truncate(max_entries);
        removed
    }

    fn save(&self) -> anyhow::Result<()> {
        save_history(self)
    }

    fn remove(&mut self, id: &str) {
        self.entries.retain(|e| e.id != id);
    }

    fn filter_by_date_range(
        &self,
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
    ) -> Vec<&HistoryEntry> {
        self.entries
            .iter()
            .filter(|e| {
                if let Some(start) = from {
                    if e.timestamp < start {
                        return false;
                    }
                }
                if let Some(end) = to {
                    if e.timestamp > end {
                        return false;
                    }
                }
                true
            })
            .collect()
    }
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
    fn test_trim_to_limit_no_op_when_under() {
        let mut history = History::default();
        history.add(HistoryEntry::new(
            "One".to_string(),
            5.0,
            "uk".to_string(),
        ));
        history.add(HistoryEntry::new(
            "Two".to_string(),
            5.0,
            "uk".to_string(),
        ));

        history.trim_to_limit(10);
        assert_eq!(history.entries.len(), 2);
    }

    #[test]
    fn test_remove_nonexistent_id_is_noop() {
        let mut history = History::default();
        history.add(HistoryEntry::new(
            "Test".to_string(),
            5.0,
            "uk".to_string(),
        ));

        history.remove("nonexistent-id");
        assert_eq!(history.entries.len(), 1);
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

    // === Trait Implementation Tests ===

    #[test]
    fn test_trait_search_finds_matching_entries() {
        use crate::domain::traits::HistoryRepository;

        let mut history = History::default();
        history.add(HistoryEntry::new(
            "apple pie".to_string(),
            5.0,
            "uk".to_string(),
        ));
        history.add(HistoryEntry::new(
            "banana split".to_string(),
            5.0,
            "uk".to_string(),
        ));
        history.add(HistoryEntry::new(
            "Apple sauce".to_string(),
            5.0,
            "uk".to_string(),
        ));

        let results = HistoryRepository::search(&history, "apple");
        assert_eq!(results.len(), 2); // case-insensitive
    }

    #[test]
    fn test_trait_search_returns_empty_on_no_match() {
        use crate::domain::traits::HistoryRepository;

        let mut history = History::default();
        history.add(HistoryEntry::new(
            "hello world".to_string(),
            5.0,
            "uk".to_string(),
        ));

        let results = HistoryRepository::search(&history, "xyz");
        assert!(results.is_empty());
    }

    #[test]
    fn test_trait_cleanup_old_returns_removed_count() {
        use crate::domain::traits::HistoryRepository;

        let mut history = History::default();
        history.add(entry_at("old", Utc::now() - Duration::days(100)));
        history.add(entry_at("recent", Utc::now() - Duration::days(1)));

        let removed = HistoryRepository::cleanup_old(&mut history, 30);
        assert_eq!(removed, 1);
        assert_eq!(history.entries.len(), 1);
    }

    #[test]
    fn test_trait_trim_to_limit_returns_removed_count() {
        use crate::domain::traits::HistoryRepository;

        let mut history = History::default();
        for i in 0..10 {
            history.add(HistoryEntry::new(
                format!("entry {}", i),
                5.0,
                "uk".to_string(),
            ));
        }

        let removed = HistoryRepository::trim_to_limit(&mut history, 5);
        assert_eq!(removed, 5);
        assert_eq!(history.entries.len(), 5);
    }

    #[test]
    fn test_trait_trim_to_limit_returns_zero_when_under() {
        use crate::domain::traits::HistoryRepository;

        let mut history = History::default();
        history.add(HistoryEntry::new(
            "one".to_string(),
            5.0,
            "uk".to_string(),
        ));

        let removed = HistoryRepository::trim_to_limit(&mut history, 10);
        assert_eq!(removed, 0);
    }

    #[test]
    fn test_trait_entries_matches_field() {
        use crate::domain::traits::HistoryRepository;

        let mut history = History::default();
        history.add(HistoryEntry::new(
            "test".to_string(),
            5.0,
            "uk".to_string(),
        ));

        let trait_entries = HistoryRepository::entries(&history);
        assert_eq!(trait_entries.len(), history.entries.len());
        assert_eq!(trait_entries[0].text, history.entries[0].text);
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
