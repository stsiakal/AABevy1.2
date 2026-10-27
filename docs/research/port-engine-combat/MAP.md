# Port map: the engine links of 1.2 client combat

> **Restored 2026-10-27 from `archive/fork-main-2026-10-27`, UNVERIFIED.** The client was restarted from NickMesser's `main`; nothing described here is wired into the game. Addresses, call chains and offsets must be re-checked against the decompile ([R]/[I]/[?]) before use. A 2026-10-27 audit found 151 of the 269 EXACT/TOLERANCE labels in `proof-levels.*` weak or wrong (report: `D:\aaemu-client\handoff\2026-10-27\audit\report-proofs.md`).

Address map of everything between `x2game`'s combat code (`docs/research/port-combat/MAP.md`) and
what the player sees and feels when casting and when being hit: the skill-animation scheduler, the
layer/weight maths, the animation-event (fire-time) pump, weapon visuals, hit reactions, projectiles,
camera shake and combat stance. For a function-by-function Rust port and for differential tests
against the running hidden 1.2 client (`aadriver.dll`). Roadmap v2 **E6**, phase relevance **P2**.

## 0. Sources, conventions and licensing

### 0.1 Licensing

Derived **only** from the shipped 1.2 binaries and their decompiles. The published CRYENGINE
source was not used, quoted or consulted. Class names in `CClass` form come from the binaries' own
RTTI/profiler/log/assert strings or from the named 10.x build; behaviour that has no name in the
binary is marked *(inferred)*. Port from this document and the decompiles, never from the
CRYENGINE source.

### 0.2 Sources

| source | path | base |
|---|---|---|
| x2game decompile | `C:/AAEMU_DISK/aa-re/client12/out-x2game/x2game.dll.c` (+ `funcs.csv`, `calls.tsv`, `strings.txt`, `strings-xrefs.tsv`) | `0x38FF0000` |
| x2game image + `.data` | `D:/aa-re/client12/x2game_unpacked_data.dll` | `0x38FF0000` |
| CryAction / CryAnimation / CryEntitySystem | `D:/aa-re/client12/engine/out/` | `0x304f0000` / — / `0x32000000` |
| Cry3DEngine / CryMovie / CryInput / x2ui | `D:/aa-re/client12/engine2/out/` | see E1 |
| CrySystem / CryScriptSystem | `D:/aa-re/client12/engine3/out/` (`crysystem.dll.c` now exists) | `0x36500000` |
| existing maps | [`port-combat/MAP.md`](../port-combat/MAP.md), [`port-engine-anim/MAP.md`](../port-engine-anim/MAP.md) (E2), [`port-engine-loop/MAP.md`](../port-engine-loop/MAP.md) (E1), [`port-engine-char/MAP.md`](../port-engine-char/MAP.md) (E3), [`port-movement/MAP.md`](../port-movement/MAP.md) §4, [`port-mount-anim/MAP.md`](../port-mount-anim/MAP.md) | — |

Evidence markers: **[R]** read from the binary/decompile, **[I]** inference, **[?]** open.
All addresses are VAs at the preferred bases above; x2game `0x39xxxxxx` are in the same image.

### 0.3 What this map does *not* redo

* The x2game combat logic itself (skill use, packets, cooldowns, targeting, damage dispatch,
  reaction-table selection) is [`port-combat/MAP.md`](../port-combat/MAP.md). This map starts at
  the calls that map leaves at "start the animation / notify the game client".
* The CryAnimation playback internals (clip time, playback-speed scaling, FIFO entries, layer
  ramps, root motion, attachments, IK) are E2 [`port-engine-anim/MAP.md`](../port-engine-anim/MAP.md).
  This map only adds the x2game-side producer of those commands and the **layer numbers, weights,
  flags and the animation-event names** that E2 does not cover.
* The frame order and the timer are E1 [`port-engine-loop/MAP.md`](../port-engine-loop/MAP.md).
  This map only fixes where the combat animation work sits inside that frame (§9).
* The camera itself is [`port-movement/MAP.md`](../port-movement/MAP.md) §4. This map adds the
  combat/skill camera-request block it does not cover.

### 0.4 Two corrections to the combat map

1. **`FUN_3940a3e0` is not the animation start.** It builds the floating combat-message UI payload
   (`"FireUiCombatMessage"`, `MELEE_DAMAGE`/`PHYSICAL`/`DROWNING`/`HEALTH` strings, 2,008 B). The
   annotation in `port-combat/MAP.md` §3.1 step 8 ("start the skill animation/controllers") and
   §3.3 step 9 ("build the hit/hit-effect block") only refers to the damage-text half. The real
   animation start is `game_vtable+0x120` = the unit model's `PlayAnim`, mapped in §2 here.
2. **`desc+0x10` is `active_weapon_id`, not an animation id.** The `skills` SELECT column order
   (`id, ability_id, ability_level, actability_group_id, active_weapon_id, …`) puts the 5th u32 at
   `+0x10`; `PlayAnim` uses it as the *weapon key* of the animation entry, and value `3` means
   "keep the current weapon" (`FUN_39157680` replaces a `3` with `*(controller+0x6c)`). The same
   enum appears in `FUN_39360920` (weapon slots 1/2/4, `3` = current).

### 0.5 Phase relevance

E6 is a **phase 2** map. Phase 1 (movement/riding) needs only two things from here, and both are
already covered elsewhere: the per-frame `ClientUnit::Update` slot (§9) and the model/controller
objects (§1.2). The links marked **phase-2 critical** are §2 (scheduler + layers), §3 (cast/fire),
§4.2-4.3 (weapon swap), §5.3 (hit timing), §6.1 (fire-time events) and §8.1 (combat stance).

---
---

## 1. The handover: x2game combat -> model -> animation controller -> engine

### 1.1 The one-screen chain

```
SCSkillStarted (0x0a1)
└─ StartSkill                              x2game 0x3939a810
   ├─ anim id = FUN_39898ef0( packed skill/weapon key )      combat map §3.1
   └─ unit_model->vt[0x120](&out, animId, desc+0x10, data, 0, castSec, chanSec)
        x2game model slot -> FUN_390bb430 (66 B, adjustor)
        │  ecx = *(model+0x164)                // the anim controller
        └─ FUN_39157680(controller, out, animId, weaponId, data, 0, cast, channel)   2442 B
           ├─ stop ids 255/256 -> FUN_39155de0  "additive_all_re_combat_miss"
           ├─ id -> desc  FUN_39156310 (id map) / FUN_39155d50 (name map)
           ├─ overlay path (controller+0x20) -> FUN_391572f0
           │    └─ char->vt[0x14]() = ISkeletonAnim -> vt[0x34](name,0,0,0,&params)
           └─ normal path: build entry, FUN_39156e00 -> list +0x24 / +0x30
                 └─ FUN_39156e50: read the CAF events of the clip
                      "combat_sync" -> entry+0x30, "<name>_start" -> entry+0x34,
                      clip length -> entry+0x2c

per frame (unit update)
└─ X2::GameClient::ClientUnit::Update       x2game 0x3936a0b0
   ├─ model->vt[0x260] = X2::ActorUnitModel::Update 0x390dd080
   ├─ FUN_39899e50(ms)                       cooldown tick (combat map §4)
   ├─ FUN_393888b0(ms)                       ClientBuffMan::Update (buffs, buff anims)
   └─ ...
       (the anim controller tick `FUN_39156ac0(controller, seconds)` has no static caller;
        it is a virtual called once per frame, see U1)
        ├─ FUN_39155660(entry, now, apply)   per-entry event pump -> fire-time events
        │    └─ FUN_39395950(skillMan, instId, eventType, vec3) -> VFX/projectile
        └─ FUN_39155930(entry, now, apply)   end-of-anim "hit" text / next anim

SCSkillFired (0x0a2) -> FireSkill 0x3939b010 -> the *second* model vt[0x120] (fire anim)
SCSkillEnded (0x0a3) -> EndSkill  0x39397ff0 -> model vt[0x120]( &out, 46, 3, target, 0,0,0 )
SCUnitDamaged        -> FUN_39398c80 -> FUN_393917f0 -> model vt[0x144] (victim) / vt[0x120] (attacker)
```

### 1.2 Objects and fields (all **[R]** unless noted)

**Unit** (`X2::Unit`, the `this` of the combat code):

| off | meaning |
|---|---|
| `+0x3ba8` | active weapon id (1 mainhand / 2 offhand / 3 keep / 4 twohand; `DAT_3a1699f8` = none) |
| `+0x3bb0` | weapon-visual sub-block passed to `FUN_3935a780`/`FUN_39360580` |
| `+0x3be8` | unit model (`X2::ActorUnitModel`, vtable `0x399a4f68`) |
| `+0x3c30` | weapon-visual flag byte (read by `UpdateUnitWeaponVisuals`) |
| `+0x3bba` | "in combat" byte (first hit / force attack) |
| `+0x5fc0` | last-combat millisecond timestamp |
| `+0x5fc4` | "in combat" latch (1 s samples, 10 s timeout) |
| `+0x5fc8/5fcc` | 1 s grace timers for `+0x5fc5/+0x5fc6` |
| `+0x5fd8` | last buff/effect animation timestamp |
| `+0x3ac4/+0x3ac8/+0x3acc/+0x3ad0` | cast/channel slots (combat map §3.2) |
| `+0x2c48` | inline cooldown manager (combat map §4) |

**Unit model** (`X2::ActorUnitModel`, vtable `0x399a4f68`; base `X2::UnitModel` ctor
`FUN_390e4cb0`, vtables `0x399a4c90/4c88/4c84`):

