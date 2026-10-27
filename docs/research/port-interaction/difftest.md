# Differential tests against the running 1.2 client — interaction

Goal: prove each ported Rust function matches the original by calling the **original** function
inside the hidden 1.2 client (`aadriver.dll`, already injected in-process and running Lua) with
chosen inputs, then comparing with the Rust port. This file assumes the calling conventions and
harness plumbing of [`../port-movement/difftest.md`](../port-movement/difftest.md) §1 and §4 and
[`../port-combat/difftest.md`](../port-combat/difftest.md); only the interaction-specific state and
recipes are listed here.

No original code is shipped; the driver only calls the shipped binary in memory.

## 1. State to capture

| name | what | size | where |
|---|---|---|---|
| `IM` | world interaction manager | `0x60` | `DAT_39aff9bc` |
| `NM` | NPC interaction manager | `0x64` | `DAT_39aff9b4` |
| `LM` | loot manager | `0x30` | `DAT_39aff9b8` |
| `BAR` | dynamic action bar | `0x30` | `DAT_39affa0c` |
| `PICK` | pick block | `0x24` | stack of `FUN_39428d80` (copy it) |
| `PG` | pick globals | `0x18` | `DAT_3a5a4dbc/dc0/dc4/dcc/dd0` |
| `HP` | local player unit | live | `*(DAT_39b02294+0x10c)` |
| `NPC` | current NPC unit | live | `NM+4` → `FUN_39360020` |
| `DO` | current doodad | live | `FUN_39035110(id)` |
| `NPD` | NPC descriptor | live | `FUN_396962c0(*(FUN_39340280(npcId)+0x10))` |
| `DDD` | doodad func descriptor | live | `FUN_393bc200(skill, type, doodad)` / `FUN_3963b250(group)` |
| `ROWS` | loot rows | `0xD0 * n` | `LM+0x0c .. +0x10` |

The driver should snapshot/restore `IM`, `NM`, `LM` and `BAR` (plus the four global pointers) around
each call, so a test cannot poison the next one. The manager globals are runtime-initialized and
must be read after the client has entered the world (`FUN_39174520` created them).

The **send hook** `0x3ac1058c` and the **UI event hooks** `FUN_39424220/39424240/39424260` are the
second oracle; record them as `(opcode, bytes)` / `(eventId, payload)` ring buffers.

## 2. Pure/near-pure leaves (synthetic state, no world)

### 2.1 Action table and bar dispatch

* `FUN_393f8d20(name)` for every one of the 104 names at `0x39ac8e68` and ten near-misses;
  compare the returned index with the Rust enum. Also call the four stubs `0x393f8290…c0` with a
  fake bar whose shown vector has 1 entry and capture the `FUN_393b1de0` arguments (hook it).
* `FUN_393b2b70(i)` and `FUN_39440850(i)` with `DAT_39affa0c` restored: compare the call target
  and the index passed to `FUN_393b1de0`.

### 2.2 Candidate set / order (`FUN_393bab40`, `393ba1b0`, `393b6e90`, `393ba370`, `393bd240`)

* Build a zeroed 0x40-byte set object (head node with the MSVC `+0x14/+0x15` colour bytes) and
  insert `{skill, extra}` pairs in several orders; record the node keys and the in-order walk.
  Inputs: ids `{10994,12079,13356,13877,14047}`, duplicates, equal keys with different extras.
* Compare: node count, key order, the value at `+0x10/+0x14`, and that duplicates are rejected.

### 2.3 Permission switch (`FUN_393b3300`)

* Inputs: a synthetic 0x40-byte row with `+0x20 = 0..8`, a synthetic doodad/unit with the owner id
  fields, and the player id. For each case compare the boolean. The owner/raid/zone predicates
  need a live-ish actor table; use a real captured doodad and vary the row's `+0x20`.
* Also compare `FUN_393b2c40` (types 6/0x35 vs others) and `FUN_3983b4e0`.

### 2.4 Range leaves (`FUN_39093800`, `39093860`, `3989bf90`)

* Two live objects (player and a doodad/unit at a known distance) or two synthetic object vtables
  whose `+0x18` returns a collider list. Sweep the limit around the real surface gap:
  `0.0`, `gap-ε`, `gap`, `gap+ε`, `2.1`, `3.0`, `9.0` (squared variant too).
* Compare the bool, the `outDist`, and the clamped minimum; the float tolerance is 1e-6.

### 2.5 Loot row bookkeeping (`FUN_394f9360`, `394f9620`, `394fa6c0`, `394f9c20`)

