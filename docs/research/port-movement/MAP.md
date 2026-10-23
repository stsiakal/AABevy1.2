# Port map: the ArcheAge 1.2 local movement controller

Address map for a function-by-function Rust port of the 1.2 client's local player/mount movement
and camera, with a differential-test plan that calls the original functions inside the running
hidden 1.2 client (`aadriver.dll`).

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
| 1.2 cryphysics | `D:/aa-re/cryphysics/x86_12/out/`, ported by `aaemu-physics` |
| 10.x names | `D:/aa-re/zone/out/`, `x2game-dev_dedicate` decompile (naming only) |
| bindings | `game/default_binding.g` in `D:/AA1.2/game_pak` (extracted with `tools/pakx.py`) |
| existing notes | `docs/research/re-movement.md`, `re-mounts*.md`, `re-constants.md` |

x2game addresses are virtual addresses at the DLL's preferred base `0x38FF0000` (same numbering
as `x2game.dll.c`). The unpacked tail code (`0x3a6xxxxx`–`0x3acxxxxx`) is part of the same image;
`thunk_FUN_xxx` at `0x39xxxxxx` are 5-byte jumps into it (verified with capstone).

Evidence markers: **[R]** read from the binary/decompile, **[I]** inference, **[?]** open.

### 0.3 Layout of this folder

| file | content |
|---|---|
| `MAP.md` (this) | call chains, turn rate, camera, remote smoothing, port order, unknowns |
| [`functions.md`](functions.md) | every function in scope: address, size, callers/callees, fields, pure/stateful |
| [`structs.md`](structs.md) | the Rust struct layouts to define |
| [`difftest.md`](difftest.md) | in-process differential harness recipes per function |

### 0.4 Port scope size

The inventory in `functions.md` documents **100 x2game entries, 90,669 bytes**, all from
`funcs.csv` (the 5-byte send entry `0x39186810` is listed there as `thunk_FUN_3ac1058c`). Remote smoothing is a separate later bucket: 7 functions, 5,296 B (section 6).

---

## 1. The call chain

### 1.1 One frame, on foot

```
engine frame
├─ X2::GameClient::PrePhysicsUpdate              x2game 0x39171590  (288 B)
│  └─ X2::GameClient::InputManager::Update        x2game 0x39406540  (264 B)
│     └─ per-unit UpdateMovement                  x2game 0x39404d50 (5971 B)
│        ├─ axes 0..3 (fwd/back/left/right)       0x39424580 -> 0x393fd240
│        ├─ combiner                              x2game 0x39402590  (246 B)
│        ├─ axis preference                       x2game 0x39402690  (371 B)
│        ├─ ladder-axis filter                     x2game 0x39402f60  (126 B)
│        ├─ turn delta (keyboard or mouse)         x2game 0x39402e00  (349 B)
│        │  ├─ keyboard: cvar keyboard_rotate_speed
│        │  └─ mouse:    0x3915c700 (yaw), 0x394fab30 (notify)
│        ├─ velocity smoothing                    x2game 0x39402c50  (427 B)
│        ├─ build MovementRequest at [ebp-0x130]  (0x9C bytes, §2.6)
│        └─ submit -> GameClient vtable +0x3C
│
├─ engine entity PrePhysicsUpdate (virtual, per actor)
│  └─ X2::CPlayer::PrePhysicsUpdate               x2game 0x39133280  (267 B)
│     ├─ CActor::UpdateMovementRequest            x2game 0x39118650 (1965 B)
│     └─ post hook                                x2game 0x39113f60   (29 B)
│
├─ character / animation update
│  ├─ X2::CPlayer::ProcessFrameMovement           x2game 0x39135fe0  (378 B)
│  │  └─ CPlayerMovement::Process                 x2game 0x3a6ab5f3  (338 B)
│  │     └─ IEntity vtable +0x98 (apply movement to the animated entity)
│  └─ X2::CPlayer::ProcessFrameMovementRotation   x2game 0x39135ed0  (264 B)
│     ├─ CPlayerRotation ctor                     x2game 0x3913ded0  (252 B)
│     ├─ bind to actor                            x2game 0x3913dfd0  (980 B)
│     └─ dispatch                                 x2game 0x391407b0 (1682 B)
│        ├─ ProcessNormal                         x2game 0x3913ff90 (2067 B)
│        ├─ ProcessNormalRoll (no-x2game-name)    x2game 0x3913e780  (322 B)
│        ├─ ClampAngles                           x2game 0x3913e8d0 (1867 B)
│        ├─ ProcessLadderRotation                 x2game 0x3913e6a0  (221 B)
│        └─ rotation path A / B (unnamed)        0x3913f020 / 0x3913f710
│
├─ engine physics
│  └─ CLivingEntity::Action(action_move)          cryphysics 0x35030E90 (slot 10, id 1)
│
└─ Client Game PostUpdate                          x2game 0x391716b0  (324 B)
   └─ FUN_39406740  (movement packet pump)          x2game 0x39406740  (307 B)
      └─ FUN_39406650 (per-unit timer)              x2game 0x39406650  (228 B)
         └─ FUN_394064b0 (choose mover)             x2game 0x394064b0  (139 B)
            ├─ on foot: FUN_394046f0                x2game 0x394046f0 (1624 B)
            └─ mounted: FUN_394045f0                x2game 0x394045f0  (243 B)
               -> opcode 0x89 (CSMoveUnit), body copied by 0x39024610,
                  sent through 0x39186810 -> 0x3ac1058c
```

**Module split [R].** The whole input -> request -> actor-update -> rotation chain is in
**x2game** (`X2::CPlayer`, `CActor`, `X2::GameClient`, `CPlayerMovement`, `CPlayerRotation`);
crygame's same-named `CPlayer`/`CPlayerMovement` (base `0x39000000`, e.g. `FUN_3906a7f0`) is the
engine-side actor and is **not** on this path. The engine enters the chain only through the
entity vtable (`+0x98` apply movement, `+0x108` physical entity) and the physics `Action`
(slot 10). cryaction/crygame are therefore not part of the input/rotation core, **with one caveat**:
`re-mounts.md` identifies crygame `FUN_3906a7f0` (0x3906a7f0, 11284 B) as the writer of the
graph's `PseudoSpeed`/`RequestedTurnSpeedLZ` from the movement request. That function lives in
crygame (base `0x39000000`) and has no x2game caller in `calls.tsv`; it is reached from the
engine actor update. The movement request and the graph input are therefore two views of the
same data; a live hook should record `FUN_3906a7f0`'s request input as a second oracle (U1).

