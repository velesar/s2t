//! Batch audio chunking for CLI transcription.
//!
//! Uses `SplitFinder` to segment pre-loaded audio into chunks, then transcribes
//! each chunk individually to avoid OOM with large files (especially TDT backend).

use crate::domain::traits::Transcription;
use crate::recording::split::{SplitConfig, SplitFinder, SplitPoint};
use crate::vad::{create_vad, VadConfig};
use anyhow::{Context, Result};

/// Configuration for the batch audio chunker.
#[derive(Default)]
pub struct ChunkerConfig {
    pub split: SplitConfig,
    pub vad: VadConfig,
}

/// A chunk of audio with position metadata.
///
/// References a range within the original audio buffer (no copies).
#[derive(Debug, Clone)]
pub struct AudioChunk {
    pub start_sample: usize,
    pub end_sample: usize,
    /// Whether this chunk was created by a force-split.
    pub has_overlap: bool,
    /// Number of leading samples that overlap with the previous chunk.
    pub leading_overlap_samples: usize,
}

/// Batch audio chunker that segments and transcribes long audio files.
pub struct AudioChunker {
    config: ChunkerConfig,
}

impl AudioChunker {
    pub fn new(config: ChunkerConfig) -> Self {
        Self { config }
    }

    /// Segment pre-loaded audio into chunks using cascading split strategy.
    pub fn segment(&self, samples: &[f32]) -> Result<Vec<AudioChunk>> {
        let max_samples = self.config.split.max_segment_samples();

        // Short audio: single chunk, no segmentation needed
        if samples.len() <= max_samples {
            return Ok(vec![AudioChunk {
                start_sample: 0,
                end_sample: samples.len(),
                has_overlap: false,
                leading_overlap_samples: 0,
            }]);
        }

        // Create VAD and scan for silence regions
        let vad = create_vad(&self.config.vad).context("Failed to create VAD for chunking")?;
        let finder = SplitFinder::new(self.config.split.clone());
        let silences = finder.scan_silences(samples, vad.as_ref());

        let mut chunks = Vec::new();
        let mut pos = 0;

        while pos < samples.len() {
            let remaining = samples.len() - pos;

            // If remaining audio fits in one chunk, take it all
            if remaining <= max_samples {
                chunks.push(AudioChunk {
                    start_sample: pos,
                    end_sample: samples.len(),
                    has_overlap: false,
                    leading_overlap_samples: 0,
                });
                break;
            }

            // Find best split within the next max_segment window
            let window_end = (pos + max_samples).min(samples.len());
            let split = finder.find_best_split(&silences, pos, window_end);

            match split {
                SplitPoint::Silence { sample, .. } => {
                    chunks.push(AudioChunk {
                        start_sample: pos,
                        end_sample: sample,
                        has_overlap: false,
                        leading_overlap_samples: 0,
                    });
                    pos = sample;
                }
                SplitPoint::ForceSplit {
                    sample,
                    overlap_samples,
                } => {
                    chunks.push(AudioChunk {
                        start_sample: pos,
                        end_sample: sample,
                        has_overlap: true,
                        leading_overlap_samples: 0,
                    });
                    // Next chunk starts with overlap
                    pos = sample.saturating_sub(overlap_samples);
                    // Record overlap for the next chunk
                    if let Some(next_overlap) =
                        chunks.last().map(|c| c.end_sample.saturating_sub(pos))
                    {
                        // Will be set on the next chunk when it's created
                        // Store temporarily — applied below
                        if pos < samples.len() {
                            // Peek: the next chunk will have leading overlap
                            let _ = next_overlap; // used in next iteration
                        }
                    }
                }
                SplitPoint::None => {
                    // Shouldn't happen for long audio, but handle gracefully
                    chunks.push(AudioChunk {
                        start_sample: pos,
                        end_sample: samples.len(),
                        has_overlap: false,
                        leading_overlap_samples: 0,
                    });
                    break;
                }
            }
        }

        // Fix up leading overlap for chunks after force-splits
        for i in 1..chunks.len() {
            if chunks[i - 1].has_overlap {
                let overlap = chunks[i - 1]
                    .end_sample
                    .saturating_sub(chunks[i].start_sample);
                chunks[i].leading_overlap_samples = overlap;
            }
        }

        Ok(chunks)
    }

