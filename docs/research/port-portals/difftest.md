# Differential tests: portal / teleport / return-point / navigation

Goal: prove each ported Rust function matches the original 1.2 client by calling the **original**
function inside the hidden client (`aadriver.dll`) with chosen inputs and comparing with the Rust
port. The driver's `call` runs on the client main thread and supports `cdecl`, `stdcall`,
`thiscall` and `fastcall`, pointer arguments in tracked `alloc` buffers, and post-call `dump=`
buffers (`C:/Users/stsiakal/AAEmu/rust/docs/client/LUA-DRIVER.md` §13).

This file assumes the plumbing of [`../port-movement/difftest.md`](../port-movement/difftest.md)
§1/§4 and the packet hook `0x3AC1058C` from [`../port-combat/difftest.md`](../port-combat/difftest.md)
§1.

No original code is shipped; the driver only calls the shipped binary in memory.

## 0. Addressing and state

All x2game VAs here are at the preferred base `0x38FF0000`. The driver takes
`x2game.dll+<rva>` (it relocates) — **never** a hard-coded VA:

| name | VA | RVA |
|---|---|---|
| `GameClient` global (`DAT_39B02294`) | `0x39B02294` | `0xB12294` |
| `ClientCharacter::Update` | `0x3930E7E0` | `0x31E7E0` |
| auto-use portal | `0x3930CFF0` | `0x31CFF0` |
| row builder (`GetPortalList` body) | `0x3930CB90` | `0x31CB90` |
| delete private / district | `0x3930F620` / `0x3930F6B0` | `0x31F620` / `0x31F6B0` |
| append district (`SCCharacterBound`) | `0x3930FD80` | `0x31FD80` |
| 10 s portal reuse gate | `0x391827F0` | `0x1927F0` |
| `SavePortal` request | `0x3938F7D0` | `0x39F7D0` |
| `OpenPortal` request | `0x3938FAA0` | `0x39FAA0` |
| `CSUsePortal` body | `0x397AC7B0` | `0x7BC7B0` |
| `CSUseTeleport` body | `0x397B0FB0` | `0x7C0FB0` |
| item 4045 lookup | `0x394E0960` / `0x397CD0A0` | `0x4F0960` / `0x7DD0A0` |
| item-use dispatcher | `0x394EAAC0` | `0x4FAAC0` |
| `OnCharacterBound` (0x049) | `0x391D4240` | `0x1E4240` |
| navi mark event | `0x393B0C10` | `0x3C0C10` |
| navi naming event | `0x393B0FC0` | `0x3C0FC0` |
| death countdown (reuse) | `0x39495C00` | `0x4A5C00` |

```sh
T=<build>/aaemu-clientdriver.exe
$T mod x2game.dll                       # actual base + size
GC=$($T peek-global x2game.dll+b12294 ptr)   # GameClient*
CC=$(printf '0x%x' $((GC + 0x40)))           # ClientCharacter*
U=$(printf '0x%x' $((GC + 0x10c)))           # local unit*
```

Use `read`/`write --unsafe` for live fields, but prefer a **scratch object** for the book state
functions: they only read/write the offsets listed in [`structs.md`](structs.md) §1, so a
`alloc 0xB30` zeroed block with forged `+0xB00..+0xB18` and `+0x608` is a safe oracle. Always
restore any live bytes you touch (read them first; write them back at the end), and never leave a
forged vector in the live character across frames.

## 1. Pure leaves (synthetic buffers, no live state)

### 1.1 Body setters

For each function, `alloc 0x40`, fill with `0xCC`, write sentinels, call, and `dump`:

| function | convention | inputs |
|---|---|---|
| `0x397AC7B0` | `__thiscall` (`this=buf`) | `u32 id`, `u8 flag` |
| `0x397AC7D0` | `__thiscall` (`this=buf`) | `u32 type`, `u32 id` |
| `0x397AC8A0/B0/C0` | `__thiscall` (`this=buf`) | `u32 doodadId` |
| `0x397B0FB0` | `__thiscall` (`this=buf`) | `u32 returnPointId`, `u32 money`, `u32 npcUnitId` |

Expected: only the documented payload words change; all other bytes stay `0xCC`. This proves the
struct offsets before any send-path test.

```sh
A=$($T alloc 64); $T write $A $(python -c 'print("cc"*64)')
$T call x2game.dll+7bc7b0 thiscall this=$A u32:0x12345678 u32:1 dump=$A:32
```

