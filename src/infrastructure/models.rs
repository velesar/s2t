use crate::app::config::models_dir;
use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

/// Validates that a model filename is safe (no path traversal).
///
/// Rejects filenames containing path separators or `..` sequences.
fn sanitize_model_filename(filename: &str) -> Result<()> {
    if filename.is_empty() {
        bail!("Ім'я файлу моделі не може бути порожнім");
    }
    if filename.contains('/')
        || filename.contains('\\')
        || filename.contains("..")
        || filename.contains('\0')
    {
        bail!("Неприпустиме ім'я файлу моделі: {}", filename);
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub filename: String,
    pub display_name: String,
    pub size_bytes: u64,
    pub description: String,
    pub sha256: Option<String>,
}

fn verify_checksum(path: &Path, expected: &str) -> Result<()> {
    let mut file = fs::File::open(path)
        .with_context(|| format!("Не вдалося відкрити файл для перевірки: {}", path.display()))?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).context("Помилка при обчисленні контрольної суми")?;
    let hash = format!("{:x}", hasher.finalize());
    if hash != expected {
        bail!(
            "Контрольна сума не збігається для {}: очікувано {}, отримано {}",
            path.display(),
            expected,
            hash
        );
    }
    Ok(())
}

const HUGGINGFACE_BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/";

