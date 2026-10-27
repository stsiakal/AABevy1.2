# Interaction packets (1.2 client ↔ our AAEmu server)

Companion to [`MAP.md`](MAP.md). x2game writers are in the `0x397abxxx`/`0x397acxxx` group and all
end in the send finalizer `thunk_FUN_3ac1058c` (the same hook used by the movement/combat maps).
Field order on the wire is taken from our server's readers (`aaemu-game/src/systems/...`) because
that is the layout the 1.2 client was written against; the **client-side** writer argument order and
the pre-serialization field offsets are cited per function. C→S opcodes are from
`aaemu-game/src/opcodes.rs` (`cs`), S→C from the same file (`sc`).

## 1. C→S

### 1.1 CSSelectInteractionEx 0x06B

`bc targetId, i32 interactionEx, i32 var1` (server `systems/npcs/npc/packets.rs`
`CSSelectInteractionExPacket`).

* sender `FUN_393b3120(manager, interaction, var1)`; writer `FUN_397ac0d0(objId, interaction,
  var1)` stores `+0xc = objId` (read from `unit+0x1aa0`), `+0x10 = interaction`, `+0x14 = var1`,
  then `thunk_FUN_3ac1058c` sends [R].
* reached from `X2Interaction:SelectInteraction(id)` `FUN_3947dac0` when the manager has a current
  interaction object (`DAT_39aff9bc+0x48 != 0`); otherwise SelectInteraction goes straight to the
  doodad use path (`FUN_393b3ec0`).

### 1.2 CSStartInteraction 0x068

`bc npcObjId, bc objId, i32 extraInfo, i32 pickId, u8 mouseButton, i32 modifierKeys` (server
`systems/npcs/npc/packets.rs`).

* sender `FUN_393be030(unit, obj, extra, mouseButton, modifiers)`; writer `FUN_397ac0a0` stores
  `+0xc = param1`, `+0x10 = DAT_3a5e1158` (0), `+0x14 = param2`, `+0x18 = param3`,
  `+0x1c = byte param4`, `+0x20 = param5` [R].
* called from the click dispatcher `FUN_390f3780` (`DAT_3a5a4dbc` target), and from the service
  handlers (`FUN_393bf310`/`FUN_393bf500`) when the server must answer with an interaction skill
  list.

### 1.3 CSInteractNPC 0x065 / CSInteractNPCEnd 0x066

* `CSInteractNPC`: `bc npcObjId, bool isTargetChanged`; writer `FUN_397ab740(objId, byte)`
  (`+0xc = objId`, `+0x10 = byte`), sent by `FUN_393be280` when the target's `+0x410 == 1`.
* `CSInteractNPCEnd`: `bc objId`; writer `FUN_397ab760(objId)` (`+0xc = objId`), sent by
  `FUN_393be180(1)` when the previous target was kind 1.

### 1.4 CSCofferInteraction 0x06C

`bc objId, u8 open`; writer `FUN_397ac0f0(objId, byte)` (`+0xc = objId`, `+0x10 = byte`), sender
`FUN_3902e190(doodadId)` → `FUN_397ac0f0(id, 1)` then the finalizer. Reached from the coffer func
(`FUN_393b7c00`) and the coffer window.

### 1.5 CSChangeDoodadPhase 0x0E9

`bc objId, u32, u32, u32` (server `CSChangeDoodadPhasePacket`, logged only). Writer
`FUN_397ac8d0(objId, skill, funcField, actualFunc)` stores `+0xc = objId`, `+0x18 = skill`,
`+0x10 = funcField`, `+0x14 = actualFunc`; sent by `FUN_393b8240` (func-type case 0x3F) [R].

### 1.6 CSStartSkill 0x052 (doodad target)

`u32 skillId, u8 casterType=0, bc casterObjId, u8 targetType=4, bc targetObjId, u8 flag=0`
(server `systems/skills/`; see `doodad-use-server.md`). The interaction code uses
`thunk 0x39397e50` (`thunk_FUN_3a6aa27f`) with

