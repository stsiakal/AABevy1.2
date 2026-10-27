# S4 - 1.2 housing and decoration placement: port map

> **Restored 2026-10-27 from `archive/fork-main-2026-10-27`, UNVERIFIED.** The client was restarted from NickMesser's `main`; nothing described here is wired into the game. Addresses, call chains and offsets must be re-checked against the decompile ([R]/[I]/[?]) before use. A 2026-10-27 audit found 151 of the 269 EXACT/TOLERANCE labels in `proof-levels.*` weak or wrong (report: `D:\aaemu-client\handoff\2026-10-27\audit\report-proofs.md`).

Scope: mapping only (read-only). The later phase ports these functions line by line into the Bevy
client and proves each one by calling the original in the running hidden 1.2 client (the in-process
`aadriver.dll`). This document is the map for `port-program.md` step 1, system **S4**.

Sources (no CRYENGINE source used):

* `C:/AAEMU_DISK/aa-re/client12/out-x2game/x2game.dll.c` (**1.2 x2game decompile**, image base
  `0x38FF0000`), `funcs.csv`, `calls.tsv`, `strings.txt`, `strings-xrefs.tsv`;
  `D:/aa-re/client12/x2game_unpacked.dll` and the `.data` dump
  `D:/aa-re/client12/x2game_unpacked_data.dll` for constants/vtables.
* 10.x reference for **names/structure only**: `D:/aa-re/zone/bin/x2game-dev_dedicate.dll{,.strings.txt}`
  (marked **[R10]**); never behaviour.
* Existing notes: `docs/research/re-placement.md`, `re-housing.md`, `re-constants.md`;
  `docs/research/port-program.md`; `C:/AAEMU_DISK/aaemu-client/AABevy1.2/crates/aa-net` for the
  server-side packet layouts (byte-exact).
* Driver: `C:/Users/stsiakal/AAEmu/rust/crates/aaemu-clientdriver/src/{lib.rs,usage.txt}`.

Confidence: **[R]** read in 1.2 code/data; **[I]** inferred; **[?]** not found. Every function row
cites `FUN_x` and its VA; the generated [`inventory.md`](inventory.md) has all 197 functions in
scope with size, callers and callees.

---

## 0. Summary

* **Entering placement** is an item-use task: the dispatcher `FUN_394eaac0` cases 5 (house design),
  6 (decoration) and 9 (seed/sapling) call `FUN_39330ba0`, `FUN_3932c120`, `FUN_393a9540`
  respectively.
* **House builder** (`FUN_39330ba0`) creates a temporary housing unit from the design
  (`FUN_3932fb20`) and one of five builder classes by `housings.category_id`:
  `HouseBuilder 0x220 B`, tower `0x240`, stair `0x240`, wall/gate `0x254`, castle/keep `0x240`
  (section 2). The per-frame driver is `HousingManager::Update` `FUN_3932a3b0`.
* The state machine is **two clicks**: state 1 (follow mouse / terrain) -> state 2 (rotate) on the
  first click; a second click sends `CSConstructHouseTax` (0x56) for house-like designs, or
  `CSCreateHouse` (0x57) for the others (after an optional elevation state for towers/walls).
  Right click steps back/cancels; Escape cancels. `BUILDER_STEP` 0x224 carries
  `stanby/position/rotation/elevation`.
* **Wheel step is resolved**: normal = `axis * 1 deg * builder_rotate_angle`; the cvar default is
  **5.0** (1.2 descriptor `0x39ac5658`, value object `0x39adac58`; 10.x name/help under
  `0x3A01A178`); small = 0.01 rad, large = pi/12; elevation = -0.02 rad. No config overrides it.
  See [`constants.md`](constants.md) §1.
* **Validation** is a fixed order: range, housing area/category, dominion, water, OBB overlap,
  height; then id checks at click/create. All error codes and keys are in [`checks.md`](checks.md).
* **Plot dots** are terrain-sampled points in the model's rotated frame, drawn at `0xAFFFFFFF`;
  the sample step is the terrain object's `vtbl+0x20c`; height spread > **6.0 m** (not 3 m) gives
  error 0x74. The dots are gated by the debug object `DAT_3a167e84`.
* **Snapping** exists only for the connector builders (tower/wall/stair/castle): `FUN_39338d40`
  finds the nearest connector, moves the model to its Z and sets `+0x220`; errors 0x75/0x76.
  The 1.2 HouseBuilder itself has no connector snap. 10.x's `use builder snap` / `cd_builder_snap`
  cvars are not in 1.2.
* **Decoration** uses a 0x54 B cursor (`FUN_3903bf60`) and the same two-click rule; validation is
  `FUN_3903c5a0` (surface, garden, in/out of house, overlap) and the confirm is
  `FUN_39327de0` -> `CSDecorateHouse` 0x58. The ghost uses the same
  `materials/special/housing_build_invalid` tint; 1.2 has no `housing_build_valid` material.
* **Seeds/saplings** (`FUN_393a9540`) build their own ghost/builder (`DAT_3a17a948`) and use
  `FUN_393a8fa0` for range/area/permission/overlap; success sentinel is 0x118, confirm sends
  `CSCreateDoodad` 0xE6.
* **House and seed packets (0x56/0x57/0xE6) convert the position with `FUN_3961dd70`**:
  `round(f * 2^44) + origin` (origin at `GameClient+0x118`), z as f32; `2^44 = 0x399f9a80`,
  matching aa-net `coord_to_long`. `CSDecorateHouse` 0x58 is different: it sends house-local
  floats from the cursor transform, no fixed-point conversion.

Function count in scope: **197 functions, 84,374 bytes** (generated `inventory.md`), of which the
primary path is ~70 functions (~40 KB). No CRYENGINE source was consulted.

---

## 1. Entry points and shared pump

### 1.1 Item-use task dispatcher

`FUN_394eaac0` (`0x394eaac0`, 1882 B) is called by `FUN_394eb270` (728 B, item-task executor).
`param_3` is the task descriptor; `*(param_3+0x48)` is the task type:

| case | action | function |
|---|---|---|
| 5 | house/farm design item | `FUN_39330ba0(*puVar4, param_1, param_2)` (section 2) |
| 6 | decoration item | `FUN_3932c120(house+4, item, task, target)` -> requires a house within reach, else 199 = 0xc7 (`FUN_39328630`) |
| 9 | seed/sapling | permission checks `FUN_39053360`/`FUN_390533f0` (errors 0x2ed/0x2ec) then `FUN_393a9540(doodadType, item, user)` (section 4) |

The same function declines when the player is in a special state (task type 4/5 + player flag
`+0xd & 0x10` -> error 0x2f), for other task types (11/14/17/20/23/26) it runs unrelated item
features, so the port needs only the three cases above. `[R]` (`x2game.dll.c` FUN_394eaac0).

### 1.2 `HousingManager::Update`

`FUN_3932a3b0` (`0x3932a3b0`, 1245 B; name from the **1.2** profiler string at `0x399cf724`)
pumps the placement state every frame:

```
if (manager+0x30) { FUN_39332030(builder); FUN_39017410(0, builder+0xc, builder+0x10); }
if (manager+0x34) { FUN_3903e020(cursor); FUN_39017410(1, cursor+0x44, cursor+0x40);
                    FUN_3903d870(cursor); }
if (manager+0x30) { builder->vtable[1](); preview spin (DAT_3a167f00, _DAT_399cf5ac=pi,
                    FUN_3908c870, 2*pi wrap); }
```

The `FUN_39017410` calls are the cursor/`BUILDER_STEP` updater (section 3.4). The builder and cursor
are both disabled while a UI mode is active (`FUN_392aa250(1/3)`); the details of that UI gate are
shared with movement/camera and not part of this port.

---

## 2. Area A - house placement mode

### 2.1 Flow

1. **StartBuild** `FUN_39330ba0(this=manager, designItem, target, user)` (`0x39330ba0`, 604 B):
   * `FUN_39016fd0`/`FUN_39017620`: global checks (`0xce` not ready, `0x14e` gliding, `0x14c` dead,
     `0xde` mounted, `0x163` other build, `0x2f` special).
   * `FUN_39017620` (`0x39017620`, 258 B) reads `DAT_39aff9f8`, `DAT_39aff9cc`, `DAT_39aff98c`,
     player `+0x3c26`/`+0x3af0/0x3af4`, and `FUN_39428370(code)` on failure.
   * `FUN_398a7a30(1, char, pos)` (server-region/instance check; error directly).
   * `FUN_393304b0` (114 B): resets the temp fence/neighbour lists (`FUN_3932f520`) and stores the
     local player unit (`FUN_39360020`), clears the house-link target (`player_unit vtable+0x1a4(0)`).
   * `FUN_3932fb20(design)` (272 B) creates the **"temp housing"** unit, model from the design's
     `housings.main_model_id`; `FUN_3932fa00`/`FUN_3932f6b0` build the Housing object, then
     `FUN_3932d0f0` copies the net/template fields and `LoadConnectors`.
   * `FUN_39330830`/`FUN_3932c900` initialise the neighbour/fence links for the temp house.
   * class by `FUN_3908c810(design)` (section 2.2); new object in `manager+0x30`; if the design is
     used from a task, a new item reference is stored in `manager+0x2c` (`FUN_394ef900`,
     `FUN_394e84e0`).
   * `FUN_39016fb0` sets a 1000 ms input guard.
