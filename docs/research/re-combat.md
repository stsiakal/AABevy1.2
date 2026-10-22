# Combat and HUD (ArcheAge 1.2 client)

## 1. Scope and sources

Covers skill casting (cast/channel/instant, GCD, cooldowns, refusal texts and target gates), targeting
(click, Tab, target-of-target, drop), action bars (slots, pages, drag, dark states), buff/debuff bars,
player/target frames, floating combat text, death/resurrection UI and the combat packets.

Sources: `x2game.dll.c` (VA in `FUN_<addr>`, image base `0x38FF0000`), the 1.2 UI LuaQ bytecode
(`D:/aa-re/client12/ui/game/scriptsbin/x2ui/.../*.alb`; line numbers are the retained Lua source lines),
`compact.sqlite3` (server copy), and the Bevy client (`crates/aa-client`, `aa-net`, `aa-ui`).
The bytecode was disassembled with `D:/aa-re/client12/tools/albtool.py`.

## 2. The rules

### 2.1 Casting state machine

The client starts every ordinary skill with **`CSStartSkillPacket`, opcode `0x052`** (server table
`aaemu-game/src/opcodes.rs:82`). `X2Unit`-side `StartSkill` (`FUN_3939a810`) resolves caster/target,
creates the local skill instance (`FUN_3939a440`; failure logs `start skill: fail to create skill`), sets the
unit's cast slot, plays the start/fire animation, applies a pending cooldown-tag estimate
(`FUN_39899df0`/`FUN_39899f30`), and sends the packet. The server answers with
**`SCSkillStartedPacket` (`0x0a1`)** whose `extra_flags` byte selects a result byte (`SkillResult`), a
`ushort` or a `uint`; field order is `id:u32, tl:u16, SkillCaster, SkillCastTarget, SkillObject,
real_cast_time_div10:u16, base_cast_time_div10:u16, cast_synergy:u8, extra_flags:u8` plus the optional
result (`aaemu-game/src/systems/skills/packets.rs:15-80`). A non-zero result is the refusal path; the
same numbers drive the client-side message builder `FUN_39390900` (see 2.3).

`SCSkillStarted` is handled by `FUN_391d57f0` (fires the script event `OnSkillStarted`, then
`FUN_3939a810` with the packet's caster/target/skill object and cast times). `SCSkillFiredPacket` (`0x0a2`,
`FUN_391d58a0` -> `FUN_3939b010`) carries `id, tl, caster, target, skill_object, computed_delay:i16,
channeling_time:i16, u8 0, fire_anim_id:u32, u8 flag`; the two i16 are stored as `value/10 + 10`.
`SCSkillEndedPacket` (`0x0a3`, just `tl:u16`; `FUN_391d5bf0`) and `SCSkillStoppedPacket` (`0x0a4`,
`obj_id:u32, skill_id:u32`; `FUN_391d5c70`) end the animation/instance. `SCCastingStoppedPacket` (`0x0a5`, `tl:u16,
duration:u32`; `FUN_391d5cf0`) and the cast-delayed packet (`tl:u16, delay:u32`; `FUN_391d5d70`,
`OnCastingDelayed`) update the bar.

**`X2Unit:UnitCastingInfo(unit)`** is `FUN_394bcf60`. It returns **four values**: skill name (desc
`+0x104`), a first time, a second time, and a boolean (`+0x13a`). It checks, in order, four per-unit
states: normal cast `+0x3ac4` (skill `+0x138` and start `+0x144`, value = `min(total, now-start)`),
channel `+0x3acc` (`+0x140` total, `+0x140-(now-+0x148)` remaining), cast-delay `+0x3ac8` (`+0x74` total,
now-`+0x70`), channel-delay `+0x3ad0` (`+0x84` total, `+0x80+0x84-now`). If none is live it pushes nil.
The Lua cast bar understands the pair as total/current: `components/castingbar.alb:32-78` fills while
`current` grows to `total` and empties while it shrinks (channel), and `player_casting_bar.alb:1-7`
registers `SPELLCAST_START`, `SPELLCAST_STOP`, `SPELLCAST_SUCCEEDED` for the `"player"` token; the target
and target-of-target frames create the same bar for `"target"`/`"targettarget"`
(`unitframe/target.alb:1-5`, `unitframe/target_to_target.alb:1-5`).

