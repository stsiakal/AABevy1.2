# Quests, NPC interaction and NPC shops (1.2 client)

Tags: `[R]` read in code/data, `[I]` inferred, `[?]` not found.

## 1. Scope and sources

This spec covers the client side of: NPC talk (interaction list and windows), quest offer /
accept / decline / talk / hand-in / drop, the directing-mode dialogue, the journal and tracker,
objective counters and time limits, world quest marks, map/quest-area indicators, reward choice,
and merchant buy / sell / buy-back with currency and prices. Server code is used only for packet
layouts that the 1.2 C# server mirrors; behaviour is the client's.

Sources used: `x2game.dll.c` (addresses are VAs; image base `0x38FF0000`), `strings.txt` /
`strings-xrefs.tsv`, Lua UI bytecode disassembled with `aa-lua dis` (the bracketed numbers are
the original Lua source lines), `game_pak` data (`quest_sign_sphere.g`), and
`compact.sqlite3`. The client's own SQL strings confirm it loads the same quest tables
(`x2game strings 0x39a195b8` quest_components, `0x39a1dd18` quest_acts, `0x39a19db0`
quest_contexts, `0x39a0c590` unit_reqs, `0x39a1e344` quest_supplies,
`0x39a1a738` quest_act_obj_talks, `0x39a193c8`/`0x39a1a610` objective acts,
`0x39a19b70` quest_chat_bubbles).

## 2. The rules

### 2.1 Quest data and steps

The client loads `quest_contexts` (id, category, level, detail_id, repeatable, selective,
let_it_done, grade_id, chapter_idx, zone_id, use_quest_camera, score), `quest_components`
(kind, next_component, npc_id, or_unit_reqs, hide_quest_marker), `quest_acts` and one detail
table per act type (`QuestAct*` strings, `x2game strings 0x39a1d6c0`), component texts, chat
bubbles, objective aliases, monster groups, quest mails, quest cameras and cinemas. `[R]`

Component kinds in the shipped data are 1..8 with counts
`1:5, 2:4697, 3:826, 4:4244, 5:52, 6:3553, 7:9, 8:4465`
(`compact.sqlite3 quest_components.component_kind_id`). Kind 2 = Start, 4 = Progress,
6 = Ready (hand-in), 8 = Reward. `[R]` (also the names in
`aaemu-game/src/systems/quests/model.rs:11-17`).

An active quest carries exactly **five** objective counters (`ActiveQuest.objectives [i32;5]`,
`aaemu-game/src/models/char_data.rs:391`); the step's component id and accept time travel with
them. Statuses are 1 progress, 2 failed, 3 ready-to-report, 4 dropped, 5 completed, 6 daily
completed (`aa-net/src/quest.rs:15-30`). `[R]`

Objectives are numbered across the components of the current kind, in component order
(server `quests.rs` `objective` assignment); the client's Lua reads them one-based through
`X2Quest:GetQuestJournalObjectiveCount(i)` / `GetQuestJournalObjectiveText(i,k)`
(`questcontext/quest_context_common.alb:25,44`). Only the five packet counters carry
progress, so a step showing more than five acts has no wire slot for the extras. `[R]`

### 2.2 NPC interaction

Talking is a four-packet handshake:

* `CSInteractNPCPacket` 0x065 — `obj_id: bc`, `is_target_changed: bool`; sets the server's
  current interaction object (and target when the flag is set) and answers `SCAiAggro`
  (`aaemu-game/src/systems/npcs/npc/packets.rs:317-363`). `[R]`
* `CSStartInteractionPacket` 0x068 — `npc_obj_id: bc`, `obj_id: bc`, `extra_info: i32`,
  `pick_id: i32`, `mouse_button: u8`, `modifier_keys: i32`; answered by
  `SCNpcInteractionSkillListPacket` 0x07b — `npcObjId: bc`, `objId: bc`, `extraInfo: i32`,
  `pickId: i32`, `mouseButton: u8`, `skillCount: i32`, `skillId: u32 * n`,
  `interactable: u8`, `modifierKeys: i32` (`.../npc/packets.rs:44-77`). `[R]`
* `CSInteractNPCEndPacket` 0x066 — `obj_id: bc`; clears the interaction object. `[R]`
* `X2Interaction:CancelNPCInteraction()` is what the UI calls when the talk ends
  (`npc_interaction.alb:308`). `[R]`

