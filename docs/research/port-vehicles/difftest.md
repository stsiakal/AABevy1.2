# Differential tests against the running 1.2 client (S3)

Goal: prove each ported Rust function matches the original by calling the **original** function
inside the hidden 1.2 client (`aadriver.dll`, `aaemu-clientdriver`) with chosen inputs, then
comparing outputs. The driver only calls the shipped binary in memory; no original code is
shipped.

Driver docs: `C:/Users/stsiakal/AAEmu/rust/crates/aaemu-clientdriver/src/lib.rs`,
`native/aadriver.cpp` (op list at the top), `C:/Users/stsiakal/AAEmu/rust/docs/client/LUA-DRIVER.md`.

## 1. Driver commands used here

```
'c' <target> <conv> [this=0x...] [dump=0x...:<len>]... [unsafe] [void] [args...]
'A' <len>            -> address of tracked scratch memory
'D' <address>        free scratch
'R' <addr> <len>     read memory (hex)
'w' <addr> <hex>     write (scratch, or anywhere with `unsafe`)
'g' <addr> <type>    peek a global (u32/u16/u8/f32/f64/str/ptr)
'M' <module>         module base/size/timestamp
```

Argument forms: `0x...`/decimal (u32), `u32:0x...`, `f:1.5`, `p:0x...` (pointer to scratch).
`this=` is ECX for `thiscall`; for `fastcall` it is the first register argument and `this=`
must be omitted (use `ecx` as the first arg in the call list). `void` clears `capture_st0`
for functions that do not return a float.

Targets are `x2game.dll+<rva>` (preferred base `0x38FF0000`, so `rva = VA - 0x38FF0000`) or a raw
address. `M x2game.dll` confirms the loaded base once per session.

### 1.1 Conventions for the functions in this map

| function form | call |
|---|---|
| `__thiscall f(this, a, b)` | `c x2game.dll+0x... thiscall this=0x... <a> <b>` |
| `__fastcall f(this)` (x2game style) | `c x2game.dll+0x... fastcall this=ctx 0` **or** `thiscall this=ctx` — check the prologue; the decompiler prints `__fastcall` for some `this`-only methods |
| `__cdecl f(a)` | `c x2game.dll+0x... cdecl <a>` |
| `float`/`float10` return | default `capture_st0`; pass `void` when the function returns no value so the x87 stack stays balanced |

Some functions return a 64-bit value (e.g. `FUN_39024610` returns the destination pointer in
EAX). Use `void` and depend on the `dump=` buffers.

## 2. Pure leaves (phase 0) — no game objects

### 2.1 `FUN_3922aed0` clamp (0x3922aed0, 39 B, `__cdecl float(float v, float lo, float hi)`)

```
A 16
w <buf> <float bits of v, lo, hi>
c x2game.dll+0x3922aed0 cdecl p:<buf> p:<buf+4> p:<buf+8>     ; reads from scratch
```
Better: pass `f:` arguments (the driver pushes the bit pattern):
`c x2game.dll+0x3922aed0 cdecl f:3.0 f:0.0 f:1.0` → ST0. Cases: v = -1, 0, 0.5, 1, 2.

### 2.2 Position codec `FUN_3961e2c0` (0x3961e2c0, 972 B)

This is a *method*: `void f(FixedPos* out, Archive* ar)`, `out` in ECX. `FixedPos` is the
20-byte engine fixed (`i64 x, i64 y, f32 z`); the call site in the serializer is
`lea ecx,[move+8]` (0x397c0524). The decompile shows two params and the caller passes the
archive on the stack. So:
`c x2game.dll+0x3961e2c0 thiscall this=<out_scratch> dump=<out_scratch>:20 <archive_ptr>`
where `archive_ptr` is the test-double archive (below).

* Write direction: archive is in write mode (`+0x1c` returns 0); the double captures the 9
  bytes. Input `out = {x,y,z}`. Compare the 9 bytes and then decode them with the Rust port.
* Read direction: archive in read mode (`+0x1c` returns 1) preloaded with the 9 key/value bytes;
  the function writes the fixed to `this`. Compare the 20 bytes.
* Test vectors: (0,0,0), (0,0,-100), (0,0,4096), (1,2,3), (-1.5, 0.25, 7.125), a random sweep
  over the whole range, and the boundary bits of `iz2` (x/y signs and z bits 16..21).
