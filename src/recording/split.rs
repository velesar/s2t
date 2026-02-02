//! Shared split-point finding logic for audio segmentation.
//!
//! Provides a `SplitFinder` that implements a three-tier cascading strategy
//! for determining where to split audio:
//!
//! 1. **Semantic**: Long silences (>2s) indicating topic boundaries
//! 2. **VAD**: Shorter silences (>500ms) indicating sentence boundaries
//! 3. **Size**: Force-split at max segment length with overlap
//!
//! Used by both streaming (`SegmentationMonitor`) and batch (`AudioChunker`) modes.

use crate::domain::traits::VoiceDetection;
use std::time::Duration;

/// A detected silence region in audio.
#[derive(Debug, Clone)]
pub struct SilenceRegion {
    /// First silent sample (index into audio buffer).
    pub start_sample: usize,
    /// First non-silent sample after the gap.
    pub end_sample: usize,
    /// Silence duration in milliseconds.
    pub duration_ms: u32,
}

impl SilenceRegion {
    /// Midpoint sample of this silence region.
    pub fn midpoint(&self) -> usize {
        (self.start_sample + self.end_sample) / 2
    }
}

/// Configuration for split-finding.
#[derive(Debug, Clone)]
pub struct SplitConfig {
    /// Minimum silence duration for semantic (tier 1) splits.
    pub semantic_silence_ms: u32,
    /// Minimum silence duration for VAD (tier 2) splits.
    pub vad_silence_ms: u32,
    /// Maximum segment length before force-split (tier 3).
    pub max_segment_secs: u32,
    /// Minimum segment length (avoid tiny segments).
    pub min_segment_secs: u32,
    /// Overlap duration for force-splits (to reduce boundary artifacts).
    pub overlap_secs: u32,
    /// Audio sample rate.
    pub sample_rate: u32,
}

impl Default for SplitConfig {
    fn default() -> Self {
        Self {
            semantic_silence_ms: 2000,
            vad_silence_ms: 500,
            max_segment_secs: 300,
            min_segment_secs: 1,
            overlap_secs: 2,
            sample_rate: 16000,
        }
    }
}

impl SplitConfig {
    pub fn max_segment_samples(&self) -> usize {
        self.max_segment_secs as usize * self.sample_rate as usize
    }

    fn min_segment_samples(&self) -> usize {
        self.min_segment_secs as usize * self.sample_rate as usize
    }

    fn overlap_samples(&self) -> usize {
        self.overlap_secs as usize * self.sample_rate as usize
    }
}

/// Result of a split decision.
#[derive(Debug, Clone, PartialEq)]
pub enum SplitPoint {
    /// Split at a silence boundary (midpoint of the silence region).
    Silence {
        sample: usize,
        tier: SplitTier,
    },
    /// Force-split at max length with overlap.
    ForceSplit {
        sample: usize,
        overlap_samples: usize,
    },
    /// No split needed (audio is short enough).
    None,
}

/// Which cascade tier triggered the split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitTier {
    Semantic,
    Vad,
}

/// VAD frame size for scanning (30ms at 16kHz = 480 samples).
const VAD_FRAME_MS: u32 = 30;

/// Minimum silence duration to record (ignore shorter blips).
const MIN_SILENCE_MS: u32 = 100;

/// Shared segmentation logic for both streaming and batch modes.
pub struct SplitFinder {
    config: SplitConfig,
}

impl SplitFinder {
    pub fn new(config: SplitConfig) -> Self {
        Self { config }
    }

    /// Scan audio buffer and find all silence regions using VAD.
    ///
    /// Processes audio in VAD-frame-sized steps, tracking speech→silence
    /// transitions and recording each silence region.
    pub fn scan_silences(
        &self,
        samples: &[f32],
        vad: &dyn VoiceDetection,
    ) -> Vec<SilenceRegion> {
        let frame_size = (self.config.sample_rate * VAD_FRAME_MS / 1000) as usize;
        let min_silence_samples =
            (self.config.sample_rate as usize * MIN_SILENCE_MS as usize) / 1000;

        let mut regions = Vec::new();
        let mut silence_start: Option<usize> = None;

        let mut pos = 0;
        while pos + frame_size <= samples.len() {
            let frame = &samples[pos..pos + frame_size];
            let is_speech = vad.is_speech(frame).unwrap_or(false);

            if is_speech {
                // Speech detected — close any open silence region
                if let Some(start) = silence_start.take() {
                    let duration_samples = pos - start;
                    if duration_samples >= min_silence_samples {
                        let duration_ms = (duration_samples as u64 * 1000
                            / self.config.sample_rate as u64)
                            as u32;
                        regions.push(SilenceRegion {
                            start_sample: start,
                            end_sample: pos,
                            duration_ms,
                        });
                    }
                }
            } else if silence_start.is_none() {
                // Silence starts
                silence_start = Some(pos);
            }

            pos += frame_size;
        }

        // Close trailing silence region
        if let Some(start) = silence_start {
            let duration_samples = samples.len() - start;
            if duration_samples >= min_silence_samples {
                let duration_ms =
                    (duration_samples as u64 * 1000 / self.config.sample_rate as u64) as u32;
                regions.push(SilenceRegion {
                    start_sample: start,
                    end_sample: samples.len(),
                    duration_ms,
                });
            }
        }

        regions
    }

