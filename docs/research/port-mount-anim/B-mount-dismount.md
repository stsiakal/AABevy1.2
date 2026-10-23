# B. Mount and dismount

Scope: the interaction prompt, eligibility, the `MountHorse Controller` / `UnMountHorseController`,
approach/auto-walk, `CSMountMate`/`CSUnMountMate`, attach/detach, the seat bone, the side get-on /
get-off clips, `CSCreateSkillController`, and the camera during the transition. x2game VAs at
base `0x38ff0000`; sizes from `funcs.csv`; constants from
`D:/aa-re/client12/x2game_unpacked_data.dll` (VA - base).

The mount example is the same Brown Lilyut Horse (npc 5431, model 450, actor model 206,
`mount_pose_id 1`). All mate poses use the same controller; `FUN_39571f10` maps the actor model's
`mount_pose_id` to the graph key: 0 `none`, 1 `horse`, 2 `lion`, 3 `bear`, 4 `wildboar`,
5 `submarine`, 6 `smallboat`, 7 `coupleduckship`, 8 `speedcar`, 9 `elephant`, 10 `pangolin`
(other -> `invalid mount_pose`).

---

## 1. The interaction prompt (F key) — evidence

**F binding.** `game/default_binding.g:29` (extracted
`.../pak/game/default_binding.g`): `f do_interaction_1`; `g do_interaction_2`,
`h do_interaction_3`, `j do_interaction_4` (lines 29-33). There is no default mount key in this
file.

**Widget.** The prompt is the **dynamic action bar**:

- `x2ui/dynamic_action_bar.alb` (strings: `CreateDynamicActionBar`, `MAX_DYNAMIC_ACTION_COUNT`,
  `DYNAMIC_ACTION_BAR_SHOW/HIDE`, `do_interaction_`, `keyBindingLabel`, `interactionTargetId`,
  `X2Interaction`, `GetInteractionSkillIconPath`, `%s%s`, `_o.dds`, `_c.dds`, `_d.dds`,
  `IsValidIconPath`, `ACTION_BUTTON_WIDTH/HEIGHT/GAP`).
- `x2ui/dynamic_action_bar_view.alb` (strings `CreateEmptyWindow`, `SetUILayer("DEFAULT")`,
  `ConvertAbbreviatedBindingText`, `X2Hotkey:GetBinding`, `CreateLabel`, `BOTTOM`,
  `ALIGN_CENTER`, `EXP_ORANGE`, `CreateIconImageDrawable`).
