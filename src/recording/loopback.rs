use anyhow::{Context, Result};
use async_channel::Receiver;
use parking_lot::Mutex;
use pipewire as pw;
use pw::spa;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::thread;

use super::core::{calculate_rms, RecordingCore, WHISPER_SAMPLE_RATE};

pub(crate) struct LoopbackRecorder {
    core: RecordingCore,
    /// Channel sender to signal the PipeWire MainLoop to quit.
    /// Stored between start/stop calls; `None` when not recording.
    stop_sender: Mutex<Option<pw::channel::Sender<()>>>,
}

impl LoopbackRecorder {
    pub fn new() -> Self {
        Self {
            core: RecordingCore::new(),
            stop_sender: Mutex::new(None),
        }
    }

    /// Get current audio amplitude (0.0 - 1.0 range, normalized RMS)
    pub fn get_amplitude(&self) -> f32 {
        self.core.get_amplitude()
    }

    /// Start recording system audio via native PipeWire Stream API.
    ///
    /// Creates a capture stream targeting the default monitor source
    /// (sink output). Audio is received as F32LE mono 16 kHz — the format
    /// Whisper expects — with PipeWire handling resampling automatically.
    pub fn start_loopback(&self) -> Result<()> {
        let handles = self.core.prepare_recording();

        let samples = handles.samples;
        let current_amplitude = handles.current_amplitude;
        let completion_tx = handles.completion_tx;

        // Discover the monitor source name via pactl (works on both
        // PipeWire and PulseAudio systems). The name is used as the
        // target.object property so PipeWire routes audio from the
        // right sink.
        let target_node = find_monitor_source();

        // Create a pw::channel so we can signal the PipeWire MainLoop
        // to quit from stop_loopback() on another thread.
        let (pw_sender, pw_receiver) = pw::channel::channel::<()>();
        *self.stop_sender.lock() = Some(pw_sender);

        thread::spawn(move || {
            pw::init();

            if let Err(e) =
                run_pipewire_capture(&target_node, pw_receiver, samples, current_amplitude)
            {
                eprintln!("PipeWire loopback error: {e:#}");
            }

            let _ = completion_tx.send_blocking(());
        });

        Ok(())
    }

    pub fn stop_loopback(&self) -> (Vec<f32>, Option<Receiver<()>>) {
        // Signal the PipeWire MainLoop to quit.
        if let Some(sender) = self.stop_sender.lock().take() {
            let _ = sender.send(());
        }
        self.core.stop()
    }
}

impl Default for LoopbackRecorder {
    fn default() -> Self {
        Self::new()
    }
}

// === Trait Implementation ===

use crate::domain::traits::AudioRecording;

impl AudioRecording for LoopbackRecorder {
    fn start(&self) -> Result<()> {
        self.start_loopback()
    }

    fn stop(&self) -> (Vec<f32>, Option<Receiver<()>>) {
        self.stop_loopback()
    }

    fn amplitude(&self) -> f32 {
        self.get_amplitude()
    }

    fn is_recording(&self) -> bool {
        self.core.is_recording()
    }
}

// ─── PipeWire internals ────────────────────────────────────────────────

/// User data passed into the PipeWire stream process callback.
struct CaptureData {
    samples: Arc<Mutex<Vec<f32>>>,
    current_amplitude: Arc<AtomicU32>,
}

