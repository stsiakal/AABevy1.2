# 1.2 client formula data, enums and the expression language

All addresses are VAs in `x2game.dll` (image base `0x38ff0000`), decompile
`C:/AAEMU_DISK/aa-re/client12/out-x2game/x2game.dll.c`, data dump
`D:/aa-re/client12/x2game_unpacked_data.dll`. Database rows are from the read-only server copy
`C:/Users/stsiakal/AAEmu/rust/crates/aaemu-game/app/Data/compact.sqlite3`; the client loads its own
(encrypted) copy of the same tables, see the unknowns list.

Citation marks: **[R]** read in code/data, **[I]** inferred, **[?]** not found.

## 1. Tables the client loads (SQL string VA, loader VA, storage)

Every loader is called from the single game-DB load function `FUN_3960b820` (or from
`FUN_396092f0` for the second group). `FUN_3955b3d0` is the SQLite prepare/step wrapper,
`FUN_3953a390` / `FUN_3953a310` read int / float columns, `FUN_3953a480` reads text **[R]**.

| Table | SQL string | Loader | Destination in the game-DB object |
|---|---|---|---|
| `formulas` | `39a196e4` `SELECT id, formula FROM formulas` | `FUN_39894790` (404 B) | ids at `+0xc944`, descs at `+0xc948` stride 8: id, interned text [R] |
| `constants` | `39a1e420` `SELECT id, value FROM constants` | `FUN_39894600` (386 B) | ids at `+0xc8dc`, float values at `+0xc8e0` [R] |
| `unit_formulas` | `39a19708` `SELECT id, formula, kind_id, owner_type_id FROM unit_formulas` | `FUN_39894b00` (651 B) | ids `+0xca14`, descs `+0xca18` stride 0x1c: text, kind, owner [R] |
| `unit_formula_variables` | `39a1e500` `SELECT id, key, unit_formula_id, value, variable_kind_id FROM unit_formula_variables` | `FUN_39894930` (459 B) | ids `+0xc9ac`, rows `+0xc9b0` stride 0x10: key, formula id, value f32, kind [R] |
| `wearable_formulas` | `399ffa8c` `SELECT kind_id, formula FROM wearable_formulas` | `FUN_396c7cc0` (403 B) | kind array `+0x2dd4`, interned text `+0x2dd8` [R] |
| `holdables` | `39a00770` (full column list) | `FUN_396c7700` (1051 B) | holdable descs; formula texts `formula_dps/mdps/hdps/armor` [R] |
| `item_grades` | `39a00970` (full column list) | `FUN_396c8fb0` (836 B) | grade descs [R] |
| `wearables` (armor bp) | `399f7450` `SELECT armor_bp, armor_type_id, slot_type_id FROM wearables` | `FUN_395720f0` (336 B) | table at game-DB `+0x104` (see getter below) [R] |
| `wearables` (magic resist bp) | `399f74e8` `SELECT magic_resistance_bp, armor_type_id, slot_type_id FROM wearables` | `FUN_39572260` (336 B) | table at game-DB `+0x404` [R] |
| item grade order | `399f7ee8` `SELECT id FROM item_grades ORDER BY grade_order ASC` | `FUN_395b2b80` (285 B) | grade order vector [R] |
| `item_socket_chances` | `399f7f30` `SELECT success_ratio FROM item_socket_chances ORDER BY num_sockets ASC` | `FUN_395b2d60` (503 B) | vector of success ratios [R] |
| `item_grade_enchanting_supports` | `39a036c8` (full column list) | `FUN_396cd310` (591 B) | ids `+0x3ec4`, rows `+0x3ec8` stride 0x2c [R] |
| `physical_enchant_abilities` | `39a03e68` `SELECT id, armor, enchant_level, min_friendship, npc_id, success_ratio ...` | `FUN_396ce260` (508 B) | ids `+0x44c0`, rows `+0x44c4` stride 0x14 [R] |
| `recover_exp_effects` | `39a0ad88` `SELECT id, need_labor_power, need_money, need_priest, penaltied ...` | `FUN_3972fa10` (520 B) | ids `+0x2244`, bitset `+0x2248` [R] |
| `specialties` | `39a03050` `SELECT id, col_zone_group_id, profit, ratio, row_zone_group_id, vendor_exist FROM specialties` | `FUN_396cabf0` (507 B) | specialty descs [R] |
| `specialty_bundle_items` | `39a03118` | `FUN_396cadf0` (459 B) | bundle item descs [R] |
| `world_spec_configs` | `399f81d0` `SELECT world_id, specialty_mod, specialty_adjust_ratio FROM world_spec_configs` | `FUN_395b5520` (455 B) | world ratio modifiers [R] |

