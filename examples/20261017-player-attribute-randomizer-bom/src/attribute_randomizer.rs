use std::time::{Duration, Instant};

use eldenring::{
    cs::{ChrInsExt, WorldChrMan},
    util::input,
};
use fromsoftware_shared::FromStatic;
use rand::Rng;
use windows::Win32::{
    System::Diagnostics::Debug::MessageBeep,
    UI::WindowsAndMessaging::{MB_ICONHAND, MB_ICONINFORMATION},
};

use crate::config::AttributeConfig;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DamageAttribute {
    Physical,
    Magic,
    Fire,
    Lightning,
    Holy,
}

impl DamageAttribute {
    pub const ALL: [Self; 5] = [
        Self::Physical,
        Self::Magic,
        Self::Fire,
        Self::Lightning,
        Self::Holy,
    ];

    pub const fn effect_id(self) -> i32 {
        match self {
            Self::Physical => 320010,
            Self::Magic => 320011,
            Self::Fire => 320012,
            Self::Lightning => 320013,
            Self::Holy => 320014,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Physical => "物理",
            Self::Magic => "魔力",
            Self::Fire => "火",
            Self::Lightning => "雷",
            Self::Holy => "圣",
        }
    }

    pub const fn color(self) -> [f32; 4] {
        match self {
            Self::Physical => rgb(180, 140, 255),
            Self::Magic => rgb(90, 130, 255),
            Self::Fire => rgb(255, 40, 40),
            Self::Lightning => rgb(230, 140, 0),
            Self::Holy => rgb(150, 160, 200),
        }
    }
}

