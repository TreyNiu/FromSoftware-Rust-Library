use std::{
    ffi::c_void,
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use serde::{Deserialize, Serialize};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;

pub const CONFIG_FILE_NAME: &str = "resizer_stamina_add_on.toml";
pub const HTML_FILE_NAME: &str = "resizer_stamina_add_on.html";
pub const RESIZER_CONFIG_FILE_NAME: &str = "Resizer_config.ini";
const CONFIG_AUTHOR: &str = "梅琳娜的刀";
const MAX_SCALE_PERCENT: u32 = 10_000;
const MAX_UPDATE_INTERVAL_FRAMES: u32 = 216_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModPaths {
    pub config_path: PathBuf,
    pub html_path: PathBuf,
    pub resizer_config_path: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct ResizerStaminaAddOnConfig {
    pub resizer: ResizerStaminaConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct ResizerStaminaConfig {
    pub update_interval_frames: u32,
    pub min_player_scale_percent: u32,
    pub max_player_scale_percent: u32,
    pub base_stamina: u32,
}

impl Default for ResizerStaminaAddOnConfig {
    fn default() -> Self {
        Self {
            resizer: ResizerStaminaConfig::default(),
        }
    }
}

impl Default for ResizerStaminaConfig {
    fn default() -> Self {
        Self {
            update_interval_frames: 5,
            min_player_scale_percent: 10,
            max_player_scale_percent: 300,
            base_stamina: 200,
        }
    }
}

impl ResizerStaminaConfig {
    pub fn sanitize(&mut self) {
        self.update_interval_frames = self
            .update_interval_frames
            .clamp(1, MAX_UPDATE_INTERVAL_FRAMES);
        self.min_player_scale_percent = self.min_player_scale_percent.min(MAX_SCALE_PERCENT);
        self.max_player_scale_percent = self.max_player_scale_percent.min(MAX_SCALE_PERCENT);
        if self.min_player_scale_percent > self.max_player_scale_percent {
            std::mem::swap(
                &mut self.min_player_scale_percent,
                &mut self.max_player_scale_percent,
            );
        }
        self.base_stamina = self.base_stamina.max(1);
    }
}

pub fn resolve_paths(hmodule_raw: usize) -> ModPaths {
    let dll_path = dll_path_from_module(hmodule_raw);
    mod_paths_from_dll_path(&dll_path)
}

pub fn load_or_create_config(path: &Path) -> ResizerStaminaAddOnConfig {
    if !path.exists() {
        let config = ResizerStaminaAddOnConfig::default();
        write_default_config(path, &config);
        return config;
    }

    match load_config(path) {
        Some(config) => config,
        None => {
            let config = ResizerStaminaAddOnConfig::default();
            write_default_config(path, &config);
            config
        }
    }
}

pub fn load_config(path: &Path) -> Option<ResizerStaminaAddOnConfig> {
    let mut config = fs::read_to_string(path)
        .ok()
        .and_then(|text| toml::from_str::<ResizerStaminaAddOnConfig>(&text).ok())?;
    config.resizer.sanitize();
    Some(config)
}

pub fn config_modified_time(path: &Path) -> Option<SystemTime> {
    fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
}

fn write_default_config(path: &Path, config: &ResizerStaminaAddOnConfig) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let settings = &config.resizer;
    let text = format!(
        r#"author = {CONFIG_AUTHOR:?}

[resizer]
# 每多少个游戏帧读取一次耐力并尝试更新 Resizer_config.ini。
update_interval_frames = {}

# 当耐力上限等于 base_stamina 时，空绿条和满绿条对应的体型百分比。
# 例如 10 表示 10%，300 表示 300%。
min_player_scale_percent = {}
max_player_scale_percent = {}

# 用于修正耐力上限的基准值：最终体型还会乘以 耐力上限 / base_stamina。
base_stamina = {}
"#,
        settings.update_interval_frames,
        settings.min_player_scale_percent,
        settings.max_player_scale_percent,
        settings.base_stamina,
    );
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
    let directory = dll_path.parent().unwrap_or_else(|| Path::new("."));
    ModPaths {
        config_path: directory.join(CONFIG_FILE_NAME),
        html_path: directory.join(HTML_FILE_NAME),
        resizer_config_path: directory.join(RESIZER_CONFIG_FILE_NAME),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_resolved_beside_the_dll() {
        let paths = mod_paths_from_dll_path(Path::new(r"C:\mods\resizer_stamina_add_on.dll"));
        assert_eq!(
            paths.config_path,
            PathBuf::from(r"C:\mods\resizer_stamina_add_on.toml")
        );
        assert_eq!(
            paths.html_path,
            PathBuf::from(r"C:\mods\resizer_stamina_add_on.html")
        );
        assert_eq!(
            paths.resizer_config_path,
            PathBuf::from(r"C:\mods\Resizer_config.ini")
        );
    }

    #[test]
    fn invalid_values_are_sanitized() {
        let temp_dir = std::env::temp_dir().join("resizer-stamina-config-sanitize");
        let path = temp_dir.join("config.toml");
        let _ = fs::create_dir_all(&temp_dir);
        fs::write(
            &path,
            "[resizer]\nupdate_interval_frames = 0\nmin_player_scale_percent = 300\nmax_player_scale_percent = 10\nbase_stamina = 0\n",
        )
        .unwrap();

        let config = load_config(&path).unwrap();
        assert_eq!(config.resizer.update_interval_frames, 1);
        assert_eq!(config.resizer.min_player_scale_percent, 10);
        assert_eq!(config.resizer.max_player_scale_percent, 300);
        assert_eq!(config.resizer.base_stamina, 1);

        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&temp_dir);
    }
}
