use crate::settings::{DspSlider, SettingsMessage};
use spektrum_core::{BandAggregation, Transform, Weighting, WindowFunction};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::mpsc;

pub const SOCKET_PATH: &str = "/tmp/vividspektrum.sock";

pub fn spawn_ipc_server() -> mpsc::Receiver<SettingsMessage> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = std::fs::remove_file(SOCKET_PATH);
        let listener = match UnixListener::bind(SOCKET_PATH) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("[ipc] failed to bind {SOCKET_PATH}: {e}");
                return;
            }
        };
        eprintln!("[ipc] listening on {SOCKET_PATH}");
        for stream in listener.incoming() {
            if let Ok(stream) = stream {
                let tx = tx.clone();
                std::thread::spawn(move || handle_client(stream, tx));
            }
        }
    });
    rx
}

fn handle_client(stream: UnixStream, tx: mpsc::Sender<SettingsMessage>) {
    let reader = match stream.try_clone() {
        Ok(s) => BufReader::new(s),
        Err(_) => return,
    };
    let mut writer = stream;
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(response) = handle_query(line) {
            let _ = writeln!(writer, "{response}");
            let _ = writer.flush();
            continue;
        }
        match parse_command(line) {
            Ok(msgs) => {
                let mut ok = true;
                for msg in msgs {
                    if tx.send(msg).is_err() {
                        ok = false;
                        break;
                    }
                }
                let _ = writeln!(writer, "{}", if ok { "ok" } else { "error: channel closed" });
            }
            Err(e) => {
                let _ = writeln!(writer, "error: {e}");
            }
        }
        let _ = writer.flush();
    }
}

fn handle_query(line: &str) -> Option<String> {
    match line.to_lowercase().as_str() {
        "list-colormaps" => Some(spektrum_core::all_colormap_names().join("\n")),
        "list-profiles" => Some(spektrum_core::profiles::list_profile_names().join("\n")),
        "list-overlays" => {
            let mut names = vec!["none".to_string()];
            names.extend(spektrum_core::overlay::builtin_overlay_names().iter().cloned());
            Some(names.join("\n"))
        }
        "help" => Some(HELP_TEXT.to_string()),
        _ => None,
    }
}

const HELP_TEXT: &str = "\
Commands:\n\
  colormap <name>\n\
  colormap-stops <name> <pos,r,g,b> <pos,r,g,b> ... [light-contrast <f>] [chroma-scale <f>]\n\
  colormap-save <name> <pos,r,g,b> <pos,r,g,b> ... [light-contrast <f>] [chroma-scale <f>]\n\
  contrast <0.0-3.0>\n\
  saturation <0.0-3.0>\n\
  opacity <0.0-1.0>\n\
  profile <name>\n\
  overlay <name>\n\
  source <label>\n\
  window-fn <hann|hamming|blackman|blackman-harris>\n\
  weighting <none|a|c>\n\
  transform <stft|cqt>\n\
  band-agg <nearest|triangular>\n\
  centered <true|false>\n\
  shared-bg <true|false>\n\
  additive-blend <true|false>\n\
  set <param> <value>\n\
  toggle-menu\n\
  close-menu\n\
  list-colormaps\n\
  list-profiles\n\
  list-overlays\n\
  help\n\
\n\
DSP params for 'set': fft-size, hop, log-bins, f-min, f-max,\n\
  db-floor, db-ceil, smoothing, gamma, temporal-alpha,\n\
  peak-decay, cqt-bins, freq-scale-exp, history";

