# Combat function inventory (1.2 x2game)

Companion to [`MAP.md`](MAP.md). All addresses are x2game VAs at base `0x38FF0000` unless the
module column says otherwise. Sizes are from `funcs.csv`; `tail` marks code in the unpacked tail
(`0x3a6…`–`0x3ac…`) which `funcs.csv` does not size. `C:` = callers, `E:` = callees (from
`calls.tsv`; resolved where the tail loses the edge). Purity: **pure** = output is a function of
the listed inputs/fields only; **state** = reads/writes live game state (needs a live object or a
captured snapshot).

Evidence markers in the role column: **[R]** read from the decompile, **[I]** inference from names
or call sites, **[?]** open.

## A. Input, use request, prechecks

| addr | size | role | C | E | fields / globals | purity |
|---|---|---|---|---|---|---|
| `0x39209d80` | 380 | **action use front end**: builds the target/caster blocks and calls `FUN_393989e0` with the bound skill id **[L]** | 3920xxxx binding handler | 393989e0, 39360020, 3a6779eb | unit `+0x1aa0`, target block | state |
| `0x393f8d20` | 70 | action name → action index (0x68-entry table `0x39ac8e68`) **[R]** | 393f8d70, 393f8db0, 393f8df0, 393ffc20 | `XlCompareString` | table `0x39ac8e68`; indices used here: 22/23 cycle hostile fwd/back, 24/25 friendly, 39 toggle_force_attack, 40 self_target, 41 round_target, 42 team_target, 44 pet_target, 82 action_bar_button, 84 pet_, 85 mode_ | **pure** |
| `0x394005f0` | 211 | register nine built-in actions (`open_config`…`rotatepitch`) **[R]** | init | 393ffc20 | 0x394005fe table | state (init) |
| `0x39424580` | 19 | analogue value of action index `i` (**[R]** movement doc) | many | 393fd240 | action map | state/read |
| `0x392c4dd0` | 485 | action-bar slot hotkey label refresh; fires event `0x25` with `hotkey_activated` **[R]** | 392c6b90? | 393fd1f0, 393fc4a0, 3927ba00 | slot `+0x340/+0x351/+0x353`, event 0x25 | state |
| `0x39428630` | 223 | arm use-skill targeting mode **2** (skill cast mode) with skill id + target block | indirect (UI) | 393f4500 | `DAT_3a5a4ea0=2`, `DAT_3a5a4eb0`, `_DAT_3a5a4eb8…d0`, `ee4` | state |
| `0x39428710` | 228 | arm mode **2** with a SkillObject-ish extra byte at `param_3[3]` | indirect | 393f4500 | same + `DAT_3a5a4eac` | state |
| `0x39428800` | 148 | arm use-skill targeting mode **3** (unit target mode) | indirect | — | `DAT_3a5a4ea0=3`, `DAT_3a5a4eb0…` | state |
| `0x394288a0` | 267 | arm mode **8** (position/ground target) | indirect | 39360020, 390b3750, 390c9f40 | `DAT_3a5a4ea0=8`, target block | state |
| `0x39428a30` | 368 | arm mode **4** (item / special item target) | indirect | 394e0960, 3961e8f0, 390468b0, 3920fa60 | `DAT_3a5a4ea0=4`, `_DAT_3a5a4eb8…` | state |
| `0x390f50a0` | 384 | `ControlMouseClick`: dispatches click mode `DAT_3a5a4ea0` (0 normal pick, 2/3/8 skill confirm, 4 item, 6 interact) **[R]** | input | 390f4960, 390f4c30, 390f38f0, 390f3b20, 39016fd0, 39017220, 3942fb50, 39348800/70 | `DAT_3a5a4ea0`, `DAT_3a5a4da8==6` | state |
| `0x390f4960` | 715 | `MouseClickControl::Normal`: select target under cursor / clear target (`mouse_clear_targeting`) | 390f50a0 | 39429e40, 393be4e0, 390f3780, 393be030, 393b79a0, 39429d00, 390f37f0 | `setup+0x2d4`, `DAT_3a5a4dbc/dc0`, `DAT_3a5a4dcc` | state |
| `0x390f38f0` | 560 | click confirm for mode 3 with position (`FUN_393989e0`) | 390f50a0 | 394294b0, 3921ac90, 39023b90, 39360020, 393989e0, 39426a10 | `DAT_3a5a4ed8…ef4` | state |
| `0x390f3b20` | 345 | click confirm for mode 8 with hit position | 390f50a0 | 39360020, 3961dd70, 393989e0, 39390900 | same | state |
| `0x390f4c30` | 1129 | click confirm for mode 2 (skill-object positions) | 390f50a0 | 396f6390, 3961dd70, 39007590, 390f3130, 390f3180, 393989e0 | skill desc `+0x1b0` target type | state |
| `0x393989e0` | 657 | **`ClientSkillActionManager::UseSkill`** (client request front-end) **[R]** | 39016dc0, 3905abc0, 390f38f0, 390f3b20, 390f4c30, 39209d80, 393995d0, 39399630, 39399690, 393fe9a0 | 396f6390, 393910e0/90, 393be4e0, 39390f80, 39397f90, 393925c0, 39393920, `thunk 0x39396e20`, 39390900 | unit, target block (32 B), caster block (0xd0 + tail), `DAT_39d1fba4` | state |
| `0x39396e20` | 5 | thunk → `FUN_3ab63fb7` (tail): real start/queue request; args `(manager, &result, unit, skillId, &target, &caster, &object, 1)` | 393989e0 (via call) | tail | result struct `{u8 result, u16 arg, u32 value}` | state |
| `0x3ab63fb7` | tail | real request body (Ghidra emits only an exception fragment); packet + queue side effects **[?]** | thunk | — | — | state |
| `0x3939b8c0` | 1004 | `ClientSkill::Start` (local skill from script/AI): calls StartSkill then allocates the 0x2e0 client skill and arms its timers | 39409d20, 394b7f90 | 3939a810, 3950a1b0, 393911d0, 393e8460, 393e1020, 393eef*, 39898ef0, 39395ed0, 3939a8… | 0x2e0 instance, `+0x2d0/2d4` times, `+0x2da` local flag | state |
| `0x3939a810` | 2036 | **`StartSkill`** from `SCSkillStarted`: resolve caster/target, stop old skill, GCD estimate, create local instance, start events, UI listeners **[R]** | 391d57f0, 3939b010, 3939b8c0 | see MAP §3.1 | desc `+0x1bc,0x9c,0x94,0xf8,0xf9,0x1b0,0x1e8`; unit `+0x3ac4`, `+0x3ad5`; manager cooldown at `unit+0x2c48` | state |
| `0x3939a440` | 967 | create/insert the client skill entry in the manager map (skill id, caster, target, object, cast time) **[R]** | 3939a810 | 39396ca0, 39360020, 393e8460, 393937f0, 393eb7f0, 393e1020, 393eacf0, 39354310, 393eaf60, 393ebc60, 393ed520, 3915afb0, 39396b50, 39399a20, 39399e90, 3939a300 | map at manager `+0x0c` (tree), instance fields | state |
| `0x39390530` | 128 | caster reference validity: switch on `SkillCaster.type` 0/1/2/3/4 → `FUN_393904c0(id)` **[R]** | 3939a810, 3939b010 | 39035110, 39177dc0, 393904c0 | caster `+0,+8,+0x14` | **pure** |
| `0x393904c0` | 107 | unit-id → actor exists? (name lookup) **[I]** | 39390530 ×4 | 39360020 | id | state/read |
| `0x3936c500` | 227 | per-requirement owner/target filter (`unit_reqs` owner_type, value pairing) | 39045300, 39045470, 393a0510 | — | UnitReq `+0,+4` | **pure** |
| `0x393a0510` | 1087 | **requirement dispatcher** (`skills.or_unit_reqs` / `unit_reqs` kinds): iterates desc `+0x1e8..+0x23c` vector, switch(kind) → evaluator **[R]** | indirect (manager vtable) | 3936c500, 3939d3f0, 3939d4c0, 3939ea60, 3939f0d0, 3939ffa0, 3939d670, 3939d8b0, 3939d7b0, 3939d700, 3939d0d0, 3939d150, 3939d8f0, 393a0130, 3939ebb0, 3939d960, 3939ef50, 3939ee20, 3939d9d0, 3939ecf0, 3939f990, 3939dab0, 3939de20, 3939db60, 3939dc20, 3939dd10, 3939ff50, 3939f030, 3939f800, 3939d280, 3939e9c0, 3939e930, 3939dea0 | returns `{u8 result, u16 arg, u32 value}` | **pure** (given data) |
| `0x3939d670` | 140 | req evaluator (caster null / self mismatch → `0x1e`) | 393a0510 | 3930cb60 | — | pure |
| `0x3939d700` | 176 | req evaluator (target type 3 / doodad-like, `0x22` invalid target) | 393a0510, 394e5af0 | 39695dc0, 394df370, 3961e9a0, 397cc350, 397d2cf0 | — | state/read |
| `0x3939d7b0` | 243 | req evaluator (level/class gate; `0x21` no target, `0x2c` not now, `1` failure) | 393a0510 | 3938fbd0, 393c78e0 | unit `+0x4ac` | state |
| `0x3939d8b0` | 56 | req evaluator (bit 9 of a skill/effect field; `0x2e`) | 393a0510 | 394e0960, 3961e830 | — | state |
| `0x3939d8f0` | 110 | req evaluator (`0x21`/`0x22` target checks) | 393a0510 | 3935c8d0 ×2 | — | state |
| `0x3939d960` | 97 | req evaluator (item/level count `0x21` + text `0x29e`) | 393a0510 | 3989a7b0 | — | state |
| `0x3939d9d0` | 222 | req evaluator (target type 3 + npc kind) | 393a0510 | 394e0960, 3961e9a0, 39695dc0, 397cc350 | — | state |
| `0x3939dab0` | 173 | req evaluator (target type 3 + `FUN_397ce340`) | 393a0510 | same | — | state |
| `0x3939db60` | 189 | req evaluator (target npc kind == param) | 393a0510 | same + 397ce3d0 | — | state |
| `0x3939dc20` | 234 | req evaluator (target + `FUN_395a72d0`, `FUN_395ac9c0`) | 393a0510 | same | — | state |
| `0x3939dd10` | 264 | req evaluator (equipped item level vs skill value) | 393a0510 | 39744620, 39360020 | unit `+0x3bcc+4*i` | state |
| `0x3939de20` | 113 | req evaluator (time/cooldown `0x2c`, `0x23`) | 393a0510 | 392f3530, 398391c0, `XlGetCurrentFileTime` | — | state |
| `0x3939dea0` | 135 | req evaluator (force-attack / team gate; msg `0x33f/0x340`) | 393a0510 | 39182a00 | `DAT_39b02294+0x11bd` | state |
| `0x393925c0` | 193 | **target override / selection gate**: if desc `+0xe8` names a skill category currently locked by a unit, replace the target with that unit's stored skill object | 393989e0, 3943f310, 393f61c0 | 396f6390, 39695dc0, 396a6e40, 397cf7a0, 39044360 | unit `+0x4b8 + 0xc0*cat`; desc `+0xe8`, `+0xe4` | state |
| `0x393be4e0` | 645 | `CanAutoAttackTargetUnit` — relation/validity gate for auto-attack | 390f4960, 393989e0, 39393f30? | 393578e0, 39360020, 39028240, 39898e60 | unit `+0x3c26/3c27/3c28`, relation | state |
| `0x39390900` | 1295 | **refusal/UI message builder**: switch on `SkillResult` low byte; resolves name ids; emits event `0x15` and text key via `FUN_393f4890` **[R]** | 30+ sites | 394df580, 39428370, 396f6390, 39182840, 39695dc0, 392f8100, 39856420, 39424280, 393f4890 | result name table `FUN_39897990`; `DAT_3a5a4ea4` | state (UI) |
| `0x39897990` | 955 | `SkillResult` → localized key string (0..0x82) **[R]** | 39390900 | — | strings `399a1f…` | **pure** |
| `0x393f4890` | 151 | `ui_texts` lookup by lower-case name (`Crc32Gen::GetCRC32Lowercase`) **[R]** | 39390900 | — | `ui_texts` DB | state/read |

