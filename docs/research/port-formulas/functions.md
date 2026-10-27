# Function inventory: formulas and stats (1.2 client)

See [`functions-inventory.md`](functions-inventory.md) for the machine-generated table of every
function in this map (VA, size from `funcs.csv`, caller/callee counts from `calls.tsv`). This file
adds the role, the state it touches and the differential-test entry point. VAs are `x2game.dll`
(base `0x38ff0000`). "P" = pure (depends only on arguments/read-only data), "S" = stateful.

## 1. Expression parser (area A)

| VA | size | function | role | state | test |
|---|---|---|---|---|---|
| `0x398b0b20` | 199 | parse entry | `parse(ctx, text) -> node*`; logs the syntax error and returns 0 | P (uses stack parser state) | call cdecl `ctx,text_ptr`; EAX = node |
| `0x398b0450` | 234 | lexer driver | copy token, `atof` number, 4-state DFA | P | indirectly via parse |
| `0x398afbf0` | 348 | lexer state 0 | start/delimiters/operators | P | via parse; token-level cases can be tested with a scratch parser state |
| `0x398afde0` | 156 | lexer state 1 | identifier continuation | P | via parse |
| `0x398afef0` | 202 | lexer state 2 | number continuation | P | via parse |
| `0x398b0030` | 174 | lexer state 3 | finalized identifier/lookahead | P | via parse |
| `0x398b0a70` | 165 | additive level | builds `+`/`-` (types 2/3), left-assoc | P | via parse |
| `0x398b09c0` | 165 | multiplicative level | builds `*`/`/` (types 4/5), left-assoc | P | via parse |
| `0x398b08a0` | 281 | power level | builds `^` (type 6), right-assoc; unary-minus special case | P | via parse |
| `0x398b0840` | 85 | unary level | builds unary `-` (type 7) | P | via parse |
| `0x398b0680` | 433 | atom level | number/identifier/parenthesized expression | P | via parse |
| `0x398b0560` | 257 | argument list | comma-separated expressions | P | via parse |
| `0x398b0150` | 756 | builtin dispatch | builds the function nodes, arity checks, unknown-name fallback | P | via parse |
| `0x398afbc0` | 44 | node helper | allocates a 3-operand node (0xc..0xf) | P | via parse |
| `0x39568140` | 58 | variable-name lookup | name -> slot 0..0x22, unknown -> 0x23 | P | call with a name pointer, EAX |
| `0x39568180` | 58 | function-name lookup | 2-entry table, unknown -> 1 | P | call with a name pointer |
| `0x398b17b0` | 300 | string interner | copies formula text into 1 MB pages | S (global pool) | not needed by a Rust port |

## 2. Expression evaluator (area A)

The evaluator table is `0x3a5bd4c0[24]`. Node layout: `[0]=ctx`, `[1]=type`, `[2..4]=args`.
Every node function is `cdecl(node_ptr) -> float10` (ST0) and reads variables from
`ctx+0x10+index*4` **[R]**. Test recipe: parse a small formula into a scratch context, read the
node type at `node+4`, call the matching evaluator, read ST0.

| type | VA | size | role |
|---|---|---|---|
| 0 | `0x39566d00` | 11 | constant `node[2]` |
| 1 | `0x39567fb0` | 44 | variable `ctx+0x10+idx*4`; out-of-range log |
| 2 | `0x395676c0` | 117 | add |
| 3 | `0x39567740` | 117 | sub |
| 4 | `0x395677c0` | 117 | mul |
| 5 | `0x39567840` | 142 | div (zero log) |
| 6 | `0x395678d0` | 127 | pow (`_CIpow`) |
| 7 | `0x39567a70` | 63 | neg |
| 8 | `0x39567ab0` | 63 | sin |
| 9 | `0x39567af0` | 63 | cos |
| 10 | `0x39567b30` | 67 | tan |
| 11 | `0x39567b80` | 67 | atan |
| 12 | `0x39567bd0` | 188 | clamp (3 args, exclusive max) |
| 13 | `0x39567c90` | 155 | if_zero |
| 14 | `0x39567d30` | 151 | if_positive |
| 15 | `0x39567dd0` | 147 | if_negative |
| 16 | `0x39567950` | 132 | min |
| 17 | `0x395679e0` | 136 | max |
| 18 | `0x39567e70` | 87 | floor |
| 19 | `0x39567ed0` | 71 | log10 (log2*0.3010299956639812) |
| 20 | `0x39567f20` | 72 | sqrt |
| 21 | `0x39567f70` | 63 | abs |
| 22 | `0x39569190` | 273 | registered function call (id 0 only) |
| 23 | `0x39569540` | 121 | table path (not used by formulas) |

