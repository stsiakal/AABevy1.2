# Classes, vtables and struct layouts (1.2 housing placement)

All addresses are x2game VAs (base `0x38FF0000`). VTables were read from `x2game_unpacked.dll`
(`.rdata`); object layouts from the constructors in `x2game.dll.c`. Class names in parentheses are
from the 10.x dev build's profiler/assert strings **[R10]**; 1.2 only ships the
`X2::GameClient::HousingManager::Update` profiler string (`strings.txt:0x399cf724`) and the
`X2::GameClient::Housing::*` strings (`0x399cf6d8/0x399cf7c0/0x399cf7ec`), which are **[R1.2]**.

## 1. Housing manager (`DAT_39aff98c`)

Global pointer. The manager is allocated by `FUN_3932a3b0`'s owner (client init); fields read by the
placement code:

| offset | type | role |
|---|---|---|
| +0x08 | `std::map<tl, Housing*>` | house table (`FUN_3932b510`) |
| +0x30 | builder* | current house/other builder (null when idle) |
| +0x34 | cursor* | current decoration cursor (null when idle) |
| +0x3c | list | temporary fence/neighbour list (`FUN_393304b0`, `FUN_3932f520`) |
| +0x48/+0x4c | vector | temporary fence objects; destroyed in `FUN_3932f520` |
| +0x58/+0x5c | vector | temporary neighbour entries (`FUN_3932f520`) |
| +0x78 | u32 | selected house unit id (`FUN_393befd0`) |
| +0x7c | u32 | interacting unit id (`FUN_393befd0`) |

`X2::GameClient::HousingManager::Update` (`FUN_3932a3b0`, `0x3932a3b0`, 1245 B) is the per-frame
driver:

```
if (manager+0x30 && !ui_mode) { FUN_39332030(builder); FUN_39017410(0, builder+0xc, builder+0x10); }
if (manager+0x34 && !ui_mode) { FUN_3903e020(cursor); FUN_39017410(1, cursor+0x44, cursor+0x40); }
if (manager+0x34 && !ui_mode) { FUN_3903d870(cursor); }          // preview box
if (manager+0x30) { builder->vtable[+4](); ...preview spin / link animation... }
```

`FUN_39017410` is the cursor/`BUILDER_STEP` updater (see `checks.md` §5).

Housing units (`X2::GameClient::Housing`) carry `tl` and are looked up by `FUN_3932b510(tl)`; the
fields used by construction/decoration are the ones listed in `docs/research/re-housing.md` §1.1
(`+8` template, `+0x140/+0x144` all/current actions, `+0x18` unit obj id, `+4` tl, `+0x13c`
connector vector, `+0x16c/+0x170` fence doodads, `+0x220` connector-snapped flag, `+0x224..+0x23c`
subclass spawn data).

## 2. Builder class hierarchy

`FUN_39330ba0` (`StartBuild`, 604 B) chooses the class from `FUN_3908c810(design)`:

| kind | class ctor | vtable | allocation | 1.2 categories (DB) | 10.x name |
|---|---|---|---|---|---|
| 1 | `FUN_39336140` (`0x39336140`, 324 B) | `0x399cf920` | `0x220` | 1,7-13,15-18 | `HouseBuilder` |
| 2 | `FUN_393377d0` (`0x393377d0`, 34 B) | `0x399cf994` | `0x240` | 2 (castle tower) | `TowerBuilder` [I] |
| 3, 14 | `FUN_39337920` (`0x39337920`, 121 B) | `0x399cf9fc` | `0x254` | 3 (wall), 14 (gate) | `WallBuilder` [I] |
| 4 | `FUN_39337800` (`0x39337800`, 34 B) | `0x399cf9c8` | `0x240` | 4 (stair) | `StairBuilder` [I] |
| other (5,6,…) | `FUN_39336520` (`0x39336520`, 202 B) | `0x399cf94c` | `0x240` | 5 stronghold, 6 foundation | `CastleBuilder`/`KeepBuilder` (1.2 has one default class) [I] |

