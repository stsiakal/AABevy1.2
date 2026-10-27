# Constants read from the 1.2 image

Source: `D:/aa-re/client12/x2game_unpacked_data.dll` (runtime dump, base `0x38FF0000`), read at
`VA - 0x38FF0000` through the section table. Values marked **[R]** are static bytes in the dump;
**[runtime]** globals were zero/`null` at dump time. Function addresses are x2game VAs.

Reading helper used for this table (Python 3, no dependencies):

```python
import struct
data = open(r"D:/aa-re/client12/x2game_unpacked_data.dll","rb").read()
BASE = 0x38ff0000
pe = struct.unpack_from("<I", data, 0x3c)[0]
ns = struct.unpack_from("<H", data, pe+6)[0]
opt = pe+24; so = opt+struct.unpack_from("<H", data, pe+20)[0]
secs=[]
for i in range(ns):
    o=so+i*40; n=data[o:o+8].rstrip(b"\0").decode()
    vs,va,rs,ra=struct.unpack_from("<IIII",data,o+8); secs.append((n,va,vs,ra,rs))
def off(va):
    r=va-BASE
    for n,sva,vs,ra,rs in secs:
        if sva <= r < sva+max(vs,rs): return ra+r-sva
def f32(va): return struct.unpack_from("<f",data,off(va))[0]
def u32(va): return struct.unpack_from("<I",data,off(va))[0]
```

## 1. Move-struct codecs (`packets.md`)

| VA | value | meaning | used by |
|---|---|---|---|
| 0x399abe78 | 3.0518509447574615e-05 (1/32767) | i16 full scale | all i16 codecs |
| 0x39a12660 | 0.9975000023841858 | quaternion `w` reconstruction bound | `397adc00`/case 1/2/3/5 |
| 0x39a12664 | 0.01666666753590107 (1/60) | type-1 velocity scale | `397ad5a0` |
| 0x39a12668 | 0.03333333507180214 (1/30) | type-2/3/5 velocity scale | `397ad8c0` |
| 0x3999f7c0 | 30.0 | type-2/3/5 velocity scale (multiplier) | `397c0470` |
| 0x3999d01c | 60.0 | type-1 velocity scale | `397c0470` |
| 0x3999ddc8 | 50.0 | type-0 velocity scale | `397c0470` |
| 0x399a28dc | 0.007874015718698502 (1/127) | delta-movement u8 scale | `397ad740` |
| 0x39a13764 | 0.0078125 (1/128) | fall-velocity u16 scale | case 1 |
| 0x399c8c1c | 128.0 | fall-velocity inverse | case 1 |
| 0x399f9b84 | 2.384185791015625e-07 (1/2^22) | position z scale | `3961e2c0` |
| 0x399f9bac | 4194304.0 (2^22) | position z multiplier | `3961e2c0` |
| 0x399e6b34 | 32767.0 | i16 full scale (float form) | `397ad5a0`/`397ad8c0`/`397ada60` |
| 0x399f9bb0 | 4096.0 | position z max | `3961e2c0` |
| 0x399f9bb4 | -100.0 | position z min | `3961e2c0` |
| 0x399f9bb8 | 4196.0 | default cell size (`DAT_3a5e1100`) | `3961e2c0` |
| 0x3999d538 | 100.0 | z bias | `3961e2c0` |
| 0x39a12670 / 0x39a12678 | 0.0 (sign-mask temporary in the dump) | MSVC rounding/masking temporaries | `397ad8c0`/`397ae020` |
| 0x3999cc10 | 0.0 | zero | `397ad8c0` |
| 0x3999d500 | NaN bit mask (0x7fffffff) | `fabsf` idiom (`x & 0x7fffffff`) | many |
| 0x3999c5f0 | 0x80000000 (sign bit) | `-x` via xor | many |
| 0x3999c488 | 1.0 | one | many |
| 0x3999c47c | 0.5 | half | many |
| 0x3999c478 | 1.5 | 1.5 | quaternion normalise |
| 0x3999c484 | 0.0 | zero | many |
| 0x3999c480 | 1.1920928955078125e-07 | FLT_EPSILON (vector length guard) | `394aabd0`, `390af740` |
| 0x3999cc18 | 1.1754943508222875e-38 | FLT_MIN (avoid /0) | `39404d50` |

