mod attribute_randomizer;
mod config;

use eldenring::{
    cs::{CSTaskGroupIndex, CSTaskImp},
    fd4::FD4TaskData,
    util::system::wait_for_system_init,
};
use fromsoftware_shared::{FromStatic, program::Program, task::*};
use hudhook::hooks::dx12::ImguiDx12Hooks;
use hudhook::imgui::{
    Condition, Context, FontConfig, FontGlyphRanges, FontId, FontSource, StyleColor, StyleVar, Ui,
};
use hudhook::{Hudhook, ImguiRenderLoop, RenderContext, eject, windows};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime},
};

use crate::{
    attribute_randomizer::{AttributeStatus, DamageAttribute, PlayerAttributeRandomizer},
    config::{
        OverlayConfig, PlayerAttributeRandomizerBomConfig, config_modified_time, load_config,
        load_or_create_config, resolve_paths,
    },
};

const CONFIG_RELOAD_INTERVAL: Duration = Duration::from_secs(1);
// Keep the same input polling path and cadence as the working speed randomizer.
const INPUT_CHECK_INTERVAL: Duration = Duration::from_millis(500);
const HUD_STARTUP_DELAY: Duration = Duration::from_secs(10);

#[unsafe(no_mangle)]
/// # Safety
/// This entry point is called by Windows LoadLibrary. Do not call it directly.
pub unsafe extern "C" fn DllMain(hmodule: usize, reason: u32) -> bool {
    if reason == 0 {
        return true;
    }
    if reason != 1 {
        return true;
    }

    std::thread::spawn(move || {
        let paths = resolve_paths(hmodule);
        let config = load_or_create_config(&paths.config_path);
        let shared = Arc::new(Mutex::new(OverlayShared {
            config: config.overlay.clone(),
            status: AttributeStatus::default(),
        }));

        if wait_for_system_init(&Program::current(), Duration::MAX).is_err() {
            return;
        }

        let Ok(cs_task) = (unsafe { CSTaskImp::instance() }) else {
            return;
        };

        let mut state = State::new(config, paths.config_path, Arc::clone(&shared));
        state.publish_status();
        cs_task.run_recurring(
            move |_: &FD4TaskData| {
                let _ = catch_unwind(AssertUnwindSafe(|| state.tick()));
            },
            CSTaskGroupIndex::ChrIns_PostPhysics,
        );

        std::thread::sleep(HUD_STARTUP_DELAY);
        let hook_result = Hudhook::builder()
            .with::<ImguiDx12Hooks>(PlayerAttributeOverlay {
                shared,
                countdown_font: None,
                attribute_font: None,
                small_font: None,
            })
            .with_hmodule(windows::Win32::Foundation::HINSTANCE(hmodule as _))
            .build()
            .apply();
        if hook_result.is_err() {
            eject();
        }
    });

    true
}

struct OverlayShared {
    config: OverlayConfig,
    status: AttributeStatus,
}

struct PlayerAttributeOverlay {
    shared: Arc<Mutex<OverlayShared>>,
    countdown_font: Option<OverlayFontId>,
    attribute_font: Option<OverlayFontId>,
    small_font: Option<OverlayFontId>,
}

#[derive(Clone, Copy)]
struct OverlayFontId(FontId);

// FontId is an opaque pointer owned by the single ImGui context used by this
// render loop. Hudhook requires the render loop object itself to be Send+Sync,
// so only this context-local handle is wrapped.
unsafe impl Send for OverlayFontId {}
unsafe impl Sync for OverlayFontId {}

impl ImguiRenderLoop for PlayerAttributeOverlay {
    fn initialize(&mut self, ctx: &mut Context, _render: &mut dyn RenderContext) {
        ctx.style_mut().use_dark_colors();

        // Rasterize each displayed size directly. Scaling one small font up and
        // down with SetWindowFontScale makes Chinese glyphs look soft.
        if let Ok(font_data) = std::fs::read(r"C:\Windows\Fonts\simfang.ttf") {
            ctx.fonts().clear();
            let font_config = || FontConfig {
                glyph_ranges: FontGlyphRanges::chinese_simplified_common(),
                ..FontConfig::default()
            };
            self.countdown_font = Some(OverlayFontId(ctx.fonts().add_font(&[
                FontSource::TtfData {
                    data: &font_data,
                    size_pixels: 32.0,
                    config: Some(font_config()),
                },
            ])));
            self.attribute_font = Some(OverlayFontId(ctx.fonts().add_font(&[
                FontSource::TtfData {
                    data: &font_data,
                    size_pixels: 48.0,
                    config: Some(font_config()),
                },
            ])));
            self.small_font = Some(OverlayFontId(ctx.fonts().add_font(&[
                FontSource::TtfData {
                    data: &font_data,
                    size_pixels: 22.0,
                    config: Some(font_config()),
                },
            ])));
        }
    }

