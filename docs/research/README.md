# Reverse-engineering specs (`docs/research/re-*.md`)

A `re-<area>.md` file is the reverse-engineering spec for one parity area: what the original ArcheAge 1.2
client does, with evidence, and where the Bevy client still differs. It is not a task list and not a
wish list; the ordered work queue is `remaining.md` and the completion ledger is
`../aa-client-parity.md`.

## Required structure

1. **Scope and sources** - the actor/model/UI/system covered; the exact binaries, tables and assets.
2. **The rules** - state machines, selection rules, ordering, error/fallback behaviour, packet flows.
3. **Numeric parameters** - DB columns and values, engine constants, attach points, speeds, angles,
   timings, opcodes and field layouts.
4. **What is inferred** - mark every claim: `[R]` read in code/data, `[I]` inferred, `[?]` not found.
   Keep the inferred list explicit; it is the next RE work.
5. **Gaps vs Bevy** - `crate/path.rs:line` for the current implementation and the concrete difference.
6. **Open questions** - what a live capture or one more decompile would settle.
7. **Live oracle measurements** - date, packet logs/screenshots, what was measured, what still was not.

Citations must be checkable: a VA for a client function (1.2 `x2game` image base `0x38FF0000`),
`file:line` for Lua/XML/prefab data, a `table`/`row` for `compact.sqlite3`, or a `game_pak` path.
Name the Ghidra project, the log or the screenshot set (`tmp/oracle/...`, `reference/...`).

## Where the RE sources live

`C:/Users/stsiakal/AAEmu/REVERSING.md` is the server repo's map of everything below (clients, tools,
Ghidra projects, the physics oracle, rules). Start there.

* Original client (read-only): `D:\AA1.2`; `game_pak` via `aa-pak` (`tools/aa-pak.exe`).
* 1.2 logic oracle: Ghidra project `C:\AAEMU_DISK\aa-re\client12\ghidra\proj-aa12u`
  (`x2game_unpacked.dll`, `crynetwork_unpacked.dll`; `.data` is absent, so data globals are read from a
  running client or resolved by reference).
* Databases: `D:\aaemu-live\rs-game\Data\compact.sqlite3` (plain, read-only; the client's own copy is
  encrypted).
* UI/Lua: `data/extract/game/scriptsbin/` (`.alb` bytecode, `toc.g`) and `tools/aa-pak.exe cat`.
* Reference captures: `reference/<screen>/`; oracle captures/logs: cite the job `tmp/oracle/` path.
* Physics: `C:/Users/stsiakal/AAEmu/rust/crates/aaemu-physics` (bit-exact port, `physoracle.exe`
  scenarios and goldens) - use it instead of re-deriving CryPhysics by hand.
* 10.x client/zone binaries and decompiles: `D:\aa-re\zone\` (reference only; check 1.2 first).

## Current specs

| Spec | Area | State |
|---|---|---|
| `re-placement.md` | House/farm placement builder, construction windows, packets | gaps 1-7, 9-12 done; gap 8 open; oracle-measured once |
| `re-mounts.md` | Mounts: summon/mount/dismount, attach, gait inputs, packets | gaps 1-6, 8 done; 7, 9 open; oracle-measured once |
| `re-mounts-full.md` | Horse + rider graphs, rider coupling, speeds, movement code | spec complete; inferences listed |
| `re-wagons.md` | Farm Wagon: model, boarding, driving, cargo, packets | gaps 1-3, 5, 7-8 done; 4 partly; 6, 9-10 open; oracle-measured once |
| `re-unit-colors.md` | Name plates, target frame and ring colours by relation | implemented; retail capture only, no live side-by-side |
| `re-movement.md` | Walk/run/strafe, jump/fall, ladders, camera, collision | gaps 1, 3-5 done; 2, 6-10 open |
| `re-housing.md` | Decoration, maintain/sale/tax/rename, farm windows | gaps 1-5 done; 6-12 open |
| `re-combat.md` | Combat flow, targeting, action bar, combat text, resurrection | gaps 1-5 done; 6-9 open |
| `re-inventory.md` | Bag/bank, item security, tooltips, equipment getters | gap 1 done; 2-11 open |
| `re-quests.md` | Quest marks, talk/timer, NPC services, map | gaps 1-4 done; 5-10 open |
| `re-transport.md` | Ships, gliders, transfers, siege weapons, packets | gaps 1, 2, 4 done; 3, 5-13 open |
| `re-crafting.md` | Crafting, gathering, workbenches, trade packs | gaps 1-5 done; 6-10 open |
| `re-social.md` | Chat, party/raid, friends/family/guild, mail/trade | gap 1 done; 2-9 open |
| `re-uitoolkit.md` | Options, key bindings, UI data, toolkit | gaps 1-3 done; 4-8 open |
| `re-equipment.md` | Costume slot clearing, attachment rotation, heel use | narrow note |
| `re-skills.md` | Skill controllers, knockdown, effect lookup, beams, stealth | narrow note |

All specs above except `re-equipment.md` and `re-skills.md` are end-to-end area specs; those two are narrow
notes. A parity row's RE spec is only `done` when an area spec covers the whole row.
