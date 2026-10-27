# Port map: the ArcheAge 1.2 client doodad and NPC interaction code

> **Restored 2026-10-27 from `archive/fork-main-2026-10-27`, UNVERIFIED.** The client was restarted from NickMesser's `main`; nothing described here is wired into the game. Addresses, call chains and offsets must be re-checked against the decompile ([R]/[I]/[?]) before use. A 2026-10-27 audit found 151 of the 269 EXACT/TOLERANCE labels in `proof-levels.*` weak or wrong (report: `D:\aaemu-client\handoff\2026-10-27\audit\report-proofs.md`).

Address map for a function-by-function Rust port of the 1.2 client's interaction system (S2):
picking and the dynamic interaction bar, doodad use (gathering, farming, coffers, switches, phases),
NPC talk/service/quest routing, and loot interaction, with a differential-test plan that calls the
original functions inside the running hidden 1.2 client (`aadriver.dll`).

## 0. Sources, conventions and licensing

### 0.1 Licensing

Everything here is derived **only** from the shipped 1.2 binaries and their decompiles. The
published CRYENGINE source was not used, quoted or consulted (same rule as
`aaemu-physics/CRYPHYSICS-MAP.md` and `port-combat/MAP.md`). Names in `CClass` form come from the
binaries' own RTTI, profiler/log/assert strings, cvar names, or the named 10.x build; behaviour that
has no name in the binary is marked *(inferred)*. Port from this document and the decompiles, never
from the CRYENGINE source.

### 0.2 Sources

| source | path |
|---|---|
| 1.2 x2game decompile | `C:/AAEMU_DISK/aa-re/client12/out-x2game/x2game.dll.c` |
| 1.2 x2game functions/calls/strings | `C:/AAEMU_DISK/aa-re/client12/out-x2game/{funcs.csv,calls.tsv,strings-xrefs.tsv}` |
| 1.2 x2game image + `.data` (constants, tables, vtables) | `D:/aa-re/client12/x2game_unpacked_data.dll`, base `0x38FF0000` |
| 1.2 engine decompiles | `D:/aa-re/client12/engine/out/`, `D:/aa-re/client12/engine2/out/` |
| 1.2 cryphysics (character mover) | `D:/aa-re/cryphysics/x86_12/`, ported by `aaemu-physics` |
| 10.x names/structure only | see `C:/Users/stsiakal/AAEmu/REVERSING.md` |
| UI scripts (LuaQ bytecode) | `data/extract/game/game/scriptsbin/x2ui/…` (this worktree has no extract; used `D:/aaemu-client/wt/pr/data/extract/game/`) |
| bindings | `game/default_binding.g` (extracted at `D:/aaemu-client/wt/pr/data/extract/game/default_binding.g`) |
| existing notes | `docs/research/re-quests.md`, `re-housing.md`, `re-crafting.md`, `re-constants.md`, `doodad-use-server.md`, `port-mount-anim/MAP.md` |
| server-side field names/layouts | `C:/Users/stsiakal/AAEmu/rust/crates/aaemu-game/src/` (our own port; packet layouts only) |

x2game addresses are VAs at the preferred base `0x38FF0000`. Evidence markers: **[R]** read from
the binary/decompile, **[I]** inference, **[?]** open. Function entry addresses are `FUN_xxxxxxxx`
as named by Ghidra; the four `do_interaction_*` stubs at `0x393f8290…0x393f82c8` are not listed in
`funcs.csv` (they sit inside the cold end of `FUN_393f81a0`) and are sized by hand (8 bytes each).

Several functions in this subsystem are `__thiscall` with the object in `ECX` while Ghidra prints it
as the first parameter; where argument order matters for the harness it is called out explicitly
(see [`difftest.md`](difftest.md)).

### 0.3 Layout of this folder

| file | content |
|---|---|
| `MAP.md` (this) | call chains, rules, port order, unknowns |
| [`functions.md`](functions.md) | every function in scope: address, size, callers/callees, fields, purity |
| [`structs.md`](structs.md) | the Rust struct layouts and offsets |
| [`packets.md`](packets.md) | client packet handlers/writers and their field offsets |
| [`difftest.md`](difftest.md) | in-process differential recipes per function |

### 0.4 Scope size

[`functions.md`](functions.md) documents **220 distinct function entries**: 216 are sized by
`funcs.csv` (**57,346 bytes**), the other four are the 8-byte `do_interaction_*` stubs
(`0x393f8290…c0`, **32 bytes**), for **220 entries / ~57,378 bytes**.
The subsystem is the contiguous cluster `0x393b0xxx–0x393c2xxx` (the game-client interaction
managers, dynamic action bar and handler registry), the packet handlers `0x391d8xxx–0x391da9xx` /
`0x391d5xxx`, the loot manager `0x394fa0xx–0x394fbxxx`, the pick/click layer `0x39428d80` /
`0x390f3xxx–0x390f5xxx`, and the shared lookup/range leaves.

---

## 1. The call chains

### 1.1 Input: F/G/H/J and the dynamic action bar

`game/default_binding.g` lines 29–33 bind `f do_interaction_1`, `g do_interaction_2`,
`h do_interaction_3`, `j do_interaction_4`. The action-name table is at **`0x39ac8e68`**
(104 entries, `char*` each; names via `FUN_393f8d20`). Indices [R, read from `.data`]:

| index | action | handler (from `0x3a59f1c0+0x13c+4*index`) |
|---|---|---|
| 31 | `do_interaction_1` | `0x393f8290` |
| 32 | `do_interaction_2` | `0x393f82a0` |
| 33 | `do_interaction_3` | `0x393f82b0` |
| 34 | `do_interaction_4` | `0x393f82c0` |
| 72 | `quick_interaction` | `0x393f8650` (empty stub) |

The handlers are fixed stubs `push index; call FUN_393b2b70` [R]. `FUN_393b2b70` is a 20-byte
wrapper around the real entry:

```
F 0..3 → FUN_393b2b70(i) → FUN_393b1de0(bar, i)
FUN_393b1de0:                       // dynamic-bar execute
  if i < 0 or i >= (bar+0x24 - bar+0x20)/4: return
  if FUN_393bc7e0(i) or FUN_393bc8c0(i): return   // loot one/all special cases
  skill = bar->shown[i]
  if skill == 10994 (DAT_3a17d7d0) or skill == 10994 (DAT_39ac7820): return
  if bar+0x0c != empty (doodad):
      doodad = FUN_39035110(bar+0x0c)
      FUN_393b3ec0(skill, DAT_3a17d7d8, doodad, 1)
  else if bar+0x08 != empty (unit):
      unit = FUN_39360020(bar+0x08)
      FUN_393be280(skill, unit, 0, 1)
```

The Lua bar (`dynamic_action_bar.alb`, `dynamic_action_bar_view.alb`) shows **10** buttons
(`MAX_DYNAMIC_ACTION_COUNT = 10`, view script line 1), each with a 48×48 icon and the
`ConvertAbbreviatedBindingText(X2Hotkey:GetBinding("do_interaction_"..i))` key label; a click calls
`X2:ExecuteDynamicAction(i)`. The four classic keys are therefore only the first four of the ten
slots; the rest are click-only.

The bar object is **`DAT_39affa0c`** (0x30 bytes, allocated in `FUN_39174520`), see
[`structs.md`](structs.md) §3. Bindings [R]:

| binding | native | meaning |
|---|---|---|
| `X2:GetDynamicActionCount()` | `0x39442370` | shown-list length |
| `X2:GetDynamicActionSkill(idx)` | `0x39442400` → `0x393b1da0(idx-1)` | shown-list skill id |
| `X2:IsDynamicActionSkillToggled(idx)` | `0x394424c0` | toggled/target state |
| `X2:ExecuteDynamicAction(idx)` | `0x39440850` → `0x393b1de0(idx-1)` | execute |
| `X2Interaction:IsDynamicActionBarVisible()` | `0x3947e060` | `bar+3 != 0 && bar+4 == 0` |

### 1.2 Picking (mouse-over and click)

Per frame **`UpdatePicking` `0x3942c640`** [R]:

```
if mouse is over UI / dragging: clear (FUN_394273e0(1)), return
FUN_394273e0(0)
FUN_39428d80(cursorX, cursorY, ...)         // screen ray
  → pick result {unitId, doodadId, gimmickId, …, pickingId, pos}
  → globals DAT_3a5a4dbc / 3a5a4dc0 / 3a5a4dc4 / 3a5a4dd0
refine to the nearest unit (FUN_39354210 list, FUN_39358c00 distance)
if the nearest is closer than the current: DAT_3a5a4dbc = it
FUN_393b51a0(doodad)                        // doodad sign/tooltip event 0x1A7/0x1A8
```

`FUN_39428d80` builds a ray from the camera position (camera object `DAT_3a5bb1d8[3]`, view
vtable `+0x16c`) through the cursor, normalises it, and runs **two** `RayWorldIntersection(Game)`
passes (masks `0x8002048f` rwi `0x117`, then `0x200048f` rwi `0x11f`; the second only when the first
misses or its result is unusable) [R]. The debug log string is decisive for the result layout:

```
"Picking : Pos(%06.2f, %06.2f, %06.2f)  unitId(%05u)  doodadId(%05u)  gimmickId(%05u)  pickingId(%d)"
```

so the pick block is `{+0 u32 unitId, +4 u32 doodadId, +8 u32 gimmickId, +0x14 i32 pickingId,
+0x18/+0x1c/+0x20 f32 pos}`; `DAT_3a5a4dcc` is the "current target was clicked/selected" flag.