    fn render(&mut self, ui: &mut Ui) {
        let (config, status) = match self.shared.lock() {
            Ok(shared) => (shared.config.clone(), shared.status),
            Err(poisoned) => {
                let shared = poisoned.into_inner();
                (shared.config.clone(), shared.status)
            }
        };
        draw_overlay(
            ui,
            &config,
            status,
            self.countdown_font,
            self.attribute_font,
            self.small_font,
        );
    }
}

fn draw_overlay(
    ui: &Ui,
    config: &OverlayConfig,
    status: AttributeStatus,
    countdown_font: Option<OverlayFontId>,
    attribute_font: Option<OverlayFontId>,
    small_font: Option<OverlayFontId>,
) {
    const ATTRIBUTE_SLOT_WIDTH: f32 = 96.0;
    const SMALL_ATTRIBUTE_WIDTH: f32 = 22.0;
    const COUNTDOWN_TEXT_WIDTH: f32 = 130.0;

    unsafe {
        (*hudhook::imgui::sys::igGetIO()).DisplayFramebufferScale =
            hudhook::imgui::sys::ImVec2 { x: 1.0, y: 1.0 };
    }

    let screen = ui.io().display_size;
    if screen[0] <= 0.0 || screen[1] <= 0.0 {
        return;
    }

    let scale = config.scale.clamp(0.25, 4.0);
    let padding_x = 12.0 * scale;
    let countdown_gap = config.countdown_attribute_gap.max(0.0) * scale;
    let attribute_text_gap = config.attribute_text_gap.max(0.0) * scale;
    let left_width = if config.show_countdown {
        COUNTDOWN_TEXT_WIDTH * scale + countdown_gap
    } else {
        0.0
    };
    let right_width =
        ATTRIBUTE_SLOT_WIDTH * scale + attribute_text_gap + SMALL_ATTRIBUTE_WIDTH * scale;
    let window_width = left_width + right_width + padding_x * 2.0;
    let window_height = config.height.clamp(48.0, 400.0) * scale;
    let window_padding = ui.push_style_var(StyleVar::WindowPadding([12.0 * scale, 10.0 * scale]));
    let bg = ui.push_style_color(StyleColor::WindowBg, [0.0, 0.0, 0.0, 0.72]);
    let border = ui.push_style_var(StyleVar::WindowBorderSize(1.0 * scale));
    let rounding = ui.push_style_var(StyleVar::WindowRounding(8.0 * scale));

    ui.window("##player_attribute_randomizer_overlay")
        .position(
            [config.offset_x.max(0.0), config.offset_y.max(0.0)],
            Condition::Always,
        )
        .size([window_width, window_height], Condition::Always)
        .no_decoration()
        .no_inputs()
        .movable(false)
        .resizable(false)
        .focus_on_appearing(false)
        .bring_to_front_on_focus(false)
        .build(|| {
            ui.set_window_font_scale(scale);
            if config.show_countdown {
                ui.columns(2, "##player_attribute_columns", false);
                ui.set_column_width(0, left_width);

                let countdown = format_countdown(status.countdown_ms);
                let countdown_font_token = countdown_font.map(|font| ui.push_font(font.0));
                let countdown_size = ui.calc_text_size(&countdown);
                let cursor = ui.cursor_pos();
                let content_height = ui.content_region_avail()[1];
                ui.set_cursor_pos([
                    cursor[0],
                    cursor[1] + ((content_height - countdown_size[1]) * 0.5).max(0.0),
                ]);
                ui.text_colored([1.0, 1.0, 1.0, 1.0], countdown);
                drop(countdown_font_token);

                ui.next_column();
                draw_attribute_panel(ui, status, attribute_font, small_font);
                ui.columns(1, "##player_attribute_columns_end", false);
            } else {
                draw_attribute_panel(ui, status, attribute_font, small_font);
            }

            ui.set_window_font_scale(1.0);
        });

    rounding.end();
    border.end();
    bg.end();
    window_padding.end();
}

