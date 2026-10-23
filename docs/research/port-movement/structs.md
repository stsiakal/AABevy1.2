# Struct layouts for the port

All offsets are **1.2 x2game** offsets (base `0x38FF0000`) unless marked otherwise. "R" = read
from the decompile/capstone, "I" = inferred from use. These are the Rust `#[repr(C)]` shapes the
differential harness must pass to the original functions; keep them in a single
`aa-movement` crate so the addresses and field order cannot drift.

## 1. InputState (per controllable unit; `0x39404d50` `this`, `0x394046f0` `param_1`)

Size >= 0xC8; allocated/owned by `X2::GameClient::InputManager`. `FUN_39403850` constructs it
(0x39403850: object id at +0, `this+0xd` = 0, `this+0x21` = 0, `this+0x2d` = -1, pointer at
`+0xc4`).

```rust
#[repr(C)]
pub struct InputState {
    pub unit_id: u32,            // +0x00  R (FUN_39360020(*this))
    pub send_timer: f32,         // +0x04  R accumulated dt, reset on send (39406650)
    pub flag_08: u8,             // +0x08  R cleared with +0x09 as a u16 at 0x39404cd0
    pub flag_09: u8,             // +0x09  R -> CSMoveUnit flag 0x20
    pub flag_0a: u8,             // +0x0a  R -> CSMoveUnit flag 0x80
    pub unknown_0b: u8,          // +0x0b
    pub requested_vel: [f32; 3], // +0x0c  R smoothed input velocity (FUN_39402c50 result)
    pub external_vel:  [f32; 3], // +0x18  R from the external target path (local_80/local_78)
    pub smoothed_vel:  [f32; 3], // +0x24  R FUN_39402c50 state
    pub unknown_30: f32,         // +0x30
    pub accel: f32,              // +0x34  R smoothing factor, default 30.0
    pub turn_accum: f32,         // +0x38  R mounted keyboard-turn ramp (FUN_39402e00 this+0x38)
    pub autorun_or_lock: u8,     // +0x3c  R byte set to 0/1 (0x39405251)
    pub unknown_3d: [u8; 3],     // +0x3d
    pub timer_40: f32,           // +0x40  R accumulates mouse axes when turn starts (39405249)
    pub last_quat: [f32; 4],     // +0x44  R last sent rotation quaternion
    pub last_pos:  [f32; 3],     // +0x54  R last sent position
    pub last_rot:  [f32; 3],     // +0x60  R last sent rotation
    pub last_vel:  [f32; 3],     // +0x6c  R last sent velocity
    pub last_stance: u8,         // +0x78  R
    pub last_flags: u8,          // +0x79  R
    pub pad_7a: u16,             // +0x7a
    pub last_fall_vel: f32,      // +0x7c  R
    pub last_support_id: u32,    // +0x80  R
    pub input_flags: u32,        // +0x84  R bit0 moving, 0x800 air, 0x1000 walk, 0x2000 ...
    pub prev_axes: [u8; 4],      // +0x88..0x8b R (copies of 0x8a..0x8d)
    pub axis_c: u8,              // +0x8c
    pub axis_d: u8,              // +0x8d
    pub axis_e: u8,              // +0x8e
    pub axis_f: u8,              // +0x8f
    pub axis_g: u8,              // +0x90
    pub axis_h: u8,              // +0x91
    pub pad_92: u16,             // +0x92
    pub axis_vals: [f32; 2],     // +0x94/+0x98 R (copies of local_64/local_68)
    pub press_time_l: f32,       // +0x9c  R FUN_39402690
    pub press_time_r: f32,       // +0xa0
    pub press_time_f: f32,       // +0xa4
    pub press_time_b: f32,       // +0xa8
    pub turn_l_start: f32,       // +0xac  R game time when turnleft went non-zero
    pub turn_r_start: f32,       // +0xb0  R same for turnright (used to pick the newer axis)
    pub goto_unit_id: u32,       // +0xb4  R external target unit id (0 = none)
    pub flags_b8: u8,            // +0xb8  R -> CSMoveUnit flag 0x02
    pub unknown_b9: u8,          // +0xb9
    pub unknown_ba: u8,          // +0xba
    pub moving: u8,              // +0xbb  R derived "moving" for autorun
    pub hold_fwd: u8,            // +0xbc  R (esi+0xbc in 394050db)
    pub hold_back: u8,           // +0xbd
    pub hold_left: u8,           // +0xbe
    pub hold_right: u8,          // +0xbf
    pub hold_timer: f32,         // +0xc0
    pub latch: *mut u32,         // +0xc4  R pair of ids (FUN_39403850 allocates 8 bytes)
}```