- The bar's key label is `ConvertAbbreviatedBindingText(X2Hotkey:GetBinding("do_interaction_"..
  index))` (`dynamic_action_bar.alb` disassembly strings); F/G/H/J for the four buttons.
- The icon is `X2Interaction:GetInteractionSkillIconPath(skill)`: callback `FUN_3947e130`
  (186 B) resolves the skill (`FUN_396f6390`), builds the icon path with `FUN_394e6c10`
  (125 B), which prefixes the string `"Game\\ui\\icon\\"` and appends the skill icon asset
  (`FUN_396f6290`); the script then derives `_o`, `_c`, `_d` variants. So the art path is
  `Game\ui\icon\<skill icon>.dds` (for example the interaction icons are
  `interaction/icon_interactionNN.dds`, `docs/research/re-placement.md` §5).
- Native data path: `X2::GameClient::InteractionIndicator::MakeCandidate FUN_393bdad0` (799 B)
  ray-picks the unit/doodad under the cursor and calls `FUN_393bd850` (336 B, unit candidate) or
  `FUN_393bd050`; the unit's skill list is `FUN_393bcb90` (134 B). The bar show/hide is
  `X2::GameClient::DynamicActionBar::Show FUN_393b1eb0` -> event `0x19d`/`0x19e`
  (`DYNAMIC_ACTION_BAR_SHOW/HIDE`; event ids from the table in `FUN_39214380`, indices 413/414).

**Own mount / mode-bar correction.** The only native bar entry that is specifically a mount
command is `command_unmount`, but it belongs to the **mode action bar**, not the F dynamic bar:
`FUN_392045f0` (263 B) resets 15 mode entries (stride 0x54 at `+0x14c`, count `+0x13b0`), adds
the entry only when the local player's mount id (`player+0x3af0`) resolves to a unit of kind 5
for which `FUN_3933b1b0` is **false**, fires `MODE_ACTIONS_UPDATE` `0x5f` and calls
`FUN_39204320(&callback, "ui/icon/icon_skill_vehicle03.dds", 0x13, "command_unmount")`.
`FUN_3933b1b0` (60 B) returns 1 exactly when the unit is kind 5 and its owner id
(`unit+0x420`) equals the local player's id, i.e. it **suppresses** the entry for one's **own**
mate. The mode bar is `hud/mode_action_bar/mode_action.alb`, whose buttons use the
`mode_action_bar_button` bindings (R/T/Y/U by `default_binding.g`), so `command_unmount` is not
an F prompt. The owner capture `re-mounts.md` §10 confirms no bar while riding the own mount.
(The `re-mounts.md` §2 phrase "dynamic action bar (the F-key bar, FUN_392045f0)" is imprecise:
event id 0x5f and the `mode_action.alb` consumer identify it as the mode bar.)

**Pet-bar mount button.** The actual mount command is the pet bar's second command button
(`PET_CMD_MOUNT`): `pet_view.alb` builds the command buttons with the icon table
`PET_CMD_EQUIP -> ui/icon/pet_command/pet_equip.dds`, `PET_CMD_MOUNT ->
ui/icon/pet_command/pet_mount.dds`, `PET_CMD_PASSENGER_GET_OFF ->
.../pet_passenger_get_off.dds`, `PET_CMD_CYA -> .../pet_cya.dds` (extracted
`pet_view.alb` disassembly, lines 155-158). Buttons are 52x52 (same file, 159/52/52 table).
`pet.alb` sets each button's key label from
`ConvertAbbreviatedBindingText(X2Hotkey:GetBindingButton("PetCommandButton"..i, 1))`; there is
**no default binding** for `PetCommandButton2` in `default_binding.g` (the binding API in
`crates/aa-client/src/settings/bindings.rs` shows it is a user binding).

**Answer.** F near an interactable own mount shows no native mount prompt. The F-key UI is the
dynamic action bar; it has no own-mate entry. The mode-bar `command_unmount` is for riding a
mount that is not one's own mate and uses the R/T/Y/U mode bindings. Mounting is the pet-bar
`PetCommandButton2` with `ui/icon/pet_command/pet_mount.dds` (no default key), or Lua
`X2Mate:MountMate()`. If a client build shows an F prompt for mounting, it is not in the 1.2
native decompile; the Bevy reach in `re-mounts.md` §10 is a design addition, not a port.

---

## 2. Entry points and eligibility

| function | size | role |
|---|---:|---|
| `FUN_390697f0` | 105 | Lua `X2Mate:MountMate()`: reads the script `"target"` argument (`FUN_3945fcd0`); if it is a unit id, tries `FUN_393642f0(target)`; otherwise (or on failure) uses the own mate unit (`FUN_3933f5e0()->+0x20`) |
| `FUN_3906a440` | 284 | Lua `CanMount()`: selected unit first, then own mate; calls `FUN_3935aa10` with the same gates and `FUN_39058bd0` (passenger-seat test) |
| `FUN_393642f0` | 267 | starts the controller: resolve unit (`FUN_3935f560`), `FUN_3935aa10`, cooldown write `_DAT_3a169944 = now`, `FUN_39362c70`, choose attach point `uVar3 = 1` if `unit+0x420 == localPlayerId` else `2`, `FUN_390ca4d0(unitId, point)`, `FUN_39429e40` |
| `FUN_3935aa10` | 556 | eligibility result struct `{byte ok, u16 error, u32 ?}`: unit kind 5, not attached (`+0x3c26`), local player not mounted, no stun-like flags (`player+0x3c26/+0x3a73/+0xe9c/+0x3a71`), state sentinels (`+0x3ac4..`), distance squared <= 625 (`_DAT_399cff68`), 2000 ms cooldown (`_DAT_3a169944`), casting/pending gate `FUN_39387690(DAT_39ac7448=1579)`, flags `FUN_3933aa20/aa40`; error detail 0x205 = too far |
| `FUN_390697b0` | 27 | Lua `UnMountMate()`: `FUN_39361e80(9)` |
| `FUN_39361e80` | 314 | `CSUnMountMate` 0xa8 `{unit+0x418, player+0x3af4, reason}`; refuses if buffs `0x2b05`/`0x2b15` are present (`FUN_396f6390`/`FUN_3989a3e0`) or within 2000 ms; else sends and updates the cooldown |
| `FUN_39362070` | 254 | `DismissPet`: if mounted on the same mate -> `FUN_39361e80(10)`; else start `UnMountHorseController(unit, 10, 0)`, set `Mate+0xbc`, fire `DISMISS_PET` 0x111 (A) |

`FUN_3935aa10` returns `ok` in byte 0 and a detail/error word in bytes 2-3; the caller
`FUN_393642f0` shows the error via `FUN_39390900(code, unit, DAT_3a169964, 0)`. Error strings
0x69..0x6f (hire, corpse, no space, repair, dead, resurrected) are the ones listed in
`re-mounts.md` §2.

---

## 3. Mount controller

`FUN_390ca4d0` (132 B) allocates 0x38 bytes, calls `FUN_390a5f00(this, mateId, point)` and stores
the pointer at the actor's `+0x58`; `FUN_390ca140` (151 B) then notifies the controller listeners
and fires the `CSCreateSkillController` "started" callback (see §5).

`FUN_390a5f00` (388 B, `this=actor`, args `(mateId, point)`):

- writes vtable `PTR_FUN_399a30b8`, base vtable `PTR_FUN_399a28e8`;
- `+0x04` start time, `+0x08`/`+0x10` zero, `+0x14` actor (the `param_2` passed by
  `FUN_390ca4d0`), `+0x18` mate id, `+0x1c` u16 state/finished byte, `+0x2c` point (1/2),
  `+0x30` sent byte, `+0x34` expiry = now + 10.0 (`DAT_3999cce8`);
- requires the mate unit (`FUN_39360020`) and the local unit (`FUN_390b3750`, unit kind 1);
- sets the controller name `"MountHorse Controller"` (`vt[0x24](0, name)`), calls
  `FUN_390b3850`/`vt[0x228]` (transform), `FUN_3909ac60` (LOS raycast mate<->player, 206 B);
  on blocked LOS fires system message `0x6b` (`mate_cannot_mount_blocked`); if `point == 1`
  checks `FUN_3909ad30` (occupancy/approach); on success `FUN_390a2c90(mate)` stores the
  approach point in controller `+0x20..+0x28`;
- on any failure sets `+0x1c = 1` (finished).

Vtable `0x399a30b8` (16 slots dumped from `.data`; each small method was disassembled):

| slot | address | meaning |
|---:|---|---|
| +0x00 | `390a60e0` | destructor |
| +0x04 | `390aaf30` | **Update** (186 B) |
| +0x08 | `390a60a0` | `GetType()` = **8** (CSCreateSkillController type) |
| +0x0c | `390a60b0` | returns 0 |
| +0x10 | `390a6090` | `IsFinished()` = byte `+0x1c` |
| +0x14 | `390943b0` | generic 0 |
| +0x18 | `390a60c0` | returns 0 |
| +0x20 | `390a60d0` | returns 1 |
| +0x1c.. | generic 0 |

### Update `FUN_390aaf30`

```
if mate != 0 and all input axes 1..5 (FUN_39424580) <= 0:
    unit = lookup(mateId)
    if unit:
        FUN_390aa730(unit)      # auto-walk / approach
        FUN_3909b070(unit)      # maybe send CSMountMate; sets sent/expiry
        if now <= expiry: return
