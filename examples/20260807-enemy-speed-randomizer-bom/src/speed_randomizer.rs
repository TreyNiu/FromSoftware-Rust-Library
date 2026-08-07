use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use eldenring::{
    cs::{CSChrBehaviorModule, ChrType, WorldChrMan},
    util::input,
};
use fromsoftware_shared::FromStatic;
use rand::Rng;
use windows::Win32::{
    System::Diagnostics::Debug::MessageBeep,
    UI::WindowsAndMessaging::{MB_ICONHAND, MB_ICONINFORMATION},
};

use crate::config::EnemySpeedConfig;

pub struct EnemySpeedRandomizer {
    config: EnemySpeedConfig,
    enabled: bool,
    toggle_was_pressed: bool,
    last_input_check: Instant,
    channel: SpeedChannelState,
    uses_first_pool: bool,
    current_pool_bounds: (u32, u32),
    modified_any_enemy: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct SpeedStatus {
    pub enabled: bool,
    pub speed_enabled: bool,
    pub multiplier: f32,
    pub countdown_ms: Option<u64>,
    pub individual_enemy_speed: bool,
}

struct SpeedChannelState {
    target_percent: Option<u32>,
    enemy_targets: HashMap<usize, u32>,
    last_randomized: Instant,
    has_randomized: bool,
}

impl EnemySpeedRandomizer {
    pub fn new(config: &EnemySpeedConfig, input_check_interval: Duration) -> Self {
        Self {
            config: config.clone(),
            enabled: false,
            toggle_was_pressed: false,
            last_input_check: Instant::now() - input_check_interval,
            channel: SpeedChannelState::new(),
            uses_first_pool: true,
            current_pool_bounds: pool_1_bounds(config),
            modified_any_enemy: false,
        }
    }

    pub fn tick(&mut self, input_check_interval: Duration) {
        self.update_toggle_state(input_check_interval);

        if self.enabled && self.config.enable {
            if self.config.randomize_each_enemy {
                if self
                    .channel
                    .should_randomize(self.config.randomize_interval_ms)
                {
                    self.randomize_enemy_targets();
                }
                self.apply_individual_enemy_speed();
            } else {
                if self
                    .channel
                    .should_randomize(self.config.randomize_interval_ms)
                {
                    let (min_percent, max_percent) = self.next_pool_bounds();
                    let percent = random_percentage(min_percent, max_percent, &mut rand::rng());
                    self.channel.set_target(percent);
                }
                self.apply_shared_speed();
            }
        } else {
            self.restore_speed();
        }
    }

    pub fn status(&self) -> SpeedStatus {
        let speed_enabled = self.enabled && self.config.enable;
        SpeedStatus {
            enabled: self.enabled,
            speed_enabled,
            multiplier: if speed_enabled && !self.config.randomize_each_enemy {
                self.channel.current_multiplier()
            } else {
                1.0
            },
            countdown_ms: speed_enabled
                .then(|| self.channel.countdown_ms(self.config.randomize_interval_ms)),
            individual_enemy_speed: self.config.randomize_each_enemy,
        }
    }

    pub fn update_config(&mut self, config: &EnemySpeedConfig) {
        if self.config == *config {
            return;
        }
        self.config = config.clone();
        self.toggle_was_pressed = false;
        self.channel.clear_targets();
        self.uses_first_pool = true;
        self.current_pool_bounds = pool_1_bounds(config);
    }

    fn update_toggle_state(&mut self, input_check_interval: Duration) {
        if self.last_input_check.elapsed() < input_check_interval {
            return;
        }
        self.last_input_check = Instant::now();

        let pressed = input::is_key_pressed(self.config.toggle_virtual_key);
        if pressed && !self.toggle_was_pressed {
            self.enabled = !self.enabled;
            self.channel.clear_targets();
            self.uses_first_pool = true;
            self.current_pool_bounds = pool_1_bounds(&self.config);
            beep_toggle(self.enabled);
        }
        self.toggle_was_pressed = pressed;
    }

    fn next_pool_bounds(&mut self) -> (u32, u32) {
        let bounds = if self.uses_first_pool {
            pool_1_bounds(&self.config)
        } else {
            pool_2_bounds(&self.config)
        };
        self.uses_first_pool = !self.uses_first_pool;
        self.current_pool_bounds = bounds;
        bounds
    }

