# Differential tests against the running 1.2 client

Goal: prove each ported Rust function matches the original by calling the **original** function
inside the hidden 1.2 client (`aadriver.dll`, already injected in-process and running Lua) with
chosen inputs, then comparing with the Rust port. This file assumes the calling conventions and
harness plumbing of [`../port-movement/difftest.md`](../port-movement/difftest.md) §1 and §4; only
the combat-specific state and recipes are listed here.

No original code is shipped; the driver only calls the shipped binary in memory.

## 1. State to capture

| name | what | size | where |
|---|---|---|---|
| `U` | local player unit object | live | `FUN_39360020(local_id)`; local id at `DAT_39b02294+0x10c` → `+8` |
| `CD` | cooldown manager | `0x1260` | `U + 0x2c48` (inline; `lea ecx,[esi+0x2c48]` at `0x3939aab9`) |
| `CAST` | cast state | `0x20` | `U + 0x3ac0` (`+0x3ac4/3ac8/3acc/3ad0/3ad5`) |
| `COMBAT` | combat/force bytes | `8` | `U + 0x3bba` (`+0x3bba/3bbc/3bbe`) |
| `BAR` | X2Action object + 85 slots | `0x90 + 85*0x10` | singleton found from a live `X2Action` script call or by hooking `FUN_3943fa10` |
| `INST` | skill instance | `0x2e0` | `FUN_393953a0(id)` for the active skill ids in `CAST` |

`aa_combat_capture(kind)` copies `U+0x2c48 .. +0x3f00` (cooldowns + cast + combat + target id) and
`aa_combat_restore(kind)` writes it back after each call, so tests cannot poison each other.

The **packet stream** is the second oracle: hook `0x3ac1058c` (the send finalizer, see the movement
map §5) and copy the buffer. C→S combat opcodes of interest: `0x052` CSStartSkill, `0x02c`
CSChangeTarget, `0x04e` CSResurrect, plus the force-attack packet (opcode not yet captured).

## 2. Pure leaves (synthetic state, no world)

### 2.1 `SkillResult` → text (`FUN_39897990`, 955 B)

* `__cdecl const char* f(int result)`; inputs `0..0x82`, plus `-1`, `0x100`, `0x7fffffff`.
* Record the returned pointer's string for every value; the Rust table must match byte-for-byte.
* Also call `FUN_393f4890(result)` only with a live UI; that hits `ui_texts` (not pure).

### 2.2 Caster/target equality (`FUN_39391da0`, `FUN_39391e30`)

* `__fastcall int f(void* a, void* edx, void* b)`; 0x24/0x40-byte zero buffers.
* Matrix: each type 0..4, equal and unequal ids; NaN, ±0, ±1.0 in the float fields.
* Compare the returned eax and the untouched bytes (functions must not write).

### 2.3 Cooldown manager (`FUN_39899d00 … FUN_3989a450`)

Synthetic `CD` buffer (zeroed), `unit` pointer at `+0` = live `U`.

| function | wrapper | inputs | expected |
|---|---|---|---|
| `0x39899d00` | `__fastcall(CD)` | — | `+8/+0xc == 0` |
| `0x39899d10` | `__fastcall(CD, edx, unit, unitId)` | — | `+0=U`, `+4=id`, lists zero |
| `0x39899d40` | `__fastcall(CD, edx, list)` | 0, 1, 150, 151 entries; ids 1/2/3 | append order, cap 150, `remaining/total` copied |
| `0x39899df0` | `__fastcall(CD, edx, tagKey, customMs)` | tagKey from `FUN_39005b20()`, custom `{0,1,500,3000}`, attr `0x4a` `{0,500,1000}` | `gcd = custom*1000/(1000+attr)` in `+8/+0xc` |
| `0x39899f30` | `__fastcall(CD, edx, tagKey)` | same attr values | `gcd = 1e6/(1000+attr)` |
| `0x39899e50` | `__fastcall(CD, edx, dt)` | dt 0/1/16/1000 with populated lists | each remaining decreases, clamps at 0, global pair |
| `0x39899ec0` / `0x39899f00` | `__fastcall(CD, edx, id)` | missing/duplicate/zero ids | only matching `remaining=0`, no compaction |
| `0x39899f90` | `__fastcall(CD, edx, descOrTag, ms)` | desc `+0x21` = `DAT_3a5e12f0` vs not; 149/150 entries | insert-or-max, compaction at 150, log lines |
| `0x3989a290` | `__fastcall(CD, edx, desc, &r, &t)` | synthetic desc: `+0=id`, tag vector at `+0x238`; `+0xdc=0/1` | max remaining/total, ignore-global zero pair |
| `0x3989a3e0` / `0x3989a400` / `0x3989a450` | same | populated manager | pair, state 0/1/2, remaining |

