# Combat structs to define in Rust

Companion to [`MAP.md`](MAP.md). Offsets are byte offsets from the object pointer used by the
1.2 code (`x2game`, base `0x38FF0000`). Names in `CClass`/`field` form come from the binary's own
strings or from the matching 10.x class where one exists; unnamed fields are marked *(inferred)*.
All arrays are C arrays unless stated.

## 1. `SkillDesc` (DB `skills` row)

Loaded by `FUN_39731870` (`LoadSkillDescs`, SQL in `strings-xrefs.tsv:20645`); looked up by
`FUN_396f6390(skillId)`. The loader is 8 KB of column reads; the table below is only what the
combat code reads, with the call site that proves the width.

| off | type | call-site role | evidence |
|---|---|---|---|
| `0x00` | u32 | skill id (`FUN_3989a290` compares it to the skill-cooldown key) | **[R]** 3989a290 |
| `0x04` | u32 | cast/instance key (copied to instance `+0x04`, then `FUN_396f6390`) | **[R]** 3939b010 |
| `0x10` | u32 | animation/event id passed to `game_vtable+0x120` | **[R]** 3939a810, 3939b010 |
| `0x19` | u8 | "silent/refusal suppressed" (error builder returns early) | **[I]** 39390900 |
| `0x38` | u8 | `start_autoattack` — start auto-attack on cast | **[I]** 3939a810 |
| `0x6c` | i32 | channel/repeat duration (>0 = channel/ticks) | **[I]** 3939a810, 3939b010, 3939b8c0 |
| `0x94` | i32 | `custom_gcd` (ms; positive) | **[I]** 3939a810 |
| `0x9c` | u8 | "custom gcd valid" (`0` → use `+0x94`, else tag GCD) | **[I]** 3939a810 |
| `0xb0` | i32 | base duration copied into `ClientSkill+0x138` | **[R]** 3939b8c0 |
| `0xc0` | u8 | start-skill-controller flag | **[I]** 3939b010, 3939b8c0 |
| `0xc4` | i32 | cooldown-ish value copied into `ClientSkill+0x140` / passed to `FUN_39898ef0` | **[R]** 3939b8c0 |
| `0xdc` | u8 | `ignore_global_cooldown` — `FUN_3989a3e0` zeroes the GCD pair when set | **[R]** 3989a290/3989a3e0 |
| `0xdd` | u8 | "skip" flag used at fire | **[I]** 3939a810, 3939b010 |
| `0xe4` | i32 | skill category id for the target-override gate | **[R]** 393925c0 |
| `0xe8` | i32 | skill-category index (-1 = none) → unit `+0x4b8 + 0xc0*idx` | **[R]** 393925c0 |
| `0xf8` | u8 | has cast time (fire computes seconds from `desc+0x14`?) | **[I]** 3939a810 |
| `0xf9` | u8 | has channel time | **[I]** 3939a810 |
| `0x104` | char[] | skill name (used for the cast bar) | **[R]** 394bcf60 |
| `0x114` | i32 | timing/attack-speed scaling input | **[I]** 3939b8c0 |
| `0x118` | i32 | skill-controller key | **[R]** 3939a810, 3939b8c0 |
| `0x128` | i32 | "skill kind/sequence" compared to `DAT_3a177724` | **[I]** 3939b010, 3939b8c0 |
| `0x13a` | u8 | show target casting time / cast-bar visible | **[R]** 394bcf60, 3939a810, 3939b010 |
| `0x13c` | i32 | controller/target kind gate | **[I]** 3939b010 |
| `0x140` | u8 | skill-controller start gate | **[I]** 3939b010 |
| `0x158` | u8 | `casting_delayable` (start-delay path `FUN_39390f80`) | **[I]** 39390f80 |
| `0x160` | i32 | camera/controller parameter | **[R]** 3939b8c0 |
| `0x164` | i32 | camera/controller parameter | **[R]** 3939a810 |
| `0x188` | i32 | "big hit" threshold (`float(desc+0x188) > 1.0` → alt presentation) | **[R]** 39391950 |
| `0x1b0` | i32 | target selection type (0 none, 1/4/5/6/0xd switch in Fire/click) | **[R]** 3939b010, 390f4c30 |
| `0x1b4` | i32 | effect/attack range? | **[?]** |
| `0x1bc` | i32 | `default_gcd` (1 = 1 s tag GCD) | **[I]** 3939a810, 3939b010 |
| `0x1c0` | i32 | skill category (compared to `DAT_3a17770c`) | **[I]** 3939b010 |
| `0x1c4`, `0x1cc` | i32 | timing/model ids passed to `FUN_39005b20` | **[R]** 3939b8c0 |
| `0x1c8` | i32 | attack-speed/timing input | **[R]** 3939a810, 3939b8c0 |
| `0x1d1` | u8 | `use_weapon_cooldown_time` (weapon-speed vs fixed cooldown) | **[I]** 3939b010, 3939b8c0 |
| `0x1d2` | u8 | enable camera shake (`FUN_3915afb0`) | **[I]** 3939a440 |
| `0x1e8..0x1ec` | vector | skill-controller list: each entry is `{i32 kind; i32 value; ptr params}` and the param block has `+0x1c → +4 kind, +8 value` | **[R]** 3939a810, 3939b010 |
| `0x238..0x23c` | vector | skill tag ids (from `tagged_skills`); scanned by `FUN_3989a290` | **[R]** 3989a290 |