    fn randomize_enemy_targets(&mut self) {
        let Some(enemy_addresses) = current_enemy_addresses() else {
            return;
        };
        let bounds = self.next_pool_bounds();
        let mut rng = rand::rng();
        let targets = enemy_addresses
            .into_iter()
            .map(|address| (address, random_percentage(bounds.0, bounds.1, &mut rng)))
            .collect();
        self.channel.set_enemy_targets(targets);
    }

    fn apply_shared_speed(&mut self) {
        let Some(percent) = self.channel.target_percent else {
            return;
        };
        let Ok(world_chr_man) = (unsafe { WorldChrMan::instance_mut() }) else {
            return;
        };
        let target_speed = percentage_to_multiplier(percent);
        let mut modified_any_enemy = false;

        for chr_set in world_chr_man.chr_sets.iter().flatten() {
            for chr in chr_set.characters() {
                if !is_enemy(chr.chr_type) || chr as *const _ as usize == 0 {
                    continue;
                }

                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let behavior: &mut CSChrBehaviorModule = chr.modules.behavior.as_mut();
                    behavior.animation_speed = target_speed;
                    modified_any_enemy = true;
                }));
            }
        }
        self.modified_any_enemy |= modified_any_enemy;
    }

    fn apply_individual_enemy_speed(&mut self) {
        let Ok(world_chr_man) = (unsafe { WorldChrMan::instance_mut() }) else {
            return;
        };
        let mut rng = rand::rng();
        let bounds = self.current_pool_bounds;
        let mut modified_any_enemy = false;

        for chr_set in world_chr_man.chr_sets.iter().flatten() {
            for chr in chr_set.characters() {
                if !is_enemy(chr.chr_type) || chr as *const _ as usize == 0 {
                    continue;
                }

                let address = chr as *const _ as usize;
                let percent = *self
                    .channel
                    .enemy_targets
                    .entry(address)
                    .or_insert_with(|| random_percentage(bounds.0, bounds.1, &mut rng));
                let target_speed = percentage_to_multiplier(percent);

                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let behavior: &mut CSChrBehaviorModule = chr.modules.behavior.as_mut();
                    behavior.animation_speed = target_speed;
                    modified_any_enemy = true;
                }));
            }
        }
        self.modified_any_enemy |= modified_any_enemy;
    }

    fn restore_speed(&mut self) {
        if !self.modified_any_enemy {
            return;
        }
        let Ok(world_chr_man) = (unsafe { WorldChrMan::instance_mut() }) else {
            return;
        };

        for chr_set in world_chr_man.chr_sets.iter().flatten() {
            for chr in chr_set.characters() {
                if !is_enemy(chr.chr_type) || chr as *const _ as usize == 0 {
                    continue;
                }

                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let behavior: &mut CSChrBehaviorModule = chr.modules.behavior.as_mut();
                    behavior.animation_speed = 1.0;
                }));
            }
        }
        self.modified_any_enemy = false;
        self.channel.clear_targets();
    }
}

impl SpeedChannelState {
    fn new() -> Self {
        Self {
            target_percent: None,
            enemy_targets: HashMap::new(),
            last_randomized: Instant::now(),
            has_randomized: false,
        }
    }

    fn should_randomize(&self, interval_ms: u64) -> bool {
        !self.has_randomized
            || self.last_randomized.elapsed() >= Duration::from_millis(interval_ms.max(1))
    }

    fn set_target(&mut self, percent: u32) {
        self.target_percent = Some(percent);
        self.enemy_targets.clear();
        self.last_randomized = Instant::now();
        self.has_randomized = true;
    }

    fn set_enemy_targets(&mut self, targets: HashMap<usize, u32>) {
        self.target_percent = None;
        self.enemy_targets = targets;
        self.last_randomized = Instant::now();
        self.has_randomized = true;
    }

    fn clear_targets(&mut self) {
        self.target_percent = None;
        self.enemy_targets.clear();
        self.has_randomized = false;
    }

