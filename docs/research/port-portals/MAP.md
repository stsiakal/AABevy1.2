# Port map: the ArcheAge 1.2 portal book, world portals, teleports, return points and navigation

Address map for a function-by-function Rust port of the 1.2 client's portal/teleport/return-point
system (programme S7, `remaining.md` H5), with a differential-test plan that calls the original
functions inside the running hidden 1.2 client (`aadriver.dll`).

## 0. Sources, conventions and licensing

### 0.1 Licensing

Everything here is derived **only** from the shipped 1.2 binaries and their decompiles. The
published CRYENGINE source was not used, quoted or consulted (same rule as
`aaemu-physics/CRYPHYSICS-MAP.md`, `port-movement/MAP.md`, `port-combat/MAP.md`). Class and
function names in `CClass` / `X2*` form come from the binaries' own RTTI, profiler/log/assert
strings, cvar and Lua-binding strings, or the 10.x builds; behaviour with no name in the binary is
marked *(inferred)*. Port from this document and the decompiles, never from the CRYENGINE source.

### 0.2 Sources

| source | path |
|---|---|
| 1.2 x2game decompile | `C:/AAEMU_DISK/aa-re/client12/out-x2game/x2game.dll.c` |
| 1.2 x2game functions/calls/strings | `C:/AAEMU_DISK/aa-re/client12/out-x2game/{funcs.csv,calls.tsv,strings-xrefs.tsv}` |
| 1.2 x2game image + `.data` (constants, tables, vtables) | `D:/aa-re/client12/x2game_unpacked_data.dll`, base `0x38FF0000` |
| 1.2 engine decompiles | `D:/aa-re/client12/engine/out/`, `D:/aa-re/client12/engine2/out/` |
| 1.2 client database (read-only) | `D:/aaemu-live/rs-game/Data/compact.sqlite3` (items, npcs, zones, doodad funcs) |
| 10.x names/structure only | `C:/Users/stsiakal/AAEmu/REVERSING.md`, `D:/aa-re/zone/out/` |
| UI scripts (Lua; via prior research) | `docs/research/portals.md` §3 and its cited `data/extract/aa-lua/decompiled/x2ui/portal/` |
| existing notes | `docs/research/portals.md` (gameplay/server/UI), `re-movement.md`, `re-combat.md` §2.9, `port-movement-extra/`, `port-combat/` |
| driver | `C:/Users/stsiakal/AAEmu/rust/docs/client/LUA-DRIVER.md` §13 (in-process `call`) |

x2game addresses are VAs at the preferred base `0x38FF0000`; RVA = VA − `0x38FF0000`. The driver
must be given `x2game.dll+<rva>` because the module relocates. Evidence markers: **[R]** read from
the binary/decompile/DB, **[I]** inference, **[?]** open.

### 0.3 Layout of this folder

| file | content |
|---|---|
| `MAP.md` (this) | call chains, behaviour rules, port order, unknowns |
| [`functions.md`](functions.md) | every function in scope: address, size, callers/callees, fields, purity |
| [`packets.md`](packets.md) | opcodes, handlers, readers/writers, field offsets |
| [`structs.md`](structs.md) | the Rust struct layouts and offsets |
| [`difftest.md`](difftest.md) | in-process differential recipes per function |

### 0.4 Scope size

[`functions.md`](functions.md) documents **102 new function entries, 41,238 bytes** (all sizes from
`funcs.csv`) plus **16 reused entries, 9,245 bytes** that are already mapped elsewhere and are only
cross-referenced. Total touched surface: **118 entries / 50,483 bytes**.

Out of scope (already mapped, do not re-map): `FUN_3930c9e0` and the teleport movement/camera reset
(`port-movement-extra/click-move-jump-teleport.md` §6); the death window, `SCNotifyResurrection`,
`SCCharacterResurrected`, `CSResurrectCharacter`, `X2Player:GetResurrectionCountdown` and their
Rust implementation (`port-combat/MAP.md`, `re-combat.md` §2.9). Those are listed in the "reused"
table below only to keep the packet flow readable.

