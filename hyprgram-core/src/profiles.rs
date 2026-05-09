use crate::{CoreError, SpectrumConfig, SpectrogramImageConfig};
use std::path::Path;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Profile {
    pub spectrum: SpectrumConfig,
    #[serde(default)]
    pub image: Option<ProfileImage>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ProfileImage {
    #[serde(default = "default_width")]
    pub width: u32,
    #[serde(default = "default_height")]
    pub height: u32,
    #[serde(default = "default_scroll")]
    pub scroll_right_to_left: bool,
    #[serde(default = "default_colormap")]
    pub colormap: String,
}

fn default_width() -> u32 { 800 }
fn default_height() -> u32 { 200 }
fn default_scroll() -> bool { true }
fn default_colormap() -> String { "viridis".into() }

impl Profile {
    pub fn to_image_config(&self) -> SpectrogramImageConfig {
        let img = self.image.as_ref();
        SpectrogramImageConfig {
            spectrum: self.spectrum.clone(),
            width: img.map_or(800, |i| i.width),
            height: img.map_or(200, |i| i.height),
            scroll_right_to_left: img.map_or(true, |i| i.scroll_right_to_left),
            colormap: img.map_or_else(|| "viridis".into(), |i| i.colormap.clone()),
        }
    }
}

pub fn load_profile(path: &Path) -> Result<Profile, CoreError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| CoreError::Dsp(format!("failed to read profile: {e}")))?;
    toml::from_str(&content)
        .map_err(|e| CoreError::Dsp(format!("invalid profile TOML: {e}")))
}

pub fn builtin_profile(name: &str) -> Option<Profile> {
    match name {
        "laptop" => Some(Profile {
            spectrum: SpectrumConfig {
                window_size: 4096,
                hop_size: 512,
                sample_rate: 48000,
                log_bins: 128,
                ..Default::default()
            },
            image: None,
        }),
        "default" => Some(Profile {
            spectrum: SpectrumConfig::default(),
            image: None,
        }),
        "foobar-like" => Some(Profile {
            spectrum: SpectrumConfig {
                window_size: 32768,
                hop_size: 128,
                sample_rate: 48000,
                log_bins: 512,
                ..Default::default()
            },
            image: None,
        }),
        _ => None,
    }
}

pub fn builtin_profile_names() -> &'static [&'static str] {
    &["laptop", "default", "foobar-like"]
}
