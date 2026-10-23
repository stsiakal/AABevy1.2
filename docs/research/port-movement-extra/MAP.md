# Port map: 1.2 movement extras (glider, swim, fall, click-to-move, jump, teleport)

Address map for the later function-by-function port of the parts of the 1.2 client movement
system that sit **outside** the core controller already mapped in
[`../port-movement/MAP.md`](../port-movement/MAP.md):
gliders, the remaining swim/fall behaviour, click-to-move / auto-run / follow, jump, and the
movement/camera side of teleports and portals. The differential-test plan calls the original
functions inside the running hidden 1.2 client (`aadriver.dll`).

## 0. Sources, conventions and licensing

### 0.1 Licensing

Everything here is derived **only** from the shipped 1.2 binaries and their decompiles. The
published CRYENGINE source was not used, quoted or consulted. Class names in `CClass` form come
from the binaries' own RTTI, profiler strings, log/assert strings or cvar names; behaviour that
has no name in the binary is marked *(inferred)*. Port from this document and the decompiles,
never from the CRYENGINE source.

### 0.2 Sources

| source | path |
|---|---|
| 1.2 x2game decompile | `C:/AAEMU_DISK/aa-re/client12/out-x2game/x2game.dll.c` |
| 1.2 x2game functions/calls/strings | `C:/AAEMU_DISK/aa-re/client12/out-x2game/{funcs.csv,calls.tsv,strings-xrefs.tsv}` |
| 1.2 x2game image (constants) | `D:/aa-re/client12/x2game_unpacked_data.dll`, base `0x38FF0000` |
| 1.2 engine decompiles | `D:/aa-re/client12/engine/out/`, `D:/aa-re/client12/engine2/out/` |
| 1.2 cryphysics (living entity) | `D:/aa-re/cryphysics/x86_12/`, ported by `aaemu-physics` |
| 10.x names / structure only | `D:/aa-re/zone/boot/root/Bin64/decompiled/x2game-dev_dedicate.dll.c` |
| game DB | `D:/aaemu-live/rs-game/Data/compact.sqlite3` (read-only) |
| bindings | `game/default_binding.g` in `D:/AA1.2/game_pak` |
| existing notes | `docs/research/re-movement.md`, `re-mounts*.md`, `re-constants.md`, `port-movement/` |

x2game addresses are VAs at the preferred base `0x38FF0000` (same numbering as
`x2game.dll.c`). The unpacked tail (`0x3a6xxxxx`–`0x3acxxxxx`) is part of the same image.

Evidence markers: **[R]** read from the binary/decompile/DB, **[I]** inference, **[?]** open.

### 0.3 Scope inventory

This map documents **75 unique x2game entries, 64,691 bytes** (overlaps between areas counted
once): 28 glider, 18 swim/fall, 13 click-move/auto-run/follow, 8 jump (mostly overlapping the
glider dispatcher) and 16 teleport/camera. Existing specs (`re-movement.md`,
`re-mounts-full.md`) are not repeated except where this work changes or sharpens them.

### 0.4 Corrections / sharpening of the existing notes

1. **`actor+0x1c1 == 4` is the gliding movement state (and possibly also used by mounts).**
   `FUN_3912dd40` (0x3912dd40) dispatches stance 4 to `FUN_3912d860` (glide) and stance 5 to
   `FUN_3912b280` (glide straight); the mounted branch is selected by `actor+0xf8 != 0` ->
   `FUN_3912c770` (0x3912c770), not by the stance byte. `FUN_3911d6c0(1)` (0x3911d6c0) sets
   `+0x1c1 = 4` while riding the glider. The mount turn ramp in `port-movement/MAP.md` §3.2
   reads the same byte as "quadruped"; value 4 alone therefore does not identify a mount, and
   the `FUN_39402e00` mount case must be re-checked in the core port **[R/I]**.
2. **The 0x50-byte glider parameter block is mapped** (section 2.2): it is the buff's 17
   `gliding_*` values plus three runtime words, copied to `actor+0x168` by `FUN_3911fc80`.
3. **The jump impulse site is found** in `FUN_3912b330` (0x3912b330): the jump vector is built
   there from the movement block and `actor+0x470` (`game_jump_height`), and flagged as mode 7;
   the literal physics `action_move.iJump` call is still not in x2game (U9 remains partially
   open).
4. **Fall damage is skipped while gliding (state 4) and swimming (state 2)**; see
   `FUN_39116f00` (0x39116f00).
5. **Click-to-move's move request is `FUN_390a58b0`**, queued from `FUN_390a6af0`; the 1.5
   pseudo-speed constant is `DAT_3999c478` (1.5) used by `FUN_393ba480` (0x393ba480).

---

## 1. Gliders

### 1.1 Deploy / fold call chain

```
buff / item applied
├─ X2::GameClient::ClientBuffMan::ApplyAttrs   FUN_39389440 (1830 B)
│  └─ FUN_39357920(player, desc, 0)            FUN_39357920 (610 B)   at 0x736056
│     ├─ FUN_39005b20(..., 7/8/9) * 0.001      three move-speed values (integer -> f32)
│     ├─ builds 0x50-byte glide block on stack (section 2.2)
│     ├─ FUN_3911fc80(1, &block)               0x3911fc80 (551 B)
│     │  ├─ memcpy(actor+0x168, block, 0x50)
│     │  ├─ sets stance 4 (fly) or 5 (land/slide) via actor+0x1c1
│     │  └─ initialises timers (+0x1b8/+0x1bc, lift duration)
│     └─ FUN_391219c0(1)                       0x391219c0 (308 B)  attach item slot 0x1a
│
equipment part change (backpack slot 26)
├─ X2::GameClient::ApplyChangedParts           FUN_393220a0 (1266 B)
│  ├─ FUN_39357b90(part, item)                 checks backpack + flags
│  ├─ FUN_396f63d0(item)                       get item desc
│  └─ FUN_39357920(player, item, 1)            at 0x649085
│
buff removed / landed
├─ FUN_39389d60 (1146 B)
│  ├─ actor+0x151 set: FUN_3911d6c0(0) + FUN_391219c0(0)
│  └─ else: FUN_3911fc80(0, 0)
│
animation event 0x11d ("CleanupGlider")
└─ FUN_39121c10 (870 B) case 0x11d
   ├─ FUN_3911d6c0(0)                          reset stance to 0, play "endloop"
   ├─ FUN_391219c0(0)                          detach slot 0x1a
   └─ actor+0x1a8 = 0                          clear sliding timer
```

