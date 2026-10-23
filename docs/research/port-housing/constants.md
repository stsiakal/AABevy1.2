# Constants, globals and cvars used by 1.2 housing placement

All x2game addresses are VAs in the shipped 1.2 image (base `0x38FF0000`). Constants were read from the
unpacked image/data dump `D:/aa-re/client12/x2game_unpacked_data.dll` (same sections as
`D:/aa-re/client12/x2game_unpacked.dll`); decompile citations are `x2game.dll.c` function markers.
Names from the 10.x dev build are marked **[R10]** (`D:/aa-re/zone/bin/x2game-dev_dedicate.dll`,
strings file). No CRYENGINE source is used.

## 1. The wheel rotation step - resolved

`FUN_39016fe0` (`0x39016fe0`, 252 B) is the complete builder rotation delta. It tests six actions in
this order and returns the first non-zero product (assembly `0x39016fe0..0x390170da`):

| action id | binding name (`game/default_binding.g`) | coefficient VA | value | term |
|---|---|---|---|---|
| `0x5a` | `builder_rotate_left_normal` | `0x3999cf98` | +0.017453292 (1 deg in rad) | `axis * 1deg * X` |
| `0x5b` | `builder_rotate_right_normal` | `0x3999cf94` | -0.017453292 | `axis * -1deg * X` |
| `0x5c` | `builder_rotate_left_small` | `0x3999cf90` | +0.01 rad | `axis * 0.01` |
| `0x5d` | `builder_rotate_right_small` | `0x3999cf8c` | -0.01 rad | `axis * -0.01` |
| `0x5e` | `builder_rotate_left_large` | `0x3999cf80` / `0x3999cf88` | +pi / 12 | `axis * 15deg` |
| `0x5f` | `builder_rotate_right_large` | `0x3999cf84` / `0x3999cf88` | -pi / 12 | `axis * -15deg` |

`X` is read as a virtual float getter on the object pointer `DAT_39adac58`:
`mov ecx,[0x39adac58]; mov eax,[ecx]; call [eax+0xc]` at `0x39016fe4..0x39016fef` and again at
`0x3901701b..0x39017028`. The extra value in ECX at entry (`param_1`) is discarded when ECX is
reloaded, so `FUN_39016fe0` has no meaningful input.

**Resolution of the multiplier.** The `.data` descriptor table entry at `0x39ac5658` is
`{ name=0x3999c708 (empty), help=0x3999c708, dest=0x39adac58, default=5.0f (0x40a00000), flags=0x80 }`
(read from `x2game_unpacked_data.dll`, VA `0x39ac5658`; the same table also holds the camera
entries at `0x39ac5540..0x39ac566c`). The 10.x dev build has the identical entry, now named:

* `builder_rotate_angle` (`0x3A01A178`), help **"builder rotation angle per single input"**
  (`0x3A01A190`), value pointer `0x3A5F1F00`, default **5.0**, flags `0x80`
  (descriptor at `0x3A52EAB0`: name, value, `0x8040A00000` = flags 0x80 | float 5.0). **[R10]**
* The string `builder_rotate_angle` does **not** occur in the 1.2 x2game strings (`strings.txt`), so
  the 1.2 name is taken from 10.x; the 1.2 descriptor's value/default/flags match exactly.

No shipped 1.2 config overrides it: all 139 `game/config*/**` files extracted from `game_pak`
(`data/extract/housing/game/config*`) contain no `builder_rotate_angle`/`builder` setting. Therefore
the effective multiplier is **5.0 degrees per single input**, and one wheel notch produces
`axis * 1deg * 5.0`; the only unmeasured quantity left is the input axis amplitude produced by one
notch (`FUN_39424580(0x5a)`), which the in-process harness must read once (see `difftest.md`, U1).

Elevation (state 3, non-house builders) is `FUN_393fd240(9) * _DAT_399cf8bc`, and
`_DAT_399cf8bc = -0.02` rad (read from `.data`, `0x399cf8bc`): **-0.02 rad per axis unit**
(approximately -1.146 deg), with no cvar (`FUN_39332030` at `0x3933204f`).

## 2. Position conversion (house/seed packets 0x56/0x57/0xE6)

`FUN_3961dd70` (`0x3961dd70`, 85 B, `__thiscall`) converts a float position into the i64 fixed point
used by `CSConstructHouseTax` 0x56, `CSCreateHouse` 0x57 and `CSCreateDoodad` 0xE6 (0x58 uses
house-local floats):

```
out.x = round(in.x * 2^44) + origin.x          // 2^44 = 0x399f9a80 (+17592186044416.0)
out.y = round(in.y * 2^44) + origin.y
out.z = in.z + origin.z
```

