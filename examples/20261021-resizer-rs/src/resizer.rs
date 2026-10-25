use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
    time::{Duration, Instant, SystemTime},
};

use eldenring::cs::{AtkParam_Pc, ChrIns, LockCamParam, SoloParamRepository, WorldChrMan};
use fromsoftware_shared::FromStatic;

use crate::{
    config::{ResizerConfig, config_modified_time, load_config, load_or_create_config},
    hooks::VmtHook,
    log, memory,
    skeleton::{SkeletonScaleInfo, scale_character_skeleton},
};

const CONFIG_CHECK_INTERVAL: Duration = Duration::from_millis(500);
const CHR_CTRL_OFFSET: usize = 0x58;
const CHR_NPC_PARAM_ID_OFFSET: usize = 0x60;
const CHR_FIELD_INS_HANDLE_OFFSET: usize = 0x08;
const CHR_CLOTH_STATE_OFFSET: usize = 0x548;
const CTRL_SCALE_X_OFFSET: usize = 0x2D4;
const CTRL_SCALE_Y_OFFSET: usize = 0x2D8;
const CTRL_SCALE_Z_OFFSET: usize = 0x2DC;
const PLAYER_HANDLE_MASK: u64 = 0xFFFF_FFFF_16F0_0000;
const SUMMON_HANDLE_MASK: u64 = 0xFFFF_FFFF_1710_0000;
const HANDLE_RANGE_MAX: u64 = 0xFF;
const TORRENT_NPC_PARAM_ID: i32 = 0x04C4_B400;
const SUMMON_MIMIC_NPC_PARAM_IDS: [i32; 2] = [0x05F5_E10A, 0x07F0_7C68];
const WORLD_MIMIC_NPC_PARAM_IDS: [i32; 2] = [0x1F5B_A9E5, 0x1F5B_A654];

pub struct ResizerState {
    config_path: std::path::PathBuf,
    config: Arc<RwLock<ResizerConfig>>,
    last_config_check: Instant,
    last_config_modified: Option<SystemTime>,
    debug_scales: HashMap<usize, DebugScale>,
    cloth_states: HashMap<usize, ClothRuntimeState>,
    lock_cam_baselines: HashMap<u32, [f32; 2]>,
    atk_param_baselines: HashMap<u32, [f32; 4]>,
    params_dirty: bool,
    tick_count: u64,
    logged_characters: HashSet<usize>,
    skeleton_logged: HashSet<usize>,
    _hooks: Vec<VmtHook>,
}

impl ResizerState {
    pub fn new(
        config_path: std::path::PathBuf,
        config: Arc<RwLock<ResizerConfig>>,
        hooks: Vec<VmtHook>,
    ) -> Self {
        let last_config_modified = config_modified_time(&config_path);
        Self {
            config_path,
            config,
            last_config_check: Instant::now(),
            last_config_modified,
            debug_scales: HashMap::new(),
            cloth_states: HashMap::new(),
            lock_cam_baselines: HashMap::new(),
            atk_param_baselines: HashMap::new(),
            params_dirty: true,
            tick_count: 0,
            logged_characters: HashSet::new(),
            skeleton_logged: HashSet::new(),
            _hooks: hooks,
        }
    }

