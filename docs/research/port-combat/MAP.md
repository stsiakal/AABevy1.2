# Port map: the ArcheAge 1.2 client combat code

Address map for a function-by-function Rust port of the 1.2 client's skill use, cast/channel
timeline, cooldowns/GCD, targeting and client-side hit presentation, with a differential-test plan
that calls the original functions inside the running hidden 1.2 client (`aadriver.dll`).

## 0. Sources, conventions and licensing

### 0.1 Licensing

Everything here is derived **only** from the shipped 1.2 binaries and their decompiles. The
published CRYENGINE source was not used, quoted or consulted (same rule as
`aaemu-physics/CRYPHYSICS-MAP.md`). Class names in `CClass` form come from the binaries' own RTTI,
profiler/log/assert strings, cvar names, or the named 10.x build; behaviour that has no name in
the binary is marked *(inferred)*. Port from this document and the decompiles, never from the
CRYENGINE source.

### 0.2 Sources

| source | path |
|---|---|
| 1.2 x2game decompile | `C:/AAEMU_DISK/aa-re/client12/out-x2game/x2game.dll.c` |
| 1.2 x2game functions/calls/strings | `C:/AAEMU_DISK/aa-re/client12/out-x2game/{funcs.csv,calls.tsv,strings-xrefs.tsv}` |
| 1.2 x2game image + `.data` (constants, tables, vtables) | `D:/aa-re/client12/x2game_unpacked_data.dll`, base `0x38FF0000` |
| 1.2 engine decompiles | `D:/aa-re/client12/engine/out/`, `D:/aa-re/client12/engine2/out/` |
| 1.2 cryphysics (living entity) | `D:/aa-re/cryphysics/x86_12/`, ported by `aaemu-physics` |
| 10.x names/structure only | `D:/ArcheAge Returns 10.0.2.13 - 8yx - r575 - 2026-06-18/Bin64/decompiled/x2game-dev_dedicate.dll.c` |
| UI scripts (LuaQ bytecode) | `D:/aa-re/client12/ui/game/scriptsbin/x2ui/…` |
| bindings | `default_binding.g` (extracted at `wt/pr/data/extract/game/default_binding.g`) |
| existing notes | `docs/research/re-combat.md`, `re-constants.md`, `re-mounts*.md`, `re-skills.md` |
| server-side field names | `C:/Users/stsiakal/AAEmu/rust/crates/aaemu-game/src/systems/skills/` (our own port) |

x2game addresses are VAs at the preferred base `0x38FF0000`. The unpacked tail code
(`0x3a6…`–`0x3ac…`) is part of the same image; `thunk_FUN_xxx` at `0x39xxxxxx` are 5-byte jumps
into it. Evidence markers: **[R]** read from the binary/decompile, **[I]** inference, **[?]** open.

### 0.3 Layout of this folder

| file | content |
|---|---|
| `MAP.md` (this) | call chains, rules, port order, unknowns |
| [`functions.md`](functions.md) | every function in scope: address, size, callers/callees, fields, purity |
| [`structs.md`](structs.md) | the Rust struct layouts and offsets |
| [`packets.md`](packets.md) | client packet handlers and their field offsets |
| [`difftest.md`](difftest.md) | in-process differential recipes per function |

### 0.4 Scope size

[`functions.md`](functions.md) documents **151 distinct x2game functions, 83,078 bytes** (152
table rows, `FUN_39360020` appears in two groups). The count includes the ~30 requirement
evaluators and 15 cooldown-manager methods. Tail bodies split across cold blocks (`FUN_3ab63fb7`, the skill
loaders `FUN_39731870`/`FUN_39733a90`) are only sized by their `funcs.csv` hot fragment.

---

## 1. The call chains

### 1.1 From input to a skill request

```
key / mouse
├─ binding 1..0, SHIFT-1..0 ("action_bar_button", action index 82)   [default_binding.g]
│  └─ (key→slot dispatch not statically linked: U8)
├─ mouse click on a unit / ground
│  └─ ControlMouseClick                     x2game 0x390f50a0  (384 B)
│     ├─ mode 0: MouseClickControl::Normal   0x390f4960  (715 B)  → pick/clear target
│     ├─ mode 2: FUN_390f4c30 (1129 B)  → UseSkill with skill-object position
│     ├─ mode 3: FUN_390f38f0  (560 B)  → UseSkill with picked position
│     └─ mode 8: FUN_390f3b20  (345 B)  → UseSkill with hit position
├─ UI/script: X2Player / skillbook / auto-repeat / AI (client units)
│  └─ FUN_3905abc0 (216 B) or auto skill 0x39016dc0 (483 B)
│
└─ X2::GameClient::ClientSkillActionManager::UseSkill   0x393989e0 (657 B)
   ├─ skill desc = FUN_396f6390(skillId)
   ├─ target fix-up      FUN_393925c0 (193 B)   target override / selection
   ├─ self-target check  FUN_39426d10 (239 B)   "is moving"
   ├─ caster fix-up      FUN_39391090/910e0 (155/128 B)
   ├─ auto-attack gate   FUN_393be4e0 (645 B)
   ├─ target equality    FUN_393925c0
   ├─ request body       thunk 0x39396e20 (5 B) → FUN_3ab63fb7 (tail)
   │  └─ unit_req checks FUN_393a0510 (1087 B) → 47 evaluators
   │  └─ build + send CSStartSkill (opcode 0x052, writer in the tail: U2)
   └─ auto-attack start  FUN_39393920 (243 B)
```

The client **does not create the local skill on request**. The local timeline starts only when
the server answers `SCSkillStarted` (`0x0a1`) → `StartSkill` `0x3939a810`. This is proven by the
call graph: `0x3939a810` has only three callers (`391d57f0` the packet handler, `3939b010`
fire-retry, `3939b8c0` the script/client-only start) and `FUN_3939a440`, which creates the skill
instance, is called only from `0x3939a810` [R, `calls.tsv`]. `UseSkill` returns a packed result
`{u8 result, u16 arg, u32 value}`; a non-zero result is shown through `FUN_39390900`.

### 1.2 Server-driven cast, fire and end

```
SCSkillStarted (0x0a1)  handler 0x391d57f0 (162 B)
└─ StartSkill                       0x3939a810 (2036 B)
   ├─ resolve caster      FUN_39360020
   ├─ stop same-target old skill  FUN_39394070 / FUN_39397ff0
   ├─ GCD estimate        FUN_39899df0 (custom) | FUN_39899f30 (tag/default)
   ├─ create instance     FUN_3939a440 (967 B) → manager map +0x0c
   ├─ start anim/controllers, caster-side events
   └─ notify UI           event 0x14

SCSkillFired (0x0a2)   handler 0x391d58a0 (181 B)
└─ FireSkill                         0x3939b010 (2218 B)
   ├─ find instance by target       0x39394e70
   ├─ old instance compare/stop     0x39391da0 / 0x39391e30 / 0x39397ff0
   ├─ write timings (delay/channel), fire anim, controllers
   ├─ local damage/heal path        0x39398c80 / 0x3921ad60? (see §6)
   ├─ combo add                     0x3939ce90 (419 B)
   └─ notify UI                     event 0x15

SCSkillEnded (0x0a3)    handler 0x391d5bf0 → EndSkill 0x39397ff0 (1050 B)
SCSkillStopped (0x0a4)  handler 0x391d5c70 → StopSkill(autoattack) 0x39398410 (311 B)
SCCastingStopped (0x0a5) handler 0x391d5cf0 → FUN_39396000 (344 B)
SCCastingDelayed (0x0a6) handler 0x391d5d70 → FUN_39395760 (317 B) + FUN_3938c520 (291 B)

per frame
ClientSkillActionManager::Update      0x3939c4a0 (1061 B)
├─ UpdateClientOnlySkills             0x3939bfd0 (569 B)
│  ├─ start pending fire at +0x2d0    → Fire
│  ├─ delayed local damage at +0x2d4  → FUN_39398c80
│  └─ remove at +0x2d4 + 1000 ms
└─ SkillComboMan::Update              0x3939cb80 (461 B) → pop expired combo, clear cooldown
```

### 1.3 Cooldowns