* Convention check: `3989a290` is `__thiscall` with two output pointers on the stack; wrap as
  `__fastcall(CD, edx, desc, rPtr, tPtr)`.
* The tag vector in a **synthetic** desc must point to a real array; the function only reads it.
  A live desc from `FUN_396f6390(id)` is the safer oracle for the tag-scan test; snapshot 0x10
  bytes at `desc+0x238` and restore.

### 2.4 GCD producers against the real skill desc

For a loaded skill id with a known `custom_gcd` and `default_gcd` (e.g. 10501 custom 500,
`default_gcd f`; 12039 `t`; 12759 `t`), call `FUN_39899df0`/`FUN_39899f30` on a zeroed `CD` with
the *real* desc-derived arguments and compare `+8/+0xc` with the Rust formula. This settles the
"does `global_cooldown_mul` scale GCD" question (`MAP.md` §4.3).

### 2.5 Reaction table (`FUN_393917f0`, 295 B)

* `__cdecl void f(int hitType, int unit, int* animOut, int* kindOut)`; unit may be `NULL`
  (the function only uses it for `FUN_397cd5f0`/`FUN_39354600` gates).
* Inputs: hit types 0..0x18 + 0xff, with `FUN_39354600` forced both ways (drive the weapon/skill
  state or patch the return in the driver).
* Compare `(animOut, kindOut)` exactly (integer ids).

### 2.6 Event payload ctor (`FUN_39392010`, 79 B)

* `__thiscall(this, a,b,c,d,q1..q4)`; zero 0x40-byte buffer.
* Compare the 0x38 bytes after the call; vtable must be `0x399d1c18`.

### 2.7 Action tables

* `FUN_393f8d20(name)` for all 0x68 names in `0x39ac8e68` and 10 near-miss strings.
* Slot accessor: read the X2Action singleton `+0x90 + slot*0x10` for slots 0..84 after loading a
  known `SCActionSlots` save; compare `(kind,id)` with the Rust `ActionBar`.

## 3. Live-state functions

### 3.1 `StartSkill` (`FUN_3939a810`, 2036 B) — the central timeline test

Convention: `__thiscall(manager, casterId, skillId, targetId, &caster, &target, &object,
castTime, castTime2, byte, extra, 0)`; `manager` is the skill-action manager
(`DAT_39d1fba4` at runtime, or the object passed to `FUN_3939a810` from the packet handler).

Recipe:

1. Load the world, learn/choose a skill with a known cast time (e.g. 12039 channel 700 ms).
2. Snapshot `CAST`, `CD`, the skill instance for that target (`FUN_393953a0` ids) and the four
   target-frame UI values.
3. Call the original with a synthetic `SkillCaster/SkillCastTarget/SkillObject` built for:
   self-target instant, unit-target cast, ground-target, mount caster (`type 2`), item caster
   (`type 1`), dead target, protected target.
4. Compare:
   * `U+0x3ac4/3ac8/3acc/3ad0/3ad5` before/after;
   * `CD+8/+0xc` (GCD producer);
   * the instance fields `+0x04/+0x138/+0x140/+0x144/+0x148/+0x13a`;
   * the UI event stream (hook `FUN_39202db0` and record `(id, payload)`);
   * the C→S buffer (CSStartSkill).
5. Restore state; run the Rust `start_skill` with the same inputs and the same captured
   unit/manager snapshot; compare field by field.
6. Repeat with `param_10 != 0` (the server-started early-return path) to prove it sends **no**
   request and creates no instance.

Tolerance: integers exact; times in ms exact; floats 1e-6.

### 3.2 `UseSkill` front-end (`FUN_393989e0`, 657 B) and the real body (thunk `0x39396e20`)

* `UseSkill(unit, skillId, &target, &caster, &object)` → `bool`; it validates with
  `FUN_393925c0` and `FUN_393be4e0`, then calls `thunk 0x39396e20(manager, &result, unit, skillId,
  &target, &caster, &object, 1)`.
* Test the wrapper with a synthetic target/caster:
  * `skillId` nonexistent → 0, no packet;
  * `target` type 0 id 0 → `FUN_393925c0` may rewrite; capture;
  * protected/friendly (`+0x3c28 & 0x100`) → `FUN_393910e0` rewrites to caster;
  * self + moving (`FUN_39426d10()!=0`) → flag byte 0 vs 1.
* The tail body has no usable decompile, so the oracle is the **packet buffer** plus the manager
  state. Hook the send at `0x3ac1058c` and compare the CSStartSkill bytes for each case; this also
  settles `MAP.md` U2 (the exact request fields).
