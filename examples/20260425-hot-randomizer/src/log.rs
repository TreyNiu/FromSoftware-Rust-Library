use std::{
    fs::OpenOptions,
    io::Write,
    path::PathBuf,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

use windows::Win32::{
    System::{Diagnostics::Debug::MessageBeep, SystemInformation::GetLocalTime},
    UI::WindowsAndMessaging::{MB_ICONHAND, MB_ICONINFORMATION},
};

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();
static LOG_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn initialize_log(log_path: PathBuf, enabled: bool) {
    let _ = LOG_PATH.set(log_path);
    set_log_enabled(enabled);
}

pub fn set_log_enabled(enabled: bool) {
    LOG_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn log_event(message: impl AsRef<str>) {
    if !LOG_ENABLED.load(Ordering::Relaxed) {
        return;
    }

    let Some(log_path) = LOG_PATH.get() else {
        return;
    };
    let timestamp = local_timestamp();

    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) else {
        return;
    };

    let _ = writeln!(file, "[{timestamp}] {}", message.as_ref());
}

fn local_timestamp() -> String {
    let time = unsafe { GetLocalTime() };

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
        time.wYear,
        time.wMonth,
        time.wDay,
        time.wHour,
        time.wMinute,
        time.wSecond,
        time.wMilliseconds
    )
}

pub fn beep_toggle(enabled: bool) {
    let sound = if enabled {
        MB_ICONINFORMATION
    } else {
        MB_ICONHAND
    };

    if unsafe { MessageBeep(sound) }.is_err() {
        log_event(format!("toggle beep failed: enabled={enabled}"));
    }
}
