# Struct layouts for the formula/stat port

VAs are `x2game.dll` (base `0x38ff0000`). Offsets were obtained from the decompile and confirmed
by disassembly where the decompiler loses the register-returning conventions. `[R]` = read in
code, `[I]` = inferred, `[?]` = open.

## 0. Two pointer bases: `unit` and `actor`

The 1.2 code mixes two pointers into the same allocation:

* `unit` = the pointer returned by the unit registry (`FUN_39360020`) and used by the target,
  cooldown, unit-info and combat code.
* `actor` = `unit + 8`, the base used by the attribute/formula code and by the movement/actor
  modules.

The 8-byte difference is proved by the writers/callers:

* `FUN_390b7940` is a method on `unit` (`ecx`), writes the target id at `unit+0x1aa0` and calls the
  stat getter with `lea ecx,[unit+0x1aa8]` **[R]** (`0x390b7980`, `0x390b79e0`).
* `FUN_394270a0` (target setter) atomically reads/writes `unit+0x1aa0` (disassembly
  `lea esi,[edi+0x1aa0]` at `0x394270f8`) **[R]**.
* `FUN_394bdc10` (UnitInfo) resolves the unit, then `add edi,8` before calling `FUN_398aa290`
  (disassembly at `0x394bdd6e`) **[R]**, and `docs/aa-ui.md` documents the same.
* The stat manager starts at `unit+0x1aa8`. Its size is exactly 0x11a0, ending at `unit+0x2c48`,
  which is the documented cooldown manager start (`port-combat/MAP.md` §4) **[R]**.
* Therefore `actor+0x1aa0 = unit+0x1aa8` = the stat manager, which is why the stat code
  (`FUN_398af1e0`, `FUN_39567110`) uses `actor+0x1aa0`.

Port implication: use one `Unit` struct with `#[repr(C)]`-like explicit offsets, or (better) name
`unit` and `actor` as two views and never mix them. The fields below use the **actor** base unless
the row says `unit`.

## 1. `UnitAttribute` manager (`unit+0x1aa8`, `actor+0x1aa0`), size 0x11a0

This is the inline per-unit cache/modifier store. `FUN_398a0bc0` is its getter. `[R]`

| Offset | Type | Meaning |
|---|---|---|
| `+0x000` | `Actor*` | back-pointer to `actor` (the getter reads `[this]` as the actor) |
| `+0x008` | i32 | version/stamp, compared with the per-entry stamp |
| `+0x00c + i*8` | i32 | cached value, XOR-obfuscated with `+0x119c` |
| `+0x010 + i*8` | i32 | stamp (`0` = never cached; otherwise compare with `+0x008`) |
| `+0x5e4` | i32 | threshold; the gear/set pass is skipped for entries whose stamp is not below it [I] |
| `+0xbc0 + i*8` | i32 | final cached value (validated by the next byte) |
| `+0xbc4 + i*8` | u8 | valid flag |
| `+0x1198` | pointer | modifier-list head (`FUN_398a0060` uses it) |
| `+0x119c` | i32 | XOR mask / salt |
| `+0x11a0` | - | end; `unit+0x2c48` cooldown manager follows |

`i` is a `UnitAttribute` index (0..186, see §2). The valid range of the arrays ends at
`0xbc0 + 186*8 + 8 = 0x1198`, exactly where the modifier pointer starts **[R]**.

## 2. `UnitAttribute` index space (server `models/unit.rs:110` **[R]**)

The client's stat index space is the same enum as the server's `UnitAttribute`. Confirmed on the
hot entries: 0..5 Str..Fai, 6/7 MaxHealth/MaxMana, 8 Armor, 10 MoveSpeedMul, 11/12 Health/ManaRegen,
13 Facets, 16 MeleeCritical, 17 MeleeCriticalBonus, 33 MeleeDpsInc, 36 MainhandSpeed,
37/38 MainhandDamageMin/Max, 41 OffhandSpeed, 46 RangedSpeed, 47/48 RangedDamageMin/Max,
57 IgnoreArmor, 64 MagicResist, 67/68 Persistent regen, 71 CastingTimeMul, 74 GlobalCooldownMul,
77 MeleeCriticalMul, 87 SpellDps, 92 FallDamageMul, 96/97/98 Main/Off/RangedDps, 120 HealMul,
121..123 backattack, 142/144/146/148 incoming damage add, 173 HealDps, 178 Dodge, 180 DodgeMul,
181 BullsEye, 182 BattleResist, 183 Flexibility. The tables in [`formulas.md`](formulas.md) §5 and
`tooltips.md` give the formulas. The 21-row `unit_attribute_limits` table clamps the result
(`FUN_395729f0`).

## 3. Actor fields used by this system (`actor = unit+8`) **[R]**

