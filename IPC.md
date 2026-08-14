# IPC — External Control via Unix Socket

vividspektrum listens on a Unix domain socket at `/tmp/vividspektrum.sock` when running the live spectrogram window (Linux). External applications can send text commands to change settings in real time — colormap, contrast, profile, DSP parameters, and more.

## Quick Start

```bash
# Change colormap
echo "colormap inferno" | socat - /tmp/vividspektrum.sock

# Set contrast
echo "contrast 1.5" | socat - /tmp/vividspektrum.sock

# Apply a profile
echo "profile high_quality" | socat - /tmp/vividspektrum.sock
```

If `socat` is not installed, use netcat:

```bash
echo "colormap inferno" | nc -U /tmp/vividspektrum.sock
```

## Protocol

- **Transport**: Unix domain socket, line-based text protocol
- **Socket path**: `/tmp/vividspektrum.sock`
- **Encoding**: UTF-8, one command per line
- **Response**: Each command receives one line back:
  - `ok` — command accepted
  - `error: <message>` — command rejected (unknown command, bad value, etc.)
  - For query commands (`list-colormaps`, `list-profiles`, `list-overlays`, `help`): the data is returned directly, one item per line

## Commands

### Colors

| Command | Example | Description |
|---------|---------|-------------|
| `colormap <name>` | `colormap inferno` | Set colormap for the active source |
| `colormap-stops <name> <stops>` | `colormap-stops my-cm 0.0,0.0,0.0,0.0 0.5,0.5,0.0,0.5 1.0,1.0,1.0,1.0` | Apply a custom colormap from gradient stops (not saved) |
| `colormap-save <name> <stops>` | `colormap-save my-cm 0.0,0.0,0.0,0.0 1.0,1.0,1.0,1.0` | Save a custom colormap to disk and apply it (appears in `list-colormaps`) |
| `contrast <value>` | `contrast 1.5` | GPU contrast (0.0–3.0, 1.0 = neutral) |
| `saturation <value>` | `saturation 0.8` | GPU saturation (0.0–3.0, 1.0 = neutral) |
| `opacity <value>` | `opacity 0.7` | Layer opacity (0.0–1.0, 1.0 = fully opaque) |

### Profiles & Overlays

| Command | Example | Description |
|---------|---------|-------------|
| `profile <name>` | `profile high_quality` | Apply a named profile (DSP + colors + overlay + source) |
| `overlay <name>` | `overlay guitar-standard` | Set frequency overlay (use `none` to disable) |

### Audio Source

| Command | Example | Description |
|---------|---------|-------------|
| `source <label>` | `source Output 1 · alsa_output.pci-...` | Set audio source for the active source slot. Use the label as shown in the UI. Run `pactl list sources` to find available PipeWire/PulseAudio source names. |

### DSP Settings

| Command | Example | Description |
|---------|---------|-------------|
| `window-fn <name>` | `window-fn blackman-harris` | Window function: `hann`, `hamming`, `blackman`, `blackman-harris` |
| `weighting <name>` | `weighting a` | Frequency weighting: `none`, `a`, `c` |
| `transform <name>` | `transform cqt` | Transform: `stft`, `cqt` |
| `band-agg <name>` | `band-agg triangular` | Band aggregation: `nearest`, `triangular` |
| `centered <bool>` | `centered true` | Centered analysis window |
| `shared-bg <bool>` | `shared-bg true` | Use darkest colormap color as shared background |
| `additive-blend <bool>` | `additive-blend true` | Additively combine source colors (sum toward white) |

### DSP Parameters (`set`)

Use `set <param> <value>` to adjust individual DSP sliders:

| Parameter | Range | Description |
|-----------|-------|-------------|
| `fft-size` | 256–32768 | FFT window length (samples) |
| `hop` | 64–8192 | STFT hop size (samples) |
| `log-bins` | 64–8192 | Number of log-frequency bins |
| `f-min` | 10–2000 | Minimum frequency (Hz) |
| `f-max` | 2000–24000 | Maximum frequency (Hz) |
| `db-floor` | -120–0 | dB floor (darkest color) |
| `db-ceil` | -60–6 | dB ceiling (brightest color) |
| `smoothing` | 0–5 | Gaussian frequency smoothing sigma |
| `gamma` | 0–2 | Amplitude gamma |
| `temporal-alpha` | 0–1 | EMA temporal smoothing |
| `peak-decay` | 0–0.999 | Peak hold decay per frame |
| `cqt-bins` | 12–96 | CQT bins per octave |
| `freq-scale-exp` | 0.1–2.0 | Frequency scale exponent |
| `history` | 100–10000 | Number of spectrogram columns in buffer |

