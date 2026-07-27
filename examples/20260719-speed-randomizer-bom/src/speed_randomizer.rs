use std::{
    ptr,
    time::{Duration, Instant},
};

use eldenring::{
    cs::{CSChrBehaviorModule, CSFlipper, WorldChrMan},
    util::input,
};
use fromsoftware_shared::FromStatic;
use rand::Rng;
use windows::Win32::{
    System::Diagnostics::Debug::MessageBeep,
    UI::WindowsAndMessaging::{MB_ICONHAND, MB_ICONINFORMATION},
};

use crate::config::SpeedRandomizerConfig;

pub struct SpeedRandomizer {
    config: SpeedRandomizerConfig,
    enabled: bool,
    toggle_was_pressed: bool,
    last_input_check: Instant,
    global: SpeedChannelState,
    player: SpeedChannelState,
    player_uses_first_pool: bool,
    global_was_modified: bool,
    modified_player_behavior_address: Option<usize>,
}

#[derive(Clone, Copy, Debug)]
pub struct SpeedStatus {
    pub enabled: bool,
    pub global_enabled: bool,
    pub global_multiplier: f32,
    pub global_countdown_ms: Option<u64>,
    pub player_enabled: bool,
    pub player_multiplier: f32,
    pub player_countdown_ms: Option<u64>,
}

struct SpeedChannelState {
    target_percent: Option<u32>,
    last_randomized: Instant,
}

impl SpeedRandomizer {
    pub fn new(config: &SpeedRandomizerConfig, input_check_interval: Duration) -> Self {
        Self {
            config: config.clone(),
            // 与武器随机一致：DLL 加载后默认关闭，由热键手动开启。
            enabled: false,
            toggle_was_pressed: false,
            last_input_check: Instant::now() - input_check_interval,
            global: SpeedChannelState::new(),
            player: SpeedChannelState::new(),
            player_uses_first_pool: true,
            global_was_modified: false,
            modified_player_behavior_address: None,
        }
    }

    pub fn tick(&mut self, input_check_interval: Duration) {
        self.update_toggle_state(input_check_interval);

        if self.enabled {
            self.randomize_targets_if_due();
        }

        if self.enabled && self.config.enable_global_speed {
            self.apply_global_speed();
        } else {
            self.restore_global_speed();
        }

        if self.enabled && self.config.enable_player_speed {
            self.apply_player_speed();
        } else {
            self.restore_player_speed();
        }
    }

    pub fn status(&self) -> SpeedStatus {
        let global_enabled = self.enabled && self.config.enable_global_speed;
        let player_enabled = self.enabled && self.config.enable_player_speed;

        SpeedStatus {
            enabled: self.enabled,
            global_enabled,
            global_multiplier: if global_enabled {
                self.global.current_multiplier()
            } else {
                1.0
            },
            global_countdown_ms: global_enabled.then(|| {
                self.global
                    .countdown_ms(self.config.global_speed_randomize_interval_ms)
            }),
            player_enabled,
            player_multiplier: if player_enabled {
                self.player.current_multiplier()
            } else {
                1.0
            },
            player_countdown_ms: player_enabled.then(|| {
                self.player
                    .countdown_ms(self.config.player_speed_randomize_interval_ms)
            }),
        }
    }

    pub fn update_config(&mut self, config: &SpeedRandomizerConfig) {
        if self.config == *config {
            return;
        }

        self.config = config.clone();
        self.toggle_was_pressed = false;
        self.global.target_percent = None;
        self.player.target_percent = None;
        self.player_uses_first_pool = true;
    }

    fn update_toggle_state(&mut self, input_check_interval: Duration) {
        if self.last_input_check.elapsed() < input_check_interval {
            return;
        }
        self.last_input_check = Instant::now();

        let pressed = input::is_key_pressed(self.config.toggle_virtual_key);
        if pressed && !self.toggle_was_pressed {
            self.enabled = !self.enabled;
            self.global.target_percent = None;
            self.player.target_percent = None;
            self.player_uses_first_pool = true;
            beep_toggle(self.enabled);
        }
        self.toggle_was_pressed = pressed;
    }

    fn randomize_targets_if_due(&mut self) {
        let mut rng = rand::rng();

        if self.config.enable_global_speed
            && self
                .global
                .should_randomize(self.config.global_speed_randomize_interval_ms)
        {
            let percent = random_percentage(
                self.config.global_speed_min_percent,
                self.config.global_speed_max_percent,
                &mut rng,
            );
            self.global.set_target(percent);
        }

        if self.config.enable_player_speed
            && self
                .player
                .should_randomize(self.config.player_speed_randomize_interval_ms)
        {
            let (min_percent, max_percent) = self.next_player_pool_bounds();
            let percent = random_percentage(min_percent, max_percent, &mut rng);
            self.player.set_target(percent);
        }
    }

