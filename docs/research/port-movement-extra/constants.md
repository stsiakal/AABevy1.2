# Constants read from the 1.2 `.data` dump and DB

All VAs are in `D:/aa-re/client12/x2game_unpacked_data.dll` at base `0x38FF0000`; floats were
read with the section-aware helper described in `re-constants.md`. Cvar offsets are into the
game config object (`DAT_39aefb98`); the registration function is `FUN_390f7b40` (0x390f7b40).

## 1. Movement / flight cvars

| cvar | game_cfg off | default VA | value |
|---|---|---|---|
| `swim_back_speed_mul` | +0x15c | `0x399a1d94` | 0.8 |
| `swim_side_speed_mul` | +0x160 | `0x399a24cc` | 0.6 |
| `swim_up_speed_mul` | +0x164 | `0x399a87e8` | 0.7 |
| `swim_down_speed_mul` | +0x168 | `0x3999c47c` | 0.5 |
| `swim_buoy_speed` | +0x16c | `0x3999cffc` | 2.0 |
| `swim_jump_speed` | +0x270 | `0x3999f9d0` | 12.0 |
| `swim_jump_permission_range` | +0x274 | `0x3999dd34` | 0.2 |
| `swim_jump_end_depth` | +0x278 | `0x3999dd34` | 0.2 |
| `fly_stance_enable` | +0x280 | — | 1 |
| `pl_fall_start_height` | +0x170 | `0x3999cffc` | 1.0 |
| `pl_fall_start_velocity` | +0x174 | `0x3999cce8` | 10.0 |
| `pl_fallDamage_SpeedSafe` | +0x178 | `0x3999f8f0` | 20.0 |
| `pl_fallDamage_SpeedFatal` | +0x17c | `0x399a8584` | [data] |
| `pl_fallDamage_SpeedBias` | +0x188 | `0x3999c478` | 1.5 |
| `pl_flyingVelocityMultiplier` | +0x1dc | `0x3999f7c0` | 30.0 |
| `gliding_mouse_ad` | +0x34c | `0x3999cf2c` | 8.0 |
| `gliding_mouse_ws` | +0x350 | `0x3999cf2c` | 8.0 |
| `glider_start_with_double_jump` | +0x2ac | — | 0 |
| `glider_hide_at_sheath` | +0x2b0 | — | 0 |
| `camera_dive_angle` | +0x2f8 | — | 30 |
| `camera_dive_enable` | +0x2fc | — | 1 |
| `camera_dive_pitch` | +0x300 | `0x3999f8f0` | 20.0 |
| `keyboard_rotate_speed` | +0x308 | `0x3999c47c` | 0.5 |

## 2. Plain constants (movement/fly/fall)

| value | VA | meaning |
|---|---|---|
| 1.0 | `0x3999c488` | unit |
| 0.0 | `0x3999c484` | zero |
| 1.5 | `0x3999c478` | pseudoSpeed used by `FUN_393ba480` |
| 0.5 | `0x3999c47c` | half / keyboard turn cvar default |
| 0.1 | `0x3999ccf0` | small threshold |
| 0.2 | `0x3999dd34` | swim-jump permission/end, vertical lift scale |
| 0.3 | `0x3999ccd0` | |
| 2.0 | `0x3999cffc` | 2·g / fall damage scale, travel speed |
| 5.0 | `0x3999ccf4` | |
| 8.0 | `0x3999cf2c` | gliding mouse thresholds |
| 10.0 | `0x3999cce8` | fall-start velocity |
| 20.0 | `0x3999f8f0` | fall safe speed |
| 30.0 | `0x3999f7c0` | flying velocity multiplier default |
| 40.0 | `0x3999f668` | `2g` in fall damage (`sqrt(40*drop)`) |
| 12.0 | `0x3999f9d0` | swim jump speed |
| 0.033 | `0x399a3040` | dt cap |
| 0.01 | `0x3999cf90` | movement/ray threshold |
| -0.1 | `0x399a2f24` | glide vertical-input threshold |
| 0.9 | `0x399a3158` | air-control multiplier in swim control |
| 0.48 / 0.235 | `0x399a2998` / `0x399a2994` | smoothing denominator |
| 60 / 42.75 | cfg | camera FOV action/classic |
| 10 / 18 | cfg | camera max distance action/classic |

## 3. Animation/event IDs

* Glider cleanup: `0x11d` (285), `CleanupGlider`; registration block `0x390f2e40`–`0x390f2f08`.
* `CleanupBackpackAnim` `0x11e` (286), `BindHorseStirrup` `0x11f` (287), `SwimDead` `0x120` (288).
* 10.x differs: 285 CleanupGlider, 286 StartGlider, 287 CleanupBackpackAnim — do not copy the
  10.x event table into 1.2 (`x2game-dev_dedicate.dll.c`, registration `sub_392A9B50`).
* Water events from `CActor::UpdateSwimStats`: 10 enter, 11 leave, 12 breath direction.
* Special-move trigger text: `0=Jump, 1=SpeedSprint, 2=StrengthJump` (`0x399ae3c4`).

## 4. DB fields used

`buffs`: `gliding`, `gliding_{move,fall}_speed_{slow,normal,fast}`, `gliding_rotate_speed`,
`gliding_startup_{speed,time}`, `gliding_smooth_time`, `gliding_lift_{count,height,speed,
duration,valid_time}`, `gliding_sliding_time`, `gliding_land_height`, `remove_on_land`.

`actor_models`: `game_jump_height`, `physics_living_{gravity,air_resistance,k_air_control,
max_vel_ground,min_slide_angle,max_climb_angle,min_fall_angle,time_impulse_recover}`,
`swim_height`, `turn_speed`, `fly_mode`, `underwater_creature`.

`game_stances`: `stance_id` 1 combat, 2 relaxed, 3 swim, 4 coswim, 5 combat (second row),
6 stealth, 7 crouch, 8 prone, 9 fly; model 1 relaxed max 5.4, swim max 3.0, fly max 5.4.
