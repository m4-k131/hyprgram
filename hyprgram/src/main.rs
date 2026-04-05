use anyhow::Result;
use clap::Parser;

#[cfg(target_os = "linux")]
mod linux;

use hyprgram_core::{DEFAULT_FFT_HOP_SAMPLES, DEFAULT_FFT_WINDOW_SAMPLES};

#[derive(Parser, Debug, Clone)]
#[command(name = "hyprgram", about = "PipeWire live spectrogram (Wayland window)")]
pub struct Args {
    #[arg(long, help = "PipeWire target object id or name for capture stream")]
    pub target_object: Option<String>,
    #[arg(long, default_value_t = 256)]
    pub log_bins: usize,
    #[arg(
        long = "fft",
        alias = "window",
        default_value_t = DEFAULT_FFT_WINDOW_SAMPLES,
        help = "Real FFT / STFT window length (samples). Δf ≈ sample_rate / fft; larger ⇒ finer frequency resolution, more CPU/latency"
    )]
    pub window: usize,
    #[arg(
        long = "hop",
        default_value_t = DEFAULT_FFT_HOP_SAMPLES,
        help = "STFT hop (samples); must be ≤ fft, use 0 for half-window; larger values are clamped"
    )]
    pub hop: usize,
    #[arg(long, default_value_t = 800)]
    pub width: u32,
    #[arg(long, default_value_t = 200)]
    pub height: u32,
    #[arg(long, default_value_t = 512, help = "Time rows in waterfall; below --width/--height the effective value is raised so time can map ~1 texel per pixel")]
    pub history: u32,
    #[arg(long, default_value_t = 48000)]
    pub sample_rate: u32,
    #[arg(long, help = "Scroll time top-to-bottom instead of right-to-left (newest on the right)")]
    pub legacy_vertical_scroll: bool,
}

// Phase 4 manual verification (Linux/Wayland):
// - Latency vs resolution: window/hop/sample-rate tradeoff (see hyprgram-core dsp defaults).
// - CPU: profile with perf; watch extra copies between PipeWire ring, DSP, and GPU upload.

fn main() -> Result<()> {
    let args = Args::parse();
    #[cfg(not(target_os = "linux"))]
    {
        let _ = args;
        anyhow::bail!("hyprgram requires Linux with Wayland and PipeWire");
    }
    #[cfg(target_os = "linux")]
    {
        linux::run(args)
    }
}
