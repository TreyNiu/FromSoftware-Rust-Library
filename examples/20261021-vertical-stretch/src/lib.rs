#![allow(non_snake_case)]

use std::{
    collections::HashMap,
    panic::{AssertUnwindSafe, catch_unwind},
    time::{Duration, Instant},
};

use eldenring::{
    cs::{CSTaskGroupIndex, CSTaskImp, WorldChrMan},
    fd4::FD4TaskData,
    util::system::wait_for_system_init,
};
use fromsoftware_shared::{F32ModelMatrix, F32Vector4, FromStatic, program::Program, task::*};
use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState;

const VK_F5: i32 = 0x74;
const BPM: f64 = 130.0;
const BEATS_PER_SECOND: f64 = BPM / 60.0;

const HEIGHT_MIN: f32 = 0.5;
const HEIGHT_AMPLITUDE: f32 = 0.7;
const WIDTH_MAX: f32 = 2.0;
const WIDTH_SWING: f32 = 1.13;

/// The fast prototype uses the game's debug scale fields. It affects much more than the
/// rendered model (collision, root motion, etc.).
///
/// `FinalModelMatrix` is the high-fidelity, visual-only path. It runs at `Draw_Pre`, after the
/// engine has rebuilt `ChrCtrl::model_matrix`. If the effect disappears on a specific ER
/// version, the equivalent model-update function must be hooked instead.
/// const SCALE_MODE: ScaleMode = ScaleMode::DebugScale;
const SCALE_MODE: ScaleMode = ScaleMode::FinalModelMatrix;

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum ScaleMode {
    DebugScale,
    FinalModelMatrix,
}

#[unsafe(no_mangle)]
/// # Safety
///
/// This is exposed this way such that Windows LoadLibrary API can call it. Do not call it
/// yourself.
pub unsafe extern "C" fn DllMain(_hmodule: usize, reason: u32) -> bool {
    if reason != 1 {
        return true;
    }

    std::thread::spawn(|| {
        if wait_for_system_init(&Program::current(), Duration::MAX).is_err() {
            return;
        }

        let Ok(cs_task) = (unsafe { CSTaskImp::instance() }) else {
            return;
        };

        let task_group = match SCALE_MODE {
            ScaleMode::DebugScale => CSTaskGroupIndex::FrameBegin,
            ScaleMode::FinalModelMatrix => CSTaskGroupIndex::Draw_Pre,
        };

        let mut state = VerticalStretchState::default();
        cs_task.run_recurring(
            move |_: &FD4TaskData| {
                let _ = catch_unwind(AssertUnwindSafe(|| state.tick()));
            },
            task_group,
        );
    });

    true
}

#[derive(Default)]
struct VerticalStretchState {
    enabled: bool,
    f5_down: bool,
    started_at: Option<Instant>,
    base_scales: HashMap<usize, BaseScale>,
}

#[derive(Clone, Copy)]
struct BaseScale {
    x: f32,
    y: f32,
    z: f32,
}

impl VerticalStretchState {
    fn tick(&mut self) {
        if self.take_f5_press() {
            self.enabled = !self.enabled;
            self.started_at = self.enabled.then(Instant::now);
        }

        let Some(world_chr_man) = (unsafe { WorldChrMan::instance_mut() }).ok() else {
            return;
        };

        if !self.enabled {
            self.restore_debug_scales(world_chr_man);
            return;
        }

        let (height, width) = wave_scales(self.started_at.unwrap_or_else(Instant::now));

        for chr_set in world_chr_man.chr_sets.iter().flatten() {
            for chr in chr_set.characters() {
                let chr_address = chr as *mut _ as usize;

                match SCALE_MODE {
                    ScaleMode::DebugScale => {
                        let ctrl = chr.chr_ctrl.as_mut();
                        let base = *self.base_scales.entry(chr_address).or_insert(BaseScale {
                            x: ctrl.scale_size_x,
                            y: ctrl.scale_size_y,
                            z: ctrl.scale_size_z,
                        });

                        ctrl.scale_size_x = base.x * width;
                        ctrl.scale_size_y = base.y * height;
                        ctrl.scale_size_z = base.z * width;
                    }
                    ScaleMode::FinalModelMatrix => {
                        let ctrl = chr.chr_ctrl.as_mut();
                        stretch_model_matrix(&mut ctrl.model_matrix, height, width);
                    }
                }
            }
        }
    }

