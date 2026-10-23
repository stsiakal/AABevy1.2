# Interaction structures (1.2 x2game)

Companion to [`MAP.md`](MAP.md). Offsets are byte offsets inside the client objects; they were read
from the decompile and checked against the `.data` dump (`D:/aa-re/client12/x2game_unpacked_data.dll`).
Offsets marked **[?]** are read in the code but the field's meaning is not proven. The shared
`Unit`/actor fields (`+0x1aa0` target id, `+0x410` kind/stance, `+0x3c26` mounted/attached,
`+0x3c28` relation bytes) come from `port-movement/structs.md` and `port-combat/structs.md` and are
not re-derived here.

## 1. `ClientDoodad` (the fields this subsystem touches)

The doodad object is a `CommonObject` subclass; the base constructor `FUN_3983b690(objId, model*,
ownerObjId, …)` sets `+0x1f*4 = +0x7c = objId` and `+0x20*4 = +0x80 = model/desc*`. The
`Doodad` constructor `FUN_393a5930` sets the rest.

| off | type | meaning | evidence |
|---|---|---|---|
| +0x08 | u32 | actor/unit id of the object (used when changing the target) | `0x393bd050` (`[ebx+8]`) |
| +0x18 | float[3]+ | position / orientation | `0x393b3ec0` (`FUN_393b31e0(doodad+0x18)`) |
| +0x7c | u32 | **world objId** | `FUN_3983b690`, `FUN_393b3ec0`, `FUN_393a6c60` |
| +0x80 | u32 | model/desc key compared to 6563 in the interaction precheck | `0x393b3d50` **[?]** |
| +0x88 | u32 | owner unit id (`FUN_39360020(+0x88)`) | `0x393b8400` **[?]** |
| +0x218 | ptr | current template/desc pointer | `0x393bd290` |
| +0x220 | u32 | current **func group id** | `0x393a6c60`, `0x393bd290` |
| +0x224 | ptr | current func descriptor (`FUN_3963b250`) | `0x393a6c60` |
| +0x270 | u32 | fifth phase-change field (from `SCDoodadPhaseChanged`) | `0x393a6c60` |
| +0x28c | u32 | cleared when the phase changes | `0x393a6c60` |
| +0x2a8 | u32 | phase time / time-left field | `0x393a6c60` **[?]** |
| +0x354 | u32 | puzzle/phase companion field | `0x393a6c60` **[?]** |
| +0x4b4 | u8 | level of the doodad (used by the level gate) | `0x393b3ec0`, `0x393bf310` |
| +0xd9 | u32 | "has owner/permission" flag in the candidate builder | `0x393bd290` **[?]** |
| +0xdc | u32 | item-template/currency field in the candidate builder | `0x393bd290` **[?]** |
| +0x5f90 | — | not a doodad field: the local player's current **clicked object** id (written by `FUN_393576a0`, read by `0x390f4960`) | `0x393576a0` |

Doodad `+0x410` is read as a kind in the generic object paths (see `FUN_393bcf00`); its meaning on a
doodad (as opposed to a unit) is **[?]**.

## 2. `ClientDoodad` func descriptor (loaded from `doodad_funcs`)

`FUN_3963b1d0(id)` / `FUN_3963b250(group)` are the two sparse-table lookups. The descriptor was
observed at:

