# Rust structs and observed offsets (S3)

These are the layouts the port needs. Offsets are the **decompile's** offsets for the `this` of
the listed functions. Two object views recur and must not be mixed:

* **M** = the unit-model subobject: the `this` of `ShipUnitModel::Update` / `VehicleUnitModel::Update`
  / the `Update*` visual functions. Its vtable is `0x399a5930` (ship) / `0x399a5e08` (vehicle)
  (24 slots).
* **F** = the full allocation (`0x3a8` bytes) returned by the factory `FUN_390e95a0`; the factory
  returns **F+8 = M**. The model base constructor `FUN_390e8330` runs with `this = F`, and the
  local controller is handed `F-8`? No: `0x390c1b80` pushes `M-8 = F` to the controller ctor
  (`lea ecx,[esi-8]` at 0x390c1c1c, where `esi = M`). So the controller's owner pointer is **F**,
  and offsets shift by 8 between the controller's view and M.

The fields below are the ones actually read/written by S3 code; they are not a complete class
layout, and padding is preserved only where the move struct requires it.

## 1. `MoveUnit` (0x130 bytes) — `packets.md` for the wire layout

```rust
#[repr(C)]
pub struct MoveUnit {
    pub move_type: u8,          // +0x00
    _pad0: [u8; 3],
    pub time: u32,              // +0x04
    pub pos_fixed: FixedPos,    // +0x08  { i64 x, i64 y, f32 z } (20 B engine fixed type)
    pub rot: [f32; 4],          // +0x20  quat x,y,z,w
    pub vel: [f32; 3],          // +0x30
    pub flags: u8,              // +0x3c
    _pad1: [u8; 3],
    pub sc_type: u16,           // +0x40 (if flags & 0x10)
    pub phase: u8,              // +0x44
    _pad2: [u8; 0x4b],          // to +0x90
    // union of the type-specific tails:
    pub vehicle: VehicleTail,   // +0x98
    _pad3: [u8; 0x2c],          // tail padding to 0x130
}

#[repr(C)]
pub struct FixedPos {           // the engine's 64-bit fixed-position delta
    pub x: i64,                 // +0x08
    pub y: i64,                 // +0x10
    pub z: f32,                 // +0x18 (world metres)
}
```

`MoveUnit` is **0x130** bytes as a queue/slot record; the wire `unitId` is a 3-byte prefix in the
packet shells (`packets.md` §10), not a field of this struct. The fixed unit is U13 (open).

```rust

#[repr(C)]
pub struct VehicleTail {        // type 2
    pub ang_vel: [f32; 3],      // +0x98
    pub steering: f32,          // +0xa4
    pub wheel_count: u8,        // +0xa8
    _pad: [u8; 3],
    pub wheel_ang_vel: [f32; 18], // +0xac (max 18 used by the apply clamp)
}

#[repr(C)]
pub struct ShipTail {           // type 3
    pub ang_vel: [f32; 3],      // +0xf4
    pub steering: i8,           // +0x100
    pub throttle: i8,           // +0x101
    pub zone_id: u16,           // +0x102
    pub stuck: u8,              // +0x104
    _pad: [u8; 3],
    pub request_throttle: i8,   // +0x108 (type 4)
    pub request_steering: i8,   // +0x109
    _pad2: [u8; 2],
    pub transfer: TransferTail, // type 5
}

#[repr(C)]
pub struct TransferTail {
    pub ang_vel: [f32; 3],      // +0x10c
    pub steering: i32,          // +0x118
    pub path_point: i32,        // +0x11c
    pub speed: f32,             // +0x120
    pub reverse: u8,            // +0x124
}
```

The exact tail offsets are verified by `FUN_397c0470` (field keys) and `FUN_39024610` (copy
sequence: it copies `+0x94..+0x124` dword-wise; the fields above are the ones it names).

## 2. `VehicleModelRow` (0xbc) — `constants.md` §2

