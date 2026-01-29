use anyhow::{Context, Result};
use chrono::Utc;
use std::fs;
use std::path::PathBuf;

use super::HistoryEntry;

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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

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
}