`kind = 1` when `FUN_39897070(category)` is true (categories 1,7,8,9,10,11,12,13,15,16,17,18;
`0x39897070`, 34 B); otherwise `kind = category` (`FUN_3908c810`, 29 B). `FUN_3908c850(unit)` returns
the housing desc byte `+0xd` (plot style); `FUN_3908c810(unit)` reads `unit+0xc` (the temp unit mirror of the housing category).

The 10.x assert strings (`D:/aa-re/zone/bin/x2game-dev_dedicate.dll.strings.txt`) name the
`UpdateLocating`/`UpdateRotating`/`UpdateElevation`, `SnapToNearestConnector`, `CastleBuilder`,
`KeepBuilder`, `TowerBuilder`, `StairBuilder`, `WallBuilder` methods; the 1.2 binaries keep only the
function bodies (no names).

### 2.1 HouseBuilder object (`0x220` bytes, written by `FUN_39336140`)

| offset | type | role |
|---|---|---|
| +0x00 | vtable | `0x399cf920` (`FUN_39336140`) |
| +0x04 | ptr | `&DAT_39ac3044` (type info/secondary vtable) |
| +0x08 | Unit* | temporary "temp housing" unit created by `FUN_3932fb20` |
| +0x0c | i32 | state: 1 position, 2 rotation, 3 elevation, 4 tax-pending |
| +0x10 | i32 | reported error code (first error wins) |
| +0x14 | i32 | secondary error code (terrain spread / height) |
| +0x18..0x1fb | 0x1e4 B | snapshot of the last validated placement / collision state |
| +0x1fc/+0x200/+0x204 | vector | plot sample points (x,y,z float3), end, cap |
| +0x208 | ? | (base/subclass scratch) |
| +0x210 | ptr | list head used by `FUN_393354c0` / neighbour sets |
| +0x214 | i32 | 0 |
| +0x218 | i32 | 0 |
| +0x21c | i32 | total tax/money from `SCConstructHouseTax` (set by `FUN_3932e530`) |
| +0x220 | byte | connector-snapped flag (set by `FUN_39338d40` for non-house classes) |

Subclass fields added after `FUN_39336140`:

* `FUN_39336520` (default): `+0x224..+0x234` zeroed, `+0x238` byte, `+0x240` dword; copies from a
  re-used object when `param_3 != 0`.
* `FUN_39337920` (wall/gate): `+0x240..+0x250` (spawn position/flags copied from `template+0x244`,
  `+0x24c`, `+0x248`), bytes `+0x24c`.
* The tower/stair classes use their 0x240/0x254 tails for spawn yaw/position; see the constructors.

### 2.2 HouseBuilder vtable (`0x399cf920`, slots used by the port)

| slot | function | role |
|---|---|---|
| +0x00 | `FUN_39337f80` (30 B) | deleting destructor |
| +0x04 | `FUN_39334ab0` (1190 B) | draw plot dots (kind 1) |
| +0x08 | `FUN_393322d0` (233 B) | draw connector/plot grid for garden designs |
| +0x0c | `FUN_3933a300` (122 B) | **SetPos** (state 1) |
| +0x10 | `FUN_3933a380` (124 B) | **Rotate** (state 2) |
| +0x14 | `FUN_39336290` (16 B) | stub `ret 4` (elevation not used by kind 1) |
| +0x18 | `FUN_393364e0` (51 B) | reset + revalidate + tint |
| +0x1c | `FUN_393317e0` (8 B) | state = 1 |
| +0x20 | `FUN_393317f0` (8 B) | state = 2 |
| +0x24 | `FUN_39331800` (8 B) | state = 3 |
| +0x28 | `FUN_39331810` (8 B) | state = 4 |
| +0x2c | `FUN_39338d10` (36 B) | deleting destructor (base variant) |
| +0x30 | `FUN_39337490` (823 B) | Render (house/neighbour fence links) |
| +0x34 | `FUN_393322d0` | connector/plot grid again |
| +0x38 | `FUN_39337320` (113 B) | alternative SetPos (snap + height band) |
| +0x3c | `FUN_393373a0` (161 B) | alternative Rotate |
| +0x40 | `FUN_39337450` (52 B) | Revalidate |
| +0x44 | `FUN_393364e0` | reset + revalidate |
| +0x48 | `FUN_39332690` (101 B) | orientation/step helper [I] |
| +0x4c | `FUN_39332700` (101 B) | orientation/step helper [I] |
| +0x50 | `FUN_39332770` (101 B) | orientation/step helper [I] |
| +0x54 | `FUN_39331810` | state = 4 |
| +0x58 | `FUN_3933a400` (412 B) | castle/keep SetPos variant (connector snap) |
| +0x5c | `FUN_393365f0` (16 B) | stub `ret` |

