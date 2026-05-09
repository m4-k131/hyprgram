use anyhow::Result;
use clap::Parser;

#[cfg(target_os = "linux")]
mod linux;

#[derive(Parser, Debug, Clone)]
#[command(name = "hyprgram", about = "PipeWire live spectrogram (Wayland window)")]
pub struct Args {
    #[arg(long, help = "PipeWire target object id or name for capture stream")]
    pub target_object: Option<String>,
    #[arg(long, help = "Built-in profile: laptop, default, foobar-like")]
    pub profile: Option<String>,
    #[arg(long, help = "Path to a TOML profile file")]
    pub config: Option<std::path::PathBuf>,
    #[arg(long, help = "Override: number of log-spaced frequency bins")]
    pub log_bins: Option<usize>,
    #[arg(
        long = "fft",
        alias = "window",
        help = "Override: FFT window length (samples)"
    )]
    pub window: Option<usize>,
    #[arg(
        long = "hop",
        help = "Override: STFT hop (samples)"
    )]
    pub hop: Option<usize>,
    #[arg(long = "window-fn", help = "Override: window function (hann, hamming, blackman, blackman-harris)")]
    pub window_fn: Option<String>,
    #[arg(long, help = "Override: window width (px)")]
    pub width: Option<u32>,
    #[arg(long, help = "Override: window height (px)")]
    pub height: Option<u32>,
    #[arg(long, default_value_t = 512, help = "Time rows in waterfall")]
    pub history: u32,
    #[arg(long, help = "Override: sample rate (Hz)")]
    pub sample_rate: Option<u32>,
    #[arg(long, help = "Scroll time top-to-bottom instead of right-to-left")]
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