The server resolves the primary function by a fixed priority — banker, ability changer,
auctioneer, priest, repairman, merchant, stabler, expedition/recruiter, blacksmith (plus
`REPAIR` for a blacksmith) — with the
skill ids: warehouse 13238, skillsets 12082, auctioneer 12083, blessing 12084, repair 12086,
store 12087, stabler 12195, guild 13736, war support 12660, item fusion 23478
(`.../npc/packets.rs:20-31, 186-215`). The client turns each id into an interaction kind and a
window (`npc_interaction.alb:3-18`):

| id | Lua kind | window |
|---|---|---|
|1|skill_trainer|skillTrainer|
|2|ability_change|abilityChangeFrame|
|3|store|storeFrame|
|4|auction|auctionWindow|
|5|priest|divine-blessing message box|
|6|teleporter|teleporter|
|7|blacksmith|repairWindow|
|8|stabler|stabler message box|
|9|recruiter|battlefield entrance|
|10|bank|bank inventory (`GetBankInventory`)|
|11|trader|backpackStore|
|12|priest_recover_exp|recover-exp box|
|13|specialty|specialty store box|
|14|dominion_npc|territoryFrame|
|15|expedition|expedition-creation box|
|16|look_converter|changeItemLookWindow|

Quest actions are not in that table. `NPC_INTERACTION_START("quest", what, npcId)` selects
`talk` if any talk objective, else `complete` if any ready hand-in, else `start` if any offer
(`npc_interaction.alb:248-262`). `CanCompleteNpcQuest` is exactly
`GetNpcQuestContextCountComplete() > 0` (`:163-171`). Starting calls `X2Quest:CallQuestUi(0,
qtype, npcId)`; completing calls `CallQuestUi(2, qtype, npcId)`; talking loops **all** talk
objectives of the NPC and calls `CallQuestTalk(qtype, ctype, 0)` for each
(`npc_interaction.alb:183-234`). `[R]`

The three native entry points encode the request:

* `CallQuestUi`: type 0 → internal event `0xEA` with the quest type; type 1/2 → `0xF0`
  (`x2game FUN_3949eb40`). `[R]`
* `CallQuestTalk`: resolves the NPC string id and sends internal event `0xED`
  (`FUN_3949ec10`, `FUN_3949e990`). `[R]`
* `TryStartQuestContext(who,qtype,doodadId,npcId)` and
  `TryCompleteQuestContext(who,qtype,doodadId,npcId,selected)` are the four/five-argument
  forms the UI calls (`questcontext/quest_context.alb:31-33, 5-29`). `[R]`

### 2.3 Offer, accept, decline, directing mode

The offer set is computed from Start (2) components with a `QuestActConAcceptNpc` for the NPC;
the hand-in set from Ready (6) components with `QuestActConReportNpc`; the talk set from
Progress (4) `QuestActObjTalk` acts that are not finished (the same rule Bevy also
implements, `aa-client/src/hud/quests.rs:567-614`). Component `unit_reqs` gate the offer
(`or_unit_reqs` = any instead of all); the client validates them with its generic unit-req code
(`x2game FUN_39897990`, error keys `UNIT_REQS_OR_FAIL` / `TOO_FAR_RANGE`, strings
`0x39a1f350`, `0x39a1f5b0`). `[R]`

Acceptance prechecks run before the packet: `FUN_3937a2d0` resolves the quest and calls
`FUN_397d5aa0` (state/requirements); failure raises a quest error code, success with an NPC
interaction raises UI event `0xF4` and then sends `CSStartQuestContextPacket` 0x0d5 from
`FUN_393762c0`. Packet fields: `quest_context_id: u32`, `npc_obj_id: bc`,
`doodad_obj_id: bc`, `sphere_id: u32` (`aaemu-game/src/systems/quests/packets.rs:197-235`).
The same packet is used for item/sphere starts with the corresponding id. `[R]`

Decline is **client-only**: `DeclineDirectingQuest` calls `FUN_3937caa0(0,-1)` →
`FUN_3937c9d0` case 0, which closes the dialogue and sends no packet (`x2game
FUN_3949b210`, `FUN_3937c9d0`). Accept/talk/complete are cases 1/2/3 of the same function;
case 3 carries the selected reward index (`FUN_3949b250`). `[R]`