/// Run the PipeWire MainLoop with a capture stream.
///
/// This function blocks until the MainLoop is quit (via `pw_receiver`).
fn run_pipewire_capture(
    target_node: &str,
    pw_receiver: pw::channel::Receiver<()>,
    samples: Arc<Mutex<Vec<f32>>>,
    current_amplitude: Arc<AtomicU32>,
) -> Result<()> {
    let mainloop =
        pw::main_loop::MainLoopRc::new(None).context("Failed to create PipeWire MainLoop")?;
    let context = pw::context::ContextRc::new(&mainloop, None)
        .context("Failed to create PipeWire Context")?;
    let core = context
        .connect_rc(None)
        .context("Failed to connect to PipeWire daemon")?;

    // Attach the stop-signal receiver: when stop_loopback() sends (),
    // we quit the main loop.
    let _stop_receiver = pw_receiver.attach(mainloop.loop_(), {
        let mainloop = mainloop.clone();
        move |()| mainloop.quit()
    });

    // Build stream properties.
    let mut props = pw::properties::properties! {
        *pw::keys::MEDIA_TYPE => "Audio",
        *pw::keys::MEDIA_CATEGORY => "Capture",
        *pw::keys::MEDIA_ROLE => "Music",
        // Capture from the sink's monitor ports (system audio output).
        *pw::keys::STREAM_CAPTURE_SINK => "true",
    };
    if !target_node.is_empty() {
        props.insert(*pw::keys::TARGET_OBJECT, target_node);
    }

    let stream = pw::stream::StreamBox::new(&core, "s2t-loopback", props)
        .context("Failed to create PipeWire stream")?;

    let user_data = CaptureData {
        samples,
        current_amplitude,
    };

    // Register the stream listener with our process callback.
    let _listener = stream
        .add_local_listener_with_user_data(user_data)
        .process(|stream, data| {
            process_audio_buffer(stream, data);
        })
        .register()
        .context("Failed to register PipeWire stream listener")?;

    // Build format parameters: F32LE mono 16 kHz.
    let mut audio_info = spa::param::audio::AudioInfoRaw::new();
    audio_info.set_format(spa::param::audio::AudioFormat::F32LE);
    audio_info.set_rate(WHISPER_SAMPLE_RATE);
    audio_info.set_channels(1);

    let obj = spa::pod::Object {
        type_: spa::utils::SpaTypes::ObjectParamFormat.as_raw(),
        id: spa::param::ParamType::EnumFormat.as_raw(),
        properties: audio_info.into(),
    };
    let values: Vec<u8> = spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &spa::pod::Value::Object(obj),
    )
    .map_err(|e| anyhow::anyhow!("Failed to serialize audio format pod: {e:?}"))?
    .0
    .into_inner();

    let mut params =
        [spa::pod::Pod::from_bytes(&values)
            .context("Failed to create Pod from serialized bytes")?];

    // Connect the stream for input capture.
    // We intentionally omit RT_PROCESS so the callback runs on the
    // MainLoop thread, allowing safe Mutex usage in process_audio_buffer.
    stream
        .connect(
            spa::utils::Direction::Input,
            None,
            pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
            &mut params,
        )
        .context("Failed to connect PipeWire stream")?;

    // Block until quit() is called.
    mainloop.run();

    Ok(())
}

/// Extract audio samples from a PipeWire buffer and store them.
fn process_audio_buffer(stream: &pw::stream::Stream, data: &mut CaptureData) {
    let Some(mut buffer) = stream.dequeue_buffer() else {
        return;
    };

    let datas = buffer.datas_mut();
    if datas.is_empty() {
        return;
    }

    let d = &mut datas[0];
    let chunk = d.chunk();
    let n_bytes = chunk.size() as usize;
    let n_samples = n_bytes / std::mem::size_of::<f32>();

    if n_samples == 0 {
        return;
    }

    let Some(raw) = d.data() else {
        return;
    };

    // Reinterpret the raw byte slice as f32 samples.
    // Safety: We requested F32LE format from PipeWire and the chunk
    // size is already validated to be a multiple of sizeof(f32).
    let audio_samples: &[f32] =
        unsafe { std::slice::from_raw_parts(raw.as_ptr() as *const f32, n_samples) };

    // Update amplitude for the UI level meter.
    let amplitude = calculate_rms(audio_samples);
    data.current_amplitude
        .store(amplitude.to_bits(), Ordering::Relaxed);

    // Append samples to the shared buffer.
    data.samples.lock().extend_from_slice(audio_samples);
}

/// Discover the first PulseAudio/PipeWire monitor source via `pactl`.
///
/// Returns the node name (e.g. `alsa_output.pci-0000_00_1f.3.analog-stereo`)
/// without the `.monitor` suffix, since `target.object` + `stream.capture.sink`
/// is the correct way to target monitor ports in PipeWire.
fn find_monitor_source() -> String {
    std::process::Command::new("pactl")
        .args(["list", "sources", "short"])
        .output()
        .ok()
        .and_then(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .find(|line| line.contains(".monitor"))
                .and_then(|line| line.split_whitespace().nth(1))
                .map(|name| name.trim_end_matches(".monitor").to_string())
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_monitor_source_strips_suffix() {
        // Simulate what find_monitor_source does to the name
        let raw = "alsa_output.pci-0000_00_1f.3.analog-stereo.monitor";
        let stripped = raw.trim_end_matches(".monitor");
        assert_eq!(stripped, "alsa_output.pci-0000_00_1f.3.analog-stereo");
    }

    #[test]
    fn test_find_monitor_source_no_suffix() {
        let raw = "some_source_without_monitor";
        let stripped = raw.trim_end_matches(".monitor");
        assert_eq!(stripped, raw);
    }

    #[test]
    fn test_loopback_recorder_default() {
        let recorder = LoopbackRecorder::default();
        assert!(!recorder.is_recording());
        assert_eq!(recorder.get_amplitude(), 0.0);
    }

    #[test]
    fn test_stop_sender_initially_none() {
        let recorder = LoopbackRecorder::new();
        assert!(recorder.stop_sender.lock().is_none());
    }
}
