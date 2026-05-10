//! hyprgram-core: DSP, ring buffer, and (Linux) PipeWire capture.

pub mod colormap;
pub mod dsp;
pub mod error;
#[cfg(target_os = "linux")]
pub mod pipewire;
pub mod render;
pub mod ring;
pub mod profiles;

pub use dsp::{
    normalize_hop_size, BandAggregation, SpectrumConfig, SpectrumProcessor, Transform, Weighting,
    WindowFunction, DEFAULT_FFT_HOP_SAMPLES, DEFAULT_FFT_WINDOW_SAMPLES,
};
pub use error::CoreError;
pub use render::{render_spectrogram_png, samples_to_spectrogram, SpectrogramImageConfig};
pub use ring::SampleRing;
