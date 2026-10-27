# Port map: the ArcheAge 1.2 vehicle and ship code (S3)

> **Restored 2026-10-27 from `archive/fork-main-2026-10-27`, UNVERIFIED.** The client was restarted from NickMesser's `main`; nothing described here is wired into the game. Addresses, call chains and offsets must be re-checked against the decompile ([R]/[I]/[?]) before use. A 2026-10-27 audit found 151 of the 269 EXACT/TOLERANCE labels in `proof-levels.*` weak or wrong (report: `D:\aaemu-client\handoff\2026-10-27\audit\report-proofs.md`).

Address map for a function-by-function Rust port of the 1.2 client's **vehicle, ship and transfer**
logic (system S3 of `docs/research/port-program.md`), with a differential-test plan that calls the
original functions inside the running hidden 1.2 client (`aadriver.dll`).

The physics engine itself (`cryphysics`'s living entity and vehicle/wheeled bodies) is already
ported bit-exact in `C:/Users/stsiakal/AAEmu/rust/crates/aaemu-physics`. This document maps **only**
the x2game / CryAction side that *drives* it and reads it back.

---

## 0. Sources, conventions and licensing

### 0.1 Licensing

Everything here is derived **only** from the shipped 1.2 binaries and their decompiles. The
published CRYENGINE source was **not** used, quoted or consulted (same rule as
`C:/Users/stsiakal/AAEmu/rust/docs/physics/CRYPHYSICS-MAP.md` §Licensing).

Class names in `X2::`/`C` form come from the binaries' own RTTI, profiler scope strings
(`FUN_39004bb0`), log/assert strings or cvar names. Behaviour that has no name in the binary is
marked **(unnamed)**; a meaning inferred from behaviour is marked **(inferred)**.

Port from this document and the decompiles, never from the CRYENGINE source.

### 0.2 Sources

| source | path |
|---|---|
| 1.2 x2game decompile | `C:/AAEMU_DISK/aa-re/client12/out-x2game/x2game.dll.c` |
| 1.2 x2game functions/calls/strings | `C:/AAEMU_DISK/aa-re/client12/out-x2game/{funcs.csv,calls.tsv,strings.txt,strings-xrefs.tsv}` |
| 1.2 x2game image (constants) | `D:/aa-re/client12/x2game_unpacked_data.dll`, base `0x38FF0000` |
| 1.2 x2game raw image (disassembly) | `D:/aa-re/client12/x2game_unpacked.dll`, base `0x38FF0000` |
| 1.2 engine decompiles | `D:/aa-re/client12/engine/out/`, `D:/aa-re/client12/engine2/out/` |
| 1.2 cryphysics | `D:/aa-re/cryphysics/x86_12/out/`, ported by `aaemu-physics` |
| 10.x names (naming only, never behaviour) | `D:/ArcheAge Returns 10.0.2.13 - 8yx - r575 - 2026-06-18/Bin64/decompiled/x2game-dev_dedicate.dll.c` |
| existing notes (reused, corrected where this map disagrees) | `re-wagons.md`, `re-transport.md`, `re-constants.md`, `port-movement/MAP.md` |
| driver | `C:/Users/stsiakal/AAEmu/rust/crates/aaemu-clientdriver/` (`feat/driver-call`) |

Evidence markers: **[R]** read in the binary/decompile/data dump, **[I]** inference, **[?]** open.

### 0.3 Addresses

All x2game addresses are virtual addresses at the DLL's preferred base `0x38FF0000`, identical to
`x2game.dll.c` and `funcs.csv`. The unpacked tail code (`0x3a6xxxxx`–`0x3acxxxxx`) is part of the
same image; `thunk_FUN_xxx` at `0x39xxxxxx` are 5-byte jumps into it. CryPhysics addresses are
given when they are the callee of an x2game thunk.

Function sizes in this folder are the `size` column of `funcs.csv` (bytes). Nine small leaves
used by the vehicle code are not in `funcs.csv` (thunks/padding that Ghidra did not split); they
are marked `(unlisted)` with the byte count measured by disassembly where it matters.

### 0.4 Layout of this folder

| file | content |
|---|---|
| `MAP.md` (this) | architecture, call chains per area, port order, unknowns, corrections |
| [`functions.md`](functions.md) | every function in scope: address, size, callers/callees, offsets, pure/stateful, role |
| [`structs.md`](structs.md) | the Rust struct layouts and the observed object/field offsets |
| [`packets.md`](packets.md) | the move-struct (`CSMoveUnit`/`SCUnitMovement`) field map and codecs |
| [`constants.md`](constants.md) | every constant read from `.data`, with address and value |
| [`difftest.md`](difftest.md) | in-process differential harness recipes per function group |

### 0.5 Scope size

The inventory in `functions.md` documents **182 functions listed in `funcs.csv`** (**115,163
bytes**) plus **9 unlisted leaves** (0x39094330, 0x390af550, 0x392043d0, 0x390e3050, 0x390e3070,
0x390e3080, 0x390e87e0, 0x390e87f0, 0x390cb1c0; ~40 B total). This includes the shared movement
send pump and the attach/seat plumbing that S3 shares with F1. The authoritative per-area rows
and the exact count are `functions.md` §1–2.

---

## 1. Architecture

### 1.1 Object graph

```
X2::GameClient                        global pointer DAT_3a5bb1d8 (set by FUN_39508e10)
  +0x04  control mode (1 on foot, 2/3/4 otherwise)          [R: FUN_394046f0, 394aabd0]
  +0x10c local player actor (X2::Actor*)                     [R: FUN_39346290]
  +0x120 list of current attachments (point -> entity)      [R: FUN_390dea00, FUN_390c9f40]
  +0x117c queued remote moves (list, +0x1180 count)         [R: FUN_391e55c0, FUN_391d4d60]

X2::Actor (player/NPC/vehicle unit; id -> actor via FUN_39360020)
  +0x08      unit id
  +0x14      "unit" pointer (used by Init logs)             [R: FUN_390e8b30]
  +0x410     seat state: 2 = on a slave/vehicle, 4 = on a transfer   [R: FUN_39346290, 393565c0]
  +0x41c     slave template id (tl id; sent by CSBindSlave)
  +0x3af0    parent object id (DAT_3a59f858 = none)         [R: FUN_390b3f10]
  +0x3af4    attach point (u32)                             [R: FUN_390b3f10]
  +0x3c26    dead flag (blocks bind)                        [R: FUN_39346290]
  +0x3a70..0x3a7b  busy flags                               [R: FUN_39346290]

X2::UnitModel subobject (the `this` of ShipUnitModel::Update 0x390ebc50 / VehicleUnitModel::Update 0x390ebaa0)
  +0x00      vtable: ship 0x399a5930, vehicle 0x399a5e08  [R: vtable + profiler strings]
  +0x08      entity interface object (accessors below)
  +0x14      unit pointer
  +0x100     model/type row lookup (via FUN_39695a70)       [R: 0x390d2990]
  +0x138..+0x13c  attach-point array begin/end (0x28 stride) [R: 0x390d2990]
  +0x1f0     damage/model state (1..6)                      [R: FUN_390eb610]
  +0x238     effect/particle host (unnamed; gated in 0x390e8d70)  [R]
  +0x298     use_wheeled_vehicle_simulation (byte)          [R: FUN_390f0510]
  +0x29c     local vehicle movement controller*             [R: FUN_390c1b80]
  +0x2a0     NetActorMovementController*                    [R: FUN_390c1b80]
  +0x2a4..0x2ac  ship sail entries (0x48 stride)            [R: FUN_390d1c60/390e8e40]
  +0x2ac..0x2b0  vehicle wheel entries (0x9c stride)        [R: FUN_390d6b10/390cf180]
  +0x2c0..0x2c4  exhaust attachments                        [R: FUN_390c78d0]
  +0x2d0..0x2d4  sail flag bone indices                     [R: FUN_390d1c60]
  +0x2e0..0x2e4  water-ripple points                        [R: FUN_390c7640]
  +0x2f0..0x2f4  ship "bubbles"/emitters                   [R: FUN_390c78d0]
  +0x330     current speed (written by the apply)           [R: FUN_390af740]
  +0x358     throttle (ship)                                [R: FUN_390be0d0]
  +0x35c     steering (ship)                                [R: FUN_390be0b0]
  +0x360     helm wheel angle (smoothed)                     [R: FUN_390d2990]
  +0x364..0x374  helm doodad transform (quat+pos)           [R: FUN_390e8e40]
  +0x380     stuck flag (ship)                              [R: FUN_390be0f0]
  +0x3a0     last throttle for the anim                     [R: FUN_390db700]
  +0x3f8     center-spindle angle (vehicle)                 [R: FUN_390cffd0]
  +0x3fc     steering-wheel angle (vehicle)                 [R: FUN_390d0800]

X2::NetActorMovementController (NetActorMovementController; created by FUN_390b2260)
  vtable 0x399a3368: +0 dtor 0x390b2380, +4 reset 0x390aff70, +8 update/interpolate 0x390b2420, +0xc 0x39094330
  +4         owner unit/model
  +0x104     zone id/state, +0x10a zone, +0x10c stuck       [R: FUN_390b0040]
  +0x134..+0x140  applied velocity (f32 x4)                 [R: FUN_390b0040]
  +0x144..+0x14c  applied position                          [R: FUN_390b0040]
  +0x188..   target pose, +0x198.. target velocity          [R: FUN_390afe10]
  +0x1a8/+0x1b0  target position, +0x1b8/+0x1bc steering/throttle  [R: FUN_390b0040]

Local vehicle movement controller (unnamed `X2::...`, created by FUN_390abe10; 0x1a0 bytes)
  vtable 0x399a316c: +0 dtor 0x390abf90, +4 reset 0x390964e0, +8 step 0x390acee0,
                    +0xc 0x39094330 (unlisted), +0x10 0x390967c0
  +8         owner model
  +0x15c..   drive state (steering/throttle/gear, see §2.4)
  +0x16c/0x170/0x174  angular velocity (pitch/yaw/roll)      [R: FUN_390a3de0]
  +0x178/0x17c/0x180  linear velocity                        [R: FUN_390a3de0]
  +0x188..0x19c  pose cache (quat+pos)                       [R: FUN_3909c660]
```

The **unit-model `this`** of the visual functions (0x390ebc50, 0x390d1c60, …) is a subobject: the
loader `FUN_390e95a0` allocates `0x3a8` bytes, constructs the base at the block start and returns
**block+8** as the unit-model pointer [R: disassembly 0x390e95e0–0x390e95eb]. The controller is
handed `this-8` [R: 0x390c1c1c `lea ecx,[esi-8]`], therefore controller offsets are shifted by 8
against the visual functions' offsets (e.g. wheeled flag: controller `+0x2a0` == model `+0x298`).
`structs.md` gives both views.

### 1.2 One frame (vehicle/ship relevant part)

```
engine frame / X2::GameClient::PrePhysicsUpdate (F1, mapped)
├─ InputManager per-unit UpdateMovement             (F1: 0x39404d50)
├─ local controller step                            S3: 0x390acee0
│  ├─ wheeled sim  -> 0x3909c660 (PE_WHEELEDVEHICLE action + pose readback)
│  ├─ kinmatic     -> 0x390a3de0 (integrate euler rates+velocity, write pose)
│  └─ else         -> 0x390ac300 -> cryphysics 0x3a9c33fa (external thunk)
├─ unit pose apply (local + remote)                 S3: 0x390af740  (vtable 0x399a3348+0x10)
│  ├─ on-foot/creature -> 0x390eed80 (interpolating pose apply)
│  ├─ vehicle/ship     -> 0x390c70f0 (pose apply + ground align)
│  ├─ physics wheel/steer actions -> 0x390c1ff0 + IPhysicalEntity::Action
│  └─ model speed +0x330, sails 0x390db410
├─ model Update (vtable slot 0)
│  ├─ VehicleUnitModel::Update 0x390ebaa0
│  │  ├─ PrefabUnitModel::Update 0x390eb7d0
│  │  ├─ UpdateCenterSpindle 0x390cffd0, UpdateWheels 0x390d6b10
│  │  ├─ UpdateSteeringAnim 0x390da800, UpdatePushAnim 0x390dac80
│  │  ├─ UpdateDriver 0x390d0800, UpdateGunTurret 0x390d14b0
│  │  ├─ UpdatePlank 0x390db140, UpdateSound 0x390c6780
│  └─ ShipUnitModel::Update 0x390ebc50
│     ├─ wind 0x390c22c0, UpdateSail 0x390d1c60, UpdateRudder 0x390c7410,
│     ├─ UpdateSteeringWheel 0x390d2990, UpdateExhaust 0x390c78d0,
│     ├─ UpdateSound 0x390c7bf0, UpdateWaterRipple 0x390c7640, UpdateThrottleAnim 0x390db700
├─ PostUpdate movement pump (F1: 0x39406740 -> 0x39406650 -> 0x394064b0)
│  └─ 0x394045f0 (non-foot) -> GameClient vtable+0x60 fills a 0x130 move struct
│     -> opcode 0x89 (CSMoveUnit) send via thunk 0x3ac1058c     [R]
└─ network receive
   ├─ single move: OnOneUnitMovement 0x391d4e70 -> GameClient vtable+0x40(move)   [R]
   └─ list move:   0x391e55c0 queues copies (FUN_39024610); flush 0x391d4d60     [R]
```

### 1.3 Module split

* **x2game** owns everything in this map: the unit models, the local controller, the vehicle/ship
  visual chains, the packet codecs, the seats/attach rules, the transfer manager.
* **cryphysics** owns the actual living-entity / wheeled-vehicle simulation (already ported).
  x2game reaches it through three thunks: `0x3a9c33fa` (generic vehicle action, callee of
  `FUN_390ac300`), `0x3a9c0607` (callee of a `thunk_FUN` in `FUN_390acee0`), and
  `0x3ac109e3` (used for wheel spin readback in `FUN_3909c660`) [R].
* **cryaction/crygame** are not on the drive path (they are reached only via the entity vtable
  used for pose/anim accessors) [R: no x2game caller of the engine movement controllers in
  `calls.tsv`].

### 1.4 The move packet path (shared by A/B/C/D)

`CSMoveUnit`/`SCUnitMovement` carry a single `0x130`-byte **move struct**. Its type selects the
layout [R: `FUN_397c0470` cases; `FUN_39024610` copies exactly this struct]:

| type | meaning | 1.2 evidence |
|---|---|---|
| 0 | generic/actor (legacy) | `FUN_397c0470` default branch |
| 1 | actor on foot (delta movement, fall velocity, stance, alertness) | case 1 |
| 2 | **land vehicle** (pos, quat, velocity, angVel, steering, wheels) | case 2 |
| 3 | **ship state** (pos, quat, velocity, angVel, i8 steering/throttle, zone, stuck) | case 3 |
| 4 | **ship helm request** (i8 throttle, i8 steering) | case 4 |
| 5 | **transfer** (pos, quat, velocity, angVel, i32 steering/pathPoint, speed, reverse) | case 5 |

Field offsets and the exact codecs are in `packets.md`. Two corrections to earlier notes:

* The velocity/quaternion i16 scale for types 2/3/5 is **30.0** (`0x3999f7c0`), not 50 [R:
  `FUN_397c0470`, `constants.md`]. Type 1 uses 60.0 (`0x3999d01c`), type 0 uses 50.0
  (`0x3999ddc8`).
* Position is a **9-byte packed 24-bit triplet** (`ix0..iz2`, 1/8 unit; z scaled by
  2.384e-7*4196 − 100), codec `FUN_3961e2c0` [R: string keys 0x399f9ba8..0x399f9b88].

Cadence: the **non-foot** pump `FUN_394045f0` sends at most every
`DAT_3a59f84c->vtable[8]()` ms (`*0.001`, constant `0x3999cf30`) and has no change/min-interval
test; the **on-foot** pump `FUN_394046f0` adds the `0.01 m` change test and the minimum interval
`DAT_3a59f850` [R: 0x394045f0/0x394046f0, both timer objects are runtime pointers, not in
`.data`]. The live veh-golden lane measured the non-foot interval at 100 ms (§8.2, U2).

---

## 2. A. Land vehicles (Farm Wagon / carts / speedcars)

### 2.1 Data and identity

Reused from `re-wagons.md` §0–2 and `re-transport.md` §6 (not re-proved): the wagon is
`slaves.id 60`, `model_id 1008`, `slave_kind_id 6`; `vehicle_models.id 38` (`use_wheeled_vehicle_simulation f`,
`use_center_spindle t`, `velocity 4.0`, `angVel 0.4`, `max_climb_ang 45`); speedcars are
`use_wheeled_vehicle_simulation t`. The **row struct offsets** used by this code are now resolved
from the SQL loader `FUN_39736bd0` (see `constants.md` §2):

| row field | offset | read by |
|---|---|---|
| `angVel` | +0x00 | `UpdateDriver` 0x390d0800, `UpdateCenterSpindle` 0x390cffd0 |
| `auto_level`, `can_fly` | +0x04, +0x05 | physicalize 0x390cf180, init 0x390f0510 |
| `damaged25/50/75`, `dead`, `dying` | +0x08/+0x0c/+0x10/+0x14/+0x1c | damage state 0x3989f8b0 (shared) |
| `floating_height/wave_height/period` | +0x20/+0x24/+0x28 | floating transfers |
| `installed_turret` | +0x2c | turret assembly |
| `lin_inertia`, `max_climb_ang`, `rot_inertia` | +0x34, +0x38, +0x44 | physicalize |
| `turret_pitch_angle_max/min/angvel` | +0x58/+0x5c/+0x60 | `UpdateGunTurret` (rate at +0x60) |
| `turret_yaw_angle_max/min/angvel` | +0x64/+0x68/+0x6c | `UpdateGunTurret` (rate at +0x6c) |
| `use_center_spindle` | +0x70 | `UpdateDriver`/`UpdateCenterSpindle` gate |
| `use_wheeled_vehicle_simulation` | +0x72 | `VehicleUnitModel::Init`, controller dispatch |
| `velocity` | +0x74 | physicalize, steering anim ratio |
| `wheel`, `wheel2` | +0x78, +0x7c (string ids) | `FUN_390e7b70` wheel builder |
| `wheeled_vehicle_ballast_mass/pos_y` | +0x80/+0x84 | physicalize |
| `wheeled_vehicle_brake_torque` | +0x88 | physicalize |
| `wheeled_vehicle_drive` | +0x8c (0/1/2) | physicalize: FWD/RWD/AWD branch |
| `wheeled_vehicle_front/rear_optimal_sa` | +0x90/+0xb0 | physicalize |
| `wheeled_vehicle_gear_speed_ratio_1..3/reverse` | +0x94..+0xa0 | physicalize |
| `wheeled_vehicle_mass` | +0xa4 | physicalize |
| `wheeled_vehicle_max_gear` | +0xa8 | physicalize (clamped 1..5) |
| `wheeled_vehicle_power` | +0xac (hp × 745.7 → W) | physicalize |
| `wheeled_vehicle_susp_damping/stroke` | +0xb4/+0xb8 | physicalize |

### 2.2 Summon and placement

Reused from `re-wagons.md` §1 and `re-transport.md` §1/§2; addresses re-verified:

| function | addr | size | role |
|---|---|---|---|
| `X2::GameClient::SlaveLocator::Update` | 0x3934a3e0 | 332 | placement ghost per frame; state machine (`param_1[0]=` active flag vs `DAT_3a168a64`; `param_1[1]` = 1 cursor ray / 2 wheel rotate) |
| cursor ray | 0x39349ad0 | 146 | ray to ground, sets ghost pose |
| wheel rotate | 0x39349b70 | 112 | mouse-wheel (axis 8) yaw |
| land validity | 0x393488d0 | 792 | OBB `PrimitiveWorldIntersection` flags `0xd`; valid when hits ≤ 0 |
| boat validity | 0x39348bf0 | 869 | distance² ≤ `0x399cfcfc` = **6400.0** (80 m) + OBB flags `0x10d` |
| invalid material | 0x39346580 | 145 | swaps the ghost to `materials/special/housing_build_invalid` |
| `OnSlaveCreated` | 0x3934a6c0 | 289 | register unit, fire Lua event 0x66, spawn fx unless `hideSpawnEffect` |
| create/register unit | 0x39347d90 / 0x39349020 | — | via `OnSlaveCreated` [R: callees] |

Summoning itself is the item skill 15802 (`re-wagons.md` §1); the client part is only the locator
and `SCSlaveCreated` registration. **No auto-bind** exists (the only caller of the bind sender is
the interaction path `0x39362f50`).

### 2.3 Boarding, seats, unbind

| function | addr | size | role |
|---|---|---|---|
| target interaction | 0x393be280 | 606 | resolves the interaction on the target |
| interaction range gate | 0x393bf270 | 151 | surface gap vs `0x39ac79fc` = 9.0 (3 m) / `0x39ac7a00` = 12.25 (3.5 m when target `+0x3c26`) |
| interaction dispatch | 0x393bf310 | 485 | interaction-id switch: slave -> 0x39362f50, transfer -> 0x393565c0 |
| slave bind path | 0x39362f50 | 390 | precheck + send |
| bind precheck | 0x39346290 | 517 | dead `+0x3c26` -> 0x144; busy `+0x3a70..7b` -> 0x13; range `0x39ac7408` = 16.0 (4 m) -> 0x9a; occupied -> 0x66 |
| `CSBindSlave` 0x31 | 0x393464a0 | 103 | log "requesting bind slave(slaveId:%u)", send |
| `OnSlaveBound` | 0x391d4760 | 160 | stores slave state, fires 0x61 via 0x39345d00 |
| own seat state | 0x39345d00 | 192 | `+0x10 = &DAT_3a168a98` (my slave), `+0x14 = slave tl id`, `+0x18 = slave object id`, camera slot `+0x98(0)`, event 0x61 |
| `CSDiscardSlave` 0x32 | 0x39345aa0 | 335 | refuses above `0x3999f85c` = 25.0 (5 m/s); log "requesting discard slave" |
| `SCUnitDetached` -> unbind event | 0x39345dc0 | — | fires 0x62 [R: re-wagons, not re-read] |
| `OnEscapeSlave` | 0x39345ea0 | 109 | repositions on `SCEscapeSlave` |
| `SCUnitAttached` | 0x391d49d0 | 155 | log "client OnUnitAttached. parent=%u, child=%u"; child+0xc, parent+0x10, payload+0x18 |
| attach to unit | 0x39362770 | 300 | calls `X2::Unit::AttachTo` 0x390b3f10; if local player and state 2 and point != 1 fires 0x61 and refreshes the mode bar |
| `X2::Unit::AttachTo` | 0x390b3f10 | 225 | log "Unit::AttachTo: id=%u cur=%u new=%u"; sets `+0x3af0` parent, `+0x3af4` point |
| game entity attach | 0x390e16b0 | 94 | stores parent/point, calls entity vtable `+0x278` and parent `+0x288` |
| attachment list add | 0x390dea00 | 109 | list at `GameClient+0x120`, nodes `{next, prev, entity, point}` |
| attachment lookup | 0x390c9f40 | 42 | `GameClient` vtable helper: entity at a point |
| surface gap | 0x39093860 | 90 | min gap over collision shapes |
| shape gap helpers | 0x3989bf90 / 0x398b1ab0 / 0x398b1af0 | 101/64/92 | physics (ported already) |

The wagon has **one seat** (attach point 1, `anim_action 12`); ships/cannons add passenger/gunner
points. There is no client-side auto-board.

### 2.4 The local vehicle movement controller

Activation and creation [R: disassembly]:

| function | addr | size | role |
|---|---|---|---|
| `CPlayer::ActivateLocalVehicleController(bool)` (unnamed, vtable slot 0 of `0x399a5bf8`) | 0x390c1b80 | 273 | log `activating local vehicle movement controller(%s -> %s) unit(%u : %s)`; resets the NetActorMovementController; on activate allocs `0x1a0` and calls 0x390abe10 with `this-8`; stores at model `+0x29c` |
| controller ctor | 0x390abe10 | 373 | vtable `0x399a316c`; `+8 = owner model`; initialises `+0x15c..+0x184` |
| controller reset | 0x390964e0 | 264 | zeroes `+0x34/+0xc/+0xa0/+0x38/+0x15c..+0x184` |
| controller step | 0x390acee0 | 85 | (1) model->vtable+0x34 != 0 -> return; (2) `thunk_FUN_3a9c0607`; (3) if model `+0x2a0` (wheeled) -> 0x3909c660; (4) else if entity desc `+0x34+5` (flying) -> 0x390a3de0; (5) else 0x390ac300 |
| wheeled branch | 0x3909c660 | 1837 | reads physics transform/velocity, writes pose cache `+0x188/+0x190/+0x198`, builds the CryPhysics wheeled-vehicle action/params (motor/brake/steer; inferred: `IPhysicalEntity` action) and applies it; log "wheeled vehicle warped from(%.1f, %.1f, %.1f) to (%.1f, %.1f, %.1f). resetting.." over 128 m (`0x399a2f7c` = 16384 squared) |
| kinematic branch | 0x390a3de0 | 1175 | integrates `+0x16c/+0x170/+0x174` (angular rates) and `+0x178/+0x17c/+0x180` (linear velocity) into quat+pos at `dt` from `DAT_3a5bb1d8[5]->+0x30`, writes via entity `+0x244`/`+0x78`/`+0x7c` |
| generic physics branch | 0x390ac300 | 29 | `FUN_3a9c33fa` (cryphysics thunk) |
| velocity filter (controller vtable +0x10) | 0x390967c0 | 231 | zeroes a 3-vector when `|v·|` below `0x399a2d5c` = 0.0873 (5°/s-ish) or below `+0x174 * (+0x15c/x) * 0.5` |
| controller vtable +0xc | 0x39094330 | ~16 (unlisted) | `movq xmm0,[ecx]; ...` tiny getter (pose readback) |
| dtor | 0x390abf90 | 137 | |

The controller's drive state (`+0x15c..+0x184`) is written by the movement input path (F1) and
consumed here. Exact per-field mapping is partly inferred:

| field | meaning | evidence |
|---|---|---|
| `+0x15c` | signed drive/steering magnitude | `FUN_390967c0` compares `in[1] * +0x15c`; `FUN_3909c660` reads it as `param_1[0x57]` for the gear/steer interpolation |
| `+0x160` | current throttle/gear (float) | `FUN_3909c660`: `local_8 = param_1[0x58]`, sign flip resets `+0x160/+0x164/+0x168` |
| `+0x164`, `+0x168` | steering/wheel accumulators | reset together with `+0x160` |
| `+0x16c`, `+0x170`, `+0x174` | angular velocity (pitch, yaw, roll) | `FUN_390a3de0` half-angle integration |
| `+0x178`, `+0x17c`, `+0x180` | linear velocity | `FUN_390a3de0` `pos += v*dt` |
| `+0x184` | mode flag | set to `_DAT_3999c488` (1.0) in ctor |

`use_wheeled_vehicle_simulation` picks the wheeled branch; `can_fly`/floating models take the
kinematic branch; everything else goes to CryPhysics' generic action. The wagon (flag `f`) takes
the generic branch, the speedcars (flag `t`) the wheeled one [R: `FUN_390acee0` + data].

### 2.5 Physics: what x2game hands to cryphysics

`FUN_390cf180` (`Physicalize`, 3455 B) builds the physics descriptor from `vehicle_models` +
the model's wheel entries and calls the entity's `+0x104` (Physicalize) then reads back
`+0x1a0` (GetPhysics). The descriptor fields, with evidence [R]:

```
local_1d0 = 3                        ; object type (wheeled vehicle? see below)
wheel radius  = wheel_entry+0x74     ; per-wheel bbox radius from FUN_390e7b70
drive         = row+0x8c             ; 0 FWD, 1 RWD, 2 AWD (mismatch -> "AWD vehicle has different tire size")
mass          = model+0x328
gear ratios   = row+0x94/+0x98/+0x9c, reverse row+0xa0
ballast       = row+0x80 (mass), row+0x84 (y)
brake torque  = row+0x88
max gear      = clamp(row+0xa8 + 2, 1, 5)
power         = row+0xac * 745.7 (hp -> W)
front/rear SA = row+0x90 / row+0xb0
susp damping/stroke = row+0xb4 / row+0xb8
max steer     = 0x399a433c = 0.10472 rad (6 deg)  ; used as (mass*0.10472/velocity)*radius
```

Wheel entries are `0x9c` bytes; their `+0x74` is the radius, `+0x94/+0x98/+0x9c/+0xa0/+0xa4/...`
mirror the row (suspension/wheel geometry). `FUN_390d6b10` (`UpdateWheels`, 15,584 B) then
ray-casts (`"RayWorldIntersection(Game)"`, flags `0x101`/`0x8008f`), finds the `surspension_front/rear`
helpers and the `wheelN` bones, and writes each wheel's bone transform + spin; it also drives the
non-wheeled "fake" wheel path [R].

`aaemu-physics` already implements the counterpart living-entity / wheeled body; the port only
needs to produce this descriptor and to read back the pose at the same points (`+0x228` world
transform, `+0xa4` velocity, `+0x1a0` physics).

### 2.6 Vehicle pose -> `CSMoveUnit` type 2 and cadence

1. The controller/model keeps the current pose in the NetActorMovementController (target `+0x188`,
   current `+0x144/+0x134`) [R: 0x390b0040/0x390af740].
2. `FUN_394064b0` picks the sender: control mode 1 -> on-foot 0x394046f0, otherwise 0x394045f0
   [R].
3. `FUN_394045f0` (243 B) sends only when the *unit being sent* is not on a transfer
   (`+0x410 != 4`) and has no parent (`+0x3af0 == DAT_3a59f858`), then:
   * builds the packet header on the stack: `[esp-0x13c]` = packet vtable/ctor,
     `[esp-0x138]` = opcode `0x89`, `[esp-0x130]` = unit id, move struct at `[esp-0x12c]`;
   * `GameClient->vtable[0x60](move)` fills the `0x130`-byte move struct (type 2 or 4, inferred
     from the controlled unit's physics) — the method address is runtime [?];
   * `thunk_FUN_3ac1058c` sends it; `[this+4]` (per-unit timer) is reset; the last-sent pose is
     cached; the non-foot pump has no change suppression (that is the on-foot pump) [R].
4. The move struct's type-2 layout is serialized by `FUN_397c0470` case 2; the wire codecs are
   in `packets.md`.

Cadence guard [R: 0x394045f0/0x394046f0]:
```
if (now_ms*0.001 >= timer) and (state changed or now_ms*0.001 >= min_interval) send
```
`DAT_3a59f84c` = "max interval" timer object, `DAT_3a59f850` = "min interval" (both runtime).

### 2.7 The model: init, wheels, steering, driver, turret

| function | addr | size | role / offsets |
|---|---|---|---|
| `VehicleUnitModel::Init` | 0x390f0510 | 732 | model sub-type 3; copies `use_wheeled_vehicle_simulation` from row `+0x72` to model `+0x298`; damage state -> `+0x1f0`; prefab variant `FUN_397a7810`; brush `FUN_390d5f20`; member setup `FUN_390e69d0`; initial pose via entity `+0x244` and, when in-world and not a flying model, `FUN_390eed80`; `FUN_390ff790(0xfe)`; vtable `+0xf4(1)`, `+0xd0(param_2)` |
| prefab loader | 0x390d5f20 | 357 | log "loading prefab for unit model. unitName(%s), unitId(%u), path(%s)" |
| member setup | 0x390e69d0 | 3773 | post-load parts (anim objects, attachments) |
| part/attach binding | 0x390e1aa0 | 1909 | binds helpers/attach points (doodads) |
| wheel builder | 0x390e7b70 | 1979 | loops bone names `wheel%d` (model = row `+0x78`) and `wheel_type2%d` (row `+0x7c`); creates a wheel object per bone (`FUN_3910ade0`), radius = ½ bbox, looks for `$IK_Grip_L/R`, suspension bones; errors "Vehicle wheel radius is invalid", "AWD vehicle has different tire size" |
| `UpdateWheels` | 0x390d6b10 | 15584 | per-wheel ground raycasts, suspension, bone transforms, spin; branches on the wheeled flag `+0x2a0` |
| `UpdateCenterSpindle` | 0x390cffd0 | 2082 | Ackermann-style spindle angle from the wheel travel difference, smoothed by `min(dt,0.1)` — `DAT_3999ccf0` = 0.1; angle stored at `+0x3f8`; write `Bone_spindle`; cache wheel poses `+0x148..0x160`; gate `row+0x70` (use_center_spindle) |
| `UpdateDriver` | 0x390d0800 | 2016 | IK grips `$IK_Grip_L/R` to the helm, steering-wheel angle at `+0x3fc` (smoothed, `angVel` rate); `0x3999edbc` = 3.0 turn scale; gate `row+0x70` |
| `UpdateSteeringAnim` | 0x390da800 | 1137 | plays `fist_ac_steering` / `fist_ac_steering_backward` from the steering value |
| `UpdatePushAnim` | 0x390dac80 | 1205 | plays `fist_ac_push_f` / `fist_ac_push_b` |
| `UpdatePlank` | 0x390db140 | 716 | plays `spread` / `fold` for the plank/ramp |
| `UpdateGunTurret` | 0x390d14b0 | 1510 | aim from model `+0x2cc/+0x2d0`, limits/rates from `row+0x58..+0x6c`, writes `bone_yaw`/`bone_pitch`; `param_2 != 0` snaps |
| `UpdateSound` | 0x390c6780 | 1723 | engine/water loop |

Vehicle `Update` order (0x390ebaa0): base update `0x390eb7d0`, then for states != 5/6
`UpdateCenterSpindle`, `UpdateWheels`, `UpdateSteeringAnim`, `UpdatePushAnim`, `UpdateDriver`,
`UpdateGunTurret(0)`, `UpdatePlank`, `UpdateSound`, then `0x390c6780` again for non-5/6 states
[R]. It also checks the physics vehicle type against 1/3 and re-physicalizes via `FUN_390cf180`
when it changed.

### 2.8 Damage, despawn, UI

* Damage/model state is a pure function of HP, `FUN_3989f8b0` (HP% > 75 -> 1, > 50 -> 2, > 25 ->
  3, > 0 -> 4, 0 -> 6) [R: already in `re-transport.md` §4].
* `PrefabUnitModel::UpdateModelChange` 0x390eb610 compares the state and calls 0x390e7b20
  (`prefab unit model : switching to(%d)`) [R].
* Mode bar: `FUN_39204500` fills from `slave_mount_skills`, adds unbind; `FUN_392045f0` fires
  `MODE_ACTIONS_UPDATE` (0x5f) [R: re-transport §2].
* `SHOW_SLAVE_INFO` (0x60) is fired only by the 8-byte stub 0x392043d0 (`push 0x60; call
  0x39424220`) [R: disassembly].
* Live info getters: `GetSiegeWeaponSpeed` 0x394aabd0 (696 B, signed forward speed; requires
  local player's parent `+0x410 == 2` and GameClient mode 3/4), `GetSiegeWeaponTurnSpeed`
  0x394aae90 (319 B, angVel z × 57.2958 `0x399a44f4`), `GetVehicleInfo` 0x394ab270 (373 B, only
  for wheeled sims), `IsDriver` 0x394aafd0 (249 B, point == 1) [R].
* Despawn/recall: `SCSlaveRemoved` 0x062 / `SCSlaveDespawn` 0x063 -> `SLAVE_DESPAWN`;
  `CSDespawnSlave` 0x2f, `CSDestroySlave` 0x30 [R: existing notes, opcodes].
* Ship equipment/customizing UI natives: `GetSlaveEquipSlotInfo` 0x394abd90, `CanSlaveEquipPickedItem`
  0x394ac1e0, binding table 0x394ac320 [R: re-transport §3].

---

## 3. B. Ships

### 3.1 Data and pose authority

Reused from `re-transport.md` §1: `models.sub_type = 'ShipModel'`, `ship_models` row; the server
owns the hull and broadcasts type 3. The client never integrates the hull (the transfer/hull pose
arrives in `SCUnitMovement`); the local controller exists but for ships its physics output is not
authoritative [I, supported by the absence of any type-3 writer besides the packet path].

**Buoyancy**: there is no client-side buoyancy code on this path. `vehicle_models.floating_height`,
`floating_wave_height`, `floating_wave_period_ratio` and `ship_models.water_*` are consumed by the
physics / transfer layer, not by `ShipUnitModel`; the visible ship position is the server type-3
pose applied as in §3.2. The only water feedback in the client is the ripple/exhaust/sound chain
in §3.4 [R: `ship_models`/`vehicle_models` columns + absence; see re-transport §4].

### 3.2 Applying a received ship pose (type 3)

```
OnOneUnitMovement 0x391d4e70 (or list 0x391e55c0 + flush 0x391d4d60)
  -> GameClient->vtable[0x40](move)                       [runtime; ?]
     -> NetActorMovementController target fields (0x188.., 0x1b8/0x1bc)
        -> 0x390b2420 (vtable +8): interpolate, timeout 5000 ms
           -> 0x390b0040: read entity transform/velocity, choose ground-align
              -> 0x390ad150 / 0x390ae4a0 (on a slope / ground align)
              -> IPhysicalEntity->vtable[0x60](state)      ; set pose+velocity
              -> entity +0x78 (set rotation), +0x7c (set angles)
              -> model setters: 0x390be0b0(steering +0x35c), 0x390be0d0(throttle +0x358),
                 0x390be0f0(stuck +0x380), 0x390be100(zone +0x384)
              -> cache applied pose +0x134/+0x144
```

`0x390b0040` is where **type-3 `ship.steering`/`ship.throttle` become the visual state**; the
values come from the controller `+0x1b8/+0x1bc`, which the packet apply writes (the GameClient
method at vtable +0x40 is runtime, so the exact write instruction is not statically resolved — [?]
U3). The interpolation method `0x390b2420` clamps to a 5 s timeout and then snaps [R].

### 3.3 Helm input -> `CSMoveUnit` type 4

* Input comes from the normal movement axes (`F1` `FUN_39404d50`); when the controlled unit is a
  ship, the move struct type is 4 and carries only `shipRequest.throttle` (+0x108, i8) and
  `shipRequest.steering` (+0x109, i8) [R: `FUN_397c0470` case 4].
* The packet is sent by the same `FUN_394045f0` pump (opcode `0x89`), because the ship unit is
  parentless and not on a transfer [R: 0x394045f0 condition].
* Field mapping input-axis -> i8 throttle/steering: built by `GameClient->vtable[0x60]` (runtime
  [?] U3); the wire order is throttle then steering [R: case 4 serialization order].
* The earlier note that the driver sends "signed throttle then steering" is confirmed; the
  earlier "i8 steering/throttle" in type 3 is also confirmed (`FUN_397c0470` case 3 writes
  steering at +0x100 and throttle at +0x101 with the u8/i8 writer `archive+0x54`).

### 3.4 The ship visual chain

`ShipUnitModel::Update` 0x390ebc50 (325 B) order [R]:
`PrefabUnitModel::Update` -> speed = `DAT_3a5bb1d8[5]->+0x30(0)` -> `0x390c22c0` (wind area) ->
`UpdateSail(speed)` -> `UpdateRudder` -> `UpdateSteeringWheel(speed)` -> `UpdateExhaust` ->
`UpdateSound` -> `UpdateWaterRipple` -> `UpdateThrottleAnim`.

| function | addr | size | role / offsets |
|---|---|---|---|
| wind area update | 0x390c22c0 | 579 | if the weather manager says windy, calls the entity's `+0xb4` (velocity) and the 3D engine's wind query `+0x370`; sets an area wind |
| `UpdateSail` | 0x390d1c60 | 3225 | sail entries at `+0x2a4..0x2ac` (0x48 stride), flags at `+0x2d0..0x2d4`; filters at `+0x34c/+0x350/+0x348` (0.5/2.0 factors); wind `DAT_39ae6c94`; billow uses `bone_yaw`; the 0.48/0.235 mix and 0.628 rad/s flag limit are from the earlier note (re-transport §2), not re-derived here |
| `UpdateRudder` | 0x390c7410 | 550 | if the "rudder" child exists (`+0x2bc`? = `param_1+700`), builds the quaternion from the steering `+0x35c` and writes it; missing rudder logs "ship unit model(%u) has no rudder" (0x390c7330) |
| `UpdateSteeringWheel` | 0x390d2990 | 1031 | skips `smallboat`/`soloship`/`coupleduckship` (row type name `+0x3c`); attach point 0x23 (35); target = 45°-quantised (`0x3999f5bc` = 45.0) with a runtime wind/offset term (`0x3999f0e4` = 7.0 default); eases `+0x360`; plays helm sound `0x390d2900`; writes the helm doodad rotation `0x3903bd80` |
| `UpdateThrottleAnim` | 0x390db700 | 531 | gate `+0x39c`; compares `+0x358` to `+0x3a0`; plays `idle`/`run_f`(rate)/`run_b`(rate) via entity `+0x114` with flags 0x5082/0x508a |
| `UpdateExhaustEffect` | 0x390c78d0 | 790 | exhaust list `+0x2c0..0x2c4`; speed-scaled particle params via `0x390fed20` |
| `UpdateWaterRipple` | 0x390c7640 | 656 | ripple points `+0x2e0..0x2e4` (0xc stride); transforms them to world and emits via the 3D engine `+0x2e0`; point count from the entity transform |
| `UpdateSound` | 0x390c7bf0 | 628 | moving-water loop; stops at states 5/6 |
| ripple object init | 0x390e8d70 | 196 | `ripple_effect%i` (5) via `FUN_391047b0` |
| attach-point state | 0x390e8e40 | 904 | point 0x23 (helm) -> `+0x35c..+0x374` (7 floats); points 0x15/0x16 (sails) -> the sail array (0x48 entry, 12 floats, doodad object, `bone_yaw`); log "found doodad sail no(%d). model type(%d). offset(...)" |
| sail attach | 0x390c2510 | 226 | attaches a doodad to a sail slot, resolves `bone_yaw` |
| sail detach | 0x390db920 | 142 | clears the slot |
| sail compact | 0x390db5e0 | 285 | sorts/compacts the sail array |
| speed helper | 0x390db410 | 434 | writes the model speed `+0x330` and drives the speed-based anim/scale |
| `ShipUnitModel::Init` | 0x390e8b30 | 569 | model sub-type 4; damage state; prefab; `FUN_390d5f20`, then vtable `+0xd0/+0x244/+0x50/+0xf4` |

`ground align` helpers: `0x390ad150` (372 B) and `0x390ae2c0` (411 B) are called from the
apply/pose path and clamp the vehicle up vector to the surface (unnamed; the string
`EVehicleGroundAlign` 0x399a5e6b has no xref) [I].

### 3.5 Boarding, deck and moving supports

* Boarding uses the same interaction -> `CSBindSlave` path as land vehicles (§2.3). The server
  picks the seat and answers `SCUnitAttached` (child, point, reason, parent) + `SCSlaveBound`
  [R: re-transport].
* Deck standing is **not** a separate client integrator: `OnUnitAttached` (0x391d49d0) ->
  `0x39362770` -> `X2::Unit::AttachTo` 0x390b3f10 -> `GameClient::AttachEntity` 0x390e16b0,
  which registers the parent/point and lets the engine parent the child entity; the per-frame
  pose comes from the parent's transform (the engine). No local deck physics was found [R: code +
  absence; see re-transport §10.6].
* The attachment list (`GameClient+0x120`) is also how `GetAttachedEntity(point)` finds
  passengers for boarding, helms and turrets (`0x390c9f40`).
* Hanging/supports: no dedicated 1.2 code or string was found; the ropes in the binary
  (`$rope_anchor`, `rope_skill_controller_*` 0x399a339c..) belong to the rope skill controller,
  not to ship supports [R: strings + absence].

### 3.6 Cannons and turrets

* Cannons are separate child slaves (`slave_kind_id 8`, bound at the hull's cannon points); the
  gunner sends `CSBindSlave` for the cannon's tl id [R: re-transport §3].
* Aim: `UpdateGunTurret` 0x390d14b0 reads the model aim `+0x2cc` (yaw) and `+0x2d0` (pitch),
  the rates from `vehicle_models` `+0x60` (pitch) / `+0x6c` (yaw), wraps to ±π (`0x399a36b0`,
  `0x399a36b4`), and writes `bone_yaw`/`bone_pitch`. The min/max angles (`+0x58/+0x5c/+0x64/
  +0x68`) are not applied in this function; they gate the aim producer [R/I].
* Firing is a normal slave/mount skill (`StartSiegeWeaponSkill` native; the projectile and damage
  are server/skill effects). The client-side fire is the skill animation + FX; the turret recoil
  is the skill's anim, not a separate turret function [R: no fire function in the vehicle model;
  `StartSiegeWeaponSkill` is the Lua native].
* The `InitVehicleTurret` profiler string (0x399d8a18) is a **world-start scope**, not turret
  setup: its xref 0x39430ac0 is the in-world boot function [R: decompile].

### 3.7 Stuck ship

`IsMountingShipStucked` 0x394aa840 (282 B) compares the target with the current transfer/mount and
reads the stuck flag (`ShipMoveType.stuck` -> model `+0x380`); `DestroyStuckedShip` 0x394aaad0
(247 B) sends the target/skill request when stuck [R: re-transport §3, addresses re-verified].

---

## 4. C. Transfers (carriages, ferries, airships, lifts)

| function | addr | size | role |
|---|---|---|---|
| `TransferManager::LoadPaths` | 0x393539e0 | 608 | profiler "X2::GameClient::TransferManager::LoadPaths"; iterates world transfers, parses each `transfer_path.xml`, builds splines |
| path parse | 0x393536c0 | 795 | reads `<Transfer Name Type><Points><Point Pos>` blocks |
| spline build | 0x39351b20 / 0x39352470 / 0x39353200 / 0x39352f80 / 0x39352bb0 | 168/39/168/357/73 | per-leg points -> spline |
| `TransferManager::Update` | 0x393540a0 | 336 | per frame, per transfer: path progress; profiler string |
| transfer board packet | 0x393565c0 | 86 | if player `+0x410 == 4` and a free point 2..7 exists, sends `CSBoardingTransfer` 0x67 `{tl u16, ap u8}` |
| free attach point | 0x393546b0 | 64 | first free point 2..7 (via `0x390c9f40`); requires GameClient mode 3 |
| `X2.UnBoardingTransfer` | 0x39440d30 | 76 | sends 0x67 `{transfer tl, ap 0}` and calls the UI callback |
| transfer move history | 0x39350160 | 115 | pushes a move into a 100-entry list (transfer riders) |
| transfer pose step | 0x393524a0 | 390 | for a unit on a transfer (`+0x410 == 4`), compares rider/transfer motion state and marks `+0x180 = 1` when it should move |
| transfer path sync | 0x39353c50 | 821 | refresh path data around the player |
| board dispatch | 0x393bf310 | 485 | interaction id == transfer -> 0x393565c0 |
| transfer UI | — | — | `x2ui/transfer.alb` listens to `TRANSFER_BOARDING` 0x64 / `TRANSFER_UNBOARDING` 0x65 [R: re-transport] |

Riding: the transfer's own pose arrives as `SCUnitMovement` type 5; riders are attached to the
transfer by `SCUnitAttached` and follow the parent transform (same path as §3.5). The client does
not integrate the path (it only displays the server pose) [R: re-transport §7 + this map].
`CSMoveUnit` type 5 is never sent by the client (only the server writes it) [R: `FUN_397c0470`
case 5 exists on the shared serializer; no client sender found].

---

## 5. D. Remote and local unit/vehicle smoothing

| function | addr | size | role |
|---|---|---|---|
| `OnOneUnitMovement` | 0x391d4e70 | 187 | single move: unit id +0x10, move +0x18; if unit `+0x410 == 4` push to the transfer history; `GameClient->vtable[0x40](move)` |
| move list queue | 0x391e55c0 | 263 | SCUnitMovement list: count +0xc, entries +0x18 stride 0x130; `FUN_39024610` copy into `GameClient+0x117c` under a critical section; log "too many unit movements.." above 0x1000 |
| queue flush | 0x391d4d60 | — | pops each queued move and applies it via `GameClient->vtable[0x40]` |
| move struct copy | 0x39024610 | 270 | field-for-field copy of the 0x130 struct |
| `NetActorMovementController` ctor | 0x390b2260 | 275 | vtable `0x399a3368`; `+4 = owner` |
| controller update/interpolate | 0x390b2420 | 406 | if `+0x4c` (timeout start) == 0: stop (`0x390af650`); if older than 5000 ms: destroy/kill; else `0x390b0040` |
| controller apply | 0x390b0040 | 1198 | interpolated pose apply: chooses ground align when the target lies within `0x399a3364` = **400.0** (20 m) of the current position, else snap; writes model steering/throttle/stuck/zone, entity pose, cached pose |
| reset from entity | 0x390aff70 | 201 | copies entity transform/quat into `+0x150/+0x168` |
| stop/reset | 0x390af650 | 236 | zeroes steering/throttle/wheel speeds and stops the physics action |
| generic pose apply | 0x390eed80 | 6024 | 12-argument pose apply used by vehicle Init (`0x390f0510`) and network moves; smooths position/quat, applies a vertical timeout, calls `0x390eda60` (path collision) and `0x390c70f0` (vehicle) |
| vehicle pose apply | 0x390c70f0 | 423 | writes the entity pose and model speed |
| unit movement apply | 0x390af740 | 1732 | vtable `0x399a3348+0x10`: on-foot -> `0x390eed80`; vehicle/ship -> `0x390c70f0`; then per-wheel physics actions (`0x390c1ff0`), model speed `+0x330`, sails `0x390db410` |
| ground align | 0x390ad150 / 0x390ae2c0 | 372/411 | surface normal alignment |

Key numbers: 5 s interpolation timeout [R: 0x390b2420 compares `5000 < now - +0x4c`], 400.0
squared snap threshold [R: `0x399a3364`], wheeled count clamp 18 [R: `0x390af740`], default
interp step `0x3999cf90` = 0.01 m [R: `FUN_394046f0` change detection].

---

## 6. Function inventory (summary)

Full table with callers/callees, field notes and the per-function difftest recipe:
[`functions.md`](functions.md). It lists **191 unique entries** (182 in `funcs.csv` +
9 unlisted leaves), **115,163 bytes** of listed code. The pure leaves (codecs, copy, clamp,
quaternion helpers) are 16 of them and are the first test targets; the rest are stateful and
need a synthetic or live object (`functions.md` §2).

## 7. Constants (summary)

Read from `D:/aa-re/client12/x2game_unpacked_data.dll` (base `0x38FF0000`). Full table with dump
values and the Bevy state: [`constants.md`](constants.md).

| address | value | used by |
|---|---|---|
| 0x399abe78 | 1/32767 (3.05185e-5) | quaternion/velocity i16 codec |
| 0x3999f7c0 | 30.0 | move type 2/3/5 velocity scale |
| 0x3999d01c | 60.0 | move type 1 velocity scale |
| 0x3999ddc8 | 50.0 | move type 0 velocity scale |
| 0x39a12660 | 0.9975 | quaternion w reconstruction limit |
| 0x3999c488 / 0x3999c47c / 0x3999c478 / 0x3999c484 | 1.0 / 0.5 / 1.5 / 0.0 | general math; smoothing |
| 0x3999cffc | 2.0 | half-angle, distance factors |
| 0x3999cf90 | 0.01 | change-detection threshold |
| 0x3999cf30 | 0.001 | ms -> s for the send timers |
| 0x399a3364 | 400.0 | interpolation snap distance² (20 m) |
| 0x399a2f7c | 16384.0 | wheeled-vehicle warp reset distance² (128 m) |
| 0x399a2d5c | 0.0873 | velocity filter minimum |
| 0x399a440c | (not dumped, code constant) | max steer angle; see constants.md |
| 0x399a433c | 0.10472 (6°) | physicalize steer/mass factor |
| 0x399a4338 | 745.7 | hp -> W |
| 0x399a36b0 / 0x399a36b4 | π / 2π | turret angle wrap |
| 0x3999cf98 | 1° in rad | turret rate, driver rates |
| 0x3999f5bc | 45.0 | helm wheel quantization |
| 0x3999edbc | 3.0 | driver steering-wheel scale |
| 0x399a44f4 | 57.2958 | rad -> deg (turn-speed readout) |
| 0x399cfcfc | 6400.0 | boat placement range² (80 m) |
| 0x39ac79fc / 0x39ac7a00 | 9.0 / 12.25 | interaction 3 m / 3.5 m |
| 0x39ac7408 | 16.0 | bind range 4 m |
| 0x3999f85c | 25.0 | unbind speed limit 5 m/s |
| 0x3999ccf0 / 0x3999ccf4 | 0.1 / 5.0 | min dt / speed epsilon |
| 0x3999cf84 | -π | turret wrap |
| 0x3999cd70 | 2^31 as float (2.1475e9) | int->float correction in UI events |

Runtime-only (not in the dump): `DAT_3a59f84c` (max send interval), `DAT_3a59f850` (min send
interval), `DAT_39ae6c90/94` (wind direction/speed objects), `DAT_3a5bb1d8` (GameClient),
`DAT_39b02294` (local player actor), `DAT_3a168a64` (no-object sentinel), `DAT_3a168a98`
(my-slave sentinel), `DAT_3a168a88/a8c` (?) — see `constants.md` §5.

### 7.1 Data tables read by this code

| table | use | evidence |
|---|---|---|
| `vehicle_models` | row layout in `constants.md` §2 | `FUN_39736bd0` SELECT |
| `models` | `sub_type` (`VehicleModel`/`ShipModel`), `sub_id`, `show_name_tag`, `camera_distance`, prefab paths | `FUN_390f0510` (sub-type 3), `FUN_390e8b30` (4) |
| `slaves` / `slave_kind_id` | wagon/ship identity, seats | existing notes |
| `slave_bindings` | hull -> cannon/passenger attach points | re-transport §3 |
| `slave_doodad_bindings` | default part doodads per attach point | re-transport §3 |
| `attach_anims` | seat/rider clip per model+point | re-wagons §3 |
| `transfers`, `transfer_paths` | scripted transports and leg waits | `FUN_393539e0`/`FUN_393536c0` |
| `item_slave_equipments` | ship part items -> doodads | re-transport §3 |
| `ship_models` | hull physics/visual variants | re-transport §1, §4 |

No x2game-registered **vehicle cvar** was found (the `vehicle`/`wheel` cvars at 0x399a8a34.. are
CryPhysics' own console variables, not used by these functions); the x2game tunables are the
constants in §7/`constants.md`.

---

## 8. Proposed port order (testable leaves first)

Every step is a **single function or a small pure cluster** with a differential test that calls
the original at its VA (recipes in `difftest.md`). Steps 1–6 need no live world.

**Phase 0 – pure codecs and constants (no game objects)**
1. `0x3961e2c0` position codec (9-byte packed 24-bit) + inverse. Test: random `vec3` round-trips.
2. `0x397ad8c0` / `0x397ae020` quaternion codec (i16, w reconstruction 0.9975).
3. `0x397ad5a0` / `0x397ada60` / `0x397adc00` / `0x397ad740` per-type velocity/delta codecs.
4. `0x397c0470` whole move serializer: feed a synthetic archive and compare bytes for each type
   (0..5). This is the contract for A/B/C/D, test before anything stateful.
5. `0x39024610` move-struct copy (byte compare).
6. `0x3922aed0` clamp and the small math leaves used by the visual chain.

**Phase 1 – vehicle_models and the descriptor**
7. `0x39736bd0` row loader: feed the SQL strings? (cannot call without a DB). Instead test the
   *consumer* mapping: `0x390cf180` is stateful; port the descriptor construction as a pure
   function `descriptor(row, wheels) -> ...` and difftest it by calling `0x390cf180` on a synthetic
   model (needs a live model, phase 2). Draft the constants now.
8. `0x390e7b70` wheel-bone scan: pure given a skeleton + row (needs a synthetic skeleton; call
   with a synthetic model if the driver can allocate one — otherwise extract the bone loop as a
   pure helper and test with the live model in phase 3).

**Phase 2 – model state machines (need a live/hidden model)**
9. `0x3989f8b0` damage state (already in `re-transport`; port + test first: pure).
10. `0x390eb610` + `0x390e7b20` model-change compare/switch.
11. `0x390e8e40` attach-point state: helm 7-float payload and sail 12-float entry. Test: call with
    a live ship model + fake doodad at points 21/22/35.
12. `0x390db920` / `0x390c2510` / `0x390db5e0` sail-array slot add/remove/compact.
13. `0x390c22c0` wind area (needs the weather manager; assert the no-wind path).

**Phase 3 – visual chains (need a live model + animation)**
14. `0x390c7410` rudder (pure math given steering + child transform).
15. `0x390d2990` helm wheel (quantisation + smoothing; pure given dt/steering).
16. `0x390db700` throttle anim (assert the chosen clip/rate/flags).
17. `0x390d0800` driver IK + `0x390cffd0` center spindle (bone matrices; compare bone transforms).
18. `0x390d14b0` turret aim (bone matrices; compare yaw/pitch quats).
19. `0x390da800` / `0x390dac80` / `0x390db140` steering/push/plank anims (clip selection).
20. `0x390c7640` / `0x390c78d0` / `0x390c7bf0` ripple/exhaust/sound (assert emitter calls).
21. `0x390d6b10` wheels (largest; split into the ray-cast helper and the bone writer; compare each).

**Phase 4 – movement apply and packets**
22. `0x390af740` / `0x390c70f0` / `0x390af650` apply/stop (pose + physics action args).
23. `0x390b0040` + `0x390ad150`/`0x390ae2c0` ship/vehicle state apply (steering/throttle/zone).
24. `0x390af740` wheel physics actions per wheel (`0x390c1ff0`).
25. `0x390b2420` + `0x390b2260` + `0x390afe10` interpolation (timeout/snap).
26. `0x394045f0` + `0x394064b0` + `0x39406650` send cadence (hook the packet body; compare).
27. `0x391d4e70` / `0x391e55c0` / `0x391d4d60` receive/queue/flush (compare the queued bytes).

**Phase 5 – flow (needs the world/server)**
28. Summon locator `0x3934a3e0` + validity `0x393488d0`/`0x39348bf0` (ghost pose + material).
29. Bind/unbind `0x39346290`, `0x393464a0`, `0x39345aa0`, `0x39345d00`.
30. Attach/deck `0x391d49d0`, `0x39362770`, `0x390b3f10`, `0x390e16b0`, `0x390dea00`.
31. Transfers `0x393565c0`, `0x393546b0`, `0x39440d30`, `0x393539e0`, `0x393540a0`.
32. UI getters `0x394aabd0`, `0x394aae90`, `0x394ab270`, `0x394aafd0`.

Rule: a function is ported only after its difftest is green; the Rust unit tests are generated
from the goldens so CI runs without the hidden client (same convention as `port-movement`).

### 8.1 Port status (branch `port/vehicles-a`)

Phase A is ported in `crates/aa-client/src/port/vehicles/` and proven bit-exact against the
original through the veh-golden lane (`rust/tools/capture_vehicle_goldens.py`):

| original | Rust | live cases | result |
|---|---|---|---|
| `FUN_3961e2c0` position codec | `codec::write_position` / `read_position` | 25 | bit-exact |
| `FUN_397ad5a0` vel 1/60 | `codec::write_i16_vec3(.., 1/60)` | 30 | bit-exact |
| `FUN_397ad8c0` vel 1/30 | `codec::write_i16_vec3(.., 1/30)` | 30 | bit-exact |
| `FUN_397ada60` vel 1/50 | `codec::write_i16_vec3(.., 1/50)` | 30 | bit-exact |
| `FUN_397ad740` delta i8 | `codec::write_delta_i8_vec3` | 26 | bit-exact |
| `FUN_397adc00` quat read | `codec::read_quat_i16` | 29 | bit-exact |
| `FUN_397ae020` quat write | `codec::write_quat_i16` | 31 | bit-exact |
| `FUN_397adca0` quat 3×i8 write | `codec::write_quat_i8` | 31 | bit-exact |
| `FUN_3938b460` quat 3×i8 read | `codec::read_quat_i8` | 30 | bit-exact |
| `FUN_397c0470` move serializer | `move_unit::serialize_move` | 23 | bit-exact (write log + read struct) |
| `FUN_39024610` move copy | `move_unit::copy_move` | 20 | bit-exact (padding gaps preserved) |
| `FUN_39094340` 12-byte copy | `math::copy12` | 22 | bit-exact |
| `FUN_390bf0f0` frame builder | `math::build_basis` | 28 | bit-exact |
| `FUN_390967c0` velocity filter | `math::velocity_filter` | 26 | bit-exact |
| `FUN_3989f8b0` damage state | `data::ship_damage_state` (reused) | 20 | bit-exact |

The serializer is tested against a scratch `IArchive` test double built in the lane; write mode
compares the full call sequence (slot/key/value) and read mode replays it into a zeroed move and
compares all 0x130 bytes. Types 0–5 and the type-1 flag branches (0x20/0x40/0x60/0x80) are covered.

### 8.2 Port status (branch `port/vehicles-b`)

Phase B is ported in `crates/aa-client/src/port/vehicles/` and proven against the original through
the veh-golden lane (`rust/tools/capture_vehicle_phase_b.py`):

| original | Rust | live cases | result |
|---|---|---|---|
| `FUN_390cf180` wheeled descriptor (pure part) | `model::build_descriptor` | 20 | bit-exact gear array (up to `gear_count`) + power; AWD/radius logs |
| `FUN_390c1b80` controller activation | `controller::activation` | 20 on/off cycles | controller created/destroyed, ctor fields exact |
| `FUN_390abe10` controller ctor | `controller::ControllerState::new` | 20 | +0x0c/+0x34/+0x38/+0xa0/+0x15c..+0x184 + pose cache |
| `FUN_390964e0` controller reset | `controller::ControllerState::reset` | 20 dirty states | all fields + pose cache |
| `FUN_390a3de0` kinematic integrator | `controller::integrate_kinematic` | 20 | bit-exact quat/pos/world velocity |
| `FUN_394045f0` non-foot send pump | `send::pump_decision` | 20 timer cases | send/no-send and timer reset exact |
| `FUN_390e8e40` ship attach-point sink | `helm::apply_attach` | 20 (rowboat) | helm 7 floats + sail entries bit-exact |
| actor `vt+0x60` move fill (type 2) | (recorded only) | 20 | move struct bytes |
| actor `vt+0x60` move fill (type 4) | (recorded only) | 20 idle + 20 steering | throttle/steering i8 at +0x108/+0x109 |

The receive path (`OnOneUnitMovement` -> `NetActorMovementController`), the smoothing/interpolation
(`0x390b2420`/`0x390b0040`), the pose applies (`0x390af740`/`0x390eed80`) and the visual chains
(rudder/sail/wheel/exhaust, turrets) remain for the next phase; the map entries above stay the
reference. The type-2/type-4 fill goldens are the oracle for the runtime `vt+0x60` method once it is
ported.

---

## 8.3 Phase-B corrections found while porting

1. **The non-foot pump has no change suppression.** `FUN_394045f0` only checks the maximum
   interval (`DAT_3a59f84c`, live 100 ms in the lane); the min-interval/0.01 m change test belongs
   to the on-foot pump `FUN_394046f0`. §2.6 step 3/4 is corrected accordingly.
2. **The move fill is an actor virtual, not a GameClient virtual.** `FUN_394045f0` calls
   `FUN_390b3750(ECX=unit)` -> `*(unit+0x3be8)`, and `[that vtable + 0x60]` fills the move.
   Live type 2 (wagon) and type 4 (rowboat) fills are recorded.
3. **Type-4 request mapping:** `move+0x108 = i8 throttle`, `move+0x109 = i8 steering`; idle gives
   throttle 127/steering 0, holding `a` gives throttle 0/steering 129 (`-127` i8).
4. **The helm sink never writes a new sail entry's key.** A fresh entry keeps key 0, so point 0x15
   always reuses the first key-0 slot while point 0x16 appends a key-0 entry on every call; the
   live goldens show the array grow (2 -> 3 -> 4). `helm::apply_attach` reproduces this.
5. **The helm sink is ship-only in practice.** Calling `FUN_390e8e40` on the Farm Wagon crashes
   inside `FUN_390db5e0` (it reads the ship-only `+0x330` list); the live capture uses a rowboat.
6. **Kinematic quaternion product order:** `x = ((dz*qy - dy*qz) + dx*qw) + dw*qx` (the earlier
   draft used `dy*qx`, which only matched y/z/w). The other three terms are
   `y = ((qz*dx - dz*qx) + dy*qw) + dw*qy`, `z = ((dy*qx - qy*dx) + dz*qw) + dw*qz`,
   `w = dw*qw - ((dy*qy + dx*qx) + dz*qz)`.
7. **The controller pose cache is a `FixedPos`:** 20 bytes (`i64 x, i64 y, f32 z`) plus four bytes
   of untouched stack garbage. `PoseCache` models the three values.
8. **`FUN_390acee0`'s first callee is `0x3909c1f0`**, a 5-byte jump to the cryphysics thunk
   `0x3a9c0607`; the `vt+0x34` check is a model vtable slot called with `ECX = M` and the
   can-fly check re-reads the row through the entity accessor `+0x1e0`.

### 8.4 Port status (branch `port/vehicles-c`)

Phase C is ported in `crates/aa-client/src/port/vehicles/` and proven against the
original in the veh-golden lane (`capture_vehicle_phase_c.py`,
`capture_vehicle_phase_c_synth.py`):

| original | Rust | live cases | result |
|---|---|---|---|
| `FUN_3961de90` fixed -> world | `fixed::fixed_to_world` | 20 | bit-exact doubles (`2^-44`) |
| `FUN_390c63c0` wagon actor apply (U1) | `apply::wagon_pose_entry` / dispatch | 20 | 0x38 pose entries byte-exact |
| `FUN_390b2e30` net-controller mark | `apply::move_moving` (+0x154) | 20 | exact vs recorded `moving` |
| `FUN_3903bd80` 3x4 * 3x3 | `visual::mat3x4_mul` | 20 | bit-exact |
| `FUN_390c7410` rudder | `visual::rudder_transform` | 20 | bit-exact child basis |
| `FUN_39094330`, `FUN_390af550` (U10) | `step::empty_stub` | 20 | both are single `ret` leaves |
| `FUN_390ac300` CryPhysics thunk | `step::cry_physics_step` | 20 | explicit call record |
| `FUN_391d4e70` receive dispatch | `recv::receive_action` | 20 | state hook + actor slot |
| `FUN_391e55c0`/`FUN_391d4d60` queue | `recv::queue_batch` / `flush_queue` | 20 | 0x1000 limit, batch stride |
| `FUN_390b2420` timeout / `FUN_390b0040` snap | `timeout::net_update` / `within_snap_distance` | decompile-derived | 5000 ms, 400 m^2 |
| `FUN_3909c660` wheeled values | `step::wheeled_values` / `wheeled_physics_calls` | decompile-derived | call order + explicit values |

Still open from this phase: the helm wheel quantisation (`FUN_390d2990`), the turret
bone writer (`FUN_390d14b0`), the wheel-bone scan (`FUN_390e7b70`) and the slerp body
of `FUN_390eed80`; they need a live ship model/skeleton and are not claimed as ported.

---

## 9. Differential test plan (summary)

Calling convention, object resolution and per-group recipes: [`difftest.md`](difftest.md).
Highlights:

* Pure leaves (Phase 0) need only `calloc`'d state: no game objects. Call the function pointer from
  `GetModuleHandleA("x2game.dll") + (va - 0x38FF0000)`.
* Stateful model functions need a live model. Resolve it from the local player actor
  (`FUN_39360020(my_id)`), then the parent/seat unit, then the model pointer the same way the
  client does (`actor+0x3af0` -> parent actor; the model is reached through the unit model accessor
  used at 0x390e8b30 / 0x390f0510). `difftest.md` gives the exact pointer chain and a fallback
  hook.
* The move serializer `0x397c0470` is the single highest-value test: it fully defines the packet
  body for all five types and is pure given a synthetic archive object (the archive vtable is the
  standard CryEngine `Serialization::IArchive`; a minimal test double with the used slots
  `+0x1c` (is-reading), `+0x3c/0x40/0x44/0x4c/0x50/0x54/0x74/0x78` writers is described in
  `difftest.md`).
* `0x394045f0` is tested by hooking the send thunk and comparing the `0x130` bytes (goldens), then
  calling the factory `GameClient->vtable[0x60]` is not needed.
* No test allocates a cryphysics world: the physics side is `aaemu-physics`' own oracle.

---

## 10. Unknowns

| # | unknown | why | next step |
|---|---|---|---|
| U1 | move apply `vt+0x40` / move fill `vt+0x60` | **resolved**: the apply slot is the actor's `vt+0x40` = `0x390c63c0` (wheeled) / `0x390c2040` (ship). The wheeled path assembles the 0x38 pose entry and calls `0x390b2e30`; 20 live entries are byte-exact (`apply.json`). | none for the wheeled path; ship type-3/4 routing is ported as `apply::ship_apply` |
| U2 | send max/min intervals (`DAT_3a59f84c`, `DAT_3a59f850`) | **max resolved**: `DAT_3a59f84c->vt[8]()` returned 100 ms in the veh-golden lane; the non-foot pump has no min-interval/change test. `DAT_3a59f850` belongs to the foot pump and is not read here. | none for the non-foot pump |
| U3 | type-4 field mapping and the controller `+0x1b8/+0x1bc` writer | **mapping resolved**: the actor fill writes throttle at `move+0x108`, steering at `move+0x109` (idle 127/0, `a` 0/-127). The controller write instruction is still inside the unported apply path. | port the apply path; capture `+0x1b8/+0x1bc` while steering |
| U4 | controller drive-state field meanings `+0x15c..+0x184` | **resolved for the wheeled controller**: nothing in x2game writes the block; the values are filled by the CryPhysics vehicle as the controller is stepped. Live sampling: `+0x15c` steer (-1..1), `+0x16c..+0x174` angular velocity, `+0x178..+0x180` linear velocity, `+0x184` drive sign (+1 forward, -1 reverse); `FUN_390967c0` filters the target velocity into `+0x178..`. | the explicit values are `aaemu-physics`' inputs; `step::wheeled_values` records the client-side half |
| U5 | does the client activate the local controller for ships? | **observed yes**: a rowboat has `M+0x29c` (local controller) and `M+0x2a0` (net controller) populated; the same activation path is proven live on the wagon. | none |
| U6 | turret angle limits (`+0x58..+0x68`) are not applied in `0x390d14b0` | the clamp may live in the aim producer (player aim `+0x2cc/+0x2d0`) | trace writes to model `+0x2cc/+0x2d0` |
| U7 | water lookup in the boat placement ghost | `FUN_39348bf0` only does distance + OBB; the summon skill/server may enforce water | trace the skill placement path or measure a ghost over land (owner oracle) |
| U8 | transfer path-point indexing semantics (`type 5` `pathPointIndex`, `speed`, `reverse`) | the client displays but does not integrate; the exact mapping to the spline segment is not read | read `0x393524a0`/`0x39353c50` with a live transfer, or treat the server pose as authoritative |
| U9 | moving deck collision surface for passengers | the client parents the rider; no local collision query was found | owner test on a moving ship; compare with Bevy's parented transform |
| U10 | `0x39094330` (controller vtable +0xc) and `0x390af550` | **resolved**: both are single-byte `ret` leaves (empty virtuals); `0x39094340` beside them is `math::copy12` | none |
| U13 | the unit of the 64-bit move position (`S+0x08`) | **resolved**: `FUN_3961de90` scales by `DAT_3a5e1098 = 2^-44`; one wire step (`high_dword >> 3`) is `2^-9 m` (1.953125 mm). 20 live conversions are bit-exact (`fixed.json`). | none |
| U15 | sail entry key semantics | **resolved**: the sink never writes the key of an appended entry (stays 0); point 0x16 appends every call, point 0x15 reuses the first key-0 slot | none |
| U16 | helm sink model requirements | **resolved**: calling it on a non-ship model crashes in `FUN_390db5e0` (ship-only `+0x330` list); use a ShipModel for live tests | none |
| U17 | receive/smoothing/visual chains | **partly resolved**: receive dispatch/queue, the 5000 ms timeout, the 400 m^2 snap, the wagon pose apply and the rudder chain are ported and tested. Helm wheel, turret, wheel builder and the `FUN_390eed80` slerp body stay open (need a live ship skeleton). | capture on a live ship model in the veh-golden lane |
| U11 | who fires `SLAVE_SHIP_BOARDING/UNBOARDING` (0x67/0x68) | ids exist in the event table, no fire site found | hook the Lua event dispatcher `0x39424220` |
| U12 | `FUN_390c22c0` wind-area target (which entity/wind field) | partially read (weather manager + 3D engine wind) | owner test in windy weather |
| U14 | list-entry overlap: stride 0x130 but the move at entry+0x18 writes up to +0x124 | observed in `FUN_397c5e70`/`FUN_391e55c0`; harmless for types 0..4, overlaps the next entry for type 5 | capture a multi-move list; compare the queued copies |

---

## 11. Corrections and additions to the earlier notes

1. **Velocity scale**: type 2/3/5 use **30.0**, not 50 (`re-transport.md` §9 row "type 5"). Type 0
   is the 50 one (`0x3999ddc8`). Type 1 is 60 (`0x3999d01c`). [R]
2. **Position wire format**: 9 bytes (`ix0..iz2`), 24-bit fixed point at 1/8 unit, z biased by
   4196*2.384e-7 and −100 [R: `FUN_3961e2c0` + keys]. Earlier notes did not describe it.
3. **`FUN_390e8e40` is the attach-point state sink**, not just a doodad hook: point 35 (helm)
   carries 7 floats into `+0x35c..+0x374`; points 21/22 carry the sail entry [R]. Use this for the
   helm visual replay.
4. **`UpdateSteeringWheel` skips only `smallboat/soloship/coupleduckship`** (by the type name at
   row `+0x3c`), not all small hulls [R].
5. **`GetSiegeWeaponSpeed` requires model state 2 and GameClient mode 3/4**, not just "mounted"
   [R: 0x394aabd0].
6. **No ship "sail control" skills**: confirmed again; the sails are driven only by speed/wind
   [R: no mount-skill rows; `UpdateSail` is speed/wind only].
7. **The transfer boarding packet is `0x67`** with `{tl u16, ap u8}`; the leave path uses the same
   opcode with the transfer's object id and `ap = 0`, sent from `0x39440d30`; the free point search
   is `0x393546b0` (2..7, first free) [R].
8. **The precheck errors** are confirmed with their message ids: 0x144 dead, 0x13 busy, 0x9a
   range, 0x66 occupied [R: 0x39346290].
9. **Move struct offsets corrected**: the position is the 20-byte fixed at `S+0x08` (not three
   floats at +0x30); the quaternion is at +0x20 and the velocity at +0x30. See `packets.md`
   §1 and the disassembly of `FUN_397c0470` (0x397c0524 `lea ecx,[esi+8]`, 0x397c05f3
   `lea edi,[esi+0x20]`, 0x397c05f6 `movq [esi+0x30]`).
10. **The make-up of the move position** is an engine 64-bit fixed (`i64 x, i64 y, f32 z`) that
    `FUN_3961dd70`/`FUN_3961ddd0` add to the entity transform and `FUN_3961de30` converts with
    2^-44 (`0x399f9b80`); the move struct at `S+8` is that type, and the wire codec truncates it
    by 3 bits per axis. The fixed's exact unit is U13.
11. **The move serializer's `angVel` is one archive call on slot `+0x64`, not three `+0x74`
    calls**: `FUN_397c0470` writes `vehicle.angVel`, `ship.angVel` and `transfer.angVel` as a
    single float-3 field (`0x397c0b5f` `mov edx,[edx+0x64]`). The earlier recipe in
    `difftest.md` omitted `+0x64`; phase A uses it and the goldens record one 12-byte entry.
12. **`FUN_397ad740` takes a unit delta**: it clamps the raw input to [-1, 1] and multiplies by
    127; it does not pre-scale by 1/127 (the reader multiplies by 1/127). [R: 0x397ad740]
13. **Three labels in `functions.md` were wrong** (found while porting): `FUN_390bf0f0` builds a
    3×4 frame from a quaternion + translation (not axis-angle), `FUN_390bf210` initialises a large
    object (not a quaternion multiply), and `FUN_390699b0` is a Lua stack/value helper (not the
    round-to-0.1 leaf). The round-to-0.1 function used by the UI getters is still unlocated.