Clicking is **`ControlMouseClick` `0x390f50a0`** (dispatches on `DAT_3a5a4ea0`), normal mode →
**`0x390f4960`** [R]:

```
if mounted: return
FUN_39429e40(DAT_3a5a4dbc, DAT_3a5a4dc0)    // set/clear target + clicked doodad
FUN_393be4e0(0,0)                           // auto-attack gate
FUN_390f3780()                              // default interaction
if button == 3: … else FUN_390f37f0(...)
```

**`FUN_390f3780`** is the actual default-interaction dispatcher [R]:

| picked object | call | effect |
|---|---|---|
| unit `DAT_3a5a4dbc` | `FUN_393be030(unit, obj, extra, mouseButton, modifiers)` | `CSStartInteraction` 0x068 |
| doodad `DAT_3a5a4dc0` | `FUN_393b79a0(doodadId)` | use the doodad's default skill |
| gimmick `DAT_3a5a4dc4` | `FUN_39429d00(gimmickId, kind)` | `StartGimmickInteraction` |

`FUN_393b79a0` builds the doodad's client-side candidate set (`FUN_393bd290`), picks its first
entry, and calls `FUN_393b3ec0(skill, 0, doodad, 1)` [R].

### 1.3 Doodad use: the entry points

There are four ways a doodad skill is used; all end in `CSStartSkill` 0x052 with a doodad target:

```
1) F / bar / click:  FUN_393b1de0 → FUN_393b3ec0(…, 1) → thunk 0x39397e50
2) Lua SelectInteraction(id): FUN_3947dac0 → FUN_393b3120 → CSSelectInteractionEx 0x06B
   (server may answer SCWorldInteractionSkillList 0x07E → FUN_393b7440 → thunk 0x39397e50)
3) Lua StartDoodadInteraction(id): FUN_3947db20 → FUN_393b3ec0(…, 1)   (same as 1)
4) a phase-change func: FUN_393b8240 → CSChangeDoodadPhase 0x0E9
```

`thunk 0x39397e50` = `thunk_FUN_3a6aa27f`, the split-tail "use skill with target" request writer
(the same tail as the combat `UseSkill` body region; see `port-combat/MAP.md` §2.5). Its target
block for a doodad is `{type 4, objId = doodad+0x7c}` and its caster block is
`{type 0, id = player+8}` [R, `0x393b418c`–`0x393b419d`; `+0x7c` is the world objId, set by
`FUN_3983b690` from `SCDoodadCreated` field 0]. A unit target uses type 0 with the unit objId.

`FUN_393b7440` (SCWorldInteractionSkillList) auto-uses the first listed skill after copying the
server's 0x40-byte block and re-running the same gates; the target block there is
`{type 4, id = manager+4}` (the packet's doodad objId) [R, `0x393b7750`–`0x393b7770`].

### 1.4 NPC interaction

```
F / bar / click / ExecuteDynamicAction
└─ FUN_393be280(skill, npcUnit, extra, changed)          // StartNpcInteraction
   ├─ FUN_393be0e0(skill)                                // skill/quest special ids
   ├─ mounted → 0x14c
   ├─ FUN_393b2c70(now, lastTime)                         // 500 ms rate limit
   ├─ FUN_393c01d0(player, npc)                          // target gate
   │   └─ 0 = ok, 3 = busy, 0x66/0xdf/0x166 = refusal
   ├─ FUN_393bf270(skill, player, npc, changed)          // 3.0 m gate (+auto-walk)
   ├─ FUN_39429e40(npcObjId, empty)                      // select target
   ├─ FUN_393bf310(skill, player, npcObjId, npc, &flag)  // mounted/vehicle/loot/pvp services
   ├─ FUN_393bf500(skill, &npcQuestLists, npc)           // quest routing: UI event 0x6B
   ├─ FUN_393c1280(skill, npc)                           // service handler registry
   ├─ target kind == 1 → CSInteractNPC 0x065            // FUN_397ab740(npcObjId, changed)
   └─ FUN_393bfc70(player, npc, facing)                  // face/talk anim + camera
```

The server's answer `SCNpcInteractionSkillList` 0x07B → `FUN_391d8f80` → `FUN_393be850`
stores the list, optionally injects a client-side `quest`/`talk` entry, and immediately calls
`FUN_393be280(firstSkill, npc, packetArg, 1)` [R]. `CSStartInteraction` 0x068 is sent by
`FUN_393be030` (the click path) and inside `FUN_393bf310`/`FUN_393bf500` when a service window
needs the server; its writer is `FUN_397ac0a0`.

### 1.5 Loot

```
SCLootableState 0x0F6 → FUN_391d9780 → FUN_394fa110 / FUN_394faf40   // mark lootable
SCLootBagData   0x0F8 → FUN_391d9890 → FUN_394fa9a0                // fill rows, event 0x7E
SCLootItemTook  0x0F9 → FUN_391d9910 → FUN_394fa6c0                // decrement/remove
SCLootItemFailed 0x0FA → FUN_391e56d0 → FUN_394f9c20               // clear in-flight flag
F on a corpse: bar has 12079/14047 → FUN_393bc7e0 / FUN_393bc8c0
   → open (FUN_394fb010/394fb160, CSLootOpenBag 0x08E)
   → take (FUN_394f9790, CSLootItem 0x08F) per slot
X2Loot:LootItem(slot-1) → FUN_394f9790
X2Loot:CloseLoot()      → FUN_394fab30 (CSLootCloseBag 0x090)
```

---

## 2. A — picking and the dynamic interaction bar

### 2.1 The bar object and refresh

`DAT_39affa0c` (0x30 bytes) [R, `0x393b23d0` ctor, `0x393b2530` reset]:

| off | type | meaning |
|---|---|---|
| +0x00 | u8 | visible |
| +0x01 | u8 | (unused in the code read) |
| +0x02 | u8 | show pending (set by `FUN_393b2490`/`24e0`, cleared by `FUN_393b2760`) |
| +0x03 | u8 | has-actions (read by `0x3947e060`) |
| +0x04 | u8 | hidden (read by `0x3947e060`) |
| +0x08 | u32 | current unit/npc objId (empty sentinel `DAT_3a17d7cc`) |
| +0x0c | u32 | current doodad objId (empty sentinel `DAT_3a17d7d4`) |
| +0x10/+0x14 | u32* | raw candidate vector begin/end (cap +0x18) |
| +0x20/+0x24 | u32* | shown vector begin/end (cap +0x28) |

`FUN_393b25a0` (`DynamicActionBar::RefreshActions`) rebuilds the raw list (`FUN_393bc620` +
`FUN_393bd1b0`), then:

* empty raw list → drop the shown list, fire `DYNAMIC_ACTION_BAR_HIDE` **0x19E**;
* raw list differs from the shown list → copy it and rebuild the buttons (`FUN_393b2230`);
* equal → no event.

`FUN_393b2760` (`DynamicActionBar::Update`) calls `RefreshActions` and then `FUN_393b1eb0(true)`
(`Show`), which fires `DYNAMIC_ACTION_BAR_SHOW` **0x19D** with `("npc", unitId)` or
`("doodad", doodadId)` (strings `0x3999d144`, `0x3999d134`). `FUN_393b1eb0(false)` fires 0x19E.

### 2.2 Candidate builders (which skills appear)

The bar is built from the picked/selected object by one of two paths:

* **unit / NPC** — `FUN_393bd050` sets `bar+8 = objId` (`FUN_393b2490`) and calls
  `FUN_393bcc20(player, object)` + `FUN_393bcf00(object, bar)`; the per-kind switch is in
  `FUN_393bcf00`: `unit+0x410` kind 1→`FUN_393bb9d0`, 2→`FUN_393bbb80`, 3→`FUN_393bbf10`,
  4→`{}`, 5→`FUN_393bbc90`, 6→`FUN_393bbd70` [R].
* **doodad** — `FUN_393bd850` sets `bar+0xc = objId` (`FUN_393b24e0`) and calls
  `FUN_393bd290(doodad, bar)` (the current-phase func candidate builder).

`FUN_393bb9d0` (kind 1, NPC) [R]:
1. if the NPC is mounted (`+0x3c26`) and lootable (`FUN_394f9ec0`): add **14047** (loot all) and
   **12079** (loot) and return;
2. `FUN_393c01d0(player,npc) != 0` → return (refuse);
3. if `npc+0x5fd4` and not mounted: resolve the NPC component (`FUN_39340280`), if it has quests
   (`FUN_393c17e0`) add **13877** (quest); otherwise add **13356** (talk) only when the NPC has a
   non-default greeting (`FUN_393bed90`); then `FUN_393bb420(npcDesc)` adds the service skills
   from the NPC slots (see §4.5).
4. `FUN_393bb910` / `FUN_393bb420` add any remaining received unit skills.

`FUN_393bb420` maps NPC descriptor slots to the hard-coded interaction skills [R]. Each entry is a
non-zero test on a descriptor field (the fields look like 32-bit interaction-skill slots; the code
tests only the low byte/word, so a zero slot means "not offered"):

| NPC desc field (byte offset) | skill | service |
|---|---|---|
| `+0xD0` byte | 12081 | learn skill |
| `+0x04` byte | 12082 | ability change |
| `+0x24` word | 12083 | auction |
| `+0x34` byte | 12086 | repair |
| `+0x25` byte | 13238 | warehouse |
| `+0x58` byte | 13736 | expedition |
| `+0x6C` byte | 23478 | item look change |
| `+0x78` byte | 12087 | store |
| `+0x90` pointer | `FUN_39696280` list | extra skills (`+4` = id) |
| `+0xAC` byte | 17063 | pray |
| `+0xB0` dword != empty | 12660 | battlefield |
| `+0xB4` byte | 15297 | pet heal 2 |
| `+0xDC` byte | `FUN_393bb370` | special |
| `+0xE4` byte | 12195 | pet heal |
| `+0xE5` byte | 12085 | teleport |
| `+0xED` byte | 14866 | sell backpack |

`FUN_393bd290` (doodad) builds the list from the **current phase func descriptor** [R]:
`doodad+0x218` is the phase/model object; its vtable `+4` returns a struct whose first dword is the
func group id, and `FUN_3963b250(group)` returns the descriptor; the descriptor's vector at
`+0x24..+0x28` holds one row per active func. Each row is filtered by
`FUN_393b3300(row+0x20 /* perm_id */, doodad)` (§4.5), `FUN_3983b4e0(row, playerId, currentTarget)`,
`FUN_393b2c40(playerId, row)`, `FUN_393b9a80` (quest requirement), then its skill id (`*row`) is
added. The row's `+0x20` value **is** the DB `doodad_funcs.perm_id` (0..8; see §4.5); the row's
`+0x08` is the actual func type used by the `FUN_393bc260` switch.

### 2.3 Ordering and de-duplication

Candidates are inserted into a `std::set`-like ordered container keyed by the 32-bit field at
`candidate+0xc`:

* `FUN_393bab40(&keyStruct)` allocates a 0x18-byte set node and copies `{key, value}` from an
  8-byte stack struct; the key is the first dword.
* `FUN_393ba1b0` / `FUN_393b6e90` / `FUN_393c0ff0` insert, keeping ascending key order and
  rejecting duplicates (the insert compares `newKey < node[3]`).
* Callers pass the **skill id** as the key (`FUN_393bb320(id)`, `FUN_393bd240(id, extra)`), so the
  bar is ordered by ascending skill id, not by DB order or discovery order. Duplicate ids collapse.
* The candidate list is then copied into the raw vector (cap 0x100 entries, `FUN_393bcb90`), and
  `RefreshActions` copies it to the shown vector; the UI displays the first 10.

### 2.4 Range and LOS

| gate | function | value |
|---|---|---|
| doodad use/bar | `FUN_39093800(player, doodad, limit, outDist)` | `max_interaction_doodad_distance` = **2.1 m** (`DAT_39aefb98+0x2a0`, static `0x399a7660`) |
| NPC/unit use | `FUN_393bf270` → `FUN_39093860(player, unit, limit², …)` | **3.0 m** (`0x39ac79fc = 9.0`), **3.5 m** when the interacting unit is mounted (`0x39ac7a00 = 12.25`) |
| bar visibility (cvar) | `dynamic_action_bar_distance` | **25 m** (`DAT_39aefb98+0x29c`, static `0x3999f85c`) — registered in `FUN_390f7b40`, **no x2game reader found** (U8) |
| indicator candidate query | `FUN_393601a0` | 2.1 m for doodads (when the passed limit is ≤ 0), +0.8 m for a mate (`0x3999d7e4`) |
| pick ray | `FUN_39428d80` | max 2000 (`0x399a1ec0`), two RWI masks |
| loot bag open | `FUN_394fb010`/`fb160` | none client-side; server validates |

`FUN_39093800(a, b, limit, out)` asks the second object for its distance shape
(`vtable+0x18`), resolves the actor (`FUN_390b3ae0`) and takes the minimum surface gap over the
object's collider list (`FUN_3989bf90` → `FUN_398b1ab0`); it returns true when `dist < limit`.
`FUN_39093860` does the same with a **squared** limit (it takes `SQRT`).

LOS for the bar is **`FUN_393bc980(actorHandle, skillArray, target)`** [R]: it takes the camera/eye
position (`FUN_3989ac50`), builds a ray toward the target, and calls `RayWorldIntersection(Game)`
with mask `0x2048f`, rwi `4`; it returns true when the ray hits nothing (line of sight clear). The
caller supplies the bar's skill array as the skip list. The stack order is read from the call site;
confirm the exact roles in the wrapper (U1). If the check fails, the bar is reset
(`FUN_393bcff0` + `FUN_393b2530`).

The generic range predicate `FUN_39093800`/`FUN_39093860` is the same one the rest of the client
uses (mounts, housing, trade); see `re-constants.md`.

### 2.5 The hover indicator and refresh

`FUN_393bddf0` (`InteractionIndicator::Update`) runs only when the player and game client exist and
`FUN_393bc630()` is true; it calls `FUN_393bdad0` (`MakeCandidate`). `FUN_393bdf10`
(`UpdateInteractionIndicator`) is the per-frame driver: it calls the indicator update and
`DynamicActionBar::Update` (`FUN_393b2760`).

`FUN_393bdad0` [R]:
1. skip when the mouse is dragging / UI is capturing (`FUN_39016fd0`, `FUN_390171f0`,
   `FUN_3902db50(0x1e)`, `FUN_3902cc10`);
2. skip when mounted (`player+0x3c26 != 0`) or `player+0x3b94 != DAT_3a17e6fc`;
3. build a query from the player position and the camera forward vector
   (`FUN_390b3870`, normalised with the `0x3999c478/47c/484` rsqrt constants), collect candidates
   (`FUN_393601a0`, `FUN_3902f010`), then `FUN_393bd650` resolves the pick;
4. if the pick result kind (`+8`) is 2 → `FUN_393bd850(object at +0x10, doodad path)`, else
   `FUN_393bd050(object at +0xc, unit path)`; no result → reset the bar.

The pick result block used by `FUN_393bd650` is a set node at `+3`: `+8` = type (0 = none,
1 = unit, 2 = doodad, from `node[5]`), `+0xc` = the unit object (`node[6]`, built with
`FUN_393bcf00`), `+0x10` = the doodad object (`node[7]`, built with `FUN_393bd290`) [R]. It also
consults the two builders to see whether the candidate has any usable skill; if not, the indicator
clears.

The indicator's candidate search is a cone/ray (`FUN_393601a0` for actors, `FUN_3902f010` for
doodads) plus the nearest-unit scan in `UpdatePicking` (§1.2); the exact ray/cone parameters are in
the query builders and are marked *inferred* where the decompiler loses the vector arguments (U1).