## B. Cast / channel timeline, cancel, queue, combo

| addr | size | role | C | E | fields / globals | purity |
|---|---|---|---|---|---|---|
| `0x39392010` | 79 | skill UI-event object ctor (vtable `0x399d1c18`); fields `+4..+0x38` | 3939a810, 39398410 | — | event payload | **pure** ctor |
| `0x39202fb0` | 106 | game-event notifier table (`0x48` entries × 0x10) | many | — | `DAT_39d1f4f0` | state (init) |
| `0x39202db0` | 463 | dispatch event `id` to notifier listeners (`id=0x14/0x15/0x17/0x18`) | many | `FUN_3950a1b0` | `DAT_3a5bb1d8` | state |
| `0x3939b010` | 2218 | **`FireSkill`** (`SCSkillFired`): find instance, stop old, set delay/channel, play fire anim, controllers, send event `0x15`, apply local damage/heal paths **[R]** | 391d58a0, 3939bfd0 | 39394e70, 39391da0/1e30, 39397ff0, 3939a810, 39360020, 39395b50, 396f6390, 393938a0, 393f1160, 393e1020, 393eed90/ef50/ee10/ee80, 39368f90, 390b3ab0, 39354670, 39356440, 39397a20, 3939ce90, 39202fb0/db0 | desc `+0x128,0x6c,0x1c0,0x1bc,0xdd,0x1d1,0x140,0xc0,0x13a,0x1e8`; instance `+0x138/0x13c/0x140/0x148/0x154`; unit `+0x410/0x3ac4/0x3ad5` | state |
| `0x39397ff0` | 1050 | **`EndSkill`** (`SCSkillEnded`): clear cast state, stop anims/controllers, run end-skill controllers, send UI event `0x16` **[R]** | 391d5bf0, 39398550, 3939a810, 3939b010, 3939bfd0 | 39394e70, 39202fb0/db0, 394e2650, 396f6390, 39360020, 390b3670, 390b45b0/4610/3ab0, 394278a0, 393f0a00, 393953e0 | unit `+0x3ac4/3ac8/3acc/3ad0` | state |
| `0x39398410` | 311 | **`StopSkill`** for the auto-attack pseudo skill (`DAT_39ac769c`): clears `+0x3ad5`, sends event `0x17` | 391d5c70 | 39360020, 396f6390, 39368f90, 39397a20, 39392010, 39202fb0/db0 | unit `+0x3ad5` | state |
| `0x39396000` | 344 | `SCCastingStopped` impl: stop cast, apply cooldown duration `FUN_39899f90`, send event `0x18` | 391d5cf0, 393966d0/67c0 | 39394e70, 39395b50, 396f6390, 39899f90, 394e2650, 39202fb0/db0 | instance caster/target blocks | state |
| `0x39395760` | 317 | cast-delay: add `duration` to instance `+0x150` total, adjust anim speed `game_vtable+0x16c/0x170`, log `Casting anim speed %f -> %f` | 391d5d70 | 39394e70, 396f6390, 39023b90, 39360020, 390b3750 | desc `+0xf8`; instance `+0x150/+0x15c` | state |
| `0x3938c520` | 291 | channel-delay: same for instance `+0x88/+0x8c/+0x90` | 391d5d70 | 3938c270, 39360020, 390b3750 | desc? instance `+0x88/8c/90` | state |
| `0x39395b50` | 512 | **`StopCasting`**: clear unit `+0x3ac4/+0x3ac8` (or `+0x3acc/+0x3ad0`), stop anim/sound, call skill-controller `0x18/10` stop events, hide cast bar `FUN_394277f0` **[R]** | 39396000, 39398550, 3939b010 | 39394e70, 39360020, 393ed520, 393f0a00, 390b3750, 396f6390, 393b6220, 394277f0 | unit cast fields; desc `+0x1e8` controllers; `+0x13a` | state |
| `0x3939bfd0` | 569 | `UpdateClientOnlySkills`: walks the manager's list `+0x6c`, starts pending fire (`event 0x15`→`Fire`), fires delayed local damage, ends after `+0x2d4+1000 ms` | 3939c4a0 | 3939b010, 39398c80, 39397ff0, 3939c0e0?, 396f6390, 390b3670, 39407550 | instance `+0x2d0/2d4/2d9/2da`, `+0xb4/b5` | state |
| `0x3939c4a0` | 1061 | **`ClientSkillActionManager::Update`**: rebuilds active-skill vector; ages/expires skills; calls the two updates | 391739e0? (game update) | 393ec740, 393f0a50, 3933f8d0, 39396160, 3939bfd0, 3939cb80, … | manager `+0x0c` map, `+0x6c` vector | state |
| `0x3939cb80` | 461 | **`SkillComboMan::Update`**: pops combo entries whose `+0x14 + +0xc + +0x8` expired; clears cooldown (`39899ec0`), runs combo fire (`39368f90`) | 3939c4a0 | 395a7200, 3930bb30, 396f6390, 39899ec0, 39368f90 | entries `{+0,+4,+0xc,+0x14,+0x10}` stride 0x18 | state |
| `0x3939ce90` | 419 | **`SkillComboMan::Add`**: replaces/inserts combo entry for `(slotActionId, skillId, duration)`; sets cooldown estimate `39899f90` | 3938f100, 3939b010 | 3939c950, 396f6390, 395a7200, 3930bb30, 39899ec0, 39899f90 | combo vector | state |
| `0x3939c950` | 85 | combo helper: current combo skill id | 3939ce90 | — | — | state |
| `0x3939cde0` | 168 | combo vector insert | 3939ce90 | — | — | state |
| `0x39393920` | 243 | auto-attack start from a skill request (`FUN_39393530`, then `FUN_39094150`/`FUN_39095ed0`) | 393989e0, 3939d… | 39393530, 396f6390, 390b3750, 390ca3d0, 39094150, 39095ed0 | `DAT_39ac68d5` | state |
| `0x39391da0` | 129 | `SkillCaster`-like struct equality (type/id) **[R]** | 3939b010 | — | — | **pure** |
| `0x39391e30` | 465 | `SkillCastTarget`/`SkillObject` equality incl. float compare **[R]** | 3939b010 | — | — | **pure** |
| `0x39394e70` | 110 | skill tree lookup by target id → map value (cast state at `+0x18`) | many | — | tree at `manager+0x0c` | **pure** (read) |
| `0x39394f30` | 98 | manager map lookup by unit id (second tree) | many | — | tree at `manager+0x18`? | **pure** (read) |
| `0x393953a0` | 49 | skill state lookup by target key → `treeValue+0x18` (same tree as `39394e70`); 0 if absent **[R]** | many | 39394e70 | tree at manager `+0x0c` | **pure** (read) |
| `0x393953e0` | 90 | `ClientSkillRef world no skill: %u` (target key) | 39397ff0 | 39394e70 | — | state |
| `0x3938c2e0` | 48 | plot/effect map lookup by key → `value+0x18` (used for cast-delay/channel-delay states) | 394bcf60 | 3938c270 | tree at manager `+0x04` | **pure** (read) |
| `0x39395440` | 90 | `ClientSkillRef anim no skill: %u` | many | — | — | state |
| `0x393954a0` | 90 | `ClientSkillRef proj no skill: %u` | many | — | — | state |
| `0x39395ed0` | 101 | arm local client skill timers (`+0x2d0/2d4`) | 3939b8c0 | — | — | state |
| `0x39394070` | 558 | update per-unit skill state *before* a new cast (stops/clears the old instance) | 3939a810 | — | — | state |
| `0x3939a200` | 203 | erase skill entry from manager map | 3939a200/3939bfd0 | — | red-black tree | **pure** (small) |
| `0x3939a300` | 314 | get-or-insert skill entry in manager map | 3939a440 | 39396ca0, 39396b50, 39399a20, 39399f80 | map | state |
| `0x39399e10` | 105 | tree node free | 3939a200/2d0 | — | — | **pure** |
| `0x39399af0` | 752 | tree erase helper | 3939a200 | — | — | **pure** |