Not read here but present in the same row/SELECT: `casting_time` (used indirectly by
`+0x14`-derived cast time in the packet), `channeling_time`, `cooldown_time`, `cooldown_tag_id`,
`min_range`, `max_range`, `target_angle`, `front_angle`, `check_obstacle` → server-side validation
only in 1.2 (`re-combat.md` §2.3).

## 2. `SkillCaster` / `SkillCastTarget` / `SkillObject`

Used by value on the stack; passed by pointer.

### 2.1 `SkillCaster` (`0x24` bytes; read by `FUN_39390530`, `FUN_39391da0`)

```
+0x00 u32 type          // 0 unit, 1 item, 2 mount/slave, 3 ?, 4 ?
+0x04 u32 pad
+0x08 u32 objId         // type 0/3
+0x0C ...
+0x14 u32 objId2        // type 2
```
* type 1: item object resolved through `FUN_39035110(+0x08)`, the item id is at item `+0x84`
  (`FUN_39390530`).
* type 4: object resolved through `FUN_39177dc0(+0x08)`, id at `+0x18`.
* Equality: `FUN_39391da0` compares `type`, then the type-specific ids.

### 2.2 `SkillCastTarget` (`0x20` bytes; `FUN_39391090/910e0`, `FUN_39391e30`)

```
+0x00 u32 type     // 0 unit, 1 position, 2 position2, 3 item, 4 doodad, 5 ? (10.x enum)
+0x04 u32 pad
+0x08 u32 objId
+0x0C f32 x / y / z ...
+0x18 u32 ...      // second object / end point
```
`FUN_39391090` (tank/self-cast) rewrites type 4 → 0 and fills `+0x08` with the caster id.
`FUN_393910e0` additionally replaces a protected/friendly target (`+0x3c28 & 0x100`) with the
caster.

### 2.3 `SkillObject` (`0x28+`; `FUN_39391e30`)

Opaque in the client; only compared (`FUN_39391e30`) and broadcast in the event. 10.x names the
variants `PortalInfo / SavePortalInfo / Text / Position / Unk5 / Unk6 / ItemGradeEnchantingSupport`
(`aaemu-game/src/systems/skills/types.rs`); 1.2's parser (`FUN_397bfd50`) accepts the same
case 1..7 layout.

## 3. `CooldownManager` (inline in `Unit` at `+0x2c48`)

