//! Normal window (not layer-shell): sine → same DSP + spectrogram shader as the main app.
//! Run: `cargo run -p hyprgram --bin sine_preview`
use clap::Parser;
use hyprgram::dev::{effective_spectrogram_history, SpectrogramDevConfig};
use hyprgram::spectrogram::SpectrogramProgram;
use hyprgram_core::{
    SpectrumConfig, SpectrumProcessor, DEFAULT_FFT_HOP_SAMPLES, DEFAULT_FFT_WINDOW_SAMPLES,
};
use iced::widget::container;
use iced::widget::shader::Shader;
use iced::{Element, Length, Size, Subscription, Task};
use std::collections::VecDeque;
use std::f32::consts::PI;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Parser, Debug, Clone)]
#[command(about = "Sine generator → spectrogram (for tuning visuals without PipeWire/Hyprland)")]
struct PreviewArgs {
    #[arg(long, default_value_t = 440.0)]
    freq_hz: f32,
    #[arg(long, default_value_t = 256)]
    log_bins: usize,
    #[arg(
        long = "fft",
        alias = "window",
        default_value_t = DEFAULT_FFT_WINDOW_SAMPLES,
        help = "Real FFT / STFT window length (samples)"
    )]
    window: usize,
    #[arg(long, default_value_t = DEFAULT_FFT_HOP_SAMPLES, help = "STFT hop (samples)")]
    hop: usize,
    #[arg(long, default_value_t = 800)]
    width: u32,
    #[arg(long, default_value_t = 200)]
    height: u32,
    #[arg(long, default_value_t = 512)]
    history: u32,
    #[arg(long, default_value_t = 48000)]
    sample_rate: u32,
    #[arg(long, help = "Scroll time top-to-bottom instead of right-to-left")]
    legacy_vertical_scroll: bool,
}

#[derive(Debug, Clone)]
enum Message {
    Tick,
}

struct Preview {
    proc: SpectrumProcessor,
    phase: f32,
    freq_hz: f32,
    sample_rate: u32,
    hop: usize,
    prog: SpectrogramProgram,
    scratch: Vec<f32>,
}

impl Preview {
    fn new(args: PreviewArgs) -> Self {
        let rtl = !args.legacy_vertical_scroll;
        let history = effective_spectrogram_history(args.history, args.width, args.height, rtl);
        let mut cfg = SpectrumConfig::default();
        cfg.window_size = args.window;
        cfg.hop_size = args.hop;
        cfg.sample_rate = args.sample_rate;
        cfg.log_bins = args.log_bins;
        let proc = SpectrumProcessor::new(cfg).expect("spectrum processor");
        Self {
            proc,
            phase: 0.0,
            freq_hz: args.freq_hz,
            sample_rate: args.sample_rate,
            hop: args.hop,
            prog: SpectrogramProgram {
                pending_spectra: Arc::new(Mutex::new(VecDeque::new())),
                bins: args.log_bins as u32,
                history,
                dev: SpectrogramDevConfig {
                    scroll_right_to_left: rtl,
                },
            },
            scratch: Vec::with_capacity(args.hop.max(1)),
        }
    }
}

fn update(p: &mut Preview, message: Message) -> Task<Message> {
    match message {
        Message::Tick => {
            let n = p.hop.max(1);
            p.scratch.resize(n, 0.0);
            let sr = p.sample_rate as f32;
            let step = 2.0 * PI * p.freq_hz / sr;
            for x in p.scratch.iter_mut() {
                *x = p.phase.sin();
                p.phase += step;
            }
            if p.phase > 2.0 * PI {
                p.phase -= 2.0 * PI * (p.phase / (2.0 * PI)).floor();
            }
            let mut cols = Vec::new();
            p.proc.push_samples(&p.scratch, &mut cols);
            let mut q = p.prog.pending_spectra.lock().unwrap();
            for c in cols {
                q.push_back(c);
            }
            Task::none()
        }
    }
}

fn view(p: &Preview) -> Element<'_, Message> {
    let sh = Shader::new(p.prog.clone()).width(Length::Fill).height(Length::Fill);
    container(sh).width(Length::Fill).height(Length::Fill).into()
}

fn subscription(_p: &Preview) -> Subscription<Message> {
    iced::time::every(Duration::from_millis(16)).map(|_| Message::Tick)
}

fn main() -> iced::Result {
    let args = PreviewArgs::parse();
    let size = Size::new(args.width as f32, args.height as f32);
    iced::application(move || Preview::new(args.clone()), update, view)
        .title("hyprgram sine preview")
        .window_size(size)
        .centered()
        .subscription(subscription)
        .theme(iced::Theme::Dark)
        .run()
}