2. **Per-frame** `FUN_39332030` (`0x39332030`, 202 B):
   * if `FUN_392aa2e0()` is false (cursor not over the world) and state != 4 -> error `0x70`;
   * state 1: `FUN_3921ac90` mouse position -> `FUN_394298c0(out, x, y, 200.0, 0, 0)` -> vtable
     `+0xc` (**SetPos**);
   * state 2: `FUN_39016fe0` wheel delta -> vtable `+0x10` (**Rotate**);
   * state 3: `FUN_393fd240(9) * -0.02` -> vtable `+0x14` (elevation, non-house classes);
   * state 4: vtable `+0x18` (revalidate).
3. **SetPos** `FUN_3933a300` (`0x3933a300`, 122 B): clears `+0x10/+0x14`, zeroes the 0x1e4
   snapshot, `FUN_39325160(pos)` (temp unit vtable+0x44), if `FUN_39325190()==2` `FUN_39332100`
   (garden plot), then `FUN_3908c850()`: 0 -> `FUN_39332180` (6 m spread + commit), else
   `FUN_39337fb0` (terrain-grid plot), then `FUN_3932eef0` (fences), `FUN_393363b0` (validation),
   `FUN_39334460` (tint).
4. **Rotate** `FUN_3933a380` (`0x3933a380`, 124 B): same reset, reads temp unit yaw
   (vtable+0xc), `FUN_39326800(yaw + delta)`, plot rebuild, validation, tint.
5. **Left click** `FUN_39332200` (`0x39332200`, 208 B):
   * kind 1: state 1 -> vtable `+0x20` (state 2); state 2 -> `FUN_39325f60` (quote 0x56) and on
     success vtable `+0x28` (state 4);
   * other kinds: state 1 -> state 2; state 2 and kind 2/3 -> vtable `+0x24` (state 3);
     otherwise (or after elevation) `FUN_394df370`, `FUN_397ccd50`, `FUN_3932fe60` (0x57).
   The 0x70 special branch resets a stale error and sets 0x215 for castle/keep [I].
6. **Right click** `FUN_39331760` (`0x39331760`, 123 B): kind 1 state 1 -> cancel; state 2 ->
   state 1; state 4 -> fire **`HOUSE_BUILD_INFO` 0x100** then state 2. Other kinds: state 1 ->
   cancel, state 2 -> 1, state 3 -> 2.
7. **Cancel** `FUN_39330120` (`0x39330120`, 108 B): `BUILDER_END` 0x225, free `manager+0x2c`,
   destroy the builder object (vtable+0, flag 1), `FUN_3932f520`, cursor mode 0.
8. **Create** `FUN_3932fe60` (`0x3932fe60`, 701 B, section 5.2) or **quote**
   `FUN_39325f60` (`0x39325f60`, 176 B, section 5.1). The 5 s rate limit lives in the create path.

The state setters (vtable `+0x1c..0x28`) are one-instruction functions at `0x393317e0/…/0x39331810`
(8 B each): `mov [ecx+0xc], imm; ret`.

### 2.2 Builder classes and plot

`FUN_3908c810` (`0x3908c810`, 29 B) = 1 iff `FUN_39897070(category)` (categories
1,7-13,15-18; `0x39897070`, 34 B); otherwise it returns the category. It reads the temp unit's
`+0xc` (the housing category mirror), so the builder class is chosen by `housings.category_id`:

| kind | class | ctor | vtable | alloc | DB categories | 10.x name |
|---|---|---|---|---|---|---|
| 1 | HouseBuilder | `FUN_39336140` (324 B) | `0x399cf920` | 0x220 | 1,7-13,15-18 | `HouseBuilder` |
| 2 | tower | `FUN_393377d0` (34 B) | `0x399cf994` | 0x240 | 2 | `TowerBuilder` [I] |
| 3,14 | wall/gate | `FUN_39337920` (121 B) | `0x399cf9fc` | 0x254 | 3,14 | `WallBuilder` [I] |
| 4 | stair | `FUN_39337800` (34 B) | `0x399cf9c8` | 0x240 | 4 | `StairBuilder` [I] |
| other | default | `FUN_39336520` (202 B) | `0x399cf94c` | 0x240 | 5,6 | `Castle/KeepBuilder` [I] |

The 1.2 plane has only one "default" class; the 10.x code split it into `CastleBuilder` and
`KeepBuilder`. `FUN_39336140` writes the vtable, `+4=&DAT_39ac3044`, `+8=temp unit`, `+0xc=state`,
`+0x10/+0x14=errors`, zeroes `+0x18..0x1fb` (0x1e4), `+0x1fc/+0x200/+0x204` plot vector,
`+0x210` list, `+0x21c=0`. The subclass ctors add spawn data at `+0x224..+0x254`. Full layout and
vtables: [`classes.md`](classes.md) §2.

Plot:
* Kind 1 with desc `+0xd` != 0: `FUN_39337fb0` (`0x39337fb0`, 3415 B) samples a rotated terrain
  grid, step = terrain `vtbl+0x20c`, height = terrain `vtbl+0x204`; stores float3 points in the
  `+0x1fc` vector; if the height spread > `0x3999ecf4` (6.0 m) sets `+0x14 = 0x74`. Draw
  `FUN_39334ab0` (1190 B) at vtable `+0x04`, colour `0xAFFFFFFF` (white, alpha 0.69), plus two
  outline boxes; the whole draw is gated by `DAT_3a167e84->vtbl+8() > 0`.
* Kind 1 with desc `+0xd` == 0: `FUN_39332180` (124 B) does a simple 6 m spread check
  (`DAT_3999ecf4`) and commits the position.
* Other kinds: `FUN_39337490` (823 B) renders the 20 m grid (`FUN_39334740`, 619 B) and the
  connector boxes; `FUN_393322d0` (233 B) draws the garden/connector point grid (green/red).

### 2.3 Snapping and connectors

For the connector builders (tower/wall/stair/castle) every position/rotation variant ends with
`FUN_39338d40` (`0x39338d40`, 2047 B):

* gathers houses within 18 m (`_DAT_3999f928 = 18.0`) via `FUN_3932dbb0`;
* for each house iterates its connector records (`house+0x13c`, 0x40-byte records), transforms
  them into world space and keeps the nearest vertical distance;
* moves the candidate model to the found connector Z (`FUN_39325160`), sets `+0x220 = 1` when a
  connector was used, then `FUN_39334540` (height band);
* sets **0x75** (`connector_missed`) when the connector constraint fails and **0x76**
  (`not_connected_neighbor`) from `FUN_39336600` when the neighbour is not connected.

This is the 1.2 equivalent of 10.x `HouseBuilder::SnapToNearestConnector` (string `0x3A090BC0`).
The 1.2 HouseBuilder (kind 1) never calls it, so a normal house only snaps to the terrain height
from the mouse ray. `use builder snap`/`cd_builder_snap` do **not** exist in 1.2 (`strings.txt`).

### 2.4 Core function table (A)

`callers`/`callees` are address-only summaries; the full list is in [`inventory.md`](inventory.md).
`R/W` = state read/written; `P/S` = pure/stateful.

