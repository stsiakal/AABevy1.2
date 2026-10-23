# Swim, dive and fall notes (1.2 x2game)

Addresses are VAs at base `0x38FF0000`; sizes from `funcs.csv`.

## 1. Water state

### 1.1 `CActor::UpdateSwimStats` `FUN_39120300` (0x39120300, 2072 B)

Callers: none in `calls.tsv` (virtual / actor update); callees: `FUN_39004bb0` (profile),
`FUN_39111da0` (bottom ray), `FUN_391121a0`, `FUN_39113b50`, `FUN_390b3750`, `FUN_3911f3f0`.

Fields (actor pointer = `param_1`):

| off | use |
|---|---|
| `+0x0c` | entity pointer (`+0x108` = physical entity) |
| `+0x1f` | in-water flag (byte); events 10/11 fire on the crossing |
| `+0x7c` | signed water/submersion timer (`param_1[0x1f]`); events 10/11 on crossing |
| `+0x80` | surface timer (`param_1[0x20]`); `jump_splash` when it crosses 0 |
| `+0x84` | water level (float, world Z relative, `param_1[0x21]`) |
| `+0x64` / `+0x68` | water-surface distance / previous value |
| `+0x830..+0x838` | last position used for the water probe (`param_1[0x20c..0x20e]`) |
| `+0x904` | bottom-level ray object (`param_1[0x241]`) |
| `+0x81c` | dead/swim-dead flag (`param_1[0x207]`) |
| `+0x81d` | water probe disable flag |

Order:

1. Profile guard `CActor::UpdateSwimStats` (0x399ac374) and the `- actual update` guard.
2. Compare `actor->entity->GetWorldPosition()` with `actor+0x830`; if the unit moved more than
   `DAT_399a321c`, or the probe is forced (`+0x904`, `+0x81c`, `+0x81d`, `FUN_391121a0`), update
   the stored position and run the probe.
3. The probe (`vtable+0x70`/`0x98`) gives the water surface; `local_44` is the surface Z and
   `local_20` the relative water level. The code writes `actor+0x84` and `actor+0x1a` and fires
   `actor vtable +0x184` events:
   * id 10 when entering water (the old flag was 0 and the new relative level is negative);
   * id 11 when leaving water;
   * id 12 when the breath-bar direction changes.
4. If leaving water upward and `actor+0x161` is set and `+0x20` was negative, it plays
   `"jump_splash"` and calls the sound/particle unit (`game vtable +0x2e0`) **[R]**.
5. `FUN_39111da0` updates the bottom-level ray (`actor+0x904`), `FUN_3911f3f0` is the dead-swim
   path (`swim_jump_speed`-like effect on death) **[R]**.
6. Breath: `actor+0x19` is the breath timer used by the UI; the actual start/capacity are in the
   player object (`+0xb24/+0xb28`, section 3).

The exact depth threshold is not a literal in this function; the water level is supplied by the
engine probe and the enter/leave decision is the sign of the relative level **[R/I]**. The
`0.2 m` values in `re-movement.md` are the `swim_height`/`swim_jump` cvars, not the enter-water
threshold.

### 1.2 `FUN_3911c970` (0x3911c970, 184 B) - water/land transition selector

Arguments: `(actor, inWater, deep, unknown)`. It maps the booleans to a small state 0..3 and:

* when the new state is 2 (the water/landing branch), if the previous state was 3 or `actor+0x78 != 0`, it calls
  `FUN_39117650` (water landing) and sets `actor+0x161 = 0x100`, `+0x164 = 0`;
* when leaving state 3, it calls `FUN_39117530(0)` (stance reset);
* it flushes `actor+0x54`/`+0x5c` and stores the state at `actor+0x50`.

`FUN_3911c560` (0x3911c560, 537 B) is the generic “end current action”: it resets stance,
velocity, `+0x1cc=-1`, etc.; `FUN_3911c8a0` (0x3911c8a0, 205 B) handles the upward transition
and clears `+0x161`; `FUN_3911c780` (section 4) handles the fall start **[R]**.

### 1.3 `FUN_3912ce00` (0x3912ce00, 2648 B) - swim control

Inputs:

* `controller+0x0c/+0x10/+0x14` = requested movement axes (from the request);
* `controller+0x48` = view quaternion (actor+0x2d0);
* `controller+0x08` = actor+0x50 (movement block);
* `controller+0x10`? (the same request copy) and `controller+0x23..0x25` = accumulated swim
  velocity (before the call).

