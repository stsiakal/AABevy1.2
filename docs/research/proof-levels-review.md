# Proof-level rows for manual review (generated)

> **Restored 2026-10-27 from `archive/fork-main-2026-10-27`, UNVERIFIED.** The client was restarted from NickMesser's `main`; nothing described here is wired into the game. Addresses, call chains and offsets must be re-checked against the decompile ([R]/[I]/[?]) before use. A 2026-10-27 audit found 151 of the 269 EXACT/TOLERANCE labels in `proof-levels.*` weak or wrong (report: `D:\aaemu-client\handoff\2026-10-27\audit\report-proofs.md`).

20 of 839 rows need a decision. Review each against the test source; fix the script's heuristic rather than the row.

The other 352 non-EXACT rows are **reference-only**: addresses of original functions that a port comment names but that this tree does not implement. They are classified `REFERENCED`, not unproven port code; the CSV/table still lists each one with its file and line.

## golden-only address: no ported source names it (2)

| slice | address | level | Rust function | source | goldens |
|---|---|---|---|---|---|
| interaction | `393f8290` | GOLDEN-ONLY |  |  | stubs.json |
| interaction | `3a59f1c0` | GOLDEN-ONLY |  |  | handlers.json |

## live capture exists but no test loads it (kept DECOMPILE) (18)

| slice | address | level | Rust function | source | goldens |
|---|---|---|---|---|---|
| interaction | `390f3780` | DECOMPILE+UNWIRED | default_interaction (crates/aa-client/src/port/interaction/click.rs:74) | crates/aa-client/src/port/interaction/click.rs:74 | click_live.json |
| interaction | `390f4960` | DECOMPILE | click (crates/aa-client/src/port/interaction/click.rs:187) | crates/aa-client/src/port/interaction/click.rs:187 | click_live.json |
| interaction | `393b25a0` | DECOMPILE+UNWIRED | refresh_actions (crates/aa-client/src/port/interaction/bar.rs:102) | crates/aa-client/src/port/interaction/bar.rs:102 | barlive.json |
| interaction | `393b2760` | DECOMPILE+UNWIRED | update (crates/aa-client/src/port/interaction/bar.rs:118) | crates/aa-client/src/port/interaction/bar.rs:118 | barlive.json |
| interaction | `393b2c70` | DECOMPILE | rate_limit_ok (crates/aa-client/src/port/interaction/npc.rs:49) | crates/aa-client/src/port/interaction/npc.rs:49 | npc_rate.json |
| interaction | `393bc7e0` | DECOMPILE+UNWIRED | bar_loot (crates/aa-client/src/port/interaction/loot.rs:268) | crates/aa-client/src/port/interaction/loot.rs:268 | loot_f.json |
| interaction | `393bc8c0` | DECOMPILE+UNWIRED | source_loot (crates/aa-client/src/port/interaction/loot.rs:294) | crates/aa-client/src/port/interaction/loot.rs:294 | loot_f.json |
| interaction | `393be0e0` | DECOMPILE+UNWIRED | service_access_refusal (crates/aa-client/src/port/interaction/npc.rs:57) | crates/aa-client/src/port/interaction/npc.rs:57 | npc_special.json |
| interaction | `393bed90` | DECOMPILE+UNWIRED | pick_greeting (crates/aa-client/src/port/interaction/npc.rs:290) | crates/aa-client/src/port/interaction/npc.rs:290 | npc_greeting.json |
| interaction | `393bf270` | DECOMPILE+UNWIRED | npc_range_gate (crates/aa-client/src/port/interaction/range.rs:109) | crates/aa-client/src/port/interaction/range.rs:109 | npc_range.json |
| interaction | `393bf500` | DECOMPILE | quest_route (crates/aa-client/src/port/interaction/npc.rs:265) | crates/aa-client/src/port/interaction/npc.rs:265 | npc_quest.json |
| interaction | `393c01d0` | DECOMPILE+UNWIRED | target_gate (crates/aa-client/src/port/interaction/npc.rs:93) | crates/aa-client/src/port/interaction/npc.rs:93 | npc_gates.json |
| interaction | `39428d80` | DECOMPILE+UNWIRED | cursor_ray (crates/aa-client/src/port/interaction/pick.rs:66) | crates/aa-client/src/port/interaction/pick.rs:66 | pick_ray.json |
| interaction | `3942c640` | DECOMPILE+UNWIRED | update_picking (crates/aa-client/src/port/interaction/pick.rs:159) | crates/aa-client/src/port/interaction/pick.rs:159 | pick_update.json |
| interaction | `394f9790` | DECOMPILE+UNWIRED | begin_take (crates/aa-client/src/port/interaction/loot.rs:119); take_request (crates/aa-client/src/port/interaction/loot.rs:208) | crates/aa-client/src/port/interaction/loot.rs:119 | loot_take.json |
| interaction | `394fab30` | DECOMPILE+UNWIRED | close_bag (crates/aa-client/src/port/interaction/loot.rs:222) | crates/aa-client/src/port/interaction/loot.rs:222 | loot_close.json |
| interaction | `394fb010` | DECOMPILE+UNWIRED | open_loot_bag (crates/aa-client/src/port/interaction/loot.rs:173) | crates/aa-client/src/port/interaction/loot.rs:173 | loot_open.json |
| interaction | `394fb160` | DECOMPILE+UNWIRED | open_loot_bag (crates/aa-client/src/port/interaction/loot.rs:173) | crates/aa-client/src/port/interaction/loot.rs:173 | loot_open.json |