| function (VA size) | callers -> callees | R/W fields | P/S | differential |
|---|---|---|---|---|
| `FUN_39330ba0` (0x39330ba0 604) StartBuild | 394eaac0 -> 39017620,3932fb20,3908c810,ctors | manager+0x30/+0x2c/+0x24 | S | start live from item; compare class/vtable, temp unit, item ref |
| `FUN_3908c810` (29) kind | several -> 39897070 | unit+0xc (category) | P | scratch unit/desc category matrix |
| `FUN_39897070` (34) category predicate | 4 callers | arg | P | all categories |
| `FUN_3908c850` (23) dot flag | 3933a300,3933a380 -> 3963b390 | unit desc+0xd | P | scratch desc +0xd |
| `FUN_393304b0` (114) reset | 39330ba0 -> 3932f520,39360020 | manager lists, player | S | start twice, list/player stable |
| `FUN_3932fb20` (272) temp house | 39330ba0,393301a0 -> 39368fb0,3932fa00 | design, unit | S | one live start; compare unit model/design |
| `FUN_39330830` (869) neighbours | 39330ba0 | manager+0x3c.., world | S | live snapshot; compare fence list |
| `FUN_3932c900` (1090) connectors | 39330ba0 | same | S | live snapshot |
| `FUN_39336140` (324) HouseBuilder ctor | StartBuild, default ctor | object+0..0x220 | S | construct in scratch from live unit; field compare |
| `FUN_39336520/377d0/37800/37920` (202/34/34/121) ctors | StartBuild | object tail | S | same |
| `FUN_39332200` (208) left click | 393327e0,39332ad0 -> 39325f60,3932fe60 | state+0x0c/+0x10 | S | click matrix; compare state/error/packet |
| `FUN_39331760` (123) right click | vtable -> 39330120 | state | S | click matrix |
| `FUN_39332030` (202) per-frame | 3932a3b0 -> 3921ac90,394298c0,39016fe0 | state, errors | S | mouse/wheel matrix |
| `FUN_3933a300` (122) SetPos | vtable -> 39325160,393363b0 | +8..+0x14, plot | S | positions matrix, snapshot/restore |
| `FUN_3933a380` (124) Rotate | vtable -> 39326800,393363b0 | yaw, plot | S | deltas matrix |
| `FUN_393317e0..1810` (8 each) state setters | vtable | +0xc | S | trivial |
| `FUN_3933a400/3a5a0/3a6c0/3a740/3a7b0/3a040` (412/282/114/106/145/689) class SetPos/Rotate | vtables | transform, +0x220 | S | per-class placements |
| `FUN_393363b0` (280) validation | 14 callers -> 39333cb0..354c0 | +0x10/+0x14 | S | error matrix (section 3) |
| `FUN_39333cb0` (134) gather | 39335860,393363b0 | +8..+0x14, player | S | live vs scratch |
| `FUN_39333d40` (214) range | 393363b0 -> 3963b390 | player pos, +8..+0x10 | S | distances near/far |
| `FUN_39333e20` (573) area | 393363b0 -> 39897280 | category, pos | S | area/category matrix |
| `FUN_39334060` (799) dominion | 393363b0 -> world | pos, area | S | live points |
| `FUN_39334380` (218) water | 393363b0 -> water query | pos, desc+0x154 | S | water level matrix |
| `FUN_39335860` (2234) overlap | 393363b0 -> 39333cb0,393349e0 | OBB, houses/units | S | house/unit overlap |
| `FUN_393354c0` (566) neighbour diff | 393363b0 | list+0x210 | S | list compare |
| `FUN_393349e0` (203) OBB intersect | 39335860 | boxes | P (given boxes) | array matrix |
| `FUN_39334540` (502) height band | class SetPos | pos, terrain | S | height matrix |
| `FUN_39334460` (216) tint/error | 14 callers -> 3900c0f0,39327b50 | +0x10 | S | read material pointer/error text |
| `FUN_39337fb0` (3415) plot | 3933a300,3933a380,39337f.. | +0x1fc.., terrain | S | plot vector + 0x74 |
| `FUN_39334ab0` (1190) plot draw | vtable+0x04 | +0x1fc.. | S | draw calls/color; screenshot |
| `FUN_39332180` (124) simple plot | 3933a300,3933a380 -> 39326c10 | pos, 6 m | S | height matrix |
| `FUN_39338d40` (2047) connector snap | class SetPos variants | +0x13c, +0x220 | S | connector positions |
| `FUN_39337490` (823) render | vtable+0x30 | house list | S | render/plot compare |
| `FUN_39334740` (619) grid draw | 39337490 | pos | S | graphics |
| `FUN_39330120` (108) cancel | 6 callers | manager+0x30 | S | cancel matrix |
| `FUN_39325f60` (176) quote | 39332200 -> 397abc50 | builder+8/+0x21c | S | capture 0x56 |
| `FUN_3932fe60` (701) create | 39332200,39477cb0 -> 397b09d0 | builder+0x21c/+0x24 | S | capture 0x57 + 5 s |
| `FUN_397abc50` (45) quote ctor | 39325f60 | packet object | P | field bytes |
| `FUN_397b09d0` (94) create ctor | 3932fe60 | packet object | P | field bytes |
| `FUN_39016fe0` (252) wheel/elevation delta | 39332030,3903e020,393a9690 -> 39424580 | action axes + `0x39adac58` | S (reads input/cvar) | axis/cvar matrix, `client_wheel` |
| `FUN_39017410` (396) cursor/BUILDER_STEP | 3932a3b0,393a9830 -> 397a7b40,39424240 | builder+0xc/+0x10, `0x39adac5c` | S | state/error vs cursor image/event |
| `FUN_394298c0` (605) screen->terrain ray | 39332030,393a9690,393a4cc0 -> engine 3D | mouse x/y, terrain | S | window size + mouse matrix |
| `FUN_3921ac90` (6) mouse position | 39332030,3903e020,393a9690 | input | S (read) | mouse matrix |
| `FUN_393fe030` (197) hotkey mode | 390173b0,39017220 | 0x39ac9020 | S | mode on/off around state 2 |

See [`classes.md`](classes.md) for the vtable layout and the kind-specific method list, and
[`packets.md`](packets.md) for the packet field order.

---

## 3. Area B - construction steps / builder progress

The client does not have a per-house skill table. Construction actions are ordinary unit skills
from `SCUnitState`; the client only displays state and casts what the server put in the list.

| function (VA size) | role | callers/callees | P/S |
|---|---|---|---|
| `FUN_39327990` (0x39327990 327) | `SCHouseBuildProgress`: store `all` in `house+0x140`, `current` in `+0x144`; if current changed, update the model (`FUN_390b4ce0`), play `start_effect` (`FUN_39326770`), fire **0x103** | server handler -> 39360020, 390b4ce0, 39326770, 39424240 | S |
| `FUN_394785d0` (211) | `GetHouseBaseProgress` = `+0x140` | X2House registration -> 39460110 | S (read) |
| `FUN_394784f0` (217) | `GetHouseRemainProgress` = `+0x140 - +0x144` | same | S (read) |
| `FUN_3947b3b0` (1065) | `GetHouseConstructionStepInfo`: one row per `housing_build_steps` item (`FUN_39479460` adds `isLaborPower`, count, `needActions`, icon/step) | Lua X2House -> 3963b390, 39479460, 39421e20 | S |
| `FUN_393bcb90` (134) | Fill the dynamic action list from the controlled unit's skill vector (`FUN_3910a950`) plus doodad/slave skills | interaction indicator -> 390b3750, 3910a950 | S |
| `FUN_393b1de0` (202) | `ExecuteDynamicAction(i)` casts the i-th listed skill; refuses the caster's own slave and invalid target | Lua dynamic bar -> 393bc7e0/8c0, 393b3ec0/be280 | S |
| `FUN_393befd0` (442) | House interaction: 3 m surface gap (`sqrt(9.0)`), `CSRequestHouseTax` 0x5c, event **0x6d** with the category string | interaction -> 39093800/860, 397abcd0, 39424280 | S |
| `FUN_3932e210` (786) | `SCHouseTaxInfo`: due/time formatting, event **0xff** | server handler -> 394245c0, 39429b20, 390155a0 | S |

The Lua registration is `FUN_3947c030` (X2House table): `IsHouseUnderConstruction`
(`strings.txt:0x399de248`), `GetHouseBaseProgress` (`0x399de160`), `GetHouseRemainProgress`
(`0x399de178`), `GetHouseConstructionStepInfo` (`0x399de0e0`); the C getters above are what those
bindings call. Only 0x140/0x144 matter for "under construction": finished houses send 0/0, so
`current < all` is under construction; 0/0 is finished. This matches `re-housing.md` §1.1 and
`houses.rs:86`.

The material window is opened by the stock Lua on `HOUSE_INTERACTION_START`; it never casts a build
skill. F/G entry icons/skills come from the unit's received skill list, not from a client table
(`FUN_393bcb90`/`3910a950` have no `IsMyHouse` test). The port should keep the lists as data and
only port the cast/refusal rules (`FUN_393b1de0`).

---

## 4. Area C - decoration placement inside/around a house

### 4.1 Flow