### 1.2 Count and getter leaves

| function | convention | this | expected |
|---|---|---|---|
| `0x3930B410` | `__fastcall` (`this` in ECX) | scratch + fake vector | `(end-begin)/0x9C` |
| `0x3930B480` | `__fastcall` | scratch | same for `+0xB10` |
| `0x3938FD20` | `__fastcall` no args | — | `GameClient+0x40` → `+0x608` |
| `0x39424820` | `__cdecl` | — | unsigned int → hex ASCII, `0x11` max |

For the counts, set `+0xB00=0x1000`, `+0xB04=0x1000+3*0x9C` (and `+0xB10` analogously) in a
scratch block; compare 3, then 0, 1 and a 100-row value (the 0x05C packet cap).

### 1.3 Portal row builder (`FUN_3930CB90`)

This function needs a Lua table argument, so use the live Lua path:

1. Save the live `ClientCharacter+0xB00/+0xB04/+0xB08` (private vector) and `+0xB10/+0xB14/+0xB18`.
2. `alloc 0x9C`, write a crafted `PortalEntry` (id 0x11223344, name `"diff"`, zone 2, x/y/z
   `1.5/-2.5/3.5`, zRot 90).
3. Point `+0xB00`/`+0xB04` at it, `+0xB08 = +0xB04` (do **not** let the client modify it).
4. `$T lua -t ui 'return X2Warp:GetPortalListCount(), X2Warp:GetPortalList()'`.
5. Compare the returned row with the Rust `entry -> row` function; repeat for the return list with
   `X2Warp:GetReturnList()` (type string must be `"return"`), and for an unknown zone (zone id 0 /
   missing) to exercise the null `zone_name`/`world_name` branches.
6. Restore the three words, `free` the entry.

This tests `portal_index` (1-based), `portal_id`, `portal_type`, `zone_id`, `zone_name`,
`world_name`, `name`, `x/y/z`. The zone/world names are looked up through
`FUN_395779F0`/`FUN_397F2180`/`FUN_397F23D0`; use a known zone id (e.g. the current zone) for a
non-null result.

## 2. Book state machine (scratch object)

### 2.1 Append / replace / add / delete

Scratch `alloc 0xB30`; set a real `alloc 0x9C*N` row array for the vector. Call:

| test | function | this=scratch | input |
|---|---|---|---|
| append 3 private rows | `0x3930FA50` | `scratch` | `count=3`, `rows` pointer |
| replace 2 district rows + bound 7 | `0x3930FD40` | `scratch` | `count=2`, rows, `bound=7` |
| add a private row | `0x3930FC80` | `scratch` | row pointer (id new) |
| rename the same row | `0x3930FC80` | `scratch` | same id, new name |
| delete a private row | `0x3930F620` | `scratch` | existing id |
| delete a district row | `0x3930F6B0` | `scratch` | non-bound id |
| delete the bound district row | `0x3930F6B0` | `scratch` | `id == +0x608`; expect 0, error 0x19B, **no** event 0xE6 |
| append a district row | `0x3930FD80` | `scratch` | row pointer |

After each call dump `+0xB00/+0xB04/+0xB10/+0xB14/+0x608` and compare the Rust `Vec<PortalEntry>`
contents (not the allocator addresses). The event side can be observed from the UI Lua: register a
handler for `SAVE_PORTAL`/`DELETE_PORTAL`/`RENAME_PORTAL` and count fires.

### 2.2 `SCCharacterBound` 0x049

Call the real handler with a synthetic packet object: `alloc 0xC0`, set `+0xC` unitId,
`+0x10` returnDistrict, `+0x14` resurrectionDistrict, `+0x18` changed (0/1), `+0x1C` a crafted
`PortalEntry`; call `0x391D4240` `__thiscall this=GameClient`, `pkt`. Compare against a scratch
ClientCharacter? The handler reads the live character (`GameClient+0x40`) and writes it, so save and
restore `+0x608/+0x60C/+0xB10..+0xB18`; use a **non-bound** district id so the test does not leave
a real binding. Check:
* `+0x60C == packet+0x14`
* `+0x608 == entry.id` iff changed
* the district vector gains the row
* `SYSMSG` fires with text id 0x60 when changed.

## 3. Requests (`X2Warp`) and packets