---

## 3. B — doodad interaction

### 3.1 `StartDoodadInteraction` (`FUN_393b3ec0`, 787 B)

Signature (after the `this` in `ECX`): `(u32 skillId, u32 param2, ClientDoodad* doodad, int mode)`;
`mode` is 1 from the UI/click/F paths, 2/3 from the deferred re-entry (`FUN_393b3000`). Order [R]:

1. reset the bar (`FUN_393bc620`, `FUN_393bc660`), resolve the skill desc (`FUN_396f6390`).
2. reject when the desc is missing, the doodad is null, or the skill is the sentinel 10994
   (`DAT_39ac783c`) / the empty id (`DAT_3a17e288`); clearing the current interaction.
3. `FUN_39044770()` true → skill-result refusal `0x7` and return.
4. 500 ms rate limit on `manager+0x44` (`lastTime + 500`), skipped when `mode == 1` and the stamp
   is still in the future.
5. `FUN_393b3d50(doodad, desc)` prechecks (§3.9). Failure → return.
6. distance gate `FUN_39093800(player, doodad, 2.1, 0)`; if too far:
   * store `manager+4 = doodad->objId`, `manager+0x10 = skillId`, `manager+0x38 = 1`,
     `DAT_3a5a4dcc = 1`;
   * `FUN_390a6af0(1 or 3, empty, doodadObjId, skillId)` starts the auto-walk;
   * return.
7. `manager+0x5c != doodadObjId` → store it and `FUN_393b31e0(doodad+0x18)` (face the doodad).
8. `FUN_39429e40(empty, doodadObjId)` (select/clear), `DAT_3a17e280 = doodadObjId`.
9. `mode != 2 && FUN_393b3c50(doodad, desc)` true → level check: player level `+0x17` vs doodad
   `+0x4b4`; enough → `FUN_393b3000(skill, 2)` (deferred), else refusal `0x40`.
10. `mode != 3 && FUN_393bc480(skill, 0, doodad)` true → `FUN_393b3000(skill, 3)` (deferred).
11. `FUN_393ba9c0(skill, doodadObjId)` true → done (the func handled it).
12. `FUN_393bc260(skill, 0, doodad)` true → done (func-type dispatch, §3.4).
13. `FUN_393b9a80(player, doodad, &out)` false → done (quest requirement fails).
14. `FUN_393b7c40(skill)` false → refusal `0x1d`.
15. skill `!= 14047` → build the caster block `{type 0, id = player+8}` and the target block
    `{type 4, id = doodad->objId}`, then `thunk 0x39397e50(manager, player, skill, &caster,
    &target)` = `CSStartSkill` 0x052. Skill 14047 (loot-all) → `FUN_393bc8c0(1)` instead.
16. `FUN_393b2ab0` clears the current interaction.

`FUN_393b3000(skill, mode)` queues a 0x14-byte action with `{skill, doodadObjId, mode}` on the
game-client action queue (`FUN_3902d670` after `FUN_3902db50(0x1e)`) [R]. The queued action is
expected to call back into `FUN_393b3ec0` with that mode so the same function can skip the gate it
just took [I]; the mode-2/3 skip in `FUN_393b3ec0` is [R].

### 3.2 `SelectInteractionEx` (`FUN_393b3120`, 157 B) and the world list

`X2Interaction:SelectInteraction(id)` (`FUN_3947dac0`):