## 2. `vehicle_models` row layout (from the loader `FUN_39736bd0`)

The loader reads the columns in this order and stores them in a `0xbc`-byte record; the offsets
below are the record offsets (column index in parentheses). This is the struct the vehicle code
reads through `entity+0x1e0 -> +0x34` (see `structs.md`).

| off | type | column | notes |
|---|---|---|---|
| +0x00 | f32 | `angVel` (1) | max turn rate (rad/s) |
| +0x04 | u8 | `auto_level` (2) | bool |
| +0x05 | u8 | `can_fly` (3) | bool |
| +0x08 | u32 | `damaged25` (4) | string id (prefab variant) |
| +0x0c | u32 | `damaged50` (5) | string id |
| +0x10 | u32 | `damaged75` (6) | string id |
| +0x14 | u32 | `dead` (7) | string id |
| +0x18 | u8 | `driver_walk` (8) | bool |
| +0x1c | u32 | `dying` (9) | string id |
| +0x20 | f32 | `floating_height` (10) | |
| +0x24 | f32 | `floating_wave_height` (11) | |
| +0x28 | f32 | `floating_wave_period_ratio` (12) | |
| +0x2c | u8 | `installed_turret` (13) | bool |
| +0x30 | f32 | `lin_deaccel_inertia` (14) | |
| +0x34 | f32 | `lin_inertia` (15) | |
| +0x38 | f32 | `max_climb_ang` (16) | |
| +0x3c | u32 | `normal` (17) | string id |
| +0x40 | f32 | `rot_deaccel_inertia` (18) | |
| +0x44 | f32 | `rot_inertia` (19) | |
| +0x48 | u32 | `sound_id` (20) | id, remapped through a table |
| +0x4c | u8 | `susp_axle` (21) | bool |
| +0x50 | f32 | `susp_stroke` (22) | |
| +0x54 | f32 | `trail_align_ratio` (23) | |
| +0x58 | f32 | `turret_pitch_angle_max` (24) | |
| +0x5c | f32 | `turret_pitch_angle_min` (25) | |
| +0x60 | f32 | `turret_pitch_angvel` (26) | pitch rate (deg/s) |
| +0x64 | f32 | `turret_yaw_angle_max` (27) | |
| +0x68 | f32 | `turret_yaw_angle_min` (28) | |
| +0x6c | f32 | `turret_yaw_angvel` (29) | yaw rate (deg/s) |
| +0x70 | u8 | `use_center_spindle` (30) | bool |
| +0x71 | u8 | `use_proxy_collision` (31) | bool |
| +0x72 | u8 | `use_wheeled_vehicle_simulation` (32) | bool |
| +0x74 | f32 | `velocity` (33) | max speed (m/s) |
| +0x78 | u32 | `wheel` (34) | cgf string id |
| +0x7c | u32 | `wheel2` (35) | cgf string id |
| +0x80 | f32 | `wheeled_vehicle_ballast_mass` (36) | |
| +0x84 | f32 | `wheeled_vehicle_ballast_pos_y` (37) | |
| +0x88 | f32 | `wheeled_vehicle_brake_torque` (38) | |
| +0x8c | u32 | `wheeled_vehicle_drive` (39) | 0/1/2 = FWD/RWD/AWD |
| +0x90 | f32 | `wheeled_vehicle_front_optimal_sa` (40) | |
| +0x94 | f32 | `wheeled_vehicle_gear_speed_ratio_1` (41) | |
| +0x98 | f32 | `wheeled_vehicle_gear_speed_ratio_2` (42) | |
| +0x9c | f32 | `wheeled_vehicle_gear_speed_ratio_3` (43) | |
| +0xa0 | f32 | `wheeled_vehicle_gear_speed_ratio_reverse` (44) | |
| +0xa4 | f32 | `wheeled_vehicle_mass` (45) | |
| +0xa8 | u32 | `wheeled_vehicle_max_gear` (46) | physicalize adds 2, clamps 1..5 |
| +0xac | f32 | `wheeled_vehicle_power` (47) | hp; ×745.7 -> W |
| +0xb0 | f32 | `wheeled_vehicle_rear_optimal_sa` (48) | |
| +0xb4 | f32 | `wheeled_vehicle_susp_damping` (49) | |
| +0xb8 | f32 | `wheeled_vehicle_susp_stroke` (50) | |