The bar is anchored bottom-centre at `(0,-150)` from `UIParent` (`player_casting_bar.alb:6-7`). The
component uses a 200 ms start animation and a 2000 ms end animation when the spell name is empty
(`castingbar.alb:13-30`). The target frame registers the same three events and forwards them to its
own bar (`target.alb:8-108`).

### 2.2 GCD and cooldowns

Each unit has one cooldown manager. Its layout is visible in `FUN_39899d00`/`FUN_39899d40`/`FUN_3989a290`:
global cooldown `remaining,total` at `+8/+0xc`; tag cooldowns `count` at `+0x10`, 3-dword entries at
`+0x14` (`tag id, remaining, total`); skill cooldowns `count` at `+0x71c`, entries at `+0x720`, **150
entries per list** (`FUN_39899f90` logs `skill cooldown is full` / `tag cooldown is full` at 150).

`SCCooldownsPacket` (`0x04a`) is two lists (`skills`, `tags`), each an `i32` count and
`id:u32, total_ms:u32, remaining_ms:u32` records (sent after a skill use/reconnect to report the
running timers); the client appends them to the manager
(`FUN_391d43c0` -> `FUN_39899d40`), so an empty packet is not a reset. `SCSkillCooldownResetPacket`
(`0x0ad`: `obj_id:u32, skill_id:u32, tag_id:u32, gcd:bool`) clears the skill and tag entries
(`FUN_39899ec0`/`FUN_39899f00`) and, when `gcd`, clears both global values (`FUN_39899d00`); handler
`FUN_391d6420`.

`X2Action:GetActionCooldown(slot)` returns one pair and `GetActionCooldownEx(slot)` returns **four**
values (`FUN_3943f310`, `FUN_3943f420` -> `FUN_393f61c0` + `FUN_3943ecb0`); the native forwards two
remaining/total pairs, i.e. the skill/tag cooldown and the global cooldown. The action-slot widget updates
from that object: `FUN_392c7360` fires `global_cooldown_start(value)`/`global_cooldown_done` and
`skill_cooldown_start`/`skill_cooldown_done` as event `0x25` on the slot widget, with the remaining/total
pair it reads through the cooldown object's vtable (`+0x18`). `SCSkillCooldownReset` is therefore the only
authoritative GCD cancel shown in the client.

Skill data has `skills.default_gcd` (flag), `skills.custom_gcd` (ms), `ignore_global_cooldown`,
`cooldown_time`, `cooldown_tag_id`, `use_weapon_cooldown_time`, and the attribute
`global_cooldown_mul` (`skills` loader `FUN_39731870`; attribute name table at `399e41ac`). The exact
formula that turns these into the client's global value was **not** found [I]; the Bevy rule
("custom_gcd, else 1 s when default_gcd") is not proven against the 1.2 code.

Every cast also sets a per-skill "last used" estimate at start (`FUN_3939a810` ->
`FUN_39899df0` for a positive cooldown, `FUN_39899f30` for the single tag), which the action bar can show
before the server's `SCCooldowns` arrives.

### 2.3 Range, LOS, facing and refusal texts