* if the manager has a current interaction object (`DAT_39aff9bc+0x48 != 0`):
  `FUN_393b2af0(manager, id)` → current target id, then `FUN_393b3120(manager, id, targetId)`:
  writes the packet `{0x6B, targetId, id, var1}` via `FUN_397ac0d0` and sends it with
  `0x3ac1058c`; clears the object and the list.
* otherwise calls `FUN_393b3ec0(id, 0, currentDoodad, 1)` directly (the fallback).

`SCWorldInteractionSkillList` 0x07E handler `FUN_391d9080` → `FUN_393b7440`:
copies the 16-dword block to `manager+0`, filters the skill list at `manager+0x10` (count
`+0x38`), resolves the doodad by `manager+4` (`FUN_39035110`), and:
* distance gate 2.1 m; too far → store the pending interaction and auto-walk (`FUN_390a6af0`);
* otherwise take `manager+0x10` (the first skill) and run the same gates as `FUN_393b3ec0`
  (`FUN_393b3c50`, `FUN_393bc480`, `FUN_393b3d50`, `FUN_393ba9c0`, `FUN_393bc260`,
  `FUN_393b9a80`, `FUN_393b7c40`) and finally `thunk 0x39397e50` with `{type 4, id = manager+4}`.

`SCWorldInteractionCanceled` 0x07F → `FUN_391d9100` → `FUN_393b30a0` (not the current source) or
`FUN_393b30e0` (current) — both clear the current interaction and fire UI event `0xD2`
`INTERACTION_END`.

### 3.3 Current-interaction state and cancel

`DAT_3a17e6a0` is the interaction-UI state, `DAT_3a17e6a4` the bound object id [R]:

| state | set by | UI |
|---|---|---|
| 1 | `FUN_393b8450` | open paper/book (`0x201`, "book"/"page") |
| 2 | `FUN_393b8610` | item renewal (`0x8A`) |
| 5 | coffer path (`FUN_393b7c00`/`0x3902e190`) | coffer |
| 7 | `FUN_393b82a0` | UCC/emblem upload (`0x1A4`) |
| 8 | `FUN_393b80d0` | deferred book action |
| 9 | `FUN_393bc260` case 8 | (UI event 9) |
| 10 | `FUN_393b8700` | open common farm info (`0x102`) |
| 13 | `FUN_393bc260` case 0x51 (inline) | coffer |
| 14 | `FUN_393b8b80` | auction (`0x6B`, "auction") |
| 15 | `FUN_393b8c20` | bank (`0x6B`, "bank") |
| 17 | `FUN_393b8cc0` | store (`0x6B`, "store") |
| 18 | `FUN_393b8a30` | specialty/territory |

`FUN_393b7b10(state, objId)` switches to a state (closing the previous one through
`FUN_393b7aa0`), `FUN_393b7b50(state)` cancels a state (used by the crafting cancel path:
`FUN_394d0ae0` → `FUN_393b7b50(3)`), and `FUN_393b7aa0` is the single cleanup: it fires the
per-state close event (coffer close, `NPC_INTERACTION_END` 0x6C for 0x0E/0x0F/0x11, a 30 ms
delayed action for 0x10, `INTERACTION_END` 0xD2) and clears the manager.

### 3.4 Func-type dispatch (`FUN_393bc260`, 567 B)

`FUN_393bc260(skill, 0, doodad)` first resolves the current phase's func row
(`FUN_393bc200` → `FUN_3963b1d0`/`FUN_393bc100`), checks `FUN_393b3300(row+0x20, doodad)`
(permission), then switches on `row+8` (**actual func type enum**) [R]:

| case | handler | identified as |
|---|---|---|
| 8 | `FUN_393b7b10(9, doodad)` | (UI event 9) |
| 0x34 | `FUN_393bb090` | crime report / housing action (`0x202 REPORT_CRIME`) **[I]** |
| 0x3d | `FUN_393b82a0` | UCC/emblem upload (`0x1A4 OPEN_EMBLEM_UPLOAD_UI`) **[R]** |
| 0x3f | `FUN_393b8240` | **phase switch**: `CSChangeDoodadPhase` 0x0E9 **[R]** |
| 0x40 | `FUN_393b80d0` | deferred book/page action (`0x201`) **[R]** |
| 0x43 | `FUN_393b8400` | unit interaction: `FUN_393befd0(unit)` **[I]** |
| 0x46 | `FUN_393b8450` | open paper (`0x201`, "book"/"page") **[R]** |
| 0x47 | `FUN_393b8610` | item renewal (`0x8A RENEW_ITEM_WND_SHOW`) **[R]** |
| 0x48 | `FUN_393b8700` | **open common farm info** (`0x102`, farm type = `*(row+0xc)`) **[R]** |
| 0x4a | `FUN_393b7b10(0x12, doodad)` | craft info/act **[I]** |
| 0x4f | `FUN_393b8870` | craft/act with `FUN_397cf5a0` gate **[I]** |
| 0x50 | `FUN_393b8a30` | second craft/act variant (`FUN_397cf660`) **[I]** |
| 0x51 | inline in `FUN_393bc260` | coffer: `FUN_3983b3b0(doodad+0x80)` then `0x243 COFFER_INTERACTION_START` **[R]** |
| 0x53 | — | refusal text `2` **[R]** |
| 0x54 | — | purchase/exchange: money/item counts, refunds **[I]** |
| 0x55 | `FUN_393b8b80` | auction (`0x6B`, "auction") **[R]** |
| 0x56 | `FUN_393b8c20` | bank (`0x6B`, "bank") **[R]** |
| 0x57 | `FUN_393b8cc0` | store (`0x6B`, "store") **[R]** |

The numeric values not marked **[R]** need alignment with the DB `doodad_funcs.actual_func_type`
strings (U3); the handlers' own events/strings are cited in [`functions.md`](functions.md).

### 3.5 Phases

`SCDoodadPhaseChanged` 0x10F handler **`FUN_391da5c0`** → `FUN_39035280`
(`DoodadManager::HandleDoodadPhaseChanged`) [R]:

```
FUN_39035280(objId, +0x10, +0x18, +0x14, +0x1c)
  FUN_39034a50(&found, &objId)        // doodad by objId
  if found != manager->+8:
      FUN_393a6c60(+0x10, +0x18, +0x14, +0x1c)
      manager->+0x14 = 1
      FUN_39383f40()                  // refresh quest marks
```

`FUN_393a6c60` updates the doodad object: `+0x220` current func group, `+0x224` =
`FUN_3963b250(new group)`, `+0x28c` clear, `+0x270` = the fifth packet field, `+0x2a8`, `+0x354`,
and calls the model/anim/quest-mark refreshers
(`FUN_393a1c90`, `FUN_393a2e10`, `FUN_393a3070`). The server field order is bc objId,
u32 funcGroupId, u32 timeLeft, i32 puzzleGroup, u32 itemTemplateId; which of the last three
lands in `+0x2a8`/`+0x354` is marked *inferred* (U2).

`SCDoodadPhaseMsg` 0x157 → `FUN_391dc720` → `FUN_3902e0f0`: looks up the phase func
(`FUN_3963b250`) and fires `DOODAD_PHASE_MSG` **0xFD** with the func's text when set.

`CSChangeDoodadPhase` 0x0E9 is written by `FUN_397ac8d0(objId, skill, row+0x1c, *row)` and sent
from `FUN_393b8240` (case 0x3F).

### 3.6 Coffers

* A doodad func of type 0x51/`DoodadFuncCoffer` opens the coffer UI: event
  `COFFER_INTERACTION_START` **0x243**, fired inline in `FUN_393bc260` case 0x51 (gated by
  `FUN_3983b3b0(doodad+0x80)`); the coffer window then requests the contents with 0x06C
  (`FUN_393b7c00` → `FUN_3902e190`).
* `X2Coffer` and the coffer window request the contents with `CSCofferInteraction` **0x06C**,
  writer `FUN_397ac0f0(objId, 1)` from `FUN_3902e190` (called by `FUN_393b7c00` and the
  coffer-window path).
* `SCCofferContentsUpdate` **0x096** → `FUN_391d5590` [R]:
  * resolve the doodad by `packet+0xc` (`FUN_39035110`);
  * require the player and `FUN_39093800(player, doodad, 2.1, 0)` (in range);
  * if the coffer target changed: `FUN_394d0820(doodad, packet+0x10, packet+0x14)` and fire
    game events 0x1F/0x20;
  * then read `packet+0x18` item count, and for each item `FUN_394d0810(slotType, itemRow)` +
    `FUN_394d0730()`; item rows are 0xC0 bytes (`packet+0x19` slot types, `packet+0x38` first
    row).
* `OnSplitCofferItemResult` → `FUN_391d56c0`: on error `FUN_39428370(packet+0xc)`; otherwise
  `FUN_394df370`/`FUN_397d2200` refresh both stacks.
* Coffer listing, move, split and permission are a separate subsystem (`FUN_3945ecb0` binding,
  `X2Coffer`); this map covers only the interaction side.

### 3.7 Gathering, farming, switches

* Gathering/farming is the generic doodad path: the current phase's func row carries a
  `func_skill_id`; the client casts it at the doodad (`CSStartSkill` 0x052) and the server runs
  the interaction. No gathering-specific window or range exists client-side beyond §2.4; the
  cast bar is the combat cast bar (`port-combat/MAP.md` §3.2).
* Workbench/craft funcs are `0x4F`/`0x50`/`0x4A`-style handlers (§3.4) plus the crafting UI
  (`X2Craft`), not a separate range gate.
* Switches: phase-change funcs use case 0x3F → `CSChangeDoodadPhase` 0x0E9. The client also
  updates the doodad locally when `SCDoodadPhaseChanged` arrives (§3.5).