Evidence: `FUN_39736bd0` column reads, and the consumers `FUN_397a6c00` (+0x78/+0x7c),
`FUN_390d14b0` (+0x58..+0x6c), `FUN_390cf180` (+0x74..+0xb8), `FUN_390f0510` (+0x72).
Caution: `UpdateDriver`/`UpdateCenterSpindle` write `*(char *)(local_28 + 0x1c)` where `local_28`
is a `float*` to the row, i.e. **row+0x70** (`use_center_spindle`), not `+0x1c` (`dying`). [R]

## 3. Vehicle/ship visual constants

| VA | value | meaning | used by |
|---|---|---|---|
| 0x3999ccf0 | 0.1 | min dt / clamp | `390cffd0`, `390d0800`, `390cf180` |
| 0x3999ccf4 | 5.0 | speed epsilon | `390b0040`, `390cf180` |
| 0x399a2f7c | 16384.0 | wheeled warp reset distance² (128 m) | `3909c660` |
| 0x399a2d5c | 0.0872664675 (5°) | velocity filter minimum | `390967c0` |
| 0x3999cffc | 2.0 | half-angle / distance factor | quat integration, `390c22c0` |
| 0x3999cf90 | 0.0099999998 | change-detection epsilon | `394046f0`, `390cf180` |
| 0x3999cf30 | 0.001 | ms -> s | send timers, phys factor |
| 0x399a3364 | 400.0 | interpolation snap distance² (20 m) | `390b0040` |
| 0x399a440c | 0.5235988 (30°) | max steer angle | `390cffd0` |
| 0x399a3aac / 0x399a3ab0 | -10000.0 / 10000.0 | spindle angle clamp | `390cffd0` |
| 0x399a36b0 / 0x399a36b4 | π / 2π | angle wrap | `390d14b0`, `390cffd0` |
| 0x3999cf84 | -π | angle wrap low | `390d14b0` |
| 0x3999cf98 | 0.017453292 (1°) | turret/driver rate unit | `390d14b0`, `390d0800` |
| 0x399a36b4 | 6.2831855 | 2π | `390d14b0` |
| 0x399a44f4 | 57.295776 | rad -> deg | `394aae90`, `390d2990` |
| 0x3999f5bc | 45.0 | helm wheel quantisation | `390d2990` |
| 0x3999f0e4 | 7.0 | default wind/offset term | `390d2990` |
| 0x3999edbc | 3.0 | steering-wheel scale | `390d0800` |
| 0x399a445c | 0.6108652 (35°) | driver wheel range | `390d0800` |
| 0x399a29c8 | 0.7853982 (45°) | driver wheel offset | `390d0800` |
| 0x3999d1ac | -0.7853982 | driver wheel low | `390d0800` |
| 0x399a36e0/36e4/36e8 | 0.0 / 10.0 / 0.0 | grip IK axis | `390d0800` |
| 0x399a36f0 | 0.0 (3 floats) | driver base rotation | `390d0800` |
| 0x399a232c | 0.05 | pose apply step | `390cf180`, `390af740` |
| 0x399a24b8 | 0.4 | physicalize inertia factor | `390cf180` |
| 0x399a3158 | 0.9 | max lateral factor | `390af740` |
| 0x399d5ee4 | -0.9 | min lateral factor | `390af740` |
| 0x399ad2f0 | -0.3 | slope factor | `39404d50` |
| 0x399a433c | 0.10471976 (6°) | physicalize steer/mass factor | `390cf180` |
| 0x399a4338 | 745.6999 | hp -> W | `390cf180` |
| 0x399a2f24 | -0.1 | clamp | `390cf180` |
| 0x399a2ea0/2ea8/2eb0 | 0.3535534 / (mask) / 0.7071068 | normalise helpers | `39164ff0` (player aim) |
| 0x399a2748 | 0.0 | — | `39164ff0` |

