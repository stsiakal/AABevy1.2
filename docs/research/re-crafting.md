# Crafting, gathering and trade packs (original 1.2 client)

> **Restored 2026-10-27 from `archive/fork-main-2026-10-27`, UNVERIFIED.** The client was restarted from NickMesser's `main`; nothing described here is wired into the game. Addresses, call chains and offsets must be re-checked against the decompile ([R]/[I]/[?]) before use. A 2026-10-27 audit found 151 of the 269 EXACT/TOLERANCE labels in `proof-levels.*` weak or wrong (report: `D:\aaemu-client\handoff\2026-10-27\audit\report-proofs.md`).

Scope: Folio/crafting window (recipe browsing, materials, labor, proficiency, batching, workbench
connection), gathering doodads (action bar, interaction list, cast, loot), specialty/trade packs
(crafting, carrying, selling, zone price ratios), labor regeneration/display and their packets.
Tags: `[R]` read in code/data, `[I]` inferred, `[?]` not found.

## 1. Sources

* 1.2 logic: `x2game.dll.c` (`C:/AAEMU_DISK/aa-re/client12/out-x2game/`), image base `0x38ff0000`.
  The Lua-native registrars are the primary map: `X2Craft` `FUN_39463c10`, `X2Equipment`
  `FUN_39471060`, `X2Store` `FUN_394afe40`, `X2Player` `FUN_39499170`.
* 1.2 UI bytecode: `D:/aa-re/client12/ui/game/scriptsbin/x2ui/` (`crafting/crafting_renewal.alb`,
  `crafting/crafting_renewal_view.alb`, `hud/main_menu_bar/laborpower_bar_set.alb`,
  `store_backpack.alb`, `messageboximpl/message_box_from_npc_interaction.alb`,
  `specialty/info.alb`, `interaction.alb`, `npc_interaction.alb`), disassembled with
  `D:/aa-re/client12/tools/albtool.py`. Proto ids and bytecode line numbers below are albtool's.
* Data: `D:/aaemu-live-bevy-srv/rs-game/Data/compact.sqlite3` (read-only).
* Packet layouts: `C:/Users/stsiakal/AAEmu/rust/crates/aaemu-game` (Rust port of the 1.2 server).
* Comparison: `D:/aaemu-client/AABevy1.2` (read-only).
* No client/server was run; section 8 says what is unmeasured.

## 2. Model and data

Recipes are `crafts` rows (7,010; `crafts.id`), joined to `skills` (`skill_id`, `consume_lp`,
`casting_time`, `max_range`, `mainhand_tool_id`, `actability_group_id`) and to
`craft_materials`/`craft_products`. Categories `actability_categories` (273) belong to groups
`actability_groups` (34): the 1.2 crafting groups are Alchemy, Construction, Cooking, Handicrafts,
Husbandry, Farming, Fishing, Logging, Gathering, Machining, Metalwork, Printing, Mining, Masonry,
Tailoring, Leatherwork, Weaponry, Carpentry, Commerce (31), Artistry; Larceny (20) is hidden and
excluded by the client's group enumeration (`FUN_39463c10`, Bevy `catalog.rs:285-300`). `[R]`

A workbench is a doodad whose `doodad_funcs` include `DoodadFuncCraftStart` (craft list in
`doodad_func_craft_start_crafts`) or `DoodadFuncCraftPack` (pack list `craft_pack_crafts`,
150 packs / 4,130 rows). 137 distinct doodad templates carry one. Both func types have no skill:
`DoodadFuncCraftStart`/`CraftPack`/`CraftAct` are client-side window openers; the Rust server
treats them as logging-only stubs (`systems/npcs/doodad/funcs/mod.rs:199-205`). `[R]`

A trade pack is an item in `item_backpacks` with `backpack_type_id = 3` (372 of 466 rows),
`normal_specialty='t'`, usually `heavy='t'` (391 rows). Most type-3 packs (302) carry
`items.buff_id = 1454` (8 more use 7221), "Overburdened": movement reduced, no teleport/glide
(23589, 24920 in `items`; `buffs` 1454/7221). `[R]`

## 3. The rules

### 3.1 Opening and modes

`ToggleRecipeBook(true)` (`crafting_renewal.alb` proto 10, menu entry) hides a visible window,
otherwise calls `SetDoodad(0,0)`, `FillCraft()`, `Show()`: a Folio with **no** workbench. With no doodad the
window shows the ability-group page only; the detail page and the craft buttons are hidden
(`crafting_renewal.alb:9.26`, which branches on `isDoodadCrafting`). `[R]`

