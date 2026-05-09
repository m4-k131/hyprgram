# hyprgram

A **foobar2000-class spectrogram visualizer** in Rust. Generate high-resolution spectrograms from audio — offline as PNG images, or live via PipeWire capture with GPU rendering on Linux/Wayland.

## Quick start

### Offline PNG (Windows, macOS, Linux)

```bash
cargo run -p hyprgram --bin audio_to_png -- song.mp3 spectrogram.png
```

Supports WAV, MP3, FLAC, AAC, Ogg Vorbis.

### Live window (Linux/Wayland only)

```bash
cargo run -p hyprgram -- song.mp3
```

Captures system audio via PipeWire and renders a scrolling spectrogram in a Wayland window using GPU shaders.

## Profiles

Use built-in presets or your own TOML config:

```bash
# Built-in profiles
cargo run -p hyprgram --bin audio_to_png -- song.mp3 out.png --profile laptop
cargo run -p hyprgram --bin audio_to_png -- song.mp3 out.png --profile foobar-like

# Custom TOML config
cargo run -p hyprgram --bin audio_to_png -- song.mp3 out.png --config my_profile.toml

# Override any setting from the CLI
cargo run -p hyprgram --bin audio_to_png -- song.mp3 out.png --profile foobar-like --width 1200 --height 600 --window-fn blackman-harris
```

| Profile | FFT window | Hop | Bins | Use case |
|---------|-----------|-----|------|----------|
| `laptop` | 4096 | 512 | 128 | Fast, low CPU |
| `default` | 20480 | 256 | 256 | Balanced quality |
| `foobar-like` | 32768 | 128 | 512 | Maximum resolution |

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `--profile` | `default` | Built-in profile: `laptop`, `default`, `foobar-like` |
| `--config` | — | Path to a TOML profile file |
| `--fft` / `--window` | 20480 | FFT window size (samples) |
| `--hop` | 256 | STFT hop between frames (samples) |
| `--log-bins` | 256 | Number of log-spaced frequency bins |
| `--window-fn` | hann | Window function: `hann`, `hamming`, `blackman`, `blackman-harris` |
| `--width` | 800 | Image width (px) |
| `--height` | 200 | Image height (px) |
| `--legacy-vertical-scroll` | off | Render time top-to-bottom instead of right-to-left |

## How it works

1. **Decode** audio to mono f32 samples (via Symphonia)
2. **STFT** — sliding window → FFT → log-magnitude mapping, parallelized across all CPU cores
3. **Render** — CPU-based viridis colormap → PNG, or GPU shader → Wayland window

## Building

Requires Rust (stable). See `BUILD.md` for Linux system dependencies (PipeWire), or `WINDOWS.md` for Windows notes.

```bash
cargo check --workspace
cargo build --release
```

## Project structure

| Crate | Purpose |
|-------|---------|
| `hyprgram-core` | DSP library: STFT/FFT, rendering, profiles, ring buffer |
| `hyprgram` | Application: CLI, live window, GPU shaders, offline PNG binary |

## Roadmap

See `ROADMAP.md` for the full feature plan. Currently in Phase 1 — core spectrogram quality improvements (window functions done, band aggregation and smoothing next).