    pub fn tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);
        self.reload_config_if_needed();

        let config = self
            .config
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default();

        if self.tick_count == 1 || self.tick_count.is_multiple_of(300) {
            log::info(format!(
                "tick={} character_scale={} param_scale={} bones={} cloth_disabled={}",
                self.tick_count,
                config.enable_character_scaling,
                config.enable_param_scaling,
                config.scale_bones,
                config.disable_cloth_physics,
            ));
        }

        if !config.enable_param_scaling || (self.params_dirty && self.apply_param_scales(&config)) {
            self.params_dirty = false;
        }

        if config.enable_character_scaling {
            self.apply_character_scales(&config);
        } else if self.tick_count == 1 {
            log::info("character scaling disabled by config");
        }
    }

    fn reload_config_if_needed(&mut self) {
        if self.last_config_check.elapsed() < CONFIG_CHECK_INTERVAL {
            return;
        }
        self.last_config_check = Instant::now();

        let modified = config_modified_time(&self.config_path);
        if modified == self.last_config_modified {
            return;
        }

        let new_config = if modified.is_none() {
            log::info("config file missing, recreating defaults");
            load_or_create_config(&self.config_path)
        } else if let Some(config) = load_config(&self.config_path) {
            config
        } else {
            log::error("config reload failed; keeping previous config");
            return;
        };

        let old_config = self
            .config
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default();
        if new_config == old_config {
            self.last_config_modified = config_modified_time(&self.config_path);
            return;
        }

        if old_config.camera_scale != new_config.camera_scale
            || old_config.weapon_scale != new_config.weapon_scale
            || old_config.enable_param_scaling != new_config.enable_param_scaling
        {
            self.params_dirty = true;
        }

        if let Ok(mut config) = self.config.write() {
            *config = new_config.clone();
        }
        self.last_config_modified = config_modified_time(&self.config_path);
        log::info(format!("config reloaded: {new_config:?}"));
    }

    fn apply_character_scales(&mut self, config: &ResizerConfig) {
        let Ok(world_chr_man) = (unsafe { WorldChrMan::instance_mut() }) else {
            if self.tick_count == 1 {
                log::error("WorldChrMan::instance_mut failed");
            }
            return;
        };

        let mut visited = HashSet::new();
        let mut character_count = 0usize;
        for chr_set in world_chr_man.chr_sets.iter().flatten() {
            for chr in chr_set.characters() {
                character_count += 1;
                let chr_ptr = chr as *mut ChrIns;
                let address = chr_ptr as usize;
                visited.insert(address);

                let Some(raw_handle) = (unsafe { read_raw_handle(chr_ptr) }) else {
                    log_character_once(
                        &mut self.logged_characters,
                        address,
                        format!("address=0x{address:x} invalid FieldInsHandle"),
                    );
                    continue;
                };
                let Some(npc_param_id) =
                    (unsafe { memory::read::<i32>(address + CHR_NPC_PARAM_ID_OFFSET) })
                else {
                    log_character_once(
                        &mut self.logged_characters,
                        address,
                        format!("address=0x{address:x} invalid npc_param_id"),
                    );
                    continue;
                };

                let scale = select_character_scale(raw_handle, npc_param_id, config);
                let Some(ctrl) = (unsafe { memory::read::<usize>(address + CHR_CTRL_OFFSET) })
                else {
                    log_character_once(
                        &mut self.logged_characters,
                        address,
                        format!(
                            "address=0x{address:x} handle=0x{raw_handle:x} npc={npc_param_id} invalid ChrCtrl pointer"
                        ),
                    );
                    continue;
                };
                let Some(base) = self.debug_scale_for(address, ctrl) else {
                    log_character_once(
                        &mut self.logged_characters,
                        address,
                        format!(
                            "address=0x{address:x} handle=0x{raw_handle:x} npc={npc_param_id} ctrl=0x{ctrl:x} invalid scale fields"
                        ),
                    );
                    continue;
                };

                let x = base.x * scale;
                let y = base.y * scale;
                let z = base.z * scale;
                let writes_ok = unsafe {
                    memory::write::<f32>(ctrl + CTRL_SCALE_X_OFFSET, x)
                        && memory::write::<f32>(ctrl + CTRL_SCALE_Y_OFFSET, y)
                        && memory::write::<f32>(ctrl + CTRL_SCALE_Z_OFFSET, z)
                };

                log_character_once(
                    &mut self.logged_characters,
                    address,
                    format!(
                        "address=0x{address:x} handle=0x{raw_handle:x} npc={npc_param_id} ctrl=0x{ctrl:x} scale={scale:.4} xyz=({x:.3},{y:.3},{z:.3}) writes_ok={writes_ok}"
                    ),
                );

                if !writes_ok {
                    continue;
                }

                self.update_cloth_state(address, scale, config);

                if config.scale_bones && (scale - 1.0).abs() > f32::EPSILON {
                    let result = unsafe { scale_character_skeleton(chr_ptr as *mut u8, scale) };
                    if self.skeleton_logged.insert(address) {
                        match result {
                            Ok(info) => log_skeleton_success(address, scale, info),
                            Err(stage) => log::error(format!(
                                "skeleton scale failed: address=0x{address:x} scale={scale:.4} stage={stage}"
                            )),
                        }
                    }
                }
            }
        }

        if self.tick_count == 1 || self.tick_count.is_multiple_of(300) {
            log::info(format!(
                "character scan found {character_count} live characters"
            ));
        }

        self.debug_scales
            .retain(|address, _| visited.contains(address));
        self.cloth_states
            .retain(|address, _| visited.contains(address));
        self.logged_characters
            .retain(|address| visited.contains(address));
        self.skeleton_logged
            .retain(|address| visited.contains(address));
    }

    fn debug_scale_for(&mut self, address: usize, ctrl: usize) -> Option<DebugScale> {
        if let Some(base) = self.debug_scales.get(&address) {
            return Some(*base);
        }

        let base = DebugScale {
            x: unsafe { memory::read::<f32>(ctrl + CTRL_SCALE_X_OFFSET) }?,
            y: unsafe { memory::read::<f32>(ctrl + CTRL_SCALE_Y_OFFSET) }?,
            z: unsafe { memory::read::<f32>(ctrl + CTRL_SCALE_Z_OFFSET) }?,
        };
        self.debug_scales.insert(address, base);
        Some(base)
    }

    fn update_cloth_state(&mut self, address: usize, scale: f32, config: &ResizerConfig) {
        let should_disable = config.disable_cloth_physics && (scale - 1.0).abs() > f32::EPSILON;
        let state = self
            .cloth_states
            .entry(address)
            .or_insert(ClothRuntimeState {
                disabled_by_us: false,
            });
        let slot = address + CHR_CLOTH_STATE_OFFSET;
        let Some(current) = (unsafe { memory::read::<u32>(slot) }) else {
            log_character_once(
                &mut self.logged_characters,
                address,
                format!("address=0x{address:x} invalid cloth state at +0x548"),
            );
            return;
        };

        if should_disable {
            if current != 1 && unsafe { memory::write::<u32>(slot, 1) } {
                state.disabled_by_us = true;
                log::info(format!("cloth disabled by mod: address=0x{address:x}"));
            }
        } else if state.disabled_by_us {
            if current == 1 {
                // -5 is the re-enable/reset value used by ERSkeletonMan's mounted cloth modifier.
                let _ = unsafe { memory::write::<u32>(slot, (-5i32) as u32) };
                log::info(format!("cloth re-enable requested: address=0x{address:x}"));
            }
            state.disabled_by_us = false;
        }
    }

    fn apply_param_scales(&mut self, config: &ResizerConfig) -> bool {
        log::info("applying parameter scales");
        let Ok(params) = (unsafe { SoloParamRepository::instance_mut() }) else {
            log::error("SoloParamRepository::instance_mut failed; will retry");
            return false;
        };

        for (id, row) in params.rows_mut::<LockCamParam>() {
            let baseline = self
                .lock_cam_baselines
                .entry(id)
                .or_insert([row.cam_dist_target(), row.chr_org_offset_y()]);
            row.set_cam_dist_target(baseline[0] * config.camera_scale);
            row.set_chr_org_offset_y(baseline[1] * config.camera_scale);
        }

        for (id, row) in params.rows_mut::<AtkParam_Pc>() {
            let baseline = self.atk_param_baselines.entry(id).or_insert([
                row.hit0_radius(),
                row.hit1_radius(),
                row.hit2_radius(),
                row.hit3_radius(),
            ]);
            row.set_hit0_radius(baseline[0] * config.weapon_scale);
            row.set_hit1_radius(baseline[1] * config.weapon_scale);
            row.set_hit2_radius(baseline[2] * config.weapon_scale);
            row.set_hit3_radius(baseline[3] * config.weapon_scale);
        }

        log::info(format!(
            "parameter scaling applied: camera={:.3}, weapon={:.3}, lockcam_rows={}, atkpc_rows={}",
            config.camera_scale,
            config.weapon_scale,
            self.lock_cam_baselines.len(),
            self.atk_param_baselines.len(),
        ));
        true
    }
}