* **U13**: the fixed's metre unit is not in the codec. Measure it once: hook the sender
  (`0x394045f0`) or the GameClient filler, encode a known entity position, and read the 9 bytes;
  record `wire_step = 8 * fixed_unit`. Keep it a named constant in the port.

### 2.3 Quaternion codec `FUN_397ad8c0` / `FUN_397ae020`

`FUN_397ad8c0(i16 out[3], const float v[3])` (ECX = out):
`c x2game.dll+0x397ad8c0 thiscall this=<out> dump=<out>:6 p:<in>`.
`FUN_397ae020(i16 out[3], const float q[4])` (ECX = out):
`c x2game.dll+0x397ae020 thiscall this=<out> dump=<out>:6 p:<quat>`.
Vectors: unit axes, 45°, a q with w<0, q not normalised, q with x²+y²+z² > 0.9975 (w = 0), and
round-trip pairs. Compare the 6 bytes and the reconstructed floats.

### 2.4 Velocity codecs `FUN_397ad5a0`, `FUN_397ada60`, `FUN_397ad740`, `FUN_397adc00`

Same shape as 2.3 (3×i16 or 3×u8). Sweep scale edges: ±30, ±60, ±50, 0, 1/32767, and
`32767/scale` (saturation).

### 2.5 The whole move serializer `FUN_397c0470` (0x397c0470, 3,410 B) — **do this first**

Signature: `void f(MoveUnit* move, Archive* ar)` (ECX = move, archive on the stack). It writes
or reads the whole move for `move->type` in {0,1,2,3,4,5}.

**Test-double archive.** Allocate a 0x400-byte struct whose first dword is a vtable array with
the slots used by the serializer:

| slot | signature (write mode) | (read mode) |
|---|---|---|
| +0x1c | `bool is_reading()` → **0** | → **1** |
| +0x3c | `void u32(key, u32* v, 0)` | copies from the scripted map into `*v` |
| +0x40 | `void u16(key, u16* v, 0)` | ditto |
| +0x44 | `void u8(key, u8* v, 0)` | ditto |
| +0x4c | `void i32(key, i32* v, 0)` | ditto |
| +0x50 | `void i16(key, i16* v, 0)` | ditto |
| +0x54 | `void i8(key, i8* v, 0)` | ditto |
| +0x74 | `void f32(key, f32* v, 0)` | ditto |
| +0x78 | `void bool8(key, u8* v, 0)` | ditto |
| +0x18 | `void end_scope()` | — |
| +0x14 | `bool begin_scope(name, 1)` | — |

The writer appends `{slot, key, raw bytes}` to a buffer in call order; the reader returns the
next scripted value for a key. Implement the double as a small C stub compiled into the driver
harness (or a raw x86 thunk table allocated with `'A'`; a C stub is simpler and is not original
code).

**Test cases**

1. For each type 0..5, build a synthetic `MoveUnit` (all fields distinct: pos (1,2,3),
   quat (0.1,0.2,0.3), vel (4,5,6), angVel (0.7,0.8,0.9), steering 0.5, wheels 0.1..0.4,
   ship steering -3, throttle 7, zone 0x1234, stuck 1, transfer fields).
2. Run write mode, capture the call sequence and bytes; run read mode from the same call
   sequence into a zeroed `MoveUnit`; compare the resulting struct field-for-field with the
   input (allowing only the documented lossy fields: position 1/8, i16 quantization, i8
   truncation).
3. Repeat with boundary values (0, ±1, saturation, NaN-free) and with `flags` bits 0x10/0x20/
   0x40/0x80 to exercise `scType`/`phase` and actor flags.
4. Type 2 with 0..18 wheels (the apply clamp is 18, the serializer loops `count`, so test 0, 1,
   2, 4, 18; larger counts are not produced by the client).

This single test covers `packets.md` §5–§8 and is the contract for all four areas.

### 2.6 Move-struct copy `FUN_39024610` (0x39024610, 270 B)

`MoveUnit* f(MoveUnit* dst, const MoveUnit* src)` (ECX = dst). Zero a 0x130 buffer, fill the
source with a pattern, call, dump 0x130 from dst, byte-compare with a Rust copy. Include a case
with the source == destination (aliasing) if the client ever does it (it does not).