---

## 1. The call chains

### 1.1 A — the Teleport Book (portal book)

```
S→C 0x05B SCCharacterPortals (append)
  handler 0x391d3760 OnCharacterPortals
    └─ FUN_3930fa50(count, rows)      append count × 0x9C rows to ClientCharacter+0xB00
S→C 0x05C SCCharacterReturnDistricts (replace + bound id)
  handler 0x391d37e0
    └─ FUN_3930fd40(count, rows, boundId)
         ├─ FUN_3930f880             clear + insert rows into ClientCharacter+0xB10
         └─ [ClientCharacter+0x608] = boundId
S→C 0x05D SCPortalInfoSaved (add or rename one private portal)
  handler 0x391d3860
    └─ FUN_3930fc80(row)            update in +0xB00 else append; event 0xE5 (add) / 0xE7 (rename)
S→C 0x05E SCPortalDeleted
  handler 0x391d38e0
    ├─ type == 2 → FUN_3930f620(id) private list (+0xB00), event 0xE6
    └─ type == 1 → FUN_3930f6b0(id) district list (+0xB10); refuses the bound id
S→C 0x049 SCCharacterBound (Memory Tome binding)
  handler 0x391d4240 OnCharacterBound
    ├─ ClientCharacter+0x60C = resurrectionDistrict
    ├─ FUN_3930fd80(row)           add-or-update the return district in +0xB10
    └─ if changed: ClientCharacter+0x608 = row.id; SYSMSG text 0x60 "return_district_binding"
S→C 0x060 SCUnitPortalUsed
  handler 0x391d3970 → FUN_393558e0  play effect 57 for 5 s on the gate unit
```

Lua side (`X2Warp`, all native implementations in `0x394caxxx`, registration `0x394cb5e0`):

```
X2Warp.GetPortalList()       0x394caff0 -> FUN_3938fc00 -> FUN_3930cfb0 -> FUN_3930cb90
X2Warp.GetReturnList()       0x394cb120 -> FUN_3938fc60 -> FUN_3930cfd0 -> FUN_3930cb90
GetPortalListCount           0x394caf70 -> FUN_3938fbd0 -> FUN_3930b410
GetReturnListCount           0x394cb0a0 -> FUN_3938fc30 -> FUN_3930b480
GetBoundPortalId             0x394cb1d0 -> FUN_3938fd20   ([ClientCharacter+0x608])
X2Warp.SavePortal(name)      0x394cace0 -> XlIsValidName(name,5) -> FUN_3938f7d0
X2Warp.RenamePortal(id,name) 0x394cad10 -> FUN_3938f950
X2Warp.OpenPortal(type,id)   0x394cad40 -> "portal"→2 / "return"→1 -> FUN_3938faa0
X2Warp.DeletePortal(type,id) 0x394cadf0 -> "portal"→2 / "return"→1 -> FUN_3938fc90
X2Warp.IsTeleporterNpc()     0x394caee0 -> target NPC desc +0xE5 (`teleporter`)
X2Warp.UseTeleport(id,cost)  0x394cb4b0 -> CSUseTeleport 0x0B3
X2Warp.EndPortalInteraction  0x394cadd0 -> FUN_393b7b50(6)
```

`FUN_3930cb90` is the row builder: for each 0x9C entry it creates a Lua table with
`portal_index` (1-based), `portal_id`, `portal_type` (the **string** `"portal"` or `"return"`),
`zone_id`, `zone_name`, `world_name`, `name`, `x`, `y`, `z` and appends it to the result table
[`structs.md`](structs.md) §2. The event ids are the ones registered by `FUN_39214380`
(`TOGGLE_PORTAL_DIALOG` 0xE4, `SAVE_PORTAL` 0xE5, `DELETE_PORTAL` 0xE6, `RENAME_PORTAL` 0xE7)
[R, computed from the registration order; cross-checked against the handler behaviour].