* Cut-down/vegetation is `SCVegetationCutdowning` 0x081 → `FUN_391da940` →
  `FUN_393b7800(unit, doodad, 100)`: creates an `InteractionEffect` (0x54 bytes, duration
  `100 * 0.001 s`) in the interaction manager list. `InteractionManager::Update`
  (`FUN_393b5f60`) ticks it, and when the effect reports finished it logs
  `InteractionManager() finishing interaction. source(%s) target(%s) interaction(%d)` and frees
  it; a 1000 ms watchdog calls `FUN_393b7e40` (drop the selected doodad if out of range).
  `FUN_393b6220` (`OnInteractionEffectSkillCanceled`) removes the effect matching
  `(source,target,interaction)`; its only caller is the combat `StopCasting` `0x39395b50`, so a
  cast cancel also cancels the cut-down effect [R].

### 3.8 Cast bar and cancel rules

The interaction code never builds a cast bar itself. It requests the skill (0x052) and the local
timeline starts on `SCSkillStarted` 0x0A1 (`StartSkill` 0x3939a810, `port-combat/MAP.md` §3.1).
The interaction-specific cancel pieces are:

* `FUN_393b7b50(state)` / `FUN_393b7aa0` close an open interaction window (not the cast);
* `FUN_393b6220` cancels the interaction effect when the skill cast stops
  (`FUN_39395b50` from `SCCastingStopped`/`EndSkill`);
* moving cancels the cast server-side (the client sends `CSStopCasting` 0x054 as for any cast;
  no interaction-specific movement cancel was found) (U9, same as combat U9).

### 3.9 Refusals

| text id | call site | condition |
|---|---|---|
| 0x07 | `FUN_393b3ec0` | `FUN_39044770()` says the player is in a state that forbids interaction (shop/trade) **[I]** |
| 0x14c | `FUN_393b3d50`, `FUN_393be280` | mounted (`unit+0x3c26 != 0`) |
| 0x14e | `FUN_393b3d50` | the player's vtable `+0x38 → +0x14` says "cannot interact" (climbing/attached) **[I]** |
| 0xdf | `FUN_393b3d50` | `FUN_390b4d40()` and `doodad+0x80 == 6563` |
| 0xde | `FUN_393b3d50` | a current target/one-target state blocks it (not a pet) |
| 0x66 | `FUN_393b3d50`, `FUN_393c01d0` | current target is kind 2 |
| 0x40 | `FUN_393b3ec0` | player level below the doodad's `+0x4b4` for the gated func |
| 0x1d | `FUN_393b3ec0`, `FUN_393b7440` | skill desc unusable (`FUN_393b7c40` fails) |
| 0x12 | `FUN_393c0320` | house/unit interaction object missing |
| 0x57 | `FUN_393bc4e0`, `FUN_393c0500`, `FUN_393c05f0` | unit/item lookup failed |
| 0x1b | `FUN_393bc260` case 0x54 | not enough money/items |
| 0xd7 | `FUN_39348800` | boarding state invalid |
| 0x166 | `FUN_393c01d0` | relation/hostility gate (silent: the caller does not show 0x166) |
| 0x2f4 | `FUN_393b8f40` | hang/climb throttle (2000 ms since `player+0x40`+0xb20) |
| 0x169/0x58/0xe1 | `FUN_393c0500/05f0` | no house / wrong zone / not allowed |
| 0x13b | `FUN_393bf840` | specialty trade not allowed in this zone |
| 0x14 | `FUN_393bf310` | battlefield level gate |
| 2 | `FUN_393bc260` case 0x53 | generic "cannot use" |

The strings for the interaction/NPC errors live at `0x399f3ff0–0x399f44xx`
(`interaction_permission_deny`, `interaction_npc_description_not_found`,
`interaction_npc_not_exists`, `cannot_interact_while_climb`, `cannot_interact_while_mounting`,
…). The numeric ids index the client's encrypted `ui_texts`; the exact id→key table is unresolved
(U7).

---

## 4. C — NPC interaction

### 4.1 `StartNpcInteraction` (`FUN_393be280`, 606 B)

Order [R]:
1. `FUN_393bc620`/`FUN_393bc660` (clear/refresh), `FUN_393be0e0(skill)`:
   * skill 12083 (auction) and 13238 (warehouse) are checked against the player's money/bank
     access (`FUN_398a7a30(3/6, …)`); failure shows the returned text and aborts;
   * other skills pass.
2. player exists; if mounted → `0x14c`.
3. if the target unit is null → return 0.
4. if the target changed (`manager+4 != unit->objId`):
   * `FUN_393be180(1)` cancels the previous NPC interaction (sends `CSInteractNPCEnd` 0x066 when
     the previous target was kind 1);
   * else the 500 ms rate limit `FUN_393b2c70(now, manager+0x10)`.
   `manager+0x10 = now + 500`.
5. target gate `FUN_393c01d0(player, target)` (below). Non-zero:
   * 0x166 → silent return;
   * others → `FUN_39428370(code)`.
6. range `FUN_393bf270(skill, player, target, changed)`; if too far it starts the auto-walk and
   returns 1 (the interaction is deferred until arrival).
7. `FUN_39429e40(target->objId, empty)` selects the target.
8. kind 3 (vehicle/slave) → `FUN_393c0040`; kind 6 (house) → `FUN_393c0320`; else:
   * `manager+8 != skill` → close the old handler (`FUN_393bec00`) and
     `manager+0xc = FUN_393c1310(skill, target, 1)` (handler name);
   * `manager+8 = skill`;
   * `FUN_393bf310(...)` (mounted/vehicle/loot/pvp services) → if handled, return;
   * `FUN_393bf500(...)` (quest) → if handled, return;
   * `FUN_393c1280(skill, target)` (service registry handler);
   * if target kind 1 → `CSInteractNPC` 0x065 (`FUN_397ab740(objId, changed)`);
   * if the target changed → `manager+4 = objId` and `FUN_393bfc70(player, target, facing)`.
9. return 1.

### 4.2 Target gate (`FUN_393c01d0`, 329 B)

Returns 0 = allowed, otherwise a text id [R]:

| condition | result |
|---|---|
| player has a cast/channel (`+0x3ac4`/`+0x3ac8` non-empty) | 3 |
| player's current interaction target (`+0x3af0`) is kind 2 | 0x66 |
| player's `+0x3b00` non-empty | 0xdf |
| target kind 6 and the house object's `+0xa8 == 1000` | 3 |
| mouse-look state and target kind 2 | 3 |
| target not mounted and kind not 3/6, relation check `FUN_39027f40` says hostile | 0x166 |

### 4.3 Range and auto-walk

`FUN_393bf270(skill, player, target, flag)` [R]:
* returns 0 immediately for `flag == 0`, skill 12077 (mount), or skill 10994 (cancel);
* uses `FUN_39093860(player, target, 9.0, 0)` = **3.0 m**; 12.25 (3.5 m) when the target's
  `+0x3c26` is set;
* too far → `DAT_3a5a4dcc = 1` and `FUN_390a6af0(1 or 3, target+8 /* actor id */,
  DAT_3a17f56c, skill)` starts the walk-to; returns 1.

The walk-to/arrival path is the movement controller (see `re-mounts-full.md`); the interaction is
re-entered when the player arrives because `FUN_393be280` is called again by the bar/queued action.

### 4.4 `CSInteractNPC` and `CSStartInteraction`

* `CSInteractNPC` **0x065** — `bc npcObjId, bool isTargetChanged`; writer
  `FUN_397ab740(npcObjId, byte)` called from `FUN_393be280` only when `target+0x410 == 1` (NPC).
* `CSInteractNPCEnd` **0x066** — `bc objId`; writer `FUN_397ab760`, called by
  `FUN_393be180(1)` when the previous target was kind 1.
* `CSStartInteraction` **0x068** — `bc npcObjId, bc objId, i32 extraInfo, i32 pickId,
  u8 mouseButton, i32 modifierKeys`; writer `FUN_397ac0a0`. Sent by `FUN_393be030` (click path)
  and by the service handlers when they need the server's skill list.
* `CSSelectInteractionEx` **0x06B** — `bc targetId, i32 interactionEx, i32 var1`; writer
  `FUN_397ac0d0`, sender `FUN_393b3120`.
* `CSCofferInteraction` **0x06C** — `bc objId, u8 open`; writer `FUN_397ac0f0`, sender
  `FUN_3902e190`.

### 4.5 Service routing (the handler registry)

`FUN_393c14c0` fills a global `std::map` at **`0x3a17f58c`** with 17 entries
(handler vtables read from `.data`) [R]:

| skill | tag (handler object +4) | service (DB `skills.name`) | handler entry | behaviour |
|---|---|---|---|---|
| 12081 | `skill_trainer` | learn skill | `0x393bf750` | `NPC_INTERACTION_START` 0x6B |
| 12082 | `ability_change` | ability change | `0x393c10d0` | 0x6B |
| 12083 | `auction` | auction | `0x393c10d0` | 0x6B |
| 12085 | `teleporter` | teleport | `0x393c10d0` | 0x6B |
| 12086 | `blacksmith` | repair | `0x393c10d0` | 0x6B |
| 12087 | `store` | store | `0x393c10d0` | 0x6B |
| 12195 | `stabler` | pet heal | `0x393c10d0` | 0x6B |
| 13238 | `bank` | warehouse | `0x393c10d0` | 0x6B |
| 13356 | `talker` | talk | `0x393bf8d0` → `0x393bee90` | greeting bubble |
| 13736 | `expedition` | expedition | `0x393c10d0` | 0x6B |
| 14866 | `trader` | sell backpack | `0x393c0500` | house/zone checks |
| 15297 | `repairman` | pet heal 2 | `0x393c10d0` | 0x6B |
| 16376 | `specialty_trader` | sell specialty | `0x393c05f0` | trade pack |
| 16912 | `siege-merchant` | territory info | `0x393bf8f0` → `0x393bf1b0` | `0x6B` |
| 17063 | `priest_recover_exp` | pray | `0x393bf7c0` | 0x6B |
| 17135 | `specialty_store` | buy specialty | `0x393bf840` | zone check + `CSListSpecialtyGoods` 0x044 |
| 23478 | `look_converter` | item look change | `0x393c1180` | reset trade + 0x6B |