1. Item use (task 6) -> `FUN_3932c120` (`0x3932c120`, 1305 B):
   * global check `FUN_39017620`, 1000 ms cooldown `DAT_39adcc24`, item/task object lookup
     `FUN_3932ac80`, house lookup (`FUN_39328630`) -> 199 = 0xc7 if none;
   * `FUN_39327c50` pre-checks (war/permission/limits/actability, see [`checks.md`](checks.md) §3);
   * target groundwork `FUN_3963b3d0`, `FUN_393b03b0` (load doodad model/ghost), rotation from
     the player + a direction basis, `FUN_3903bf60` (0x54 cursor) stored in `manager+0x34`;
   * `manager+0x28` gets the task item; `manager+0x2c` a new reference; `FUN_394e84e0`.
2. The cursor state machine `FUN_3903e020` (position/rotation) and the confirm `FUN_3903b9d0`
   are described in [`checks.md`](checks.md) §3; the position ray is `FUN_3903da00`, the wheel is
   the same `FUN_39016fe0` as the house builder (5 deg default), rotation revalidates through
   `FUN_3903c5a0`.
3. Validation `FUN_3903c5a0` (`0x3903c5a0`, 4339 B): surface flags -> 0x7f; garden -> 0x121;
   in/out of house -> 0x7e/0x122; wall box -> 0x121; primitive overlap -> 0x7d; 0 = valid.
4. Confirm `FUN_39327de0` (`0x39327de0`, 2119 B) fires `BUILDER_END`, checks the cursor error,
   sends `CSDecorateHouse` 0x58 (section 5.3), frees the item/cursor, cursor mode off.
5. Packet result: server spawns the bonded doodad; `FUN_39330270` fires `HOUSE_DECO_UPDATED`
   0x10E when the changed house is selected.

### 4.2 Rules/data

`housing_decorations` (1.2 SELECT `0x399fec78`) is the rule source: `actability_group_id`,
`actability_up`, `allow_mesh_on_garden`, `allow_on_ceiling`, `allow_on_floor`, `allow_on_wall`,
`allow_pivot_on_garden`, `deco_actability_group_id`, `doodad_id`. Per-house limits:
`housings.deco_limit`, `housing_deco_limit_elems`; counters in section 5.6. The 1.2 loader does
**not** have the 10.x `allow_garden_id`/`collision_by_proxy`/`vertical_aligned` columns.

Furniture already placed: no separate 1.2 client "move furniture" packet or builder was found; the
only path is pick up/consume the item and use it again (the same item-task-6 path), or a UI drag
that is server-side. See U7.

### 4.3 Core function table (C)

| function (VA size) | role | callers -> callees | P/S |
|---|---|---|---|
| `FUN_3932c120` (0x3932c120 1305) | deco start (pre-checks, ghost, cursor) | 394eaac0 -> 39017620,3932ac80,393b03b0,3903bf60 | S |
| `FUN_3932ac80` (98) | item -> deco design | 16 callers | S (read) |
| `FUN_39327c50` (388) | deco pre-checks (0x294/0x7c/0xb5/0x275/0xc8) | 3932c120 -> 390b3850,3963b390,39327bc0 | S |
| `FUN_39327bc0` (141) | actability group/count | 39327c50 -> 3963b3d0/350,39897400 | S |
| `FUN_393b03b0` (1423) | load doodad model / choose ghost class | 3932c120,393a1210,393b0940 | S |
| `FUN_393a5930` (853) | ghost ctor (design +4, parent +0x4c) | 393a7a10,393b03b0 path | S |
| `FUN_3903bf60` (892) | cursor ctor (0x54) | 3932c120 -> 3963b3d0,3932ee80 | S |
| `FUN_3903e020` (90) | cursor per-frame (position/wheel) | 3932a3b0 -> 3921ac90,39016fe0,3903c5a0 | S |
| `FUN_3903da00` (1565) | cursor position from ray; init transform/surface | 3903e020 -> 3963b3d0,3917f4c0,3903c5a0 | S |
| `FUN_3903c5a0` (4339) | deco validity (surface/garden/house/overlap) | 3903da00,3903e020 -> 3963b3d0,39326b40,3917f4c0 | S |
| `FUN_3903d6a0` (454) | apply transform + invalid material | 3903da00/3903e020 -> ghost vtable+0x3c/0x30 | S |
| `FUN_3903d870` (388) | draw preview box | 3932a3b0 -> 3903c2e0,3932dbb0 | S |
| `FUN_3903c2e0` (691) | preview corners | 3903d870,3903c5a0 | P (given transform) |
| `FUN_3903b9d0` (33) | confirm state machine | cursor vtable -> 39327de0 | S |
| `FUN_3903ba00` (25) | back/cancel | cursor vtable -> 39325210 | S |
| `FUN_3903b8a0` (130) | cleanup | 39325210,39330e20 (and the inline call in 39327de0) | S |
| `FUN_39327de0` (2119) | DecoratorOK: 0x58 + cleanup | 3903b9d0 -> 397ae590,3903b8a0,390173b0 | S |
| `FUN_397ae590` (81) | CSDecorateHouse ctor | 39327de0 | P |
| `FUN_39330270` (158) | HOUSE_DECO_UPDATED 0x10E | server -> 3956db10 | S |
| `FUN_394796b0` (102) | GetHouseDecoCount | X2House | S |
| `FUN_394797c0/79890/7a040/7a310` (194/295/708/446) | actability deco counters | X2House | S |
| `FUN_3903ba20` (841) | axis-angle/matrix | 3903da00,3903c5a0 | P (math) |

The 0x54 cursor layout, position/rotation transform and state values are in
[`classes.md`](classes.md) §3. The 0x58 packet fields are in [`packets.md`](packets.md) §3.

---

## 5. Area D - seeds/saplings and farm plots

### 5.1 Flow

1. Item use (task 9) -> permission checks (`FUN_39053360`/`FUN_390533f0`, errors 0x2ed/0x2ec) ->
   `FUN_393a9540` (`0x393a9540`, 312 B): `FUN_39017620`, guard `DAT_3a17a950` (2 s, set on cleanup), destroy an
   old builder, `FUN_393a7a10(item)` -> ghost (0x3b0, `FUN_393a5930`) -> `FUN_393a8470` creates the
   0x54 builder `DAT_3a17a948`; `DAT_3a5a4ea0 = 7`; cursor class 2 (`FUN_39017410(2,...)`).
2. `FUN_393a9690` (`0x393a9690`, 275 B) per frame: `FUN_393a8540`, `FUN_392aa2e0` (error 0x145),
   state 1: mouse position -> `FUN_393a6d50` + `FUN_394298c0` ray -> `FUN_393a1050`, error 0x145 if
   no hit else 0x118; state 2: `FUN_39016fe0` -> `FUN_393a7f20` rotate; then `FUN_393a8fa0`,
   `FUN_393a7300`, `FUN_393a7110`, marker drawing.
3. `FUN_393a8fa0` (`0x393a8fa0`, 927 B) validates: permission/area (0x114), doodad group overlap
   (0x114), water/terrain (0x114), distance > 30 m (0x9a), farm permission (0x2ed), doodad overlap
   (0x115), unit overlap (0x116); success 0x118.
4. `FUN_393a9340` (`0x393a9340`, 392 B) confirm: `FUN_3915b270()==6` -> 0xcf + cancel; error !=
   0x118 -> show and keep; otherwise build/send `CSCreateDoodad` 0xE6 (section 5.4), set
   `builder+0x2c = 1`.
5. `FUN_393a4cc0` (`0x393a4cc0`, 1023 B) places `guide_leaf_r/g/y.cgf` markers around the spot:
   yellow neutral, green valid, red invalid. Count `ghost+0x900`, list `ghost+0x38c..0x390`.

### 5.2 Core function table (D)

| function (VA size) | role | callers -> callees | P/S |
|---|---|---|---|
| `FUN_393a9540` (0x393a9540 312) | start seed builder | 394eaac0 -> 39017620,393a7a10,393a8470 | S |
| `FUN_393a7a10` (941) | seed ghost (doodad) + cursor init | 3939ffa0,393a9540 -> 3963b290,393a5930,393a1210 | S |
| `FUN_393a8470` (202) | seed builder ctor (vtable 0x399d2474) | 393a9540 -> 394e84e0,3932f0d0 | S |
| `FUN_393a9690` (275) | per-frame position/rotation/validation | 393a9830 -> 3921ac90,394298c0,39016fe0,393a8fa0 | S |
| `FUN_393a8fa0` (927) | validation (0x114/9a/2ed/115/116/118) | 393a9690 -> 393a7170,393a7720,393a85a0,393a4cc0 | S |
| `FUN_393a7170` (188) | farm/area permission | 393a8fa0 -> 39328630,39326b40,39325930 | S |
| `FUN_393a7720` (184) | height difference > 6 m -> 0x117 | 393a8fa0 -> 393a73a0,393a1050 | S |
| `FUN_393a9340` (392) | CSCreateDoodad 0xE6 + finish | 393a9810 -> 3915b270,397ac850 | S |
| `FUN_393a4cc0` (1023) | leaf markers | 393354c0,393a7300,393a85a0,393a8fa0 -> 39328630,39326b40 | S |
| `FUN_393a6d40` (6) | builder getter | 5 callers | P (read global) |
| `FUN_393a94f0` (55) | cancel seed builder | 3 callers | S |
| `FUN_393a89d0` (505) | seed builder destructor/cleanup | 4 callers | S |
| `FUN_393a9830` (122) | cursor 2 update/render | server/input -> 393a9690,39017410 | S |
| `FUN_397ac850` (77) | CSCreateDoodad ctor | 393a9340 | P |
| `FUN_3961dd70` (85) | position conversion | 33 callers | P (given origin) |
| `FUN_3908c870` (72) | player position | 26 callers | S (read) |