The client's refusal vocabulary is the enum in `FUN_39897990` (0..130): `SUCCESS, FAILURE,
SOURCE_DIED, SOURCE_ALIVE, TARGET_DIED, TARGET_DESTROYED, TARGET_ALIVE, ON_CASTING, COOLDOWN_TIME,
NO_TARGET, LACK_HEALTH, LACK_MANA, OBSTACLE, OUTOF_HEIGHT, TOO_CLOSE_RANGE, TOO_FAR_RANGE, OUTOF_ANGLE,
CANNOT_CAST_IN_COMBAT, CANNOT_CAST_WHILE_MOVING, CANNOT_CAST_IN_STUN, CANNOT_CAST_WHILE_WALKING,
CANNOT_CAST_IN_SWIMMING, BLANK_MINDED, SILENCE, CRIPPLED, CANNOT_CAST_IN_CHANNELING,
CANNOT_CAST_IN_PRISON, NEED_STEALTH, NEED_NOCOMBAT_TARGET, TARGET_IMMUNE, INVALID_SKILL,
INACTIVE_ABILITY, NOT_ENOUGH_ABILITY_LEVEL, INVALID_SOURCE, INVALID_TARGET, INVALID_LOCATION,
NEED_REAGENT, ITEM_LOCKED, NEED_MONEY, NEED_LABOR_POWER, SOURCE_IS_HANGING, SOURCE_IS_RIDING,
HIGHER_BUFF, NOT_PVP_AREA, NOT_NOW, NO_PERM, BAG_FULL, PROTECTED_FACTION, PROTECTED_LEVEL,
UNIT_REQS_OR_FAIL, SKILL_REQ_FAIL, ... URK_*` (64..130) and `URK_UNKNOWN`.

The text is looked up by **lower-cased name** in the `ui_texts` table: `FUN_393f4890` takes the enum
name, `Crc32Gen::GetCRC32Lowercase`, finds the row and shows it as a UI toast/chat line. The rows are
category 89 with the exact English text, e.g. `skill_obstacle` = "No line of sight.",
`skill_outof_height` = "Target is on a different elevation.", `skill_too_close_range` = "Target is too
close.", `skill_too_far_range` = "Target is too far.", `skill_outof_angle` = "Invalid target direction.",
`skill_cannot_cast_in_stun` = "Can't be used while stunned.", `skill_lack_mana` = "Insufficient
mana to use this.", `skill_need_reagent` = "Not enough $1." (localized_texts rows 72291-72325; the Korean
source rows are `ui_texts` 1729, 1730, 1731, 1732, 1733, 1728, 1751).

The same failure names are used by the script alert `SKILL_MSG(name, ...)` and by the action-bar "dark
slot" state. The relevant `skills` columns are `min_range`, `max_range`, `target_angle`, `front_angle`,
`target_offset_angle`, `target_offset_distance`, `check_obstacle`, `check_terrain`, `valid_height`,
`casting_tolerance`, `casting_cancelable`, `casting_delayable`, `casting_inc`, `stop_casting_by_turn`,
`stop_casting_on_big_hit`, `stop_channeling_on_big_hit` (`SELECT` in `FUN_39731870`). Examples from
`skills`: Bull Rush 10501 `min 0 / max 4 m / target_angle 180 / check_obstacle t`; Lasso 12039 `max 20 m,
channel 700 ms`; Mana Force 12759 `max 10 m`; Pin Down 13344 `max 4 m`; cooldowns 30 s, 24 s, 60 s, 21 s.

Which checks the 1.2 client itself makes before sending was not isolated [I]; `FUN_39390900` is the
error-message builder and `FUN_3939a810` logs `start skill:` only for structural failures. The server's
order is documented in `aaemu-game/src/systems/skills/validation.rs`: source alive/dead, swim, walking,
stun/sleep/knockdown, blank mind, target alive/dead, water, finite positions, target angle,
weapon/range, min/max range, height, obstacle (server file: 126-400).

### 2.4 Targeting

`SCTargetChangedPacket` (`0x084`: `obj_id:u32, target_obj_id:u32`) is handled by `FUN_391d5270`
(`OnTargetChanged`, script event `TARGET_CHANGED`, id 2). `CSChangeTargetPacket` is `0x02c`; the client
requests a change for a unit id and the server echoes it. `TARGET_TO_TARGET_CHANGED` (id 3) is a separate
script event.

Unit tokens are not just names: `FUN_3945fcd0` parses `player`, `playerpet`, `target`, `watchtarget`,
`team`, `loginch`, etc., and repeatedly consumes the suffix `target`, so **`targettarget` means the
target's target** (and `playertarget`, `targettargettarget` work). The target-of-target frame is
`unitframe/target_to_target.alb`: it creates `ttargetFrame`, anchors `TOPLEFT` to the target frame's
`TOPRIGHT`, and uses the `"targettarget"` token for name/HP/MP/buffs; `TARGET_TO_TARGET_CHANGED` and
`UNIT_COMBAT_STATE_CHANGED` refresh it (`target_to_target.alb:1-176`, `259-350`). `WATCH_TARGET_CHANGED`
(id 563) backs `X2Unit:SetWatchTarget`/`ReleaseWatchTarget`.

**Tab targeting** is native and configurable by seven cvars (defaults read from `x2game.dll` at
`0x3999edbc` etc.; registration `FUN_390f7b40`):

| cvar | default |
|---|---|
| `tab_targeting_round_dist` | 4.0 m |
| `tab_targeting_fan_dist` | 50.0 m |
| `tab_targeting_fan_angle` | 60 deg |
| `tab_targeting_z_limit` | 20.0 m |
| `tab_targeting_history_expire_time` | 3.0 s |
| `tab_targeting_history_max` | 10 targets |
| `tab_targeting_dir` | 0; exact meaning not read [I] |

The behaviour is "fan in front within 60 deg/50 m, plus a 4 m round catch, height limit 20 m, remember the
last 10 targets for 3 s so repeated Tab cycles instead of toggling the nearest" [I from the cvar names and
defaults; the selection loop itself was not located]. `mouse_clear_targeting` defaults to 0.

Click selection: a left click on a unit sets the target; a click on empty ground drops it when
`mouse_clear_targeting` is on. Escape first closes the topmost close-on-escape window, then
`ReleaseTarget` (`X2Unit:ReleaseTarget`). The exact engine pick/ray rule was not found [?].

### 2.5 Action bars

The client has **85 action slots, 0..84** (`FUN_3930bc30` scans `< 0x55`) and **4 pages** (0..3;
`FUN_393f6910` refuses `page >= 4`). Lua sees pages **1-based**: `X2Action:SetActionBarPage(page)` calls
`FUN_393f6910(page-1)`, returns `page`; `GetActionBarPage` reads the current 0-based page and pushes
`index+1` (`FUN_3943f8a0`, `FUN_3943f930`). A change fires `ACTION_BAR_PAGE_CHANGED` with the new 1-based
page (`FUN_393f6910` event `0x229` = `ACTION_BAR_PAGE_CHANGED`, id 553). Page buttons are `prevPageButton`/`nextPageButton` and
`ACTION_BAR_PAGE_CHANGED` (`new_actionbar_view.alb:236-274`, `new_actionbar.alb:1-84,127-251`);
`SetPageByItemCount`/`UseActionBarPageRotate` are the view helpers.

Action types are `0 None, 1 Item, 2 Spell, 3 Macro, 4 Item_Id` (pointer table at `0x39ac9974`, strings
`399dc2c0/399ac0a0/399d7f68/399dc2b8/399dc2b0`). `GetActionType(slot)` returns the type; `IsActionEmtpty`
is true for type 0; `IsEmptySlot` returns `type==0` (`FUN_3943ee70`, `FUN_3943ef40`, `FUN_3943f470`).

`SCActionSlotsPacket` (`0x12d`, sent when the character enters the world and after a slot save) is the
whole list, one record per slot: `kind:u8`, then `u32` for
Item/Spell/RidePetSpell, `u64` for Item_Id, nothing for None (`aaemu-game/src/models/char_data.rs:62-83`).
Handler `FUN_391d4440` loads the list and fires `ACTIONS_UPDATE` (event `0x5e`, id 94); Lua's `ACTION_BAR_...`/`ACTIONS_UPDATE`
refresh then runs. Dragging a skill/item onto a slot sends **`CSUpdateActionSlotPacket` (`0x0b6`)**, written
by `FUN_3930ba60` (type 2 = spell), `FUN_3930bad0` (type 5) and `FUN_3930bbd0` (type 0 = clear); the
client first validates the action at that index.

`X2Action:GetActionLastIndex` scans 0..84 and returns the highest non-empty slot (`FUN_3930bc30`). The
action-bar scripts read `GetActionInfo`, `GetActionInfoForActionBar`, `GetActionTooltipText`,
`GetActionCooldown/Ex`, `IsAutoRepeatAction`, `IsCurrentAction`, `GetComboActionInfo` and
`BaseActionBarEmptySlotCount` (`X2Action` group, ui-api.json). Slots register `LeftButton` drag and
`OnClick`; the tooltip is the `ISLOT_ACTION` tooltip (`new_actionbar.alb:116-251`). Slots dim by the
action's `can_use` result (the same SkillResult numbers).

### 2.6 Buffs and debuffs

A unit holds **32 buffs** (`+0x20` count, 0x68-byte rows at `+0x28`), **24 debuffs** (`+0xd28` count,
rows at `+0xd30`) and a third 24-row list at `+0x16f0`/`+0x16f8` (not exposed by `X2Unit`). Insertion is
append-only at the next free index (`FUN_39386e00`); there is no timer sort. `X2Unit:UnitBuff(unit, i)` /
`UnitDeBuff` use the **1-based** index (`FUN_39386d70(i-1)`), count via `FUN_39386dd0(1|2)`,
icon via `FUN_394c2fc0`/`FUN_394c31c0`. Getter bodies: `FUN_394c0230` (buff), `FUN_394c0360` (count),
`FUN_394c0410`/`FUN_394c0540` (debuff), `FUN_394c2fc0`/`FUN_394c31c0` (icons).

`SCBuffCreatedPacket` (`0x0b6`) fields: `SkillCaster, caster_id:u32, owner_obj_id:u32, index:u32,
buff_id:u32, caster_level:u8, ab_level:i16, skill_id:u32, PISC(charge, duration/10, 0, tick/10)`
(`aaemu-game/src/systems/skills/packets.rs:201-240`). `SCBuffRemovedPacket` (`0x0b7`) is `owner:u32,
index:u32`; `SCBuffUpdatedPacket` (`0x0b9`) is owner/index plus two values (duration/charge; handler
`FUN_391d5f20`). `SCBuffLearnedPacket` is `0x107`.

The buff window (`components/buffwindow.alb:106-164`) iterates `1..#button` and reads `UnitBuff` or
`UnitDeBuff` at the same index, so **screen order equals the unit's internal append order**, not remaining
time. Stack text is `x%d` only when the stack count is >1 (`buffwindow.alb:29-47`); the life-time bar
alpha fades over the last 800 ms (`buffwindow.alb:55-66`). Tooltip on hover is `ShowTooltip(buffTable)`
(`buffwindow.alb:16-20`). The unit frame puts the buff window at the frame's right and the debuff window
at its left (`unitframe_view.alb`).