finished = 1
else:
    finished = 1
```

Any movement input cancels immediately (the graph input axes 1..5 are the movement actions). The
10 s expiry is the second cancel.

### Approach point `FUN_390a2c90`

Called with the mate unit. It calls `FUN_3909ae90(mate)` (entry side), obtains the mate's
orientation (`FUN_390b3870` -> forward/right basis) and builds the side vector:
`(right * 2.0 - world_up)` normalized; for side 2/3/4 it negates the vector or flips components;
the result is stored at controller `+0x20/+0x24/+0x28`. Constants: `DAT_3999cffc = 2.0`,
`_DAT_3999c488 = 1.0`, `_DAT_3999cc18 = FLT_MIN`, `DAT_3999c478 = 1.5` (Newton rsqrt),
`DAT_3999c47c = 0.5`, `_DAT_3999cffc` etc. [R for the constants and code; the exact
"side vs back vs front" basis signs are [I] because the decompiler drops the quaternion helper's
internals]

### Auto-walk `FUN_390aa730` (2,037 B)

Called each frame. It:

1. returns if controller `+0x1d` (arrived) is set;
2. gets the local actor's animation graph/controller (`vt[0x208]`), aborts if the player is on a
   ladder/climb (`+0x3a70/3a71/3a72`);
3. `FUN_390a8dc0` (movement request reset), reads the current transform (`vt[0x228]`);
4. builds the target from the controller's `+0x20..0x28` stored approach point;
5. horizontal distance squared to target; `DAT_3999c480 = FLT_EPS`, `0.81 = DAT_3999cc20`,
   `1.0/0.0` constants;
6. if `dist2 <= 0.64` (0.8 m) and vertical target `-1.5..2.5` (`0x399a3238/323c`) -> arrived:
   set `+0x1d = 1`; stop the move request;
7. else if `dist2 <= 2.25` (1.5 m, `0x399a3244`) apply the 0.25 speed scale (`0x399a26e4`);
8. `FUN_39097bd0`/`FUN_390983f0` normalize the direction, `FUN_39098380` sets the move-to target
   and `FUN_390a5d00` submits the movement request; request flag bit 0x800 and bit 8 are set,
   plus 0x80000000 if axis 6 (autorun?) is held;
9. `vt[0x*]` on the controlled unit applies the request.

The thresholds are exactly the ones in `re-constants.md` (`0.64`, `2.25`, `-1.5`, `2.5`).

### Send `FUN_3909b070`

```
if !sent && arrived && mate+0x3c26 == 0 && mate->kind == 5:
    if distance2(mate, player) < 9.0 (0x39ac3878):
        if local player mounted-id == sentinel DAT_39ae4eb8:
            reason = FUN_3909ae90(mate)
            packet 0xa7 {tl=mate+0x418, point=controller+0x2c, reason}
    sent = 1
    expiry = now + 2.0 (0x3999cffc)