* Synthetic `LM` (0x30) with a vector of 0/1/3 rows of 0xD0; set row `+8/+0xc/+0x10/+0x18/+0xc8`.
* `FUN_394f9360` → count; `FUN_394f9620` → node; `FUN_394fa6c0(item,index,owner,count)` →
  row count/flags/removal; `FUN_394f9c20` → in-flight clear.
* Compare every field of the rows and the 0x7E event count (hook `FUN_39424220`).

### 2.6 Event id table (`FUN_39214380`)

* Not a function to port (it only builds strings), but extract the 626 `(id, name)` pairs from the
  ECX immediates in the decompile (the ids are `(ECX-0x39d22ce8)/4`) and make them a Rust table.
  The harness can verify a sample by asking Lua for the globals (`EVENT_NAME` values).

## 3. Read-only bar model

### 3.1 `DynamicActionBar` (`FUN_393b23d0`, `2530`, `2490`, `24e0`, `25a0`, `2760`, `1eb0`)

1. Snapshot `BAR` + the UI event ring.
2. Call `FUN_393b2490(unitId)` / `FUN_393b24e0(doodadId)`; compare `+0x08/+0x0c/+0x02` and the
   cleared raw vector.
3. Seed the raw vector with `{12079, 13877, 13356}` and call `RefreshActions` (`393b25a0`):
   compare the shown vector, `+0x03/+0x04`, and the 0x19D/0x19E events.
4. Call `Show(true/false)` (`393b1eb0`) and compare the event payload `("npc"/"doodad", id)`.
5. Call the script getters `39442370`, `39442400`, `394424c0` with indices `0, 1, n-1, n, -1` and
   compare the values/nil.

### 3.2 Candidate builders (`FUN_393bd290`, `393bcf00`, `393bb9d0`, `393bb420`, `393bbf10`,
`393bbc90`, `393bbd70`)

* Call with a **captured** live unit/doodad and a zeroed bar; compare the raw vector (skill ids and
  order) and `bar+0x08/+0x0c`. Use all six unit kinds if available; otherwise call the per-kind
  builders directly with a synthetic `+0x410` and the captured descriptor pointers.
* `FUN_393bb420` is best tested with a captured NPC descriptor; vary one flag byte at a time and
  compare the added skill set.

### 3.3 Indicator (`FUN_393bdad0`, `393bddf0`, `393bdf10`)

* Needs a live camera/player and a doodad/NPC in front. Snapshot `BAR`, call the update, and
  compare the pick block, `BAR` ids and shown vector, and the events. Repeat with the doodad out of
  the 2.1 m range, behind a wall (LOS false), and while mounted (skip).
* `FUN_393bc980` can be called separately with a captured target and a zeroed skill array; compare
  the bool for clear/blocked rays.

## 4. NPC flow

### 4.1 Gates (`FUN_393be0e0`, `393b2c70`, `393c01d0`, `393bf270`)

* `FUN_393b2c70(now, last)`: pure matrix `0/last/now/now-1`.
* `FUN_393c01d0(player, npc)`: force the player's cast fields (`+0x3ac4/+0x3ac8`), `+0x3af0`
  target and `+0x3b00`, and the target kind/relation; compare the returned code
  `{0,3,0x66,0xdf,0x166}`.
* `FUN_393bf270(skill, player, npc, flag)`: place the NPC at 2.9/3.0/3.1 m and 3.4/3.6 m for a
  mounted participant; compare the bool and the auto-walk call (`FUN_390a6af0` args).

### 4.2 Start/cancel (`FUN_393be280`, `393be180`, `393bfc70`)

* Snapshot `NM`, the player/NPC and the packet ring.
* Call `FUN_393be280(skill, npc, 0, 1)` for skills `{13356, 13877, 12083, 12089, 10994}` and
  target kinds `1,2,3,5,6`; compare `NM+4/+8/+0xc/+0x10`, the 0x6B/0x68/0x65/0x66 events and the
  talk animation request.
* Call `FUN_393be180(1)` with an NPC target; compare the 0x66 bytes and the camera/anim calls.

### 4.3 Service registry (`FUN_393c14c0`, `393c1280`, `393c1310`, `393c0ca0`)

* After init, read the 17 map entries and compare the skill → vtable table with Rust.
* Call `FUN_393c1280(skill, npc)` for each of the 17 ids; compare the UI event stream (0x6B tag,
  0x44 packet, 0x201/0x8A/0x1A4 from the sub-services) and the packet bytes.
