# Client-side world-rule numbers (labor, craft, enchant, specialty, repair, fall)

These are the places where the 1.2 client computes a number that mirrors a world/server rule, so a
Bevy client can show the same value and the server can share the maths. VAs in `x2game.dll`
(base `0x38ff0000`). `[R]` = read in code, `[I]` = inferred, `[?]` = not isolated.

## 1. Crafting window (`FUN_39461860`, 1272 B)

Called by the crafting-window Lua binding (no static caller; it is registered). Builds a table
with **[R]**:

| Key | Source |
|---|---|
| `craft_type`, `ac_type`, `title`, `tool_type`, `skill_type`, `cast_delay`, `recommend_level` | craft recipe desc |
| `consume_lp` | `FUN_396f6390(recipe+0x24)` -> actability/skill desc `+0x78` |
| `needed_lp` | `FUN_397a9420(item_type, consume_lp, actability+bonus, flags)` |
| `laborpower_satisfied` | `needed_lp <= local_player.labor_power` |
| `actability_satisfied` | current actability points + house bonus >= recipe requirement |
| `consume_lp` display | skill desc `+0x78` |

The actability bonus is read with `FUN_390b3a80(actability_group.unit_attr_id, 0)` (client stat
getter) when the actability group desc is present. The needed-LP helper is
`FUN_397a9420` -> `FUN_397a8dd0` (239 B / hidden-arg helper). The exact rounding of
`FUN_397a9420` was not recovered from the decompiler; it returns an int in EAX and takes the
recipe type, consume_lp, actability total and a flag **[?]**.

Server: crafting uses the skill's `consume_labor_power` (`systems/economy/craft.rs:119,446`) and
`inventory::change_labor`; the actability time multiplier is in `inventory::actability_time_multiplier`
(`craft.rs:459`). The client `needed_lp` display formula is **not** ported server-side **[R]**.

## 2. Crafting product list and success display (`FUN_39462a70`, 886 B)

Builds the product list from the recipe's products **[R]**:

| Key | Source |
|---|---|
| `itemType`, `item_name` | product item desc |
| `amount` | product count |
| `success_rate` | product desc `+0x10` (`piVar1[4]`); the DB column is `craft_products.rate` (displayed as a percentage) |
| `useGrade`, `productGrade` | product flags/grade |

Server: no `success_rate` consumer was found in `craft.rs` (grep of `success` in that file is
empty); the server currently does not display/roll this client value **[R]**. The client value is
a pure display of the `craft_products.rate` row **[I]**.

## 3. Socketing and grade-enchant chances

### 3.1 Socket chances

`FUN_395b2d60` loads `item_socket_chances` (one int per socket count, ordered by
`num_sockets ASC`) into a vector; the consumer of that vector was **not isolated** in 1.2 (the
item/socket tooltips show `socketInfo`/`maxSocket` but no percentage) **[?]**. The server
implements the roll: `data/items.rs:1041` (`socket_chance(n)`) and
`systems/economy/item_effects.rs:719` (`gem_chance = socket_chance(gem_count)`).

### 3.2 Grade-enchant display (`GetRatioInfos`)

The 1.2 grade-enchant window calls `X2ItemEnchant:GetRatioInfos()` (`399a022c`). The binding is
registered in `FUN_39065f90`; the handler is `FUN_39065d70` (288 B) **[R]**.

* `FUN_39065d70` -> `FUN_390450b0` (578 B), which reads the item-grade success/break/downgrade
  flags from the item grade desc (`FUN_3961e850(item)->+0x28/+0x10/+0x20`) and, if a support item
  is present, adds the per-support variance ratios via `FUN_39044120` **[R]**.
* Data: `item_grades.grade_enchant_success_ratio`, `..._great_success_ratio`,
  `..._break_ratio`, `..._downgrade_ratio`; `item_grade_enchanting_supports.add_*_ratio/mul`
  (`FUN_396cd310`); `physical_enchant_abilities.success_ratio` (`FUN_396ce260`).
* The actual percentage strings (`successPossibility`, `destroyPossibility`,
  `downGradePossibility`) are computed in the Lua
  `x2ui/inventory/enchant/grade_enchant_view.alb` (`GetRatioInfos` result, `SetRatioText`), using
  `success`, `success_var_ratio`, `break`, `break_var_ratio`, `downgrade`,
  `downgrade_var_ratio` **[R]**.

Server: `systems/economy/item_effects.rs:535-565` implements the same chance composition
(`enchant_success_ratio` + support `add_success_ratio/mul`, same for great-success/break/downgrade)
and rolls with `next_i32(0,10000)`; `data/items.rs:1202,1687` load the tables. This is a genuine
share candidate.

### 3.3 Physical (NPC) enchant

`physical_enchant_abilities` is loaded by `FUN_396ce260` (ids `+0x44c0`, rows `+0x44c4` stride
0x14: armor flag, enchant level, min friendship, npc id, success ratio). The consumer is the NPC
enchant UI **[?]**. Server has no equivalent (`grep physical_enchant_abilities` in Rust is empty).

## 4. Trade-pack / specialty price

Client handlers in the X2Store binding `FUN_394afe40` (2167 B) **[R]**:

