use std::time::{Duration, Instant};

use eldenring::{cs::CSFlipper, util::input};
use fromsoftware_shared::FromStatic;
use rand::Rng;
use windows::Win32::{
    System::Diagnostics::Debug::MessageBeep,
    UI::WindowsAndMessaging::{MB_ICONHAND, MB_ICONINFORMATION},
};

use crate::config::GameSpeedConfig;

pub struct GameSpeedRandomizer {
    config: GameSpeedConfig,
    enabled: bool,
    toggle_was_pressed: bool,
    last_input_check: Instant,
    channel: SpeedChannelState,
    speed_was_modified: bool,
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

impl GameSpeedRandomizer {
    pub fn new(config: &GameSpeedConfig, input_check_interval: Duration) -> Self {
        Self {
            config: config.clone(),
            enabled: false,
            toggle_was_pressed: false,
            last_input_check: Instant::now() - input_check_interval,
            channel: SpeedChannelState::new(),
            speed_was_modified: false,
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
            let percent = random_percentage(
                self.config.min_percent,
                self.config.max_percent,
                &mut rand::rng(),
            );
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

    pub fn update_config(&mut self, config: &GameSpeedConfig) {
        if self.config == *config {
            return;
        }
        self.config = config.clone();
        self.toggle_was_pressed = false;
        self.channel.target_percent = None;
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
            beep_toggle(self.enabled);
        }
        self.toggle_was_pressed = pressed;
    }

    fn apply_speed(&mut self) {
        let Some(percent) = self.channel.target_percent else {
            return;
        };
        let Ok(flipper) = (unsafe { CSFlipper::instance_mut() }) else {
            return;
        };

        flipper.game_speed = percentage_to_multiplier(percent);
        self.speed_was_modified = true;
    }

    fn restore_speed(&mut self) {
        if !self.speed_was_modified {
            return;
        }
        let Ok(flipper) = (unsafe { CSFlipper::instance_mut() }) else {
            return;
        };

        flipper.game_speed = 1.0;
        self.speed_was_modified = false;
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
            GameSpeedRandomizer::new(&GameSpeedConfig::default(), Duration::from_millis(500));
        assert!(!randomizer.enabled);
    }

    #[test]
    fn percentage_uses_integer_hundredths() {
        assert_eq!(percentage_to_multiplier(50), 0.5);
        assert_eq!(percentage_to_multiplier(150), 1.5);
    }

    #[test]
    fn reversed_bounds_are_supported() {
        let mut rng = rand::rng();
        for _ in 0..100 {
            assert!((50..=150).contains(&random_percentage(150, 50, &mut rng)));
        }
    }
}