    fn take_f5_press(&mut self) -> bool {
        let down = unsafe { GetKeyState(VK_F5) } < 0;
        let pressed = down && !self.f5_down;
        self.f5_down = down;
        pressed
    }

    fn restore_debug_scales(&mut self, world_chr_man: &mut WorldChrMan) {
        if SCALE_MODE != ScaleMode::DebugScale || self.base_scales.is_empty() {
            return;
        }

        for chr_set in world_chr_man.chr_sets.iter().flatten() {
            for chr in chr_set.characters() {
                let chr_address = chr as *mut _ as usize;
                let Some(base) = self.base_scales.remove(&chr_address) else {
                    continue;
                };

                let ctrl = chr.chr_ctrl.as_mut();
                ctrl.scale_size_x = base.x;
                ctrl.scale_size_y = base.y;
                ctrl.scale_size_z = base.z;
            }
        }
    }
}

fn wave_scales(started_at: Instant) -> (f32, f32) {
    let phase = started_at.elapsed().as_secs_f64() * BEATS_PER_SECOND;
    let oscillator = (std::f64::consts::TAU * phase).sin() as f32;
    scales_for_blend((oscillator + 1.0) * 0.5)
}

fn scales_for_blend(blend: f32) -> (f32, f32) {
    (
        HEIGHT_MIN + HEIGHT_AMPLITUDE * blend,
        WIDTH_MAX - WIDTH_SWING * blend,
    )
}

/// Scales the model matrix's X/Z axes by `width` and its Y axis by `height`.
///
/// `F32ModelMatrix` is row-major and its final row is the translation, so the character stays
/// anchored while only the basis vectors change.
fn stretch_model_matrix(matrix: &mut F32ModelMatrix, height: f32, width: f32) {
    matrix.0 = scale_row(matrix.0, width);
    matrix.1 = scale_row(matrix.1, height);
    matrix.2 = scale_row(matrix.2, width);
}

fn scale_row(row: F32Vector4, scale: f32) -> F32Vector4 {
    F32Vector4(row.0 * scale, row.1 * scale, row.2 * scale, row.3 * scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wave_ranges_are_correct_and_opposed() {
        assert_eq!(scales_for_blend(0.0), (0.5, 2.0));
        assert_eq!(scales_for_blend(0.5), (0.85, 1.435));

        let (height_max, width_min) = scales_for_blend(1.0);
        assert!((height_max - 1.2).abs() < 1e-6);
        assert!((width_min - 0.87).abs() < 1e-6);
    }

    #[test]
    fn matrix_scaling_preserves_translation_row() {
        let mut matrix = F32ModelMatrix::new(
            F32Vector4(1.0, 0.0, 0.0, 0.0),
            F32Vector4(0.0, 1.0, 0.0, 0.0),
            F32Vector4(0.0, 0.0, 1.0, 0.0),
            F32Vector4(10.0, 20.0, 30.0, 1.0),
        );

        stretch_model_matrix(&mut matrix, 1.2, 0.87);

        assert_eq!(matrix.0, F32Vector4(0.87, 0.0, 0.0, 0.0));
        assert_eq!(matrix.1, F32Vector4(0.0, 1.2, 0.0, 0.0));
        assert_eq!(matrix.2, F32Vector4(0.0, 0.0, 0.87, 0.0));
        assert_eq!(matrix.3, F32Vector4(10.0, 20.0, 30.0, 1.0));
    }
}