| Lua method | Handler | What it does |
|---|---|---|
| `GetSpecialtyRatio` | `FUN_394aeb80` (121 B) | returns the cached server ratio `DAT_3a5ba4e0` (`FUN_394fb2f0`), set by the `SCSpecialtyRatio` packet handler `FUN_394fb610` |
| `GetSpecialtyRatioBetween(from,to)` | `FUN_394aec00` (139 B) | calls `FUN_394fb6b0` (throttled query; global `DAT_3a5ba4c4`, `DAT_39ac9b7c`) -> `FUN_397aceb0` |
| `GetSpecialtyRatioRefund(refund)` | `FUN_394ae8c0` (696 B) | resolves the player's zone/backpack, computes the base price with `FUN_395ac920`/`FUN_398a9bf0`/`FUN_398a9cb0` and compares refund eligibility |
| `SellBackPackGoods` | registered in the same function | sends the sale request |

Base-price helper `FUN_398a9bf0` (189 B) multiplies the specialty `profit * ratio` (f32, with
rounding) and adds the item refund; `FUN_398a9cb0` divides by the specialty ratio divisor
(`FUN_395720e0(0x16)`). Data: `specialties` (row/col zone group, profit, ratio, vendor_exist),
`specialty_bundle_items` (profit, ratio), `world_spec_configs` (specialty_mod,
specialty_adjust_ratio), `specialty_npcs`.

Server: `systems/economy/specialty.rs` implements the ratio (60-160), the sale
(`sell_specialty`, line 295) and the base price `floor(profit * ratio/1000) + refund`
(`get_base_price_for_specialty`, ~line 275), plus `SCSpecialtyRatioPacket` (line 574). The client's
`GetSpecialtyRatio` is only the last packet value, so the two are intentionally coupled by the
protocol, not recomputed identically on both sides.

## 5. Repair cost / durability

Client: `FUN_397ce6b0` (945 B) computes the current/max durability from the item grade
(`item_grades.durability_value`, `FUN_39572460`), the item level/stack and the damaged state; it
logs `Invalid durability [%f] for item[%u], grade[%u]` when the result is outside 0..255 **[R]**.
`FUN_3941fc20` exposes `durability`/`maxDurability` in the item tooltip. `FUN_3947f670`
(`X2ItemEnchant:Info`) and `FUN_39471060` (`X2Item:Info`) expose repair cost/durability fields in
the item tables.

Server: `models/item.rs:777-815` (`try_max_durability`, `max_durability`, `repair_cost`) uses the
same `item_grades` durability and `config.durability_repair_cost_factor`, plus
`refund_multiplier`; `systems/services/repair.rs`/inventory handle the actual repair **[R]**.

## 6. Experience recovery (labor/money)

Client: `FUN_3972fa10` loads `recover_exp_effects` (`need_labor_power`, `need_money`,
`need_priest`, `penaltied`) into a bitmask table. The recovery UI uses formula id 7
(`LaborPowerForRecoverExp`) and id 6 (`MoneyForRecoverExp`) from the generic formula table
(`FUN_39566eb0(ctx, 6/7)` / `FUN_39567030`/`FUN_395670a0`) **[R]/[I]**.

Server: `systems/skills/data/effects.rs:899` loads `recover_exp_effects`;
`systems/skills/validation.rs:86` and `systems/economy/bridges.rs:679-740` implement the
labor/money/priest checks and the recovery. The cost is `FUN_398a75f0`-style level scaling on the
client and `needed` in the server; compare before sharing **[I]**.

## 7. Fall damage

Already mapped and ported: see
[`../port-movement-extra/MAP.md`](../port-movement-extra/MAP.md) §2.6 and `swim-fall.md`.
`CActor::UpdateFallDamage` `FUN_39116f00` computes `impact = sqrt(40 * drop)` (constant
`0x3999f668 = 40.0`), clamps it to the current fall speed, and reports it in the movement packet;
the client does not apply damage. The server applies it in `systems/skills/combat.rs` (the docs
cite `combat.rs:1562-1610`) **[R]**. Do not re-map it here.

## 8. Things that are **not** in 1.2

* **Gearscore.** There is no `gearScore`, `GetGearScore`, `bareGearScore` or `totalGearScore`
  string in the 1.2 `x2game.dll`; those are 10.x-only. The 1.2 character window shows the
  UnitInfo fields, not a gear score **[R]**. A 10.x `GetGearScore` exists but must not be used as
  1.2 behaviour.
* **The 1.2 `formulas` table ends at id 27.** Server `FormulaKind` ids 28+ (gear score,
  conquest, sockets, etc.) are later-version additions **[R]**.

## 9. Server share candidates (summary)

| Rule | Client | Server | Share? |
|---|---|---|---|
| expression engine | f32 parser/evaluator (§`formulas.md`) | Jace-compatible f64 engine (`data/formulas.rs`) | **No** for bit-exact; share only the DB text/AST loader design |
| unit formula kinds / variables | §`formulas.md` | `data/formulas.rs:148-200` | yes, enum names and semantics |
| stat aggregation | `398a0bc0` + modifiers | `models/stats.rs`, `systems/skills/stats.rs` | yes, after the client clamp table is added |
| item wearable/holdable formulas | `39568560/610` | `models/item.rs:892-1001` | **yes** (identical DB text, same params) |
| item repair/durability | `397ce6b0` | `models/item.rs:789-815` | yes |
| grade enchant chance | `390450b0` + Lua | `item_effects.rs:535-565` | yes |
| socket chance | loaded only | `item_effects.rs:719` | yes (server has the rule) |
| specialty price | `394aeb80`/`394ae8c0` + Lua | `economy/specialty.rs` | yes (protocol ratio) |
| craft labor/success | `39461860`/`39462a70` | partial (`craft.rs`) | client display not ported server-side |
| fall damage | link to movement map | `combat.rs` | already shared |
| gear score | absent | absent in 1.2 | n/a |
