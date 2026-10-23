# Differential tests against the running 1.2 client

Goal: prove each ported Rust function matches the original by calling the **original** function
inside the hidden 1.2 client (`aadriver.dll`, already injected in-process and running Lua) with
chosen inputs, then comparing the outputs with the Rust port.

This file lists the calling convention, the object/state each call needs, and a recipe per
function group. It does not propose shipping any original code; the driver only calls the
shipped binary in memory.

## 1. Calling conventions on MSVC x86

| binary form | declaration for the driver |
|---|---|
| `__thiscall f(this, a, b)` | `__fastcall f(void* this, void* edx_dummy, a, b)` (MSVC passes `this` in ECX, the next arg in EDX; put a dummy in EDX and keep the stack order) |
| `__fastcall f(this)` (x2game style) | `__fastcall f(void* ecx, void* edx)` |
| `__cdecl f(a, b)` | `__cdecl f(a, b)` |
| `float` / `float10` return | declare `float` (x87 ST0 is the ABI return; do not declare `double`) |
| 64-bit struct return | avoid; all listed functions return scalars or write through pointers |

The `thunk_FUN_xxx` at `0x39xxxxxx` are 5-byte `jmp` trampolines into the unpacked tail; call the
original VA from `funcs.csv` (which is the tail body) or the thunk, both work.

All addresses below are **runtime VAs** (module base `0x38FF0000`). Resolve them from
`GetModuleHandleA("x2game.dll") + (va - 0x38FF0000)` in the driver; the image is loaded at its
preferred base in the shipped client.

## 2. Building the state

### 2.1 Synthetic state (phase A leaves)

For pure functions, allocate zeroed buffers and fill the documented fields; no game objects are
needed.

```c
typedef struct { float x, y, z; } vec3;
static void* alloc_state(size_t n) { void* p = calloc(1, n); return p; }
```

Call in the driver's Lua-exposed C function, e.g.:

```c
// float FUN_3922aed0(float v, float lo, float hi)  (__cdecl)
typedef float (__cdecl *clamp_fn)(float, float, float);
float r = ((clamp_fn)va(0x3922aed0))(v, 0.0f, 3.0f);
```

### 2.2 Live state (phases B-D)

Resolve the live objects once per frame through the same accessors the client uses:

```c
// actor from a unit id (FUN_39360020)
typedef void* (__cdecl *get_unit_fn)(uint32_t id);
void* actor = ((get_unit_fn)va(0x39360020))(my_unit_id);

// GameClient = *(actor + 0x3be8)
void* game = *(void**)((char*)actor + 0x3be8);

// controlled unit = GameClient->vtable[0x208]()
typedef void* (__fastcall *get_controlled_fn)(void* game, void* edx);
void* unit = ((get_controlled_fn)vtable_slot(game, 0x208))(game, 0);

// physics entity = (*(entity + 0x108))()
void* entity = *(void**)((char*)actor + 0x0c);
void* phys   = ((void* (__fastcall*)(void*, void*))vtable_slot(entity, 0x108))(entity, 0);
```

The player actor pointer is stable while the world is loaded; re-resolve after a map change.
The `InputState` used by UpdateMovement is **inline at `wrapper+4`** in the object passed to
`0x39406540` (`cmp byte [esi],0`; `lea ecx,[esi+4]` at 0x394065e3); it is not a pointer.
The simplest capture is a one-time `jmp` hook on `0x39404d50` that records `ECX` into a global.
Use that pointer for all input tests.

## 3. Per-function recipes

### 3.1 `FUN_3922aed0` clamp (0x3922aed0, 24 B)

* convention: `__cdecl float (float v, float lo, float hi)`
* inputs: `v` in {-1, 0, 0.5, 3, 4}, `lo=0`, `hi=3`
* expected: `min(max(v,lo),hi)`
* tolerance: exact bits (no NaN).
* Rust target: `clamp`.

### 3.2 `FUN_39112690` direction factor (0x39112690, 1615 B)

* convention: `__thiscall float (Actor* actor, float* dir)` (ECX=actor; the direction pointer
  is the only stack arg). Verified at the caller `0x3912b54a-0x3912b55b`: `mov ecx,[esi+0x40]`
  (the actor field of the CPlayerMovement object).
* live/synthetic state: a zeroed actor whose `+0x3ec` movement block has the DB values from
  `structs.md` §7
  (sprint 1.5, forward 1.0, strafe 1.0, back-diag 0.65, back 0.65, walk set 0.45/0.45/0.45/
  0.2925/0.2925; walk flag set/clear).
* inputs: 8 cardinal/diagonal unit vectors; walk on/off; a few non-unit vectors.
* expected: the multiplier from the table; check the exact sector boundaries (45° steps).
* tolerance: 1e-6.

### 3.3 `FUN_39402590` combiner (0x39402590, 246 B)

