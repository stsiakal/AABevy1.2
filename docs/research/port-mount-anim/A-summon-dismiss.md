# A. Summon and dismiss of a mate

Scope: item/pet-bar summon through the server packets to the visible mount, and the dismiss
sequence. Addresses are x2game VAs at base `0x38ff0000` (decompile
`C:/AAEMU_DISK/aa-re/client12/out-x2game/x2game.dll.c`). Sizes from `funcs.csv`. Data constants
from `D:/aa-re/client12/x2game_unpacked_data.dll` (VA - base maps through the PE sections).

The mount used as the example everywhere: item `8159` -> `item_summon_mates` -> npc `5431`
(`item_summon_mates` row `(10, 8159, 5431)` in `D:/aaemu-live/rs-game/Data/compact.sqlite3`),
npc `5431` -> model `450` / actor model `206` (`re-mounts.md` §1). The same path works for every
summonable mate/slave.

---

## 1. Timeline

```
client                                   server
------                                   ------
item use / pet-bar "summon"
  -> skills.use_skill_id 10602
  -> CSStartSkill 0x52 {10602, caster=player, target=0}
                                         SCSkillStarted ...
                                         SCMateSpawned 0x125 (mate record: tl, item, state, exp, mileage)
FUN_391db180 OnMateSpawned
  -> FUN_3933fd00  alloc 0xc0 Mate record, vtable 399cfb18, bind fields
                                         SCUnitState (ordinary unit spawn of npc 5431, server transform)
FUN_391d4c30 OnUnitState
  -> FUN_391b90a0 (split body)
     -> FUN_3936b580  alloc 0x6008 ClientUnit, ctor, InitFromNetUnit
        -> FUN_3936b000 InitFromNetUnit
           -> FUN_390b95e0 Unit::InitModel(modelId, netPos, appearance)
              -> engine actor create (FUN_390eb3b0 / FUN_390e92e0)
     -> if unit kind 5 && own: FUN_3933f0e0 -> FUN_3933e3a0
        -> Mate::SetUnit (binds unit id), state 4, event 0x110 SPAWN_PET
     -> event 0x27 UNIT_ENTERED_SIGHT for the unit
model stream completes
  -> FUN_3935fcb0 OnModelStreamLoadingComplete
     -> FUN_390ba9c0 Unit::ChangeModel if the loaded model id differs
```

Order of `SCMateSpawned` and the unit state does not matter: the unit is created by the ordinary
unit-state path; the Mate record binds to the unit when either side arrives second
(`FUN_3933f1b0` periodically re-checks `Mate+0xb8`, and `FUN_3933f0e0` binds immediately when the
own-mate unit state arrives).

---

## 2. Item use -> skill cast

- The item's skill is resolved by `FUN_3946e770` (182 B): it looks up the item desc, rejects
  `Fists`, accepts categories 0/1/0x16/0x18, calls `FUN_397cf7a0(item, useType)` for the use skill
  and `FUN_396f6390` for the skill desc; logs `can't use equipped item - invalid ...` on failure.
  This is the item-use helper, not the cast itself.
- The Lua `StartSkill` binding is `FUN_394b9fa0` (5,726 B, caller `FUN_39174520`); it is the generic
  skill cast path used by items and skills. The summon is not a special client request: the skill
  id is `10602`, the target is 0/self, and the server owns the spawn.
- Data (`skills` row 10602): `casting_time 0`, `fire_anim_id 393` (`fist_ac_whistle`),
  `fx_group_id 929`; `fx_items 1451` = `x2_sounds.particles.whistle` (`fx_location_id 1`,
  `FxParticle`). There is **no spawn particle/model effect** in the item data [R].
  `re-mounts.md` §1 records the same item chain.

The caster-side whistle is the only summon effect; the horse appears from the server's unit state.

---

## 3. SCMateSpawned -> Mate record

| function | size | role |
|---|---:|---|
| `FUN_391db180` | 117 | packet listener `OnMateSpawned`; wraps `FUN_390115a0("OnMateSpawned",2,...)` and calls `FUN_3933fd00(*(u32*)(packet+0xc), packet+0x10)` |
| `FUN_3933fd00` | 134 | creates the Mate record: `alloc(0xc0)`, vtable `PTR_FUN_399cfb18`, calls `FUN_3933ab60` + `FUN_3933ac10(tl, item, state, exp, mileage, gender, slots, ?)` then `FUN_3933fb80` inserts it into the mate map |
| `FUN_3933ac10` | 274 | Mate field init, see layout below |
| `FUN_3933ab60` | 162 | base/list init (`memset` only) |
| `FUN_3933fb80` | 125 | map insert (`FUN_3933f140`/`FUN_3933ee90`) |
| `FUN_391df0e0` | 132 | `OnMateState` listener; `FUN_39360020(unitId)` then `FUN_39899d40(packet+0x10)` (167 B) updates the record state |

