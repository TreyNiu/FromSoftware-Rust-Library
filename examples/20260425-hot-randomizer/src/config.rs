use std::{fs, path::Path, time::SystemTime};

use serde::{Deserialize, Serialize};

use crate::weapon_pools::DEFAULT_WEPMOTION_CATEGORIES;

const CONFIG_PATH: &str = "hot-randomizer.toml";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct HotRandomizerConfig {
    pub input_check_interval_millis: u64,
    pub weapon: WeaponRandomizerConfig,
    pub parts: PartsRandomizerConfig,
    pub spell: SpellRandomizerConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct WeaponRandomizerConfig {
    #[serde(alias = "enable_left_hand")]
    pub allow_left_hand: bool,
    #[serde(alias = "enable_right_hand")]
    pub allow_right_hand: bool,
    /// 是否允许失色/特殊武器进入随机武器池。
    pub include_unique_weapons: bool,
    pub randomize_interval_seconds: u64,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct PartsRandomizerConfig {
    pub allow: bool,
    pub randomize_interval_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct SpellRandomizerConfig {
    pub allow: bool,
    pub randomize_interval_seconds: u64,
}

impl Default for HotRandomizerConfig {
    fn default() -> Self {
        Self {
            input_check_interval_millis: 500,
            weapon: WeaponRandomizerConfig::default(),
            parts: PartsRandomizerConfig::default(),
            spell: SpellRandomizerConfig::default(),
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

impl Default for PartsRandomizerConfig {
    fn default() -> Self {
        Self {
            allow: false,
            randomize_interval_seconds: 5,
        }
    }
}

impl Default for SpellRandomizerConfig {
    fn default() -> Self {
        Self {
            allow: false,
            randomize_interval_seconds: 5,
        }
    }
}

pub fn load_or_create_config() -> HotRandomizerConfig {
    let path = Path::new(CONFIG_PATH);
    if !path.exists() {
        let config = HotRandomizerConfig::default();
        write_default_config(path, &config);
        return config;
    }

    match load_config() {
        Some(config) => config,
        None => {
            let config = HotRandomizerConfig::default();
            write_default_config(path, &config);
            config
        }
    }
}

pub fn load_config() -> Option<HotRandomizerConfig> {
    fs::read_to_string(CONFIG_PATH)
        .ok()
        .and_then(|text| toml::from_str::<HotRandomizerConfig>(&text).ok())
}

pub fn config_modified_time() -> Option<SystemTime> {
    fs::metadata(CONFIG_PATH)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
}

fn write_default_config(path: &Path, config: &HotRandomizerConfig) {
    if let Ok(text) = toml::to_string_pretty(config) {
        let _ = fs::write(path, text);
    }
}

fn default_weapon_categories() -> Vec<u16> {
    DEFAULT_WEPMOTION_CATEGORIES.to_vec()
}