Proved by `lea ecx,[esi+0x2c48]` at `0x3939aab9`/`0x3939aad7` and by `FUN_39899d10`.

```
+0x000  Unit*   owner                 // *param_1; owner+0x3af1 disables the manager
+0x004  u32     unitId                // passed to FUN_3989f900
+0x008  i32     gcdRemainingMs
+0x00C  i32     gcdTotalMs
+0x010  i32     tagCount              // max 150
+0x014  Entry   tags[150]             // 0xc bytes each
+0x71C  i32     skillCount            // max 150
+0x720  Entry   skills[150]           // 0xc bytes each
+0xDC   u8      ignoreGlobal (copy of the queried desc's flag)  // [I] set by 3989a3e0 caller
struct Entry { u32 id; i32 remainingMs; i32 totalMs; };
```
* `FUN_39899d00` clears `+8/+0xc`; `FUN_39899d40` appends; `FUN_39899e50` ticks.
* `FUN_39899f90` inserts into the list keyed by `param_2[0x21]` (`DAT_3a5e12f0` = skill list,
  otherwise tag list), keeping the larger remaining/total, compacting when full, and logs at 150.
* `FUN_3989a290` queries max remaining for skill id `*desc` and all tags `desc+0x238..0x23c`;
  `ignore` (`desc+0xdc`) returns `(0,0)` for the GCD pair.

## 4. `Unit` combat fields (x2game `X2Unit` actor object)

| off | type | meaning | set/read |
|---|---|---|---|
| `0x008` | u32 | object id | everywhere |
| `0x00C` | char[] | name | `FUN_393905b0` |
| `0x1AA0` | u32 | current target object id (atomic 4-byte read/write) | `FUN_394270a0`, `FUN_3945fcd0` |
| `0x2C48` | `CooldownManager` | inline cooldown manager | 3939a810 |
| `0x3AC4` | u32 | target key of the active normal cast (sentinel `DAT_3a177710`); the state is `FUN_393953a0(key)` | 39395b50, 394bcf60 |
| `0x3AC8` | u32 | key of the active cast-delay/plot event (sentinel `DAT_3a17772c`); state is `FUN_3938c2e0(key)` | 39395b50, 394bcf60 |
| `0x3ACC` | u32 | target key of the active channel | 39395b50, 394bcf60 |
| `0x3AD0` | u32 | key of the active channel-delay/plot event | 39395b50, 394bcf60 |
| `0x3AD5` | u8 | auto-attack pseudo-skill active | 39398410, 39393920 |
| `0x3BBA` | u8 | in-combat byte (`UnitCombatState`) | 390b36f0/3710 |
| `0x3BBC` | u8 | force-attack byte (`UnitIsForceAttack`) | 391d3c60 |
| `0x3BBE` | u8 | "interactable/attackable?" read by target frames | 122724 etc. |
| `0x3BE8` | `GameClient*` | back-pointer | 390b3750 |
| `0x3C26` | u8 | mounted / attached-to-mount | 39391670, 393925c0 |
| `0x3C27` | u8 | rider/attachment partner flag | 39391670 |
| `0x3C28` | u32 | relation bits (0x100 = protected/friendly target) | 393910e0 |
| `0x0410` | i32 | unit kind (5 = player-owned?; 1 = mount) | 39391670, 3939a810 |
| `0x0420` | u32 | owner/controller id | 39391670 |
| `0x04AC` | i32 | class id (4 = ?) used by a requirement evaluator | 3939d7b0 |
| `0x04B8` | `SkillCategory[?]` | 0xc0-byte per-category state (`+0x4b8 + cat*0xc0`; `+8` = locked skill object id) | 393925c0 |
| `0x05E8` | u32 | first-hitter object id | 394bd9f0 |
| `0x05EC` | u32 | first-hitter team/relation (0 = self, `DAT_39d44c50` = my team) | 394bd9f0 |
| `0x0D88` | u32 | equipped mainhand item id (weapon cooldown selector) | 39898ef0 |
| `0x0B48` | u32 | equipped offhand item id | 39898ef0 |
| `0x0C00` | u32 | ranged weapon id | 39898ef0 |
| `0x020` | i32 | buff count (max 32), rows at `+0x28`, 0x68 B each | 39386e00 |
| `0xD28` | i32 | debuff count (max 24), rows at `+0xd30` | 39386e00 |
| `0x16F0` | i32 | hidden third list count (max 24), rows at `+0x16f8` | 39386e00 |