Get functions used by the tooltip code:

* `FUN_395720e0(kind)` -> game-DB `[kind]` (a small wearable table value).
* `FUN_39572240(armor_type, slot_type)` -> `game_db + (armor_type*0x20 + slot_type)*4 + 0x104`
  (armor basis points) **[R]**.
* `FUN_395723b0(armor_type, slot_type)` -> same but `+0x404` (magic-resistance basis points).

## 2. Formula manager (global object `0x39ad7180`)

The client creates **one global formula context** at `0x39ad7180`; every unit shares it. The
per-unit part is the attribute cache (§4) and the actor passed as an argument to the evaluator.
`FUN_39005b20` refreshes and returns `&DAT_39ad7180` **[R]**.

Object layout **[R]** (proved by the stores/loads listed; sizes are the memsets in
`FUN_39568860`):

| Offset | Size | Contents |
|---|---|---|
| `+0x00` | 1 | mode flag: `0` = dynamic (evaluate DB text), non-zero = static (hard-coded mirrors) |
| `+0x04`, `+0x08` | 4+4 | desc-package id / previous id used by the hot-reload path `FUN_3956a880` |
| `+0x0c` | 0xa0 | parser/evaluator state. Node `+0` points here. `+0x10` (i.e. object `+0x1c`) starts 35 f32 variable slots; `+0x9c` (object `+0xa8`) is the 2-entry function-name table |
| `+0xa8` | 8 | two function pointers (`none`, `FFK_MAX`; names at `0x39acb0e0`) |
| `+0xac` | 0x30 | 11 constants loaded from `constants` (ids 1..11; consumer not found) |
| `+0xdc` | 0x70 | 26 generic formula nodes, indexed by formula id 2..27 (**0x398ac290** static mirror) |
| `+0x14c` | 0x524 | unit formula nodes `[kind][owner]`, 47 kinds x 7 owners; index = `kind*7 + owner` |
| `+0x670` | 0x69c | `char_race` variable table, `[kind][race]`, 47 x 9 |
| `+0xd0c` | 0x758 | `npc_template` variable table, `[kind][template]`, 47 x 10 |
| `+0x1464` | 0xbc0 | `npc_kind` variable table, `[kind][npc_kind]`, 47 x 16 |
| `+0x2024` | 0xf6c | `mate_kind` variable table, `[kind][mate_kind]`, 47 x 21 |
| `+0x2f90` | 0x69c | `npc_grade` variable table, `[kind][grade]`, 47 x 9 |
| `+0x362c` | 5 x 0x10 | vector descriptors (begin/end/cap) of holdable formula nodes; entries 0/1/2/4 used, 3 unused [?] |
| `+0x367c` | 8 | two wearable formula nodes (kinds 0/1) |
| `+0x3684` | 4 | pointer to the `item_grades` desc array (stride 0x1c) |

`FUN_395682e0` clears the five vector descriptors and the two wearable nodes; `FUN_395683d0`
resets caches. `FUN_39569cd0` is the initializer: `395695c0` (function registration) ->
`395681c0` (constants) -> `39568220` (generic formulas) -> `39568860` (unit formulas) ->
`395699b0` (item formulas). It returns false if any step fails **[R]**.

### 2.1 Mode selection and verification (`expr_mode`)

* Cvar `expr_mode` is registered with default **`"dynamic"`** in `FUN_390f7b40`
  (`"expr_mode","dynamic",0x100,"expr mode [dynamic/static/safe_static]"`, line 196098 of the
  decompile) **[R]**.
* `FUN_39005bc0` (line 4414) reads it: `"static"` or `"safe_static"` sets a local flag that runs
  the verifier; any other value (including the default) skips it **[R]**.
* The initializer is called with the mode byte **0**: `FUN_39005bc0` at `0x39005c24` pushes 0,
  calls `FUN_39572400` (which does not pop it), pushes the return value and `&global`, then calls
  `FUN_39569cd0`; the callee reads the third stack argument at `0x39569cdd` and writes it to
  `global[0]` at `0x39569ce3`. Production evaluation is therefore **dynamic**, always **[R]**.
* The static path runs only inside `FUN_3956ae80`: the verifier sets `global[0]=0`, calls
  `FUN_39567110`, sets `global[0]=1` (`0x3956abd6`), calls it again, compares the two results
  (abs diff > 0.1), and restores the original byte at the end. `expr_mode=static/safe_static` only
  asks for this check; it does not change what the game uses **[R]**.