## C. Cooldowns and GCD

The cooldown manager is **inline in the unit at `+0x2c48`** (`lea ecx,[esi+0x2c48]` in
`0x3939aab9/0x3939aad7`), layout in [`structs.md`](structs.md) §3.

| addr | size | role | C | E | fields / globals | purity |
|---|---|---|---|---|---|---|
| `0x39899d00` | 9 | clear the **global** cooldown pair `+8/+0xc` | 391d6420 | — | +8/+0xc | **pure** |
| `0x39899d10` | 35 | manager reset/init (`unit,id`, zero lists) | many | — | +0..+0x14 | **pure** |
| `0x39899d40` | 167 | append `SCCooldowns` lists (skills then tags), cap 150 each **[R]** | 391d43c0, 391df020, 391df0e0 | — | +0x71c/+0x720, +0x10/+0x14 | **pure** |
| `0x39899df0` | 94 | **custom GCD producer**: `gcd = custom_gcd_ms * 1000 / (1000 + attr(0x4a))`; writes `+8/+0xc` **[R]** | 3939a810 | 398a0bc0 | desc `+0x94`, `+0x9c`, `+0x1bc` | **pure** (given attr) |
| `0x39899f30` | 90 | **tag/default GCD producer**: `gcd = 1_000_000 / (1000 + attr(0x4a))` = 1000 ms scaled; writes `+8/+0xc` **[R]** | 3938abe0, 3939a810 | 398a0bc0 | attr 0x4a | **pure** |
| `0x39899e50` | 112 | tick: subtract `dt` from tag/skill remaining and global remaining | game update | — | lists | **pure** |
| `0x39899ec0` | 61 | clear one skill cooldown (`remaining=0`) | 391d6420, 3939cb80, 3939ce90 | — | skill list | **pure** |
| `0x39899f00` | 48 | clear one tag cooldown | 391d6420 | — | tag list | **pure** |
| `0x39899f90` | 768 | **add a skill/tag cooldown** (if `param_2[0x21]==DAT_3a5e12f0` skill else tag), keep max, compact when full, logs `skill cooldown is full. unit %s(%u) skill %u` / `tag cooldown is full…` **[R]** | 3938c410, 3938d970, 39396000, 3939ce90, 3989a360 | `memcpy`, `X2Log` | skill/tag lists, cap 150 | **pure** |
| `0x3989a290` | 190 | **query cooldown**: for desc `*desc` (skill id) and desc `+0x238/+0x23c` tag vector; if `ignore_global` (`desc+0xdc`) return `0,0`; else max(remaining),total | 3943f310, 393f61c0, 3989a3b0/a3e0/a400/a450 | — | desc `+0`, `+0x238..0x23c`, `+0xdc` | **pure** (read) |
| `0x3989a360` | 77 | compute a skill cooldown (`FUN_3989f900`) and add it | 3939a360 | 3989f900, 39899f90 | — | state |
| `0x3989a3b0` | 37 | "is this skill on cooldown" | many | 3989a290 | — | **pure** |
| `0x3989a3e0` | 32 | global pair query for a desc (`ignore_global` aware) | 3943f310, 393f61c0, 393919f0 | 3989a290 | desc `+0xdc` | **pure** |
| `0x3989a400` | 67 | state enum: 1 = skill/tag cooldown, 2 = GCD, 0 = ready | many | 3989a290 | +8 | **pure** |
| `0x3989a450` | 38 | remaining cooldown for a desc | many | 3989a290 | — | **pure** |
| `0x39898ef0` | 188 | per-weapon value selector (8 candidates; `+0xd88/+0xb48/+0xc00`, categories 0x15/0x16/0x17) — used for the skill animation/timing id, **not** GCD **[R]** | 393888b0, 39389440, 3939a810, 3939b8c0 | 396a6d40, 39695ab0, 397cf8b0 | unit `+0xd88/+0xb48/+0xc00` | state |
| `0x398a0bc0` | 407 | unit attribute getter `(unit+0x1aa0, key, attrId, default)`; attr `0x4a` is the GCD divisor (name `global_cooldown_mul` is inferred) | many | — | attr table | **pure** (read) |
| `0x3943f310` | 261 | `X2Action:GetActionCooldown(slot)` → callback with `(remaining,total)` of the slot's skill/tag | script | 390898b0, 390898d0/930, 39695dc0, 396f6390, 3989a3e0, 393a6d90, 3905aca0 | X2Action slots `+0x90` | state |
| `0x3943f420` | 78 | `GetActionCooldownEx(slot)` → `(skill, tag, gcd pairs)` via `393f61c0` + `3943ecb0` | script | 393f61c0, 3943ecb0 | — | state |
| `0x393f61c0` | 283 | resolve slot → skill desc → `(tag pair, skill pair, gcd pair)` | 3943f420 | 390898b0/d0/930, 39695dc0, 396f6390, 3989a3e0, 3989a290, 393a6d90 | — | state |
| `0x393f62e0` | 123 | slot → spell id | many | 390898b0/930 | — | state |
| `0x393f6360` | 438 | slot action info → `{type,id,…}` (UI query) | script | 390898b0/930, 39695dc0 | — | state |
| `0x3943ecb0` | 298 | invoke a Lua callback with four integer args (the Ex query) | 3943f420 | — | — | state |
| `0x392c7360` | 1118 | action-slot widget cooldown mask update; fires `global_cooldown_start/done` and `skill_cooldown_start/done` (event 0x25) **[R]** | UI update | 392c6b90, 39182840, 390075e0, 39360020, 392c2df0, 39208f60, 39209430, 3927ba00, 392c3890 | slot `+0x34c` cooldown object, `+0x2d0`, `+0x311/312` | state |
| `0x394bdc10` | 4132 | `ScriptBindUnitAddon::UnitInfo` — exposes unit stats incl. `global_cooldown_mul` **[R]** | script | 398a0bc0 etc. | stat list | state |
| `0x394bec40` | 5608 | second stat-bind variant (`global_cooldown_mul` at 0x399e41ac) | script | — | — | state |

