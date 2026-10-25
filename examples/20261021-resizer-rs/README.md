# resizer-rs

Rust prototype based on the reversed `EldenResizer` DLL.

## Current behavior

- Reads `Resizer_config.ini` next to the built DLL.
- Hot-reloads the config every 500 ms.
- Selects player, Torrent, Mimic Tear, and other spirit summons by `FieldInsHandle` / `NpcParam`.
- Applies `ChrCtrl::scale_size_x/y/z`.
- Scales all live Havok bone lengths and root bone thickness.
- Keeps native cloth enabled by default; `disableClothPhysics = true` restores the original Resizer workaround.
- Scales `LockCamParam::cam_dist_target` and `chr_org_offset_y`.
- Hooks `CS::CSChrTaeAnimEvent` slot 2 and handles TAE event 152 (`CameraZoomOut`).
- Hooks `CS::CSFD4LocationChrAsmDmypolyIdModifier` slot 5 and scales the ten-float dmypoly transform output.
- Scales `AtkParam_Pc` `hit0..hit3_radius` for direct weapon hitboxes.

## Cloth behavior

When `disableClothPhysics = false`, native Havok cloth remains active while the full character
skeleton, including garment-driving bones, is scaled by the static Resizer multiplier.

This is a partial cloth solution rather than a cloth-solver rewrite:

- garment bones follow the resized skeleton;
- native cloth constraints and secondary motion remain enabled;
- cloth particle radii, collision thickness, and solver parameters are not independently scaled;
- extreme scales may still require `disableClothPhysics = true`.

This follows the useful part of the `DynamicScaleLength` DLL without its sinusoidal 130 BPM motion.

## Not implemented yet

- Root-motion scaling through `CS::CSBehCharaProxyDriver`.
- `noFallDamage` SpEffect application.
- Ragdoll scaling.
- Projectile hitbox scaling.

## Build

```powershell
cargo build -p resizer-rs --release
```

Place `resizer_rs.dll` and `Resizer_config.ini` in the game's mod directory and load the DLL through
Mod Engine 2 / Elden Mod Loader.

If a newer game patch changes the Havok layout, set `scaleBones = false` first. Debug `ChrCtrl`
scaling, camera, and weapon hooks will still run. Check `resizer_rs_log.txt` next to the DLL for hook
installation and config-reload messages.

## Config

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
disableClothPhysics = false
enableCharacterScaling = true
enableParamScaling = true
enableCameraHook = true
enableWeaponHook = true
scaleBones = true
```

`player`, `auto`, percentages, and plain numbers are accepted.

## Crash diagnostics

`resizer_rs_log.txt` is written next to the DLL. It records:

- config path and parsed config;
- system initialization and `CSTaskImp` acquisition;
- RTTI class name, VMT RVA, slot entry, original function and detour addresses;
- parameter scaling row counts;
- first N character addresses, handles, NpcParam IDs, `ChrCtrl` and scale-field validation;
- the exact Havok skeleton chain stage that failed, if any.

A ready-to-use safe config is included as `Resizer_config_safe.ini`. Copy it over `Resizer_config.ini` for a no-op startup test, then enable one feature at a time.

To isolate a crash manually, temporarily set the relevant switches to `false` and restart the game:

```ini
enableCharacterScaling = true
enableParamScaling = true
enableCameraHook = true
enableWeaponHook = true
scaleBones = true
disableClothPhysics = false
```

Recommended bisect order:

1. Disable `enableCameraHook` and `enableWeaponHook`.
2. Disable `enableParamScaling`.
3. Disable `scaleBones`.
4. Disable `enableCharacterScaling`.

Hook switches are read during DLL startup and require a restart. Character, parameter, bone and cloth settings are hot-reloadable.