The mount keyboard-turn ramp is `turn_accum` at **`+0x38` of this state** (`FUN_39402e00`
`this+0x38`; see MAP §3.2). It is reset to 0 whenever the turn axis is zero.

## 2. InputManager / per-unit wrapper

`FUN_39406540`'s `this` is a two-field wrapper `{ u8 paused; *mut InputState state }`
(0x394065d9–0x394065e3: `cmp byte [esi],0`; `lea ecx,[esi+4]`). `FUN_39404d50` and
`FUN_39402e00` are called with `ECX = state` (the InputState), **not** the InputManager.
The mount keyboard-turn ramp lives in `InputState+0x38`.

```rust
#[repr(C)]
pub struct InputBinding {
    pub paused: u8,               // +0x00
    pub pad: [u8; 3],
    pub state: InputState,        // +0x04  inline, not a pointer
}
```

The InputManager global fields are not needed by the movement port; the camera has its own
manager (`CameraManager`, §9).

## 3. MovementRequest (`0x9C` bytes, `FUN_39404d50` stack `[ebp-0x130]`)

Passed by pointer to `GameClient vtable +0x3C` at 0x394063a6. Default-initialised by
`FUN_390a8dc0` (0x39405fdd): sentinels `-1` at +0x28/+0x2c/+0x6c/+0x78/+0x84/+0x94, `0.5` at
+0x74, `1.0` at +0x5c, 0 at +0x98.
Field meanings are from the stores and the flag tests; bits not yet named in MAP §9 U2.

```rust
#[repr(C)]
pub struct MovementRequest {
    pub flags: u32,            // +0x00
    pub external_dir: [f32; 3],// +0x04  external move-to direction (only when goto_unit_id != 0)
    pub pad_10: [u8; 0x10],    // +0x10  includes 1.0f at +0x10 and type u32 at +0x1c
    pub velocity: [f32; 3],    // +0x20  smoothed input velocity
    pub delta1: [f32; 3],      // +0x2c  optional (flags & 0x40)
    pub delta2: [f32; 3],      // +0x38  optional (flags & 0x20)
    pub pad_44: [u8; 0x50],    // +0x44
    pub unit_kind: u32,        // +0x94  0 normal, 2 mounted/mate
    pub pseudo_speed: f32,     // +0x98  0 / 1.0 / 0.4 (walk bit 0x1000)
}
```

`+0x10` and `+0x1c` need a live dump to be named (U2). The request is **not** the packet body;
the packet body is a 0x128-byte struct (`FUN_39024610`) filled by `GameClient vtable +0x60`.

## 4. Actor (X2::CPlayer / CActor) fields used by movement

The actor is a large, mostly unknown object. Do **not** mirror it field for field in Rust; use a
partial struct with explicit padding up to the fields you need, or access it through
`offset_of`-style helpers. Ascending offset table (R = read from the decompile/capstone):