The tag strings are stored **inline** at `handler_object+4` (`0x39ac7a34`, `0x39ac7a7c`, …); the
generic handler `FUN_393c10d0` copies that string to the 0x6B event payload. The handler objects
themselves are at `0x39ac7a30`, `0x39ac7a78`, … and their first dword is the vtable.

`FUN_393c1280(skill, npc)` looks the skill up (`FUN_393c11c0`) and calls the vtable's first
method; if absent it falls back to `FUN_393c0ca0`, which searches the NPC's extra skill list
(`npcDesc+0x90`) and casts the skill at the NPC (`thunk 0x39397e50`).

The special skill ids that bypass this registry:
* 12076 occupy / 13863 passenger seat / 19165 destroy → `FUN_39362f50`
* 12077 mount / 13719 resurrect → `FUN_39365230`
* 12078 board → `FUN_393565c0`
* 12079 loot → `FUN_393beb30`
* 12660 battlefield → recruiter level check (`FUN_393bf310` tail)
* 13877 quest → `FUN_393bf500` (§4.6)
* 12079/14047 in the bar → `FUN_393bc7e0`/`393bc8c0` (§5.4)

### 4.6 Quest routing (`FUN_393bf500`, 576 B)

Runs only for skill **13877** (`DAT_39ac79c8`) [R]. It reads the NPC's quest lists through
`FUN_39340280(npc->objId)` → `FUN_39424820` and compares the first quest id:

| first id | UI event 0x6B payload |
|---|---|
| 12089 (`DAT_39ac799c`) | `("quest", "start", npcId)` (`0x3999d2f4`, `0x3999dcd4`) |
| 12090 (`DAT_39ac79a0`) | `("quest", "talk", npcId)` (`0x399cb45c`) |
| 12092 (`DAT_39ac79a4`) | `("quest", "complete", npcId)` |

The Lua `npc_interaction.alb` then selects `talk`/`complete`/`start` and calls
`X2Quest:CallQuestUi(0/2, qtype, npcId)`; this matches `re-quests.md` §2.2. `FUN_393be850`
injects the same **13877** quest entry into the server list.

### 4.7 Talk and greeting

`FUN_393bee90(npc)` [R]:
* `FUN_393bed90(npc)` scans the NPC's `OnClientGreeting` acts (`FUN_39340210` → descriptor,
  `+0xf4` vector, `+0xc` name, `+0x1c..+0x20` text list), filters the act by requirements
  (`FUN_3936cda0`) and picks a **random** line (`CryRandom`); no act → default text
  `DAT_3a17f560`.
* Then `FUN_39696200` resolves the localized text and
  `thunk_FUN_3a98400a(...)` opens the chat bubble; `FUN_393f4ee0` registers/plays it.

`FUN_393bfc70(player, npc, face)` faces the NPC (`FUN_393b31e0`), stops the auto-attack
(`FUN_39360920`), plays the `fist_ac_talk_12` talk animation, and points the camera at the NPC.

### 4.8 NPC status and end

* `SCNpcInteractionStatusChanged` 0x080 → `FUN_391d91a0` → `FUN_393be250`: sets
  `unit+0x5fd4` (has interaction/greeting) after resolving the unit; the bar reads it.
* `SCNpcInteractionEndedByZone` 0x07C → `FUN_391d9000` → `FUN_393be180(0)`: if the ended objId is
  the manager's current target, cancel without sending 0x066.
* `X2Interaction:CancelNPCInteraction()` → `FUN_3947da90` → `FUN_393be180(1)`.

---

## 5. D — loot interaction

### 5.1 Manager and rows

`DAT_39aff9b8` (0x30 bytes, ctor `FUN_394fa610`) [R]:

| off | type | meaning |
|---|---|---|
| +0x00 | u16 | current owner type (server `LootOwnerType`; 0 = none) |
| +0x02 | u16 | current owner type check (see `FUN_394fa6c0`) |
| +0x04 | u32 | current owner objId |
| +0x08 | u8 | bag open |
| +0x0c/+0x10/+0x14 | row* | row vector begin/end/cap |
| +0x20 | tree | lootable-object set (type,id keyed) |
| +0x24 | u32 | set size |

A row is **0xD0 bytes** [R, `FUN_394f9790`, `FUN_394fa6c0`]: `+0x08 u32 itemId`,
`+0x0c u32 index/ownerType`, `+0x10 u32 type/empty sentinel`, `+0x18 i32 count remaining`,
`+0xc8 u8 in-flight`.
The server packet layouts (`aaemu-game/src/systems/economy/loot_packets.rs`):

* `SCLootableState` 0x0F6: `u16 unused, u16 ownerType, bc ownerId, u8 0, bool hasLoot`
  (the client handler reads `+0x10` as the packed `type<<16`, `+0x14` as the owner id,
  `+0x18` as the flag).
* `SCLootBagData` 0x0F8: `u8 count` + `Item` rows + `bool lootAll`.
* `SCLootItemTook` 0x0F9: `u32 itemTemplateId, u16 itemIndex, u16 ownerType, bc ownerId,
  u8 0, i32 count`.
* `SCLootItemFailed` 0x0FA: `u32 itemTemplateId, u16 itemIndex` (handler reads `+0x10/+0x14`)
  and a reason at `+0x18`.
* `SCUnitLootingState` 0x0F7: `bc unitObjId, u8 looting`.
* `SCLootDice` 0x0FB / `SCLootDiceNotify` 0x0FC / `SCLootDiceSummary` 0x0FD.

### 5.2 Packet handlers

| opcode | handler | effect |
|---|---|---|
| 0x0F6 | `FUN_391d9780` | `hasLoot ? FUN_394fa110 : FUN_394faf40` |
| 0x0F7 | `FUN_391d9810` | `FUN_393546f0(packet+0x10)` looting stance |
| 0x0F8 | `FUN_391d9890` | `FUN_394fa9a0(bag body, lootAll)` |
| 0x0F9 | `FUN_391d9910` | `FUN_394fa6c0(item, index, type, owner, count)` |
| 0x0FA | `FUN_391e56d0` | `FUN_394f9c20` + error toast |
| 0x0FB | `FUN_391d9990` | `FUN_394fae70` + `FUN_394fb250` |
| 0x0FC | `FUN_391d9a10` | dice-notify event |
| 0x0FD | `FUN_391d9b50` | `FUN_394f9410` summary |
| 0x0E2 | `FUN_391d9bd0` | team looting rule |

### 5.3 Open / take / close

* **Open bag** `FUN_394fb010(owner, flag)` (unit) / `FUN_394fb160(owner, flag)` (npc):
  refuses when the player is mounted (`+0x3c26`); returns 2 if no bag / 3 if already the current
  bag; otherwise closes the old bag and sends `CSLootOpenBag` **0x08E** with
  `FUN_397abed0`/`FUN_397abef0`.
* **Take item** `FUN_394f9790(slot)`:
  * `slot < 0` or out of range → false;
  * empty row (`+0x10 == DAT_3a5ba4ac`) or `+0xc8 != 0` → false; set `+0xc8 = 1`;
  * build `CSLootItem` **0x08F** `{itemIndex, ownerType, ownerObjId, u1, u2}` with
    `FUN_397abf10` and send.
* **Close bag** `FUN_394fab30`: no-op when `+2 == 0`; sends `CSLootCloseBag` **0x090**
  (`FUN_397abf40(type, ownerId)`), resets the manager, fires `LOOT_BAG_CHANGED` 0x7E.
* **Dice** `X2Loot:DoDiceAction(key, roll)` → `FUN_394fb250` + `FUN_394fa770`;
  `RequestNextDiceItem` → `FUN_394fb250` + `FUN_394fad50`.

### 5.4 F on a corpse / loot bag

The bar for an NPC whose `+0x3c26` is set (mounted/attached) and that is lootable gets
**14047** (loot all) and **12079** (loot) from `FUN_393bb9d0`
(§2.2). `FUN_393b1de0` intercepts those two ids before the normal dispatch:

* `FUN_393bc7e0(i)` — bar loot gate: if `bar[i]` is **14047** or **12079** and the current loot
  source (`DAT_3a17e6f4+4`) has a bag (`FUN_394f9ec0`), it opens it (`FUN_394fb010`) with
  `loot_all = (bar[i]==14047)`. If the bag was already open (return 3), 14047 takes **every row**
  (`FUN_394f9360` count → `FUN_394f9790` for each), 12079 takes **slot 0**.
* `FUN_393bc8c0(i)` — loot-source gate used for every bar press and for the `14047` case of
  `FUN_393b3ec0`: if `DAT_3a17e6f4` is a lootable source (`FUN_394f9f20`), it opens it with
  `FUN_394fb160(source, i==0)`; if already open, `i == 0` takes every row, otherwise slot 0.
  `FUN_393b3ec0` calls it with `i = 1` (slot 0).

Both return true when they handled the press, so the bar does not send a normal skill.

### 5.5 Client-side loot rules