* The verifier:
  * builds a test actor (`FUN_398a1b70(DAT_3a5bd524, local)`, `FUN_398a0170(local,0)`),
  * runs `FUN_3956ab30` (character), `FUN_3956acd0` (NPC), then `FUN_3956a9d0` several times
    (the literal `2..6` is stored in a local before each call; the exact extra argument is hidden
    by the decompiler),
  * compares every unit formula kind; a difference > **0.1** logs
    `"formula calculation mismatch %s, owner %s, %.2f != %.2f"` (`399efa54`),
  * on failure `FUN_39005bc0` sets `DAT_39ad7180 = 0` (dynamic; it is already 0 in production) **[R]**.
* The verification loops tell the port exactly what "static == dynamic" means and are the best
  first oracle: they already exist in the shipped client.

### 2.2 Formula hot reload

`FUN_3956a880` (alloca-heavy) is called from `FUN_39005b20` on every evaluation wrapper with
`(global, thread_id=0)`. It compares the current desc-package id with the manager's and, if it
changed, initializes a temporary manager on the stack, copies it over, and logs
`"reloaded expressions from desc package %d thread %d"` (`399efa20`) or
`"failed to reload game expr ..."` (`399ef9d0`) **[R]**. A port can treat the DB as immutable.

## 3. Expression language (client, not Jace)

### 3.1 Node

A node is 0x14 bytes (`FUN_3950a1b0(0x14)`): `[0]` = context/state pointer, `[1]` = node type,
`[2..4]` = operands/arguments **[R]**. Node types and their evaluator (table at `0x3a5bd4c0`,
24 entries) **[R]**:

| type | VA | meaning | semantics |
|---|---|---|---|
| 0 | `0x39566d00` | constant | `*(f32*)(node+8)` |
| 1 | `0x39567fb0` | variable | `ctx[0x10 + index*4]`, index 0..0x22; out of range logs `"expr: no registered variable named '%d'"` and returns 0 |
| 2 | `0x395676c0` | `+` | f32 add |
| 3 | `0x39567740` | `-` | f32 sub |
| 4 | `0x395677c0` | `*` | f32 mul |
| 5 | `0x39567840` | `/` | f32 div; zero denominator logs `"expr: divide by zero"` and returns 0 |
| 6 | `0x395678d0` | `^` | x87 `_CIpow`, result narrowed to f32 |
| 7 | `0x39567a70` | unary `-` | negate |
| 8 | `0x39567ab0` | `sin` | x87 `fsin` |
| 9 | `0x39567af0` | `cos` | x87 `fcos` |
| 10 | `0x39567b30` | `tan` | x87 `fptan` |
| 11 | `0x39567b80` | `atan` | x87 `fpatan(x,1)` |
| 12 | `0x39567bd0` | `clamp(x,min,max)` | `x<min ? min : (x<max ? x : max)` (upper bound exclusive) |
| 13 | `0x39567c90` | `if_zero(x,a,b)` | `x==0 ? a : b` |
| 14 | `0x39567d30` | `if_positive(x,a,b)` | `x>0 ? a : b` |
| 15 | `0x39567dd0` | `if_negative(x,a,b)` | `x<0 ? a : b` |
| 16 | `0x39567950` | `min(a,b)` | 2 args only |
| 17 | `0x395679e0` | `max(a,b)` | 2 args only |
| 18 | `0x39567e70` | `floor` | `floor((double)f32)` |
| 19 | `0x39567ed0` | `log10` | `log2(x) * 0.3010299956639812` (not libm `log10`) |
| 20 | `0x39567f20` | `sqrt` | x87 `SQRT` |
| 21 | `0x39567f70` | `abs` | `ABS(x)` |
| 22 | `0x39569190` | registered function call | `[2]`=function id, `[3]`=argc, `[4]`=args; only id 0 is accepted (`"expr: no registered function named '%d'"`) |
| 23 | `0x39569540` | table path | not produced by the formula parser; used by the game-expression engine |

All arithmetic is on **f32 values**, computed in x87 and narrowed back to f32. There is no
comparison, logical, `%` or unary-plus node. `&&`/`||`, `<`, `>=`, `round`, `if`, `avg`, `median`
of the server's Jace engine do not exist in the client **[R]**.

### 3.2 Lexer (`FUN_398b0450` + four character-state functions)

`FUN_398b0450(parser_state, token_out)` drives a 4-state DFA. Character classes:

* `FUN_398afbf0` (state 0): `0` end; `(`->10, `)`->11, `*`->7, `+`->5, `,`->12, `-`->6, `.`->3,
  `/`->8, `^`->9; letter/`_`->1; digit->2; space keeps state; anything else is a syntax error
  (`token=2`, return 0) **[R]**.