```rust
#[repr(C)]
pub struct VehicleModelRow {
    pub ang_vel: f32,                    // +0x00
    pub auto_level: u8,                  // +0x04
    pub can_fly: u8,                     // +0x05
    _pad0: [u8; 2],
    pub damaged25: u32,                  // +0x08 (string id)
    pub damaged50: u32,                  // +0x0c
    pub damaged75: u32,                  // +0x10
    pub dead: u32,                       // +0x14
    pub driver_walk: u8,                 // +0x18
    _pad1: [u8; 3],
    pub dying: u32,                      // +0x1c
    pub floating_height: f32,            // +0x20
    pub floating_wave_height: f32,       // +0x24
    pub floating_wave_period_ratio: f32, // +0x28
    pub installed_turret: u8,            // +0x2c
    _pad2: [u8; 3],
    pub lin_deaccel_inertia: f32,        // +0x30
    pub lin_inertia: f32,                // +0x34
    pub max_climb_ang: f32,              // +0x38
    pub normal: u32,                     // +0x3c
    pub rot_deaccel_inertia: f32,        // +0x40
    pub rot_inertia: f32,                // +0x44
    pub sound_id: u32,                   // +0x48
    pub susp_axle: u8,                   // +0x4c
    _pad3: [u8; 3],
    pub susp_stroke: f32,                // +0x50
    pub trail_align_ratio: f32,          // +0x54
    pub turret_pitch_max: f32,           // +0x58
    pub turret_pitch_min: f32,           // +0x5c
    pub turret_pitch_angvel: f32,        // +0x60
    pub turret_yaw_max: f32,             // +0x64
    pub turret_yaw_min: f32,             // +0x68
    pub turret_yaw_angvel: f32,          // +0x6c
    pub use_center_spindle: u8,          // +0x70
    pub use_proxy_collision: u8,         // +0x71
    pub use_wheeled_vehicle_simulation: u8, // +0x72
    _pad4: [u8; 1],
    pub velocity: f32,                   // +0x74
    pub wheel: u32,                      // +0x78 (cgf string id)
    pub wheel2: u32,                     // +0x7c
    pub ballast_mass: f32,               // +0x80
    pub ballast_pos_y: f32,              // +0x84
    pub brake_torque: f32,               // +0x88
    pub drive: u32,                      // +0x8c (0 FWD, 1 RWD, 2 AWD)
    pub front_optimal_sa: f32,           // +0x90
    pub gear_ratio: [f32; 3],            // +0x94..+0x9c
    pub gear_ratio_reverse: f32,         // +0xa0
    pub mass: f32,                       // +0xa4
    pub max_gear: u32,                   // +0xa8
    pub power: f32,                      // +0xac
    pub rear_optimal_sa: f32,            // +0xb0
    pub susp_damping: f32,               // +0xb4
    pub susp_stroke_wheeled: f32,        // +0xb8
}
```

`FUN_39736bd0` stores string ids for the model paths and resolves them with `FUN_397a6c00`; in
Rust use `String`/`Arc<str>` and resolve at load.

## 3. `UnitModel` (M) — visual state

| off | type | field | evidence |
|---|---|---|---|
| +0x00 | ptr | vtable (ship 0x399a5930, vehicle 0x399a5e08) | vtable dumps |
| +0x08 | object | embedded entity interface (vtable at `*(M+8)`) | `0x390cffd0` prologue |
| +0x14 | ptr | unit/actor | `0x390e8b30`, `0x390f0510` |
| +0x100 | u32 | type-row handle | `0x390d2990` |
| +0x134..+0x140 | f32×4 | last applied velocity (x,y,z,?) | `0x390b0040` |
| +0x144..+0x14c | f32×3 | last applied position | `0x390b0040` |
| +0x148..+0x160 | f32×7 | spindle/wheel world poses | `0x390cffd0` |
| +0x188.. | f32×? | target pose/velocity cache | `0x390b0040` |
| +0x1f0 | u32 | model damage state (1..6) | `0x390eb610`, `0x390e8b30` |
| +0x138..+0x13c | ptr×2 | attach-point array begin/end (0x28 stride, 10 dwords) | `0x390d2990`: `[M+0x138]`, `[M+0x13c]` |
| +0x238 | ptr | effect/particle host (unnamed; gated in `0x390e8d70`) | `0x390e8d70` |
| +0x298 | u8 | `use_wheeled_vehicle_simulation` | `0x390f0510` |
| +0x29c | ptr | local vehicle movement controller | `0x390c1b80` |
| +0x2a0 | ptr | NetActorMovementController | `0x390c1b80`; base ctor `F+0x2a8` |
| +0x2a4..+0x2ac | ptr×3 | sail entry vector (0x48 stride) | `0x390d1c60`, `0x390e8e40` |
| +0x2ac..+0x2b0 | ptr×3 | wheel entry vector (0x9c stride) | `0x390d6b10`, `0x390cf180` |
| +0x2c0..+0x2c4 | ptr×3 | exhaust attachment vector | `0x390c78d0` |
| +0x2d0..+0x2d4 | ptr×2 | sail flag bone indices | `0x390d1c60` |
| +0x2e0..+0x2e4 | ptr×3 | ripple point vector (0xc stride) | `0x390c7640` |
| +0x2f0..+0x2f4 | ptr×3 | bubble/emitter vector | `0x390c78d0` |
| +0x330 | f32 | current speed | `0x390af740` |
| +0x358 | f32 | throttle (ship) | `0x390be0d0`, `0x390db700` |
| +0x35c | f32 | steering (ship) | `0x390be0b0`, `0x390c7410`, `0x390d2990` |
| +0x360 | f32 | helm wheel angle (smoothed) | `0x390d2990` |
| +0x364..+0x374 | f32×5 | helm doodad transform | `0x390e8e40`, `0x390d2990` |
| +0x380 | u8 | stuck flag | `0x390be0f0` |
| +0x384 | u16 | zone id | `0x390be100` |
| +0x39c | u8 | throttle-anim gate | `0x390db700` |
| +0x3a0 | f32 | last throttle | `0x390db700` |
| +0x3f8 | f32 | center-spindle angle | `0x390cffd0` |
| +0x3fc | f32 | steering-wheel angle | `0x390d0800` |