### 2.7 Quaternion helpers `FUN_390bf0f0` / `FUN_390bf210`

`FUN_390bf0f0` builds a quaternion from an axis-angle (ECX = out? check the prologue) and
`FUN_390bf210` multiplies. Test with random unit axes/angles against glam, then against the
original. These two are used by the ship helm/rudder and the driver IK.

## 3. Stateful functions with a synthetic object

### 3.1 Controller velocity filter `FUN_390967c0` (0x390967c0, 231 B)

`void f(float out[3], const float in[3])` with `this` = controller. Allocate a 0x1a0 zeroed
buffer, set `+0x15c` and `+0x174`, call, dump 12 bytes. Cases: in along each axis, near the
0.0873 threshold, signs, and `+0x15c` = 0 (the `0/0` branch).

### 3.2 Kinematic integrator `FUN_390a3de0` (0x390a3de0, 1,175 B)

Needs `this` = controller and a fake entity via the controller's owner (`+8 -> F`, `F+8 = M`,
`M+8` = the embedded entity with the function-pointer table). Too much to fake by hand; do it
with a live model (phase 3) or extract the pure math (quaternion integration, `pos += v*dt`)
and test that as a Rust unit test with the original's constants, then compare the whole
function once on the live model.

### 3.3 Ship attach-point state `FUN_390e8e40` (0x390e8e40, 904 B)

`void f(Doodad* d, u32 point, float* payload7)` with `this` = M. A live ship model is required
for the sail array allocation but the *math* is pure: given `payload7` and the point, compare
the written fields. Since the function also detaches the previous doodad, use a live model and a
real/fake doodad whose vtable methods only refcount. The driver can pass a scratch doodad whose
first dword is a 2-entry vtable (`+4` addref, `+8` release) and whose `+0x70` returns null (the
bone lookup path is skipped). Test points 21, 22, 35 and an unrelated point.

### 3.4 Visual functions with a live model

`UpdateRudder` (0x390c7410), `UpdateSteeringWheel` (0x390d2990), `UpdateThrottleAnim`
(0x390db700), `UpdateCenterSpindle` (0x390cffd0), `UpdateGunTurret` (0x390d14b0) all need a real
model + skeleton/anim. Recipe:

1. Board/summon a ship or wagon in the hidden client (Lua or the normal flow).
2. Capture the model pointer `M` once with a 5-byte `jmp` hook on the model's `Update`
   (vtable slot 0 = `0x390ebaa0`/`0x390ebc50`): write `E9 rel32` at the function start, the
   stub stores `ECX` in a global and jumps back. The driver writes the stub with `'A'` and the
   bytes with `'w ... unsafe'`; remove the hook with `'w'` of the original 5 bytes (keep a copy).
3. With `M` known, set the input fields (e.g. `M+0x358 = throttle`, `M+0x35c = steering`) with
   `'w ... unsafe'`, call the function with `this=M`, read the resulting bone/child transforms
   with `'R'`, compare with the Rust port driven by the same inputs.
4. For bone comparisons, snapshot the skeleton bone array address once (`M+8` entity ->
   `+0x1e8` skeleton) and dump the affected bone matrices before/after.

Test vectors per function:

| function | inputs | compared |
|---|---|---|
| rudder | steering -1, -0.5, 0, 0.5, 1; child present/absent | child quaternion/pos |
| helm wheel | dt 0.016/0.1, steering ±1, wind 0/7 | `+0x360`, helm quat |
| throttle anim | throttle -4, -1, 0, 1, 4; gate 0/1 | chosen clip name/id, rate, flags |
| spindle | wheel travel pairs, dt 0.01/0.1 | `+0x3f8`, `Bone_spindle` quat |
| turret | aim yaw/pitch ±180°, pitch ±90°, `param_2` 0/1 | `bone_yaw`/`bone_pitch` quats |

## 4. Movement apply and packets (phase 4)

### 4.1 Send pump `FUN_394045f0` (0x394045f0, 243 B)

