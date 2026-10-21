# resizer-rs

Rust prototype based on the reversed `EldenResizer` DLL.

## Current behavior

- Reads `Resizer_config.ini` next to the built DLL.
- Hot-reloads the config every 500 ms.
- Selects player, Torrent, Mimic Tear, and other spirit summons by `FieldInsHandle` / `NpcParam`.
- Applies `ChrCtrl::scale_size_x/y/z` and disables cloth physics for scaled characters.
- Scales live Havok bone lengths and root bone thickness.
- Scales `LockCamParam::cam_dist_target` and `chr_org_offset_y`.
- Hooks `CS::CSChrTaeAnimEvent` slot 2 and handles TAE event 152 (`CameraZoomOut`).
- Hooks `CS::CSFD4LocationChrAsmDmypolyIdModifier` slot 5 and scales the ten-float dmypoly transform output.
- Scales `AtkParam_Pc` `hit0..hit3_radius` for direct weapon hitboxes.

## Not implemented yet

- Root-motion scaling through `CS::CSBehCharaProxyDriver`.
- `noFallDamage` SpEffect application.
- Ragdoll scaling.
- Projectile hitbox scaling.

## Build

```powershell
cargo build -p resizer-rs --release
```

Place `resizer_rs.dll` and `Resizer_config.ini` in the game's mod directory and load the DLL through Mod Engine 2 / Elden Mod Loader.

If a newer game patch changes the Havok layout, set `scaleBones = false` in the config first. Debug `ChrCtrl` scaling, camera, and weapon hooks will still run. Check `resizer_rs_log.txt` next to the DLL for hook installation and config-reload messages.

## Config

The config uses the original Resizer key names:

```ini
[settings]
playerScale = 100%
cameraScale = player
weaponScale = player
torrentScale = player
mimicScale = player
summonScale = player
movementInfluence = 100%
noFallDamage = auto
scaleBones = true
```

`player`, `auto`, percentages, and plain numbers are accepted.