Caveat: `+0x2a4..+0x2b0` (sails) and `+0x2ac..+0x2b0` (wheels) overlap as *vectors* but belong to
different subclasses; a Rust enum (`ShipModel` vs `VehicleModel`) avoids the overlap. The port
should not use one flat struct for both.

## 4. `LocalVehicleController` (C, 0x1a0) — vtable 0x399a316c

| off | type | field | evidence |
|---|---|---|---|
| +0x00 | ptr | vtable | ctor `0x390abe10` |
| +0x08 | ptr | owner F (model block, = M-8) | ctor `0x390abe10`, caller `0x390c1b80` |
| +0x0c | — | zeroed in reset | `0x390964e0` |
| +0x34 | — | zeroed in reset | `0x390964e0` |
| +0x38 | i32 | -1 sentinel | `0x390964e0` |
| +0x15c | f32 | drive/steer magnitude | `0x390967c0`, `0x3909c660` |
| +0x160 | f32 | throttle/gear accumulator | `0x3909c660` |
| +0x164, +0x168 | f32 | steering/wheel accumulators | `0x3909c660` |
| +0x16c, +0x170, +0x174 | f32 | angular velocity (pitch,yaw,roll) | `0x390a3de0` |
| +0x178, +0x17c, +0x180 | f32 | linear velocity | `0x390a3de0` |
| +0x184 | f32 | 1.0 default | ctor |
| +0x188, +0x190, +0x198 | f32 pairs | pose cache | `0x3909c660` |
| +0x1a0 | — | end of object | allocation size 0x1a0 |

## 5. `NetActorMovementController` (N) — vtable 0x399a3368

| off | type | field | evidence |
|---|---|---|---|
| +0x00 | ptr | vtable | `0x390b2260` |
| +0x04 | ptr | owner | ctor |
| +0x104 | u32 | (vehicle) state | `0x390b0040` |
| +0x108, +0x10a | u32/u16 | zone/extra | `0x390b0040` |
| +0x10c | u8 | stuck | `0x390b0040` |
| +0x130..+0x14c | f32 | applied pose/velocity | `0x390b0040` |
| +0x18c | ptr/alloc | wheel-entry vector (0x9c) | `0x390afe10` |
| +0x1a4 | f32 | applied step | `0x390afe10` |
| +0x1a8, +0x1b0 | f32×3 | target position | `0x390b0040` |
| +0x1b4 | f32 | ? | `0x390afe10` |
| +0x1b8 | f32 | steering | `0x390b0040` |
| +0x1bc | f32 | throttle | `0x390b0040` |
| +0x4c | u32 | interpolation timeout start (ms) | `0x390b2420` |

## 6. Attachment and seats

```rust
pub struct AttachmentNode { // GameClient+0x120 list
    pub next: *mut AttachmentNode,
    pub prev: *mut AttachmentNode,
    pub entity: u32,        // node[2] = entity/unit pointer or id
    pub point: u32,         // node[3] = attach point id
}
```

* `FUN_390dea00` adds `{entity, point}` to `GameClient+0x120`; `FUN_390c9f40` finds by point.
* `X2::Unit::AttachTo` (`0x390b3f10`) stores parent at actor `+0x3af0` and point at `+0x3af4`;
  `DAT_3a59f858` = no parent.
* `OnUnitAttached` (`0x391d49d0`) reads child `+0xc`, parent `+0x10`, payload `+0x18`.
* `OnSlaveBound` (`0x391d4760`) stores the own-seat values on the player:
  `+0x10 = &DAT_3a168a98`, `+0x14 = slave tl id`, `+0x18 = slave object id`.

## 7. Transfer

```rust
pub struct Transfer {
    pub model_id: u32,
    pub wait_time: f32,
    pub cyclic: bool,
    pub path_smoothing: f32,
    // runtime:
    pub path: Vec<TransferLeg>,
    pub moved: bool,           // +0x180
    pub rider_state: [RiderState; 5], // +0x6c.. (7 dwords each)
}

pub struct TransferLeg {
    pub path_name: String,     // transfer_paths.path_name
    pub points: Vec<[f32; 3]>, // transfer_path.xml <Point Pos>
    pub wait_start: f32,
    pub wait_end: f32,
}
```

DB columns reused from `re-transport.md` §7; the client-side runtime fields come from
`0x393524a0` (`+0x180`), `0x393540a0` and the path parser `0x393536c0`.

## 8. Ship equipment / turret

* Slot/equipment data is DB-driven (`re-transport.md` §3), not a client struct in S3.
* Turret aim is model state `+0x2cc` (yaw) and `+0x2d0` (pitch), rates/limits from
  `VehicleModelRow` `+0x58..+0x6c`.
* Cannon hull points come from `slave_bindings`; the gunner is bound by `CSBindSlave` for the
  cannon slave's tl id (no separate cannon struct in x2game).