    /// Transcribe audio with chunking, merging results.
    pub fn transcribe_chunked(
        &self,
        samples: &[f32],
        language: &str,
        backend: &dyn Transcription,
    ) -> Result<String> {
        let chunks = self.segment(samples)?;

        if chunks.len() == 1 {
            // Single chunk — no overhead
            let chunk = &chunks[0];
            return backend.transcribe(&samples[chunk.start_sample..chunk.end_sample], language);
        }

        eprintln!(
            "Audio segmented into {} chunks for processing",
            chunks.len()
        );

        let mut texts = Vec::with_capacity(chunks.len());
        for (i, chunk) in chunks.iter().enumerate() {
            eprintln!(
                "  Transcribing chunk {}/{} ({:.1}s)...",
                i + 1,
                chunks.len(),
                (chunk.end_sample - chunk.start_sample) as f64
                    / self.config.split.sample_rate as f64
            );
            let text =
                backend.transcribe(&samples[chunk.start_sample..chunk.end_sample], language)?;
            texts.push(text.trim().to_string());
        }

        Ok(merge_chunk_results(&texts))
    }
}

/// Merge transcription results from multiple chunks.
///
/// Simple whitespace join, filtering empty results.
fn merge_chunk_results(texts: &[String]) -> String {
    texts
        .iter()
        .filter(|t| !t.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::traits::VoiceDetection;
    use std::cell::RefCell;
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// Mock VAD for chunker tests.
    struct MockChunkerVad;

    impl VoiceDetection for MockChunkerVad {
        fn is_speech(&self, samples: &[f32]) -> anyhow::Result<bool> {
            let rms = if samples.is_empty() {
                0.0
            } else {
                let sum: f32 = samples.iter().map(|s| s * s).sum();
                (sum / samples.len() as f32).sqrt()
            };
            Ok(rms > 0.01)
        }

        fn detect_speech_end(&self, _samples: &[f32]) -> anyhow::Result<bool> {
            Ok(false)
        }

        fn reset(&self) {}
    }

    /// Mock transcription backend.
    struct MockTranscription {
        loaded: AtomicBool,
        call_count: RefCell<usize>,
    }

    impl MockTranscription {
        fn new() -> Self {
            Self {
                loaded: AtomicBool::new(true),
                call_count: RefCell::new(0),
            }
        }

        #[allow(dead_code)]
        fn call_count(&self) -> usize {
            *self.call_count.borrow()
        }
    }

    impl Transcription for MockTranscription {
        fn transcribe(&self, samples: &[f32], _language: &str) -> anyhow::Result<String> {
            *self.call_count.borrow_mut() += 1;
            Ok(format!("[chunk:{}samples]", samples.len()))
        }

        fn is_loaded(&self) -> bool {
            self.loaded.load(Ordering::SeqCst)
        }

        fn model_name(&self) -> Option<String> {
            Some("mock".to_string())
        }

        fn load_model(&mut self, _path: &Path) -> anyhow::Result<()> {
            Ok(())
        }
    }

    // Safety: MockTranscription uses AtomicBool and RefCell (single-threaded tests)
    unsafe impl Send for MockTranscription {}
    unsafe impl Sync for MockTranscription {}

    #[test]
    fn test_short_audio_single_chunk() {
        let chunker = AudioChunker::new(ChunkerConfig::default());
        // 10 seconds at 16kHz — well under 300s max
        let audio = vec![0.5_f32; 16000 * 10];
        let chunks = chunker.segment(&audio).unwrap();

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].start_sample, 0);
        assert_eq!(chunks[0].end_sample, audio.len());
        assert!(!chunks[0].has_overlap);
    }

    #[test]
    fn test_empty_audio() {
        let chunker = AudioChunker::new(ChunkerConfig::default());
        let audio: Vec<f32> = Vec::new();
        let chunks = chunker.segment(&audio).unwrap();

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].start_sample, 0);
        assert_eq!(chunks[0].end_sample, 0);
    }

    #[test]
    fn test_exact_max_segment() {
        let config = ChunkerConfig {
            split: SplitConfig {
                max_segment_secs: 10,
                ..Default::default()
            },
            ..Default::default()
        };
        let chunker = AudioChunker::new(config);
        // Exactly 10 seconds
        let audio = vec![0.5_f32; 16000 * 10];
        let chunks = chunker.segment(&audio).unwrap();

        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn test_long_audio_gets_chunked() {
        let config = ChunkerConfig {
            split: SplitConfig {
                max_segment_secs: 5, // 5 second max for testing
                ..Default::default()
            },
            ..Default::default()
        };
        let chunker = AudioChunker::new(config);
        // 15 seconds of speech
        let audio = vec![0.5_f32; 16000 * 15];
        let chunks = chunker.segment(&audio).unwrap();

        assert!(
            chunks.len() > 1,
            "Expected multiple chunks, got {}",
            chunks.len()
        );
        // All audio should be covered
        assert_eq!(chunks[0].start_sample, 0);
        assert_eq!(chunks.last().unwrap().end_sample, audio.len());
    }

    #[test]
    fn test_silence_split() {
        let config = ChunkerConfig {
            split: SplitConfig {
                max_segment_secs: 10,
                ..Default::default()
            },
            ..Default::default()
        };
        let chunker = AudioChunker::new(config);

        // 4s speech + 3s silence + 4s speech + 3s silence + 4s speech = 18s
        let sr = 16000;
        let mut audio = Vec::new();
        audio.extend(vec![0.5_f32; sr * 4]);
        audio.extend(vec![0.0_f32; sr * 3]); // Long silence — semantic split
        audio.extend(vec![0.5_f32; sr * 4]);
        audio.extend(vec![0.0_f32; sr * 3]);
        audio.extend(vec![0.5_f32; sr * 4]);

        let chunks = chunker.segment(&audio).unwrap();
        assert!(
            chunks.len() >= 2,
            "Expected at least 2 chunks for audio with silence gaps, got {}",
            chunks.len()
        );
    }

    #[test]
    fn test_merge_chunk_results() {
        let texts = vec![
            "Hello world".to_string(),
            "".to_string(),
            "Foo bar".to_string(),
        ];
        assert_eq!(merge_chunk_results(&texts), "Hello world Foo bar");
    }

    #[test]
    fn test_merge_empty() {
        let texts: Vec<String> = vec![];
        assert_eq!(merge_chunk_results(&texts), "");
    }

    #[test]
    fn test_merge_all_empty() {
        let texts = vec!["".to_string(), "".to_string()];
        assert_eq!(merge_chunk_results(&texts), "");
    }

    #[test]
    fn test_transcribe_chunked_single() {
        let chunker = AudioChunker::new(ChunkerConfig::default());
        let backend = MockTranscription::new();
        let audio = vec![0.5_f32; 16000 * 5]; // 5s — single chunk

        let result = chunker.transcribe_chunked(&audio, "en", &backend).unwrap();
        assert!(result.contains("chunk:"));
        assert_eq!(backend.call_count(), 1);
    }

    #[test]
    fn test_transcribe_chunked_multiple() {
        let config = ChunkerConfig {
            split: SplitConfig {
                max_segment_secs: 5,
                ..Default::default()
            },
            ..Default::default()
        };
        let chunker = AudioChunker::new(config);
        let backend = MockTranscription::new();
        let audio = vec![0.5_f32; 16000 * 15]; // 15s — should be split

        let result = chunker.transcribe_chunked(&audio, "en", &backend).unwrap();
        assert!(backend.call_count() > 1);
        assert!(result.contains("chunk:"));
    }

    #[test]
    fn test_chunker_config_default() {
        let config = ChunkerConfig::default();
        assert_eq!(config.split.max_segment_secs, 300);
        assert_eq!(config.vad.engine, crate::vad::VadEngine::WebRTC);
    }
}
