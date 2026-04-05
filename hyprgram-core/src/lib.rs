//! hyprgram-core: DSP, ring buffer, and (Linux) PipeWire capture.

pub mod dsp;
pub mod error;
#[cfg(target_os = "linux")]
pub mod pipewire;
pub mod ring;

pub use dsp::{
    normalize_hop_size, SpectrumConfig, SpectrumProcessor, DEFAULT_FFT_HOP_SAMPLES,
    DEFAULT_FFT_WINDOW_SAMPLES,
};
pub use error::CoreError;
pub use ring::SampleRing;
