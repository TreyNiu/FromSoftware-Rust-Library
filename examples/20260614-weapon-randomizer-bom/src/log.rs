use windows::Win32::{
    System::Diagnostics::Debug::MessageBeep,
    UI::WindowsAndMessaging::{MB_ICONHAND, MB_ICONINFORMATION},
};

pub fn log_event(_message: impl AsRef<str>) {
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