#[derive(Clone, Copy)]
struct DebugScale {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Clone, Copy)]
struct ClothRuntimeState {
    disabled_by_us: bool,
}

fn log_character_once(logged: &mut HashSet<usize>, address: usize, message: String) {
    if logged.insert(address) {
        log::info(message);
    }
}

fn log_skeleton_success(address: usize, scale: f32, info: SkeletonScaleInfo) {
    log::info(format!(
        "skeleton scale ok: address=0x{address:x} scale={scale:.4} bones={} hkb=0x{:x} hka=0x{:x} bone_data=0x{:x}",
        info.bone_count, info.hkb_character, info.hka_skeleton, info.bone_data
    ));
}

pub fn select_character_scale(raw_handle: u64, npc_param_id: i32, config: &ResizerConfig) -> f32 {
    if (raw_handle ^ PLAYER_HANDLE_MASK) <= HANDLE_RANGE_MAX {
        return config.player_scale;
    }

    if (raw_handle ^ SUMMON_HANDLE_MASK) <= HANDLE_RANGE_MAX {
        if npc_param_id == TORRENT_NPC_PARAM_ID {
            return config.torrent_scale;
        }
        if SUMMON_MIMIC_NPC_PARAM_IDS.contains(&npc_param_id) {
            return config.mimic_scale;
        }
        return config.summon_scale;
    }

    if WORLD_MIMIC_NPC_PARAM_IDS.contains(&npc_param_id) {
        return config.mimic_scale;
    }

    1.0
}

