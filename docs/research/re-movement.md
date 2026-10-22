# Movement and camera (ArcheAge 1.2 client)

## 1. Scope and sources

Local player ground movement, swimming/diving, breath, ladders, click-to-move, follow, the
outbound movement packet, and the unit camera. Mounts, ships, gliders and remote-unit
interpolation are other areas (`re-mounts*.md`, `remaining.md` N4) and are only mentioned where
they change the local ground code.

Sources used, all read-only:

* `x2game.dll` logic: `C:/AAEMU_DISK/aa-re/client12/out-x2game/x2game.dll.c`, image base
  `0x38FF0000`; function VAs below are from that file. `funcs.csv`, `strings-xrefs.tsv`.
* Data: `D:/aaemu-live-bevy-srv/rs-game/Data/compact.sqlite3` (`actor_models`, `game_stances`);
  `D:/AA1.2/game_pak` for `game/default_binding.g`,
  `game/config64/cvargroups/option_camera_fov_set.cfg`, entity Lua and the breath-bar bytecode.
* Prior RE: `re-mounts-full.md` section 6, `docs/aa-client-movement.md`, `remaining.md` 2.2-2.3.
* Bevy side: `crates/aa-client/src/ingame/{player,swim,jump,sprint,camera,anim}.rs`,
  `crates/aa-net/src/{model,session}.rs`, `hud/breath.rs`.

This job produced no new capture; everything below is static evidence plus earlier live checks
cited in the parity ledger.

## 2. The rules

### 2.1 Bindings and input resolution

**Bindings [R].** The shipped 1.2 default is `game/default_binding.g` (the 2026-10-14
`D:/AA1.2/hotkey.g` is a generated/edited user copy and maps `a moveleft`; do not use it as the
default). Primary manager: `q moveleft` (:9), `w moveforward` (:11), `e moveright` (:12),
`a turnleft` (:26), `s moveback` (:27), `d turnright` (:28); `.` toggle_walk (:47), Space jump
(:49), NumLock autorun (:56); Home/End/Insert/Delete set right/back/front/left camera (:60-72),
PageUp/PageDown cycle the camera (:63,:68), wheel zooms (:73-74), mouse dx/dy rotate yaw/pitch
(:75-76). The `second` manager adds arrow-key forward/turn/back (:134-137) and Mouse4 autorun
(:138). Note there is no sprint key: `sprint` is commented out in `defaultprofile.xml:247-250`.

