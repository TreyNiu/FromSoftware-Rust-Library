use std::{
    ffi::c_void,
    fs,
    path::{Path, PathBuf},
    fmt::Write as _,
    time::SystemTime,
};

use serde::{Deserialize, Serialize};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;

use crate::weapon_pools::DEFAULT_WEPMOTION_CATEGORIES;

pub const CONFIG_FILE_NAME: &str = "weapon_randomizer_bom.toml";
const DEFAULT_INPUT_CHECK_INTERVAL_MILLIS: u64 = 500;
const CONFIG_AUTHOR: &str = "梅琳娜的刀";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModPaths {
    pub config_path: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct WeaponRandomizerBomConfig {
    #[serde(skip)]
    pub input_check_interval_millis: u64,
    #[serde(flatten)]
    pub weapon: WeaponRandomizerConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct WeaponRandomizerConfig {
    #[serde(alias = "enable_left_hand")]
    pub allow_left_hand: bool,
    #[serde(alias = "enable_right_hand")]
    pub allow_right_hand: bool,
    /// 是否允许失色/特殊武器进入随机武器池。
    #[serde(skip)]
    pub include_unique_weapons: bool,
    pub randomize_interval_seconds: u64,
    /// 是否给当前随机到的武器再随机一份战灰。
    #[serde(skip)]
    pub randomize_ashes: bool,
    /// 打开后忽略战灰兼容性限制，并允许失色/特殊武器也被强制装上战灰。
    #[serde(skip)]
    pub ignore_ash_compatibility: bool,
    pub scale_to_player_level_cap: u32,
    pub enabled_wepmotion_categories: Vec<u16>,
    pub toggle_left_virtual_key: i32,
    pub toggle_right_virtual_key: i32,
}

impl Default for WeaponRandomizerBomConfig {
    fn default() -> Self {
        Self {
            input_check_interval_millis: DEFAULT_INPUT_CHECK_INTERVAL_MILLIS,
            weapon: WeaponRandomizerConfig::default(),
        }
    }
}

impl Default for WeaponRandomizerConfig {
    fn default() -> Self {
        Self {
            allow_left_hand: true,
            allow_right_hand: true,
            include_unique_weapons: true,
            randomize_interval_seconds: 5,
            randomize_ashes: true,
            ignore_ash_compatibility: true,
            scale_to_player_level_cap: 80,
            enabled_wepmotion_categories: default_weapon_categories(),
            toggle_left_virtual_key: 0x70,
            toggle_right_virtual_key: 0x71,
        }
    }
}

pub fn resolve_paths(hmodule_raw: usize) -> ModPaths {
    let dll_path = dll_path_from_module(hmodule_raw);
    mod_paths_from_dll_path(&dll_path)
}

pub fn load_or_create_config(path: &Path) -> WeaponRandomizerBomConfig {
    if !path.exists() {
        let config = WeaponRandomizerBomConfig::default();
        write_default_config(path, &config);
        return config;
    }

    match load_config(path) {
        Some(config) => config,
        None => {
            let config = WeaponRandomizerBomConfig::default();
            write_default_config(path, &config);
            config
        }
    }
}

pub fn load_config(path: &Path) -> Option<WeaponRandomizerBomConfig> {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| toml::from_str::<WeaponRandomizerBomConfig>(&text).ok())
}

pub fn config_modified_time(path: &Path) -> Option<SystemTime> {
    fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
}

fn write_default_config(path: &Path, config: &WeaponRandomizerBomConfig) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let _ = fs::write(path, render_default_config(config));
}

fn dll_path_from_module(hmodule_raw: usize) -> PathBuf {
    let mut path_buffer = [0u16; 260];
    let hmodule = HMODULE(hmodule_raw as *mut c_void);
    let len = unsafe { GetModuleFileNameW(Some(hmodule), &mut path_buffer) } as usize;
    PathBuf::from(String::from_utf16_lossy(&path_buffer[..len]))
}

fn mod_paths_from_dll_path(dll_path: &Path) -> ModPaths {
    let dll_dir = dll_path
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();

    ModPaths {
        config_path: dll_dir.join(CONFIG_FILE_NAME),
    }
}

fn default_weapon_categories() -> Vec<u16> {
    DEFAULT_WEPMOTION_CATEGORIES.to_vec()
}

fn render_default_config(config: &WeaponRandomizerBomConfig) -> String {
    let weapon = &config.weapon;
    let categories = weapon
        .enabled_wepmotion_categories
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(", ");

    let mut text = String::new();
    writeln!(text, "author = {:?}", CONFIG_AUTHOR).ok();
    text.push('\n');
    writeln!(text, "allow_left_hand = {}", weapon.allow_left_hand).ok();
    writeln!(text, "allow_right_hand = {}", weapon.allow_right_hand).ok();
    writeln!(
        text,
        "randomize_interval_seconds = {}",
        weapon.randomize_interval_seconds
    )
    .ok();
    writeln!(
        text,
        "scale_to_player_level_cap = {}",
        weapon.scale_to_player_level_cap
    )
    .ok();
    writeln!(text, "enabled_wepmotion_categories = [{categories}]").ok();
    writeln!(text, "toggle_left_virtual_key = {}", weapon.toggle_left_virtual_key).ok();
    writeln!(
        text,
        "toggle_right_virtual_key = {}",
        weapon.toggle_right_virtual_key
    )
    .ok();
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_config_omits_hidden_settings() {
        let config = WeaponRandomizerBomConfig::default();
        let text = render_default_config(&config);

        assert!(text.contains("author = \"梅琳娜的刀\""));
        assert!(text.contains("allow_left_hand = true"));
        assert!(text.contains("allow_right_hand = true"));
        assert!(text.contains("randomize_interval_seconds = 5"));
        assert!(text.contains("enabled_wepmotion_categories = [20, 21, 22"));
        assert!(!text.contains("include_unique_weapons"));
        assert!(!text.contains("randomize_ashes"));
        assert!(!text.contains("ignore_ash_compatibility"));
        assert!(!text.contains("input_check_interval_millis"));
    }

    #[test]
    fn mod_paths_use_dll_directory() {
        let dll_path = Path::new(r"D:\Games\EldenRing\mods\weapon_randomizer_bom.dll");
        let paths = mod_paths_from_dll_path(dll_path);

        assert_eq!(
            paths.config_path,
            PathBuf::from(r"D:\Games\EldenRing\mods\weapon_randomizer_bom.toml")
        );
    }

    #[test]
    fn load_or_create_config_creates_defaults() {
        let temp_dir = std::env::temp_dir().join("weapon-randomizer-bom-config");
        let path = temp_dir.join(CONFIG_FILE_NAME);
        let _ = fs::remove_file(&path);
        let _ = fs::create_dir_all(&temp_dir);

        let config = load_or_create_config(&path);

        assert!(path.exists());
        assert!(config.weapon.allow_left_hand);
        assert!(config.weapon.allow_right_hand);
        assert!(config.weapon.include_unique_weapons);
        assert!(config.weapon.randomize_ashes);
        assert!(config.weapon.ignore_ash_compatibility);

        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&temp_dir);
    }
}