Node types 2..21 are pure; 1 and 22 read the context/function table (stateful only through the
caller-provided context). `0x395692b0` (154 B) is the string-interning/hash helper used by the
table-path lookup, not by formulas.

## 3. Formula managers and loaders (area A)

| VA | size | function | role | state |
|---|---|---|---|---|
| `0x39569cd0` | 88 | initializer | constants -> formulas -> unit formulas -> item formulas; returns bool | S (global `0x39ad7180`) |
| `0x395681c0` | 92 | constants loader | `constants` ids 1..11 -> `+0xac` | S |
| `0x39568220` | 188 | generic formulas loader | ids 2..27 -> `+0xdc` | S |
| `0x39568860` | 1447 | unit formulas loader | parses `unit_formulas`, binds `unit_formula_variables`, validates NaN keys | S |
| `0x395699b0` | 785 | item formulas loader | holdable `formula_*` per holdable, 2 wearable formulas, item-grade descs | S |
| `0x395682e0` | 221 | clear caches | clears the 5 vectors and 2 wearable nodes | S |
| `0x395683d0` | 45 | reset | clears caches and descriptors | S |
| `0x3956a880` | 334 | hot reload | compares desc package, builds a temp manager, copies + logs | S |
| `0x3956ae80` | 572 | static/dynamic verifier | builds a test actor, compares all kinds/owners, tolerance 0.1 | S |
| `0x3956a9d0` | uint | verify owner 2..6 | toggles static/dynamic per kind | S |
| `0x3956ab30` | uint | verify owner 0 | same | S |
| `0x3956acd0` | uint | verify owner 1 (NPC) | same | S |
| `0x3956db10` | 74 | owner names | `character..shipyard` | P |
| `0x3956db80` | 62 | owner iterator | 0..6 | P |
| `0x39571b70` | 92 | kind iterator | 1,2,5,6,7,10..46 | P |
| `0x39571970` | 324 | kind/owner name | log formatting | P |
| `0x39571900` | 35 | invalid kind name | log formatting | P |

The exact source of the package id is `FUN_39572400(0)` (6 B, returns a global),
`FUN_39572420/460/470` (id/version helpers). They are stateful globals and not needed if the port
treats the DB as immutable.

## 4. Static mirrors (area A)

| VA | size | function | role |
|---|---|---|---|
| `0x398ac290` | 1369 | generic formula ids 2..27 | hard-coded mirror of the `formulas` text; reads the 35 variable slots |
| `0x39567110` | 445 | unit formula by kind | dynamic/static switch + variable binding (see `formulas.md` §3.6) |
| `0x39566e30` | 120 | bind NPC template/kind/grade | from the three tables |
| `0x395672d0` | 118 | bind npc/pc level + range | writes slots 6/7/8, evaluates formula 5 |
| `0x39567350` | 100 | bind `npc_size_avg`, eval id 14 | static/dynamic |
| `0x395673c0` | 100 | bind `npc_size_avg`, eval id 15 | |
| `0x39567430` | 100 | bind `npc_size_avg`, eval id 16 | |
| `0x395674a0` | 100 | bind `npc_size_avg`, eval id 17 | |
| `0x39567510` | 100 | bind `npc_size_avg`, eval id 18 | |
| `0x39567030` | 103 | bind `experience`, eval id 6 | |
| `0x395670a0` | 103 | bind `pc_level`, eval id 7 | |
| `0x39566eb0` | 86 | evaluate generic id | dynamic node or static mirror |
| `0x39566f10` | 129 | bind source/target/stealth, eval id 4 | |
| `0x39566fa0` | 129 | bind source/target/stealth, eval id 10 | |
| `0x395675f0` | 199 | `%`/fmod helper | **unused by formulas** (no `%` node) [?] |

## 5. Unit stat engine (area B)

