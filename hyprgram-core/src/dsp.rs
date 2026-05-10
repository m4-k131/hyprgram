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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Transform {
    #[default]
    Stft,
    Cqt,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Weighting {
    #[default]
    None,
    A,
    C,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BandAggregation {
    #[default]
    Nearest,
    Triangular,
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
    #[serde(default)]
    pub band_aggregation: BandAggregation,
    #[serde(default)]
    pub freq_smoothing_sigma: f32,
    #[serde(default = "default_gamma")]
    pub amplitude_gamma: f32,
    #[serde(default)]
    pub temporal_alpha: f32,
    #[serde(default)]
    pub peak_hold_decay: f32,
    #[serde(default)]
    pub weighting: Weighting,
    #[serde(default)]
    pub transform: Transform,
    #[serde(default = "default_cqt_bpo")]
    pub cqt_bins_per_octave: u32,
}

fn default_gamma() -> f32 { 1.0 }
fn default_cqt_bpo() -> u32 { 12 }

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
            band_aggregation: BandAggregation::Nearest,
            freq_smoothing_sigma: 0.0,
            amplitude_gamma: 1.0,
            temporal_alpha: 0.0,
            peak_hold_decay: 0.0,
            weighting: Weighting::None,
            transform: Transform::Stft,
            cqt_bins_per_octave: 12,
        }
    }
}

pub struct SpectrumProcessor {
    cfg: SpectrumConfig,
    r2c: Arc<dyn RealToComplex<f32>>,
    window: Vec<f32>,
    work_input: Vec<f32>,
    spectrum: Vec<Complex<f32>>,
    band_weights: Vec<Vec<(usize, f32)>>,
    smoothing_kernel: Vec<(isize, f32)>,
    prev_column: Vec<f32>,
    peak_column: Vec<f32>,
    pending: Vec<f32>,
    weighting_weights: Vec<f32>,
    cqt_weights: Vec<Vec<(usize, f32)>>,
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
        let band_weights = build_band_weights(&cfg);
        let smoothing_kernel = build_gaussian_kernel(cfg.freq_smoothing_sigma);
        let weighting_weights = build_weighting_weights(&cfg);
        let cqt_weights = build_cqt_weights(&cfg);
        let pending_cap = cfg.window_size * 2;
        Ok(Self {
            cfg,
            r2c,
            window,
            work_input,
            spectrum,
            band_weights,
            smoothing_kernel,
            prev_column: Vec::new(),
            peak_column: Vec::new(),
            pending: Vec::with_capacity(pending_cap),
            weighting_weights,
            cqt_weights,
        })
    }
    pub fn set_sample_rate(&mut self, sr: u32) {
        self.cfg.sample_rate = sr;
    }
    pub fn log_bins(&self) -> usize {
        if self.cfg.transform == Transform::Cqt {
            self.cqt_weights.len().max(1)
        } else {
            self.cfg.log_bins
        }
    }
    pub fn push_samples(&mut self, incoming: &[f32], out_columns: &mut Vec<Vec<f32>>) {
        self.pending.extend_from_slice(incoming);
        let w = self.cfg.window_size;
        let h = self.cfg.hop_size;
        let n_bins = self.log_bins();
        out_columns.clear();
        while self.pending.len() >= w {
            for i in 0..w {
                self.work_input[i] = self.pending[i] * self.window[i];
            }
            if self.r2c.process(&mut self.work_input, &mut self.spectrum).is_err() {
                break;
            }
            self.pending.drain(..h);
            let mut col = vec![0.0f32; n_bins];
            self.map_log_magnitude(&mut col);
            self.apply_temporal(&mut col);
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
        if self.cfg.transform == Transform::Cqt && !self.cqt_weights.is_empty() {
            for i in 0..col.len().min(self.cqt_weights.len()) {
                let mut mag_sum = 0.0f32;
                let mut weight_sum = 0.0f32;
                for &(k, w) in &self.cqt_weights[i] {
                    let re = self.spectrum[k].re;
                    let im = self.spectrum[k].im;
                    let mag = (re * re + im * im).sqrt() / nfft as f32 * self.weighting_weights[k];
                    mag_sum += mag * w;
                    weight_sum += w;
                }
                let mag = if weight_sum > 0.0 { mag_sum / weight_sum } else { 0.0 };
                let db = 20.0 * (mag + 1e-12).log10();
                let u = ((db - self.cfg.db_floor) / (self.cfg.db_ceil - self.cfg.db_floor).max(1e-9)).clamp(0.0, 1.0);
                col[i] = u;
            }
            return;
        }
        match self.cfg.band_aggregation {
            BandAggregation::Nearest => {
                for i in 0..col.len() {
                    let t = i as f32 / (col.len().saturating_sub(1).max(1) as f32);
                    let f = f_min * (f_max / f_min).powf(t);
                    let bin_f = f * nfft as f32 / sr;
                    let k = (bin_f.round() as usize).clamp(1, kmax);
                    let re = self.spectrum[k].re;
                    let im = self.spectrum[k].im;
                    let mag = (re * re + im * im).sqrt() / nfft as f32 * self.weighting_weights[k];
                    let db = 20.0 * (mag + 1e-12).log10();
                    let u = ((db - self.cfg.db_floor) / (self.cfg.db_ceil - self.cfg.db_floor).max(1e-9)).clamp(0.0, 1.0);
                    col[i] = u;
                }
            }
            BandAggregation::Triangular => {
                for i in 0..col.len() {
                    let mut mag_sum = 0.0f32;
                    let mut weight_sum = 0.0f32;
                    for &(k, w) in &self.band_weights[i] {
                        let re = self.spectrum[k].re;
                        let im = self.spectrum[k].im;
                        let mag = (re * re + im * im).sqrt() / nfft as f32 * self.weighting_weights[k];
                        mag_sum += mag * w;
                        weight_sum += w;
                    }
                    let mag = if weight_sum > 0.0 { mag_sum / weight_sum } else { 0.0 };
                    let db = 20.0 * (mag + 1e-12).log10();
                    let u = ((db - self.cfg.db_floor) / (self.cfg.db_ceil - self.cfg.db_floor).max(1e-9)).clamp(0.0, 1.0);
                    col[i] = u;
                }
            }
        }
        if !self.smoothing_kernel.is_empty() {
            let orig = col.to_vec();
            let n = col.len() as isize;
            for i in 0..col.len() {
                let mut sum = 0.0f32;
                let mut wsum = 0.0f32;
                for &(off, w) in &self.smoothing_kernel {
                    let j = i as isize + off;
                    if j >= 0 && j < n {
                        sum += orig[j as usize] * w;
                        wsum += w;
                    }
                }
                col[i] = if wsum > 0.0 { sum / wsum } else { orig[i] };
            }
        }
        let gamma = self.cfg.amplitude_gamma;
        if (gamma - 1.0).abs() > 1e-6 {
            for v in col.iter_mut() {
                *v = v.powf(gamma);
            }
        }
    }
    fn apply_temporal(&mut self, col: &mut Vec<f32>) {
        let alpha = self.cfg.temporal_alpha;
        let decay = self.cfg.peak_hold_decay;
        if alpha > 0.0 && self.prev_column.len() == col.len() {
            for (v, prev) in col.iter_mut().zip(self.prev_column.iter()) {
                *v = alpha * *v + (1.0 - alpha) * prev;
            }
        }
        if decay > 0.0 {
            if self.peak_column.len() != col.len() {
                self.peak_column = col.clone();
            } else {
                for (v, peak) in col.iter().zip(self.peak_column.iter_mut()) {
                    *peak = (*v).max(*peak * decay);
                }
                col.copy_from_slice(&self.peak_column);
            }
        }
        self.prev_column = col.clone();
    }
}