### 2.7 Unit frames

`X2Unit:UnitHealth` returns `ceil(raw/100)` capped at max health; `UnitMaxHealth` is attribute 6;
`UnitMana` is `floor(raw/100)`; `UnitMaxMana` is attribute 7 (`FUN_394bc070`, `FUN_394bc180`,
`FUN_394bc340`, `FUN_394bc430`). HP/MP are therefore stored in hundredths.

The target frame chooses its bar texture from `GetCombatRelationshipStr` (hostile red / neutral orange /
friendly green; exact pixels in `docs/research/re-unit-colors.md`), and applies
`S_HP_PREEMTIVE_STRIKE` when `IsFirstHitByMeOrMyTeam("target")` is true (`unitframe.alb:59-112`,
`target.alb:62-248`). The player frame uses the same styles for `"player"`. HP and MP numbers are shown
when the account options `ShowHeatlthNumber`/`ShowMagicPointNumber` are on (`unitframe.alb:280-304`).

Combat state is a per-unit byte at `+0x3bba`, set/cleared by `FUN_390b36f0`/`FUN_390b3710` from
`SCCombatEngaged` (`0x085`, handler `FUN_391d69a0`) and `SCCombatCleared` (`0x086`, `FUN_391d6a60`);
`SCCombatFirstHit` (`0x089`, `FUN_391d6ae0`) marks the first hitter. `X2Unit:UnitCombatState(unit)`
returns that byte (`FUN_394bd830`), and `UNIT_COMBAT_STATE_CHANGED` (id 36) refreshes the frames. The target
frame sets its combat icon from `UnitCombatState("target")` (`target.alb:543`); the player frame does the
same for `"player"` (`player.alb`, `UNIT_COMBAT_STATE_CHANGED` handler).