* `FUN_393c1310(skill, npc, 1)` → compare the 64-char key (e.g. `"recruiter"`).

### 4.4 Quest routing (`FUN_393bf500`) and greeting (`FUN_393bed90`, `393bee90`)

* With a captured NPC that has offer/talk/ready quests, call `FUN_393bf500(13877, &lists, npc)`
  and compare the 0x6B payloads `("quest","start"/"talk"/"complete", npcId)`.
* `FUN_393bed90(npc)` is deterministic per act except the random choice; either seed/patch the RNG
  or compare the set of allowed lines, the requirement filter, and the default text. Then call
  `FUN_393bee90` and compare the chat bubble text.

## 5. Doodad flow

### 5.1 `FUN_393b3ec0` — the central stateful test

1. Snapshot `IM`, the doodad, `BAR` and the packet/event rings.
2. Place the player at 1.9/2.1/2.2 m and call `FUN_393b3ec0(skill, 0, doodad, 1)`.
3. Matrix:
   * mounted / climbing / in combat / with a pet target;
   * skill missing, skill 10994, doodad null;
   * doodad level gate (`+0x4b4`) above/below the player;
   * each func type 8/0x34/0x3d/0x3f/0x40/0x43/0x46/0x47/0x48/0x4a/0x4f/0x50/0x51/0x53/0x54/
     0x55/0x56/0x57 (use a captured doodad/custom func descriptors when possible).
4. Compare `IM+4/+0x10/+0x38/+0x44/+0x5c`, `BAR` ids, the `CSStartSkill` bytes from the hook
   (`skillId`, caster `{0,player}`, target `{4,doodadObjId}`, flag `0`), the auto-walk args, and
   the UI events.
5. Repeat with `mode = 2/3` to prove the deferred gates are skipped.
6. `FUN_393b7440` with a synthetic 16-dword body (doodad id at `+4`, skill at `+0x10`, count
   `+0x38`); compare the same outputs and the auto-use target.

### 5.2 Func-type handlers (one per case)

For each case in §3.4 of `MAP.md`, call `FUN_393bc260(skill, 0, doodad)` with the matching phase
func and compare:
* the UI event id + payload (`0x1A4`, `0x201`, `0x8A`, `0x102`, `0x6B`, `0x243`, `0xFD`, …);
* the `DAT_3a17e6a0/6a4` state and the `FUN_393b7b10/7b50/7aa0` transitions;
* any packet (`0x0E9` for the phase func, `0x0C6` for the coffer open).
The case-0x55/0x56/0x57 handlers only fire a 0x6B tag and are the simplest; the 0x3F/0x0E9 and
0x48/0x102 cases are the next.

### 5.3 Phase change (`FUN_39035280`, `FUN_393a6c60`)

* Build a synthetic `SCDoodadPhaseChanged` body and call `FUN_39035280`; compare
  `doodad+0x220/+0x224/+0x2a8/+0x354`, the model/anim/quest-mark calls and the 1-frame later state.
* Call `FUN_393a6c60` directly with known `(group, a, b, c)` tuples to resolve U2 (which argument
  lands in which field).
* `FUN_3902e0f0` with a phase whose func text is set/unset; compare the 0xFD event.

### 5.4 Coffer/vegetation/effects

* `FUN_3902e190(doodad)` → compare the 0x6C bytes `{bc objId, u8 1}`.
* `FUN_391d5590` with a synthetic coffer body (count, slot types, 0xC0 rows); compare the coffer
  manager rows and the 0x1F/0x20 game events. Repeat out of range (2.2 m) to prove no update.
* `FUN_391d56c0` with an error and a success body; compare the toast (text id) and stack refresh.
* `FUN_393b7800(unit, doodad, 100)` + `FUN_393b5f60` on a frozen clock; compare the effect list,
  the removal time, the log line, and `FUN_393b6220` cancellation.
* `FUN_391da940` with a synthetic `SCVegetationCutdowning` body; compare the effect source/target.

## 6. Pick/click

* `FUN_39428d80(cursorX, cursorY, …)`: use a live world with a known doodad/unit in front; compare
  the returned pick block (`unitId/doodadId/gimmickId/pickId/pos`) and the three globals. Sweep
  the cursor across the object's silhouette. This is the only reliable way to fix U1.