The manager is **inline in the unit at `+0x2c48`** (`lea ecx,[esi+0x2c48]` at `0x3939aab9` and
`0x3939aad7`) [R]. Layout in [`structs.md`](structs.md) §3. Producers and queries are in §4.

```
StartSkill ── custom/default GCD ──► FUN_39899df0 / FUN_39899f30 (writes +8/+0xc)
SCCooldowns (0x04a) handler 0x391d43c0 ─► FUN_39899d40 (append skills+tags, cap 150)
SCSkillCooldownReset (0x0ad) handler 0x391d6420 ─► 39899ec0 / 39899f00 / 39899d00
SCCastingStopped (0x0a5) → 39396000 ─► 39899f90 (add one cooldown at stop)
Combo add 3939ce90 ─► 39899f90 (remove old combo cooldown, add new)
per frame 39899e50 (tick)
X2Action:GetActionCooldown/Ex → 3943f310 / 3943f420 → 393f61c0 → 3989a3e0 + 3989a290
action-slot widget 0x392c7360 → mask/events from that object
```

### 1.4 Targeting

```
SCTargetChanged (0x084) handler 0x391d5270 ─► FUN_394270a0 (826 B)
                                                 ├─ write unit+0x1aa0 (atomic)
                                                 ├─ target/target-of-target UI updates (2)
                                                 └─ watch/team side effects
Self target: 0x39186db7 ─► FUN_394270a0 + CSChangeTarget (opcode 0x2c)
Unit tokens: FUN_3945fcd0 (1003 B)   "targettarget", "playerpet", "team", "watchtarget", …
Combat:      0x390b36f0 / 0x390b3710 (26 B)  unit+0x3bba
First hit:   0x39354780 (25 B)               unit+0x5fe8/+0x5fec
Force mode:  0x391d3c60 (241 B)              unit+0x3bbc
```

---

## 2. A — skill use from input

### 2.1 Bindings and the action table

`game/default_binding.g` (primary manager) binds:

* `a turnleft`, `d turnright`, `q moveleft`, `e moveright`, `w/s forward/back`, `space jump`,
  `. toggle_walk`, `tab cycle_hostile_forward`, `CTRL-tab cycle_friendly_forward`,
  `SHIFT-tab cycle_hostile_backward`, `CTRL-SHIFT-tab cycle_friendly_backward`, `f1 round_target`,
  `CTRL-f toggle_force_attack`, `ALT-shift-r reply_last_whispered`, `SHIFT-up/down action bar page`.
* `action_bar_button` is a **multi action**: `1..0` → slots 1..10, `SHIFT-1..0` → 13..22,
  `minus/equals` → 11/12, `SHIFT-minus/equals` → 23/24.

The engine action-name → index table is at **`0x39ac8e68`** (0x68 entries, read from `.data`;
`FUN_393f8d20` compares against it). Relevant indices [R]:

| index | action | index | action |
|---|---|---|---|
| 22 | cycle_hostile_forward | 39 | toggle_force_attack |
| 23 | cycle_hostile_backward | 40 | self_target |
| 24 | cycle_friendly_forward | 41 | round_target |
| 25 | cycle_friendly_backward | 42 | team_target |
| 82 | action_bar_button | 44 | pet_target |
| 84 | pet_action_bar_button | 85 | mode_action_bar_button |
| 86/87 | action_bar_page_prev/next | 89 | set_watch_target |

`FUN_392c4dd0` (725 B) is the only code consumer of `action_bar_button`: it parses the active
multi-binding (`FUN_393fd1f0`), refreshes the slot's key label (`FUN_393fc4a0`), and fires UI
event `0x25` with the string `hotkey_activated` [R]. It does **not** use the skill. The native
dispatch that turns the key into a slot use is not statically linked (U8); the request front-end
called by every known path is `FUN_393989e0`.

### 2.2 Action slots

* 85 slots, 0x10 bytes each, at the `X2Action` object `+0x90`; `FUN_39089c60` copies 0x550 bytes
  from `SCActionSlots`, and `FUN_39089c80` scans all 85 slots (`iVar3 = 0x55`) [R].
* Slot layout: `+0 kind` (0 none, 1 item, 2 spell, 3 macro, 4 item_id), `+8 id` (u32; u64 for
  item_id) [R].
* 4 pages; Lua is 1-based, `CSUpdateActionSlot` slot numbers are 1-based (`re-combat.md` §2.5).
* Queries used by the UI: `GetActionType` `0x3943ee70`, `IsActionEmtpty` `0x3943f470`,
  `GetActionInfo` `0x3943fa10`, `GetActionCooldown` `0x3943f310`, `GetActionCooldownEx`
  `0x3943f420` [R].

### 2.3 Targeting modes armed from the UI

Five helpers write the global targeting state (`DAT_3a5a4e*`) and are invoked from the UI click
path [R]:

| function | size | mode | meaning |
|---|---|---|---|
| `0x39428630` | 223 | 2 | skill use with a skill-object/target block; `DAT_3a5a4ea0=2`, `DAT_3a5a4eb0=skillId` |
| `0x39428710` | 228 | 2 | same with an extra byte from `param_3[3]` |
| `0x39428800` | 148 | 3 | unit-target skill |
| `0x394288a0` | 267 | 8 | ground-target skill (position) |
| `0x39428a30` | 368 | 4 | item/special target |

`ControlMouseClick` `0x390f50a0` dispatches on `DAT_3a5a4ea0` and the click mode
(`DAT_3a5a4da8 == 6`). Modes 2/3/8 call `FUN_393989e0` with the picked target/position
(`0x390f4c30`, `0x390f38f0`, `0x390f3b20`). Mode 0 calls `MouseClickControl::Normal`
`0x390f4960`, which selects the unit under the cursor and honours `mouse_clear_targeting`
(`cfg+0x2d4`). The exact pick/ray rule lives in `0x39429e40` / `0x3942fb50` [?].

### 2.4 `UseSkill` (`FUN_393989e0`, 657 B) — the request front-end

Signature: `bool f(Unit* unit, u32 skillId, SkillCastTarget* target, SkillCaster* caster,
SkillObject* object)` (`__cdecl`-looking in the decompile; the first argument is the caster
unit).

Order [R]:

1. `desc = FUN_396f6390(skillId)`; 0 → return 0.
2. Resolve the caster unit; if the **caster** is the local player call `FUN_39426d10()`
   (is moving). The resulting byte is stored in the `flag` field of the `SkillObject`/event block
   and is 1 only for a local caster while not moving.
3. `FUN_39094150(target)` + `FUN_394269c0/39426a40/394fab30` (UI target-mode cleanup).
4. Caster fix-up by `desc+0x1b0` target type:
   * `1` → `FUN_393910e0` (protected/friendly target becomes self),
   * `4` → `0x3a6775ec`,
   * `5` → `FUN_39391090`.
5. Auto-attack skill id `DAT_39ac769c`: if `unit+0x3ad5` is set, return 1; else read the current
   target and `FUN_393be4e0(target)`; a failure shows `FUN_39390900`.
6. `FUN_39390f80` builds/clamps the caster block (the `+0x158` "delayable" path).
7. `FUN_393925c0` validates/overrides the target against `desc+0xe8/+0xe4` (skill-category lock).
8. Call the request body `thunk 0x39396e20(manager, &result, unit, skillId, target, caster,
   object, 1)`.
9. Ask the manager `(*DAT_39d1fba4+8)()` whether auto-attack is running; if the input was
   self-target and the result is `0x0f` (or the auto-attack result is 1), call
   `FUN_39393920(caster, target)` to start the local auto-attack.
10. Non-zero result → `FUN_39390900(result, arg, skillId, caster)`.

Purity: **state** (writes the manager, sends a packet, touches UI). The two argument checks
`FUN_393925c0` and `FUN_393be4e0` are the testable leaves.

### 2.5 The request body (thunk `0x39396e20` → `FUN_3ab63fb7`)

`FUN_3ab63fb7` is a split tail function: Ghidra decompiles only a 114-byte exception fragment,
so the body is **U1/U2**. The call site in `UseSkill` proves the argument order (7 stack args after
`this`): `&result`, `unit`, `skillId`, `&target`, `&caster`, `object`, `1`. The result is the
same 8-byte packed struct as a requirement result. The body is the only place that can send
CSStartSkill for a player request; capture its send at `0x3ac1058c` (U2).