* caster block `{type 0, id = player->actorId}`,
* target block `{type 4, id = doodad->objId}` (or `{type 0, id = unit->objId}` for a unit),
* object block `{0}`.

The doodad path never calls the combat `UseSkill` front-end `0x393989e0`; it goes straight to the
split tail writer. The tail is the same one the combat request body uses, so the packet layout is
the same `CSStartSkill` and the send hook `0x3ac1058c` sees it.

### 1.7 CSLootOpenBag 0x08E / CSLootItem 0x08F / CSLootCloseBag 0x090

* **Open** — server reads `bc obj_id, bc obj2_id, bool loot_all` (`looting.rs`
  `CSLootOpenBagPacket`). Writers:
  * `FUN_397abed0(objId, lootAll)` (unit/corpse): `+0xc = objId`, `+0x10 = 0`
    (`DAT_3a5e1158`), `+0x14 = byte`.
  * `FUN_397abef0(objId, lootAll)` (npc variant): `+0x10 = objId`, `+0xc = 0`
    (`DAT_3a5e1130`), `+0x14 = byte`.
  Sent by `FUN_394fb010`/`FUN_394fb160` after the mounted check and the already-open check.
* **Take** — server reads `u16 item_index, u16 owner_type, bc owner_obj_id, u16 u1, u16 u2`
  (`CSLootItemPacket`). Writer `FUN_397abf10(a, b, c)` writes the packed index/type via
  `FUN_397cc900` and stores `c` at `+0x18`; sent by `FUN_394f9790(slot)` from the loot row.
* **Close** — server reads `u16 item_index, u16 owner_type, bc owner_obj_id, u8 b`
  (`CSLootCloseBagPacket`). Writer `FUN_397abf40(itemIndexType, ownerObjId)` stores `+0x10` and
  `+0x14`; sent by `FUN_394fab30` when `manager+2 != 0`.

### 1.8 CSLootDice 0x091

`X2Loot:DoDiceAction(key, roll)` → `FUN_394fb250` + `FUN_394fa770`; the exact writer was not
re-read (same packet group, server `CSLootDicePacket`). `RequestNextDiceItem` →
`FUN_394fb250` + `FUN_394fad50` (0x0FB/0x0FC path).

### 1.9 CSInteractNPCEnd / CSSelectInteractionEx from Lua

`X2Interaction:CancelNPCInteraction()` → `FUN_3947da90` → `FUN_393be180(1)` (0x066 when the target
is kind 1). `X2Interaction:CancelInteraction()` → `FUN_3947da50` →
`FUN_393b3ec0(10994, 0, DAT_3a5aa6e8, ...)` (clears the interaction; no packet of its own).

## 2. S→C

All handlers are registered in the packet table of `FUN_391eb9d0` (`FUN_391be…` wrappers); the
opcode numbers below are from our server's `opcodes.rs` (the client's own names are the handler
strings). The handler reads a packet object whose body starts at `+0xc` (the `FUN_390115a0` /
`FUN_390116d0` profiler pair brackets every handler).

