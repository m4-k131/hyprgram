//! Shared between `hyprgram` and dev binaries (e.g. sine preview).
#[cfg(target_os = "linux")]
pub mod dev;
#[cfg(target_os = "linux")]
pub mod spectrogram;
