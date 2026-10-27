# Server-side status (`C:/Users/stsiakal/AAEmu/rust/crates/aaemu-game`)

The server is the AAEmu Rust port (`aaemu-game`), not the 1.2 client. It shares the DB tables but
not the client expression engine. This file records what can be reused for the client formulas /
stats port and what differs.

## 1. Expression engine

| Aspect | 1.2 client | Server | Reuse |
|---|---|---|---|
| implementation | `x2game.dll` parser `398b0b20` + 24 node evaluators (`0x3a5bd4c0`), hard-coded static mirrors | Jace 1.0.0-compatible `Expression` in `data/formulas.rs:1-120` (f64, comparison/logical/`%`, `round`, `if`, `avg`, `median`, ...) | **No bit-exact reuse**, different language and numeric type |
| number type | f32 nodes, x87 intermediates, final f32 | f64 (`f64` throughout) | client port must be f32/exact |
| builtins | `min,max,clamp,if_zero,if_positive,if_negative,floor,log,sqrt,abs` | Jace's functions + the four `FormulaManager` functions; `if_zero/positive/negative` registered | names overlap; semantics differ |
| DB text | same tables | `FormulaManager::load` (`data/formulas.rs:1500-1610`) | text/rows shareable |
| enums | kind 1..46 / owner 0..6 / variable kinds 1..5; stat index = `UnitAttribute` 0..186 | `UnitFormulaKind` (`:148`), `FormulaOwnerType` (`:136`), `UnitFormulaVariableType` (`:196`), `WearableFormulaType` (`:206`), `FormulaKind` (`:216`), `UnitAttribute` (`models/unit.rs:110`) | **yes**, but fix `UnitFormulaVariableType` kind 1: client kind 1 is **CharRace**, not `Unknown` (`formulas.md` §3.5) |
| getters | `398ac290`, `39567110`, `398af1e0` | `get_unit_formula` (`:1622`), `get_wearable_formula` (`:1655`), `get_formula_by_kind` (`:1670`) | API shape reusable |

Recommendation: add a new client-expression module (f32 AST + evaluator) to the shared data crate;
keep the server's Jace engine for the server, or replace the server's engine only after the client
goldens exist. Do not try to express the client's `clamp`/`if_*`/`^` semantics through Jace
without checking the f32 rounding.

## 2. Character / unit stats

### 2.1 What exists

| Client area | Server location | Notes |
|---|---|---|
| base attributes by level | `models/stats.rs:24-90` (Str..Fai, MaxHp/MaxMp), `systems/skills/stats.rs:296-380` (with gear, bonuses, facets) | server uses `unit_formulas` and `calculate_with_bonuses`; no unit_attribute_limits clamp |
| NPC stats | `systems/npcs/npc/stats.rs:130-270` | same formulas, NPC owner; `derived_int` etc. |
| vehicle/mate stats | `systems/vehicles/stats.rs:70-135` | owner-specific formulas |
| attribute bonuses | `models/unit.rs` (`UnitAttribute`, `Bonus`, `calculate_with_bonuses`; tests at `:930..`) | server adds flat/percent bonuses; client aggregates the same way |
| armor/magic resistance | `models/item.rs:892-932` (wearable formulas), `skills/stats.rs:611-617` | matches client `397cc6d0`/`397cc750` + DB text |
| weapon DPS / damage | `models/item.rs:939-1001` (all four holdable formulas), `skills/stats.rs:500-540` | matches client `397cc440`/`397cc4c0`/`397cd4f0` |
| GCD | `systems/skills` cooldown port; `globals` attr 0x4a | client `FUN_39899f30`/`df0` already ported bit-exact (`port-combat/MAP.md` §4.2) |
| stat display UnitInfo | **not implemented** (client-only presentation) | port into aa-ui/aa-client |
| `unit_attribute_limits` clamp | **not implemented** | add to the shared stat port |
| static formula mirrors (`398af1e0`, `398ac290`) | **not implemented** | only exercised by the `expr_mode` verifier; keep as test oracles, not behaviour |

