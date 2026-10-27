# Differential-test plan (original client vs Rust port)

The in-client driver `aaemu-clientdriver` already runs Lua on the client main thread and can call
raw 32-bit functions with `cdecl`, `stdcall`, `thiscall` or `fastcall`, pass `u32`/`f32`/pointer
arguments, allocate scratch memory, read/write memory and dump buffers after the call
(`rust/crates/aaemu-clientdriver/src/lib.rs`, `Call::parse_spec` at lines 128-190, request ops at
lines 334-394; usage in `rust/docs/client/LUA-DRIVER.md`). That is enough for every entry point
below. Use `x2game+<RVA>` targets (RVA = VA - `0x38ff0000`); raw VAs are also accepted.

## 0. Rules

* Run calls on the client main thread (the driver already queues them between frames).
* Allocate a scratch buffer per test (`Alloc`, helper `alloc_bytes`/`alloc_f32s`). Never write the
  live global formula object `0x39ad7180` unless the test is explicitly about production state;
  instead pass a scratch context to the parser/static evaluators.
* Restore `expr_mode` and any cvar you change. The default is `"dynamic"`.
* Do not call the hot-reload function on a live session. Do not change `DAT_3a5e41f0`.
* Free scratch buffers (`D`/`free`) between cases; the client's allocator is the game CRT.
* Each case is `(function, inputs) -> (EAX, ST0, dump bytes)`. The Rust port must compare bit
  patterns for ints and `f32` (not `==` with tolerance) unless the case is intentionally
  approximate.

## 1. Parser + evaluator (first, pure)

RVA: `398b0b20 -> 0x8c0b20`.

```
scratch = alloc(0x400)            # zeroed
text    = alloc_bytes("min(3, 5) + level * 2")
r = call(target="x2game+0x8c0b20", conv=cdecl,
         args=[u32(scratch + 0x0c), u32(text)])   # EAX = node*
node = r.eax
type = read_u32(node + 4)
```

* `398b0b20` is the production entry: it passes `scratch+0x0c` as the parser state; the node's
  `[0]` points there, so variable slots are at `scratch+0x1c+idx*4` (same as production
  `global+0x1c`). Fill them with `write_f32s`.
* Read the node type at `node+4`; call the evaluator by type from `0x3a5bd4c0[type]` with
  `conv=cdecl, args=[u32(node)]`, `capture_st0=true`. Compare ST0 and the node tree (dump
  `node:0x14`, operands recursively).
* Error cases: empty text, `min(1)`, `clamp(1,2)`, `foo(1)` (unknown function), `1/0`, a missing
  variable, `1.2.3`, a non-ASCII byte. The original logs to `X2Log`/`archeage.log` and returns 0
  or a node with a logged error; compare the return value and the log line.

Coverage matrix (one live case each; then generated grammar combinations):

| case | expected path |
|---|---|
| `0`, `-1`, `1.5`, `-1e2` | number/lexer, unary minus |
| `1+2*3`, `2^3^2`, `-2^2`, `2^-3^2` | precedence/associativity |
| `min(1,2)`, `max(1,2)`, `clamp(-1,0,10)`, `clamp(11,0,10)` | builtins |
| `if_negative(-1,7,8)`, `if_positive(0,7,8)`, `if_zero(0,7,8)` | conditional nodes |
| `floor(2.9)`, `log(100)`, `sqrt(2)`, `abs(-3)` | library nodes |
| `str + level`, `pi` (unknown) | variable slots |
| `1/0`, `1 2 +`, `((1)`, `a.b` | errors / lenient parses |

The parser is deterministic; the evaluator is pure for a fixed context. These cases need no actor.

## 2. Static generic formulas (`FUN_398ac290`)

RVA: `0x8bc290`. Fill a scratch context at `scratch` with the 35 variables
(`scratch+0x1c+idx*4`, table in `formulas.md` §3.4), call `cdecl` with
`args=[u32(scratch), f32(id)]` (or `u32` bit pattern), capture ST0. Compare every id 2..27 with
several variable sets, including:
* `casting_tolerance` 0/1/100/-1,
* `source_level == target_level`, ±1, ±50,
* `range` 0/1/8/100/1e6,
* `npc_size_avg` 0.1/1/10/11,
* `labor_power` 0/1/100,
* `item_level` 0/1/50/100, `item_grade` 0/1/10,
* `battle_resist` 0/8000/1e9,
* `flexibility` 0/0.1/10.

The static formula's f32 truncation/rounding is part of the contract; use the exact ST0 bits.

## 3. Unit formula dynamic path (`FUN_39567110` + parser)

For kinds that do not need the stat manager (owner 0, kinds 22..27), a scratch context and a
scratch "actor" are enough:

```
ctx   = alloc(0x400)   # *ctx = 0 (dynamic)
actor = alloc(0x800)
write_u8 (actor+0x4ac, level)     # u8 level
write_u32(actor+0x408, 0)         # owner character
write_u32(actor+0x4a4, race)      # char_race index
call 39567110 cdecl ctx, kind, actor   # RVA 0x577110
```

But the dynamic branch also reads parsed nodes from `ctx+0x14c`; a bare scratch context has none.
Two options:

1. **Static mode**: set `*ctx=1` and call `398af1e0(ctx, kind, actor)` (RVA `0x8bf1e0`). This is
   pure apart from the stat-recursion for kind 6+ and reads only `actor+0x4ac` for attributes.
   Compare all six attributes and the simple kinds (1,2,5,6,7,10..46 where no stat recursion is
   needed).
