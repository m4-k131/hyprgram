use crate::Args;
use hyprgram::dev::{effective_spectrogram_history, SpectrogramDevConfig};
use hyprgram::spectrogram::SpectrogramProgram;
use hyprgram_core::{SampleRing, SpectrumConfig, SpectrumProcessor};
use iced::widget::container;
use iced::widget::shader::Shader;
use iced::{Element, Length, Size, Subscription, Task};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone)]
enum Message {
    Tick,
}

pub struct App {
    pub prog: SpectrogramProgram,
}

impl App {
    fn bootstrap(args: Args) -> Self {
        let rtl = !args.legacy_vertical_scroll;
        let history = effective_spectrogram_history(args.history, args.width, args.height, rtl);
        let backlog_cap = (history as usize).saturating_mul(8).saturating_add(256).max(1024);
        let pending_spectra = Arc::new(Mutex::new(VecDeque::new()));
        let pending_w = pending_spectra.clone();
        let ring = SampleRing::new((args.sample_rate as usize) * 2);
        let _pw = hyprgram_core::pipewire::spawn_capture(args.target_object.clone(), ring.clone());
        let mut cfg = SpectrumConfig::default();
        cfg.window_size = args.window;
        cfg.hop_size = args.hop;
        cfg.sample_rate = args.sample_rate;
        cfg.log_bins = args.log_bins;
        let mut proc = SpectrumProcessor::new(cfg).expect("spectrum processor");
        std::thread::spawn(move || {
            let mut scratch = vec![0.0f32; 65536];
            loop {
                let n = ring.pop_into(&mut scratch);
                if n == 0 {
                    std::thread::sleep(Duration::from_millis(2));
                    continue;
                }
                let mut cols = Vec::new();
                proc.push_samples(&scratch[..n], &mut cols);
                let mut q = pending_w.lock().unwrap();
                for c in cols {
                    while q.len() >= backlog_cap {
                        q.pop_front();
                    }
                    q.push_back(c);
                }
            }
        });
        Self {
            prog: SpectrogramProgram {
                pending_spectra,
                bins: args.log_bins as u32,
                history,
                dev: SpectrogramDevConfig {
                    scroll_right_to_left: rtl,
                },
            },
        }
    }
}

fn update(_app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::Tick => Task::none(),
    }
}

fn view(app: &App) -> Element<'_, Message> {
    let sh = Shader::new(app.prog.clone()).width(Length::Fill).height(Length::Fill);
    container(sh).width(Length::Fill).height(Length::Fill).into()
}

fn subscription(_app: &App) -> Subscription<Message> {
    iced::time::every(std::time::Duration::from_millis(16)).map(|_| Message::Tick)
}

pub fn run(args: Args) -> anyhow::Result<()> {
    let size = Size::new(args.width as f32, args.height as f32);
    iced::application(move || App::bootstrap(args.clone()), update, view)
        .title("hyprgram")
        .window_size(size)
        .centered()
        .subscription(subscription)
        .theme(iced::Theme::Dark)
        .run()
        .map_err(|e| anyhow::anyhow!("{e:?}"))
}