### 2.6 Prechecks and refusals

Three layers are visible:

1. **Unit requirements** (`skills.or_unit_reqs` / `unit_reqs`). `FUN_393a0510` (1087 B) walks the
   skill's requirement vector (desc `+0x1e8..+0x1ec`, 0x0c-byte rows), filters by owner/target
   with `FUN_3936c500`, switches on the requirement kind and calls one of 30 evaluators
   (`0x3939d0d0 … 0x393a0130`). Every evaluator returns `{u8 result, u16 textId, u32 value}`.
   The first non-zero result is returned. The kind table is read but only partly named (U3); the
   client switch values are `0,6,9,0xc,0xd,0x18` (nested), `0x19,0x1a,0x1b,0x1c,0x1d,0x22,0x26`.
2. **Target/auto-attack gate**. `FUN_393be4e0` (`CanAutoAttackTargetUnit`) checks the target is
   not the local player, relation bytes `+0x3c26/3c27`, and `FUN_39898e60`; it returns a
   `SkillResult` (`0x21` no target, `0x22` invalid target).
3. **Target override**. `FUN_393925c0` replaces the target with the skill-category locked object
   (`unit+0x4b8 + 0xc0*desc+0xe8`) when the category slot holds a live id and the skill's
   `desc+0xe4` matches.

The **server** still validates range/LOS/facing/resources; the 1.2 client's own `skills` fields
(`min_range`, `max_range`, `target_angle`, `check_obstacle`, …) are read by the server-side
`StartSkill` path, not by `UseSkill` (the client only packs the target) [R for the client call
graph; I for the split].

Refusal messages: `FUN_39390900` (1295 B). `param_1` packs the `SkillResult` in the low byte and
an optional unit id in the high word; the switch (0x16/0x4b/0x4c/0x51/0x24/0x46/0x2f/0x32/0x3a/
0x3c/0x3d/0x4f/0x52/0x56/0x58/0x5a/0x5b/0x5c/0x5f/0x60/0x83/0x84/99/0x72/0x75/0x76/0x66/0x6e/
0x6f/0x7c/0x7d/0x7e) resolves the object name, then fires UI event `0x15` with
`(resultName=FUN_39897990(result), arg, value)` and calls `FUN_393f4890(result)` for the text [R].
Result name enumeration `FUN_39897990` (955 B) is pure and directly testable.

---

## 3. B — cast and channel timeline

### 3.1 `StartSkill` (`FUN_3939a810`, 2036 B, `__thiscall`)

Arguments from the `SCSkillStarted` handler (`0x391d57f0`):

```
StartSkill(manager,
           casterId, skillId, targetId,
           SkillCaster* caster, SkillCastTarget* target, SkillObject* object,
           int castTime, int castTime2, byte flag, u32 extra, 0)
```

Order [R]:

1. Resolve caster actor/unit (`FUN_39360020`), find the current instance for the target
   (`FUN_39394f30`) and stop it if it has a different skill id (`FUN_39394070`).
2. Find the old skill entry by target id (`FUN_39394e70`); if it is not the manager's sentinel,
   call `EndSkill(targetId)` (`FUN_39397ff0`).
3. Ask the camera/controller chain to stop the old cast:
   `game->vtable+0xe0 → +0x18 → +0x94(args)`.
4. If the **target id** is the "none" sentinel `DAT_3a177710` and the skill id is the
   auto-attack id `DAT_39ac769c`, set `unit+0x3ad5 = 1` and return; any other skill id with that
   sentinel is the `start skill: impossible case` log path.
5. **GCD estimate** (only when the trailing `param_12` byte is 0, a caster unit exists and
   `desc+0x1bc == 1`):
   * `desc+0x9c == 0` and `desc+0x94 > 0` → `FUN_39899df0(manager, tagKey, desc+0x94)`;
   * otherwise → `FUN_39899f30(manager, tagKey)`.
6. Create the local instance with `FUN_3939a440(targetId, desc, caster, target, object, castTime)`;
   null → `X2Log("start skill: fail to create skill: %d")`.
7. `FUN_39386e80(caster, target, object)` snapshots the three blocks into the instance.
8. Start the skill animation/controllers (`FUN_3940a3e0`), fire the UI event `0x14`, and for a
   local player:
   * store the target key at `unit+0x3ac4`;
   * if `desc+0x13a` and the UI wants it, `FUN_394276d0(name, target, unitId)` (target cast bar);
   * select a per-weapon animation/timing value with `FUN_39898ef0` and notify the game client
     `game_vtable+0x120(...)` (start anim, sound, cast bar);
   * start auto-attack (`FUN_39360920`) when `desc+0x38`;
   * run the `0x18/0x2a/0x5d/0x5e` skill-controller kinds (`FUN_390944c0`, `FUN_39083ba0`,
     `FUN_39081e30`).
9. `FUN_39390850(instance)` and, for a client-only/cast-delay case, `FUN_39397a20` +
   `FUN_39390f80`.

The early-return branch (`(char)param_10 != 0`) performs only the caster/target checks, the UI-side
`FUN_394e2650`, and the event `0x14` with the raw ids; it creates **no** instance and sends **no**
request [R]. This is the path used when the client must only mirror a server-side start.

### 3.2 Cast state and the cast bar

`X2Unit:UnitCastingInfo` → `FUN_394bcf60` (546 B) resolves the token (`FUN_3945fcd0`), then checks
four per-unit states in order [R]:

| state | unit field | object (from key) | fields | value |
|---|---|---|---|---|---|
| normal cast | `+0x3ac4` target key | skill map → `FUN_393953a0` | `+0x04` skill id, `+0x138` total, `+0x144` start | `min(total, now-start)` |
| channel | `+0x3acc` target key | skill map → `FUN_393953a0` | `+0x04` id, `+0x140` total, `+0x148` start | `total - min(total, now-start)` |
| cast delay | `+0x3ac8` key | plot map → `FUN_3938c2e0` | `+0x08` plot id, `+0x74` total, `+0x70` start | `min(total, now-start)` |
| channel delay | `+0x3ad0` key | plot map → `FUN_3938c2e0` | `+0x08` id, `+0x84` total, `+0x80` start | `(start+total) - now` |

It pushes `(desc+0x104 name, total, current, desc+0x13a)` through
`FUN_394bbad0`; the vtable receives the descriptors in that order (**name, total, current,
visible**), proven by the Stage B fake-callback goldens.  The Lua `castingbar.alb` treats the
pair as total/current and fills while current grows (cast) or empties while it shrinks (channel).
When no state is live it pushes nil. Both target and target-of-target frames use the same token
machinery (`re-combat.md` §2.1).

### 3.3 `FireSkill` (`FUN_3939b010`, 2218 B)

Called from the `SCSkillFired` handler with
`(casterId, skillId, targetId, &target, &caster, &object, timeA, ?timeB, ?targetId, flag, extra,
param12, param13)`; the exact packet-to-argument mapping is in [`packets.md`](packets.md) §2.

Order [R]:

1. If `flag != 0` and `FUN_39390530(caster)` fails, show the error and return.
2. Find the instance for the target (`FUN_39394e70`). If it exists and (`instance+0x1c field !=
   skillId` or the caster/target blocks differ), stop it and start a fresh one through
   `StartSkill`.
3. Write the event timings into the instance (`piVar10[0x4e..0x52]` = `+0x138/+0x13c/+0x140/
   +0x144/+0x148` relative to the timing base), then:
   * `game->vtable+0xe0 → +0x18 → +0x98` (cast completed notification);
   * for a non-local/caster flag: `FUN_39395b50` (stop casting) and return.
4. Resolve the desc, store `now` in the instance, compute effective cast/channel times:
   * `desc+0xf8` / `desc+0xf9` choose cast vs channel; values are `ms * 0.001`;
   * `desc+0x128 != DAT_3a177724` selects the "instant" path: `FUN_393938a0` + `FUN_393f1160`
     build the time block, `FUN_393e1020/393eed90/393eef50/393eee10/393eee80` configure the skill
     controllers.
