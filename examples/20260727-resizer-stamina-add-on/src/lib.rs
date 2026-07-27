mod config;
mod html_status;
mod resizer_stamina;

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
        ResizerStaminaAddOnConfig, config_modified_time, load_config, load_or_create_config,
        resolve_paths,
    },
    html_status::HtmlStatusWriter,
    resizer_stamina::ResizerStaminaController,
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
        let config = load_or_create_config(&paths.config_path);

        if wait_for_system_init(&Program::current(), Duration::MAX).is_err() {
            return;
        }

        let Ok(cs_task) = (unsafe { CSTaskImp::instance() }) else {
            return;
        };

        let mut state = ResizerStaminaAddOnState::new(
            config,
            paths.config_path,
            paths.html_path,
            paths.resizer_config_path,
        );
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

struct ResizerStaminaAddOnState {
    config_path: PathBuf,
    config_last_modified: Option<SystemTime>,
    last_config_check: Instant,
    controller: ResizerStaminaController,
    html: HtmlStatusWriter,
}

impl ResizerStaminaAddOnState {
    fn new(
        config: ResizerStaminaAddOnConfig,
        config_path: PathBuf,
        html_path: PathBuf,
        resizer_config_path: PathBuf,
    ) -> Self {
        let config_last_modified = config_modified_time(&config_path);

        Self {
            config_path,
            config_last_modified,
            last_config_check: Instant::now(),
            controller: ResizerStaminaController::new(&config.resizer, resizer_config_path),
            html: HtmlStatusWriter::new(html_path),
        }
    }

    fn tick(&mut self) {
        self.reload_config_if_changed();
        self.controller.tick();
        self.write_status();
    }

    fn write_status(&mut self) {
        self.html.write_if_due(self.controller.status());
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

        self.controller.update_config(&config.resizer);
        self.config_last_modified = config_modified_time(&self.config_path);
    }
}
