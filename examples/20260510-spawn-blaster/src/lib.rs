use std::time::Duration;

use eldenring::{
    cs::{
        CSTaskGroupIndex, CSTaskImp, CSWorldGeomMan, ChrInsExt, GeometrySpawnParameters,
        WorldChrMan,
    },
    fd4::FD4TaskData,
    position::BlockPosition,
    util::system::wait_for_system_init,
};
use fromsoftware_shared::{FromStatic, program::Program, task::*};
use rand::Rng;

const SP_EFFECT: i32 = 150;

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

                let Some(block_geom_data) = unsafe { CSWorldGeomMan::instance_mut() }
                    .ok()
                    .and_then(|wgm| wgm.geom_block_data_by_id_mut(&player.chr_ins.block_id()))
                else {
                    return;
                };

                let asset_id = "AEG217_063";
                let mut rng = rand::rng();

                let offset_x = rng.random_range(1.0..=2.0);
                let offset_z = rng.random_range(1.0..=2.0);
                let target_position = BlockPosition {
                    x: (player.block_position.x + offset_x),
                    y: (player.block_position.y),
                    z: (player.block_position.z + offset_z),
                    yaw: (player.block_position.yaw),
                };

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