## 4. Interaction, bind, placement

| VA | value | meaning | used by |
|---|---|---|---|
| 0x39ac79fc | 9.0 | interaction surface gap² (3 m) | `393bf270` |
| 0x39ac7a00 | 12.25 | interaction gap² when target `+0x3c26` (3.5 m) | `393bf270` |
| 0x39ac7408 | 16.0 | bind range² (4 m) | `39346290` |
| 0x3999f85c | 25.0 | unbind speed² (5 m/s) | `39345aa0` |
| 0x399cfcfc | 6400.0 | boat placement range² (80 m) | `39348bf0` |
| 0x3999cd70 | 4294967296.0 (2^32) | signed->unsigned float correction (adds 2^32 to negatives) | UI event helpers, e.g. `39345d00`/`39362770` |
| 0x399a3ab0 | 10000.0 | spindle clamp (see §3) | `390cffd0` |

Reused from `re-constants.md` (not re-read here): `0x39ac3878` = 9.0 (mount send range²),
`0x39ac3888` = 1.5, `0x39ac388c` = 1.0, `0x39ac3890` = 4.0 (dismount), `0x399a3240` = 0.64,
`0x399a3244` = 2.25, `0x399a3238` = -1.5, `0x399a323c` = 2.5 (approach), `0x399a24cc` = 0.6
(swim side), `0x399a1d94` = 0.8, `0x399a87e8` = 0.7, `0x3999c47c` = 0.5, `0x3999cffc` = 2.0,
`0x3999f9d0` = 12.0, `0x3999dd34` = 0.2.

## 5. Runtime globals (zero or pointer at dump time)

| VA | dump | meaning | evidence |
|---|---|---|---|
| 0x3a5bb1d8 | 0x3abeaf4 (tail) | GameClient singleton | `FUN_390b3750`, many |
| 0x39b02294 | ? | local player actor | `39346290`, `394045f0` |
| 0x3a59f84c | 0 | send max-interval timer object | `394045f0`, `394046f0` |
| 0x3a59f850 | 0 | send min-interval timer object | `394046f0` |
| 0x3a59f858 | 0 | "no parent/no id" sentinel (compared with object ids) | `390b3f10`, `39346290` |
| 0x3a168a64 | ? | "no object" sentinel (locator) | `3934a3e0`, `39346290` |
| 0x3a168a98 | ? | "my slave" sentinel | `39345d00` |
| 0x3a168a88 / 0x3a168a8c | ? | default (spawn x/y offset?) compared in `OnSlaveCreated` | `3934a6c0` |
| 0x39ae6c90 | 0 | wind direction object (getter at +0xc) | `390d2990`, `390c22c0` |
| 0x39ae6c94 | 0 | wind speed object (getter at +0xc) | `390d1c60`, `390c22c0` |
| 0x39ae4e90 / 0x39ae4e94 | 0 | 3D engine / cell size globals | `390b0040` |
| 0x39aefb98 | ? | game rules/config object (+0x2f8/+0x300 read by the jump/fall code) | `39404d50` |
| 0x3a5e1100 | 4196.0 | cell size (position codec) | `3961e2c0` |
| 0x3a1681f4 | ? | transfer "queued/no path" sentinel | `393540a0` |
| 0x3a5a4ea0 | 6 | locator state | `3934a3e0` |

## 6. Notes on constants that look like sentinels

* `0xffbfffff` (two floats packed: -1.0, -1.0) and `0x80000000` (0.0) are the **"field not set"**
  markers in the CryPhysics action structs (`FUN_3909c660`, `FUN_390af740`). Port them as
  `Option<f32>`/a bitmask, not as magic numbers.
* `0x3c010204` in the controller vtable area is not a constant; it is part of an adjacent
  structure. Do not port it.
* `0x3999d500` read as float is NaN by design (0x7fffffff): it is the `fabsf` mask, not a value.