```bash
echo "set fft-size 8192" | socat - /tmp/vividspektrum.sock
echo "set f-min 20" | socat - /tmp/vividspektrum.sock
```

### UI Control

| Command | Description |
|---------|-------------|
| `toggle-menu` | Toggle the settings panel |
| `close-menu` | Close the settings panel |

### Queries

| Command | Response |
|---------|----------|
| `list-colormaps` | One colormap name per line |
| `list-profiles` | One profile name per line |
| `list-overlays` | One overlay name per line |
| `help` | List of all available commands |

## Custom Colormaps

You can push a complete colormap definition (gradient stops) that isn't in the built-in or saved list. Each stop is `position,r,g,b` with all values 0.0–1.0, separated by spaces:

```bash
# Apply immediately (not persisted)
echo "colormap-stops my-custom 0.0,0.0,0.0,0.0 0.25,0.2,0.0,0.4 0.5,0.5,0.0,0.5 0.75,0.8,0.2,0.6 1.0,1.0,1.0,1.0" | socat - /tmp/vividspektrum.sock

# Save to disk (appears in list-colormaps and the UI dropdown)
echo "colormap-save my-custom 0.0,0.0,0.0,0.0 1.0,1.0,1.0,1.0" | socat - /tmp/vividspektrum.sock
```

- **`colormap-stops`** — applies the colormap to the active source immediately. The name is shown in the UI but the colormap is lost when the app restarts.
- **`colormap-save`** — saves the colormap as a TOML file in the user config directory (`~/.config/vividspektrum/colormaps/<name>.toml`), then applies it. It will appear in `list-colormaps` and the UI dropdown permanently.
- Stops are automatically sorted by position. At least 2 stops are required.

### Color Adjustment

Optional `light-contrast` and `chroma-scale` keywords can be appended to `colormap-stops` and `colormap-save`. These apply a one-time transform to the stops in HSL space before applying/saving. They are distinct from the GPU `contrast` and `saturation` slider commands — those operate at render time, while these modify the colormap stops themselves:

- **`light-contrast <factor>`** — lightness contrast around 0.5. Values >1 darken darks and brighten brights (e.g. `1.5` pushes low-intensity colors darker and high-intensity colors brighter). `1.0` = no change.
- **`chroma-scale <factor>`** — scales chroma. Values >1 increase colorfulness, <1 desaturate. `1.0` = no change. Useful for fine-tuning how vivid the colormap appears on the spectrogram.

Hue is always preserved — only lightness and chroma are adjusted.

```bash
# Push a colormap with increased lightness contrast (darker darks, brighter brights)
echo "colormap-save my-custom 0.0,0.05,0.0,0.1 0.5,0.5,0.2,0.5 1.0,1.0,0.9,0.8 light-contrast 1.4 chroma-scale 1.1" | socat - /tmp/vividspektrum.sock
```

## Examples

### Python

```python
import socket

def send_cmd(cmd: str) -> str:
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.connect("/tmp/vividspektrum.sock")
    sock.sendall((cmd + "\n").encode())
    response = sock.recv(4096).decode().strip()
    sock.close()
    return response

print(send_cmd("colormap inferno"))       # ok
print(send_cmd("contrast 1.5"))            # ok
print(send_cmd("list-colormaps"))          # viridis\ninferno\nmagma\n...
```

### Bash Script

```bash
#!/bin/bash
SOCKET="/tmp/vividspektrum.sock"

send() {
    echo "$1" | socat - "$SOCKET"
}

send "colormap inferno"
send "contrast 1.2"
send "set fft-size 4096"
send "set hop 512"
```

### Shell One-liner (no socat/netcat)

```bash
exec 3<>/dev/tcp/localhost/0  # won't work for Unix sockets
# Use python instead:
python3 -c "
import socket, sys
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.connect('/tmp/vividspektrum.sock')
s.sendall((sys.argv[1] + '\n').encode())
print(s.recv(4096).decode().strip())
s.close()
" "colormap inferno"
```

## Notes

- The socket is created when the live spectrogram window starts and removed when the process exits. If the process crashes, the stale socket file is cleaned up on next launch.
- Commands apply to the **active source** (the one currently selected in the UI). Use the UI to switch sources, or send a `profile` command that configures multiple sources.
- All commands take effect immediately — colormap and contrast/saturation changes are applied to the GPU on the next frame (~16ms).
- DSP parameter changes that require an FFT restart (e.g. `fft-size`, `log-bins`) will briefly interrupt the spectrogram while the DSP thread reinitializes.
- The socket supports concurrent connections. Each connection can send multiple commands (one per line). The connection is closed when the client disconnects.