The dialogue window is `questcontext/quest_context_directing.alb`: widgets `quest_directing_mode`,
`questdirecting`, `QuestDirectingFadeWindow`, `messageWidget`, `nextButton`, `prevButton`,
`confirmButton`, `journal`, `cinemaView`, buttons Next (F), Prev (D), Confirm
(`:196-268, 459-523`). It plays `CHAT_MSG_QUEST` per bubble, walks `next_bubble`, and asks
`TryCompleteQuestContext` with `selected=0` on the first (non-reward) pass
(`:644`). Hand-in with selective rewards reopens the reward panel and
`CompleteDirectingQuest` sends the journal's `GetSelectiveItemIndex()` (`:515`). `[R]`

Turn-in prechecks (client, `FUN_3937b860`): if the quest has selective-reward acts and
`selected == 0`, error 10 and no packet; if the quest's state is not 3, a target/state check
fails with error 2; other failures use error 0x0C. Errors are indexed into
`locale.questContext.errors[code]` and printed to the system-message window
(`quest_context.alb:258-260`). `[R]` The actual texts were not in the extracted locale
(`[?]`, see §6).

The accept/talk/complete packets are:

| Packet | Opcode | Fields |
|---|---|---|
|CSStartQuestContext|0x0d5|questId u32, npcObj bc, doodadObj bc, sphereId u32|
|CSCompleteQuestContext|0x0d6|questId u32, npcObj bc, doodadObj bc, selected i32|
|CSDropQuestContext|0x0d7|questId u32|
|CSQuestTalkMade|0x0da|npcObj bc, questId u32, componentId u32, actId u32|
|CSTryQuestCompleteAsLetItDone|0x0dd|id u32, objId bc, selected i32|
|CSRestartMainQuest|0x0fd|id u32|

(`aaemu-game/src/systems/quests/packets.rs:197-470`; client senders `FUN_393762c0` 0xd5,
`FUN_39376310` 0xda, `FUN_39376360` 0xd6, `FUN_3937a4b0` 0xd7; the 0xdd/0xfd packet
constructors are at `0x328407` and `0x329859`). `[R]`

### 2.4 Journal, tracker, counters and timers

The quest log (`questcontext/quest_context_list.alb`) is a two-column table (40 px level column,
179 px title, row 20 px, row space 10 px, `:7-21`). It groups quests into category folders;
hidden quests are hidden unless they are task quests (`quest_context_common.alb:123-127`).
Status 3 ("ready") is the report state; a quest with no objectives and status ≠ 3 gets a
synthetic "talk to <report NPC>" objective (`quest_context_common.alb:64-80`). The list refresh
plays `event_quest_list_changed` (`quest_context.alb:44`). `[R]`

The tracker (`quest_context_notifier.alb`) adds every active quest; `SetFlickeringTime(6000)`
is installed at load (`quest_context.alb:58-62`) and is the 6 s flash when the list changes.
Each tracked quest is assigned a decal index 1..9 out of `X2Decal:GetMaxDecalCount()-1`
slots (`notifier:109`; the native returns **10**, `x2game FUN_39423e80: return 10`). `[R]`

Time limits: the notifier's clock widget calls `X2Quest:GetQuestContextLeftTime(qtype)`; a
value > 0 shows the clock and calls `Update(msec)`, ≤ 0 hides it (`notifier:1639-1653`). The
native tick `X2::GameClient::UpdateQuestLeftTime` (`FUN_3937cd80`) runs at most once per
**1.0 s** (constant `0x3999c488 = 1.0`), computes
`remaining = end - (now - start) * 1000` (constant `0x3999d1b0 = 1000.0`), and
* if `remaining < 1000` ms, calls the timeout path instead of publishing;
* else sends internal event `0xF7` with `(qtype, remaining_ms)` for the tracker.

`[R]` Server-side the timer is a quest act (`QuestActCheckTimer`), and the deadline is written
into `Quest.Write`'s `LeftTime` field (`char_data.rs:459-478`). `[R]`

### 2.5 World marks and map indicators

Clients compute a per-unit "quest mark style" (`stype`) and select a prefab from a 15-entry
table (`x2game FUN_393821f0`; the strings are at `0x399d13c8..0x399d1710`):