| off | type | meaning |
|---|---|---|
| +0x00 | u32 | func id (`doodad_funcs.id`) |
| +0x04 | u32 | actual func id |
| +0x08 | u32 | **actual func type** (the numeric enum switched in `FUN_393bc260`) |
| +0x0c | u32/ptr | actual func data id (passed to `FUN_393703b0`, `FUN_397ac8d0`) |
| +0x10? | u32 | companion id (used by "book/page") **[?]** |
| +0x14 | char* | model/sound name? (`FUN_39411650`) **[?]** |
| +0x18 | ptr/u32 | phase text/name pointer (`FUN_3902e0f0` reads `+0x18`, `+0xc`) **[?]** |
| +0x1c | u32 | next/phase value used by `CSChangeDoodadPhase` (`row+0x1c`) |
| +0x20 | u32 | **permission id** (`perm_id`, switch 0..8 in `FUN_393b3300`) |
| +0x24 | u32 | usable flag returned by `FUN_393bc480` |
| +0x24/+0x28 | ptr/ptr | interaction-row vector (rows are 0x2c bytes in `FUN_393bd290`) |
| +0x34 | ptr/ptr | quest-act vector (row `+0x48/+0x4c`, `FUN_393b9a80`) |

The interaction row used by `FUN_393bd290`: `+0x00` skill id, `+0x08` actual func type,
`+0x0c` actual func id, `+0x20` permission id, `+0x24` usable flag. The row base is passed to
`FUN_393b3300(row+0x20)` and `FUN_393b2c40(row+8)`.

## 3. `DynamicActionBar` — `DAT_39affa0c`, 0x30 bytes

Constructor `FUN_393b23d0` (called after `operator new(0x30)` in `FUN_39174520`), reset
`FUN_393b2530`.

| off | type | meaning |
|---|---|---|
| +0x00 | u8 | visible |
| +0x01 | u8 | (unused in the code read) |
| +0x02 | u8 | show pending (set by `FUN_393b2490`/`24e0`, cleared by `FUN_393b2760`) |
| +0x03 | u8 | has-actions (read by `X2Interaction:IsDynamicActionBarVisible` `0x3947e060`) |
| +0x04 | u8 | hidden flag (read by `0x3947e060`) |
| +0x08 | u32 | current unit/npc objId (empty = `DAT_3a17d7cc`) |
| +0x0c | u32 | current doodad objId (empty = `DAT_3a17d7d4`) |
| +0x10 | u32* | raw candidate vector begin |
| +0x14 | u32* | raw candidate vector end |
| +0x18 | u32* | raw candidate vector cap |
| +0x20 | u32* | shown vector begin (what `GetDynamicActionCount/Skill` read) |
| +0x24 | u32* | shown vector end |
| +0x28 | u32* | shown vector cap |

`FUN_393b2490` (unit) and `FUN_393b24e0` (doodad) clear the raw vector and set the id +
show-pending byte; the id-to-slot assignment is read from the call sites (`bar+8` = unit,
`bar+0xc` = doodad).
The raw and shown vectors are plain `std::vector<u32>` arrays; `FUN_393b25a0` compares and copies
them, `FUN_393b2230` rebuilds the UI widgets.

## 4. World `InteractionManager` — `DAT_39aff9bc`, 0x60 bytes

Constructor `FUN_393b5e20` (allocated with `operator new(0x60)` in `FUN_39174520`), update
`FUN_393b5f60`, destructor `FUN_393b5ef0`.

| off | type | meaning |
|---|---|---|
| +0x00 | u32 | packet block field 0 (source/sentinel) |
| +0x04 | u32 | **target objId** (doodad id in the world-list path; set by `FUN_393b3ec0`) |
| +0x08 | u32 | packet block field 2 |
| +0x0c | u32 | packet block field 3 |
| +0x10 | u32[10] | **skill id array** (count at +0x38; cap 0x100 in the bar builder) |
| +0x38 | u32 | skill count (set to 1 on the too-far path) |
| +0x3c | u32 | packet block field 15 |
| +0x40 | u32 | cleared on close |
| +0x44 | u32 | last-use timestamp (ms) for the 500 ms rate limit |
| +0x48 | ptr | current interaction object (vtable; `FUN_393b2af0` calls `+4`) |
| +0x4c | list | interaction-effect list head (0x0c-byte nodes) |
| +0x50 | u32 | interaction-effect count |
| +0x54 | u32 | [?] |
| +0x58 | u32 | watchdog accumulator (ms, >999 → `FUN_393b7e40`) |
| +0x5c | u32 | last handled doodad objId |

