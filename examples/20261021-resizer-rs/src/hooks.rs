use std::{
    collections::HashMap,
    ffi::c_void,
    mem::{size_of, transmute},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use fromsoftware_shared::{find_rtti_classes, program::Program};
use windows::Win32::System::Memory::{
    PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS, VirtualProtect,
};

use crate::{
    config::{ResizerConfig, current_config},
    log, memory,
};

const CAMERA_EVENT_ZOOM_OUT: u32 = 0x98;
const WEAPON_TRANSFORM_FLOAT_COUNT: usize = 10;

static CAMERA_ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static WEAPON_ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static CAMERA_BASELINES: OnceLock<Mutex<HashMap<usize, f32>>> = OnceLock::new();
static CAMERA_FIRST_CALL: AtomicBool = AtomicBool::new(true);
static WEAPON_FIRST_CALL: AtomicBool = AtomicBool::new(true);

pub struct VmtHook {
    slot: usize,
    original: usize,
}

impl Drop for VmtHook {
    fn drop(&mut self) {
        unsafe {
            let slot = self.slot as *mut usize;
            let mut old = PAGE_PROTECTION_FLAGS(0);
            if VirtualProtect(
                slot as *const c_void,
                size_of::<usize>(),
                PAGE_EXECUTE_READWRITE,
                &mut old,
            )
            .is_ok()
            {
                slot.write(self.original);
                let _ = VirtualProtect(slot as *const c_void, size_of::<usize>(), old, &mut old);
            }
        }
    }
}

pub fn install_hooks(config: &ResizerConfig) -> Vec<VmtHook> {
    log::info(format!(
        "hook switches: camera={}, weapon={}",
        config.enable_camera_hook, config.enable_weapon_hook
    ));
    let mut hooks = Vec::new();

    if config.enable_camera_hook {
        match unsafe {
            install_vmt_hook(
                "CS::CSChrTaeAnimEvent",
                2,
                camera_detour as *const () as usize,
                &CAMERA_ORIGINAL,
            )
        } {
            Ok(hook) => {
                log::info("installed CS::CSChrTaeAnimEvent slot 2 camera hook");
                hooks.push(hook);
            }
            Err(error) => log::error(format!("camera hook skipped: {error}")),
        }
    } else {
        log::info("camera hook disabled by config");
    }

    if config.enable_weapon_hook {
        match unsafe {
            install_vmt_hook(
                "CS::CSFD4LocationChrAsmDmypolyIdModifier",
                5,
                weapon_dmypoly_detour as *const () as usize,
                &WEAPON_ORIGINAL,
            )
        } {
            Ok(hook) => {
                log::info("installed CSFD4 dmypoly slot 5 weapon hook");
                hooks.push(hook);
            }
            Err(error) => log::error(format!("weapon hook skipped: {error}")),
        }
    } else {
        log::info("weapon hook disabled by config");
    }

    hooks
}

unsafe fn install_vmt_hook(
    class_name: &str,
    slot_index: u32,
    detour: usize,
    original_store: &AtomicUsize,
) -> Result<VmtHook, String> {
    log::info(format!("RTTI scan for {class_name}"));
    let program = Program::current();
    let class = find_rtti_classes(&program)
        .find(|class| class.name == class_name)
        .ok_or_else(|| format!("RTTI class not found: {class_name}"))?;
    log::info(format!(
        "RTTI class {class_name}: vftable RVA=0x{:x}",
        class.vftable
    ));

    let slot = unsafe { class.vmt_index(slot_index) }
        .ok_or_else(|| format!("vtable slot {slot_index} not found for {class_name}"))?;
    let slot_ptr = slot.as_ptr();
    let original = unsafe { slot_ptr.read() } as usize;
    if !memory::executable(original) {
        return Err(format!(
            "slot {slot_index} original is not executable: 0x{original:x}"
        ));
    }
    log::info(format!(
        "class {class_name} slot {slot_index}: entry=0x{:x}, original=0x{original:x}, detour=0x{detour:x}",
        slot_ptr as usize
    ));
    original_store.store(original, Ordering::SeqCst);

    let mut old = PAGE_PROTECTION_FLAGS(0);
    unsafe {
        VirtualProtect(
            slot_ptr as *const c_void,
            size_of::<usize>(),
            PAGE_EXECUTE_READWRITE,
            &mut old,
        )
        .map_err(|error| format!("VirtualProtect(vtable) failed: {error}"))?;
        slot_ptr.write(detour as u64);
        let _ = VirtualProtect(slot_ptr as *const c_void, size_of::<usize>(), old, &mut old);
    }

    Ok(VmtHook {
        slot: slot_ptr as usize,
        original,
    })
}

#[repr(C)]
struct TaeEvent {
    kind: u32,
    _padding: u32,
    value: *mut f32,
}

type TaeEventFn = unsafe extern "system" fn(*mut c_void, *mut TaeEvent);
type DmypolyFn = unsafe extern "system" fn(*mut c_void, *mut f32);

unsafe extern "system" fn camera_detour(this: *mut c_void, event: *mut TaeEvent) {
    if CAMERA_FIRST_CALL.swap(false, Ordering::SeqCst) {
        log::info(format!(
            "camera detour first call: this=0x{this:p}, event=0x{event:p}"
        ));
    }

    let original = CAMERA_ORIGINAL.load(Ordering::SeqCst);
    if original != 0 {
        let original: TaeEventFn = unsafe { transmute(original) };
        unsafe { original(this, event) };
    }

    let _ = catch_unwind(AssertUnwindSafe(|| {
        if event.is_null() || !memory::readable(event as usize, size_of::<TaeEvent>()) {
            log::error("camera detour received an invalid event pointer");
            return;
        }
        let event_ref = unsafe { &*event };
        if event_ref.kind != CAMERA_EVENT_ZOOM_OUT
            || event_ref.value.is_null()
            || !memory::readable(event_ref.value as usize, size_of::<f32>())
        {
            return;
        }

        let address = event_ref.value as usize;
        let scale = current_config().camera_scale;
        let baselines = CAMERA_BASELINES.get_or_init(|| Mutex::new(HashMap::new()));
        let Ok(mut baselines) = baselines.lock() else {
            return;
        };
        let Some(current) = (unsafe { memory::read::<f32>(address) }) else {
            return;
        };
        let baseline = *baselines.entry(address).or_insert(current);
        let _ = unsafe { memory::write::<f32>(address, baseline * scale) };
    }));
}

unsafe extern "system" fn weapon_dmypoly_detour(this: *mut c_void, transform: *mut f32) {
    if WEAPON_FIRST_CALL.swap(false, Ordering::SeqCst) {
        log::info(format!(
            "weapon detour first call: this=0x{this:p}, transform=0x{transform:p}"
        ));
    }

    let original = WEAPON_ORIGINAL.load(Ordering::SeqCst);
    if original != 0 {
        let original: DmypolyFn = unsafe { transmute(original) };
        unsafe { original(this, transform) };
    }

    let _ = catch_unwind(AssertUnwindSafe(|| {
        let transform_size = WEAPON_TRANSFORM_FLOAT_COUNT * size_of::<f32>();
        if transform.is_null() || !memory::writable(transform as usize, transform_size) {
            log::error("weapon detour received an invalid transform pointer");
            return;
        }
        let scale = current_config().weapon_scale;
        if (scale - 1.0).abs() <= f32::EPSILON {
            return;
        }
        for index in 0..WEAPON_TRANSFORM_FLOAT_COUNT {
            let address = transform as usize + index * size_of::<f32>();
            if let Some(value) = unsafe { memory::read::<f32>(address) } {
                let _ = unsafe { memory::write::<f32>(address, value * scale) };
            }
        }
    }));
}