fn parse_command(line: &str) -> Result<Vec<SettingsMessage>, String> {
    let (cmd, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    let cmd = cmd.to_lowercase();
    let rest = rest.trim();

    match cmd.as_str() {
        "colormap" => Ok(vec![SettingsMessage::SetColormap(rest.to_string())]),
        "colormap-stops" => {
            let (name, stops_str) = rest
                .split_once(char::is_whitespace)
                .ok_or("expected: colormap-stops <name> <pos,r,g,b> ...")?;
            let stops = parse_stops(stops_str)?;
            Ok(vec![SettingsMessage::SetColormapStops(name.trim().to_string(), stops)])
        }
        "colormap-save" => {
            let (name, stops_str) = rest
                .split_once(char::is_whitespace)
                .ok_or("expected: colormap-save <name> <pos,r,g,b> ...")?;
            let stops = parse_stops(stops_str)?;
            Ok(vec![SettingsMessage::SaveColormapStops(name.trim().to_string(), stops)])
        }
        "contrast" => {
            let v: f32 = rest.parse().map_err(|_| "expected number")?;
            Ok(vec![SettingsMessage::SetContrast(v)])
        }
        "saturation" => {
            let v: f32 = rest.parse().map_err(|_| "expected number")?;
            Ok(vec![SettingsMessage::SetSaturation(v)])
        }
        "opacity" => {
            let v: f32 = rest.parse().map_err(|_| "expected number")?;
            Ok(vec![SettingsMessage::SetOpacity(v)])
        }
        "profile" => Ok(vec![SettingsMessage::SetProfile(rest.to_string())]),
        "overlay" => Ok(vec![SettingsMessage::SetOverlay(rest.to_string())]),
        "source" => Ok(vec![SettingsMessage::SetSource(rest.to_string())]),
        "window-fn" => {
            let w = match rest.to_lowercase().as_str() {
                "hann" => WindowFunction::Hann,
                "hamming" => WindowFunction::Hamming,
                "blackman" => WindowFunction::Blackman,
                "blackman-harris" => WindowFunction::BlackmanHarris,
                _ => return Err(format!("unknown window function '{rest}'")),
            };
            Ok(vec![SettingsMessage::SetWindowFn(w)])
        }
        "weighting" => {
            let w = match rest.to_lowercase().as_str() {
                "none" => Weighting::None,
                "a" => Weighting::A,
                "c" => Weighting::C,
                _ => return Err(format!("unknown weighting '{rest}'")),
            };
            Ok(vec![SettingsMessage::SetWeighting(w)])
        }
        "transform" => {
            let t = match rest.to_lowercase().as_str() {
                "stft" => Transform::Stft,
                "cqt" => Transform::Cqt,
                _ => return Err(format!("unknown transform '{rest}'")),
            };
            Ok(vec![SettingsMessage::SetTransform(t)])
        }
        "band-agg" | "band-aggregation" => {
            let a = match rest.to_lowercase().as_str() {
                "nearest" => BandAggregation::Nearest,
                "triangular" => BandAggregation::Triangular,
                _ => return Err(format!("unknown band aggregation '{rest}'")),
            };
            Ok(vec![SettingsMessage::SetBandAggregation(a)])
        }
        "centered" => {
            let b = parse_bool(rest)?;
            Ok(vec![SettingsMessage::SetCentered(b)])
        }
        "shared-bg" => {
            let b = parse_bool(rest)?;
            Ok(vec![SettingsMessage::SetSharedBg(b)])
        }
        "additive-blend" => {
            let b = parse_bool(rest)?;
            Ok(vec![SettingsMessage::SetAdditiveBlend(b)])
        }
        "set" => {
            let (param, value) = rest
                .split_once(char::is_whitespace)
                .ok_or("expected: set <param> <value>")?;
            let slider = parse_slider(param.trim())?;
            let value: f32 = value.trim().parse().map_err(|_| "expected number")?;
            Ok(vec![
                SettingsMessage::AdvancedSlider(slider, value),
                SettingsMessage::AdvancedSliderRelease(slider),
            ])
        }
        "toggle-menu" => Ok(vec![SettingsMessage::Toggle]),
        "close-menu" => Ok(vec![SettingsMessage::Close]),
        _ => Err(format!("unknown command '{cmd}'. Send 'help' for usage.")),
    }
}

fn parse_bool(s: &str) -> Result<bool, String> {
    match s.to_lowercase().as_str() {
        "true" | "1" | "on" => Ok(true),
        "false" | "0" | "off" => Ok(false),
        _ => Err(format!("expected true/false, got '{s}'")),
    }
}

fn parse_slider(name: &str) -> Result<DspSlider, String> {
    match name.to_lowercase().as_str() {
        "fft-size" | "window-size" => Ok(DspSlider::WindowSize),
        "hop" => Ok(DspSlider::HopSize),
        "log-bins" => Ok(DspSlider::LogBins),
        "f-min" => Ok(DspSlider::FMin),
        "f-max" => Ok(DspSlider::FMax),
        "db-floor" => Ok(DspSlider::DbFloor),
        "db-ceil" => Ok(DspSlider::DbCeil),
        "smoothing" => Ok(DspSlider::Smoothing),
        "gamma" => Ok(DspSlider::Gamma),
        "temporal-alpha" => Ok(DspSlider::TemporalAlpha),
        "peak-decay" => Ok(DspSlider::PeakDecay),
        "cqt-bins" => Ok(DspSlider::CqtBins),
        "freq-scale-exp" => Ok(DspSlider::FreqScaleExp),
        "history" => Ok(DspSlider::History),
        _ => Err(format!(
            "unknown parameter '{name}'. Available: fft-size, hop, log-bins, f-min, f-max, db-floor, db-ceil, smoothing, gamma, temporal-alpha, peak-decay, cqt-bins, freq-scale-exp, history"
        )),
    }
}

fn parse_stops(s: &str) -> Result<Vec<(f32, f32, f32, f32)>, String> {
    let mut stops = Vec::new();
    let mut contrast: f32 = 1.0;
    let mut saturation: f32 = 1.0;

    let mut tokens = s.split_whitespace().peekable();
    while let Some(token) = tokens.next() {
        match token.to_lowercase().as_str() {
            "light-contrast" => {
                let v = tokens.next().ok_or("expected value after 'light-contrast'")?;
                contrast = v.parse().map_err(|_| format!("bad light-contrast value '{v}'"))?;
            }
            "chroma-scale" => {
                let v = tokens.next().ok_or("expected value after 'chroma-scale'")?;
                saturation = v.parse().map_err(|_| format!("bad chroma-scale value '{v}'"))?;
            }
            _ => {
                let parts: Vec<&str> = token.split(',').collect();
                if parts.len() != 4 {
                    return Err(format!("expected pos,r,g,b — got '{token}'"));
                }
                let pos: f32 = parts[0].trim().parse().map_err(|_| format!("bad position in '{token}'"))?;
                let r: f32 = parts[1].trim().parse().map_err(|_| format!("bad red in '{token}'"))?;
                let g: f32 = parts[2].trim().parse().map_err(|_| format!("bad green in '{token}'"))?;
                let b: f32 = parts[3].trim().parse().map_err(|_| format!("bad blue in '{token}'"))?;
                stops.push((pos, r, g, b));
            }
        }
    }
    if stops.len() < 2 {
        return Err("need at least 2 stops".to_string());
    }
    if contrast != 1.0 || saturation != 1.0 {
        stops = adjust_stops(&stops, contrast, saturation);
    }
    Ok(stops)
}

fn adjust_stops(stops: &[(f32, f32, f32, f32)], contrast: f32, saturation: f32) -> Vec<(f32, f32, f32, f32)> {
    stops.iter().map(|&(pos, r, g, b)| {
        let (h, s, l) = rgb_to_hsl(r, g, b);
        let l_new = ((l - 0.5) * contrast + 0.5).clamp(0.0, 1.0);
        let s_new = (s * saturation).clamp(0.0, 1.0);
        let (r, g, b) = hsl_to_rgb(h, s_new, l_new);
        (pos, r, g, b)
    }).collect()
}

fn rgb_to_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) * 0.5;
    if (max - min).abs() < 1e-9 {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if max == r {
        ((g - b) / d) + if g < b { 4.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, s, l)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s.abs() < 1e-9 {
        return (l, l, l);
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let h_norm = if h < 0.0 { h + 360.0 } else { h } / 360.0;
    let hue_to_rgb = |t: f32| {
        let t = t.clamp(0.0, 1.0);
        if t < 1.0 / 6.0 { p + (q - p) * 6.0 * t }
        else if t < 0.5 { q }
        else if t < 2.0 / 3.0 { p + (q - p) * (2.0 / 3.0 - t) * 6.0 }
        else { p }
    };
    (hue_to_rgb(h_norm + 1.0 / 3.0), hue_to_rgb(h_norm), hue_to_rgb(h_norm - 1.0 / 3.0))
}
