use std::{
    ffi::c_void,
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use serde::{Deserialize, Serialize};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;

pub const CONFIG_FILE_NAME: &str = "game_speed_randomizer_dom.toml";
const CONFIG_AUTHOR: &str = "梅琳娜的刀";
const HTML_FILE_NAME: &str = "game_speed_randomizer_dom.html";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModPaths {
    pub config_path: PathBuf,
    pub html_path: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct GameSpeedRandomizerDomConfig {
    pub speed: GameSpeedConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct GameSpeedConfig {
    pub enable: bool,
    pub min_percent: u32,
    pub max_percent: u32,
    pub randomize_interval_ms: u64,
    pub toggle_virtual_key: i32,
}

impl Default for GameSpeedRandomizerDomConfig {
    fn default() -> Self {
        Self {
            speed: GameSpeedConfig::default(),
        }
    }
}

impl Default for GameSpeedConfig {
    fn default() -> Self {
        Self {
            enable: true,
            min_percent: 50,
            max_percent: 150,
            randomize_interval_ms: 5_000,
            toggle_virtual_key: 0x72,
        }
    }
}

pub fn resolve_paths(hmodule_raw: usize) -> ModPaths {
    let dll_path = dll_path_from_module(hmodule_raw);
    mod_paths_from_dll_path(&dll_path)
}

pub fn load_or_create_config(path: &Path) -> GameSpeedRandomizerDomConfig {
    if !path.exists() {
        let config = GameSpeedRandomizerDomConfig::default();
        write_default_config(path, &config);
        return config;
    }

    match load_config(path) {
        Some(config) => config,
        None => {
            let config = GameSpeedRandomizerDomConfig::default();
            write_default_config(path, &config);
            config
        }
    }
}

pub fn load_config(path: &Path) -> Option<GameSpeedRandomizerDomConfig> {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| toml::from_str::<GameSpeedRandomizerDomConfig>(&text).ok())
}

pub fn config_modified_time(path: &Path) -> Option<SystemTime> {
    fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
}

fn write_default_config(path: &Path, config: &GameSpeedRandomizerDomConfig) {
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
        html_path: dll_dir.join(HTML_FILE_NAME),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_config_contains_only_game_speed_settings() {
        let path = std::env::temp_dir().join("game-speed-randomizer-dom-config-test.toml");
        let _ = fs::remove_file(&path);
        write_default_config(&path, &GameSpeedRandomizerDomConfig::default());
        let text = fs::read_to_string(&path).unwrap();

        assert!(text.starts_with("author = \"梅琳娜的刀\"\n\n[speed]\n"));
        assert!(text.contains("enable = true"));
        assert!(text.contains("min_percent = 50"));
        assert!(text.contains("max_percent = 150"));
        assert!(text.contains("randomize_interval_ms = 5000"));
        assert!(text.contains("toggle_virtual_key = 114"));
        assert!(!text.contains("player"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn paths_use_dll_directory() {
        let dll_path = Path::new(r"D:\Games\EldenRing\mods\game_speed_randomizer_dom.dll");
        let paths = mod_paths_from_dll_path(dll_path);

        assert_eq!(
            paths.config_path,
            PathBuf::from(r"D:\Games\EldenRing\mods\game_speed_randomizer_dom.toml")
        );
        assert_eq!(
            paths.html_path,
            PathBuf::from(r"D:\Games\EldenRing\mods\game_speed_randomizer_dom.html")
        );
    }
}