    /// Find best split point within a sample range using the cascade:
    ///
    /// 1. Longest silence >= `semantic_silence_ms` (prefer later = larger segment)
    /// 2. Longest silence >= `vad_silence_ms`
    /// 3. Force-split at range end with overlap
    pub fn find_best_split(
        &self,
        silences: &[SilenceRegion],
        window_start: usize,
        window_end: usize,
    ) -> SplitPoint {
        let window_len = window_end.saturating_sub(window_start);
        if window_len <= self.config.min_segment_samples() {
            return SplitPoint::None;
        }

        // Filter silences within our window, excluding the very start/end edges
        let min_edge = window_start + self.config.min_segment_samples();
        let candidates: Vec<&SilenceRegion> = silences
            .iter()
            .filter(|s| s.midpoint() >= min_edge && s.midpoint() < window_end)
            .collect();

        // Tier 1: Semantic — longest silence >= semantic_silence_ms, prefer latest
        if let Some(best) = candidates
            .iter()
            .filter(|s| s.duration_ms >= self.config.semantic_silence_ms)
            .max_by_key(|s| (s.duration_ms, s.midpoint()))
        {
            return SplitPoint::Silence {
                sample: best.midpoint(),
                tier: SplitTier::Semantic,
            };
        }

        // Tier 2: VAD — longest silence >= vad_silence_ms, prefer latest
        if let Some(best) = candidates
            .iter()
            .filter(|s| s.duration_ms >= self.config.vad_silence_ms)
            .max_by_key(|s| (s.duration_ms, s.midpoint()))
        {
            return SplitPoint::Silence {
                sample: best.midpoint(),
                tier: SplitTier::Vad,
            };
        }

        // Tier 3: Force-split at window_end with overlap
        SplitPoint::ForceSplit {
            sample: window_end,
            overlap_samples: self.config.overlap_samples(),
        }
    }

