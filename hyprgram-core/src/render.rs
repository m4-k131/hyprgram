use crate::{CoreError, SpectrumConfig, SpectrumProcessor};
use image::{ImageBuffer, Rgb};
use rayon::prelude::*;
use std::path::Path;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SpectrogramImageConfig {
    pub spectrum: SpectrumConfig,
    pub width: u32,
    pub height: u32,
    pub scroll_right_to_left: bool,
}

impl Default for SpectrogramImageConfig {
    fn default() -> Self {
        Self {
            spectrum: SpectrumConfig::default(),
            width: 800,
            height: 200,
            scroll_right_to_left: true,
        }
    }
}

pub fn samples_to_spectrogram(
    samples: &[f32],
    spectrum: SpectrumConfig,
) -> Result<Vec<Vec<f32>>, CoreError> {
    let n = samples.len();
    let w = spectrum.window_size;
    let h = spectrum.hop_size;
    if n < w {
        let mut processor = SpectrumProcessor::new(spectrum)?;
        let mut columns = Vec::new();
        processor.push_samples(samples, &mut columns);
        return Ok(columns);
    }
    let num_windows = (n - w) / h + 1;
    let num_threads = rayon::current_num_threads().max(1);
    let windows_per_chunk = (num_windows + num_threads - 1) / num_threads;
    let chunks: Vec<(usize, usize)> = (0..num_windows)
        .step_by(windows_per_chunk)
        .map(|start| {
            let end = (start + windows_per_chunk).min(num_windows);
            (start, end)
        })
        .collect();
    let results: Vec<Vec<Vec<f32>>> = chunks
        .par_iter()
        .map(|&(j_start, j_end)| {
            let sample_start = j_start * h;
            let sample_end = ((j_end - 1) * h + w).min(n);
            let mut processor = SpectrumProcessor::new(spectrum.clone())
                .expect("spectrum processor");
            let mut columns = Vec::with_capacity(j_end - j_start);
            processor.push_samples(&samples[sample_start..sample_end], &mut columns);
            columns
        })
        .collect();
    let total: Vec<Vec<f32>> = results.into_iter().flatten().collect();
    Ok(total)
}

pub fn render_spectrogram_png<P: AsRef<Path>>(
    columns: &[Vec<f32>],
    config: &SpectrogramImageConfig,
    output: P,
) -> Result<(), CoreError> {
    let width = config.width.max(1);
    let height = config.height.max(1);
    let bins = config.spectrum.log_bins.max(1);
    let mut img = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let value = if config.scroll_right_to_left {
                let col = sample_index(x, width, columns.len());
                let bin = sample_index(height - 1 - y, height, bins);
                sample_column(columns, col, bin)
            } else {
                let col = sample_index(y, height, columns.len());
                let bin = sample_index(x, width, bins);
                sample_column(columns, col, bin)
            };
            img.put_pixel(x, y, Rgb(viridis_u8(value)));
        }
    }
    img.save(output)
        .map_err(|e| CoreError::Dsp(format!("failed to write PNG: {e}")))?;
    Ok(())
}

fn sample_index(pos: u32, extent: u32, len: usize) -> usize {
    if len <= 1 {
        return 0;
    }
    ((pos as f32 / extent.saturating_sub(1).max(1) as f32) * (len - 1) as f32).round() as usize
}

fn sample_column(columns: &[Vec<f32>], col: usize, bin: usize) -> f32 {
    columns
        .get(col)
        .and_then(|c| c.get(bin))
        .copied()
        .unwrap_or(0.0)
}

fn viridis_u8(t: f32) -> [u8; 3] {
    let x = t.clamp(0.0, 1.0);
    [
        to_u8(-0.148 + x * (4.07 + x * (-6.86 + x * (4.83 - x * 1.37)))),
        to_u8(0.102 + x * (0.62 + x * (1.54 + x * (-3.44 + x * 2.02)))),
        to_u8(0.195 + x * (0.02 + x * (4.31 + x * (-7.02 + x * 3.24)))),
    ]
}

fn to_u8(x: f32) -> u8 {
    (x.clamp(0.0, 1.0) * 255.0).round() as u8
}