| off | type | meaning |
|---|---|---|
| +0x0c | `IEntity*` | `entity`; `IPhysicalEntity*` = `entity->vtable[0x108]()` |
| +0x24 | ptr | non-null test in `CPlayer::PrePhysicsUpdate` |
| +0x2c | ptr | input object (`param_1[0xb]`; thunks `3a982448`, `3a979a34`) |
| +0x38 | ptr | post-hook object (`FUN_39113f60`) |
| +0x40 | f32 | stride (`CPlayerMovement+0x5c`) |
| +0x50 | ptr | animation object (`CPlayerMovement+0x08`) |
| +0x78 | u8 | flag copied to `CPlayerMovement+0xb9` |
| +0x79 | u8 | flag copied to `CPlayerMovement+0xb8` |
| +0x84 | f32 | relative water level (`UpdateSwimStats`) |
| +0x8c..0xa0 | f32[6] | swim control vectors |
| +0x118 | f32[3] | movement anchor (swim/controller ctor `3912e0c0`) |
| +0x154..0x174 | f32[8] | viewSensitivity / viewLimitPitch / viewLimitYaw / viewLimitDir / speedMultiplier / viewHeightOffset / viewDistance / viewPivot (Lua `gameParams`, `FUN_3911a5a0`) |
| +0x174 | f32 | turn speed (read by `FUN_39402e00` on the controlled unit) |
| +0x1c1 | u8 | unit kind; 4 = quadruped/mount |
| +0x1db | u8 | `isOnLadder` (0x101 while on a ladder) |
| +0x1dc..0x1df | u8[4] | ladder flags |
| +0x238 | u32 | ladder entity id |
| +0x23c | u32 | ladder kind: 4 net, 5 hrope, 7 mast, 8 guardrail, 9 45deg, 10 bigtree |
| +0x244..0x280 | f32[16] | ladder frame (4x4) |
| +0x284 | f32/ptr | copied to `CPlayerMovement+0xbc` |
| +0x2b0..0x2f0 | f32[16] | four movement quaternions (`CPlayerMovement+0x44/+0x48`, `CPlayerRotation`) |
| +0x2e0 | f32[3] | movement velocity (`CPlayerMovement+0x94`) |
| +0x2e8 | f32 | movement scalar |
| +0x2f0..0x300 | f32[4] | movement pose |
| +0x33c..0x35c | f32[8] | support/attachment quats (`CPlayerRotation+0x50`) |
| +0x410 | u32 | stance id |
| +0x960..0x968 | f32[3] | movement control (`CPlayerMovement+0x4c`) |
| +0x96c..0x978 | f32[6] | swim control (`CPlayerMovement+0xa0/+0xac`) |
| +0x9dc | u32 | copied to `CPlayerMovement+0xb4` |
| +0x1aa0 | u32 | goto/attached unit id (`FUN_39406650`, `FUN_39404d50`) |
| +0x3af0 | u32 | movement-info valid marker (CSMoveUnit guard) |
| +0x3bba | u8 | actor state flag |
| +0x3be8 | `GameClient*` | `FUN_390b3750` |
| +0x3c20..0x3c28 | u8[8] | entity flags; mounted at +0x3c26 |

Note the gaps: +0x35c..0x410, +0x414..0x1aa0, +0x1aa4..0x3af0 etc. are not modelled. Keep the
partial struct `#[repr(C)]` with explicit `[u8; N]` pads and add `const_assert` members to check
each offset at compile time.

The **movement block** inside the actor starts at `+0x3ec` (0xAC bytes) and is loaded by
`FUN_3911a5a0` (Lua fallback) and `FUN_3911b950` (DB record). DB record -> actor mapping
(0x3911b950) **[R]**:

| actor | DB column |
|---|---|
| +0x3f0 | max speed (f32 from int) |
| +0x3f4 | normal speed |
| +0x438 | sprint multiplier (`game_sprint_multiplier`) |
| +0x43c | forward diagonal |
| +0x440 | forward |
| +0x444 | strafe |
| +0x448 | backward diagonal |
| +0x44c | backward |
| +0x450 | grab |
| +0x454..0x460 | walk forward/forward-diag/strafe/backward-diag |
| +0x464 | walk backward |
| +0x468 | inertia |
| +0x46c | inertia accel |
| +0x470 | jump height |
| +0x474 | lean angle |
| +0x478 | lean shift |
| +0x494 | swim height |
| +0x498..0x4cc | other model params from 0xa4..0x104 |

## 5. CPlayerMovement (stack object built by `FUN_39135fe0`, `0x3a6ab5f3`)

Size 0xC0 (the wrapper reserves 0xE4 on the stack). Constructor `0x3a6ab5f3` (ECX=this,
`[ebp+8]`=actor, `[ebp+0xc]`=request/13-dword block, `[ebp+0x10]`=dt).

```rust
#[repr(C)]
pub struct CPlayerMovement {
    pub dt: f32,                 // +0x00
    pub movement_params: *mut MovementParams, // +0x04 = actor+0x3ec
    pub anim: *mut u8,           // +0x08 = actor+0x50
    pub request: [u32; 13],      // +0x0c..0x3f copied from [ebp+0xc] (0x34 bytes)
    pub actor: *mut Actor,       // +0x40
    pub quat_a: *mut [f32; 4],   // +0x44 = actor+0x2b0
    pub quat_b: *mut [f32; 4],   // +0x48 = actor+0x2d0
    pub quat_c: *mut [f32; 4],   // +0x4c = actor+0x960
    pub unknown_50: [u8; 0x0c],  // +0x50
    pub stride: f32,             // +0x5c = actor+0x40
    pub velocity: [f32; 3],      // +0x60..0x68
    pub accel: [f32; 3],         // +0x6c..0x74
    pub unknown_78: [u8; 0x08],  // +0x78
    pub one: f32,                // +0x80 = 1.0
    pub flag_84: u8,             // +0x84
    pub pad_85: [u8; 3],         // +0x85
    pub zero_88: u32,            // +0x88 = 0
    pub flag_8c: u8,             // +0x8c
    pub pad_8d: [u8; 3],         // +0x8d
    pub vec_90: [f32; 3],        // +0x90 = actor+0x2e0
    pub flag_9c: u8,             // +0x9c = actor+0x161
    pub pad_9d: [u8; 3],         // +0x9d
    pub swim_a: [f32; 2],        // +0xa0 = actor+0x96c
    pub swim_b: [f32; 2],        // +0xa8 = actor+0x974
    pub flag_b0: u8,             // +0xb0 (zero)
    pub pad_b1: [u8; 3],         // +0xb1
    pub val_b4: u32,             // +0xb4 = actor+0x9dc
    pub flag_b8: u8,             // +0xb8 = actor+0x79
    pub flag_b9: u8,             // +0xb9 = actor+0x78
    pub flag_ba: u8,             // +0xba
    pub pad_bb: u8,              // +0xbb
    pub val_bc: u32,             // +0xbc = actor+0x284
}```