`FUN_3933fd00` observes: if `*param_3 != DAT_3a168324` (local character id) it stores the passed
id at `Mate+0x00`; then always allocates a record and calls `FUN_3933ac10(param_2, p[2], p[3],
p[5], p[6], (char)p[4], p+8, p[7])`, where `p = packet+0x10`. Field mapping is by order; the
exact server field names are in `aa-net/src/mate.rs` (`SCMateSpawned` ten slots). [R for the
order, I for the semantic names beyond item/tl/exp/mileage documented in `re-mounts.md`]

Mate record layout (offsets written by `FUN_3933ac10`):

| offset | value |
|---:|---|
| `+0x14` | timeline/tl id (param_2) |
| `+0x18` | item id (param_3) |
| `+0x1c` | param_4 |
| `+0x20` | unit object id (set later by `FUN_3933e3a0`; initialized to sentinel `DAT_3a1682f4`) |
| `+0x24` | byte flag (set if any equip slot has implicit data) |
| `+0x28` | 0 |
| `+0x2c/+0x30` | distance timers (auto-dismiss) |
| `+0x34` | slot count (param_7) |
| `+0x3c/+0x40` | exp/mileage values |
| `+0x44..+0x58` | sentinels (`DAT_3a1682f4`), `+0x58` = `DAT_3a16831c` |
| `+0x60..+0xb0` | 10 equip slots, 8 bytes each (`item id`, byte flag); flag set when the item is
absent and the item/state are 0 |
| `+0xac` | count of slots with data |
| `+0xb0` | stuck/state flag |
| `+0xb4` | stuck timer |
| `+0xb8` | next update time (`now + param_9`) |
| `+0xbc` | dismiss-pending byte (set by `FUN_39362070`) |

---

## 4. Unit state -> ClientUnit -> model

`FUN_391d4c30` (157 B, no callers in `calls.tsv` because it is a registered packet listener)
logs `OnUnitState`, rejects the packet during cutscene playing (`+0x74` virtual of
`DAT_3a5bb1d8+0x3c`), otherwise calls `FUN_391b90a0(packet+0x10, packet+0x20b0)`.

`FUN_391b90a0` (8-byte entry, body split into MSVC funclets) is reached from
`FUN_391d4c30` and calls `FUN_3936b580` at `0x391b91c5`. The linear part at
`0x391b90a8..0x391b94a3` proves, in order:

- `FUN_39360020(net+0x00)` lookup; state kind = `net+0x438` (0..6 dispatch table at
  `0x391b9538`).
- own-player check = `net+0x440/0x448` vs local player object id
  `[gameClient+0x40]+0x18`; own player sets `byte[ebp-0xd]` and `FUN_390281c0`.
- `FUN_3936b580(netState, packetPtr, flag)` creates the ClientUnit
  (`alloc(0x6008)`, ctor `FUN_39367d30` 904 B, `FUN_3936b000`, `FUN_3935d130` insert,
  `FUN_3900c1e0`, `FUN_390b4990`/`FUN_394f8c80`).
- kind 5 (mate): `FUN_3933b1b0(unit)`; if own, `FUN_3933f0e0` (bind to Mate; SPAWN_PET);
  the non-own branch `FUN_3933bbf0`.
- kind 2 (player/slave?) logs `client OnUnitState() : slaveInfo.masterId(%d), myId(%d)` at
  `0x391b921b` and calls `FUN_39354580(unit,1000)` when the master id is set; own parent ->
  `FUN_39345d00`.
- kind 1 (NPC) -> `FUN_39342e60`, stealth state `FUN_396962c0`, etc.
- at `0x391b93ec` `FUN_390b3a80(unit,6)` then event `0x27` `UNIT_ENTERED_SIGHT` via
  `FUN_394242a0(0x27, ...)` and the unit info payload.

`FUN_3936b580` (211 B) is the ClientUnit factory:

```
u = alloc(0x6008)
if (u) iVar1 = FUN_39367d30(*netState)          # ctor
FUN_3936b000(netState, packet, flag)            # InitFromNetUnit
FUN_3935d130(&id, FUN_39362450(&id), 0)          # insert into the actor world
FUN_3900c1e0(*(u+8))
FUN_390b4990(&local); if (local != sentinel) FUN_394f8c80(local, netState+0x15a)
```