| off | meaning |
|---|---|
| `+0x14` | animation-graph holder / appearance descriptor (`Init` reads it; `+0x428` = in-combat bool) |
| `+0x164` | **anim controller** object (the scheduler's `this`); the only field the combat path reads |
| `+0x1f0` | `IEntity*` (used by `Init`, `vt[0x78]/[0x90]` position calls) |
| `+0x230` | decal/effect child list (`FUN_390c5fa0`, `FUN_390c6080`) |
| `+0x238` | shadow/effect slot |
| `+0x44` | model refcount (`Unit::ReleaseModel`) |
| `+0x100` | `UnitModel::Update` timer (`FUN_390c3d00`) |

The three vtable pointers at `+0x00/+0x04/+0x08` are the multiple-inheritance bases of
`X2::UnitModel`; do not read them as data.

**Anim controller** (`X2::ActorUnitAnimController`, name inferred; `*(model+0x164)`):

| off | meaning |
|---|---|
| `+0x04` | unit/actor (has `[+0x0c]` = entity, `[+0x38]` = owning unit) |
| `+0x08` | id/name -> `AnimDesc` map (two std::maps, `FUN_39156310`/`FUN_39155d50`) |
| `+0x0c` | `IAnimationGraphState*` (engine; `vt[4]` SetInput name, `vt[8]` SetInput int, `vt[0x24]` GetValue, `vt[0x5c]` GetInputTime) |
| `+0x10`, `+0x12`, `+0x14`, `+0x18`, `+0x1a`, `+0x1c` | interned graph input ids (Action/Stance/… in the order `FUN_39154090` uses) |
| `+0x24`/`+0x28` | list A + count (base/full-body) |
| `+0x30`/`+0x34` | list B + count (action/upper body) |
| `+0x40+type*4` | per-type current anim id |
| `+0x68` | apply-now flag |
| `+0x6c` | last weapon id |
| `+0x70` | last update time (s) |
| `+0x74/+0x78` | pending-event / clear flags |
| `+0x7e` | "fist idle" latch (see `FUN_39154ea0`) |
| `+0xa0` | clear-graph-input latch |

**Anim entry** (payload 0x44 bytes at list-node+8, node 0x4c, allocated `FUN_39155c40`;
the CAF event records are 0x34-byte nodes with a 0x2c-byte payload, `FUN_39155bb0`):

| off | meaning |
|---|---|
| `+0x00` | entry id (from `controller+0x3c` counter) |
| `+0x04` | `active_weapon_id` of this entry |
| `+0x08` | animation id |
| `+0x0c` | ? (set from the call's `flags`/data) |
| `+0x10/+0x14/+0x18` | reaction schedule written by `FUN_39157130` (start / anim id / target unit id) |
| `+0x1c` | animation descriptor (node `AnimDesc` in the controller map) |
| `+0x20` | skill instance id / cast key |
| `+0x24` | data pointer (skill object) |
| `+0x28` | start time (s); 0 until the tick first sees the entry |
| `+0x2c` | clip length / duration (s) = `desc+0x2c / speed` |
| `+0x30` | `combat_sync` (fire) time (s), from the CAF event list; also the hit delay |
| `+0x34` | `<name>_start` sub-animation time (s); also the "hit armed" byte |
| `+0x38` | cast time (s) requested by the skill |
| `+0x3c` | channel time (s) |
| `+0x40` | mode 1 = cast only, 2 = cast+channel |

Descriptor fields (inside the map node, `node+0x10` is the descriptor base): name at `+0x14`
(node), events at `node+0x38`, duration at `node+0x3c`, blocking flag at `node+0x30`, hit timing
at `node+0x40` — i.e. descriptor-relative `+0x04`, `+0x28`, `+0x2c`, `+0x20`, `+0x30`.

### 1.3 The model slot numbers that matter for combat

| slot | thunk | target | role |
|---|---|---|---|
| `vt[0x120]` | `FUN_390bb430` | `FUN_39157680` | **PlayAnim(out, animId, weaponId, data, flags, castS, chanS)** -> returns the id actually played |
| `vt[0x144]` | `FUN_390bb4e0` | `FUN_39157130` | **SetHitReaction(animId, …)** — victim-side reaction with weapon-timed start (§5.3) |
| `vt[0x260]` | — | `0x390dd080` | `ActorUnitModel::Update` (per-frame model visuals; E3/E2) |
| `vt[0x1b8]` | — | actor getter | the entity's character / actor interface used by all animation calls |
| `vt[0x1e4]/[0x1e8]/[0x1ec]` | — | — | weapon-visual sub-objects (holdable desc, attachment set, "has weapon") |
| `vt[0x204]` | — | — | unit mode/attachment state (used by weapon visuals) |

The `+0x120` signature is fixed by its four x2game callers: `(&out, animId, weaponId, data,
flags, castSeconds, channelSeconds)`; `out` receives the id that is actually running (0 when the
anim was rejected or replaced by a stop).

---

## 2. The skill-animation scheduler (`FUN_39157680` and the controller)

### 2.1 What data it uses

The scheduler is driven by the client DB, not by the `.ag` graph alone. Two tables matter for the
layer choice (loaded at startup by `FUN_397287e0` and the `LoadAnimRuleDescs` /
`LoadAnimActionDescs` siblings):

* **`anims`** (`LoadAnimDescs`, `FUN_397287e0`): `id, category_id, hang_ub, loop, move_ub, name,
  relaxed_ub, ride_ub, swim_move_ub, swim_ub`. Stored as a parallel u32 id array at
  `descManager+0xb0c` plus records of stride **0x24** at `+0xb10`:
  `{+0x00 category_id, +0x04 hang_ub, +0x08 loop, +0x0c move_ub, +0x10 name*, +0x14 relaxed_ub,
  +0x18 ride_ub, +0x1c swim_move_ub, +0x20 swim_ub}`. The six `*_ub` bytes are the **upper-body
  selection**: which animation is played on the upper body in the hang / move / relaxed / ride /
  swim / swim-move states. `category_id` is the key used by the transition rules.
* **`anim_rules`** (`LoadAnimRuleDescs`): `id, before, before_operator_id, default_operator_id,
  first_category_id, second_category_id`. `FUN_396e23e0` exposes it as 5-int records
  `{threshold_time, kind_if_before, kind_else, from_category, to_category}` (stride 0x14);
  `10` is the wildcard category. `FUN_39153ef0` / `FUN_39153f80` read it (§2.5).

The **per-character** side is the id/name map at `controller+8` (`FUN_39156310` by id,
`FUN_39155d50` by name). A node holds the animation id (`node+0xc`), the engine animation
descriptor (`node+0x10` -> `+0x14` name, `+0x3c` duration, `+0x38` event list) and its flags.
The engine animation **name** (not the numeric id) is what the character instance is asked to play.

Skill-side animation ids come from `skills` (`start_anim_id`, `fire_anim_id`,
`channeling_anim_id`, `dual_wield_fire_anim_id`, `twohand_fire_anim_id`, the six instrument ids,
`match_animation`, `match_animation_count`, `use_anim_time`) and are selected per weapon by
`FUN_39898ef0` (combat map §4.4) before the call. `active_weapon_id` (`skills` column 5,
`desc+0x10`) travels with the request and selects the animation set.

### 2.2 `FUN_39157680` -> `FUN_390bb430` (model slot `+0x120`), 2,442 B

Call shape `PlayAnim(animId, weaponId, data, flags, castS, chanS) -> outAnimId`:

1. `if (*(controller+8) == 0) return 0` — no animation set, nothing plays.
2. `if (weaponId == 3) weaponId = *(controller+0x6c)` (keep the running weapon). The result is
   stored back to `+0x6c` at every exit (`*+0x6c = weaponId`).
3. **Stop ids.** `animId == 255` or `256` (`DAT_39ac3f08/0f0c`): clear `+0x78` and play the
   additive `"additive_all_re_combat_miss"` (`FUN_39155de0`, §2.6); the lists are not cleared
   here. Returns 0. These are the two ids the reaction table uses for the weapon variant of hit
   types 4/10/14 (combat map `structs.md` §9): a whiff/miss reaction is *additive*, not a body
   animation.
4. **Player attack-speed hook.** If `animId` is 14 or 17 (`DAT_39ac3f00/0f04`) or 386/387 and the
   actor type is `"CPlayer"` (actor vt[0xf4]), call
   `entity->vt[0x168](0)->vt[0x18]()->vt[0x170](DAT_39aefb98+0x2ec, +0x2f0)` — two player-side
   animation/speed globals (exact meaning **U3**).
5. **Front/back attack variant.** 14/17 -> `FUN_39153af0(data)`: reads the skill instance's
   `+0x2f8` (angle in radians), converts with 57.29578 and returns `0` (no instance), `2` if the
   angle is **< 181 deg**, else `1`; 14 -> 417, 17 -> 418 (`DAT_39ac3f18/1f1c`). This is the
   front/back attack animation switch, **not** 1H/2H.
6. **Name lookup.** `FUN_39156310(&node, &animId)`; missing -> return 0 (`DAT_39afd370`).
   `node+0x14` is the animation name. Special names:
   * `"all_co_sk_stop"`, `"all_ac_stop"` -> stop flag; `FUN_39156760(animId,0)` and
     `FUN_39156760(animId,1)` for both lists; if either was running, set `+0x68` and return 0.
   * `"all_re_stop"` -> return 0 without playing.
   * otherwise, if `weaponId != 0`, `FUN_39156760(animId,0/1)` removes a previous copy.
7. **Overlay path** (`controller+0x20 != 0` and `FUN_391564e0(animId)` finds a name-addressed
   overlay descriptor): `FUN_391572f0` (§2.4).
8. **Normal path.** Compute the speed `local_1c` from `FUN_39355a80()` when the unit has an
   attack-speed source; `duration = desc+0x2c / speed`. `mode = 2` when both cast and channel
   times are > 0, `1` when only cast > 0, else `0`. Build the 0x4c-byte entry (§1.2), append it
   with `FUN_39156e00`, then `FUN_39156e50` resolves the CAF events (§2.3).
9. **Blend/transition decision.** `busyA = controller+0x28 != 0` (list A), `emptyB =
   controller+0x34 == 0`. If both lists are empty the entry is queued directly (`LAB_39157ffa`).
   Otherwise, when the new anim is non-blocking (`desc+0x20 == 0`) and list B has a current entry
   with `FUN_39153ef0(listBHead, animId, 0) >= 1`, switch on the returned kind 1..6 (§2.5) and
   stop either list A or list B; otherwise stop list A if busy. The blocking case uses the same
   decision with list B as the source. `*(controller+0x6c) = weaponId` on every branch.
10. `out = entry id` (0 when nothing was queued).

`FUN_39156e00` (69 B): allocate with `FUN_39155c40(list, sentinel, entry)` (0x4c bytes), append
to the circular list, `count++`.

### 2.3 The CAF events become entry times (`FUN_39156e50`, 593 B)

`FUN_39156e50(entry, desc, animInterface, flag)` copies the command block and then, when an
animation interface is passed:

```
interface = char->vt[0x108]()->vt[0x9c]()          // animation event provider
animIdx   = interface->vt[0x18](name)              // index by name
clipLen   = interface->vt[0x4c](animIdx)           // seconds
syncIdx   = interface->vt[0x30](animIdx)           // event index by name "combat_sync"
sync      = interface->vt[0x24](syncIdx,"combat_sync")   // normalised 0..1
entry+0x2c = clipLen
entry+0x30 = sync * clipLen                        // the FIRE time
interface->vt[0x2c](syncIdx, buf)                  // event payload list
  -> each payload becomes a 0x34-byte event record (FUN_39155bb0)
startIdx  = interface->vt[0x18]("<name>_start")
entry+0x34 = interface->vt[0x4c](startIdx)         // cast-start sub-animation length
```

`combat_sync_framehold` (`0x399a7868`) and its log line
`"FrameHold at combat_sync animation event[0(off)|+(holdtime)]"` are the hold variant: the
`combat_sync` payload carries an optional hold time that delays the fire by that many seconds.

### 2.4 The overlay path (`FUN_391572f0`, 449 B)

Used for `anim_actions` / emotes / jump / idle overlays (anything reachable by *name* through
`FUN_391564e0`):

1. `FUN_39108de0(animName, data)` — graph/UI side effect.
2. `char = unit->vt[0xc]->vt[0x168](0)`; `anim = char->vt[0x14]()`; then
   **`anim->vt[0x34](name, 0, 0, 0, &params)`** — `ISkeletonAnim::StartAnimation(name, layer=0,
   …)`. `params` is the 0x24-byte block (`+0x14` layer, `+0x18` flags), §2.6.
3. `FUN_39155d50(name, &id)` resolves the numeric id; missing -> abort.
4. Clears list A (`+0x24`) and appends a new entry (weapon id 0, `entry+0x2c` = desc+0x2c,
   `entry+0x38/0x3c` = the caller's times).
5. `FUN_393953a0(dataKey)` -> if a skill instance exists, `instance+0x308++`.

This is the "one animation, no blending" path: layering is entirely the engine `layer 0` start.
Overlays that must blend with locomotion use the normal path.

### 2.5 Weights, fades and the transition kinds

`FUN_39156ac0(controller, now)` (612 B, the per-frame tick) is the only place the two lists are
consumed:

```
if (controller+0xa0) { +0xa0 = 0; graph->vt[4](key, "idle", 0); }
FUN_391555d0(controller)                 // current action id -> graph "Action" input
FUN_39156160(controller)                 // list B head with data kind 6, already running > 0.2 s -> +0x68
FUN_391545e0(controller)                 // attack-speed / weapon-state graph keys
FUN_39154ea0(now - +0x70)                // "fist_ba_relaxed_rand_idle" idle fixup
FUN_39154550(now - +0x70)                // layer-0xc weight ramp (engine vt[0x78])
entry = listA head if count != 0 else (prefer the list-B entry whose desc starts with 0xb)
if (entry) {
    if (entry+0x28 == 0) { entry+0x28 = now; +0x68 = 1;
        if (entry+0x1c /*data*/ && entry+0x04 != 3) FUN_39360920(unit, entry+0x04)  // weapon swap
    } else if (now - entry+0x28 >= entry+0x2c && !blocked) {
        FUN_39156760(entry+0x20, 0); +0x68 = 1;
    }
}
if (+0x68) {
    +0x68 = 0;
    graph->vt[0x24](key, buf);                       // current graph value
    cur = FUN_39155560(1, now);                      // current entry
    action = FUN_39154ad0(cur+0x1c, cur+0x20);       // build the "Action" value string
    FUN_391547b0(1.0);                               // char->vt[0xb4](1.0)
    if (*(char*)(cur+0x1c + 0x20)) {                 // descriptor blocking/full-body
        graph->vt[4](key);  graph->vt[4](key2, "none", 0);
        FUN_391547b0(entry+0x2c / entry+0x38);
    } else {
        +0x78 = 0;  graph->vt[4](key, action, 0);
    }
}
+0x70 = now;
```

Engine calls used, all on the character instance:

| call | object | meaning |
|---|---|---|
| `char->vt[0xb4](float)` (`FUN_391547b0`) | `ICharacterInstance` | animation **scale/weight** setter [I: E2 `SetAnimationSpeed` `0x31520270`, `char+0x278`; 1.0 restores] |
| `anim->vt[0x78](0xc, weight, dt, 0)` (`FUN_39154550`) | `ISkeletonAnim` | ramp **layer 12** to `weight` over `dt`; weight is 1.0 when `unit+0x5f84` is set, else `DAT_3999cc1c` = **-1.0** ("leave") |
| `graph->vt[4](key, value, 0)` / `vt[8](key, int, 0)` | `IAnimationGraphState` | set a graph input |
| `graph->vt[0x24](key, buf)` / `vt[0x5c](key)` | `IAnimationGraphState` | read the value / its running time |
| `anim->vt[0x34](name,0,0,0,&params)` | `ISkeletonAnim` | start an animation by name (overlay path) |

**Transition kinds** (`FUN_39153ef0`, 129 B): returns `4` when no rule matches, else the rule's
`before`/`default` operator. `FUN_39157680` uses: `1..4` = stop the old list and start the new one
(`FUN_39155b70`); `5` = keep the old group on the body and add the new entry; `6` = speed-scale
the old entry with `FUN_39153f80` (multiplier * 0.01, `DAT_3999cf90`).

### 2.6 Stop, remove, additive miss

| function | address | size | role |
|---|---|---|---|
| `FUN_39156760(animId, list)` | `0x39156760` | 394 | stop one animation by id in list A (0) or list B (1); returns whether it was running. The list-B branch ends with `char->vt[0xb4](1.0)` |
| `FUN_39156900(animId)` | `0x39156900` | 410 | remove a group entry by id (model buff-remove path); if it was the list head, reset the body weight and set `+0x68` |
| `FUN_39155b70(list)` | `0x39155b70` | 42 | clear a list |
| `FUN_39155de0()` | `0x39155de0` | 236 | play the additive `"additive_all_re_combat_miss"` (no list change) |
| `FUN_39155ed0(animId, value, force)` | `0x39155ed0` | 160 | remove-by-type: type 0 = graph input, 1 = `FUN_39153b60` + `FUN_39155310`, other = `+0x40+type*4`; `force` triggers `FUN_391555d0` |
| `FUN_39155f70(type)` | `0x39155f70` | 162 | restore the per-type animation `+0x40+type*4` (types 5..8) |
| `FUN_391564e0(animId)` | `0x391564e0` | 49 | overlay-descriptor lookup |
| `FUN_391555d0()` | `0x391555d0` | 129 | push the current action id into the graph `"Action"` input when changed |
| `FUN_39156160()` | `0x39156160` | 124 | force apply when the graph input time of the entry name is > 0.2 s |
| `FUN_39154ea0(dt)` | `0x39154ea0` | 688 | idle fixup: `"fist_ba_relaxed_rand_idle"` -> `"idle"`; relaxed-idle drive |
| `FUN_391545e0()` | `0x391545e0` | 413 | attack-speed graph keys |
| `FUN_39154550(dt)` | `0x39154550` | 143 | ramp layer 0xc weight |
| `FUN_391547b0(w)` | `0x391547b0` | 72 | `char->vt[0xb4](w)` |
| `FUN_39155560(flag, now)` | `0x39155560` | 108 | entry picker (list A first, then list B preferring the `0xb` descriptor) |
| `FUN_39158090(record)` | `0x39158090` | 110 | copy an entry (0x38 bytes + nested list) |
| `FUN_39158160(map, out, left, node, entry)` | `0x39158160` | 543 | red-black insert for the descriptor map |

The additive miss block (`FUN_39155de0`) is the template for every "no-body" reaction:

```
params = { +0x00 0.1, +0x04 -1.0, +0x08 1.0, +0x0c 1.0, +0x10 1.0,
           +0x14 3 (layer), +0x18 0x80 (flags), +0x1c..+0x28 0,
           +0x2c "ie", +0x30 0 }
anim->vt[0x34]("additive_all_re_combat_miss", 0, 0, 0, &params)
```

`params+0x14 = 3` is the additive layer selector, `+0x18 = 0x80` the additive flag (the same
flag set E2 documents at `CSkeletonAnim+0x45c/+0x46c`). This is the only explicitly additive
animation in the combat path; every casting/hit body animation goes through the normal path.

---

## 3. Cast, channel and fire: which animation actually plays

### 3.1 Start of a cast (`StartSkill` -> `PlayAnim`)

`StartSkill` selects the animation **id** and calls the model once:

```
key = FUN_39005b20(unit+0x4b8, desc+0x154, desc+0x164, desc+0x118, desc+0x1c8,
                   DAT_3a177728, DAT_3a177728)
animId = FUN_39898ef0(key)                 // per-weapon selector, combat map §4.4
if (animId != DAT_3a177728 /* none */)
    model->vt[0x120](&out, animId, desc+0x10 /*active_weapon_id*/, *instance,
                     0, castTimeMs*0.001, channelTimeMs*0.001)
```

* The four packed u32s (`desc+0x154/+0x164/+0x118/+0x1c8`) are the candidates the per-weapon
  selector chooses between; their column names in `skills` are not resolved [I: weapon variants
  of `start_anim_id`]. `FUN_39898ef0` picks one from the unit's weapon categories
  (`unit+0xd88/+0xb48/+0xc00`, categories 0x15/0x16/0x17).
* `desc+0xf8` / `desc+0xf9` (has cast / has channel) decide which of the two times is passed;
  both zero means an instant cast (`FUN_39157680` then uses mode 0 and no cast blending).
* `desc+0x38` (`start_autoattack`) starts the auto-attack when the selector returned none —
  i.e. an auto-attack has no start animation, only its fire animation.

`FUN_39157680` plays the start animation as a **full-body** animation (list A, layer 0) with the
cast/channel times turned into an animation-speed factor; it does not touch the body weight
(`char->vt[0xb4]`) unless it has to stop a previous animation.

### 3.2 Fire (`FireSkill` -> two `PlayAnim` calls)

`FireSkill` first clears the cast pose and then plays the fire animation:

```
model->vt[0x120](&out, 46 /*DAT_39ac7688*/, 3 /*keep weapon*/, instance, 0, 0, 0)   // clear cast pose
if (fireAnimId /*param_12*/ != none)
    model->vt[0x120](&out, fireAnimId, desc+0x10, *instance, 0, 0, 0)
```

`fireAnimId` comes from the `SCSkillFired` payload (`packets.md` §2), not from the desc: the
server tells the client which fire animation to use, which is how `fire_anim_id` /
`dual_wield_fire_anim_id` / `twohand_fire_anim_id` / the instrument variants are chosen by the
server's weapon state. The client then computes the **effective fire delay** for the timeline:
`FUN_39356440(unit, fireAnimId)` when `desc+0x1d1` (`use_weapon_cooldown_time`) is set, else
`round(FUN_39354670(unit, fireAnimId) * 1000.0)` ms (combat map §3.3 step 6). 46 is the "idle/restore"
animation id: it is also what `EndSkill` and the channel-stop path play.

### 3.3 End / stop / cast-delay

| event | call | effect on the animation |
|---|---|---|
| `SCSkillEnded` -> `EndSkill` `0x39397ff0` | `vt[0x120](&out, 46, 3, target, 0,0,0)` | back to idle, current weapon kept |
| `SCCastingStopped` -> `FUN_39396000` | `FUN_39395b50` -> controller stop (combat map §3.4) | stops the cast pose and the target cast bar |
| channel stop `FUN_3938ac10` | `vt[0x120](&out, 46, 3, none, 0,0,0)` | clears `+0x3acc/+0x3ad0` and returns to idle |
| `SCCastingDelayed` -> `FUN_39395760` | `model->vt[0x170](speed)` | `char->vt[0xb4](speed)` (E2 `SetAnimationSpeed`, `char+0x278`), log `"Casting anim speed %f -> %f"`; `speed = oldTotal*oldSpeed/newTotal` |

The cast bar itself is a UI query (`FUN_394bcf60`, combat map §3.2); no engine animation state is
involved.

### 3.4 Upper body vs full body, additive, weights

The 1.2 client has **three distinct ways** an animation reaches the body, in order of precedence:

1. **Named overlay** (`FUN_391572f0`): `ISkeletonAnim::StartAnimation(name, layer 0, …)` plus a
   record in list A. Used for jump/emotes/`anim_actions`/`match_animation`-style whole-body
   actions. No blending against the current pose — start replaces.
2. **Normal scheduler entry** (`FUN_39157680`): an entry in list A (blocking, `entry+0x20 != 0`)
   or list B (non-blocking). The body weight/speed is driven by `char->vt[0xb4]`:
   * cast speed override (`FUN_39395760`),
   * entry speed ratio (`entry+0x2c / entry+0x38`) when a blocking entry is applied,
   * 1.0 when the entry group is cleared.
   Layer 12 (`anim->vt[0x78](0xc, weight, dt, 0)`) is ramped every frame to 1.0 (or left at
   -1.0 = "no change") — this is the **combat/action layer**; the engine layer numbers are E2's
   (`CSkeletonAnim+0xcc`, stride 0x30). `0xc` = 12 of 16.
3. **Additive** (`FUN_39155de0`): `StartAnimation("additive_all_re_combat_miss", …, layer 3,
   flags 0x80, duration 0.1 s)`. No record in any list and no body weight change. This is the
   miss/whiff reaction only.

Graph inputs are used for everything that is *not* a one-shot: `"Action"` / `"Stance"` /
`"Signal"` / `"idle"` / `"none"` / `"jumpMP01/02"` / `"Vehicle"` / `"VehicleSeat"`
(`FUN_39153b60`, `FUN_39153ff0`, `FUN_39154090`, `FUN_391554c0`). Locomotion keeps running
underneath: the client never gates movement on a playing skill animation (movement is a separate
controller — [`port-movement/MAP.md`](../port-movement/MAP.md) §1), so a moving caster visibly
slides; the original only stops the *cast* on the server's `SCCastingStopped`.

### 3.5 The anim-speed inputs (numbers)

| value | meaning |
|---|---|
| `DAT_3999cf90 = 0.01` | percent -> factor |
| `DAT_3999cf30 = 0.001` | ms -> s |
| `_DAT_3999d1b0 = 1000.0` | s -> ms for the fire delay |
| `DAT_399a232c = 0.05 s` | hit-reaction timing margin (§5.3) |
| `_DAT_3999dd34 = 0.2 s` | graph-input "already running" threshold |
| `DAT_3999cc1c = -1.0` | "leave the layer weight unchanged" |
| `_DAT_3999c488 = 1.0` | restore weight / unit scale |
| `DAT_399b027c = 181.0 deg` | front/back attack-anim switch |
| `_DAT_399a44f4 = 57.29578` | rad -> deg |
| `DAT_39ac7424 = 10000` | in-combat timeout (ms) |

---

## 4. Weapon attachment and swap during combat

### 4.1 State

| field | meaning |
|---|---|
| `unit+0x3ba8` | active weapon slot (1 mainhand, 2 offhand, 3 "current", 4 twohand; `DAT_3a1699f8` = none) |
| `unit+0x1000` / `+0x1180` / `+0x1240` | item ids of the mainhand / offhand / twohand items |
| `unit+0x3bb0` | weapon-visual state block (`+0x07/+0x08` flags used by `FUN_39360580`) |
| `unit+0x3c30` | "weapon visual changed" flag byte |
| `unit+0x5fc0` | combat timestamp cleared when the weapon changes |

### 4.2 The chain

```
SCActiveWeaponChanged  handler 0x391d4fd0  "OnActiveWeaponChanged"
└─ FUN_39360920(unit, slot)                 122 B   SetActiveWeapon
   ├─ slot 3 -> current (+0x3ba8); 1/2/4 -> item id from +0x1000/+0x1180/+0x1240
   ├─ if unchanged -> return 0
   ├─ emit "sheathe" only when slot 0: unit+0x5fc0 = 0
   └─ FUN_39360780(unit)                   396 B   UpdateUnitWeaponVisuals
      ├─ model->vt[0x1ec]()   "has a weapon visual"
      ├─ FUN_3935a690(unit+0x4b8, activeSlot)        weapon-class string ("fist"/"onehand"/…)
      ├─ model->vt[0x1e4]() -> holdable desc
      ├─ FUN_3935a780(unit+0x4b8, holdable, unit+0x3bb0, flag)   6 visual slots
      ├─ model->vt[0x204]() -> unit state; flag = (state+0x1c1 == 4, i.e. gliding)
      └─ FUN_39360580(visuals, holdable, activeSlot, flag, &unit+0x3bb0)
           for i in 0..5: visuals[i]->vt[8]() / vt[0]((&DAT_399d0380)[slot*6+i])
           -> FUN_39357580/FUN_39357600  (draw/sheathe pose choice)
           -> FUN_3935ea10(holdable)     (rebind the held mesh)
```

`FUN_393609a0` (**ChangeUnitWeaponVisual**, 516 B) is the actual mesh swap. It is also used for
remote units and for the combat animation's weapon state:

```
model->vt[0x1e8]()->vt[0x20]()->vt[0x24](itemType)    // attachment/visual set for the slot
FUN_39354840(attachment)                             // detach/clear
path = itemDesc+0x44                                 // item_assets.path
if (itemType == 0x10 && desc+0x04) path = "%s_mirror.%s" (offhand mirror, allow_mirror)
mesh = FUN_3935e610(attachment, path, 0, 1)          // load the .cgf/.chr/.cdf/.cga
attachment->vt[0x64](1); attachment->vt[0x94](mesh); attachment->vt[0x48](1)
```

`FUN_39363840` (**ChangeEntityWeapon**, 566 B) does the same for remote units from the
`HoldableDesc` / `ItemWeaponDesc` / `ItemDesc` rows; `FUN_39363a80` (452 B) is the armour
equivalent. The per-item data is `item_assets` (`path`, `allow_mirror`, `attachment_offset_*`,
`default_anim`, `more_asset_id`) and `holdables` (`pose_id`, `sheathe_priority`, `slot_type_id`,
`anim_l1..r3` + ratios, `speed`).

### 4.3 Swap driven by the animation

`FUN_39156ac0` line `if (entry+0x1c && entry+0x04 != 3) FUN_39360920(unit, entry+0x04)` means the
**animation entry's `active_weapon_id` switches the weapon when the animation actually starts**,
not when the request arrives. A skill that needs a bow (or a two-hander) therefore swaps the
weapon exactly at the first frame of the cast, and a `3` entry keeps whatever is equipped. This is
the link the combat code alone does not show: the "weapon changes during the cast animation" feel
comes from the scheduler, and it is one frame late relative to `StartSkill`.

The swap itself is a **mesh swap on a prerigged attachment**, not a re-rig: the character keeps
its animation and the attachment's mesh/material is replaced (`FUN_3935e610` ->
`attachment->vt[0x94]`). E2 §7.4 maps the CryAnimation attachment object; the attach point and
the bone are engine-side. There is **no sheathing animation separate from the anim system**: the
draw/sheathe *pose* is chosen by `FUN_39357580/FUN_39357600` from the 6-entry matrix
`DAT_399d0380`, and the actual draw animation is the normal `PlayAnim` path (`anim_actions`,
`attach_anims.anim_action_id`).

---

## 5. Hit reactions, knockback and knockdown

### 5.1 The decision (already in the combat map; the engine handover only)

`FUN_39398c80` (damage, 1,614 B) and `FUN_393992d0` (heal, 753 B) call
`FUN_393917f0(hitType, unit, &animId, &kind)` — the reaction table (combat map `structs.md` §9) —
and then choose between two engine calls:

| branch | condition (`FUN_39391950`) | engine call |
|---|---|---|
| **victim** | `hitType == 0` or `float(desc+0x188) <= 0.0` | `model->vt[0x144](unit, targetId, animId, kind, 0)` -> `FUN_39157130` (timed reaction) |
| **attacker / big hit** | `hitType != 0` and `float(desc+0x188) > 0.0` | floating text `FUN_39398880` + `model->vt[0x120](&out, animId, kind, unit, 0, 0, 0)` |

Buff damage (source kind 2) and doodad damage (kind 4) always use the `vt[0x120]` form with
`kind` as the reaction kind; heals use `FUN_39392480` (heal text/anim, the same `+0x120`).
The **screen shake** for a big hit on the local player is in `FUN_39398c80` itself:
UI event `0x72` + `gEnv[0xf]->vt[0x6c]()` (can-shake) / `vt[0x68]()` (shake) — see §7.1.

### 5.2 Reaction ids and kinds

From the table in `port-combat/structs.md` §9, with the ids resolved in this pass:

| hit types | anim id | kind | meaning |
|---|---|---|---|
| 1, 9, 0xD | 17 / 14 (crit) | 3 | generic/crit |
| 3, 0xB, 0xF | 15 | 3 | flinch A |
| 4, 0xA, 0xE | 255 / 256 | 3 | **miss/whiff -> additive `additive_all_re_combat_miss`** (§2.6) |
| 5, 0x10 | 13 | 3 | flinch B |
| 6, 0x11 | 12 | 1 | small reaction |
| 7, 0x14 | 11 / 10 | 1 | small reaction (weapon variant) |
| other | 0 | 3 | none |
| fallback crit | 17 / 14 | 3 | |
| fallback normal | 387 / 386 | 3 | |

`kind` is passed through as the animation's `active_weapon_id` (3 = keep). The ids 10..17 are the
short "flinch" family, 255/256 the additive miss, 386/387 the long fallback.

### 5.3 Victim timing: `FUN_39157130` (model slot `+0x144`), 436 B

This is what makes a hit feel synchronous with the weapon hit:

```
find entry in list A (controller+0x24) with entry+0x20 /*cast key*/ == targetId
if found:
    entry+0x10 = now; entry+0x14 = animId; entry+0x18 = targetId
    weaponFire = FUN_39354670(unit, entry+0x14)          // weapon fire time for this anim (s)
    descTiming = entry+0x1c ? *(float*)(entry+0x1c+0x30) : 0   // hit-anim timing (s)
    if (unit attack-speed source && speed != 1) { descTiming /= speed; weaponFire /= speed; }
    delay = (descTiming - weaponFire) - 0.05             // DAT_399a232c = 0.05 s
    entry+0x30 = max(delay, 0);  entry+0x34 = 1          // armed
else:
    floating text + model->vt[0x120](&out, animId, kind, targetId, 0, 0, 0)   // fallback
```

The descriptor's hit-timing field (`node+0x40`, descriptor `+0x30`) is stored from
`damage_effects.hit_anim_timing_id` (the `damage_effects` SELECT has that column). The result is
that a reaction whose animation "contact" happens later than the weapon's fire time is delayed by
the difference (minus 50 ms), so the victim's flinch starts when the weapon would have landed,
not when the packet arrived.

### 5.4 Knockback / stun / knockdown

These are **buff states**, not separate reaction animations (combat map §6.3). The engine links:

* `SCBuffCreated` -> `FUN_3938a3c0` (943 B) appends the buff row; the row's `anim_*` fields and
  the `knock_down` / `stun` / `root` / `framehold` flags come from `buffs`.
* `X2::GameClient::ClientBuffMan::Update` (`FUN_393888b0`, called per unit per frame) drives the
  buff animations through `FUN_39388210(unit, actorEvent, animId)`: it picks an anim id
  (`buffDesc+0x1b0` is one of them), optionally turns the unit to the attacker, then calls
  `model->vt[0x120](&out, animId, eventId, DAT_39ac7658, 0,0,0)`. `eventId` is `0x10d` or `0x114`
  (mounted/pose actor events, §8).
* `FUN_3938ac10` (channel stop) and `FUN_3938ade0` restore animation **46** with kind 3 and clear
  the unit's cast/channel keys; this is the "stand back up / stop the loop" path after a
  stun/knockdown buff expires.
* `buffs.idle_anim` is the idle override; `buffs.ag_stance` and `buffs.anim_action_id` feed the
  graph/stance path (§8).
* `actor_models.ragdoll`, `use_ragdoll`, `use_ragdoll_hit`, `use_ragdoll_knock_down` are physics
  flags (CryPhysics, already ported). The client requests a ragdoll only through the physics
  parameters; no extra animation code is involved.

`knockback` (the positional push) is server/physics driven — the client only shows the buff
animation and the movement gate (`FUN_39426d10`, combat map §6.3). No client-side knockback
impulse was found in the animation path.

---

## 6. Fire-time events and projectiles

### 6.1 The animation-event pump (`FUN_39155660`, 686 B)

Every scheduler entry carries the CAF event list resolved in §2.3. Each frame `FUN_39156ac0`
calls `FUN_39155660(entry, now, apply)` (and `FUN_39155930(entry, now, apply)`) for the current
entry. The event record is 0x34 bytes:

| off | meaning |
|---|---|
| `+0x00` | next link |
| `+0x08` | normalised event time (0..1) |
| `+0x10` | event **type** (runtime-assigned ids, compared with `DAT_39af4b7c` / `DAT_39af4b78`) |
| `+0x1c` | event **name** string ("hit", …) |
| `+0x20` | event **data** string |
| `+0x24..0x2c` | vec3 offset (degrees -> radians with `DAT_3999cf98 = 0.0174533`) |
| `+0x30` | "once" byte |

Firing rule (with `clip = entry+0x2c`, `start = entry+0x28`, `now` in seconds):

```
eventTime = start + event[8]*clip + desc?            (cast-compressed form when the entry is a
                                                      cast: entry+0x38/entry+0x3c over clip)
if (now >= eventTime) {
    if (event.type == typeA) FUN_39395950(instanceId, event.name, event.data, vec3)
    else if (event.type == typeB) FUN_3938e3e0(...)   // sound / secondary effect
    if (event.name != "hit") FUN_3938e3e0(...)        // non-hit effects
}
```

`FUN_39395950` (95 B) is the **skill-event trigger**: it resolves the skill instance, then
`FUN_393ed520(instance+0x168, eventType, vec3, …)` spawns the visual effect at the unit/bone, and
for effect type 3 also calls `FUN_393eeed0` + `FUN_393eefa0` — the **projectile spawn**. This is
the exact moment the arrow/fireball leaves the hand: the CAF event `combat_sync` (or a `"hit"`
event) of the fire animation, not a timer.

`FUN_39155930` is the end-of-anim variant: when the entry time reaches `entry+0x30` it emits the
`"hit"` floating combat text (default radius `π/2` when the event has no offset), and if the skill
instance has `+0x300` set it plays the follow-up animation through `model vt[0x120]`.

### 6.2 Projectile manager

| function | address | size | role |
|---|---|---|---|
| `X2::GameClient::ProjectileManager` ctor | `0x393eeb10` | 95 | profiler `"ProjectileManager"`, creates the map |
| map find | `0x393eeb70` | — | `std::map` lookup by projectile id |
| insert | `0x393eec30` | 587 | map insert (id -> projectile) |
| `ProjectileManager::Update` | `0x393f0a50` | 516 | per frame: drop dead (state 2), update each live projectile through its vtable `+0xc`; expire after **10.0 s** (`DAT_3999cce8`) via `FUN_393ef0a0` |
| ballistic update | `0x393ee930` | 466 | move toward the target bone, build the velocity, spawn/update the visual: `FUN_393ede00(entity, "Ballistic Projectile", pos, vel, 0x307, 0x8f, 0, 0, 0)`; on arrival set `+0x2fc` and `FUN_393ee890(4, …)` (impact) |
| impact / destroy | `0x393ee890` | 62 | `FUN_393ed520` effect + remove |
| spawn / find helpers | `0x393eeed0` / `0x393eefa0` / `0x393f0a00` | 587 / 432 / 343 | map lookup + spawn/impact |
| `LoadProjectileDescs` | `0x3979dc80` | — | `projectiles` (`dest_bone_id`, `fx_group_id`, `ignore_z_rotation`, `is_permanent`, `proj_physic_id`, `src_bone_id`) |

The projectile itself is an entity with a physics mode (`proj_physic_id`) and an fx group; the
client does not simulate the damage (server-side). Timing: spawn at the animation event (§6.1),
then a straight/ballistic interpolation to the **destination bone** (or the stored position) at
the projectile's speed; `ignore_z_rotation` and the bone ids come from `projectiles`.

### 6.3 `effect_delay`, `use_anim_time`, `match_animation`

* `skills.effect_delay` / `effect_repeat_tick` / `effect_repeat_count` are passed to the skill
  controller (`FUN_393ed520` family) and repeat the *visual* at fixed ticks; they do not move the
  fire event.
* `skills.use_anim_time` makes the client use the animation length instead of `casting_time` for
  the timeline; the scheduler already computed `entry+0x2c` from the clip, so this only affects
  the UI cast bar and the server-facing timing.
* `skills.match_animation` / `match_animation_count` choose an animation variant to match the
  current weapon/locomotion; they go through the normal selector (`FUN_39898ef0`) before
  `vt[0x120]`.

---

## 7. Camera shake and the combat camera

### 7.1 Big-hit shake (damage path)

In `FUN_39398c80`, after the damage is applied and when the victim is the local player with the
"big hit" flag (`record+0x14`):

```
if (FUN_3921ad60()) FUN_39424220(0x72);                 // UI event "big hit"
if (gEnv[0xf] && gEnv[0xf]->vt[0x6c]()) gEnv[0xf]->vt[0x68]();   // shake trigger
```

`gEnv[0xf]` is the x2game environment slot at `DAT_3a5bb1d8+0x3c`; the same slot exposes a
time-of-day-like interface elsewhere (`FUN_390197d0` reads `vt[0x2c]()->vt[0x14]()` as an hour
value), so the interface is engine-side and its exact class is **U4**. `vt[0x6c]`/`vt[0x68]` are
the can-shake query and the shake trigger; `gEnv[0]`'s `vt[0x3c]` is the "is client" style test
used in the same path.

### 7.2 The shake/impulse block in the camera manager

`FUN_3915afb0` (137 B) writes a 12-dword block into the live camera manager:

```
DAT_39afe2b8 = X2::GameClient::CameraManager (port-movement §4)
FUN_3915ac50(camera, &block):
    backup[0..5] = current[0..5]          // camera+0xcc..+0xfb = old camera+0x9c..+0xcb
    current[0..5] = block                 // camera+0x9c..+0xcb
    if (current+0xa0 <= old+0xa0 && current+0xa0 != old+0xa0 && current+0xb0 > 0)
        current+0xbc |= 1                 // "this is a new impulse" flag
```

The block (`FUN_3915afb0` args -> offsets): `+0x00 arg1, +0x04 0, +0x08 arg2, +0x0c arg3,
+0x10 arg4, +0x14 arg5, +0x18 arg6, +0x1c arg7, +0x20 (mode 4 or 0), +0x24 0, +0x28 1,
+0x2c 0`. `mode = 4` when the last byte is set (a stronger variant). `FUN_3915b040` clears the
block with mode 8.

Callers, both gated on `desc+0x1d2 != 0` (the skill/buff "camera" flag):

* **skill**: `FUN_3939a440` (instance creation) with
  `(desc+0x20, desc+0x24, desc+0x2c, desc+0x34, desc+0x7c, desc+0x80*0.001, desc+0x30, desc+0x28)`.
* **buff**: `FUN_3938f100` (buff apply) with the same offsets on the buff desc.

`desc+0x80 * 0.001` is the duration in seconds (a ms column). The other seven are the
`camera_acceleration / camera_duration / camera_hold_z / camera_max_distance /
camera_slow_down_distance / camera_speed` group [I] — the field-to-column mapping still needs the
`skills`/`buffs` loader (**U5**).

### 7.3 Skill camera override (`FUN_3915af30`, 119 B)

A second camera block for the cinematic skill camera (the `use_skill_camera` /
`controller_camera` / `controller_camera_speed` columns):

```
if (cameraManager && gEnvCamera->vt[8]()) {
    camera+0x224 = arg6
    camera+0x24c..0x250 (u64) = arg1
    camera+0x254 = arg2
    camera+0x258..0x25c (u64) = arg3
    camera+0x21c = arg5
    camera+0x260 = arg4
    camera+0x22c = arg7
}
```

Called from the skill-controller camera path (`0x3915...` caller at `0x391...` line 192317 — the
controller deserializer) and from the doodad/skill effect deserializers. Field meanings are not
recovered (**U6**); what matters for the port is that it is a **separate** block from the shake
impulse and that it is written only while a camera controller is active.

The per-frame camera itself is mode 1 (`FUN_39169860`) from `port-movement/MAP.md` §4; neither
block changes the camera *mode*, they only feed temporary offsets/blends. The reader of
`camera+0x9c..+0xcb` was **not located** in mode 1 (no reads of those offsets in `FUN_39169860`),
so the consumer is one of the other modes or a helper called with the manager as `this`
(**U6**).

---

## 8. Combat stance and idle selection

### 8.1 The in-combat byte (10 s)

`FUN_39355ee0` (262 B) runs per unit per frame from `ClientUnit::Update`:

```
now = timer->GetTime(0)                              // ms
if (unit+0x3bba /*fresh combat action*/) unit+0x5fc0 = now
if (unit+0x5fc5 && now - unit+0x5fc8 > 1000) unit+0x5fc5 = 0; else unit+0x5fc0 = now
if (unit+0x5fc6 && now - unit+0x5fcc > 1000) unit+0x5fc6 = 0; else unit+0x5fc0 = now
inCombat = (now - unit+0x5fc0) <= 10000              // DAT_39ac7424
if (inCombat != unit+0x5fc4) {
    *(bool*)(*(model+0x14) + 0x428) = inCombat        // animation-graph "in combat" input
    unit+0x5fc4 = inCombat
    if (!inCombat) unit+0x5fc0 = 0
}
```

`unit+0x3bba` is set by the combat state (`SCCombatEngaged`, first hit, force attack — combat map
§5.3); `+0x5fc5/+0x5fc6` are the 1 s grace latches. The destination is
`*(model+0x14)+0x428` — a bool in the model's animation-graph holder, i.e. the same object that
`ActorUnitModel::Init` gets at `model+0x14`. The **combat stance/idle** therefore flips one graph
input 10 s after the last combat action, and the graph (E2/F3) selects the stance/idle clip.

### 8.2 Graph input API (model slots `+0x130/+0x138/+0x140`)

| slot | thunk | target | role |
|---|---|---|---|
| `+0x130` | `0x390bb480` | `FUN_39155ed0(animId, value, force)` | remove an animation by type/key (buffs, vehicles, mounts) |
| `+0x138` | `0x390bb4a0` | `FUN_39155f70(type)` | restore the per-type animation (types 5..8) |
| `+0x140` | `0x390bb4d0` | `FUN_39153b60(key, value)` | **SetGraphInputs**: stance/action key pair |

`FUN_39153b60(controller, key, value)`:

```
if (key < 0xb) graph->vt[4](controller+0x1a, intern_string(key))   // string-valued input
if (value < 1 && value < 5) return
if (value >= 5) { FUN_39155ed0(259 /*DAT_39ac3f28*/, 0, 1); graph->vt[4](controller+0x1a, "?") ; value = 0 }
graph->vt[8](controller+0x1c, value, 0)                            // int-valued input
```

`259` is the "reset the type slot" id. The two model keys at `+0x1a/+0x1c` are the primary
(stance) and secondary (value) graph inputs.

`FUN_39153ff0` sets the jump input: `graph->vt[4](key, "jumpMP01", 0)` or `"jumpMP02"` when
`unit+0x40 & 0x1000` (sprint), and `"em_jump_in_place"` when the value is 0.

### 8.3 Idle and the "Action" value

`FUN_39154090` (477 B) builds the `"Action"` value:

* `"dance"` handling: if the current graph value is `"dance"` and the unit is not
  `unit[0x21d] > 0` / not in a special state, it uses `"%s%s"` with `"_ub"` (upper-body suffix)
  before falling back.
* `"Action"` handling: it compares the requested name and appends `"_ub"` via
  `FUN_39056380`/`DAT_399b02b8` when the upper-body variant is needed.
* `FUN_39154ad0` (504 B) builds the value for the *current entry* from `anim_actions`
  (`action_state_id`, `anim_name`, `mainhand_tool_id`, `offhand_tool_id`, `no_rotate`) and the
  `anims` `*_ub` flags; `FUN_391547b0(1.0)` restores the body weight.

Idle-specific state:

| function | address | role |
|---|---|---|
| `FUN_39154ea0` (688 B) | per-frame | if the graph value is `"fist_ba_relaxed_rand_idle"` -> `"idle"`; drives the relaxed idle |
| `FUN_39154060` (42 B) | helper | sets `+0xa0` (clear-graph) or `+0x68`, depending on list B and the current entry id `DAT_39ac3f24 = 2` |
| `FUN_391555d0` (129 B) | per frame | pushes the current action id into the `"Action"` input when it changed; `FUN_39155310(1)` re-applies |
| `X2::PrefabUnitModel::UpdateIdleAnim` | string `0x399a45c4` | idle rebuild for prefab (doodad/NPC) models |
| `FUN_3938ac10` | channel stop | restores animation 46 (idle) and clears cast/channel keys |

`anims.hang_ub`, `move_ub`, `relaxed_ub`, `ride_ub`, `swim_ub`, `swim_move_ub` choose which clips
are allowed as the **upper-body** layer in each movement state; `models.play_mount_animation` and
`models.mount_pose_id` do the same for mounted idles. `buffs.idle_anim` overrides the idle while a
buff is active; `buffs.ag_stance` feeds `FUN_39153b60`.

---

## 9. Where this sits in the frame (E1)

```
#1 startup message pump            <-- packet handlers run here: StartSkill / FireSkill / EndSkill /
                                       SCUnitDamaged call model vt[0x120]/[0x144] immediately
#2 GameClient::PreUpdate
#3 CCryAction::PreUpdate -> CSystem::Update
   #4 timer tick
   #5 CGame::PrePhysicsUpdate -> movement sampling
   #6 IInput::Update
   #7 entity PrePhysicsUpdate (event 0x1c -> CAnimatedCharacter::PrePhysicsUpdate)
   #8 ONE physics TimeStep
   #9 post-physics: movie, entity update, terrain, sound, network
#10 Client Game Update (x2game 0x391739e0)
   └─ UpdateUnits (0x3936a750) -> per unit ClientUnit::Update (0x3936a0b0)
        model vt[0x260] (ActorUnitModel::Update: decals/shadows/foot IK/rider feed)
        cooldown tick 0x39899e50(ms)
        ClientBuffMan::Update (0x393888b0)       <-- buff actor events -> model vt[0x120]
        FUN_39355ae0 -> char anim speed (graph vt[0x2c])
        FUN_39355ee0 -> in-combat byte -> graph input (m.10 s)
        [anim controller tick FUN_39156ac0(now), exact slot U1]
        [engine animation update (ProcessAnimations / pose) -- E2/E1, caller virtual]
#11 CCryAction::PostUpdate -> CSystem::Render
#12 Client Game PostUpdate (movement send pump)
```

Consequences for the port:

* A skill/cast animation requested by a packet is queued **before** the unit update of the same
  frame, so the scheduler applies it (graph inputs, weapon swap, weight) in that frame and the
  first visible pose is the first rendered frame after it. There is no extra latency.
* The weapon swap is one scheduler tick after `StartSkill`, i.e. at the first frame in which the
  entry is current — visually the same frame the animation starts.
* The victim reaction's delay (§5.3) is in *animation seconds*, added to the scheduler time, so it
  is independent of the frame rate.
* The per-frame animation speed (`FUN_39355ae0`) and the in-combat byte are written in
  `ClientUnit::Update`; whether the same frame's pose uses them depends on the virtual engine
  animation update slot (E1 U4). For the port: run both immediately before the animation pose
  update.
* The combat camera/shake blocks are written on the packet/threat path and consumed by
  `CameraManager::Update` (E1 step #10, before render) with `dt = min(frameTime, 0.033)`
  (port-movement §4.1) — not with the animation dt.

---

## 10. Function inventory

Sizes from `funcs.csv`; module = x2game unless noted.

### 10.1 Phase-2 critical (casting, hit, weapon, camera feel)

| address | size | function | callers / callees | state |
|---|---|---|---|---|
| `3939a810` | 2036 | `StartSkill` (combat map) | `SCSkillStarted`; -> `vt[0x120]` | stateful |
| `3939b010` | 2218 | `FireSkill` (combat map) | `SCSkillFired`; -> 2x `vt[0x120]` | stateful |
| `39397ff0` | 1050 | `EndSkill` (combat map) | `SCSkillEnded`; -> `vt[0x120]` 46/3 | stateful |
| `390b3750` | 7 | `Unit::GetModel` | every combat anim call | pure read |
| `390bb430` | 66 | model `vt[0x120]` thunk | StartSkill/Fire/End/damage | pure |
| `39157680` | 2442 | **PlayAnim** | thunk; -> descriptor map, lists, `FUN_39156e50` | stateful |
| `39156310` | 98 | descriptor lookup by id | PlayAnim, overlay, hit timing | pure read |
| `39155d50` | 135 | descriptor lookup by name | overlay path | read |
| `391564e0` | 49 | overlay descriptor lookup | PlayAnim | read |
| `391572f0` | 449 | named overlay start (`ISkeletonAnim::StartAnimation`) | PlayAnim | stateful |
| `39156ac0` | 612 | **controller tick** | (virtual) | stateful |
| `39155660` | 686 | per-entry event pump | tick | stateful |
| `39155930` | 473 | end-of-anim events + next anim | tick | stateful |
| `39156e50` | 593 | CAF event -> entry times (`combat_sync`) | PlayAnim | read + state |
| `39156e00` | 69 | append entry | PlayAnim, overlay | stateful |
| `39155c40` | 99 | alloc list node (0x4c) | append | stateful |
| `39155bb0` | 142 | alloc event record (0x34) | `FUN_39156e50` | stateful |
| `39154a50` | 103 | init entry record | PlayAnim, overlay, tick | pure |
| `39153ef0` | 129 | transition rule lookup | PlayAnim, tick | read |
| `39153f80` | 106 | transition speed multiplier | PlayAnim | read |
| `39156760` | 394 | stop animation by id | PlayAnim, tick, buffs | stateful |
| `39156900` | 410 | remove animation group | model buff path | stateful |
| `39155de0` | 236 | additive `additive_all_re_combat_miss` | stop ids | stateful |
| `39155ed0` | 160 | remove-by-type (model `vt[0x130]`) | buffs/vehicles | stateful |
| `39155f70` | 162 | restore-by-type (`vt[0x138]`) | buffs | stateful |
| `391547b0` | 72 | `char->vt[0xb4](scale)` (E2 `SetAnimationSpeed`) | many | stateful |
| `39154550` | 143 | layer 0xc weight ramp | tick | stateful |
| `391545e0` | 413 | attack-speed graph keys | tick | stateful |
| `391555d0` | 129 | push action id to graph | tick | stateful |
| `39155560` | 108 | current entry picker | tick | read |
| `39156160` | 124 | force apply by input time | tick | read |
| `39154ea0` | 688 | idle fixup | tick | stateful |
| `39153b60` | 150 | **SetGraphInputs** (model `vt[0x140]`) | `FUN_39155ed0/5f70` | stateful |
| `39153ff0` | 99 | jump graph input | movement | stateful |
| `39154060` | 42 | idle/clear latch | tick | stateful |
| `39154090` | 477 | `"Action"` value builder | tick | read |
| `39154ad0` | 504 | action value from `anim_actions` | tick | read |
| `390bb4e0` | 34 | model `vt[0x144]` thunk | damage | pure |
| `39157130` | 436 | **hit reaction timing** | thunk | stateful |
| `39153af0` | 101 | front/back attack variant | PlayAnim | read |
| `39156d30` | 68 | animation entry id -> data | misc | read |
| `39156d80` | 128 | animation entry copy | misc | read |

### 10.2 Weapons and camera

| address | size | function | role |
|---|---|---|---|
| `39360920` | 122 | `SetActiveWeapon` | slot state -> `UpdateUnitWeaponVisuals` |
| `39360780` | 396 | `UpdateUnitWeaponVisuals` | 6 visual slots + holdable |
| `393609a0` | 516 | `ChangeUnitWeaponVisual` | attachment mesh swap |
| `39363840` | 566 | `ChangeEntityWeapon` | remote unit weapon |
| `39363a80` | 452 | `ChangeEntityArmor` | remote unit armour |
| `3935a690` | 232 | weapon-class string | "fist"/"onehand"/… |
| `3935a780` | 113 | build 6 visual slots | from holdable desc |
| `3935a800` | — | build visual state | — |
| `39360580` | 134 | apply slot visibility matrix | `DAT_399d0380` |
| `3935e610` | 1015 | resolve/load attachment mesh | `.cgf/.chr/.cdf/.cga` |
| `39354840` | 95 | detach/reset attachment | — |
| `3915afb0` | 137 | **write camera impulse block** | `desc+0x1d2` |
| `3915ac50` | 224 | install impulse block (double buffer) | camera manager |
| `3915b040` | 88 | clear impulse block (mode 8) | camera manager |
| `3915af30` | 119 | **skill camera override** | camera manager +0x21c..+0x260 |
| `3915b160` | 23 | set camera field +0x1b0 | — |
| `3915b1b0` | 12 | read camera +0x84 (yaw) | — |
| `39398c80` | 1614 | damage dispatch (combat map §6) | big-hit shake |
| `393992d0` | 753 | heal dispatch | heal anim/text |

### 10.3 Projectiles and buff animations

| address | size | function | role |
|---|---|---|---|
| `393f0a50` | 516 | `ProjectileManager::Update` | live projectiles, 10 s |
| `393ee930` | 466 | ballistic projectile update/spawn | `FUN_393ede00` |
| `393eeb10` | 95 | `ProjectileManager` ctor | — |
| `393eeb70` / `393eec30` | — / 587 | map find / insert | — |
| `393ede00` | 668 | spawn projectile entity + fx | engine calls |
| `393ee890` | 62 | impact/destroy | — |
| `393eeed0` / `393eefa0` / `393f0a00` | 587 / 432 / 343 | spawn/impact/expire helpers | — |
| `3979dc80` | — | `LoadProjectileDescs` | `projectiles` |
| `393888b0` | — | `ClientBuffMan::Update` | -> `FUN_39388210` |
| `39388210` | — | actor event -> anim (`0x10d/0x114`) | -> `vt[0x120]` |
| `3938ac10` | — | channel stop -> anim 46/3 | — |
| `3938ade0` | 609 body | buff end -> anim 46/3 | — |
| `3938a3c0` | 943 | apply buff row | no anim call |
| `3938f100` | — | buff/skill effect apply | camera shake call |
| `393917f0` | 295 | reaction table | pure |
| `39391950` | 147 | big-hit/attacker predicate | pure |
| `393985f0` | 643 | hit effect event | — |
| `39398880` | 337 | floating combat text | — |
| `39392480` | 314 | heal presentation | — |
| `3940a3e0` | 2008 | **floating combat message UI** (not animation) | — |
| `3940a0f0` | 108 | caster/weapon -> effect pair | pure |
| `394e2650` | 236 | stop-casting VFX | — |
| `3939a440` | — | create skill instance; camera shake | stateful |
| `39355ae0` | 120 | per-frame `char->vt[0xb4]` speed | — |
| `39355ee0` | 262 | in-combat byte -> graph input | — |
| `39355a80` | 96 | attack-speed getter | — |
| `39355c00` | 115 | cast-speed helper | — |

---
## 11. Port order (testable leaves first)

**Phase 0 — data leaves (pure, synthetic goldens)**

1. `anims` / `anim_rules` / `anim_actions` loaders (`FUN_397287e0`, `LoadAnimRuleDescs`,
   `LoadAnimActionDescs`): id -> {category, 6 upper-body flags, loop}; rule table.
2. `FUN_39156310` / `FUN_39155d50`/`FUN_391564e0` descriptor lookups (feed a synthetic map).
3. `FUN_39153ef0` / `FUN_39153f80` transition lookup (synthetic rules; boundary times).
4. `FUN_39153af0` front/back variant (angle 0/180/181/360).
5. `FUN_39154a50` entry ctor, `FUN_39158090` record copy.
6. Reaction table `FUN_393917f0` + `FUN_39391950` (already combat phase A; re-use the goldens).

**Phase 1 — scheduler core (stateful, diffable field-by-field)**

7. `FUN_39157680`: feed a fake controller (map, two lists, graph stub) and compare
   `out`, list contents, `+0x6c`, `+0x68`, `+0x78` for: no anim, overlay anim, blocking skill,
   non-blocking skill, weapon 3, stop ids 255/256, names `all_co_sk_stop`/`all_ac_stop`/
   `all_re_stop`, front/back variant, missing desc.
8. `FUN_39156e00` + `FUN_39155c40` + `FUN_39155bb0` (entry/list allocation and append).
9. `FUN_39156ac0` tick + `FUN_39155560` picker + `FUN_39156760` stop (fake time steps).
10. `FUN_39156e50` event-time resolution: fake animation interface returning a known clip length
    and a `combat_sync` at 0, 0.5, 1.0; compare `entry+0x2c/0x30/0x34`.
11. `FUN_39155660` / `FUN_39155930` event pump: event table with `hit` and other names, once flag,
    fire times; compare the emitted event stream.
12. `FUN_391572f0` overlay path (fake `ISkeletonAnim` recording `vt[0x34]` args).
13. `FUN_39157130` hit timing: weapon fire time / descriptor hit timing (+0x30) / speed 1.0/1.4 / margin 0.05.

**Phase 2 — integration (packet -> engine call, live oracle)**

14. `StartSkill`/`FireSkill`/`EndSkill` -> `vt[0x120]` argument capture (combat map harness).
15. `ClientBuffMan::Update` -> `FUN_39388210` argument capture (buff anims).
16. `FUN_39360920`/`FUN_39360780`/`FUN_393609a0` with a live unit: weapon slot, visual slots,
    attachment mesh name.
17. Camera blocks: capture `camera+0x9c..+0xcb` and `+0x21c..+0x260` after a skill with
    `desc+0x1d2` and after a big hit (needs the shake trigger, U4).
18. Projectiles: spawn time relative to the animation event, pos/vel, impact after 10 s.

**Phase 3 — feel**

19. Side-by-side cast/hit at 60 fps: flinch start vs weapon contact, cast speed change on
    `SCCastingDelayed`, weapon swap on first cast frame, big-hit shake, projectile release.

---
## 12. Numeric parameters

| value | number | source |
|---|---|---|
| animation entry size | 0x4c B | `FUN_39155c40` |
| animation event record size | 0x34 B | `FUN_39155bb0` |
| controller lists | 2 (+`+0x24`, `+0x30`) + 1 extra (`+0x3c`) | `FUN_39157680`/`FUN_39156ac0` |
| combat/action engine layer | **12 (0xc)** | `FUN_39154550` (`anim->vt[0x78]`) |
| additive layer / flags | **3 / 0x80** | `FUN_39155de0` |
| additive miss clip/duration | `additive_all_re_combat_miss`, 0.1 s | `FUN_39155de0` |
| additive press time | 0.1 | `DAT_3999ccf0` |
| "already running" input time | 0.2 s | `_DAT_3999dd34` |
| hit-reaction margin | 0.05 s | `DAT_399a232c` |
| front/back attack switch | 181 deg | `_DAT_399b027c` |
| rad -> deg / deg -> rad | 57.29578 / 0.0174533 | `_DAT_399a44f4`, `DAT_3999cf98` |
| in-combat timeout | 10,000 ms | `DAT_39ac7424` |
| in-combat grace latches | 1,000 ms | `FUN_39355ee0` |
| projectile lifetime | 10.0 s | `DAT_3999cce8` |
| default hit-effect radius | pi/2 | `DAT_399a27c0` |
| ballistic spawn fx ids | 0x307 / 0x8f | `FUN_393ee930` |
| stop / reset / idle anim ids | 255, 256 / 259 / 46 | `.data`, §2 |
| reaction anim ids | 10..17, 386, 387 | combat `structs.md` §9 |
| kind / weapon ids | 1 mainhand, 2 offhand, 3 keep, 4 twohand | `FUN_39360920` |
| animation speed restore | 1.0 | `_DAT_3999c488` |
| layer weight "leave" | -1.0 | `DAT_3999cc1c` |
| percent -> factor / ms -> s | 0.01 / 0.001 | `DAT_3999cf90`, `DAT_3999cf30` |
| fire delay s -> ms | 1000.0 | `_DAT_3999d1b0` |

---
## 13. Unknowns

1. **U1 — controller tick caller.** `FUN_39156ac0(controller, seconds)` has no static caller and
   its vtable is not in the `.data` image. Candidate: the engine animation-graph listener, i.e. it
   runs inside the per-character animation update (E2's `ProcessAnimations` window). Next: hook
   `0x39156ac0` in the live client and capture the return address, or read controller `+0` at
   runtime.
2. **U2 — list A vs list B meaning.** A (`+0x24`) is consumed first and receives new skill and
   overlay entries; B (`+0x30`) is used for the transition source and prefers anim id `0xb`.
   Whether B is the "upper-body/action" set is [I]. Next: run a walking + attacking live pair and
   dump both lists.
3. **U3 — player attack-speed hook.** `char?->vt[0x18]()->vt[0x170](globals 0x39aefb98+0x2ec,
   +0x2f0)` for anim ids 14/17/386/387. The two globals need a live read.
4. **U4 — big-hit shake interface.** `gEnv[0xf]` (`DAT_3a5bb1d8+0x3c`) `vt[0x6c]`/`vt[0x68]`
   are called but not identified; the same slot is used as a time-of-day provider elsewhere.
5. **U5 — camera column mapping.** The `camera_*` columns -> `desc+0x20/+0x24/+0x2c/+0x30/+0x34/
   +0x7c/+0x80` mapping is [I]; the `skills`/`buffs` loaders must be read to name them.
6. **U6 — camera block readers.** `camera+0x9c..+0xcb` (impulse) and `+0x21c..+0x260` (skill
   camera) are written but no reader is located in mode 1 (`FUN_39169860`). Find the mode/helper
   that consumes them.
7. **U7 — animation event type ids.** `DAT_39af4b7c` / `DAT_39af4b78` are 0 in the image (runtime
   assigned). Next: read them after a live skill start.
8. **U8 — descriptor fields.** `desc+0x30` (blocking), `desc+0x34` (start offset in the event
   time formula) and the descriptor hit timing (+0x30) are engine-side; read them live for one
   CAF with a known `combat_sync`.
9. **U9 — buff anim field.** `buffDesc+0x1b0` is one anim id used by `ClientBuffMan::Update`;
   which of `anim_start_id` / `anim_end_id` / `anim_action_id` / `idle_anim` it is needs the
   buff loader.
10. **U10 — projectile state machine.** States (2 = dead), `proj+0xa9` (start time in
    `GetCurTime` units), the `vt[0xc]` update and `FUN_393ef0a0` (expire/impact) are not fully
    decoded; the speed/arc come from `projectiles.proj_physic_id` and the source/dest bones.
11. **U11 — attack-speed graph keys.** `FUN_391545e0` writes graph keys per weapon speed; the key
    ids and the formula are not decoded.
12. **U12 — `*_ub` semantics.** Whether the six `anims` upper-body flags select a layer or a clip
    variant (E2 §4.5 lists the layer mask bits) needs a live layer dump while moving + attacking.
13. **U13 — controller lifetime.** Who constructs/destroys the object at `model+0x164` and when it
    is registered with the graph (`FUN_39154090`-family), not mapped.
14. **U14 — frame slot of the controller tick** (see §9): if the tick runs inside the engine
    animation update, a `PlayAnim` issued from `Client Game Update` would only show next frame.
15. **U15 — `FUN_39153b60` key ids.** Keys 0..0xa and the two inputs at `controller+0x1a/+0x1c`
    are unnamed; the stance values are 0..4 (>=5 resets). Needs a live graph-input dump.

---
## 14. Size of the work

Counted from `funcs.csv` on the E6 scope (x2game only; E2's CryAnimation internals and the
already-ported combat functions are excluded).

**(a) What phase 2 (combat feel) needs first: 69 functions, 18,712 bytes (18.3 KB).**
The scheduler core (`FUN_39157680`, tick, event pump, lists, descriptor lookups, transition and
graph-input helpers), the hit-timing leaf (`FUN_39157130`), the weapon visual chain, the
projectile manager, the camera blocks and the two per-frame leaves (`FUN_39355ae0`,
`FUN_39355ee0`). Of these, 9 are tiny thunks (7-72 B) and 14 are < 150 B; the two big ones are
`FUN_39157680` (2,442 B) and `FUN_391554/39153` helpers.

**(b) The rest: ~454 functions, ~94 KB** in the same clusters as an upper bound —
anim-controller helpers and animation-event/clip plumbing (128 fn / 30.2 KB in
`0x39152xxx-0x3915a4ff`, of which the phase-1 set covers 69 / 18.3 KB), the full weapon visual
system (160 fn / 38.1 KB in `0x3935axxx-0x39363fff`), the projectile manager (71 fn / 15.0 KB in
`0x393edxxx-0x393f0fff`), the buff-animation side of `ClientBuffMan` (135 fn / 25.1 KB in
`0x39388000-0x3938e7ff`) and the remaining camera helpers (29 fn / 4.0 KB in
`0x3915a000-0x3915b1ff`). The exact remainder after the phase-1 set is ~454 functions / ~94 KB;
the count is an upper bound because those ranges also contain combat and UI functions that are not
animation-related.

The engine side is E2's: the commands these functions emit land in `ISkeletonAnim`
(`StartAnimation`, layer-0xc weight ramp, `char->vt[0xb4]` speed) and `IAnimationGraphState`
(`vt[4]/vt[8]/vt[24]/vt[5c]`), i.e. the ~1,197 functions / 342 KB E2 already sized for phase 1.
Nothing in this map requires new CryAnimation work beyond E2.

---
## 15. In-process oracle recipes (`aadriver`)

The hidden client exposes plain C functions; a driver wrapper can call them with the module base
applied. All pointers below are VAs in `x2game` (base `0x38FF0000`).

**Reaching the objects**

```
unit       = FUN_39360020(unitId)            // 0 if absent
model      = *(void**)(unit + 0x3be8)        // X2::ActorUnitModel
controller = *(void**)(model + 0x164)        // anim controller (the scheduler's this)
char       = (*(void*(**)(int))model)[0x1b8]? -> use the existing E2 helper:
             entity = *(void**)(model + 0x1f0); char = entity->vt[0x168](0)
```

**PlayAnim (the combat call)** — call the thunk so the controller indirection is exercised:

```
call 0x390bb430 with ECX=model and stack args
     (u32* out, int animId, int weaponId, int data, int flags, float castS, float chanS)
// cdecl wrapper in the driver: push args, set ECX, call, read *out
```

Expected: `*out` = the id played (0 if none); `controller+0x6c` = weapon id; list A/B contents
changed. Compare against the Rust port with a fake controller.

**Tick**

```
call 0x39156ac0 with ECX=controller and now = *(float*)(timer+0x38) accumulated
   (the engine pass uses seconds; `CTimer::GetCurTime(0)` = timer vtable+0x0c)
```

After the call read `controller+0x68/+0x70/+0x78` and the entry fields; set a breakpoint on
`0x39155660` to capture the event stream.

**Lookups** — `FUN_39156310(&node, &id)` with ECX=controller; `FUN_39155d50(name, &id)`.
Read `node+0x14` (name), `node+0x3c` (duration), `node+0x30` (blocking), `node+0x40` (hit
timing), `node+0x38` (event list).

**Weapon** — `FUN_39360920(unit, slot)`; read `unit+0x3ba8`, `unit+0x3bb0`, `unit+0x3c30`.
Attach/detach is visible on the character attachments (E2 §7.4) and in the `.cgf` path passed to
`attachment->vt[0x94]`.

**Hit reaction** — `FUN_39157130` via the model thunk `0x390bb4e0` with ECX=model and
`(animId, ..., targetId)`; read the list-A entry `+0x10/+0x14/+0x18/+0x30/+0x34`.

**Camera** — `camera = *(void**)0x39afe2b8`; read the impulse block `camera+0x9c..+0xcb` and the
skill-camera block `camera+0x21c..+0x260` before/after a skill with `desc+0x1d2` and after a big
hit. Trigger the shot with a real skill on a dummy; `DAT_3a5bb1d8+0x3c` (`gEnv[0xf]`) is a second
hook if the shake interface is needed.

**Projectiles** — `ProjectileManager::Update` runs from `ClientSkillActionManager::Update`
(`0x3939c4a0`); hook `0x393ee930` (ballistic update) and `0x393ede00` (spawn) to capture spawn
time, position, velocity and the fx ids. `FUN_39155660` gives the animation-event timestamp for
the same frame; compare the delta.

**Event capture** — hook `0x39155660` entry and `0x39395950`; log `(animId, event name, event
time, now)`; the first `hit`/`combat_sync` call must be at `entry+0x30` seconds after
`entry+0x28`, independent of frame rate.

---