| stype | prefab (`prefab://prefabs/quest_mark.xml/...`) |
|---|---|
|1|marker.exclamation_mark_green|
|2|marker.exclamation_mark_yellow|
|3|marker.exclamation_mark_repeat|
|5|marker.question_mark_ing|
|6|marker.question_mark_green|
|7|marker.question_mark_yellow|
|8|marker.question_mark_more|
|9|marker.question_mark_half|
|0xB|marker.exclamation_mark_red_70|
|0xC|marker.exclamation_mark_blue_70|
|0xD|marker.huntsign|
|0xE|marker.flower_mark_01|
|0xF|marker.flower_mark_02|
|0,4,10,other|marker.question_mark_blue|

`[R]` The state machine `FUN_393838f0` (`ClientUnit::UpdateQuestMark`, called through
`FUN_39359c60`) derives those codes from the NPC's start/ready/talk lists and quest type:
state 9 when an objective is partly done, 8 when the NPC has additional quests, 6/7 for
ready hand-ins, 5 while in progress, 3 for repeatable offers, 1/2 for available offers, and
0xD/0xE/0xF for the hunt/gather special cases (types 8/11 and 10).
`[R]` for the codes and conditions, `[I]` for the human-readable meaning of each code.
Doodads get their own mark update (`ClientDoodad::UpdateQuestMark`, `FUN_393a2c10`).
`UpdateQuestContextMarks` (`FUN_39383e20`) refreshes unit and doodad marks when the local
player or quest tables change. `[R]`

Quest objective areas come from each zone's
`game/worlds/<world>/level_design/zone/<zone>/client/quest_sign_sphere.g`:

```
area
    qtype 179
    ctype 2381
    pos ( x 543.493, y 1064.27, z 188.048 )
    radius 30
```

(`zone/133/client/quest_sign_sphere.g:1-5`; radii observed 5, 20, 30, 50, 60, 70, 80;
zone 173 has a single radius-20 area). `[R]` `qtype` is the quest, `ctype` the component.
The tracker asks the map for a decal slot: `X2Map:AddNotifyQuestInfo(decalIndex, qType, show)`
(`notifier:23`); update/remove use `UpdateNotifyQuestInfo` / `RemoveNotifyQuestInfo(qType)` /
`RemoveAllNotifyQuestInfo` (`notifier:27,38`). Native signatures are confirmed by
`FUN_39069160` (`AddNotifyQuestInfo decalIndex, qType, show`; `SetNotifyCoords count, coords`;
`SetNotifyAreaCoords count, coords`; `SetNotifyAreaColors count, colors`). `[R]` The map draws
the tracked quest's circles in that decal's colour with a grey 1..9 number; decal 0 is the
spare entry (`markers.lua` ports of `map/common.alb:1873-2014`). `[R]/[I]`

`SCShowQuestArea` 0x1af is handled by `OnShowQuestArea` (`FUN_391df7b0`, reading a flag at
`+0xc`, a position at `+0x10` and a radius at `+0x28`) and calls `FUN_393840b0(1, …)` to draw
a quest area. `[R]` Its precise field order is not in the server (`[?]`).

### 2.6 Results and rewards

A completed step sends `SCQuestContextCompletedPacket` 0x14b:
`quest_id u32`, `body[8]`, `component_id u32`; a reset uses 0x14c with the same layout, and a
failure 0x146 (`aaemu-game opcodes.rs:856-863`). `SCQuestContextStarted` 0x147 carries the
full `Quest.Write` structure then `component_id`; `SCQuestContextUpdated` 0x149 adds four
`i32` parameters (`.../quests/packets.rs:29-83`). The client starts/updates/resets the quest
from those and raises `QUEST_CONTEXT_UPDATED(qtype, "started"|"updated"|"completed"|
"dropped")` (`quest_context.alb:67-80`). `[R]`

The reward panel is built from `GetQuestContextRewardExp`, `...Copper`,
`...RewardItem*`, `...RewardSelectiveItem*`, `RewardLaborPower`, `RewardHonorPoint`,
`RewardCrimePoint`, `RewardLivingPoint`, `GetQuestContextRewardAAPoint`,
`GetQuestContextRewardAppellation` (`questcontext_common.alb:745-793`). Up to **three**
selective item buttons are shown (loops `1..3` at `:1077` and `:1086`). If the quest has no
selective reward, `quest_context.alb:10-12` completes directly with `selected=0`; otherwise
the journal's `GetSelectiveItemIndex()` supplies the index to `CSCompleteQuestContext`.
Rewards otherwise come from Reward (8) component supply acts plus `quest_supplies` for the
quest level (`compact.sqlite3 quest_supplies`: level 1 = 420 exp / 33 copper; level 2 = 490 /
42; …; levels 1..55). `[R]`