Assembly: `fld [pos]`, `fmul [0x399f9a80]`, `call 0x398b5706` (float→u64 rounding), `add eax,[ecx]`,
`adc edx,[ecx+4]`, then the same for y, then `addss` for z (`0x3961dd70..0x3961ddc2`).
`ecx` is the caller-supplied origin (`GameClient+0x118` in every placement caller);
`edx`/`[ebp+0xc]` is the output, `[ebp+8]` is the float input. The scale is exactly the aa-net
`coord_to_long` scale (`(v * 4096) << 32`, `crates/aa-net/src/wire.rs:283`). The origin at
`[[0x39b02294]+0x118]` is a world/zone offset; its semantics beyond "i64 x/y + f32 z added to the
local model position" are **[I]**.

## 3. Geometry and interaction constants

| VA | value | used by |
|---|---|---|
| `0x3999ecf4` | 6.0 m | maximum terrain height spread (`FUN_39332180` `0x3933218a`, `FUN_39337fb0` `0x39338c..`; error 0x74) |
| `0x399cf898` | 30.0 m | extra range added to `FUN_3963b390(unit+8)+0x40` in `FUN_39333d40` (error 0x9a) |
| `0x399a319c` | 200.0 | screen ray length argument to `FUN_394298c0` from `FUN_39332030` |
| `0x399d2464` | 900.0 (= 30 m^2) | seed/doodad max squared distance to player (`FUN_393a8fa0`, error 0x9a) |
| `0x399d2460` | 2*pi | random scale range for doodads (`FUN_393a9340`) |
| `0x3999d538` | 100.0 | deco preview box height (`FUN_3903d870`/`FUN_3903c2e0`) |
| `0x3999dd60` | ~1e-20 | quaternion normalisation epsilon (`FUN_3903da00`) |
| `0x3999ddc8` | 50.0 | deco cursor ray step/scale (`FUN_3903da00`) |
| `0x3999ddcc` | 1,000,000.0 | finite/abs sanity cap for ray results (`FUN_394298c0`, `FUN_3903da00`) |
| `0x3999c488` | 1.0 | generic one |
| `0x3999c478` | 1.5 | `rsqrt` Newton iteration constant |
| `0x3999c47c` | 0.5 | generic half / `rsqrt` Newton constant |
| `0x3999cffc` | 2.0 | quaternion-multiply factor in dot/QN math |
| `0x399a2f9c` | 2*pi | full turn (manager spin/rotation helpers) |
| `0x399cf5ac` | pi | house preview spin factor in `FUN_3932a3b0` |
| `0x3999dd34` | 0.2 | debug label offset (`FUN_3903b930`) |
| `0x399a3520` | -1,000,000.0 | no-water sentinel compared in `FUN_39334380` |
| `0x3a167ea0` + 5000 | 5000 ms | `CSCreateHouse` client rate limit (`FUN_3932fe60` `0x3932ffb7..0x3932ffc5`, error text `CANNOT_LOCATE_DOODAD_NOW`) |
| `0x39adcc24` + 1000 | 1000 ms | doodad/deco cursor cooldown (`FUN_3903b8a0` `0x3903b8bb`) |
| `0x39ac79fc` | 9.0 | house interaction surface distance squared (`FUN_393befd0`, 3 m gap) |
| `0x3999cf30` | 0.001 | ms→s in profiler spans (not placement logic) |

`FUN_3961dd70`'s scale `2^44` is 0x399f9a80; `FUN_39333d40` uses
`sqrt((dx)^2+(dy)^2+(dz)^2) > unit_range + 30.0` against the player; `unit_range` is the live
player unit's `+0x40` (read at `0x39333d4a`).

## 4. Globals