| VA | size | function | role | offsets/globals | kind |
|---|---|---|---|---|---|
| `0x398a0bc0` | 407 | **stat getter** | cache, owner dispatch, modifier merge, clamp | manager `+0xbc0`, `+0xc`, `+0x10`, `+0x1198`, `+0x119c`; actor `+0x408` | S |
| `0x398a0d60` | 916 | stat -> formula-kind map | huge switch; direct helpers for special stats | — | P |
| `0x398af1e0` | 1966 | static unit formula dispatch | kind 1..46 -> leaf | ctx + tables | S (reads ctx/actor) |
| `0x398b4e80` | 1026 | owner 0/2/4/5/6 stat base | attributes, gear-based stats, formulas | actor `+0x3bc8`, `+0x4b0` | S |
| `0x398b43d0` | 832 | owner 1 (NPC) stat base | NPC desc formulas + caches | actor `+0x414` | S |
| `0x398b4880` | 36 | owner 3 (housing) stat base | only kind 6 special | actor `+0x418` | S |
| `0x398b47f0` | 280 | housing MaxHealth | unfinished-unit scaling | housing desc `+0x54/+0x90..0x94` | S |
| `0x398b56d0` | 28 | float10 -> int | truncate (default) or round-to-nearest if `DAT_3a5e41f0=0` | global `0x3a5e41f0` | P |
| `0x3989ffb0` | 146 | static template modifiers | NPC/housing/slave/mate desc lists | actor desc | S |
| `0x398a0060` | 140 | buff/provider/temp modifiers | walks the modifier list | manager `+0x1198` | S |
| `0x398a0a50` | 352 | gear buckets | 19 slots (0..18) strided 0xc0, item desc `+0xdc`, 7 gems; then `398a0350` | actor `+0x4b0` | S |
| `0x398a0350` | 291 | set/extra modifier pass | called at the end of the gear pass | actor `+0x4b0` | S |
| `0x3989fde0` | 238 | apply one modifier list | `{stat, type, value}` walker | list pointer | P |
| `0x3989fa80` | 863 | build item modifier list | item desc stats/gems | item desc | P/S |
| `0x395729f0` | 38 | clamp | `unit_attribute_limits` | game-DB `+0x101ac + stat*8` | P |
| `0x390b3a80` | 36 | stat getter wrapper | `get(unit, attr, 0)` | unit `+0x1aa8` | S |
| `0x39005b20` | 21 | global formula object | reload + return `0x39ad7180` | — | S |
| `0x398aa290` | 2487 | UnitInfo array builder | 61 f32 fields from stat getter | actor/unit | S |

The modifier application order is documented in `docs/aa-ui.md` §"Native pet information-panel
stat contracts": aggregate is a pair `(flat=0, percent=100)`, all sources add deltas, then
`(base+flat) * percent * 0.01` is truncated unless percent is exactly 100.

### 5.1 Attribute/derived leaves

| VA | size | kind/role | note |
|---|---|---|---|
| `0x398adda0` | 231 | Str | owner branches; character: `(level-1)*2+10 + (level>=51 ? (level-50)*8 : 0)` |
| `0x398adf50` | 231 | Dex | same |
| `0x398ae100` | 231 | Sta | same |
| `0x398ae2b0` | 231 | Int | same |
| `0x398ae460` | 231 | Spi | same |
| `0x398abc90` | 141 | Fai | same, no high-level term |
| `0x398ad0b0` | 250 | MaxHealth | calls stat 2 (Sta), adds level term |
| `0x398ad470` | 233 | MaxMana | calls stat 3 (Int), adds level term |
| `0x398abad0` | 195 | HealthRegen | calls stat 4 (Spi) |
| `0x398abbb0` | 186 | ManaRegen | calls stat 4 (Spi) |
| `0x398ad580` | 189 | Armor (NPC) | uses NPC desc, logs `formula armor no npc desc` |
| `0x398aed90` | 94 | IncomingMeleeDamageAdd | power formula |
| `0x398aee10` | 94 | IncomingRangedDamageAdd | |
| `0x398aee90` | 94 | IncomingSpellDamageAdd | |
| `0x398abf40` | 142 | Block | stat 0 |
| `0x398aeb50` | 157 | BaseMissPercent | level-diff clamp |
| `0x398acca0` | 558 | LevelDps | |
| `0x398aba40` | 172 | LevelMana | |
| `0x398ad800` | 176 | MagicResist | |
| `0x398ad890` | 218 | Facet | |
| `0x398adc70` | 292 | MeleeDpsInc | |
| `0x398ae7f0`, `0x398aeae0` | 300/300 | Ranged/Spell DpsInc | |
| `0x398abe00`, `0x398abe70` | 120/120 | Persistent regen | |
| `0x398aec10`, `0x398aec90` | 128/128 | KillExp | |
| `0x398aef80` | 276 | HealCritical | |
| `0x398af170` | 73 | HealDpsInc | |
| `0x398abff0`..`0x398ac230` | ~90 each | Dodge (per owner) | |