### 2.7 NPC shops

The catalog is **client data**, not a packet: the 1.2 client queries
`SELECT merchants.npc_id, merchant_goods.item_id, merchant_goods.grade_id,
merchant_packs.kind_id FROM merchants, merchant_goods, merchant_packs ... ORDER BY
merchants.npc_id, merchant_goods.item_id, merchant_goods.grade_id`
(`x2game strings 0x399f9218`). `[R]` The window grid is fixed by Lua: 2 columns,
6 buy rows (12 goods per page), sell rows `GetSellItemsMaxCount()/2`, 7 buy-back rows
(2 columns), 1 cart row with `GetSellItemsMaxCount()-6` columns; default currency is
`CURRENCY_GOLD` (`store/common.alb:1-9`). `X2Store:GetSellItemsMaxCount()` returns **16**
(`FUN_394ae840`, constant `0x3999f964 = 16.0`), so the cart holds 10 and the sell grid 8×2.
`[R]`

Buy sends `CSBuyItemsPacket` 0x0ae: `npcObjId bc`, `doodadObjId bc`, `unk u32` (0),
`buyCount u8`, `buyBackCount u8`, each buy `{itemId u32, grade u8, count i32, currency u8}`,
then each buy-back index `i32`, then `useAAPoint bool` (`aaemu-game/.../store.rs:287-329`).
Currency display chooses `money/cost`, `honorPrice`, `livingPointPrice` or AA points
(`store/common.alb:89-97`); shift-clicking a good adds `maxStack`, a plain click one
(`store/buy.alb:175-178`). `[R]`

Sell sends `CSSellItemsPacket` 0x0b0: `npc bc`, `doodad bc(0)`, `count u8`, entries
`{container u8, slot u8, itemUid u64, unk u32(0)}` (`.../store.rs:604-620`). `[R]` The refund
is `int(template.Refund * grade.RefundMultiplier / 100) * count` (single precision, truncated
per unit) (`.../store.rs:245-258`). `[R]`

Buy-back: `CSListSoldItemPacket` 0x0b1 (`npc bc`) → `SCSoldItemListPacket` 0x12c
(`count i32`, items), but the server truncates the list to **12** items
(`.../store.rs:805-828`). The window has 14 cells (2×7) and fills them newest-first:
slot `i` reads `SOLDITEMLIST[len-(i-1)]` (`store/rebuy.alb:9,30-32`). Clicking a filled
cell sends `BuyBackItem(index)` (`:59`). `[R]`

NPC service reach is **3 m** for shops ("3m should be enough for NPC shops", C# comment
preserved at `.../store.rs:201-203`); bank/auction requests use 5 m
(`.../store.rs:205-209`). `CSStartInteractionPacket` itself has only a
`TODO (C#): Distance-check` (`.../npc/packets.rs:296`), so the client is expected to gate it.
The generic interaction gate is the squared `DAT_39ac79fc = 9.0`, i.e. **3 m** (`FUN_39093860`;
the quest-talk call site itself was not isolated, but it uses that gate). `[R range / I call site]`

## 3. Numeric parameters

| Item | Value | Source |
|---|---|---|
|Objective counters per quest|5|`char_data.rs:391`|
|Quest statuses|1,2,3,4,5,6|`aa-net/src/quest.rs:15-30`|
|Start/Progress/Ready/Reward component kinds|2/4/6/8|`quest_components.component_kind_id`|
|Quest level supply table|levels 1..55|`quest_supplies`|
|Left-time tick|1.0 s|`FUN_3937cd80`, `0x3999c488`|
|Left-time conversion|1000 ms/s|`0x3999d1b0`|
|Timeout threshold|< 1000 ms|`FUN_3937cd80`|
|Tracker flicker|6000 ms|`quest_context.alb:60`|
|Decal slots|10 (0 spare, 1..9 used)|`FUN_39423e80`; `notifier:109`|
|Quest sign radius|5–80 (samples)|`quest_sign_sphere.g`|
|Sell-list max count|16|`FUN_394ae840`, `0x3999f964`|
|Buy page|2×6 = 12|`store/common.alb:3-4`|
|Cart|1×10|`store/common.alb:7-8`|
|Sell grid|2×8|`store/common.alb:5`|
|Buy-back grid|2×7 = 14 cells, 12 items|`store/rebuy.alb:9`; server truncates 12|
|NPC talk/interaction range|3 m|`DAT_39ac79fc = 9.0`; `FUN_39093860`|
|Shop range|3 m|`store.rs:201-203`|
|Bank/auction range|5 m|`store.rs:205-209`|
|Selected reward slots|3|`questcontext_common.alb:1077,1086`|