**Book trigger.** The item-use dispatcher `FUN_394eaac0` switches on `itemDesc+0x48`; case `0x0E`
(= 14) fires `TOGGLE_PORTAL_DIALOG` (0xE4) when a local player exists [R: the switch and the fire].
The link to the DB field is [I]: item 4045 "Teleport Book" has `impl_id = 14` and 0x48 is the
`impl_id` slot in the loaded item descriptor (verify with a live item-use capture). The dispatcher
is reached from `FUN_394eb270` (UseItem) [R].

### 1.2 A — request prechecks and refusals

| request | precheck | refusal |
|---|---|---|
| `SavePortal(name)` | `XlIsValidName(name,5)` in `FUN_394cace0`; Teleport Book item **4045** (`0xFCD`) must be in the inventory (`FUN_394e0960` inventory + `FUN_397cd0a0`) [R] | invalid name: no request, no message (UI validates); missing book: error **0x240** `no_portal_book` [R] |
| `RenamePortal(id,name)` | item 4045 lookup (`FUN_3938f950`) [R] | silently returns if the book is missing [R] |
| `OpenPortal(typeStr,id)` | type string must compare `"portal"` (→2) or `"return"` (→1); unknown logs `Unknown portal type = %s` [R] | no request |
| `DeletePortal(type,id)` | local removal first: district delete `FUN_3930f6b0` refuses the bound return point (`id == ClientCharacter+0x608`) with error **0x19B** `delete_current_return_district`; unknown type ignored [R] | no request; the Lua also removes the row only on success |
| `UseTeleport(id,cost)` | target must be NPC kind 1 with `teleporter` byte set (`desc+0xE5`); money check [R] | errors via `FUN_39390900`/`FUN_39428370` |

The native `OpenPortal` wrapper parses the three Lua arguments `portalTypeStr, portalId,
portalName` but `FUN_394cad40` only forwards type and id; `portalName` is ignored [R]. The request
is a `CSStartSkill` object with type 1 (PortalInfo); save/rename use type 2 (SavePortalInfo). The
observed stack payload is in [`packets.md`](packets.md) §4.

### 1.3 B — world portals: walking into a gate

`X2::GameClient::ClientCharacter::Update` (**0x3930e7e0**, 636 B) ends with
`FUN_3930cff0` (497 B), the **auto-use portal** pass *(inferred name)* [R: profiler string at the top of 0x3930e7e0
and the tail call at its end]:

```
FUN_3930cff0(ClientCharacter* cc):
  if GameClient+0x109A != 0: return                 // teleport in progress
  if now < cc+0xB20 + 1000: return                  // 1 s throttle
  if local unit == null: return
  if !FUN_3930b5b0(local): return                   // see below
  hasResurrectionBuff = buff(1128) || buff(1910)    // Weakened Body / respawn debuff
  foreach u in *nearby-unit-list (DAT_3a169998):
      if u+0x410 != 1: continue                     // NPC kind
      if u+0x41C != 3891 && u+0x41C != 5476: continue // entrance gates only
      if squaredDist(player, u) > RANGE: continue
      if hasResurrectionBuff: error 0x165; continue
      if !FUN_391827f0(now): continue               // 10 s reuse gate (error 0x319)
      send CSUsePortal { tid = u+0x418, flag = ? }   // target-exists / onlyMyPortal, U-B2
      cc+0xB20 = now; return
```

`FUN_3930b5b0` (77 B) returns true when the implicit object's variable id 6 is non-zero
(`FUN_39424580`/`FUN_393fd240`) **or** the movement speed at `mover+0x54` is > 0.2 [R read, meaning
[I]]. `FUN_391827f0` (0x391827f0) is a 10,000 ms gate against `GameClient+0x109C`, showing error
**0x319** `cannot_reuse_portal_in_delay_time` [R]. The exit NPCs 6629/6949 are **not** auto-used
(only 3891 and 5476), and the client recognises 3891, 5476 and 6629 as portals for the tooltip
(`DAT_39ac9a78/9A7C/9A80`) [R]. 6949 exists in the DB but is not in the client's portal set [R].