The builder/ghost layout and the leaf marker data are in [`classes.md`](classes.md) §4. The
`CSCreateDoodad` fields are in [`packets.md`](packets.md) §4.

---

## 6. Shared infrastructure

### 6.1 Error keys

`FUN_3956b860` (`0x3956b860`, 5512 B) is a pure code->key switch (783 cases). The housing/deco/
doodad subset and where each code is raised are in [`checks.md`](checks.md) §1. This is the first
function to port and diff (one table test covers all cases).

### 6.2 Position conversion

`FUN_3961dd70` (`0x3961dd70`, 85 B) is used by 0x56/0x57/0xE6 (and the seed ghost position).
`CSDecorateHouse` 0x58 does **not** use it (house-local floats). `round(f * 2^44) + origin`, z f32, origin at `GameClient+0x118`; scale
`0x399f9a80`; equals aa-net `coord_to_long` (`wire.rs:283`). Full proof/recipe in
[`constants.md`](constants.md) §2.

### 6.3 Packets and events

| packet | opcode | writer | ctor | reply/event |
|---|---|---|---|---|
| `CSConstructHouseTax` | 0x56 | `FUN_39325f60` | `FUN_397abc50` | `SCConstructHouseTax` -> `FUN_3932e530` -> 0x100 |
| `CSCreateHouse` | 0x57 | `FUN_3932fe60` | `FUN_397b09d0` | `SCMyHouse` -> builder end |
| `CSDecorateHouse` | 0x58 | `FUN_39327de0` | `FUN_397ae590` | `SCDoodadCreated` -> 0x10E |
| `CSCreateDoodad` | 0xE6 | `FUN_393a9340` | `FUN_397ac850` | server doodad spawn |
| `CSRequestHouseTax` | 0x5C | `FUN_393befd0` | `FUN_397abcd0` | `SCHouseTaxInfo` -> 0xFF |

Details and the 5 s create / 1 s deco / 2 s seed limits: [`packets.md`](packets.md). Events/ids are in
[`packets.md`](packets.md) §7.

### 6.4 Cvars/globals

`builder_rotate_angle` (default 5.0) is the only cvar read by the placement code. The other
constants/globals (`DAT_3a167e84` debug gate, `DAT_3a167ea0` rate limit, `DAT_3a17a948` doodad
builder, cursor tables, origin) are in [`constants.md`](constants.md) §4-§5.

---

## 7. Proposed port order (testable leaves first)

Phase P0 - pure data/leaves (no world):

1. `FUN_3956b860` error table (5512 B) - exhaustively diff 0..0x400.
2. `FUN_39897070`, `FUN_3908c810`, `FUN_3908c850` category/kind/dot flag.
3. `FUN_3961dd70` position conversion (with the live origin) and the aa-net `coord_to_long` check.
4. Math helpers: `FUN_3903ba20` (axis-angle), `FUN_393349e0` (OBB intersect), `FUN_393287c0`
   (box overlap), `FUN_39328630`/`FUN_39327830` (nearest house/distance), `FUN_39337ef0`
   (vector/bounds helper), `FUN_39336290`/`393365f0`/`39337830` stubs (trivial).

Phase P1 - builder primitives:

5. State setters `0x393317e0..1810`, state dispatch `FUN_39332200`, `FUN_39331760`,
   `FUN_39332030`, `FUN_39330120`.
6. Wheel `FUN_39016fe0` + `builder_rotate_angle`; elevation `-0.02`.
7. `FUN_3933a300`/`FUN_3933a380` SetPos/Rotate (on a live temp unit) and the class variants
   (`0x3933a5a0/6c0/740/7b0/3a400/3a040/37840/378b0/37320/373a0/37450`).
8. Validation leaves in order: `39333d40`, `39333e20`, `39334060`, `39334380`, `39335860`,
   `393354c0`, `39334540`, `39332180`, `39337fb0`; then `393363b0` as the integration.
9. `FUN_39334460` tint/error text, `FUN_393364e0` reset.
10. Connector snap `39338d40` and the class `39336600` local placement.

Phase P2 - house builder lifecycle:

11. `FUN_39330ba0` StartBuild + `393304b0`/`3932fb20`/`3932fa00`/`3932f6b0`/`3932d0f0`.
12. Neighbour/fence init `39330830`/`3932c900`/`3932ead0` (large; diff by lists).
13. `39332200`/`39331760`/`39332030` as the completed state machine; quote/create:
    `39325f60`/`397abc50`, `3932fe60`/`397b09d0`, `3932f960`, rate limit.
14. `3932e530` quote handler, `3932a3b0` manager pump, `39017410` cursor/BUILDER_STEP.

Phase P3 - decoration:

15. `3903bf60` cursor ctor, `3903e020`/`3903da00` position, `3903c5a0` validation,
    `3903d6a0`/`3903d870`/`3903c2e0` preview, `3903b9d0`/`3903ba00`/`3903b8a0`.
16. `3932c120` start + `39327c50`/`39327bc0`; `393b03b0`/`393a5930` ghost; `39327de0` +
    `397ae590` packet.
17. Counters/getters `394796b0`/`394797c0`/`39479890`/`3947a040`/`3947a310`, `39330270`.

Phase P4 - seeds/farm:

18. `393a9540`/`393a7a10`/`393a8470`/`393a89d0`/`393a94f0` builder lifecycle.
19. `393a9690`/`393a8fa0`/`393a7170`/`393a7720` validation and `393a4cc0` markers.
20. `393a9340` + `397ac850` packet.

Phase P5 - construction display (B):

21. `39327990`, `394785d0`/`394784f0`, `3947b3b0`/`39479460`, `393bcb90`/`3910a950`/`393b1de0`,
    `393befd0`, `3932e210`.

Rationale: P0 is callable with scratch buffers; P1 needs only a live temp unit/world snapshot; P2
adds the control flow; P3/P4 are largely the same cursor/validation machinery on a different object;
P5 is display/casting and can land last. The per-function diff recipes are in
[`difftest.md`](difftest.md); a row without its own recipe uses the group recipe in that file and
the calling-convention table in §1 there.

---

## 8. Unknowns (updated after the phase A port, 2026-10-24)

Resolved or narrowed by the phase A live capture (`house-golden` lane,
`rust/tools/capture_housing_goldens.py`; 1,027 error-table cases, 20+ cases per
function):