## 4. What is inferred

* `[I]` The human meaning of stype codes 1/2/3/5..9,0xD..0xF: the selector and prefab table are
  read, but the individual predicate names (`FUN_397d3500`, `FUN_397d5700`, `FUN_397d6140`,
  `FUN_397d71b0`, `FUN_397d6320`, `FUN_397d5200`) are unlabelled. The table in §2.5 is exact;
  the mapping "green/yellow/repeat/half/more" to quest states is inferred from names.
* `[I]` `hide_quest_marker` suppresses only the world marker, not the offer in the NPC window.
  The flag is loaded (`quest_components.hide_quest_marker`) but its use site was not isolated.
* `[R]` Client talk range is the 3 m generic interaction gate (`DAT_39ac79fc = 9.0`); the exact
  quest-talk call site was not isolated. Applied in `a7cdef3`.
* `[I]` Currency byte 3 = AA point (the client Lua has a `CURRENCY_AA_POINT` branch; the
  server only implements 0/1/2).
* `[I]` The exact `SCShowQuestArea` field order; the handler offsets are read but the server
  never emits the packet.
* `[?]` Locale texts for quest errors, NPC greeting and reward labels (the extracted
  `questcontext/locale/en_us.alb` only holds notifier widths; the text table was not found).

## 5. Gaps vs Bevy (ranked)

1. **Done (`3037053`): quest time limit / countdown.** The original publishes
   `QUEST_LEFT_TIME_UPDATED` once a second with remaining ms and hides the clock at ≤0
   (`FUN_3937cd80`; `notifier:1639-1653`). Bevy now exposes the clock through
   `HUD.quest_left_time`, ticks `Quest.time_left_ms` against `accept_time` once a second, raises
   `QUEST_LEFT_TIME_UPDATED(qtype, remaining_ms)` when at least 1000 ms are left and leaves the
   getter at ≤0 under that, so `X2Quest.GetQuestContextLeftTime` and the notifier hide the clock.
2. **Done (`f79ebe4`): the original 15 quest-mark stypes.** `Mark` now carries every style
   code `FUN_393821f0` selects (green/yellow/repeat "!", blue/green/yellow/more/half "?",
   the red/blue 70 marks, huntsign and the two flower marks), `QuestTables::mark` derives it
   from the NPC's offer, ready and talk lists and each quest's detail type (`FUN_393838f0`),
   and `marks.rs` draws each style with its original prefab model, material, scale and `.cdf`
   effect.
3. **Done (`a41e134`): multi-objective talks advance every objective.** Original loops every talk
   objective and calls `CallQuestTalk` for each (`npc_interaction.alb:220-234`). Bevy's
   `CallQuestTalk` now raises `PROGRESS_TALK_QUEST_CONTEXT` for every call, so each
   `(qtype, ctype, atype)` sends its own `CSQuestTalkMade`.
4. **Done (`0e42647`): all 16 NPC service windows.** The service router now maps the
   original kinds and windows (`npc_interaction.alb:3-105, 268-306`), including skill trainer,
   teleporter, recruiter, priest recover-exp, territory/dominion, expedition and the item-look
   converter, from the server's skill ids and the client's `npcs` flags; a lone recruiter still
   passes the battlefield module's level check. **The 3 m NPC talk gate is done in `a7cdef3`**
   (`DAT_39ac79fc = 9.0`, `TALK_RANGE`).
5. **Requirement kinds mostly default to true.** Original runs all `unit_reqs` kinds and
   reports `UNIT_REQS_OR_FAIL` / `TOO_FAR_RANGE` (`FUN_39897990`). Bevy's `PlayerFacts::holds`
   implements only level/race/gender/equip/item/skill/buff/quest/faction/honor/vocation/max
   level (~20 kinds) and returns `true` for the rest (`quests.rs:286-310`). Fix: add zone,
   time-of-day, labor margin, expedition, appellation, crime/jury, doodad range and target
   reqs, with the client's fail-to-hide semantics.
