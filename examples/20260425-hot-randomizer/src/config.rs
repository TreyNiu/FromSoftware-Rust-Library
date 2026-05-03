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
    pub randomize_interval_seconds: u64,
    pub randomize_ashes: bool,
    pub debug_fixed_pool: bool,
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
            randomize_interval_seconds: 5,
            randomize_ashes: true,
            debug_fixed_pool: false,
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