Evidence: profiler strings `X2::GameClient::InputManager::Update` (0x399d5ee8) and
`X2::GameClient::PrePhysicsUpdate` (0x399b2090) map to 0x39406540 / 0x39171590 in
`strings-xrefs.tsv`; `CPlayer::PrePhysicsUpdate` (0x399ad23c) -> 0x39133280; `CPlayer::ProcessFrameMovement`
(0x399ad330) -> 0x39135fe0; `CPlayer::ProcessFrameMovementRotation` (0x399ad308) -> 0x39135ed0;
`CPlayerRotation::ProcessNormal` (0x399ad740) -> 0x3913ff90; `CPlayerRotation::ClampAngles`
(0x399ad71c) -> 0x3913e8d0; `CPlayerRotation::ProcessLadderRotation` (0x399ad6cc) -> 0x3913e6a0.
`Client Game PostUpdate` (0x399b20b4 -> 0x391716b0) contains the pump call (see
`functions.md`).

**Where the per-frame entry points come from.** `X2::GameClient::PrePhysicsUpdate` is registered
as an engine listener through `FUN_390f5360` (the only caller in `calls.tsv`). The two
`CPlayer::PrePhysicsUpdate` / `Quadruped::PrePhysicsUpdate` functions are virtual methods; they
have no static caller in `calls.tsv` because the CryEngine entity system dispatches them through
the object's vtable (the `@`-prefixed profiler/RTTI strings at 0x399ac637 `@ActorMovementController::Update`
are the same mechanism) [R].

### 1.2 Mount: the controlled unit is the mate

Everything in the input layer is unchanged. What differs is **which actor is moved**:

* `local_60` in `UpdateMovement` is `(**(*GameClient + 0x208))()` - the currently controlled
  unit (0x39404da8–0x39404dc8). On horse/boat it is the mate, not the player.
* `CActor::UpdateMovementRequest` and the request path run on that unit; the mount's
  per-frame physics update is `Quadruped::PrePhysicsUpdate` 0x39129800 (1927 B), reached from
  `Client Game PrePhysicsUpdate`/entity dispatch.
* Quadruped setup and movement:
  `0x3912c640` (setup A, 297 B), `0x3912e0c0` (setup B, 216 B), `0x3912f600` (setup C, 1792 B),
  `0x3912dd40` (movement vector, 895 B), `0x3912ac50` (apply to actor, 703 B),
  `0x39129170` (copy, 74 B), then `0x39113f60` and `0x39123e80`.
* Rider/mount animation coupling: `X2::ActorUnitModel::Update` 0x390dd080 (632 B) calls
  `FUN_390dcc80` (1011 B), which copies the mount graph's `PseudoSpeed` and
  `RequestedTurnSpeedLZ` to the rider graph. No packet is sent for the rider.
* `CSMoveUnit` then carries the **mate** object id (`FUN_394046f0` uses `edi+8`, the controlled
  unit id; the vehicle writer `FUN_394045f0` uses `param_2+8`), see §5. The on-foot/vehicle
  writer split is `game_vtable[4]() == 1` (name of that slot [?]).

### 1.3 Swim

Swim is the same input/request chain with the actor in stance 3 ("swim"): the stance block
`game_stances` row 75 for model 1 (`max_speed=3.0`) is loaded by `FUN_3911a5a0`/`FUN_3911b950`.
The per-frame water state is `CActor::UpdateSwimStats` 0x39120300 (2072 B), which maintains the
water level (`actor+0x84`), the in-water flag (`+0x1f`) and the submersion timer (`+0x20`), and
fires events 10/11/12. The swim control that feeds the graph is `FUN_3912ce00` (2648 B), called
from `FUN_3912dd40` (quadruped) and from the animated-character update; it drives
`SwimControlX/Y/Z` and the `_deep` variants.

Swim multipliers (registration at `FUN_390f7b40`, defaults read from `.data`):

| cvar | global | value |
|---|---|---|
| `swim_up_speed_mul` | `0x399a87e8` | 0.7 |
| `swim_down_speed_mul` | `0x3999c47c` (shared 0.5 constant) | 0.5 |
| `swim_back_speed_mul` | `0x399a1d94` | 0.8 |
| `swim_side_speed_mul` | `0x399a24cc` | 0.6 |
| `swim_buoy_speed` | `0x3999cffc` | 2.0 |
| `swim_jump_speed` | `0x3999f9d0` | 12.0 |
| `swim_jump_permission_range` | `0x3999dd34` | 0.2 |

### 1.4 Ladder

Not a separate controller: the ladder state changes how the input layer maps the turn axes and
how the rotation is processed.

* Grab: `FUN_390c1330` -> `Player::GrabOnLadder` `FUN_391388f0` (323 B): stores ladder entity id
  at `actor+0x238`, ladder kind at `+0x23c`, copies the ladder frame to `+0x244..+0x280`, sets
  `isOnLadder` (`+0x1db = 0x101`), calls `FUN_39137470`.
* Per-frame: `FUN_39137470` (4319 B) validates the ladder entity and updates the transform/IK.
* Input mapping: `FUN_39402f60` (126 B) is called from `UpdateMovement` with the controlled
  unit; it switches on `unit+0x23c`:
  - kind 4 (`net`) or 8 (`guardrail`): a positive turn-right/turn-left axis (`FUN_39424580(5)` /
    `(4)`) sets the right/left flag and the axis is zeroed;
  - kind 6 (`hrope`), 7 (`mast`) or 10 (`bigtree`): the same right/left flags without zeroing
    the axes; other kinds do nothing.
  Verified from capstone and differential goldens (phase A; `port/movement/input.rs`).
* Leave: `Player::LeaveLadder` `FUN_39135570` (1732 B), modes 2–7; rotation through
  `CPlayerRotation::ProcessLadderRotation` `FUN_3913e6a0`.
* Kind names from `FUN_39132be0` (0x39132be0): 4 net, 5 hrope, 7 mast, 8 guardrail,
  9 45-degree ladder, 10 bigtree.
* The ladder also sets actor flag 0x40 and appends the 32-bit `climb_data` in the movement
  packet (server reader `aa-net` `model.rs:395-402`).

---

## 2. Input layer, step by step

### 2.1 Bindings and the action table