* Call `FUN_39426d10` separately (pure-ish) for the moving matrix; it reads input axes 0..5 and
  `FUN_392aa250(3)`; the driver can zero/set those.

### 3.3 `FireSkill` (`FUN_3939b010`, 2218 B)

* Precondition: an instance in `CAST+0x3ac4` created by 3.1.
* Snapshot the instance, unit, `CD`, and the UI event stream.
* Call with `(casterId, skillId, targetId, &caster, &target, &object, castTime, targetId?, objectPtr?,
  flag, extra, param12, param13)`. Start with the packet-handler argument set from a real
  `SCSkillFired` (capture it first with a hook on `FUN_391d58a0`).
* Compare the instance timings, `CD` (if it adds), the event id (`0x15`), the controller list walk
  and the two `game_vtable+0x120` notifications.
* Matrix: instant (no cast), cast, channel, skill with `desc+0x140 != 0` (ground target),
  `desc+0x13a` on/off, `param_13 != 0` (client-only path).

### 3.4 `EndSkill` (`FUN_39397ff0`), `StopSkill` (`FUN_39398410`), casting stop/delay

* Call with a live instance; compare `CAST`, the instance, the controller stop events and the
  event id (`0x17/0x18`).
* `FUN_39396000(manager, targetId, durationMs)`: snapshot `CD`; compare
  `FUN_39899f90` insertion (remaining/total) and the event payload.
* `FUN_39395760(manager, unitId, deltaMs)` and `FUN_3938c520(manager, instanceId, deltaMs)`:
  compare instance `+0x150/+0x15c` / `+0x88/+0x8c` and the anim-speed getter pair
  (`game_vtable+0x16c` value, `+0x170` setter). Because the setter touches the animation graph,
  capture/restore the graph speed around the call.

### 3.5 Cast bar query (`FUN_394bcf60`, 546 B)

* `X2Unit:UnitCastingInfo(token)` is a script callback: easiest is to call the leaf
  `FUN_394bcf60(callbackObj, token)` and capture the four pushed values via a fake callback object.
* Matrix: no cast, normal cast (start/mid/end), cast-delay, channel, channel-delay, two states
  live at once (priority order normal > channel > cast-delay > channel-delay), target-of-target
  token `"targettarget"`, invalid token.
* Compare `(name pointer/string, total, current, desc+0x13a)` with the Rust `casting_info`.

### 3.6 Update/queue/combo (`FUN_3939c4a0`, `3939bfd0`, `3939cb80`, `3939ce90`)

* Drive one frame by calling `ClientSkillActionManager::Update` on the live manager with a frozen
  "now" (the function reads the engine timer through `DAT_3a5bb1d8[5]+0x10`; the driver can hook
  that slot or step the real clock).
* Seed 1..3 client-only skills with different `+0x2d0/+0x2d4`, then check:
  * transition at `+0x2d0` → `Fire` event (`0x15`);
  * transition at `+0x2d4` → delayed local damage (`FUN_39398c80`) when `+0x2da`;
  * removal after `+0x2d4 + 1000 ms`;
  * combo entries expiring at `+0xc + +0x14 + +8` and `39899ec0` clearing.
* Compare the manager vectors (copy the `+0x6c` vector) and the event stream, not the whole heap.

### 3.7 Requirement dispatcher (`FUN_393a0510`, 1087 B)

* Call with a **real** skill desc that has `or_unit_reqs` (find ids via
  `SELECT owner_id FROM unit_reqs WHERE owner_type='Skill'`), the live caster struct and the live
  target id. Compare the returned `{result,textId,value}` with the Rust `validate`.
* Pure evaluators can be called individually (see table in `functions.md`) with synthetic
  `UnitReq` rows; for each kind, sweep `value1/value2` around the boundaries.
* The client switch has kinds `0,6,9,0xc,0xd,0x18,0x19,0x1a,0x1b,0x1c,0x1d,0x22,0x26` and a
  nested sub-kind under `0x18`; the semantics of each case are `[I]` until the table is aligned
  with `unit_reqs`/`skill_reqs` (U3).

### 3.8 Targeting and combat state

* `FUN_394270a0(unitId, targetId)`:
  * snapshot the target frame fields and `U+0x1aa0`;
  * inputs: self, dead unit, out-of-range unit, `0` (clear), target-of-target chain;
  * compare `+0x1aa0`, the event ids (`2`, `3`), the watch/team side effects.