The runtime creation in `FUN_39174520` allocates 0x60 bytes; the `.data` dump has all ids 0 (the
"empty" sentinels are zero at runtime).

## 5. NPC `NpcInteractionManager` — `DAT_39aff9b4`, 0x64 bytes

Constructor `FUN_393be770` (allocated with `operator new(100)`). Start `FUN_393be280`, cancel
`FUN_393be180`, list `FUN_393be850`.

| off | type | meaning |
|---|---|---|
| +0x00 | u8 | enabled/valid (ctor 1) |
| +0x04 | u32 | current target objId (empty = `DAT_3a17f52c`) |
| +0x08 | u32 | current skill id |
| +0x0c | u32 | current handler key/name id (`FUN_393c1310` result) |
| +0x10 | u32 | last-use timestamp (ms; `+500` rate limit) |
| +0x14 | u32[16] | packet block copied by `FUN_393be850` (16 dwords) |
| +0x1c | u32[...] | skill id array inside that block (count at +0x44) |
| +0x44 | u32 | skill count |
| +0x50 | u8 | flag (ctor 1) |
| +0x51 | u8 | flag (ctor 0) |
| +0x54 | u32* | quest/talk list vector begin |
| +0x58 | u32* | quest/talk list vector end |
| +0x5c | u32* | quest/talk list vector cap |

The vector at +0x54 is the `param_2` of `FUN_393bf500` (the NPC quest lists built by
`FUN_39424820`). The rest of the 16-dword block (`+0x14..+0x53`) is the server
`SCNpcInteractionSkillList` payload [R from the copy loop].

## 6. `InteractionEffect` — 0x54 bytes

Constructor `FUN_393b7140(source, target, durationMs)`, added by `FUN_393b7800`, ticked by
`FUN_393b5f60`, canceled by `FUN_393b6220`.

| off | type | meaning |
|---|---|---|
| +0x00 | vtable | first base |
| +0x04 | vtable | second base |
| +0x08 | u32/float | start time (engine clock) |
| +0x0c | float | duration in seconds (`durationMs * 0.001`, constant `0x3999cf30`) |
| +0x10 | u32 | source unit id |
| +0x14 | ptr | target doodad/unit pointer |
| +0x18 | u8 | active flag (1) |
| +0x1c | u32 | 5 (loop/kind) |
| +0x20 | u32 | 5 |
| +0x24 | float | 0.4 (constant `0x399a24b8`) |
| +0x28 | float | 0.4 |
| +0x2c..+0x48 | — | zeroed |
| +0x50 | u8 | zeroed |

Virtual slots observed: `+4` = finished?, `+8` = update, `+0xc` = stop, `+0x10` = source id,
`+0x14` = target id, `+0x1c` = interaction id. `FUN_393b5f60` calls `+8` then `+4` and frees the
node when finished; `FUN_393b6220` compares the three getters.

## 7. Interaction service registry — `0x3a17f58c`

A `std::map<u32, handler*>` object at `0x3a17f58c`, sentinel node at `0x3a17f590`. The node layout
is the MSVC red-black node: `+0x00 left, +0x04 right, +0x08 parent, +0x0c key, +0x10 handler*`
([R] `FUN_393c1450`). `FUN_393c14c0` inserts 17 entries (skill id → handler object); `FUN_393c1280`
looks up, reads `[handler]` (vtable) and calls `vtable[0](skill, npc)`; `FUN_393c1310` reads
`vtable[1]` (the handler name, special "recruiter" for 12660).

The handler objects are **inline in `.data`** at `0x39ac7a30` (and +0x48 steps), each:

| off | type | meaning |
|---|---|---|
| +0x00 | ptr | vtable (`0x399d33c4`, `0x399d33dc`, …) |
| +0x04 | char[] | inline **tag string** (`skill_trainer`, `ability_change`, `auction`, `priest_recover_exp`, `teleporter`, `blacksmith`, `store`, `trader`, `specialty_trader`, `specialty_store`, `bank`, `stabler`, `expedition`, `repairman`, `talker`, `siege-merchant`, `look_converter`) |

