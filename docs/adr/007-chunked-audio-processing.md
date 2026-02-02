# ADR-007: Cascading Audio Chunking with Unified Segmentation

## Status
**Accepted** (2026-02-02)

## Context

Three related problems in the current audio processing pipeline:

1. **TDT OOM**: `parakeet-rs` passes entire audio to ONNX runtime in one call (`src/transcription/tdt.rs`). Files longer than ~10 minutes cause out-of-memory errors because the entire waveform is allocated in the ONNX session.

2. **Duplicated segmentation logic**: The streaming GUI path (`SegmentationMonitor`) uses VAD + fixed-interval segmentation, while the CLI batch path has no segmentation at all. These two paths share no code and diverge in behavior.

3. **No cascading strategy**: The streaming segmentation only has two modes: VAD-triggered or fixed-interval. There is no semantic tier (long silences indicating topic boundaries), no maximum segment guard (segments can grow unbounded if VAD never triggers), and no overlap handling for force-splits.

## Decision

Introduce a **shared `SplitFinder`** that encapsulates cascading split-point logic, used by both streaming (GUI) and batch (CLI) modes.

### Three-Tier Cascade

1. **Semantic** (primary): Split at long silences (>2s) — topic/paragraph boundaries
2. **VAD** (fallback): Split at shorter silences (>500ms) — sentence boundaries
3. **Size** (last resort): Force-split at `max_segment_secs` with overlap window

### Architecture

```
src/recording/split.rs          ← Shared SplitFinder + SilenceRegion
    ↑                    ↑
    |                    |
src/recording/           src/transcription/
  segmentation.rs          chunker.rs       ← Batch AudioChunker
  (streaming GUI)          (batch CLI)
```

### Key Types

- `SplitFinder`: Stateless split-point calculator using VAD silence scanning
- `SplitConfig`: Thresholds for semantic silence, VAD silence, max segment, overlap
- `AudioChunker`: Batch-mode wrapper that segments pre-loaded audio and transcribes chunks

### CLI Integration

New flags: `--max-segment-secs` (default 300), `--no-chunking` (bypass for testing).
New config field: `max_segment_secs` (default 300, clamped 30..1800).

## Consequences

### Positive
- TDT backend can process arbitrarily long files without OOM
- Whisper benefits from chunking too (better accuracy on shorter segments)
- Streaming and batch share the same split-finding logic
- `max_segment_secs` safety limit prevents unbounded segment growth in streaming mode
- Overlap on force-splits reduces boundary artifacts

### Negative
- Minor refactor of `SegmentationMonitor` to delegate to `SplitFinder`
- Force-split boundaries may produce minor transcription artifacts at chunk edges
- Additional ~350 LOC of new code

### Neutral
- Existing streaming behavior is preserved (VAD + fixed-interval still works)
- `--no-chunking` flag allows reverting to old behavior for debugging