Evidence for the event id: the animation-event registration block at `0x390f2e40`–`0x390f2f08`
pushes the string/ID pairs `CleanupGlider 0x11d`, `CleanupBackpackAnim 0x11e`,
`BindHorseStirrup 0x11f`, `SwimDead 0x120` (capstone; strings at `0x399a603c`–`0x399a60c0`).
The 1.2 registration has **no `StartGlider` id 0x11e** (10.x uses 285/286 for
CleanupGlider/StartGlider); in 1.2 deployment is driven by the buff/skill path above, not an
animation event **[R]**. `FUN_3911d6c0` plays `"loop"`/`"endloop"` on the item model and swaps
stance (`param_2=1` -> stance 4; `param_2=0` -> previous stance) **[R]**.

The 10.x-only `SCGlideBuffRemovedPacket` and `X2::GlideBuffFinder` have no 1.2 strings
(`strings.txt` has no class/packet name); 1.2 removes the buff through the ordinary buff
manager and clears the glide block via `FUN_39389d60`/`FUN_39121c10` **[R absence]**.

### 1.2 The 0x50-byte glide block at `actor+0x168`

`FUN_3911fc80` copies exactly 0x50 bytes into `actor+0x168..0x1b7`; `FUN_3912d860` and
`FUN_3911fc80` then read the block through `actor+0x50` as base. The table is the behavioural
map; the source order is the 19-dword stack copy in `FUN_39357920` (0x39357920, offsets
`+0x88..+0xc8` of the skill/buff desc) **[R for offsets, I for names]**.

| actor off | block off | type | meaning | evidence |
|---|---|---|---|---|
| 0x168 | +0x00 | f32 | `gliding_move_speed_slow` | `FUN_3912d860` uses it when elapsed < tier threshold |
| 0x16c | +0x04 | f32 | `gliding_move_speed_normal` / straight-glide speed | `FUN_3912b280` uses it; `FUN_3912d860` normal tier |
| 0x170 | +0x08 | f32 | `gliding_move_speed_fast` | `FUN_3912d860` fast tier |
| 0x174 | +0x0c | f32 | `gliding_rotate_speed` in rad/s | source converts deg -> rad (`* 0x3999cf98`) |
| 0x178 | +0x10 | f32 | `gliding_startup_time` | elapsed (`+0x1ac`) compared with it |
| 0x17c | +0x14 | f32 | `gliding_startup_speed` (sink) | used in the startup branch and copied to `+0x1b8` |
| 0x180 | +0x18 | f32 | `gliding_fall_speed_slow` | slow tier |
| 0x184 | +0x1c | f32 | `gliding_fall_speed_normal` | normal tier |
| 0x188 | +0x20 | f32 | `gliding_fall_speed_fast` | fast tier |
| 0x18c | +0x24 | f32 | `gliding_smooth_time` | denominator of the vertical smoothing filter |
| 0x190 | +0x28 | i32 | `gliding_lift_count` | decremented when a lift is granted |
| 0x194 | +0x2c | f32 | `gliding_lift_height` | downward ray length for the lift check |
| 0x198 | +0x30 | f32 | `gliding_lift_valid_time` | lift only after elapsed >= this |
| 0x19c | +0x34 | f32 | `gliding_lift_duration` | normalised by lift count at start; lift timer |
| 0x1a0 | +0x38 | f32 | `gliding_lift_speed` | upward speed while a lift is active |
| 0x1a4 | +0x3c | f32 | `gliding_land_height` | land ray `+0.2`; same ray in `FUN_39117650` |
| 0x1a8 | +0x40 | f32 | `gliding_sliding_time` | countdown in `CActor::Update`; 0 -> leave state |
| 0x1ac | +0x44 | f32 | runtime: flight elapsed | incremented by dt in `FUN_3912d860` |
| 0x1b0 | +0x48 | f32 | runtime: lift time left | decremented by dt; set to count*duration |
| 0x1b4 | +0x4c | i32 | constant 1000 | copied by `FUN_39357920` |
| 0x1b8 | after | f32 | smoothed vertical velocity | output of the smooth filter |
| 0x1bc | after | f32 | smoothed vertical derivative | output of the smooth filter |

DB evidence for the names/order (examples; values from `buffs`):

| id | move s/n/f | fall s/n/f | rotate | startup s/t | lift count/height/speed/dur/valid | slide/land |
|---|---|---|---|---|---|---|
| 1458 | 6/8/10 | 0.5/1/1.5 | 60 | 5.0/0.7 | 2 / 1.5 / 3.0 / 0.5 / 5.0 | 2.0 / 1.0 |
| 1460 | 4/5/6 | 0.8/1.2/1.6 | 60 | 2.0/0.7 | 3 / 1.5 / 2.0 / 1.0 / 5.0 | 1.0 / 1.0 |
| 2098 | 7/9/11 | 0.3/0.5/1.5 | 60 | 3.0/1.5 | 3 / 1.5 / 5.0 / 0.5 / 5.0 | 1.5 / 1.0 |
| 7206 | 0.1/9/11 | 3.5/0.5/1.2 | 90 | 3.0/2.0 | 3 / 1.5 / 3.0 / 0.5 / 5.0 | 2.0 / 1.0 |

(`SELECT id,gliding_move_speed_*…,gliding_sliding_time,gliding_land_height FROM buffs`,
`compact.sqlite3`; all listed rows have `gliding='t'`, `remove_on_land='t'`.)

### 1.3 Per-frame flight

```
CActor::PrePhysics / Quadruped::PrePhysicsUpdate
└─ FUN_3912dd40(controller, actor)                0x3912dd40 (895 B)
   iVar3 = controller->actor_block = actor+0x50
   if (actor+0x1d0 == 0)                        // not fixed-speed flying
     if (actor+0x1d8 == 0 && actor+0x1da == 0)
       if (actor+0xf8 == 0)                     // not mounted/vehicle
         switch (actor+0x1c1)
           case 1: zero velocity
           case 4: FUN_3912d860()                // GLIDE: lift + fall + steering
           case 5: FUN_3912b280()                // GLIDE: straight at block+4
           default: swim ? FUN_3912ce00() : FUN_3912b330()
       else FUN_3912c770()                      // mounted / vehicle
     else FUN_3912b090()                        // input-driven flight
   else FUN_3912af10()                          // fixed-speed flight
   ... ground probe / slide when actor+0x1d9 ...
```

`FUN_3912d860` (0x3912d860, 1232 B) is the glide controller **[R]**:

1. `actor+0x1ac += dt` (flight elapsed); `actor+0x1b0 -= dt` (lift time left).
2. Speed tier:
   * if elapsed <= `block+0x10` (`startup_time`): horizontal = `block+4` (normal),
     vertical = `-block+0x14` (`startup_speed`, sink);
   * else if `controller+0x10 <= 0.1` (`DAT_3999ccf0`): if `controller+0x10 >= -0.1`
     (`DAT_399a2f24`) use normal/normal, else slow/slow (`block+0`/`block+0x18`);
   * else (vertical input > 0.1) use fast/fast (`block+8`/`block+0x20`).
   The tier also selects a sound id (`DAT_39ac3c98/…ca0/…c9c`) via the game unit vtable
   `+0x130`.