5. Start the real animation (`game_vtable+0x120`), handle the caster/mount cases:
   * `unit+0x1c0 == DAT_3a17770c && desc+0x1bc != 0` → `FUN_39368f90` (auto-attack/next);
   * `desc+0x6c > 0` → store `desc+0xdd` and the target key in the unit's cast counters;
   * `desc+0x1bc != 0 && desc+0xdd == 0` → `FUN_390b3ab0`.
6. Effective fire delay: `FUN_39354670`/`FUN_39356440` (weapon speed) depending on `desc+0x1d1`.
7. Start/stop controllers:
   * `desc+0x140 == 0`: start a skill controller when `desc+0x13c` and `desc+0xc0` conditions
     hold (`game_vtable+0x90` or `+0x8c/+0x98`);
   * `desc+0x13a`: UI target cast bar (`FUN_394278a0`, `FUN_394276d0`).
8. Walk `desc+0x1e8..0x1ec` controllers: kinds `0x18` with sub-ids `10`, `0x45`, `0x61`
   (camera/rope/anim).
9. `FUN_393eefa0`, `FUN_393ebc60`, build the hit/hit-effect block (`FUN_3940a3e0`), call
   `FUN_39397a20`, `FUN_3939ce90` (combo), and send UI event `0x15`.

### 3.4 End, stop, delay

* `EndSkill` `0x39397ff0` (1050 B): find the instance; if there is none it still sends UI event
  `0x16`. Otherwise stop the camera/cast state (`game_vtable+0xe0→+0x18→+0x9c`), clear the unit's
  matching cast slot (`+0x3ac4`), notify `game_vtable+0x120` with `DAT_39ac7688` and kind 3, hide
  the target cast bar (`FUN_394278a0`), run the end-skill controllers (`FUN_393f0a00`,
  `FUN_390b4610`, `FUN_390b3ab0`), call `FUN_393953e0` (missing "world" reference log),
  `FUN_39397a20(2, …)`, and send UI event `0x16`.
* `StopSkill` `0x39398410` (311 B): only for the auto-attack id `DAT_39ac769c`. If `unit+0x3ad5`
  is set, clear it, call `FUN_39368f90`, `FUN_39397a20(3, id)`, build the event and send id `0x17`.
* `FUN_39396000` (344 B, `SCCastingStopped`): `FUN_39394e70` → `FUN_39395b50` (clear cast), then
  `FUN_39899f90(desc, duration)` (cooldown estimate), `FUN_394e2650` (stop casting VFX) and UI
  event `0x18`.
* `FUN_39395760` (317 B) / `FUN_3938c520` (291 B): `SCCastingDelayed`.  In the skill map the
  node pointer is used, so the cast path adds `duration` to the **normal cast total**
  `value+0x138` (node `+0x150`) and compares against the normal start `value+0x144` (node
  `+0x15c`); the channel path (`FUN_3938c520`) works on the separate plot state
  `+0x88/+0x8c`.  Both scale the animation speed (`game_vtable+0x16c/0x170`) and log
  `Casting anim speed %f -> %f`.
* `FUN_39395b50` (512 B) is the single "stop casting" primitive: it clears `+0x3ac4/+0x3ac8`
  (normal) or `+0x3acc/+0x3ad0` (channel), stops the skill controller list (`0x18` sub-id 10 →
  `FUN_393b6220`), hides the target cast bar (`FUN_394277f0`) and sends the stop VFX. Its only
  callers are the three functions above; **the movement/jump cancel is not in this list** (U9).

### 3.5 Client-only skills, queue and combo

* `FUN_3939b8c0` (1004 B) is the script/AI/client-only `ClientSkill::Start`: it calls
  `StartSkill` with `castTime = desc+0x48`, allocates a 0x2e0 `ClientSkill` (`FUN_393911d0`),
  copies caster/target/object, sets `+0x138 = desc+0xb0` then `+= cast time`, `+0x2d0 = now +
  desc+0x48`, `+0x2d4 = now + +0x138 + desc+0x48`, and arms the local damage flag. Callers:
  `FUN_39409d20` (multi-step/combo executor) and `FUN_394b7f90` (`ScriptBindUnit::StartSkill`).
* `FUN_3939c4a0` `ClientSkillActionManager::Update` iterates the manager map (`+0x0c`) to build
  the render/update list, then calls `FUN_3939bfd0` and `FUN_3939cb80` [R].
* `FUN_3939bfd0` `UpdateClientOnlySkills` walks the client-only vector (`+0x6c`): at `+0x2d0`
  it sends the fire event/combat action; at `+0x2d4` it applies delayed local damage when
  `+0x2da`; after `+0x2d4 + 1000 ms` it calls `EndSkill`. This is the **queue** mechanism: a
  client-only skill that is already running has its fire/end timestamps advanced rather than
  being restarted.
* `FUN_3939ce90` adds/replaces a combo entry (0x18 bytes: action id, skill id, start, duration,
  end) and updates the slot cooldown via `FUN_39899f90`; `FUN_3939cb80` pops entries whose
  `start + duration + margin` expired, clears the old skill cooldown (`FUN_39899ec0`) and calls
  `FUN_39368f90` on the combo skill. `FUN_39409d20` (969 B) is the 3-step table executor
  (`DAT_39ac91c0`, `DAT_39ac9248` indexed by `row*3 + step`) used for chained/combo actions.
* There is no explicit client-side "queue the next skill" buffer beyond this combo list and the
  late-cast margin; `queued_skill_margin` exists only as a string (`0x399b172c`) with no code xref
  (U7).

---

## 4. C — cooldowns and GCD

### 4.1 Manager layout

`struct CooldownManager` (inline at `Unit+0x2c48`) in [`structs.md`](structs.md) §3:

```
+0x000 owner*        +0x004 unitId
+0x008 gcdRemaining  +0x00c gcdTotal
+0x010 tagCount      +0x014 tags[150]   (id, remaining, total; 0xc each)
+0x71c skillCount    +0x720 skills[150] (id, remaining, total; 0xc each)
```

`FUN_39899d40` appends the two `SCCooldowns` lists, cap 150 each; `FUN_39899f90` inserts one
entry (keeping the larger value, compacting when full and logging
`skill cooldown is full. unit %s(%u) skill %u`); `FUN_39899e50` ticks; `FUN_39899d00/ec0/f00`
clear the global/skill/tag entries.

### 4.2 Producers

**GCD (resolves re-combat open question 1).** `StartSkill` calls one of two producers when
`desc+0x1bc == 1` (default GCD):

| condition | function | formula |
|---|---|---|
| `desc+0x9c == 0`, `desc+0x94 > 0` | `FUN_39899df0` (94 B) | `gcd_ms = custom_gcd_ms * 1000 / (1000 + attr(0x4a))` |
| otherwise (tag/default) | `FUN_39899f30` (90 B) | `gcd_ms = 1_000_000 / (1000 + attr(0x4a))` (= 1 s scaled) |

Both write `+0x8 = +0xc = gcd_ms`; they do **not** write a per-skill cooldown. The stack
argument is the **unit**, not a tag key: `FUN_39899df0(manager, unit, custom_ms)` calls
`FUN_398a0bc0(this = owner+0x1aa0, unit, 0x4a, 0)`; `FUN_39899f30(manager, unit)` calls the same
getter with one stack argument. `attr(0x4a)` is the attribute row; the UI exposes
`global_cooldown_mul` (`0x399e41ac`), which is the only GCD multiplier in the data [R for the
formula and the getter call, I for the row name]. **U6 is resolved** (see §9).

The exact arithmetic is:

```text
attr_eff = if attr < -999 { -999 } else { attr }     // the -999 floor
denom    = u32(attr_eff + 1000)                       // wraps for huge attrs
custom   = u32(i32(custom_ms) * 1000)                 // 32-bit imul, wraps
gcd      = custom / denom                             // 32-bit unsigned div
default  = 1_000_000 / denom                          // 32-bit unsigned div
```

The `-999` floor is what makes the denominator 1 for any attribute at or below `-1000`; it is not
a "tag key" argument.

