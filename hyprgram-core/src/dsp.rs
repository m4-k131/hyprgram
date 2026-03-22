use crate::error::CoreError;
use realfft::{RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use std::f32::consts::PI;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct SpectrumConfig {
    pub window_size: usize,
    pub hop_size: usize,
    pub sample_rate: u32,
    pub log_bins: usize,
    pub f_min_hz: f32,
    pub f_max_hz: f32,
    pub db_floor: f32,
    pub db_ceil: f32,
}

impl Default for SpectrumConfig {
    fn default() -> Self {
        Self {
            window_size: 2048,
            hop_size: 1024,
            sample_rate: 48000,
            log_bins: 256,
            f_min_hz: 20.0,
            f_max_hz: 20000.0,
            db_floor: -80.0,
            db_ceil: 0.0,
        }
    }
}

pub struct SpectrumProcessor {
    cfg: SpectrumConfig,
    r2c: Arc<dyn RealToComplex<f32>>,
    hann: Vec<f32>,
    work_input: Vec<f32>,
    spectrum: Vec<Complex<f32>>,
    pending: Vec<f32>,
}

impl SpectrumProcessor {
    pub fn new(cfg: SpectrumConfig) -> Result<Self, CoreError> {
        if cfg.window_size < 8 {
            return Err(CoreError::Dsp("window_size too small".into()));
        }
        if cfg.hop_size == 0 || cfg.hop_size > cfg.window_size {
            return Err(CoreError::Dsp("invalid hop_size".into()));
        }
        let mut planner = RealFftPlanner::<f32>::new();
        let r2c = planner.plan_fft_forward(cfg.window_size);
        let spectrum = r2c.make_output_vec();
        let work_input = r2c.make_input_vec();
        let mut hann = vec![0.0f32; cfg.window_size];
        let n1 = (cfg.window_size - 1).max(1) as f32;
        for (i, w) in hann.iter_mut().enumerate() {
            *w = 0.5 * (1.0 - (2.0 * PI * i as f32 / n1).cos());
        }
        Ok(Self {
            cfg,
            r2c,
            hann,
            work_input,
            spectrum,
            pending: Vec::with_capacity(cfg.window_size * 2),
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
                self.work_input[i] = self.pending[i] * self.hann[i];
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
