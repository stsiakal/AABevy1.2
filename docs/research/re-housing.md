# Housing and farming beyond placement (original 1.2 client)

Scope: everything after a house/farm is placed — construction steps and the two construction windows, the finished-house maintain window (tax, permission, recover, sale, name, demolish), F/G actions, decoration placement, public/house farms and crops. Placement is in `re-placement.md` and not repeated.

Sources: `x2game.dll.c` (addresses are 1.2 VAs, image base `0x38FF0000`), `funcs.csv`/`calls.tsv`/`strings-xrefs.tsv`, the UI bytecode under `D:/aa-re/client12/ui/game/scriptsbin/x2ui/housing` and `commonfarm` (disassembled with `albtool.py`), `compact.sqlite3`, the AAEmu Rust server for packet layouts, and `AABevy1.2` (`main`). No Ghidra project, no client/server run, no new capture. Confidence: **[R]** code/data, **[I]** inferred, **[?]** not found.

## 1. The rules

### 1.1 Construction state and the build packets

A house is a housing unit identified by `tl` (runtime id) and design id (`housings.id`). The client keeps one object per `tl` in the housing manager and looks it up with `FUN_3932b510`; its fields used here are `+0x8` template, `+0x140` `AllAction`, `+0x144` `CurrentAction`, `+0xac` permission, `+0xb0` sale price, `+0xb4` designated buyer id, `+0xb8` buyer name, `+0x1a4` allow-recover, `+0x18` unit obj id, `+0x20` owner object id. **[R]** (`FUN_3932b510`, `FUN_3932fa00`, `FUN_39325df0`, `FUN_3932b680`, `FUN_3932be00`).

`SCHouseState` (0xBC) carries the full house (`tl u16, db_id u32, obj_id bc, template u32, model u32, co_owner u32, owner u32, owner_name str, account u32, permission u8, all i32, current i32, tax i32, x i64, y i64, z f32, name str, allow_recover bool, sell_price u32, sell_to_id u32, sell_to_name str`). **[R]** server writer `models/house.rs::write` (aaemu-game `src/models/house.rs:99`), client reader `FUN_3932fa00`/`FUN_3932f6b0`. A finished house sends `0/0` for all/current (`current_step == -1`), so **`under construction` is `current < all`, and 0/0 means finished** (`houses.rs:86` in Bevy does the same).

`SCHouseBuildProgress` (0xBD) is `tl u16, model u32, all i32, current i32` (server `packets.rs:65`). The handler `FUN_39327990` stores all/current, replaces the house model with the packet's model, plays `start_effect`, and fires `HOUSE_STEP_INFO_UPDATED` (0x103) when the current action changed. **[R]**

Construction actions are ordinary unit skills, one per `housing_build_steps` row. Build sends `CSCreateHouse`; each later *cast* of the step skill advances progress. `CSCreateHouse` is 0x57: `design u32, x i64, y i64, z f32, z_rot f32, item u64, money i32, ht i32, auto_use_aa_point bool` (server `packets.rs:240`; client `FUN_397b09d0` built by `FUN_3932fe60`). `NextStepFromBuildCheck` calls `FUN_3932fe60` with the client's "auto use AA point" option (`FUN_397ccd50`); `PrevStepFromBuildCheck` steps the builder back (`FUN_39017220(3)`), it does not cancel placement. **[R]** (`x2game FUN_39477cb0`, `FUN_39477ce0`).

### 1.2 Start Construction window (before the house exists)

`SCConstructHouseTax` (0xC5) `design u32, heavy_count i32, normal_count i32, is_heavy bool, base i32, deposit i32, total i32` (server `packets.rs:37`) is stored and fired as `HOUSE_BUILD_INFO` (0x100) by `FUN_3932e530`, which also stores the total in the builder (`+0x21c`) for the later `CSCreateHouse`. **[R]** The stock Lua `ShowBuildWindow(type, base, total, heavyCnt, normalCnt, isHeavy, deposit, taxItem)` is the popup; `build_window_view.alb` shows:

* title/name from `GetHousingModelName(type)`, "Begin construction";
* `Deposit`, `Base Tax`, `Buildings Owned`, `Total Taxes`, heavy-tax note only when `is_heavy`;
* one icon+`consumeItemStack`+`needActions` group per `GetHousingModelConstructionInfo(type)` row, arrows, "Each stage uses Labor";
* the tax-item icon with `CountTaxItemInBag()/CountTaxItemForTax(total)` as `have/need`; red and Build disabled when short. **[R]** (`build_window_view.alb` protos 0–3; `build_window.alb` proto 0).

`CSCreateHouse` is rate-limited 5000 ms in the client (`re-placement.md` §2) and the server error name is `CANNOT_LOCATE_DOODAD_NOW`.

### 1.3 Construction/material window of an unfinished house

Interaction with any house runs `FUN_393befd0`: it checks range/selection, **sends `CSRequestHouseTax` (0x5C, `tl u16`)**, stores the selected `tl`, and fires `HOUSE_INTERACTION_START` (0x6D) with `(unit type, house category, 1)`, where the category string is `seafarm` for `housings.category_id 7`, `farm` for 8/16/17, `house` for 9/10/11/12/15/18, else nil. **[R]** (`FUN_393befd0`; event id from the registration order of `FUN_39214380`, cross-checked by the measured `BUILDER_STEP` 0x224).

`housing_manager.alb` proto 0 handles the event: if `IsHouseUnderConstruction()` it calls `ShowMaterialWindow(true)`; if `IsCastle()` it also fills it; otherwise it opens the maintain window (own vs other variant). So **the interaction only opens the window; it never casts a build skill**. The window itself (`material.alb`) is created once, its `okButton` `OnHide` only refreshes step info, and its `Demolish` button (enabled only when `IsMyHouse`) asks a default dialog then calls `X2House:Demolish()`. **[R]** (`material.alb` proto 0, 0.0, 0.1; `housing_manager.alb` proto 0).

Content from `X2House`:

* `GetHouseOwnerName`, `GetHouseZoneName`, `IsCastle`, `IsMyHouse`, `IsSiegePeriod`;
* `GetHouseBaseProgress()` = `AllAction`, `GetHouseRemainProgress()` = `AllAction − CurrentAction` (`FUN_394785d0`, `FUN_394784f0`), displayed as a/b;
* `GetHouseConstructionStepInfo()` (`FUN_3947b3b0`) builds one row per build-step material with `consumeItemStack`, `isLaborPower`, `needActions`, `lookType`/grade/step (`FUN_39479460`), the same rows as the pre-build window;
* `CountTaxItemForTax(money)` returns a count from the money string; the divisor is the tax item's 10,000-copper unit **[I]** (`FUN_39478d80`). `CountTaxItemInBag()` sums **two** tax-certificate ids (`FUN_39478cd0`): 31891 and bound 31892 **[I]**, matching `manager.rs:39-40`.

`SCHouseBuildProgress` while the window is open re-fires `HOUSE_STEP_INFO_UPDATED`, which calls `materialWindow:UpdateStepInfo("housing")`; the last step produces a finished house on the next interaction. **[R]** (`FUN_39327990`, `housing_manager.alb` proto 4).

### 1.4 F/G dynamic actions (houses and farms)