**Per-skill cooldowns** come from the server (`SCCooldowns`) and from the local estimate
`FUN_39899f90`:
* `SCCastingStopped` (`0x0a5`) → `FUN_39396000` adds the packet's duration;
* combo add/replace (`FUN_3939ce90`) adds the combo skill's duration;
* `FUN_3989a360` computes a skill cooldown from `FUN_3989f900` and inserts it.

### 4.3 Queries

`FUN_3989a290` (190 B) is the single query primitive: given a `SkillDesc`, it scans the tag list
for all tags in `desc+0x238..0x23c`, then the skill list for `desc+0` (skill id), taking the
maximum remaining/total. When `desc+0xdc` (`ignore_global_cooldown`) is set it returns a zero
global pair. Wrappers:

* `FUN_3989a3e0` — global pair for a desc;
* `FUN_3989a3b0` / `3989a450` — boolean / remaining;
* `FUN_3989a400` — state 0 ready, 1 skill/tag cooldown, 2 GCD.

The `X2Action` slot queries resolve a slot to a spell id (`FUN_390898b0` → `FUN_39695dc0` →
`FUN_396f6390`) and call the manager:

* `FUN_3943f310` (261 B) `GetActionCooldown(slot)` → one `(remaining,total)` pair;
* `FUN_3943f420` (78 B) `GetActionCooldownEx(slot)` → `393f61c0` fills four pairs
  (skill, tag, global) and `3943ecb0` invokes the Lua callback with four integers;
* `FUN_393f61c0` (283 B) is the resolver; `FUN_393f62e0` (123 B) returns the slot's spell id.
* The action-slot widget `FUN_392c7360` (1118 B) reads the cooldown object at slot `+0x34c`
  (vtable `+4`) and fires `global_cooldown_start/done`, `skill_cooldown_start/done` on event
  `0x25`; `FUN_39209430` supplies the pair [R].

**Exact query semantics (Stage A, confirmed by disassembly and goldens).**

* `FUN_3989a290(manager, desc, ignore_global, out_remaining, out_total)`: the pair starts at
  `(gcdRemaining, gcdTotal)` when `ignore_global == 0`, otherwise at `(0, 0)`; the tag vector
  `desc+0x238..+0x23c` and then the skill row `desc+0` still update it. The update is strictly
  `>` (`jbe` skips equal values).
* `FUN_3989a3b0(manager, desc, ignore_global)` returns `remaining != 0`.
* `FUN_3989a3e0(manager, desc, out_remaining, out_total)` is `a290` with
  `desc+0xdc` as the flag.
* `FUN_3989a400(manager, desc)` queries with flag 1, returns 1 if the skill/tag remaining is
  non-zero, else 2 when `desc+0xdc == 0` and `manager+8 != 0`, else 0.
* `FUN_3989a450(manager, desc)` returns the remaining pair's first value with `desc+0xdc`.
* The slot resolver `FUN_393f61c0(slot, out_global_rem, out_global_total, out_skill_rem,
  out_skill_total)` calls `a3e0` for the global pair and `a290(desc, 1)` for the skill/tag pair.
  `FUN_3943f310` passes the global pair's `(remaining, total)` to `FUN_3905aca0`; `FUN_3943f420`
  passes the four values to `FUN_3943ecb0` in the call order `(global total, global remaining,
  skill total, skill remaining)`. The item-kind-9 branch (`item+0x48 == 9`) overwrites the
  skill/tag pair with the item-use cooldown (`FUN_393a6d90`: remaining = `DAT_3a17a950 - now`,
  total = 2000, when the global is not 0/sentinel and `now < DAT_3a17a950`).
* `FUN_39395510` (cast/cooldown progress) is **cdecl** with two stack arguments `(unit, out)`
  and two register inputs: `ECX` = the skill desc and `EAX` = a pointer to the kind byte
  (`7` = cast states, `8` = cooldown query).  Kind 8 calls `FUN_3989a450(unit+0x2c48, desc)`;
  kind 7 uses the same four states as `UnitCastingInfo` (normal `+0x138/+0x144`, channel
  `+0x140/+0x148`, plot `+0x74/+0x70` and `+0x84/+0x80`).  The normal/channel result is
  `total - min(total, now-start)`; the channel-delay result is `(start+total) - now`.
* `FUN_39395d90`/`FUN_39395e40` (can cancel) look up the active instance via the unit slot and
  return true when the desc's `+0x15a` (cast) or `+0x15d` (channel) byte is set; the cast one
  also accepts a controller of kind `0x22`.

**`SCCooldowns` layout.** `FUN_39899d40` reads a count at `+0`, then 150 rows of 12 bytes
starting at `+4` in the order `(id, total, remaining)`; the tag count is at `+0x70c` and its rows
start at `+0x710`. Each list is capped at 150 independently.

### 4.4 Per-weapon value selector (not a cooldown producer)

`FUN_39898ef0` (188 B) chooses one of up to eight values by unit equipment: `unit+0xd88`
(mainhand), `unit+0xb48` (offhand), `unit+0xc00` (ranged). It checks the item category at `+0x20`
(`0x15/0x16/0x17`) and the weapon type (`0x10`), then falls back to `param_3`/`param_8`.
`StartSkill` uses the result as an animation/timing id in `game_vtable+0x120`; it is **not** a GCD
producer (that is §4.2).

---

## 5. D — targeting and combat state

### 5.1 Unit tokens

`FUN_3945fcd0` (1003 B) parses a token string and returns a 64-bit value: `EAX` = kind
(`1` unit, `2` team-member object, `3` object, `0` failure), `EDX` = pointer [R, confirmed by the
capture trampoline]. It recognises the case-sensitive prefixes `player`, `playerpet`, `target`,
`watchtarget`, `team<n>`, `loginch<n>`, `previewch<a><b>`, `house` and `doodad`; after a unit is
resolved, every trailing `target` suffix moves to that unit's `+0x1aa0` target id
(`FUN_39360020`), so `targettarget` and longer chains work [R]. `target` falls back to the focus
item (`local actor+0x5f90` -> `FUN_39035110`, kind 3) when the target is empty/unresolvable; a
`target` suffix still left after that fallback is an error. `team<n>` returns kind 2 directly
when `FUN_3935e370` finds no unit. `doodad` returns kind 3. Used by `UnitCastingInfo`,
`UnitCombatState`, `UnitHealth/Mana`, `IsFirstHitByMeOrMyTeam` and many `X2Unit` bindings.

`FUN_394270a0` (826 B) is the target setter: it resolves both ids, reads/writes the unit's
`+0x1aa0` atomically, updates the target/target-of-target frames (`FUN_3920fa60` /
`FUN_3920ff80` with 2), and handles watch/team side effects. The script event meta is fired by
`FUN_390115a0`, not by this function. The `SCTargetChanged` handler
`0x391d5270` (115 B) calls it. Self-target `0x39186db7` writes the target then sends
CSChangeTarget (opcode `0x2c`).

### 5.2 Tab targeting and click selection

The seven cvars are registered by `FUN_390f7b40` with defaults read from `.data`
(`re-constants.md` §re-combat):

| cvar | config offset | default |
|---|---|---|
| `tab_targeting_round_dist` | `+0x2d0` | 4.0 |
| `tab_targeting_fan_dist` | `+0x2c8` | 50.0 |
| `tab_targeting_fan_angle` | `+0x2cc` | 60.0 |
| `tab_targeting_z_limit` | `+0x2c4` | 20.0 |
| `tab_targeting_history_expire_time` | `+0x2c0` | 3.0 |
| `tab_targeting_history_max` | `+0x2bc` | 10 |
| `tab_targeting_dir` | `+0x2b8` | 0 |

The **selection loop itself was not located**: the only static code read of `mouse_clear_targeting`
(`+0x2d4`) is in `0x390f4960`, and no function reads the `+0x2b8..+0x2d0` block inline (U4). The
action indices 22–25 are known, and the 10.x build has the same cvars, so the loop is native but
reached indirectly. The harness can settle it by hooking the CSChangeTarget send / `FUN_394270a0`
while pressing Tab and reading the caller. Click selection is `MouseClickControl::Normal`
`0x390f4960`: it picks the unit under the cursor, applies `FUN_39429e40` (clear-target rule when
`cfg+0x2d4`), and calls `FUN_394270a0`; the exact ray/pick is in `0x39429e40`/`0x3942fb50` [?].