* `FUN_3942c640` on a frozen frame: compare the nearest-unit refinement and the tooltip event.
* `FUN_390f4960` / `FUN_390f3780`: with `DAT_3a5a4dbc/dc0/dc4` forced, compare the calls
  (`393be030` args / `393b79a0` / `39429d00`) and the packet ring.
* `FUN_393bd650(pickBlock)` with synthetic blocks for kind 0/2/3 and a candidate list; compare the
  returned kind/object pointer.

## 7. Loot flow

* `FUN_394fb010(owner, flag)` / `FUN_394fb160`: place the player mounted/not, same/different owner;
  compare the return code (0/2/3), the 0x8E bytes and the close-if-open call.
* `FUN_394f9790(slot)`: slots `-1, 0, n-1, n` on a zeroed row vector with/without `+0xc8`; compare
  the 0x8F bytes, the row flag and the empty-row rejection.
* `FUN_394fab30`: closed/open manager; compare the 0x90 bytes and the reset.
* `FUN_394fa9a0` with a synthetic 0x0F8 body (count 1/3, lootAll true/false, mounted player);
  compare the rows, the 0x7E event and the stance calls.
* `FUN_393bc7e0/393bc8c0` with a row vector and an open/closed bag; compare the open+takes and the
  return value.

## 8. Harness exports (suggested)

```c
extern "C" __declspec(dllexport) int aa_interact_call(int which, const void* args, int nargs,
                                                      void* out, int out_cap);
extern "C" __declspec(dllexport) int aa_interact_capture(int slot);   // IM + NM + LM + BAR
extern "C" __declspec(dllexport) int aa_interact_restore(int slot);
extern "C" __declspec(dllexport) int aa_interact_events(int on);      // 39424220/40/60
extern "C" __declspec(dllexport) int aa_interact_packet(int on);      // 3ac1058c
extern "C" __declspec(dllexport) int aa_interact_pick(int x, int y);  // 39428d80
```

`aa_interact_call` dispatches by id (0 = action lookup, 1 = set insert, 2 = permission, 3 =
distance, 4 = bar refresh, 5 = bar show, 6 = ExecuteDynamicAction, 7 = StartNpcInteraction, 8 =
StartDoodadInteraction, 9 = world list, 10 = phase change, 11 = loot open, 12 = loot take, …),
validates the pointers against the captured managers, and copies the returned bytes to Lua.

Because several functions are `__thiscall` with the `this` in `ECX` and stack arguments (and Ghidra
sometimes prints the `ECX` object as a normal parameter), each wrapper should set `ECX` explicitly
and push the stack arguments in reverse order. The wrappers in `port-combat/difftest.md` §4 are the
template; add a `__thiscall` trampoline if the driver does not expose one.

## 9. Tolerances and failure policy

| class | tolerance |
|---|---|
| ids, skill ids, indices, result codes | exact |
| times (ms), durations, rate limits | exact integer |
| distances / positions | 1e-6 relative |
| vectors (bar candidates, loot rows) | exact order and size |
| event ids / payload integers | exact |
| packet bytes | exact (except U5 fields) |
| strings (bar labels, service tags) | byte-exact |

A mismatch logs `(function, inputs, original, rust, delta)` under
`data/extract/aa-interaction/` (git-ignored) and keeps the seed case, as in the movement harness.

## 10. Open items the harness must settle

* **U1** — the pick block kind (`result+8`) and the indicator cone/ray arguments; sweep the cursor
  and read `FUN_39428d80`'s output and `FUN_393601a0`/`3902f010` stack slots.
* **U2** — the SCDoodadPhaseChanged field order; call `FUN_393a6c60` with distinct values.
* **U3** — align the func-type switch values with `doodad_funcs.actual_func_type`.
* **U4** — the service handler vtables: capture `0x6B` tags per skill and compare with the
  "which window" column.
* **U5** — the `CSStartInteraction` field semantics (`extraInfo`, `pickId`, `mouseButton`,
  `modifierKeys`); read the writer's stack arguments at entry and the click globals.
* **U6** — confirm the `GetNPCInteraction*` stubs are unused by the 1.2 Lua.
* **U7** — the interaction error text ids → keys (find the `ui_texts` rows in the live client).
* **U8** — whether `dynamic_action_bar_distance` is read by the UI/10.x only.
* **U9** — movement cast cancel: breakpoint `FUN_39395b50` during a gathering cast + W.
* **U10** — the 0x07E world list layout (our server never sends it).
* **U11** — the deferred mode 2/3 semantics.
* **U12** — the coffer row layout (inventory `Item`, see `re-inventory`).