const fn rgb(red: u8, green: u8, blue: u8) -> [f32; 4] {
    [
        red as f32 / 255.0,
        green as f32 / 255.0,
        blue as f32 / 255.0,
        1.0,
    ]
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AttributeStatus {
    pub enabled: bool,
    pub attribute: Option<DamageAttribute>,
    pub countdown_ms: Option<u64>,
}

pub struct PlayerAttributeRandomizer {
    config: AttributeConfig,
    enabled: bool,
    toggle_was_pressed: bool,
    last_input_check: Instant,
    current_attribute: Option<DamageAttribute>,
    next_randomize_at: Option<Instant>,
}

impl PlayerAttributeRandomizer {
    pub fn new(config: &AttributeConfig, input_check_interval: Duration) -> Self {
        Self {
            config: config.clone(),
            enabled: false,
            toggle_was_pressed: false,
            last_input_check: Instant::now() - input_check_interval,
            current_attribute: None,
            next_randomize_at: None,
        }
    }

    pub fn tick(&mut self, input_check_interval: Duration) {
        self.update_toggle_state(input_check_interval);

        if !(self.enabled && self.config.enable) {
            self.disable();
            return;
        }

        if self
            .next_randomize_at
            .is_none_or(|deadline| Instant::now() >= deadline)
        {
            self.randomize_attribute();
        }
    }

    pub fn status(&self) -> AttributeStatus {
        let enabled = self.enabled && self.config.enable;
        AttributeStatus {
            enabled,
            attribute: enabled.then_some(self.current_attribute).flatten(),
            countdown_ms: enabled.then(|| self.countdown_ms()),
        }
    }

    pub fn disable(&mut self) {
        if self.current_attribute.is_some() {
            remove_all_player_attributes();
        }
        self.enabled = false;
        self.current_attribute = None;
        self.next_randomize_at = None;
    }

    pub fn update_config(&mut self, config: &AttributeConfig) {
        if self.config == *config {
            return;
        }
        self.config = config.clone();
        self.toggle_was_pressed = false;
        if !self.config.enable {
            self.disable();
        }
    }

    fn update_toggle_state(&mut self, input_check_interval: Duration) {
        if self.last_input_check.elapsed() < input_check_interval {
            return;
        }
        self.last_input_check = Instant::now();

        let pressed = input::is_key_pressed(self.config.toggle_virtual_key);
        if pressed && !self.toggle_was_pressed {
            if self.enabled {
                self.disable();
            } else if self.config.enable {
                self.enabled = true;
                self.current_attribute = None;
                self.next_randomize_at = Some(Instant::now());
                beep_toggle(true);
            }
            if !self.enabled {
                beep_toggle(false);
            }
        }
        self.toggle_was_pressed = pressed;
    }

    fn randomize_attribute(&mut self) {
        let mut rng = rand::rng();
        let attribute = random_attribute(self.current_attribute, &mut rng);
        if !apply_player_attribute(attribute) {
            return;
        }

        self.current_attribute = Some(attribute);
        self.next_randomize_at = Some(Instant::now() + random_interval(&self.config, &mut rng));
    }

    fn countdown_ms(&self) -> u64 {
        self.next_randomize_at
            .map(|deadline| {
                deadline
                    .saturating_duration_since(Instant::now())
                    .as_millis() as u64
            })
            .unwrap_or_default()
    }
}

fn apply_player_attribute(attribute: DamageAttribute) -> bool {
    let Ok(world_chr_man) = (unsafe { WorldChrMan::instance_mut() }) else {
        return false;
    };
    let Some(main_player) = world_chr_man.main_player.as_mut() else {
        return false;
    };

    for other in DamageAttribute::ALL {
        main_player.chr_ins.remove_speffect(other.effect_id());
    }
    main_player.apply_speffect(attribute.effect_id(), true);
    true
}

fn remove_all_player_attributes() {
    let Ok(world_chr_man) = (unsafe { WorldChrMan::instance_mut() }) else {
        return;
    };
    let Some(main_player) = world_chr_man.main_player.as_mut() else {
        return;
    };

    for attribute in DamageAttribute::ALL {
        main_player.chr_ins.remove_speffect(attribute.effect_id());
    }
}

fn random_attribute(current: Option<DamageAttribute>, rng: &mut impl Rng) -> DamageAttribute {
    let candidates: Vec<_> = DamageAttribute::ALL
        .into_iter()
        .filter(|attribute| Some(*attribute) != current)
        .collect();
    candidates[rng.random_range(0..candidates.len())]
}

fn random_interval(config: &AttributeConfig, rng: &mut impl Rng) -> Duration {
    let (min_ms, max_ms) = if config.randomize_interval_min_ms <= config.randomize_interval_max_ms {
        (
            config.randomize_interval_min_ms,
            config.randomize_interval_max_ms,
        )
    } else {
        (
            config.randomize_interval_max_ms,
            config.randomize_interval_min_ms,
        )
    };
    Duration::from_millis(rng.random_range(min_ms.max(1)..=max_ms.max(1)))
}

fn beep_toggle(enabled: bool) {
    let sound = if enabled {
        MB_ICONINFORMATION
    } else {
        MB_ICONHAND
    };
    let _ = unsafe { MessageBeep(sound) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_disabled() {
        let randomizer =
            PlayerAttributeRandomizer::new(&AttributeConfig::default(), Duration::from_millis(100));
        assert_eq!(randomizer.status(), AttributeStatus::default());
    }

    #[test]
    fn random_attribute_excludes_current_result() {
        let mut rng = rand::rng();
        for _ in 0..100 {
            assert_ne!(
                random_attribute(Some(DamageAttribute::Fire), &mut rng),
                DamageAttribute::Fire
            );
        }
    }

    #[test]
    fn fixed_interval_stays_at_the_configured_value() {
        let mut config = AttributeConfig::default();
        config.randomize_interval_min_ms = 1_234;
        config.randomize_interval_max_ms = 1_234;
        let mut rng = rand::rng();

        assert_eq!(
            random_interval(&config, &mut rng),
            Duration::from_millis(1_234)
        );
    }

    #[test]
    fn attributes_map_to_the_requested_speffects() {
        assert_eq!(DamageAttribute::Physical.effect_id(), 320010);
        assert_eq!(DamageAttribute::Magic.effect_id(), 320011);
        assert_eq!(DamageAttribute::Fire.effect_id(), 320012);
        assert_eq!(DamageAttribute::Lightning.effect_id(), 320013);
        assert_eq!(DamageAttribute::Holy.effect_id(), 320014);
    }
}
