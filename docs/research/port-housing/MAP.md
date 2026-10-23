# S4 - 1.2 housing and decoration placement: port map

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

## 8. Unknowns

1. **U1 - wheel axis amplitude.** The cvar is resolved to 5.0 (section 0/constants), but one live
   count is needed to confirm that one notch gives `FUN_39424580(0x5a) == 1.0` (and that a notch
   is not e.g. 0.5). Recipe: one `client_wheel +1`, read the axis, call `FUN_39016fe0`.
2. **U2 - initial yaw / "faces the player".** The HouseBuilder ctor does not set it and
   `FUN_3933a300` does not rotate; `FUN_3933a380` changes it only on wheel. The exact first yaw
   (`FUN_3932fb20`/`FUN_39368fb0` model default, or a `FUN_393323c0` call for the non-house
   classes) was not isolated. Needs a live capture of the temp unit quaternion before the first
   Rotate.
3. **U3 - dot spacing.** The plot step is the terrain object's `vtbl+0x20c`; the numeric value
   (1 m? 1.5 m? 2 m?) is runtime. The Bevy 1.0 m estimate is not proven. Recipe: read the terrain
   object and the point count on a known 24 m plot.
4. **U4 - `housings` desc `+0xd`.** It selects dot-style vs simple plot; the DB column it maps to
   (not the category) is not identified. The desc struct is re-ordered by the loader; only the
   fields used in code (`+0xc` category, `+0xd`, `+0x154`, `+0x158`, `+0x168`, limits) are mapped.
5. **U5 - full `housings`/`housing_decorations` struct offsets.** We have the fields used by the
   placement functions, not the whole loader schema; the 1.2 SELECT strings are listed in
   `constants.md` §6.
6. **U6b - packet position source.** `FUN_3961dd70` is exact, but its input is
   `FUN_3908c870(builder+8)`, which returns the current **view** position through
   `actor+0x3be8 -> GameClient vtable+0x228 + 0x10`. Whether the builder temporarily makes the temp
   housing the view target (expected, otherwise the quote/create would use the player position) is
   **[I]**; one live 0x56/0x57 capture settles it. The same helper is used by seed 0xE6.
7. **U6 - CSDecorateHouse source fields.** The ctor layout is exact; the source of design
   (`ghost+4`), parent (`ghost+0x4c`), house `tl` (`interface+4`) and item (`manager+0x28` first
   u64) is read from the object graph and marked **[I]** until one live 0x58 capture confirms it.
8. **U7 - furniture move/rotate of placed items.** No distinct 1.2 client packet/builder was found.
   The 10.x RTTI has no `CSMoveDoodad` in the strings searched; the likely flow is pick up the item
   and use it again (item task 6) or a server-side house edit. Needs a live capture of moving a
   placed chair.
9. **U8 - per-kind helper roles.** `FUN_39332690/32700/32770/327e0/32ad0`, `FUN_393379a0`,
   `FUN_39336600` and the vtable tails have no 1.2 names; roles are **[I]** from their call
   patterns (`FUN_39332200` = left click, `FUN_3908cb30/cae0` = direction steps).
10. **U9 - `DAT_3a167e84` gate.** It controls dots/connector boxes/material debug text; whether it
   is the guide-decal option (`toggle_show_guide_decal`) or a debug cvar was not resolved. A live
   toggle settles it; the owner's capture proves the dots are visible in retail.
11. **U10 - housing-area/garden primitives.** `FUN_39326b40`/`39326c10`/`39327830` are used as
    point-in-house/garden tests; their exact shape vs the server's housing-area polygons needs one
    diff run per area type (castle/dominion/farm).
12. **U11 - `builder_rotate_angle` 1.2 registration site.** The 1.2 descriptor at `0x39ac5658`
    (dest `0x39adac58`, default 5.0) is not referenced by any direct x2game code; the registration
    is through the shared option/cvar table (10.x has the same descriptor and the name). The value
   /default is proven; the registration function is not.
13. **U12 - 0x2ed vs 0x2ec seed permission.** `FUN_39053360` -> 0x2ed, `FUN_390533f0` -> 0x2ec in
    `FUN_394eaac0`; the exact farm-permission semantics (`only_nation_owner` vs
    `locate_invalid_dominion_area`) come from the server and are marked **[I]**.
14. **U13 - castle/keep split.** 1.2 has a single default class (`0x399cf94c`); the 10.x
    `CastleBuilder`/`KeepBuilder` split and the `0x215` castle-area branch in `FUN_39332200` are
    not fully mapped.

---

## 9. Acceptance / verification notes

* All 197 functions in scope are listed in [`inventory.md`](inventory.md) with VA, size (funcs.csv
  or measured), callers, callees, role and state/purity. Sizes sum to 84,374 bytes.
* The primary path (A+C+D+B, excluding UI getters) is ~73 KB. The 1.2 decompile was the only
  behavioural source; 10.x supplied names only (marked **[R10]**).
* Constants used by the port were read from the `.data` dump (`x2game_unpacked_data.dll`), not
  inferred from traces: wheel (5.0/1 deg/0.01/pi/12), elevation (-0.02), spread (6.0), range
  (+30), doodad (900), create limit (5000), cooldown (1000), position (2^44).
* The live original was **not** run for this task (the map is static). Every "how to diff" recipe
  in [`difftest.md`](difftest.md) is designed for the existing `aadriver` `call`/`read`/`write`
  commands and a small send-hook addition for packets.
* No files outside `docs/research/port-housing/` were changed. No CRYENGINE source was consulted
  or quoted.

### Follow-ups for the orchestrator

1. Review the addresses in [`inventory.md`](inventory.md) against `funcs.csv`; the five manual
   sizes (`0x39327de0` etc.) are the only non-`funcs.csv` entries.
2. Approve the port order (P0..P5) and the scope split: the maintain/sale/tax window functions in
   group U are mapped only for reuse, not proposed for this port.
3. Add the send-hook (`0x3ac1058c`, opcodes 0x56/0x57/0x58/0xE6) to `aadriver` before the packet
   tests; the rest of the diff harness needs only `call`/`read`/`write`.