**Resolution [R].** `input::UpdateMovement` is `FUN_39404d50` (0x39404d50, RVA 0x414d50);
`FUN_39402590` (0x39402590, RVA 0x412590) is its key/mouse combiner. The combiner sets forward
from the forward action or **both mouse buttons**, and when a mouse button is held it clears the
turn axes and turns A/D into strafe, with the turn axis that also has a strafe action cancelled
(`FUN_39402590` lines 14-61; Bevy's `resolve_keys` reproduces it, `player.rs:766-793`). The
character faces the camera when the right button is held and turns to it once when movement
starts without buttons; the left button orbits only (`docs/aa-client-movement.md`, "Keys").

**Keyboard turn [R].** `FUN_39402e00` (0x39402e00) returns
`axis * min(dt, 0.033) * keyboard_rotate_speed * pi`; the cvar default is 0.5
(`FUN_390f7b40:551`), giving 90 deg/s. Bevy uses `TURN_RATE = pi/2` and a 33 ms cap
(`player.rs:84-85`).

**Walking [R].** The walk toggle is bit `0x1000` of the input flags, set in
`FUN_39404d50:656-659` from the two walk inputs, and copied to `param_1[0x21]` at :655.
A/D turn rate is independent of walking; walking changes the speed factor only.

**Auto-run [R].** The same function maintains the auto-run latch: while the forward/back actions
or both mouse buttons are held it clears the running flag (`:312-323`); after they are released
the next press cancels. Bevy's `AutoRun` implements this exact arm/cancel order
(`player.rs:807-832`).

### 2.2 Ground speeds and acceleration

**Per-model values [R].** `actor_models` row 1 (Nuian male) gives
`game_sprint_multiplier=1.5`, `game_strafe_multiplier=1.0`,
`game_backward_multiplier=0.65`, `game_walk_multiplier=0.45`,
`game_walk_strafe_multiplier=0.45`, `game_walk_backward_multiplier=0.2925`,
the forward/diagonal multipliers 1.0/1.0/0.65/0.45/0.2925, `game_inertia=0`,
`game_inertia_accel=0`, `game_jump_height=1.3`, `turn_speed=360` (model facing-rate cap [?]),
`physics_living_gravity=25`, `physics_living_air_resistance=0.29`,
`physics_living_k_air_control=0.9`, `physics_living_max_vel_ground=30`,
`physics_living_min_slide_angle=45`, `physics_living_max_climb_angle=50`,
`physics_living_min_fall_angle=50`, `physics_living_time_impulse_recover=1.0`,
`swim_height=0.2`, `actor_height=1.7`, `radius=0.55`. Model rows 1-12 (all playable races) share
`game_inertia=0`, `game_inertia_accel=0`, `game_jump_height=1.3`.

**Where they come from [R].** `FUN_3911a5a0` first parses the entity's Lua `gameParams` stance
block (`stance`, `maxSpeed`, `heightCollider`, `heightPivot`, `size`, `useCapsule` and the
multipliers) into the actor's movement block at +0x3ec..+0x494. `FUN_3911b950` then copies the
DB actor-model record into the same slots: multipliers at `+0x438` (sprint), `+0x43c..+0x44c`
(forward/diagonal/strafe/backward), `+0x454..+0x464` (walking set), inertia `+0x468/+0x46c`,
jumpHeight `+0x470`, swimHeight `+0x494`. Because the same slots are written, the DB record is
the runtime source and the Lua values are the fallback (e.g. `basicactor.lua:855-865` has walk
0.4, backward 0.7, inertia 20/11; the DB has 0.45, 0.65, 0/0) [I].

**Base speed [R].** `game_stances` row 74 (model 1, stance `relaxed`) has
`normal_speed=1.5`, `max_speed=5.4`; row 73 `combat` max 5.4; row 75 `swim` max 3.0;
row 78 `stealth` max 5.4 normal 0.6; row 79 `crouch` max 2.0; row 80 `prone` max 2.0. The run
speed the client and the animation graph use is 5.4 m/s: the run state selects at PseudoSpeed
0.8-3 (`player_full_body.xml:1940`) and 5.4/5.4 = 1.0. Walk = 0.45 x 5.4 = 2.43 m/s;
backward 0.65 x 5.4 = 3.51; walk-backward 0.2925 x 5.4 = 1.58; sprint 1.5 x 5.4 = 8.1.
(The player entity's Lua fallback says 4.8, `player.lua:55-62`; the DB/root motion value that
the graph is authored for is 5.4 [I].)

**Direction factor [R].** `FUN_39112690` (0x39112690) selects and interpolates one multiplier
for the eight input sectors from `+0x43c..+0x44c` or the walking set `+0x454..+0x464` when bit
0x1000 is set; a walking result is divided by the walk multiplier at :207-208. `FUN_3912b330`
(0x3912b330) calls it with the desired direction and builds the animated-character movement
vector (`jumpVec`, :266). Thus speed = stance max_speed x buff multiplier x direction factor;
the factor scales the whole diagonal (a backward diagonal is 0.65 along the diagonal, not only
its backward component).

**PseudoSpeed [R/I].** The graph gets PseudoSpeed = horizontal speed / stance max_speed (a data
fit: the run band 0.8-3 at 5.4 m/s; `re-mounts-full.md` section 5 derives the same for mounts).
Bevy passes 1.0 for a run, 1.5 for a sprint-motion run and 0.5/0.45 for walk (`anim.rs:338-470`).

**Acceleration and air [R/I].** Player models have `game_inertia=0` and `inertia_accel=0`, so
there is no ground speed ramp; velocity follows the input immediately. In the air the physics
parameters are `k_air_control=0.9`, air resistance 0.29, `max_vel_ground=30` and
`time_impulse_recover=1.0`; the exact CryPhysics combination was not decoded [?], but the client
does not simply replace horizontal velocity every frame [I] (see gap 6).

**Jump [I].** `game_jump_height=1.3` and gravity 25 give `v = sqrt(2*25*1.3) = 8.06 m/s` and
about 0.645 s of air; the special-move trigger documents jump as SpecialMove 0
(`strings.txt` 399ae3c4, `FUN_3914ae50`) and the animation state plays `jumpMP01`/`jumpMP02`
(`FUN_39132be0`, `FUN_39153ff0`). The actual impulse call was not located [?]; the value above is
the physically consistent reading of the DB pair and matches Bevy's measured 8.06
(`swim.rs:92-94`, `player.rs:561-565`).

**Fall damage [R].** `CActor::UpdateFallDamage` `FUN_39116f00` decrements a landing timer at
`+0x148`, compares the fall speed `+0x13c` against `pl_fallDamage_SpeedSafe` (game config +0x178;
server constant 20 m/s, `collision/fall.rs:27`), and on a qualifying landing computes
`impact = sqrt(40 * drop)` (0x3999f668 = 40 = 2g), clamps it to the current fall speed, stores it
at `+0x144`, and clears the fall speed/start height. The client does not apply damage; the value
is reported in `fall_vel` and the server's `DoFallDamage` sends the combat text and
`SCEnvDamage` (source `FALLING`) (`combat.rs:1562-1610`). `fall_damage_mul` (attribute 0x5c,
`FUN_3956dbd0`) is applied by the server.

### 2.3 Swimming, diving and breath

**Water state [R].** `CActor::UpdateSwimStats` `FUN_39120300` reads the water surface, maintains
the relative water level (`+0x84`), the in-water state (`+0x1f`) and a submersion timer
(`+0x20`), and fires events 10/11 on the water-level crossing (:170-183) and 12 for breath-bar
direction (:251). Crossing the surface upward plays `jump_splash` (:198-200). Stance becomes
`swim`; the graph uses `fist_ba_idle_swim`, `fist_mo_swim_*` and `_deep` below the authored water
level (`player_full_body.xml:2248-2286, 2717-2785`).

**Speeds [R/I].** The swim stance row is 3.0 m/s for model 1; the prior extraction gives
`swim_up_speed_mul 0.7`, `swim_down_speed_mul 0.5`, `swim_back_speed_mul 0.8`,
`swim_buoy_speed 2.0`, `swim_jump_permission_range 0.2`, `swim_jump_end_depth 0.2`,
`swim_jump_speed 12` (`FUN_390f7b40:274-296`; values [I] from the encrypted `.data`). Unit
attributes modify this: `swim_speed_mul` 0x42 and `underwater_swim_speed_mul` 0x8b
(`FUN_3956dbd0`). `FUN_39120300:221-223` calls the swim control; `FUN_3912ce00` sets the graph's
`SwimControlX`/`Y`.

**Breath [R].** The local breath controller is UI-side:
`FUN_3930b660` (0x3930b660) returns `(capacity, left)` from start time `+0xb24` and capacity
`+0xb28`; on a lung-capacity change it rescales the start time
(`start = now - round(new_capacity * elapsed / old_capacity)`). `FUN_3930b600` starts a dive
(`start = now`, `capacity = unit attribute 0x5b` = `lung_capacity`), and ends it (`start = 0`).
`FUN_3930b050` applies a server `SCSetBreath` correction:
`start = now - capacity + breath`. The Lua binding `X2Player.GetBreathTime`
(`FUN_39495d10` -> `FUN_3930b660`, registered at `FUN_39499170`) returns total and left; the
window `game/scriptsbin/x2ui/unitframe/player_breath_bar.alb` subscribes to `DIVE_START` /
`DIVE_END`, calls `GetBreathTime` and shows/hides the bar (bytecode strings at offsets 470/486,
656/808). Bevy's `hud/breath.rs:20-103` reproduces this arithmetic and the `DIVE_START/END`
events. Drowning damage is server-side (`breath_tick`, `regen.rs:160-190`); the client only
shows the bar and fires the dive events.

**Swim start/stop [I].** The depth threshold is not a literal in `UpdateSwimStats`; Bevy uses
`height - swim_height` = 1.3 m over the ground with 0.2 m hysteresis (`swim.rs:88`, doc
"Swimming"), and documents this as the untraced part. Marked [I].

### 2.4 Ladders

The client has a full ladder state, independent of click-to-move.

* **Grab [R].** `FUN_390c1330` finds a ladder under the use trace and calls
  `Player::GrabOnLadder` `FUN_391388f0` with reason 10: it stores the ladder object id at
  `+0x238`, the ladder kind at `+0x23c`, copies the ladder frame (+0x40..+0x60 into
  `+0x244..+0x280`), sets `isOnLadder` (`+0x1db = 0x101`), and calls the ladder update
  `FUN_39137470`.
* **Kinds [R].** `FUN_39132be0` maps kind 4 `net`, 5 `hrope`, 7 `mast`, 8 `guardrail`,
  9 45-degree ladder, 10 `bigtree`; tops use `exit_ladder_top`/`exit_70_ladder_top`
  (`FUN_3916c280`). The script stat is `isOnLadder` (`FUN_39136dc0:50`).
* **Move/leave [R].** `FUN_39137470` is the per-frame ladder transform/IK update; it validates
  the ladder entity and logs `my ladder(%u) is missing` if it vanished (:91-95).
  `Player::LeaveLadder` `FUN_39135570` has modes 2-7 for the exit clips; mode 2 sets the climb-up
  flag `+0x1dd` and plays special move 1, others clear the ladder and play the exit.
  `CPlayerRotation::ProcessLadderRotation` `FUN_3913e6a0` copies the source rotation.
* **Speed/packet [R/I].** `ladder_speed_mul` is unit attribute 0x5d (`FUN_3956dbd0` case 0x5d);
  crouch is "used for climbing ladder or rope" (`basicactor.lua:821`). A unit on a ladder sets
  actor flag 0x40 and appends a 32-bit `climb_data` after the movement block; `aa-net` reads
  exactly this (`model.rs:395-402`). The client writer was not located [?].

### 2.5 Click-to-move and follow

**Click-to-move [R/I].** Option `OPTION_ITEM_CLICK_TO_MOVE` is 54
(`crates/aa-ui/src/lua/constants.rs:427`) and defaults to 0 (`settings.rs:492`). The movement
itself is gated by the cvar `x2_effects.ui.click_to_move`
(`FUN_390a6af0:187-188`, 0x390a6af0). `MouseClickControl::Normal` `FUN_390f4960` calls it when a
click did not resolve a target/attack (:68-84). `FUN_390a6af0` takes the camera ray, raycasts
against world/objects (`RayWorldIntersection(Game)`, :131-132), rejects some target model types
(:78-83), validates the hit point and distance (:96-101), then walks/paths the local unit via
`FUN_390ca3d0` / `FUN_390b2af0` and `FUN_39095e70` (sets the movement target and starts the
controller). Keyboard movement cancels it through the normal input path [I].

**Follow [R/?].** The client exposes `TOGGLE_FOLLOW`, `toggle_follow_target`,
`GO_TO_FOLLOW_UNIT`, `followUnit` (`strings.txt` 399be05c, 399d5784, 399cfa84, 399cfaa8) and the
server error strings 495-500: `cannot_follow_in_combat`, `cannot_follow_non_party`,
`cannot_follow_too_far`, `msg_following_begin`, `msg_following_end`
(`FUN_3956b860:953-963`). No local-player follow movement code was found; the visible
`followUnit` implementation is the AI/NPC behaviour `FUN_3933c8f0`, not CPlayer. Treat player
follow as server-authoritative [?].

### 2.6 Outbound movement packet

`CSMoveUnit` opcode `0x089`, level 1. The server-side layout is in
`aaemu-game/src/packets/c2g_world.rs:217-245` and
`aaemu-game/src/models/movement.rs:249-471`; the client writer was not in the plain decompile [I].

| field | type | notes |
|---|---|---|
| `obj_id` | `bc` | the controlled unit |
| `type` | u8 | 0 default, 1 unit, 2 vehicle, 3 ship, 4 ship request, 5 transfer |
| `time` | u32 | sender physics/session time, ms |
| `flags` | u8 | 0x01, 0x02 moving, 0x04 stopping, 0x08 in combat, 0x10 sc_type+phase, 0x20, 0x40, 0x80 |
| `sc_type`+`phase` | u32+u8 | only when flags bit 0x10 |
| `pos` | 8+8+4 | x/y fixed 4096<<32, z f32 |
| `velocity` | 3 x i16 | 1/1024 m/s per axis |
| `rotation` | 3 x i8 | roll, pitch, direction byte (`2pi/128`, positive left) |
| `delta_movement` | 3 x i8 | `[right, forward, up]`, -127..127 |
| `stance` | u8 | 0 combat, 1 relaxed, 2 swim, 3 co-swim, 4 combat2, 5 stealth, 6 crouch, 7 prone, 8 fly |
| `alertness` | u8 | 0 idle, 1 alert, 2 combat |
| `actor_flags` | u8 | 0x04 standing on solid, 0x10 jumping, 0x20 on object (gc block), 0x40 hanging (gc id + climb data), 0x80 fall velocity follows |
| `fall_vel` | u16 | only with 0x80; server scale 32000 = 63.25 m/s, ignores < 20 m/s |
| support block | — | with 0x20: gc flags, part, world pos/rot; with 0x40/0x60: gc id u32; climb u32 |

**When sent [I].** Bevy sends every 0.1 s while moving/turning/airborne/swimming, one on stop,
one on landing and one on a swim start/stop (`player.rs:635-664`, `SEND_INTERVAL` :92). The
original cadence is not established (`remaining.md` 2.2); its remote physics step is capped at
33 ms (`FUN_390a30e0`), the best available hint.

**Fall velocity [R/I].** `actor_flags` 0x80 carries the landing impact speed; Bevy's
`FALL_VELOCITY_PER_MPS = 32000/63.25` (`model.rs:1278-1282`) and `MoveCommand::landing`
(`session.rs:154-160`). The original stores `+0x144` in m/s and the writer converts; the factor
was not seen [?].

### 2.7 Camera

**Manager modes [R].** `X2::GameClient::CameraManager::Update` `FUN_3916aab0` switches on a mode
byte at `+4`: 1 `FUN_39169860` (unit/third-person camera; target, zoom, collision), 2
`FUN_391632d0` (free camera [I]: `np_add/np_subtract` forward, `np_multiply/np_divide` up/down,
mouse look), 3 `FUN_3916a260`, 4 `FUN_39164100`, 5 (no per-frame update), 6 `FUN_39166390`.
Modes 3-6 are unnamed [?]; only mode 2's free-camera input is plain. `camera_max_dist` and
`camera_max_dist_debug` are registered by `FUN_3915e7a0`; `uc_set_wheel_dist`, `uc_set_pitch`,
`uc_set_yaw`, `uc_set_zOffset`, `cam_blur_amount` are console setters.

**Zoom and FOV [R].** `FUN_3915c2b0` (0x3915c2b0) settles the drawn distance towards the wanted
one: near/far clamps from cvars, acceleration = `cvar + gap^cvar` (the cvars are `tw_zoom_speed`
5 and `tw_zoom_speed_power` 1.8 in Bevy's reading). The active mode's cfg is
`game/config64/cvargroups/option_camera_fov_set.cfg`: group `[default] = 1` with
`cl_fov = 60`, `camera_max_dist = 10` (:1-6); group `[2]` (classic) has `cl_fov = 42.75`,
`camera_max_dist = 18` (:8-10). Bevy's `camera.rs:41` clamps at 10 m unconditionally and
`mode_fov` picks 60 only when option 65 == 1 (`camera.rs:141-143`); `settings.rs:95-101` records
the running client's option 65 as 2, i.e. classic 42.75 and **18 m**.

**First person and presets [R].** Under 1 m the wanted distance snaps to 0 (first person,
character hidden); zooming out snaps to 1 m (`docs/aa-client-movement.md`, "Camera"). The fixed
view actions set the camera yaw relative to the character: front = pi, left = pi/2,
right = 3pi/2, back = 0, and PageUp/PageDown step the kept fixed value by a quarter turn
(`camera.rs:241-267`, which cites x2game+0x408450..0x408620). Bevy implements them; only the
mode 2 free camera and target/unit-camera modes are absent.

**Collision and fade [R].** `ProcessUnitCamActualDist` `FUN_39165420` uses
`RayWorldIntersection(Game)` with 0x107/0x2000f and `PrimitiveWorldIntersection(Game)`
(sphere/box) to shorten the arm; `ProcessUnitCamFadeOut` `FUN_391694c0` hides a unit when the
camera is inside its radius and restores it afterwards. Bevy sweeps one sphere of radius 0.3 m
against `Mask::CAMERA` (`camera.rs:47, 516-524`) and only hides the own character below 1 m.

**Other camera inputs [R].** Start distance by race and gender (`camera.rs:76-91`, citing
x2game+0x9c0c58): Nuian 4.0/3.2, Elf 3.8/3.4, Hariharan 3.6/3.2, Ferre 4.0/3.4, other 10.
Bevy uses a fixed 0.004 rad/count mouse sensitivity (`camera.rs:55`); the original has
`viewSensitivity` and `viewLimitPitch`/`viewLimitYaw` in the actor stance block
(`FUN_3911a5a0:522-576`, values not read). Underwater uses `camera_dive_enable` (default 1),
`camera_dive_angle` (30), `camera_dive_pitch` and `camera_target_ground_align` (1)
(`FUN_390f7b40:543-551,608`); Bevy has none.

### 2.8 UI, texts and errors

* Breath bar: `player_breath_bar.alb`, `DIVE_START`/`DIVE_END`, `X2Player.GetBreathTime`
  (see 2.3). No numeric text.
* Follow errors/messages: ids 495-500 as in 2.5 (`FUN_3956b860:953-963`).
* Logs: `my ladder(%u) is missing` (missing ladder entity, `FUN_39137470:93`),
  `movement queue over flowed` / `too many unit movements..` (network queue, not local), and
  the camera debug `changedCameraType : %d -> %d`, `CameraDist= %.1f`.

## 3. Numeric parameters

| value | number | source |
|---|---|---|
| run (relaxed/combat) | 5.4 m/s | `game_stances` rows 73-74 `max_speed` |
| walk multiplier | 0.45 (2.43 m/s) | `actor_models` row 1 `game_walk_multiplier` |
| backward / walk-backward | 0.65 / 0.2925 | same row |
| strafe / diagonals | 1.0 / 1.0 / 0.65 | same row |
| sprint multiplier | 1.5 (8.1 m/s) | same row |
| swim | 3.0 m/s | `game_stances` row 75 |
| jump height / gravity | 1.3 m / 25 m/s^2 | `actor_models` row 1 |
| jump speed | 8.06 m/s | derived (`sqrt(2gh)`) [I] |
| air control / resistance | 0.9 / 0.29 | `actor_models` row 1 |
| max ground velocity | 30 m/s | same |
| slide/climb/fall angles | 45 / 50 / 50 deg | same |
| camera pivot | 1.754 m | `camera.rs:36` (view_offset 1.7 + eye) |
| distance min/max | 0..10 m action, 0..18 m classic | cfg `option_camera_fov_set.cfg:5-10` |
| FOV | 60 action, 42.75 classic | same |
| zoom notch | 0.5 x clamp(0.3 x wanted, 1, 20) | Bevy `camera.rs:94-108` [I] |
| zoom settle | `5 + gap^1.8` m/s, step <= 33 ms | `FUN_3915c2b0`, Bevy `camera.rs:110-118` |
| first person | wanted < 1 m | Bevy `camera.rs:41-43` [I] |
| breath capacity | 60,000 ms + lung bonuses | `tipcalc.rs:608`, attr 0x5b |
| swim jump permission | 0.2 m; end depth 0.2 m | `FUN_390f7b40:292-296` [I] |
| fall safe / fatal | 20 / 63.25 m/s | `collision/fall.rs:27-31` |

## 4. What is inferred

* `[I]` DB `actor_models` overwrites the Lua `gameParams` because both write the same movement
  slots (`FUN_3911a5a0`, `FUN_3911b950`); the call order was not traced.
* `[I]` Run 5.4 vs the Lua fallback 4.8: the DB row and graph bands agree on 5.4.
* `[I]` Jump 8.06 m/s from 1.3 m and g=25; the impulse call was not found.
* `[I]` Swim cvars 0.7/0.5/0.8/2.0/0.2/0.2/12 are registration defaults taken from the prior
  extraction (`swim.rs:9-11`); the values live in encrypted `.data`.
* `[I]` Ladder packet encoding follows the server reader; the client writer was not found.
* `[I]` Click-to-move cancels on keyboard input; the cancel check is not in `FUN_390a6af0`.
* `[?]` Zoom notch, first-person threshold, camera modes 3-6 and player follow.

## 5. Gaps vs Bevy (ranked)

1. **Ladders/climbing - done in `11d6122`.** The original has grab/leave, ladder frames, exit clips
   and the `isOnLadder` graph state (`FUN_391388f0`, `FUN_39135570`, `FUN_39137470`,
   `FUN_39132be0`). `aa-client`'s `ingame/ladder.rs` now grabs the nearest climb doodad with F
   (`DoodadFuncClimb` -> `climb_type_id`, kinds 1/6 ladder, 2 rope, 3 wood, 4 net, 5 hanging rope,
   7 mast, 8 guardrail, 9 45-degree ladder, 10 big tree), holds the body on the ladder plane between
   the model's bottom and top, climbs W/S at `CLIMB_SPEED` x the own buffs' `ladder_speed_mul`
   (attribute 0x5d), plays the by-path `fist_mo_<family>_up/down/idle` clips and the
   `fist_mo_ladder_end*` / `fist_mo_rope_end` exit clips, feeds 1.2's `isOnLadder` input to
   locomotion (`Motion::on_ladder`), and walks the body off at the top (or stands it at the bottom);
   Space jumps off, F plays the exit. Every hanging movement carries actor flag 0x40, the ladder's
   gc id and 32-bit `climb_data`, and `aa-net` now writes the support/hanging blocks in the server
   reader's order (`Movement::write`: 0x20 gc frame, 0x60 gc id, 0x40 climb data).

2. **Click-to-move is absent.** Original option 54 defaults off, gated by
   `x2_effects.ui.click_to_move`, with camera raycast + path (`FUN_390a6af0`, `FUN_390f4960`).
   Bevy only has skill ground targeting. Fix: setting 54, ground click raycast, path with the
   normal controller, cancel on movement/attack.

3. **Camera far limit ignores the active mode.** Classic mode is 18 m
   (`option_camera_fov_set.cfg:8-10`) while `MAX_DISTANCE` is 10 in `camera.rs:41`. If option 65
   is 2 (the running-client default recorded in `settings.rs:95-101`), zoom stops 8 m short.
   Fix: read `camera_max_dist` from the active cfg group; keep 10 for action, 18 for classic.

4. **Camera modes and underwater camera are missing.** Original `CameraManager::Update` has six
   modes including a free camera with its own move/turn speeds (`FUN_3916aab0`,
   `FUN_391632d0`, `FUN_390f7b40:28-49`) and dive/ground-align cvars
   (`FUN_390f7b40:543-551,608`). Bevy has one orbit camera plus fixed views (`camera.rs`). Fix:
   implement mode 2 free camera and the dive pitch/angle; hook the six mode actions.

5. **Collision capsule is hardcoded, not the stance data.** Bevy uses radius 0.35, height 1.8,
   step 0.5, drop 0.6 (`player.rs:74-77`); the client holds per-stance `heightCollider`,
   `heightPivot`, `size`, `modelOffset`, `useCapsule` (`FUN_3911a5a0:135-226`; DB row 74:
   height 1.2, size 0.4/0.4/0.32, capsule true) and CryPhysics 45/50/50 angles
   (`actor_models`). Fix: load the stance block and use its capsule/step.

6. **Air movement is fully authoritative and has no momentum.** Bevy applies the full input
   vector every airborne frame (`player.rs:494-510`), so releasing a key stops horizontal motion
   in mid-air. The original has 0.9 air control, 0.29 resistance and a 30 m/s ground cap
   (`actor_models`): horizontal velocity persists and input is limited. Fix: carry horizontal
   velocity through the air, add drag/control, then integrate.

7. **Stance, alertness and combat flags in the packet.** Bevy always sends `stance::RELAXED`
   on foot (or `SWIM`) and never sets `alertness`/`in_combat` (`player.rs:657-664`; defaults in
   `aa-net/src/session.rs:86-145`), while the original sends the actor's stance
   (combat/stealth/crouch/prone/fly) and the combat/SC-type flags. Fix: derive the stance byte
   and set `IN_COMBAT`/`HAS_SC_TYPE_AND_PHASE`.

8. **Swim multipliers and graph controls are not applied.** Bevy swims at a constant 3.0 m/s
   (`swim.rs:37`, `player.rs:494-496`), ignoring `move_speed_mul`, `swim_speed_mul` (0x42) and
   `underwater_swim_speed_mul` (0x8b), and does not drive `SwimControlX/Y/Z` or `waterLevel`
   (`FUN_3912ce00`). Fix: multiply the swim vector and feed the active swim controls.

9. **Camera collision/fade fidelity.** Original `ProcessUnitCamActualDist` uses several ray and
   primitive probes (`FUN_39165420:189-240`) and fades the character when the camera is near
   (`FUN_391694c0`); Bevy sweeps one 0.3 m sphere and hides only below 1 m (`camera.rs:47,
   516-524`). Fix: match the probes and add the near fade.

10. **Follow audit.** Fixed views exist (`camera.rs:241-307`), but the original also exposes
    follow (`TOGGLE_FOLLOW`, `GO_TO_FOLLOW_UNIT`) and errors 495-500; Bevy has no follow.
    Low priority until the server path is known.

## 6. Open questions

* Original movement tick and `CSMoveUnit` send cadence (`remaining.md` 2.2). A timestamped
  capture settles whether it is every frame, 10 Hz or event-driven.
* Zoom notch formula and the 1 m first-person snap; the cited x2game+0x16c2b0 is the settle
  function `FUN_3915c2b0`, not the notch.
* Jump impulse (`SpecialMove 0`) and whether the client or CryPhysics adds the vertical speed.
* Ladder trigger (key vs automatic trace), grab range and the `climb_data` writer.
* Camera modes 1, 3-6 names, and whether mode 1 is the target/unit camera or first person.
* DB `game_stances` vs the Lua `gameParams.stance` (4.8 vs 5.4).
* Fall velocity wire scale and whether the flag is set on every landing.
* Player `follow`: server-authoritative or a client path not found.

## 7. Live oracle measurements

No new capture. Prior live evidence: the running client's camera default
option 65 = 2 / classic FOV 42.75 (`settings.rs:95-101`), measured walk 5.4 m/s and swim 3.0 m/s
(`docs/aa-client-movement.md`), and the mount packet sequence (`re-mounts.md` sections 9-10).
Ladders, click-to-move, the free camera and the 18 m zoom are static-only and need a capture.
