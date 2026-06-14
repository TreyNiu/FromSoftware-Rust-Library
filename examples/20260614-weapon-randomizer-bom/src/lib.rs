mod config;
mod log;
mod weapon_pools;
mod weapon_randomizer;

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
    time::{Duration, Instant, SystemTime},
};

use eldenring::{
    cs::{CSTaskGroupIndex, CSTaskImp},
    fd4::FD4TaskData,
    util::system::wait_for_system_init,
};
use fromsoftware_shared::{FromStatic, program::Program, task::*};

use crate::{
    config::{
        WeaponRandomizerBomConfig, config_modified_time, load_config, load_or_create_config,
        resolve_paths,
    },
    log::log_event,
    weapon_randomizer::WeaponRandomizer,
};

const CONFIG_RELOAD_INTERVAL: Duration = Duration::from_secs(1);

#[unsafe(no_mangle)]
/// # Safety
///
/// This is exposed this way such that Windows LoadLibrary API can call it. Do not call this yourself.
pub unsafe extern "C" fn DllMain(hmodule: usize, reason: u32) -> bool {
    if reason != 1 {
        return true;
    }

    std::thread::spawn(move || {
        let paths = resolve_paths(hmodule);
        log_event("weapon_randomizer_bom thread started");

        // DLL 刚加载时，游戏里的全局单例不一定已经初始化，所以先等系统初始化完成。
        if let Err(err) = wait_for_system_init(&Program::current(), Duration::MAX) {
            log_event(format!("wait_for_system_init failed: {err:?}"));
            return;
        }
        log_event("system init complete");

        let Ok(cs_task) = (unsafe { CSTaskImp::instance() }) else {
            log_event("CSTaskImp::instance failed");
            return;
        };
        log_event("registering recurring task");

        let config = load_or_create_config(&paths.config_path);
        log_event(format!("loaded config: {config:?}"));
        let mut state = WeaponRandomizerBomState::new(config, paths.config_path);

        cs_task.run_recurring(
            move |_: &FD4TaskData| {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    state.tick();
                }));

                if result.is_err() {
                    log_event("panic while running weapon_randomizer_bom tick");
                }
            },
            CSTaskGroupIndex::ChrIns_PostPhysics,
        );
        log_event("recurring task registered");
    });

    true
}

struct WeaponRandomizerBomState {
    config_path: PathBuf,
    input_check_interval: Duration,
    config_last_modified: Option<SystemTime>,
    last_config_check: Instant,
    weapon: WeaponRandomizer,
}

impl WeaponRandomizerBomState {
    fn new(config: WeaponRandomizerBomConfig, config_path: PathBuf) -> Self {
        let input_check_interval = Duration::from_millis(config.input_check_interval_millis);
        let config_last_modified = config_modified_time(&config_path);

        Self {
            config_path,
            input_check_interval,
            config_last_modified,
            last_config_check: Instant::now(),
            weapon: WeaponRandomizer::new(config.weapon, input_check_interval),
        }
    }

    fn tick(&mut self) {
        self.reload_config_if_changed();
        self.weapon.tick(self.input_check_interval);
    }

    fn reload_config_if_changed(&mut self) {
        if self.last_config_check.elapsed() < CONFIG_RELOAD_INTERVAL {
            return;
        }
        self.last_config_check = Instant::now();

        let modified = config_modified_time(&self.config_path);
        if modified == self.config_last_modified {
            return;
        }

        let config = if modified.is_none() {
            log_event("config file missing, recreating weapon_randomizer_bom.toml with defaults");
            load_or_create_config(&self.config_path)
        } else {
            let Some(config) = load_config(&self.config_path) else {
                log_event("config reload skipped: failed to parse weapon_randomizer_bom.toml");
                return;
            };
            config
        };

        log_event(format!("weapon_randomizer_bom config reloaded: {config:?}"));
        self.input_check_interval = Duration::from_millis(config.input_check_interval_millis);
        self.weapon.update_config(config.weapon);
        self.config_last_modified = modified;
    }
}
