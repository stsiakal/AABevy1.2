# Move packets: `CSMoveUnit` / `SCUnitMovement` (1.2)

The 1.2 client and server share one `0x130`-byte **move struct**. Opcode `0x89` is `CSMoveUnit`
(send) and `SCUnitMovement` (receive); single and list variants differ only by the outer shell
(`FUN_397c52f0`, `FUN_397c5e70`, `FUN_397c5f60`). Evidence: the serializer `FUN_397c0470`
(3,410 B), the copy `FUN_39024610` (270 B, copies exactly 0x130), the list handler
`FUN_391e55c0` (entries at `+0x18`, stride `0x130`, count at `+0xc`), and the send pump
`FUN_394045f0` (move struct at `[esp-0x12c]`, opcode `0x89` at `[esp-0x138]`).

All offsets below are relative to the **move struct** `S`.

## 1. Common layout

Verified from the disassembly of `FUN_397c0470` (the call site `lea ecx,[esi+8]` at 0x397c0524
gives the position out-pointer; the velocity store `movq [esi+0x30]` at 0x397c05f6; the rotation
`lea edi,[esi+0x20]` at 0x397c05f3):

| off | type | field | writer/reader |
|---|---|---|---|
| +0x00 | u8 | `type` (0..5) | `FUN_397c0470`: `"type"` (`0x3999d00c`), u8 |
| +0x04 | u32 | `time` | `"time"` (`0x3999de00`), u32 |
| +0x08 | i64 | position x (64-bit fixed) | `FUN_3961e2c0` (see §2) |
| +0x10 | i64 | position y (64-bit fixed) | `FUN_3961e2c0` |
| +0x18 | f32 | position z (world metres) | `FUN_3961e2c0` |
| +0x20 | f32×4 | rotation quaternion (x, y, z, w) | `FUN_397adc00` read / `FUN_397ae020` write (§3) |
| +0x30 | f32×3 | velocity | i16 codec (§4) |
| +0x38 | — | (part of the velocity tail / padding) | |
| +0x3c | u8 | `flags` | `"flags"` (`0x399a3b80`), u8 |
| +0x40 | u16 | `scType` (if flags & 0x10) | `"scType"` (`0x39a12c08`) |
| +0x44 | u8 | `phase` (if flags & 0x10) | `"phase"` (`0x39a12c00`) |

The 20-byte position at +0x08 is the engine's **64-bit fixed-position delta type** (2×i64 + f32).
It is the same type `FUN_3961de30` converts to metres (`× 2^-44`, `_DAT_399f9b80` =
5.684341886080802e-14) and `FUN_3961dd70`/`FUN_3961ddd0` add to the entity transform. The move's
fixed lives at +0x08..+0x1b (5 dwords); `FUN_397c0470` calls `FUN_3961e2c0` with `this = S+8`.

Type-specific fields: +0x98 (type 2), +0xf4 (type 3), +0x108 (type 4), +0x108..+0x124 (type 5);
type 1 additionally uses +0x48..+0x50 (delta movement), +0x54 (fall velocity), +0x58..+0x60
(stance/alertness/flags/part), +0x80 (second rotation), +0x90 (climb data). All verified from
the type-1 disassembly (`movq [esi+0x30]`, `mov [esi+0x38]`, `[esi+0x48..0x50]`,
`[esi+0x58]`, `[esi+0x5a]`, `[esi+0x5c]`, `[esi+0x60]`).

`FUN_39024610` copies the whole struct field-by-field up to +0x124; the in-memory size used by
the queue is **0x130** (see the list stride note in §10). Do not change the 0x130 slot size.

## 2. Position codec (`FUN_3961e2c0`, 972 B)

Wire fields (9 bytes, in this order; keys from `.rdata` 0x399f9b88..0x399f9ba8):

| key | bytes | meaning |
|---|---|---|
| `ix0` | 1 | x magnitude bits 0..7 |
| `ix1` | 1 | x magnitude bits 8..15 |
| `ix2` | 1 | x magnitude bits 16..23 |
| `iy0`, `iy1`, `iy2` | 3 | y magnitude, same |
| `iz0`, `iz1` | 2 | z quantized low 16 bits |
| `iz2` | 1 | bit7 = x sign (1 = negative), bit6 = y sign, bits 0..5 = z bits 16..21 |

Decode [R: `FUN_3961e2c0` load path 0x3961e4e5..0x3961e689, encoder 0x3961e2fd..0x3961e4da]:

```
x_fixed = (x_sign ? -1 : +1) * (u24(ix0,ix1,ix2) * 8)          // i64, unit = the move fixed unit
y_fixed = (y_sign ? -1 : +1) * (u24(iy0,iy1,iy2) * 8)
zq      = (u24(iz0,iz1,iz2) & 0x3fffff)
z       = zq * (1/4194304) * cell_size - 100.0                 // 0x399f9b84 * 0x3a5e1100, 0x3999d538
```