    /// Streaming mode helper: check if audio should be split now.
    ///
    /// Used by `SegmentationMonitor` — checks if max segment time exceeded
    /// or if VAD detects speech end in recent audio.
    pub fn should_split_streaming(
        &self,
        recent_samples: &[f32],
        vad: &dyn VoiceDetection,
        elapsed_since_last_split: Duration,
    ) -> bool {
        // Safety limit: force split if max segment time exceeded
        let max_dur = Duration::from_secs(self.config.max_segment_secs as u64);
        if elapsed_since_last_split >= max_dur {
            return true;
        }

        // Need at least 1 second of audio for reliable VAD decision
        let min_samples = self.config.sample_rate as usize;
        if recent_samples.len() < min_samples {
            return false;
        }

        // Delegate to VAD's speech-end detection
        vad.detect_speech_end(recent_samples).unwrap_or(false)
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Mock VAD that returns speech for samples above a threshold.
    struct MockVad {
        threshold: f32,
        reset_count: RefCell<u32>,
    }

    impl MockVad {
        fn new(threshold: f32) -> Self {
            Self {
                threshold,
                reset_count: RefCell::new(0),
            }
        }
    }

    impl VoiceDetection for MockVad {
        fn is_speech(&self, samples: &[f32]) -> anyhow::Result<bool> {
            // Consider it speech if RMS is above threshold
            let rms = if samples.is_empty() {
                0.0
            } else {
                let sum: f32 = samples.iter().map(|s| s * s).sum();
                (sum / samples.len() as f32).sqrt()
            };
            Ok(rms > self.threshold)
        }

        fn detect_speech_end(&self, samples: &[f32]) -> anyhow::Result<bool> {
            // Check if the last portion is silence
            let tail_size = 480 * 10; // ~300ms
            if samples.len() < tail_size {
                return Ok(false);
            }
            let tail = &samples[samples.len() - tail_size..];
            let rms: f32 = {
                let sum: f32 = tail.iter().map(|s| s * s).sum();
                (sum / tail.len() as f32).sqrt()
            };
            Ok(rms <= self.threshold)
        }

        fn reset(&self) {
            *self.reset_count.borrow_mut() += 1;
        }
    }

    fn make_silence(num_samples: usize) -> Vec<f32> {
        vec![0.0; num_samples]
    }

    fn make_speech(num_samples: usize) -> Vec<f32> {
        vec![0.5; num_samples]
    }

    fn make_audio_with_silence_gap(
        speech1_secs: f32,
        silence_secs: f32,
        speech2_secs: f32,
    ) -> Vec<f32> {
        let sr = 16000;
        let mut audio = Vec::new();
        audio.extend(make_speech((speech1_secs * sr as f32) as usize));
        audio.extend(make_silence((silence_secs * sr as f32) as usize));
        audio.extend(make_speech((speech2_secs * sr as f32) as usize));
        audio
    }

    #[test]
    fn test_scan_silences_no_silence() {
        let finder = SplitFinder::new(SplitConfig::default());
        let vad = MockVad::new(0.01);
        let audio = make_speech(16000 * 5);

        let silences = finder.scan_silences(&audio, &vad);
        assert!(silences.is_empty());
    }

    #[test]
    fn test_scan_silences_detects_gap() {
        let finder = SplitFinder::new(SplitConfig::default());
        let vad = MockVad::new(0.01);
        // 2s speech, 1s silence, 2s speech
        let audio = make_audio_with_silence_gap(2.0, 1.0, 2.0);

        let silences = finder.scan_silences(&audio, &vad);
        assert_eq!(silences.len(), 1);
        assert!(silences[0].duration_ms >= 900); // ~1000ms with frame quantization
        assert!(silences[0].duration_ms <= 1100);
    }

    #[test]
    fn test_scan_silences_ignores_short_silence() {
        let finder = SplitFinder::new(SplitConfig::default());
        let vad = MockVad::new(0.01);
        // 2s speech, 50ms silence (below 100ms threshold), 2s speech
        let audio = make_audio_with_silence_gap(2.0, 0.05, 2.0);

        let silences = finder.scan_silences(&audio, &vad);
        assert!(silences.is_empty());
    }

    #[test]
    fn test_scan_silences_multiple_gaps() {
        let finder = SplitFinder::new(SplitConfig::default());
        let vad = MockVad::new(0.01);
        let sr = 16000;
        let mut audio = Vec::new();
        // speech, gap, speech, gap, speech
        audio.extend(make_speech(sr));
        audio.extend(make_silence(sr / 2)); // 500ms
        audio.extend(make_speech(sr));
        audio.extend(make_silence(sr * 2)); // 2000ms
        audio.extend(make_speech(sr));

        let silences = finder.scan_silences(&audio, &vad);
        assert_eq!(silences.len(), 2);
        assert!(silences[0].duration_ms < silences[1].duration_ms);
    }

    #[test]
    fn test_find_best_split_semantic() {
        let finder = SplitFinder::new(SplitConfig::default());
        let sr = 16000;
        // Silence region at sample 48000 (3s) with 2.5s duration
        let silences = vec![SilenceRegion {
            start_sample: sr * 3,
            end_sample: sr * 3 + sr * 5 / 2,
            duration_ms: 2500,
        }];

        let result = finder.find_best_split(&silences, 0, sr * 10);
        match result {
            SplitPoint::Silence { tier, .. } => {
                assert_eq!(tier, SplitTier::Semantic);
            }
            other => panic!("Expected Silence(Semantic), got {:?}", other),
        }
    }

    #[test]
    fn test_find_best_split_vad_fallback() {
        let finder = SplitFinder::new(SplitConfig::default());
        let sr = 16000;
        // Silence region: 800ms (above vad_silence_ms=500, below semantic=2000)
        let silences = vec![SilenceRegion {
            start_sample: sr * 3,
            end_sample: sr * 3 + sr * 4 / 5,
            duration_ms: 800,
        }];

        let result = finder.find_best_split(&silences, 0, sr * 10);
        match result {
            SplitPoint::Silence { tier, .. } => {
                assert_eq!(tier, SplitTier::Vad);
            }
            other => panic!("Expected Silence(Vad), got {:?}", other),
        }
    }

    #[test]
    fn test_find_best_split_force_split() {
        let finder = SplitFinder::new(SplitConfig::default());
        let sr = 16000;
        // No silences at all
        let silences: Vec<SilenceRegion> = vec![];

        let result = finder.find_best_split(&silences, 0, sr * 10);
        match result {
            SplitPoint::ForceSplit {
                sample,
                overlap_samples,
            } => {
                assert_eq!(sample, sr * 10);
                assert_eq!(overlap_samples, sr * 2); // 2s overlap
            }
            other => panic!("Expected ForceSplit, got {:?}", other),
        }
    }

    #[test]
    fn test_find_best_split_none_for_short_audio() {
        let finder = SplitFinder::new(SplitConfig::default());
        // Window smaller than min_segment_secs (1s = 16000 samples)
        let result = finder.find_best_split(&[], 0, 8000);
        assert_eq!(result, SplitPoint::None);
    }

    #[test]
    fn test_find_best_split_prefers_longest_semantic() {
        let finder = SplitFinder::new(SplitConfig::default());
        let sr = 16000;
        let silences = vec![
            SilenceRegion {
                start_sample: sr * 2,
                end_sample: sr * 2 + sr * 2, // 2s
                duration_ms: 2000,
            },
            SilenceRegion {
                start_sample: sr * 6,
                end_sample: sr * 6 + sr * 3, // 3s
                duration_ms: 3000,
            },
        ];

        let result = finder.find_best_split(&silences, 0, sr * 15);
        match result {
            SplitPoint::Silence { sample, tier } => {
                assert_eq!(tier, SplitTier::Semantic);
                // Should pick the 3s silence (longer + later)
                let expected_mid = (sr * 6 + sr * 6 + sr * 3) / 2;
                assert_eq!(sample, expected_mid);
            }
            other => panic!("Expected Silence(Semantic), got {:?}", other),
        }
    }

    #[test]
    fn test_should_split_streaming_max_segment() {
        let finder = SplitFinder::new(SplitConfig {
            max_segment_secs: 10,
            ..Default::default()
        });
        let vad = MockVad::new(0.01);
        let audio = make_speech(16000 * 5);

        // 11 seconds elapsed — should force split
        assert!(finder.should_split_streaming(&audio, &vad, Duration::from_secs(11)));
    }

    #[test]
    fn test_should_split_streaming_vad_speech_end() {
        let finder = SplitFinder::new(SplitConfig::default());
        let vad = MockVad::new(0.01);

        // Audio with speech followed by silence at the end
        let sr = 16000;
        let mut audio = make_speech(sr * 2);
        audio.extend(make_silence(sr)); // 1s of silence at the end
        // detect_speech_end checks last ~300ms for silence
        assert!(finder.should_split_streaming(&audio, &vad, Duration::from_secs(3)));
    }

    #[test]
    fn test_should_split_streaming_too_short() {
        let finder = SplitFinder::new(SplitConfig::default());
        let vad = MockVad::new(0.01);
        // Less than 1 second of audio
        let audio = make_silence(8000);

        assert!(!finder.should_split_streaming(&audio, &vad, Duration::from_secs(0)));
    }

    #[test]
    fn test_silence_region_midpoint() {
        let region = SilenceRegion {
            start_sample: 100,
            end_sample: 200,
            duration_ms: 100,
        };
        assert_eq!(region.midpoint(), 150);
    }

    #[test]
    fn test_split_config_defaults() {
        let config = SplitConfig::default();
        assert_eq!(config.semantic_silence_ms, 2000);
        assert_eq!(config.vad_silence_ms, 500);
        assert_eq!(config.max_segment_secs, 300);
        assert_eq!(config.min_segment_secs, 1);
        assert_eq!(config.overlap_secs, 2);
        assert_eq!(config.sample_rate, 16000);
    }

    #[test]
    fn test_split_config_sample_calculations() {
        let config = SplitConfig::default();
        assert_eq!(config.max_segment_samples(), 300 * 16000);
        assert_eq!(config.min_segment_samples(), 16000);
        assert_eq!(config.overlap_samples(), 32000);
    }

    #[test]
    fn test_scan_trailing_silence() {
        let finder = SplitFinder::new(SplitConfig::default());
        let vad = MockVad::new(0.01);
        let sr = 16000;
        // Speech followed by trailing silence
        let mut audio = make_speech(sr * 2);
        audio.extend(make_silence(sr)); // 1s trailing

        let silences = finder.scan_silences(&audio, &vad);
        assert_eq!(silences.len(), 1);
        assert_eq!(silences[0].end_sample, audio.len());
    }
}
