use std::{fs, path::PathBuf};

use eldenring::cs::WorldChrMan;
use fromsoftware_shared::FromStatic;

use crate::config::ResizerStaminaConfig;

const MAX_OUTPUT_SCALE_PERCENT: f32 = 10_000.0;

pub struct ResizerStaminaController {
    config: ResizerStaminaConfig,
    resizer_config_path: PathBuf,
    frames_until_update: u32,
    last_written_scale_percent: Option<u32>,
    status: ResizerStaminaStatus,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResizerStaminaStatus {
    pub player_available: bool,
    pub current_stamina: u32,
    pub max_stamina: u32,
    pub stamina_percent: f32,
    pub base_stamina: u32,
    pub capacity_factor: f32,
    pub base_scale_percent: f32,
    pub calculated_scale_percent: f32,
    pub written_scale_percent: u32,
    pub update_interval_frames: u32,
    pub resizer_config_found: bool,
    pub last_write_succeeded: bool,
}

impl ResizerStaminaStatus {
    fn unavailable(config: &ResizerStaminaConfig, resizer_config_found: bool) -> Self {
        Self {
            player_available: false,
            current_stamina: 0,
            max_stamina: 0,
            stamina_percent: 0.0,
            base_stamina: config.base_stamina,
            capacity_factor: 0.0,
            base_scale_percent: 0.0,
            calculated_scale_percent: 0.0,
            written_scale_percent: 0,
            update_interval_frames: config.update_interval_frames,
            resizer_config_found,
            last_write_succeeded: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ScaleCalculation {
    current_stamina: u32,
    max_stamina: u32,
    stamina_percent: f32,
    capacity_factor: f32,
    base_scale_percent: f32,
    calculated_scale_percent: f32,
    written_scale_percent: u32,
}

impl ResizerStaminaController {
    pub fn new(config: &ResizerStaminaConfig, resizer_config_path: PathBuf) -> Self {
        Self {
            config: config.clone(),
            status: ResizerStaminaStatus::unavailable(config, resizer_config_path.exists()),
            resizer_config_path,
            frames_until_update: 0,
            last_written_scale_percent: None,
        }
    }

    pub fn tick(&mut self) {
        if self.frames_until_update > 0 {
            self.frames_until_update -= 1;
            return;
        }
        self.frames_until_update = self.config.update_interval_frames.saturating_sub(1);
        self.update_from_player_stamina();
    }

    pub fn update_config(&mut self, config: &ResizerStaminaConfig) {
        if self.config == *config {
            return;
        }

        self.config = config.clone();
        self.frames_until_update = 0;
        self.last_written_scale_percent = None;
    }

    pub fn status(&self) -> ResizerStaminaStatus {
        self.status
    }

    fn update_from_player_stamina(&mut self) {
        let resizer_config_found = self.resizer_config_path.exists();
        let Some((current_stamina, max_stamina)) = read_player_stamina() else {
            self.status = ResizerStaminaStatus::unavailable(&self.config, resizer_config_found);
            return;
        };

        let calculation = calculate_scale(current_stamina, max_stamina, &self.config);
        let mut last_write_succeeded = false;

        if resizer_config_found {
            if self.last_written_scale_percent == Some(calculation.written_scale_percent) {
                last_write_succeeded = true;
            } else if rewrite_player_scale(
                &self.resizer_config_path,
                calculation.written_scale_percent,
            )
            .is_ok()
            {
                self.last_written_scale_percent = Some(calculation.written_scale_percent);
                last_write_succeeded = true;
            }
        }

        self.status = ResizerStaminaStatus {
            player_available: true,
            current_stamina: calculation.current_stamina,
            max_stamina: calculation.max_stamina,
            stamina_percent: calculation.stamina_percent,
            base_stamina: self.config.base_stamina,
            capacity_factor: calculation.capacity_factor,
            base_scale_percent: calculation.base_scale_percent,
            calculated_scale_percent: calculation.calculated_scale_percent,
            written_scale_percent: calculation.written_scale_percent,
            update_interval_frames: self.config.update_interval_frames,
            resizer_config_found,
            last_write_succeeded,
        };
    }
}

fn read_player_stamina() -> Option<(u32, u32)> {
    let world_chr_man = unsafe { WorldChrMan::instance() }.ok()?;
    let main_player = world_chr_man.main_player.as_ref()?;
    let data = main_player.chr_ins.modules.data.as_ref();

    let max_stamina = u32::try_from(data.max_stamina).ok()?;
    if max_stamina == 0 {
        return None;
    }

    let current_stamina = u32::try_from(data.stamina.max(0))
        .unwrap_or_default()
        .min(max_stamina);
    Some((current_stamina, max_stamina))
}

fn calculate_scale(
    current_stamina: u32,
    max_stamina: u32,
    config: &ResizerStaminaConfig,
) -> ScaleCalculation {
    let max_stamina = max_stamina.max(1);
    let current_stamina = current_stamina.min(max_stamina);
    let stamina_ratio = current_stamina as f32 / max_stamina as f32;
    let min_scale_percent = config.min_player_scale_percent as f32;
    let max_scale_percent = config.max_player_scale_percent as f32;
    let base_scale_percent =
        min_scale_percent + stamina_ratio * (max_scale_percent - min_scale_percent);
    let capacity_factor = max_stamina as f32 / config.base_stamina.max(1) as f32;
    let calculated_scale_percent =
        (base_scale_percent * capacity_factor).clamp(0.0, MAX_OUTPUT_SCALE_PERCENT);
    let written_scale_percent = calculated_scale_percent.round() as u32;

    ScaleCalculation {
        current_stamina,
        max_stamina,
        stamina_percent: stamina_ratio * 100.0,
        capacity_factor,
        base_scale_percent,
        calculated_scale_percent,
        written_scale_percent,
    }
}

fn rewrite_player_scale(path: &std::path::Path, scale_percent: u32) -> std::io::Result<()> {
    let contents = fs::read_to_string(path)?;
    let newline = if contents.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let had_trailing_newline = contents.ends_with('\n');
    let mut found = false;

    let mut lines = contents
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            let is_player_scale = trimmed
                .split_once('=')
                .map(|(key, _)| key.trim().eq_ignore_ascii_case("playerScale"))
                .unwrap_or(false);

            if is_player_scale {
                found = true;
                let indent_len = line.len() - trimmed.len();
                let indent = &line[..indent_len];
                format!("{indent}playerScale = {scale_percent}%")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>();

    if !found {
        lines.push(format!("playerScale = {scale_percent}%"));
    }

    let mut rewritten = lines.join(newline);
    if had_trailing_newline || !found {
        rewritten.push_str(newline);
    }
    fs::write(path, rewritten)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_example_calculates_232_point_5_percent() {
        let config = ResizerStaminaConfig {
            update_interval_frames: 5,
            min_player_scale_percent: 10,
            max_player_scale_percent: 300,
            base_stamina: 200,
        };

        let result = calculate_scale(150, 300, &config);
        assert!((result.base_scale_percent - 155.0).abs() < 0.001);
        assert!((result.capacity_factor - 1.5).abs() < 0.001);
        assert!((result.calculated_scale_percent - 232.5).abs() < 0.001);
        assert_eq!(result.written_scale_percent, 233);
    }

    #[test]
    fn empty_and_full_stamina_scale_the_entire_configured_range() {
        let config = ResizerStaminaConfig::default();

        let empty = calculate_scale(0, 300, &config);
        let full = calculate_scale(300, 300, &config);

        assert!((empty.calculated_scale_percent - 15.0).abs() < 0.001);
        assert!((full.calculated_scale_percent - 450.0).abs() < 0.001);
    }

    #[test]
    fn current_stamina_is_clamped_to_the_maximum() {
        let result = calculate_scale(999, 200, &ResizerStaminaConfig::default());
        assert_eq!(result.current_stamina, 200);
        assert!((result.stamina_percent - 100.0).abs() < 0.001);
        assert_eq!(result.written_scale_percent, 300);
    }

    #[test]
    fn rewrite_player_scale_replaces_exact_key_and_preserves_crlf() {
        let temp_dir = std::env::temp_dir().join("resizer-stamina-rewrite-replace");
        let path = temp_dir.join("Resizer_config.ini");
        let _ = fs::create_dir_all(&temp_dir);
        fs::write(
            &path,
            "foo = 1\r\nplayerScaleExtra = 80%\r\n  playerScale = 80%\r\n",
        )
        .unwrap();

        rewrite_player_scale(&path, 233).unwrap();

        let rewritten = fs::read_to_string(&path).unwrap();
        assert!(rewritten.contains("playerScaleExtra = 80%\r\n"));
        assert!(rewritten.contains("  playerScale = 233%\r\n"));
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&temp_dir);
    }

    #[test]
    fn rewrite_player_scale_appends_when_missing() {
        let temp_dir = std::env::temp_dir().join("resizer-stamina-rewrite-append");
        let path = temp_dir.join("Resizer_config.ini");
        let _ = fs::create_dir_all(&temp_dir);
        fs::write(&path, "foo = 1\n").unwrap();

        rewrite_player_scale(&path, 90).unwrap();

        let rewritten = fs::read_to_string(&path).unwrap();
        assert_eq!(rewritten, "foo = 1\nplayerScale = 90%\n");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&temp_dir);
    }
}
