# Unknowns / open questions

Each item says what is missing, why it matters, and the cheapest way to settle it (usually one
driver call in the hidden client).

1. **Client DB text vs server copy.** The client loads its own encrypted `game_pak` DB; this map
   read the plain server copy (`compact.sqlite3`). They are presumably the same 1.2 revision, but a
   different DB build would change every formula. Check: dump a few rows (`formulas` id 14/19/22,
   `unit_formulas` kind 14 owner 0, `wearable_formulas`) from the client's DB or compare a live
   stat with the server row. Evidence: `FUN_39894600..39894b00` SQL strings match the server table
   names.

2. **Identifier/function name case sensitivity.** The parser compares names with
   `XlCompareString` (imported from `XLCOMMON.DLL`, implementation not decompiled). It may be
   case-sensitive or insensitive; the DB text is lowercase. Test: parse `MIN(1,2)`, `Min(1,2)`,
   `Level + 1` in the client and compare with the Rust parser. Also check the C runtime locale
   used by `isalpha`/`isdigit`/`isspace` (`398afbf0` etc.), which affects non-ASCII bytes.

3. **NaN / signed-zero semantics.** `min`/`max` (`39567950/e0`), `clamp` (`39567bd0`),
   `if_zero/positive/negative` and the `0.0 < x` comparisons have no explicit NaN rule. Generate
   NaN via `0/0`, `sqrt(-1)`, `log(-1)` and compare. `min`/`max` may not match the server's
   Jace/LINQ rules (`max` skips NaN, `min` propagates) — do not assume.

4. **`constants` table consumer.** `FUN_395681c0` stores ids 1..11 at `ctx+0xac`, but no reader of
   that block was found. Maybe it is used by the game-expression/table-path system or dead data.
   Search the client for reads of `0x39ad7180+0xac` or test whether any formula/stat changes when
   a constant is edited (not possible without DB access). If unused, the port can load and ignore.

5. **Static formula arithmetic hidden by register returns.** `FUN_398af1e0`'s leaves return via
   XMM0/ST0 and some branches are difficult to read in the decompiler (e.g. MaxHealth `398ad0b0`
   calls stat 2 then adds the level term). The dynamic DB text is the default path, so the port can
   ignore the static mirrors for behaviour; if the startup verifier is to be reproduced, disassemble
   each leaf fully or use the DB text as the definition and the static function as a checker.

6. **`expr_mode` semantics.** Registration default is `"dynamic"` (`FUN_390f7b40` line 196098)
   and the initializer sets the mode byte to 0. `static`/`safe_static` only trigger the verifier
   (`FUN_3956ae80`), which temporarily toggles the byte to 1 and restores 0; production evaluation
   stays dynamic. Verify with a live `expr_mode=static` run (the verifier logs and the values should
   not change) and check whether any code path sets the byte to 1 outside the verifier.

7. **The fifth item formula vector (`ctx+0x365c`).** Four vectors are used for holdable
   `dps/mdps/hdps/armor`; `FUN_395682e0` clears five. The fifth descriptor's consumer was not
   found. Likely unused in 1.2 or a later item class. Check `395699b0`'s vector sizes and the
   `395684xx` selectors; mark dead if no reader.

8. **`wearable_formulas` two paths.** They are loaded at game-DB `+0x2dd4/+0x2dd8` by
   `FUN_396c7cc0` and again as descs via `FUN_39695b30(0/1)` into `ctx+0x367c` by `FUN_395699b0`.
   The tooltip uses the second path. Confirm the first path is only the source for the desc or is
   also read by item creation; `grep` found no literal reader besides the loader (the giant
   `39614d00` hit is a constructor zeroing the field).

9. **UnitInfo slots 47/48/52.** `FUN_398aa290` computes 61 f32 fields but `394bdc10` registers 58
   keys; array slots 0x2f (MaxHealth), 0x30 (MaxMana) and 0x34 (incoming base) are not exposed by
   that table (max health/mana are separate natives). Confirm no other UI consumer and whether a
   later Lua version reads them.

10. **Key -> leaf mapping in `3941f0d0`.** The else-if chain is deeply nested; the damage/heal/
    mana keys and their leaves are listed in `tooltips.md`, but a few (`charge_multiplier`,
    `return_point`, `area_radius`, some `buff_*` variants) are marked [?]. A small script that
    evaluates each placeholder against a live skill would complete the table.

11. **`ua_*` name -> attribute id.** `FUN_3941a700` calls `FUN_3956e310` to resolve the suffix;
    the exact name table and the percentage special-case list in `39419da0/e00` are not fully
    enumerated. Test all `ua_` names used by the UI scripts.

12. **`FUN_398b56d0` rounding mode.** The dump has `DAT_3a5e41f0=1` (truncate), but a runtime write
    could switch it to round-to-nearest. Check the live global; if it changes, the stat values
    change by 1. No writer was found in the decompile.

13. **`unit_attribute_limits` clamp.** The client clamps via `FUN_395729f0`; the server does not.
    Confirm the 21 rows are the client's table (min/max pairs) and add the clamp to the shared
    stat port. Also check whether the clamp applies before or after the XOR cache (it is applied
    before the final store in `398a0bc0`).

14. **Item/socket chance consumer.** `item_socket_chances` is loaded (`395b2d60`) but the 1.2 UI
    consumer was not isolated; the socket tooltip shows `socketInfo/maxSocket` only. Find the
    native that returns a percentage or confirm it is server-only in 1.2.

15. **Physical enchant UI.** `physical_enchant_abilities` is loaded (`396ce260`) but the consumer
    is not mapped; the server has no equivalent. Check the NPC enchant window scripts.

16. **Craft `needed_lp` arithmetic.** `FUN_397a9420`/`FUN_397a8dd0` are decompiler-hostile
    (hidden args). The client display may include actability/house bonuses that the server's
    `consume_labor_power` does not. Recover with disassembly or compare a live craft window with
    the DB `consume_labor_power`.

17. **Damage/heal tooltip scaling.** `3941b030`/`3941a460` read effect fields (`+0x2c`, `+0x30`,
    `+0x48`, `+0x4c`, `+0x50`, `+0x54`) whose names are inferred from usage. Confirm the effect
    desc layout with the DB (`skill_effects`/`buff_effects` columns) before porting the leaves
    line by line.

18. **Static mirror coverage for `formulas`.** `FUN_398ac290` has cases 2..27; case 6 returns 0 and
    the fallthrough handles id 7. The startup verifier covers unit formulas (owner 0..6) but does
    it also verify all generic formula ids? `FUN_3956ae80` shows only unit-formula loops; if
    generic formulas are not verified, a DB/client difference would only show in gameplay.

19. **Gearscore.** Confirmed absent in 1.2 (`grep -i gearscore` on the 1.2 `x2game` strings
    returns nothing; 10.x has the strings). If a later task expects a gearscore UI, it belongs to
    a later client version, not this port.

20. **`FORMULA` copy in the client's package.** The client also has a `desc package` reload path
    (`FUN_3956a880`) for hot-reloaded formula packages; the port can ignore it unless the client
    ships multiple packages.
