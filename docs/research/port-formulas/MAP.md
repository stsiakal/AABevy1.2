# Map: 1.2 client formulas and stats (for a line-by-line Rust port)

> **Restored 2026-10-27 from `archive/fork-main-2026-10-27`, UNVERIFIED.** The client was restarted from NickMesser's `main`; nothing described here is wired into the game. Addresses, call chains and offsets must be re-checked against the decompile ([R]/[I]/[?]) before use. A 2026-10-27 audit found 151 of the 269 EXACT/TOLERANCE labels in `proof-levels.*` weak or wrong (report: `D:\aaemu-client\handoff\2026-10-27\audit\report-proofs.md`).

| | |
|---|---|
| Worktree / branch | `D:/aaemu-client/wt/formulas`, `port/formulas-map` |
| Date | 2026-10-24 |
| Scope | A: expression engine; B: stat calculation and UnitInfo; C: skill/item/buff tooltip numbers; D: other client-side world rules |
| Status | research map, read-only; no code ported in this task |
| Sources | 1.2 `x2game.dll` decompile (VA base `0x38ff0000`), `.data` dump, `funcs.csv`, `calls.tsv`, `strings-xrefs.tsv`; engine/10.x names for naming only; server `aaemu-game` for sharing status; client DB `compact.sqlite3` |
| Licensing | Only the shipped binaries/decompiles and the 10.x/engine decompiles were used. No published CRYENGINE source was consulted, quoted or used. |

Related maps (not re-mapped here): [`../port-program.md`](../port-program.md) (programme rules),
[`../port-combat/MAP.md`](../port-combat/MAP.md) (GCD `FUN_39899df0`/`39899f30`, requirements,
cooldowns), [`../port-movement-extra/MAP.md`](../port-movement-extra/MAP.md) (fall damage
`FUN_39116f00`), [`../re-constants.md`](../re-constants.md), and the existing
`docs/aa-ui.md` §"Native pet information-panel stat contracts" (UnitInfo/UnitModifierInfo
presentation details).

Companion notes: [`formulas.md`](formulas.md) (data/enums/language), [`structs.md`](structs.md)
(layouts), [`functions.md`](functions.md) (roles and test entries),
[`functions-inventory.md`](functions-inventory.md) (raw VA/size/callers table),
[`tooltips.md`](tooltips.md) (area C), [`worldrules.md`](worldrules.md) (area D),
[`difftest.md`](difftest.md) (harness), [`server.md`](server.md) (sharing),
[`unknowns.md`](unknowns.md).

Citation marks: **[R]** read in the decompile/data, **[I]** inferred, **[?]** not settled.
Every VA is in `x2game.dll`; the decompiled file is
`C:/AAEMU_DISK/aa-re/client12/out-x2game/x2game.dll.c`, the data dump
`D:/aa-re/client12/x2game_unpacked_data.dll`.

---

## 0. Executive summary

The client has **two parallel implementations of the same formula language**:

1. **Dynamic** (shipped default): the formula text is read from the game DB and parsed/evaluated by
   the client's own f32 expression engine (`FUN_398b0b20` parser, 24 node evaluators at
   `0x3a5bd4c0`). The cvar `expr_mode` defaults to `"dynamic"` **[R]**.
2. **Static mirrors**: hand-written equivalents (`FUN_398ac290` for generic formulas,
   `FUN_398af1e0` for unit formulas). They are **not a production mode**: the initializer sets the
   global mode byte to 0 (dynamic) and the only code that sets it to 1 is the startup verifier
   `FUN_3956ae80`, which toggles it while comparing the static and dynamic results for every unit
   formula/owner and then restores 0. `expr_mode="static"/"safe_static"` only *triggers that
   verification* (tolerance 0.1, log on mismatch); production evaluation stays dynamic **[R]**.

The two are therefore *not alternatives to guess between*: the dynamic path is the shipped source
of truth; the static mirrors are a built-in oracle. This is the single most important fact for the
port, and it means the Bevy client only needs the dynamic engine for behaviour.

The rest of the system:

* **One global formula object** at `0x39ad7180` (`FUN_39005b20`), shared by every unit. It holds the
  35 formula variables, the parsed formula nodes, the per-kind unit-formula node table, and the
  variable-coefficient tables for race/npc template/kind/grade/mate kind.