`game/default_binding.g` (primary manager): `w moveforward`, `s moveback`, `q moveleft`,
`e moveright`, `a turnleft`, `d turnright`, `. toggle_walk`, `space jump`, `numlock autorun`,
`mousedx rotateyaw`, `mousedy rotatepitch`, wheel `zoom_in/zoom_out`, Home/End/Insert/Delete the
fixed camera views, PageUp/PageDown cycle the camera. The `second` manager adds arrows and
`mouse4 autorun`.

The engine maps action names to a fixed 0x68-entry table at **`0x39ac8e68`** (read from `.data`;
`FUN_393f8d20` compares against it). The first ten entries, and therefore the axis indices used
by `FUN_39424580`, are **[R]**:

| index | action | index | action |
|---|---|---|---|
| 0 | moveforward | 5 | turnright |
| 1 | moveback | 6 | jump |
| 2 | moveleft | 7 | down |
| 3 | moveright | 8 | rotateyaw (mousedx) |
| 4 | turnleft | 9 | rotatepitch (mousedy) |

`FUN_394005f0` (211 B) registers nine built-ins from the table at `0x394005fe..0x39400694`
(name, default key): `open_config/escape`, `moveforward/w`, `turnleft/a`, `turnright/d`,
`moveback/s`, `jump/space`, `rotateyaw/mousedx`, `rotatepitch/mousedy`, `zooming/mousedz`.
The **axis indices** are the separate table at `0x39ac8e68`, which is ordered
moveforward, moveback, moveleft, moveright, turnleft, turnright, jump, down, rotateyaw,
rotatepitch, ... (see the table above).

`FUN_39424580(i)` -> `FUN_393fd240` (61 B) -> `FUN_393fc580`: returns the current analogue value
of action `i`, or 0 if the action is not in the map (`this+0xec`).

### 2.2 UpdateMovement (0x39404d50)

`this` is a **per-unit input/send state** (not the InputManager): `*this` is the unit object id
(`FUN_39360020(*param_1)` at 0x39404d86), and `this+0x84` is the last input flag word. It is
called with `ECX = esi+4` from `InputManager::Update` (0x394065e3), i.e. one instance per
controllable unit.

Order of work:

1. Clear the 16-bit state at `this+0xb8`.
2. Resolve the actor: `local_44 = FUN_39360020(*this)`; return if null.
3. Resolve the controlled unit: `game = *(actor+0x3be8)` (`FUN_390b3750`),
   `unit = (**(*game + 0x208))()`. If `unit[0x74] != 0` (a "hold position"/cinematic flag),
   return early.
4. Read the turn axes (`FUN_39424580(4)`, `(5)`) and the mouse axes `(8)`, `(9)`.
5. Resolve `local_71 = FUN_392aa250(1)`, `local_48 = FUN_392aa250(3)` (a global input-mode
   table; exact semantics [?]), and `FUN_3921ad60()` (returns 0 in the dump; gating flag).
6. Mouse-button/axis combiner `FUN_39402590` (§2.3) fills forward/back/left/right flags and
   clears turn axes when a mouse button is held; `FUN_39402690` picks the stronger of two
   opposite axes when `game_cfg+0x114 == 1`.
7. Ladder filter `FUN_39402f60` (§1.4).
8. Turn delta `FUN_39402e00` (§3).
9. Build the desired direction from the four flags in camera space, normalise, convert to world
   with the camera/actor basis, then smooth with `FUN_39402c50`.
10. If an external movement target exists (`this+0xb4` = the "goto" unit, resolved through
    `actor+0x1aa0`), build the direction from that target instead (`FUN_39402fe0`).
11. Compose the **MovementRequest** on the stack at `[ebp-0x130]` (§2.6), then:
    `(*game_vtable[0x58])(flags)` and `(*game_vtable[0x3C])(&request)` at
    0x3940639c/0x394063a6.
12. Walk toggle: `FUN_390b66c0(0)` / `FUN_390b67e0()` set bit **0x1000** in the flag word
    (calls at 0x39405ff9/0x39406004, `or [ebp-0x30],0x1000` at 0x3940600d); the same
    `local_40` word is copied to `this+0x84` at 0x39405ff3. Auto-run latch: the two
    `FUN_392aa250` inputs plus the 0x1/0x2000 bits handled in `FUN_39406650` [R/I].
13. Copy the axes into `this+0x88..0x98`, the flags into `this+0x84`, and push
    `this+0x84` to `game_vtable[0x58]`.

**Authority note.** The input manager does not integrate or collide anything. It only produces
the request; the actor layer consumes it in the next physics phase.

### 2.3 Axis combiner (0x39402590)

The call site reuses the stack: `FUN_39403a90` is `ret 4` with three pushed arguments, so its
two older pushes become the last two arguments of the combiner. The real signature is
`void FUN_39402590(char mode1, char mode2, float* turnRight, float* turnLeft, byte* forward,
byte* right, byte canTurn, byte state30, byte prefer, ESI=byte* back, EDI=byte* left)`.

It reads axes 0..3 (`FUN_39424580(0..3)`): forward starts at `state30 != 0` or `axis0 != 0`,
back is `axis1 > 0`, left is `axis2 > 0`, right is `axis3 > 0`. Only when `canTurn` is set and
one of the two input-mode bytes is non-zero, a positive turn-left axis 4 is converted into the
left flag and zeroed (or, when turn-right is also positive, dropped without its flag); a positive
turn-right axis 5 sets the right flag and is zeroed. Without `canTurn` both strafe flags are
cleared. If both mode bytes are set, the forward flag is also set. Exact branch order and the
`prefer` behaviour are ported and covered by goldens (`port/movement/input.rs`).

`FUN_39402690` (371 B) reads axes 0..3 and maintains per-direction "press time" at
`this+0x9c..0xa8`; when two opposite directions are held it drops the one with the lower time.

### 2.4 Walk toggle

Walk is **not** an axis multiplier inside the combiner. It is input-flag bit **0x1000**
(`this+0x84` / request-side `local_40`), set at the end of `UpdateMovement` from
`FUN_390b66c0`/`FUN_390b67e0` (0x390b66c0 276 B, 0x390b67e0 210 B; these read the
`toggle_walk` action). The actor model's direction-factor function (`FUN_39112690`, 1615 B)
selects either `+0x43c..+0x44c` or the walking set `+0x454..+0x464` from that bit, so walking
changes speed and animation but not the turn rate.

### 2.5 Auto-run

`FUN_392aa250(1)`/`(3)` are checked at the top of `UpdateMovement`; the auto-run latch and the
pending flag bits 0x1/0x2000 are consumed in `FUN_39406650` (0x394066b0-0x3940670f).
`FUN_39406740` calls `FUN_39406650` twice, then five more input slots in a loop; each call
accumulates `dt` at `state+4` and sends the packet if the state changed (see §5). The precise arm/cancel order (press forward clears, next press starts) matches
the existing note in `re-movement.md` §2.1 and is implemented there; the version in the binary is
the same latch in `FUN_39406...` [I].