    fn next_player_pool_bounds(&mut self) -> (u32, u32) {
        let bounds = if self.player_uses_first_pool {
            (
                self.config.player_speed_pool_1_min_percent,
                self.config.player_speed_pool_1_max_percent,
            )
        } else {
            (
                self.config.player_speed_pool_2_min_percent,
                self.config.player_speed_pool_2_max_percent,
            )
        };
        self.player_uses_first_pool = !self.player_uses_first_pool;
        bounds
    }

    fn apply_global_speed(&mut self) {
        let Some(percent) = self.global.target_percent else {
            return;
        };
        let Ok(flipper) = (unsafe { CSFlipper::instance_mut() }) else {
            return;
        };

        flipper.game_speed = percentage_to_multiplier(percent);
        self.global_was_modified = true;
    }

    fn restore_global_speed(&mut self) {
        if !self.global_was_modified {
            return;
        }
        let Ok(flipper) = (unsafe { CSFlipper::instance_mut() }) else {
            return;
        };

        flipper.game_speed = 1.0;
        self.global_was_modified = false;
        self.global.target_percent = None;
    }

    fn apply_player_speed(&mut self) {
        let Some(percent) = self.player.target_percent else {
            return;
        };
        let Ok(world_chr_man) = (unsafe { WorldChrMan::instance_mut() }) else {
            return;
        };
        let Some(main_player) = world_chr_man.main_player.as_mut() else {
            return;
        };

        let behavior: &mut CSChrBehaviorModule = main_player.chr_ins.modules.behavior.as_mut();
        let behavior_address = ptr::from_mut(behavior) as usize;
        behavior.animation_speed = percentage_to_multiplier(percent);
        self.modified_player_behavior_address = Some(behavior_address);
    }

    fn restore_player_speed(&mut self) {
        let Some(modified_address) = self.modified_player_behavior_address else {
            return;
        };
        let Ok(world_chr_man) = (unsafe { WorldChrMan::instance_mut() }) else {
            return;
        };
        let Some(main_player) = world_chr_man.main_player.as_mut() else {
            return;
        };

        let behavior: &mut CSChrBehaviorModule = main_player.chr_ins.modules.behavior.as_mut();
        let behavior_address = ptr::from_mut(behavior) as usize;
        if behavior_address == modified_address {
            behavior.animation_speed = 1.0;
        }

        self.modified_player_behavior_address = None;
        self.player.target_percent = None;
    }
}

fn beep_toggle(enabled: bool) {
    let sound = if enabled {
        MB_ICONINFORMATION
    } else {
        MB_ICONHAND
    };

    let _ = unsafe { MessageBeep(sound) };
}

impl SpeedChannelState {
    fn new() -> Self {
        Self {
            target_percent: None,
            last_randomized: Instant::now(),
        }
    }

    fn should_randomize(&self, interval_ms: u64) -> bool {
        self.target_percent.is_none()
            || self.last_randomized.elapsed() >= Duration::from_millis(interval_ms.max(1))
    }

    fn set_target(&mut self, percent: u32) {
        self.target_percent = Some(percent);
        self.last_randomized = Instant::now();
    }

    fn current_multiplier(&self) -> f32 {
        self.target_percent
            .map(percentage_to_multiplier)
            .unwrap_or(1.0)
    }

    fn countdown_ms(&self, interval_ms: u64) -> u64 {
        let interval_ms = interval_ms.max(1);
        interval_ms.saturating_sub(self.last_randomized.elapsed().as_millis() as u64)
    }
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
    fn speed_randomizer_starts_disabled() {
        let randomizer = SpeedRandomizer::new(
            &SpeedRandomizerConfig::default(),
            Duration::from_millis(500),
        );

        assert!(!randomizer.enabled);
    }

    #[test]
    fn percentage_uses_integer_hundredths() {
        assert_eq!(percentage_to_multiplier(50), 0.5);
        assert_eq!(percentage_to_multiplier(100), 1.0);
        assert_eq!(percentage_to_multiplier(150), 1.5);
    }

    #[test]
    fn player_pools_alternate() {
        let mut config = SpeedRandomizerConfig::default();
        config.player_speed_pool_1_min_percent = 1;
        config.player_speed_pool_1_max_percent = 99;
        config.player_speed_pool_2_min_percent = 150;
        config.player_speed_pool_2_max_percent = 200;
        let mut randomizer = SpeedRandomizer::new(&config, Duration::from_millis(500));

        assert_eq!(randomizer.next_player_pool_bounds(), (1, 99));
        assert_eq!(randomizer.next_player_pool_bounds(), (150, 200));
        assert_eq!(randomizer.next_player_pool_bounds(), (1, 99));
    }

    #[test]
    fn reversed_bounds_are_supported() {
        let mut rng = rand::rng();
        for _ in 0..100 {
            let value = random_percentage(150, 50, &mut rng);
            assert!((50..=150).contains(&value));
        }
    }
}
