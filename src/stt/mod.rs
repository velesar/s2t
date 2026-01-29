//! STT (Speech-to-Text) capability providers.
//!
//! Groups all STT backend implementations behind the `Transcription` trait.

pub mod whisper;

#[cfg(feature = "tdt")]
pub mod tdt;

pub(crate) use whisper::WhisperSTT;

#[cfg(feature = "tdt")]
pub(crate) use tdt::ParakeetSTT;