The tower (`0x399cf994`), stair (`0x399cf9c8`) and wall (`0x399cf9fc`) vtables replace the
position/rotation slots with:

| class | SetPos (+0x0c) | Rotate (+0x10) | Elevation/revalidate (+0x14) | extra SetPos variant |
|---|---|---|---|---|
| kind 1 house | `FUN_3933a300` (122 B) | `FUN_3933a380` (124 B) | `FUN_39336290` (`ret 4`) | - |
| tower (`0x399cf994`) | `FUN_3933a5a0` (282 B) | `FUN_3933a6c0` (114 B) | `FUN_39339580` (284 B) | +0x2c `FUN_3933a400` (412 B), +0x30 `FUN_393365f0` stub |
| stair (`0x399cf9c8`) | `FUN_39337840` (97 B) | `FUN_393378b0` (105 B) | `FUN_39337830` (`ret 4`) | +0x2c `FUN_3933a400`, +0x30 `FUN_393365f0` |
| wall/gate (`0x399cf9fc`) | `FUN_3933a740` (106 B) | `FUN_3933a7b0` (145 B) | `FUN_39339960` (329 B) | +0x2c `FUN_3933a040` (689 B), +0x30 `0x393349c0` (12 B, toggle helper, not in funcs.csv) |
| default/castle (`0x399cf94c`) | `FUN_39337320` (113 B) | `FUN_393373a0` (161 B) | `FUN_39337450` (52 B) | +0x2c `FUN_3933a400`, +0x30 `FUN_393365f0` |

Exact slot lists are in `inventory.md`; the per-class `UpdatePosition/Rotation/Elevation` names are
from the 10.x asserts. All of them end with `FUN_393363b0` + `FUN_39334460`, so validation/tint is
shared.

## 3. Decoration cursor object (0x54 bytes)

Created by `FUN_3903bf60(this=alloc(0x54), callback_interface, ghost, doodad_type, flag)`
(`0x3903bf60`, 892 B); `FUN_3932c120` stores it in `manager+0x34`.

| offset | type | role |
|---|---|---|
| +0x00 | ptr | callback interface (the item task object; `this` is a `DecoTask`-like object) |
| +0x04 | u32 | doodad/design type (`param_4`) |
| +0x08 | ptr | placement ghost object (from `FUN_393b03b0`) |
| +0x0c..0x38 | float[9]+ | ghost transform (3x3 basis / position) |
| +0x3c | f32 | yaw (radians) |
| +0x40 | i32 | error code (0 = valid) |
| +0x44 | i32 | state: 1 position, 2 rotation |
| +0x48 | byte | current surface constraint (see `checks.md` §3) |
| +0x49 | byte | position initialised / valid cursor |
| +0x4c | ? | (reserved) |

Methods:

| addr | size | role |
|---|---|---|
| `0x3903b8a0` | 130 B | cancel/cleanup: clears the placement skill, `FUN_390173b0(0)`, 1000 ms cooldown, frees ghost |
| `0x3903b9d0` | 33 B | confirm: state 1 -> 2 when position valid; state 2 -> `FUN_39327de0` |
| `0x3903ba00` | 25 B | back: state 2 -> 1; otherwise `FUN_39325210` cancel |
| `0x3903c5a0` | 4339 B | validity (surface/garden/house/overlap) |
| `0x3903d6a0` | 454 B | apply transform + invalid material override |
| `0x3903d870` | 388 B | draw preview box (blue/red) |
| `0x3903da00` | 1565 B | position from mouse/ray, initialise transform |
| `0x3903e020` | 90 B | per-frame state update (position/wheel) |
| `0x3903e080` | 37 B | slot helper |

`FUN_39327de0` (`0x39327de0`, 2119 B, not in `funcs.csv` - Ghidra did not start it) is the
`CSDecorateHouse` sender/cleanup; `FUN_3903b9d0` reaches it by `jmp` with ECX = `DAT_39aff98c`
(cursor confirm -> `HousingManager::DecoratorOK`; name from 10.x `0x3A08F418`).

## 4. Placement ghost (`FUN_393a5930`, 0x3b0 bytes) and doodad builder

`FUN_393b03b0` (`0x393b03b0`, 1423 B) loads the doodad model and returns a ghost object; for the
decoration/housing path the constructor is `FUN_393a5930` (`0x393a5930`, 853 B). Fields used by the
packet writers:

| offset | type | role |
|---|---|---|
| +0x04 | u32 | design id (housing/deco design sent on the wire) |
| +0x18/+0x1c/+0x20 | f32 xyz | local/world position used by `CSCreateDoodad` |
| +0x4c | u32 | parent object id (bonded house), read by `CSDecorateHouse` |
| +0x210 | f32 | doodad scale |
| +0x218 | u32 | doodad template id |
| +0x38c..0x390 | vector | leaf marker meshes (`FUN_393a4cc0`) |
| +0x900 | int | marker count |

Seed builder global `DAT_3a17a948` (`FUN_393a8470`, 202 B; object 0x54 B):

| offset | type | role |
|---|---|---|
| +0x00 | vtable | `0x399d2474` (slot 0 update `FUN_393a9810`, slot 4 destructor) |
| +0x04 | ptr | `&DAT_39ac3044` |
| +0x08 | ghost* | placement ghost (from `FUN_393a7a10`) |
| +0x0c | i32 | state: 1 position, 2 rotation, (5/6 used by confirm) |
| +0x10 | i32 | error code (0x118 = valid) |
| +0x14 | ptr | item u64 (design item) |
| +0x28 | u32 | target doodad/unit id |
| +0x2c | byte | sent flag |
| +0x30 | ptr | cursor object (`FUN_3903bf60`) |

`FUN_393a8fa0`/`FUN_393a9690` read/write the `+0x10` error slot; `FUN_393a9340` sends the packet
and sets `+0x2c = 1`.

## 5. Connectors, fences, auto stair/door

* `X2::GameClient::Housing::LoadConnectors` (`FUN_3932a270`, 140 B) - profiler span around
  `FUN_390904b0` (the engine model data load).
* `FUN_3932d0f0` (676 B) - `InitFromNetHouse` + `LoadConnectors` (names in the 1.2 profiler strings);
  copies `SCHouseState` fields into the Housing object (`+0x14/0x18/0x1c/0x20/0x24/0xa8/0xac/0x140/
  0x144/0x14c`, desc `+0x154..+0x168`).
* `FUN_3932ead0` (940 B) - `InitFences` (1.2 profiler string `0x399cf7ec`): spawns the fence doodads
  around a placed house.
* Connector data is per-housing model; `FUN_39338d40` iterates `[house+0x13c]` (0x40-byte records)
  and other houses within 18 m (`_DAT_3999f928`), finds the nearest connector and snaps Z.
* `__housing_auto_stair__` (`strings.txt:0x399a26eb`) and `__housing_auto_door__`
  (`0x399a2704`) are model-name substitutes used by the castle/wall builders
  (`FUN_3908dbf0`, 1091 B, for the auto door).