| # | status | finding |
|---|---|---|
| U1 | **resolved** | `builder_rotate_angle` is live `5.0f` (`0x40a00000`); `FUN_39016fe0` is bit-exact for 22 injected axis sets. The axis is the input action value (one wheel notch = 1.0 by the binding), so one notch is `1 deg * 5.0 = 5 deg`; the cvar path is now covered by `goldens/u3.json`. |
| U2 | **resolved (construction)** | The temp unit is created at the origin with a zero transform; the class `SetPos` `FUN_3933a300` only copies position (`FUN_39325160`), and `Rotate` `FUN_3933a380` reads the unit yaw and adds the wheel delta (`FUN_39326800`). No code writes a non-zero initial yaw, so the initial yaw is `0.0`; the first click only changes state. |
| U3 | **resolved** | Live terrain object (`gEnv+0x2c`) `vtbl+0x20c` returns `2.0` (grid step) and `vtbl+0x210` returns `8192` (grid extent); `vtbl+0x1fc(x,y,0)` returns `168.5635` at the player. Recorded in `goldens/u3.json`; `points::TERRAIN_STEP`/`TERRAIN_GRID`. |
| U4 | **corrected** | `FUN_3908c850` does **not** read a `housings` desc: it resolves `unit+8` (a unit-template id) through `FUN_3963b390` and returns the template byte `+0xd`. The DB column behind that byte is still unknown. The builder-class selector `FUN_3908c810` reads `unit+0xc` (the housing-category mirror), as mapped. |
| U5 | open | full `housings`/`housing_decorations` offsets still not mapped. |
| U6b | **partly resolved** | The packet origin is `GameClient+0x118` (x i64, y i64, z f32); `FUN_3961dd70` is bit-exact for 24 live cases. Whether the builder temporarily makes the temp housing the view target is still `[I]`. |
| U6 | **resolved for quote/create** | `FUN_39325f60` reads `manager+0x30` builder, `builder+0x10` error, `unit+8` design and converts the player position; `FUN_3932fe60` reads `manager+0x24` item pair, `builder+0x21c` money, `DAT_3a167ebc` ht, the unit yaw and the same position, then enforces the 5 s clock `DAT_3a167ea0`. 25 live cases each. The decoration parent source is still open. |
| U7 | **resolved** | No 1.2 furniture-move packet or builder exists. Moving/rotating a placed decoration is the ordinary item-use path: pick the bonded doodad up, use the item again, and the same 0x54 cursor (`FUN_3903bf60`/`FUN_3903c5a0`/`FUN_39327de0`) places it; rotation is the same wheel delta. `decor.rs` documents and models this. |
| U8 | open | per-kind helper roles are still `[I]`. |
| U9 | **partly resolved** | `DAT_3a167e84` points at a debug object with vtable `0x399b1f10`; its second method returns 8 (`> 0`), so the gate is on, but the `housing area : ...` debug line produced no visible console/file output. |
| U10 | **resolved for the area step** | `FUN_39333e20` has three modes (`FUN_39325190`): 0 = area pointer + category, 1 = additional membership test, 2 = the `FUN_39331f70` garden mesh/footprint test. The live category matrix (161 cases) fixes the mode-0/1/2 outcomes; the garden-radius compare in `FUN_3903c5a0` is `> 0.0` (live read `[0x3999c484] = 0.0`), not `> 3`. |
| U12 | **partly resolved** | The client farm checks are `FUN_39053360` (faction/dominion/area/type) and `FUN_390533f0` (nation owner); `seeds.rs` models both. Real farm ownership remains server-side, so the client model takes the owner/area results as inputs. |
| U11 | **partly resolved** | The live cvar object returns `5.0f`; the registration function is still not identified. |
| U12 | open | seed/farm permission semantics still come from the server. |
| U13 | **resolved** | 1.2 has one default class; `FUN_398970e0` marks categories 2-5 and 14 as castle/keep-like for the `0x70 -> 0x215` reset in `FUN_39332200` (verified live for 2/3/4/5/14). |
| U14 | **resolved** | The area check uses the **player's zone-local** position: `FUN_39333cb0` copies `FUN_390b3850` (`unit+0x10`) into `builder+8`; `FUN_39333e20` passes it to the engine area query (`gEnv+0x44 -> vtbl+0xc4`) and `FUN_39897280`. Live: local `(4283.3,4925.7)` + `GameClient+0x118` origin `(15360,19456)` = world `(19643.3,24381.8)`. The `housing_area.xml` shape is `local = cell*4096 + Entity.Pos + Point`; with the cell term, the live query returns 183/184/185 for all 28 recorded points, and without it returns 0. The previous 0x70 was the missing `cell * 4096` (all three entities have `cellX/cellY = 1`). |

### Corrections from the live capture

* `CSCreateHouse` object fields: `+0x38` is **money**, `+0x3c` is **ht** (the
  earlier packets.md table had them reversed). The wire order is money then ht.
* `CSCreateHouse` has two untouched padding dwords in the object (`+0x24` and
  `+0x2c`); `CSCreateDoodad` has one (`+0x24`). The wire bodies do not contain
  them.
* `CSDecorateHouse` stores `house_tl` as a dword at `+0xc` but the wire writes a
  `u16`; the parent id is stored as a dword and written as a 3-byte `bc` id.

## 10. Phase A port record

Ported in `crates/aa-client/src/port/housing/` with live goldens in
`goldens/` (bit-exact unless noted):

| function | address | live cases | result |
|---|---|---:|---|
| `FUN_3956b860` error key | `0x3956b860` | 1,027 | exact (all keys) |
| `FUN_39897070` house category | `0x39897070` | 21 | exact |
| `FUN_3908c810` builder kind | `0x3908c810` | 21 | exact |
| `FUN_3908c850` dot flag | `0x3908c850` | 23 | exact |
| `FUN_3961dd70` position conversion | `0x3961dd70` | 24 | exact i64/f32 |
| `FUN_3903ba20` rotation matrix | `0x3903ba20` | 20 | exact f32 bits (including the `rsqrtss` branch) |
| `FUN_39327830` AABB distance | `0x39327830` | 24 | exact f64 bits |
| `FUN_393317e0..1810` state setters | `0x393317e0..1810` | 20 each | exact |
| `FUN_39336290/365f0/37830` elevation stubs | `0x39336290/365f0/37830` | 20 each | exact (no writes) |
| `FUN_39016fe0` wheel delta | `0x39016fe0` | 22 | exact f32 bits; cvar live `5.0` |
| `FUN_397abc50` 0x56 ctor | `0x397abc50` | 20 | exact object fields |
| `FUN_397b09d0` 0x57 ctor | `0x397b09d0` | 20 | exact object fields |
| `FUN_397ae590` 0x58 ctor | `0x397ae590` | 20 | exact object fields |
| `FUN_397ac850` 0xE6 ctor | `0x397ac850` | 20 | exact object fields |

Closed in phase B (commit `9172f7ac`): the collision dispatch
`FUN_393349e0` (48 live cases), its bodies `FUN_390318f0` / `FUN_39332c10` /
`FUN_39331f30` / `FUN_39331f70` (24 live cases each) and the vector push
`FUN_39337ef0` (24 live cases).  Tag 1 records are OBBs (`+8` position, `+0x18`
basis/extents/centre, `+0x54` footprint AABB); tag 2 records are segment meshes
(`+4` shape with the row/column count at `shape+0x168`, `+0x6c` an 11-slot x
count grid of 24-byte segments).  Phase B also ported the state machine
`FUN_39332200`/`FUN_39331760`/`FUN_39332030` and the quote/create/cancel
decisions `FUN_39325f60`/`FUN_3932fe60`/`FUN_39330120` (commit `265aee27`;
32/25/24/25/25/20 live cases).

Closed in phase B (commit `447c8e7d`): the house-map lookups
`FUN_39328630` (containing house, else nearest within the radius) and
`FUN_393287c0` (point box partially overlapping a plot; a fully contained box
does not match) with 24 live query cases each.  The live map (`manager+8` ->
`_Myhead`, node value at `+0x10`, in-order walk) held one guard tower
(`garden_radius` 0, skipped by `FUN_39326980`) and one inserted player
farmhouse (template 171, `garden_radius` 11) so the walk and distance paths
were exercised.

## 10b. Phase B port record (2026-10-25)

Live goldens in `goldens/` (recorded through
`rust/tools/capture_housing_phaseb.py` and `capture_housing_lifecycle.py`):

| function | address | live cases | result |
|---|---|---|---|
| `FUN_390318f0` OBB/OBB SAT | `0x390318f0` | 24 direct + 24 in dispatch | exact |
| `FUN_393349e0` collision dispatch | `0x393349e0` | 48 | exact |
| `FUN_39332c10` AABB/OBB SAT | `0x39332c10` | 24 | exact |
| `FUN_39331f30` segment/AABB footprint | `0x39331f30` | 24 | exact |
| `FUN_39331f70` mesh walk | `0x39331f70` | 24 | exact |
| `FUN_39337ef0` float3 vector push | `0x39337ef0` | 24 | exact |
| `FUN_39332200` left click | `0x39332200` | 32 | exact (dispatch + castle reset) |
| `FUN_39331760` right click | `0x39331760` | 25 | exact |
| `FUN_39332030` per frame | `0x39332030` | 24 | exact (off-world + state 4; over-world branches by inspection) |
| `FUN_39325f60` quote 0x56 | `0x39325f60` | 25 | exact (24 guards + one send) |
| `FUN_3932fe60` create 0x57 | `0x3932fe60` | 25 | exact (guards, one send, one rate-limit refusal) |
| `FUN_39330120` cancel | `0x39330120` | 20 | exact effects |
| `FUN_39328630` map find | `0x39328630` | 24 | exact |
| `FUN_393287c0` map overlap | `0x393287c0` | 24 | exact (partial-overlap rule matches the live assembly) |

The per-frame over-world branches (mouse ray, wheel, elevation) could not be
exercised hidden: `FUN_392aa2e0` stayed false in the hidden client, so states
1-3 always take the `0x70` path. State 4 revalidates as mapped. The hidden
client also made the OBB/mesh/map data stable enough for all 24 collision
cases per body.