`FUN_3936b000` (`ClientUnit::InitFromNetUnit`, 1,146 B) is the key function. It:

1. logs `[InitClientUnit] name(%s), id(%u), model(%u), pos(%3.3f, %3.3f, %3.3f)` with
   `net[0]` id, `net[0x159]` model id, position at `net+0x800` (the decompiler's varargs;
   the position comes from the net unit's transform block);
2. `FUN_398a1f90(net, this+8)` copies the net unit state; `FUN_39388440`, `FUN_398a0170`,
   `FUN_39899d10`;
3. if `this+0x3a7c == 0` -> `FUN_390b4df0`, `this+0x3c60 = 5000`;
4. copies `this+0x3c40 = net[0x159]` (model id);
5. sets the streaming/spawn state `this+0x3c54` (1/2) from `FUN_3936a980`/`FUN_39310300`/
   `FUN_39318160`; `this+0x3c58` = `FUN_39310300()` result;
6. copies the appearance/equipment block from `net+0x400` into `local_50..local_38`;
7. **`FUN_390b95e0(net[0x159], net+0x802, &local_50, param_2)`** — model id, transform at
   `net+0x2008`, appearance, spawn flag. This is the only model creation call;
8. `FUN_3936a630`, possibly `FUN_39362770(net+0x800,9)` (attach if the net state carries an
   attachment), `FUN_3917e0a0` (non-local unit), `FUN_39360920`, team/friend slots,
   `FUN_3938a770`, `FUN_390b4730`, `FUN_3935c270`, `FUN_3936a430`, `FUN_39028240`.

`FUN_390b95e0` (`X2::Unit::InitModel`, 496 B):

- `FUN_390b53c0(local)` builds the actor desc.
- normal path: `FUN_390eb3b0(this+8, desc, appearance, modelId, flag)` -> actor;
  `actor.vt[0x130](2,1,1)` (constant `DAT_39ac389c = 2`); `actor.vt[0xd0](appearance)`.
- alternate path (flag set): `FUN_390e92e0(this+8, desc, appearance, modelId)`.
- sets actor `+0x100/+0x104/+0x108/+0x10c` (view/stream values; `+0x104 = 0`,
  `+0x108 = 1.0`; for unit kind 1 `+0x100 = 0.5` and reads `model+0xa4`), then
  `FUN_390bb190`, optional shadow flags, increments `actor+0x44`, calls `this.vt[0x58]`.
- There is **no alpha fade** in this function; `+0x104/+0x108` are not an alpha ramp (the
  literals are 0 and 1.0, no time parameter). [R]

Spawn timing: the ClientUnit and actor exist synchronously. The visual model is the engine's
model stream; when it completes `FUN_3935fcb0` (`ClientUnit::OnModelStreamLoadingComplete`,
505 B) runs: if `this+0x3c54 == 0` it calls `FUN_3911e730`; else if `*modelId != this+0x3c40`
it calls `FUN_390ba9c0` (`Unit::ChangeModel`, 882 B) to swap the model, preserving transform
(`actor.vt[0xd8]`/`+0x228`) and velocity. There is no x2game fade/appear timer.

`FUN_3933f0e0` (93 B) owns the own-mate bind:

```
if (unit+0x410 == 5 && unit+0x420 == [gameClient+0x40]+0x18) {
    tl = unit+0x418;
    mate = map_lookup(tl);
    if (mate != currentMate && mate+0x10) FUN_3933e3a0(unit);
}
```

`FUN_3933e3a0` (`Mate::SetUnit`, 471 B) sets `Mate+0x20 = unit+8` (object id), reads the model
desc (`FUN_396962c0`, `FUN_39696180`, `FUN_396961c0`) to fill speed/name data, calls
`FUN_39007a10(unit, name, value)`, sets state 4 and calls `FUN_3933c8f0`. For an own mate it
fires **event `0x110`** (`FUN_39424220(0x110)` inside it) and calls
`FUN_391f0120()->vt[0x10](0x28)`. `0x110` is `SPAWN_PET` (event table `FUN_39214380`: it stores the name pointers into
consecutive globals from `0x39d22ce8`, one per index; `0x111` = `DISMISS_PET`, `0x112` =
`MOUNT_PET`, `0x113` = `UNMOUNT_PET`). The dispatch `FUN_3920fa60` logs
`FireUIEvent - %s` with `(&DAT_39d22ce8)[id]`, proving the table is indexed directly by the id
(0-based). Cross-check: index 95 = `MODE_ACTIONS_UPDATE`, and `FUN_392045f0` sends exactly
`0x5f` (95). The pet frame (`unitframe/pet.alb`) listens for `SPAWN_PET`.

The unit visual is placed exactly at the net transform; no client offset relative to the player
is applied. The local test server's "3 m in front" is a server placement
(`aaemu-game` `mates.rs:spawn_mount`), not a client rule [R for client, `re-mounts.md` §1 for the
server].

### Initial pose

The unit state's graph inputs are initialized by the ordinary actor/model path. Attach/summon
writes no graph input. The horse graph's default input set (`PseudoSpeed 0`, `Stance relaxed`,
`Action idle`) therefore selects the hub -> `Idle Start` ->
`fist_ba_relaxed_idle_anim` (`horse_full_body.xml:585`, `re-mounts-full.md` §3). No spawn state
exists in the graph. [R for the graph contents, I for the exact net-state default if the server
sends a non-idle action.]