### 5.3 Combat state, force attack, first hit

* `Unit+0x3bba` = in-combat byte. `FUN_390b36f0` sets it and calls the unit vtable `+0x50(1)`;
  `FUN_390b3710` clears it. Getter: `FUN_394bd830` (`X2Unit:UnitCombatState`).
  `SCCombatEngaged` handler `0x391d69a0` also fires UI event 3 (`FUN_39424220`) for the local player;
  `SCCombatCleared` handler `0x391d6a60` calls the unit vtable `+0x48`.
* `Unit+0x3bbc` = force-attack byte (`SCForceAttackSet` handler `0x391d3c60`; UI event `0xe8`).
  `0x3bbe` is a second, related byte used by target frames. Client-side eligibility for the
  Ctrl+F toggle and the C→S packet were not located (U5).
* `Unit+0x5fe8` = first hitter id, `+0x5fec` = first-hitter team/relation. `FUN_39354780` sets
  them from `SCCombatFirstHit`; `FUN_394bd9f0` (`IsFirstHitByMeOrMyTeam`) compares them with
  `DAT_3a5ab31c` (none) and `DAT_39d44c50` (my team). The target frame uses this for
  `S_HP_PREEMTIVE_STRIKE` [R from re-combat; verified offsets].
* `FUN_39393f30` maintains the combat-party tracker list `DAT_3a16830c`: per-party `+0x20` id,
  `+0x34` member count, `+0x44/+0x48/+0x4c` first/last hitter ids.

### 5.4 `UnitCombatState` and target-of-target

`X2Unit:UnitCombatState(token)` is `FUN_394bd830` (210 B): it resolves the token and pushes
`unit+0x3bba` (or false for non-units). `targettarget`/`playertarget…` chains come from
`FUN_3945fcd0`; `TARGET_TO_TARGET_CHANGED` and `UNIT_COMBAT_STATE_CHANGED` only refresh the Lua
frames — the client has no separate target-of-target state.

---

## 6. E — hit presentation logic

### 6.1 Damage and heal application

`SCUnitDamaged` handler `0x391d5fb0` (296 B) copies the 0x188-byte record (packet `+0x10`) and,
for a local-player target, filters/replaces some ids, then calls `FUN_39398c80` (1614 B).
`FUN_39398c80` switches on the record's source kind:

| kind | meaning | path |
|---|---|---|
| 0 | skill | resolve `FUN_393953a0(record+0x8)`, `desc = FUN_396f6390(instance+0x4)`, copy the record into the instance (0x62 dwords), apply damage, choose the hit path (`FUN_39391950`) and emit the reaction |
| 1 | item/other | `FUN_39856520` → `FUN_3938e1d0` |
| 2 | buff | `FUN_396f63d0`, `FUN_39387440`, optional target shake, `FUN_393985f0` |
| 3 | passive buff | `FUN_396f63d0` + `FUN_393985f0` |
| 4 | doodad | `FUN_3963b290` + `FUN_393985f0` + target shake |

After the switch it calls, in order: `FUN_39391670` (enter combat for source/target),
`FUN_393968b0` (aggro/threat link), `FUN_39393f30` (party last-hitter). For a local-player victim
with the "big hit" flag it fires UI event `0x72` and the camera/UI shake (`DAT_3a5bb1d8+0x3c`
vtable `+0x6c/+0x68`).

Heal: `SCUnitHealed` handler `0x391d6210` (114 B) → `FUN_393992d0` (753 B), same source-kind
switch but the presentation goes through `FUN_39392480` (heal anim/text).

### 6.2 Which reaction animation is requested