`UnitCastingInfo` (`FUN_394bcf60`) reads the four cast states in order:

```
normal  : unit+3ac4 → instance+0x04 skillId, +0x138 total, +0x144 start
channel : unit+3acc → instance+0x04 skillId, +0x140 total, +0x148 start
castdelay: unit+3ac8 → instance+0x08 skillId, +0x74 total, +0x70 start
chandelay: unit+3ad0 → instance+0x08 skillId, +0x84 total, +0x80 start + 0x84
```
and pushes `(skillName=desc+0x104, total, current, desc+0x13a)`.

## 5. Skill objects

There are **two** objects. They are often confused because both are reached through the manager
map keyed by the target id.

### 5.1 `SkillCastState` — map value `+0x18` (the timeline object)

`FUN_39394e70(manager, &key)` returns the map value; the cast state is at value `+0x18`.
`FUN_393953a0(key)` wraps exactly that: it looks up the same tree and returns `value + 0x18`
[R, `393953a0`]. Therefore:

```
state+0x004 u32 skillId               // read by UnitCastingInfo (normal/channel)
state+0x138 i32 normal-cast total
state+0x13C i32 normal-cast start?    // written by Fire ([0x4f])
state+0x140 i32 channel total
state+0x144 i32 normal-cast start
state+0x148 i32 channel start
```
The cast-delay / channel-delay states use a **different** map: `FUN_3938c2e0(id)` returns
`plotValue+0x18` (the `ClientPlotMan` tree, `FUN_3938c270`) with skill/plot id at `+0x08` and
`+0x70/+0x74` (cast delay) or `+0x80/+0x84` (channel delay). Treat it as the plot cast bar, not
the skill instance [I from the separate tree]. 
`StartSkill` stores the target id in `unit+0x3ac4/3ac8/3acc/3ad0` and `UnitCastingInfo` resolves
it with `FUN_393953a0(targetId)`; `Fire` gets the same state as `value + 0x18` and writes
`[0x4e..0x52]` (= `+0x138/+0x13c/+0x140/+0x144/+0x148`). **U1 is resolved by this identity.**

### 5.2 `ClientSkill` (0x2e0 bytes) — client-only / script-started skills

Created by `FUN_3939b8c0` (alloc `0x2e0`, `FUN_393911d0` ctor), stored in the manager's
client-only vector `+0x6c`, not in the target-keyed tree.

```
+0x000 u32 skillId                 // *local_c = param_1
+0x004 u32 secondId                // param_2
+0x008 SkillCaster (0x20)          // 4 qwords
+0x028 SkillCastTarget (0x20)      // 4 qwords
+0x060 SkillObject (0xD8 = 0x36 dwords)
+0x138 i32 duration                // desc+0xb0 then += computed cast time
+0x13C i32 desc+0x6c
+0x140 i32 desc+0xc4
+0x2D0 i32 localStartTime          // now + desc+0x48
+0x2D4 i32 localEndTime            // now + +0x138 + desc+0x48
+0x2D9 u8  fired
+0x2DA u8  local-damage flag (set when param_8 of FUN_3939b8c0)
+0x2DC..  local damage/effect copy (hit record 0x188 bytes)
```
`UpdateClientOnlySkills` (`0x3939bfd0`) drives this object. The map value in 5.1 is a different
wrapper; only its `+0x18` timing block is read by the cast-bar and fire code.