### 2.2 What is missing / differs

* The server's `FormulaManager` is a data loader + Jace engine; it does not implement the client's
  `expr_mode` verification. (The client's production path is always dynamic; `expr_mode=static`
  only runs the static-vs-dynamic check.)
* The client aggregates modifiers with a specific f32 order and truncates; the server's
  `calculate_with_bonuses` is integer/f64 and has its own order. Compare with the client goldens
  before sharing the final stat numbers.
* The client has the `unit_attribute_limits` table and clamps every stat getter; the server has no
  consumer. This can change visible values (e.g. GCD -800..4000).
* The client's `UnitInfo`/`UnitModifierInfo` presentation arithmetic (f32, percentages, the DPS
  `0.001`, persistent regen `0.2`) is client-only.

## 3. Tooltips / UI

| Area | Server | Reuse |
|---|---|---|
| skill mana cost | `systems/skills/effects/special/combat.rs:212-245` (`plot_mana_cost`), tests `:495` | the **effect charge** rule; the client tooltip uses skill desc `mana_cost`/`mana_level_md` via `39354340`/`398a01b0`. They agree for the 62 skills noted in the file; compare before sharing |
| skill damage/heal tooltip | no equivalent | port from client `3941b030`/`3941a460`/`3941a1xx` |
| item tooltip stats | item `dps`/`mdps`/`hdps`/`armor`/`base_magic_resistance` (`models/item.rs:892-1001`) | **share** |
| item durability/repair | `models/item.rs:777-815` | **share** |
| item grade/enchant data | `data/items.rs:376`, `:569`, `:1202`, `:1687` | share |
| socket chance | `data/items.rs:1041`, `item_effects.rs:719` | share |
| grade enchant chance | `item_effects.rs:535-565`, `charm_chance` at `:600` | **share** (client `390450b0` + Lua display) |
| physical enchant | none | new |
| gearscore | absent in 1.2 / server has no 1.2 gearscore | n/a |
| `ua_*` tooltip values | no equivalent | port client `3941a700` |

## 4. Economy / world rules

| Rule | Server | Client |
|---|---|---|
| labor cost for crafting | `craft.rs:446,523` uses the skill's `consume_labor_power`; `inventory.rs:1637` `change_labor` | `39461860` `needed_lp`/`consume_lp`; actability discount; client display arithmetic not ported |
| craft success rate | not used (`grep success craft.rs` empty) | `39462a70` displays `success_rate` from the product desc |
| exp by labor | `inventory.rs:1687`, `skills/progress.rs:287` (`ExpByLaborPower`) | generic formula id 19; same DB text |
| exp recovery cost | `skills/data/effects.rs:899` (`recover_exp_effects`), `economy/bridges.rs:679-740` | `3972fa10` + formula ids 6/7 |
| specialty price | `economy/specialty.rs:240` base price, `:295` sale, `:574` `SCSpecialtyRatioPacket` | `394ae8c0`/`394aeb80` + Lua; client uses the server ratio |
| repair cost | `models/item.rs:800` | `397ce6b0` + tooltip |
| fall damage | `systems/skills/combat.rs` (client docs cite `:1562-1610`) | movement map `port-movement-extra` (`FUN_39116f00`) |

## 5. Share plan

1. Move the DB loaders/enums (and a new f32 expression engine) into a shared crate used by both
   client and server, or copy `data/formulas.rs`'s loader and add `Expression::evaluate_f32`.
2. Add `unit_attribute_limits` to the shared stat module.
3. Use the server's item formulas (`models/item.rs`) as the reference for the client item tooltip;
   they already read the same DB text.
4. Reuse the enchant/socket/specialty rules from `item_effects.rs`/`specialty.rs`; add a
   bit-exact `390450b0` chance composer if the display must match the client.
5. Keep the client's `UnitInfo` presentation in aa-ui/aa-client, fed by the shared stat module.