6. **Journal data stubs.** Bevy returns constants for score quests (`false`,
   `quest.lua:84`), let-it-done/over-done (`false`, `:86-89`), grade (`"normal"`, `:98`),
   use-type quest items (`{}`, `:100`), quest body/journal body (`""`, `:118,155`), AA-point
   reward (`0`, `:168`) and appellation (`nil`, `:173`); `GetActiveQuestComparedLevel` is `0`
   (`:69`). The original reads `score`, `grade_id`, `quest_context_texts`,
   `quest_act_supply_aa_points` and computes level colour (`quest_context_common.alb:245`,
   `SetLevelColor`). The past/cut-scene tab's `GetMainQuestCutSceneReplayList`
   (`x2ui-calls.txt`, `tab_past.alb:2.2`) is not defined at all. Fix: fill these from the DB
   and packet and add the completed-quest/replay list.
7. **Quest camera and decline are no-ops.** Original `UseQuestCamera` is driven by
   `quest_contexts.use_quest_camera` and `SetUseQuestCamera` (`FUN_3949b270`,
   `FUN_3949dab0`), and `DeclineDirectingQuest` closes the dialogue through
   `FUN_3937caa0(0,-1)`. Bevy: `UseQuestCamera`/`SetUseQuestCamera` return/ignore
   (`quest.lua:336-337`), `DeclineDirectingQuest` is empty (`:338`). Fix: implement the
   camera switch and the decline close.
8. **Shop AA-point currency unsupported and catalog order differs.** Original has
   `CURRENCY_AA_POINT` and the `useAAPoint` byte (`store/common.alb:89-97`;
   `CSBuyItemsPacket` layout). Bevy rejects any currency > 2 (`store.rs:367`) and always
   leaves `use_aa_point` false. Also the original orders goods by `item_id, grade_id` (client
   SQL `0x399f9218`), Bevy by merchant row id (`store.rs:39`). Fix: accept currency 3 and
   match the SQL order.
9. **Several quest packets are not decoded.** Original handles `SCShowQuestArea` 0x1af,
   `SCQuestContextFailed` 0x146, `SCQuestUnitReqFailed` 0x148 and `SCQuestMailSent` 0x150
   (`strings 0x399b96ac`, `0x399b8e68`, `0x399b8e98`, `0x399b8f54`; the area handler is
   `FUN_391df7b0`). Bevy's opcode table has only `SCQuests` 0x0f2 /
   `SCCompletedQuests` 0x0f3 (`aa-net/src/opcodes.rs:393-394`); the others would be logged
   and dropped. Fix: decode and wire the area highlight and the error messages at least.
10. **`TryStartQuestContext` argument shape differs.** The original UI forwards four
    arguments to the native (`quest_context.alb:31-33`); Bevy declares five,
    `(who,qtype,stype,doodadId,npcId)` (`quest.lua:344`), so a four-argument original call
    shifts every value after `qtype`. Fix: match the original arity/layout.

## 6. Open questions

1. Exact quest-talk call site behind the 3 m generic interaction gate (`DAT_39ac79fc = 9.0`);
   the range itself is resolved and applied (`a7cdef3`).
2. The predicate names behind stypes: which quest states map to green vs yellow "!" and to
   `question_mark_half`/`more`/`ing`.
3. Whether `hide_quest_marker` removes only the 3D mark or also the NPC dialog entry.
4. The complete quest-error code table and its English strings.
5. Whether quest timers are enforced client-side as well as server-side (the <1000 ms path
   runs a timeout handler, but the packet that informs the server was not traced).
6. `SCShowQuestArea` field order and its lifetime.
7. Quest camera behaviour (`use_quest_camera`, `SetUseQuestCamera`) and its interaction with
   directing mode.
8. Score/let-it-done/dynamic (aggro-component) quests: the UI reads them, but no shipped
   example was exercised.

## 7. Live oracle measurements

None this session. All claims above are from static code, database rows, Lua bytecode
disassembly and extracted `quest_sign_sphere.g` data. No packet capture was taken; the timer,
marker and talk-range items in §6 need one.