### 2.8 Combat text

`SCUnitDamagedPacket` (`0x0a7`): `CastAction, SkillCaster, caster_id:u32, target_id:u32, crimeState:u8,
PISC(damage, absorbed, 0), PISC(0, 0, mana_burn), holdable_id:u8, u16 (288|hitType), flag:u8, result:u8`
(`skills/packets.rs:279-332`). `SCUnitHealedPacket` (`0x0a8`): `CastAction, SkillCaster, target_id:u32,
heal_type:u8, heal_hit_type:u8, value:i32, 0:i32, result:u8`. Handler `FUN_391d5fb0` passes the record to
`FUN_39398c80`, which applies HP/MP, plays the hit reaction and emits `COMBAT_MSG` (chat log) and
`COMBAT_TEXT` (floating number); environment damage goes through `FUN_391d60e0`.

`SCCombatTextPacket` (`0x0ab`) is `source:u32, target:u32, text_type:u8`; handler `FUN_391e90b0`:
`text_type 0` shows a system alert (event `0x19`), `text_type 1` shows a floating text. This is the
server-driven "damage indicator" path in addition to damage packets.

The scripts' parser is `combattext/combat_text.alb` (lines 71-628). It splits the localized
`spell_log_format`/`melee_log_format` text, maps hit types to `HIT/CRITICAL/MISS/DODGE/BLOCK/PARRY/IMMUNE/
RESIST/ABSORB/REFLECT`, and uses `COMBAT_TEXT_COLOR` for `DAMAGED_SWING`, `DAMAGED_SPELL`, `DOT`,
`DEBUFF`, `MISSED`, `HEAL`, `ENERGIZE_MP`, `DAMAGE_MP`, `COMBAT_START/END`, `GAIN_EXP/HONOR`. The
`damagedisplay` toolkit widget animates each number; `combat_text_animation.alb` selects
`CTA_DMGPOPUP/CTA_WEAPONATTACK/CTA_WEAPONFATALME/CTA_CHAIN.../CTA_DOT.../CTA_TEXTINFOBUFF/CTA_MESSAGEDOWN`.
Combat messages can be filtered in the chat combat page (`chat/combat_page_view.alb`).