`FUN_393c10d0` (the generic service handler) copies `handler+4` into the `NPC_INTERACTION_START`
0x6B event payload.

`FUN_393c14c0` writes are at the game-client init; the map is static for the process lifetime.

## 8. `LootManager` — `DAT_39aff9b8`, 0x30 bytes

Constructor `FUN_394fa610` (allocated with `operator new(0x30)`).

| off | type | meaning |
|---|---|---|
| +0x00 | u16 | current owner type (server `LootOwnerType`, 0 = none) |
| +0x02 | u16 | owner type copied from the current state |
| +0x04 | u32 | current owner objId |
| +0x08 | u8 | bag open |
| +0x0c | row* | row vector begin |
| +0x10 | row* | row vector end |
| +0x14 | row* | row vector cap |
| +0x18 | — | (gap; not used in the code read) |
| +0x20 | tree | lootable-object set (keyed by type+id) |
| +0x24 | u32 | set size |

### 8.1 Loot row — 0xD0 bytes

| off | type | meaning |
|---|---|---|
| +0x08 | u32 | item template id (`itemId`, `FUN_394f9790`) |
| +0x0c | u32 | item index / packed index+owner type (`FUN_394fa6c0`) |
| +0x10 | u32 | item type / "empty" sentinel `DAT_3a5ba4ac` |
| +0x18 | i32 | remaining count (decremented by `SCLootItemTook`; row removed at 0) |
| +0xc8 | u8 | in-flight flag (set by `FUN_394f9790`, cleared by `SCLootItemFailed`) |

The full item record behind `+0x08` is the inventory `Item` structure (0xB8/0xC0 bytes); this map
covers only the loot-side fields. `SCLootBagData` rows are 0xC0 bytes and are unpacked by
`FUN_3901c0a0` / `FUN_394fa880`.

## 9. Pick block (cursor ray result)

`FUN_39428d80` fills a block (base `param_5`) and the globals `DAT_3a5a4dbc/dc0/dc4/dd0`:

| off | type | meaning |
|---|---|---|
| +0x00 | u32 | unit objId (`DAT_3a5a4dbc`) |
| +0x04 | u32 | doodad objId (`DAT_3a5a4dc0`) |
| +0x08 | u32 | gimmick objId (`DAT_3a5a4dc4`) |
| +0x0c | u32 | [?] |
| +0x10 | u32 | [?] |
| +0x14 | i32 | picking id (`DAT_3a5a4dd0`) |
| +0x18 | float | world x |
| +0x1c | float | world y |
| +0x20 | float | world z |

The log string at `0x399d538` prints exactly these names (`unitId`, `doodadId`, `gimmickId`,
`pickingId`, `Pos`). `DAT_3a5a4dcc` is the "target was clicked/selected" flag used by the click
path; `DAT_3a5a4da8 == 6` selects `ControlMouseClick`.

## 10. Globals, cvars and data tables

### 10.1 Ranges, distances, timers

| global | value | used by |
|---|---|---|
| `0x39ac79fc` | f32 9.0 (3.0 m squared) | `FUN_393bf270` NPC/unit interaction |
| `0x39ac7a00` | f32 12.25 (3.5 m squared) | `FUN_393bf270` mounted variant |
| `0x399a7660` | f32 2.1 | cvar `max_interaction_doodad_distance`, `DAT_39aefb98+0x2a0` |
| `0x3999f85c` | f32 25.0 | cvar `dynamic_action_bar_distance`, `DAT_39aefb98+0x29c` (no reader found) |
| `0x399a1ec0` | f32 2000.0 | `FUN_39428d80` pick max distance |
| `0x3999d7e4` | f32 0.8 | `FUN_393601a0` mate extra range |
| `0x39ac7428` | f32 2.0 | `FUN_393601a0` mate/pick extra |
| `0x3999cf30` | f32 0.001 | ms→s for `FUN_393b7140` |
| `0x399a24b8` | f32 0.4 | interaction-effect default |
| `0x3999c488` | f32 1.0 | generic 1.0 |
| `0x3999c478/47c/484` | 1.5 / 0.5 / 0.0 | rsqrt normalisation in the pick/indicator code |
| `0x3999ddcc` | f32 1e6 | pick sanity bound |
| `0x39ac79f8` | u32 600 | ship/vehicle interaction time |
| `0x3999d538` | f32 100.0 | debug draw time |

