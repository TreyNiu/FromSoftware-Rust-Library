use std::{fs::OpenOptions, io::Write};

use windows::Win32::{
    System::{Diagnostics::Debug::MessageBeep, SystemInformation::GetLocalTime},
    UI::WindowsAndMessaging::{MB_ICONHAND, MB_ICONINFORMATION},
};

pub fn log_event(message: impl AsRef<str>) {
    let timestamp = local_timestamp();

    // 日志写在当前工作目录，方便和 DLL 放在一起时直接看 hot-randomizer.log。
    let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("hot-randomizer.log")
    else {
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
