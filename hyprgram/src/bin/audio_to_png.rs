use anyhow::{Context, Result};
use clap::Parser;
use hyprgram_core::{
    render_spectrogram_png, samples_to_spectrogram, SpectrumConfig, SpectrogramImageConfig,
    DEFAULT_FFT_HOP_SAMPLES, DEFAULT_FFT_WINDOW_SAMPLES,
};
use std::fs::File;
use std::path::{Path, PathBuf};
use symphonia::core::audio::{AudioBufferRef, SampleBuffer};
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::default::{get_codecs, get_probe};

#[derive(Parser, Debug)]
#[command(about = "Render a WAV/MP3 audio file to a hyprgram spectrogram PNG")]
struct Args {
    input: PathBuf,
    output: PathBuf,
    #[arg(long, default_value_t = 256)]
    log_bins: usize,
    #[arg(long = "fft", alias = "window", default_value_t = DEFAULT_FFT_WINDOW_SAMPLES)]
    window: usize,
    #[arg(long, default_value_t = DEFAULT_FFT_HOP_SAMPLES)]
    hop: usize,
    #[arg(long, default_value_t = 800)]
    width: u32,
    #[arg(long, default_value_t = 200)]
    height: u32,
    #[arg(long, help = "Render time top-to-bottom instead of left-to-right")]
    legacy_vertical_scroll: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let (samples, sample_rate) = decode_mono_f32(&args.input)?;
    let mut spectrum = SpectrumConfig::default();
    spectrum.window_size = args.window;
    spectrum.hop_size = args.hop;
    spectrum.sample_rate = sample_rate;
    spectrum.log_bins = args.log_bins;
    let columns = samples_to_spectrogram(&samples, spectrum.clone())?;
    let image_config = SpectrogramImageConfig {
        spectrum,
        width: args.width,
        height: args.height,
        scroll_right_to_left: !args.legacy_vertical_scroll,
    };
    render_spectrogram_png(&columns, &image_config, &args.output)?;
    Ok(())
}

fn decode_mono_f32(path: &Path) -> Result<(Vec<f32>, u32)> {
    let file = File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .context("failed to probe audio format")?;
    let mut format = probed.format;
    let track = format.default_track().context("missing default audio track")?;
    let codec_params = &track.codec_params;
    let sample_rate = codec_params.sample_rate.context("missing sample rate")?;
    let track_id = track.id;
    let mut decoder = get_codecs()
        .make(codec_params, &DecoderOptions::default())
        .context("failed to create audio decoder")?;
    let mut samples = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(Error::IoError(_)) | Err(Error::ResetRequired) => break,
            Err(err) => return Err(err).context("failed to read audio packet"),
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(audio) => push_mono_samples(audio, &mut samples),
            Err(Error::DecodeError(_)) => continue,
            Err(err) => return Err(err).context("failed to decode audio packet"),
        }
    }
    Ok((samples, sample_rate))
}

fn push_mono_samples(audio: AudioBufferRef<'_>, out: &mut Vec<f32>) {
    let spec = *audio.spec();
    let channels = spec.channels.count();
    if channels == 0 {
        return;
    }
    let mut buffer = SampleBuffer::<f32>::new(audio.capacity() as u64, spec);
    buffer.copy_interleaved_ref(audio);
    for frame in buffer.samples().chunks(channels) {
        let mut sum = 0.0;
        for sample in frame {
            sum += *sample;
        }
        out.push(sum / frame.len() as f32);
    }
}