* You cannot open a bag while mounted/attached (`FUN_394fb010`/`fb160`).
* You cannot take a row that is empty or already in flight (`FUN_394f9790`); the in-flight flag is
  cleared by `SCLootItemFailed`.
* The row count is decremented by `SCLootItemTook`; the row is removed at zero and the bag list
  event 0x7E is fired.
* Range, permission, loot rule (free-for-all / round-robin / master looter) and dice are server
  decisions; the client only renders the packets and the 0x7E/0x7F events.

---

## 6. Numeric parameters

| value | number | source |
|---|---|---|
| interaction action indices | 31/32/33/34 | `0x39ac8e68` table, `.data` |
| bar capacity (native) | 0x100 | `FUN_393bcb90`, `FUN_393bcc20` |
| bar buttons (Lua) | 10 | `dynamic_action_bar_view.alb` |
| bar key bindings | 4 (F/G/H/J) | `default_binding.g` |
| doodad use range | 2.1 m | `0x399a7660`, `FUN_390f7b40` cvar `+0x2a0` |
| NPC use range | 3.0 / 3.5 m | `0x39ac79fc = 9.0`, `0x39ac7a00 = 12.25` |
| dynamic bar distance cvar | 25 m | `0x3999f85c`, `FUN_390f7b40` cvar `+0x29c` |
| pick max distance | 2000 | `0x399a1ec0`, `FUN_39428d80` |
| pick RWI masks | 0x8002048f / 0x200048f | `FUN_39428d80` |
| LOS mask / rwi | 0x2048f / 4 | `FUN_393bc980` |
| mate candidate extra | 0.8 m | `0x3999d7e4`, `FUN_393601a0` |
| interaction rate limit | 500 ms | `FUN_393b3ec0` (`+0x44`), `FUN_393be280` (`+0x10`) |
| interaction effect tick | 100 ms → 0.1 s | `FUN_393b7800`, `FUN_393b7140`, `0x3999cf30` |
| effect watchdog | 1000 ms | `FUN_393b5f60` (`999 < +0x58`) |
| NPC list injection cap | 8 | `FUN_393be850` (`local_38 < 8`) |
| loot rows per bag | 255 | `FUN_394fa9a0` (`iVar3 < 0xff`) |
| loot row size | 0xD0 | `FUN_394f9790` stride |
| coffer item row | 0xC0 | `FUN_391d5590` stride |
| hang/climb throttle | 2000 ms | `FUN_393b8f40` (`+0xb20 + 2000`) |
| interaction handler entries | 17 | `FUN_393c14c0` |
| empty sentinels | 0 in the dump | `DAT_3a17d7cc/d7d0/d7d4/d7d8`, `DAT_3a17e2xx` |

---

## 7. Proposed port order (smallest testable leaves first)

**Phase A — pure tables and math (synthetic inputs, no world)**

1. Action-name table and lookup: `FUN_393f8d20` + the `0x39ac8e68` names, the handler table at
   `0x3a59f1c0+0x13c`, the four stubs `0x393f8290…c0`, `FUN_393b2b70`.  The lookup is byte-exact
   (see §10.2).
2. Candidate-set insert/order: `FUN_393bab40`, `FUN_393ba1b0`, `FUN_393ba370`, `FUN_393bd240`,
   `FUN_393bb320`, `FUN_393b6e90`, `FUN_393c0ff0`.
3. Permission switch `FUN_393b3300` (synthetic flag rows for perm 0..8) and the func-type switch
   `FUN_393bc260` with hand-built func descriptors (the case → event/packet decisions).
4. Range leaves `FUN_39093800`/`39093860`/`3989bf90` with two synthetic actors, and the
   squared-limit conversion in `FUN_393bf270`.
5. Loot row bookkeeping: `FUN_394f9360`, `FUN_394f9620`, `FUN_394fa6c0`, `FUN_394f9c20`,
   `FUN_394fa110`/`faf40` with a synthetic 0x30 manager and zeroed 0xD0 rows.
6. Event id table `FUN_39214380` (extract the 626 `(id, name)` pairs, ids 0..0x271, as a Rust enum).

**Phase B — read-only bar model**

7. Bar object `FUN_393b23d0`, `393b2530`, `393b2490`, `393b24e0`, RefreshActions `393b25a0`,
   Show/Hide `393b1eb0`, Update `393b2760`, queries `39442370`/`39442400`/`394424c0`.
8. Candidate builders with captured descriptors: `FUN_393bd290`, `393bcf00`, `393bb9d0`,
   `393bb420`, `393bbf10`, `393bbc90`, `393bbd70`, `393bb910`.
9. `MakeCandidate` `393bdad0` with a stubbed pick query and a captured camera/player state
   (the indicator is read-only; compare the bar fields and the 0x19D/0x19E events).

**Phase C — NPC flow (stateful, packet/event golden)**

10. `FUN_393be0e0`, `FUN_393b2c70`, `FUN_393c01d0`, `FUN_393bf270` (pure-ish gates).
11. `FUN_393be280` + `FUN_393be180` + `FUN_393bfc70` (packet hook: 0x65/0x66).
12. `FUN_393bf310` + `FUN_393bf500` + service registry (`393c14c0`, `393c1280`, `393c1310`,
    `393c0ca0`) — event stream 0x6B + 0x68.
13. `FUN_393be850`, `FUN_393be250`, `FUN_393bed90`/`393bee90` (list injection, status, greeting).

**Phase D — doodad flow**

14. `FUN_393b3ec0` + `393b3d50` + `393bc480` + `393b9a80` + `393b7c40` + direct
    `thunk 0x39397e50` (packet hook: 0x052; compare the target block).
15. `FUN_393b7440` + `FUN_393b3120` (0x07E / 0x06B writers).
16. `FUN_393bc260` handlers one by one, and the interaction-state machine
    `393b7b10`/`7b50`/`7aa0`.
17. Phase/coffer/vegetation: `39035280` + `393a6c60`, `3902e0f0`, `3902e190`,
    `391d5590`, `391d56c0`, `391da940`, `393b7800` + `393b7140`, `393b5f60`, `393b6220`.

**Phase E — pick and click**

18. `FUN_39428d80` cursor ray with captured camera/cursor inputs (compare the pick block and the
    three globals); `FUN_3942c640` with the same.
19. `FUN_390f4960` / `FUN_390f3780` / `FUN_393be030` / `FUN_393b79a0` / `FUN_39429d00` (the
    click→request decision, golden 0x68/0x6B/0x052).
20. `FUN_393bddf0` / `393bdf10` and the `FUN_393601a0`/`3902f010` query shapes.

**Phase F — loot**

21. `FUN_394fb010`/`fb160` open, `FUN_394f9790` take, `FUN_394fab30` close, `FUN_394fa9a0` bag
    data, `FUN_394fa6c0` took, `FUN_394f9c20` failed (packet hook 0x8E/0x8F/0x90).
22. `FUN_393bc7e0`/`393bc8c0` (F on a corpse).

Rationale: A and B are callable with captured buffers and pure state and give exact comparisons;
C/D/F add packets and events whose oracle is the packet/event hook; E needs the live camera and is
best verified against captured cursor/ray samples first.

---

## 8. Unknowns

1. **U1 — pick result kind and the indicator ray.** `FUN_393bd650` returns a node whose `+8` is a
type (1 = unit, 2 = doodad); the exact enum and the cone/ray arguments built by
   `FUN_393601a0`/`FUN_3902f010` are only partly recovered (the decompiler drops the vector
   arguments). Capture a live pick with the driver and read `FUN_39428d80`'s output block.
2. **U2 — SCDoodadPhaseChanged field order.** The handler passes packet `+0x10, +0x18, +0x14,
   +0x1c` into `FUN_393a6c60`; which is `timeLeft`, `puzzleGroup` and `itemTemplateId` is not
   proven. The server writes `objId, funcGroupId, timeLeft, puzzleGroup, itemTemplateId`; the
   mapping to the doodad's `+0x2a8`/`+0x354` slots needs a live phase change.
3. **U3 — doodad func-type enum.** Only cases 0x34, 0x3D, 0x3F, 0x40, 0x46, 0x47, 0x48, 0x4A,
   0x51, 0x53, 0x55, 0x56, 0x57 are tied to an event/string; the rest are inferred from the DB
   `actual_func_type`
   candidates. Align the client switch with the DB by feeding one doodad per type.
4. **U4 — service handler vtables.** The 17 registry entries' handler vtables are read but the
   class names are absent (no RTTI names in `.rdata`); the "which window" column is inferred
   from the skill id and the emitted event.
5. **U5 — CSStartInteraction field values.** The writer `FUN_397ac0a0` stores
   `{+0xc npcObjId, +0x10 const 0x3a5e1158, +0x14 arg2, +0x18 arg3, +0x1c byte, +0x20 arg5}`;
   the click globals `DAT_3a5a4dbc/4dcc/4dd0` are runtime state, so the semantic of each field
   (`extraInfo`, `pickId`, `mouseButton`, `modifierKeys`) needs a live capture.
6. **U6 — the `GetNPCInteraction*` natives are stubs.** `GetNPCInteractionMainMsg`,
   `GetNPCInteractionMsg`, `ConfirmNPCInteraction`, `SelectNpcInteraction` return nil / do
   nothing (`0x3947d9d0…0x3947da30`), and `GetNPCInteractionCount` returns 0. The live NPC menu
   is the `SCNpcInteractionSkillList` + 0x6B-event path; confirm no 1.2 UI uses the stubs.
7. **U7 — interaction error texts.** The text ids in §3.9 index the encrypted `ui_texts`; the
   key strings are at `0x399f3ff0–0x399f44xx` but the id→key table is not in the extract.