### 2.9 Death and resurrection

`SCUnitDeathPacket` (`0x071`) is `obj_id:u32, kill_reason:u8, resurrect_wait_ms:i32, lost_exp:i32,
durability_loss_rate:u8, killer_obj_id:u32` plus, when a killer exists, `gameType:u8, killStreak:u16,
u8,u8,u8, killer_name:string` (`skills/packets.rs:373-405`). Handler `FUN_391d51c0`
(`OnUnitDeath`) marks the unit and fires `UNIT_DEAD` (id 37). The Lua death window
(`messageboximpl/death_and_resurrection_window.alb`) registers `UNIT_DEAD`, `PLAYER_RESURRECTION`,
`PLAYER_RESURRECTED`, `ENTERED_WORLD`, `LEFT_LOADING`.

`SCNotifyResurrectionPacket` (`0x041`) is `SkillCaster` only; handler `FUN_391d39f0` sets the local
"resurrection offered" flag at character `+0xafc` and fires **`PLAYER_RESURRECTION` (id 92)** with the
caster name. The window then switches to resurrection mode (`death_...alb:250-320`) and offers
`btnResurrection` (in place) and `btnGraveyard` (temple). Both call `X2Player:Resurrect(inPlace)`
(`FUN_394952b0` -> `FUN_39186e70`), which sends **`CSResurrectCharacterPacket` (`0x04e`, `in_place:bool`;
`aaemu-game/src/systems/skills/resurrect.rs:29-48`)**. The server answers
**`SCCharacterResurrectedPacket` (`0x042`)**: `unit_id:u32, x:y:z f32, z_rot:f32`
(`packets_special.rs:51-71`); handler `FUN_391d3ad0` moves the unit and fires `PLAYER_RESURRECTED` (id 93).

`X2Player:GetResurrectionCountdown()` (`FUN_39495c00`) returns **three** values: the notify flag
(character `+0xafc`), a remaining time against a fixed **300 000 ms (5 min)** deadline, and a remaining
time against the death packet's `resurrect_wait_ms` (stored at `+0xaf8`, deadline `+0xaf4`). Both are
milliseconds and are pushed as floats. The death window uses the three values to refresh its text, enable
its graveyard/in-place buttons when the wait has elapsed, and close itself at the 5-minute limit, every
frame (`0.6.4`, `0.7.4`, lines 230-246 / 301-318). Penalty text: lost EXP (none below level 10),
durability loss ratio, and the `notDecreaseExp`/`expRecoverGuide` lines (`0.4`, lines 139-168). Battlefield, siege and instance deaths use
their own title/message keys (`0.3`, lines 113-137).

