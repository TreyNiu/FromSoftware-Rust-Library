#![allow(non_snake_case)]
#![allow(unsafe_op_in_unsafe_fn)]

mod config;
mod hooks;
mod log;
mod memory;
mod resizer;
mod skeleton;

use std::{
    any::Any,
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
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;

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
        log::install_panic_hook();
        log::info("resizer-rs loading");
        log_process_info();
        log::info(format!("DLL module=0x{hmodule:x}"));
        log::info(format!("config path={}", config_path.display()));
        log::info(format!("log path={}", log_path.display()));

        let config = load_or_create_config(&config_path);
        log::info(format!("initial config: {config:?}"));
        let shared_config = Arc::new(RwLock::new(config));
        set_global_config(Arc::clone(&shared_config));

        log::info("waiting for system init");
        if wait_for_system_init(&Program::current(), Duration::MAX).is_err() {
            log::error("wait_for_system_init failed");
            return;
        }
        log::info("system init complete");

        let Ok(cs_task) = (unsafe { CSTaskImp::instance() }) else {
            log::error("CSTaskImp::instance failed");
            return;
        };
        log::info("CSTaskImp acquired");

        let hook_config = shared_config
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default();
        log::info("installing VMT hooks");
        let hooks = hooks::install_hooks(&hook_config);
        log::info(format!("VMT hooks installed: {}", hooks.len()));

        let mut state = ResizerState::new(config_path, shared_config, hooks);
        log::info("registering ChrIns_PostPhysics recurring task");
        cs_task.run_recurring(
            move |_: &FD4TaskData| {
                if let Err(payload) = catch_unwind(AssertUnwindSafe(|| state.tick())) {
                    log::error(format!(
                        "caught resizer tick panic: {}",
                        panic_text(&payload)
                    ));
                }
            },
            CSTaskGroupIndex::ChrIns_PostPhysics,
        );

        log::info("resizer-rs initialized");
    });

    true
}

fn log_process_info() {
    let mut buffer = vec![0u16; 260];
    loop {
        let len = unsafe { GetModuleFileNameW(None, &mut buffer) } as usize;
        if len == 0 {
            log::error("GetModuleFileNameW(main module) failed");
            return;
        }
        if len < buffer.len() || buffer.len() >= 32_768 {
            let path = std::path::PathBuf::from(String::from_utf16_lossy(&buffer[..len]));
            let metadata = std::fs::metadata(&path).ok();
            let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
            let modified = metadata
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            log::info(format!(
                "process path={} size={} modified_unix={}",
                path.display(),
                size,
                modified
            ));
            return;
        }
        buffer.resize((buffer.len() * 2).min(32_768), 0);
    }
}

fn panic_text(payload: &Box<dyn Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_string()
    }
}