* **Per-unit stat cache/store** at `unit+0x1aa8` (`actor+0x1aa0`), size 0x11a0, ending exactly at the
  cooldown manager `unit+0x2c48`. `FUN_398a0bc0` is the stat getter: owner-specific base formula,
  then flat+percent modifier buckets (buffs, template, gear), XOR cache, `unit_attribute_limits`
  clamp.
* **Two distinct enums**: unit formula **kind** 1..46 (the same `UnitFormulaKind` as the server)
  and stat **index** 0..186 (the same `UnitAttribute` as the server). They are often confused in
  the decompiler because both are passed as small ints.
* **Character window** = the Lua `unit:UnitInfo()` table (`FUN_394bdc10`) fed by the f32 array
  builder `FUN_398aa290`; `UnitModifierInfo` (`FUN_394bec40`) is the delta/color table.
* **Tooltips**: skill/buff placeholder resolvers `FUN_3941f0d0`/`FUN_3941ed20` call shared effect
  helpers (`FUN_3941b030` damage, `FUN_3941a460` heal, `FUN_3941a700` `ua_*`); skill info
  (`manaCost`, cast/cooldown/range) comes from `FUN_3941e750`/`39354340`/`393543a0`.
* **Item tooltips** evaluate `holdables.formula_dps/mdps/hdps/armor` (4 formula vectors),
  `wearable_formulas` (2 formulas), and the `item_grades` coefficients via
  `FUN_39568560`/`FUN_39568610`; the server already implements exactly this in
  `models/item.rs:892-1001`.
* **Gearscore does not exist in 1.2**; it is a 10.x feature. The 1.2 character window is the
  UnitInfo table.
* **Area D** (labor, craft success, socket/grade-enchant chance, specialty price, repair, exp
  recovery, fall damage) is mapped in [`worldrules.md`](worldrules.md); several of these are
  client-side display computations with clear server counterparts.

Mapped seed set: **188 unique functions / 75,612 bytes** (the tables in
[`functions-inventory.md`](functions-inventory.md); tooltip sub-leaves extend the set but follow
the same pattern).

### 0.1 Proposed port order (evaluator first)

1. **Expression engine** (pure): lexer/parser/AST/evaluator with f32 semantics, exact error
   behaviour, and the 24 node types. Golden vectors from `FUN_398b0b20` + node evaluators on a
   scratch context (no actor). ← **first**
2. **Formula data/loader**: tables, enums, variable slots, unit-formula variable kinds and
   coefficient tables; dynamic/static switch and verification (static mirrors can be stubbed
   before step 3 if the port only uses dynamic).
3. **Static mirrors** `FUN_398ac290` + `FUN_398af1e0` (as a second oracle and to reproduce the
   `expr_mode` verification; not needed for normal behaviour).
4. **Stat store/getter** `FUN_398a0bc0`, owner dispatch `398b4e80/43d0/4880`, attribute leaves,
   modifier buckets, `unit_attribute_limits`.
5. **UnitInfo/UnitModifierInfo** (`398aa290`, `394bdc10`, `394bec40`) for the character/pet window.
6. **Item formulas** (`39568560/610`, `395684c0`) + item tooltip producers.
7. **Skill/buff tooltip numbers** (`3941b030`, `3941a460`, `3941a1xx`, `3941e750`).
8. **Area D display rules** (craft/enchant/specialty/repair/labor), reusing the shared parts.
9. **Server sharing pass** (enums, DB loaders, item formulas/chances, clamp table) and the
   clean-room rewrite.

Why this order: everything above depends on the evaluator and the variable binding; the stat
getter and tooltips are the two big consumers; item formulas are already proven server-side and
make a safe second step. The static mirrors are only needed for the `expr_mode` verification and
can be built from the same DB text once the evaluator exists.

---

## 1. The global formula object (`0x39ad7180`)

`FUN_39005b20` refreshes and returns `&DAT_39ad7180` **[R]**. `FUN_39569cd0` initializes it:
constants -> generic formulas -> unit formulas -> item formulas. A detailed layout is in
[`formulas.md`](formulas.md) §2 and [`structs.md`](structs.md) §3. Key points:

* The parser/evaluator state begins at object `+0xc`; node `[0]` points there. The 35 variable
  slots are object `+0x1c .. +0xa8` (state `+0x10`). The function-name table is `+0xa8`, the
  constants `+0xac..+0xdc`.
* Generic formula nodes `+0xdc` (ids 2..27), unit formula nodes `+0x14c` (`[kind][owner]`,
  47 x 7), and the five variable-coefficient tables `+0x670/+0xd0c/+0x1464/+0x2024/+0x2f90`.