Workbench mode is entered by the interaction, not by the menu: the window registers for
`CRAFTING_START` and takes `(doodadId, count)`; `SetDoodad(id, isDoodadCrafting=true)` then
`FillCraft` (`crafting_renewal.alb:9`, `:9.28`). `[R]` The window is exclusive with the bag
(`CheckExclusiveUI(GROUP_BAG_SHARE, UI_KIND_CRAFT)`, `:9.28`). `[R]` `[I]` The client opens the
bench window locally when the doodad's current phase has a craft-start/craft-pack func: our server
sends no interact-list packet for doodads (its `CSSelectInteractionEx` is a logging stub,
`systems/npcs/npc/packets.rs:401-434`), but the exact native call site that raises
`CRAFTING_START` was not located.

### 3.2 Browsing, search, details

`X2Craft` getters (`FUN_39463c10`): `EnumActabilityGroupTypes`,
`GetActabilityGroupInfoByGroupType`, `GetActabilityGroupNameByGroupType`,
`GetActabilityCategoryTypesGroupType`, `GetActabilityCategoryInfoByTypes`,
`GetActabilityCategoryCraftTypesByTypes`, `GetCraftName`, `GetCraftBaseInfo`,
`GetCraftMaterialInfo`, `GetCraftProductInfo`, `GetCraftTypeByItemType`, `GetUpperCraftsInfo`,
`GetExecutableCraftCount`, `GetAllCraftTypesInGroupByName`, `GetCraftTypesByMaterialName`,
`GetCategoryType`, `GetCategoryName`, plus the current-doodad variants
(`EnumActabilityGroupTypesFromCurrentDoodad`, `GetActabilityCategoryTypesFromCurrentDoodad`,
`GetCraftTypesFromCurrentDoodad`, `GetCurrentDoodadCraftTypesInGroupByName`,
`GetCurrentDoodadCraftTypesByMaterialName`). `[R]`

Browsing is group buttons (6 at once, `crafting_renewal.alb:9.25`, `SetButtonInfo`), a category
tree (`categoryList`, `UseChildStyle`, indent 8, `crafting_renewal_view.alb:2`), and a recipe list
with an executable-count column (`GetExecutableCraftCount`, `countList`, `:9.2`). Search has a
product/material radio (`CRAFT_SEARCH_CONDITION`: PRODUCT=1, MATERIAL=2), a 10-entry dropdown
limit, and a query of 1..30 characters; an empty result prints `crafting_search_empty_result`
("No matches.") through system chat (`crafting_renewal.alb:9.5`, `:9.5.0`). `[R]`

The detail page (`:9.14`, 510-646) shows product, materials with `count/amount`, higher-tier
crafts (`GetUpperCraftsInfo`, 5 per page, `:9.6`), and four requirement lines:
* Proficiency: `Proficiency: ` + `"%s%s %d/%d"` with group name, `point + modifyPoint`, and
  `required_actability_point`; red when `actability_satisfied` is false (`:9.11`).
* Labor: `Req. Labor: ` + `"%s%d"` with `needed_lp`; green when satisfied, red otherwise
  (`:9.12`).
* Workbench: `Workbench: ` + `doodad_name`, shown in non-doodad mode (`:9.14`, `K80-88`).
* Mileage (if `baselibLocale.useMileage`): `"%s: |b%s;"` with `bmmileage`.

The buttons enable only when the detail page is visible, labor and actability are satisfied, the
material/currency check (`IsPossibleCraft`) passes, and `X2Craft:IsWorkingCraft()` is false
(`:9.14` 644). Recommendation warning: if `recommend_level` is present and above the player's
level, a default dialog ("Warning" / "Proceed?") is shown before `ExecuteBatchCraftByType`
(`:9.18` 714-742). `[R]`