fn draw_attribute_panel(
    ui: &Ui,
    status: AttributeStatus,
    attribute_font: Option<OverlayFontId>,
    small_font: Option<OverlayFontId>,
) {
    let label = status.attribute.map(DamageAttribute::label).unwrap_or("--");
    let color = status
        .attribute
        .map(DamageAttribute::color)
        .unwrap_or([1.0, 1.0, 1.0, 1.0]);

    let right_origin = ui.cursor_pos();
    let right_width = ui.content_region_avail()[0];
    let right_height = ui.content_region_avail()[1];

    let attribute_font_token = attribute_font.map(|font| ui.push_font(font.0));
    let attribute_size = ui.calc_text_size(label);
    ui.set_cursor_pos([
        right_origin[0],
        right_origin[1] + ((right_height - attribute_size[1]) * 0.5).max(0.0),
    ]);
    ui.text_colored(color, label);
    drop(attribute_font_token);

    let small_font_token = small_font.map(|font| ui.push_font(font.0));
    let small_size = ui.calc_text_size("属");
    let small_line_height = ui.text_line_height();
    let small_x = right_origin[0] + right_width - small_size[0];
    let small_y = right_origin[1] + ((right_height - small_line_height * 2.0) * 0.5).max(0.0);
    ui.set_cursor_pos([small_x, small_y]);
    ui.text_colored([1.0, 1.0, 1.0, 1.0], "属");
    ui.set_cursor_pos([small_x, small_y + small_line_height]);
    ui.text_colored([1.0, 1.0, 1.0, 1.0], "性");
    drop(small_font_token);
}

fn format_countdown(countdown_ms: Option<u64>) -> String {
    let Some(countdown_ms) = countdown_ms else {
        return "--:--.--".to_string();
    };
    let total_seconds = countdown_ms / 1_000;
    let centiseconds = (countdown_ms % 1_000) / 10;
    format!(
        "{:02}:{:02}.{:02}",
        total_seconds / 60,
        total_seconds % 60,
        centiseconds
    )
}

#[cfg(test)]
mod tests {
    use super::format_countdown;

    #[test]
    fn countdown_is_formatted_to_centiseconds() {
        assert_eq!(format_countdown(Some(3_890)), "00:03.89");
        assert_eq!(format_countdown(Some(65_430)), "01:05.43");
        assert_eq!(format_countdown(None), "--:--.--");
    }
}

struct State {
    config_path: PathBuf,
    config_last_modified: Option<SystemTime>,
    last_config_check: Instant,
    randomizer: PlayerAttributeRandomizer,
    overlay: Arc<Mutex<OverlayShared>>,
}

impl State {
    fn new(
        config: PlayerAttributeRandomizerBomConfig,
        config_path: PathBuf,
        overlay: Arc<Mutex<OverlayShared>>,
    ) -> Self {
        Self {
            config_last_modified: config_modified_time(&config_path),
            config_path,
            last_config_check: Instant::now(),
            randomizer: PlayerAttributeRandomizer::new(&config.attribute, INPUT_CHECK_INTERVAL),
            overlay,
        }
    }

    fn tick(&mut self) {
        self.reload_config_if_changed();
        self.randomizer.tick(INPUT_CHECK_INTERVAL);
        self.publish_status();
    }

    fn publish_status(&self) {
        let status = self.randomizer.status();
        if let Ok(mut shared) = self.overlay.lock() {
            shared.status = status;
        }
    }

    fn reload_config_if_changed(&mut self) {
        if self.last_config_check.elapsed() < CONFIG_RELOAD_INTERVAL {
            return;
        }
        self.last_config_check = Instant::now();
        let modified = config_modified_time(&self.config_path);
        if modified == self.config_last_modified {
            return;
        }

        let Some(config) = (if modified.is_none() {
            Some(load_or_create_config(&self.config_path))
        } else {
            load_config(&self.config_path)
        }) else {
            return;
        };
        self.randomizer.update_config(&config.attribute);
        if let Ok(mut shared) = self.overlay.lock() {
            shared.config = config.overlay;
        }
        self.config_last_modified = config_modified_time(&self.config_path);
    }
}