* `FUN_398afde0` (state 1, identifier): delimiters -> state 3, letters/digits/`_` append, space ->
  state 3, `.` -> token 2 and return 0 (so `a.b` is a syntax error) **[R]**.
* `FUN_398afef0` (state 2, number): delimiters -> state 4, `.` appends, digit appends, space ->
  state 4; a letter sets token 2 and returns 0 (syntax error) **[R]**.
* `FUN_398b0030` (state 3, finalized identifier): delimiters -> state 4, space -> state 4;
  letter/digit/`.` sets token 2 and returns 0 (syntax error) **[R]**.

A completed number is converted by `atof` in `FUN_398b0450` (`dVar4 = atof(text); *(f32*)...`) and
stored as f32; multiple dots (`1.2.3`) are accepted by the lexer and left to `atof` **[R]**. Token
numbers: 0 initial, 1 identifier, 2 number, 3 identifier-complete, 4 number-complete, 5 `+`,
6 `-`, 7 `*`, 8 `/`, 9 `^`, 10 `(`, 11 `)`, 12 `,`.

### 3.3 Grammar (`FUN_398b0b20` entry)

Recursive descent, lowest precedence first:

| Function | Level | Builds |
|---|---|---|
| `FUN_398b0a70` | `+` / `-` | left-assoc nodes type 2/3 |
| `FUN_398b09c0` | `*` / `/` | left-assoc nodes type 4/5 |
| `FUN_398b08a0` | `^` | **right-assoc** node type 6; special-cases unary minus on the rhs |
| `FUN_398b0840` | unary `-` | node type 7 (binds tighter than `^`, so `-2^2 == 4`) |
| `FUN_398b0680` | atom | number -> type 0; identifier -> type 1 or, if followed by `(`, argument list; `(` expr `)` |
| `FUN_398b0560` | argument list | comma-separated `FUN_398b0a70` expressions, then `FUN_398b0150` |
| `FUN_398b0150` | builtin/function dispatch | `min`,`max`,`clamp`,`if_zero`,`if_positive`,`if_negative`,`floor`,`log`,`sqrt`,`abs`; unknown name -> type 22 function call |
| `FUN_398afbc0` | helper | allocates a 3-operand node (0xc/0xd/0xe/0xf) |
| `FUN_398b0b20` | entry | tokenizes, parses one expression, requires token 1 (end); logs `"expr: cannot parse '%s' at pos %d"` on failure; returns the node or 0 |

Builtin arities are strict: `min`/`max` 2, `clamp`/`if_*` 3, `floor`/`log`/`sqrt`/`abs` 1;
wrong arity logs `"expr: %s needs 1 argument"` (`39a209a8`) or `"expr: needs 3 arguments"`
(`39a209c4`) **[R]**. Identifiers are resolved to slot index by `FUN_39568140` using the
36-pointer name table at `0x39acb050` (35 names + the `FVK_MAX` sentinel); an unknown identifier
resolves to 0x23 (out of range), which
the evaluator logs and treats as 0 **[R]**. Unknown function names go through `FUN_39568180`
(two-entry table `0x39acb0e0`: `none`, `FFK_MAX`); any real unknown id fails at evaluation.

### 3.4 Variable names (slot index table `0x39acb050`, 36 pointers = 35 slots + `FVK_MAX`) **[R]**

| idx | name | idx | name | idx | name |
|---|---|---|---|---|---|
| 0 | `level_diff` | 12 | `sta` | 24 | `npc_kind` |
| 1 | `casting_tolerance` | 13 | `int` | 25 | `mate_kind` |
| 2 | `damage_percent` | 14 | `spi` | 26 | `npc_grade` |
| 3 | `source_level` | 15 | `fai` | 27 | `item_level` |
| 4 | `target_level` | 16 | `level` | 28 | `item_grade` |
| 5 | `stealth_level` | 17 | `ab_level` | 29 | `armor` |
| 6 | `npc_level` | 18 | `level_dps` | 30 | `magic_resist` |
| 7 | `pc_level` | 19 | `facets` | 31 | `equip_slot_enchant_cost` |
| 8 | `range` | 20 | `npc_size_avg` | 32 | `battle_resist` |
| 9 | `experience` | 21 | `labor_power` | 33 | `bulls_eye` |
| 10 | `str` | 22 | `char_race` | 34 | `flexibility` |
| 11 | `dex` | 23 | `npc_template` | | |