```

The server then sends `SCUnitAttached` (or the controller ends after 2 s).

### Reason / entry side `FUN_3909ae90` (478 B)

- `d = normalize(matePos - playerPos)` (3D);
- `f = mate forward` from the mate quaternion (`FUN_390b3870`);
- `dot = d · f`;
- `dot >= 0.96` (`0x399a2d28`) -> **3 (back)**, with the seat-1-occupied adjustment;
- `dot < -0.96` (`0x399a2d24`) -> **4 (front)**, same adjustment;
- otherwise `cross = d.y*f.x - d.x*f.y`; `cross >= 0` -> **2 (right)** else **1 (left)**;
- occupied seats are queried by `FUN_390c9f40(1)` and `(2)` (42 B: scans the linked list at the
  mount `+0x120`, returns `entry+8` for a matching seat key). The code forces 1 when the
  seat-2 query is nonzero for the front/back case and when the seat-1 query is nonzero for the
  side band; the polarity ("occupied" vs "free") is [I].

The result is the `CSMountMate` `reason` byte and later the rider's `VehicleSeat` value (2/3/4
stay, else 1).

---

## 4. Attach and seat binding

Packet handler `OnUnitAttached` = `FUN_391d49d0` (155 B): logs
`client OnUnitAttached. parent=%u, child=%u`, looks up the child (`packet+0xc`), then
`FUN_39362770(packet+0x10, packet+0x18)`.

`FUN_39362770` (300 B):

- resolve the parent id; if it is not in the unit map, queue a retry
  `FUN_3900c5f0(..., FUN_39364d40, 0)`; `FUN_39364d40` retries with reason 8;
- `FUN_390b3f10(child, parentActor, reason)` (Unit::AttachTo, 225 B):
  writes `child+0x3af0 = parentId`, `child+0x3af4 = seat`; if a parent actor is present,
  calls `FUN_390e16b0(parent, seat, reason)` then the child's `vt[0x70]`;
- `FUN_390e16b0` (94 B): if `parent+0x11c == 0` (not already attached), `FUN_390dea00`,
  stores `parent+0x11c = child`, `parent+0x12c = seat`, calls `vt[0x278](seat)` and
  `parent.vt[0x288](child, seat, reason)`.
- the parent's `vt[0x288]` is `On Attaching FUN_390eb520` (239 B):
  `FUN_390e96a0(parent, reason)` (4,268 B) binds the child actor to the parent skeleton and the
  attach point; then `FUN_39111970(2)`, model pose lookup `FUN_39695a70`, and
  `child.vt[0x140](poseOr1, seat)` sets the rider graph.
- `FUN_390e96a0` is the large binding function: it resolves the child's model type
  (`child+0x4b`), the model desc (`FUN_3963b490`), the child animation set, allocates a 0x44
  attachment helper (`FUN_390c31b0`), builds the local matrices from the parent skeleton bone
  (`vt[0xc8]/vt[0x0]` on the model instance), and calls the engine attachment
  (`FUN_390bea90`, `FUN_390bb7c0`, `FUN_390c31b0`, `FUN_390c5660`, `FUN_390e5730`). The bone
  names are data: `model_bindings` for model 450 attach points 1/2 ->
  `model_attach_point_strings` 1 `bone_spine_driver` (`$driver`), 2 `bone_spine_passenger`
  (`$passenger0`), 3 `bone_spine_passenger1`; `horse_brown_spot.cdf` declares both as
  `CA_BONE` attachments (`re-mounts.md` §2). `horse_rein` is false for all rows.
- `FUN_39153b60` (150 B) is the graph-input side:
  - `pose < 11`: `vt[0x04](this+0x1a, FUN_39571f10(pose))` (Vehicle key);
  - `seat 1..4`: `vt[0x08](this+0x1c, seat, 0)` (VehicleSeat int);
  - any other seat: reset Vehicle to `FUN_39571f10(0) = "none"`, seat = 0.
- `FUN_390eb520` then stops the rider's own controller (reason != 8), calls child `vt[0x44]`,
  and if the child is a unit names it `"On Attaching"`.

`On Detaching FUN_390cea80` (442 B):

- for each of the two attachments in `this+0x83..`, if it matches the child id, calls
  `FUN_390c0ef0(index)` (clear);
- if the child model is type 1, clears `+0x24c/+0x250` and finds the stirrup bones
  `"bone_spine_stirrups_l_01"` / `"bone_spine_stirrups_r_01"` on the parent and removes them
  (`vt[0x60]`);
- resolves the parent model/attachment, `FUN_390bb7c0`, `FUN_39111970(0)`, child `vt[0x44]`,
  starts the unmount controller via `FUN_390ca560(this+0x10, reason, 1)`, and names the child
  `"On Attaching"` for unit kind 1 (the naming is shared with attach).
- the per-frame stirrup IK is `FUN_390cd890` (4,590 B), called from
  `X2::ActorUnitModel::Update FUN_390dd080`; it snaps the mount's stirrup bones to the rider's
  `Bip01 L/R Toe0` after the `BindHorseStirrup` event. Only saddle/stirrup models matter.

`SCUnitDetached` 0x? -> `FUN_391d4a70` (145 B) -> `FUN_39355bd0` (36 B) ->
`FUN_390b4000` (`Unit::DetachFrom`, 102 B): logs `Unit::DetachFrom: id=.. cur=..`, clears
`+0x3af0/+0x3af4` to the sentinel, calls `FUN_390dca30(reason)` and `child.vt[0x68]`.

---

## 5. CSCreateSkillController and the controller lifetime

The controller store `FUN_390ca140` walks the actor's controller listeners. The two listener
callbacks are:

`FUN_39356820` (187 B, controller started): only for the local player; builds an opcode `0x8b`
packet with:
- `local_8 = controller->vt[0x08]()` = controller type (mount 8, unmount 9),
- `local_c = localPlayer+8` (object id),
- `local_7 = controller->vt[0x24]()` = fall-damage-immune byte.
`FUN_393568e0` (172 B, controller ended): opcode `0x8b`, type 0, object id local player.

The packet layout is the one `aa-net` uses for `CSCreateSkillController`:
`{bc obj_id, u8 type, bool fall_damage_immune}`; the mount/unmount types 8/9 match
`re-mounts.md` §9. The vtable methods +8 (type) and +0x24 (immune) are the small bytes dumped in
§3 (mount +8 = 8, unmount +8 = 9; both +0x24 return 0 for the two controllers).

Lifetime summary:

| event | controller | type | duration | send |
|---|---|---:|---:|---|
| `MountMate`/pet 2 | `MountHorse Controller` `FUN_390a5f00` | 8 | 10 s, but 2 s after the send | `CSCreateSkillController(8)` at start; `CSMountMate` 0xa7 within 3 m; `CSCreateSkillController(0)` when it ends |
| `UnMountMate(9)` | `UnMountHorseController` `FUN_390a6110` | 9 | 500 ms | `CSUnMountMate` 0xa8 sent first; `Create(9)` at start; `Create(0)` after 500 ms |
| dismiss 10 / other | same unmount controller | 9 | 1000 ms (reason != 9) | `Create(9)`, `Create(0)` after 1 s, then `CSRemoveMate` (A) |

`CSMountMate`/`CSUnMountMate` opcodes/fields: 0xa7 `{u16 tl, u8 point, u8 reason}` and 0xa8
`{u16 tl, u8 point, u8 reason}`. The `point` in `CSMountMate` is `controller+0x2c` (1 driver,
2 passenger), while `reason` is the 1..4 entry side from `FUN_3909ae90`. (The server calls the
field "attach point" and "reason"; see `aa-net`.)

---

## 6. Dismount placement and clips

`UnMountHorseController` `FUN_390a6110` (322 B):

- vtable `PTR_FUN_399a3108`, type 9, `+0x14` actor, `+0x18` mate id, `+0x20` start time,
  `+0x24` reason, `+0x28`/`+0x29` flags, `+0x1c` finished.
- On construction: resolve the mate, `FUN_39096160` (clear state), local actor; call
  `vt[0x44]` (stop existing controller), name `"UnMountHorseController"`; if the
  `param_5` flag is set, `FUN_390a2e90(actor, reason)`; for reason 10 also calls
  `vt[0x1c4](0, childId, seatByte, localPlayerId)` (the detach/seat clear).
- `FUN_390aaff0` (Update, 495 B): duration = 1000 ms, or 500 ms when reason != 10 and == 9;
  if elapsed > duration -> finished. After 100 ms sets `+0x28` and calls the rider actor's
  `vt[0x20](1)` (the actor/controller hand-off) then `FUN_390bb400`; for reason 10 it additionally resolves the mount,
  copies a pose/skeleton (`vt[0xb0]`), applies a movement/physics request and sets the rider's
  animation time (`vt[0xfc]`).
- Placement `FUN_390a2e90` (554 B): gets both transforms, `FUN_3933b270` bounds/height,
  `FUN_3933be30` overlap test; if blocked it keeps the player at the mount; else it moves the
  player to the mate position plus the height difference (`local_c - local_8`); then:
  - reasons 3/8/0xc -> set rider animation state 5;
  - default -> state 6;
  - reasons 9/10/0xb/0xd -> `FUN_3909b4a0` and return.
- `FUN_3909b1a0` (764 B, side spot): builds a left/right vector from the mate basis, tries
  `+1.5 m` left (side `0x39ac3888 = 1.5`, height `0x39ac388c = 1.0`), then the other side,
  both collision-checked (`FUN_3933be30`); returns 1 and a world position, or 0.
- `FUN_3909b4a0` (380 B): sets rider animation state 6, calls `FUN_3909b1a0`; if no free side,
  uses the saddle transform; applies the movement request (`vt[0x2ac]`), a collide/ground
  settle through the character (`vt[0x208]->vt[0x108]->vt[0x28]`), sets controller `+0x29 = 1`
  (which triggers the 100 ms action in Update). The fallback constants are
  `0x39ac3890 = 4.0` and `0x39ac3888/8c = 1.5/1.0`.

Clip choice:

- **Get-on**: attach reason 1..4 -> `VehicleSeat` 1..4 -> `Horse Start` -> one of
  `Horse_Enter_l/r/b/f`. The choice is in the state's `vehicle_transition` template:
  `seat="1"` (l), `seat="2"` (r), `seat="3"` (b), `seat="4"` (f), matched against
  `VehicleSeat`; the clips are `fist_ac_horse_{l,r,b,f}_geton`, 1.00 s each
  (`player_full_body.xml:1509-1521`; `re-mounts-full.md` §4). Seat 2/3/4 stay, everything else
  becomes 1.
- **Get-off**: `Action = jump_horse` -> `jump_horse_start` (`fist_ac_horse_l_getoff_start`,
  0.67 s) -> `jump_horse_loop` (`_loop`) -> `jump_horse_end` (`_end`, 0.83 s). Always the
  **left** get-off clip, no side selection (`player_full_body.xml:11733-11747`; the clips'
  Bip01 root motion is (0,0,0) -> (-1.0,0,-1.888), a hop from the saddle to the ground,
  `re-mounts.md` §2).

---

## 7. Camera during the transition

No function in the summon/mount/unmount/attach/detach chain calls the camera manager or changes
a camera mode (the callee lists in §8 and A contain no `3916xxxx` camera target). The camera
manager remains `X2::GameClient::CameraManager::Update FUN_3916aab0` (six modes,
`docs/research/re-movement.md` §2.7). Because the local player unit is attached to the seat
bone, the unit-camera target moves with the seat; the model `450` values `camera_distance 15.0`
and `camera_distance_for_wide_angle 12.0` (`re-mounts-full.md` §6) are not consumed by any
mounted branch found. There is no camera blend during the get-on/get-off clips; the clip root
motion is skeletal only. This is [R] for the absence of camera calls and the values, [?] for
what a live client actually does with the `camera` bone in `horse.chr`.

---

## 8. Function table

| addr | size | callers | callees | offsets / globals | kind | test |
|---|---:|---|---|---|---|---|
| `390697f0` | 105 | Lua binding | `3945fcd0`, `3933f5e0`, `393642f0` | own mate `+0x20` | stateful | call `thiscall` with a live actor and target; observe the started controller |
| `390697b0` | 27 | Lua binding | `39361e80` | — | stateful | call; observe 0xa8 |
| `3906a440` | 284 | Lua binding | `393578e0`, `39360020`, `3935aa10`, `3933f5e0`, `39058bd0` | selected/own mate | stateful | call with a live unit id; returns bool |
| `393642f0` | 267 | `390697f0`, `39365230` | `3935f560`, `3935aa10`, `39390900`, `390b3750`, `39362c70`, `390ca4d0`, `39429e40` | `_DAT_3a169944` cooldown | stateful | call with a live unit id; dump the created controller |
| `3935aa10` | 556 | `3906a440`, `393642f0` | `390b3750`, `390b3850`, `39387690`, `3933aa20`, `3933aa40` | unit `+0x410/+0x3c26`; player `+0x3c26/+0x3a71/+0x3a73/+0xe9c/+0x3ac4..`; `_DAT_399cff68=625` | pure-ish (reads world) | call `thiscall`? it is `cdecl(out, unit)` in the decompile; use `call ... cdecl p:out p:unit`, dump 8 bytes |
| `390a5f00` | 388 | `390ca4d0` | `39360020`, `390b3750`, `390b3850`, `3909ac60`, `39428370`, `3909ad30`, `390a2c90` | controller `+0x04/+0x14/+0x18/+0x1c/+0x2c/+0x30/+0x34`; `DAT_3999cce8=10.0` | stateful | call with a scratch 0x38 buffer (`this`), live actor + mate id; dump controller |
| `390aaf30` | 186 | vtable +4 | `39424580`, `39360020`, `390aa730`, `3909b070` | `+0x14/+0x18/+0x1c/+0x34` | stateful | per-frame; call `thiscall` with a live controller; inspect sent/finished |
| `390aa730` | 2037 | `390aaf30` | `39111d60`, `390a8dc0`, `390b3850`, `39097bd0`, `390983f0`, `39098380`, `39424580`, `390a5d00` | see §3; `399a3240=0.64`, `399a3244=2.25`, `399a3238=-1.5`, `399a323c=2.5`, `399a26e4=0.25` | stateful | call with a live controller+mate; compare the movement request |
| `390a2c90` | 502 | `390a5f00` | `3909ae90`, `390b3870` | controller `+0x20..+0x28` | stateful (geometry) | call with a scratch controller (`+0x14` live player) and a live mate; dump 12 bytes |
| `3909b070` | 300 | `390aaf30` | `390b3850`, `3909ae90`, `397ac280` | `0x39ac3878=9.0`, `0x3999cffc=2.0`, `DAT_39ae4eb8` | stateful | call with a live controller+mate; capture 0xa7 |
| `3909ae90` | 478 | `3909b070`, `390a2c90` | `390b3750`, `390c9f40`, `390b3850`, `390b3870` | `399a2d28=0.96`, `399a2d24=-0.96` | stateful (geometry) | call `fastcall` with ECX=mate (this) and the live player; returns 1..4 |
| `390c9f40` | 42 | `3909ae90`, `3906a640`, other | — | mount `+0x120` list; key = seat | pure-ish (reads list) | call `thiscall` this=mount, arg=seat; returns value or 0 |
| `390ca4d0` | 132 | `393642f0` | `3950a1b0`, `390a5f00`, `390ca140` | actor `+0x16*4 = +0x58` | stateful | call with a live actor + mate id/point; dump actor+0x58 |
| `390ca560` | 136 | `390cea80`, `39362070` | `3950a1b0`, `390a6110`, `390ca140` | actor `+0x58` | stateful | call with a live actor + mate id/reason |
| `390ca140` | 151 | several controller starters | listener list | actor `+0x58`, `+0x64` | stateful | call after a controller is installed; observe `CSCreateSkillController` |
| `390a6110` | 322 | `390ca560` | `39360020`, `39096160`, `390b3750`, `390a2e90`, `390b39c0` | `+0x14/+0x18/+0x1c/+0x20/+0x24/+0x28/+0x29` | stateful | call with a scratch 0x2c buffer; live actor+mate |
| `390aaff0` | 495 | vtable +4 | `390bb400`, `39360020`, `390b3750`, `390a8dc0`, `39098380`, `390a5d00` | `+0x1c/+0x24/+0x28/+0x29` | stateful | per-frame; inspect finished after 500/1000 ms |
| `390a2e90` | 554 | `390a6110` | `3933b270`, `3933be30`, `3909b4a0` | `0x39ac3888/8c/90` | stateful (geometry) | call with a live controller+mate, reason 3/8/c vs default |
| `3909b1a0` | 764 | `3909b4a0` | `3933b270`, `3933be30` | `0x39ac3888=1.5`, `0x39ac388c=1.0`, `0x3999dd60`, `0x3999c5f0` | pure-ish (geometry + world query) | call with a scratch controller (`+0x14` player) and live mate, out 12-byte vec; returns 0/1 |
| `3909b4a0` | 380 | `390a2e90` | `3909b1a0` | `0x39ac3890=4.0`; `+0x29` | stateful | call with a live controller+mate; dump the resulting transform |
| `39356820` | 187 | controller listener | `393966d0`, `393967c0`, `39403480` | opcode 0x8b, type/immune | stateful | call with a live controller; capture the packet |
| `393568e0` | 172 | controller listener | `390b3750` | opcode 0x8b, type 0 | stateful | call after ending a controller; capture the packet |
| `390b3f10` | 225 | `39362770` | `390e16b0` | child `+0x3af0/+0x3af4` | stateful | call with live child/parent actors and a reason |
| `390b4000` | 102 | `39355bd0`, `39366480` | `390dca30` | child `+0x3af0/+0x3af4` | stateful | call with a live child; inspect detach |
| `391d49d0` | 155 | packet registry | `390115a0`, `39360020`, `39362770` | packet `+0xc/+0x10/+0x18` | stateful | scratch packet + live units |
| `391d4a70` | 145 | packet registry | `390115a0`, `39360020`, `39355bd0` | packet `+0xc/+0x10` | stateful | scratch packet + live units |
| `39362770` | 300 | `391d49d0`, `39364d40` | `390b3f10`, `390e16b0`, `39424240` | parent wait, event 0x61 | stateful | integration |
| `39364d40` | 111 | retry callback | `3935f560`, `39362770` | retry reason 8 | stateful | integration |
| `390e16b0` | 94 | `390b3f10` | `390dea00` | parent `+0x11c/+0x12c` | stateful | call with live actors |
| `390eb520` | 239 | virtual +0x288 | `390e96a0`, `39111970`, `39695a70` | set Vehicle/Seat | stateful | call with live child/parent and reason |
| `390e96a0` | 4268 | `390eb520` | `3963b490`, `390bb7c0`, `390c31b0`, `390bea90`, `__libm_sse2_atan2`, `390c32d0`, `390bf0f0`, `390e5730`, `390c5660` | attach-point data, CDF bones | stateful | call with live child/parent; inspect the bone binding |
| `390cea80` | 442 | virtual +0x288 (detach) | `390c0ef0`, `3963b490`, `390bb7c0`, `39111970`, `390ca560` | stirrup bones, `+0x24c/+0x250` | stateful | call with live child/parent; inspect the controller |
| `390cd890` | 4590 | `390dd080` | — | stirrup/toe bones | stateful (per-frame IK) | compare bone transforms for a saddle model |
| `39153b60` | 150 | `390bb4d0`, `39155ed0`, `39155f70` | `39571f10` | graph inputs `this+0x1a` Vehicle, `+0x1c` VehicleSeat | pure-ish | call with a scratch graph-input id holder; dump the ids |
| `39204320` | 156 | `39204500`, `392045f0` | `XlCopyString` | dynamic bar entries `+0x14c`, stride 0x54 | stateful | call with a scratch bar; dump the entry |
| `392045f0` | 263 | many UI/interaction paths | `39360020`, `3933b1b0`, `39204320`, `39204500`, `392044a0`, `393fe030`, `39424220` | bar count `+0x13b0`, show `+0x13b4`; `icon_skill_vehicle03`, key 0x13 | stateful | call with a live local player mounted on another owner's mate; inspect the bar entries |
| `3933b1b0` | 60 | many | — | unit `+0x410/+0x420` | pure-ish | call `cdecl p:unit`; returns bool |
| `393bdad0` | 799 | `393bddf0` | cursor ray, `393bd650`, `393bd850`, `393bd050` | candidate/bar | stateful | integration; needs a cursor target |
| `393bd850` | 336 | `393bdad0` | `390b3ae0`, `393bcb90`, `393bcd60`, `393bc980`, `393bd290`, `393b24e0` | unit target `+0x8` | stateful | call with a live unit under the cursor |
| `393bcb90` | 134 | `39347fb0`, `393bcc20`, `393bcd60`, `393bd850` | `390b3750`, `3910a950`, `390ac0e0` | unit skill list | stateful | call with a live unit; dump the skill list |
| `3947e130` | 186 | UI script binding | `396f6390`, `394e6c10`, `39002b90` | skill icon path | pure-ish | call with a skill id; returns the path string |
| `3947e730` | 2550 | UI script binding | — | X2Interaction API | registration | not a unit test |
| `39571f10` | 102 | `39153b60` | — | pose switch | pure | call `cdecl` with 0..11; returns the name |

Data used by the tests: `.data` constants listed in the text; `model_bindings` /
`model_attach_point_strings` rows for model 450 (`re-mounts.md` §2); the event table
`FUN_39214380` for `0x19d/0x19e/0x19f` (`DYNAMIC_ACTION_BAR_SHOW/HIDE/EXECUTE`), `0x5f`
`MODE_ACTIONS_UPDATE`, `0x112/0x113` (`MOUNT_PET`/`UNMOUNT_PET`), and `0x110/0x111`.