fn build_band_weights(cfg: &SpectrumConfig) -> Vec<Vec<(usize, f32)>> {
    let n_bins = cfg.log_bins.max(1);
    let nfft = cfg.window_size;
    let sr = cfg.sample_rate as f32;
    let nyq = 0.499 * sr;
    let f_max = cfg.f_max_hz.min(nyq).max(cfg.f_min_hz + 1.0);
    let f_min = cfg.f_min_hz.max(1.0);
    let kmax = (nfft / 2).max(1);
    let mut weights = Vec::with_capacity(n_bins);
    for i in 0..n_bins {
        let t = i as f32 / (n_bins.saturating_sub(1).max(1) as f32);
        let fc = f_min * (f_max / f_min).powf(t);
        let t_prev = if i > 0 { (i - 1) as f32 / (n_bins.saturating_sub(1).max(1) as f32) } else { 0.0 };
        let f_lo = f_min * (f_max / f_min).powf(t_prev);
        let t_next = if i + 1 < n_bins { (i + 1) as f32 / (n_bins.saturating_sub(1).max(1) as f32) } else { 1.0 };
        let f_hi = f_min * (f_max / f_min).powf(t_next);
        let k_lo = ((f_lo * nfft as f32 / sr).floor() as usize).clamp(1, kmax);
        let k_hi = ((f_hi * nfft as f32 / sr).ceil() as usize).clamp(1, kmax);
        let mut band: Vec<(usize, f32)> = Vec::new();
        for k in k_lo..=k_hi {
            let fk = k as f32 * sr / nfft as f32;
            let w = if fk <= f_lo || fk >= f_hi {
                0.0
            } else if fk <= fc {
                (fk - f_lo) / (fc - f_lo).max(1e-9)
            } else {
                (f_hi - fk) / (f_hi - fc).max(1e-9)
            };
            if w > 0.0 {
                band.push((k, w));
            }
        }
        if band.is_empty() {
            let k = ((fc * nfft as f32 / sr).round() as usize).clamp(1, kmax);
            band.push((k, 1.0));
        }
        weights.push(band);
    }
    weights
}