## 10c. Phase C port record (2026-10-25)

Live goldens in `goldens/areas_204.json` (recorded with
`rust/tools/capture_housing_p3p4.py` in the `house-golden` lane; the script
parses the zone 204 `housing_area.xml` and calls only `FUN_39333e20` with
scratch contexts).  The direct `FUN_39897280` call was attempted but the
hidden client raises an access violation when it is entered directly (the
engine caller supplies state the function expects), so its rule table is
covered through the 161 `FUN_39333e20` cases instead.

| function | address | live cases | result |
|---|---|---:|---|
| `FUN_39333e20` area/category check | `0x39333e20` | 161 matrix + 24 position | exact (`goldens/areas_204.json`), including modes 0/1/2 |
| `FUN_39897280` housing-group rule | `0x39897280` | 161 (through 39333e20) | exact model; direct entry unstable |
| `FUN_3903c5a0` decoration validation | `0x3903c5a0` | 0 live (needs live ghost + engine ray) | transcribed decision order; pure tests |
| `FUN_39327c50` deco pre-checks | `0x39327c50` | 0 live | transcribed; pure tests |
| `FUN_3903bf60` deco cursor ctor | `0x3903bf60` | 0 live | state model; yaw from the ghost basis |
| `FUN_39327de0` deco confirm | `0x39327de0` | 0 live | guard from the 0x39327de0 disassembly; 0x58 ctor already exact |
| `FUN_393a9540` seed start | `0x393a9540` | 0 live | transcribed |
| `FUN_393a8fa0` seed checks | `0x393a8fa0` | 0 live | transcribed order (`0x114/0x117/0x9a/0x115/0x116/0x2ed/0x118`) |
| `FUN_393a7720` seed height | `0x393a7720` | 0 live | transcribed (`> 6.0 m -> 0x117`) |
| `FUN_393a7170` farm/area | `0x393a7170` | 0 live | transcribed |
| `FUN_393a9340` seed confirm | `0x393a9340` | 0 live | transcribed; 0xE6 ctor already exact |
| `FUN_39053360`/`FUN_390533f0` permissions | `0x39053360`/`0x390533f0` | 0 live | transcribed client side (U12) |
| `FUN_3903b9d0`/`ba00`/`b8a0` cursor state/cleanup | `0x3903b9d0`/`0x3903ba00`/`0x3903b8a0` | 0 live | transcribed from the calls/disassembly |

The zero-live rows above are superseded by the phase D record in §10d:
the cursor ctor, pre-checks, validation, both confirms, the seed start/checks,
height and farm/area now have live goldens.  U1/U2/U3/U14 were re-read live in
the same session: `builder_rotate_angle =
5.0`, initial yaw `0` by construction, terrain grid step `2.0` / extent
`8192`, and the area transform `local = cell*4096 + Pos`.  The live origin
was `(15360, 19456)` in zone 204 at the player (world `19643.3, 24381.8`).
No CRYENGINE source was used.

## 10d. Phase D port record (2026-10-25)

Live goldens in `goldens/phase_d.json` (recorded with
`rust/tools/capture_housing_phase_d.py` in the `house-golden` lane; hidden
client, no OS input).  The character was placed at the zone-204 test farm
(house 171, garden radius 11) with GM item grants; scratch objects were used
for the constructors and temporary code patches only for branches the live
data cannot reach.  Every patch was restored.

| function | address | live cases | result |
|---|---|---:|---|
| `FUN_3903bf60` deco cursor ctor | `0x3903bf60` | 24 | exact; `+0x48` (surface) is **1** after the ctor (the phase-C model had 0), state 1, error 0, yaw from the ghost basis, `+0x4c` = creation time |
| `FUN_39327c50` deco pre-checks | `0x39327c50` | 27 | exact, every branch: 2 / 0x294 / 0x12 / 0x7c / 0xb5 / 0x275 / 0xc8 / success |
| `FUN_3903c5a0` deco validation | `0x3903c5a0` | 50 | exact; 0x7f / 0x121 (garden and wall-box) / 0x7e / 0x122 / 0x7d / 0 |
| `FUN_39327de0` deco confirm | `0x39327de0` | 20 | exact: cursor error is checked first (report + keep cursor), missing item pair -> 0xcf, valid sends 0x58 and frees the cursor |
| `FUN_393a9540` seed start | `0x393a9540` | 20 | first use starts; a second use while a builder exists is refused by the dispatcher's global check (**0xce**) before the 2 s guard, so 0x163 was not reachable hidden |
| `FUN_393a8fa0` seed checks | `0x393a8fa0` | 38 | exact: 0x114 / 0x115 / 0x116 / 0x9a / 0x118 |
| `FUN_393a7720` seed height | `0x393a7720` | 20 | exact: strict `box_max - box_min > 6.0` -> 0x117 |
| `FUN_393a7170` farm/area | `0x393a7170` | 20 | exact: permission/garden and 0x114 for a farm target in the garden |
| `FUN_393a9340` seed confirm | `0x393a9340` | 20 | exact: valid sends 0xE6, consumes the item and frees the builder; error -> report + keep; missing item pair / world state 6 -> 0xcf + free |

Corrections this phase made to the phase-C transcription:

1. `validate_deco` uses `allow_on_wall` for the inside check (not the pivot
   flag), `allow_mesh_on_garden | allow_on_wall | allow_on_garden` for the
   outside check, and the wall-box test reads the **mesh** flag (`desc+8`).
   The live desc byte layout is `+8 mesh, +9 ceiling, +a floor, +b wall,
   +c pivot`.
2. `validate_seed`'s height test is **not terminal**: `FUN_393a7720` writes
   0x117 and the function continues through range/house/doodad/unit checks; a
   later failure overwrites 0x117.  When the spread is <= 6 m the original
   snaps the ghost to the box minimum before those checks.
3. `confirm_deco` checks the cursor error before the manager item pair.
4. The seed start's 2 s guard (`DAT_3a17a950`) is behind the dispatcher's
   global can-build check; live item use reports 0xce first.

Controlled inputs (documented in the JSON): pre-check 2/0x294/0x275 used
short-lived returns stubs for `FUN_39360020`/`FUN_392f4000`/the actability
group; height 0x117 used a stub for the box-height leaf because the live tree
ghost spread is below 6 m; seed confirm's world-state-6 case stubbed
`FUN_3915b270`.  No original code was left modified and no OS-level input was
used.

## 10e. Phase E port record (2026-10-25, `port/housing-e`)

The remaining guessed placement pieces were ported and wired in
`crates/aa-client/src/port/housing/{ray,fence,start,routing}.rs` and
`ingame/placement.rs`. The 1.2 client was driven through `aadriver`; the
capture loops used the lane's original client only.

| function | address | live cases | result |
|---|---|---:|---|
| `FUN_394298c0` placement ray | `0x394298c0` | 24 | exact; view/proj/viewport captured, direction < 2e-3 and 200 m endpoint < 0.5 m (`goldens/phase_e_ray.json`) |
| `FUN_39897170` plot half-grid | `0x39897170` | 29 | exact, every boundary; live padding `_DAT_3999edbc` is **3.0**, so 10.5 m -> 6, and `>= 12` -> 0 (`goldens/phase_e_plot.json`) |
| `FUN_39326980` plot box | `0x39326980` | call read | decompile transcribed; the live call on the temp unit returned box `[-32,-32,-4.235,16,16,11.90]`, i.e. a 48 m side although the unit's `+0x168` read 6. The min half used 6 and the max behaved as if 12; unresolved, so the box/fence functions stay decompile-proven only. |
| `FUN_3932ead0` `InitFences` / `FUN_3932e890` stone | `0x3932ead0` / `0x3932e890` | visual | decompile transcribed; the house shot spawns **88** stones on the 24 m border inset by `housings.alley` (1.0), frozen at builder start. `XLRandom` matches the xlcommon decompile; the stone yaw is the half-angle fed to cos/sin, i.e. a full `2*pi` turn. |
| invalid material (`FUN_39334460` / `FUN_39327b50` / `FUN_3903d6a0` / `FUN_393a7110`) | — | visual | the seed ghost turns flat `housing_build_invalid` red on error 0x115, the deco cup turns red on 0x7d, and the house shot shows red border stones on error 0x71 |
| `FUN_39017620` start gate | `0x39017620` | 20 | returns exact: 10 ready calls -> 1, 10 calls after `X2Item:Use(21165,0)` -> 0 (`goldens/phase_e_start.json`); codes 0xce/0x163/0x14e/0x14c/0xde stay decompile-proven. |
| `FUN_394eaac0` item routing | `0x394eaac0` | 24 | dispatcher returns exact for scratch item blocks: 5+21165, 6+8229, 9+13724 are claimed; null designs fall through; non-housing use types return 0 (`goldens/phase_e_routing.json`). Per-builder codes 199/0x77/0x2ed/0x2ec/0x114 remain decompile-only. |
| `FUN_39330ba0` StartBuild / `FUN_3932fb20` temp unit | `0x39330ba0` / `0x3932fb20` | unit read | live temp unit: `+0x158 = 11.0` (garden), `+0x164 = 1.0` (alley), `+0x168 = 6` (half-grid), fence vector at `+0x16c/+0x170`; design 171, category 18, model 1082. |