| VA | type | role |
|---|---|---|
| `0x39aff98c` | `HousingManager*` | global housing manager (`DAT_39aff98c` in every housing function) |
| manager `+0x08` | map | houses by `tl`; `FUN_3932b510` lookup |
| manager `+0x30` | builder* | active house/decoration builder (house builder/deco builder) |
| manager `+0x34` | cursor* | decoration placement cursor (0x54 B), created by `FUN_3932c120` |
| manager `+0x78` | u32 | selected/target house unit id (`FUN_393befd0`) |
| manager `+0x7c` | u32 | current interaction unit id (`FUN_393befd0`) |
| `0x39b02294` | `GameClient*` | client root; `+0x10c` local player actor, `+0x40+0x18` player unit id, `+0x118` position origin, `+0x18` flag word |
| `0x3a5bb1d8` | `gEnv`-style root | engine/interface table used for models, terrain, cursor, input |
| `0x3a167e84` | cvar/option object* | debug-draw gate (`vtbl+8` > 0) for plot dots, connector boxes and error text |
| `0x3a167ea0` | u32 | last `CSCreateHouse` send time (ms) |
| `0x3a167ebc` | i32 | `ht` field of `CSCreateHouse` |
| `0x3a167ed8` | u32 | sentinel housing design/unit type ("no house") |
| `0x3a167edc` | u32 | decoration template sentinel (`FUN_39327bc0`) |
| `0x3a167ee8` | u32 | "no housing design" sentinel (`FUN_39325f60`) |
| `0x3a167eec` | u32 | special item/housing sentinel in `FUN_39327bc0` |
| `0x3a167ef0`, `0x3a167ef4`, `0x3a167ef8`, `0x3a167efc` | 16-byte mask | sign masks for quaternion multiply |
| `0x3a167f00` | f32 | accumulated house preview spin angle |
| `0x3a168288` | byte | one-time profiler registration flag for `HousingManager::Update` |
| `0x3a17a948` | builder* | doodad/seed placement builder (not the housing manager) |
| `0x3a17a950` | u32 | doodad builder cooldown until (ms) |
| `0x3a17a96c`, `0x3a17a978`, `0x3a17a984`, `0x3a17a99c` | u32 | doodad sentinel ids (none/invalid) |
| `0x3a17a970` | u32 | doodad marker/leaf state counter |
| `0x39d44c50/58`, `0x39d44f04` | ptrs | UCC/chat helpers seen in the 0x3930xxxx unrelated code (not placement) |
| `0x39ac3054`, `0x39ac304c` | u64 pair | zero/fallback position |
| `0x39ac3084` | u32 | cursor resource id/arg for `FUN_39017410` |
| `0x39ac3088` | char*[4] | `{"stanby","position","rotation","elevation"}` `BUILDER_STEP` strings |
| `0x39ac9020` | char*[5] | hotkey mode names (`normal_mode`,`mode_action_mode`,`builder_rotate_mode`,...) |
| `0x39acb470` | char*[10] | cursor path table for modes 0,2,3: `housing_position/rotation/elevation/zoom/invalid` |
| `0x39acb458` | char*[10] | cursor path table for mode 1 (decoration): `housing_deco_position/rotation/zoom/invalid` |
| `0x39adac58` | cvar object* | `builder_rotate_angle` (see §1) |
| `0x39adac5c` | int | current cursor state (`FUN_390173b0`, `FUN_39017410`) |
| `0x39adac60` | int | last builder step state (`FUN_39017410`) |
| `0x39adac64` | u32 | no-mount sentinel compared with `player[0xebc]` in `FUN_39017620` |
| `0x39adcc24` | u32 | deco/doodad cursor cooldown (ms) |
| `0x39adcc28` | u32 | deco cursor ray step/reference value |

## 5. Cvars

Only one cvar is read by the placement code: `builder_rotate_angle` (1.2 object pointer
`0x39adac58`, 10.x name). The 1.2 strings do not contain the name; the 10.x descriptor and the 1.2
default/flags match. No other `builder_*`, `housing_*` or `deco_*` cvar is registered or read by
these functions (`strings.txt` and `strings-xrefs.tsv` searches; `FUN_39016fe0` is the only cvar
read). `ctrl`-wheel zoom is the camera path, not this function.

## 6. Data tables

| table | 1.2 SELECT string | use |
|---|---|---|
| `housings` | `strings.txt:0x399fa858` | design: category +0xc, dot flag +0xd, use range, water flag, garden radius, limits, `main_model_id` |
| `housing_build_steps` | `0x399feb48` | construction steps/items/actions/skills |
| `housing_decorations` | `0x399fec78` | deco design: `actability_group_id, actability_up, allow_mesh_on_garden, allow_on_ceiling, allow_on_floor, allow_on_wall, allow_pivot_on_garden, deco_actability_group_id, doodad_id` |
| `item_housings` | `0x39a01bb4` | item→house design |
| `item_housing_decorations` | `0x39a01c50` | item→deco design + `restore` |
| `housing_deco_limit_elems`, `housing_deco_limits` | `0x399fe9f0`, `0x399feac0` | per-group deco caps |
| `housing_group_categories`, `housing_groups` | `0x399fedb0`, `0x399faa18` | construct-count / houseless conditions |
| `housing_binding_doodads` | `0x399ff8e0` | built-in furniture excluded from counters |

The 1.2 `housing_decorations` SELECT has **no** `allow_garden_id`, `collision_by_proxy` or
`vertical_aligned` (10.x additions); do not port fields that 1.2 does not load.
