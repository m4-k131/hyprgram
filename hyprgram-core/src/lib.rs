//! hyprgram-core: DSP, ring buffer, and (Linux) PipeWire capture.

pub mod dsp;
pub mod error;
#[cfg(target_os = "linux")]
pub mod pipewire;
pub mod ring;

pub use dsp::{SpectrumConfig, SpectrumProcessor};
pub use error::CoreError;
pub use ring::SampleRing;