`CSUsePortal` is opcode **0x0DE**. The native builder `FUN_397ac7b0(id, flag)` stores id at +0xC and
a byte flag at +0x10; the serializer `FUN_397c2a50` writes `tid` (u32), `type` (u32) and `role`
(u8) [R]. The emulator documents a 2-field `objId, onlyMyPortal` body; the third byte and the exact
meaning of `u+0x418` are open ([`packets.md`](packets.md) U-A2/U-B1).

Unit-side portal presentation [R]:
* `FUN_394c1ca0` builds unit-tooltip fields: `is_portal = true` for NPC templates 3891/5476/6629,
  `portal_owner` from `unit+0x428`; `kind`/`nick_name`/`grade` precede it.
* `FUN_394c0680` reports the unit type string `"portal"` for the same templates.
* `FUN_393558e0` (SCUnitPortalUsed) creates effect **57** (`DAT_39ac7460`) with a **5.0 s**
  duration (`0x3999CCF4`) on the unit, using its current transform; it is the gate-use visual [R].

### 1.4 B — the teleport cast, transition and arrival

The cast itself (Recall 10594, Open Worldgate 11216) is the normal skill pipeline: `CSStartSkill`
0x052 and the cast bar are the generic skill/UI path (`port-combat/`), not portal-specific. The
portal part is the item/`X2Warp` request and the arrival:

```
C→S 0x0B3 CSUseTeleport      X2Warp.UseTeleport -> FUN_394cb4b0 -> builder FUN_397b0fb0
S→C 0x072 SCTeleportUnit     handler 0x391d6ed0
                               ├─ FUN_3930c9e0(type,objId,pos,yaw)   [port-movement-extra §6]
                               └─ flags: GameClient+0x109A=1, +0x10A0=1, +0x109C=now,
                                         +0x10A4 = reason (0xD default)
S→C 0x197 SCLoadInstance     handler 0x391de970 -> FUN_3936f1c0
                               ├─ clears old-zone units (FUN_39367220)
                               ├─ GameClient+0x110 = zoneId; zone/world data load
                               ├─ CSNotifySubZone 0x112 if the sub-zone changed
                               └─ sends CSInstanceLoaded 0x0E0
loading art: FUN_39187410(0, …) -> FUN_3942f770
             worlds/<current world>/loading.dds, else ui/loading.dds; fires ENTERED_LOADING 0x4D
```

**Same-zone vs other-zone decision [R].** It is **packet-driven**, not distance-driven: a bare
`SCTeleportUnit` (portal use, GM move) only runs the movement/camera reset and `FUN_39171800(3)`;
it does **not** call the loading-art function `FUN_3942f770`. An `SCLoadInstance` always takes the
instance path (`FUN_3936f1c0`), which reloads zone data and is the path that reaches the loading
screen art. Recall/worldgate `Return` sends `SCLoadInstance` (even back into `main_world`) followed
by `SCTeleportUnit`, which is why it shows the loading screen (`portals.md` §2.3).

The art is keyed by the **current world folder**: `FUN_3942f770` falls back to
`ui/loading.dds` when the world is `main_world` [R: the `XlCompareString(pcVar4,"main_world")`
branch in 0x3942f770]. `FUN_39187410` calls `FUN_3942f770` only when `mode == 0 && progress < 1`,
then fires `STILL_LOADING` 0x45 with the progress [R]. The static caller of `FUN_39187410` is not in
`calls.tsv` (world-manager vtable), so the exact edge into it is U-B4.

Arrival on the movement side is already mapped: `FUN_3930c9e0` writes the new pose, resets the
camera, sets `DAT_39aff9d0+0xC/+0xD`, and `FUN_391a4e00` sends `CSTeleportEnded` 0x0B4; the client
clears `GameClient+0x10A0` when the server confirms. Do not re-map.