## 6. Gear / item formulas (area B/C)

| VA | size | function | role |
|---|---|---|---|
| `0x398b4b90` | 302 | armor from 15 armor + 4 accessory slots | uses `wearables.armor_bp`/`FUN_397cc6d0` |
| `0x398b4ae0` | 176 | weapon/mainhand DPS from slots | `FUN_397cc750` |
| `0x398b49e0` | 250 | melee/ranged damage from weapon | `FUN_397cf870` + fist spec |
| `0x398b4cd0` | 194 | magic resistance / offhand sum | |
| `0x398b4da0` | 214 | remaining stat sum | |
| `0x398b48b0` | 295 | CalcDamage per slot | uses stats `0x60/0x61/0x62`, `0x21/0x22` |
| `0x397cc440` | 78 | holdable formula 0 (DPS) | `FUN_39568560(ctx,0,...)` |
| `0x397cc490` | 40 | copy min/max from item desc | `item+0x3c/+0x40/+0x44` |
| `0x397cc4c0` | 84 | holdable formula 2 (weapon armor) | |
| `0x397cc6d0` | 116 | wearable formula 0 (armor) | `FUN_39568610(ctx,0,...)` |
| `0x397cc750` | 116 | wearable formula 1 (magic resist) | `FUN_39568610(ctx,1,...)` |
| `0x397cd4f0` | 245 | weapon min/max damage | `FUN_39568560(ctx,0,...)` |
| `0x397ce6b0` | 945 | durability | item grade + item level/stack |
| `0x397cf870` | 59 | item desc lookup | |
| `0x395684c0` | 155 | grade enchant cost (formula 22) | item level/grade/slot |
| `0x39568560` | 156 | holdable formula evaluator | selector 0/1/2/4 |
| `0x39568610` | 135 | wearable formula evaluator | selector 0/1 |
| `0x398a7220` | 110 | rating helper | formula kind, subtract `stat(0xd)*stat(mult)*-0.001` |
| `0x398a7290` | 133 | rating helper with item check | |
| `0x398a7640` | 128 | rating helper with item +0xff8 | |
| `0x398a76c0` | 76 | rating helper with buff check | |
| `0x398a75f0` | 21 | `desc+0xfc*level + desc+0x68` | duration |
| `0x398a7710` | 103 | level scaling | `(level - min_level) * inc` clamped >=0 |
| `0x398a7780` | 127 | level scaling with desc field | round |
| `0x398a78a0` | 155 | add stat-scaled damage | |

## 7. UI producers (area B/C)