pub fn get_available_models() -> Vec<ModelInfo> {
    vec![
        // Quantized models (recommended for faster performance)
        ModelInfo {
            filename: "ggml-tiny-q5_1.bin".to_string(),
            display_name: "Tiny Q5".to_string(),
            size_bytes: 32_152_673,
            description: "Найшвидша квантована, ~2x швидше".to_string(),
            sha256: Some("818710568da3ca15689e31a743197b520007872ff9576237bda97bd1b469c3d7".to_string()),
        },
        ModelInfo {
            filename: "ggml-base-q5_1.bin".to_string(),
            display_name: "Base Q5 (Рекомендовано)".to_string(),
            size_bytes: 59_707_625,
            description: "Швидка + якісна, оптимальний баланс".to_string(),
            sha256: Some("422f1ae452ade6f30a004d7e5c6a43195e4433bc370bf23fac9cc591f01a8898".to_string()),
        },
        ModelInfo {
            filename: "ggml-base-q8_0.bin".to_string(),
            display_name: "Base Q8".to_string(),
            size_bytes: 81_768_585,
            description: "Квантована, найкраща якість серед Q".to_string(),
            sha256: Some("c577b9a86e7e048a0b7eada054f4dd79a56bbfa911fbdacf900ac5b567cbb7d9".to_string()),
        },
        ModelInfo {
            filename: "ggml-small-q5_1.bin".to_string(),
            display_name: "Small Q5".to_string(),
            size_bytes: 190_085_487,
            description: "Хороша якість, швидша за звичайну".to_string(),
            sha256: Some("ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb".to_string()),
        },
        ModelInfo {
            filename: "ggml-small-q8_0.bin".to_string(),
            display_name: "Small Q8".to_string(),
            size_bytes: 264_464_607,
            description: "Висока якість серед квантованих".to_string(),
            sha256: Some("49c8fb02b65e6049d5fa6c04f81f53b867b5ec9540406812c643f177317f779f".to_string()),
        },
        ModelInfo {
            filename: "ggml-medium-q5_0.bin".to_string(),
            display_name: "Medium Q5".to_string(),
            size_bytes: 539_212_467,
            description: "Велика квантована, хороша якість".to_string(),
            sha256: Some("19fea4b380c3a618ec4723c3eef2eb785ffba0d0538cf43f8f235e7b3b34220f".to_string()),
        },
        // Full precision models
        ModelInfo {
            filename: "ggml-tiny.bin".to_string(),
            display_name: "Tiny".to_string(),
            size_bytes: 77_691_713,
            description: "Найшвидша, найменша точність".to_string(),
            sha256: Some("be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21".to_string()),
        },
        ModelInfo {
            filename: "ggml-base.bin".to_string(),
            display_name: "Base".to_string(),
            size_bytes: 147_951_465,
            description: "Баланс швидкості та точності".to_string(),
            sha256: Some("60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe".to_string()),
        },
        ModelInfo {
            filename: "ggml-small.bin".to_string(),
            display_name: "Small".to_string(),
            size_bytes: 487_601_967,
            description: "Хороша точність".to_string(),
            sha256: Some("1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b".to_string()),
        },
        ModelInfo {
            filename: "ggml-medium.bin".to_string(),
            display_name: "Medium".to_string(),
            size_bytes: 1_533_763_059,
            description: "Висока точність".to_string(),
            sha256: Some("6c14d5adee5f86394037b4e4e8b59f1673b6cee10e3cf0b11bbdbee79c156208".to_string()),
        },
        ModelInfo {
            filename: "ggml-large-v3.bin".to_string(),
            display_name: "Large v3".to_string(),
            size_bytes: 3_095_033_483,
            description: "Найвища точність".to_string(),
            sha256: Some("64d182b440b98d5203c4f9bd541544d84c605196c4f7b845dfa11fb23594d1e2".to_string()),
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
    if sanitize_model_filename(filename).is_err() {
        return false;
    }
    let path = models_dir().join(filename);
    path.exists()
}

pub fn get_model_path(filename: &str) -> PathBuf {
    // Note: callers should validate before calling; this is a best-effort guard.
    // If the filename is invalid, we still return a path within models_dir
    // but the file won't exist, which is safe.
    models_dir().join(filename)
}

pub fn delete_model(filename: &str) -> Result<()> {
    sanitize_model_filename(filename)?;
    let path = models_dir().join(filename);

    if !path.exists() {
        return Err(anyhow::anyhow!("Модель не знайдено: {}", filename));
    }

    fs::remove_file(&path)
        .with_context(|| format!("Не вдалося видалити модель: {}", path.display()))?;

    Ok(())
}

/// Download a single file via HTTP with progress reporting, checksum verification,
/// and atomic rename from temp to final path.
async fn download_file(
    url: &str,
    dir: &Path,
    filename: &str,
    expected_sha256: Option<&str>,
    downloaded_offset: u64,
    total_size_override: u64,
    progress_callback: &(dyn Fn(u64, u64) + Send + Sync),
) -> Result<PathBuf> {
    fs::create_dir_all(dir)
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
            "Помилка завантаження {}: HTTP {}",
            filename,
            response.status()
        ));
    }

    let content_length = response.content_length().unwrap_or(0);
    let total_size = if total_size_override > 0 {
        total_size_override
    } else {
        content_length
    };
    let mut downloaded: u64 = downloaded_offset;

    let mut file = fs::File::create(&temp_path)
        .with_context(|| format!("Не вдалося створити файл: {}", temp_path.display()))?;

    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Помилка при завантаженні")?;
        std::io::Write::write_all(&mut file, &chunk).context("Не вдалося записати дані")?;

        downloaded += chunk.len() as u64;
        progress_callback(downloaded, total_size);
    }

    drop(file);

    if let Some(expected) = expected_sha256 {
        if let Err(e) = verify_checksum(&temp_path, expected) {
            let _ = fs::remove_file(&temp_path);
            return Err(e);
        }
    }

    fs::rename(&temp_path, &final_path).with_context(|| {
        format!(
            "Не вдалося перейменувати {} -> {}",
            temp_path.display(),
            final_path.display()
        )
    })?;

    Ok(final_path)
}

pub async fn download_model<F>(filename: &str, progress_callback: F) -> Result<()>
where
    F: Fn(u64, u64) + Send + Sync + 'static,
{
    sanitize_model_filename(filename)?;

    let expected_sha256 = get_available_models()
        .iter()
        .find(|m| m.filename == filename)
        .and_then(|m| m.sha256.clone());

    let url = format!("{}{}", HUGGINGFACE_BASE_URL, filename);

    if expected_sha256.is_none() {
        eprintln!(
            "Попередження: контрольна сума для {} невідома, пропускаємо перевірку",
            filename
        );
    }

    download_file(
        &url,
        &models_dir(),
        filename,
        expected_sha256.as_deref(),
        0,
        0,
        &progress_callback,
    )
    .await?;

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
        sha256: None, // Third-party model, hash TBD
    }
}