3. Lift: when `block+0x28 > 0`, `elapsed >= block+0x30`, the vertical input is below the
   threshold, and `block+0x48 <= 0`, it raycasts down with length `block+0x2c`; on a hit it sets
   `block+0x48 = block+0x28 * block+0x34` and decrements `block+0x28`.
4. If `block+0x48 > 0`, the vertical target is `-block+0x38` (lift speed, up) and the vertical
   term is scaled by 0.2 (`DAT_3999dd34`).
5. The final vertical velocity (`controller+0x6c`) is smoothed with the filter whose response is
   `block+0x24` (`gliding_smooth_time`), using the same `1 + x + 0.48x² + 0.235x³` denominator
   as remote smoothing (constants `0x399a2998`, `0x399a2994`); horizontal velocity is
   `quaternion * selected move speed` (`controller+0x48` quaternion = `actor+0x2d0`).
6. Outputs: `controller+0x64/+0x68/+0x6c` (velocity), `controller+0x60 = 2` (mode), and the
   smoothed vertical state at `actor+0x1b8/+0x1bc`.

`FUN_3912b280` (0x3912b280, 166 B) sets mode 1 and velocity `quaternion * block+4`; it is the
straight-glide state used while the glider is within `land_height`/sliding. `FUN_3912af10`
(0x3912af10, 370 B) sets mode 2 from the actor normalised direction times
`actor+0x1d4 * 0.001`; `FUN_3912b090` (0x3912b090, 486 B) builds an input-driven direction and
multiplies by `actor+0x430 * pl_flyingVelocityMultiplier` (`game_cfg+0x1dc`) when
`actor+0x1d0 == 0`. These two are the **flying-creature / special-flight** modes; the glider
itself goes through stance 4/5 **[R/I]**.

### 1.4 Landing and water

* `FUN_3911fc80(0, 0)` (0x3911fc80) casts a downward ray with length `block+0x3c + 0.2`;
  a hit sets stance 5 (sliding), no hit keeps stance 4 **[R]**.
* `CActor::Update` `FUN_3911feb0` (0x3911feb0, 1098 B): when stance 5, decrements
  `actor+0x1a8` by dt; when it reaches 0 it calls `FUN_39117530(0)` (stance 0) and the glider is
  done **[R]**.
* `FUN_39117650` (0x39117650, 1083 B) is the falling-into-water/landing effect: when the
  landfall latch `actor+0x854` is clear and `actor+0x161` (falling) or
  `actor+0x138 >= 5.0` (`DAT_3999ccf4`) holds, it raycasts, calls `FUN_390bf210` (splash),
  writes the `"landfall"`/`"speed"` graph inputs via `FUN_391117f0`, and notifies the
  sound/physics unit. It also handles the swim-jump permission (`actor+0x138 * 0.2`).
* `CActor::UpdateFallDamage` `FUN_39116f00` explicitly skips when stance is 2 or 4
  (`+0x1c1 != 2 && != 4`), so a glider landing does **not** generate fall damage **[R]**.

### 1.5 Camera

No glider-specific camera mode exists. `X2::GameClient::CameraManager::Update`
`FUN_3916aab0` still dispatches mode 1 `FUN_39169860` (0x39169860, 2558 B), which follows the
controlled unit through `FUN_390b3870` (view quaternion) and `FUN_3915d6f0`
(0x3915d6f0, 1352 B, pivot/arm). The glider changes only the actor position/stance, so the unit
camera follows the flying actor **[R]**.

