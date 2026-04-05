# Hyprgram

Hyprgram is a **PipeWire audio spectrogram** in a normal **Wayland** window (Iced + `wgpu`).

The workspace has two crates:

- **`hyprgram`** — CLI and Linux UI (Iced + `wgpu`).
- **`hyprgram-core`** — DSP (FFT, spectrum), ring buffer, and PipeWire capture on Linux.

Non-Linux platforms are not supported; the binary exits with an error if built or run elsewhere.

## Requirements (summary)

- **Linux** with **Wayland** and **PipeWire**.
- Rust toolchain and system libraries for linking PipeWire (and typical graphics stack for `wgpu`). See **[BUILD.md](BUILD.md)** for install commands and `cargo` usage.

## License

Licensed under **MIT OR Apache-2.0** (see `Cargo.toml` workspace metadata).