`GetCraftBaseInfo` (`FUN_39461860`) returns `craft_type, ac_type, title, desc, tool_type,
skill_type, cast_delay, recommend_level, doodad_name, consume_lp, needed_lp,
laborpower_satisfied, required_actability_type/name/point, actability_satisfied`. `needed_lp` is
computed from the skill's `consume_lp`, the actability's current points (plus an
enclosing-owned-spatial-object/house bonus, `FUN_398a8fe0`/`FUN_390b3a80`) and the expert row
chosen by `FUN_397a9420`: if points are below the row's next limit the previous grade's
`expert_limits.advantage` is used; the displayed cost is the base reduced by that percentage,
rounded and at least 1. `[R]` for the path, `[I]` for the exact multiply (the Ghidra body only
shows the row selection; Bevy's tested implementation `crafting.rs:92-108` is the arithmetic used
here). `laborpower_satisfied` compares it with the live labor value. `[R]`

Materials/product descriptors come from `GetCraftMaterialInfo`/`GetCraftProductInfo`
(`FUN_39462600`/`FUN_39462a70` log malformed rows). A material row has `amount`, `count`,
`mainGrade`, and an embedded item tooltip; a product row has `itemType`, `item_name`, `amount`,
`success_rate`, `useGrade`, `productGrade`. A material without a tooltip descriptor invalidates
the whole getter (Bevy test, `catalog.rs:348-360`). `[R]`

The output-grade rule is data-driven: products with `use_grade`/`item_grade_id` force a grade;
otherwise a worn/equipment material's grade is inherited and a free regrade is rolled
(`craft_products`, server `craft.rs:652-700`). The UI's warning helper tells players to reorder
materials in the bag to choose which stack is used (`warning_craft_helper`). `[R]`/`[I]`

### 3.3 Batch crafting

`ExecuteBatchCraftByType(craftType, doodadId, count)` is the only craft command
(`crafting_renewal.alb:9.18`; native `FUN_394634d0` → `FUN_39460870` → manager). It becomes
`CSExecuteCraft` **0x0F8**: `craft_id u32, obj_id BC, count i32` (`craft.rs:907-921`). `[R]`

The client does not drive the loop. The server casts the craft skill, and on the craft effect
consumes materials, hands out products, then schedules the next cast if `count > 0`
(`craft.rs:507-600`, `schedule_craft`); a stopped/failed cast ends the batch. `[R]`

The window learns progress from two native events:
* `CRAFT_STARTED(count)` — a batch cast started; `:9.29` keeps the spinner at the requested
  count.
* `CRAFT_ENDED(left)` — a cast ended; `:9.30` clears the spinner focus and re-enables the buttons;
  `left > 0` redraws the count. `X2Craft:GetLeftBatchCount()` and `IsWorkingCraft()` are exposed
  to Lua (`:9.15`, `:9.31`). `[R]`

Cancel calls `X2Craft:StopBatchCrafting()` (`:9.19`; native `FUN_39460890` →
`FUN_394d0ae0` → `FUN_393b7b50(3)`, a cast-cancel request). `EndCraftingInteraction`
(`FUN_39461f90`) reports whether an interaction is still active; closing the window is
`X2Interaction:CancelInteraction`. `[R]`

The count spinner is bounded by `GetExecutableCraftCount`; the view data also references a global
`CRAFT_MAX_COUNT` whose producer/value was not found (`crafting_renewal_view.alb` main, proto 2).
The batch buttons are disabled while a batch runs. `[R]` For reference, the server's craft labor
is charged by the skill's `ConsumeLaborPower` special effect (type 86) at full
`skills.consume_lp` (`skills/effects/special/combat.rs:469-491`), and the batch delay is the
remaining global cooldown (`craft.rs:786-826`). `[R]`

### 3.4 Workbench connection, range and permissions

The client owns three distances (`FUN_390f7b40` console variables):
`dynamic_action_bar_distance = 25.0 m` (0x3999f85c), `max_interaction_doodad_distance = 2.1 m`
(0x399a7660, also `log_doodad_interaction`). `[R]` A `FUN_39093800(obj, limit)` predicate asks the
object for the distance and returns true only when it is below the limit; coffer/doodad handlers
use it before acting. `[I]` that the same 2.1 m gate applies to workbench interaction: the action
bar only offers the action at 25 m, while the use/window is admitted at 2.1 m. The exact
end-of-interaction radius (`[?]`) was not identified.

Permission and ownership are re-checked server-side on every craft: backpack/glider slot free,
materials present, `DoodadFuncPermission` (Any, Permission1/2, OwnerOnly, Owner4,
OwnerRaidMembers, SameAccount, ZoneResidents) and housing access
(`craft.rs:282-437`, `housing_bench_access`). A failure sends
`CRAFT_CANT_ACT_ANY_MORE` + `CRAFT_PERMISSION_DENY`, `NOT_ENOUGH_REQUIRED_ITEM`,
`BACKPACK_OCCUPIED` or `ACTABILITY_NOT_ENOUGH_POINT`. `[R]`