* Item formula vectors `+0x362c` (5 descriptors), wearable nodes `+0x367c`, item-grade array
  `+0x3684`.

`FUN_39567110` (attr/kind value) and `FUN_398ac290` (generic formula) are the two evaluators the
rest of the game calls. `FUN_398af1e0` is the static unit-formula dispatcher.

---

## 2. Area A — the expression engine

Full details in [`formulas.md`](formulas.md) §3 and [`functions.md`](functions.md) §1-4.

### 2.1 Language

* Tokens: identifiers (`[A-Za-z_][A-Za-z0-9_]*`, CRT `isalpha/isdigit`), numbers with `.` (atof,
  no explicit exponent in the lexer), `+ - * / ^ ( ) ,`, whitespace (space only; the lexer uses
  `isspace` on the current byte).
* Grammar: `+/-` < `*/` < `^` (right-assoc) < unary `-` < atom. Function calls use a comma list.
  `min/max` 2 args, `clamp/if_*` 3, `floor/log/sqrt/abs` 1; unknown names become a registered
  function node (id 0 only in practice). There are no comparisons, logical ops, `%`, `round`,
  `avg` or `median`.
* Numeric type: f32 node values, x87 intermediates, narrow at each operation. `pow` uses
  `_CIpow`; `log10` is `log2 * 0.3010299956639812`; `floor` goes through double; division by zero
  logs and returns 0.
* Variables: 35 named slots (table `0x39acb050`); slots are at object `+0x1c+idx*4`. A missing
  name resolves to 0x23, logs, and returns 0. Unit formula variable rows are a *different* set:
  kind 1 char race, 2 npc template, 3 npc kind, 4 npc grade, 5 mate kind, stored in the five
  coefficient tables.

### 2.2 Caching / modes

* `expr_mode` default `"dynamic"` (registration `FUN_390f7b40`, line 196098; reader
  `FUN_39005bc0`). `static`/`safe_static` run the verifier `FUN_3956ae80`.
* Parsed nodes are stored per formula id/kind/owner; the stat getter caches final values per
  attribute. Formula text is interned by `FUN_398b17b0`.
* Hot reload `FUN_3956a880` swaps in a new desc package; irrelevant for a static DB port.

### 2.3 Loaders

Table -> SQL -> loader -> storage is tabulated in [`formulas.md`](formulas.md) §1. The DB rows
(`formulas` 26, `unit_formulas` 294, `unit_formula_variables` 2478, `wearable_formulas` 2) are
reproduced in §4/§7 of the same file.

---

## 3. Area B — stat calculation and the character window

Full detail in [`structs.md`](structs.md) §1-4, [`functions.md`](functions.md) §5, and the
UnitInfo table in [`tooltips.md`](tooltips.md) §? plus `docs/aa-ui.md` §"Native pet
information-panel stat contracts".

### 3.1 Layout (the one thing to get right)

`unit` = registry object. `actor = unit + 8`. `unit+0x1aa0` = target id. `unit+0x1aa8` =
stat store, and `actor+0x1aa0` is the same address. The store is 0x11a0 bytes, ending exactly at
`unit+0x2c48`, the cooldown manager. Proved by `FUN_390b7940` (`0x390b79e0` writes the target id,
`0x390b7980` passes `unit+0x1aa8`), `FUN_394270a0` (`0x394270f8` target), `FUN_394bdc10`
(`0x394bdd6e` advances to `unit+8`), and the 0x11a0 arithmetic. The decompiled stat functions use
the **actor** base, which is why they show `+0x1aa0` for the store.

Store fields: `+0x00` actor pointer; `+0x08` version/stamp; `+0xc+i*8` XOR/value; `+0x10+i*8`
stamp; `+0x5e4` gear-pass threshold; `+0xbc0+i*8` final value; `+0xbc4+i*8` valid byte;
`+0x1198` modifier list; `+0x119c` XOR mask. `i` is `UnitAttribute` (0..186).

### 3.2 Getter pipeline (`FUN_398a0bc0`)

1. If the valid byte is set, return the cached value.
2. If the stamp is fresh (`>= store[2]`) and not forced, return `mask ^ cached`.
3. Compute the base by owner:
   * actor owner `+0x408`: 0/2/4/5/6 -> `FUN_398b4e80`; 1 -> `FUN_398b43d0`; 3 -> `FUN_398b4880`.
