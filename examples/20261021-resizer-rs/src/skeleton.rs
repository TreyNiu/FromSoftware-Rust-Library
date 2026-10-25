use fromsoftware_shared::F32Vector4;

use crate::memory;

const CHR_MODULES_OFFSET: usize = 0x190;
const HKB_CHARACTER_CHAIN_1: usize = 0x28;
const HKB_CHARACTER_CHAIN_2: usize = 0x10;
const HKB_CHARACTER_CHAIN_3: usize = 0x30;
const HKA_SKELETON_CHAIN_1: usize = 0x90;
const HKA_SKELETON_CHAIN_2: usize = 0x28;
const HKA_SKELETON_BONE_COUNT: usize = 0x28;
const BONE_DATA_LAYOUT_CHAIN: usize = 0x38;
const BONE_DATA_OFFSET: usize = 0x54;

#[repr(C)]
struct HkBoneData {
    xzy_vec: F32Vector4,
    _q_spatial: F32Vector4,
    xzy_scale: F32Vector4,
}

#[derive(Clone, Copy, Debug)]
pub struct SkeletonScaleInfo {
    pub bone_count: usize,
    pub hkb_character: usize,
    pub hka_skeleton: usize,
    pub bone_data: usize,
}

/// Applies the Resizer-style skeleton modifiers to a live character.
///
/// This function validates every pointer-chain step and the final writable bone-data range with
/// Windows VirtualQuery before touching memory. A failure returns the stage that rejected the
/// layout, which is logged by the caller.
///
/// # Safety
///
/// `chr_ins` must point to a live `ChrIns`. The caller is responsible for the lifetime of the
/// object; this function only guards the pointer chain against invalid memory.
pub unsafe fn scale_character_skeleton(
    chr_ins: *mut u8,
    scale: f32,
) -> Result<SkeletonScaleInfo, &'static str> {
    if chr_ins.is_null() {
        return Err("ChrIns is null");
    }
    if !scale.is_finite() || scale <= 0.0 {
        return Err("scale is not a positive finite value");
    }

    let modules = read_ptr(chr_ins as usize, CHR_MODULES_OFFSET)
        .ok_or("ChrIns+0x190 modules pointer is invalid")?;
    let chain_1 = read_ptr(modules, HKB_CHARACTER_CHAIN_1)
        .ok_or("HkbCharacter chain step 0x28 is invalid")?;
    let chain_2 = read_ptr(chain_1, HKB_CHARACTER_CHAIN_2)
        .ok_or("HkbCharacter chain step 0x10 is invalid")?;
    let hkb_character = read_ptr(chain_2, HKB_CHARACTER_CHAIN_3)
        .ok_or("HkbCharacter chain step 0x30 is invalid")?;

    let hka_chain_1 = read_ptr(hkb_character, HKA_SKELETON_CHAIN_1)
        .ok_or("HkbCharacter+0x90 skeleton pointer is invalid")?;
    let hka_skeleton =
        read_ptr(hka_chain_1, HKA_SKELETON_CHAIN_2).ok_or("HkaSkeleton pointer is invalid")?;

    let bone_count = read_i32(hka_skeleton, HKA_SKELETON_BONE_COUNT)
        .ok_or("HkaSkeleton bone count is unreadable")?;
    if !(1..=4096).contains(&bone_count) {
        return Err("HkaSkeleton bone count is out of range");
    }
    let bone_count = bone_count as usize;

    let bone_data_layout = read_ptr(hkb_character, BONE_DATA_LAYOUT_CHAIN)
        .ok_or("HkbCharacter+0x38 bone-data layout is invalid")?;
    let bone_offset = read_i32(bone_data_layout, BONE_DATA_OFFSET)
        .ok_or("bone-data offset at +0x54 is unreadable")?;
    let bone_data = (bone_data_layout as isize)
        .checked_add(bone_offset as isize)
        .ok_or("bone-data offset overflow")? as usize;

    let bone_data_size = bone_count
        .checked_mul(size_of::<HkBoneData>())
        .ok_or("bone-data size overflow")?;
    if !memory::writable(bone_data, bone_data_size) {
        return Err("final bone-data range is not writable");
    }

    let bones = bone_data as *mut HkBoneData;
    let root = &mut *bones;
    scale_vec(&mut root.xzy_scale, scale);

    for index in 0..bone_count {
        let bone = &mut *bones.add(index);
        scale_vec(&mut bone.xzy_vec, scale);
    }

    Ok(SkeletonScaleInfo {
        bone_count,
        hkb_character,
        hka_skeleton,
        bone_data,
    })
}

unsafe fn read_ptr(base: usize, offset: usize) -> Option<usize> {
    let address = base.checked_add(offset)?;
    let value = unsafe { memory::read::<usize>(address)? };
    (value != 0 && memory::readable(value, 1)).then_some(value)
}

unsafe fn read_i32(base: usize, offset: usize) -> Option<i32> {
    let address = base.checked_add(offset)?;
    unsafe { memory::read::<i32>(address) }
}

fn scale_vec(value: &mut F32Vector4, scale: f32) {
    value.0 *= scale;
    value.1 *= scale;
    value.2 *= scale;
    value.3 *= scale;
}
