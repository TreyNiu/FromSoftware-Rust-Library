use std::{
    ffi::c_void,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock, RwLock},
    time::SystemTime,
};

use ini::Ini;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;

pub const CONFIG_FILE_NAME: &str = "Resizer_config.ini";
const MIN_SCALE: f32 = 0.01;
const MAX_SCALE: f32 = 100.0;
const MAX_MOVEMENT_INFLUENCE: f32 = 4.0;

static GLOBAL_CONFIG: OnceLock<Arc<RwLock<ResizerConfig>>> = OnceLock::new();

#[derive(Clone, Debug, PartialEq)]
pub struct ResizerConfig {
    pub player_scale: f32,
    pub camera_scale: f32,
    pub weapon_scale: f32,
    pub torrent_scale: f32,
    pub mimic_scale: f32,
    pub summon_scale: f32,
    pub movement_influence: f32,
    pub no_fall_damage: bool,
    /// Rust-only extension. It can be set to false while debugging if the current game
    /// version has a different Havok skeleton layout.
    pub scale_bones: bool,
}

impl Default for ResizerConfig {
    fn default() -> Self {
        Self {
            player_scale: 1.0,
            camera_scale: 1.0,
            weapon_scale: 1.0,
            torrent_scale: 1.0,
            mimic_scale: 1.0,
            summon_scale: 1.0,
            movement_influence: 1.0,
            no_fall_damage: false,
            scale_bones: true,
        }
    }
}

impl ResizerConfig {
    fn sanitize(&mut self) {
        self.player_scale = sanitize_scale(self.player_scale);
        self.camera_scale = sanitize_scale(self.camera_scale);
        self.weapon_scale = sanitize_scale(self.weapon_scale);
        self.torrent_scale = sanitize_scale(self.torrent_scale);
        self.mimic_scale = sanitize_scale(self.mimic_scale);
        self.summon_scale = sanitize_scale(self.summon_scale);
        self.movement_influence =
            sanitize_finite(self.movement_influence, 1.0).clamp(0.0, MAX_MOVEMENT_INFLUENCE);
    }
}

pub fn config_path_from_module(hmodule_raw: usize) -> PathBuf {
    let hmodule = HMODULE(hmodule_raw as *mut c_void);
    let mut buffer = vec![0u16; 260];

    loop {
        let len = unsafe { GetModuleFileNameW(Some(hmodule), &mut buffer) } as usize;
        if len == 0 {
            return PathBuf::from(CONFIG_FILE_NAME);
        }
        if len < buffer.len() {
            let dll_path = PathBuf::from(String::from_utf16_lossy(&buffer[..len]));
            return dll_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(CONFIG_FILE_NAME);
        }
        if buffer.len() >= 32_768 {
            return PathBuf::from(CONFIG_FILE_NAME);
        }
        buffer.resize((buffer.len() * 2).min(32_768), 0);
    }
}

pub fn set_global_config(config: Arc<RwLock<ResizerConfig>>) {
    let _ = GLOBAL_CONFIG.set(config);
}

pub fn global_config() -> Option<&'static Arc<RwLock<ResizerConfig>>> {
    GLOBAL_CONFIG.get()
}

pub fn current_config() -> ResizerConfig {
    global_config()
        .and_then(|config| config.read().ok().map(|guard| guard.clone()))
        .unwrap_or_default()
}

pub fn load_or_create_config(path: &Path) -> ResizerConfig {
    if !path.exists() {
        let config = ResizerConfig::default();
        let _ = write_default_config(path, &config);
        return config;
    }

    load_config(path).unwrap_or_else(|| {
        let config = ResizerConfig::default();
        let _ = write_default_config(path, &config);
        config
    })
}

pub fn load_config(path: &Path) -> Option<ResizerConfig> {
    let text = fs::read_to_string(path).ok()?;
    let ini = Ini::load_from_str(&text).ok()?;
    let section = ini
        .section(Some("settings"))
        .or_else(|| ini.section(Some("Settings")));

    let get = |key: &str| section.and_then(|section| section.get(key));
    let player_scale = parse_scale(get("playerScale")).unwrap_or(1.0);

    let mut config = ResizerConfig {
        player_scale,
        camera_scale: parse_reference_scale(get("cameraScale"), player_scale)
            .unwrap_or(player_scale),
        weapon_scale: parse_reference_scale(get("weaponScale"), player_scale)
            .unwrap_or(player_scale),
        torrent_scale: parse_reference_scale(get("torrentScale"), player_scale)
            .unwrap_or(player_scale),
        mimic_scale: parse_reference_scale(get("mimicScale"), player_scale).unwrap_or(player_scale),
        summon_scale: parse_reference_scale(get("summonScale"), player_scale)
            .unwrap_or(player_scale),
        movement_influence: parse_scale(get("movementInfluence")).unwrap_or(1.0),
        no_fall_damage: parse_no_fall_damage(get("noFallDamage"), player_scale),
        scale_bones: parse_bool(get("scaleBones")).unwrap_or(true),
    };
    config.sanitize();
    Some(config)
}