8. **U8 — `dynamic_action_bar_distance`.** Registered at `DAT_39aefb98+0x29c` = 25 m but no
   x2game reader was found; the bar is actually driven by the pick range (2.1/3 m) and the
   candidate sets. Determine whether the UI or a 10.x-only native consumes it.
9. **U9 — cast-cancel on movement.** As in combat U9: the interaction code has no movement
   cancel; confirm with a breakpoint on `FUN_39395b50` during a gathering cast + W.
10. **U10 — 0x07E world interaction list layout.** Our server never sends it; the client layout
    is inferred from `FUN_393b7440`'s 16-dword copy. Capture a real doodad list if one exists in
    the 1.2 data.
11. **U11 — deferred modes 2/3.** `FUN_393b3000` re-enters `FUN_393b3ec0` with mode 2/3; the exact
    window (ship boarding, merchant interaction, level gate) is read from the case bodies but not
    exercised live.
12. **U12 — the coffer item row.** `SCCofferContentsUpdate` reads 0xC0-byte item rows into the
    coffer manager; the row layout is the inventory `Item` structure (see `re-inventory`), not
    re-mapped here.

---

## 9. Cross-check against the existing notes

Agreements:

* `re-quests.md`: `CSStartInteraction` 0x068 and `SCNpcInteractionSkillList` 0x07B are the NPC
  path; `CSInteractNPC` 0x065 / `CSInteractNPCEnd` 0x066; the `quest`/`start`/`talk`/`complete`
  routing; the 3 m generic interaction gate (`DAT_39ac79fc`); the quest UI events 0xEA/0xF0.
* `re-crafting.md`: `max_interaction_doodad_distance = 2.1 m` and the 25 m
  `dynamic_action_bar_distance`; `CSSelectInteractionEx` is the logged-only doodad request.
* `re-housing.md`: the dynamic action bar is the "interaction indicator" and is filled from the
  received unit skill vector plus the current doodad/slave; `FUN_39442370/39442400` are the
  count/skill getters; `FUN_393b8700` is the public-farm info func (now **confirmed** by the
  0x102 event).
* `port-mount-anim/MAP.md`: F = `do_interaction_1`, the dynamic bar widget, the icon binding
  `FUN_394e6c10`; no native F entry for mounting an own mate.

Sharpened or corrected:

* The F/G/H/J dispatch is now statically linked: action indices 31–34 →
  `0x393f8290…c0` → `FUN_393b2b70` → `FUN_393b1de0`; `quick_interaction` (72) is an empty stub.
* The bar is an **ordered set keyed by a 32-bit key** (the skill id for the normal additions;
  ascending, de-duplicated), then copied to a 0x100-entry vector and a 10-button UI; it is not an
  append-only list.
* Doodad use is a direct `CSStartSkill` 0x052 with a **type-4 doodad target** (objId), not a
  `CSStartInteraction`; the `CSSelectInteractionEx` 0x06B path is only the Lua
  `SelectInteraction` request and the (server-unused) 0x07E answer.
* The doodad "default click" skill is the first candidate from the doodad's current phase funcs
  (`FUN_393bd290` + `FUN_393b79a0`), and the client already gates it with 2.1 m, permission
  (`perm_id`), quest requirements and LOS before sending the request.
* NPC service windows are opened **client-side** by `NPC_INTERACTION_START` 0x6B with a string
  tag (`"store"`, `"bank"`, `"auction"`, `"skill_trainer"`, `"teleporter"`, `"blacksmith"`,
  `"stabler"`, `"expedition"`, `"talker"`, `"trader"`, `"specialty_trader"`, `"specialty_store"`,
  `"priest_recover_exp"`, `"siege-merchant"`, `"look_converter"`, `"quest"`, `"recruiter"`), from
  the handler registry and the func-type handlers, before/without the server list.
* Loot has a real client-side state machine (`LootManager`, 0xD0 rows, in-flight flag) and a
  "loot all" bar path; the server still owns range/permission/dice.

---

## 10. Port status after phases A and B

The Rust port lives in `crates/aa-client/src/port/interaction/` (`action`, `candidates`,
`permission`, `functype`, `range`, `loot`, `bar`, `builders`).  Goldens are in
`.../port/interaction/goldens/`; they were recorded from the hidden 1.2 client in the
`combat-golden` lane with `rust/tools/capture_interaction_goldens.py` (attached by pid; spawns
dummy doodads with `/spawn doodad <templateId>` and records the original leaves).  Each module
cites the original `FUN_` addresses; the JSON integer results are compared exactly, the range
distances as `f32` within 1e-5.

### 10.1 Live evidence

| golden | functions | live cases |
|---|---|---|
| `action_names.json` | `FUN_393f8d20` | 112 (all 104 names, near-misses, NULL) |
| `handlers.json` | live `0x3a59f1c0+0x13c` table + `FUN_393f8d70` | 104 |
| `stubs.json` | `0x393f8290..c0` → `FUN_393b2b70` | 12 |
| `candidates.json` | `FUN_393bab40/393ba1b0/393ba370/393bb320/393bd240` | 18 |
| `permission.json` | `FUN_393b3300` | 72 (8 doodads × perm 0..8) |
| `functype.json` | `FUN_393bc260` | 4 resolved rows |
| `range.json` | `FUN_39093800/39093860` | 96 |
| `builders.json` | `FUN_393bd290` / `393bcf00` / `393bb9d0` | 30 (10 doodads, 20 NPCs) |
| (unit tests) | `FUN_393b23d0..393b2760`, `FUN_393b1eb0`, `394f9360/fa6c0/f9c20/f9790` | decompile-derived matrices |

### 10.2 Resolved or corrected

* **`XlCompareString` is byte-exact, not case-insensitive.** `FUN_393f8d20("MOVEforward")`
  returns `0x68`; only the exact lower-case names hit.  `action_index` therefore compares
  exactly.
* **The hotkey table is runtime data.** The live 104 slots at `0x3a59f1c0+0x13c` were dumped;
  `FUN_393f8d70` is just the null-checked lookup + call.
* **`do_interaction_1..4` pass 0..3** to `FUN_393b2b70`, verified with an entry trampoline on
  `FUN_393b2b70`.
* **Candidate container key semantics.** The ordered set is ascending and de-duplicated.  For a
  doodad row the key is the row's bar key (`row+0x18`; e.g. `18784` for the mailbox template
  `7496`) and the value is the row's skill (`11374`); for NPCs the key is the skill id.  The
  `+0x18` vector inside the candidate list holds pointers to the inserted values, **not** the
  bar's raw skill vector.
* **`FUN_393b3300` cases 2/4/5/6/7/8 delegate to other subsystems.** The live answer per perm
  was recorded and is the port's helper input; cases 0/1/3 are decided by the port itself.
* **Range leaves.** `FUN_39093800` works for live doodads (e.g. 55.2 m for a spawned mailbox);
  `FUN_39093860` faults on model-less spawned doodads, so its live cases use unit targets.
  The existing `crate::interaction` sphere/box gap helpers are reused (no duplicate distance
  math); the port adds the `FUN_3989bf90` minimum/early-exit and `FUN_39093800/60` wrappers.
* **Doodad templates/spawning.** Placed/runtime doodads live in `doodad_almighties` (not
  `doods`/`doodads`); `/spawn doodad <templateId>` creates a runtime-only doodad, whose client
  objId is in the `21075+` range and is found with `FUN_39035110`.  The current phase's func
  rows are `desc+0x24..+0x28` (pointers).  The `+0x18` row value is the bar key and `+0x00` the
  skill, as above.
* **The `+3` has-actions byte is not written by `RefreshActions`.** The live bar keeps it 0 even
  when the shown vector is non-empty; the Lua/UI path owns it.  The port leaves it untouched.

### 10.3 Open / not proven live

* **B — bar mutation.** Writing scratch `std::vector` storage into `DAT_39affa0c` and calling
  `FUN_393b25a0` makes the original free non-owned memory and crashes the client.  The bar
  model (`393b23d0/2530/2490/24e0/25a0/2760/1eb0`, `39442370/39442400`) is therefore covered by
  decompile-derived unit tests; live bar **contents/order** are evidenced by the builder sets.
* **A3 — func-type matrix.** `FUN_393bc260` resolves the row through `DAT_3a17e6f4` (candidate
  list); only the current phase row is reachable live, so the full 0x08/0x34/0x3d/0x3f/0x40/
  0x43/0x46/0x47/0x48/0x4a/0x4f/0x50/0x51/0x53/0x54/0x55/0x56/0x57 matrix is unit-tested from
  the switch.  The event hook records the payload pointer, not the id; event ids in this area
  still need a payload decoder.
* **A2 — `FUN_393b6e90`.** Its argument is an interaction-effect set/object, not a skill id, so
  it was not exercised with the bar candidate list.  `FUN_393c0ff0` (map insert) has the same
  unique-insert behaviour as `FUN_393ba1b0` and is fused with it in the port.
* **A4 — loot.** The row bookkeeping (`394f9360/f9790/fa6c0/f9c20`) is unit-tested from the
  decompile; no live loot bag was opened.  `SCLootableState` insert/remove (`394fa110/faf40`)
  is modelled as tree membership only (markers/UI not ported).
* **U1/U2/U3/U5/U7/U8/U9/U10/U11/U12 remain open** as listed in §8; this phase did not touch
  the pick ray, phase-change field order, the numeric func-type↔DB alignment, the
  `CSStartInteraction` field semantics, the error-text table, the 25 m bar cvar, movement
  cancel, the 0x07E layout, deferred modes 2/3 or the coffer row layout.