### Spawn effect

- The only item effect is the whistle (`fx_items 1451`, see §2). [R]
- The generic entity spawn-effect scanner `FUN_39109120` (122 B) walks the model's animation list
  for a name beginning `_spawn_effect` and calls `FUN_391082f0(1)`. `FUN_391091a0` does the same
  for `_despawn_effect`. These are for NPC spawners/doodads with authored spawn animations.
- `horse_full_body.xml` contains no `_spawn_effect`/`_despawn_effect` state (grep count 0), so
  nothing fires for a summoned mate. [R]
- The NPC spawner effect tables (`npc_spawner_spawn_effects`, loaded by `FUN_39730250`) are a
  server-side NPC-spawner feature, not the mate summon path. [R for the loader, I for the
  conclusion]

---

## 5. Dismiss

Two entry points:

- Lua `X2Mate:DismissPet()` -> `FUN_39362070` (254 B).
- The pet bar's CYA command (`PET_CMD_CYA`) -> the same `FUN_39362070` (the script calls
  `DismissPet`; `pet.alb` strings).

`FUN_39362070`:

1. `FUN_39357830(&result)` gating (casting/vehicle state); on refusal `FUN_39390900`.
2. `ownMate = FUN_3933f5e0()` (current own Mate record).
3. If the local player is attached to that mate (`[gameClient+0x10c]+0x3af0 == ownMate->unit`):
   `FUN_39361e80(10)` -> `CSUnMountMate` 0xa8 `{unit+0x418, player+0x3af4, 10}`; return. The
   normal unmount path then runs (B).
4. Else after the 2 s cooldown (`now - _DAT_3a169944 > 1999`): start
   `UnMountHorseController` via `FUN_390ca560(ownMate->unit, 10, 0)`; set
   `ownMate+0xbc = 1`; fire **`0x111` `DISMISS_PET`**.

The controller ends after 1000 ms (reason 10) and the client sends `CSRemoveMate` 0xa4 from
`FUN_3933f040` (156 B): it maps tl -> unit, builds the `0xa4` packet, sends it, removes the
record (`FUN_3933ab60`), and fires `0x111` again (`FUN_39424220(0x111)`). Its callers are
`FUN_3933f1b0` (auto-dismiss: too far for 5 s, or "stuck" timer 60 s) and `FUN_3936f540` (a
server-message continuation; exact opcode/flow [?]). The observed live sequence is in
`re-mounts.md` §9.

Auto-dismiss thresholds are static squared distances: `0x39ac7404 = 3600.0` (60 m²) and
`0x39ac7400 = 49.0` (7 m²); timers 5000 ms and 60000 ms. [R for constants and code, I for the
semantic "too far/stuck"]

---

## 6. Function table