## D. Targeting, combat state, force attack

| addr | size | role | C | E | fields / globals | purity |
|---|---|---|---|---|---|---|
| `0x394270a0` | 826 | **set current target**: write unit `+0x1aa0` atomically, update target/target-of-target frames, UI calls with 2, watch/team handling **[R]** | 391d5270, 39186db7 | 39360020, 3a6779eb, 3a9bfb21, 390b3750, 39056540, 39056430, 393548c0, 3920fa60, 393642a0, 3920ff80, 39342f50, 393871e0, … | unit `+0x1aa0`, `+0x410`, `+0x420` | state |
| `0x39186db7` | 175 | self-target request: writes target = self and sends CSChangeTarget (opcode `0x2c`) **[R]** | 39209c04 | 3a6779eb, 39360020, 394270a0, 394fab30, 397ab790, 3ac1058c | `DAT_39b02294+0x10c` | state |
| `0x3945fcd0` | 1003 | **unit token parser** (`player`, `playerpet`, `target`, `watchtarget`, `team`, …; repeatedly consumes `target` suffixes) **[R]** | 390697f0, 3906a760, 390f3f30, 390f3fd0, 391f46c0, 3921b170, 3924d120, 392ce8d0, 392d05f0, 392d0810, 3946e950, … | 39360020, `XlComparePrefix`, 3933f5e0, 3a6779eb | unit `+0x1aa0` | state/read |
| `0x394bd190` | 299 | `UnitDistance(token)` (distance to token/position) | script | 3945fcd0, 390b4b50, 390b3850, 390699b0, 394bb870 | — | state |
| `0x394bd830` | 210 | `UnitCombatState(token)` → unit `+0x3bba` **[R]** | script | 3945fcd0 | +0x3bba | **pure** (read) |
| `0x390b36f0` | 26 | set combat state `+0x3bba=1` and call unit vtable `+0x50(1)` **[R]** | 391d69a0, 39391670 ×2, 393992d0 | unit vtable +0x50 | +0x3bba | state |
| `0x390b3710` | 26 | clear combat state (+vtable `+0x50(0)`) | 39355ec0 | — | +0x3bba | state |
| `0x394bd9f0` | 256 | `IsFirstHitByMeOrMyTeam(token)` → first-hitter `+0x5fe8`, relation `+0x5fec`, `DAT_3a5ab31c` (none), `DAT_39d44c50` (my team?) **[R]** | script | 3945fcd0, 39058bd0 | +0x5fe8/+0x5fec | **pure** (read) |
| `0x39354780` | 25 | set first-hit fields (`FUN_394bd9f0` state) from `SCCombatFirstHit` | 391d6ae0 | — | +0x5fe8/+0x5fec | state |
| `0x391d69a0` | 178 | `OnCombatEngaged` (`0x085`): set state, fire UI event 3 for the local player | packet | 39360020, 390b36f0, 3a6779eb, 39424220 | +0x3bba | state |
| `0x391d6a60` | 124 | `OnCombatCleared` (`0x086`): unit vtable `+0x48` | packet | 39360020 | — | state |
| `0x391d6ae0` | 132 | `OnCombatFirstHit` (`0x089`): `FUN_39354780` | packet | 39360020, 39354780 | +0x5fe8/5fec | state |
| `0x391d5270` | 115 | `OnTargetChanged` (`0x084`): `FUN_394270a0` | packet | 394270a0 | — | state |
| `0x39393f30` | 234 | update combat relationship trackers (`DAT_3a16830c` list): first/last hitter ids `+0x44/+0x48/+0x4c`, party size `+0x34` | 39398c80 | — | party rows | state |
| `0x39391670` | 162 | enter combat for source/target (mounted checks `+0x3c26/3c27`, `+0x410/0x420`) | 39398c80 | 390b36f0 ×2, 3935e370 | — | state |
| `0x393968b0` | 256 | aggro/threat link on hit (`+0x3bba` gate, `FUN_3935b2e0`/`3935d050`/`393737e0`) | 39398c80 | 39360020, 39023b90, 3935b2e0, 3935d050, 393737e0 | +0x3bba,+0x38 | state |
| `0x394c43d0` | script | `X2Unit` bind group (exposes `UnitIsForceAttack` etc.) | script | — | — | state |
| `0x391d3c60` | 241 | `OnForceAttackSet`: unit `+0x3bbc = packet byte`, event `0xe8` | packet | 39360020, 39424820, 39424260 | +0x3bbc | state |
| `0x39426d10` | 239 | **"is moving"** used by UseSkill: any axes 0..5 > 0, or mouse-look mode, or unit vtable +0x8c/+0x10 false **[R]** | 393989e0, 39209d80 | 393fd240 ×6, 392aa250, 390b3750 | input axes | state/read |
| `0x39426e60` | 220 | stop movement (clear axes 0..5) used by click-cast | 39391090? | 393fd280 ×6, 390b3750 | — | state |
| `0x394269c0` | 74 | cancel/close target-select UI mode 2 | many | — | `DAT_3a5a4ea0` | state |
| `0x39426a10` | 44 | cancel mode 3 | many | — | — | state |
| `0x39426af0` | 93 | cancel mode 4 | many | — | — | state |
| `0x3942fb50` | 514 | target/ground click pick (`DAT_3a5a4e78` bits) — the ray/pick rule **[?]** | 390f50a0 | — | pick result | state |
| `0x39429e40` | 567 | apply mouse-clear-target (when `setup+0x2d4` on) | 390f4960 | — | `DAT_3a5a4dcc/dd0` | state |

