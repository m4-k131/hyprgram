use crate::error::CoreError;
use realfft::{RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use std::f32::consts::PI;
use std::sync::Arc;

/// Default real FFT length: Hann STFT window size in samples. Frequency bin spacing ≈ `sample_rate / window_size`.
pub const DEFAULT_FFT_WINDOW_SAMPLES: usize = 20480;
/// Default hop between STFT frames (samples). ~5.3 ms @ 48 kHz — coarse enough for ~1/128-type timing vs CPU; raise `fft` / lower hop for heavier overlap.
pub const DEFAULT_FFT_HOP_SAMPLES: usize = 256;

/// STFT hop must satisfy `1 <= hop <= window_size`. `hop == 0` is treated as “use half window” (50% overlap). Larger values are clamped to `window_size`.
pub fn normalize_hop_size(window_size: usize, hop: usize) -> usize {
    if window_size < 1 {
        return 1;
    }
    let max_h = window_size;
    let h = if hop == 0 { (window_size / 2).max(1) } else { hop };
    h.clamp(1, max_h)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WindowFunction {
    #[default]
    Hann,
    Hamming,
    Blackman,
    BlackmanHarris,
}

impl WindowFunction {
    pub fn generate(&self, size: usize) -> Vec<f32> {
        let n = size.max(1);
        let n1 = (n - 1).max(1) as f32;
        match self {
            WindowFunction::Hann => {
                (0..n).map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / n1).cos())).collect()
            }
            WindowFunction::Hamming => {
                (0..n).map(|i| 0.53836 - 0.46164 * (2.0 * PI * i as f32 / n1).cos()).collect()
            }
            WindowFunction::Blackman => {
                (0..n).map(|i| {
                    let a = 2.0 * PI * i as f32 / n1;
                    0.42 - 0.5 * a.cos() + 0.08 * (2.0 * a).cos()
                }).collect()
            }
            WindowFunction::BlackmanHarris => {
                (0..n).map(|i| {
                    let a = 2.0 * PI * i as f32 / n1;
                    0.35875 - 0.48829 * a.cos() + 0.14128 * (2.0 * a).cos() - 0.01168 * (3.0 * a).cos()
                }).collect()
            }
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SpectrumConfig {
    pub window_size: usize,
    pub hop_size: usize,
    pub sample_rate: u32,
    pub log_bins: usize,
    pub f_min_hz: f32,
    pub f_max_hz: f32,
    pub db_floor: f32,
    pub db_ceil: f32,
    #[serde(default)]
    pub window_fn: WindowFunction,
}

impl Default for SpectrumConfig {
    fn default() -> Self {
        Self {
            window_size: DEFAULT_FFT_WINDOW_SAMPLES,
            hop_size: DEFAULT_FFT_HOP_SAMPLES,
            sample_rate: 48000,
            log_bins: 256,
            f_min_hz: 20.0,
            f_max_hz: 20000.0,
            db_floor: -80.0,
            db_ceil: 0.0,
            window_fn: WindowFunction::Hann,
        }
    }
}

pub struct SpectrumProcessor {
    cfg: SpectrumConfig,
    r2c: Arc<dyn RealToComplex<f32>>,
    window: Vec<f32>,
    work_input: Vec<f32>,
    spectrum: Vec<Complex<f32>>,
    pending: Vec<f32>,
}

impl SpectrumProcessor {
    pub fn new(mut cfg: SpectrumConfig) -> Result<Self, CoreError> {
        if cfg.window_size < 8 {
            return Err(CoreError::Dsp("window_size too small".into()));
        }
        cfg.hop_size = normalize_hop_size(cfg.window_size, cfg.hop_size);
        let mut planner = RealFftPlanner::<f32>::new();
        let r2c = planner.plan_fft_forward(cfg.window_size);
        let spectrum = r2c.make_output_vec();
        let work_input = r2c.make_input_vec();
        let window = cfg.window_fn.generate(cfg.window_size);
        let pending_cap = cfg.window_size * 2;
        Ok(Self {
            cfg,
            r2c,
            window,
            work_input,
            spectrum,
            pending: Vec::with_capacity(pending_cap),
        })
    }
    pub fn set_sample_rate(&mut self, sr: u32) {
        self.cfg.sample_rate = sr;
    }
    pub fn log_bins(&self) -> usize {
        self.cfg.log_bins
    }
    pub fn push_samples(&mut self, incoming: &[f32], out_columns: &mut Vec<Vec<f32>>) {
        self.pending.extend_from_slice(incoming);
        let w = self.cfg.window_size;
        let h = self.cfg.hop_size;
        out_columns.clear();
        while self.pending.len() >= w {
            for i in 0..w {
                self.work_input[i] = self.pending[i] * self.window[i];
            }
            if self.r2c.process(&mut self.work_input, &mut self.spectrum).is_err() {
                break;
            }
            self.pending.drain(..h);
            let mut col = vec![0.0f32; self.cfg.log_bins];
            self.map_log_magnitude(&mut col);
            out_columns.push(col);
        }
    }
    fn map_log_magnitude(&self, col: &mut [f32]) {
        let sr = self.cfg.sample_rate as f32;
        let nyq = 0.499 * sr;
        let f_max = self.cfg.f_max_hz.min(nyq).max(self.cfg.f_min_hz + 1.0);
        let f_min = self.cfg.f_min_hz.max(1.0);
        let nfft = self.cfg.window_size;
        let kmax = self.spectrum.len().saturating_sub(1).max(1);
        for i in 0..col.len() {
            let t = i as f32 / (col.len().saturating_sub(1).max(1) as f32);
            let f = f_min * (f_max / f_min).powf(t);
            let bin_f = f * nfft as f32 / sr;
            let k = (bin_f.round() as usize).clamp(1, kmax);
            let re = self.spectrum[k].re;
            let im = self.spectrum[k].im;
            let mag = (re * re + im * im).sqrt() / nfft as f32;
            let db = 20.0 * (mag + 1e-12).log10();
            let u = ((db - self.cfg.db_floor) / (self.cfg.db_ceil - self.cfg.db_floor)).clamp(0.0, 1.0);
            col[i] = u;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hop_clamps_high() {
        assert_eq!(normalize_hop_size(16384, 32768), 16384);
    }
    #[test]
    fn hop_zero_is_half_window() {
        assert_eq!(normalize_hop_size(16384, 0), 8192);
    }
    #[test]
    fn hop_one_to_window_preserved() {
        assert_eq!(normalize_hop_size(100, 50), 50);
    }
}
