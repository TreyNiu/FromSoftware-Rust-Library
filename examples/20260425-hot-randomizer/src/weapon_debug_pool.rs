use eldenring::cs::{EquipParamGem, EquipParamWeapon, SoloParamRepository, SwordArtsParam};
use rand::{Rng, prelude::IndexedRandom};

use crate::{
    log::log_event,
    weapon_randomizer::{AshCandidate, WeaponCandidate, can_mount_ash},
};

// 调试池单独放在这里，方便临时缩小随机范围。
// 是否启用调试池由 hot-randomizer.toml 里的 weapon.debug_fixed_pool 控制。
const DEBUG_WEAPON_IDS: [u32; 2] = [21_070_000, 21_080_000];
const DEBUG_GEM_IDS: [u32; 3] = [20_500, 21_400, 21_600];

pub fn collect_debug_weapon_candidates(params: &SoloParamRepository) -> Vec<WeaponCandidate> {
    let candidates = DEBUG_WEAPON_IDS
        .iter()
        .filter_map(|&param_id| {
            let Some(weapon) = params.get::<EquipParamWeapon>(param_id) else {
                log_event(format!("debug weapon missing: id={param_id}"));
                return None;
            };

            log_event(format!(
                "debug weapon candidate: id={}, wep_type={}, material_set={}, icon_id={}, sword_art={}",
                param_id,
                weapon.wep_type(),
                weapon.material_set_id(),
                weapon.icon_id(),
                weapon.sword_arts_param_id()
            ));

            Some(WeaponCandidate {
                base_param_id: param_id,
                unique: weapon.material_set_id() == 2200,
                icon_id: weapon.icon_id(),
                sword_art_id: weapon.sword_arts_param_id(),
                wep_type: weapon.wep_type(),
            })
        })
        .collect::<Vec<_>>();

    log_event(format!(
        "debug fixed pool active: weapon_ids={DEBUG_WEAPON_IDS:?}, gem_ids={DEBUG_GEM_IDS:?}"
    ));
    candidates
}

pub fn choose_debug_ash(
    params: &SoloParamRepository,
    weapon: WeaponCandidate,
    rng: &mut impl Rng,
) -> Option<AshCandidate> {
    let gem_param_id = *DEBUG_GEM_IDS
        .choose(rng)
        .expect("debug gem list is not empty");

    let Some(gem) = params.get::<EquipParamGem>(gem_param_id) else {
        log_event(format!(
            "debug gem missing from EquipParamGem: id={gem_param_id}"
        ));
        return None;
    };

    let sword_art_id = gem.sword_arts_param_id();
    let compatible = can_mount_ash(gem, weapon.wep_type);
    log_event(format!(
        "debug gem candidate: gem_id={gem_param_id}, sword_art={sword_art_id}, default_attr={}, compatible={compatible}",
        gem.default_wep_attr()
    ));

    if let Some(sword_art) = params.get::<SwordArtsParam>(sword_art_id as u32) {
        log_event(format!(
            "debug sword art resolved from gem: id={}, text_id={}, icon_id={}, type_new={}",
            sword_art_id,
            sword_art.text_id(),
            sword_art.icon_id(),
            sword_art.sword_arts_type_new()
        ));
    } else {
        log_event(format!(
            "debug gem resolved to missing SwordArtsParam: gem_id={gem_param_id}, sword_art={sword_art_id}"
        ));
    }

    // Debug 模式故意不因为“不兼容”而跳过，方便验证“强制写战灰”路径。
    if !compatible {
        log_event(format!(
            "debug gem is not marked compatible with wep_type={}; forcing its SwordArtsParam anyway",
            weapon.wep_type
        ));
    }

    Some(AshCandidate {
        gem_param_id: Some(gem_param_id),
        sword_art_id,
        default_weapon_attr: gem.default_wep_attr(),
    })
}