4. Add modifiers: `FUN_398a0060` (buff/provider/temp), `FUN_3989ffb0` (static template), and for
   owner 0/5 the three gear buckets `FUN_398a0a50` (19 slots 0..18 + gems, then the set pass
   `FUN_398a0350`).
5. For each stat whose stamp is below `store[0x5e4]`, add the per-stat extra components
   (`FUN_398a0a50` stores them at `+0x17a/+0x17b` arrays).
6. `value = (base + flat)`, then if percent != 100 `value = trunc(f32(value) * f32(percent) *
   0.01f)`; clamp with `FUN_395729f0` (`unit_attribute_limits`); store `mask ^ value` and the
   valid byte.

The server's aggregate is a pair `(flat, percent)` initialized to `(0,100)`; all sources add
deltas; the final formula is the same. The **client order** is the contract; see `docs/aa-ui.md`
for the modifier buckets and the `398a7220` negative `0.001` multiplier.

### 3.3 Enums

* **Stat index** = `UnitAttribute` 0..186 (server `models/unit.rs:110`). The client's
  `FUN_398a0d60` covers most stat indices; `FUN_398b4e80` has explicit cases for 8 armor
  (`FUN_398b4b90`), 36..48 weapon slots, 64 magic resist (+`FUN_398b4ae0`), 87 spell dps,
  140..146 incoming adds and 173 heal dps. Examples from `FUN_398a0d60`: 0..5 -> kinds 22..27,
  6 -> kind 14, 7 -> kind 15, 11/12 -> kinds 16/17, 13 -> kind 20, 33..35 -> kinds 21/28/29,
  67/68 -> kinds 31/32.
* **Unit formula kind** = `UnitFormulaKind` 1..46; `FUN_398af1e0` maps each kind to a leaf
  (table in [`formulas.md`](formulas.md) §5). Kind 38 `RangedParry` has no case and returns 0
  (DB row is 0).
* **Owner** = 0 character, 1 npc, 2 slave, 3 housing, 4 transfer, 5 mate, 6 shipyard
  (`FUN_3956db10`).
* **Unit formula variable kind** = 1 char race, 2 npc template, 3 npc kind, 4 npc grade,
  5 mate kind (`formulas.md` §3.5). The server enum calls kind 1 `Unknown` — a naming bug for 1.2.

### 3.4 Attributes and derived stats

The five attributes Str..Spi are `10 + (level-1)*2 + (level>=51 ? (level-50)*8 : 0)` in 1.2
(disassembly `FUN_398adda0` etc.; DB kinds 22..26); Fai (kind 27) is `10 + (level-1)*2` without the
high-level term. Owner/gear bonuses are added afterwards. MaxHealth/MaxMana are
the DB formulas `level*150+sta*12+100` / `level*100+int*12+100` (kind 14/15), not a separate
hard-coded formula. The rest of the 42 kind formulas are in `formulas.md` §4.2/§7.

### 3.5 Character window

`FUN_394bdc10` = `X2Unit:UnitInfo(unit)`; it resolves the actor, calls `FUN_398aa290` to build a
61-f32 array, and registers 58 keys: melee/ranged/spell/heal dps, armor + %, magic resist + %,
str/dex/sta/int/spi, move_speed (from `FUN_390b3750`), casting/global cooldown, weapon speeds,
success/crit/parry/block/dodge/flexibility/battle resist, regen, min/max dps, damage muls,
incoming muls/vals. The array formulas are the code at `FUN_398aa290`; representative values:
`melee_dps = (MainhandDps + MeleeDpsInc)*0.001`, `armor% = 100*armor/(armor+5300)`,
`persistent_regen = 0.2*stat`, `GCD = 100000/(GlobalCooldownMul+1000)`.
`FUN_394bec40` = `UnitModifierInfo` (raw deltas and colors; `394bb740`). `docs/aa-ui.md` has the
full pet/crit/dodge presentation table and the f32 ordering.

---

## 4. Area C — tooltip numbers

Full table in [`tooltips.md`](tooltips.md).

### 4.1 Skill info (`manaCost`, cast, cooldown, range)