| Offset | Type | Meaning | Evidence |
|---|---|---|---|
| `+0x408` | i32 | owner type 0..6 (character, npc, slave, housing, transfer, mate, shipyard) | `FUN_398af1e0`, `FUN_39567110`, `FUN_398b4e80` |
| `+0x410` | i32/u8 | unit kind (NPC kind for `owner==1`) | `docs/aa-ui.md` battlefield getter, `FUN_398b43d0` |
| `+0x414` | u32 | desc/template id (NPC, housing, mate, slave depending on owner) | `FUN_39566e30`, `FUN_398b43d0`, `FUN_398b47f0` |
| `+0x418` | i16/u16 | housing progress/level for the MaxHealth formula | `FUN_398b47f0` |
| `+0x41c` | u32 | NPC template id (for `GetNpcTypeIndex`) | `docs/aa-ui.md` |
| `+0x4a4` | i32 | `char_race` selector 0..8 | `FUN_39567110` cached table index |
| `+0x4ac` | u8 | level | `FUN_39567110`, attributes |
| `+0x4b0` | array | 27 equipment/item slots, stride 0xc0; slot `i` at `actor+0x4b0+i*0xc0`. `FUN_398a0a50` scans 0..18, `FUN_398b4b90` scans 0..14 + four accessory slots at `+0xff8` | `FUN_398a0a50`, `FUN_398b4b90` |
| `+0x4b8` | u32 | item id inside each slot (`slot+8`) | `FUN_398a0a50`, `FUN_398b4b90` |
| `+0x4c4` | u8 | item "usable/equipped" byte inside each slot (`slot+0x14`) | `FUN_398b4b90` (checks `+0x15` from a `piVar6` that started at `+0x4b8`) |
| `+0x3af1` | u8 | cooldown/GCD disabled flag (`*(*manager)+0x3af1`) | `FUN_39899f30` |
| `+0x3bc8..+0x3bdc` | 6 x i32 | additive attribute values (Str..Fai). `FUN_398b4e80` cases 0..4 add only the first five (`+0x3bc8..+0x3bd8`); Fai (`+0x3bdc`) is not added on that path in 1.2 [R] | `FUN_398b4e80` cases 0..4 |
| `+0x3be8` | f32/i32 | movement speed value returned by `FUN_390b3750` (used for `move_speed`) | `FUN_390b3750`, `FUN_394bdc10` |
| `+0x2c40` | - | `actor+0x2c40 = unit+0x2c48` cooldown manager (see `port-combat`) | layout arithmetic + `port-combat/MAP.md` |

The six attribute leaves read `actor+0x4ac` (level) and the item functions read the slot array at
`actor+0x4b0`. `unit+0x1aa0` is the target id and `unit+0x1aa8` the manager; do not confuse them
with the actor offsets used by the decompiled stat functions.

## 4. Equipment/item objects touched by the stat code **[R]**

| Offset | Type | Meaning |
|---|---|---|
| item id slot | u32 | at `actor+0x4b0 + slot*0xc0 + 8` (the value passed to `FUN_39695dc0`) |
| item desc `+0xdc` | modifier list | `FUN_3989fde0` walks it as `{stat, type, value}` triples (item stats) |
| item desc `+0x48` | i32 | item type/impl discriminator (2 / 0x18 for weapons/accessories) |
| item desc `+0x4c` | i8 | armor type / item class (used as the formula index) |
| item desc `+0x74` | i32 | holdable/slot type |
| item desc `+0x7c` | i32 | item level (signed) |
| item desc `+0x30` | i32 | damage variance ratio (weapon min/max) |
| item desc `+0x38` | i32 | damage scale |
| item desc `+0x1000`, `+0x10c0`, `+0x1180` | item ids | mainhand/offhand/ranged slot item ids in the skill tooltip path |
| item `+0x2a4..+0x2a8` | vector | skill effect list; entries `{type=1 damage, type=2 heal}` with the desc at `entry+8` |
| item `+0x1ac`, `+0x68` | i32 | skill tick divisor / duration used by the tooltip per-tick division |
| item `+0x2b4` | ua list | unit-attribute modifiers for the `ua_*` tooltip keys |

The exact item desc offsets are best verified in `docs/aa-ui.md` (pet/item audits) and in
[`tooltips.md`](tooltips.md).

## 5. Formula descriptors

### 5.1 `unit_formulas` desc (`FUN_39894b00`, game-DB `+0xca18`, stride 0x1c) **[R]**

| Offset | Type | Meaning |
|---|---|---|
| `+0x00` | char* | interned formula text (from `FUN_398b17b0`) |
| `+0x04` | u32 | `kind_id` |
| `+0x08` | u32 | `owner_type_id` |
| `+0x0c` | vector | `unit_formula_variables` rows, 0x10 each: key, formula id, value f32, kind |

`FUN_39568860` reads `[+0x00]` for the text, `[+0x04]` kind, `[+0x08]` owner, and the vector `[+0x0c]`
for binding.

### 5.2 `formulas` desc (`FUN_39894790`, game-DB `+0xc948`, stride 8) **[R]**

`{u32 id, char* interned text}`; ids in a parallel array at `+0xc944`.

### 5.3 `wearable_formulas` desc (`FUN_396c7cc0`) **[R]**

Kind array at game-DB `+0x2dd4`, interned formula text at `+0x2dd8`; the same two rows are also
fetched as descs by `FUN_39695b30(0/1)` and parsed into the global context `+0x367c`.

### 5.4 `item_grades` desc (tooltip fields) **[R]**

0x1c bytes per grade. Fields used by `FUN_395684c0` / `FUN_39568560` / `FUN_39568610`:
`+0x00/+0x04/+0x08/+0x0c` (holdable formula coefficients), `+0x10` and `+0x14` (wearable armor /
magic-resistance coefficients), `+0x18` (enchant cost coefficient), and the DB columns
`var_holdable_dps`, `var_holdable_armor`, `var_holdable_magic_dps`, `var_holdable_heal_dps`,
`var_wearable_armor`, `var_wearable_magic_resistance`.