Encode:

```
xq = |x_fixed| >> 3              // arithmetic shift with sign handling, see 0x3961e2fd..0x3961e3ea
yq = |y_fixed| >> 3
zq = round_half_up((clamp(z, -100, 4096) + 100) / cell_size * 4194304)   // 0x399f9bac = 2^22
iz2 = (x_neg << 7) | (y_neg << 6) | ((zq >> 16) & 0x3f)
```

`0x3a5e1100` is the **cell size** runtime global (default `DAT_399f9bb8` = 4196.0);
`0x399f9bb0` = 4096.0 and `0x399f9bb4` = -100.0 are the z clamps. Use 4196.0 for a static port
and keep the cell size configurable. **Open (U13)**: the move-fixed unit is not visible in this
function (the wire step is `8 * fixed_unit`); measure it once by hooking the sender or by calling
the codec with a known entity position. Do not re-derive from the 10.x code.

Port the encoder/decoder exactly from the decompile; the sign handling and the `+0.5` floor are
part of the wire contract (the difftest compares the 9 bytes).

## 3. Rotation codec (`FUN_397adc00` read, `FUN_397ae020` write)

Wire: three i16 `rot.x`, `rot.y`, `rot.z` (writer `archive+0x50` = i16 ×3).

```
read:  q.xyz = i16 / 32767.0
       d = x*x + y*y + z*z
       q.w   = d <= 0.9975000023841858 ? sqrt(1 - d) : 0
write: normalise q; if (normalised w < 0) negate x,y,z; wire = (i16)(component * 32767 + 0.5)
```

Constants: `0x399abe78` = 1/32767, `0x39a12660` = 0.9975, `0x3999c478` = 1.5,
`0x3999c47c` = 0.5, `0x3999c488` = 1.0 [R]. The writer's rounding is the MSVC `floor(x+0.5)`
sequence (`DAT_39a12670/78` = 0 in the dump only because they are the sign-mask temporaries;
the visible constants are 0.5 and the `0x80000000` sign mask) — port the decompiled sequence,
not a "banker's rounding" guess.

## 4. Velocity codecs

Each type stores a 3-vector as i16 with a type-specific scale. The reader computes
`world = i16 * (1/32767) * scale`; the writer computes the inverse.

| type | codec | scale constant | value |
|---|---|---|---|
| 0 | `FUN_397ada60` | `0x3999ddc8` | 50.0 |
| 1 | `FUN_397ad5a0` | `0x3999d01c` | 60.0 |
| 2, 3, 5 | `FUN_397ad8c0` | `0x3999f7c0` | **30.0** |
| 1 (delta movement) | `FUN_397ad740` (u8 ×3) | `0x399a28dc` | 1/127 |

Field keys: `vel.x`, `vel.y`, `vel.z`; type 1 also `actor.deltaMovement.x/y/z` (u8),
`actor.fallVel` (u16, scale `0x39a13764` = 1/128 and `0x399c8c1c` = 128), `actor.stance`,
`actor.alertness`, `actor.flags`, `actor.gcFlags`, `actor.gcPartId`, `actor.gcId`,
`actor.climbData` [R].

## 5. Type 2 — land vehicle

Serialization order after the header [R: `FUN_397c0470` case 2]:

| off | type | field | key |
|---|---|---|---|
| +0x08 | fixed | position delta | header codec (§2) |
| +0x20 | f32×4 | rotation quat | `rot.x/y/z` (w reconstructed) |
| +0x30 | f32×3 (i16×3 on wire) | velocity | `vel.x/y/z`, scale 30.0 |
| +0x98 | f32×3 | angular velocity | `vehicle.angVel` |
| +0xa4 | f32 | steering (-1..1) | `vehicle.steering` |
| +0xa8 | u8 | wheel count `n` | `vehicle.wheelVelCount` |
| +0xac | f32×n | wheel angular speeds | `vehicle.wheelAngVel` |

Note the 1.2 vehicle body has **no throttle i8** (that is 10.x only; type 2 there is a different
numbering). `n` is written from the model's actual wheel-bone count (wagon = 4: two `wheelN`,
two `wheel_type2N`; cars = 4), not from `vehicle_models.wheel` (which is a cgf path) [R:
`FUN_390e7b70`, `FUN_390d6b10`].

## 6. Type 3 — ship state (server -> client)

| off | type | field | key |
|---|---|---|---|
| +0x08 | fixed | position delta | header codec (§2) |
| +0x20 | f32×4 | rotation quat | `rot.x/y/z` |
| +0x30 | f32×3 (i16×3 on wire) | velocity | `vel.x/y/z`, scale 30.0 |
| +0xf4 | f32×3 | angular velocity | `ship.angVel` |
| +0x100 | i8 | steering | `ship.steering` |
| +0x101 | i8 | throttle | `ship.throttle` |
| +0x102 | u16 | zone id | `ship.zoneId` |
| +0x104 | bool | stuck | `ship.stucked` |