2. **Production dynamic context**: call `39567110` with the live global context
   (`0x39ad7180`) and the live local actor. This exercises the exact DB text. Snapshot/restore the
   global context's variable slots before/after if the test runs while the UI is idle.

For kinds 6+ (MaxHealth/MaxMana/etc.) the static leaves call `398a0bc0`, which requires a real
stat manager. Use the live local unit (`unit+0x1aa8`, actor=`unit+8`) and compare against the
in-game character window (`unit:UnitInfo()`), not a synthetic actor.

## 4. Stat getter (`FUN_398a0bc0`)

RVA `0x8b0bc0`, `thiscall this=unit+0x1aa8 (manager)`, stack args
`(global_ctx, statIndex, force)`:

```
r = call(target="x2game+0x8b0bc0", conv=thiscall, this=unit+0x1aa8,
         args=[u32(0x39ad7180), u32(stat), u32(0)])
eax = r.eax
```

* Get `unit` from the live local player: the game global `DAT_39b02294+0x10c` is the local actor
  pointer; `unit = actor - 8` (see `structs.md`). Add a small Lua/native helper or read the global
  through the driver's `ReadMemory`/`peek global` if the driver exposes it.
* Compare all `UnitAttribute` ids for one character (level, gear, buffs) against the Rust port.
  The call mutates the cache (`+0xbc0`, `+0xc`, `+0x10`); run it at a quiet moment and do not
  restore the cache (it is self-consistent). Do not force `force=1` unless testing invalidation.
* The same call is the backend of `390b3a80`; a Lua test can compare `unit:UnitInfo()` and
  `unit:UnitModifierInfo()` end-to-end (see §5).

## 5. UnitInfo / character window

Call the Lua directly (no raw C++): `lua -t ui 'return unit:UnitInfo("player")'` (or the actual
binding name used by `character_info.alb`). The driver serialises tables to depth 3. Compare the 58
keys against the Rust `UnitInfo` builder. Raw form: `FUN_398aa290(this=dest, actor, global_ctx)`
(RVA `0x8ba290`) writes 0xf4 bytes; compare those too. This covers the f32 presentation
arithmetic; it does not cover the raw stat manager (use §4 for that).

For the item tooltip, call the Lua item tooltip for a known item and compare the table (or the
formatted text) — the C++ producers are `3941fc20`/`39420310`/`394206d0` and `39421e20`.

## 6. Item formulas (pure leaves)

`FUN_39568560(ctx, selector, holdable_id, grade, ???)` (RVA `0x578560`) and
`FUN_39568610(ctx, selector, slot, grade)` (RVA `0x578610`) need:
* a global/scratch context with the item formula vectors and the item-grade descs initialized
  (easiest: use the live global context after startup; it is read-only for these calls),
* a holdable id / slot and grade.

Call `cdecl` with `[ctx, selector, id, grade, ...]`, capture ST0. Compare the four holdable
formulas and the two wearable formulas for the DB rows and a matrix of item levels/grades. The
`item_grades` desc array at `ctx+0x3684` is what `395684c0/560/610` index; the differential test
should use the same live array, so the case is reproducible with the shipped DB.

`FUN_395684c0(ctx, grade, slot, item_level)` (RVA `0x5784c0`) tests formula 22 (grade enchant
cost). Compare against the DB text and the server `item_effects.rs:650` port.

## 7. Tooltip helpers

`FUN_3941b030` / `3941a460` / `3941a160` / `3941a240` / `3941a300` / `3941a3c0` are effect/desc
readers. For a first pass use real skill/buff instances from the client:
* pick a few skills with one damage effect, one heal effect, one DoT, one HoT;
* read the effect desc pointers from the live skill instance (the layout is in
  `tooltips.md` §4);
* call the helpers with a live caster and level, capture min/max/avg.

For the placeholder chain, the Lua tooltip call is the cleanest end-to-end check: render the
tooltip text for a chosen skill/buff and compare it with the Rust formatter. The C++ leaves are
small and can be unit-tested directly once the desc layout is trusted.

## 8. Enchant / socket / specialty / craft

* `GetRatioInfos` is a Lua call; compare the returned table (`success`, `break`, `downgrade`,
  `*_var_ratio`) with the Rust chance composition (`390450b0`). Use real items at several grades
  and with/without a support item.
* `GetSpecialtyRatio` returns a cached packet value; test the **price** path through
  `GetSpecialtyRatioRefund`/`SellBackPackGoods` with a live backpack and NPC, or compare
  `FUN_398a9bf0`/`398a9cb0` directly with a synthetic `item_backpacks`/`specialties` desc.
* Crafting: `39461860`/`39462a70` need a real recipe/doodad; use the crafting window Lua and
  compare `needed_lp`, `consume_lp`, `success_rate`, `amount`.
* Fall damage is already covered by the movement oracle (`port-movement-extra/difftest.md` or the
  physics scenarios); do not duplicate.

## 9. Suggested first goldens (CI without the original)

1. Parser/evaluator vectors (no client): all node types, precedence, functions, error cases.
2. `398ac290` static formula ids 2..27 for a 30-row parameter matrix (values captured once).
3. Attribute leaves (`398adda0`..`398abc90`) for levels 1..100.
4. UnitInfo builder (`398aa290`) for a synthetic stat getter (Rust-only after the stat port).
5. Item formulas (`39568560`/`610`) for holdable ids 0..26, grades 0..11, item levels 1..100.
6. Chance composition (`390450b0` logic) for base/support ratios.

The live client is only needed to create the goldens; the Rust CI compares against the checked-in
values.