* convention: `__cdecl void (char fwd, char back, float* axisL, float* axisR, char* left,
  char* right, char mouse0, char mouse1, char flag9)`.
* Note: the decompile has `unaff_ESI/unaff_EDI` for the left/right outputs; confirm the real
  argument order with capstone at the call site `0x39405092` before writing the wrapper.
* inputs: all 2^4 key combos x {axis0..3 = 0, 0.5, 1} x mouse0/1 x flag9.
* expected: the Rust combiner bit-for-bit; assert the four output bytes and the two floats.

### 3.4 `FUN_39402e00` turn delta (0x39402e00, 349 B) - **the key test**

* convention: `__fastcall float (void* input_state_this, void* edx, void* unit, float axis5,
  float axis4, uint32_t use_mouse)`.
* state:
  - `input_state_this`: the live per-unit InputState captured from the hook on `0x39404d50`
    (or a 0x100-byte zero buffer; only `+0x38` is used). The driver should restore `+0x38`
    after each call.
  - `unit`: the live controlled unit pointer. For the mount case it must be the horse actor with
    `+0x1c1 == 4` and a valid `+0x174`; for the foot case pass `NULL` or the player actor
    (`+0x1c1 != 4`).
  - `use_mouse = 0` for keyboard tests; `1` for the mouse path.
* inputs:
  - axes: (-1,0,1) for (axis5 turnright, axis4 turnleft), including both at once;
  - dt: use the live game dt (the function reads it itself, so no dt argument); to test the cap,
    slow the client with a debugger pause between frames or call twice in a row and compare;
  - `unit->+0x174` = 6.283185 (full) and 0.0 (edge).
* expected:
  - foot: `(axis4-axis5) * min(dt,0.033) * 0.5 * π`;
  - mount: `(axis4-axis5) * min(dt,0.033) * 0.5 * clamp(acc,0,3) * (unit[0x174]/3)`;
  - mouse: `FUN_3915c700()` (call it separately with the same mouse axis and compare).
* tolerance: 1e-6 relative; for the mount ramp, drive `turn_accum` from 0 to 3 in 0.1 steps and
  compare the whole curve.
* This test pins MAP §3 and resolves U6 (units of `unit+0x174`) by trying both readings.

### 3.5 `FUN_3915c700` / `0x3915c7d0` mouse deltas

* convention: `__cdecl float (void)` (they read the live axis 8/9 and dt through the InputManager).
* inputs: inject mouse dx/dy via the client's input (or call `FUN_393fd280` to set the action
  state); run twice with the same dt.
* expected: the Rust `mouse_yaw(dt, dx, sensitivity)` and the pitch counterpart.
* tolerance: 1e-6.

### 3.6 `FUN_39402c50` smoothing (0x39402c50, 427 B)

* convention: `__fastcall void (InputState* this, void* edx, vec3* desired)`.
* state: `this+0x24..0x2c` current velocity, `this+0x34` accel; save/restore the 0x14-byte
  block `+0x24..+0x38` around the call.
* inputs: desired vectors of length 0, 1e-4, 0.01, 1, 10; accel 0, 1, 10; dt = live.
* expected: zero for tiny input, snap for accel <= 0, otherwise the accelerated step; the exact
  near-zero shortening is U11 (see below) - record the original output table and port it.
* tolerance: 1e-6.

### 3.7 `FUN_39402f60` ladder filter (0x39402f60, 126 B)

* convention: `__cdecl` with the unit in EAX (the decompile uses `in_EAX`); use an `__asm`
  wrapper or a `__fastcall` with the unit in ECX if capstone shows ECX.
* state: a fake `unit` with `+0x23c` set to 4,5,6,7,8,9,10 and `+0x1c1` arbitrary.
* inputs: forward/back/turn floats >0.
* expected: per-kind flags; compare the 4 bytes + 2 floats.
* This resolves the direction of the conversion (MAP §1.4 [I]).

### 3.8 `FUN_39404d50` UpdateMovement (integration)

Do not diff the whole 5971-byte body first. Instead:

1. Hook `0x39404d50` entry, save `ECX` (InputState*), run one frame, and copy the 0x9C-byte
   request at the `GameClient vtable +0x3C` call (hook `0x394063a6`; the request pointer is in
   ECX/stack there).
2. Feed the same captured InputState fields and the same dt into the Rust `update_movement`.
3. Compare:
   - the request bytes (flags, velocity, kind, pseudo-speed) - this is the real contract;
   - the updated InputState fields `+0x38/+0x44..0x98/+0xac..0xc0`.
4. Matrix: idle, W, W+A, A only, S, walk toggle, autorun, jump held, glide, mount, ladder kinds
   4/6/7/8/10, external goto target, swim (stance 3).
5. Once the flags are named (U2), tighten to exact equality; before that, compare field by field.

