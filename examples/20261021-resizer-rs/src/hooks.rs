use std::{
    collections::HashMap,
    ffi::c_void,
    mem::{size_of, transmute},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};

use fromsoftware_shared::{find_rtti_classes, program::Program};
use windows::Win32::System::Memory::{
    PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS, VirtualProtect,
};

use crate::{config::current_config, log};

const CAMERA_EVENT_ZOOM_OUT: u32 = 0x98;
const WEAPON_TRANSFORM_FLOAT_COUNT: usize = 10;

static CAMERA_ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static WEAPON_ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static CAMERA_BASELINES: OnceLock<Mutex<HashMap<usize, f32>>> = OnceLock::new();

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

pub fn install_hooks() -> Vec<VmtHook> {
    let mut hooks = Vec::new();

    match unsafe {
        install_vmt_hook(
            "CS::CSChrTaeAnimEvent",
            2,
            camera_detour as *const () as usize,
            &CAMERA_ORIGINAL,
        )
    } {
        Some(hook) => {
            log::info("installed CS::CSChrTaeAnimEvent slot 2 camera hook");
            hooks.push(hook);
        }
        None => log::error("failed to install camera hook"),
    }

    match unsafe {
        install_vmt_hook(
            "CS::CSFD4LocationChrAsmDmypolyIdModifier",
            5,
            weapon_dmypoly_detour as *const () as usize,
            &WEAPON_ORIGINAL,
        )
    } {
        Some(hook) => {
            log::info("installed CSFD4 dmypoly slot 5 weapon hook");
            hooks.push(hook);
        }
        None => log::error("failed to install weapon dmypoly hook"),
    }

    hooks
}

unsafe fn install_vmt_hook(
    class_name: &str,
    slot_index: u32,
    detour: usize,
    original_store: &AtomicUsize,
) -> Option<VmtHook> {
    let program = Program::current();
    let class = find_rtti_classes(&program).find(|class| class.name == class_name)?;
    let slot = unsafe { class.vmt_index(slot_index)? };
    let slot_ptr = slot.as_ptr();
    let original = unsafe { slot_ptr.read() } as usize;
    original_store.store(original, Ordering::SeqCst);

    let mut old = PAGE_PROTECTION_FLAGS(0);
    unsafe {
        VirtualProtect(
            slot_ptr as *const c_void,
            size_of::<usize>(),
            PAGE_EXECUTE_READWRITE,
            &mut old,
        )
        .ok()?;
        slot_ptr.write(detour as u64);
        let _ = VirtualProtect(slot_ptr as *const c_void, size_of::<usize>(), old, &mut old);
    }

    Some(VmtHook {
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
    let original = CAMERA_ORIGINAL.load(Ordering::SeqCst);
    if original != 0 {
        let original: TaeEventFn = unsafe { transmute(original) };
        unsafe { original(this, event) };
    }

    let _ = catch_unwind(AssertUnwindSafe(|| {
        if event.is_null() {
            return;
        }
        let event = unsafe { &*event };
        if event.kind != CAMERA_EVENT_ZOOM_OUT || event.value.is_null() {
            return;
        }

        let address = event.value as usize;
        let scale = current_config().camera_scale;
        let baselines = CAMERA_BASELINES.get_or_init(|| Mutex::new(HashMap::new()));
        let Ok(mut baselines) = baselines.lock() else {
            return;
        };
        let baseline = *baselines
            .entry(address)
            .or_insert_with(|| unsafe { *event.value });
        unsafe {
            *event.value = baseline * scale;
        }
    }));
}

unsafe extern "system" fn weapon_dmypoly_detour(this: *mut c_void, transform: *mut f32) {
    let original = WEAPON_ORIGINAL.load(Ordering::SeqCst);
    if original != 0 {
        let original: DmypolyFn = unsafe { transmute(original) };
        unsafe { original(this, transform) };
    }

    let _ = catch_unwind(AssertUnwindSafe(|| {
        if transform.is_null() {
            return;
        }
        let scale = current_config().weapon_scale;
        if (scale - 1.0).abs() <= f32::EPSILON {
            return;
        }
        for index in 0..WEAPON_TRANSFORM_FLOAT_COUNT {
            unsafe {
                *transform.add(index) *= scale;
            }
        }
    }));
}