## 6. `SkillEvent` (UI event payload, vtable `0x399d1c18`)

Constructed by `FUN_39392010(this, a, b, c, d, q1, q2, q3, q4)`:

```
+0x00 vtable 0x399d1c18
+0x04 u32  a
+0x08 u32  b
+0x0C u32  c
+0x10 u32  d
+0x18..0x38  4 qwords (caster/target/skill object snapshot)
```
Event ids seen: `0x14` skill started (`StartSkill`), `0x15` skill fired (`Fire`), `0x16` skill ended
(`EndSkill`), `0x17` auto-attack stop (`FUN_39398410`), `0x18` casting stopped (`FUN_39396000`). Dispatched by
`FUN_39202db0(table, id, &event)`.

## 7. `ActionBar` / `X2Action`

* 85 slots, `0x10` bytes each, at X2Action object `+0x90` (`FUN_39089c60` copies `0x550`).
* Slot: `+0x0 u8 kind` (0 none, 1 item, 2 spell, 3 macro, 4 item_id), `+0x8 u32/u64 id`
  (`FUN_39089c80` checks `slot+0 == 2` and rewrites `slot+8`).
* Action pages: 4 (0..3); Lua is 1-based (`FUN_393f6910`, `FUN_3943f8a0`, `FUN_3943f930`).
* `action_bar_button` is action index **82** in the fixed table `0x39ac8e68`; the multi-binding
  value is the 1-based slot (1..24 from `default_binding.g`).

## 8. `RequirementResult` (8 bytes)

Every `FUN_393a0510` evaluator returns:

```
+0 u8  result    // SkillResult (FUN_39897990); 0 = pass
+1 pad
+2 u16 textId    // optional message id (e.g. 0x29e, 0x33f, 0x340)
+4 u32 value     // failing item/skill/unit id
```

`FUN_3936c5f0` uses the same shape for buff `skill_reqs` (`0x32` = `UNIT_REQS_OR_FAIL`).

## 9. `Reaction` table (`FUN_393917f0`)

Input `hitType` → `(animId, kind)`, all ids from `.data`:

| hitType | anim id | kind |
|---|---|---|
| 1, 9, 0xD | `DAT_3a177728` (none) | 3 |
| 3, 0xB, 0xF | `0x39ac7680` | 3 |
| 4, 0xA, 0xE | `0x39ac768c` / `0x39ac7690` (weapon-dependent) | 3 |
| 5, 0x10 | `0x39ac7678` | 3 |
| 6, 0x11 | `0x39ac7674` | 1 |
| 7, 0x14 | `0x39ac7670` / `0x39ac766c` | 1 |
| other, crit | `0x39ac7684` / `0x39ac767c` | 3 |
| other, normal | `0x39ac7698` / `0x39ac7694` | 3 |

`FUN_39354600()` selects the "crit/skill" vs "melee" column; `FUN_397cd5f0` gates one variant.

## 10. `HitRecord` (SCUnitDamaged payload, copied to the instance)

`FUN_391d5fb0` copies `0x62` dwords (`0x188` bytes) from packet `+0x10`; `FUN_39398c80` reads:

```
+0x00 i32 sourceKind (0 skill, 1 ?, 2 buff, 3 passive buff, 4 doodad)
+0x04 u32 sourceId
+0x08 u32 skillOrBuffId
+0x18 SkillCaster block (type at +0x18, target ref at +0x20 / +0x2C)
+0x3C u32 targetId            // first FUN_39360020 argument in 39398c80
+0x44 u32 position/flag word  // passed to FUN_393917f0 with the hit type
+0x48 i32 hitType             // 393917f0 / 3938fe00
+0x4C i32 absorbed
+0x50 i32 damage
+0x54 i32 manaBurn
+0x6E u8  "engage combat" flag (39391670)
+0xDC u8  combat-text/notification flag (39398c80)
```