unsafe fn read_raw_handle(chr: *const ChrIns) -> Option<u64> {
    unsafe { memory::read::<u64>(chr as usize + CHR_FIELD_INS_HANDLE_OFFSET) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> ResizerConfig {
        ResizerConfig {
            player_scale: 2.0,
            camera_scale: 2.0,
            weapon_scale: 1.0,
            torrent_scale: 1.5,
            mimic_scale: 1.25,
            summon_scale: 0.75,
            ..ResizerConfig::default()
        }
    }

    #[test]
    fn classifies_player_torrent_mimic_and_summon() {
        let config = config();
        assert_eq!(select_character_scale(PLAYER_HANDLE_MASK, 0, &config), 2.0);
        assert_eq!(
            select_character_scale(SUMMON_HANDLE_MASK, TORRENT_NPC_PARAM_ID, &config),
            1.5
        );
        assert_eq!(
            select_character_scale(SUMMON_HANDLE_MASK, SUMMON_MIMIC_NPC_PARAM_IDS[0], &config),
            1.25
        );
        assert_eq!(
            select_character_scale(SUMMON_HANDLE_MASK, 1234, &config),
            0.75
        );
        assert_eq!(
            select_character_scale(0, WORLD_MIMIC_NPC_PARAM_IDS[0], &config),
            1.25
        );
        assert_eq!(
            select_character_scale(0, SUMMON_MIMIC_NPC_PARAM_IDS[0], &config),
            1.0
        );
        assert_eq!(select_character_scale(0, 1234, &config), 1.0);
    }
}
