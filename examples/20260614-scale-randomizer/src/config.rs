use ini::Ini;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::ffi::c_void;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;

pub const HOT_RELOAD_DEBOUNCE_MS: u64 = 200;
pub const DEFAULT_MAX_PLAYER_SCALE: u32 = 150;
pub const DEFAULT_MIN_PLAYER_SCALE: u32 = 25;
pub const DEFAULT_UPDATE_INTERVAL_MS: u64 = 5_000;
pub const MAX_ALLOWED_PLAYER_SCALE: u32 = 10_000;
pub const MAX_ALLOWED_UPDATE_INTERVAL_MS: u64 = 10_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScaleRandomizerOptions {
    pub max_player_scale: u32,
    pub min_player_scale: u32,
    pub update_interval_ms: u64,
}

impl Default for ScaleRandomizerOptions {
    fn default() -> Self {
        Self {
            max_player_scale: DEFAULT_MAX_PLAYER_SCALE,
            min_player_scale: DEFAULT_MIN_PLAYER_SCALE,
            update_interval_ms: DEFAULT_UPDATE_INTERVAL_MS,
        }
    }
}

impl ScaleRandomizerOptions {
    pub fn sanitize(&mut self) {
        self.max_player_scale = self.max_player_scale.clamp(0, MAX_ALLOWED_PLAYER_SCALE);
        self.min_player_scale = self.min_player_scale.clamp(0, MAX_ALLOWED_PLAYER_SCALE);

        if self.min_player_scale > self.max_player_scale {
            std::mem::swap(&mut self.min_player_scale, &mut self.max_player_scale);
        }

        self.update_interval_ms = self
            .update_interval_ms
            .clamp(1, MAX_ALLOWED_UPDATE_INTERVAL_MS);
    }
}

pub fn get_config_path(hmodule_raw: usize) -> PathBuf {
    let mut path_buffer = [0u16; 260];
    let hmodule = HMODULE(hmodule_raw as *mut c_void);
    let len = unsafe { GetModuleFileNameW(Some(hmodule), &mut path_buffer) } as usize;
    let dll_path = PathBuf::from(String::from_utf16_lossy(&path_buffer[..len]));
    let stem = dll_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    dll_path
        .parent()
        .unwrap_or(Path::new("."))
        .join(format!("{}_config.ini", stem))
}

pub fn read_write_ini(config_path: &Path) -> ScaleRandomizerOptions {
    if !config_path.exists() {
        let defaults = ScaleRandomizerOptions::default();
        let _ = write_ini(config_path, &defaults);
        return defaults;
    }

    let options = read_ini(config_path);

    let needs_rewrite = std::fs::read_to_string(config_path)
        .map(|text| {
            !text.contains("max_player_scale")
                || !text.contains("min_player_scale")
                || !text.contains("update_interval_ms")
        })
        .unwrap_or(true);

    if needs_rewrite {
        let _ = write_ini(config_path, &options);
    }

    options
}

pub fn read_ini(config_path: &Path) -> ScaleRandomizerOptions {
    let conf = Ini::load_from_file(config_path).unwrap_or_else(|_| Ini::new());
    let mut options = ScaleRandomizerOptions::default();

    if let Some(section) = conf.section(Some("Settings")) {
        options.max_player_scale = parse_percent_value(
            section.get("max_player_scale").map(|v| v.as_ref()),
            options.max_player_scale,
        );
        options.min_player_scale = parse_percent_value(
            section.get("min_player_scale").map(|v| v.as_ref()),
            options.min_player_scale,
        );
        options.update_interval_ms = parse_u64_value(
            section.get("update_interval_ms").map(|v| v.as_ref()),
            options.update_interval_ms,
        );
    }

    options.sanitize();
    options
}

pub fn write_ini(config_path: &Path, options: &ScaleRandomizerOptions) -> std::io::Result<()> {
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut text = String::new();
    text.push_str("; Edit this file while the game is running to live reload changes.\n");
    text.push_str("; This mod randomizes the player's scale by rewriting playerScale in Resizer_config.ini.\n");
    text.push('\n');
    writeln!(text, "[Settings]").ok();
    text.push('\n');
    text.push_str("; Maximum playerScale percentage. Accepts a value with or without a % sign. (EX: 150 or 150%).\n");
    writeln!(text, "max_player_scale = {}%", options.max_player_scale).ok();
    text.push('\n');
    text.push_str("; Minimum playerScale percentage. Accepts a value with or without a % sign. (EX: 25 or 25%).\n");
    writeln!(text, "min_player_scale = {}%", options.min_player_scale).ok();
    text.push('\n');
    text.push_str(
        "; How often to randomize playerScale and update Resizer_config.ini, in milliseconds.\n",
    );
    writeln!(text, "update_interval_ms = {}", options.update_interval_ms).ok();

    std::fs::write(config_path, text)
}

