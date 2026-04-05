# Hyprgram: foobar-like spectrogram roadmap

This document lists **features and techniques** (not magic numbers) toward a **foobar2000-class** spectrogram: high resolution, CPU-heavy but acceptable. It is meant for **planning and handoff**; implement in order of phases unless a later item is explicitly scoped.

**Reference:** third-party component `foo_vis_spectrum_analyzer` (and wiki), not a single built-in default. See `.cursorrules` (Spectrogram quality target) for links.

---

## A. Acquisition and timing

| Feature / technique | What it buys you | Plan |
|---------------------|------------------|------|
| Clock-locked analysis | Analysis aligned to real playback time, less drift vs what you hear. | Define a single timebase (monotonic clock + sample counter); document how PipeWire timestamps map to STFT frames; optional latency calibration. |
| Reaction / lookahead alignment | Foobar exposes “reaction alignment” (centered vs causal window). | Add an explicit **analysis delay policy** (centered Hann vs causal) and document perceptual tradeoff. |
| Refresh decoupled from FFT rate | UI can draw at display Hz while STFT runs faster (foobar: >60 Hz refresh). | Separate **analysis cadence** from **present cadence**; queue already helps—formalize max backlog and “catch-up” policy. |

---

## B. Time–frequency transforms (beyond plain STFT)

| Feature / technique | What it buys you | Plan |
|-----------------------|------------------|------|
| Constant-Q (CQT) or filter-bank STFT | Log-frequency resolution that matches musical pitch; fewer misleading bins at low end. | Spike: small CQT path or use a **library**; compare CPU vs FFT+log mapping; mono channel first. |
| SWIFT / IIR-style bands (foobar option) | Alternative time–frequency tiling; different CPU profile. | Research milestone: when CQT/STFT is not enough; low priority unless foobar parity by mode is required. |
| Configurable window family | Hann vs Hamming vs Gaussian/Kaiser (foobar has window + skew). | Pluggable **window function** + optional **skew** parameter; keep Hann as default. |

---

## C. FFT pipeline polish

| Feature / technique | What it buys you | Plan |
|-----------------------|------------------|------|
| Non-power-of-two FFT (optional) | Foobar allows custom sizes at CPU cost; matches arbitrary ms windows. | Ensure planner path supports arbitrary **N**; benchmark; guard with CLI or “expert” flag. |
| Per-bin aggregation | min/max/mean/RMS across FFT bins mapped to one display band (foobar has many modes). | After FFT, define **band mapping** layer: triangular or nearest-bin → **one value per display column** with selectable norm. |
| Lanczos (or similar) smoothing across frequency | Softer, less “sparkly” spectrum; foobar documents Lanczos kernel size. | Post-FFT **1D smoothing along frequency** (or along log index) with width as a parameter. |

---

## D. Frequency axis and display mapping

| Feature / technique | What it buys you | Plan |
|-----------------------|------------------|------|
| Triangular / mel / bark filter banks | Perceptual weighting; closer to “analyzer” sound than raw FFT magnitude. | Roadmap: **mel** or **triangular** as second mapping mode beside current log-sparse columns. |
| Brown–Puckette-style CQT mapping | Foobar-specific option for CQT path. | Only after a real CQT or hybrid exists. |
| Suppress mirror / Nyquist guard | Cleaner high-frequency end. | Cheap: zero or fade bins near Nyquist in display mapping. |

---

## E. Amplitude and dynamics

| Feature / technique | What it buys you | Plan |
|-----------------------|------------------|------|
| dB scale + stable floor/ceiling | Foobar uses dB ranges on axes; avoids “everything is neon”. | Unify **normalization**: reference level, **noise floor** in dB, optional **gamma** on magnitude (foobar mentions gamma on some scales). |
| Temporal smoothing (per bin or per frame) | Less flicker; foobar has smoothing factor + peak hold modes. | Optional **EMA** or **peak decay** on scalars before GPU upload. |
| A/C-weighting (optional) | Loudness-relevant spectrum. | Optional **IIR weighting** stage before FFT or on magnitude (document phase implications). |

---

## F. GPU / visualization

| Feature / technique | What it buys you | Plan |
|-----------------------|------------------|------|
| Interpolation in shader | Sub-texel scrolling; less blocky than nearest-neighbor history. | Bilinear where format allows, or **separate** RGBA8 filterable path for display only. |
| Colormap control | Foobar-grade presets (gradient stops, SoX-style, etc.). | Uniform or 1D LUT texture; **preset** file format later. |
| Multi-pass or mip / blur | Cheap glow / temporal smear without more FFTs. | Optional post-process pass on spectrogram texture. |

---

## G. Product / engineering

| Feature / technique | What it buys you | Plan |
|-----------------------|------------------|------|
| Preset export/import | Match foobar workflow (named tunings). | After core parameters stabilize: JSON/TOML **preset** for fft/hop/window/mapping/smoothing. |
| CPU profiles | “Laptop” vs “foobar-like”. | Named **profiles** that set hop, FFT size, smoothing, refresh—not single defaults. |
| Regression captures | Know when a change breaks “look”. | Golden **PNG** or vector dumps from sine/chirp—optional CI later. |

---

## Phased implementation order

1. **Phase 1 — Honest baseline**  
   Unify **timebase + latency** story; document **reaction alignment**; formalize **analysis vs render** rates; add **amplitude pipeline** doc (dB, floor, optional gamma).

2. **Phase 2 — STFT quality without changing transform family**  
   Pluggable **window**; **band aggregation** + **triangular or mel** mapping; **frequency-domain smoothing** (Lanczos or lighter); **temporal smoothing** on scalars.

3. **Phase 3 — Transform upgrade (large)**  
   **CQT** or **constant-Q filter bank** path; compare to STFT+log; optional **non-power-of-two** FFT for ms-based windows.

4. **Phase 4 — Polish and parity extras**  
   Weighting filters; shader **interpolation** / colormap presets; **profiles** and **preset files**; optional **>60 Hz** present path.

5. **Phase 5 — Optional / research**  
   SWIFT/analog modes; heavy **visual** post-processing; **CI** golden visuals.

---

## Notes for implementers

- Prefer **correctness and resolution** over shaving CPU until a knob or profile says otherwise (see `.cursorrules`).
- Exact foobar **defaults** are preset-dependent; bit-identical parity requires capturing **preset files** or metrics from a reference install—do not assume one global numeric default.