The waiting table is `resurrection_waiting_times`: `penalty_duration` 600 s for all rows, `waiting_time`
0/5/45/90/180 s by row, `siege_waiting_time` 20/15/10/5/0 s (rows 1-5 and 10).

## 3. Numeric parameters

| Item | Value | Source |
|---|---|---|
| action slots | 85 (0..84) | `FUN_3930bc30` |
| action pages | 4 (Lua 1..4) | `FUN_393f6910` |
| action types | 0 None, 1 Item, 2 Spell, 3 Macro, 4 Item_Id | `0x39ac9974` table |
| buff rows / debuff rows | 32 / 24 (+24 hidden) | `FUN_39386e00` |
| cooldown lists | 150 each | `FUN_39899f90` |
| GCD cancel | `SCSkillCooldownReset.gcd` | `FUN_391d6420` |
| res timer | 300 000 ms | `FUN_39495c00` |
| death window res timer | server `resurrect_wait_ms` | `SCUnitDeath` |
| Tab round / fan / angle / z | 4 m / 50 m / 60 deg / 20 m | `FUN_390f7b40` |
| Tab history | 10 / 3 s | `FUN_390f7b40` |
| cast bar offset | (0, -150) from UIParent BOTTOM | `player_casting_bar.alb:6-7` |
| cast bar animations | 200 ms start, 2000 ms empty-name end | `castingbar.alb:13-30` |
| buff stack label | only >1 | `buffwindow.alb:29-47` |
| buff bar fade | last 800 ms | `buffwindow.alb:55-66` |

## 4. What is inferred

* `[R]` The failure enum, the four `UnitCastingInfo` states, the 32/24 buff arrays, the 85/4 action-bar
  limits, the cooldown-manager layout and the 5-minute resurrection deadline are read in code.
* `[I]` Tab target selection is a cone (60 deg) within 50 m plus a 4 m round catch, with a 10-entry/3 s
  history; only the cvar defaults are read, the selection loop was not located.
* `[I]` GCD length comes from `default_gcd`/`custom_gcd` and `global_cooldown_mul`; the producer that
  writes manager `+8/+0xc` was not found.
* `[I]` Click-target picking and the `mouse_clear_targeting` rule are engine-side; only the cvars and the
  Lua click handlers are read.
* `[?]` Exact `CSStartSkill`/`CSUpdateActionSlot` field widths (read from server writers, not the client
  reader); `SCBuffUpdated` payload detail; `tab_targeting_dir` semantics.

## 5. Gaps vs Bevy (ranked)

1. **Action-bar pages - done in `88945c4`.** Original has 4 pages, a 1-based Lua page API and
   `ACTION_BAR_PAGE_CHANGED` (`FUN_393f6910`, `FUN_3943f8a0`). `aa-net::ActionBar` now carries the
   0-based page, maps the main 12 slots per page (0, 48, 60, 72; `FUN_392c5a50`) and the sub bars
   stay at 12..23; the real `GetActionBarPage`/`SetActionBarPage`, Shift+Up/Down page keys, the event
   and the page-aware `CSUpdateActionSlot` are wired in `aa-client`. Original gap text: Bevy's
   `aa-ui/src/lua/stubs.rs:367` (`GetActionBarPage`) and `:1554` (`SetActionBarPage`) were nil stubs
   and `aa-net::ActionBar` had only the flat 85 slots (`aa-net/src/items.rs:121`). Fix: expose page
   state, map page N to slot offset, fire the event, and send the same `CSUpdateActionSlot` when a
   page's slot changes.
2. **Target combat state is hardcoded off.** Original `target.alb:543` calls
   `UnitCombatState("target")`; Bevy writes `in_combat:false` for every target
   (`aa-client/src/hud/panels.rs:488`). Fix: carry `SCCombatEngaged/Cleared` per unit and return it.