### 2.6 MovementRequest (`0x9C` bytes, built at `[ebp-0x130]`)

The request is default-initialised by **`FUN_390a8dc0`** (245 B: `lea ecx,[ebp-0x130]; call`
at 0x39405fdd) and then filled by the code in `0x39405fe2–0x39406310` (capstone).
`FUN_390a8dc0` sets the sentinel `-1` at +0x28, +0x2c, +0x6c, +0x78, +0x84, +0x94, `0.5` at
+0x74, `1.0` at +0x5c and 0 at +0x98.
Offsets are relative to the request pointer passed to `GameClient vtable +0x3C`.

| off | type | value / meaning |
|---|---|---|
| 0x00 | u32 | flags (see below) |
| 0x04 | vec3 | external move-to direction/point (`FUN_39402fe0`, only when `this+0xb4` is set) |
| 0x10 | f32 | 1.0 (quaternion w / unit scale) |
| 0x1C | u32 | movement type from `FUN_394037d0` (1 = normal, 5/6 = scripted/cinematic, 0 = not allowed) [R] |
| 0x20 | vec3 | smoothed velocity (copied from `this+0x0c` after `FUN_39402c50`) |
| 0x2C | vec3 | velocity delta block 1 (optional, flag 0x40) |
| 0x38 | vec3 | velocity delta block 2 (optional, flag 0x20) |
| 0x44 | ? | second basis/quaternion (only used by the external-target path) |
| 0x94 | u32 | controlled-unit kind: 0 normal, 2 when `game->cfg+0x428 != 0` (mate/mount) |
| 0x98 | f32 | pseudo-speed: 0 idle, 1.0 moving, 0.4 walking (walk bit 0x1000) |

Observed flag writes (all in `FUN_39404d50`, capstone; the conditions are **not** named, U2):

| bit | OR/AND site |
|---|---|
| 0x1 | set 0x394062df, cleared 0x394062fe |
| 0x8 | 0x39406224 |
| 0x20 | 0x394061b9 |
| 0x40 | 0x39406113 |
| 0x800 | 0x39406071 |
| 0x8000000 | 0x39406301 |
| 0x20000000 | 0x3940623b |
| 0x80000000 | 0x39406201 |

The Rust port must reproduce the exact bit word because it is also what the server sees in
`CSMoveUnit` (flags byte + actor flags). A live dump
of the request at the `GameClient vtable +0x3C` call for the 12-case matrix is the oracle.

---

## 3. Keyboard turn: exact source of the rate

### 3.1 The turn-delta function

`FUN_39402e00` (`float10 __thiscall(int this, int unit, float axis5, float axis4, byte useMouse)`,
349 B; `this` = the **per-unit InputState** (`ECX = edi = param_1` at 0x394052c1), `unit` = the
controlled actor) **[R]**:

```c
// called at 0x394052c3 with ECX=InputState, args (unit, axis5, axis4, flag)
if (useMouse) {                       // [ebp+0x14] byte != 0
    float d = FUN_3915c700();         // mouse yaw delta (see 3.4)
    if (d != 0.0f) FUN_394fab30();    // notify/UI packet path
    return d;
}
float axis = -axis5 + axis4;          // turnleft - turnright
if (axis == 0.0f) { this->turnAccum = 0; return 0; }

float rate = M_PI;                    // 0x399d5ea0 = 3.14159274
if (unit != NULL && *(u8*)(unit + 0x1c1) == 4) {      // quadruped / mount
    this->turnAccum += game_dt;                        // InputState+0x38
    this->turnAccum  = clamp(this->turnAccum, 0, 3);   // 0x3922aed0, DAT_399d5ed0 = 3.0
    rate = this->turnAccum * *(float*)(unit + 0x174) * (1.0f/3.0f); // 0x399a1c40
}
float dt  = min(game_dt, 0.033f);     // 0x399a3040 = 0.033
float cvar = *(float*)(game_cfg + 0x308);   // "keyboard_rotate_speed"
return axis * dt * cvar * rate;
```

* `keyboard_rotate_speed` is registered at `FUN_390f7b40` line 196206
  (`(0,"keyboard_rotate_speed",param_1+0x308,DAT_3999c47c,0x82,...)`), string at `0x399a7314`.
* `.data` dump: `0x3999c47c = 0x3f000000 = 0.5`.
* Therefore on foot the delta is `axis * min(dt,0.033) * 0.5 * π` = **90°/s** at axis = 1 **[R]**.
* `game_cfg` is `DAT_39aefb98` (a heap object in the dump, so the cvar value is runtime; the
  default is the static 0.5 above).

### 3.2 Mounted turn rate

`unit+0x1c1 == 4` identifies the local controlled mate as a quadruped. `unit+0x174` is the
per-model turn speed and comes from `actor_models.turn_speed` (SQLite: model 1 = 360.0,
model 206 horse = 360.0). The code multiplies it by `turnAccum/3` where `turnAccum` ramps from 0
to 3 over **3 s of continuous turn** (reset to 0 by a zero axis). **[R]** for the formula,
**[I]** for the unit interpretation:

* if `+0x174` is stored in **radians/s** (the fast-turn behaviour is 2× on foot), the full rate is
  `0.5 * (2π) = π rad/s = 180°/s`;
* if it is stored in **degrees/s**, the last factor is 180 (interpreted as radians by the
  downstream code), which is physically impossible, so the radian reading is the consistent one.
  The writer of `+0x174` was not isolated; the read is at 0x39402ecc–0x39402edb **[?]**.

### 3.3 Mouse look override

The `useMouse` byte at `[ebp+0x14]` is set at the call site (0x394052a4–0x394052c0) as

```c
useMouse = (local_48 != 0 && local_23 != 0);
```

where `local_48 = FUN_392aa250(3)` (a global input mode) and `local_23` is set by the
`FUN_394039c0` branch when the camera is free-looking or movement has just started
(0x394051e6 / 0x39405257). When it is set, `FUN_39402e00` **returns the mouse yaw delta instead
of the keyboard turn**; the two are never added. The exact button/state that drives
`FUN_392aa250(3)` is **unknown (U3)**, but the observed ingame behaviour is the right-mouse
button: the character turns to the camera while held, and A/D keyboard turn is cancelled.