Hook the send thunk `thunk_FUN_3ac1058c` (0x39186810) or the `thunk_FUN_3ac1058c` call site:
store the 5-byte original, jump to a stub that copies the packet object (the pointer in ECX/arg)
to scratch, then returns. The move struct is at `packet+0x10` if the header is 0x10 bytes
(`local_13c` vs `local_12c`). Safer: hook at the call instruction and dump `[esp+4]`
(the packet pointer), then `R` `ptr+0x10` for 0x130 bytes.

Compare:
1. opcode (`0x89`), unit id, timing (timer reset),
2. the 0x130 move struct against the Rust builder for the same input state,
3. the change suppression (send twice with no change; expect one packet).

### 4.2 Receive `FUN_391d4e70` / `FUN_391e55c0` / `FUN_391d4d60`

Read the queue (`GameClient+0x117c`, count `+0x1180`) after a server move: the queued entries
are 0x130 move copies. Compare the bytes with the Rust decoder for the same server packet. For
the flush, hook `GameClient->vtable[0x40]` (U1) or compare the resulting controller fields
(`N+0x188..N+0x1bc`) with the Rust apply.

### 4.3 Interpolation `FUN_390b2420` / `FUN_390b0040`

With a live vehicle and a scripted target (`N+0x1a8/+0x1b0`, `N+0x1b8/+0x1bc`, `N+0x4c`), call
the controller update with a fixed dt (the function reads the engine time itself; freeze the
time by pausing the client or accept the measured dt and use it in the Rust comparison). Compare
the applied pose (`+0x144/+0x134`) and the model setters' arguments. Also test:
* target 0: stop path (`0x390af650`),
* target 1 (timeout): kill path,
* target within 400.0: ground-align branch,
* target beyond 400.0: snap branch.

## 5. Flow tests (phase 5)

### 5.1 Bind precheck `FUN_39346290` (0x39346290, 517 B)

`bool f(target_actor)` with `this` = a state object; reads the local player from
`DAT_39b02294+0x10c`. Cases: valid slave, dead, busy, out of range (4 m), occupied, on a
transfer, and the "no parent" special path. Compare the return and the error id passed to
`FUN_39428370` (hook it or read the system message).

### 5.2 Interaction dispatch `FUN_393bf310`

Feed each interaction id (`DAT_39ac7974`, `...79c4`, `...79e8`, `...7978`, `...79bc`, `...797c`,
`...7980`, `...79b0`) and compare which sub-function is called (hook the callees) and the bool
written to `param_5`.

### 5.3 Transfer board/leave

Board a transfer: compare the `CSBoardingTransfer` bytes (`tl u16`, `ap u8`) and the free-point
choice. Leave: `ap = 0` and the transfer tl. Then apply a type-5 move and compare the player's
parent/point and pose with the Rust port.

### 5.4 Summon locator

Summon a wagon and a ship; move the cursor/scroll wheel; compare:
* the ghost pose (position + yaw),
* the validity bool (`FUN_393488d0`/`FUN_39348bf0`),
* the material swap (`FUN_39346580`).

The locator object address is found the same way (hook `0x3934a3e0` and capture ECX).

### 5.5 UI getters

With a live vehicle: `GetSiegeWeaponSpeed` and `GetSiegeWeaponTurnSpeed` for forward/back/side
motion and at rest; compare the rounded values with the Rust port. `GetVehicleInfo` must be nil
for the wagon/ship and non-nil only for a wheeled sim (speedcar).

## 6. Goldens and CI

Store each test as a JSON line:

```json
{"fn":"397c0470","case":"type2-4wheels","inputs":"...","calls":[[3,"flags",1],[80,"vel.x",123]],
 "out":"<hex>","note":"write mode"}
```

The harness replays the call sequence against the Rust port (a small `archive` mock) and
compares. CI runs only the Rust side; the hidden-client run is a manual/oracle step that
regenerates the JSON. Keep the original bytes out of the repo if they contain client data; the
test vectors above are synthetic.

## 7. Safety and cleanup

* The driver runs on the client main thread; every call is SEH-guarded. A crash in a called
  function kills only the hidden client — acceptable, but free scratch (`D`) and remove hooks
  before detaching.
* Never hook a function that the client is currently executing in another thread; do it between
  frames (the driver already queues to the main thread).
* Keep the original 5 bytes of every hook in the harness log so a failed run can be undone.
* The client must be in the world with the test vehicle summoned; no server-side state is
  needed beyond that.
