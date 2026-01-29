use crate::config::models_dir;
use anyhow::{Context, Result};
use futures_util::StreamExt;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub filename: String,
    pub display_name: String,
    pub size_bytes: u64,
    pub description: String,
}

const HUGGINGFACE_BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/";

pub fn get_available_models() -> Vec<ModelInfo> {
    vec![
        // Quantized models (recommended for faster performance)
        ModelInfo {
            filename: "ggml-tiny-q5_1.bin".to_string(),
            display_name: "Tiny Q5".to_string(),
            size_bytes: 32_000_000,
            description: "Найшвидша квантована, ~2x швидше".to_string(),
        },
        ModelInfo {
            filename: "ggml-base-q5_1.bin".to_string(),
            display_name: "Base Q5 (Рекомендовано)".to_string(),
            size_bytes: 60_000_000,
            description: "Швидка + якісна, оптимальний баланс".to_string(),
        },
        ModelInfo {
            filename: "ggml-base-q8_0.bin".to_string(),
            display_name: "Base Q8".to_string(),
            size_bytes: 83_000_000,
            description: "Квантована, найкраща якість серед Q".to_string(),
        },
        ModelInfo {
            filename: "ggml-small-q5_1.bin".to_string(),
            display_name: "Small Q5".to_string(),
            size_bytes: 190_000_000,
            description: "Хороша якість, швидша за звичайну".to_string(),
        },
        ModelInfo {
            filename: "ggml-small-q8_0.bin".to_string(),
            display_name: "Small Q8".to_string(),
            size_bytes: 264_000_000,
            description: "Висока якість серед квантованих".to_string(),
        },
        ModelInfo {
            filename: "ggml-medium-q5_0.bin".to_string(),
            display_name: "Medium Q5".to_string(),
            size_bytes: 539_000_000,
            description: "Велика квантована, хороша якість".to_string(),
        },
        // Full precision models
        ModelInfo {
            filename: "ggml-tiny.bin".to_string(),
            display_name: "Tiny".to_string(),
            size_bytes: 75_000_000,
            description: "Найшвидша, найменша точність".to_string(),
        },
        ModelInfo {
            filename: "ggml-base.bin".to_string(),
            display_name: "Base".to_string(),
            size_bytes: 148_000_000,
            description: "Баланс швидкості та точності".to_string(),
        },
        ModelInfo {
            filename: "ggml-small.bin".to_string(),
            display_name: "Small".to_string(),
            size_bytes: 488_000_000,
            description: "Хороша точність".to_string(),
        },
        ModelInfo {
            filename: "ggml-medium.bin".to_string(),
            display_name: "Medium".to_string(),
            size_bytes: 1_500_000_000,
            description: "Висока точність".to_string(),
        },
        ModelInfo {
            filename: "ggml-large-v3.bin".to_string(),
            display_name: "Large v3".to_string(),
            size_bytes: 3_100_000_000,
            description: "Найвища точність".to_string(),
        },
    ]
}

pub fn list_downloaded_models() -> Vec<ModelInfo> {
    let dir = models_dir();
    let available = get_available_models();

    available
        .into_iter()
        .filter(|model| {
            let path = dir.join(&model.filename);
            path.exists()
        })
        .collect()
}

pub fn is_model_downloaded(filename: &str) -> bool {
    let path = models_dir().join(filename);
    path.exists()
}

pub fn get_model_path(filename: &str) -> PathBuf {
    models_dir().join(filename)
}

pub fn delete_model(filename: &str) -> Result<()> {
    let path = models_dir().join(filename);

    if !path.exists() {
        return Err(anyhow::anyhow!("Модель не знайдено: {}", filename));
    }

    fs::remove_file(&path)
        .with_context(|| format!("Не вдалося видалити модель: {}", path.display()))?;

    Ok(())
}

pub async fn download_model<F>(filename: &str, progress_callback: F) -> Result<()>
where
    F: Fn(u64, u64) + Send + 'static,
{
    let url = format!("{}{}", HUGGINGFACE_BASE_URL, filename);
    let dir = models_dir();

    fs::create_dir_all(&dir)
        .with_context(|| format!("Не вдалося створити директорію: {}", dir.display()))?;

    let temp_path = dir.join(format!("{}.downloading", filename));
    let final_path = dir.join(filename);

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("Не вдалося підключитися: {}", url))?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Помилка завантаження: HTTP {}",
            response.status()
        ));
    }

    let total_size = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;

    let mut file = fs::File::create(&temp_path)
        .with_context(|| format!("Не вдалося створити файл: {}", temp_path.display()))?;

    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Помилка при завантаженні")?;
        std::io::Write::write_all(&mut file, &chunk).context("Не вдалося записати дані")?;

        downloaded += chunk.len() as u64;
        progress_callback(downloaded, total_size);
    }

    fs::rename(&temp_path, &final_path).with_context(|| {
        format!(
            "Не вдалося перейменувати {} -> {}",
            temp_path.display(),
            final_path.display()
        )
    })?;

    Ok(())
}

pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{} KB", bytes / KB)
    } else {
        format!("{} B", bytes)
    }
}

// Sortformer model management
pub fn get_sortformer_model_info() -> ModelInfo {
    ModelInfo {
        filename: "diar_streaming_sortformer_4spk-v2.1.onnx".to_string(),
        display_name: "Sortformer v2.1 (4 speakers)".to_string(),
        size_bytes: 492_000_000, // ~492MB
        description: "NVIDIA Streaming Sortformer для speaker diarization (до 4 мовців)"
            .to_string(),
    }
}

pub fn get_sortformer_model_path() -> PathBuf {
    crate::config::sortformer_models_dir().join("diar_streaming_sortformer_4spk-v2.1.onnx")
}

pub fn is_sortformer_model_downloaded() -> bool {
    get_sortformer_model_path().exists()
}

pub fn delete_sortformer_model() -> Result<()> {
    let path = get_sortformer_model_path();
    if path.exists() {
        fs::remove_file(&path)
            .with_context(|| format!("Не вдалося видалити модель: {}", path.display()))?;
    }
    Ok(())
}

pub async fn download_sortformer_model<F>(progress_callback: F) -> Result<()>
where
    F: Fn(u64, u64) + Send + 'static,
{
    // Download from HuggingFace (altunenes/parakeet-rs contains pre-converted ONNX models)
    let url =
        "https://huggingface.co/altunenes/parakeet-rs/resolve/main/diar_streaming_sortformer_4spk-v2.1.onnx";
    let dir = crate::config::sortformer_models_dir();
    let filename = "diar_streaming_sortformer_4spk-v2.1.onnx";

    fs::create_dir_all(&dir)
        .with_context(|| format!("Не вдалося створити директорію: {}", dir.display()))?;

    let temp_path = dir.join(format!("{}.downloading", filename));
    let final_path = dir.join(filename);

    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("Не вдалося підключитися: {}", url))?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Помилка завантаження: HTTP {}",
            response.status()
        ));
    }

    let total_size = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;

    let mut file = fs::File::create(&temp_path)
        .with_context(|| format!("Не вдалося створити файл: {}", temp_path.display()))?;

    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Помилка при завантаженні")?;
        std::io::Write::write_all(&mut file, &chunk).context("Не вдалося записати дані")?;

        downloaded += chunk.len() as u64;
        progress_callback(downloaded, total_size);
    }

    fs::rename(&temp_path, &final_path).with_context(|| {
        format!(
            "Не вдалося перейменувати {} -> {}",
            temp_path.display(),
            final_path.display()
        )
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size_bytes() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1023), "1023 B");
    }

    #[test]
    fn test_format_size_kilobytes() {
        assert_eq!(format_size(1024), "1 KB");
        assert_eq!(format_size(2048), "2 KB");
        assert_eq!(format_size(1024 * 1023), "1023 KB");
    }

    #[test]
    fn test_format_size_megabytes() {
        assert_eq!(format_size(1024 * 1024), "1 MB");
        assert_eq!(format_size(148_000_000), "141 MB");
        assert_eq!(format_size(500 * 1024 * 1024), "500 MB");
    }

    #[test]
    fn test_format_size_gigabytes() {
        assert_eq!(format_size(1024 * 1024 * 1024), "1.0 GB");
        assert_eq!(format_size(3_100_000_000), "2.9 GB");
    }

    #[test]
    fn test_get_available_models_count() {
        let models = get_available_models();
        assert_eq!(models.len(), 11); // 6 quantized + 5 full precision
    }

    #[test]
    fn test_get_available_models_contains_base() {
        let models = get_available_models();
        let base = models.iter().find(|m| m.filename == "ggml-base.bin");
        assert!(base.is_some());
        let base = base.unwrap();
        assert_eq!(base.display_name, "Base");
    }

    #[test]
    fn test_get_available_models_has_quantized_and_full() {
        let models = get_available_models();
        let quantized_count = models
            .iter()
            .filter(|m| m.filename.contains("q5_") || m.filename.contains("q8_"))
            .count();
        let full_count = models
            .iter()
            .filter(|m| !m.filename.contains("q5_") && !m.filename.contains("q8_"))
            .count();

        assert!(
            quantized_count >= 5,
            "Should have at least 5 quantized models"
        );
        assert!(
            full_count >= 5,
            "Should have at least 5 full precision models"
        );
    }

    #[test]
    fn test_model_info_has_all_fields() {
        let models = get_available_models();
        for model in models {
            assert!(!model.filename.is_empty());
            assert!(!model.display_name.is_empty());
            assert!(!model.description.is_empty());
            assert!(model.size_bytes > 0);
            assert!(model.filename.ends_with(".bin"));
        }
    }

    #[test]
    fn test_get_model_path_constructs_correctly() {
        let path = get_model_path("ggml-base.bin");
        assert!(path.to_string_lossy().contains("whisper"));
        assert!(path.to_string_lossy().ends_with("ggml-base.bin"));
    }
}
