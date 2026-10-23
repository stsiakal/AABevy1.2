# Differential tests for housing placement against the running 1.2 client

Tool: `aaemu-clientdriver` (in-process `aadriver.dll`; `usage.txt` §`call`, `read`, `write`,
`alloc`, `peek-global`; `src/lib.rs` `Call`/`Arg`). All x2game targets are
`x2game+<VA-0x38FF0000>`; `--pid` optional when one client runs. The driver wraps `call` in SEH.

Rust side: ported function with the same name; golden data from these calls lives in the crate's
tests, so CI runs without the original client (`port-program.md` step 4).

## 1. Calling conventions

| function | declaration | driver |
|---|---|---|
| `FUN_3956b860(code)` | `__cdecl const char* (int)` | `call x2game+0x57b860 cdecl 0x70` |
| `FUN_39897070(cat)` | `__cdecl int (u32)` | `call x2game+0x8a7070 cdecl 0xc` |
| `FUN_3908c810(desc)` | `__fastcall int (void* desc)` (ECX) | `call x2game+0x9c810 fastcall this=<desc>` |
| `FUN_3908c850(unit)` | `__fastcall u8 (void* unit)` | `call x2game+0x9c850 fastcall this=<unit>` |
| `FUN_3961dd70(origin, out, in)` | `__thiscall (void* origin, void* out, float3* in)` | `call x2game+0x62dd70 this=<origin> p:<out> p:<in>` |
| `FUN_39016fe0()` | `__fastcall float (void* junk)` (no input used) | `call x2game+0x26fe0 fastcall` |
| `FUN_3933a300(builder, pos)` | `__thiscall (builder*, float3*)` | `call x2game+0x34a300 this=<builder> p:<pos>` |
| `FUN_3933a380(builder, delta)` | `__thiscall (builder*, float)` | `call x2game+0x34a380 this=<builder> f:<delta>` |
| `FUN_393363b0(builder)` | `__fastcall (builder*)` (ECX) | `call x2game+0x3463b0 fastcall this=<builder>` |
| `FUN_3903c5a0(cursor)` | `__fastcall (cursor*)` | `call x2game+0x4c5a0 fastcall this=<cursor>` |
| `FUN_393a8fa0(builder, unit)` | `__thiscall (builder*, unit*)` | `call x2game+0x3b8fa0 this=<builder> p:<unit>` |
| `FUN_39327de0(manager)` | `__fastcall (manager*)` (no stack args) | `call x2game+0x337de0 fastcall this=<manager>` |

`float10` returns are plain `float` on the ABI; the driver prints raw ST0 as f64 (use `void` when
the function returns nothing). `__thiscall` is `__fastcall` with ECX=`this`; the driver's
`this=` handles it.

Live pointers:
* `GameClient` = `peek-global 0x39b02294 ptr`.
* `manager` = `peek-global 0x39aff98c ptr`; `builder` = `read <manager>+0x30 4`, `cursor` = `+0x34`.
* `local player actor` = `GameClient+0x10c`; unit id = `GameClient+0x40+0x18`; unit =
  `FUN_39360020(id)` (no source needed for the wrapper: call with the id).

## 2. Test levels

### 2.1 Pure leaves (no live state)

1. **Error table** `FUN_3956b860`: for `code = 0..0x400`, call and compare the returned C string
   (read at EAX; string table is static, NUL-terminated). This covers the 783 cases in one table test.
2. **Category/kind** `FUN_39897070`, `FUN_3908c810`, `FUN_3908c850`: allocate a 16-byte scratch
   desc with `alloc`, write the category at `+0xc`/dot flag at `+0xd`, call, compare. Matrix: all
   categories in the DB (1..18, 0, 0xffffffff).
3. **Position conversion** `FUN_3961dd70`: allocate 12-byte input float3 and 24-byte output; use
   `this=GameClient+0x118`; input values {-1000, -1, 0, 0.5, 1024.5, 2000.25, 1e5}, compare the
   i64/f32 output with `coord_to_long + origin`. This is the exact fixed-point contract of 0x56,
   0x57 and 0xE6 (0x58 is house-local floats and needs no conversion).
4. **Quaternion/vector helpers** `FUN_3903ba20` (axis-angle → 3x3, 841 B), `FUN_39328630` (nearest
   OBB house, 386 B), `FUN_393287c0` (box overlap, 500 B), `FUN_393289c0` (neighbour overlap, 1719 B):
   feed synthetic arrays (alloc/write) and compare floats; all are pure given the arrays.

### 2.2 Builder state machine (live builder, synthetic inputs)

Trigger a real placement once (use a housing design item; Lua/item task), then capture:

```
manager  = peek-global 0x39aff98c ptr
builder  = read manager+0x30 4
alloc 0x220  -> snapB
read builder 0x220 -> snapB                 # copy live object
read builder+8 4 -> unitptr; read unitptr 0x400 -> snapU
```

For each test: `write builder 0x220 <snapB>` (restore), call `SetPos`/`Rotate`/`FUN_393363b0` with a
scratch position, then read `builder+0x0c` state, `+0x10`/`+0x14` errors, the plot vector
`+0x1fc/+0x200` and the unit transform. Compare with the Rust builder fed the same position and
world.

Matrix:
* SetPos: player position + {0, 1, 10, 30, 31, 100} m along x/y, a position outside the housing
  area, over water, on a slope > 6 m, inside another house, inside a unit.