    fn current_multiplier(&self) -> f32 {
        self.target_percent
            .map(percentage_to_multiplier)
            .unwrap_or(1.0)
    }

    fn countdown_ms(&self, interval_ms: u64) -> u64 {
        interval_ms
            .max(1)
            .saturating_sub(self.last_randomized.elapsed().as_millis() as u64)
    }
}

fn current_enemy_addresses() -> Option<Vec<usize>> {
    let Ok(world_chr_man) = (unsafe { WorldChrMan::instance_mut() }) else {
        return None;
    };
    let mut addresses = Vec::new();

    for chr_set in world_chr_man.chr_sets.iter().flatten() {
        for chr in chr_set.characters() {
            if is_enemy(chr.chr_type) && chr as *const _ as usize != 0 {
                addresses.push(chr as *const _ as usize);
            }
        }
    }
    Some(addresses)
}

fn pool_1_bounds(config: &EnemySpeedConfig) -> (u32, u32) {
    (config.pool_1_min_percent, config.pool_1_max_percent)
}

fn pool_2_bounds(config: &EnemySpeedConfig) -> (u32, u32) {
    (config.pool_2_min_percent, config.pool_2_max_percent)
}

fn is_enemy(chr_type: ChrType) -> bool {
    chr_type == ChrType::Npc
}

fn beep_toggle(enabled: bool) {
    let sound = if enabled {
        MB_ICONINFORMATION
    } else {
        MB_ICONHAND
    };
    let _ = unsafe { MessageBeep(sound) };
}

fn random_percentage(min_percent: u32, max_percent: u32, rng: &mut impl Rng) -> u32 {
    let (min_percent, max_percent) = if min_percent <= max_percent {
        (min_percent, max_percent)
    } else {
        (max_percent, min_percent)
    };
    rng.random_range(min_percent..=max_percent)
}

fn percentage_to_multiplier(percent: u32) -> f32 {
    percent as f32 / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_disabled() {
        let randomizer =
            EnemySpeedRandomizer::new(&EnemySpeedConfig::default(), Duration::from_millis(500));
        assert!(!randomizer.enabled);
    }

    #[test]
    fn pools_alternate() {
        let mut config = EnemySpeedConfig::default();
        config.pool_1_min_percent = 1;
        config.pool_1_max_percent = 99;
        config.pool_2_min_percent = 150;
        config.pool_2_max_percent = 200;
        let mut randomizer = EnemySpeedRandomizer::new(&config, Duration::from_millis(500));

        assert_eq!(randomizer.next_pool_bounds(), (1, 99));
        assert_eq!(randomizer.next_pool_bounds(), (150, 200));
        assert_eq!(randomizer.next_pool_bounds(), (1, 99));
    }

    #[test]
    fn individual_mode_keeps_a_separate_target_for_each_enemy() {
        let mut channel = SpeedChannelState::new();
        let targets = HashMap::from([(1, 50), (2, 150)]);
        channel.set_enemy_targets(targets);

        assert_eq!(channel.enemy_targets.get(&1), Some(&50));
        assert_eq!(channel.enemy_targets.get(&2), Some(&150));
        assert_ne!(channel.enemy_targets.get(&1), channel.enemy_targets.get(&2));
    }

    #[test]
    fn individual_mode_is_reported_without_a_single_multiplier() {
        let mut config = EnemySpeedConfig::default();
        config.randomize_each_enemy = true;
        let randomizer = EnemySpeedRandomizer::new(&config, Duration::from_millis(500));

        let status = randomizer.status();
        assert!(status.individual_enemy_speed);
        assert_eq!(status.multiplier, 1.0);
    }

    #[test]
    fn only_regular_npcs_are_targets() {
        assert!(is_enemy(ChrType::Npc));
        assert!(!is_enemy(ChrType::Local));
        assert!(!is_enemy(ChrType::WhiteSummonNpc));
        assert!(!is_enemy(ChrType::BloodyFingerNpc));
        assert!(!is_enemy(ChrType::RecusantNpc));
    }

    #[test]
    fn percentage_uses_integer_hundredths() {
        assert_eq!(percentage_to_multiplier(50), 0.5);
        assert_eq!(percentage_to_multiplier(150), 1.5);
    }
}