* `FUN_3945fcd0(token)`:
  * tokens `"player"`, `"target"`, `"targettarget"`, `"playerpet"`, `"team"`, `"watchtarget"`,
    `"playertargettarget"`, `""`, `NULL`, an over-long token;
  * expected `(type, unitPtr)`; compare the pointer with `FUN_39360020(id)` for the same id.
* `FUN_394bd830(token)`, `FUN_394bd9f0(token)`: compare the byte/boolean after forcing
  `U+0x3bba`, `+0x5fe8/+0x5fec` to known values (capture/restore).
* `FUN_390b36f0`/`390b3710`: call and compare `+0x3bba` and the vtable `+0x50` notification count.
* `FUN_391d3c60` (force attack): call with a synthetic packet object and compare `+0x3bbc` and
  event `0xe8`.

### 3.9 Hit presentation

* `FUN_393917f0` — pure table test (§2.5).
* `FUN_39398c80` with a synthetic 0x188-byte record: sweep `sourceKind` 0/2/3/4, hit types,
  `+0x6e` engage flag, local-player vs remote target. Compare:
  * HP/MP deltas through the public getters (`X2Unit:UnitHealth/Mana`);
  * the event stream (`COMBAT_TEXT`, `COMBAT_MSG`, `UNIT_COMBAT_STATE_CHANGED`);
  * the requested reaction id (hook `FUN_393917f0` return) and the anim request
    (`game_vtable+0x120` args).
* `FUN_393985f0` — capture the two `FUN_3940a0f0` outputs and the `FUN_3940abc0` arguments.
* Buff-created stun/knockdown: send a real `SCBuffCreated` for a stun buff, then compare the
  movement input gating (`FUN_39426d10`, axes) and the unit state bytes with the Rust side.
  Buff rows are 0x68 bytes; see `structs.md` §4 and `re-combat.md` §2.6.

## 4. Harness exports

```c
extern "C" __declspec(dllexport) int aa_combat_call(int which, const void* args, int nargs,
                                                   void* out, int out_cap);
extern "C" __declspec(dllexport) int aa_combat_capture(int slot);   // CD + CAST + COMBAT + target
extern "C" __declspec(dllexport) int aa_combat_restore(int slot);
extern "C" __declspec(dllexport) int aa_combat_events(int on);      // hook 39202db0
extern "C" __declspec(dllexport) int aa_combat_packet(int on);      // hook 3ac1058c
```

* `aa_combat_call` dispatches by id (`0` = result-name, `1` = caster compare, `2` = cooldown add,
  `3` = StartSkill, `4` = Fire, `5` = UseSkill, …), validates the pointers against the captured
  unit/manager first, and copies the returned bytes to Lua.
* Event hook: record `(eventId, 0x40-byte payload)` ring buffer; the payload is read-only.
* Packet hook: record the send buffer + length; the driver never modifies it.

## 5. Tolerances and failure policy

| class | tolerance |
|---|---|
| ids, result bytes, event ids, slot kinds | exact |
| cooldown/GCD milliseconds | exact (integer math) |
| cast times | exact ms |
| animation ids / reaction kind | exact |
| floats (target positions, angles) | 1e-6 relative |
| packet bytes | exact after U2 is resolved |

Mismatch handling is the same as the movement harness: log `(function, inputs, original, rust,
delta)` under `data/extract/aa-combat/` (git-ignored), keep the seed case.

## 6. Open items the harness must settle

* **U1 (resolved, keep as a regression check)** — `FUN_393953a0(key)` and `FUN_39394e70`
  return `treeValue`; the cast state is `+0x18`, so `Fire`'s `local_18+0x18` is the same pointer.
  Dump both at `0x3939b0b6` once to confirm.
* **U2** — CSStartSkill byte layout and which fields the thunk `0x39396e20` writes; capture on
  `0x3ac1058c` for instant/cast/channel/ground/mount/item casts.
* **U3** — requirement kind table: map every `FUN_393a0510` case to `unit_reqs`/`skill_reqs`
  rows; the client switch values are read but only partly named.
* **U4** — tab-target loop and click pick: no static xref to `tab_targeting_*`; hook
  `0x3ac1058c` while pressing Tab / clicking, or breakpoint `FUN_394270a0` after the pick.
* **U5** — force-attack request opcode and client eligibility; capture on `0x3ac1058c` after
  Ctrl+F.
* **U6** — `FUN_39899df0`/`FUN_39899f30` argument identity (`FUN_39005b20` static object,
  `[ebp+8]`/`[ebp+0xc]`); read the two stack slots at entry with a wrapper.
* **U7** — which queue margin is used before a cast ends (the `queued_skill_margin` string has no
  code xref); measure by pressing a second skill at varying offsets.