## 6. CPlayerRotation (stack object, `0x3913ded0`)

Partial object; the ctor `0x3913ded0` (ECX=this, stack: actor, request, scale) fills these
groups:

| off | source / meaning |
|---|---|
| +0x00 | u32 scale |
| +0x04 | actor+0x3ec `MovementParams*` |
| +0x08 | actor+0x50 (`anim`) |
| +0x0c | actor pointer |
| +0x10 | f32 actor+0x40 (`stride`) |
| +0x14..0x20 | 3 floats from request+0x0c..0x14 (`move_quat`) |
| +0x20..0x50 | 12 floats from actor+0x2b0..0x2e0 (four movement quats) |
| +0x60..0x80 | 8 floats from actor+0x33c..0x35c (support quats) |
| +0x80..0x8c | 3 floats from actor+0x960 (`swim/move control`) |
| +0x8c..0x94 | 2 floats from actor+0x154 (`viewLimit` pair) |
| +0x94 | u32 from actor+0x15c (`viewLimitDir`) |
| +0x98 | end (size 0x9c) |

`ProcessNormal` `0x3913ff90` reads/writes the current/target quaternions at `+0x20..0x4c`,
the support quats at `+0x40..0x4c` (its `local_40`/`local_38`), and the output quat at
`+0x80..0x88`. `0x3913dfd0` writes `actor+0x950..0x95c` from `+0x50..0x5c`, normalises
`+0x40/+0x48` into `actor+0x2b0..0x2dc`, and `+0x60/+0x68` into `actor+0x33c..0x348`.

`0x3913dfd0` writes `actor+0x950..0x95c` from `this+0x50..0x5c`, normalises
`this+0x40/0x48` into `actor+0x2b0..0x2dc`, and `this+0x60/0x68` into `actor+0x33c..0x348`.

## 7. MovementParams (actor+0x3ec)

```rust
#[repr(C)]
pub struct MovementParams {
    pub max_speed: f32,          // +0x00 (actor+0x3ec, = actor+0x3f0? see note)
    pub normal_speed: f32,       // +0x04
    pub pad: [u8; 0x48],
    pub sprint: f32,             // +0x4c? (= actor+0x438)
    pub forward_diag: f32,
    pub forward: f32,
    pub strafe: f32,
    pub backward_diag: f32,
    pub backward: f32,
    pub grab: f32,
    pub walk_forward: f32,
    pub walk_forward_diag: f32,
    pub walk_strafe: f32,
    pub walk_backward_diag: f32,
    pub walk_backward: f32,
    pub inertia: f32,
    pub inertia_accel: f32,
    pub jump_height: f32,
    pub lean_angle: f32,
    pub lean_shift: f32,
    // ... swim_height at +0xa8 relative to 0x3ec (= actor+0x494)
}
```

Because the block base is `actor+0x3ec`, the DB field offsets in §4 are base+0x4c, +0x50, +0x54,
+0x58, +0x5c, +0x60, +0x64, +0x68..0x78, +0x7c, +0x80, +0x84, +0x88. Recompute them from the
table; do not hard-code both.

## 8. Quadruped controller (`0x3912c640` / `0x3912e0c0`)