fn build_gaussian_kernel(sigma: f32) -> Vec<(isize, f32)> {
    if sigma <= 0.0 {
        return Vec::new();
    }
    let radius = (3.0 * sigma).ceil() as isize;
    let mut taps = Vec::new();
    let mut total = 0.0f32;
    for i in -radius..=radius {
        let x = i as f32;
        let w = (-0.5 * (x / sigma).powi(2)).exp();
        taps.push((i, w));
        total += w;
    }
    if total > 0.0 {
        for (_, w) in taps.iter_mut() {
            *w /= total;
        }
    }
    taps
}

fn build_weighting_weights(cfg: &SpectrumConfig) -> Vec<f32> {
    let nfft = cfg.window_size;
    let sr = cfg.sample_rate as f32;
    let n_bins = nfft / 2 + 1;
    let mut w = vec![1.0f32; n_bins];
    match cfg.weighting {
        Weighting::None => {}
        Weighting::A => {
            for k in 0..n_bins {
                let f = k as f32 * sr / nfft as f32;
                let f2 = f * f;
                let num = 12194.0f32.powi(2) * f2 * f2;
                let den = (f2 + 20.6f32.powi(2))
                    * (f2 + 107.7f32.powi(2)).sqrt()
                    * (f2 + 737.9f32.powi(2)).sqrt()
                    * (f2 + 12194.0f32.powi(2));
                let ra = if den > 0.0 { num / den } else { 0.0 };
                let ref_f = 1000.0;
                let ref_f2 = ref_f * ref_f;
                let ref_num = 12194.0f32.powi(2) * ref_f2 * ref_f2;
                let ref_den = (ref_f2 + 20.6f32.powi(2))
                    * (ref_f2 + 107.7f32.powi(2)).sqrt()
                    * (ref_f2 + 737.9f32.powi(2)).sqrt()
                    * (ref_f2 + 12194.0f32.powi(2));
                let ra_ref = ref_num / ref_den;
                let a_weight = if ra_ref > 0.0 { ra / ra_ref } else { 1.0 };
                w[k] = a_weight;
            }
        }
        Weighting::C => {
            for k in 0..n_bins {
                let f = k as f32 * sr / nfft as f32;
                let f2 = f * f;
                let num = 12194.0f32.powi(2) * f2;
                let den = (f2 + 20.6f32.powi(2)) * (f2 + 12194.0f32.powi(2));
                let rc = if den > 0.0 { num / den } else { 0.0 };
                let ref_f = 1000.0;
                let ref_f2 = ref_f * ref_f;
                let ref_num = 12194.0f32.powi(2) * ref_f2;
                let ref_den = (ref_f2 + 20.6f32.powi(2)) * (ref_f2 + 12194.0f32.powi(2));
                let rc_ref = ref_num / ref_den;
                let c_weight = if rc_ref > 0.0 { rc / rc_ref } else { 1.0 };
                w[k] = c_weight;
            }
        }
    }
    w
}

fn build_cqt_weights(cfg: &SpectrumConfig) -> Vec<Vec<(usize, f32)>> {
    if cfg.transform != Transform::Cqt {
        return Vec::new();
    }
    let bpo = cfg.cqt_bins_per_octave.max(1) as f32;
    let q = 1.0 / (2.0f32.powf(1.0 / bpo) - 1.0);
    let f_min = cfg.f_min_hz.max(1.0);
    let sr = cfg.sample_rate as f32;
    let nyq = 0.499 * sr;
    let f_max = cfg.f_max_hz.min(nyq).max(f_min + 1.0);
    let nfft = cfg.window_size;
    let kmax = (nfft / 2).max(1);
    let num_bins = ((f_max / f_min).log2() * bpo).ceil() as usize;
    let mut weights = Vec::with_capacity(num_bins);
    for k_cqt in 0..num_bins {
        let fc = f_min * 2.0f32.powf(k_cqt as f32 / bpo);
        let bw = fc / q;
        let f_lo = (fc - bw * 0.5).max(0.0);
        let f_hi = (fc + bw * 0.5).min(nyq);
        let k_lo = ((f_lo * nfft as f32 / sr).floor() as usize).clamp(1, kmax);
        let k_hi = ((f_hi * nfft as f32 / sr).ceil() as usize).clamp(1, kmax);
        let mut band: Vec<(usize, f32)> = Vec::new();
        for k in k_lo..=k_hi {
            let fk = k as f32 * sr / nfft as f32;
            let w = if fk <= f_lo || fk >= f_hi {
                0.0
            } else if fk <= fc {
                (fk - f_lo) / (fc - f_lo).max(1e-9)
            } else {
                (f_hi - fk) / (f_hi - fc).max(1e-9)
            };
            if w > 0.0 {
                band.push((k, w));
            }
        }
        if band.is_empty() {
            let k = ((fc * nfft as f32 / sr).round() as usize).clamp(1, kmax);
            band.push((k, 1.0));
        }
        weights.push(band);
    }
    weights
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
