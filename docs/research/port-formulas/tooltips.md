# Tooltip and item-formula code (1.2 client)

VAs in `x2game.dll` (base `0x38ff0000`). The Lua tooltip text is authored in the client's
`x2ui` scripts; the C++ code below substitutes numeric placeholders and builds the tables the Lua
reads. `[R]` = read in code, `[I]` = inferred.

## 1. Three separate tooltip producers

| Producer | Binding / caller | Purpose |
|---|---|---|
| `FUN_3941ed20` (931 B) | called from the unit/buff tooltip binding | **buff** tooltip placeholders (a buff instance object) |
| `FUN_3941f0d0` (2893 B) | called from the skill tooltip binding | **skill** tooltip placeholders (a skill object, or a buff resolved from a skill) |
| `FUN_3941fc20` / `39420310` / `394206d0` | `FUN_39421e20` (item info table) | **item** tooltip fields (armor, damage, durability, DPS, magic resistance) |
| `FUN_394ace10` / `3941e750` | X2Skill `Info` table | **skill info** table (`manaCost`, `castingTime`, `cooldownTime`, ranges, ...) |

`FUN_39421e20` (5876 B) is the large item-info table builder; it calls the three item tooltip
producers for the class-specific fields and adds `socketInfo`, `maxSocket`, `gradeEnchantable`,
`livingPointPrice`, `honorPrice`, `refuel`/charge fields, etc. **[R]**. `FUN_394ace10` (X2Skill
`Info`) calls `FUN_3941e750` (638 B), which fills the skill info values from the skill desc **[R]**.

## 2. Buff tooltip (`FUN_3941ed20`)

The object layout used here: `param_1+4` = buff desc, `+8` = level/stack, `+0xc` = a second value
**[R]**. Placeholder -> function:

| Placeholder | Function | Value |
|---|---|---|
| `duration` | `FUN_398a75f0` + `FUN_3941ea10` | `desc+0xfc * level + desc+0x68` **(ms)** then formatted |
| `tick` | `FUN_39419cd0` | tick count |
| `min_damage` / `max_damage` | `3941cc00` / `3941cca0` | min/max per tick from `FUN_3941b030` |
| `total_min_damage` / `total_max_damage` | `3941ca80` / `3941cb00` | totals (not divided by `desc+0x1ac`/duration) |
| `min_heal` / `max_heal` / `avg_heal` | `3941ced0` / `3941cf70` / `3941d010` | per-tick heal from `FUN_3941a460` |
| `total_min_heal` / `total_max_heal` | `3941cd40` / `3941cdc0` | totals |
| `min_mana_burn` / `max_mana_burn` | `3941d220` / `3941d2b0` | mana burn per tick |
| `total_min_mana_burn` / `total_max_mana_burn` | `3941d0c0` / `3941d130` | totals |
| `avg_restore_mana` / `total_avg_restore_mana` | `3941d4b0` / `3941d3e0` | mana restore |
| `ua_*` | `3941a700` | unit attribute by name |

The per-tick damage/heal values divide by `desc+0x1ac` (`FUN_3941cc00`/`ced0`: `iVar3 =
desc+0x68 / desc+0x1ac`; the total variants skip the division) **[R]**.

## 3. Skill tooltip (`FUN_3941f0d0`)

The skill object layout used: `param_1+4` = skill desc/caster, `+8` = skill level, `+0xc` = a
second level/flag **[R]**. Prefix handling:

* `buff_tooltip<N>` -> `FUN_3941a920` (renders the referenced buff tooltip text)
* `buff_duration<N>` / `buff_tick` -> `FUN_398a75f0`/`3941ea10`, `39419cd0`
* `buff_min_damage`, `buff_max_damage`, `buff_total_min_damage`, `buff_total_max_damage`
  -> `3941cc00`, `3941cca0`, `3941ca80`, `3941cb00` on the **buff** desc resolved by
  `FUN_3941bb30`
* `buff_min_heal`, `buff_max_heal`, `buff_avg_heal`, `buff_total_min_heal`,
  `buff_total_max_heal` -> `3941ced0`, `3941cf70`, `3941d010`, `3941cd40`, `3941cdc0`
* `buff_*_mana_burn` / `buff_*_restore_mana` -> the `3941d0c0..d4b0` family
* `buff_ua_` -> `3941a700`

Plain placeholders (skill effect):

