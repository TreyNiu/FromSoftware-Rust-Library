use eldenring::cs::{EquipParamGem, EquipParamWeapon, SoloParamRepository, SwordArtsParam};
use rand::Rng;

use crate::{
    log::log_event,
    weapon_randomizer::{AshCandidate, WeaponCandidate, can_mount_ash, choose_non_default_ash},
};

// 调试池单独放在这里，方便临时缩小随机范围。
// `weapon.debug_fixed_pool = true` 时：武器和战灰都走这里。
// `weapon.debug_fixed_ash_pool = true` 时：只有战灰走这里，武器仍走正常大池。
// `weapon.ignore_ash_compatibility = true` 时：这里也会允许失色武器随机战灰，并忽略兼容性。
const DEBUG_WEAPON_IDS: [u32; 2] = [21_070_000, 21_080_000];
const DEBUG_GEM_IDS: [u32; 3] = [20_500, 21_400, 21_600];

pub fn collect_debug_weapon_candidates(
    params: &SoloParamRepository,
    include_unique_weapons: bool,
) -> Vec<WeaponCandidate> {
    let candidates = DEBUG_WEAPON_IDS
        .iter()
        .filter_map(|&param_id| {
            let Some(weapon) = params.get::<EquipParamWeapon>(param_id) else {
                log_event(format!("debug weapon missing: id={param_id}"));
                return None;
            };

            let unique = weapon.material_set_id() == 2200;
            if unique && !include_unique_weapons {
                log_event(format!(
                    "debug weapon skipped: id={param_id} is unique and include_unique_weapons=false"
                ));
                return None;
            }

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
                unique,
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
    ignore_compatibility: bool,
    rng: &mut impl Rng,
) -> Option<AshCandidate> {
    // 默认仍尊重“失色/特殊武器不随机战灰”的规则；
    // 只有打开 ignore_ash_compatibility 时，才把它们也纳入强制测试。
    if weapon.unique && !ignore_compatibility {
        log_event(format!(
            "debug ash skipped: weapon base={} is unique, keep original sword art",
            weapon.base_param_id
        ));
        return None;
    }

    let ashes = DEBUG_GEM_IDS
        .iter()
        .filter_map(|&gem_param_id| {
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

            // 强制模式下故意不因为“不兼容”而跳过，方便验证“硬装战灰”路径。
            if !compatible && ignore_compatibility {
                log_event(format!(
                    "debug gem is not marked compatible with wep_type={}; forcing its SwordArtsParam anyway",
                    weapon.wep_type
                ));
            } else if !compatible {
                log_event(format!(
                    "debug ash skipped: gem_id={gem_param_id} is not compatible with wep_type={}",
                    weapon.wep_type
                ));
                return None;
            }

            Some(AshCandidate {
                gem_param_id: Some(gem_param_id),
                sword_art_id,
                default_weapon_attr: gem.default_wep_attr(),
            })
        })
        .collect::<Vec<_>>();

    choose_non_default_ash(&ashes, weapon.sword_art_id, rng)
}