The live byte stream from the hook is also the oracle for the Rust `MovementRequest` layout.

### 3.9 `FUN_39118650` CActor::UpdateMovementRequest

* convention: `__fastcall void (void* actor, void* edx)`.
* state: the live player actor; must have `actor+0x2c` (input object) and `actor+0xc` (entity).
* inputs: the InputState request from 3.8; run the same frame in the original and in Rust.
* compare: `actor+0x2b0..0x2dc`, `+0x96c..0x974`, `+0x9dc`, `+0x54c`, `+0x1a4..` after the call.
* caveat: the function reads the physics status through the entity, so the Rust side needs the
  same ground/water query results. Snapshot them from `GetStatus` (slot 9) and feed the Rust
  actor.

### 3.10 `CPlayerMovement::Process` (0x3a6ab5f3) and `CPlayerRotation`

* convention: `__fastcall` with ECX=&movement (0xC0 bytes), `[ebp+8]=actor`,
  `[ebp+0xc]=request` (13 dwords), `[ebp+0x10]=dt`.
* call it with a live actor and the real request, then compare `actor+0x2b0..0x2dc`,
  `actor+0x33c..0x358`, `actor+0x950..0x95c` and the movement/rotation out-quats.
* rotation: call `0x3913ff90` (`__fastcall`, ECX=&rotation) with a zeroed `CPlayerRotation`
  built by `0x3913ded0`, feed quats from the actor, compare the out quat.
* these are stateful; always restore the actor fields after each call (copy 0x400 bytes around).

### 3.11 Camera mode 1 (`0x39169860`)

* convention: `__thiscall void (CameraManager* cam, float dt)`.
* state: `DAT_39afe2b8` is the live manager; snapshot 0x360 bytes, call the original, then the
  Rust camera with the same inputs (target id, char quat, mouse deltas, cvars).
* compare: `+0x20/+0x24` (distance), `+0x3c..0x44` (position), `+0x80..0x90`, `+0x268..0x274`.
* matrix: standing, running, keyboard turning (camera follows char), mouse look, zoom in/out,
  collision (stand in a doorway), first person (wanted < 1), mount.
* for collision/fade, first diff only the geometric part (before `0x39165420`) to separate the
  ray queries from the math.

### 3.12 Packet

* hook `0x3ac1058c` (the packet finalizer) and dump the buffer for one walk/stop/turn/jump.
* compare with the Rust serialiser byte-for-byte (opcode 0x89 at +4, id at +8, body after that).
* cadence: log the return value of the timer at `state+4` on every call; this settles U5.

## 4. Harness plumbing in `aadriver.dll`

Recommended exports (names illustrative):

```c
// movement_test.cpp (native part of the driver)
extern "C" __declspec(dllexport) int  aa_move_call(int which, const void* args, int nargs,
                                                   void* out, int out_cap);
extern "C" __declspec(dllexport) int  aa_move_capture(int which);   // snapshot live state
extern "C" __declspec(dllexport) int  aa_move_restore(int which);
```

* `aa_move_call` dispatches by id (0 = clamp, 1 = direction factor, 2 = combiner, ...), copies
  the returned bytes to Lua, and never lets a bad pointer escape (validate against the known
  actor/InputState addresses first).
* `aa_move_capture`/`restore` copy the 0x100/0x400/0x360-byte regions listed above; tests run
  original -> restore -> Rust and compare, so one test cannot poison the next.
* Run the client with `--shot`/`--exit-after` or the existing hidden-window mode; the tests are
  headless and read-only with respect to the pak.

## 5. Tolerances and failure policy

| class | tolerance |
|---|---|
| integer flags/ids | exact |
| float math (turn, direction, zoom) | 1e-6 relative or 1e-7 absolute |
| quaternions | 1e-5 per component, sign-insensitive (q and -q equal) |
| packet bytes | exact after the U2/U4 mapping |
| position after one physics step | 1e-4 m (physics port tolerance) |

Any mismatch is a port bug until proven otherwise; log `(function, inputs, original, rust, delta)`
to a file under `data/extract/aa-movement/` (git-ignored) and keep the seed case in the test.

## 6. Open items that the harness must settle

* U1: breakpoint on `CLivingEntity__Action` 0x35030E90 during walk/jump; record id, dir, iJump,
  dt cap.
* U2: dump the 0x9C request for the 12-case matrix; name every flag bit.
* U3: set `DAT_39d2ff30[3]` from Lua/driver and observe `FUN_39402e00`'s `use_mouse` branch.
* U4: capture the serialized CSMoveUnit buffer for one frame and align the 0x128-byte body.
* U5: log the send timer and the `DAT_3a59f850` value per frame.
* U6: set `unit+0x174` to 6.283185 vs 360 and compare the turn rate.
* U9: hold jump during a walk and breakpoint every physics action.
