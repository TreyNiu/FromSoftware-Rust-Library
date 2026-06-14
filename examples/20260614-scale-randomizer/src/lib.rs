#![allow(non_snake_case)]

mod config;

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant},
};

use config::{ScaleRandomizerOptions, get_config_path, read_write_ini, start_hot_reload_thread};
use eldenring::{
    cs::{CSTaskGroupIndex, CSTaskImp},
    fd4::FD4TaskData,
    util::system::wait_for_system_init,
};
use fromsoftware_shared::{FromStatic, Program, SharedTaskImpExt};
use rand::Rng;

static LAST_WRITTEN_SCALE: Mutex<Option<u32>> = Mutex::new(None);
const RESIZER_CONFIG_FILE: &str = "Resizer_config.ini";

#[unsafe(no_mangle)]
/// # Safety
///
/// This is exposed this way such that Windows LoadLibrary API can call it. Do not call this yourself.
pub unsafe extern "C" fn DllMain(hmodule: usize, reason: u32) -> bool {
    if reason != 1 {
        return true;
    }

    std::thread::spawn(move || {
        wait_for_system_init(&Program::current(), Duration::MAX)
            .expect("Could not await system init.");

        let cs_task = unsafe { CSTaskImp::instance().unwrap() };

        let config_path = get_config_path(hmodule);
        let config_dir = config_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));

        let settings = Arc::new(RwLock::new(read_write_ini(&config_path)));
        start_hot_reload_thread(config_path, Arc::clone(&settings));

        let mut last_update = Instant::now() - Duration::from_millis(100);

        cs_task.run_recurring(
            move |_: &FD4TaskData| {
                let update_interval_ms = settings
                    .read()
                    .map(|guard| guard.update_interval_ms)
                    .unwrap_or(100);

                if last_update.elapsed() < Duration::from_millis(update_interval_ms) {
                    return;
                }
                last_update = Instant::now();

                update_resizer_scale(&settings, &config_dir);
            },
            CSTaskGroupIndex::FrameBegin,
        );
    });

    true
}

fn update_resizer_scale(settings: &Arc<RwLock<ScaleRandomizerOptions>>, config_dir: &Path) {
    let options = match settings.read() {
        Ok(guard) => guard.clone(),
        Err(_) => return,
    };

    let scale_percent = random_scale_percent(&options);

    let mut last = LAST_WRITTEN_SCALE.lock().unwrap();
    if *last == Some(scale_percent) {
        return;
    }
    *last = Some(scale_percent);
    drop(last);

    let resizer_config_path = config_dir.join(RESIZER_CONFIG_FILE);

    if !resizer_config_path.exists() {
        return;
    }

    let _ = rewrite_player_scale(&resizer_config_path, scale_percent);
}

fn random_scale_percent(options: &ScaleRandomizerOptions) -> u32 {
    rand::rng().random_range(options.min_player_scale..=options.max_player_scale)
}

fn rewrite_player_scale(path: &Path, scale_percent: u32) -> std::io::Result<()> {
    let contents = fs::read_to_string(path)?;

    let mut found = false;
    let new_contents = contents
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();

            if trimmed.starts_with("playerScale") {
                found = true;
                let indent_len = line.len() - trimmed.len();
                let indent = &line[..indent_len];
                format!("{indent}playerScale = {scale_percent}%")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let final_contents = if found {
        new_contents
    } else {
        format!("{new_contents}\nplayerScale = {scale_percent}%\n")
    };

    fs::write(path, final_contents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrite_player_scale_replaces_existing_value() {
        let temp_dir = std::env::temp_dir().join("scale-randomizer-tests-replace");
        let path = temp_dir.join("Resizer_config.ini");
        let _ = fs::create_dir_all(&temp_dir);
        fs::write(&path, "foo = 1\nplayerScale = 80%\nbar = 2\n").unwrap();

        rewrite_player_scale(&path, 125).unwrap();

        let rewritten = fs::read_to_string(&path).unwrap();
        assert!(rewritten.contains("playerScale = 125%"));
        assert!(!rewritten.contains("playerScale = 80%"));
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&temp_dir);
    }

    #[test]
    fn rewrite_player_scale_appends_when_missing() {
        let temp_dir = std::env::temp_dir().join("scale-randomizer-tests-append");
        let path = temp_dir.join("Resizer_config.ini");
        let _ = fs::create_dir_all(&temp_dir);
        fs::write(&path, "foo = 1\nbar = 2\n").unwrap();

        rewrite_player_scale(&path, 90).unwrap();

        let rewritten = fs::read_to_string(&path).unwrap();
        assert!(rewritten.ends_with("playerScale = 90%\n"));
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&temp_dir);
    }

    #[test]
    fn random_scale_stays_within_bounds() {
        let options = ScaleRandomizerOptions {
            min_player_scale: 25,
            max_player_scale: 30,
            update_interval_ms: 5000,
        };

        for _ in 0..200 {
            let value = random_scale_percent(&options);
            assert!((25..=30).contains(&value));
        }
    }
}
