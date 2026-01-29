pub mod whisper;
#[cfg(feature = "tdt")]
pub mod tdt;
pub mod service;
pub mod diarization;

pub(crate) use whisper::WhisperSTT;
#[cfg(feature = "tdt")]
pub(crate) use tdt::ParakeetSTT;
pub use service::TranscriptionService;