pub fn start_hot_reload_thread(
    config_path: PathBuf,
    settings: Arc<RwLock<ScaleRandomizerOptions>>,
) {
    std::thread::spawn(move || {
        let mut last_update = Instant::now() - Duration::from_millis(HOT_RELOAD_DEBOUNCE_MS);

        let mut watcher: RecommendedWatcher = match notify::recommended_watcher({
            let settings = Arc::clone(&settings);
            let config_path = config_path.clone();
            move |res: notify::Result<notify::Event>| {
                if let Ok(event) = res {
                    match event.kind {
                        notify::EventKind::Modify(_) | notify::EventKind::Create(_) => {
                            if last_update.elapsed() < Duration::from_millis(HOT_RELOAD_DEBOUNCE_MS)
                            {
                                return;
                            }
                            last_update = Instant::now();
                            let new_settings = read_ini(&config_path);
                            if let Ok(mut guard) = settings.write() {
                                *guard = new_settings;
                            }
                            println!("[scale_randomizer] Config reloaded");
                        }
                        _ => {}
                    }
                }
            }
        }) {
            Ok(w) => w,
            Err(_) => return,
        };

        if watcher
            .watch(&config_path, RecursiveMode::NonRecursive)
            .is_err()
        {
            return;
        }

        loop {
            std::thread::park();
        }
    });
}

fn parse_percent_value(value: Option<&str>, default: u32) -> u32 {
    let Some(raw) = value else {
        return default;
    };

    let trimmed = raw.trim();
    let without_percent = trimmed.strip_suffix('%').unwrap_or(trimmed).trim();

    without_percent.parse::<u32>().unwrap_or(default)
}

fn parse_u64_value(value: Option<&str>, default: u64) -> u64 {
    value
        .map(|v| v.trim().replace('_', ""))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_ini_accepts_percent_and_plain_numbers() {
        let temp_dir = std::env::temp_dir().join("scale-randomizer-config-parse");
        let path = temp_dir.join("config.ini");
        let _ = std::fs::create_dir_all(&temp_dir);
        std::fs::write(
            &path,
            "[Settings]\nmax_player_scale = 150%\nmin_player_scale = 25\nupdate_interval_ms = 5000\n",
        )
        .unwrap();

        let options = read_ini(&path);
        assert_eq!(options.max_player_scale, 150);
        assert_eq!(options.min_player_scale, 25);
        assert_eq!(options.update_interval_ms, 5000);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    #[test]
    fn read_ini_normalizes_inverted_scale_bounds() {
        let temp_dir = std::env::temp_dir().join("scale-randomizer-config-swap");
        let path = temp_dir.join("config.ini");
        let _ = std::fs::create_dir_all(&temp_dir);
        std::fs::write(
            &path,
            "[Settings]\nmax_player_scale = 25\nmin_player_scale = 150\nupdate_interval_ms = 5000\n",
        )
        .unwrap();

        let options = read_ini(&path);
        assert_eq!(options.min_player_scale, 25);
        assert_eq!(options.max_player_scale, 150);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    #[test]
    fn read_ini_uses_defaults_for_missing_or_invalid_values() {
        let temp_dir = std::env::temp_dir().join("scale-randomizer-config-defaults");
        let path = temp_dir.join("config.ini");
        let _ = std::fs::create_dir_all(&temp_dir);
        std::fs::write(
            &path,
            "[Settings]\nmax_player_scale = nope\nupdate_interval_ms = invalid\n",
        )
        .unwrap();

        let options = read_ini(&path);
        assert_eq!(options.max_player_scale, DEFAULT_MAX_PLAYER_SCALE);
        assert_eq!(options.min_player_scale, DEFAULT_MIN_PLAYER_SCALE);
        assert_eq!(options.update_interval_ms, DEFAULT_UPDATE_INTERVAL_MS);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&temp_dir);
    }
}
