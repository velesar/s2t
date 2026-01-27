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
- **Input device**: Microphone (existing functionality)
- **Output device**: Loopback/playback device (new)

**Challenges:**
- Loopback recording on Linux requires access to PulseAudio/PipeWire monitor sources
- Different audio systems: ALSA, PulseAudio, PipeWire
- Synchronization between two audio streams (timing alignment)
- CPAL doesn't have native loopback support on Linux (see [ADR-003](../adr/003-loopback-recording-approach.md))

**Solution:**
See detailed research and recommendations in [ADR-003: Loopback Recording Implementation Approach](../adr/003-loopback-recording-approach.md).

**Recommended approach (MVP):**
- Try using CPAL to enumerate devices and find monitor sources (names containing ".monitor")
- If CPAL doesn't see monitor sources, use direct PulseAudio API via `pulse-binding-rs`
- For PipeWire systems, use PipeWire bindings or PulseAudio compatibility layer
- Create separate recording threads for input and output
- Use timestamps to synchronize streams during merge

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

**Recommended approaches:**

1. **Channel-Based (MVP - Recommended)**: 
   - Use separate channels (microphone vs system audio) as speaker identifiers
   - Label as "Ви" (You) for microphone input and "Учасник" (Participant) for system audio
   - Simplest implementation, works immediately
   - Sufficient for basic conference case (2 speakers)

2. **pyannote-rs (Advanced - Optional)**:
   - Use `pyannote-rs` Rust crate for full speaker diarization
   - Identifies multiple speakers in system audio
   - Requires model download (~100-200MB)
   - Better for complex scenarios with 3+ speakers

**Recommended**: Start with Channel-Based for MVP, add pyannote-rs as optional advanced feature.

**Dependencies:**
- Channel-Based: None (uses existing infrastructure)
- Advanced: `pyannote-rs = "0.3"` or `native-pyannote-rs = "0.3"`

#### 5. Transcription with Speaker Labels
Extend Whisper transcription to include speaker information.

**Channel-Based approach:**
- Transcribe microphone and system audio separately
- Combine with speaker labels: `[Ви] текст` and `[Учасник] текст`
- Simple concatenation with timestamps

**Advanced approach (with pyannote-rs):**
- Segment audio by speaker using diarization
- Transcribe each segment with Whisper
- Combine with speaker labels: `[Speaker 1]`, `[Speaker 2]`, etc.
- More sophisticated, handles multiple speakers in system audio

See [ADR-004](../adr/004-speaker-diarization-approach.md) for detailed implementation approaches.

### Code Structure

#### New Module: `src/conference_recorder.rs`
```rust
pub struct ConferenceRecorder {
    input_samples: Arc<Mutex<Vec<f32>>>,
    output_samples: Arc<Mutex<Vec<f32>>>,
    // ... similar to AudioRecorder
}

impl ConferenceRecorder {
    pub fn start_recording(&self) -> Result<()> {
        // Start input recording (microphone)
        // Start output recording (loopback)
        // Synchronize timestamps
    }
    
    pub fn stop_recording(&self) -> (Vec<f32>, Vec<f32>, Option<Receiver<()>>) {
        // Return both streams
    }
    
    pub fn save_to_file(&self, input: &[f32], output: &[f32], path: &Path) -> Result<()> {
        // Merge and encode to audio file
    }
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

## Dependencies

### Required New Dependencies
- **Audio encoding**: `hound = "0.4"` (WAV) or `symphonia = "0.5"` (multiple formats)
- **Optional**: `webrtc-vad = "0.4"` for voice activity detection
- **Optional**: `rodio = "0.17"` for audio playback in UI

### System Dependencies
- PulseAudio or PipeWire for loopback recording
- May need additional permissions for accessing system audio

## Technical Challenges

1. **Loopback Recording on Linux**:
   - PulseAudio: Use monitor sources (`pactl list sources`)
   - PipeWire: Similar monitor sources
   - ALSA: More complex, may need `alsa-utils`
   - Solution: Detect audio system and use appropriate API

2. **Synchronization**:
   - Two audio streams may have different latencies
   - Need timestamp-based alignment
   - Solution: Use system timestamps and align during merge

3. **Speaker Diarization**:
   - Complex ML problem
   - Initial simple solution: channel-based (input vs output)
   - Future: ML-based diarization for multiple speakers

4. **File Size**:
   - Uncompressed WAV files are large (1 hour ≈ 500MB)
   - Consider compression (OGG/MP3)
   - Solution: Make format configurable, default to compressed

## Priority
P3 - Future (complex feature, requires ADR-003 and ADR-004 research)

## Status
📋 **Planned** - Requires loopback recording research (ADR-003) and speaker diarization (ADR-004)

## Related Files
- [src/audio.rs](../../src/audio.rs) - Current audio recording implementation
- [src/config.rs](../../src/config.rs) - Configuration structure
- [src/ui.rs](../../src/ui.rs) - Main UI where mode selector will be added
- [src/whisper.rs](../../src/whisper.rs) - Whisper integration for transcription
- [src/history.rs](../../src/history.rs) - History management (may extend for recordings)
- [src/history_dialog.rs](../../src/history_dialog.rs) - Reference for recordings dialog UI

## Future Enhancements
- Multiple speaker identification (more than 2 speakers)
- Automatic speaker naming (learn names from transcription)
- Real-time transcription during recording
- Export to various formats (SRT subtitles, etc.)
- Cloud sync for recordings
- Integration with calendar apps to auto-record meetings