### 10.2 Sentinels and "empty" ids

| global | value in the dump | meaning |
|---|---|---|
| `DAT_3a17e284/e288/e28c` | 0 | empty object ids |
| `DAT_3a17e2b4/e2b8/e2bc` | 0 | empty source/target ids |
| `DAT_3a17d7cc/d7d0/d7d4/d7d8` | 0 | empty bar ids / skill 10994 sentinel |
| `DAT_3a17d7d0` | 0 (= empty) | "no skill" in `FUN_393b1da0` |
| `DAT_3a17e680/684/69c/6a0/6a4/6a8/6ac` | 0 | interaction-state sentinels |
| `DAT_3a17e6f4` | 0 | current loot/interaction source |
| `DAT_3a17f52c/530/534/538/544/560/564/568/570/57c/580/584/586/588/590` | 0 | NPC/interaction empty ids |
| `DAT_39aefb9c/fbf0/fc08` | 0 | empty picked object ids |
| `DAT_3a5a4dbc/dc0/dc4/dcc/dd0` | 0 | pick result (runtime) |

### 10.3 Interaction skill ids (`.data` `0x39ac7800–0x39ac7a2c`)

The client hard-codes the interaction/default skills it needs. The full list read as i32:

| addr | id | DB name (KO) | meaning |
|---|---|---|---|
| `0x39ac7800` | 12081 | 스킬 배우기 | learn skill |
| `0x39ac7804` | 12082 | 능력 바꾸기 | ability change |
| `0x39ac7808` | 12085 | 순간 이동하기 | teleport |
| `0x39ac780c` | 11000 | — | (unknown) |
| `0x39ac7810` | 11823 | — | (unknown) |
| `0x39ac7814` | 11117 | — | (unknown) |
| `0x39ac7818` | 11001 | — | (unknown) |
| `0x39ac781c` | 12077 | 올라타기 | mount |
| `0x39ac7820` | 10994 | 취소 | cancel (bar sentinel) |
| `0x39ac783c` | 10994 | 취소 | cancel (rejection sentinel) |
| `0x39ac7840` | 10995 | 에러 | error |
| `0x39ac7844` | 11400 | 우편함 열기 | open mailbox |
| `0x39ac7848` | 14047 | 모두 줍기 | loot all |
| `0x39ac785c` | 6563 | — | doodad model/type key |
| `0x39ac78a4` | 12079 | 줍기 | loot |
| `0x39ac78a8` | 12081 | 스킬 배우기 | learn skill |
| `0x39ac78ac` | 12082 | 능력 바꾸기 | ability change |
| `0x39ac78b0` | 12083 | 경매장 이용하기 | auction |
| `0x39ac78b4` | 12085 | 순간 이동하기 | teleport |
| `0x39ac78b8` | 12086 | 수리하기 | repair |
| `0x39ac78bc` | 12087 | 상점 이용하기 | store |
| `0x39ac78c0` | 12195 | 소환수 부상치료 | pet heal |
| `0x39ac78c4` | 12660 | 전장 지원하기 | battlefield |
| `0x39ac78c8` | 13238 | 창고 이용하기 | warehouse |
| `0x39ac78d0` | 13356 | 대화하기 | talk |
| `0x39ac78d4` | 13719 | 일으켜세우기 | resurrect |
| `0x39ac78d8` | 13736 | 원정대 결성 | expedition |
| `0x39ac78dc` | 13878 | — | (unknown) |
| `0x39ac78e0` | 13863 | 보조석 점유 | passenger seat |
| `0x39ac78e4` | 13877 | 대화하기 | quest |
| `0x39ac78e8` | 14047 | 모두 줍기 | loot all |
| `0x39ac78ec` | 14866 | 등짐 팔기 | sell backpack |
| `0x39ac78f0` | 15297 | 소환수 부상치료 | pet heal 2 |
| `0x39ac78f4` | 16376 | 특산품 판매 | sell specialty |
| `0x39ac78f8` | 16912 | 영지 정보 보기 | territory info |
| `0x39ac78fc` | 17135 | 특산품 구매 | buy specialty |
| `0x39ac7900` | 17063 | 기도 드리기 | pray |
| `0x39ac7904` | 19165 | 파괴 | destroy |
| `0x39ac7908` | 20558 | — | (unknown) |
| `0x39ac790c` | 23478 | 장비 외형 변경 의뢰하기 | item look change |
| `0x39ac7930` | 826 | — | (table count/limit?) |
| `0x39ac7938` | 10994 | 취소 | cancel |
| `0x39ac793c` | 12079 | 줍기 | loot |
| `0x39ac7940` | 14047 | 모두 줍기 | loot all |
| `0x39ac7944` | 14770 | — | (unknown) |
| `0x39ac794c` | 12945 | 철거 | demolish |
| `0x39ac7950` | 12083 | 경매장 이용하기 | auction |
| `0x39ac7954` | 13238 | 창고 이용하기 | warehouse |
| `0x39ac7958` | 13356 | 대화하기 | talk |
| `0x39ac795c` | 13877 | 대화하기 | quest |
| `0x39ac7970` | 11001 | — | (special target kind) |
| `0x39ac7974` | 12076 | 점유 | occupy |
| `0x39ac7978` | 12077 | 올라타기 | mount |
| `0x39ac797c` | 12078 | 탑승 | board |
| `0x39ac7980` | 12079 | 줍기 | loot |
| `0x39ac7984` | 12081 | 스킬 배우기 | learn skill |
| `0x39ac7988` | 12082 | 능력 바꾸기 | ability change |
| `0x39ac798c` | 12083 | 경매장 이용하기 | auction |
| `0x39ac7990` | 12085 | 순간 이동하기 | teleport |
| `0x39ac7994` | 12086 | 수리하기 | repair |
| `0x39ac7998` | 12087 | 상점 이용하기 | store |
| `0x39ac799c` | 12089 | 퀘스트 시작 | quest start |
| `0x39ac79a0` | 12090 | 퀘스트 대화 | quest talk |
| `0x39ac79a4` | 12092 | 퀘스트 완료 | quest complete |
| `0x39ac79a8` | 12106 | 정보 보기 | info |
| `0x39ac79ac` | 12195 | 소환수 부상치료 | pet heal |
| `0x39ac79b0` | 12660 | 전장 지원하기 | battlefield |
| `0x39ac79b4` | 13238 | 창고 이용하기 | warehouse |
| `0x39ac79b8` | 13356 | 대화하기 | talk |
| `0x39ac79bc` | 13719 | 일으켜세우기 | resurrect |
| `0x39ac79c0` | 13736 | 원정대 결성 | expedition |
| `0x39ac79c4` | 13863 | 보조석 점유 | passenger seat |
| `0x39ac79c8` | 13877 | 대화하기 | quest |
| `0x39ac79cc` | 14866 | 등짐 팔기 | sell backpack |
| `0x39ac79d0` | 15297 | 소환수 부상치료 | pet heal 2 |
| `0x39ac79d4` | 16376 | 특산품 판매 | sell specialty |
| `0x39ac79d8` | 16912 | 영지 정보 보기 | territory info |
| `0x39ac79dc` | 17135 | 특산품 구매 | buy specialty |
| `0x39ac79e0` | 17063 | 기도 드리기 | pray |
| `0x39ac79e4` | 18458 | — | (special) |
| `0x39ac79e8` | 19165 | 파괴 | destroy |
| `0x39ac79ec` | 23478 | 장비 외형 변경 의뢰하기 | item look change |
| `0x39ac79f0` | 23469 | 성별 전환하기 | gender swap |
| `0x39ac79f4` | 22780 | 미용실 들어가기 | beauty salon |