## E. Hit presentation logic (reaction selection)

| addr | size | role | C | E | fields / globals | purity |
|---|---|---|---|---|---|---|
| `0x391d5fb0` | 296 | `OnUnitDamaged` (`0x0a7`): copies 0x188-byte record, local-player filter, calls apply | packet | 393978e0, 3901be50, 396a6d40, 39398c80 | target `+0x410` owner | state |
| `0x39398c80` | 1614 | **apply damage**: resolve caster/target, hit type; per-source type (skill `0`, buff `2/3`, doodad `4`, …); apply HP, request reaction, emit combat text **[R]** | 391d5fb0, 391d60e0, 3939bfd0 | 39360020, 393905b0/660, 39898cc0, 39856520, 3938e1d0, 393985f0, 393917f0, 39391950, 39391670, 393968b0, 39393f30, 39398880, 390b3750 | hit record (0x188 B), `+0x18` caster, `+0x48` hit type, `+0x6e` engage | state |
| `0x393917f0` | 295 | **hit reaction selector**: maps hit type (`1,3,4,5,6,7,9,10,0xb,0xd,0xe,0xf,0x11,0x14`) to reaction anim ids `DAT_39ac766c…0x39ac7698` and kind 1/3 **[R]** | 39398c80, 393992d0 | 397cd5f0, 39354600 | anim id table | **pure** |
| `0x393985f0` | 643 | build/emit a hit *effect* event (`FUN_3940a0f0` caster/type, `+0x50..0x6c` values) | 39398c80 ×3, 393992d0 | 3940a0f0, 393878… | hit record | state |
| `0x39391950` | 147 | decide caster-vs-target presentation path (hit type `+0x48`, big-hit value `desc+0x188` vs 1.0) | 39398c80 | 39360020, 390b3750 | desc `+0x188` | **pure** |
| `0x393992d0` | 753 | **apply heal** (`SCUnitHealed`): same dispatch for heal types; calls `FUN_39392480` (heal text/anim) | 391d? | 39360020, 396f6390, 396f63d0, 39387440, 393ebc60, 393ed520, 39392480, 3938fe00, 39398880 | heal record | state |
| `0x39392480` | 314 | heal presentation (`FUN_3940a0f0`, `FUN_3940abc0`), caster/target heal anim | 393992d0 | 3940a0f0, 3940a3e0, 3940abc0 | — | state |
| `0x39398880` | 337 | damage/heal floating combat text event | 39398c80, 393992d0 | `DAT_399b03a8` | — | state |
| `0x391d60e0` | 114 | `OnUnitDamaged` environment variant (no caster) | packet | 39398c80 | — | state |
| `0x3938a3c0` | 943 | apply a created buff: choose list (buff/debuff/hidden), stack rules, apply stun/knockdown/root state fields | 391d5e00 | 39386e00, 39387440, … | `+0xf4` kind (1/2/3) → `+0x20/+0xd28/+0x16f0` counts | state |
| `0x39386e00` | 120 | append buff row: 32 buffs at `+0x28` (0x68 B), 24 debuffs at `+0xd30`, 24 hidden at `+0x16f8` **[R]** | 3938a3c0 | — | counts +0x20/+0xd28/+0x16f0 | **pure** |
| `0x391d5e00` | 144 | `OnBuffCreated` (`0x0b6`): `FUN_3938a3c0` | packet | 39360020, 3938a3c0 | — | state |
| `0x391d5f20` | 140 | `OnBuffUpdated` (`0x0b9`) | packet | 39360020, 393886c0 | — | state |