For every request, the oracle is the **packet stream**: hook `0x3AC1058C` (see
`port-combat/difftest.md` §1) and copy the buffer, or read the lane server's log. Call the Lua
method in the real UI state, then compare opcode + payload with the Rust encoder.

| Lua call | expected packet | checks |
|---|---|---|
| `X2Warp:SavePortal("diff")` | `CSStartSkill` with object type 2, id 0, name `"diff"` | item 4045 present; missing -> error 0x240, no packet |
| `X2Warp:RenamePortal(id,"diff2")` | `CSStartSkill` type 2, id, name | id 0 / missing item -> no packet |
| `X2Warp:OpenPortal("portal", id)` | `CSStartSkill` type 1, type 2, id | `"return"` -> type 1; unknown string -> only the log line `Unknown portal type = %s` |
| `X2Warp:DeletePortal("return", boundId)` | none | error 0x19B, vector unchanged |
| `X2Warp:DeletePortal("portal", id)` | `CSDeletePortal` 0x0DF | local row removed first; capture the body and settle U-A3 |
| `X2Warp:UseTeleport(id, 0)` (target a teleporter NPC) | `CSUseTeleport` 0x0B3 | capture `(id, cost, unitId)`; no NPC in 1.2 data has the flag |

Also call the low-level builders directly with a scratch object and compare the payload bytes:
`FUN_3938F7D0`/`F950`/`FAA0` need a live local player (item lookup), while the pure setters in §1.1
cover the body offsets.

