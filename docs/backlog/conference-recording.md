# Режим запису конференцій

## User Story
As a user, I want to record both my microphone input and system audio output (from speakers/headphones) during online conferences, merge them into a single audio file, and transcribe the conversation with speaker identification, so that I can have a complete record of the meeting with clear attribution of who said what.

## Acceptance Criteria
- [ ] User can switch between "Диктовка" (dictation) and "Конференція" (conference) recording modes
- [ ] In conference mode, application records both:
  - Microphone input (user's voice)
  - System audio output (audio from speakers/headphones - remote participants)
- [ ] Two audio streams are merged into a single synchronized audio file
- [ ] Audio files are saved to disk (not just transcription)
- [ ] Audio files are stored in a dedicated directory (e.g., `~/.local/share/voice-dictation/recordings/`)
- [ ] Recordings can be played back from the application
- [ ] Speaker diarization identifies different speakers in the conversation
- [ ] Transcription includes speaker labels (e.g., "Speaker 1: текст", "Speaker 2: текст")
- [ ] User can see list of saved recordings with metadata (date, duration, participants count)
- [ ] User can delete old recordings to free up space
- [ ] Conference mode works on both X11 and Wayland (loopback recording support)

## Technical Details

### Implementation Approach

#### 1. Dual Audio Recording
Extend `AudioRecorder` in [src/audio.rs](src/audio.rs) to support recording from two sources simultaneously:
- **Input device**: Microphone (existing functionality via CPAL)
- **Output device**: Loopback/monitor source (new, via PipeWire)

**Challenges:**
- Loopback recording on Linux requires access to PipeWire monitor sources
- ❌ CPAL doesn't see monitor sources on Linux (tested on Fedora 41)
- Synchronization between two audio streams (timing alignment)

**Solution — Decided (ADR-003, 2026-01-28):**

| Component | Library | Notes |
|-----------|---------|-------|
| Microphone | CPAL (existing) | Works, no changes needed |
| Loopback | **`pipewire` crate v0.9** | Direct access to monitor sources |

**Implementation approach:**
```rust
// Microphone: existing CPAL code (src/audio.rs)
let mic_device = host.default_input_device()?;

// Loopback: new PipeWire code (src/loopback.rs)
use pipewire as pw;
let props = pw::properties! {
    *pw::keys::MEDIA_TYPE => "Audio",
    *pw::keys::MEDIA_CATEGORY => "Capture",
    *pw::keys::NODE_TARGET => "alsa_output.*.monitor",
};
let stream = pw::stream::Stream::new(&core, "loopback", props)?;
```

**System requirements:**
```bash
# Fedora
sudo dnf install pipewire-devel

# Ubuntu/Debian
sudo apt install libpipewire-0.3-dev
```

See [ADR-003](../adr/003-loopback-recording-approach.md) for full details.

#### 2. Audio Stream Merging
Merge two mono streams into a single stereo or mono file:
- **Option A**: Stereo file (L = input, R = output)
- **Option B**: Mixed mono (sum both channels with optional gain control)
- **Option C**: Keep separate but synchronized (for better speaker diarization)

**Recommended**: Option C initially (separate streams), then mix for playback. This preserves audio quality and helps with speaker diarization.

#### 3. Audio File Storage
- Use audio encoding library (e.g., `symphonia`, `rodio`, or `hound` for WAV)
- Save as WAV (uncompressed) or OGG/MP3 (compressed)
- File naming: `conference_YYYY-MM-DD_HH-MM-SS.wav`
- Store metadata: duration, sample rate, channels, file size
- Location: `~/.local/share/voice-dictation/recordings/`

**Dependencies:**
- Audio encoding: `hound` (WAV) or `symphonia` (multiple formats)
- File I/O: existing `std::fs`

#### 4. Speaker Diarization
Identify different speakers in the conversation.

**Important**: Whisper **does not support** speaker diarization natively. See detailed research in [ADR-004: Speaker Diarization Implementation Approach](../adr/004-speaker-diarization-approach.md).

**Solution — Decided (ADR-004, 2026-01-28):**

| Phase | Approach | Speakers | Library |
|-------|----------|----------|---------|
| **MVP** | Channel-Based | 2 (fixed) | None |
| **Production** | Sortformer ⭐ | 2-4 | `parakeet-rs` |
| Optional | pyannote-rs | 5+ | `pyannote-rs` |

**Phase 1: Channel-Based (MVP)**
- Use separate channels (microphone vs system audio) as speaker identifiers
- Label as "[Ви]" for microphone, "[Учасник]" for system audio
- 100% accuracy for 2 speakers, zero dependencies

**Phase 2: Sortformer (Production) ⭐ RECOMMENDED**
- NVIDIA Streaming Sortformer — SOTA model (2025)
- Real-time streaming diarization
- Up to 4 speakers
- Fast on CPU

```rust
use parakeet_rs::sortformer::{Sortformer, DiarizationConfig};

let mut sortformer = Sortformer::with_config(
    "diar_streaming_sortformer_4spk-v2.onnx",
    None,
    DiarizationConfig::callhome(),
)?;

let segments = sortformer.diarize(audio, 16000, 1)?;
```

**Phase 3: pyannote-rs (Optional)**
- Only if >4 speakers needed
- Batch processing (not streaming)

**Dependencies:**
```toml
# MVP: none

# Production (optional feature):
parakeet-rs = { version = "0.3", features = ["sortformer"], optional = true }

# Advanced (optional feature):
pyannote-rs = { version = "0.3", optional = true }
```

**Model licenses:**
- Sortformer: CC-BY-4.0 (NVIDIA attribution required)
- pyannote: MIT

#### 5. Transcription with Speaker Labels
Extend Whisper transcription to include speaker information.

**MVP (Channel-Based):**
```
[Ви] Привіт, як справи?
[Учасник] Добре, дякую. А у тебе?
[Ви] Теж добре. Почнемо?
```

**Production (Sortformer):**
```
[Speaker 1] Привіт, як справи?
[Speaker 2] Добре, дякую. А у тебе?
[Speaker 1] Теж добре. Почнемо?
[Speaker 3] Я також готовий.
```

**Implementation:**
1. Diarization runs first (Sortformer or channel-based)
2. Audio segmented by speaker
3. Each segment transcribed with Whisper
4. Results merged with timestamps and speaker labels

See [ADR-004](../adr/004-speaker-diarization-approach.md) for detailed implementation approaches.

### Code Structure (Updated 2026-01-28)

#### New Module: `src/loopback.rs`
PipeWire-based loopback recording.
```rust
use pipewire as pw;

pub struct LoopbackRecorder {
    samples: Arc<Mutex<Vec<f32>>>,
    stream: Option<pw::stream::Stream>,
}

impl LoopbackRecorder {
    pub fn new() -> Result<Self>;
    pub fn start_recording(&mut self, monitor_source: &str) -> Result<()>;
    pub fn stop_recording(&mut self) -> Vec<f32>;
}
```

#### New Module: `src/conference_recorder.rs`
Dual recording combining CPAL (mic) + PipeWire (loopback).
```rust
pub struct ConferenceRecorder {
    mic_recorder: AudioRecorder,      // Existing CPAL-based
    loopback_recorder: LoopbackRecorder, // New PipeWire-based
    start_time: Instant,
}

impl ConferenceRecorder {
    pub fn start_recording(&mut self) -> Result<()> {
        self.start_time = Instant::now();
        self.mic_recorder.start_recording()?;
        self.loopback_recorder.start_recording("auto")?; // auto-detect monitor
        Ok(())
    }

    pub fn stop_recording(&mut self) -> ConferenceAudio {
        let mic_samples = self.mic_recorder.stop_recording();
        let loopback_samples = self.loopback_recorder.stop_recording();
        ConferenceAudio { mic_samples, loopback_samples }
    }
}
```

#### New Module: `src/diarization.rs`
Speaker diarization with multiple backends.
```rust
pub enum DiarizationMode {
    ChannelBased,  // MVP: mic = "Ви", loopback = "Учасник"
    Streaming,     // Production: parakeet-rs Sortformer
    Advanced,      // Optional: pyannote-rs
}

pub struct Diarizer {
    mode: DiarizationMode,
    #[cfg(feature = "sortformer")]
    sortformer: Option<Sortformer>,
}
```

#### New Module: `src/recordings.rs`
```rust
pub struct Recording {
    pub id: String,
    pub file_path: PathBuf,
    pub duration_secs: f32,
    pub created_at: DateTime<Utc>,
    pub speakers: Vec<String>, // Speaker labels
}

pub struct RecordingsManager {
    recordings_dir: PathBuf,
}

impl RecordingsManager {
    pub fn list_recordings() -> Vec<Recording>;
    pub fn delete_recording(id: &str) -> Result<()>;
    pub fn get_recording_path(id: &str) -> Option<PathBuf>;
}
```

#### UI Changes: `src/ui.rs`
- Add mode selector (radio buttons or dropdown): "Диктовка" / "Конференція"
- Add "Записи" (Recordings) button to view saved conference recordings
- Show speaker labels in transcription result
- Add playback controls for saved recordings

#### New Dialog: `src/recordings_dialog.rs`
Similar to `history_dialog.rs`, but for audio recordings:
- List of recordings with metadata
- Play button (use `gtk4::MediaFile` or external player)
- Delete button
- Export transcription button

### Integration Points

1. **Audio Recording**: Extend [src/audio.rs](src/audio.rs) or create `ConferenceRecorder`
2. **Config**: Add `recording_mode: RecordingMode` enum to [src/config.rs](src/config.rs)
3. **UI**: Add mode selector in [src/ui.rs](src/ui.rs)
4. **Storage**: Create recordings directory structure
5. **Transcription**: Modify Whisper integration in [src/whisper.rs](src/whisper.rs) to handle speaker segments
6. **History**: Extend [src/history.rs](src/history.rs) to include recording file paths and speaker info

## Configuration

Add to `Config` struct:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecordingMode {
    Dictation,      // Current mode - microphone only
    Conference,     // New mode - microphone + system audio
}

pub struct Config {
    // ... existing fields
    pub recording_mode: RecordingMode,
    pub recordings_dir: PathBuf,  // Default: ~/.local/share/voice-dictation/recordings/
    pub max_recordings: usize,    // Limit number of saved recordings
    pub recording_format: String,  // "wav" or "ogg"
}
```

Add to `config.toml`:
```toml
recording_mode = "Dictation"  # or "Conference"
recordings_dir = "~/.local/share/voice-dictation/recordings"
max_recordings = 100
recording_format = "wav"
```

## Dependencies (Updated 2026-01-28)

### Required New Dependencies
```toml
# Loopback recording (required for conference mode)
pipewire = "0.9"

# Audio encoding
hound = "0.4"  # WAV format

# Optional: Speaker diarization
[features]
default = []
sortformer = ["parakeet-rs/sortformer"]
diarization-advanced = ["pyannote-rs"]

[dependencies.parakeet-rs]
version = "0.3"
features = ["sortformer"]
optional = true

[dependencies.pyannote-rs]
version = "0.3"
optional = true
```

### System Dependencies
```bash
# Fedora
sudo dnf install pipewire-devel

# Ubuntu/Debian
sudo apt install libpipewire-0.3-dev

# Models (downloaded on first use)
# Sortformer: nvidia/diar_streaming_sortformer_4spk-v2 (~50MB, CC-BY-4.0)
# pyannote: segmentation-3.0 + wespeaker (~100-200MB, MIT)
```

### Compatibility
- ✅ PipeWire systems: Fedora 34+, Ubuntu 22.04+, Arch, etc.
- ⚠️ Pure PulseAudio: Fallback needed (pulse-binding-rs)
- ❌ Pure ALSA: Not supported

## Technical Challenges (Updated 2026-01-28)

1. **Loopback Recording on Linux** ✅ RESOLVED
   - ❌ CPAL doesn't see monitor sources (tested)
   - ✅ Solution: Use `pipewire` crate directly
   - See [ADR-003](../adr/003-loopback-recording-approach.md)

2. **Synchronization** ⏳ TODO
   - Two audio streams may have different latencies
   - Need timestamp-based alignment
   - Solution: Use system timestamps and align during merge

3. **Speaker Diarization** ✅ RESOLVED
   - ✅ MVP: Channel-based (mic vs loopback) — 100% accuracy for 2 speakers
   - ✅ Production: Sortformer (parakeet-rs) — SOTA, up to 4 speakers
   - ✅ Optional: pyannote-rs — unlimited speakers
   - See [ADR-004](../adr/004-speaker-diarization-approach.md)

4. **File Size** ⏳ TODO
   - Uncompressed WAV files are large (1 hour ≈ 500MB)
   - Consider compression (OGG/MP3)
   - Solution: Make format configurable, default to compressed

5. **Model Attribution** ⚠️ NEW
   - Sortformer models are CC-BY-4.0 (NVIDIA)
   - Must include attribution in app/docs

## Priority
**P2** - Medium (research completed, ready for implementation)

## Status
🚀 **Ready for Implementation** - ADR-003 and ADR-004 research completed (2026-01-28)

## Implementation Phases

### Phase 1: MVP (Channel-Based)
- [ ] Create `src/loopback.rs` with PipeWire capture
- [ ] Create `src/conference_recorder.rs` (mic + loopback)
- [ ] Add mode selector UI: "Диктовка" / "Конференція"
- [ ] Channel-based diarization: `[Ви]` / `[Учасник]`
- [ ] Save audio files to disk

**Dependencies:** `pipewire = "0.9"`, `hound = "0.4"`

### Phase 2: Production (Sortformer)
- [ ] Add `parakeet-rs` with Sortformer feature
- [ ] Create `src/diarization.rs` with streaming diarization
- [ ] Support up to 4 speakers
- [ ] Add recordings management UI

**Dependencies:** `parakeet-rs = { version = "0.3", features = ["sortformer"] }`

### Phase 3: Polish
- [ ] Recordings playback with speaker highlighting
- [ ] Export to SRT subtitles
- [ ] Compression options (OGG/MP3)
- [ ] Optional pyannote-rs for >4 speakers

## Related Files

### Source Code
- [src/audio.rs](../../src/audio.rs) - Current audio recording (CPAL, mic only)
- [src/config.rs](../../src/config.rs) - Configuration structure
- [src/ui.rs](../../src/ui.rs) - Main UI where mode selector will be added
- [src/whisper.rs](../../src/whisper.rs) - Whisper integration for transcription
- [src/history.rs](../../src/history.rs) - History management (extend for recordings)

### New Modules (to be created)
- `src/loopback.rs` - PipeWire loopback recording
- `src/conference_recorder.rs` - Dual recording (mic + loopback)
- `src/diarization.rs` - Speaker diarization
- `src/recordings.rs` - Recordings management
- `src/recordings_dialog.rs` - Recordings UI

### ADRs
- [ADR-003](../adr/003-loopback-recording-approach.md) - Loopback recording ✅ Accepted
- [ADR-004](../adr/004-speaker-diarization-approach.md) - Speaker diarization ✅ Accepted

### Research
- [loopback-recording-test.md](../research/loopback-recording-test.md) - Test results
- [speaker-diarization-test.md](../research/speaker-diarization-test.md) - Diarization comparison

## Future Enhancements

### Now Possible (with current tech stack)
- ✅ Multiple speaker identification (up to 4 with Sortformer)
- ✅ Real-time transcription during recording (Sortformer is streaming)
- ⏳ Unlimited speakers (pyannote-rs optional feature)

### Planned
- Automatic speaker naming (learn names from transcription)
- Export to various formats (SRT subtitles, etc.)
- Playback with speaker highlighting

### Future
- Cloud sync for recordings
- Integration with calendar apps to auto-record meetings
- GPU acceleration (CUDA, CoreML)