One glider-specific position path exists: `FUN_39134440` (0x39134440, 870 B) checks the profile
flag at `DAT_3a5bb1d8+0x94`, obtains the attached item model, asks for bone id **0x2d** and
transforms the actor position by that bone when the unit has one. This is the most likely
glider/camera attachment point (the glider model's `camera` bone) **[I]**.
`models.camera_distance` / `camera_distance_for_wide_angle` (per mount/glider model, loaded by
`FUN_396c7090`) are the only model-side camera distances **[R data, I use]**.

### 1.6 Packets

* No 1.2 glider packet: the client sends the ordinary `CSMoveUnit` (opcode 0x89) built by
  `FUN_394046f0`/`FUN_394045f0`; stance and actor flags in that packet carry the flight state
  (core map §5) **[R absence + core map]**.
* `SCUnitFlyingStateChanged` (opcode 0x7a) is for flying **creatures**, not gliders
  (`re-transport.md` §9); no glider-specific opcode was found **[R]**.
* Buff/skill packets are the ordinary buff add/remove and skill-use packets; the deploy trigger
  itself is the skill controller / backpack item path in 1.1 **[R/I]**.

### 1.7 Glider function inventory

| function | addr | size | role | callers / callees | state |
|---|---|---|---|---|---|
| `X2::GameClient::ClientBuffMan::ApplyAttrs` | 0x39389440 | 1830 | buff apply; starts glider at 0x736056 | -> FUN_39357920, FUN_3911fc80 | stateful |
| `X2::GameClient::ApplyChangedParts` | 0x393220a0 | 1266 | backpack slot change; starts at 0x649085 | -> FUN_39357b90, FUN_396f63d0, FUN_39357920 | stateful |
| glider start | 0x39357920 | 610 | build block + start | -> FUN_3911fc80, FUN_391219c0; callers 393220a0, 39389440 | stateful |
| `FUN_39357b90` | 0x39357b90 | 48 | backpack item gate (slot 0x1a) | caller 393220a0 | pure-ish |
| `FUN_396f63d0` | 0x396f63d0 | 62 | item desc lookup | caller 393220a0 | stateful |
| `FUN_3911fc80` | 0x3911fc80 | 551 | set/copy glide block, choose stance 4/5 | callers 39357920, 39389d60 | stateful |
| `FUN_3911feb0` `CActor::Update` | 0x3911feb0 | 1098 | sliding timer, per-frame actor update | caller 391387c0 | stateful |
| `FUN_3912dd40` | 0x3912dd40 | 895 | movement mode dispatch (glide/mount/swim/ground) | caller 39129800 (Quadruped); player tail copy in `CPlayerMovement` | stateful |
| `FUN_3912d860` | 0x3912d860 | 1232 | glide: lift/fall/steering/smoothing | caller 3912dd40 | stateful |
| `FUN_3912b280` | 0x3912b280 | 166 | glide straight at block+4 | caller 3912dd40 | stateful |
| `FUN_3912af10` | 0x3912af10 | 370 | fixed-speed flight | caller 3912dd40 | stateful |
| `FUN_3912b090` | 0x3912b090 | 486 | input flight (`pl_flyingVelocityMultiplier`) | caller 3912dd40 | stateful |
| `FUN_3912b330` | 0x3912b330 | 3199 | ground + jump + `jumpVec` | caller 3912dd40 | stateful |
| `FUN_3912c770` | 0x3912c770 | 1678 | mounted/vehicle movement | caller 3912dd40 | stateful |
| `FUN_3912ce00` | 0x3912ce00 | 2648 | swim control | caller 3912dd40 | stateful |
| `FUN_3912bfb0` | 0x3912bfb0 | 1670 | ground align/final velocity | caller 3912dd40 | stateful |
| `FUN_3911d6c0` | 0x3911d6c0 | 330 | set/restore ride stance, loop/endloop | callers 39121b90/39121c10/39362d20/39389d60 | stateful |
| `FUN_391219c0` | 0x391219c0 | 308 | attach/detach item slot 0x1a (glider/backpack model) | callers 39121b90/39121c10/39138a40/39357920/39362d20/39389d60 | stateful |
| `FUN_39121c10` | 0x39121c10 | 870 | animation event dispatch (0x11d CleanupGlider) | callers 39126e40/39129310/39132710 | stateful |
| `FUN_39117650` | 0x39117650 | 1083 | water landing / landfall effect / swim-jump gate | caller 3911c970 | stateful |
| `FUN_39134440` | 0x39134440 | 870 | bone-based position (bone 0x2d) | no static caller (virtual) | read-only |
| `X2::GameClient::CameraManager` mode 1 | 0x39169860 | 2558 | unit camera | caller 3916aab0 | stateful |
| `FUN_3915d6f0` | 0x3915d6f0 | 1352 | camera pivot/arm | caller 39169860 | stateful |
| cvar registration | 0x390f7b40 | 12489 | `gliding_mouse_ad/ws`, `glider_*`, `pl_flyingVelocityMultiplier` | registration function | stateful init |

---

## 2. Swimming and diving (remaining details)

### 2.1 State update

`CActor::UpdateSwimStats` `FUN_39120300` (0x39120300, 2072 B) is called from
`CActor::UpdateStats`/actor update; the existing core map covers the water level and events. The
parts needed for the port:

* It reads the entity world position (`entity vtable +0x98`, `FUN_3911da0` bottom ray) and keeps
  the previous world position at `actor+0x830..0x838` (`param_1[0x20c..0x20e]`); a fast move
  (> `DAT_399a321c`) or a forced update re-runs the water probe **[R]**.
* Water level is `actor+0x84` (float, `param_1[0x21]`); in-water is `actor+0x1f` (byte); the
  signed water timer is `actor+0x7c` (`param_1[0x1f]`) and the surface timer is `actor+0x80`
  (`param_1[0x20]`); the water-crossing events are fired through `actor vtable +0x184`
  event ids 10 (entered water) and 11 (left water) at the crossing, and 12 for the breath bar
  direction (`re-movement.md` §2.3) **[R]**.
* It calls `FUN_39111da0` (0x39111da0, 180 B) to update the bottom-level ray, and
  `FUN_3911f3f0` (0x3911f3f0, 489 B) for the swimming effects when dead.
* On an upward surface crossing it plays `"jump_splash"` through the actor event vtable
  (`FUN_39120300` at the `jump_splash` branch) **[R]**.
* `FUN_3911c970` (0x3911c970, 184 B) is the entry/exit selector: it maps
  `(inWater, deep, ...)` to state 0/1/2/3, calls `FUN_39117530(0)` when leaving state 3 and
  `FUN_39117650` when state 2 is entered; `FUN_3911c8a0` (0x3911c8a0, 205 B) handles the
  upward/landing transition and `FUN_3911c780` (0x3911c780, 274 B) the fall start **[R]**.

### 2.2 Swim control

`FUN_3912ce00` (0x3912ce00, 2648 B) is called by the movement dispatch while swimming. It:

* copies the controlled unit's axis values (movement request fields `controller+0xc/+0x10/+0x14`)
  into a world direction, applies the camera/actor quaternion (`controller+0x48`) and the
  swim multipliers, then integrates buoyancy/drag into `controller+0x8c..0x94` and
  `controller+0x64..0x6c` **[R]**;
* reads `swim_back_speed_mul` at `game_cfg+0x15c` and `swim_side_speed_mul` at `game_cfg+0x160`
  (registration lines in `FUN_390f7b40`; defaults `0.8` and `0.6` read from
  `0x399a1d94` / `0x399a24cc`) **[R]**;
* reads zero-G/fly multipliers at `game_cfg+0x194/+0x198/+0x19c/+0x1a0/+0x1b8/+0x1bc`
  (registration in `FUN_390f7b40`; `pl_zeroGSpeedMult*`) **[R]**;
* writes the animation graph inputs `SwimControlX`, `SwimControlY`, `SwimControlZ` by name
  through `FUN_39131ea0`/`FUN_39131f10` (the same input helper used by `CPlayerMovement`) **[R]**.

The up/down/buoy multiplier cvars (`+0x164/+0x168/+0x16c`) are registered but **no direct
`DAT_39aefb98 + offset` read was found** in x2game; they may be applied in cryphysics or through
another config object. Marked U-S1. The values are `0.7 / 0.5 / 2.0` (`0x399a87e8`,
`0x3999c47c`, `0x3999cffc`) **[R data, ? use]**.

### 2.3 Breath

| function | addr | size | role |
|---|---|---|---|
| `X2Player.GetBreathTime` target | 0x3930b660 | 246 | `(capacity,left)` from `player+0xb24` start and `+0xb28` capacity; rescales on capacity change |
| dive start/end | 0x3930b600 | 91 | start=`now`, capacity=unit attribute 0x5b; end start=0 |
| server correction | 0x3930b050 | 43 | `start = now - capacity + breath` |
| jump/dive key check | 0x3930b5b0 | 77 | true when jump axis (`FUN_39424580(6)`) or `unit+0x54 > 0.2` |

`FUN_3930b660` is bound from `FUN_39495d10` (0x39495d10) and called by the Lua
`GetBreathTime`; the server correction `FUN_3930b050` is called from the breath packet handler
`FUN_391ddf30`; `FUN_3930b600` is called from `FUN_391dde80`/`FUN_39369c10` **[R from
calls.tsv]**. The UI side (`player_breath_bar.alb`, `DIVE_START/END`) is unchanged from
`re-movement.md` §2.3 and is not re-mapped here.

### 2.4 Swimming jump

`FUN_3930b5b0` (0x3930b5b0, 77 B) is the permission gate for a surface jump: true when the jump
action is held or the unit is more than `0.2` (`DAT_3999dd34`) above its support. This is the
`swim_jump_permission_range` constant from the registration (`game_cfg+0x274`, default
`0x3999dd34=0.2`) **[R]**. The actual vertical impulse is applied by the shared jump/physics
path (section 4); `swim_jump_speed` `12.0` (`0x3999f9d0`, registration `+0x270`) and
`swim_jump_end_depth` `0.2` (`+0x278`) have **no direct read site in x2game** (U-S2).

### 2.5 Current / flow

No client-side water current/flow variable or helper was found in x2game or the engine
decompiles: `strings.txt` contains no `water_flow`/`current_speed`/flow-field symbol used by
movement, and `UpdateSwimStats` has no horizontal advection term. Water movement is therefore
either server/physics-driven or absent in 1.2 (U-S3) **[R absence + I]**.

### 2.6 Fall start, landing, damage

| function | addr | size | role |
|---|---|---|---|
| `CActor::UpdateStats` | 0x3911eff0 | 1020 | calls the water/fall sub-state functions (`3911c560/c970/c8a0/c780`) |
| fall start | 0x3911c780 | 274 | if `+0x1c1==0`, `+0xf8==0`, `+0x874<1`, fall speed `+0x138 >= pl_fall_start_velocity` (`game_cfg+0x174`, default 10) and ground ray gap > `pl_fall_start_height` (`+0x170`, default 1.0), call `FUN_39117530(1)` and start the fall |
| fall height/speed tracker | 0x39117590 | 190 | when vertical velocity is negative, records the start height `+0x140` and the maximum downward speed `+0x13c` |
| fall damage | 0x39116f00 | 609 | landing timer `+0x148`; if fall speed `+0x13c >= pl_fallDamage_SpeedSafe` (`game_cfg+0x178`, default 20) and not stance 2/4 and `+0x874<1`: `impact = sqrt(max(0, startHeight - groundZ) * 40.0)` (`0x3999f668=40`), clamp to current fall speed, store at `+0x144`, clear `+0x13c/+0x140` |
| underwater/bottom | 0x39116d20 | 475 | water/ground ray under the actor; sets `+0x152` and disables physics when below the bottom |
| water landing | 0x39117650 | 1083 | splash/landfall/speed input (also section 1.4) |

Constants: `pl_fall_start_height=1.0` (`0x3999cffc`), `pl_fall_start_velocity=10.0`
(`0x3999cce8`), `pl_fallDamage_SpeedSafe=20.0` (`0x3999f8f0`), `pl_fallDamage_SpeedFatal`
(`0x399a8584`), `pl_fallDamage_SpeedBias` (`0x3999c478=1.5`), all registered in
`FUN_390f7b40` and read at `game_cfg+0x170/+0x174/+0x178/+0x17c/+0x188` **[R]**. The client
does not apply damage; `+0x144` is what the movement packet later reports (`fall_vel`).

---

## 3. Click-to-move, auto-run, follow, move-to-point

### 3.1 Click-to-move

```
MouseClickControl::Normal                 FUN_390f4960 (715 B)  profiler string
└─ FUN_390a6af0(clickType, unit1, unit2, param4)   0x390a6af0 (1848 B)
   ├─ reject dead / mounted / vehicle / using states (player+0x3a70.., +0x3b00, +0x3b94)
   ├─ camera ray: FUN_390b3850 (position), FUN_390b3870 (quaternion)
   ├─ RayWorldIntersection(Game), mask 0x117/0x2048f
   ├─ target filters: unit type 4/2 reject; FUN_390b4fa0, FUN_39096a10
   ├─ if a unit target: FUN_39095e70(targetId, target+0x1f, param4)
   └─ if ground: FUN_390ca3d0 (create/restart path controller)
      └─ FUN_390b2af0 (0x1a0-byte path follower) -> FUN_3902f390
      if cvar x2_effects.ui.click_to_move (!=0):
        FUN_390a58b0(queue, point, 0x400000)       0x390a58b0 (1094 B)
```

`FUN_390a6af0` reads the cvar by name from the config object
(`"x2_effects.ui.click_to_move"`, `0x399a3180`; registered through `FUN_393fff...` not chased)
and only then enqueues the move **[R]**. `FUN_39095e70` is the “set movement target” write:
`this+0x2c = targetId`, `this+0x30/+0x34 = args`, then it primes the controller and calls
`FUN_391130c0(0)` **[R]**. `FUN_390ca3d0` (242 B) allocates/reuses the path controller
(`FUN_390b2af0`, 504 B) and calls `FUN_390ca140`; it is also called from `FUN_39393920` (doodad
interaction) **[R]**. Cancelling on keyboard input is not in `FUN_390a6af0`; the core input path
clears the target in `FUN_39404d50` (U-C1).

### 3.2 Auto-run

The auto-run latch is in the core input state (`FUN_39404d50`, 0x39404d50) and the send pump:
`FUN_39406650` (0x39406650, 228 B) consumes bits 0x1/0x2000 and calls the controlled unit;
`FUN_39406740` (0x39406740, 307 B) calls it for the local player and the five extra input slots
(core map §2.5/§5.1). The cvar `autorun` is a binding action (string `0x399d5bec`), not a
separate movement function. Reuse the core port; nothing new to port here **[R reuse]**.

### 3.3 Move-to-point used by interactions

`FUN_393ba480` (0x393ba480, 1340 B) builds a movement request from a world point:

* resolves the controlled unit (`FUN_390b3750()->vtable 0x208`), gets its movement object
  (`vtable 0xb0`) and calls `FUN_390a8dc0` to default the request **[R]**;
* normalises the direction, applies the camera quaternion (`FUN_390b3750()->vtable 0x228`),
  computes the remaining distance and uses the constant `DAT_3999c478` = **1.5** (the
  “pseudoSpeed 1.5” noted in the task) **[R data]**;
* sets the request flags `0x800` and calls `FUN_39097bd0`/`FUN_39098380`, then submits the
  request through the unit movement vtable and `FUN_390a5d00` **[R]**.

Callers: `FUN_393b31e0` (30 B wrapper) from `FUN_393b3ec0` (787 B, interaction approach) and
`FUN_393bfc70` (talk). `FUN_393b3ec0` first tries `FUN_390a6af0` (click move) at
`0x7707xx`; if the target is already within interaction range it calls
`FUN_393b31e0` -> `FUN_393ba480`, then `FUN_39429e40` and `FUN_393b2ab0` **[R]**.

### 3.4 Follow

`followUnit` / `GO_TO_FOLLOW_UNIT` are the **AI** controller `FUN_3933c8f0` (0x3933c8f0,
624 B), not a player controller: it sets the AI state `GO_TO_FOLLOW_UNIT` and stores the
followed unit id in the AI object (`+0x34`), calling `FUN_390242c0("followUnit", …)` and
`FUN_3933b070(0,"GO_TO_FOLLOW_UNIT",0)` **[R]**. The player-side follow action
(`TOGGLE_FOLLOW` 0x399be05c, `toggle_follow_target` 0x399d5784, errors 495–500
`cannot_follow_*`) has **no local-player movement function**; it is server-authoritative or UI
only (U-C2). The client can be tested by sending the same follow command; no client port is
proposed.

---

## 4. Jump

### 4.1 Input and state

* Action index 6 is `jump` (`0x39ac8e68` table, core map §2.1); `FUN_39424580(6)` is read in
  `FUN_390a6af0`/`FUN_3930b5b0` and in the “jump held” checks **[R]**.
* `FUN_3912b330` (0x3912b330, 3199 B) contains the jump impulse, in the `controller+0x31`
  (“jump requested”) branch and only if `controller+0x98 == 0` (not already airborne) **[R]**:
  1. if horizontal input is non-zero and `actor+0x5c > 0` (movement block byte/float at
     actor+0x5c), set movement mode 7 (`controller+0x60 = 7`) and build a jump vector from
     `actor+0x9c..0xa4` (direction) and `actor+0xd4..0xdc` (external impulse);
  2. the vertical speed uses `sqrt(2 * directionMagnitude * actor+0x470) / directionMagnitude`
     minus `0.5` when the direction is non-zero; `actor+0x470` is the `game_jump_height` field
     copied by `FUN_3911b950` (0x3911b950) from `actor_models.game_jump_height` (model 1:
     1.3) **[R/I]**;
  3. it sets `controller+0x98 = 1` (airborne) and `+0x99 = 1` (“jumped” event).
  4. with no horizontal input it either skips if the current animation contains
     `"jump_in_place"` or sets `controller+0x98 = 0x101` and calls the unit vtable `+0x2a8(0)`.
* `FUN_3912ac50` (0x3912ac50, 703 B) copies the request to the graph and fires
  `actor vtable +0x184("jumped", 0, 0)` when `controller+0x99` is set; it also writes the
  velocity/rotation/airborne fields into the actor **[R]**.
* `FUN_3912bfb0` (0x3912bfb0, 1670 B) is the final ground-align/velocity pass; it is called by
  `FUN_3912dd40` after the mode function and cancels horizontal velocity when the unit is not
  allowed to move **[R]**.
* `FUN_3914ae50` (0x3914ae50, 856 B) registers the “SpecialMove 0 = Jump, 1 = SpeedSprint,
  2 = StrengthJump” trigger data (`strings.txt` `0x399ae3c4`); it is data/trigger plumbing, not
  the impulse **[R]**.

The literal `IPhysicalEntity::Action(action_move)` with `iJump` is still not in x2game: the
movement is submitted through the animated-character/entity path (`controller+0x64..0x6c`,
`FUN_3912ac50`), and cryphysics `CLivingEntity::Action` 0x35030E90 is only reached through the
entity vtable (core map U1). This map resolves the impulse *computation*, not the engine call;
U9 is therefore “partially resolved” **[R/I]**.

### 4.2 Air control/momentum, double/mount jump

* Air momentum is the same request path: `FUN_3912b330` keeps horizontal `jumpVec` and
  `FUN_3912bfb0`/`FUN_3912ac50` write it to the actor velocity; the model’s
  `physics_living_k_air_control=0.9`, `air_resistance=0.29` are consumed by cryphysics
  (`actor_models` row 1) **[R data, I physics]**.
* Double jump / glider deploy is a *skill* (`glider_start_with_double_jump` cvar default 0 at
  `game_cfg+0x2ac`, registration `FUN_390f7b40`); it is not a second impulse in the movement
  code. The second jump with a glider equipped deploys the glider through the item/skill path
  (section 1.1); no double-jump field exists in `FUN_3912b330` **[R]**.
* Mount jump is the same input and the same `jumpMP01` action on the mount graph
  (`re-mounts-full.md` §3/§6); the controlled actor is the mate, so `FUN_3912b330` runs on the
  mate’s `CPlayerMovement` **[R/I]**.

---

## 5. Teleports and portals (movement/camera side)

### 5.1 Packet handlers

| handler | addr | size | packet / role |
|---|---|---|---|
| `OnTeleportUnit` | 0x391d6ed0 | 370 | `SCTeleportUnit`: (type, objId, pos+yaw) -> `FUN_3930c9e0`; type 2 sends opcode 0x6e back; sets global teleport flags at `GameClient+0x109a/0x10a0/0x10a4` |
| `OnAboxTeleport` | 0x391dc5a0 | 246 | `SCAboxTeleport`: position from current pos minus 0x400*axis, ground Z + 10.0; relocates the local unit via `vtable+0x44` |
| `OnNaviTeleport` | 0x391dc8c0 | 138 | `SCNaviTeleport`: `FUN_3930c9e0(0, 0, pos, 0)` |
| `OnUnitPortalUsed` | 0x391d3970 | 122 | `SCUnitPortalUsed`: `FUN_393558e0` (portal effect) |
| portal book | 0x391d3760 / 0x391d3860 / 0x391d38e0 | 118 / 114 / 144 | `OnCharacterPortals` / `OnPortalSaved` / `OnPortalDeleted` (UI data only) |

### 5.2 Position/camera reset

`FUN_3930c9e0` (0x3930c9e0, 378 B) is the movement/camera reset **[R]**:

1. if the “type” argument is non-zero, it calls `FUN_39428370(type)` (UI/error path) and
   returns;
2. otherwise, if the local player and unit exist, it teleports the unit by writing the
   supplied position/quaternion (`vtable+0x44`), sets the camera yaw from the yaw argument
   (`cos/sin` of `yaw * 0.5`? — angles `DAT_3999c47c=0.5` and `fsin/fcos`), calls
   `FUN_3915e930(0)` (reset camera yaw offset `camera+0x84`), `FUN_3915af10` (reset FOV blend
   `camera+0x1f8/+0x1fc`), clears the input hold flags at `DAT_39aff9d0+0xc/+0xd`, calls
   `FUN_391a4e00` (send `0xb4` position/quaternion notify), and `FUN_39171800(3)` (screen
   fade/loading transition);
3. if the local player is missing it calls `FUN_391a4e00` only.

`FUN_391a4e00` (0x391a4e00, 270 B) builds the position/rotation notification from
`FUN_390b3850`/`FUN_390b3870` and sends packet `0xb4` via `FUN_397ae690`; it clears
`GameClient+0x10a0` **[R]**. `FUN_39171800` (0x39171800, 425 B) is the loading/fade state
machine (calls `FUN_39016c40`, `FUN_39039c20`, `FUN_39052670`, `FUN_39179d80`, `FUN_39186810`,
`FUN_391eede0`, `FUN_39330470`, `FUN_39342870`) **[R]**.

### 5.3 Safe-zone teleport

`X2::GameClient::PhysicsManager::UpdateSafeZoneTeleporting` `FUN_39181cc0` (285 B) dequeues up
to 16 pending safe-zone entries (`+0x24 + index*0x14`) when the timestamp allows and calls
`ProcessSafeZoneTeleporting` `FUN_39181a20` (660 B). `FUN_39181a20` checks the distance to the
target (`DAT_399b2cf0`), resolves the unit, tests the `"safe_zone"` world position
(`FUN_39181660`), and writes the safe-zone position through the client vtable `+0x44` **[R]**.
The portal UI/book (`FUN_394cb5e0`, `FUN_394cad40`, `FUN_394c0680`) is a later system; only the
movement/camera side above is in scope.

---

## 6. Structs to add to the shared movement module

Keep the core `InputState`/`MovementRequest` from `port-movement/structs.md`. Add:

```rust
#[repr(C)]
pub struct GlideParams {          // actor+0x168, 0x50 bytes; FUN_3911fc80 copies it
    pub move_slow: f32,           // +0x00
    pub move_normal: f32,         // +0x04
    pub move_fast: f32,           // +0x08
    pub rotate_speed: f32,        // +0x0c  rad/s
    pub startup_time: f32,        // +0x10
    pub startup_speed: f32,       // +0x14  sink speed
    pub fall_slow: f32,           // +0x18
    pub fall_normal: f32,         // +0x1c
    pub fall_fast: f32,           // +0x20
    pub smooth_time: f32,         // +0x24
    pub lift_count: i32,          // +0x28
    pub lift_height: f32,         // +0x2c
    pub lift_valid_time: f32,     // +0x30
    pub lift_duration: f32,       // +0x34
    pub lift_speed: f32,          // +0x38
    pub land_height: f32,         // +0x3c
    pub sliding_time: f32,        // +0x40
    pub elapsed: f32,             // +0x44  runtime
    pub lift_left: f32,           // +0x48  runtime
    pub const_1000: i32,          // +0x4c
    // followed by actor+0x1b8/0x1bc vertical smoothing state
}
```

The movement controller used by the animated character (call it `PlayerMovement`) is the object
constructed by `FUN_3a6ab5f3` (0x3a6ab5f3) and `FUN_3912e0c0` (0x3912e0c0): fields
`+0x00 mode`, `+0x04 actor_movement_params (actor+0x3ec)`, `+0x08 actor_input (actor+0x50)`,
`+0x0c..+0x40 request copy`, `+0x40 actor`, `+0x44 actor+0x2b0`, `+0x48 actor+0x2d0` (view
quaternion), `+0x4c actor+0x118`, `+0x60 movement mode`, `+0x64/+0x68/+0x6c velocity`,
`+0x98 airborne`, `+0x99 jumped`, `+0x9c?`. Confirm the full layout with `port-movement/structs.md`
before defining it in Rust.

---

## 7. Differential-test plan (in-process, `aadriver.dll`)

Conventions and the driver `call` command are the same as `port-movement/difftest.md`:
`__thiscall` is `__fastcall` with a dummy EDX, x87 `float10` is declared `float`, and all state
is read from the live client. Resolve VAs from
`GetModuleHandleA("x2game.dll") + (va - 0x38FF0000)`.

### 7.1 Pure/leaf tests

1. **Glide block decode** - call `FUN_39357920` on a live player with a synthetic buff desc
   (the 19 dwords at desc+0x88..+0xc8) and compare the 0x50 stack block it passes to
   `FUN_3911fc80`. This proves the field order and the `deg -> rad` conversion
   (`0x3999cf98`) without touching the actor. Then decode the block in Rust and compare
   field-for-field.
2. **Tier selection** - port `FUN_3912d860`'s tier switch (startup / slow / normal / fast) as a
   pure helper and compare against the original on a matrix of
   `(elapsed, vertical_input, block values)` by calling `FUN_3912d860` with a synthetic
   movement object.
3. **Pseudo-speed 1.5** - call `FUN_393ba480` with a live unit at a fixed point and hook
   `FUN_390a8dc0`/`FUN_390a5d00` to capture the request; compare the `1.5`
   (`DAT_3999c478`) and flags.

### 7.2 Glider

* **Start/fold**: on a live actor with a real glider item, call
  `FUN_3911fc80(1, block)` with a synthetic 0x50 block; read back `actor+0x168..0x1bc` and
  stance `+0x1c1`; then call `FUN_3911fc80(0, 0)` and compare the landing ray result
  (stance 4/5). Restore the actor stance afterwards.
* **Flight**: build the `PlayerMovement` context by hand (fields above) around the live actor,
  call `FUN_3912d860(context)` for a matrix of `elapsed`, `block+0x28` (lift count), vertical
  input and `block+0x24` (smooth time); compare `context+0x64/+0x68/+0x6c` and
  `actor+0x1b8/+0x1bc`. The raycast needs real world geometry; run at a flat spot and
  separately over water.
* **Straight glide**: same, `FUN_3912b280(context)`; compare velocity and `context+0x60`.
* **Dispatcher**: call `FUN_3912dd40(context, actor)` with the actor stance set to 1/4/5 and
  `actor+0xf8/0x1d0/0x1d8` toggled; compare which mode function ran (hook the five callees) and
  the final `context+0x60..0x6c`.
* **Lift**: set `block+0x28=2`, `+0x30=0`, `+0x34=0.5`, `+0x38=3.0`, a ground plane, and verify
  the two lifts, the `block+0x48` timer and the upward velocity; then prove that no third lift
  occurs.
* **Landing / no fall damage**: call `FUN_39116f00` with stance 4 and a large `+0x13c`; assert it
  does **not** write `+0x144`.
* Hook `FUN_39131ea0` to capture the animation inputs (`"landfall"`, `"speed"`,
  `"SwimControl*"`, `"jumpVec"`).

### 7.3 Swim / fall

* `FUN_39120300` on a live actor at the water surface, then below it; compare water level
  `+0x84`, `+0x7c`, `+0x80`, the event ids and the breath timer. Use the real world water or a
  controlled test spot.
* `FUN_3912ce00` with a synthetic movement context and actor; compare the velocity and the
  `SwimControlX/Y/Z` values captured at `FUN_39131ea0`.
* `FUN_39117590` / `FUN_39116f00` with a synthetic actor: set `+0x13c`, `+0x140`, `+0x148`,
  stance and ground Z; compare `+0x144` and the clears. Cover `drop=0`, `drop=1.3`,
  `drop=20`, safe speed exactly 20, stance 2/4, and `+0x874>=1`.
* `FUN_39117650` at a water surface: compare the `"landfall"`/`"speed"` inputs and the
  splash event.

### 7.4 Jump

* `FUN_3912b330` with the synthetic movement context and live actor:
  - jump key held, no horizontal input -> `+0x98=0x101` / no vector;
  - forward + jump -> mode 7, `+0x60`, velocity `+0x64..0x6c`; compare with the Rust formula;
  - `actor+0x470` (jump height) matrix 0.5/1.3/2.0;
  - animation action containing `jump_in_place` -> the skip branch;
  - `FUN_3912ac50` then fires `"jumped"` exactly once.
* Air control: hold a direction after the impulse, call `FUN_3912dd40` repeatedly, compare the
  horizontal velocity decay with the original.

### 7.5 Click-to-move / teleport

* `FUN_390a6af0` with the live camera and a controlled click; hook `FUN_390a58b0` and
  `FUN_39095e70` to capture the queue/target; compare with the Rust click-to-move.
* `FUN_393ba480(unit, point)` with a live unit; capture the request at `FUN_390a5d00`; compare
  flags, direction, `1.5`, and the final position after a few frames.
* `FUN_3930c9e0(0, 0, pos, yaw)` on the live local player; compare the actor position, the
  camera manager fields `+0x84` (yaw offset), `+0x1f8/+0x1fc` (FOV blend), and the
  `DAT_39aff9d0+0xc/+0xd` flags. Do not use type 2 (it sends opcode 0x6e).
* `FUN_39181a20` with a synthetic safe-zone entry (position + target id) and a live world;
  compare the resulting position write.

---

## 8. Proposed port order (testable leaves first)

Phase A - data and pure helpers:

1. `GlideParams` layout + `from_skill_controller` (replicating `FUN_39357920`'s 19-dword copy,
   deg->rad, ms->s). Test: block bytes, not the actor.
2. Glide tier selection / speed constants extracted from `FUN_3912d860`.
3. `1.5` pseudo-speed and request defaults from `FUN_393ba480` / `FUN_390a8dc0`.
4. Fall/land predicates: `FUN_39117590`, `FUN_39116f00` arithmetic, `FUN_3911c780` gates.
5. Jump impulse arithmetic (`FUN_3912b330` branch) as a pure helper.

Phase B - glider state machine:

6. `FUN_3911fc80` start/stop (copy block, stance 4/5, timer init).
7. `FUN_3911d6c0`/`FUN_391219c0` attach/detach + `FUN_39121c10` event 0x11d.
8. `FUN_3912b280`, `FUN_3912d860`, `FUN_3912af10`, `FUN_3912b090` and the `FUN_3912dd40`
   dispatch. Diff each mode against a synthetic movement context.
9. `FUN_3911feb0` sliding timer and `FUN_39117650` water landing.

Phase C - swim/fall:

10. `FUN_39120300` state update + `FUN_3912ce00` control + swim graph inputs.
11. Breath functions `FUN_3930b660`/`b600`/`b050` (already mirrored in Bevy `hud/breath.rs`,
    but port the native arithmetic for the oracle).

Phase D - jump:

12. `FUN_3912b330` jump branch + `FUN_3912ac50` "jumped" event + `FUN_3912bfb0` final pass.

Phase E - click-to-move / move-to-point / follow:

13. `FUN_39095e70` target setter, `FUN_390ca3d0`/`FUN_390b2af0` path controller,
    `FUN_390a6af0` + `FUN_390a58b0`, `FUN_393ba480`.
14. Follow: no player port; document the server/AI boundary.

Phase F - teleport/portal camera:

15. `FUN_3930c9e0` + `FUN_391a4e00` + `FUN_3915e930`/`FUN_3915af10`.
16. `FUN_39181cc0`/`FUN_39181a20` safe-zone.
17. Portal UI is a later system.

Rationale: every phase-A item is a byte/number comparison with no frame dependency; each later
phase only uses the state produced by the previous one, and the dispatcher is the integration
point where all mode leaves become testable at once.

---

## 9. Unknowns

| id | unknown | next probe |
|---|---|---|
| U-G1 | The 1.2 `skill_controllers` row 2687 is not in the reference DB; the source field order of the glide block comes from usage. | Dump the live desc at `FUN_39357920` entry and compare with the DB buff row; the block bytes are the oracle. |
| U-G2 | `controller+0x10` in `FUN_3912d860` (vertical input / W-S pitch) is not named. | Sweep the value and observe the tier; correlate with `gliding_mouse_ws`. |
| U-G3 | Whether boost skills re-copy the block (`FUN_39389440` -> `FUN_39357920`) or only change a buff attribute. | Breakpoint on `FUN_3911fc80` while using boost 13435/23040. |
| U-G4 | Bone `0x2d` in `FUN_39134440` is the likely glider camera bone; not proven. | Call it with/without the glider and read the returned point. |
| U-S1 | Up/down/buoy swim cvars (`+0x164/+0x168/+0x16c`) have no x2game read site. | Check whether cryphysics/AI consumes them; or dump the movement request on a dive. |
| U-S2 | `swim_jump_speed`/`end_depth` (`+0x270/+0x278`) read site not found. | Breakpoint on `CLivingEntity::Action` during a surface jump. |
| U-S3 | No client water current/flow. | Confirm the server sends the position; no port expected. |
| U-F1 | Fall start uses `pl_fall_start_velocity` and `pl_fall_start_height`; the exact relation (which one is compared to what) is literal in `FUN_3911c780` but not yet golden. | Matrix test on `FUN_3911c780` with synthetic fields. |
| U-J1 | Jump impulse formula and whether cryphysics adds its own `iJump`; `actor+0x470` is jump height, gravity is not read in x2game. | Hook `CLivingEntity::Action` during a jump; compare `iJump` and the request velocity. |
| U-C1 | Click-to-move cancellation is not in `FUN_390a6af0`. | Hook `FUN_390a58b0`/target controller while pressing W. |
| U-C2 | Player follow: no local movement function. | Check the follow packet/opcode and whether the server moves the player. |
| U-T1 | `FUN_3930c9e0` parameter order/typing (`type, objId, pos, yaw`) is inferred from `OnTeleportUnit`. | Call with a captured `SCTeleportUnit` body and compare the actor pose. |

---

## 10. Cross-check against the existing notes

* The swim cvars and defaults in `re-movement.md` §2.3 and `re-constants.md` are confirmed:
  `swim_side_speed_mul=0.6` at `0x399a24cc`, `swim_back=0.8`, `up=0.7`, `down=0.5`,
  `buoy=2.0`, `jump=12.0`, `permission/end=0.2` (registration in `FUN_390f7b40`).
* The fall-damage formula `sqrt(40 * drop)` and the 20 m/s safe speed in `re-movement.md` §2.2
  are confirmed; the glider/swim exemption is new.
* The click-to-move gate `x2_effects.ui.click_to_move` and the raycast in `FUN_390a6af0` are
  confirmed; the queue function is now identified as `FUN_390a58b0`.
* The glider per-frame integrator, previously “not located” (`re-transport.md` §10.3,
  `re-movement.md` §6), is `FUN_3912d860` + `FUN_3912b280` with the block at actor+0x168.
* `port-movement/MAP.md` §1.3 calls stance 4 “quadruped/ mount”; the dispatcher evidence here
  shows 4 is also the glide state and the mounted branch is `actor+0xf8`. Re-check the mount
  branch before porting the turn formula.
* The core map's “no local jump impulse” (U9) is narrowed: the impulse is computed in
  `FUN_3912b330`, but the physics `action_move.iJump` submit is still not in x2game.