| VA | size | function | role | test |
|---|---|---|---|---|
| `0x394bdc10` | 4132 | `X2Unit:UnitInfo(unit)` table | 58 keys from `FUN_398aa290`; `move_speed` from `FUN_390b3750` | call via Lua `UnitInfo`; dump the Lua table |
| `0x394bec40` | 5608 | `X2Unit:UnitModifierInfo(unit)` table | modifier deltas from `394bb740`; label colors | Lua |
| `0x398aa290` | 2487 | UnitInfo f32 array builder | 61 fields; stat getter calls; see `tooltips.md` §? | thiscall `dest, actor, global_ctx`; dump 0xf4 bytes |
| `0x3941fc20` | 1767 | item tooltip (weapon/armor variant) | armor/DPS/durability/min-max | Lua item tooltip or direct call |
| `0x39420310` | 959 | armor item tooltip | armor/magic resistance | |
| `0x394206d0` | 828 | armor item tooltip variant | | |
| `0x39420b30` | 3053 | skill tooltip detail table | melee/ranged/spell/heal multipliers, ranges | |
| `0x39421e20` | 5876 | item info table | socket/enchant/price/durability | |
| `0x394ace10` | 1585 | X2Skill `Info` table | mana/cast/cooldown/ranges; calls `3941e750` | |
| `0x3941e750` | 638 | skill info value filler | desc fields, `39354340/a0`, `390b4110` | |
| `0x3941ed20` | 931 | buff tooltip placeholders | | |
| `0x3941f0d0` | 2893 | skill tooltip placeholders | | |
| `0x3941b030` | 1015 | damage range of an effect | | pure except stat getter |
| `0x3941a460` | 393 | heal range of an effect | | |
| `0x3941a160`, `0x3941a240` | 212/187 | level damage / damage per hit | | |
| `0x3941a300`, `0x3941a3c0` | 179/154 | level heal / heal per hit | | |
| `0x3941a700` | 306 | `ua_*` tooltip value | | |
| `0x3941cc00`..`0x3941d4b0` | 111..203 | damage/heal/mana placeholder leaves | | small pure wrappers |
| `0x39354340`, `0x393543a0` | 46/121 | skill field + modifiers | | |
| `0x390b4110` | 326 | min/max range | | |

## 8. World-rule functions (area D)

| VA | size | function | role |
|---|---|---|---|
| `0x39461860` | 1272 | crafting window table: `consume_lp`, `needed_lp`, satisfaction | |
| `0x39462a70` | 886 | crafting products: `success_rate`, amount, grade | |
| `0x397a9420` | 239 | needed-labor helper | |
| `0x397a9330` | 47 | recipe/actability lookup | |
| `0x397a8d10` | 48 | recipe flag lookup | |
| `0x3932a190` | 220 | actability/house bonus | |
| `0x390450b0` | 578 | grade-enchant ratio composition | |
| `0x39065d70` | 288 | `GetRatioInfos` handler | |
| `0x39044120` | 54 | support-item ratio | |
| `0x395b2d60` | 503 | socket-chance loader | |
| `0x396cd310` | 591 | enchant-support loader | |
| `0x396ce260` | 508 | physical-enchant loader | |
| `0x3972fa10` | 520 | recover-exp-effects loader | |
| `0x394aeb80` | 121 | `GetSpecialtyRatio` | |
| `0x394aec00` | 139 | `GetSpecialtyRatioBetween` | |
| `0x394ae8c0` | 696 | `GetSpecialtyRatioRefund` | |
| `0x394fb2f0` | 6 | cached specialty ratio | |
| `0x394fb6b0` | 168 | specialty ratio between zones (throttled) | |
| `0x398a9bf0` | 189 | base price (profit*ratio + refund) | |
| `0x398a9cb0` | 133 | price / ratio divisor | |
| `0x395ac920` | 150 | specialty zone comparison | |

## 9. Notes on testability

* Parser/evaluator/node leaves can be tested with a **scratch context**: allocate 0x400 bytes,
  parse with `398b0b20(scratch+0xc, text)`, set variables at `scratch+0x1c+idx*4`, call the node
  evaluator; no game state needed. This is the recommended first oracle (see `difftest.md`).
* `FUN_398ac290` (static generic formulas) takes the context base and reads `+0x1c+idx*4`; fill a
  scratch context and call it for every id/parameter combination.
* Unit formula/stat functions need an actor (and for kind 6+ a stat manager). Use the live local
  player's `unit` pointer: manager = `unit+0x1aa8`, actor = `unit+8`; call `398af1e0` with a scratch
  formula context (static mode flag `*ctx=1`) and the live actor, or call `39567110` with the
  shared global context and the live actor in dynamic mode. Read/write only the scratch context;
  do not leave `expr_mode` changed.
* `398a0bc0` mutates the per-unit cache (and `+0x5e4`/stamps). Run it on the live player between
  frames and restore any field you force; the caches are recomputed.
* `FUN_398aa290`/`394bdc10` are UI producers; compare through Lua (`unit:UnitInfo()`) instead of
  constructing a Lua table by hand.
* The item tooltip producers need a real item instance (or a crafted desc with valid vectors);
  the pure item-formula leaves (`39568560`/`610`) can be fed a scratch grade desc and a holdable id
  and are better first targets.