Variable slot address = **global formula object + 0x1c + idx*4** (the parser state starts at
`+0xc`, and its `+0x10` is the first slot) **[R]**. In the production DB only these identifiers
occur in formula text: `ab_level, armor, battle_resist, bulls_eye, casting_tolerance,
damage_percent, dex, equip_slot_enchant_cost, experience, flexibility, int, item_grade,
item_level, labor_power, level, level_diff, level_dps, magic_resist, mate_kind, npc_grade,
npc_kind, npc_level, npc_size_avg, npc_template, pc_level, range, source_level, spi, sta,
stealth_level, str, target_level` (query of the three formula tables). `char_race` is loaded and
validated but **not referenced by any formula text** in this DB revision **[R]**.

### 3.5 Unit formula variable kinds (`unit_formula_variables`)

`FUN_39568860` initializes the five per-kind tables with `quiet_NaN` and then, for every
`unit_formulas` desc (kind `[desc+4]`, owner `[desc+8]`), parses the text into
`+0x14c[kind*7+owner]` and writes each variable row into the table selected by
`variable_kind_id` (jump table `0x39568e18`) **[R]**:

| kind | table | base | stride | valid keys | name/values function | log string |
|---|---|---|---|---|---|---|
| 1 | char race | `+0x670` | 9 | 0..8 | `FUN_395701e0` (`none,nuian,fairy,dwarf,elf,hariharan,ferre,returned,warborn`) | `no char race var` `399ef7bc` |
| 2 | npc template | `+0xd0c` | 10 | 1..9 | `FUN_395709c0` (`tanker,caster,p_dealer,m_dealer,dungeon_* ,default`) | `no npc template var` `399ef780` |
| 3 | npc kind | `+0x1464` | 16 | 1..5,8..15 | `FUN_39570740` (`human,beast,undead,devil,spirit,dragon,siege_weapon,ship,horse,carriage,fantastic,machine,unknown`) | `no npc kind var` `399ef74c` |
| 4 | npc grade | `+0x2f90` | 9 | 1..8 | `FUN_39570670` (`normal,elite,boss_a,boss_b,boss_c,weak,strong,boss_s`) | `no npc grade var` `399ef6e4` |
| 5 | mate kind | `+0x2024` | 21 | 1..20 | `FUN_39570860` (`horse,good_horse,lion,good_lion,elk,good_elk,tare,good_tare,yata,donkey,wolf,cat,storm,land,thunder_rider,polarbear,pet_polarbear,submarine,temp1,temp2`) | `no mate kind var` `399ef718` |

Important details:

* The tables are indexed by **kind only**, not `(kind, owner)`. Loading owner 0..6 in order means a
  later owner's rows for the same kind overwrite earlier shared keys. The dynamic formula of each
  owner only reads the keys it references, and the DB rows supply the union per kind.
* Validation loops every valid key of every kind and logs `no <...> var: unit formula [%s] <kind>
  [%s]` when the slot is still NaN; any hit returns false from the loader **[R]**. The loader
  therefore treats the DB as complete.
* `FUN_39568860` also checks every `[kind][owner]` node is non-null and logs
  `no formula: unit formula [%s] owner type [%s]` (`399ef7f0`), `failed to parse ...`
  (`399ef820`) and `invalid formula ...` (`399ef85c`) **[R]**.
* The owner enum names come from `FUN_3956db10`:
  `character,npc,slave,housing,transfer,mate,shipyard` (0..6) **[R]**.

### 3.6 How the dynamic evaluator binds variables

`FUN_39567110(ctx, kind, actor)` (445 B) is the per-kind value function **[R]**:

1. If `ctx[0] != 0` (the verifier's temporary static toggle), tail-call
   `FUN_398af1e0(ctx, kind, actor)`. In production `ctx[0]` is 0 (dynamic) [R].
2. Else set `ctx+0x5c` (slot 16 `level`) from `actor+0x4ac`.
3. If kind is outside 22..27, refresh slots 10..15 (`str..fai`) by calling the attribute getter
   `FUN_398a0bc0(actor+0x1aa0, 0..5, 0)`; for kinds 22..27 only the level is needed.
4. Bind the owner-specific selector into the shared table:
   * owner 0: `ctx[0x74]` (`char_race`) = `table670[kind*9 + actor[0x4a4]]`;
   * owner 1: `FUN_39566e30` copies `table_d0c[kind*10 + desc+0xa0]` to `npc_template`,
     `table_1464[kind*16 + desc+0x94]` to `npc_kind`, `table_2f90[kind*9 + desc+0x8c]` to
     `npc_grade`, with fallback constants;
   * owner 5: `ctx[0x80]` (`mate_kind`) = `table_2024[kind*21 + mate_desc+0x74]`;
   * owner 4 (transfer) has **no** binding and no DB formula uses `mate_kind` for it.
5. Fetch the node at `ctx+0x14c[(kind*7 + actor[0x408])*4]`. If non-null and its type < 24, call
   the node evaluator and return; otherwise return 0.

The static mirrors use the same tables directly (e.g. `FUN_39567110` stores the race coefficient
in slot 22 before evaluating).

### 3.7 Generic formulas (`formulas`, ids 2..27)

`FUN_39568220` parses rows 2..27 into `ctx+0xdc[id*4]` using the same parser **[R]**.
Static mirrors are the switch in `FUN_398ac290(ctx, id)` (1369 B); its inputs are the 35 variable
slots and its `.rdata` constants. The static function is **not** a reimplementation of some other
rule: it is a hand-written mirror of the DB text (the startup verifier compares it with the parsed
one) **[R]**. Ids and DB text are listed in §7.

Accessors:

* `FUN_39566eb0(ctx, id)` — returns `ctx+0xdc[id*4]` evaluated, or `FUN_398ac290(ctx,id)` when the
  verifier has temporarily set `ctx[0]=1` **[R]**.
* `FUN_39566f10(ctx,a,b,c)` / `FUN_39566fa0` — write `source_level`, `target_level`, `stealth_level`
  (slots 3/4/5) then evaluate formula ids 4 and 10.
* `FUN_39567030(ctx, v)` writes slot 9 `experience`, evaluates id 6;
  `FUN_395670a0(ctx, v)` writes slot 7 `pc_level`, evaluates id 7.
* `FUN_39567350` / `395673c0` / `39567430` / `395674a0` / `39567510` write slot 20
  `npc_size_avg` and evaluate ids 14/15/16/17/18 (NPC size formulas).
* `FUN_395672d0(ctx, a, actor, value)` writes slots 6/7/8 (npc_level/pc_level/range) and evaluates
  id 5.

## 4. Database contents (server copy) **[R]**

### 4.1 `formulas` (26 rows, ids 2..27)

Listed in §7. Only `min`, `clamp`, `floor`, `if_negative`, `if_positive`, `abs` and the operators
`+ - * / ^` occur. The client parser additionally supports `max` and `if_zero`, unused here.

### 4.2 `unit_formulas` (294 rows = 42 kinds x 7 owners)

Kinds present: 1,2,5,6,7,10..46 (42 rows), each with owners 0..6. The kind numbers are exactly
the **`UnitFormulaKind`** enum in the server port (`crates/aaemu-game/src/data/formulas.rs:148`):
1 MeleeCritical, 2 MeleeAntiMiss, 5 MeleeParry, 6 RangedCritical, 7 RangedAntiMiss,
10 SpellCritical, 11 SpellAntiMiss, 12 LevelDps, 13 LevelMana, 14 MaxHealth, 15 MaxMana,
16 HealthRegen, 17 ManaRegen, 18 Armor, 19 MagicResist, 20 Facet, 21 MeleeDpsInc,
22 Str, 23 Dex, 24 Sta, 25 Int, 26 Spi, 27 Fai, 28 RangedDpsInc, 29 SpellDpsInc,
30 CastingTolerance, 31 PersistentHealthRegen, 32 PersistentManaRegen, 33 BaseMissPercent,
34 KillExp, 35 MeleeCriticalBonus, 36 RangedCriticalBonus, 37 SpellCriticalBonus,
38 RangedParry, 39 IncomingMeleeDamageAdd, 40 IncomingRangedDamageAdd,
41 IncomingSpellDamageAdd, 42 HealCritical, 43 HealDpsInc, 44 HealCriticalBonus,
45 Block, 46 Dodge.

Owner 0 formulas (character) are summarised in the static dispatch table of §5; the other owners
are in the DB. The
owner-0 attribute formulas are all
`10 + (level-1)*2 + if_negative(level-51, 0, (level-50)*8)` (kinds 22..27, Fai lacks the high-level
term) **[R]**, matching the static code `FUN_398adda0` etc. (§4.3).

### 4.3 `unit_formula_variables` (2478 rows)

| kind | rows | key range | note |
|---|---|---|---|
| 1 | 378 | 0..8 | char race coefficient |
| 2 | 378 | 1..9 | npc template coefficient |
| 3 | 546 | 1..15 (13 distinct: 6,7 missing) | npc kind coefficient |
| 4 | 336 | 1..8 | npc grade coefficient |
| 5 | 840 | 1..20 | mate kind coefficient |

The client's loader expects exactly the key sets shown in §3.5 and uses `value` as f32.

### 4.4 `wearable_formulas` (2 rows)

| id | kind | text |
|---|---|---|
| 1 | 0 (max base armor) | `floor(item_level ^ 1.1 * 45 + 100) * item_grade` |
| 2 | 1 (max base magic resistance) | `floor(item_level ^ 1.1 * 45 + 100) * item_grade` |

Server names: `WearableFormulaType::MaxBaseArmor` / `MaxBaseMagicResistance`
(`data/formulas.rs:206`).

### 4.5 `constants` (11 rows, ids 1..11)

`20, 0.6, 50, 30, 9, 12, 1, 100, 20, 10, 30`. `FUN_395681c0` stores them at `ctx+0xac+id*4`; no
reader of that block was found in the decompile beyond the loader [?]. Formula text does not use
them; the static mirrors use `.rdata` constants. Treat as data the port should load but may not
need for formulas.

### 4.6 `unit_attribute_limits` (client clamp table)

`FUN_395729f0(stat, value)` clamps against a table at `PTR_DAT_39acb0e8 + stat*8 + 0x101ac`
(min,max int). The DB table `unit_attribute_limits` has 21 rows (e.g. attribute 74/GCD
`-800..4000`, attribute 33 `0..10000000`, attribute 8 `0..1000000`). The server does **not** read
this table (no Rust reference) **[R]**, so a shared stat port must add the clamp.

## 5. Static unit formula dispatch (`FUN_398af1e0`, 1966 B) **[R]**

`FUN_398af1e0(ctx, kind, actor)` switches on `kind` 1..46 (jump table `0x398af990`). Case 0 is not
used. The table below is the **owner-0 path**; owner-specific branches are inside the leaf.

| kind(s) | leaf function(s) | DB owner-0 text (summary) |
|---|---|---|
| 1 | `FUN_398ac930` -> attribute | `dex*10*100` |
| 2 | `FUN_398aacc0` + owner branches | `((str+spi)/2+30)*9*100` |
| 5,6,7 | `FUN_398aaf80`.., `398ab220`.., `398ab4c0`.. per owner | crit/parry/anti-miss ratings |
| 10,11 | `FUN_398aca70`, `FUN_398ab780`.. | crit ratings |
| 12 | `FUN_398acca0` | `(ab_level*1.6+8)*3` |
| 13 | `FUN_398aba40` | `level_dps/3.65` |
| 14 | `FUN_398ad0b0` | `level*150+sta*12+100` |
| 15 | `FUN_398ad470` | `level*100+int*12+100` |
| 16 | `FUN_398abad0` | `spi*0.2+50` |
| 17 | `FUN_398abbb0` | `spi*0.2+15` |
| 18 | `FUN_398ad580` (NPC) / constant (owner 6) | `0` for character; NPC armor formula |
| 19 | `FUN_398ad800` | `spi*1` |
| 20 | `FUN_398ad890` | `((level^1.3+level*3+17)*100)*100` |
| 21 | `FUN_398adc70` | `str*200` |
| 22..27 | `FUN_398adda0`, `adf50`, `ae100`, `ae2b0`, `ae460`, `abc90` | attributes (§4.2) |
| 28,29 | `FUN_398ae7f0`, `FUN_398aeae0` | `dex*200`, `int*200` |
| 30 | constant `100.0` (`0x3999d538`) | `100` |
| 31,32 | `FUN_398abe00`, `FUN_398abe70` | `0` |
| 33 | `FUN_398aeb50` | `clamp(10+10*(level_diff-4)/16,10,20)` |
| 34 | `FUN_398aec10` (char) / `398aec90` (npc) | kill exp |
| 35,36,37,44 | constant `1500.0` (`0x39a206c0`) | `1500` |
| 39,40,41 | `FUN_398aed90`, `aee10`, `aee90` | `-((armor*5/(armor+300))^1.8)*5` (resp. magic resist) |
| 42 | `FUN_398aef80` | `(spi*10)*100` |
| 43 | `FUN_398af170` | `spi*200` |
| 45 | `FUN_398abf40` | `(str*6)*100` |
| 46 | `FUN_398abff0`..`ac230` per owner | `((dex+int)/2*8)*100` |
| 38 (`RangedParry`) | **no case** -> 0 | `0` |

All 42 kinds are evaluated server-side through the same `UnitFormulaKind` enum
(`data/formulas.rs:148`) in `models/stats.rs`, `systems/skills/stats.rs`, `systems/npcs/npc/stats.rs`
and `systems/vehicles/stats.rs`; the DB text is the shared contract. The client leaves below are
the static oracle only.

The five attribute leaves for owner 0 compute, in f32:
`x = (level - 1.0)*2.0 + 10.0; if (level >= 51) x += (level - 50.0)*8.0`
(disassembly of `FUN_398adda0` at `0x398addb9`..`0x398ade05`; the other five are byte-identical in
this path) **[R]**. `FUN_398ae460`/`398abc90` add owner-type branches but the character path is the
same.

Owner dispatch in `FUN_398b4e80` (the per-stat getter) is separate and is covered in
[`structs.md`](structs.md) and `MAP.md` §3.

## 6. Numeric conversion and rounding

* Formula nodes hold f32. Arithmetic uses x87 (f80) intermediates and narrows; `pow` uses
  `_CIpow`, `log10` uses `log2 * 0.3010299956639812`, `floor` converts to double first **[R]**.
* `FUN_398b56d0(x87 ST0)` converts to `int`. In the shipped dump `DAT_3a5e41f0 = 1`, so it takes
  the **truncate-toward-zero** branch (`(int)ST0`); if a runtime write clears that global it uses
  round-to-nearest with a correction. No writer was found in the decompile **[R]/[?]**.
* Attribute/stat results are integer (`FUN_398b56d0` or C `(int)` casts); tooltip values often stay
  f32 and are formatted by Lua.

## 7. The 26 generic formulas (ids 2..27) **[R]**

| id | DB text |
|---|---|
| 2 | `0 * (100 / casting_tolerance) * (1 + (1 * floor(damage_percent / 10)))` |
| 3 | `300 * (100 / casting_tolerance) * (0.42 * floor(damage_percent / 5))` |
| 4 | `min(40, 12 * (1 + (source_level - target_level) / 50) / (1 + stealth_level / 50))` |
| 5 | `clamp(1 + (npc_level - pc_level) * 0.03, 0.5, 1) * range` |
| 6 | `0` |
| 7 | `if_negative(pc_level - 51, pc_level, 50 + (pc_level - 50) * 20)` |
| 8 | `experience / 20` |
| 9 | `experience * 4 / 100` |
| 10 | `0.5` |
| 11 | ` if_negative(range - 1, 1, 1.05 + (min(range, 100)/100) )` |
| 12 | `if_negative(range - 8, 0.8, 1)` |
| 13 | `(ab_level ^ 2) / 15 + 30` |
| 14 | `clamp(npc_size_avg, 0.1, 10.0) * 13.0 + 15.0` |
| 15 | `(clamp(npc_size_avg, 0.1, 10.0) * 13.0 + 15.0) / 2.0` |
| 16 | `clamp(npc_size_avg, 0.1, 10.0) * 5.0` |
| 17 | `clamp(npc_size_avg, 0.1, 10.0) * 7.0` |
| 18 | `clamp(npc_size_avg, 0.1, 10.0) * 10.0` |
| 19 | `(( ( pc_level * 4.5 ) + 37.5 ) / 5) * labor_power ` |
| 20 | `if_negative(pc_level - npc_level, 1, 1 + (pc_level - npc_level) * 0.04)` |
| 21 | `1` |
| 22 | `(((item_level / 3) ^ 5) * (item_grade ^ 3 / 50000) + (item_level * 5000) + (item_grade * 5000)) * (equip_slot_enchant_cost / 10)` |
| 23 | `battle_resist / ( battle_resist + 8000 )` |
| 24 | `bulls_eye * 105` |
| 25 | `flexibility * 90` |
| 26 | `flexibility / 10` |
| 27 | `16000 + 1400 * pc_level^( ( pc_level - 49 ) / 6 )` |

Semantic names (server `FormulaKind`, `data/formulas.rs:216`) are:
2 CastingCancelPercent, 3 CastingDelayTime, 4 DetectStealthRangeFront, 5 NpcPerceptionRangeMod,
6 MoneyForRecoverExp, 7 LaborPowerForRecoverExp, 8 PenaltyExp, 9 RecoverableExp,
10 DetectStealthRangeBack, 11 DamageMultiplierByHeight, 12 DamageMultiplierByRange,
13 ArmorGradeBuffABLevel, 14..18 NPC fly pass/dive/quick-turn distances and heights,
19 ExpByLaborPower, 20 DamageBonusMultiplierByLevel, 21 DamagePenaltyMultiplierByLevel,
22 GradeEnchantCost, 23 DamageReduceRadioByBattleResist, 24 FacetsForBullsEye,
25 FlexibilityRatio, 26 FlexibilityBonus, 27 ExpBySkillEffect **[R]**. The 1.2 table stops at
27; ids 28+ in the server enum are later-version additions.