pub fn config_modified_time(path: &Path) -> Option<SystemTime> {
    fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
}

fn parse_scale(value: Option<&str>) -> Option<f32> {
    let raw = value?.trim();
    let without_percent = raw.strip_suffix('%').unwrap_or(raw).trim();
    let number = without_percent.parse::<f32>().ok()?;
    number.is_finite().then_some(number / 100.0)
}

fn parse_reference_scale(value: Option<&str>, player_scale: f32) -> Option<f32> {
    let raw = value?.trim();
    match raw.to_ascii_lowercase().as_str() {
        "player" => Some(player_scale),
        "auto" => Some(player_scale.max(1.0)),
        _ => parse_scale(Some(raw)),
    }
}

fn parse_no_fall_damage(value: Option<&str>, player_scale: f32) -> bool {
    match value.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("true") | Some("1") | Some("yes") | Some("on") => true,
        Some("false") | Some("0") | Some("no") | Some("off") => false,
        // Matches the original behavior: auto enables the protection at 200% scale or higher.
        _ => player_scale >= 2.0,
    }
}

fn parse_bool(value: Option<&str>) -> Option<bool> {
    match value.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("true") | Some("1") | Some("yes") | Some("on") => Some(true),
        Some("false") | Some("0") | Some("no") | Some("off") => Some(false),
        _ => None,
    }
}

fn sanitize_scale(value: f32) -> f32 {
    sanitize_finite(value, 1.0).clamp(MIN_SCALE, MAX_SCALE)
}

fn sanitize_finite(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

fn write_default_config(path: &Path, config: &ResizerConfig) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let text = format!(
        r#"; Configuration for resizer-rs.
; Changes are hot-reloaded while the game is running.

[settings]

; Player size multiplier. 50% = half size, 200% = double size.
playerScale = {player:.0}%

; Camera distance multiplier. Use 'player' to follow playerScale.
cameraScale = player

; Equipped weapon size and direct attack hitbox multiplier.
; 'auto' uses max(playerScale, 100%).
weaponScale = player

; Torrent size multiplier. Use 'player' to follow playerScale.
torrentScale = player

; Mimic Tear size multiplier.
mimicScale = player

; Other spirit summon size multiplier.
summonScale = player

; Root-motion multiplier is pow(scale, movementInfluence).
movementInfluence = 100%

; true / false / auto. 'auto' enables at playerScale >= 200%.
noFallDamage = auto

; Rust extension: scale Havok bone lengths and thickness.
; Set to false to test only ChrCtrl debug scaling on a new game version.
scaleBones = {scale_bones}
"#,
        player = config.player_scale * 100.0,
        scale_bones = config.scale_bones,
    );
    fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_reference_and_auto_scales() {
        let temp = std::env::temp_dir().join("resizer-rs-config-parse");
        let path = temp.join(CONFIG_FILE_NAME);
        let _ = fs::create_dir_all(&temp);
        fs::write(
            &path,
            "[settings]\nplayerScale = 200%\ncameraScale = player\nweaponScale = auto\ntorrentScale = 50\nnoFallDamage = auto\n",
        )
        .unwrap();

        let config = load_config(&path).unwrap();
        assert!((config.player_scale - 2.0).abs() < f32::EPSILON);
        assert!((config.camera_scale - 2.0).abs() < f32::EPSILON);
        assert!((config.weapon_scale - 2.0).abs() < f32::EPSILON);
        assert!((config.torrent_scale - 0.5).abs() < f32::EPSILON);
        assert!(config.no_fall_damage);

        let _ = fs::remove_file(path);
        let _ = fs::remove_dir(temp);
    }

    #[test]
    fn weapon_auto_never_scales_down() {
        let temp = std::env::temp_dir().join("resizer-rs-config-auto");
        let path = temp.join(CONFIG_FILE_NAME);
        let _ = fs::create_dir_all(&temp);
        fs::write(&path, "[settings]\nplayerScale = 50%\nweaponScale = auto\n").unwrap();

        let config = load_config(&path).unwrap();
        assert!((config.player_scale - 0.5).abs() < f32::EPSILON);
        assert!((config.weapon_scale - 1.0).abs() < f32::EPSILON);
        assert!(!config.no_fall_damage);

        let _ = fs::remove_file(path);
        let _ = fs::remove_dir(temp);
    }
}