| addr | size | callers | callees | offsets / globals | kind | test |
|---|---:|---|---|---|---|---|
| `3946e770` | 182 | Lua item-use binding | `39695dc0`, `397cf7a0`, `396f6390` | item desc `+0x70/+0x48`; `DAT_3a5aa6b4` sentinel | pure-ish | call cdecl with a scratch item desc copy; returns skill id or 0 |
| `394b9fa0` | 5726 | `39174520` | (skill cast chain) | generic skill cast | stateful | only as an integration entry; not a unit test |
| `391db180` | 117 | packet registry | `390115a0`, `3933fd00`, `390116d0` | packet `+0xc` tl, `+0x10` payload | stateful | call `thiscall` with a scratch packet (`this` unused? ECX = listener) and a live mate map |
| `3933fd00` | 134 | `391db180` | `3950a1b0`, `3933ab60`, `3933ac10`, `3933fb80` | Mate `+0x14/+0x18/+0x1c/+0x20/+0x24/+0x2c/+0x30/+0x34/+0x3c/+0x40/+0x44..+0xbc` | stateful | call `thiscall` (ECX=map, args packet ptr, payload); dump 0xc0 record |
| `3933ac10` | 274 | `3933fd00` | `3963b5d0` | see record layout | stateful | call `thiscall` on a scratch 0xc0 zero buffer; dump fields |
| `3933ab60` | 162 | `3933fd00`, `3933f040`, `3933f9d0` | `memset` | base | stateful | zero-init scratch; no return |
| `3933fb80` | 125 | `3933fd00` | `3933f140`, `3933ee90` | map | stateful | insert/lookup with a scratch map |
| `391df0e0` | 132 | packet registry | `390115a0`, `39360020`, `39899d40` | packet `+0xc` unit id, `+0x10` state | stateful | scratch packet + live unit |
| `3933f1b0` | 696 | `3936a750` | `3933e3a0`, `3933f040`, `390b3ad0`, `3933cb60` | Mate `+0x14/+0x24/+0x2c/+0x30/+0x34/+0x50/+0xb0/+0xb4/+0xb8` | stateful (per-frame) | call `thiscall` on a live Mate with a controlled clock; inspect timers |
| `3933f040` | 156 | `3933f1b0`, `3936f540` | `3933ed80`, `397ac220`, `39360020`, `3933e580`, `3933ab60` | opcode 0xa4 | stateful | call with a live mate; capture the outbound packet |
| `391d4c30` | 157 | packet registry | `390115a0`, `391b90a0`, `390116d0` | packet `+0x10` state, `+0x20b0` slave block | stateful | call with a scratch unit-state packet; needs the live world |
| `391b90a0` | 8 | `391d4c30` | split body (see §4) | `net+0x00/0x438/0x440/0x448` | stateful | not separately callable (funclet entry) |
| `3936b580` | 211 | `391b90a0` (split) | `3950a1b0`, `39367d30`, `3936b000`, `39362450`, `3935d130`, `3900c1e0` | 0x6008 object | stateful | call with a scratch net unit + flag; returns ClientUnit* |
| `39367d30` | 904 | `3936b580` | `390bad40`, `3950a1b0`, `39410b40`, `3938a2d0` | ClientUnit fields | stateful | ctor; call on scratch 0x6008 buffer |
| `3936b000` | 1146 | `3936b580` | see §4 | ClientUnit `+0x3c40/+0x3c54/+0x3c58/+0x3be8`, net `+0x159/+0x800/+0x802` | stateful | call `thiscall` with a scratch net state; dump ClientUnit |
| `390b95e0` | 496 | `3936b000`, `393661c0` | `390b53c0`, `390eb3b0`, `390e92e0`, `396962c0`, `390bb190` | actor `+0x100/+0x104/+0x108/+0x10c`, `this+0xefa` | stateful | call with a live actor; inspect created model |
| `390ba9c0` | 882 | `3935fcb0` and other model changes | `390b53c0`, `390e3100`, `390eb3b0`, `390e92e0`, `390bf530`, `390e3200` | actor `+0x100..+0x10c/+0x44` | stateful | call with a live actor and a target model id |
| `3935fcb0` | 505 | `3917ab00` (stream callback) | `3911e730`, `390ba9c0`, `390b4990`, `394f8c80` | `+0x3c54/+0x3c40/+0x3c4c` | stateful | call with a live ClientUnit after forcing a stream completion |
| `3933f0e0` | 93 | packet dispatch (split) | `3933ed80`, `3933e3a0` | unit `+0x410/+0x418/+0x420` | stateful | call with a live own mate unit |
| `3933e3a0` | 471 | `39096030`, `3933f0e0`, `3933f1b0` | see §4 | Mate `+0x20/+0x28/+0x34/+0x38/+0xb0`, event `0x110` | stateful | call with a live Mate + unit; capture event |
| `39424220` | 19 | many | `3920fa60` | event id (0..625) | thin | call `cdecl` with an event id; UI observability |
| `39109120` | 122 | `390cf130`, `390e7910`, `390eb7d0`, `391797f0` | `XlStringLen`, `XlComparePrefixI`, `391082f0` | model animation list `+0x18/+0x1c` | stateful | call with a live model; proves no `_spawn_effect` for the horse |

Data used by the tests: event ids from the registration order in `FUN_39214380`
(`0x110 SPAWN_PET`, `0x111 DISMISS_PET`, `0x27 UNIT_ENTERED_SIGHT`); constants
`0x39ac7404 3600.0`, `0x39ac7400 49.0`; item/skill rows as in §2.
