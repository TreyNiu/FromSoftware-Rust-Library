use std::{
    mem::size_of,
    ptr, slice,
    time::{Duration, Instant},
};

use eldenring::util::input;
use eldenring::{
    cs::{
        CSGaitemImp, CSGemGaitemIns, CSWepGaitemIns, EquipGameData, EquipParamGem,
        EquipParamWeapon, GaitemHandle, GameDataMan, ItemCategory, ItemId, SoloParamRepository,
    },
    param::{EQUIP_PARAM_GEM_ST, EQUIP_PARAM_WEAPON_ST},
};
use fromsoftware_shared::{FromStatic, Superclass};
use rand::{Rng, prelude::IndexedRandom};

use crate::{
    config::WeaponRandomizerConfig,
    log::{beep_toggle, log_event},
    weapon_debug_pool::{choose_debug_ash, collect_debug_weapon_candidates},
    weapon_pools::{enabled_pool_summary, enabled_weapon_ids},
};

pub struct WeaponRandomizer {
    config: WeaponRandomizerConfig,
    left: WeaponHandState,
    right: WeaponHandState,
    left_toggle_was_pressed: bool,
    right_toggle_was_pressed: bool,
    last_input_check: Instant,
}

struct WeaponHandState {
    hand: Hand,
    enabled: bool,
    last_randomized: Instant,
    backup: Option<WeaponRandomizerBackup>,
}

impl WeaponRandomizer {
    pub fn new(config: WeaponRandomizerConfig, input_check_interval: Duration) -> Self {
        let randomize_interval = Duration::from_secs(config.randomize_interval_seconds);

        Self {
            // 随机器启动时永远不自动开启；必须由按键触发。
            left: WeaponHandState::new(Hand::Left, false, randomize_interval),
            right: WeaponHandState::new(Hand::Right, false, randomize_interval),
            config,
            left_toggle_was_pressed: false,
            right_toggle_was_pressed: false,
            last_input_check: Instant::now() - input_check_interval,
        }
    }

    pub fn tick(&mut self, input_check_interval: Duration) {
        self.update_toggle_state(input_check_interval);
        tick_hand(&mut self.left, &self.config);
        tick_hand(&mut self.right, &self.config);
    }

    pub fn update_config(&mut self, config: WeaponRandomizerConfig) {
        log_event(format!("weapon randomizer config reloaded: {config:?}"));
        if !config.allow_left_hand {
            set_hand_enabled(&mut self.left, false, &config);
        }
        if !config.allow_right_hand {
            set_hand_enabled(&mut self.right, false, &config);
        }
        self.config = config;
    }

    fn update_toggle_state(&mut self, input_check_interval: Duration) {
        // task 每帧都会跑；按键按配置间隔检查，避免长按 F1/F2 时反复切换。
        if self.last_input_check.elapsed() < input_check_interval {
            return;
        }
        self.last_input_check = Instant::now();

        let left_pressed = input::is_key_pressed(self.config.toggle_left_virtual_key);
        if self.config.allow_left_hand && left_pressed && !self.left_toggle_was_pressed {
            toggle_hand(&mut self.left, &self.config);
        }
        self.left_toggle_was_pressed = left_pressed;

        let right_pressed = input::is_key_pressed(self.config.toggle_right_virtual_key);
        if self.config.allow_right_hand && right_pressed && !self.right_toggle_was_pressed {
            toggle_hand(&mut self.right, &self.config);
        }
        self.right_toggle_was_pressed = right_pressed;
    }
}

impl WeaponHandState {
    fn new(hand: Hand, enabled: bool, randomize_interval: Duration) -> Self {
        Self {
            hand,
            enabled,
            last_randomized: if enabled {
                Instant::now() - randomize_interval
            } else {
                Instant::now()
            },
            backup: if enabled {
                capture_weapon_randomizer_backup(hand)
            } else {
                None
            },
        }
    }
}

fn toggle_hand(hand_state: &mut WeaponHandState, config: &WeaponRandomizerConfig) {
    set_hand_enabled(hand_state, !hand_state.enabled, config);
    beep_toggle(hand_state.enabled);
}

fn set_hand_enabled(
    hand_state: &mut WeaponHandState,
    enabled: bool,
    config: &WeaponRandomizerConfig,
) {
    if hand_state.enabled == enabled {
        return;
    }

    hand_state.enabled = enabled;
    log_event(format!(
        "{:?} hand toggled weapon randomizer: enabled={}",
        hand_state.hand, hand_state.enabled
    ));

    if hand_state.enabled {
        hand_state.backup = capture_weapon_randomizer_backup(hand_state.hand);
        log_event(format!(
            "captured {:?} weapon backup: slots={}, param_rows={}",
            hand_state.hand,
            hand_state
                .backup
                .as_ref()
                .map(|backup| backup.slots_len())
                .unwrap_or(0),
            hand_state
                .backup
                .as_ref()
                .map(|backup| backup.param_rows_len())
                .unwrap_or(0)
        ));

        // 开启后允许立即随机一次；没有手动开启时不会写玩家数据。
        hand_state.last_randomized =
            Instant::now() - Duration::from_secs(config.randomize_interval_seconds);
    } else {
        if let Some(backup) = hand_state.backup.as_ref() {
            restore_weapon_randomizer_backup(backup);
        }
        hand_state.backup = None;
    }
}