`SetCraftingPay(money)`, `GetCraftingPayStr`, `CalcCraftingTotalPayStr` and a `CRAFTING_PAY` event
exist in the native object (`FUN_39463c10`, `FUN_39462360`), and `CSSetCraftingPayPacket`
(0xFFF; not registered in the 1.2 server) carries `obj_id BC, money i32`. No 1.2 UI script
calls these; only the item tooltip key `crafting_pay` ("Wage:") remains. `[R]` They are legacy,
not a parity requirement.

### 3.5 Gathering

Gathering uses the generic doodad path. A harvestable doodad's current phase has funcs such as
`DoodadFuncCutdowning`, `DoodadFuncOreMine`, `DoodadFuncFiberCollect`, `DoodadFuncFruitPick`,
`DoodadFuncCropHarvest`, each with `func_skill_id` (example: logging skill 10741 "Chop Tree",
`max_range 4`, `casting_time 10000`, `consume_lp 15`, `mainhand_tool_id 2657`, actability group 8
Logging; fruit pick 11109, 5 s, 1 LP, group 9 Gathering). `[R]`

Left/right-clicking or F casts the skill at the doodad; the server's InteractionEffect executes
the matching world interaction, whose loot comes from `DoodadFuncLootItem`
(`item_id, count_min/max, percent, remain_time, group_id`) or a loot pack
(`DoodadFuncLootPack`), with inventory/full checks (`doodad/funcs/items.rs:40-125`). The client
shows only the cast bar, the loot window (`SCLootItemPacket` family) and item-change toasts; no
gathering-specific window exists. `[R]`

The interaction window (`interaction.alb`) offers one button per interaction skill from the
server's list (`SCNpcInteractionSkillListPacket` 0x07B / `SCWorldInteractionSkillListPacket`
0x07E); names come from the `WI_*` keys (`WI_HARVEST`, `WI_ORE_MINE`, `WI_FIBER_COLLECT`, ...)
and `X2Interaction:GetInteractionSkillCount/Id/Name`; clicks call `SelectInteraction`
(`[I]` → `CSSelectInteractionExPacket` 0x06B) or `CancelInteraction` (`interaction.alb` main;
`FUN_39463c10`-independent natives `StartDoodadInteraction`, `GetDoodadInteractionList`). `[R]`
Our Rust server sends only the NPC list (0x07B); `CSSelectInteractionEx` is logged and ignored
(`npc/packets.rs:401-434, 439-442`), so the practical 1.2-against-our-server path is the direct
skill cast. `[R]`

Labor and proficiency are charged by the skill: `ChangeLabor(-consume_lp, actability_group_id)`
sends `SCCharacterLaborPowerChangedPacket` **0x045**: `amount i32, action i32 (actability id),
point i32 (capped delta), step u8` (`skills/progress.rs:32-51`), and adds
`|amount| * actability_rate` points to the group (`progress.rs:240-298`). `[R]`

### 3.6 Trade packs

**Crafting.** Specialty packs are ordinary `crafts` rows with `skills.actability_group_id = 31`
(Commerce), e.g. craft 2 "Specialty: Solzreed Strawberry Jam": 1×4747 + 100×8006 → 24920,
`skill 16766` "Commerce: Specialty Crafting and Packing" (6,000 ms, 60 LP), cast delay 10,000 ms,
`wi_id 41` (→ `wi_details.wi_id=74`, `lp=1`, `apply_expert=t`). 184 recipes use Commerce. `[R]`

The recipe is opened at a specialty workbench (`DoodadFuncCraftPack`/`CraftStart`, e.g. doodad
4238 "Inister's Specialty Crafting Table"); 137 templates carry bench funcs. `[R]` When the
product is a trade pack (`item_backpacks.backpack_type_id=3`), the server *auto-equips* it into
the `ES_BACKPACK` slot during the craft transaction, failing with `BACKPACK_OCCUPIED` if the slot
is taken (`craft.rs:672-700`). `[R]`