Steps (from the decompile):

1. Read the physical entity velocity (`vtable+0x98`) and the water contact (`vtable+0x108`,
   `+0x24`); store the surface velocity at `controller+0x8c..0x94` if the controller reports it.
2. Build the input direction in camera space:
   `h = quat * (controller+0x0c, +0x10, +0x14)`, with the vertical component scaled by
   `DAT_399a3158 = 0.9` when moving backward (air-control-like) and `controller+0x1c` when
   swimming down.
3. Read the multipliers:
   * `game_cfg+0x15c` = `swim_back_speed_mul` (0.8) applied when the input Z (`controller+0x10`)
     is negative;
   * `game_cfg+0x160` = `swim_side_speed_mul` (0.6) applied to the side/vertical terms;
   * `game_cfg+0x194/+0x198/+0x19c/+0x1a0/+0x1b8/+0x1bc` = zero-G/fly multipliers and the
     impulse-recover factors.
4. Apply the entity contact response (`unit vtable +0x158`; the local `116`/`117` flags) and
   integrate the velocity into `controller+0x8c..0x94` and `controller+0x64..0x6c`.
5. If the actor is swimming and has a graph (`controller+0x10` = graph/model), set
   `SwimControlX`, `SwimControlY`, `SwimControlZ` by name through `FUN_39131ea0` (or the
   `FUN_39131f10` variant) **[R]**.

The up/down/buoy cvars (`game_cfg+0x164/+0x168/+0x16c`) are registered but no direct read site
was found in x2game (U-S1). The constants are `0.7`, `0.5`, `2.0` at `0x399a87e8`,
`0x3999c47c`, `0x3999cffc`.

## 2. Swim constants

Registration is in `FUN_390f7b40` (0x390f7b40, 12489 B). Offsets and defaults:

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
| `pl_flyingVelocityMultiplier` | +0x1dc | `0x3999f7c0` | 30.0 (registration default) |
| `gliding_mouse_ad` | +0x34c | `0x3999cf2c` | 8.0 |
| `gliding_mouse_ws` | +0x350 | `0x3999cf2c` | 8.0 |

(`FUN_390f7b40` lines around 195940–196260 in the decompile; values read from the `.data` dump.)

## 3. Breath

| function | addr | size | use |
|---|---|---|---|
| `FUN_3930b660` | 0x3930b660 | 246 | `GetBreathTime`: writes `left`/`capacity`; if capacity changed, rescales `start = now - round(newCap * elapsed / oldCap)` |
| `FUN_3930b600` | 0x3930b600 | 91 | start dive: `start=now`, `capacity=unit attr 0x5b`; end: `start=0` |
| `FUN_3930b050` | 0x3930b050 | 43 | server correction: `start = now - capacity + breath` |
| `FUN_3930b5b0` | 0x3930b5b0 | 77 | true if `FUN_39424580(6)` (jump) or `unit+0x54 > 0.2` (swim-jump permission) |

`FUN_3930b660` is called from `FUN_39495d10` (the `X2Player.GetBreathTime` binding), and the
Lua UI (`player_breath_bar.alb`) subscribes to `DIVE_START`/`DIVE_END`; the native arithmetic is
already mirrored by `hud/breath.rs` in Bevy. No new movement code is needed; the differential
test can call the original through the Lua binding.

## 4. Fall start / landing / damage

### 4.1 `FUN_3911c780` (0x3911c780, 274 B) - fall start

```c
if (actor+0xf8 == 0 && actor+0x1c1 == 0 && actor+0x874 < 1 &&
    actor+0x138 >= game_cfg[pl_fall_start_velocity] &&           // +0x174, 10.0
    actor+0x138 != game_cfg[pl_fall_start_velocity]) {
    groundZ = entity->vtable[0x7c]();                            // ground probe
    if (game_cfg[pl_fall_start_height] + groundZ < actor.z) {    // +0x170, 1.0
        FUN_39117530(1);                                          // stance/pose
    }
}
FUN_39117590();                                                   // track height
if (dt != 0) { actor+0x54 += dt; actor+0x58 = 1; }
```

`FUN_39117590` (0x39117590, 190 B): if the velocity dotted with the actor up axis is positive
(falling), and `actor+0x138 == 0`, it records the current ground Z at `+0x140`; it sets
`+0x138 = -verticalSpeed` and keeps `+0x13c` as the maximum downward speed. `-0.5`-style
constants are `0x3999c47c=0.5` and `0x3999ccf0=0.1` **[R]**.