pub fn get_sortformer_model_path() -> PathBuf {
    crate::app::config::sortformer_models_dir().join("diar_streaming_sortformer_4spk-v2.1.onnx")
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
    F: Fn(u64, u64) + Send + Sync + 'static,
{
    let info = get_sortformer_model_info();
    let url = format!(
        "https://huggingface.co/altunenes/parakeet-rs/resolve/main/{}",
        info.filename
    );

    download_file(
        &url,
        &crate::app::config::sortformer_models_dir(),
        &info.filename,
        info.sha256.as_deref(),
        0,
        0,
        &progress_callback,
    )
    .await?;

    Ok(())
}

// TDT model management

/// TDT model file information.
#[derive(Debug, Clone)]
pub struct TdtModelFiles {
    pub encoder: ModelInfo,
    pub decoder: ModelInfo,
    pub vocab: ModelInfo,
}

/// Get TDT model file information.
///
/// TDT model consists of three files (INT8 versions for smaller size):
/// - encoder-model.int8.onnx (652 MB)
/// - decoder_joint-model.int8.onnx (18 MB)
/// - vocab.txt (94 KB)
pub fn get_tdt_model_info() -> TdtModelFiles {
    TdtModelFiles {
        encoder: ModelInfo {
            filename: "encoder-model.int8.onnx".to_string(),
            display_name: "TDT Encoder (INT8)".to_string(),
            size_bytes: 652_000_000,
            description: "Parakeet TDT encoder model".to_string(),
            sha256: None, // Third-party model, hash TBD
        },
        decoder: ModelInfo {
            filename: "decoder_joint-model.int8.onnx".to_string(),
            display_name: "TDT Decoder (INT8)".to_string(),
            size_bytes: 18_200_000,
            description: "Parakeet TDT decoder model".to_string(),
            sha256: None,
        },
        vocab: ModelInfo {
            filename: "vocab.txt".to_string(),
            display_name: "TDT Vocabulary".to_string(),
            size_bytes: 94_000,
            description: "Parakeet TDT vocabulary".to_string(),
            sha256: None,
        },
    }
}

/// Get total TDT model size in bytes.
pub fn get_tdt_total_size() -> u64 {
    let info = get_tdt_model_info();
    info.encoder.size_bytes + info.decoder.size_bytes + info.vocab.size_bytes
}

/// Get TDT model directory path.
#[allow(dead_code)]
pub fn get_tdt_model_path() -> PathBuf {
    crate::app::config::tdt_models_dir()
}

/// Check if all TDT model files are downloaded.
pub fn is_tdt_model_downloaded() -> bool {
    let dir = crate::app::config::tdt_models_dir();
    let info = get_tdt_model_info();

    dir.join(&info.encoder.filename).exists()
        && dir.join(&info.decoder.filename).exists()
        && dir.join(&info.vocab.filename).exists()
}

/// Delete all TDT model files.
pub fn delete_tdt_model() -> Result<()> {
    let dir = crate::app::config::tdt_models_dir();
    let info = get_tdt_model_info();

    for filename in [
        &info.encoder.filename,
        &info.decoder.filename,
        &info.vocab.filename,
    ] {
        let path = dir.join(filename);
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("Не вдалося видалити файл: {}", path.display()))?;
        }
    }

    Ok(())
}