DB names are from `compact.sqlite3 skills` (KO); the English meaning column is **[I]** from the
call site. Ids not in the `skills` table (6563, 14770, 18458, 20558, 11000/1, 11823, 11117, 826)
are client-side sentinels or non-skill interaction ids.

### 10.4 UI event ids used here

From `FUN_39214380` (the id is `(ECX - 0x39d22ce8)/4` at each `FUN_39002bf0` call; the
registration order is not the id order, so the ids below were read from the ECX immediates):

| id | name | fired by |
|---|---|---|
| 0x6B | `NPC_INTERACTION_START` | service handlers, quest routing, insert |
| 0x6C | `NPC_INTERACTION_END` | `FUN_393b7aa0` |
| 0x6D/0x6E | `HOUSE_INTERACTION_START/END` | `FUN_393c0320`/housing |
| 0xD1/0xD2 | `INTERACTION_START/END` | `FUN_393b7aa0`, coffer |
| 0x7C | `ITEM_ACQUISITION_BY_LOOT` | loot text |
| 0x7D | `MONEY_ACQUISITION_BY_LOOT` | loot text |
| 0x7E | `LOOT_BAG_CHANGED` | loot bag data/took/failed/close |
| 0x7F | `LOOT_BAG_CLOSE` | `FUN_394fab30` |
| 0x8A | `RENEW_ITEM_WND_SHOW` | `FUN_393b8610` |
| 0xFD | `DOODAD_PHASE_MSG` | `FUN_3902e0f0` |
| 0x102 | `OPEN_COMMON_FARM_INFO` | `FUN_393b8700` |
| 0x19D | `DYNAMIC_ACTION_BAR_SHOW` | `FUN_393b1eb0` |
| 0x19E | `DYNAMIC_ACTION_BAR_HIDE` | `FUN_393b1eb0`, `FUN_393b25a0` |
| 0x19F | `DYNAMIC_ACTION_EXECUTE` | bar execute path |
| 0x1A4 | `OPEN_EMBLEM_UPLOAD_UI` | `FUN_393b82a0` |
| 0x1A7/0x1A8 | `DRAW_DOODAD_SIGN_TAG`/`DRAW_DOODAD_TOOLTIP` | `FUN_393b51a0`, `FUN_393b2c10` |
| 0x201 | `OPEN_PAPER` | `FUN_393b8450`/`FUN_393b80d0` |
| 0x202 | `REPORT_CRIME` | `FUN_393bb090` **[I]** |
| 0x243 | `COFFER_INTERACTION_START` | coffer func |
| 0xEA | `START_QUEST_CONTEXT_NPC` | quest Lua |
| 0xF0 | `COMPLETE_QUEST_CONTEXT_NPC` | quest Lua |

### 10.5 Cvars (registered by `FUN_390f7b40`)

| cvar | config offset | default | source |
|---|---|---|---|
| `dynamic_action_bar_distance` | `DAT_39aefb98+0x29c` | 25.0 | `0x3999f85c` |
| `max_interaction_doodad_distance` | `DAT_39aefb98+0x2a0` | 2.1 | `0x399a7660` |
| `log_doodad_interaction` | `DAT_39aefb98+0x2a4` | 0 | — |

`re-constants.md` lists the other cvars used by the neighbouring systems; the 2.1 m gate is the
only one read by this subsystem's code.