## F. Skill data and shared helpers

| addr | size | role | notes |
|---|---|---|---|
| `0x39731870` | tail | `LoadSkillDescs` (SQL `SELECT … FROM skills`) — the `SkillDesc` allocator/loader | see `structs.md` §1 |
| `0x39733a90` | tail | `LoadBuffDescs` (`SELECT … FROM buffs`) | buff flags stun/knockdown etc. |
| `0x396f6390` | 62 | skill id → `SkillDesc*` (0 if absent) | used everywhere |
| `0x39695dc0` | 62 | unit/item id → unit object | used everywhere |
| `0x39360020` | 57 | unit id → actor/unit object | used everywhere |
| `0x390b3750` | 7 | actor → GameClient (`*(actor+0x3be8)`) | movement doc |
| `0x39360020` | 57 | world unit lookup | — |
| `0x39005b20` | 21 | returns the shared skill-tag/attribute key object `&DAT_39ad7180` (passed to the GCD producers) | **[R]** address, **[I]** role |
| `0x39424220` | 19 | fire UI event by id (one int) | — |
| `0x39424240` | 23 | fire UI event by id (one value) | — |
| `0x39424260` | 27 | fire UI event by id (two values) | — |
| `0x39428370` | 8 | throw a `SkillResult` toast (plain wrapper) | `FUN_3a6a126a` |
| `0x394df580` | 38 | UI message row by text id | — |
| `0x39420b30` / `0x39421720` | tail | unit/target UI snapshot builders used by combat text | — |

### Totals

The rows above are 152 entries (**151 distinct functions**, 83,078 bytes) with `funcs.csv`
sizes. The count includes the ~30 requirement evaluators reached from `FUN_393a0510` and the 15
cooldown-manager methods, which are the small, high-value differential targets. `FUN_3ab63fb7` is counted as its 114-byte entry in
`funcs.csv`; its real body is split into tail cold blocks that Ghidra does not size (see U1/U2).