`FUN_394ace10` (X2Skill `Info`) -> `FUN_3941e750`: `manaCost = 39354340(desc,0)`,
`castingTime = 393543a0(desc)`, `cooldownTime = 39354340(desc,1)`, ranges = `390b4110`,
channeling = kind 0xb, area radius = kind 3. `39354340` -> `398a01b0` selects the skill desc field
(mana `+0xf0/+0xf4` with `level_dps`/`level_mana` scaling, cooldown `+0x88`, range `+0xfc`, area
`+0x188`, cast `+0x48`), then `3989f900` applies flat/percent modifiers and clamps to >=1.
The server's `ManaCost` effect port (`combat.rs:212-245`) uses the effect values
(`value1 + value2/100*scale`); they agree for the 62 skills noted there but are not guaranteed
identical for all skills.

### 4.2 Skill/buff tooltip placeholders

`FUN_3941ed20` (buff) and `FUN_3941f0d0` (skill) match placeholder names and call the shared
effect helpers:

* damage: `3941cc00/cca0` min/max per tick, `3941ca80/cb00` totals, `3941cb80` avg; base helper
  `3941b030` (weapon dps + effect coefficients + level scaling + skill modifiers).
* heal: `3941ced0/cf70/d010` per tick, `3941cd40/cdc0` totals; base helper `3941a460`.
* mana burn/restore: `3941d0c0..d4b0`.
* detail/level: `3941c600/c720/c840/c960` -> `3941a160`/`3941a240`/`3941a300`/`3941a3c0`.
* `ua_*`: `3941a700` (stat modifier walk + percentage formatting).
* duration/tick: `398a75f0` (`desc+0xfc*level+desc+0x68`), `3941ea10`, `39419cd0`.
* per-tick division: `desc+0x68 / desc+0x1ac` (`3941cc00`/`ced0`).

### 4.3 Item tooltip

`FUN_3941fc20` (weapon/armor variant), `39420310` (armor), `394206d0` (another armor variant),
`39421e20` (the big info table). Formula sources:

* holdable `formula_dps/mdps/hdps/armor` evaluated by `FUN_39568560(ctx, 0/1/2/4, holdable, grade)`
  in `397cc440/4c0/750` and `397cd4f0`;
* wearable `formula` 0/1 evaluated by `FUN_39568610(ctx, 0/1, slot, grade)` in `397cc6d0/750`;
* `item_grades` coefficients (`var_*`) at desc `+0x10/+0x14/+0x18`;
* durability `397ce6b0`; item desc `397cf870`/`397ced30`.

The server already implements the same items in `models/item.rs:892-1001`; the client's tooltip
arithmetic (f32, basis points, temper override) is in those functions and is the reference for the
Bevy client.

### 4.4 Gearscore

Absent in 1.2. `grep -i gearscore` on the 1.2 `x2game` strings finds nothing; the strings
`gearScore`, `GetGearScore`, `bareGearScore`, `totalGearScore` exist only in the 10.x client. Do
not introduce it into the 1.2 window.

---

## 5. Area D — other client-side world rules

See [`worldrules.md`](worldrules.md) for the full table. Highlights:

* **Craft window** `FUN_39461860`: `consume_lp`, `needed_lp` (via `397a9420`/`397a8dd0`),
  `laborpower_satisfied`, actability checks. **Craft products** `39462a70`: `success_rate`,
  amount, grade.
* **Grade-enchant chances** `GetRatioInfos` (`39065d70` -> `390450b0` + `39044120`) and the Lua
  `grade_enchant_view.alb` display; **socket chances** loaded by `395b2d60`; **physical enchant**
  loaded by `396ce260`.
* **Specialty price** `GetSpecialtyRatio` (`394aeb80`, cached packet value), `...Between`
  (`394aec00`), `...Refund` (`394ae8c0`), base price `398a9bf0`/`398a9cb0`; server
  `economy/specialty.rs`.
* **Repair/durability** `397ce6b0`; server `models/item.rs:789-815`.
* **Exp recovery** `3972fa10` + generic formulas 6/7; server `effects.rs:899`/`bridges.rs:679`.
* **Fall damage** is already mapped/ported in
  [`../port-movement-extra/MAP.md`](../port-movement-extra/MAP.md) §2.6 (do not duplicate).

---

## 6. Server status and sharing

The detailed table is in [`server.md`](server.md). In one line: **the server cannot supply the
client's bit-exact formula engine** (Jace/f64 vs client f32), but it already implements the DB
loaders/enums, character/NPC/vehicle stat formulas, item wearable/holdable formulas, enchant and
socket chances, specialty price, repair and labor. Add `unit_attribute_limits` and the client
f32 evaluator; share the rest.

---

## 7. Differential-test plan