### 1.5 C — return points and resurrection (reuse, plus the book part)

```
Memory Tome doodad use -> skill 14602 (generic) -> S→C 0x049 SCCharacterBound
  handler 0x391d4240: adds the district row, sets bound id +0x608, SYSMSG "return_district_binding"
S→C 0x05C SCCharacterReturnDistricts: full district list + bound id
X2Warp.GetReturnList / GetBoundPortalId: the book's "Default Site" tab and the marker row

death (re-combat §2.9, not re-mapped):
  S→C 0x071 SCUnitDeath -> OnUnitDeath 0x391d51c0 -> FUN_39369c10
        ClientCharacter+0xAF4 = death time; +0xAF8 = server resurrect_wait_ms
  S→C 0x041 SCNotifyResurrection -> 0x391d39f0: +0xAFC=1, PLAYER_RESURRECTION (id 92)
  C→S 0x04E CSResurrectCharacter(inPlace) -> X2Player.Resurrect -> 0x394952b0 -> 0x39186e70
  S→C 0x042 SCCharacterResurrected -> 0x391d3ad0: clears +0xAF4/+0xAFC, PLAYER_RESURRECTED (id 93)
  X2Player.GetResurrectionCountdown -> 0x39495c00 -> (flag, 5-min remaining, wait remaining)
```

The client does **not** choose a respawn point: the server picks the nearest allowed `respawns.json`
entry (`portals.md` §1.5). The death window's only choice is the boolean `inPlace` ("resurrect
here" vs "to graveyard/temple"), and `X2Player:IsInIndunWithGraveyard` (0x39495b20) tells it which
layout to use. `port-combat` already implements the countdown and the two events; this map only
adds the **return-point book** (`SCCharacterBound` 0x049 + `GetReturnList`/`GetBoundPortalId`) [R].

### 1.6 D — navigation: navi doodads, map pins, quest marks

```
doodad funcs (actual_func_type):
  0x2D NaviMarkPosToMap  -> FUN_393b0c10: read (x,y,z) from the func row and fire
                            NAVI_MARK_POS_TO_MAP 0x127 { markerIndex, x, y, z }
  NaviNaming             -> FUN_393b0fc0: fire OPEN_NAVI_DOODAD_NAMING_DIALOG 0x129 { doodadId }
  NaviOpenMailbox        -> FUN_393b1040: fire TOGGLE_MAIL 0xCE {1, doodadId}; end interaction
  NaviOpenPortal         -> FUN_393b1250: if player at the saved navi spot, send CS 0x0E8 {doodadId}
  NaviTeleport           -> FUN_393b12b0: same gate, send CS 0x0E7 {doodadId}
  NaviRemove             -> FUN_393b1140: send CS 0x0EA {doodadId} [I: opcode/semantics]
S→C 0x0E7 SCNaviTeleport   handler 0x391dc8c0 -> FUN_3930c9e0(0,0,pos,0)
S→C 0x1A2 SCAboxTeleport   handler 0x391dc5a0: pos = current + axis*-1024; z = ground/water + 10.0
```

Doodad funcs `DoodadFuncEnterInstance` / `DoodadFuncRemoveInstance` are the dungeon/worldgate doodad
path; they use the generic doodad-use machinery (`port-interaction`, S2), not the portal book, and
are out of scope here. Map markers are registered by `FUN_3904ce90` (6834 B): index **0x45** is
`M_PORTAL` with type 0x0B,
so a portal is a standard map pin. Quest marks are the unit/doodad overlays:
`ClientUnit::UpdateQuestMark` 0x39359c60 (547 B) picks a `prefab://prefabs/quest_mark.xml/...`
prefab and binds it to the unit; `UpdateQuestContextMarks` 0x39383e20 (281 B) refreshes the set;
`ClientDoodad::UpdateQuestMark` 0x393a2c10 (488 B) is the doodad variant; `FUN_39382790` (587 B)
loads the prefab mark resources. These are client-side selection/visibility logic; the actual
arrow/emblem meshes are assets.