**U-A1 test.** Hook `X2Warp:OpenPortal("portal", id)`, dump the `CSStartSkill` buffer, and check
whether (x,y,z) are zero. If the server accepts zeros, the port must send zeros; if not, find the
code that fills them (or the server's fallback).

**U-A2/U-A3 test.** Capture a real `CSUsePortal` (walk into a gate, §4) and a real `CSDeletePortal`
(GM `/add_portal`, then delete the row). Dump all bytes; compare with the reader in the lane server.
This settles whether `type`/`role` are extra fields or a direction-specific class.

## 4. Auto-use portal (`FUN_3930CFF0`)

Setup: stand the live player within 3 m of a spawned portal gate (server NPC 3891 or 5476), no
resurrection buff, `GameClient+0x109A == 0`, `+0x109C` older than 10 s. Call:

```sh
CC=$($T peek-global x2game.dll+b12294 ptr); CC=$((CC+0x40))
$T call x2game.dll+31cff0 fastcall this=$CC
```

Observe the `CSUsePortal` on the packet hook / server log. Matrix:

| input | expected |
|---|---|
| gate at 2.9 m / 3.1 m | packet / no packet (settle the range U-B2) |
| gate at 3.01 m + per-unit var 6 = 1 | packet (if var 6 bypasses the movement requirement) |
| `cc+0xB20 = now-500` | no packet (1 s throttle) |
| `GameClient+0x109A = 1` | no packet |
| `GameClient+0x109C = now-5000` | error 0x319, no packet |
| buff 1128 or 1910 present | error 0x165, no packet |
| NPC 6629 / 6949 in range | no packet |
| NPC 3891 vs 5476 | packet in both |

Then repeat with the Rust decision function fed the same captured unit fields and compare the
action (packet/no packet/error id). Restore all fields after each row.

`FUN_3930B5B0` (auto-use precondition) can be tested directly: scratch unit with `+0x3BE8` pointing
to a mover whose `+0x54` is 0.0/0.19/0.21, plus the per-unit map entry 6 set/unset. Compare the
boolean.

`FUN_391827F0` (10 s gate) is a leaf on `GameClient+0x109C`: call with `now` values
`last+9999/10000/10001` and check EAX (1 = allowed, 0 = refused) and the error call (`0x319`).

## 5. Unit presentation / portal-used effect

* `FUN_394C1CA0`: call on a live portal unit (or a scratch unit with `+0x410=1`,
  `+0x41C=3891/5476/6629`, `+0x428` name) and inspect the Lua tooltip table: `is_portal` true,
  `portal_owner` copied, `kind`/`grade` before it. Test 6949 -> no `is_portal`.
* `FUN_394C0680`: type string `"portal"` for the same set.
* `FUN_393558E0`: call on a live unit and verify effect 57 spawns at the unit transform for 5 s
  (effect handle returned by `FUN_393F1160`; compare the effect id/duration, not the pointer).

## 6. Teleport transition

* Same-zone: cause a portal use / GM move so the server sends `SCTeleportUnit`. Hook
  `FUN_3930C9E0` (or the `0x391D6ED0` handler) and compare the flags
  `GameClient+0x109A/+0x109C/+0x10A0/+0x10A4` and the `CSTeleportEnded` send. The movement pose is
  already covered by `port-movement-extra` §7.4; do not duplicate it.
* Other-zone: enter an instance (or use Recall) so the server sends `SCLoadInstance`. Call the
  handler `0x391DE970` with a synthetic 0x2C packet (`iid`, `zoneId`, pos, ang) and check:
  `GameClient+0x110 == zoneId`, the old-zone units are removed, `CSInstanceLoaded` 0x0E0 is sent,
  and `CSNotifySubZone` 0x112 is sent when the sub-zone changed.
* Loading art: call `FUN_3942F770` with the current world set to `main_world` and to an instance
  world and compare the selected texture (`ui/loading.dds` vs
  `worlds/<world>/loading.dds`) and the `ENTERED_LOADING` 0x4D event. Then call
  `FUN_39187410(0, progress)` for progress 0/0.5/1 and check when the art call happens and what
  `STILL_LOADING` 0x45 carries.
* `OnAboxTeleport` (0x391DC5A0): craft a packet body with (x,y) ints and compare the written
  position: `camera/axis * -1024`, `z = waterOrGround + 10.0`. This is a live-world test (the
  height query needs terrain).

## 7. Return point and resurrection

The death/resurrection half is already implemented from `port-combat`; only re-run its oracle
suite. New here:

* `SCCharacterBound` 0x049: §2.2 above.
* `GetReturnList`: with a live visited-district vector, compare the Lua table with the Rust rows.
* `GetBoundPortalId`: set `cc+0x608` to 0, 1, 0xFFFFFFFF and compare the Lua number (0 is returned
  as 0, not nil).
* If desired, call `FUN_39495C00` through `X2Player:GetResurrectionCountdown()` with crafted
  `+0xAF4/+0xAF8/+0xAFC` and compare the three returned floats (U-C1). This is a cross-check of
  the existing port, not a new port.

## 8. Navigation

* **Map marker 0x127.** Register a Lua handler for `NAVI_MARK_POS_TO_MAP`, use a Portable Sign
  (doodad with `DoodadFuncNaviMarkPosToMap`), call `FUN_393B0C10` on its doodad object, and compare
  the four event args with the Rust `(markerIndex, x, y, z)`. Vary the marker index
  (`DAT_39AC7770`) and a zero position (the code falls through to the quest-mark branch).
* **Naming dialog 0x129.** Call `FUN_393B0FC0` on a navi doodad and check the single arg is the
  doodad `+0x7C` id.
* **Mailbox 0xCE.** Call `FUN_393B1040` and check `(1, doodadId)` and that the interaction is
  ended (`DAT_3A17E6A0`).
* **CS 0x0E7/0x0E8.** `FUN_393B1250/12B0` only send when `player+0x3AC4/3AC8` equal
  `DAT_3A17D7A8/A4`; set both pairs equal and unequal and capture the packet. Compare the id with
  `doodad+0x7C`.
* **OnNaviTeleport.** Feed a synthetic 0x0E7 packet (`hasPing`, pos, insId) and check
  `FUN_3930C9E0(0,0,pos,0)` (same pose/camera checks as `port-movement-extra` §7.4).
* **Quest marks.** `ClientUnit::UpdateQuestMark` is stateful and visual: use a quest with a known
  mark, dump the unit `+0x5FA4/+0x5FA8/+0x5FB0` and the spawned prefab path, then compare the Rust
  mark selection for the same quest context. Prefab paths are in `0x399D13C8…` (marker.*).

## 9. Test hygiene

* The client is shared with live sessions: never leave a forged pointer in `GameClient` or
  `ClientCharacter`, restore saved bytes even after a failed call, and `free` every `alloc`.
* Do not call functions that block, wait for the renderer/network, or take a lock the caller holds
  (`LUA-DRIVER.md` §13).
* `call` is SEH-wrapped; an access violation is a driver error, not a crash, but logical corruption
  (a bad vector) can still break the client. Use scratch objects for the book state.
* Record the exact client revision/`mod x2game.dll` output with every golden so later runs are
  comparable.