| Placeholder | Function | Meaning |
|---|---|---|
| `level_damage` | `3941c2a0` -> `FUN_3941a160` | average level-scaled damage |
| `damage_multiplier` | `3941c2e0` -> `FUN_3941a240` | per-hit damage after stat/skill modifiers |
| `min_damage` / `max_damage` / `avg_damage` | `3941c330` / `3941c390` / `3941c3f0` | `FUN_3941b030` min/max/avg |
| `level_heal` | `3941c450` -> `FUN_3941a300` | average level-scaled heal |
| `heal_multiplier` | `3941c490` -> `FUN_3941a3c0` | per-hit heal |
| `min_heal` / `max_heal` / `avg_heal` | `3941c4e0` / `3941c540` / `3941c5a0` | `FUN_3941a460` |
| `detail_melee_damage` | `3941c600` | melee detail text (`3941a160` + `3941a240`) |
| `detail_ranged_damage` | `3941c720` | ranged detail |
| `detail_spell_damage` | `3941c840` | spell detail |
| `detail_heal_damage` | `3941c960` | heal detail (`3941a300` + `3941a3c0`) |
| `return_point` | `3941ec10` [?] | return point formatting |
| `area_radius` | `3941ec10` [?] | area radius |
| `charge_multiplier` | one of `3941cXXX` [?] | charge multiplier |

Each value is formatted through `FUN_392e3420` (`"number_value_format"`) and
`XlFormatString` **[R]**.

## 4. Shared effect computations

### 4.1 `FUN_3941b030` — damage range of one effect (1015 B)

Inputs: `(caster_or_0, skill_or_0, effect_desc, level, item_context, *min, *max)`. It:

1. adds the effect's flat fields (`effect+0x3c/0x3e/0x40/0x42` family, gated by bytes
   `+0x92..+0x97`) to min/max **[R]**.
2. If `effect+0x93` (level-scaled) and a caster is present, computes the level scale
   `FUN_398a7710` and calls `FUN_398a7440` (which calls `FUN_398a7380`) to add the scaled
   min/max.
3. For an item context, adds the weapon DPS from the current main/off/ranged item:
   `FUN_390b3a80(0x60/0x61/0x62, 0)` -> multiply by the effect's weapon coefficient
   (`effect+0xc`), plus `effect+0xd` times the weapon damage. It also uses the item's holdable
   damage (`FUN_397cd4d0`/`FUN_397ced30`/`FUN_397cc400`) for the min/max split **[R]**.
4. Applies a level/stat multiplier from `FUN_398a7780` and the skill modifier
   `FUN_39354370(param_1, 10, value)` (SkillAttribute `Damage`).
5. If the effect has a `+0x25` flag, adds the mainhand weapon's damage; `+0x95`/`+0x97` add
   offhand/ranged contributions **[R]**.
6. Finally applies the effect's multiplier `effect+0x17` to both bounds and the
   `FUN_39354370`/`FUN_39354450` modifiers.

The exact arithmetic is in the decompile at `0x3941b030..0x3941b426`; the function is a good
oracle leaf because it only reads its arguments and the global stat getter.

### 4.2 `FUN_3941a460` — heal range (393 B) **[R]**

Same shape for heals: flat fields, level scaling `FUN_398a7490`, the caster's heal stat
`FUN_390b3a80(0x78)` (attribute 120 `HealMul`), skill modifier kind `0xc` (`Heal`), and the
effect multiplier.

### 4.3 `FUN_3941a160` / `FUN_3941a300` — level damage / heal

`FUN_3941a160` averages the level-scaled effect min/max, multiplies by the caster's melee/ranged/
spell DPS-increase attribute (`0x33/0x34/0x35`, selected by `effect+0x2c`), then applies skill
modifier kind `10` **[R]**. `FUN_3941a300` does the same for heals with attribute `0x78` and
modifier kind `0xc` **[R]**.

### 4.4 `FUN_3941a700` — `ua_<attribute>` tooltip

Resolves the name to a `UnitAttribute` id via `FUN_3956e310`, formats the value with
`FUN_3989fde0` (the same modifier walker as the stat engine), and special-cases percentage-like
attributes (`10,0x33..0x35,0x38,0x4d,0x4e,0x51,0x52,0x53,0x56,0x58,0x9a,0xb3,0xb4,0xb9` etc.)
through `FUN_39419da0`/`39419e00` **[R]**.

## 5. Skill info values (`FUN_3941e750`, `FUN_39420b30`)

`FUN_3941e750` fills **[R]**:

| Output | Computation |
|---|---|
| `manaCost` | `FUN_39354340(context, desc, 0)` |
| `castingTime` | `FUN_393543a0(context, desc)` |
| `cooldownTime` | `FUN_39354340(context, desc, 1)` |
| `minRange`/`maxRange` | `FUN_390b4110(desc, actor, &tuning, &min, &max)` |
| `channeling` | `FUN_39354340(context, desc, 0xb)` (in `FUN_39420b30`) |
| `targetAreaRadius` | `FUN_39354340(context, desc, 3)` |
| `skillPoints` | `desc+0x144` |
| `nextLearnLevel` / `upgrade_cost` | `FUN_39898030` / `FUN_39899060` |

`FUN_39354340`/`FUN_393543a0` call `FUN_398a01b0`, which selects the desc field by SkillAttribute
**[R]**:

| kind | field | scaling |
|---|---|---|
| 0 ManaCost | `desc+0xf0` | `FUN_398a7580` uses `desc+0xf4` and unit formula kinds 0xc/0xd (`level_dps`, `level_mana`) |
| 1 Cooldown | `desc+0x88` | `FUN_3989f900` modifiers |
| 2 Range | `desc+0xfc` | modifiers |
| 3 AreaRadius | `desc+0x188` | modifiers |
| 4 CastTime | `desc+0x48` | x `(1 + level_scale)` |
| 0xb Channeling | `desc+0x6c` | modifiers |

The final value is passed through `FUN_3989f900` (flat + percent modifiers from the caster/buffs,
truncated, clamped to at least 1 for some callers) **[R]**. This is the client tooltip rule; the
server's `ManaCost` effect port (`systems/skills/effects/special/combat.rs:212-245`) uses the
effect's `value1 + value2/100*scale` instead of the skill desc fields. The two agree for the 62
skills noted in that file; they are not guaranteed identical for every skill.

## 6. Item tooltip (`FUN_3941fc20`, `FUN_39420310`, `FUN_394206d0`)

The item tooltip producers evaluate the **item formulas** loaded into the global formula context.
The DB field names come from `holdables` and the server's item model; the exact
vector-index -> field association is inferred from these callers and the server code **[I]**:

* `FUN_397cc440(item, slot, ...)` -> `FUN_39568560(ctx, 0, holdable, grade, ...)`: holdable
  `formula_dps`, multiplied by the item-grade `var_holdable_dps` coefficient, f32, optional temper
  override; used for `DPS` / `extraDPS`.
* `FUN_397cc4c0` -> `FUN_39568560(ctx, 2, ...)`: holdable `formula_armor` (weapon armor).
* `FUN_397cd4f0` -> `FUN_39568560(ctx, 0, ...)`: min/max damage split from `item+0x30/0x38/0x7c`
  and the weapon damage; used for `minDamage`/`maxDamage`.
* `FUN_397cc6d0` -> `FUN_39568610(ctx, 0, ...)`: **wearable formula kind 0** (max base armor) times
  `wearables.armor_bp`; used for `armor`.
* `FUN_397cc750` -> `FUN_39568610(ctx, 1, ...)`: wearable formula kind 1 (max base magic
  resistance) times `wearables.magic_resistance_bp`; used for `magicResistance`.
* `FUN_397ce6b0(item, grade)` -> durability (uses the item-grade durability value and the item's
  stack/level; logs `"Invalid durability ..."` on out-of-range) **[R]**.
* `FUN_397cf870(item)` -> item desc; `FUN_397ced30` sets the holdable desc used by the damage
  split.

The item tooltip fields and their source (from `FUN_3941fc20` / `39420310` / `394206d0`) **[R]**:

| Key | Source |
|---|---|
| `slotType`, `slotTypeNum`, `repairable`, `baseEquipment`, `useAsStat`, `useAsSkin` | item desc/flags |
| `durability`, `maxDurability` | `FUN_397ce6b0` / item grade |
| `armor`, `extraArmor` | wearable formula 0 / holdable formula 2 |
| `DPS`, `extraDPS` | holdable formula 0 |
| `minDamage`, `maxDamage` | `FUN_397cd4f0` |
| `magicResistance` | wearable formula 1 |
| `armorType`, `slotType` (armor variant) | item desc |
| `resultbuffkind`, `ability`, `skillLevel`, `learnLevel`, `levelStep`, `casting`, `channeling`, `cooldown`, `targetAreaRadius` (skill/equip info variant) | `FUN_39420b30` / `3941e750` |

## 7. Buff/skill duration formatting

`FUN_3941ea10` formats a millisecond duration into the localized `D H:M:S` string (it uses the
`number_value_format`/`FormatDurationTimeInDesc` Lua-side helper for some paths) **[R]**. The raw
duration for a buff is `desc+0xfc * level + desc+0x68` (`FUN_398a75f0`) **[R]**; the skill duration
for a tooltip comes from the effect/skill desc, not a formula id.

## 8. Server status (summary; details in `server.md`)

* Skill mana/cast/cooldown/range: server has the data fields and the `ManaCost` effect rule, but no
  tooltip producer. The `plot_mana_cost` port is `systems/skills/effects/special/combat.rs:212`.
* Item armor/weapon formulas: **implemented** in `models/item.rs:892-1001` (wearable and all four
  holdable formulas, grade coefficients, temper).
* Durability/repair: **implemented** in `models/item.rs:789-815`.
* Gearscore: **not in 1.2** (no `gearScore`/`GetGearScore` strings; those appear only in 10.x).
* Buff durations/tick counts: server has `buffs.duration/tick`; no tooltip producer.
* Unit attribute/character window: server has the stat formulas, but not the UnitInfo presentation
  table (client-only).
