# Audio Pipeline Analysis

## Current Pipelines

### Dictation Mode

```
Mic -> CPAL capture -> resample 16kHz -> [no denoise] -> VAD (optional) -> segment -> Whisper/TDT -> clipboard
```

- Audio captured via CPAL at device native rate
- Resampled to 16kHz mono
- **No denoising applied** (despite `denoise_enabled` config option existing)
- VAD optionally segments continuous speech
- Each segment transcribed independently
- Result copied to clipboard / pasted via xdotool

### Conference Mode

```
Mic -------> CPAL capture -> resample 16kHz ----\
                                                  +-> save stereo WAV -> batch transcribe + diarize
Loopback --> parec capture -> resample 16kHz ---/
```

- Two audio sources captured simultaneously
- Both resampled to 16kHz
- **No denoising applied**
- **No VAD / segmentation** -- entire recording transcribed as one batch
- Stereo WAV saved to disk (left=mic, right=loopback)
- Diarization via channel separation or Sortformer neural model

### CLI `transcribe` Command

```
WAV file -> read -> channel select -> resample 16kHz -> [denoise if --denoise] -> transcribe
```

- **Only place denoising is actually used** (via `--denoise` flag)
- Supports all backends and diarization methods
- No VAD processing

## Differences: Essential vs Accidental

| Aspect | Dictation | Conference | Essential? |
|--------|-----------|------------|------------|
| Audio sources | Mic only | Mic + Loopback | **YES** -- defines the mode |
| Denoising | None | None | **NO** -- both need it |
| VAD / Segmentation | Optional (WebRTC/Silero) | None | **NO** -- conference should support it |
| Real-time streaming | Yes (segmented) | No (batch after stop) | **NO** -- conference could stream too |
| Recording save | No | Yes (stereo WAV) | **NO** -- make optional for both |
| Diarization | None | Channel / Sortformer | **YES** -- conference-specific |
| Dual amplitude bars | No | Yes (mic + loopback) | **YES** -- UI-only, follows from dual sources |

## Key Insight

Conference mode is dictation + loopback + diarization-by-default. The differences in denoising, VAD, and streaming are **accidental** -- they arose from separate implementations rather than intentional design.

## Problem: Missing Denoiser in GUI

The denoiser (`NnnoiselessDenoiser`) exists and works:
- `src/recording/denoise.rs` implements 16kHz -> 48kHz -> RNNoise -> 16kHz pipeline
- `src/cli/wav_reader.rs` calls it when `--denoise` is passed to CLI
- Config has `denoise_enabled: bool` field

But neither `AudioRecorder` (dictation) nor `ConferenceRecorder` passes audio through the denoiser before VAD or transcription. Raw noisy 16kHz samples go directly to:
1. VAD -- causing false positives on background noise
2. Whisper/TDT -- causing hallucinations and garbage output on noisy input

## Proposed Unified Pipeline

```
[Mic] ---------\
                 +-> resample 16kHz -> [Denoise?] -> [VAD/Segment?] -> [Transcribe] -> [Diarize?] -> [Output]
[Loopback?] ---/
```

Configuration controls:
- **Sources**: Mic only (dictation) or Mic + Loopback (conference)
- **Denoise**: On/off per config (`denoise_enabled`)
- **VAD**: Engine selection (WebRTC/Silero/None) and thresholds
- **Segmentation**: Continuous vs single-shot
- **Diarization**: None / Channel / Sortformer
- **Output**: Clipboard, file, or both

## Evaluation Tool

The `denoise-eval` CLI command (`cargo run --release -- denoise-eval`) measures denoiser impact:

```bash
# Signal metrics only (fast)
cargo run --release -- denoise-eval input.wav --channel left -o /tmp/out/

# Add VAD comparison
cargo run --release -- denoise-eval input.wav --channel left --vad

# Full A/B with transcription
cargo run --release -- denoise-eval input.wav --channel left --vad --transcribe
```

Output: JSON report with signal metrics (RMS, peak), VAD speech percentages (WebRTC + Silero), and optional transcription comparison.