`FUN_393917f0` (295 B) maps the hit type (`record+0x48`) to `(animId, kind)` [R]; full table in
[`structs.md`](structs.md) §9. The table's two flags are `FUN_39354600(unit)` (reads
`unit+0x5fc4`, the crit/skill column) and `FUN_397cd5f0(*(u32*)(unit+0x1000))` (the weapon
variant; `true` when the item type row's `+0x74` is `0x10`) [R]. `FUN_39391950` (147 B) decides
whether the "attacker view" or "victim view" presentation runs: it returns true when there is no
hit type or `float(desc+0x188) > 0.0` — the constant at `0x3999c484` is **0.0f**, not 1.0f (the
1.0f at `0x3999c488` is a different constant). Ordinary hits therefore use the victim animation
path unless the big-hit value is positive.

`FUN_393985f0` (643 B) builds the hit-effect event (`FUN_3940a0f0` maps caster/weapon to a
`(type, anim)` pair; `+0x50..0x6c` are the effect values), and `FUN_39398880` (337 B) emits the
floating combat text.

### 6.3 Knockback/stun/knockdown on the local player

These are **buff states**, not hit-reaction decisions: `SCBuffCreated` (`0x0b6`) handler
`0x391d5e00` → `FUN_3938a3c0` (943 B) applies the buff row; the list is chosen by `buff+0xf4`
(1 buff, 2 debuff, 3 hidden) and appended by `FUN_39386e00` (120 B) into 32/24/24 rows of 0x68
bytes. The stunt/knockdown/root flags live in the buff desc and gate movement/inputs; the client
does not synthesize an extra "hit reaction state" for them beyond the buff list. `SCCombatEngaged`
and the party tracker cover entering combat on a hit; `FUN_39391670` skips mounted/attached units
(`+0x3c26/3c27`).

---

## 7. Numeric parameters (1.2)

| value | number | source |
|---|---|---|
| action slots / pages | 85 (0x55) / 4 | `FUN_39089c60`, `FUN_3930bc30`, `FUN_393f6910` |
| action record | 0x10 bytes | `FUN_39089c60` (0x550) |
| cooldown list cap | 150 (0x96) each | `FUN_39899d40`, `FUN_39899f90` |
| GCD custom formula | `custom_ms*1000/(1000+attr0x4a)` | `FUN_39899df0` |
| GCD default formula | `1e6/(1000+attr0x4a)` | `FUN_39899f30` |
| cooldown tick unit | ms | `FUN_39899e50` |
| skill instances | global table, sentinel `0xfffffffe` | `FUN_393953a0`, `FUN_39409d20` |
| UI skill event ids | 0x14 start, 0x15 fire, 0x17 autoattack stop, 0x18 casting stop | `39202db0` calls |
| cast-bar states | 4 (normal/channel/cast-delay/channel-delay) | `FUN_394bcf60` |
| target mode values | 2/3/4/8 (plus 0 normal, 6 interact) | `0x39428630…39428a30`, `0x390f50a0` |
| buff rows | 32 buff / 24 debuff / 24 hidden, 0x68 B | `FUN_39386e00` |
| tab cvars | 4 / 50 / 60 / 20 / 3 / 10 / 0 | `FUN_390f7b40`, `.data` |
| auto-attack skill id | `DAT_39ac769c` | `0x393989e0`, `0x39398410` |
| empty ids | `DAT_3a1776e0` (unit), `DAT_3a5ab31c` (first-hit none), `DAT_3a177710/728` | `.data` |
| first-hit team | `DAT_39d44c50` | `0x394bd9f0` |
| client skill size | 0x2e0 | `0x3939b8c0` |
| hit record size | 0x188 | `0x391d5fb0` |

---

## 8. Proposed port order (smallest testable leaves first)

**Phase A — pure data and math (safe synthetic differential tests)**

1. `FUN_39897990` result-name table (955 B).
2. `FUN_39391da0` / `FUN_39391e30` caster/target/object equality (129/465 B).
3. `FUN_393f8d20` action-name lookup + the 85-slot/4-page action bar model
   (`FUN_39089c60`, `FUN_39089c80`).
4. `CooldownManager` (`structs.md` §3): `39899d00/d10/d40/e50/ec0/f00/f90/a290/a3b0/a3e0/a400/
   a450` (9..768 B each).
5. GCD producers `39899df0`/`39899f30` + the attribute-0x4a getter path `398a0bc0`.
6. `FUN_393917f0` reaction table and `FUN_39391950` big-hit predicate.
7. `FUN_39392010` event payload ctor and the event-id constants.
8. `FUN_3945fcd0` token parser (state-read; a small unit table is enough).

**Phase B — read-only queries**

9. `FUN_394bcf60` `UnitCastingInfo` and the four cast-state fields.
10. `FUN_394bd830`, `FUN_394bd9f0`, `FUN_394bd190` unit queries.
11. `FUN_3943f310` / `3943f420` / `393f61c0` action-slot cooldown queries.

**Phase C — cast timeline (stateful, diffed field-by-field)**

12. `FUN_3939a810` `StartSkill` (GCD + instance creation + events).
13. `FUN_3939b010` `FireSkill`.
14. `FUN_39397ff0` `EndSkill`, `FUN_39398410` `StopSkill`.
15. `FUN_39396000`, `FUN_39395760`, `FUN_3938c520`, `FUN_39395b50`.
16. `FUN_3939bfd0`, `FUN_3939c4a0`, `FUN_3939cb80`, `FUN_3939ce90`, `FUN_39409d20`.

**Phase D — request and target**

17. `FUN_393989e0` `UseSkill`, `FUN_393925c0`, `FUN_393be4e0`, `FUN_393a0510` + evaluators.
18. `FUN_3ab63fb7` (thunk `0x39396e20`) request body + CSStartSkill bytes.
19. `FUN_394270a0` set-target, `FUN_39186db7` self-target, combat/force state functions.
20. The UI arm helpers (`39428630…39428a30`) and `ControlMouseClick` (`390f50a0`).

**Phase E — presentation and buffs**

21. `FUN_39398c80` damage, `FUN_393992d0` heal, `FUN_393985f0`, `FUN_39398880`.
22. `FUN_39391670`, `FUN_393968b0`, `FUN_39393f30` combat engagement.
23. `FUN_3938a3c0` / `FUN_39386e00` buff application (stun/knockdown state).

Rationale: A and B can be called with captured buffers and compared exactly; C only needs the
state A/B produce; D and E add packets and UI effects, and their oracle is the event/packet hook.

---

## 9. Unknowns

1. **U1 (resolved) — the two skill objects.** `FUN_393953a0(key)` returns `treeValue+0x18`, and
   `Fire` uses `local_18+0x18` from the same tree; they are the **same** cast-state object.
   `UnitCastingInfo` reads it directly. The separate 0x2e0 `ClientSkill` (client-only list
   `+0x6c`) is the other object (`structs.md` §5). Keep a pointer dump at `0x3939b0b6` as a
   regression check.
2. **U2 (partly resolved) — CSStartSkill bytes and the request body.** The unit-caster /
   unit-target / no-object body was captured live from the game server
   (`RUST_LOG=aaemu_game::net::fields=trace`, 13-byte payload):
   `u32 skill_id, u8 caster_kind, write_bc(caster_id), u8 target_kind, write_bc(target_id),
   u8 flag`; `write_bc` is a fixed 3-byte little-endian id, and `flag & 0x0f == 0` selects no
   skill object. Example (`front.json`): skill 10024, caster 41779, target 38444 →
   `28 27 00 00 00 33 a3 00 00 2c 96 00 00`. The precheck order inside `FUN_3ab63fb7` is still
   only observable through behaviour: the live goldens record result `0x08`/arg `1` for a local
   cooldown, `0x1e` for an invalid/unsupported target type, `0x2f` from the auto-attack gate,
   and success for unit-target casts. Item/mount/doodad caster bodies and ground targets remain
   uncaptured.
3. **U3 — requirement kind table.** `FUN_393a0510` switches on values 0/6/9/0xc/0xd/0x18
   (nested)/0x19/0x1a/0x1b/0x1c/0x1d/0x22/0x26; the nested sub-kind under 0x18 is read from
   `value1`. Only part of the mapping is named from `unit_reqs`; align each case with a DB row.
4. **U4 — tab-target loop and click pick.** The seven cvars are never read inline in x2game
   (only `mouse_clear_targeting`), and the cycle action indices have no static consumer. Hook
   `FUN_394270a0`/CSChangeTarget while pressing Tab and clicking.
5. **U5 — force-attack request.** The state byte and the `SCForceAttackSet` handler are known;
   the Ctrl+F sender and its eligibility checks were not located.
6. **U6 (resolved) — GCD producer arguments.** Disassembly of `FUN_39899df0`/`FUN_39899f30`
   shows the stack argument is the **unit**, passed to
   `FUN_398a0bc0(this = owner+0x1aa0, unit, 0x4a, 0)`; there is no tag-key argument. The formulas
   and the `-999` attribute floor are in §4.2. Goldens: `gcd.json` (`custom` × `attr` × disabled,
   including `attr = 0`, negative and `i32::MAX`).
7. **U7 (resolved) — queue margin.** There is no client-side queue buffer and no
   `queued_skill_margin` consumer (the string has no code xref). The only margin is the combo
   entry's controller `values[2]` (entry `+0x14`), and the client-only list removes an entry at
   `+0x2d4 + 1000 ms` (strict `>`). Proven by `client_only.json` (20 live cases) and
   `combo.json` (`Update` expiry at `start + duration + margin`). A second key press does not
   queue a skill; it is a fresh `UseSkill` request.
8. **U8 — action-bar key → slot → UseSkill.** `action_bar_button` index 82 is consumed only by
   the label updater `FUN_392c4dd0`; the native dispatcher is indirect. Hook `FUN_393989e0` while
   pressing slot keys to capture the caller and the exact arguments.
9. **U9 (resolved) — cast cancel on movement/jump.** The cancel is **server-driven**: after a
   `CSMoveUnit`/jump the game server answers `SCCastingStopped` (`0x0a5`), whose handler
   `0x391d5cf0` calls `FUN_39396000` → `FUN_39395b50`. The complete direct-call graph of
   `FUN_39395b50` is `FUN_3939b010` (fire error), `FUN_39396000` (casting stopped),
   `FUN_39398550` (combo/executor clear) and `FUN_3939b010`'s non-local path; no input,
   movement or jump function calls it or writes `Unit+0x3ac4/3acc`. The client therefore only
   mirrors the server stop; there is no separate local movement-cancel rule to port beyond the
   already-ported `StopCasting`/`SCCastingStopped` pair. `timeline_wide.json` covers
   `casting_stopped` and `stop_casting` live (20 cases each).

### Resolved by the Stage A port (`port/combat-a`, 2026-10-23)

* **U6** — the GCD producers take the **unit**, not a tag key; formulas and the `-999` floor are
  in §4.2. `gcd.json` covers `attr ∈ {0, negative, i32::MAX, …}` and disabled.
* **Big-hit threshold** — `FUN_39391950` compares against `0.0f` (`DAT_3999c484`), not `1.0`;
  the `1.0f` at `0x3999c488` is a different constant. `bighit.json` covers every branch.
* **Cooldown manager** — size `0xE28`; there is no manager-side ignore-global byte (`a3e0` reads
  `SkillDesc+0xdc`); `SCCooldowns` rows are `(id, total, remaining)` starting at `+4`/`+0x710`.
  `cooldown.json` covers empty/full/expiry/compaction and all queries.
* **Token parser** — the 64-bit return is `EAX` = kind (1 unit, 2 team member, 3 object, 0
  failure), `EDX` = pointer; the exact prefix set, the `target` fallback and the trailing
  `target` chain are documented in §5.1 and proven by `token.json`.
* **Slot queries** — `FUN_393f61c0` fills `(global pair, skill/tag pair)`; `FUN_3943f310`
  passes the global `(remaining, total)` and `FUN_3943f420` the four values in the order
  `(global total, global remaining, skill total, skill remaining)`; the item kind-9 override is
  in §4.3. `slot.json` proves the resolver.
* **Reaction table** — the exact anim ids/flags are in [`structs.md`](structs.md) §9; the weapon
  flag is `FUN_397cd5f0(*(u32*)(unit+0x1000))` and the crit flag is `unit+0x5fc4`.

### Resolved by the Stage C port (`port/combat-c`, 2026-10-24)

* **StopSkill calling convention** — `FUN_39398410` is `__thiscall` with `ECX` = the skill
  action manager (`ret 8`); the earlier synthetic fault was a `cdecl` call with a garbage
  `ECX` plus the event payload ctor, not the animation. With the live manager its core writes
  are `Unit+0x3ad5 = 0`, `FUN_39368f90(desc)` → `FUN_39368440` inserting the auto-attack
  cooldown (`desc+0x88`, 300 ms) and UI event `0x17`. `stop.json` has 20 live cases.
* **Combo entry key** — `SkillComboMan::Add` (`FUN_3939ce90`) keys entries by **`entry+0x08` =
  the next skill id**, not by the current one. Layout: `+0x00` original starter, `+0x04`
  current skill, `+0x08` next skill, `+0x0c` start, `+0x10` duration, `+0x14` margin.
  `Update` (`FUN_3939cb80`) expires at `start + duration + margin` and calls
  `FUN_39368f90(next_desc)` → `FUN_39368440`, which inserts the next skill's own cooldown
  (`desc+0x88`) — it is not just a clear. `combo.json`: 20 add cases and 20 update cases.
* **UpdateClientOnlySkills** — `FUN_3939bfd0` calls `FUN_3939b010` (`FireSkill`) with the
  client skill's stored blocks, **not** `StartSkill`; `+0x2d0` fires (strict `<`), `+0x2d4`
  marks the local damage, and the entry is removed after `+0x2d4 + 1000 ms` (strict `>`). The
  fire args are 12 dwords: `(skill, target, caster, target, object, +0x138, 0, +0x13c,
  +0x140, 0, 0, 0)`; `FireSkill` writes `+0x13c = arg7` and `+0x140 = arg9`. `client_only.json`
  records 20 live transitions with the Fire call arguments.
* **UseSkill argument order** — `FUN_393989e0(ECX=manager, unit*, skillId, caster*, target*,
  object*)`: the third block is the **caster** and the fourth the **target**. The map's
  `(target, caster)` naming was reversed; the live `CSStartSkill` fields prove it
  (`caster` = local, `target` = selected unit). The wrapper's target-type fix-ups replace the
  caster for `desc+0x1b0 ∈ {1,4,5}`.
* **Refusal text** — `FUN_39897990` is the complete result-name table (`SUCCESS` …
  `EXCEPT_READY_QUEST_CONTEXT`); every name is ported byte-for-byte. `FUN_39390900` packs the
  low-byte result and high-word arg (`Refusal::packed`), looks up the object name for
  item/buff/doodad/quest results and fires UI event `0x15` with `(result name, object name,
  value)`; the object-name resolvers are UI-table lookups and are modelled as caller data.
* **Unit requirements / request prechecks** — for the ported skills (`or_unit_reqs = f`) the
  body returns success; the observed client refusals are the local cooldown (`0x08`/arg 1),
  invalid/unsupported target type (`0x1e`), and the auto-attack relation gate (`0x2f`). The
  30 evaluator kinds remain a data-driven dispatcher (`FUN_393a0510`); their per-kind
  `unit_reqs` alignment is still U3.

### Resolved by the Stage B port (`port/combat-b`, 2026-10-23)

* **UnitCastingInfo** — the fake-callback goldens prove the descriptor order
  `(name, total, current, visible)`.  Normal cast reads `+0x138/+0x144`, channel
  `+0x140/+0x148`; the two plot branches read `+0x74/+0x70` and `+0x84/+0x80` and always push
  visible `true`.  `current` is `min(total, now-start)` for cast/cast-delay and
  `total - min(total, now-start)` for channel; channel-delay is `(start+total)-now`.
* **Cast progress** — `FUN_39395510` is `cdecl(unit, out)` with `ECX` = desc and `EAX` = the kind
  byte, not a manager argument; the manager is derived from `unit+0x2c48`.  See §4.3.
* **Cancel checks** — `FUN_39395d90`/`FUN_39395e40` read desc `+0x15a`/`+0x15d` and controller
  kind `0x22`; the goldens cover both true and false branches.
* **StartSkill timings** — the instance value is keyed by the target key.  `value+0x138` is the
  cast total and `value+0x144` the cast start; `+0x140/+0x148` are channel total/start.  The
  normal cast slot `Unit+0x3ac4` is set only when the cast-time argument is non-zero; the
  server-mirror flag (the low byte of the 9th stack argument, `param_10`) creates no instance
  and only emits event `0x14`.  A zero target key with skill `DAT_39ac769c` (2) arms
  `Unit+0x3ad5`; any other skill is the "impossible case".
* **FireSkill timings** — `param_7` lands at `value+0x13c`, `param_9` at `value+0x140` and the
  current engine time at `value+0x148`.  A non-zero normal total clears `Unit+0x3ac4`; a desc
  with `+0x6c > 0` sets `Unit+0x3acc` to the instance key.  The fallback path calls StartSkill,
  so it emits `0x14` before `0x15`.
* **End/stop/casting stop** — `EndSkill` removes the instance/clears its slots and always emits
  `0x16` (even when the instance is missing).  `StopSkill` only acts on the auto-attack id and
  emits `0x17`; `StopCasting` just clears the matching slot; `SCCastingStopped` adds the packet
  duration as a skill cooldown and emits `0x18`.  `SCCastingDelayed` adds to `+0x138` while the
  cast is live.
* **Combo** — `SkillComboMan::Add` keys entries by the current skill id and stores the **next**
  skill id from controller kind `0x18` sub `0x30` (`values[1]`), the start, the duration and
  `values[2]` as the expiry margin.  `Update` expires at `start + duration + margin` and clears
  the next skill's cooldown.  The goldens are synthetic: driving `FUN_3939ce90` in the harness
  reaches `FUN_3930bb30` for a skill that is not on the action bar and faults there.
* **Client-only list** — `+0x2d0` fires, `+0x2d4` marks the local damage, `+0x2d4+1000` ends and
  removes the entry; the fields are in `structs.md` §5.2.  Synthetic tests cover the transitions
  because the live element and its hit record are created by the game, not the harness.
* **UI event ids** — `0x14` start, `0x15` fire, `0x16` end, `0x17` auto-attack stop, `0x18`
  casting stopped.  The harness captures them by redirecting the five `call 0x39202db0` sites to
  an RWX stub that records the id and tail-jumps; a stub that pushes `pushfd/pushad` and restores
  them crashed the client, a stub that only clobbers EAX/EDX does not.  Fire's normal path did
  not reach its event call in the harness (the state writes still matched), so its `0x15` id is
  from the decompile and the `SPELLCAST_SUCCEEDED` Lua callback.
* **U7** remains open (no explicit queue margin; the client-only timestamps are fixed
  `+0x2d0/+0x2d4` with a 1000 ms grace). **U9** remains open (movement/jump cancel is outside the
  ported timeline; `FUN_39395b50` still has only the three server-driven callers).


---

## 10. Cross-check against `re-combat.md`

Agreements: `CSStartSkill` is `0x052`; `SCSkillStarted/Fired/Ended/Stopped/CastingStopped/
CastingDelayed` handlers and their addresses; `UnitCastingInfo` four states and the 0x138/0x140/
0x144/0x148 fields; 85 slots/4 pages/1-based slots; buff rows 32/24; 150-entry cooldown lists;
first-hit fields; `IsFirstHitByMeOrMyTeam`; target tokens; tab cvars and defaults.

Sharpened or corrected:

* The client **requests** a skill in `FUN_393989e0` → tail `FUN_3ab63fb7`; the local cast starts
  in `FUN_3939a810` only when `SCSkillStarted` arrives. `FUN_3939a810` is not the request builder.
* The cooldown manager is **inline at `unit+0x2c48`**, and the two clientside producers
  `FUN_39899df0`/`f30` write the **global** pair `+8/+0xc`, not a skill cooldown. Their formulas
  are in §4.2; `global_cooldown_mul` is the attribute `0x4a` (resolves open question 1).
* `FUN_39899f90` is the per-skill/tag cooldown inserter (150 cap), not a GCD writer.
* Client-side range/LOS/facing checks are **not** in `UseSkill`; the client packs the target and
  the server validates. The client-side prechecks are unit requirements + target override /
  auto-attack gates (§2.6).
* `re-combat`'s "pending cooldown-tag estimate" at StartSkill is the GCD estimate, not a skill
  cooldown estimate.
* The first-hit fields are `unit+0x5fe8/+0x5fec` (not `+0x...` as guessed).
