use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use eldenring::util::input;
use eldenring::{
    cs::{
        CSGaitemImp, CSWepGaitemIns, EquipGameData, EquipParamWeapon, GaitemHandle, GameDataMan,
        ItemCategory, ItemId, SoloParamRepository,
    },
};
use fromsoftware_shared::{FromStatic, Superclass};
use rand::{Rng, SeedableRng, prelude::IndexedRandom, rngs::StdRng};

use crate::{
    config::WeaponRandomizerConfig,
    log::{beep_toggle, log_event},
    weapon_pools::enabled_weapon_ids,
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
    last_randomized_bucket: Option<u64>,
    backup: Option<WeaponRandomizerBackup>,
}

impl WeaponRandomizer {
    pub fn new(config: WeaponRandomizerConfig, input_check_interval: Duration) -> Self {
        Self {
            // 随机器启动时永远不自动开启；必须由按键触发。
            left: WeaponHandState::new(Hand::Left, false),
            right: WeaponHandState::new(Hand::Right, false),
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
            set_hand_enabled(&mut self.left, false);
        }
        if !config.allow_right_hand {
            set_hand_enabled(&mut self.right, false);
        }
        self.left.last_randomized_bucket = None;
        self.right.last_randomized_bucket = None;
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
    fn new(hand: Hand, enabled: bool) -> Self {
        Self {
            hand,
            enabled,
            last_randomized_bucket: None,
            backup: if enabled {
                capture_weapon_randomizer_backup(hand)
            } else {
                None
            },
        }
    }
}

fn toggle_hand(hand_state: &mut WeaponHandState, config: &WeaponRandomizerConfig) {
    let _ = config;
    set_hand_enabled(hand_state, !hand_state.enabled);
    beep_toggle(hand_state.enabled);
}

fn set_hand_enabled(hand_state: &mut WeaponHandState, enabled: bool) {
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
            "captured {:?} weapon backup: slots={}",
            hand_state.hand,
            hand_state
                .backup
                .as_ref()
                .map(|backup| backup.slots_len())
                .unwrap_or(0)
        ));

        // 开启后允许立即随机一次；没有手动开启时不会写玩家数据。
        hand_state.last_randomized_bucket = None;
    } else {
        if let Some(backup) = hand_state.backup.as_ref() {
            restore_weapon_randomizer_backup(backup);
        }
        hand_state.backup = None;
        hand_state.last_randomized_bucket = None;
    }
}

fn tick_hand(hand_state: &mut WeaponHandState, config: &WeaponRandomizerConfig) {
    if !hand_state.enabled {
        return;
    }

    let bucket = current_time_bucket(config.randomize_interval_seconds);
    if hand_state.last_randomized_bucket == Some(bucket) {
        return;
    }

    if hand_state.backup.is_none() {
        hand_state.backup = capture_weapon_randomizer_backup(hand_state.hand);
    }

    let Some(_backup) = hand_state.backup.as_ref() else {
        log_event(format!(
            "skip: {:?} hand weapon randomizer backup unavailable",
            hand_state.hand
        ));
        hand_state.last_randomized_bucket = Some(bucket);
        return;
    };

    if !randomize_selected_weapon(hand_state, config, bucket) {
        log_event(format!(
            "{:?} hand randomization tick did not apply",
            hand_state.hand
        ));
    }
    hand_state.last_randomized_bucket = Some(bucket);
}

#[derive(Clone, Copy)]
pub struct WeaponCandidate {
    pub base_param_id: u32,
    pub unique: bool,
    pub sword_art_id: i32,
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
}

#[derive(Clone, Copy)]
struct EquippedSlotBackup {
    slot: WeaponSlot,
    original_equipment_param_id: i32,
    original_item_id: ItemId,
}

