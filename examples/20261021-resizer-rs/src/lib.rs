#![allow(non_snake_case)]
#![allow(unsafe_op_in_unsafe_fn)]

mod config;
mod hooks;
mod log;
mod resizer;
mod skeleton;

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, RwLock},
    time::Duration,
};

use eldenring::{
    cs::{CSTaskGroupIndex, CSTaskImp},
    fd4::FD4TaskData,
    util::system::wait_for_system_init,
};
use fromsoftware_shared::{FromStatic, program::Program, task::*};

use crate::{
    config::{config_path_from_module, load_or_create_config, set_global_config},
    resizer::ResizerState,
};

#[unsafe(no_mangle)]
/// # Safety
///
/// Exposed for Windows LoadLibrary. Do not call this directly.
pub unsafe extern "C" fn DllMain(hmodule: usize, reason: u32) -> bool {
    if reason != 1 {
        return true;
    }

    std::thread::spawn(move || {
        let config_path = config_path_from_module(hmodule);
        let log_path = config_path.with_file_name("resizer_rs_log.txt");
        log::init(&log_path);
        log::info("resizer-rs loading");

        let config = load_or_create_config(&config_path);
        log::info(format!("initial config: {config:?}"));
        let shared_config = Arc::new(RwLock::new(config));
        set_global_config(Arc::clone(&shared_config));

        if wait_for_system_init(&Program::current(), Duration::MAX).is_err() {
            log::error("wait_for_system_init failed");
            return;
        }

        let Ok(cs_task) = (unsafe { CSTaskImp::instance() }) else {
            log::error("CSTaskImp::instance failed");
            return;
        };

        let hooks = hooks::install_hooks();
        let mut state = ResizerState::new(config_path, shared_config, hooks);

        cs_task.run_recurring(
            move |_: &FD4TaskData| {
                if catch_unwind(AssertUnwindSafe(|| state.tick())).is_err() {
                    log::error("panic while running resizer tick");
                }
            },
            CSTaskGroupIndex::ChrIns_PostPhysics,
        );

        log::info("resizer-rs initialized");
    });

    true
}
