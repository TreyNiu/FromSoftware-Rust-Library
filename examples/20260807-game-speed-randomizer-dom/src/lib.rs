mod config;
mod html_status;
mod speed_randomizer;

use eldenring::{
    cs::{CSTaskGroupIndex, CSTaskImp},
    fd4::FD4TaskData,
    util::system::wait_for_system_init,
};
use fromsoftware_shared::{FromStatic, program::Program, task::*};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
    time::{Duration, Instant, SystemTime},
};

use crate::{
    config::{
        GameSpeedRandomizerDomConfig, config_modified_time, load_config, load_or_create_config,
        resolve_paths,
    },
    html_status::HtmlStatusWriter,
    speed_randomizer::GameSpeedRandomizer,
};

const CONFIG_RELOAD_INTERVAL: Duration = Duration::from_secs(1);
const INPUT_CHECK_INTERVAL: Duration = Duration::from_millis(500);

#[unsafe(no_mangle)]
/// # Safety
/// This entry point is called by Windows LoadLibrary. Do not call it directly.
pub unsafe extern "C" fn DllMain(hmodule: usize, reason: u32) -> bool {
    if reason != 1 {
        return true;
    }

    std::thread::spawn(move || {
        let paths = resolve_paths(hmodule);
        let config = load_or_create_config(&paths.config_path);
        if wait_for_system_init(&Program::current(), Duration::MAX).is_err() {
            return;
        }
        let Ok(cs_task) = (unsafe { CSTaskImp::instance() }) else {
            return;
        };
        let mut state = State::new(config, paths.config_path, paths.html_path);
        state.write_status();
        cs_task.run_recurring(
            move |_: &FD4TaskData| {
                let _ = catch_unwind(AssertUnwindSafe(|| state.tick()));
            },
            CSTaskGroupIndex::ChrIns_PostPhysics,
        );
    });
    true
}

struct State {
    config_path: PathBuf,
    config_last_modified: Option<SystemTime>,
    last_config_check: Instant,
    randomizer: GameSpeedRandomizer,
    html: HtmlStatusWriter,
}

impl State {
    fn new(config: GameSpeedRandomizerDomConfig, config_path: PathBuf, html_path: PathBuf) -> Self {
        Self {
            config_last_modified: config_modified_time(&config_path),
            config_path,
            last_config_check: Instant::now(),
            randomizer: GameSpeedRandomizer::new(&config.speed, INPUT_CHECK_INTERVAL),
            html: HtmlStatusWriter::new(html_path),
        }
    }

    fn tick(&mut self) {
        self.reload_config_if_changed();
        self.randomizer.tick(INPUT_CHECK_INTERVAL);
        self.write_status();
    }

    fn write_status(&mut self) {
        self.html.write_if_due(self.randomizer.status());
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
        let Some(config) = (if modified.is_none() {
            Some(load_or_create_config(&self.config_path))
        } else {
            load_config(&self.config_path)
        }) else {
            return;
        };
        self.randomizer.update_config(&config.speed);
        self.config_last_modified = config_modified_time(&self.config_path);
    }
}