[`difftest.md`](difftest.md) gives exact call specs. The driver already supports `cdecl`,
`thiscall`, `f32` arguments, ST0 capture, scratch allocation and dumps
(`aaemu-clientdriver/src/lib.rs:128-190,334-394`). First oracles:

1. Parser/evaluator on a scratch context (no actor): `398b0b20` -> node -> `0x3a5bd4c0[type]`.
2. `398ac290` static generic formulas with a filled scratch context.
3. `398af1e0` attributes with a scratch actor, then live-stat `398a0bc0` with the live
   `unit+0x1aa8` manager for the derived stats.
4. `398aa290`/`394bdc10` through Lua `UnitInfo`.
5. `39568560`/`39568610` item formulas with the live global context.
6. `390450b0`/`394aeb80`/`398a9bf0` for chances and prices.

The scratch-context trick (parse into a zeroed 0x400 buffer, pass `scratch+0xc` as the parser
context, fill slots at `scratch+0x1c+idx*4`) isolates the engine from production state and makes
the parser/evaluator tests deterministic.

---

## 8. Port order and acceptance

1. **Evaluator** (`398b0b20`, `398afbf0..398b0b20`, node evaluators, `39568140/180`) + f32
   semantics and error behaviour; goldens from §7.1. This is the first and highest-value port.
2. **Formula data** (tables, enums, 35 slots, five coefficient kinds, dynamic/static switch).
3. **Static mirrors** (`398ac290`, `398af1e0` + leaves) as the second oracle and for
   `expr_mode=static`.
4. **Stat getter** (`398a0bc0`, `398b4e80/43d0/4880`, attribute leaves, modifier buckets,
   `unit_attribute_limits`).
5. **UnitInfo/UnitModifierInfo** (`398aa290`, `394bdc10`, `394bec40`).
6. **Item formulas/tooltips** (`39568560/610`, `3941fc20`/`39420310`/`394206d0`/`39421e20`).
7. **Skill/buff tooltip numbers** (`3941b030`, `3941a460`, `3941a1xx`, `3941e750`).
8. **Area D** display rules.
9. **Server sharing** + clean-room rewrite.

Acceptance for this map: every function below has a VA/size/caller/callee row in
[`functions-inventory.md`](functions-inventory.md), every formula table has its SQL/loader/offset
in [`formulas.md`](formulas.md), every layout has evidence in [`structs.md`](structs.md), and every
differential entry has a call recipe in [`difftest.md`](difftest.md). The unknowns are explicit in
[`unknowns.md`](unknowns.md); none of them blocks the evaluator-first port.

---

## 9. Address quick reference

| Item | Address / file |
|---|---|
| global formula object | `0x39ad7180`, `FUN_39005b20` |
| parser entry | `FUN_398b0b20` (`0x398b0b20`) |
| node evaluator table | `0x3a5bd4c0` (24 entries) |
| generic static formulas | `FUN_398ac290` |
| unit static dispatcher | `FUN_398af1e0` |
| dynamic unit value | `FUN_39567110` |
| static/dynamic verifier | `FUN_3956ae80` |
| `expr_mode` default | `FUN_390f7b40` line 196098 (`"dynamic"`) |
| stat getter | `FUN_398a0bc0` |
| stat index map | `FUN_398a0d60` |
| owner dispatch | `FUN_398b4e80` / `398b43d0` / `398b4880` |
| attribute leaves | `FUN_398adda0`, `adf50`, `ae100`, `ae2b0`, `ae460`, `abc90` |
| UnitInfo array/table | `FUN_398aa290` / `FUN_394bdc10` |
| UnitModifierInfo | `FUN_394bec40` |
| skill info | `FUN_394ace10` -> `FUN_3941e750` |
| skill tooltip | `FUN_3941f0d0` |
| buff tooltip | `FUN_3941ed20` |
| damage/heal helpers | `FUN_3941b030` / `FUN_3941a460` |
| item tooltip | `FUN_3941fc20`, `39420310`, `394206d0`, `39421e20` |
| item formula eval | `FUN_39568560`, `FUN_39568610`, `FUN_395684c0` |
| enchant chance | `FUN_39065d70` -> `FUN_390450b0` |
| specialty ratio/price | `FUN_394aeb80`, `394aec00`, `394ae8c0`, `398a9bf0` |
| stat store | `unit+0x1aa8` / `actor+0x1aa0`, size 0x11a0 |
| target id | `unit+0x1aa0` |
| cooldown manager | `unit+0x2c48` (see `port-combat`) |