3. **`SCCombatText` (0x0ab) is dropped.** Original `FUN_391e90b0` shows `text_type 0` as a system alert
   and `text_type 1` as a floating text. `aa-net` decodes it (`session.rs:681,1997`) but no aa-client
   system reads `Event::CombatText`. Fix: route it to `SYSMSG`/`COMBAT_TEXT` like damage.
4. **Resurrection countdown loses the flag and the server wait.** Original `FUN_39495c00` returns
   `(flag, 300000-remaining, waitMs-remaining)`, all in ms. Bevy `hud/api.rs:581` returns one 5-minute
   value in seconds and ignores the death packet's `resurrect_wait_ms` and the notify flag; the death
   window's text and button gates (`0.7.4`, lines 301-318) cannot work. Fix: return the three values
   from the death packet and the 5-minute deadline.
5. **`PLAYER_RESURRECTION` is never fired.** Original `FUN_391d39f0` fires it (id 92), which switches
   the death window to resurrection mode (`death_...alb:250-320`). Bevy only fires `UNIT_DEAD` and
   `PLAYER_RESURRECTED` (`hud/alerts.rs:222-228`); `revive_caster` is tracked and exposed as `can_resurrect_here`, but the event is not fired. Fix: fire
   `PLAYER_RESURRECTION` with the caster name when `ResurrectionOffered` arrives.
6. **Tab targeting does not match the original.** Bevy `hud/targeting.rs:13` uses a 40 m nearest-unit
   list sorted by distance, no 60 deg fan, no 50 m fan distance, no 20 m z limit, no 4 m round catch and no
   3 s/10-entry history. Fix: implement the seven `tab_targeting_*` defaults above (4/50/60/20/3/10).
7. **No target-of-target unit token.** Original `FUN_3945fcd0` walks the `target` suffix, and
   `target_to_target.alb` uses `"targettarget"` for name/HP/MP/buffs/cast bar. Bevy's cast info only
   knows `"player"`/`"target"` (`hud/cast.rs:57-61`), and no `targettarget` snapshot exists. Fix: resolve the
   token chain and feed the `ttarget` frame.
8. **GCD length ignores the attribute multiplier and the slot query.** Bevy does read `custom_gcd`/
   `default_gcd` (`hud/data.rs:558`) and starts the GCD at `SkillStarted` (`aa-net/src/state/skills.rs:314`),
   but never applies `global_cooldown_mul` (the attribute exists only in `hud/tipcalc.rs:1121`) and exposes
   one global pair, not the per-slot four values of `GetActionCooldownEx` (`FUN_3943f420` ->
   `FUN_393f61c0`). Original action slots read that object and start/stop the mask from it
   (`FUN_392c7360`). Fix: apply the attribute and return the skill/tag plus global pairs per slot.
9. **No first-hit/preemptive-strike state.** Original `X2Unit:IsFirstHitByMeOrMyTeam` reads the unit's
   first-hit fields (`FUN_394bd9f0`, set from `SCCombatFirstHit` `0x089` by `FUN_391d6ae0`), and the
   target frame applies `S_HP_PREEMTIVE_STRIKE` / `L_HP_PREEMTIVE_STRIKE` (`unitframe.alb:104-112`).
   Bevy has no such state or binding: `hud/panels.rs:353` only carries `combat_relation`. Fix: track the
   packet per target and expose the predicate/bar row.

Lower-risk differences: the buff window's 800 ms fade and stack `xN` formatting are in the shipped Lua and
should already run; the target frame's combat icon texture and `UNIT_COMBAT_STATE_CHANGED` registration are
in the Lua, so only the data feed (gap 2) is missing.

## 6. Open questions

1. What writes the cooldown manager's global pair (`+8/+0xc`) on a cast, and is the value
   `custom_gcd`/`default_gcd` scaled by `global_cooldown_mul`?
2. Exact Tab selection loop (fan vs history ordering, `tab_targeting_dir`).
3. Exact click pick rule and `mouse_clear_targeting` drop semantics.
4. `CSStartSkill`/`CSUpdateActionSlot` exact field widths and whether the client sends a page with the slot.
5. `SCBuffUpdated` payload (charge vs duration order) and the third buff list's purpose/visibility.

## 7. Live oracle measurements

None in this job. The claims above are code/data reads; the cast-bar and death-window timings should be
checked with a retail capture before the Bevy fixes are marked verified.
