use std::{
    ffi::c_void,
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use serde::{Deserialize, Serialize};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;

use crate::weapon_pools::DEFAULT_WEPMOTION_CATEGORIES;

pub const CONFIG_FILE_NAME: &str = "hot-randomizer.toml";
pub const LOG_FILE_NAME: &str = "hot-randomizer.log";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModPaths {
    pub config_path: PathBuf,
    pub log_path: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct HotRandomizerConfig {
    pub general: GeneralConfig,
    pub weapon: WeaponRandomizerConfig,
    pub speed: SpeedRandomizerConfig,
    pub parts: PartsRandomizerConfig,
    pub spell: SpellRandomizerConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub input_check_interval_millis: u64,
    /// 调试日志默认关闭，也不会写入自动生成的配置文件。
    /// 如需日志，请在 [general] 下手动添加 `dbg_log = true`。
    #[serde(skip_serializing)]
    pub dbg_log: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct WeaponRandomizerConfig {
    pub enable_left_hand: bool,
    pub enable_right_hand: bool,
    /// 是否允许失色/特殊武器进入随机武器池。
    pub include_unique_weapons: bool,
    pub randomize_interval_ms: u64,
    /// 是否给当前随机到的武器再随机一份战灰。
    pub randomize_ashes: bool,
    /// 打开后忽略战灰兼容性限制，并允许失色/特殊武器也被强制装上战灰。
    pub ignore_ash_compatibility: bool,
    /// 调试模式：武器和战灰都固定走一个很小的测试池。
    pub debug_fixed_pool: bool,
    /// 调试模式：武器仍走正常大池，但战灰只从小测试池里挑。
    pub debug_fixed_ash_pool: bool,
    pub scale_to_player_level_cap: u32,
    pub enabled_wepmotion_categories: Vec<u16>,
    pub toggle_left_virtual_key: i32,
    pub toggle_right_virtual_key: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct SpeedRandomizerConfig {
    pub enable_global_speed: bool,
    pub global_speed_min_percent: u32,
    pub global_speed_max_percent: u32,
    pub global_speed_randomize_interval_ms: u64,
    pub enable_player_speed: bool,
    pub player_speed_min_percent: u32,
    pub player_speed_max_percent: u32,
    pub player_speed_randomize_interval_ms: u64,
    pub toggle_virtual_key: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct PartsRandomizerConfig {
    pub enable: bool,
    pub randomize_interval_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct SpellRandomizerConfig {
    pub enable: bool,
    pub randomize_interval_ms: u64,
}

impl Default for HotRandomizerConfig {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            weapon: WeaponRandomizerConfig::default(),
            speed: SpeedRandomizerConfig::default(),
            parts: PartsRandomizerConfig::default(),
            spell: SpellRandomizerConfig::default(),
        }
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            input_check_interval_millis: 500,
            dbg_log: false,
        }
    }
}

impl Default for WeaponRandomizerConfig {
    fn default() -> Self {
        Self {
            enable_left_hand: true,
            enable_right_hand: true,
            include_unique_weapons: true,
            randomize_interval_ms: 5_000,
            randomize_ashes: true,
            ignore_ash_compatibility: false,
            debug_fixed_pool: false,
            debug_fixed_ash_pool: false,
            scale_to_player_level_cap: 80,
            enabled_wepmotion_categories: default_weapon_categories(),
            toggle_left_virtual_key: 0x70,
            toggle_right_virtual_key: 0x71,
        }
    }
}

impl Default for SpeedRandomizerConfig {
    fn default() -> Self {
        Self {
            enable_global_speed: false,
            global_speed_min_percent: 50,
            global_speed_max_percent: 150,
            global_speed_randomize_interval_ms: 5_000,
            enable_player_speed: false,
            player_speed_min_percent: 50,
            player_speed_max_percent: 150,
            player_speed_randomize_interval_ms: 5_000,
            toggle_virtual_key: 0x72,
        }
    }
}

impl Default for PartsRandomizerConfig {
    fn default() -> Self {
        Self {
            enable: false,
            randomize_interval_ms: 5_000,
        }
    }
}

impl Default for SpellRandomizerConfig {
    fn default() -> Self {
        Self {
            enable: false,
            randomize_interval_ms: 5_000,
        }
    }
}

pub fn resolve_paths(hmodule_raw: usize) -> ModPaths {
    let dll_path = dll_path_from_module(hmodule_raw);
    mod_paths_from_dll_path(&dll_path)
}

pub fn load_or_create_config(path: &Path) -> HotRandomizerConfig {
    if !path.exists() {
        let config = HotRandomizerConfig::default();
        write_default_config(path, &config);
        return config;
    }

    match load_config(path) {
        Some(config) => config,
        None => {
            let config = HotRandomizerConfig::default();
            write_default_config(path, &config);
            config
        }
    }
}

pub fn load_config(path: &Path) -> Option<HotRandomizerConfig> {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| toml::from_str::<HotRandomizerConfig>(&text).ok())
}

pub fn config_modified_time(path: &Path) -> Option<SystemTime> {
    fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
}

fn write_default_config(path: &Path, config: &HotRandomizerConfig) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    if let Ok(text) = toml::to_string_pretty(config) {
        let _ = fs::write(path, text);
    }
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
        log_path: dll_dir.join(LOG_FILE_NAME),
    }
}

fn default_weapon_categories() -> Vec<u16> {
    DEFAULT_WEPMOTION_CATEGORIES.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_config_uses_general_section_and_omits_dbg_log() {
        let text = toml::to_string_pretty(&HotRandomizerConfig::default()).unwrap();

        assert!(text.starts_with("[general]\n"));
        assert!(text.contains("input_check_interval_millis = 500"));
        assert!(!text.contains("dbg_log"));
        assert!(text.contains("[weapon]"));
        assert!(text.contains("enable_left_hand = true"));
        assert!(text.contains("enable_right_hand = true"));
        assert!(text.contains("randomize_interval_ms = 5000"));
        assert!(!text.contains("randomize_interval_seconds"));
        assert!(text.contains("[speed]"));
        assert!(text.contains("enable_global_speed = false"));
        assert!(text.contains("global_speed_min_percent = 50"));
        assert!(text.contains("global_speed_max_percent = 150"));
        assert!(text.contains("enable_player_speed = false"));
        assert!(text.contains("player_speed_min_percent = 50"));
        assert!(text.contains("player_speed_max_percent = 150"));
        assert!(text.contains("toggle_virtual_key = 114"));
        assert!(text.contains("[parts]\nenable = false"));
        assert!(text.contains("[spell]\nenable = false"));
        assert!(!text.contains("allow_left_hand"));
        assert!(!text.contains("allow_right_hand"));
    }

    #[test]
    fn dbg_log_can_be_enabled_manually_under_general() {
        let config: HotRandomizerConfig =
            toml::from_str("[general]\ninput_check_interval_millis = 500\ndbg_log = true\n")
                .unwrap();

        assert!(config.general.dbg_log);
    }

    #[test]
    fn mod_paths_use_dll_directory() {
        let dll_path = Path::new(r"D:\Games\EldenRing\mods\hot-randomizer.dll");
        let paths = mod_paths_from_dll_path(dll_path);

        assert_eq!(
            paths.config_path,
            PathBuf::from(r"D:\Games\EldenRing\mods\hot-randomizer.toml")
        );
        assert_eq!(
            paths.log_path,
            PathBuf::from(r"D:\Games\EldenRing\mods\hot-randomizer.log")
        );
    }
}