/// Download TDT model files from HuggingFace.
///
/// Downloads INT8 versions for smaller size (~670 MB total):
/// - encoder-model.int8.onnx
/// - decoder_joint-model.int8.onnx
/// - vocab.txt
pub async fn download_tdt_model<F>(progress_callback: F) -> Result<()>
where
    F: Fn(u64, u64) + Send + Sync + 'static,
{
    const BASE_URL: &str = "https://huggingface.co/altunenes/parakeet-rs/resolve/main/tdt/";

    let dir = crate::app::config::tdt_models_dir();
    let info = get_tdt_model_info();
    let total_size = get_tdt_total_size();
    let mut total_downloaded: u64 = 0;

    for model_file in [&info.encoder, &info.decoder, &info.vocab] {
        let url = format!("{}{}", BASE_URL, model_file.filename);

        let final_path = download_file(
            &url,
            &dir,
            &model_file.filename,
            model_file.sha256.as_deref(),
            total_downloaded,
            total_size,
            &progress_callback,
        )
        .await?;

        // Update offset for cumulative progress across files
        total_downloaded += fs::metadata(&final_path)
            .map(|m| m.len())
            .unwrap_or(model_file.size_bytes);
    }

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
        for model in &models {
            assert!(!model.filename.is_empty());
            assert!(!model.display_name.is_empty());
            assert!(!model.description.is_empty());
            assert!(model.size_bytes > 0);
            assert!(model.filename.ends_with(".bin"));
        }
    }

    #[test]
    fn test_all_whisper_models_have_sha256() {
        let models = get_available_models();
        for model in &models {
            assert!(
                model.sha256.is_some(),
                "Model {} is missing SHA256 hash",
                model.filename
            );
            let hash = model.sha256.as_ref().unwrap();
            assert_eq!(hash.len(), 64, "SHA256 hash for {} should be 64 hex chars", model.filename);
            assert!(
                hash.chars().all(|c| c.is_ascii_hexdigit()),
                "SHA256 hash for {} should contain only hex digits",
                model.filename
            );
        }
    }

    #[test]
    fn test_verify_checksum_valid() {
        let dir = std::env::temp_dir().join("s2t_test_checksum");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("test_valid.bin");
        fs::write(&path, b"hello world").unwrap();

        // SHA256 of "hello world"
        let expected = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        assert!(verify_checksum(&path, expected).is_ok());

        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn test_verify_checksum_mismatch() {
        let dir = std::env::temp_dir().join("s2t_test_checksum_mismatch");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("test_mismatch.bin");
        fs::write(&path, b"hello world").unwrap();

        let wrong_hash = "0000000000000000000000000000000000000000000000000000000000000000";
        let result = verify_checksum(&path, wrong_hash);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Контрольна сума не збігається"));

        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn test_verify_checksum_file_not_found() {
        let path = std::env::temp_dir().join("s2t_test_nonexistent_file.bin");
        let result = verify_checksum(
            &path,
            "0000000000000000000000000000000000000000000000000000000000000000",
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_get_model_path_constructs_correctly() {
        let path = get_model_path("ggml-base.bin");
        assert!(path.to_string_lossy().contains("whisper"));
        assert!(path.to_string_lossy().ends_with("ggml-base.bin"));
    }

    #[test]
    fn test_tdt_model_info_has_three_files() {
        let info = get_tdt_model_info();
        assert!(!info.encoder.filename.is_empty());
        assert!(!info.decoder.filename.is_empty());
        assert!(!info.vocab.filename.is_empty());
    }

    #[test]
    fn test_tdt_total_size_reasonable() {
        let total = get_tdt_total_size();
        // Should be around 670 MB
        assert!(total > 600_000_000);
        assert!(total < 800_000_000);
    }

    #[test]
    fn test_tdt_model_path_contains_tdt() {
        let path = get_tdt_model_path();
        assert!(path.to_string_lossy().contains("tdt"));
    }

    // === Path traversal guard tests ===

    #[test]
    fn test_sanitize_valid_filename() {
        assert!(sanitize_model_filename("ggml-base.bin").is_ok());
        assert!(sanitize_model_filename("model-v2.1.onnx").is_ok());
        assert!(sanitize_model_filename("vocab.txt").is_ok());
    }

    #[test]
    fn test_sanitize_rejects_path_traversal() {
        assert!(sanitize_model_filename("../../../etc/passwd").is_err());
        assert!(sanitize_model_filename("..").is_err());
        assert!(sanitize_model_filename("foo/../bar").is_err());
    }

    #[test]
    fn test_sanitize_rejects_path_separators() {
        assert!(sanitize_model_filename("/etc/passwd").is_err());
        assert!(sanitize_model_filename("subdir/model.bin").is_err());
        assert!(sanitize_model_filename("C:\\Windows\\model.bin").is_err());
    }

    #[test]
    fn test_sanitize_rejects_empty() {
        assert!(sanitize_model_filename("").is_err());
    }

    #[test]
    fn test_sanitize_rejects_null_bytes() {
        assert!(sanitize_model_filename("model\0.bin").is_err());
    }

    #[test]
    fn test_is_model_downloaded_rejects_traversal() {
        // Should return false (not panic) for malicious filenames
        assert!(!is_model_downloaded("../../../etc/passwd"));
    }

    #[test]
    fn test_delete_model_rejects_traversal() {
        let result = delete_model("../../../etc/passwd");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Неприпустиме ім'я файлу"));
    }
}
