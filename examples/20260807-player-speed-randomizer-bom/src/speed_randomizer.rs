use std::{
    ptr,
    time::{Duration, Instant},
};

use eldenring::{
    cs::{CSChrBehaviorModule, WorldChrMan},
    util::input,
};
use fromsoftware_shared::FromStatic;
use rand::Rng;
use windows::Win32::{
    System::Diagnostics::Debug::MessageBeep,
    UI::WindowsAndMessaging::{MB_ICONHAND, MB_ICONINFORMATION},
};

use crate::config::PlayerSpeedConfig;

pub struct PlayerSpeedRandomizer {
    config: PlayerSpeedConfig,
    enabled: bool,
    toggle_was_pressed: bool,
    last_input_check: Instant,
    channel: SpeedChannelState,
    uses_first_pool: bool,
    modified_behavior_address: Option<usize>,
}

#[derive(Clone, Copy, Debug)]
pub struct SpeedStatus {
    pub enabled: bool,
    pub speed_enabled: bool,
    pub multiplier: f32,
    pub countdown_ms: Option<u64>,
}

struct SpeedChannelState {
    target_percent: Option<u32>,
    last_randomized: Instant,
}

impl PlayerSpeedRandomizer {
    pub fn new(config: &PlayerSpeedConfig, input_check_interval: Duration) -> Self {
        Self {
            config: config.clone(),
            enabled: false,
            toggle_was_pressed: false,
            last_input_check: Instant::now() - input_check_interval,
            channel: SpeedChannelState::new(),
            uses_first_pool: true,
            modified_behavior_address: None,
        }
    }

    pub fn tick(&mut self, input_check_interval: Duration) {
        self.update_toggle_state(input_check_interval);

        if self.enabled
            && self.config.enable
            && self
                .channel
                .should_randomize(self.config.randomize_interval_ms)
        {
            let (min_percent, max_percent) = self.next_pool_bounds();
            let percent = random_percentage(min_percent, max_percent, &mut rand::rng());
            self.channel.set_target(percent);
        }

        if self.enabled && self.config.enable {
            self.apply_speed();
        } else {
            self.restore_speed();
        }
    }

    pub fn status(&self) -> SpeedStatus {
        let speed_enabled = self.enabled && self.config.enable;
        SpeedStatus {
            enabled: self.enabled,
            speed_enabled,
            multiplier: if speed_enabled {
                self.channel.current_multiplier()
            } else {
                1.0
            },
            countdown_ms: speed_enabled
                .then(|| self.channel.countdown_ms(self.config.randomize_interval_ms)),
        }
    }

    pub fn update_config(&mut self, config: &PlayerSpeedConfig) {
        if self.config == *config {
            return;
        }
        self.config = config.clone();
        self.toggle_was_pressed = false;
        self.channel.target_percent = None;
        self.uses_first_pool = true;
    }

    fn update_toggle_state(&mut self, input_check_interval: Duration) {
        if self.last_input_check.elapsed() < input_check_interval {
            return;
        }
        self.last_input_check = Instant::now();

        let pressed = input::is_key_pressed(self.config.toggle_virtual_key);
        if pressed && !self.toggle_was_pressed {
            self.enabled = !self.enabled;
            self.channel.target_percent = None;
            self.uses_first_pool = true;
            beep_toggle(self.enabled);
        }
        self.toggle_was_pressed = pressed;
    }

    fn next_pool_bounds(&mut self) -> (u32, u32) {
        let bounds = if self.uses_first_pool {
            (
                self.config.pool_1_min_percent,
                self.config.pool_1_max_percent,
            )
        } else {
            (
                self.config.pool_2_min_percent,
                self.config.pool_2_max_percent,
            )
        };
        self.uses_first_pool = !self.uses_first_pool;
        bounds
    }

    fn apply_speed(&mut self) {
        let Some(percent) = self.channel.target_percent else {
            return;
        };
        let Ok(world_chr_man) = (unsafe { WorldChrMan::instance_mut() }) else {
            return;
        };
        let Some(main_player) = world_chr_man.main_player.as_mut() else {
            return;
        };

        let behavior: &mut CSChrBehaviorModule = main_player.chr_ins.modules.behavior.as_mut();
        self.modified_behavior_address = Some(ptr::from_mut(behavior) as usize);
        behavior.animation_speed = percentage_to_multiplier(percent);
    }

    fn restore_speed(&mut self) {
        let Some(modified_address) = self.modified_behavior_address else {
            return;
        };
        let Ok(world_chr_man) = (unsafe { WorldChrMan::instance_mut() }) else {
            return;
        };
        let Some(main_player) = world_chr_man.main_player.as_mut() else {
            return;
        };

        let behavior: &mut CSChrBehaviorModule = main_player.chr_ins.modules.behavior.as_mut();
        if ptr::from_mut(behavior) as usize == modified_address {
            behavior.animation_speed = 1.0;
        }
        self.modified_behavior_address = None;
        self.channel.target_percent = None;
    }
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
        interval_ms
            .max(1)
            .saturating_sub(self.last_randomized.elapsed().as_millis() as u64)
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
            PlayerSpeedRandomizer::new(&PlayerSpeedConfig::default(), Duration::from_millis(500));
        assert!(!randomizer.enabled);
    }

    #[test]
    fn pools_alternate() {
        let mut config = PlayerSpeedConfig::default();
        config.pool_1_min_percent = 1;
        config.pool_1_max_percent = 99;
        config.pool_2_min_percent = 150;
        config.pool_2_max_percent = 200;
        let mut randomizer = PlayerSpeedRandomizer::new(&config, Duration::from_millis(500));

        assert_eq!(randomizer.next_pool_bounds(), (1, 99));
        assert_eq!(randomizer.next_pool_bounds(), (150, 200));
        assert_eq!(randomizer.next_pool_bounds(), (1, 99));
    }

    #[test]
    fn percentage_uses_integer_hundredths() {
        assert_eq!(percentage_to_multiplier(50), 0.5);
        assert_eq!(percentage_to_multiplier(150), 1.5);
    }
}
