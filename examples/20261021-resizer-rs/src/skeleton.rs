use std::ffi::CStr;

use fromsoftware_shared::F32Vector4;

const CHR_MODULES_OFFSET: usize = 0x190;
const HKB_CHARACTER_CHAIN_1: usize = 0x28;
const HKB_CHARACTER_CHAIN_2: usize = 0x10;
const HKB_CHARACTER_CHAIN_3: usize = 0x30;
const HKA_SKELETON_CHAIN_1: usize = 0x90;
const HKA_SKELETON_CHAIN_2: usize = 0x28;
const HKA_SKELETON_BONE_COUNT: usize = 0x28;
const HKA_SKELETON_BONE_NAME_LAYOUT: usize = 0x30;
const BONE_DATA_LAYOUT_CHAIN: usize = 0x38;
const BONE_DATA_OFFSET: usize = 0x54;

#[repr(C)]
struct HkBoneData {
    xzy_vec: F32Vector4,
    _q_spatial: F32Vector4,
    xzy_scale: F32Vector4,
}

/// Applies the Resizer-style skeleton modifiers to a live character.
///
/// The original mod runs this once per skeleton update:
/// - every bone's local length vector is scaled,
/// - the root bone's local size vector is scaled so thickness propagates down,
/// - Torrent's Saddle bone gets an inverse size correction.
///
/// # Safety
///
/// `chr_ins` must point to a valid, live `ChrIns` on the game thread.
pub unsafe fn scale_character_skeleton(chr_ins: *mut u8, scale: f32) -> bool {
    if chr_ins.is_null() || !scale.is_finite() || scale <= 0.0 {
        return false;
    }

    let Some(modules) = read_ptr(chr_ins as *const u8, CHR_MODULES_OFFSET) else {
        return false;
    };
    let Some(chain_1) = read_ptr(modules, HKB_CHARACTER_CHAIN_1) else {
        return false;
    };
    let Some(chain_2) = read_ptr(chain_1, HKB_CHARACTER_CHAIN_2) else {
        return false;
    };
    let Some(hkb_character) = read_ptr(chain_2, HKB_CHARACTER_CHAIN_3) else {
        return false;
    };

    let Some(hka_chain_1) = read_ptr(hkb_character, HKA_SKELETON_CHAIN_1) else {
        return false;
    };
    let Some(hka_skeleton) = read_ptr(hka_chain_1, HKA_SKELETON_CHAIN_2) else {
        return false;
    };

    let bone_count = read_i32(hka_skeleton, HKA_SKELETON_BONE_COUNT).unwrap_or(0);
    if !(1..=4096).contains(&bone_count) {
        return false;
    }

    let Some(bone_name_layout) = read_ptr(hka_skeleton, HKA_SKELETON_BONE_NAME_LAYOUT) else {
        return false;
    };
    let Some(bone_data_layout) = read_ptr(hkb_character, BONE_DATA_LAYOUT_CHAIN) else {
        return false;
    };
    let bone_offset = read_i32(bone_data_layout, BONE_DATA_OFFSET).unwrap_or(0);
    let bone_data = bone_data_layout.offset(bone_offset as isize) as *mut HkBoneData;
    if bone_data.is_null() {
        return false;
    }

    scale_vec(&mut (*bone_data).xzy_scale, scale);

    let names = bone_name_layout as *const *const i8;
    for index in 0..bone_count as usize {
        let bone = bone_data.add(index);
        let name_ptr = names.add(index * 2).read_unaligned();
        let is_saddle = !name_ptr.is_null() && CStr::from_ptr(name_ptr).to_bytes() == b"Saddle";

        if is_saddle {
            scale_vec(&mut (*bone).xzy_scale, scale.recip());
        }
        scale_vec(&mut (*bone).xzy_vec, scale);
    }

    true
}

unsafe fn read_ptr(base: *const u8, offset: usize) -> Option<*mut u8> {
    if base.is_null() {
        return None;
    }
    let value = (base.add(offset) as *const usize).read_unaligned();
    (value != 0).then_some(value as *mut u8)
}

unsafe fn read_i32(base: *const u8, offset: usize) -> Option<i32> {
    if base.is_null() {
        return None;
    }
    Some((base.add(offset) as *const i32).read_unaligned())
}

fn scale_vec(value: &mut F32Vector4, scale: f32) {
    value.0 *= scale;
    value.1 *= scale;
    value.2 *= scale;
    value.3 *= scale;
}