**Carrying.** The pack occupies `ES_BACKPACK` and applies `items.buff_id` (1454 "Overburdened" or
7221). The buff is duration 0, kind 1, `system=t`, `max_stack=1`; its text: "Carrying heavy
objects reduces movement speed and prevents teleporting or gliding" (`buffs` 1454 en_us). It is
tagged 831 "Heavy", 306 "Backpack", 367 "Backpack slot in use" (`tagged_buffs`), and the server's
portal refusal checks tag 831 (`npcs/portal/mod.rs:52, 908-916`). Trade packs are also blocked
from portals (`CANNOT_USE_PORTAL_WITH_BACKPACK` 610), jury duty (557), and glider change in flight
(543); "Already carrying a pack." is `BACKPACK_OCCUPIED`. `[R]` The exact speed multiplier is
`[?]`; `[I]` that `item_backpacks.heavy` plus buff 1454 is the gate (no `buff_modifiers` row exists
for 1454, so the slow is client-hardcoded).

**Selling.** Two client paths use the same server command:
* Specialty trader: `X2Store:GetSpecialtyRatioRefund(refund)` returns `(true, amount[, coin])`
  for the worn pack and the current target NPC; if it fails, `store_specialty.noGoods`
  ("You have nothing that interests the merchant."); the dialog shows
  `prices: <ratio>%` (`"%s: %d%%"`, `X2Store:GetSpecialtyRatio`) and, on OK,
  `X2Store:SellBackPackGoods()` (`message_box_from_npc_interaction.alb:6`, `:6.5.4`).