impl WeaponRandomizerBackup {
    pub fn slots_len(&self) -> usize {
        self.slots.len()
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

fn randomize_selected_weapon(
    hand_state: &mut WeaponHandState,
    config: &WeaponRandomizerConfig,
    time_bucket: u64,
) -> bool {
    let hand = hand_state.hand;
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

    let Ok(solo_params) = (unsafe { SoloParamRepository::instance() }) else {
        log_event("skip: SoloParamRepository::instance failed");
        return false;
    };

    let mut rng = deterministic_rng(config.random_seed, hand, slot, time_bucket);
    let Some(weapon) = choose_weapon_candidate(solo_params, config, &mut rng) else {
        log_event("skip: no weapon candidates");
        return false;
    };

    // ER 的武器 param ID 会把强化等级和质变编码进去；这里只选择真实存在的派生 row。
    let Some(randomized_param) =
        randomized_weapon_param(solo_params, weapon, player_level, config, &mut rng)
    else {
        log_event(format!(
            "skip: no valid derived weapon param rows for base={}",
            weapon.base_param_id
        ));
        return false;
    };

    let applied = apply_randomized_weapon(slot, randomized_param.param_id);

    if applied {
        log_event(format!(
            "{hand:?} hand id-only randomize: slot={slot:?}, bucket={time_bucket}, randomized_base={}, source_row={}, default_sword_art={}, target_param={}, unique={}, infusion_offset={}, reinforcement=+{}",
            weapon.base_param_id,
            randomized_param.source_param_id,
            weapon.sword_art_id,
            randomized_param.param_id,
            weapon.unique,
            randomized_param.infusion_offset,
            randomized_param.reinforcement_level,
        ));
    }

    applied
}

fn choose_weapon_candidate(
    params: &SoloParamRepository,
    config: &WeaponRandomizerConfig,
    rng: &mut impl Rng,
) -> Option<WeaponCandidate> {
    collect_weapon_candidates(params, config).choose(rng).copied()
}

fn current_time_bucket(randomize_interval_seconds: u64) -> u64 {
    let interval = randomize_interval_seconds.max(1);
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() / interval)
        .unwrap_or(0)
}

fn deterministic_rng(
    random_seed: u64,
    hand: Hand,
    slot: WeaponSlot,
    time_bucket: u64,
) -> StdRng {
    let hand_seed = match hand {
        Hand::Left => 0x4c45_4654_u64,
        Hand::Right => 0x5249_4748_54_u64,
    };
    let slot_seed = match slot.position {
        SlotPosition::Primary => 1,
        SlotPosition::Secondary => 2,
        SlotPosition::Tertiary => 3,
    };

    let seed = random_seed
        ^ hand_seed
        ^ time_bucket.wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ (slot_seed << 48);
    StdRng::seed_from_u64(seed)
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
                original_equipment_param_id: equipment_param_id,
                original_item_id: equipment_entry_item_id(equipment, slot),
            }
        })
        .collect::<Vec<_>>();

    Some(WeaponRandomizerBackup { slots })
}

pub fn restore_weapon_randomizer_backup(backup: &WeaponRandomizerBackup) {
    restore_equipped_slot_items(backup);
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

            let unique = weapon.material_set_id() == 2200;

            if weapon.wep_type() == 0 {
                zero_wep_type_rows += 1;
                return None;
            }

            if unique && !config.include_unique_weapons {
                return None;
            }

            Some(WeaponCandidate {
                base_param_id: param_id,
                unique,
                sword_art_id: weapon.sword_arts_param_id(),
            })
        })
        .collect::<Vec<_>>();

    candidates
}

fn randomized_weapon_param(
    params: &SoloParamRepository,
    weapon: WeaponCandidate,
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
    candidate_infusion_offsets(weapon)
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

fn candidate_infusion_offsets(weapon: WeaponCandidate) -> Vec<u32> {
    const STANDARD: u32 = 0;
    const HEAVY: u32 = 100;
    const KEEN: u32 = 200;
    const QUALITY: u32 = 300;

    const BASE: [u32; 4] = [STANDARD, HEAVY, KEEN, QUALITY];

    if weapon.unique {
        return vec![STANDARD];
    }

    BASE.to_vec()
}

fn apply_randomized_weapon(
    slot: WeaponSlot,
    param_id: u32,
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

    weapon_gaitem.gaitem_ins.item_id = item_id.into();
}