fn tick_hand(hand_state: &mut WeaponHandState, config: &WeaponRandomizerConfig) {
    let randomize_interval = Duration::from_secs(config.randomize_interval_seconds);
    if !hand_state.enabled || hand_state.last_randomized.elapsed() < randomize_interval {
        return;
    }

    if hand_state.backup.is_none() {
        hand_state.backup = capture_weapon_randomizer_backup(hand_state.hand);
    }

    let Some(backup) = hand_state.backup.as_ref() else {
        log_event(format!(
            "skip: {:?} hand weapon randomizer backup unavailable",
            hand_state.hand
        ));
        hand_state.last_randomized = Instant::now();
        return;
    };

    if !randomize_selected_weapon(hand_state.hand, backup, config) {
        log_event(format!(
            "{:?} hand randomization tick did not apply",
            hand_state.hand
        ));
    }
    hand_state.last_randomized = Instant::now();
}

#[derive(Clone, Copy)]
pub struct WeaponCandidate {
    pub base_param_id: u32,
    pub unique: bool,
    pub icon_id: u16,
    pub sword_art_id: i32,
    pub wep_type: u16,
}

#[derive(Clone, Copy)]
pub struct AshCandidate {
    pub gem_param_id: Option<u32>,
    pub sword_art_id: i32,
    pub default_weapon_attr: u8,
}