```rust
#[repr(C)]
pub struct QuadrupedController {
    pub dt_or_scale: f32,        // +0x00
    pub movement_params: *mut MovementParams, // +0x04 = actor+0x3ec
    pub anim: *mut u8,           // +0x08 = actor+0x50
    pub request: [u32; 13],      // +0x0c..0x3f
    pub actor: *mut Actor,       // +0x40
    pub quat_a: *mut [f32; 4],   // +0x44 = actor+0x2b0
    pub quat_b: *mut [f32; 4],   // +0x48 = actor+0x2d0
    pub extra: *mut u8,          // +0x4c = actor+0x960 (c640) or actor+0x118 (e0c0)
    pub unknown_50: [u8; 0x10],
    pub stride: f32,             // +0x60 = actor+0x40
    pub velocity: [f32; 3],      // +0x64
    pub accel: [f32; 3],         // +0x70
    pub one: f32,                // +0x80
    // ... same tail as CPlayerMovement
}
```

The mount's graph inputs are written by `FUN_3912ac50` into `actor+0x2e0` (velocity),
`actor+0x96c/0x974` (swim), `actor+0x9dc`, and the rider coupling is `FUN_390dcc80`.

## 9. CameraManager (`0x39169860 this`)

Partial object; do not mirror it wholesale. Ascending offsets used by mode 1 / zoom / collision:

| off | type | meaning |
|---|---|---|
| +0x04 | u32 | mode (1 unit, 2 free, 3 target, 4, 6) |
| +0x20 | f32 | drawn distance |
| +0x24 | f32 | wanted distance |
| +0x28/+0x2c/+0x30/+0x34 | f32 | distance settle/fov accumulators |
| +0x3c..0x44 | f32[3] | camera world position (output) |
| +0x48 | f32 | debug/time value |
| +0x4c..0x68 | f32[7] | target/eye offsets and pivot |
| +0x68..0x7c | f32[5] | yaw basis/offsets |
| +0x80 | f32 | last character yaw |
| +0x84 | f32 | accumulated yaw offset |
| +0x88 | f32 | current yaw rate / pitch |
| +0x8c | f32 | total camera yaw (rad) |
| +0x90 | f32 | accumulated mouse pitch |
| +0x94 | f32 | last value from `DAT_3a5bb1d8+0x90` |
| +0x134 | u32 | target unit id |
| +0x138 | u32 | previous target id |
| +0x140 | u8 | target-changed flag |
| +0x1f8/+0x1fc | f32 | FOV blend duration / elapsed |
| +0x218 | u8 | free-look latch |
| +0x219 | u8 | pitch-clamp latch |
| +0x268..0x278 | f32[4] | final basis quaternion |
| +0x2a0 | f32 | zoom speed reference |
| +0x2ac | f32 | distance settle timer |
| +0x2c8..0x2d4 | f32[4] | desired target position |

`0x3915c040` reads/writes `+0x84/+0x88/+0x8c/+0x90`; `0x3915c2b0` reads/writes
`+0x20/+0x24/+0x34/+0x48`; `0x39165420` reads `+0x2ac/+0x2c8..0x2d0`; `0x391694c0` reads the
target id and radius.

## 10. Packet body (0x128 bytes, `FUN_39024610`)

Do **not** model this as the wire layout. It is the client's `SUnitMoveInfo`-style block:

| off | type | notes |
|---|---|---|
| 0x00 | u32 | unit id (the writer sets packet+8 separately; verify) |
| 0x04 | u32 | type (server `UnitMoveType`) |
| 0x08 | u32 | time (ms) |
| 0x0c | u8 | flags |
| 0x10.. | | position / velocity / rotation / delta / stance / flags, copied verbatim |
| 0xF4 | | tail values used by `FUN_390b2f30` (`+0x150..0x174` mirror) |

The mapping to the wire is unknown U4; the server reader is authoritative for the wire
(`aa-net` `model.rs:249-471`). Use a hook on `0x3ac1058c` to capture the serialized bytes and
then align this struct.

## 11. Physics living entity (cryphysics, already ported)

From `aaemu-physics` (`docs/physics/cryphysics/entities.md` §3), 1.2 x86 offsets:

| off | field |
|---|---|
| 408 | pos |
| 420 | orientation quat |
| 448 | vel |
| 460 | velRequested |
| 472 | gravity |
| 484 | ground normal |
| 496 | displacement velocity |
| 512 | dt cap (from `action_move` +20) |
| 516..528 | inertia, inertiaAccel, airControl, airResistance |
| 532/536/540/544/548/552 | heightCollider/heightEye/heightPivot/sizeCollider |
| 1120 | "use raw vel next step" byte (action_move iJump=3) |

`action_move` = id 1, 24 bytes; `Action` slot = vtable +0x28. Reuse the ported struct instead of
redefining it in `aa-movement`.
