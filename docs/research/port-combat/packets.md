# Combat packets as the 1.2 client handles them

Companion to [`MAP.md`](MAP.md). Server-side layouts are from `aaemu-game` (our own Rust port of
the C# server) and are used only to name fields; the client offsets below are read directly from
the decompiled handlers. Handler addresses are x2game VAs.

## 1. Registration

The handlers are installed by the big network-init function at `0x391b…` with one setter per
opcode, e.g. `FUN_391bef60(FUN_391d57f0, 0)` for `OnSkillStarted`; each setter writes
`{handler, user_data, flags}` into the client's dispatch table at `this + 4*slot`
(`FUN_391bef60` writes slot `0x288/4 = 0xa2`; the table has a one-slot header, so the slot is the
server opcode + 1). The `On*` names are the binary's own strings.

## 2. S→C handlers

| opcode (server) | handler | size | fields read (client packet object) | effect |
|---|---|---|---|---|
| `0x0a1` SCSkillStarted | `0x391d57f0` | 162 | `+0xc` caster id, `+0x10` skill id, `+0x20` target id, `+0x40` SkillCaster, `+0x78` SkillCastTarget, `+0x150/+0x154` cast times, `+0x158` byte, `+0x14`, `+0x18` | `StartSkill` (`0x3939a810`) |
| `0x0a2` SCSkillFired | `0x391d58a0` | 181 | `+0xc` caster, `+0x10` skill, `+0x18` target, `+0x38` target block, `+0x70` object, `+0x148` time, `+0x14c` byte, `+0x150/+0x154/+0x158/+0x15c` times, `+0x160` byte | `FireSkill` (`0x3939b010`) |
| `0x0a3` SCSkillEnded | `0x391d5bf0` | 114 | `+0xc` target/unit id | `EndSkill` (`0x39397ff0`) |
| `0x0a4` SCSkillStopped | `0x391d5c70` | 118 | `+0xc` unit id, `+0x10` skill id | `StopSkill` (`0x39398410`) |
| `0x0a5` SCCastingStopped | `0x391d5cf0` | 118 | `+0xc` unit id, `+0x10` duration ms | `FUN_39396000` (stop + cooldown) |
| `0x0a6` SCCastingDelayed | `0x391d5d70` | 139 | `+0xc` target/unit key, `+0x10` second key, `+0x14` delay ms | `FUN_39395760` + `FUN_3938c520` |
| `0x04a` SCCooldowns | `0x391d43c0` | 127 | list struct at `+0xc` | `FUN_39899d40` |
| `0x0ad` SCSkillCooldownReset | `0x391d6420` | 192 | `+0xc` unit id, `+0x10` skill id, `+0x14` tag id, `+0x18` gcd bool | `FUN_39899ec0`, `FUN_39899f00`, `FUN_39899d00` |
| `0x084` SCTargetChanged | `0x391d5270` | 115 | `+0xc` unit id, `+0x10` target id | `FUN_394270a0` (fires `TARGET_CHANGED`, id 2) |
| `0x085` SCCombatEngaged | `0x391d69a0` | 178 | `+0xc` unit id | `U+0x3bba=1`; fires UI event 3 for the local player |
| `0x086` SCCombatCleared | `0x391d6a60` | 124 | `+0xc` unit id | unit vtable `+0x48` (leave combat) |
| `0x089` SCCombatFirstHit | `0x391d6ae0` | 132 | `+0xc` unit id, `+0x10` hitter id, `+0x14` team/relation | `FUN_39354780` → `+0x5fe8/+0x5fec` |
| force-attack set | `0x391d3c60` | 241 | `+0xc` unit id, `+0x10` byte | `U+0x3bbc`, UI event `0xe8` |
| `0x0a7` SCUnitDamaged | `0x391d5fb0` | 296 | 0x188-byte record at `+0x10` | `FUN_39398c80` |
| `0x0a8` SCUnitHealed | `0x391d6210` | 114 | record at `+0x10` | `FUN_393992d0` |
| `0x0ab` SCCombatText | `0x391e90b0` | — | `source`, `target`, `text_type` | type 0 system alert event `0x19`; type 1 floating text |
| `0x071` SCUnitDeath | `0x391d51c0` | 165 | `+0xc` unit id, `+0x10` killer id, `+0x14` reason, `+0x20` death block | `FUN_39395ae0` / `FUN_39369c10` |
| `0x0b6` SCBuffCreated | `0x391d5e00` | 144 | `+0x10` buff data, `+0x30` caster, `+0x34` owner id, `+0x38` stack/charge, `+0x3c` buff block | `FUN_3938a3c0` |
| `0x0b7` SCBuffRemoved | `0x391d5e90` | 132 | `+0xc` owner id, `+0x10` index | `FUN_39389d60` |
| `0x0b9` SCBuffUpdated | `0x391d5f20` | 140 | `+0xc` owner, `+0x10` index, `+0x14/+0x18` values | `FUN_393886c0` |

The server layouts (field order and widths) are in `aaemu-game/src/systems/skills/packets.rs`
(SCSkillStarted at 15-80, SCSkillFired, SCCooldowns, SCUnitDamaged at 279-332, SCUnitHealed,
SCBuffCreated at 201-240). Use them only to *name* the client offsets; the client pointer offsets
above are the oracle.

## 3. C→S sends

| opcode (server) | builder | evidence |
|---|---|---|
| `0x052` CSStartSkill | inside the tail body `FUN_3ab63fb7` (thunk `0x39396e20`) called by `UseSkill` | the only caller chain from `FUN_393989e0`; packet bytes not yet dumped (**U2**) |
| `0x02c` CSChangeTarget | `FUN_39186db7`: `packet.opcode=0x2c`, `FUN_397ab790(targetId)`, `3ac1058c` | **[R]** disassembly of 39186db7 |
| `0x04e` CSResurrectCharacter | `FUN_39186e70` (`PTR_LAB_399b2eec`, 0x4e) | re-combat §2.9 |
| force attack | not located; the client state is `U+0x3bbc` | **U5** |
| `0x0b6` CSUpdateActionSlot | `FUN_3930ba60/bad0/bbd0` | re-combat §2.5 |

Packet classes are built on the stack as `{vtable, opcode, payload}`; the finalizer is
`0x3ac1058c` (see `port-movement` §5.3). The send hook used by the combat harness is this address.