The navi func descriptors are loaded from the DB by `FUN_39673b90` / `FUN_39674d30` and the
per-table loaders `FUN_397a5780` (`doodad_func_navi_mark_pos_to_maps`), `FUN_397a5d90`
(`..._open_portals`), `FUN_397a5ef0` (`..._teleports`), `FUN_397a54a0` (`..._namings`),
`FUN_397a5600` (`..._open_mailboxes`), `FUN_397a5a90` (`..._remove_timers`),
`FUN_397a5c10` (`..._removes`), `FUN_397a5930` (`..._donations`), `FUN_397a6050`
(`..._open_bounties`) [R: SELECT strings].

`FUN_393b1250/12b0` gate the navi teleport on the local player standing at its saved navi position:
`player+0x3AC4 == DAT_3a17D7A8` and `player+0x3AC8 == DAT_3a17D7A4` [R]. `FUN_3930c9e0` is the
same arrival routine as an ordinary teleport, so the navi teleport is a same-zone jump with yaw 0.

---

## 2. State and rules (summary)

### 2.1 The client character's portal state

All offsets are on `ClientCharacter` = `GameClient+0x40` [R: every handler below reads it that way].
The full struct is in [`structs.md`](structs.md) §1.

| offset | type | meaning |
|---|---|---|
| `+0xB00/+0xB04/+0xB08` | vector begin/end/cap | private "Recorded Site" entries, 0x9C each |
| `+0xB10/+0xB14/+0xB18` | vector begin/end/cap | visited-district "Default Site" entries, 0x9C each |
| `+0x608` | u32 | bound return-point id (`GetBoundPortalId`; delete guard) |
| `+0x60C` | u32 | resurrection district id (from `SCCharacterBound`) |
| `+0xB20` | u32 | last auto-portal-use timestamp (ms) |
| `+0xAF4` | u32 | death timestamp (ms) |
| `+0xAF8` | u32 | server `resurrect_wait_ms` |
| `+0xAFC` | u8 | resurrection offered / can resurrect |
| `+0x4B8` | object | inventory manager used to find item 4045 |

### 2.2 Rules proven from the client

* The book's private and district lists are **append/replace vectors of fixed 0x9C entries**;
  the wire string is a fixed 128-byte name buffer in the packet object, zRot is a float after the
  position ([`packets.md`](packets.md) §2).
* `portal_type` is a **string** `"portal"` / `"return"`; the native side maps it to 2 (private)
  and 1 (district) for `OpenPortal`/`DeletePortal` [R: `FUN_394cad40`, `FUN_394cadf0`]. This
  settles the "opaque/possibly a string" note in `portals.md` §3.2.
* Deleting the bound district is refused **client-side** with error 0x19B and no packet [R].
* `SavePortal` needs item 4045; the failure is error 0x240 [R].
* Portals are entered by **walking within range of an entrance NPC** (3891/5476), on a 1 s scan
  and a 10 s reuse gate; a resurrection debuff (1128/1910) blocks it with error 0x165 [R].
* Same-zone `SCTeleportUnit` never shows the world loading art; `SCLoadInstance` is what reaches
  it [R].
* The death window does not pick a point; only the `inPlace` boolean is sent [R, plus
  `portals.md` §1.5].

---

## 3. Proposed port order (testable leaves first)

1. **Data + wire leaves.** `PortalEntry`/`PortalBook`; packet bodies; the pure setters/readers
   `FUN_397ac7b0`, `FUN_397ac7d0`, `FUN_397ac8a0/b0/c0`, `FUN_397b0fb0`, `FUN_397c2a50`,
   `FUN_397aef40`, `FUN_39424820`; the count/getters `FUN_3930b410/480`, `FUN_3938fd20`.
2. **Wire codecs.** `PortalWire` read/write (`FUN_397b1e30` + `FUN_397c2a50` style), 0x05B/0x05C/
   0x05D/0x05E/0x060/0x049/0x041/0x042/0x197/0x0E7 readers. Unit tests from captured packet bytes;
   the lane server's Rust writer is the byte oracle.