* Shadow Merchant window (`store_backpack.alb`): its update calls
  `X2Equipment:GetBackPackGoodsInfo`; nil prints `STORE_BACKPACK_NOGOODS` ("You have nothing that
  interests the merchant.") and hides, otherwise it draws the pack icon, tooltip and price
  (`"\n|m%d;"` with `refund`), OK calls `SellBackPackGoods`, and it closes on
  `NPC_INTERACTION_END`/`UNIT_EQUIPMENT_CHANGED` (`store_backpack.alb:5`, `:0`). `[R]`

`SellBackPackGoods` (registered by `FUN_394afe40`) sends `CSSellBackpackGoodsPacket` **0x042**:
`obj_id BC` (`store.rs:895-918`). The server then, for `npc_obj_id`: requires labor ≥ 60
(consumes 60 LP, Commerce), checks target distance ≤ 2.5 m, the NPC's
`specialty_bundle_id`, the pack's bundle item and item refund; price =
`floor(bundle.profit × bundle.ratio/1000) + item.refund`, multiplied by the dynamic zone ratio
(percent) and +5% interest, with a negotiation bonus; the payment is mailed after 22 h; when the
crafter differs, 80% seller / 20% crafter (`specialty.rs:230-330`; English UI: "The payment will
be mailed in 22 hours with 5% interest", "Total", "Item(s) Sold"). `[R]`

The dynamic ratio is per item and destination zone; the server defaults are start 130, −0.5 per
pack sold (ceil), +5 per hour, clamped 70..130. The client requests the current ratio with
`CSSpecialtyRatioPacket` 0x043 (`id u32`) and receives `SCSpecialtyRatioPacket` 0x099 (`ratio
i32`) (`specialty.rs:40-180, 574-586`). `[R]`

**Trade info window.** `specialty/info.alb` ("Trade Info") has a production-zone combo
(`X2Store:GetProductionZoneGroups`, `FUN_394af270`: distinct `specialties.row_zone_group_id`
with zone names) labelled "Crafted", a destination combo (`GetSellableZoneGroups(from)`,
`FUN_394aedd0`: rows with `row_zone_group_id = from` and `vendor_exist = 't'`) labelled "Sold",
and a search that calls `GetSpecialtyRatioBetween(from,to)` under a short cooldown ("Please wait
a moment."). Rows show item icon/name and ratio `%` (`specialty/info.alb`). The server side is
`CSRequestSpecialtyCurrentPacket` **0x131** (`from u16, to u16`) →
`SCSpecialtyCurrentPacket` **0x1F4** (`count i32, from u16, to u16, (item u32, rate u32)[]`)
(`specialty.rs:462-608`). `[R]`

### 3.7 Labor regeneration and display

The bar is `laborpower_bar_set.alb`: text `"%s: |,%d; / |,%d;\n"` (labor_power, current, max),
then one policy tip chosen from `X2Player:GetPayMethod`/`GetPayLocation`/`GetFeatureSet`:
person=5/5 min online and offline; premium="$1 online, $2 offline per 5 min"; PC-bang 10 (en_us)
or 15 (ko) online per 5 min; trial/siege 5/5 min online only. OnEnter shows
`labor_power_premium_service_tip` / `labor_power_person_person_tip` etc. (`:0`). The tick is 5
minutes in every row. `[R]` The bar repaints on `LABORPOWER_CHANGED`, `PLAYER_BM_POINT`,
`PREMIUM_GRADE_CHANGE`, `CHANGE_PAY_METHOD` (`:4`), reading `GetLaborPower`/`GetMaxLaborPower`.
`[R]`

`SCCharacterLaborPowerChangedPacket` is the only labor delta; the client adds the signed amount
(native `3930b4d0`, Bevy `aa-net/state/stats.rs:195-203`). `[R]` Maximum labor is computed, not
packed: `GetMaxLaborPower` (`FUN_39496130`) branches on a premium feature bit, otherwise
`FUN_397a7aa0` = clamp(`C25 + (param / C27) × C28`, `C25`, `C26`) with content_configs
25=1000, 26=5000, 27=100, 28=10 (so 2,000 normal / 5,000 premium).
`premium_benefits` rows give `max_labor` 2000/5000 and online/offline 5/0 and 10/5.
`GetPremiumServiceOnlineLaborPower`/`Offline` (`FUN_39496240`/`39496330`) supply the tooltip
numbers. `[R]`/`[I]` for the `param` producer.

## 4. Numeric parameters

| Item | Value | Source |
|---|---|---|
| Recipes / groups / categories | 7,010 / 34 / 273 | `crafts`, `actability_groups`, `actability_categories` |
| Bench doodad templates | 137 | `doodad_funcs` CraftStart/CraftPack |
| Action-bar distance | 25.0 m | cvar `dynamic_action_bar_distance`, `FUN_390f7b40` |
| Interaction distance | 2.1 m | cvar `max_interaction_doodad_distance`, `FUN_390f7b40` |
| Batch count | spinner max = `GetExecutableCraftCount`; `CRAFT_MAX_COUNT` unknown `[?]` | `crafting_renewal_view.alb` |
| Search query | 1..30 chars | `:9.5`, `:9.14` |
| Higher-tier page | 5 items | `PAGE_PER_HIGH_RANK_ITEM = 5` |
| Labor tick | 5 min | `ui_texts` labor tips; server `config.rs:579` |
| Max labor | 2,000 / 5,000 | `FUN_39496130`, `premium_benefits` |
| Expertise rows | advantage 0,5,10,15,20,23,26,29,30 | `expert_limits` |
| Specialty interest / mail delay | 5% / 22 h | `sell_backpack_content`, `specialty.rs` |
| Specialty ratio (server defaults) | 70..130, +5/h, −0.5/pack | `specialty.rs:40-47` |
| Sell labor | 60 LP, Commerce 31 | `specialty.rs:284-300` |
| Pack SLOT | `ES_BACKPACK` (equipment slot 26, slotType 30) | `item_backpacks`, `store_backpack.alb` |

Packets: `CSExecuteCraft` 0x0F8 (`u32, BC, i32`), `SCCraftFailed` 0x1BF (`u32 id, i32 count,
u32 craftId × count ≤20`; `craft.rs:960-990`), `SCCharacterLaborPowerChanged` 0x045
(`i32,i32,i32,u8`), `CSSellBackpackGoods` 0x042 (`BC`), `CSSpecialtyRatio` 0x043 (`u32`),
`SCSpecialtyRatio` 0x099 (`i32`), `CSRequestSpecialtyCurrent` 0x131 (`u16,u16`),
`SCSpecialtyCurrent` 0x1F4 (`i32,u16,u16,(u32,u32)[]`). `[R]`

## 5. What is inferred

* `[I]` The 2.1 m cvar is the workbench/window range and the 25 m value is the bar's offer range;
  `FUN_39093800` proves a `< limit` predicate but no call site for the craft window was found.
* `[I]` `needed_lp` = base LP reduced by the selected expert row's `advantage`; the decompiled
  `FUN_397a9420` shows row selection only (Bevy's formula is the arithmetic used).
* `[I]` Trade-pack slowdown is keyed off `item_backpacks.heavy` (no buff modifier row for 1454);
  the multiplier was not found.
* `[I]` `GetMaxLaborPower`'s `param` is the premium max-labor benefit (formula fits 2,000/5,000).
* `[?]` End-of-interaction radius; the crafting window did not visibly re-check range on the
  craft click.
* `[?]` Whether the client fires `ACTABILITY_EXPERT_CHANGED` on every labor-charged point gain.
* `[?]` `GetBackPackGoodsInfo`'s full field set (the Ghidra body sets name/itemType/itemGrade
  only; the window also reads `lookType`, `refund`, `itemPrice`).

## 6. Gaps vs Bevy (ranked)

1. **World workbenches never open the Folio.** Original: any doodad with `DoodadFuncCraftStart`
   /`DoodadFuncCraftPack` opens the window from the interaction/action bar. Bevy loads the craft
   lists (`crafting/data.rs:285`) but only calls `open_bench` for a finished *own* house's
   workbench (`hud/doodad.rs:321, 875, 968`), and `Tables::add` drops skill-less funcs
   (`doodad.rs:156-166`), so a town anvil/farm table offers nothing. Fix: treat craft funcs as a use that
   calls `open_bench(object, template)` and fires `CRAFTING_START`; drop the ownership test and
   let the server's permission check answer. **Done (6d00d0d):** craft funcs are a `Uses` entry on
   the bar and open the Folio from the first/second action; the world doodad is no longer
   ownership-gated.
2. **Hide-only workbenches of other players.** `hud/doodad.rs:321` requires
   `state.owner_id == data.character_id`; the original offers the bench with the owner's
   `DoodadFuncPermission` (SameAccount/ZoneResidents) and `SetCraftingPay`. Fix: offer G to
   permitted visitors, server already validates (`craft.rs:330-430`). **Done (6d00d0d):** the
   client resolves the phase's first-func `perm_id` and offers Any/SameAccount/ZoneResidents to
   visitors (own house always); the server remains authoritative.
3. **Trade-pack sale missing.** `X2Store:SellBackPackGoods` (stub, `aa-ui/src/lua/stubs.rs:1541`)
   never sends `CSSellBackpackGoods` 0x042; `X2Equipment:GetBackPackGoodsInfo` is a stub
   (`stubs.rs:408`), so the Shadow Merchant window (`store_backpack.alb`) cannot price or sell.
   Bevy has the quote (`hud/specialty/quote.rs:105-190`) but no command/sale path. Fix: implement
   both natives, send 0x042 with the target NPC, and handle the 22 h mail receipt. **Done (wire
   075260d, natives 38a2f5c):** 0x042 is one `bc` NPC id checked against the server readers,
   `GetBackPackGoodsInfo` returns the type-3 item tooltip/refund, `SellBackPackGoods` queues the
   sale and resets the native ratio; the delayed `SYS_SELL_BACKPACK` mail arrives on the existing
   `SCGotMail`/mailbox path. **Routing done (8f1255a):** the native client adds the trade action
   from the NPC template itself, not from the server's interaction reply — `npcs.trader` raises
   `NPC_INTERACTION_START("trader")` (the `store_backpack.alb` Shadow Merchant window) and
   `npcs.specialty` raises `"specialty"`; the server's interaction skills 14866 / 16376 are also
   recognised. A lone trade action opens at once, like the other service windows.
4. **Trade-info window absent.** `GetProductionZoneGroups` (`stubs.rs:813`),
   `GetSellableZoneGroups` (`:912`), `GetSpecialtyRatioBetween` (`:941`) are stubs; no 0x131
   request or 0x1F4 receipt is decoded. Original: `specialty/info.alb` lists production /
   destination zones and dynamic ratios (server `specialty.rs:462-608`). Fix: load
   `specialties`/zone names, send 0x131, feed `SC 0x1F4` into the window. **Done (codec 6405a87,
   natives 8f1255a):** `GetProductionZoneGroups` returns the distinct `specialties.row_zone_group_id`
   with zone names, `GetSellableZoneGroups(from)` the `vendor_exist` destinations, and
   `GetSpecialtyRatioBetween(from,to)` sends `CS 0x131` under the native 5 s cooldown (`0x1F4`,
   `count i32, from u16, to u16, (item u32, rate u32)[]`); `SC 0x1F4` raises
   `SPECIALTY_RATIO_BETWEEN_INFO` with `{ itemInfo, ratio }` rows for `specialty/info.alb`.
5. **Workbench range not enforced.** Original admits interaction at <2.1 m (cvar; `FUN_39093800`)
   and offers the bar at 25 m; Bevy uses `skill.max_range + 1` or `BASE_REACH 4 m`
   (`hud/doodad.rs:55-56`) and `batch::command` never checks distance (`craft/batch.rs:180-210`),
   so a Folio left open lets the player craft from anywhere. Fix: gate the action/window and each
   craft submission on the 2.1 m surface distance. **Done (6d00d0d):** `surface_distance` (centre
   minus player/`sim_radius`) gates opening the Folio and `batch::command`; the bar is offered at
   25 m.
6. **Heavy-pack penalties not applied client-side.** Original: pack buff 1454/7221
   ("Overburdened") reduces speed and blocks teleport/glide, with dedicated refusals
   (`CANNOT_USE_PORTAL_WITH_BACKPACK` 610, glider change 543). Bevy applies the item buff
   (`economy/bridges.rs:293`) but has no `heavy`/BACKPACK check in sprint (`ingame/sprint.rs`),
   glide (`ingame/glide.rs:573-605`) or portal (`hud/portal.rs`); the portal refusal comes only
   from the server's tag-831 check (`npcs/portal/mod.rs:915`). Fix: walk speed + glider/portal
   refusal while a type-3/heavy pack is worn.
7. **Proficiency display goes stale after a craft.** Original redraws the detail page's
   Proficiency line on `ACTABILITY_EXPERT_CHANGED` (`crafting_renewal.alb:9.34`); `SC045`
   updates the points in `aa-net/actability.rs:208-219`, but Bevy only fires
   `LABORPOWER_CHANGED` (`hud/api.rs:1179`), so `GetMyActabilityInfo` numbers shown in an open
   Folio do not advance. Fix: fire `ACTABILITY_EXPERT_CHANGED`/`ACTABILITY_REFRESH_ALL` when a
   labor packet changes an actability point or step.
8. **Max labor hardcoded and premium tooltip dead.** Bevy sets `max_labor: LABOR_MAX`
   (`hud/api.rs:822`; `LABOR_MAX=2000`) instead of 2,000/5,000 from premium
   (`FUN_39496130`, `premium_benefits`), and `GetPremiumServiceOnlineLaborPower`/`Offline`,
   `GetPayMethod`, `GetPayLocation` are stubs (`stubs.rs:774-798`), so the premium/PC-bang tooltip
   lines are wrong and a premium bar overflows 2,000. Fix: load `premium_benefits`, expose the
   four getters, choose the max by premium state.

Lower-ranked but real: (9) `EnumCraftTypesFromCurrentDoodad`, `SetCraftingPay`,
`GetCraftingPayStr`, `CalcCraftingTotalPayStr`, `EndCraftingInteraction` are absent from Bevy
(unused by the 1.2 UI); (10) the client ignores `SCWorldInteractionSkillList` (decoded in
`aa-net/interaction.rs:108-140`, no HUD consumer), using locally built menus instead — harmless
with our server but not the original path.

## 7. Open questions

1. Does the original fire `ACTABILITY_EXPERT_CHANGED` on each `SC045` point gain (a capture of an
   open Folio during repeated crafts settles gap 7)?
2. What is the heavy-pack speed multiplier, and is it `item_backpacks.heavy` or a hardcoded
   `backpack_type_id == 3` rule?
3. Which permission mode admits a non-owner to a house workbench, and does `SetCraftingPay`
   appear in 1.2 housing at all (no UI caller found)?
4. Is the craft window closed by range (2.1 m) or only by `INTERACTION_END`?
5. Does the client's `GetExecutableCraftCount` apply labor for material recipes? Bevy explicitly
   does not (`craft/catalog.rs:174-205`), claiming a getter difference; one retail measurement
   would fix it.
6. The client displays a discounted `needed_lp` for high proficiency, but our server's
   `ConsumeLaborPower` effect charges the full `skills.consume_lp`; a capture of the labor delta
   during a master-grade craft settles whether the discount is real.
7. The value/producer of the `CRAFT_MAX_COUNT` global (view data) was not found.

## 8. Live oracle measurements

None. This pass was static only (2026-10-23): no client, server or capture was started. The
existing `docs/aa-client-specialty.md` captures cover only the ratio receipt and quote dialog; no
capture covers workbench opening, gathering, batch progression or a pack sale.