### 3.4 Mouse delta

`FUN_3915c700` (193 B) **[R]** reads axis 8 (`mousedx`). Let
`c = (min(dt, 0.033) * 10.0) * 2π` and `a = axis * -0.002` (both `f32`, constants
`0x3999cce8 = 10.0`, `0x399b050c = 2π`, `0x399b0724 = -0.002`). The first fold
`t = ((a+c) - |a-c|) * 0.5` is SSE `f32`; the second fold
`((t-c) + |t+c|) * 0.5` runs in x87 80-bit and is rounded once to `f32` on return. For finite
values the result is `clamp(a, -c, c)`, i.e. the mouse delta is limited to
`±dt*10*2π*0.002` rad before the final `0.5` gain. `FUN_3915c7d0` is the same code on axis 9
(`mousedy`). The camera also calls both directly in mode 1 (§4). Phase A resolves **U12**;
the exact port is `port/movement/mouse.rs` and all 14 captured (dt, axis) cases per function are
bit-exact.

### 3.5 Summary table

| situation | source | rate |
|---|---|---|
| on foot, A/D | `FUN_39402e00`, `keyboard_rotate_speed` 0.5 × π | 90°/s |
| mounted, A/D | `FUN_39402e00`, `(turnAccum/3) × unit[0x174] × 0.5`, 3 s ramp | 180°/s at full (inferred) |
| mouse held (free look / RMB) | `FUN_3915c700` | `mouse dx` × dt, cvar-scaled |
| camera yaw | mode-1 camera adds the same mouse delta and follows the character yaw | see §4 |

---

## 4. Camera

### 4.1 Per-frame chain

```
Client Game Update                       x2game 0x391739e0 (705 B)
└─ FUN_3916ac70 (guard)                  x2game 0x3916ac70  (16 B)
   └─ CameraManager::Update              x2game 0x3916aab0 (421 B)
      switch (manager+4):
        1 -> FUN_39169860  unit / third person   (2558 B)
        2 -> FUN_391632d0  free camera           (3622 B)
        3 -> FUN_3916a260  mode 3 (unnamed)        (2123 B)
        4 -> FUN_39164100                        (1052 B)
        5 -> (no per-frame update)
        6 -> FUN_39166390                        (2898 B)
```

Mode 1 is the local player camera; the others are camera modes for cutscenes/screenshots
(mode 2 is the free camera from the existing notes; 3-6 are unnamed) and are not needed for
the first port. The per-function roles below are [R] for addresses/fields and [I] for the
"what it computes" descriptions unless a profiler string exists. `CameraManager::Update` uses `min(dt,0.033)` (0x3916aab0,
`DAT_399a3040`) **[R]**.

### 4.2 Mode 1 (`FUN_39169860`)

State accessed on the camera manager (`param_1`) **[R]**:

| off | meaning |
|---|---|
| +0x04 | mode byte |
| +0x20 | current drawn distance |
| +0x24 | wanted distance |
| +0x34 | distance-settle accumulator |
| +0x3C..0x44 | camera world position (output) |
| +0x68..0x7C | target/eye offset and yaw basis |
| +0x80 | last character yaw |
| +0x84 | accumulated yaw offset (character yaw delta) |
| +0x88 | current yaw rate / pitch state |
| +0x8C | total camera yaw (rad) |
| +0x90 | accumulated mouse/pitch |
| +0x134 / +0x138 | target unit id / previous target id |
| +0x140 | target-changed flag |
| +0x1FC/+0x1F8 | FOV blend time / duration |
| +0x218 | "free look" latch |
| +0x219 | pitch-clamp latch |
| +0x268..0x270 | final basis quaternion |
| +0x2A0 | zoom speed reference |
| +0x2AC | distance settle timer |
| +0x2C8.. | desired target position (from the target's view) |

Order in mode 1:

1. `target = FUN_39360020(camera+0x134)`; if a new target is pending, switch.
2. If the local player exists: `mouseYaw = FUN_3915c700()`, `mousePitch = FUN_3915c7d0()` when
   the free-look state is active; `FUN_39402880()` ("HoldPosition") zeroes the mouse yaw.
3. Character yaw: `FUN_390b3870()` returns the controlled unit's view quaternion; the yaw is
   `atan2` of its components at 0x39169a1c–0x39169a60. The code accumulates the character-yaw
   delta into `+0x84` so the camera keeps its offset while the character turns (keyboard turn
   therefore rotates the camera **through the character**, with no separate keyboard input in
   the camera).
4. Pitch clamp and free-look handling: `local_14 = -(float)(controlled+0x68)` compared with the
   pitch limits `DAT_39afe28c`/`0x39afe290` (runtime cvars), latch at `+0x219`.
5. Apply the deltas: `FUN_3915c040(dt, &mousePitch, freeLookByte)` at 0x39169a86; it updates
   `+0x88`, `+0x8C`, `+0x84`, `+0x90` and clamps with `camera_dive_*` (see below).
6. `FUN_3915ccf0` + `FUN_3915c2b0(dt)` settle the drawn distance (§4.3).
7. `(*game_vtable[0xCC])(&matrix)` returns the target's view matrix.
8. `FUN_3915d6f0(dt, &pos, targetActor)` computes the camera position from the pivot and the
   wanted distance; `FUN_39165420(target, dt)` shortens it by collision; `FUN_39161690(dt)`
   computes FOV/aspect and the final transform.
9. The final basis is written to `+0x3C..+0x44`, then published to the engine camera through
   `(*game_vtable[0xB]+0x28/0x2C)` and `FUN_3915dd40`.
10. `FUN_391694c0(target)` fades/hides the character when the camera is inside its radius.

### 4.3 Distance / zoom (`FUN_3915c2b0`)

`FUN_3915c2b0(camera, dt)` (649 B) **[R]**:

* `drawn (+0x20)` moves toward `wanted (+0x24)` with a per-second speed
  `cvar_speed + |gap|^cvar_power` (`_CIpow` at 0x3915c36x), clamped so a step never overshoots
  (`|gap|/dt`) and integrated with `dt <= 0.033` (formula inferred from the x87 sequence);
* two distance cvars are read at `DAT_39afe24c` (snap-to-zero threshold) and `DAT_39afe1cc`
  (lower clamp); the acceleration/power cvars are `DAT_39afe204`/`0x39afe208` (roles to
  confirm, U13);
* when the wanted distance is below the near limit the drawn distance snaps to 0 (first person);
* the distance-settle accumulator `+0x34` and the debug value `+0x48` are updated.

Cvar registrations: `FUN_3915e7a0` (0x3915e7a0) registers `camera_max_dist` with default
`_DAT_3999f928 = 18.0` and `camera_max_dist_debug` (same default), plus the console setters
`uc_set_wheel_dist`, `uc_set_pitch`, `uc_set_yaw`, `uc_set_zOffset`, `cam_blur_amount`.
`game/config64/cvargroups/option_camera_fov_set.cfg` overrides `camera_max_dist` to 10 in the
"action" group and 18 in the "classic" group; `cl_fov` 60 / 42.75 respectively.

Other camera cvars (registration defaults, all in `.data`): `camera_dive_angle` 30,
`camera_dive_enable` 1, `camera_dive_pitch` 20, `camera_target_ground_align` 1,
`camera_building_something_fadeout_vel` 10, `keyboard_rotate_speed` 0.5.

Start distance table at **`0x399b0c58`** (race/gender pairs; read from `.data`):
Nuian 4.0/3.2, Elf 3.8/3.4, Hariharan 3.6/3.2, Ferre 4.0/3.4, others 10.0.

### 4.4 Collision and fade

* `X2::GameClient::CameraManager::ProcessUnitCamActualDist` **0x39165420** (3937 B): ray and
  primitive probes (`RayWorldIntersection`, `PrimitiveWorldIntersection`) shorten the arm.
* `ProcessUnitCamFadeOut` **0x391694c0** (919 B): hides a unit when the camera is inside its
  radius and restores it afterwards.
* `FUN_39161690` (3476 B): FOV and the projection basis (uses `_CIpow`, `atan2`).

---

## 5. CSMoveUnit build

### 5.1 Pump and cadence

`Client Game PostUpdate` 0x391716b0 (324 B) calls `FUN_39406740` (307 B). That function:

1. calls `FUN_39406650(dt)` twice (the local player and the primary controlled unit), then
2. loops five more embedded input slots (unit id at `+0x194 + 0x90*i`, flags at `+0x218 + 0x90*i`)
   (other controllable mates/vehicles), and
3. for each state: `state+4 += dt`, then `FUN_394064b0(actor)`.

`FUN_39406650` (228 B) also consumes the pending bits 0x1 and 0x2000 of `state+0x84` [I] and
pushes the remaining flags through `GameClient vtable +0x58`.

`FUN_394064b0` (139 B) picks the writer:

* if the game's "controlled unit manager" (`game_vtable[0x8C]`) reports a special state
  (`+0x34()` true, then `+0x38()`), it uses that branch;
* otherwise `game_vtable[4]() == 1` -> on-foot writer `FUN_394046f0`, else mounted writer
  `FUN_394045f0`.

### 5.2 On-foot writer `FUN_394046f0` (1624 B) **[R]**

```
unit  = game_vtable[0x208]()                       // controlled unit
motion = unit_vtable[0xB0]()                       // movement state (vel)
if all components of motion->velocity are ~0 and nothing else changed -> return 0
viewPos   = FUN_390b3850(actor)                    // GameClient vtable+0x228 + 0x10
viewQuat  = FUN_390b3870(actor)                    // GameClient vtable+0x228 + 0x00
body      = game_vtable[0x60](&local_158)        // 0x128-byte unit-movement block
...
changed = position/quaternion/velocity/rotation/stance/actor-flags differ from last sent
          (threshold 0.01; absolute value, sign-masked)
if (!changed && timer not expired) return 0
flags = (myUnit ? 1 : 0)
      | (state+9  ? 0x20 : 0)
      | (state+10 ? 0x80 : 0)
      | (state+0xb8 ? 0x02 : 0)
      | (moving ? 0x04 : 0)      // flags at 0x39404c03-0x39404c33
packet = { vtable=0x3999d574, opcode=0x89, objId=unit+8, body=copy of body }
FUN_39024610(&packet, body)                        // 0x128-byte body copy
packet[+?] |= (myUnit ? 1 : 0)
FUN_39186810(&packet)                              // jmp 0x3ac1058c (send)
game_vtable[0x64]()                                // ack/notify
state+4 = 0                                        // reset timer
state+0x44..0x54 = viewQuat (4 floats)
state+0x54..0x5C = viewPos
state+0x60..0x68 = body rotation
state+0x6C..0x74 = body velocity
state+0x78      = stance
state+0x79      = actor flags
state+0x7C      = fall speed
state+0x80      = support/gc id
```

The gate reads two timer objects, `DAT_3a59f84c` and `DAT_3a59f850` (their `vtable+8` returns
milliseconds; `DAT_3999cf30 = 0.001` converts to seconds). Unchanged state is usually
suppressed, but the exact interval and the fallback rules are **U5**.

`FUN_39024610` (270 B) copies a 0x128-byte block: fields +0, +4, +8, +0x10, +0x18, +0x20,
+0x24, +0x28, +0x2C, +0x30, +0x38, +0x40, +0x4C, +0x50, +0x98 (0x17 dwords), +0xF4, +0xFC,
+0x104, +0x108, +0x10C, +0x110, +0x114, +0x11C, +0x124. The first bytes of the body are the
server's `obj_id`/type/time layout (aa-net `model.rs`), but the field-by-field serializer lives
in the packet vtable at `0x3999d574` and is **not yet mapped (U4)**.

### 5.3 Mounted writer `FUN_394045f0` (243 B)

Same packet class and opcode 0x89; it takes the unit id from `unit+8`, tests
`unit+0x410 != 4` and `unit+0x3af0 == 0` (movement-info validity), builds the packet with
`game_vtable[0x60]` and sends with `FUN_3ac1058c`. It is used when
`game_vtable[4]() != 1` (the slot's exact name is [?]; the second writer is the
non-player/vehicle variant, used for the mount per `re-mounts.md`). The rider itself sends
nothing; the mate id is the packet's `obj_id`.

### 5.4 Physics side

The local actor's movement reaches the engine through the **entity**, not through a literal
`IPhysicalEntity::Action` call in x2game: `CPlayerMovement::Process` ends with
`(**(code **)(**(int **)(actor + 0xc) + 0x98))()` (disassembly `3a6ab743`), i.e. the CryEngine
entity applies the movement. The physics access pattern used by `CActor` is nevertheless
visible and verified:

* `entity = actor + 0xc`; `phys = (**(*entity + 0x108))()` -> `IPhysicalEntity*`
  (e.g. `FUN_3911a2b0` at 0x3911a2b0, `CActor::UpdateStance`);
* `GetStatus` = vtable +0x24 (slot 9), `SetParams` = +0x1C (slot 7), `Action` = +0x28 (slot 10)
  — e.g. the awake action `{id=8}` at 0x3911a2b0, the set-velocity action `{id=10}` at
  0x391303b0, the reset `action_move{id=1,dir=0}` at 0x390c47f0;
* the living entity's `Action` slot is `CLivingEntity__Action` **cryphysics 0x35030E90**
  (1.2 x86); `action_move` is id 1, 24 bytes: +4 vec3 dir, +16 int jump, +20 float dt cap;
  validator `0x3502B250`; it sets `velRequested`/`vel` in `CLivingEntity` (+460/+448) and the
  physics step integrates them (already ported in `aaemu-physics`).

The exact site that turns the local request into `action_move` is inside the entity/animated
character layer and is **unknown (U1)**; only the reset and remote paths were found as literal
`Action` calls. This is the first thing a live hook should settle (§9).

---

## 6. Remote units: smoothing (later phase, addresses only)

| function | address | size | role |
|---|---|---|---|
| `X2::NetActorMovementController::Update` | 0x390a3bb0 | 558 | per-frame controller; collapses the queued target (`+0x430/+0x438/+0x440`) and calls UpdateLivingEntity |
| `X2::NetActorMovementController::UpdateLivingEntity` | 0x390a30e0 | 2759 | interpolation/extrapolation into the living entity |
| `X2::NetActorMovementController::PrePhysicsUpdate` | 0x390af040 | 1281 | pre-physics reconcile |
| `OnUnitMovements` | 0x391e6350 | 108 | SCUnitMovements handler |
| movement queue overflow | 0x391e55c0 | 263 | parses the batch, logs `too many unit movements..` (0x399b9f4c) |
| `OnOneUnitMovement` | 0x391d4e70 | 187 | single movement |
| `OnGimmickMovement` | 0x391db9d0 | 140 | gimmick movement (0x399b8d34) |

Controller fields: `+0x04` actor/entity, `+0xcd` active flag, `+0x418/+0x420/+0x428` previous
delta, `+0x430/+0x438/+0x440` target delta, `+0x1b8`/`+0x1c0`/`+0x1c4` timing. The local
`movement queue over flowed` string is also used at 0x390b2750/0x390b2840/0x390b2980 (outbound
queue) and 0x3917a0a0.

No port work is proposed for these in this phase; the addresses are here so the later
interpolation task can start from the existing notes (`remaining.md` N4).

---

## 7. Function inventory, structs and port order

The complete per-function table (address, size, callers, callees, offsets, globals, purity) is
in [`functions.md`](functions.md). The Rust layouts are in [`structs.md`](structs.md).

### 7.1 Proposed port order (smallest testable leaves first)

Phase A - pure math and input mapping (no world state):

1. `FUN_3922aed0` clamp (24 B) - used by the turn ramp. Pure.
2. `FUN_39112690` direction factor (1615 B) - pure given the actor's movement block; port with
   a plain `MovementParams` struct and table tests.
3. `FUN_3915c700` / `FUN_3915c7d0` mouse yaw/pitch (193 B each) - pure given axes, dt, cvars.
4. `FUN_39402590` combiner (246 B) and `FUN_39402690` axis preference (371 B) - pure/stateful
   but only touch the input state; easy to diff.
5. `FUN_39402e00` turn delta (349 B) - the first real differential target (§3).
6. `FUN_39402f60` ladder-axis filter (126 B) - pure given unit kind and axes.

Phase B - input state machine:

7. `InputState` struct (`structs.md` §1), `FUN_39402b30` reset (283 B), `FUN_394037d0` (119 B),
   `FUN_394039c0` (200 B), `FUN_39403330` (107 B), `FUN_39403a90` (87 B),
   `FUN_39402c50` smoothing (427 B), `FUN_39402fe0` external target (198 B).
8. `FUN_39404d50` UpdateMovement (5971 B) as the integration of 7; diff the resulting
   `MovementRequest` bytes, not the whole function.

Phase C - actor/mover:

9. `CPlayerMovement` ctor/Process `0x3a6ab5f3` (338 B) and its Rust `MovementInput` struct.
10. `CPlayerRotation` ctor `0x3913ded0`, bind `0x3913dfd0`, dispatch `0x391407b0`,
    `ProcessNormal` `0x3913ff90`, `ClampAngles` `0x3913e8d0`, `ProcessLadderRotation`
    `0x3913e6a0`.
11. `CActor::UpdateMovementRequest` `0x39118650` (1965 B) - stateful; diff against a live actor.
12. Quadruped: `0x3912c640`, `0x3912e0c0`, `0x3912f600`, `0x3912dd40`, `0x3912ac50`.
13. Swim: `0x39120300`, `0x3912ce00`.
14. `CPlayer::ProcessFrameMovement`/`Rotation` wrappers `0x39135fe0`/`0x39135ed0`.

Phase D - packet and camera:

15. `CSMoveUnit` writer `0x394046f0` / `0x394045f0` and the pump `0x39406740`/`0x39406650`.
16. Camera mode 1 (`0x39169860`), zoom (`0x3915c2b0`), collision (`0x39165420`), fade
    (`0x391694c0`).

Rationale: every item in phase A can be called with a synthetic `this`/inputs and compared
numerically; each later item only needs the state produced by the previous phase.

### 7.2 Differential-call summary

The detailed recipes are in [`difftest.md`](difftest.md). In short:

| function class | convention | how to call in-process |
|---|---|---|
| leaf math (`0x3922aed0`, `0x39112690`, `0x39402590`, `0x39402f60`) | `__cdecl` / `__thiscall` | plain C wrapper; pass a pointer to a Rust-owned struct |
| turn delta `0x39402e00` | `__thiscall` (ECX=InputState), 4 stack args | `__fastcall` wrapper with a dummy EDX |
| movement request `0x39404d50` | `__thiscall` (ECX=InputState) | use the live input state captured from `InputManager::Update` (hook) or build one |
| actor update `0x39118650` | `__fastcall` (ECX=actor) | pass the live player actor pointer (`FUN_39360020(playerId)`) |
| packet writer | `__thiscall` (ECX=input state), `__fastcall` internals | hook `0x3ac1058c` and compare the serialized bytes |
| camera `0x39169860` | `__thiscall` (ECX=CameraManager) | use the live manager (`DAT_39afe2b8`) |

x87 returns (`float10` in Ghidra) are plain `float` on the ABI and can be declared as `float`
in the driver; `__thiscall` on MSVC x86 is `__fastcall` with an unused EDX.

---

## 8. Numeric parameters (1.2, .data / DB)

| value | number | source |
|---|---|---|
| keyboard turn cvar | 0.5 | `0x3999c47c`, `FUN_390f7b40:196206` |
| on-foot turn | 90°/s | `FUN_39402e00`, π at `0x399d5ea0` |
| mount turn ramp | 0..3 s | `DAT_399d5ed0 = 3.0`, `0x399a1c40 = 1/3` |
| mount `turn_speed` | 360.0 | `actor_models` rows 1 and 206 |
| dt cap | 0.033 s | `0x399a3040` |
| mouse yaw scale | 10.0 × 2π × 0.002 | `0x3999cce8`, `0x399b050c`, `0x399b0724` |
| walk flag | 0x1000 | `FUN_39404d50`, `or [ebp-0x30]` at 0x3940600d |
| camera_max_dist | 18.0 | `_DAT_3999f928`, `FUN_3915e7a0` |
| camera FOV | 60 / 42.75 | `option_camera_fov_set.cfg` |
| camera dive angle/pitch | 30° / 20° | `FUN_390f7b40` defaults |
| start distance | 4.0/3.2 etc. | `0x399b0c58` |
| swim side | 0.6 | `0x399a24cc` |
| ladder kinds | 4,8 vs 6,7,10 | `FUN_39402f60`, `FUN_39132be0` |
| movement thresholds | 0.01 m / 0.001 s | `0x3999cf90`, `0x3999cf30` |

---

## 9. Unknowns

1. **U1 - local `action_move` site / request consumer.** The local transform is applied through
   the entity (`actor+0xc` vtable +0x98, `3a6ab743`), not through a literal
   `IPhysicalEntity::Action(action_move)` in x2game. The only literal `action_move` calls found
   are resets (0x390c47f0) and remote paths. The request may be consumed by crygame
   `FUN_3906a7f0` (CPlayer::Update, 0x3906a7f0, 11284 B; see the existing re-mounts note).
   Settle with breakpoints on `CLivingEntity__Action` 0x35030E90 and on 0x3906a7f0 during a
   live walk, recording the request pointer and the `iJump`/dt fields.
2. **U2 - request flags.** The 0x9C-byte request's flag bits (0x1/0x8/0x20/0x40/0x800/
   0x8000000/0x20000000/0x80000000) are observed, not named. A live dump of the request at the
   `GameClient vtable +0x3C` call for idle/walk/run/jump/swim/mount/ladder gives the truth table.