#[derive(Clone, Copy)]
struct RandomizedWeaponParam {
    param_id: u32,
    source_param_id: u32,
    infusion_offset: u32,
    reinforcement_level: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hand {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SlotPosition {
    Primary,
    Secondary,
    Tertiary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WeaponSlot {
    hand: Hand,
    position: SlotPosition,
}

pub struct WeaponRandomizerBackup {
    slots: Vec<EquippedSlotBackup>,
    param_rows: Vec<WeaponParamBackup>,
}

#[derive(Clone, Copy)]
struct EquippedSlotBackup {
    slot: WeaponSlot,
    original_param_row: u32,
    original_equipment_param_id: i32,
    original_item_id: ItemId,
}

struct WeaponParamBackup {
    param_id: u32,
    bytes: Vec<u8>,
}

impl WeaponRandomizerBackup {
    pub fn slots_len(&self) -> usize {
        self.slots.len()
    }

    pub fn param_rows_len(&self) -> usize {
        self.param_rows.len()
    }

    fn target_row_for_slot(&self, slot: WeaponSlot) -> Option<u32> {
        self.slots
            .iter()
            .find(|backup| backup.slot == slot)
            .map(|backup| backup.original_param_row)
    }
}

impl Hand {
    fn slots(self) -> [WeaponSlot; 3] {
        [
            WeaponSlot {
                hand: self,
                position: SlotPosition::Primary,
            },
            WeaponSlot {
                hand: self,
                position: SlotPosition::Secondary,
            },
            WeaponSlot {
                hand: self,
                position: SlotPosition::Tertiary,
            },
        ]
    }
}

impl WeaponSlot {
    fn chr_asm_index(self) -> usize {
        match (self.hand, self.position) {
            (Hand::Left, SlotPosition::Primary) => 0,
            (Hand::Right, SlotPosition::Primary) => 1,
            (Hand::Left, SlotPosition::Secondary) => 2,
            (Hand::Right, SlotPosition::Secondary) => 3,
            (Hand::Left, SlotPosition::Tertiary) => 4,
            (Hand::Right, SlotPosition::Tertiary) => 5,
        }
    }
}

pub fn randomize_selected_weapon(
    hand: Hand,
    backup: &WeaponRandomizerBackup,
    config: &WeaponRandomizerConfig,
) -> bool {
    log_event(format!("{hand:?} hand randomization tick"));

    let Some((slot, player_level)) = selected_weapon_slot_and_level(hand) else {
        log_event(format!(
            "skip: selected {:?} weapon slot or player level unavailable",
            hand
        ));
        return false;
    };

    // 存档/角色还没读完时常见 level == 0。这个状态下不要碰装备槽，避免写到未稳定数据。
    if player_level == 0 {
        log_event("skip: player level is 0, save data may not be loaded yet");
        return false;
    }

    let Some(target_param_row) = backup.target_row_for_slot(slot) else {
        log_event(format!("skip: no backup row found for slot={slot:?}"));
        return false;
    };

    log_event(format!(
        "current slot: {slot:?}, player level: {player_level}, target_param_row={target_param_row}"
    ));

    let Ok(solo_params) = (unsafe { SoloParamRepository::instance() }) else {
        log_event("skip: SoloParamRepository::instance failed");
        return false;
    };

    let weapons = if config.debug_fixed_pool {
        collect_debug_weapon_candidates(solo_params)
    } else {
        collect_weapon_candidates(solo_params, config)
    };
    log_event(format!("weapon candidates: {}", weapons.len()));
    if weapons.is_empty() {
        log_event("skip: no weapon candidates");
        return false;
    }

    let mut rng = rand::rng();
    let weapon = *weapons
        .choose(&mut rng)
        .expect("candidate list is not empty");
    let ash = if !config.randomize_ashes {
        None
    } else if config.debug_fixed_pool {
        choose_debug_ash(solo_params, weapon, &mut rng)
    } else {
        choose_ash_for_weapon(solo_params, weapon, &mut rng)
    };

    log_event(format!(
        "selected weapon base={}, unique={}, wep_type={}, gem={}, sword_art={}",
        weapon.base_param_id,
        weapon.unique,
        weapon.wep_type,
        ash.and_then(|ash| ash.gem_param_id)
            .map(|gem_param_id| gem_param_id as i32)
            .unwrap_or(-1),
        ash.map(|ash| ash.sword_art_id).unwrap_or(-1)
    ));

    // ER 的武器 param ID 会把强化等级和质变编码进去；这里只选择真实存在的派生 row。
    let Some(randomized_param) =
        randomized_weapon_param(solo_params, weapon, ash, player_level, config, &mut rng)
    else {
        log_event(format!(
            "skip: no valid derived weapon param rows for base={}",
            weapon.base_param_id
        ));
        return false;
    };
    let sword_art_id = ash
        .map(|ash| ash.sword_art_id)
        .unwrap_or(weapon.sword_art_id);

    log_event(format!(
        "applying param_id={}, source_param_id={}, icon_id={}, sword_art_id={sword_art_id}, infusion_offset={}, reinforcement=+{}",
        randomized_param.param_id,
        randomized_param.source_param_id,
        weapon.icon_id,
        randomized_param.infusion_offset,
        randomized_param.reinforcement_level
    ));

    apply_randomized_weapon(
        slot,
        randomized_param.param_id,
        randomized_param.source_param_id,
        target_param_row,
        weapon.icon_id,
        ash,
        sword_art_id,
    )
}

fn selected_weapon_slot_and_level(hand: Hand) -> Option<(WeaponSlot, u32)> {
    let game_data = unsafe { GameDataMan::instance() }.ok()?;
    let player_game_data = &game_data.main_player_game_data;

    // chr_asm 记录当前显示/装备选择状态。这里把左右手差异压到 Hand 里。
    let selected_slot = match hand {
        Hand::Left => {
            player_game_data
                .equipment
                .chr_asm
                .equipment
                .selected_slots
                .left_weapon_slot
        }
        Hand::Right => {
            player_game_data
                .equipment
                .chr_asm
                .equipment
                .selected_slots
                .right_weapon_slot
        }
    };

    let position = match selected_slot {
        0 => SlotPosition::Primary,
        1 => SlotPosition::Secondary,
        2 => SlotPosition::Tertiary,
        _ => return None,
    };

    Some((WeaponSlot { hand, position }, player_game_data.level))
}

pub fn capture_weapon_randomizer_backup(hand: Hand) -> Option<WeaponRandomizerBackup> {
    let game_data = unsafe { GameDataMan::instance() }.ok()?;
    let equipment = &game_data.main_player_game_data.equipment;

    // 开启随机时备份该手 3 个槽位，而不是只备份当前槽。
    // 这样玩家开启后切换左一/左二/左三或右一/右二/右三，关闭时仍能恢复整只手。
    let slots = hand
        .slots()
        .into_iter()
        .map(|slot| {
            let chr_asm_index = slot.chr_asm_index();
            let equipment_param_id = equipment.chr_asm.equipment_param_ids[chr_asm_index];

            EquippedSlotBackup {
                slot,
                original_param_row: strip_reinforcement_level(equipment_param_id as u32),
                original_equipment_param_id: equipment_param_id,
                original_item_id: equipment_entry_item_id(equipment, slot),
            }
        })
        .collect::<Vec<_>>();

    Some(WeaponRandomizerBackup {
        param_rows: backup_weapon_param_rows(&slots),
        slots,
    })
}

fn backup_weapon_param_rows(slots: &[EquippedSlotBackup]) -> Vec<WeaponParamBackup> {
    let Ok(params) = (unsafe { SoloParamRepository::instance() }) else {
        log_event("backup skipped: SoloParamRepository::instance failed");
        return Vec::new();
    };

    slots.iter().fold(Vec::new(), |mut backups, slot_backup| {
        let param_id = slot_backup.original_param_row;
        // 多个槽可能指向同一个原始 param row，只备份一次即可。
        if backups
            .iter()
            .any(|backup: &WeaponParamBackup| backup.param_id == param_id)
        {
            return backups;
        }

        let Some(weapon) = params.get::<EquipParamWeapon>(param_id) else {
            log_event(format!("backup skipped: weapon row {param_id} not found"));
            return backups;
        };

        backups.push(WeaponParamBackup {
            param_id,
            bytes: weapon_param_to_bytes(weapon),
        });
        log_event(format!("backed up weapon param row={param_id}"));
        backups
    })
}

pub fn restore_weapon_randomizer_backup(backup: &WeaponRandomizerBackup) {
    restore_weapon_param_rows(backup);
    restore_equipped_slot_items(backup);
}

fn restore_weapon_param_rows(backup: &WeaponRandomizerBackup) {
    let Ok(params) = (unsafe { SoloParamRepository::instance_mut() }) else {
        log_event("restore skipped: SoloParamRepository::instance failed");
        return;
    };

    for row_backup in &backup.param_rows {
        let Some(weapon) = params.get_mut::<EquipParamWeapon>(row_backup.param_id) else {
            log_event(format!(
                "restore skipped: weapon row {} not found",
                row_backup.param_id
            ));
            continue;
        };

        write_weapon_param_from_bytes(weapon, &row_backup.bytes);
        log_event(format!("restored weapon param row={}", row_backup.param_id));
    }
}

fn restore_equipped_slot_items(backup: &WeaponRandomizerBackup) {
    let Ok(game_data) = (unsafe { GameDataMan::instance_mut() }) else {
        log_event("restore slot items skipped: GameDataMan::instance failed");
        return;
    };

    let equipment = &mut game_data.main_player_game_data.equipment;
    for slot_backup in &backup.slots {
        let slot = slot_backup.slot;
        let chr_asm_index = slot.chr_asm_index();

        set_equipment_entry_item_id(equipment, slot, slot_backup.original_item_id);
        equipment.chr_asm.equipment_param_ids[chr_asm_index] =
            slot_backup.original_equipment_param_id;

        let weapon_handle = equipment.chr_asm.gaitem_handles[chr_asm_index];
        sync_equipped_inventory_item_id(equipment, weapon_handle, slot_backup.original_item_id);
        sync_weapon_gaitem_item_id(weapon_handle, slot_backup.original_item_id);

        log_event(format!(
            "restored slot item: slot={slot:?}, item_id={:?}, equipment_param_id={}",
            slot_backup.original_item_id, slot_backup.original_equipment_param_id
        ));
    }
}

fn collect_weapon_candidates(
    params: &SoloParamRepository,
    config: &WeaponRandomizerConfig,
) -> Vec<WeaponCandidate> {
    let ids = enabled_weapon_ids(&config.enabled_wepmotion_categories);
    let mut missing_rows = 0usize;
    let mut zero_wep_type_rows = 0usize;

    // weapon_pools.rs 里只有从 CSV 生成的“基础武器 ID 白名单”。
    // 这里再到运行时 EquipParamWeapon 表里确认 row 真的存在，并顺手取 wep_type/icon/默认战技。
    let candidates = ids
        .iter()
        .filter_map(|&param_id| {
            let Some(weapon) = params.get::<EquipParamWeapon>(param_id) else {
                missing_rows += 1;
                return None;
            };

            if weapon.wep_type() == 0 {
                zero_wep_type_rows += 1;
                return None;
            }

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
        "weapon pool scan: enabled_ids={}, missing_rows={missing_rows}, zero_wep_type_rows={zero_wep_type_rows}, candidates={}, enabled_pools=[{}]",
        ids.len(),
        candidates.len(),
        enabled_pool_summary(&config.enabled_wepmotion_categories)
    ));

    candidates
}

fn choose_ash_for_weapon(
    params: &SoloParamRepository,
    weapon: WeaponCandidate,
    rng: &mut impl Rng,
) -> Option<AshCandidate> {
    // 特殊/失色武器通常有固定战技，这里不随机它们的战灰。
    if weapon.unique {
        return None;
    }

    // EquipParamGem 里有每类武器的可装配 bit，直接用 generated getter 判断兼容性。
    let ashes = params
        .rows::<EquipParamGem>()
        .filter(|(gem_param_id, ash)| {
            *gem_param_id >= 10_000
                && ash.sword_arts_param_id() >= 0
                && can_mount_ash(ash, weapon.wep_type)
        })
        .map(|(gem_param_id, ash)| AshCandidate {
            gem_param_id: Some(gem_param_id),
            sword_art_id: ash.sword_arts_param_id(),
            default_weapon_attr: ash.default_wep_attr(),
        })
        .collect::<Vec<_>>();

    ashes.choose(rng).copied()
}

fn randomized_weapon_param(
    params: &SoloParamRepository,
    weapon: WeaponCandidate,
    ash: Option<AshCandidate>,
    player_level: u32,
    config: &WeaponRandomizerConfig,
    rng: &mut impl Rng,
) -> Option<RandomizedWeaponParam> {
    let max_reinforcement_level = if weapon.unique { 10 } else { 25 };
    let level = scaled_reinforcement_level(
        player_level,
        max_reinforcement_level,
        config.scale_to_player_level_cap,
    );

    // CSV 里通常只列基础 row 和质变 row；强化等级 row 以游戏运行时 param 表为准。
    // 这里先算目标强化等级，再向下回退查找真实存在的 row。这样遇到不支持 +25/+10
    // 的特殊武器、盾牌或奇怪 DLC row 时，不会整次随机直接失败。
    candidate_infusion_offsets(weapon, ash)
        .into_iter()
        .filter_map(|infusion| {
            let param_id = weapon.base_param_id + infusion + level;
            params
                .get::<EquipParamWeapon>(weapon.base_param_id + infusion)
                .map(|_| RandomizedWeaponParam {
                    param_id,
                    source_param_id: weapon.base_param_id + infusion,
                    infusion_offset: infusion,
                    reinforcement_level: level,
                })
        })
        .collect::<Vec<_>>()
        .choose(rng)
        .copied()
}

fn scaled_reinforcement_level(
    player_level: u32,
    max_reinforcement_level: u32,
    scale_to_player_level_cap: u32,
) -> u32 {
    if max_reinforcement_level == 0
        || scale_to_player_level_cap == 0
        || player_level >= scale_to_player_level_cap
    {
        return max_reinforcement_level;
    }

    // 对应旧 C# 工具里的“按玩家等级缩放武器强化”：
    // 0 级附近是 +0，到配置里的等级上限时达到该武器最高强化。
    let levels = scale_to_player_level_cap as f32 / max_reinforcement_level as f32;
    (player_level as f32 / levels).floor() as u32
}

fn candidate_infusion_offsets(weapon: WeaponCandidate, ash: Option<AshCandidate>) -> Vec<u32> {
    const STANDARD: u32 = 0;
    const HEAVY: u32 = 100;
    const KEEN: u32 = 200;
    const QUALITY: u32 = 300;
    const FIRE: u32 = 400;
    const FLAME_ART: u32 = 500;
    const LIGHTNING: u32 = 600;
    const SACRED: u32 = 700;
    const MAGIC: u32 = 800;
    const COLD: u32 = 900;
    const POISON: u32 = 1000;
    const BLOOD: u32 = 1100;
    const OCCULT: u32 = 1200;

    const BASE: [u32; 4] = [STANDARD, HEAVY, KEEN, QUALITY];
    const ALL: [u32; 13] = [
        STANDARD, HEAVY, KEEN, QUALITY, FIRE, FLAME_ART, LIGHTNING, SACRED, MAGIC, COLD, POISON,
        BLOOD, OCCULT,
    ];
    const MAGIC_FAMILY: [u32; 6] = [STANDARD, HEAVY, KEEN, QUALITY, MAGIC, COLD];
    const FIRE_FAMILY: [u32; 6] = [STANDARD, HEAVY, KEEN, QUALITY, FIRE, FLAME_ART];
    const SACRED_FAMILY: [u32; 6] = [STANDARD, HEAVY, KEEN, QUALITY, LIGHTNING, SACRED];
    const OCCULT_FAMILY: [u32; 7] = [STANDARD, HEAVY, KEEN, QUALITY, POISON, BLOOD, OCCULT];

    if weapon.unique {
        return vec![STANDARD];
    }

    // default_weapon_attr 控制这个战灰允许哪些质变家族。
    // 这里先给出“可能的质变 offset”，真正可用性仍会在 randomized_weapon_param 里按 row 存在性过滤。
    match ash.map(|ash| ash.default_weapon_attr) {
        Some(0..=3) => ALL.to_vec(),
        Some(4..=5) => FIRE_FAMILY.to_vec(),
        Some(6..=7) => SACRED_FAMILY.to_vec(),
        Some(8..=9) => MAGIC_FAMILY.to_vec(),
        Some(10..=12) => OCCULT_FAMILY.to_vec(),
        _ => BASE.to_vec(),
    }
}

fn apply_randomized_weapon(
    slot: WeaponSlot,
    param_id: u32,
    source_param_id: u32,
    patch_target_param_id: u32,
    icon_id: u16,
    ash: Option<AshCandidate>,
    sword_art_id: i32,
) -> bool {
    let Ok(game_data) = (unsafe { GameDataMan::instance_mut() }) else {
        log_event("apply failed: GameDataMan::instance failed");
        return false;
    };

    let Ok(item_id) = ItemId::new(ItemCategory::Weapon, param_id) else {
        log_event(format!("apply failed: invalid weapon item id {param_id}"));
        return false;
    };

    let equipment = &mut game_data.main_player_game_data.equipment;
    let chr_asm_index = slot.chr_asm_index();

    // equipment_entries 更接近玩家装备数据本身；
    // chr_asm 是运行时/渲染侧当前装备选择视图。两边都写，游戏更容易立即反映变化。
    set_equipment_entry_item_id(equipment, slot, item_id);
    equipment.chr_asm.equipment_param_ids[chr_asm_index] = param_id as i32;

    let weapon_handle = equipment.chr_asm.gaitem_handles[chr_asm_index];
    sync_equipped_inventory_item_id(equipment, weapon_handle, item_id);
    sync_weapon_gaitem_item_id(weapon_handle, item_id);

    replace_equipped_weapon_param(
        patch_target_param_id,
        source_param_id,
        icon_id,
        sword_art_id,
    );
    patch_equipped_gaitem_ash(slot, ash, sword_art_id);
    log_event("apply complete");
    true
}

fn equipment_entry_item_id(equipment: &EquipGameData, slot: WeaponSlot) -> ItemId {
    match (slot.hand, slot.position) {
        (Hand::Left, SlotPosition::Primary) => equipment.equipment_entries.weapon_primary_left,
        (Hand::Right, SlotPosition::Primary) => equipment.equipment_entries.weapon_primary_right,
        (Hand::Left, SlotPosition::Secondary) => equipment.equipment_entries.weapon_secondary_left,
        (Hand::Right, SlotPosition::Secondary) => {
            equipment.equipment_entries.weapon_secondary_right
        }
        (Hand::Left, SlotPosition::Tertiary) => equipment.equipment_entries.weapon_tertiary_left,
        (Hand::Right, SlotPosition::Tertiary) => equipment.equipment_entries.weapon_tertiary_right,
    }
}

fn set_equipment_entry_item_id(equipment: &mut EquipGameData, slot: WeaponSlot, item_id: ItemId) {
    match (slot.hand, slot.position) {
        (Hand::Left, SlotPosition::Primary) => {
            equipment.equipment_entries.weapon_primary_left = item_id
        }
        (Hand::Right, SlotPosition::Primary) => {
            equipment.equipment_entries.weapon_primary_right = item_id
        }
        (Hand::Left, SlotPosition::Secondary) => {
            equipment.equipment_entries.weapon_secondary_left = item_id
        }
        (Hand::Right, SlotPosition::Secondary) => {
            equipment.equipment_entries.weapon_secondary_right = item_id
        }
        (Hand::Left, SlotPosition::Tertiary) => {
            equipment.equipment_entries.weapon_tertiary_left = item_id
        }
        (Hand::Right, SlotPosition::Tertiary) => {
            equipment.equipment_entries.weapon_tertiary_right = item_id
        }
    }
}

fn sync_equipped_inventory_item_id(
    equipment: &mut EquipGameData,
    weapon_handle: GaitemHandle,
    item_id: ItemId,
) {
    let mut found = false;

    // 装备栏 UI 会读 inventory entry；只改 chr_asm 时，手里模型会变，但背包图标/详情可能不变。
    for entry in equipment.equip_inventory_data.items_data.items_mut() {
        if entry.gaitem_handle == weapon_handle {
            log_event(format!(
                "sync inventory entry: handle={weapon_handle}, old_item_id={:?}, new_item_id={item_id:?}",
                entry.item_id
            ));
            entry.item_id = item_id;
            found = true;
            break;
        }
    }

    if !found {
        log_event(format!(
            "sync inventory entry skipped: no entry found for handle={weapon_handle}"
        ));
    }
}

fn sync_weapon_gaitem_item_id(weapon_handle: GaitemHandle, item_id: ItemId) {
    let Ok(gaitems) = (unsafe { CSGaitemImp::instance_mut() }) else {
        log_event("sync weapon gaitem skipped: CSGaitemImp::instance failed");
        return;
    };

    // gaitem 是游戏运行时的物品实例。动作、显示和战灰实例状态经常会从这里继续往下读。
    let Some(weapon_gaitem) = gaitems.gaitem_ins_by_handle_mut(&weapon_handle) else {
        log_event(format!(
            "sync weapon gaitem skipped: handle not found {weapon_handle}"
        ));
        return;
    };

    let Some(weapon_gaitem) = weapon_gaitem.as_subclass_mut::<CSWepGaitemIns>() else {
        log_event(format!(
            "sync weapon gaitem skipped: handle is not CSWepGaitemIns, item_id={:?}",
            weapon_gaitem.item_id
        ));
        return;
    };

    log_event(format!(
        "sync weapon gaitem: handle={weapon_handle}, old_item_id={:?}, new_item_id={item_id:?}",
        weapon_gaitem.gaitem_ins.item_id
    ));
    weapon_gaitem.gaitem_ins.item_id = item_id.into();
}

fn replace_equipped_weapon_param(
    target_param_id: u32,
    source_param_id: u32,
    icon_id: u16,
    sword_art_id: i32,
) {
    let Ok(params) = (unsafe { SoloParamRepository::instance_mut() }) else {
        log_event("param replacement skipped: SoloParamRepository::instance failed");
        return;
    };

    let Some(source_weapon) = params.get::<EquipParamWeapon>(source_param_id) else {
        log_event(format!(
            "param replacement skipped: source weapon row {source_param_id} not found"
        ));
        return;
    };
    let source_bytes = weapon_param_to_bytes(source_weapon);

    let Some(target_weapon) = params.get_mut::<EquipParamWeapon>(target_param_id) else {
        log_event(format!(
            "param replacement skipped: target weapon row {target_param_id} not found"
        ));
        return;
    };

    log_event(format!(
        "replacing weapon param row: target={target_param_id}, source={source_param_id}, old_icon={}, old_sword_art={}, new_icon={icon_id}, new_sword_art={sword_art_id}",
        target_weapon.icon_id(),
        target_weapon.sword_arts_param_id()
    ));

    // 直接覆盖原装备 row，背包详情页会读到新武器的补正、重量、属性需求等完整字段。
    // 战技仍然用本轮随机出来的战灰覆盖，因为 source row 自己通常还是武器默认战技。
    write_weapon_param_from_bytes(target_weapon, &source_bytes);
    target_weapon.set_icon_id(icon_id);
    target_weapon.set_sword_arts_param_id(sword_art_id);
    log_event("param replacement complete");
}

fn weapon_param_to_bytes(weapon: &EQUIP_PARAM_WEAPON_ST) -> Vec<u8> {
    unsafe {
        slice::from_raw_parts(
            ptr::from_ref(weapon).cast::<u8>(),
            size_of::<EQUIP_PARAM_WEAPON_ST>(),
        )
        .to_vec()
    }
}

fn write_weapon_param_from_bytes(weapon: &mut EQUIP_PARAM_WEAPON_ST, bytes: &[u8]) {
    if bytes.len() != size_of::<EQUIP_PARAM_WEAPON_ST>() {
        log_event(format!(
            "weapon param byte copy skipped: expected {} bytes, got {}",
            size_of::<EQUIP_PARAM_WEAPON_ST>(),
            bytes.len()
        ));
        return;
    }

    unsafe {
        ptr::copy_nonoverlapping(
            bytes.as_ptr(),
            ptr::from_mut(weapon).cast::<u8>(),
            size_of::<EQUIP_PARAM_WEAPON_ST>(),
        );
    }
}

fn patch_equipped_gaitem_ash(
    slot: WeaponSlot,
    ash: Option<AshCandidate>,
    fallback_sword_art_id: i32,
) {
    let chr_asm_index = slot.chr_asm_index();

    let Ok(game_data) = (unsafe { GameDataMan::instance() }) else {
        log_event("gaitem ash patch skipped: GameDataMan::instance failed");
        return;
    };

    let weapon_handle = game_data
        .main_player_game_data
        .equipment
        .chr_asm
        .gaitem_handles[chr_asm_index];

    log_event(format!(
        "gaitem slot lookup: slot={slot:?}, chr_asm_index={chr_asm_index}, weapon_handle={weapon_handle}"
    ));

    let Ok(gaitems) = (unsafe { CSGaitemImp::instance_mut() }) else {
        log_event("gaitem ash patch skipped: CSGaitemImp::instance failed");
        return;
    };

    let Some(weapon_gaitem) = gaitems.gaitem_ins_by_handle_mut(&weapon_handle) else {
        log_event("gaitem ash patch skipped: weapon gaitem handle not found");
        return;
    };

    let Some(weapon_gaitem) = weapon_gaitem.as_subclass_mut::<CSWepGaitemIns>() else {
        log_event(format!(
            "gaitem ash patch skipped: handle is not CSWepGaitemIns, item_id={:?}",
            weapon_gaitem.item_id
        ));
        return;
    };

    let weapon_item_id = weapon_gaitem.gaitem_ins.item_id;
    let gem_handle = weapon_gaitem.gem_slot_table.gem_slots[0].gaitem_handle;
    log_event(format!(
        "weapon gaitem: item_id={:?}, gem_handle={gem_handle}",
        weapon_item_id
    ));

    if gem_handle.0 == 0 {
        log_event("gaitem ash patch skipped: weapon has no gem handle in slot 0");
        return;
    }

    let Some(gem_gaitem) = gaitems.gaitem_ins_by_handle_mut(&gem_handle) else {
        log_event("gaitem ash patch skipped: gem gaitem handle not found");
        return;
    };

    let Some(gem_gaitem) = gem_gaitem.as_subclass_mut::<CSGemGaitemIns>() else {
        log_event(format!(
            "gaitem ash patch skipped: gem handle is not CSGemGaitemIns, item_id={:?}",
            gem_gaitem.item_id
        ));
        return;
    };

    let Some(ash) = ash else {
        log_event(format!(
            "gaitem ash patch skipped: no gem candidate for sword_art={fallback_sword_art_id}"
        ));
        return;
    };

    let Some(gem_param_id) = ash.gem_param_id else {
        log_event(format!(
            "gaitem ash patch skipped: sword_art={} has no compatible gem row for this weapon",
            ash.sword_art_id
        ));
        return;
    };

    let Ok(gem_item_id) = ItemId::new(ItemCategory::Gem, gem_param_id) else {
        log_event(format!(
            "gaitem ash patch skipped: invalid gem item id {}",
            gem_param_id
        ));
        return;
    };

    log_event(format!(
        "patching gem gaitem: old_item_id={:?}, new_gem_id={}, sword_art={}, old_weapon_handle={}, new_weapon_handle={weapon_handle}",
        gem_gaitem.gaitem_ins.item_id, gem_param_id, ash.sword_art_id, gem_gaitem.weapon_handle
    ));

    // DLC 后战灰更像是武器实例上的“宝石/Gem 道具”状态；只改武器 param
    // 可能足够更新图标，但战技名和实际施放可能仍会继续读这个 gem gaitem。
    gem_gaitem.gaitem_ins.item_id = gem_item_id.into();
    gem_gaitem.weapon_handle = weapon_handle;
    log_event("gaitem ash patch complete");
}

fn strip_reinforcement_level(param_id: u32) -> u32 {
    // 旧 C# 版是 DeleteFromEnd(id, 2) * 100：
    // 去掉最后两位强化等级，但保留质变偏移。
    (param_id / 100) * 100
}

pub fn can_mount_ash(ash: &EQUIP_PARAM_GEM_ST, wep_type: u16) -> bool {
    // wep_type 来自 EquipParamWeapon::wep_type。
    // 每个 generated getter 对应 EquipParamGem 里一个可装配武器类型 bit。
    match wep_type {
        1 => ash.can_mount_wep_dagger(),
        3 => ash.can_mount_wep_sword_normal(),
        5 => ash.can_mount_wep_sword_large(),
        7 => ash.can_mount_wep_sword_gigantic(),
        9 => ash.can_mount_wep_saber_normal(),
        11 => ash.can_mount_wep_saber_large(),
        13 => ash.can_mount_wep_katana(),
        14 => ash.can_mount_wep_sword_double_edge(),
        15 => ash.can_mount_wep_sword_pierce(),
        16 => ash.can_mount_wep_rapier_heavy(),
        17 => ash.can_mount_wep_axe_normal(),
        19 => ash.can_mount_wep_axe_large(),
        21 => ash.can_mount_wep_hammer_normal(),
        23 => ash.can_mount_wep_hammer_large(),
        24 => ash.can_mount_wep_flail(),
        25 => ash.can_mount_wep_spear_normal(),
        27 => ash.can_mount_wep_spear_large(),
        28 => ash.can_mount_wep_spear_heavy(),
        29 => ash.can_mount_wep_spear_axe(),
        31 => ash.can_mount_wep_sickle(),
        35 => ash.can_mount_wep_knuckle(),
        37 => ash.can_mount_wep_claw(),
        39 => ash.can_mount_wep_whip(),
        41 => ash.can_mount_wep_axhammer_large(),
        50 => ash.can_mount_wep_bow_small(),
        51 => ash.can_mount_wep_bow_normal(),
        53 => ash.can_mount_wep_bow_large(),
        55 => ash.can_mount_wep_closs_bow(),
        56 => ash.can_mount_wep_ballista(),
        57 => ash.can_mount_wep_staff(),
        58 => ash.can_mount_wep_sorcery(),
        61 => ash.can_mount_wep_talisman(),
        65 => ash.can_mount_wep_shield_small(),
        67 => ash.can_mount_wep_shield_normal(),
        69 => ash.can_mount_wep_shield_large(),
        87 => ash.can_mount_wep_torch(),
        // DLC 新增武器类型。getter 已经由当前 paramdef 生成出来了，
        // 之前只是这里的 wep_type -> can_mount bit 映射还没补上。
        88 => ash.can_mount_wep_hand_to_hand(),
        89 => ash.can_mount_wep_perfume_bottle(),
        90 => ash.can_mount_wep_thrusting_shield(),
        91 => ash.can_mount_wep_throwing_weapon(),
        92 => ash.can_mount_wep_reverse_hand_sword(),
        93 => ash.can_mount_wep_light_greatsword(),
        94 => ash.can_mount_wep_great_katana(),
        95 => ash.can_mount_wep_beast_claw(),
        _ => false,
    }
}
