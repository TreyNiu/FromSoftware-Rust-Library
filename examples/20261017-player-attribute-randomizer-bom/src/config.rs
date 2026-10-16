use std::{
    ffi::c_void,
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use serde::{Deserialize, Serialize};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;

pub const CONFIG_FILE_NAME: &str = "player_attribute_randomizer_bom.toml";
const CONFIG_AUTHOR: &str = "梅琳娜的刀";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModPaths {
    pub config_path: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct PlayerAttributeRandomizerBomConfig {
    pub attribute: AttributeConfig,
    pub overlay: OverlayConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct AttributeConfig {
    pub enable: bool,
    /// Minimum delay between attribute changes, in milliseconds.
    pub randomize_interval_min_ms: u64,
    /// Maximum delay between attribute changes, in milliseconds.
    pub randomize_interval_max_ms: u64,
    pub toggle_virtual_key: i32,
}

impl Default for PlayerAttributeRandomizerBomConfig {
    fn default() -> Self {
        Self {
            attribute: AttributeConfig::default(),
            overlay: OverlayConfig::default(),
        }
    }
}

impl Default for AttributeConfig {
    fn default() -> Self {
        Self {
            enable: true,
            randomize_interval_min_ms: 3_000,
            randomize_interval_max_ms: 7_000,
            toggle_virtual_key: 0x72,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct OverlayConfig {
    /// Horizontal offset from the game's top-left corner, in pixels.
    pub offset_x: f32,
    /// Vertical offset from the game's top-left corner, in pixels.
    pub offset_y: f32,
    /// Overlay height in pixels before applying scale.
    pub height: f32,
    /// Whether the countdown occupies the left side of the overlay.
    pub show_countdown: bool,
    /// Distance between the countdown text and the attribute panel, in pixels
    /// before applying scale.
    pub countdown_attribute_gap: f32,
    /// Distance between the large attribute text and the small vertical
    /// "属性" label, in pixels before applying scale.
    pub attribute_text_gap: f32,
    /// ImGui font/window scale. Values are clamped to 0.25..=4.0 when drawn.
    pub scale: f32,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            offset_x: 24.0,
            offset_y: 24.0,
            height: 96.0,
            show_countdown: true,
            countdown_attribute_gap: 16.0,
            attribute_text_gap: 16.0,
            scale: 1.0,
        }
    }
}

pub fn resolve_paths(hmodule_raw: usize) -> ModPaths {
    let dll_path = dll_path_from_module(hmodule_raw);
    mod_paths_from_dll_path(&dll_path)
}

pub fn load_or_create_config(path: &Path) -> PlayerAttributeRandomizerBomConfig {
    if !path.exists() {
        let config = PlayerAttributeRandomizerBomConfig::default();
        write_default_config(path, &config);
        return config;
    }

    match load_config(path) {
        Some(config) => config,
        None => {
            let config = PlayerAttributeRandomizerBomConfig::default();
            write_default_config(path, &config);
            config
        }
    }
}

pub fn load_config(path: &Path) -> Option<PlayerAttributeRandomizerBomConfig> {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| toml::from_str::<PlayerAttributeRandomizerBomConfig>(&text).ok())
}

pub fn config_modified_time(path: &Path) -> Option<SystemTime> {
    fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
}

fn write_default_config(path: &Path, config: &PlayerAttributeRandomizerBomConfig) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let mut text = format!("author = {CONFIG_AUTHOR:?}\n\n");
    if let Ok(config_text) = toml::to_string_pretty(config) {
        text.push_str(&config_text);
    }
    let _ = fs::write(path, text);
}

fn dll_path_from_module(hmodule_raw: usize) -> PathBuf {
    let hmodule = HMODULE(hmodule_raw as *mut c_void);
    let mut path_buffer = vec![0u16; 260];

    loop {
        let len = unsafe { GetModuleFileNameW(Some(hmodule), &mut path_buffer) } as usize;
        if len == 0 {
            return PathBuf::from(".");
        }
        if len < path_buffer.len() {
            return PathBuf::from(String::from_utf16_lossy(&path_buffer[..len]));
        }
        if path_buffer.len() >= 32_768 {
            return PathBuf::from(".");
        }
        path_buffer.resize((path_buffer.len() * 2).min(32_768), 0);
    }
}

fn mod_paths_from_dll_path(dll_path: &Path) -> ModPaths {
    let dll_dir = dll_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));

    ModPaths {
        config_path: dll_dir.join(CONFIG_FILE_NAME),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_config_contains_attribute_settings() {
        let path = std::env::temp_dir().join("player-attribute-randomizer-bom-config-test.toml");
        let _ = fs::remove_file(&path);
        write_default_config(&path, &PlayerAttributeRandomizerBomConfig::default());
        let text = fs::read_to_string(&path).unwrap();

        assert!(text.starts_with("author = \"梅琳娜的刀\"\n\n[attribute]\n"));
        assert!(text.contains("enable = true"));
        assert!(text.contains("randomize_interval_min_ms = 3000"));
        assert!(text.contains("randomize_interval_max_ms = 7000"));
        assert!(text.contains("toggle_virtual_key = 114"));
        assert!(text.contains("[overlay]"));
        assert!(text.contains("offset_x = 24.0"));
        assert!(text.contains("offset_y = 24.0"));
        assert!(text.contains("height = 96.0"));
        assert!(text.contains("show_countdown = true"));
        assert!(text.contains("countdown_attribute_gap = 16.0"));
        assert!(text.contains("attribute_text_gap = 16.0"));
        assert!(text.contains("scale = 1.0"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn paths_use_dll_directory() {
        let dll_path = Path::new(r"D:\Games\EldenRing\mods\player_attribute_randomizer_bom.dll");
        let paths = mod_paths_from_dll_path(dll_path);

        assert_eq!(
            paths.config_path,
            PathBuf::from(r"D:\Games\EldenRing\mods\player_attribute_randomizer_bom.toml")
        );
    }
}