3. **Book state machine.** `FUN_3930fa50`, `FUN_3930fd40`, `FUN_3930fc80`, `FUN_3930fd80`,
   `FUN_3930f620`, `FUN_3930f6b0`; assert the vector contents and the 0xE5/0xE6/0xE7 events.
4. **Row builder + X2Warp API.** `FUN_3930cb90`, `FUN_3930cfb0/cfd0`, the 12 wrappers, the request
   builders `FUN_3938f7d0/f950/faa0/fc90` and their prechecks. Compare Lua rows and captured
   `CSStartSkill`/`CSDeletePortal`/`CSUsePortal` packets.
5. **Item trigger + book UI.** `FUN_394eaac0` case 14 (`TOGGLE_PORTAL_DIALOG`), the VNT_PORTAL
   policy in `FUN_394c9b40`; then the existing `portal.lua`.
6. **Auto-use and unit presentation.** `FUN_3930cff0`, `FUN_3930b5b0`, `FUN_391827f0`,
   `FUN_394c1ca0`, `FUN_394c0680`, `FUN_393558e0`. Test range/cooldown/buff gates with a spawned
   gate.
7. **Teleport transition wiring.** `OnTeleportUnit` flags, `OnLoadInstance`/`FUN_3936f1c0`,
   `FUN_3942f770` art selection, `CSInstanceLoaded`; wire to `port-movement-extra`'s reset and the
   existing world switch.
8. **Return-point binding.** `SCCharacterBound` 0x049 handler + `FUN_3930fd80` + bound id + sysmsg;
   `GetReturnList`/`GetBoundPortalId`; the death UI stays as already implemented in `port-combat`.
9. **Navigation.** `FUN_393b0c10`, `FUN_393b0fc0`, `FUN_393b1040`, `FUN_393b1250/12b0`,
   `FUN_393b1140`, `OnNaviTeleport`, the navi DB loaders; map pins and quest marks last.
10. **Oracle suite:** item 4045 -> book; record/rename/delete with Memory Ink; walk into a spawn
    gate; Recall (SCLoadInstance + SCTeleportUnit); Memory Tome binding; death + respawn; navi
    sign teleport.

---

## 4. Unknowns

