use crate::spectrogram::SpectrogramProgram;
use crate::Args;
use hyprgram_core::{SampleRing, SpectrumConfig, SpectrumProcessor};
use iced::widget::shader::Shader;
use iced::widget::container;
use iced::{Color, Element, Length, Task};
use iced_layershell::reexport::{Anchor, Layer};
use iced_layershell::settings::{LayerShellSettings, Settings, StartMode};
use iced_layershell::{application, to_layer_message};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[to_layer_message]
#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    IcedEvent(iced::Event),
}

pub struct App {
    rx: std::sync::mpsc::Receiver<Vec<f32>>,
    pub prog: SpectrogramProgram,
}

impl App {
    fn bootstrap(args: Args) -> Self {
        let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<f32>>(4);
        let column = Arc::new(Mutex::new(vec![0.0f32; args.log_bins]));
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
                for c in cols {
                    let _ = tx.try_send(c);
                }
            }
        });
        Self {
            rx,
            prog: SpectrogramProgram {
                column,
                bins: args.log_bins as u32,
                history: args.history,
            },
        }
    }
}

fn namespace() -> String {
    "hyprgram".to_string()
}

fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::Tick => {
            while let Ok(c) = app.rx.try_recv() {
                *app.prog.column.lock().unwrap() = c;
            }
            Task::none()
        }
        Message::IcedEvent(_) => Task::none(),
    }
}

fn view(app: &App) -> Element<Message> {
    let sh = Shader::new(app.prog.clone()).width(Length::Fill).height(Length::Fill);
    container(sh).width(Length::Fill).height(Length::Fill).into()
}

fn subscription(_app: &App) -> iced::Subscription<Message> {
    iced::Subscription::batch(vec![
        iced::time::every(Duration::from_millis(16)).map(|_| Message::Tick),
        iced::event::listen().map(Message::IcedEvent),
    ])
}

fn style(_app: &App, theme: &iced::Theme) -> iced::theme::Style {
    iced::theme::Style {
        background_color: Color::TRANSPARENT,
        text_color: theme.palette().text,
    }
}

pub fn run(args: Args) -> anyhow::Result<()> {
    let anchor = Anchor::Bottom | Anchor::Left | Anchor::Right;
    let ls = LayerShellSettings {
        anchor,
        layer: Layer::Background,
        exclusive_zone: 0,
        size: Some((args.width, args.height)),
        start_mode: StartMode::Active,
        ..Default::default()
    };
    let settings = Settings {
        layer_settings: ls,
        ..Settings::default()
    };
    application(move || App::bootstrap(args), namespace, update, view)
        .subscription(subscription)
        .settings(settings)
        .style(style)
        .run()
        .map_err(|e| anyhow::anyhow!("{e:?}"))
}