3. **U3 - mouse-look flag.** The `useMouse` byte is `FUN_392aa250(3) && local_23`; which input
   state writes that table entry (right button? camera mode?) is not resolved. A driver test can
   set the entry directly and observe `FUN_39402e00`.
4. **U4 - CSMoveUnit serializer.** Packet class vtable `0x3999d574`, body copied by 0x39024610;
   the field-by-field wire order is in the vtable slot 0 (probably 0x3901c160, which is not a
   function start) and was not decoded. The server layout is known; the client writer needs a
   packet capture or a hook on `0x3ac1058c`.
5. **U5 - send cadence.** The pump passes a timer from `DAT_3a59f850->vtable+8()` in 0.001 s
   units; the interval itself (cvar or constant) was not isolated. Measure inter-packet times in
   the live client while walking.
6. **U6 - `unit+0x174` units.** The mount turn formula is confirmed but the stored unit
   (radians/s vs degrees/s) is inferred; the writer was not found.
7. **U7 - `CPlayer::ProcessFrameMovement` callers.** The wrapper and its tail code are thin
   `jmp` trampolines (`0x3913a620` -> `0x3a6ab5f3`, `0x3913da40` -> `0x3a98bb00`,
   `0x39139470` -> `0x3a9918bb`); the exact engine callback that invokes them is a vtable slot
   of the animated character, not a named x2game call.