The client applies it through `OnOneUnitMovement` -> `GameClient->vtable[0x40]` -> the
NetActorMovementController (see MAP §3.2). The visual fields are the model's `+0x35c` (steering)
and `+0x358` (throttle), written by `FUN_390be0b0`/`FUN_390be0d0`.

## 7. Type 4 — ship helm request (client -> server)

| off | type | field | key |
|---|---|---|---|
| +0x108 | i8 | throttle | `shipRequest.throttle` |
| +0x109 | i8 | steering | `shipRequest.steering` |

Only two bytes after the header. Sent only by the driver; the server answers with type 3. The
input producer is `GameClient->vtable[0x60]` (MAP U3).

## 8. Type 5 — transfer

| off | type | field | key | codec |
|---|---|---|---|---|
| +0x08 | fixed | position delta | header codec (§2) |
| +0x20 | f32×4 | rotation quat | `rot.x/y/z` | quat codec |
| +0x30 | f32×3 (i16×3 on wire) | velocity | `vel.x/y/z` | 30.0 |
| +0x10c | f32×3 | angular velocity | `transfer.angVel` | f32 |
| +0x118 | i32 | steering | `transfer.steering` | i32 |
| +0x11c | i32 | path point index | `transfer.pathPointIndex` | i32 |
| +0x120 | f32 | speed | `transfer.speed` | f32 |
| +0x124 | bool | reverse | `transfer.reverse` | bool |

The client never sends type 5; it only applies it (and `FUN_39350160` pushes a copy into a
100-entry history for the TransferManager).

## 9. Type 1 — on foot (for completeness; F1)

Header (position +0x08, quat +0x20) + `vel.x/y/z` (i16, scale 60) + `actor.deltaMovement.x/y/z`
(u8, 1/127, floats at +0x48..+0x50) + `actor.stance` (+0x58), `actor.alertness` (+0x59),
`actor.flags` (+0x5a), optional `actor.fallVel` (u16 0..65535 <-> float 0..128 at +0x54; `0x39a13764` = 1/128, `0x399c8c1c` = 128),
`actor.gcFlags` (+0x5b), `actor.gcPartId` (+0x5c) + a second rotation (+0x80), `actor.gcId`
(+0x60), `actor.climbData` (+0x90) [R: case 1 disassembly 0x397c0524..0x397c0xxx]. Not ported
here; listed so the shared serializer test covers it.

## 10. Outer shells

* Single: `FUN_397c52f0` (93 B) — move struct at packet **+0x10**, optional `unitId` 3 bytes at
  packet **+0x0c** (`lea edi,[ebx+0xc]`, `lea ecx,[ebx+0x10]`), then `FUN_397c0470`.
* List: `FUN_397c5e70` (233 B) — `count` u16 at +0x0c (cap 400); per entry `i`: `unitId` 3 bytes
  at `packet+0x10+i*0x130`, move struct at `packet+0x18+i*0x130` (`lea edi,[ebx+0x10]`,
  `lea ecx,[ebx+0x18]`, `imul ebx,ebx,0x130`). The serializer writes the move up to +0x124, i.e.
  0x13c bytes from the entry start — **the 0x130 stride overlaps the next entry's first 0x0c
  bytes** for the last fields. This is the shipped layout; treat it as U14 and verify the copy
  extents with a captured multi-move list before relying on >2 entries of type 5. For types
  0..4 the used size (≤0xa8 + wheels) fits.
* Parse-side constructor `FUN_391e3660`/`391e36f0` (131/108 B) allocate and call `0x397c5e70`;
  `FUN_391cea00`/`391cea80` (125/95 B) do the same for the single shell.

The send path uses a stack buffer:
`[esp-0x13c]` = packet object/vtable, `[esp-0x138]` = opcode `0x89` (u32),
`[esp-0x130]` = unit id, `[esp-0x12c]` = move struct (0x130). The send thunk is
`0x3ac1058c` (via `thunk_FUN_3ac1058c`). The difftest hook compares the bytes at
`[esp-0x12c]` with the Rust builder.

## 11. Cadence

`FUN_394045f0` (vehicle/mount/ship) and `FUN_394046f0` (on foot):

```
timer += dt                                  // per-unit field at this+4
if (now_ms * 0.001 >= timer) {
    if (state changed || now_ms * 0.001 >= min_interval) send and timer = 0
} else if (state changed) send and timer = 0   // actually: change forces a send only after ? see code
```

The exact guard is in the decompile at 0x394049xx (on foot) / 0x394045f0 (vehicle); the port must
copy the boolean expression verbatim. The two interval objects are runtime (`DAT_3a59f84c` =
max, `DAT_3a59f850` = min); U2.