### 4.2 `FUN_39116f00` `CActor::UpdateFallDamage` (0x39116f00, 609 B) - landing

```c
if (actor+0x148 > 0) actor+0x148 -= dt;             // landing timer
if (actor+0x162 != 0 || actor+0x68 <= 0 ||
    (actor+0x13c * 0.1 >= actor+0x138 && actor+0x138 != 0)) {
    if (actor+0x13c >= game_cfg[pl_fallDamage_SpeedSafe] &&   // +0x178, 20.0
        actor+0x874 < 1 && actor+0x1c1 != 2 && actor+0x1c1 != 4 &&
        actor+0x148 <= 0) {
        drop = actor+0x140 - groundZ;
        impact = sqrt(max(0, drop) * 40.0);                  // 0x3999f668 = 40
        impact = min(impact, actor+0x13c);
        actor+0x13c = 0;
        actor+0x144 = impact;                                // reported as fall_vel
    }
} else if (actor+0x10 == 0 && actor+0x50 == 3) {
    return;                                                   // no landing
}
actor+0x140 = 0; actor+0x138 = 0;
```

The `+0x874 < 1` gate and stance `2/4` checks skip fall damage for swimming and gliding. The
client never subtracts health; `+0x144` is serialized into the movement packet's `fall_vel`
(server `DoFallDamage` applies the damage, `re-movement.md` §2.2).

### 4.3 Water landing `FUN_39117650` (0x39117650, 1083 B)

When the landfall latch `actor+0x854` is clear and the unit is falling (`actor+0x161 != 0` or
`actor+0x138 >= 5.0` = `DAT_3999ccf4`), not dead, and the entity exists:

1. raycast from the actor down/forward with the `Game` mask (0x117/0x40f), get the water depth
   (`FUN_390b3750` / `FUN_390b3ad0` water query);
2. if the water surface is below the actor by more than `0.01`, it calls `FUN_390bf210` (splash
   particle), builds `"landfall"` and `"speed"` graph inputs (`FUN_391117f0`; speed is the
   horizontal speed / 0.1 clamp), and calls the sound/particle unit;
3. if the unit is above the water, it can set the “swim jump” permission
   (`actor+0x138 * 0.2`, `swim_jump_permission_range`) and stage the sound event.

This is the “entered water from a fall” effect; the actual stance change is in
`FUN_3911c970`/`FUN_39120300`.

### 4.4 Underwater/bottom `FUN_39116d20` (0x39116d20, 475 B)

Computes the actor bottom vs water surface, stores `actor+0x84` and `actor+0x68`; if the bottom
is above the surface minus the threshold it sets `actor+0x152 = 1` and disables the physics
entity (`FUN_390981b0`, params block) — the “out of water / drowning/bottom” path **[R/I]**.

## 5. Current/flow

No `water_flow`, `current_speed` or flow-field symbol is used by x2game movement; the strings
contain only unrelated “current” names. `UpdateSwimStats` has no horizontal advection; the
server/physics owns the position. Do not port a client current unless a live test shows one
(U-S3).

## 6. Test recipes

1. **Surface/dive**: place the live actor at a known water spot; call `FUN_39120300(dt)` with
   dt=0.033 and read `+0x84`, `+0x1f`, `+0x20`; repeat above/below the surface. Hook
   `vtable+0x184` to capture event ids 10/11/12.
2. **Swim control**: build the movement context (as in glider.md) around the live actor; call
   `FUN_3912ce00(controller)` for forward/back/side/up/down and compare
   `controller+0x64..0x6c`, `+0x8c..0x94` and the `SwimControl*` values captured at
   `FUN_39131ea0`.
3. **Breath**: call `FUN_3930b660(&left,&capacity)` after `FUN_3930b600(1)`; advance the game
   time and compare; then `FUN_3930b050(breath)`.
4. **Fall start**: synthetic actor with `+0x138`, `+0x13c`, `+0x1c1=0`, `+0x874=0`; call
   `FUN_3911c780(dt)` and compare the ground probe and `+0x140`.
5. **Fall damage**: synthetic actor with `+0x13c`, `+0x140`, `+0x148`, `+0x1c1`; call
   `FUN_39116f00(dt)` and compare `+0x144` for drops 0, 1.3, 20 m, speed safe exactly 20,
   stance 2/4 and `+0x874=1`.
6. **Water landing**: call `FUN_39117650` at a water spot and capture the `"landfall"`/`"speed"`
   inputs and the splash event.
