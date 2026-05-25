use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Profile {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub video_codec: Option<String>,
    #[serde(default)]
    pub audio_codec: Option<String>,
    #[serde(default)]
    pub preset: Option<String>,
    #[serde(default)]
    pub crf: Option<u32>,
    #[serde(default)]
    pub video_bitrate: Option<String>,
    #[serde(default)]
    pub audio_bitrate: Option<String>,
    #[serde(default)]
    pub extra_flags: Option<Vec<String>>,
    #[serde(default)]
    pub hwaccel: Option<String>,
}

pub fn load_profiles(path: &Path) -> HashMap<String, Profile> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };
    serde_yaml::from_str(&content).unwrap_or_default()
}

pub fn builtin_profiles() -> HashMap<String, Profile> {
    let yaml = include_str!("default_profiles.yml");
    serde_yaml::from_str(yaml).expect("default_profiles.yml is invalid")
}