The client does **not** have a per-house skill table. The action bar is the current "interaction indicator" list at `DAT_39affa0c+0x20`; for a unit the list is filled by `FUN_393bcb90` → `FUN_3910a950`, which appends every skill id in the unit's received `SCUnitState` skill vector (vtable `+0x60`), plus the interaction skills of the unit's current doodad/slave. **[R]** `GetDynamicActionCount` is the list length (`FUN_39442370`), `GetDynamicActionSkill(i)` the i-th skill (`FUN_39442400`), `ExecuteDynamicAction(i)` casts it through `FUN_393b1de0` (which refuses when the target unit is the caster's own slave or when no target). **[R]**

So **the F/G entries and the own-vs-other difference are whatever the server put in the house unit's skill list**, not a client ownership test: `FUN_393bcb90`/`FUN_3910a950` have no `IsMyHouse` filter, and whether the farm craft skill is sent only to the owner is a server decision **[I]**. Relevant skills: info 15212 (6 m) / 11001 (4 m); build 14575/14641/14574 (25 LP, 10 m, 30 s), 18553 (10 LP, 10 m, 10 s); demolish 11404 (20 m, 20 s); crops 13625 Water Plant (6 m, 1.5 s, 0.5 s CD), 13788 Fertilize, 13789 Uproot, 13980 Harvest. **[R]** `compact.sqlite3 skills`.

### 1.5 Maintain window of a finished house

Same interaction opens it: `HOUSE_INTERACTION_START` → `ShowHousingMainTainWindow(true)` → `UpdateMyHouseDefaultInfo` (own) or `UpdateHouseDefaultInfo` (other) → `UpdateMaintainWindowHeight`. Own: permission radios, rename, furniture-lock, sale registration, buy, demolish. Other: owner/zone/permission/sale info and Buy when for sale. **[R]** (`maintain_window.alb` protos 4–9; `housing_manager.alb` proto 0).

**Tax.** Every interaction sends `CSRequestHouseTax`; the answer `SCHouseTaxInfo` (0xC4: `tl u16, dominion_rate i32, money i32, money2 i32, due i64, paid bool, weeks i32, heavy bool`; server `packets.rs:142`) is converted by `FUN_3932e210` (due → `os.date("*t")` table, remaining time string) and fired as `HOUSE_TAX_INFO` (0xFF). The Lua picks `UpdateMyHouseTaxInfo(8 args)` or `UpdateHouseTaxInfo(3 args)`; an unfinished house routes it to the material window instead. **[R]**

**Permission.** `GetHousePermission()` returns `wire + 1` (`FUN_39478430`), an `always_public` flag from the template, and the stock radio mapping `to_permission = {1,4,2,3}` (private, family, public, guild); `SetHousePermission(i)` subtracts 1 and sends `CSChangeHousePermission` (0x5A: `tl u16, permission u8`; server `packets.rs:338`) only when the value changed. The client pre-checks the family (3) and guild (1) paths (errors 0x178/0x137) plus one template flag (0x80) before sending (`FUN_3932bf90`; the flag's meaning is **[I]**). The server answers `SCHousePermissionChanged` (0xBE), which updates the house and fires `HOUSE_PERMISSION_UPDATED` (0x10D) when it is the selected house. **[R]** (`FUN_39325df0`).

**Recover / furniture lock.** `GetHouseAllowRecover()` is `house+0x1a4`; the checkbox is inverted: `SetHouseAllowRecover(not furnitureCheckbox:GetChecked())` sends `CSAllowHousingRecover` (0x5D: `tl u16`) only when it differs and the house is the player's (`FUN_39477d80`). The server's `SCHousingRecoverToggle` (0xC6: `tl, bool`) updates the field and fires `HOUSE_INFO_UPDATED` (0x10F). **[R]** (`FUN_3932be00`, `maintain_window.alb` proto 0.8).

**Name.** The rename button opens the shared edit dialog, which calls `X2House:SetHouseName(newName)`; `FUN_3932be40` validates the name (`XlIsValidName`; bad name = error 0x150), applies the name policy, and sends `CSChangeHouseName` (0x59: `tl u16, name str`) (`FUN_397abc80`). `SCHouseOwnerNameChanged`/state updates the unit name. **[R]** (`FUN_39477c30/39477c40`, `components/edit_dialog_handler.alb` proto 6.0).

**Sale.** `GetHouseSaleInfo()` (`FUN_3947a4d0`) returns `canSell`, `onSale`, `canBuy`, `targetName` (only when a buyer is designated), `price`, `gold`, `silver`, `copper`. Its rules are:

* `canSell` = template `is_sellable` **and** player state bit `0x8` (some "allowed to sell" flag) **and** `IsMyHouse` **and** not a castle category. It deliberately does *not* test under-construction; the sale action does that separately.
* `onSale` = `sell_price != 0`.
* `canBuy` = `onSale` **and** (no designated buyer **or** the designated buyer is the local player). **[R]**

`SetHouseForSale(gold, silver, copper, name)` sends `CSSellHouse` (0x5E: `tl u16, money u32, sell_to str`) after checks: owner, not castle, not under construction (0x29C), not already for sale, template sellable, enough appraisal certificates (0x287), not self (0x2A9) (`FUN_3932c640`). The certificate count is `ceil(price / 1,000,000)` copper (100 gold) and the item is 28085 (`FUN_39479a40`; `GetHouseSaleItemInfo` returns `itemType, needCount, hasCount`). `CancelHouseForSale` sends `CSSellHouseCancel` (0x5F: `tl`, `FUN_3932c800`); `BuyHouse` checks not-for-sale (0x28E), owner (0x28F), designated buyer (0x290) and money (0x291), then sends `CSBuyHouse` (0x60: `tl u16, money u32`, `FUN_3932e020`). The S→C answers are `SCHouseSetForSale` (0xC8), `SCHouseResetForSale` (0xC9), `SCHouseSold` (0xCA); the client fires `HOUSE_SET_SELL_SUCCESS` (0x10A), `HOUSE_CANCEL_SELL_SUCCESS` (0x10C), then `HOUSE_INFO_UPDATED`; for a sale it fires `HOUSE_BUY_SUCCESS` (0x10B) to the buyer, `HOUSE_SALE_SUCCESS` (0x109) to the seller, else `HOUSE_INFO_UPDATED`. Failure error ranges raise `HOUSE_SET_SELL_FAIL` (0x106), `HOUSE_BUY_FAIL` (0x107), `HOUSE_CANCEL_SELL_FAIL` (0x108) **[I]** (the code→event map was not isolated); `chat_msg_event.alb` turns each into a localized notice. **[R]** (`FUN_3932b680/790/8a0`; error keys in `ui_texts` 5289–5295, 5139–5156).

**Demolish.** `X2House:Demolish()` does not send a house packet: it casts the 20 s, 20 m skill 11404 at the selected house (`FUN_39478740`; the stock demolish dialog text is `ui_texts` 4154 and `demolish_ok` 4782). Refused when tax is overdue (`undemolishable_house_overdue`, 5285), with a warning text. **[R]**

### 1.6 Decorations and furniture (inside a house)

Using a decoration item on a house starts the client's decoration builder (`FUN_3932c120` from item-task type 6; item→design link `item_housing_decorations`). It has its own cursors `housing_deco_position/rotation/zoom/invalid.dds` (`strings.txt` 39a12568–39a125d0) and, on confirm, sends `CSDecorateHouse` (0x58: `house_tl u16, design u32, x f32, y f32, z f32, quat[4] f32, parent_obj u32 bc, item u64`; server `packets.rs:277`). The server consumes the item, spawns the doodad bonded to the house and answers with `SCDoodadCreated`; the client's housing object then fires `HOUSE_DECO_UPDATED` (0x10E) and `WAIT_REPLY_FROM_SERVER` (0x234) when the changed house is selected (`FUN_39330270`). **[R]** for the packet/event; **[I]** that the builder is the item-task-6 path (the decompile shows the builder, not the packet call).

Rules are in `housing_decorations`: `allow_on_floor`, `allow_on_wall`, `allow_on_ceiling`, `allow_mesh_on_garden`, `allow_pivot_on_garden`, `actability_up` (deco points), `deco_actability_group_id`, `doodad_id`; `housings.deco_limit` (40 for all three farm designs), `absolute_deco_limit` (51), `garden_radius` (4/8/11 m), `housing_deco_limit_id`; `housing_deco_limit_elems` gives per-group caps (Thatched Farmhouse limit 10: 3 storage group 5 and 3 professional-furniture group 1); `deco_actability_groups` names the groups. **[R]** `compact.sqlite3`.

Client error keys (all in `ui_texts`/`strings.txt`): `house_cannot_decorate`, `house_not_decoratable_state` (unfinished/demolishing), `house_too_many_decorations`, `house_cannot_decorate_overlap`, `house_cannot_decorate_out_of_house`, `house_cannot_decorate_out_of_garden`, `house_cannot_decorate_surface`, `house_cannot_decorate_in_house`, `too_far_to_decorate_house`, `no_more_space_to_decorate` ("major furniture over 40"), `housing_actability_deco_limited`, `acatbility_deco_arrang_fail`, `cannot_decorate_house_during_war`, `house_cannot_spawn`. `GetHouseDecoCount()` (`FUN_394796b0`) returns `(placed − builtins, deco_limit − builtins)`; `GetActabilityDecoCount` counts the house's deco groups; `GetDecoActabilityByType(i)` returns `{name, arrangeCount, maxCount}`; `GetDecoActabilityPoint(i)` returns `{name, decoPoint}`; `GetDecoActabilityLimitGroup()` returns the group caps. **[R]**

### 1.7 Farms, crops, planting, growth, harvest

Two farm kinds share the machinery:

* **House farm** — a housing unit with `housings.garden_radius`; Scarecrow Garden 267 (category 16, radius 4 m, one step: 1 action, Lumber 1×, skill 18553) and Pumpkin Scarecrow 89 (category 17, 8 m); Thatched Farmhouse 171 (category 18, 11 m, 10+5+5 actions of Stone/Iron/Lumber packs). Their interaction type is `farm`/`house` (see §1.3). Permission/planting rights are the house's (`FUN_39053360`/`FUN_390533f0` before the seed builder; errors 0x2EC/0x2ED). **[R]** `compact.sqlite3 housings`, `housing_build_steps`, `skill_effects`.
* **Public farm** — sub-zones mapped in `farm_groups`/`common_farms`: farm (max 10), nursery (5), ranch (5), stable (5); `guard_time` 86,400,000 ms (24 h) for every 1.2 common farm. **[R]** DB.

Planting: using a seed/sapling item enters the doodad placement builder (`FUN_393a9540` → `FUN_393a7a10`); it validates range (0x9A), surface/overlap (0x114/0x115/0x116), the farm permission when the item is used on a farm (0x2EC/0x2ED), draws leaf markers at the candidate cells (`guide_leaf_r/g/y.cgf`, `FUN_393a4cc0`; red = invalid, green = valid, yellow = neutral), and on confirm sends `CSCreateDoodad` (0xE6: `id u32, x i64, y i64, z f32, z_rot f32, scale f32, item u64`; server `doodad/packets.rs:353`, client `FUN_393a9340` with random scale for templates that ask for one). **[R]** `FUN_393a9540`, `FUN_393a9340`, `FUN_393a4cc0`.

Growth is server phase progression; the client only renders it. `SCHouseFarm`/`SCDoodadPhaseChanged` aside, the visible data is: `doodad_almighties.growth_time` (rice 10,320,000 ms = 2 h 52 m; potato 600,000 ms; spruce 92,580,000 ms), phase groups per crop (rice: 4366 seedling → 4403 small → 4404 ripe → 4405 loot → 4406 gone), and `doodad_func_growths` scaling (rice growth 557: 1,032,000 ms, 0.5→0.75, next 4403). The client's `FarmFieldModel` (`FUN_3908c060`/`FUN_3908c6f0`) additionally draws a field of crop meshes for a farm-field phase, picking the visual step from the model's own thresholds; it is graphics, not a rule. **[R]** DB + `FUN_3908c6f0`; **[I]** that the field phase comes from a server `DoodadFunc` descriptor.

Crop actions are ordinary doodad func skills: Water Plant 13625 (4366→4403), Uproot 13789 (4403→gone), Harvest 13980 (4404→4405), loot 4405→4406. **Watering is a normal doodad use**, not a window; Bevy's generic doodad actions cover it. **[R]** `doodad_funcs` 4366/4403/4404/4405/4406 + `skills`.

Public-farm window (`commonfarm/info.alb`, `my_farm_info.alb`): `OPEN_COMMON_FARM_INFO` (0x102) is fired by the public-farm info interaction (`FUN_393b8700`, interaction switch case 0x48, which reads a farm type; DB `doodad_func_open_farm_infos.farm_id`), then the Lua asks `X2.GetCommonFarmInfo(type)` (type, name, guard ms), `GetFarmGorupInfo` (type, max, name), `GetFarmGroupDoodadList` (item names), `GetFarmGroups` (`{name,type}` tabs) and `GetRuntimeCommonFarmDoodadsInfo(type)` / `...Count`. The row fields are `itemType, name, leftGrowthTime, leftProtectTime, growUp, location, farmType, vPosX, vPosY` (`FUN_39035620`); times are computed from the doodad's end timestamps/plant time, and `leftProtectTime` is `guard_time − (now − plant_time)`, clamped at 0. `RequestRuntimeCommonFarmDoodadInfo` sends `CSRequestCommonFarmList` (0x10B, empty), but the client throttles it: `FUN_39035560` refuses while `DAT_39aefb98+0xf4` is set (error 0x242), and otherwise only sends when an elapsed-time constant `_DAT_3999d848` has passed, firing `COMMON_FARM_UPDATED` (0x101) from the cached data. It also refuses in instance zones (`cannot_request_commonfarm_info_in_instant_zone`). **[R]** code; the 0x48↔`DoodadFuncOpenFarmInfo` identification is **[I]**. (`FUN_393b8700`, `FUN_394445b0/44630/44730/447c0`, `FUN_39035560`, `FUN_39035620`).

Ownership/theft: the client has no lock-out of crop use. It shows the crop's protect time and `growUp`; the rules (who may harvest during/after guard time, house-farm permission) are enforced server-side. `SCHouseFarm` (0xC3: `name str, total i32, harvestable i32`) is turned into `HOUSE_FARM_MSG` (0xFE) with those three values (`FUN_39326010`); `chat_msg_event.alb` registers that event for a notice. **[R]** code/[I] exact theft policy.

### 1.8 Packet summary

S→C: `SCHouseState` 0xBC, `SCHouseBuildProgress` 0xBD, `SCHousePermissionChanged` 0xBE, `SCHouseBuildPayChanged` 0xBF, `SCHouseDemolished` 0xC0, `SCMyHouse` 0xC1, `SCMyHouseRemoved` 0xC2, `SCHouseFarm` 0xC3, `SCHouseTaxInfo` 0xC4, `SCConstructHouseTax` 0xC5, `SCHousingRecoverToggle` 0xC6, `SCHouseSetForSale` 0xC8, `SCHouseResetForSale` 0xC9, `SCHouseSold` 0xCA, `SCHouseOwnerNameChanged` 0xCB, `SCLoginCharInfoHouse` 0x57, `SCShowCommonFarm` 0x1B0 (`farmType i32, count i32, x i64, y i64, z f32`), `SCResponseCommonFarmList` 0x1B1 (twice `count`, then per crop `farmType u32, template u32, timeLeft u32, funcGroup u32, position, plantTime i64`), `SCIsUnitInFarm` 0x1C9 (`i32`). **[R]** server writers, opcodes in `aaemu-game src/opcodes.rs`.

C→S: `CSConstructHouseTax` 0x56, `CSCreateHouse` 0x57, `CSDecorateHouse` 0x58, `CSChangeHouseName` 0x59, `CSChangeHousePermission` 0x5A, `CSRequestHouseTax` 0x5C, `CSAllowHousingRecover` 0x5D, `CSSellHouse` 0x5E, `CSSellHouseCancel` 0x5F, `CSBuyHouse` 0x60, `CSCreateDoodad` 0xE6, `CSRequestCommonFarmList` 0x10B. **[R]** same.

## 2. Numeric parameters

| Thing | Value | Evidence |
|---|---|---|
| Farmhouse build | 10 stone + 5 iron + 5 lumber actions | `housing_build_steps` 754–756; item packs 17684/17685/17683 |
| Farmhouse step skill | 14575/14641/14574, 25 LP, 10 m, 30 s cast | `skills`, `skill_effects` 8727/8728/8882 |
| Scarecrow build | 1 action, Lumber 8337 ×1, skill 18553, 10 LP, 10 m, 10 s | `housing_build_steps` 838 |
| House tax base | farmhouse 150,000; large garden 100,000; small garden 50,000 copper | `taxations` 10/9/8 |
| Tax certificate | 31891 / 31892 (bound), 10,000 copper each | `items.price`; server `manager.rs:39-40` |
| Appraisal certificate | 28085; need = ceil(price / 1,000,000) | `FUN_39479a40`; `COPPER_PER_APPRAISAL` |
| Demolish | skill 11404, 20 m, 20 s, no LP | `skills` |
| Info skill | 15212, 6 m (11001, 4 m) | `skills` |
| Deco limit | 40 counted, 51 absolute; farmhouse groups 3+3 | `housings`, `housing_deco_limit_elems` |
| Garden radius | 4 / 8 / 11 m (267/89/171) | `housings.garden_radius` |
| Public farm cap | 10 / 5 / 5 / 5 | `farm_groups.count` |
| Guard time | 86,400,000 ms | `common_farms.guard_time` |
| Crop skills | Water 13625 (6 m, 1.5 s, 0.5 s CD), Fertilize 13788, Uproot 13789, Harvest 13980 | `skills` |
| Rice growth | 10,320,000 ms; 4366→4403 growth 1,032,000 ms 0.5→0.75 | `doodad_almighties`, `doodad_func_growths` 557 |
| Packet rate limits | `CSCreateHouse` 5 s (re-placement); `CSRequestCommonFarmList` elapsed constant [?] | `re-placement.md`; `FUN_39035560` |

## 3. Inferred / not found

* [I] Event ids are the registration order in `FUN_39214380`; the anchor `BUILDER_STEP = 0x224` from `re-placement.md` is reproduced exactly (index 548).
* [I] `HOUSE_INTERACTION_START`'s first arg is the unit-type name (`"housing"`); the second is the house category string, which the stock Lua does not read.
* [I] `FUN_3932c120` is the decoration builder (item task 6); the `CSDecorateHouse` writer was not isolated.
* [?] House interaction range (`FUN_39093860` argument), farm-list interval `_DAT_3999d848`, `canSell` player-state bit 0x8, the second tax id's semantics, whether `SCShowCommonFarm` is ever sent, theft policy, and the UI effect of `SCIsUnitInFarm` (`FUN_391e0c80`→`FUN_3902de60`).

## 4. Gaps vs Bevy (ranked)

1. **Decoration/furniture placement is absent.** Original: using a decoration item starts the deco builder (position/rotation/zoom cursors), validates surface/overlap/limits and sends `CSDecorateHouse` 0x58; the server spawns a bonded doodad and the client fires `HOUSE_DECO_UPDATED`. Bevy: `HouseCommand::DecorateHouse` is defined (`aa-net/src/housing.rs:305`) but never sent, and `decoration_design_for_item` (`hud/data/housing.rs:416`) has no caller; there is no deco session or cursors. Evidence `FUN_3932c120`, `FUN_39330270`, strings 39a12568–39a125d0, server `packets.rs:277`. Fix: add item-task-6 detection, a builder with the four cursors, `housing_decorations` surface flags/limits, and send 0x58.
2. **The furniture/deco counters are stubbed.** **Done in `5e52514`.** Original `GetHouseDecoCount` returns `(count, limit)` (placed doodads minus built-ins, `deco_limit` minus built-ins); `GetActabilityDecoCount`, `GetDecoActabilityByType/Point/LimitGroup` return per-group used/max/points, refreshed by `HOUSE_DECO_UPDATED`. Bevy returns `nil` (`hud/house_maintain.rs:210-212`), so `maintain_window.alb` proto 3 hides the furniture line and never fires 0x10E. Evidence `FUN_394796b0`, `FUN_394797c0/79890/a040/a310`, `FUN_39330270`. Fix: count house-parented doodads (excluding `housing_binding_doodads` built-ins), use `housing_deco_limit_elems`/`deco_actability_groups`, fire 0x10E on bond/unbond.
3. **Buy is never enabled.** **Done in `5e52514`.** Original `GetHouseSaleInfo` sets `canBuy = onSale && (no designated buyer || buyer == me)` (`FUN_3947a4d0`); `maintain_window.alb` proto 6 does `buyButton:Enable(info.canBuy)`. Bevy sets only `canSell/onSale/targetName/price/gold/silver/copper` (`hud/house_maintain.rs:129-143`), so `canBuy` is nil and the Buy button stays disabled; it also always sets `targetName` (empty string) where the original omits it. Fix: add `canBuy` with the designated-buyer test; omit `targetName` when none.
4. **Bound tax certificates are not counted.** **Done in `5e52514`.** Original `CountTaxItemInBag` sums two item ids (`FUN_39478cd0`), 31891 and 31892 (`manager.rs:39-40`). Bevy sums only `TAX_ITEM = 31891` (`hud/houses.rs:28,261`), so a player holding bound certificates sees Build disabled and a wrong `have/need`. Fix: sum both ids.
5. **House rename is not installed.** **Done in `5e52514`.** Original `SetHouseName` validates and sends `CSChangeHouseName` 0x59 (`FUN_3932be40`, registered `FUN_39477c30`). Bevy's `X2House` table has no `SetHouseName` (`hud/houses.rs:204-343`); the name is only a logged stub (`aa-ui/src/lua/stubs.rs:1672`), so the maintain window's rename dialog (`maintain_window.alb` `changeNameButton`) sends nothing. Fix: implement it and send `ChangeName`.
6. **Right-click on an unfinished house casts the build skill.** Original interaction (`FUN_393befd0`) only sends `CSRequestHouseTax` and fires `HOUSE_INTERACTION_START`; the material window's OK only hides; building is the G/F dynamic action. Bevy `right_click_build` sends `Target` + `UseSkill(step)` (`hud/houses.rs:551-552`, called at 445-522), so every right click consumes a build action, and it skips `RequestTax`. Fix: open the window and request tax only; leave casting to the action bar.
7. **Buy/cancel-sale failures are silent.** Original raises `HOUSE_BUY_FAIL` 0x107 / `HOUSE_CANCEL_SELL_FAIL` 0x108 for `HOUSE_CANNOT_BUY_*`/`HOUSE_CANNOT_CANCEL_SELL_*`; Bevy maps only `HOUSE_CANNOT_SELL*` to `HOUSE_SET_SELL_FAIL` (`hud/house_maintain.rs:319-321`). Fix: map all three prefixes.
8. **`SCHouseFarm` is decoded but unused.** Original fires `HOUSE_FARM_MSG` 0xFE with `(name,total,harvestable)` (`FUN_39326010`), registered by `chat_msg_event.alb`; Bevy stores `Houses.last_farm` (`aa-net/src/housing.rs:458`) with no consumer. Fix: fire 0xFE.
9. **The public-farm window opens from the wrong trigger.** Original fires `OPEN_COMMON_FARM_INFO` 0x102 from the public-farm info interaction (`FUN_393b8700`, `DoodadFuncOpenFarmInfo` [I]) and only then requests the list, throttled and refused in instance zones (`FUN_39035560`). Bevy fires it only on `SCShowCommonFarm` (`hud/farm.rs:191-209`) and sends `CSRequestCommonFarmList` unthrottled; the AAEmu server sends no 0x1B0, and Bevy has no `DoodadFuncOpenFarmInfo` use. Fix: handle that func (fire 0x102 with `farm_id`), add the throttle and instance-zone refusal, and keep 0x1B0 as a secondary path.
10. **F/G are synthesized, not read from the unit.** Original builds the bar from the received `SCUnitState` skill list (`FUN_393bcb90`→`FUN_3910a950`, `FUN_39442370/42400`); Bevy hardcodes `INFO_SKILL`/`step_skill`/`CRAFT_ICON_SKILL` (`hud/doodad.rs:280-350`, `hud/houses.rs:31-33`). A server that sends a different list (extra house skills, no info skill, another craft skill) gets the wrong keys. Fix: use `unit.skills` first, keep the synthesis as a fallback.
11. **`IsCastle`/`IsSiegePeriod` are hardcoded false.** Original reads the house category (`FUN_398970e0`) and the global siege state; Bevy returns false (`hud/houses.rs:299`, `hud/house_maintain.rs:207`), so castle/guard-tower window variants and sale restrictions are wrong. Fix: derive castle from `housings.category_id` and track the received siege state.
12. **Smaller parity items.** `canSell` omits the original player-state bit and adds an under-construction test (`hud/house_maintain.rs:136` vs `FUN_3947a4d0`); `GetTaxations`/`GetTaxItem`, `GetGuardTowerStepInfo`, `GetCurrencyForTax/HouseSale` and the build-pay getters are not installed; `SCHouseBuildPayChanged` has no UI.

## 5. Open questions

* Does the retail client cast the build-step skill on interaction or only on G? The code says only the window opens; a capture settles it.
* House interaction range and farm-list request interval.
* Second tax certificate id (31892) and whether `CountTaxItemForTax` uses the item price or a hardcoded divisor.
* A decoration capture for the validation order, cursor states and the exact `CSDecorateHouse` quaternion/parent semantics.
* How the 1.2 server fills a house unit's skill list (the F/G source) and whether the owner-only craft skill is per-viewer.
* Whether `SCShowCommonFarm` is ever sent; if not, the information interaction is the only window trigger.

## 6. Live oracle measurements

None: no original client was run. Bevy's retail captures in `re-placement.md` and `ingame/placement.rs` (2026-10-21) cover the seed ghost/leaf ring and placement clicks only, not construction, maintain, decoration or farm windows.