| opcode | name | handler | called function | fields read |
|---|---|---|---|---|
| 0x07B | `SCNpcInteractionSkillList` | `FUN_391d8f80` | `FUN_393be850` | `+0xc` npcId, `+0x1c..` skills, `+0x4c` count/arg |
| 0x07C | `SCNpcInteractionEndedByZone` | `FUN_391d9000` | `FUN_393be180(0)` | `+0xc` objId |
| 0x07E | `SCWorldInteractionSkillList` | `FUN_391d9080` | `FUN_393b7440` | `+0x14` block, `+0x54` byte, `+0x58` |
| 0x07F | `SCWorldInteractionCanceled` | `FUN_391d9100` | `FUN_393b30a0/30e0` | `+0xc` source, `+0x10` target |
| 0x080 | `SCNpcInteractionStatusChanged` | `FUN_391d91a0` | `FUN_393be250` | `+0xc` objId, `+0x10` byte |
| 0x081 | `SCVegetationCutdowning` | `FUN_391da940` | `FUN_393b7800` | `+0xc` unit, `+0x10` doodad |
| 0x096 | `SCCofferContentsUpdate` | `FUN_391d5590` | coffer manager | `+0xc` doodad, `+0x10/+0x14` target, `+0x18` count, `+0x19` slot types, `+0x38` rows |
| 0x0F6 | `SCLootableState` | `FUN_391d9780` | `FUN_394fa110/faf40` | `+0x10/+0x14` type/id, `+0x18` hasLoot |
| 0x0F7 | `SCUnitLootingState` | `FUN_391d9810` | `FUN_393546f0` | `+0xc` unit, `+0x10` looting |
| 0x0F8 | `SCLootBagData` | `FUN_391d9890` | `FUN_394fa9a0` | `+0x10` rows, `+0xc051` lootAll |
| 0x0F9 | `SCLootItemTook` | `FUN_391d9910` | `FUN_394fa6c0` | `+0xc` item, `+0x10` index, `+0x14` owner, `+0x18` count |
| 0x0FA | `SCLootItemFailed` | `FUN_391e56d0` | `FUN_394f9c20` | `+0x10/+0x14` item/index, `+0x18` reason |
| 0x0FB | `SCLootDice` | `FUN_391d9990` | `FUN_394fae70`, `FUN_394fb250` | — |
| 0x0FC | `SCLootDiceNotify` | `FUN_391d9a10` | dice UI | — |
| 0x0FD | `SCLootDiceSummary` | `FUN_391d9b50` | `FUN_394f9410` | — |
| 0x0E2 | `SCTeamLootingRuleChanged` | `FUN_391d9bd0` | `FUN_394b1f50`-bound UI | — |
| 0x10F | `SCDoodadPhaseChanged` | `FUN_391da5c0` | `FUN_39035280` → `FUN_393a6c60` | `+0xc` objId, `+0x10`, `+0x14`, `+0x18`, `+0x1c` |
| 0x157 | `SCDoodadPhaseMsg` | `FUN_391dc720` | `FUN_3902e0f0` | `+0xc` objId, `+0x10` phase |
| 0x097? | `SCSplitCofferItemResult` | `FUN_391d56c0` | error/toast + refresh | `+0xc` error, `+0x10/+0x14` item uids |

The coffer split opcode is not in our server's `opcodes.rs` (the name is the client's own
`OnSplitCofferItemResult`); the handler body was read but the S→C opcode is **[?]**.

`SCDoodadCreated` 0x10C / `SCDoodadsCreated` 0x112 / `SCDoodadRemoved` 0x10D / `SCDoodadsRemoved`
0x113 feed the doodad manager (`FUN_391da420` → `FUN_390382d0`); they are outside the S2 rules but
are needed for the doodad objects the interaction code looks up. `SCDoodadCreated` body starts with
`bc objId, u32 templateId, …` (`FUN_391da420` logs `type(+0x14), id(+0x10)`), and the client stores
`objId` at `+0x7c` (base ctor `FUN_3983b690`) and the phase func group at `+0x220`.

## 3. Send/receive hooks for the harness

* **C→S**: hook `0x3ac1058c` (the finalizer) and copy the packet buffer; the movement/combat maps
  already use this. Every writer in §1 reaches it.
* **S→C**: the driver can call the handler functions directly with a synthetic packet object
  (`body` at `+0xc`) or call the inner functions (`FUN_393be850`, `FUN_393b7440`, …); both are
  modelled in [`difftest.md`](difftest.md).
* **UI events**: hook `FUN_39202db0` for the *game* event stream and `FUN_39424220`/`FUN_39424240`/
  `FUN_39424260` for the Lua UI events. The relevant UI event ids are in
  [`structs.md`](structs.md) §10.4.
* The interaction manager/loot manager objects are globals (`DAT_39aff9bc`, `DAT_39aff9b4`,
  `DAT_39aff9b8`, `DAT_39affa0c`); the driver can snapshot/restore them between tests.
