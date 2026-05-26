use std::time::Duration;

use eldenring::{
    cs::{
        CSHavokMan, CSTaskGroupIndex, CSTaskImp, CSWorldGeomMan, ChrInsExt,
        GeometrySpawnParameters, PlayerIns, WorldChrMan,
    },
    fd4::FD4TaskData,
    position::{BlockPosition, HavokPosition, PositionDelta},
    util::system::wait_for_system_init,
};
use fromsoftware_shared::{FromStatic, program::Program, task::*};
use rand::Rng;

const SP_EFFECT: i32 = 140;
const MAX_SPAWN_ATTEMPTS: usize = 30;
const SPAWN_DISTANCE_MIN: f32 = 1.0;
const SPAWN_DISTANCE_MAX: f32 = 3.0;
const RAYCAST_HEIGHT_UP: f32 = 3.0;
const RAYCAST_HEIGHT_DOWN: f32 = 8.0;
const RAYCAST_FILTER: u32 = 0;

#[unsafe(no_mangle)]
/// # Safety
///
/// This is exposed this way such that windows LoadLibrary API can call it. Do not call this yourself.
pub unsafe extern "C" fn DllMain(_hmodule: usize, reason: u32) -> bool {
    if reason != 1 {
        return true;
    }

    // Kick off new thread.
    std::thread::spawn(|| {
        wait_for_system_init(&Program::current(), Duration::MAX)
            .expect("Could not await system init.");

        let mut had_sp_effect = false;
        let cs_task = unsafe { CSTaskImp::instance().unwrap() };
        cs_task.run_recurring(
            move |_: &FD4TaskData| {
                let Some(player) = unsafe { WorldChrMan::instance() }
                    .ok()
                    .and_then(|w| w.main_player.as_ref())
                else {
                    return;
                };

                let has_sp_effect = player
                    .chr_ins
                    .special_effect
                    .entries()
                    .any(|effect| effect.param_id == SP_EFFECT);

                if !has_sp_effect {
                    had_sp_effect = false;
                    return;
                }

                if had_sp_effect {
                    return;
                }

                let Some(target_position) = random_spawn_position(player, &mut rand::rng()) else {
                    return;
                };

                let Some(block_geom_data) = unsafe { CSWorldGeomMan::instance_mut() }
                    .ok()
                    .and_then(|wgm| wgm.geom_block_data_by_id_mut(&player.chr_ins.block_id()))
                else {
                    return;
                };

                let asset_id = "AEG217_063";
                block_geom_data.spawn_geometry(
                    asset_id,
                    &GeometrySpawnParameters {
                        position: target_position,
                        rot_x: 0.0,
                        rot_y: 0.0,
                        rot_z: 0.0,
                        scale_x: 1.0,
                        scale_y: 1.0,
                        scale_z: 1.0,
                    },
                );

                had_sp_effect = true;
            },
            CSTaskGroupIndex::ChrIns_PostPhysics,
        );
    });

    true
}

fn random_spawn_position(player: &PlayerIns, rng: &mut impl Rng) -> Option<BlockPosition> {
    let physics = &player.chr_ins.modules.physics;
    let forward = glam::Quat::from(physics.orientation).mul_vec3(glam::vec3(0.0, 0.0, -1.0));
    let forward_xz = glam::vec2(forward.x, forward.z).normalize_or_zero();
    if forward_xz == glam::Vec2::ZERO {
        return None;
    }

    let havok_man = unsafe { CSHavokMan::instance() }.ok()?;
    let phys_world = havok_man.phys_world.as_ref();

    (0..MAX_SPAWN_ATTEMPTS).find_map(|_| {
        let angle = rng.random_range(-std::f32::consts::FRAC_PI_2..=std::f32::consts::FRAC_PI_2);
        let distance = rng.random_range(SPAWN_DISTANCE_MIN..=SPAWN_DISTANCE_MAX);
        let (sin, cos) = angle.sin_cos();
        let offset_xz = glam::vec2(
            forward_xz.x * cos - forward_xz.y * sin,
            forward_xz.x * sin + forward_xz.y * cos,
        ) * distance;

        let ray_origin = HavokPosition::from_xyz(
            physics.position.0 + offset_xz.x,
            physics.position.1 + RAYCAST_HEIGHT_UP,
            physics.position.2 + offset_xz.y,
        );
        let ground = phys_world.cast_ray(
            RAYCAST_FILTER,
            &ray_origin,
            PositionDelta(0.0, -(RAYCAST_HEIGHT_UP + RAYCAST_HEIGHT_DOWN), 0.0),
            player,
        )?;

        Some(BlockPosition {
            x: player.block_position.x + offset_xz.x,
            y: player.block_position.y + ground.1 - physics.position.1,
            z: player.block_position.z + offset_xz.y,
            yaw: player.block_position.yaw,
        })
    })
}