8. **U8 - camera mode semantics 3-6.** Only mode 1 is mapped in detail; modes 3-6 are named by
   address only.
9. **U9 - jump impulse.** The jump key is action 6 and is used by the gliding vertical input in
   `UpdateMovement` (0x39404d50, `fVar13 = FUN_39424580(6)`), but no local vertical impulse site
   was found. The physics `action_move.iJump` path (1.2 cryphysics) is the likely mechanism.
10. **U10 - `FUN_392aa250` table.** The global input-mode table at `0x39d2ff30` is only read in
    x2game (0x39d2ff2c/0x39d2ff10); its writer is external/indirect.
11. **U11 - velocity smoothing near zero.** `FUN_39402c50` scales the delta by
    `1 - |delta| * 0.55` (`DAT_399c4e20 = 0.55`) when `|delta| <= 1.0`; the decompile also has
    an artefactual `*(float*)&param_2` in the `accel*dt > 1` branch, so record the original's
    output table for `accel*dt >= 1` and match that.
12. **U12 - mouse yaw fold. RESOLVED (phase A).** The two folds are the exact `f32`/x87
    sequence `t = ((a+c)-|a-c|)*0.5`, `result = ((t-c)+|t+c|)*0.5` with
    `a = axis*-0.002`, `c = (min(dt,0.033)*10)*2π`, equivalent to `clamp(a,-c,c)`; see §3.4.
    Captured goldens in `crates/aa-client/src/port/movement/goldens/mouse_*.json`.
13. **U13 - camera zoom cvar roles.** Which of `DAT_39afe204/208/24c/1cc` is speed, power, near
    and far is inferred from the code order; a live setter test (`uc_set_wheel_dist` etc.)
    settles it.

---

## 10. Cross-check against the existing notes

This map agrees with `re-movement.md` on the 90°/s keyboard turn, the 0.033 s cap, the 0x1000
walk bit, the ladder kinds, and the 18 m classic camera distance; it sharpens the following:

* `keyboard_rotate_speed` default is proven to be `0.5` at `0x3999c47c` (dump), not just inferred.
* the mount turn is not 360°/s: it is a 3 s ramp to `unit[0x174] × 0.5` (180°/s inferred).
* `camera_max_dist`'s **cvar default is 18.0**; the 10 m action-FOV value comes from the cfg
  group, not from the registration.
* the movement request is a 0x9C-byte stack object submitted through `GameClient vtable +0x3C`;
  it is not the same object as the packet body (0x128 bytes copied by 0x39024610).
* `CSMoveUnit` is built in x2game, not in crynetwork: `FUN_394046f0` (opcode 0x89) and
  `FUN_394045f0` (mounted).