| id | unknown | how to settle |
|---|---|---|
| U-A1 | `OpenPortal` fills no position in the `PortalInfo` skill object (the 0xD0 payload is zeroed; only type/id set). The server/emulator expects (x,y,z) for the gate. | Hook `0x3AC1058C` in-process, call `X2Warp:OpenPortal`, dump the packet; check the server's `OpenPortalEffect` behaviour with zeros. |
| U-A2 | `CSUsePortal` serializer writes `tid`(u32), `type`(u32), `role`(u8), while the emulator documents `objId`+`onlyMyPortal`. Which field is the gate id and which is the safe-mode flag? | Capture the real packet from `X2Warp`/auto-use and compare with the lane server parse; verify with 3-byte vs 4-byte ids. |
| U-A3 | `CSDeletePortal` serializer `FUN_397aef40` writes only `tid` (u32); the local builder stores type+id. Is the type byte dropped in 1.2 (server infers) or serialized by a variant? | Capture a delete of a private and a district row; compare the server handler. |
| U-B1 | `unit+0x418` (the `tid` passed to `CSUsePortal`) semantics: unit object id vs template id. `UseTeleport` uses `unit+8` as the NPC id, which suggests +0x418 is something else. | Read a live gate unit: compare `+8`, `+0x418`, `+0x41C`; then use the gate and watch the server accept/reject. |
| U-B2 | The auto-use range source (`FUN_3989ac80()` with a lost `this`) and the meaning of per-unit variable 6. | Call on a live player with gates at 2.9/3.0/3.1 m; vary the variable with `write` and observe. |
| U-B3 | Exit NPC 6949 (server) vs 6629 (client portal set). | Spawn both and check tooltip/auto-use; reconcile server data. |
| U-B4 | The exact edge from `FUN_3936f1c0` (SCLoadInstance) to `FUN_39187410`/`FUN_3942f770` (loading art) is a world-manager vtable, not a static call. | Breakpoint/hook the art function while entering an instance and walking the stack. |
| U-C1 | `FUN_398B56D0` (used by `GetResurrectionCountdown` and `CSResurrectCharacter`) is not identified; units of `+0xAF8` are assumed ms (port-combat says ms). | Call `GetResurrectionCountdown` with crafted `+0xAF4/+0xAF8/+0xAFC` and compare the three returned values. |
| U-D1 | The numeric `actual_func_type` ids for the other navi funcs besides 0x2D (naming/mailbox/open portal/teleport/remove) are read from the same if-chain but not individually proven. | Read the loaded desc at `doodad+0x7C`/func list and match the handler; or map the if-chain assignments. |
| U-D2 | Argument order of event 0x127 (`markerIndex`, x, y, z) and the meaning of the four Lua values in `FUN_394242a0`. | Hook the UI event in Lua, call `FUN_393b0c10`, compare with `NAVI_MARK_POS_TO_MAP` in the map Lua. |
| U-D3 | `FUN_393b1140`'s opcode 0x0EA and its relation to `DoodadFuncNaviRemove`. | Capture after using a Portable Sign; compare with the server's navi handlers (currently stubs). |
| U-E1 | `0x115` was listed with H5 in `remaining.md`, but its reader (`FUN_397b50d0`) is an `err` byte + ten `(type,index)` pairs + `money` packet with no portal/return fields; it is treated as out of scope here. | Identify the handler/opcode name from the SC table and close H5's reference. |

---

## 5. Reused functions (already mapped)

| addr | size | where |
|---|---|---|
| `0x3930C9E0` | 378 | teleport movement/camera reset — `port-movement-extra/click-move-jump-teleport.md` §6.2 |
| `0x391D6ED0` | 370 | `SCTeleportUnit` handler — same, §6.1 (only the portal flags are repeated here) |
| `0x391D39F0` | 215 | `SCNotifyResurrection` — `port-combat/MAP.md`, `re-combat.md` §2.9 |
| `0x391D3AD0` | 399 | `SCCharacterResurrected` — same |
| `0x391D51C0` | 165 | `OnUnitDeath` — same |
| `0x39369C10` | 1183 | local death state / `+0xAF4/+0xAF8` — same |
| `0x39395AE0` | 106 | death packet gate — same |
| `0x39499170` | 4875 | `X2Player` registration (Resurrect/countdown and unrelated player APIs) — same |
| `0x39495AA0` | 117 | `IsPlayerAimming` — same |
| `0x39495B20` | 222 | `IsInIndunWithGraveyard` — same |
| `0x39495C00` | 259 | `GetResurrectionCountdown` — same |
| `0x394952B0` | 50 | `Resurrect(inPlace)` wrapper — same |
| `0x39186E70` | 198 | `CSResurrectCharacter` 0x04E sender — same |
| `0x39495680` | 214 | three-value Lua return helper — same |
| `0x397C85B0` | 431 | `SCNotifyResurrection` SkillCaster reader — same |
| `0x397C5C20` | 63 | `SCCharacterResurrected` reader — same |

Shared leaves used by this system but owned elsewhere: `FUN_39360020` (unit lookup, 57 B),
`0x39186810`/`0x3AC1058C` (packet send finalizer), `FUN_39424220/40/60/A0` (event fire),
`FUN_3920FA60`/`FUN_3920FCF0` (event dispatch), `FUN_39428370` (error message),
`FUN_393F1160` + `393EEE10/80`/`393EEFA0` (effect spawn), `FUN_3921A9F0` + `FUN_39058B50`
(Lua table), `FUN_394E0960` + `FUN_397CD0A0` (item lookup).
