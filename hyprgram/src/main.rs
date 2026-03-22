use anyhow::Result;
use clap::Parser;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
mod spectrogram;

#[derive(Parser, Debug, Clone)]
#[command(name = "hyprgram", about = "Hyprland PipeWire spectrogram layer shell widget")]
pub struct Args {
    #[arg(long, help = "PipeWire target object id or name for capture stream")]
    pub target_object: Option<String>,
    #[arg(long, default_value_t = 256)]
    pub log_bins: usize,
    #[arg(long, default_value_t = 2048)]
    pub window: usize,
    #[arg(long, default_value_t = 1024)]
    pub hop: usize,
    #[arg(long, default_value_t = 800)]
    pub width: u32,
    #[arg(long, default_value_t = 200)]
    pub height: u32,
    #[arg(long, default_value_t = 512)]
    pub history: u32,
    #[arg(long, default_value_t = 48000)]
    pub sample_rate: u32,
}

// Phase 4 manual verification (Linux/Hyprland):
// - Latency vs resolution: window/hop/sample-rate tradeoff (see hyprgram-core dsp defaults).
// - CPU: profile with perf; watch extra copies between PipeWire ring, DSP, and GPU upload.
// - Wayland: hyprctl reload vs compositor restart; on restart expect disconnect; layer shell should handle closed/output events per compositor.

fn main() -> Result<()> {
    let args = Args::parse();
    #[cfg(not(target_os = "linux"))]
    {
        let _ = args;
        anyhow::bail!("hyprgram requires Linux with Wayland, PipeWire, and a layer-shell compositor (e.g. Hyprland)");
    }
    #[cfg(target_os = "linux")]
    {
        linux::run(args)
    }
}