* Rotate: delta from each of the six wheel actions, including two actions at once (the function
  returns the first non-zero only), yaw wrap across ±pi.
* `FUN_394298c0` screen ray is exercised by calling SetPos with the ray output; for the ray itself
  use the same window size as the capture and compare the three floats.

`FUN_39016fe0` test: call `FUN_39424580(0x5a)` separately to read the live axis in the same frame,
then call the delta function; compare with `axis * 1deg * cvar`. Repeat after injecting one wheel
notch (`client_wheel +1`) and after Shift/Alt/Ctrl chords. This settles U1 (the axis amplitude).

### 2.3 Validation functions

Call the check functions directly on the restored builder (and on synthetic copies of the world
objects). The error slot write is the observable:

* `FUN_39333d40`: positions at range-1, range, range+1; no player vs live player.
* `FUN_39333e20`: category/housing-area matrix; use `FUN_39897070` categories.
* `FUN_39334060`: inside/outside a dominion/guard radius (pick live points).
* `FUN_39334380`: above/at/below the water level (read the engine water query first).
* `FUN_39335860`/`FUN_393354c0`: place the candidate OBB against another live house/unit.
* `FUN_39334540`: height band below/inside/above.
* `FUN_39332180`/`FUN_39337fb0`: 6 m spread, count and spacing of the plot points (compare the
  float3 vector, not just the error).

Compare `(error, plot points, unit transform)`; the side effects on the world (temp unit) are
restored from the 0x400-byte unit snapshot.

### 2.4 Decoration

Start one live decoration (item task 6), then:

```
manager = peek-global 0x39aff98c ptr
cursor  = read manager+0x34 4
alloc 0x54; read cursor 0x54 -> snapC
```

For each candidate: `write cursor 0x54 <snapC-with-edited transform>`, call `FUN_3903c5a0`
(this=cursor), read cursor+0x40. Edit `+0xc..+0x3c` (transform/yaw) and `+0x48` (surface) only; the
pointer fields (`+0`, `+4`, `+8`) stay live. Matrix: a point inside the house, in the garden, outside
the house, on a wall, on the ceiling, overlapping another doodad, near a unit; each surface flag from
`housing_decorations`.

`FUN_3903da00` needs the mouse; drive it through `client_pointer` (MCP) and compare the resulting
`+0xc..0x3c/+0x49/error` with the Rust ray cursor. `FUN_3903d6a0` is observable through the ghost
material pointer and transform (read ghost+0xc..0x3c).

### 2.5 Seed/farm placement

Trigger item task 9, then `builder = DAT_3a17a948` (`peek-global 0x3a17a948 ptr`), snapshot 0x54 and
the ghost 0x3b0. Call `FUN_393a8fa0(this=builder, unit)` with the live unit and candidate transforms;
read `builder+0x10`. Matrix: farm vs non-farm, another farm's crop, player unit overlap, doodad
overlap, >30 m, height difference >6 m, water/terrain invalid.

`FUN_393a4cc0` marker meshes: compare the count (`ghost+0x900`) and the three CGF names/positions;
graphics-only, exact string compare.

### 2.6 Packets

The `call` command cannot see the packet stack object. Add (in the driver, later phase) a capture
hook on the finalizer `0x3ac1058c` (or on `FUN_39186810`) that copies the object for the opcodes
0x56/0x57/0x58/0xe6 into a scratch buffer, then:

1. Perform one placement via real input / the Lua item use.
2. Read the captured buffer: opcode, then the fields in the layouts of `packets.md`.
3. Compare with the Rust serialized bytes (aa-net writer) and with a Rust `Packet` struct built by
   the ported function.

Golden cases: farmhouse (design 171, two clicks + quote + build), scarecrow garden (267), one
decoration on a farmhouse, one seed on a public farm. Capture the request and the server reply for
the refusal paths (no tax certificates) to prove that `BUILDER_END` is not fired on refusal.

## 3. Snapshot/restore and tolerances

* Builder snapshot 0x220 (house) / 0x240-0x254 (others) / 0x54 (deco/seed) + ghost 0x3b0 + unit
  0x400; restore with `write` before each case so cases cannot poison each other. Cursor/mouse
  cooldown globals `DAT_39adcc24`, `DAT_3a17a950`, `DAT_3a167ea0` are reset to 0 between cases.
* Ints/flags/error codes: exact.
* Floats: 1e-6 relative, or on the wire the exact i64 (`coord_to_long`) and f32 bits.
* Positions after packet conversion: exact i64; z f32 exact.
* Quaternions: 1e-5 per component, sign-insensitive.
* A mismatch is a port bug until proven otherwise; log
  `(function, inputs, original, rust, delta)` to `data/extract/port-housing/` (git-ignored).

## 4. Minimal call sequences that establish the unknown list

| unknown | call/measure |
|---|---|
| U1 wheel axis per notch | one `client_wheel +1`, then `FUN_39424580(0x5a)` and `FUN_39016fe0` in the same frame |
| U2 initial yaw | start a placement, snapshot the unit's view quat before any Rotate; compare with design/first SetPos |
| U3 dot spacing | read the terrain object's `vtbl+0x20c` and the plot vector count over a known 24 m plot |
| U5 `housings` desc offsets | read the loaded desc object for a known design (id/category/garden/limits) and match DB columns |
| U6 deco packet sources | capture 0x58 for one decoration placement on a known house |
| U9 debug gate | set/toggle the object at `0x3a167e84` and watch whether dots appear |