Live-verified constants: `0x3999ddc8 = 50.0` (deco cursor step/scale),
`0x3999edbc = 3.0` (plot padding), `0x3999cffc = 2.0`, `0x399a26e4 = 0.25`,
`0x3999c484 = 0.0`, `0x399a2f9c = 2*pi`, `0x3999c47c = 0.5`, seed uses
`0x399c66a8 = 500`, house uses `0x399a319c = 200`.

Wiring: `placement.rs` uses the ported ray for house/seed cursors, the ported
box/border for the plot stones, the ported invalid flag for ghost material,
and refuses a second ported builder with 0x163. The HUD path routes item use
through `routing::builder_for_tables`; the scripted `seed_place` command uses
the same ported seed session. `--legacy-housing` still selects the guessed path.

## 9. Acceptance / verification notes

* All 197 functions in scope are listed in [`inventory.md`](inventory.md) with VA, size (funcs.csv
  or measured), callers, callees, role and state/purity. Sizes sum to 84,374 bytes.
* The primary path (A+C+D+B, excluding UI getters) is ~73 KB. The 1.2 decompile was the only
  behavioural source; 10.x supplied names only (marked **[R10]**).
* Constants used by the port were read from the `.data` dump (`x2game_unpacked_data.dll`), not
  inferred from traces: wheel (5.0/1 deg/0.01/pi/12), elevation (-0.02), spread (6.0), range
  (+30), doodad (900), create limit (5000), cooldown (1000), position (2^44).
* The live original was run for the phase A and phase B ports (hidden 1.2 client through
  `aadriver`; see the two capture scripts in the `house-golden` lane). No OS-level input was
  used. The over-world per-frame branches remain unexercised hidden, but U14 is now resolved and the
  area/category step has 161 live cases.
* No files outside `docs/research/port-housing/` were changed. No CRYENGINE source was consulted
  or quoted.

### Follow-ups for the orchestrator

1. Review the addresses in [`inventory.md`](inventory.md) against `funcs.csv`; the five manual
   sizes (`0x39327de0` etc.) are the only non-`funcs.csv` entries.
2. Approve the port order (P0..P5) and the scope split: the maintain/sale/tax window functions in
   group U are mapped only for reuse, not proposed for this port.
3. Add the send-hook (`0x3ac1058c`, opcodes 0x56/0x57/0x58/0xE6) to `aadriver` before the packet
   tests; the rest of the diff harness needs only `call`/`read`/`write`.

---

## 11. In-game wiring map (2026-10-25, `wire/housing-a`)

How the live Bevy placement code maps onto the port. The in-game path is
`crates/aa-client/src/ingame/placement.rs` (the systems behind the HUD's
`PlantPressed` / `HousePressed` / `DecorationPressed` messages); the older
`ingame/housing/placement.rs` builder and `ground_target/plant.rs` are not on
that HUD route and stay on the guessed code. `--legacy-housing` keeps every
guessed decision below.

| guessed in `ingame/placement.rs` | port replacement | notes |
|---|---|---|
| `check_house` (housing polygon, category, square overlap, water) | `port::housing::areas::area_check` + `AreaWorld` adapter; `port::housing::collide::shapes_overlap` for OBB dispatch | area calls keep the world-frame polygons (`houses::HousingAreas`, U14: `(zone origin + cell) * 1024 + Pos`); the packet-origin subtraction cancels |
| `check` (seed range/water/surface/footprint) | `port::housing::seeds::validate_seed` + `SeedWorld` adapter (`height_check`, farm/area) | adapter binds collision/terrain, `Doodads`, `Units`, `Farms`, house plots |
| `start_deco_session` house lookup, limits, state checks | `port::housing::decor::start_deco` + `precheck_deco`; `port::housing::mapwalk::find_house` | `housing_decorations` + `housings.deco_limit` / `absolute_deco_limit` feed `DecoDesc` |
| `check_deco` + `deco_limit_error` + `deco_counts` | `port::housing::decor::validate_deco` (+ `surface_flag`, `cursor_frame`) | house ray, wall box and primitive overlap stay engine inputs |
| `hit_under` / `deco_orientation` | kept (world/engine input to the cursor) | the port does not own the ray |
| `track` house branch | `port::housing::lifecycle::frame_action` (`SetPositionFromMouse` / `Rotate` / `Revalidate` / `NotOverWorld` 0x70) | state 1/2/4 |
| `Session::step_back` (house) | `port::housing::lifecycle::right_click` | `Cancel` / `SetPosition` / `SetRotation` |
| `Session::step_back` / `cursor_back` (deco) | `port::housing::decor::cursor_back`, `cursor_confirm` | state 1/2 |
| house left click (`rotating` -> quote) | `port::housing::lifecycle::left_click` + `quote` | kind from `category::builder_kind` |
| `Session::tax_request` | `port::housing::lifecycle::quote` + `packets::ConstructHouseTax` (0x56) | `QuoteRefusal` codes use `error::error_key` |
| `Session::create_house` / `Placement::send_create` / `create_refused` | `port::housing::lifecycle::create` + `packets::CreateHouse` (0x57) | five-second `CREATE_RATE_LIMIT_MS` comes from the port |
| `Session::request` (seed) | `port::housing::seeds::confirm_seed` + `packets::CreateDoodad` (0xE6) | `SEED_VALID` (`0x118`) is the send guard |
| `Session::decorate_request` | `port::housing::decor::confirm_deco` + `packets::DecorateHouse` (0x58) | cursor error is checked before the item pair |
| `wheel_turn` / `ROT_STEP*` | `port::housing::builder::rotation_delta` (`RotationAxes`, live `rotate_angle = 5.0`) | guessed plain step was 10 deg; port is 5 deg (U1/U11) |
| `house_error_key` / `deco_error_key` / `error_message` | `port::housing::error::error_key` | one table for 0x56/0x57/0x58/0xE6 refusals |
| `builder_step` strings | `port::housing::builder::{STATE_POSITION..}` | `BUILDER_STEP`/`BUILDER_END` glue stays |
| `houses::HousingAreas` parser (validation) | port area adapter | kept for the yellow posts/plot visuals; the port algorithm decides validity |

Kept guessed (no ported function yet): the house-builder *start* (`FUN_39330ba0`
and the neighbour/fence setup are P2 items 11-12), the ghost/prefab streaming
and all visuals, the engine mouse/camera rays, the HUD item routing, and the
gold-shop/start-window plumbing. Decoration/seed *world* inputs (terrain hit,
model boxes, farm ownership) are adapters, not new rules.

### Live check (2026-10-25)

Headless `--offscreen-shot` clients against the `house-golden` lane, no OS input;
the scripted `house_place` / `deco_place` / `plant` helpers drive the same
`placement.rs` systems as the HUD and `--legacy-housing` keeps the guessed path.
The ported and legacy runs sent the same bodies; only the spots, item instances,
quote and wheel delta differ:

| step | ported | `--legacy-housing` | server |
|---|---|---|---|
| house `0x56`/`0x57` | zone 179, design 171 at `(15318.53, 14632.33)`, wheel `5 deg` (`z_rot -1.4835`) | design 171 at `(15338.53, 14602.33)`, wheel `10 deg` (`z_rot -2.9671`) | `SCMyHouse`, house rows 16/17 (client tl ids 17/18); the zone-204 attempt was refused with 340 (undominated province), as the client has no ported dominion pre-check |
| decoration `0x58` | design 62, house tl 16, local `(0, 0, 0)`, quat `(0,0,0,1)`, parent 271 | same house/design/quat at local `(2, 0, 0)`, item instance +1 | no error |
| seed `0xE6` | doodad 377 at `(19650, 24377, 168.56)`, z_rot 0, scale 1 | doodad 377 at `(19651.5, 24385, 168.56)` | `DoodadsCreated`, no error |

Invalid spots showed the port keys (`0x244`, `0x71`, `0x7b` for the unfinished
new house) and the guessed path's own texts; the ported `plant`/`deco_place`
refusals use the same `ERROR_MSG` keys as the HUD notifications.
