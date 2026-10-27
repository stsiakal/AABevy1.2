# Ridden mount + rider — full animation/movement spec (AA 1.2 client)

> **Restored 2026-10-27 from `archive/fork-main-2026-10-27`, UNVERIFIED.** The client was restarted from NickMesser's `main`; nothing described here is wired into the game. Addresses, call chains and offsets must be re-checked against the decompile ([R]/[I]/[?]) before use. A 2026-10-27 audit found 151 of the 269 EXACT/TOLERANCE labels in `proof-levels.*` weak or wrong (report: `D:\aaemu-client\handoff\2026-10-27\audit\report-proofs.md`).

Scope: Brown Lilyut Horse (item 8159 → npc 5431 → model 450 → actor model 206) and its rider.
Read-only work. Extracted assets: `tmp/re/mount-full/pak/...`; Ghidra dumps: `tmp/re/mount-full/gh/out*.txt`.
Addresses are VAs in the unpacked 1.2 DLLs (x2game base 0x38ff0000).

## 1. Which graph each side uses

| | Horse | Rider |
|---|---|---|
| asset | `game/animations/graphs/horse_full_body.xml` (140,337 B) + `.ag` (85,410 B) | `game/animations/graphs/player_full_body.xml` (1,736,415 B) + `.ag` (1,439,027 B) |
| selected by | `actor_models.animation_graph` of actor model 206 = `horse_full_body.xml`; `models.id=450 → sub_type=ActorModel, sub_id=206, mount_pose_id=1` | `actor_models.animation_graph='player_full_body.xml'` (41 models; upper-body graph `player_upper_body.xml`) |
| CDF | `model_file=objects/Characters/animals/horse/horse_brown_spot.cdf` → `objects/characters/animals/horse/horse.chr` | `nu_m.cdf` → `nu_m_base.chr` (e.g. Nuian male model 1) |
| clip library | `game/objects/characters/global_horse.cal` (locomotion groups in lines 2–9, clips `fist_mo_normal_*`) | `game/objects/characters/global_player.cal` (rider groups lines 18–19, get-on/off lines 1344–1351; horse-prefixed run clips lines 3783–3821) |
| graph note | graph header points at `game/objects/characters/monster/polarbear/polarbear_rider.cdf` (`horse_full_body.xml:2`) — the same graph is shared by all 78 mount/pet actor models | graph header `objects/Characters/nuian/male/nude/nu_m_base.cdf` (`player_full_body.xml:2`) |

The `.ag` is the compiled graph; all state/motion/input names below are present in both `.xml` and `.ag`
(verified by string scan of `horse_full_body.ag`). The DB stores the `.xml` name; the pak ships the matching
`.ag` (read from `horse_full_body.ag` for verification). **No `.chrparams` files exist in the 1.2 pak**;
the CDF declares only attachments/skin slots, and the CHR is the skinned model. Graph selection is purely
the `actor_models.animation_graph` table column (entity class `ActorModel`, `models.sub_type='ActorModel'`).

## 2. Graph inputs (horse; rider differs only in extra keys/variables)

| input | type | default | range / keys | meaning |
|---|---|---|---|---|
| `PseudoSpeed` | float | 0 | 0…30, mixinFilter=1 | gait urgency; **the** gait selector (`horse_full_body.xml:81`) |
| `RequestedTurnSpeedLZ` | float | 0 | −50…50 | requested yaw rate for turn-in-place / run turn blend (`:98`) |
| `ActualTurnSpeedLZ`, `ActualMoveSpeed*`, `RequestedMoveSpeed*` | float | 0 | −50…50 | engine-measured/requested speeds, not used by the mount state conditions |
| `Stance` | key | relaxed | combat, playing, relaxed, stealth, swim | locomotion stance (`:211`) |
| `Action` | key | idle | 47 keys incl. `jumpMP01/02`, `freefall`, `idle`, skill actions | one-shot/action selector |
| `Signal` | key (signalled) | none | 110 keys incl. `idle_rand`, `fall`, skill signals | state-machine events |
| `RequestedMoveDir4LH` / `ActualMoveDir4LH` | key | s | b,bl,br,f,fl,fr,l,r,s | 8-way direction substitutes (`[dir]`) |
| `Vehicle` | key | — | US_ltv, none (horse graph) | rider graph has 20 keys incl. `horse` |
| `VehicleSeat` | int | — | 0…10 | 0 = driving; 1–4 = get-on sides; also passenger seats |
| `waterLevel` | float | — | −1000…1000 | swim depth: `>=−1` shallow, `<−1.01` deep |
| `SwimControlX/Y/Z` | float | — | −10…10 | swim steering; −0.3…0.3 = idle |
| `UsingLookIK`, `Health`, `Firing`, `Aiming`, `AnimPhase` | int/float | 2/–/0/0.5/– | — | look-IK/action gating |

`MovementControlMethodH` values are the CryEngine enum in `crygame.dll.strings.txt` (`eMCM_Undefined=0, eMCM_Entity=1, eMCM_Animation=2, eMCM_DecoupledCatchUp=3, eMCM_ClampedEntity=4, eMCM_SmoothedEntity=5`).
Across the horse graph's 151 states: **131 are H=1/V=1** (entity), **19 are H=3/V=1** (the five locomotion states plus Skill/Spell start/end and all gliding states), **1 is H=2/V=1** (`Alive`). All V=1 (Entity). `H=3` = animation-driven with entity catch-up; `H=1` = entity-controlled (idle/jump/swim). All states have `snapshotTime=0.5` and `additionalTurnMultiplier=3` except the turn state (0).

## 3. Horse graph state machine (selected states)

Format: id — H/V, select condition — template (loop/one-shot, transition) — clips.

| state (line) | H/V | SelectWhen | Template / blend | Clips |
|---|---|---|---|---|
| `fist_ba_[stance]_idle_anim` (`:585`) | 1/1 | PseudoSpeed 0…0.1 & Stance | `locomotion_idle_entity`, loop, transition 0.3; `SetInput time=5 → Signal idle_rand`; additive idle rands 1–6 at 10 % and 7 at 68 % | `fist_ba_relaxed_idle` (+ stance overrides) |
| `FIST_MO_[STANCE]_WALK` (`:285`) | 3/1 | PseudoSpeed 0.11…0.7 & Stance | `locomotion_move_entity`, loop, transition 0.3, `structure=Moving`, select-speed 0.8/0.2 | S8_1 tree `_FIST_MO_NORMAL_WALK` = `fist_mo_normal_walk_f/fl/fr/l/b/bl/br/r` |
| `FIST_MO_IDLETORUN` (`:248`) | 3/1 | PseudoSpeed 0…1.5 | `idle_to_movement_horse`, `leave_at_stop=1`, transition 0.2, select-speed 2/0.6 | M2I8 `_FIST_MO_RELAXED_IDLETORUN` = `fist_mo_relaxed_idletorun_f` + run l/b/bl/br/r examples |
| `FIST_MO_NORMAL_RUN` (`:254`) | 3/1 | PseudoSpeed 0.8…3.0 | `locomotion_move_entity`, loop, transition 0.2, `structure=HorseRun`, select-speed 2/0.6 | XL14 `_FIST_MO_NORMAL_RUN`: 16 run rows — 8 dirs (`fist_mo_normal_run_f/fl/l/bl/b/br/r/fr`) then lturn/rturn + uphill/downhill extra rows — followed by the same 16 rows as `fist_mo_normal_sprint_*` (`fist_mo_normal_run.lmg`) |
| `FIST_MO_NORMAL_RUNTOIDLE` (`:261`) | 3/1 | PseudoSpeed 0…1.5 | `movement_to_idle_horse`, `playback_speed=1.2`, `leave_actual_changed=1`, `leave_request_changed=1`, transition 0.2 | M2I8 `_FIST_MO_RELAXED_RUNTOIDLE` = `runtoidle_f/fl/fr/l/r`, idle for b/bl/br |
| `FIST_MO_[STANCE]_TURN` (`:267`) | 3/1 | Action=idle & PseudoSpeed 0…1.5 & Stance | `IdleOneShot`, one-shot, no transition time | TSTP `_FIST_MO_[STANCE]_TURN` = `fist_mo_relaxed_turn_r`, `fist_ba_relaxed_idle_stop`, `fist_mo_relaxed_turn_l` (blended by requested turn rate) |
| `fist_mo_jump_[dir]_start` (`:978`) | 1/1 | Action=jumpMP01 & Stance & UsingLookIK=1 | `JumpMain`, one-shot, transition 0.1 | `fist_mo_jump_[dir]_start` |
| `fist_mo_jump_[dir]_loop` (`:940`) | 1/1 | Action=jumpMP01 & Item1 & Stance | `JumpLoop`, loop, transition 0.1 | `fist_mo_jump_[dir]_loop` |
| `fist_mo_jump_[dir]_land` (`:868`) | 1/1 | RequestedMoveDir4LH=[dir] | `JumpEnd`, one-shot, transition 0.2 | `fist_mo_jump_[dir]_land` |
| `fist_mo_jump_freefall` | 1/1 | Action=freefall | `fall`, loop, transition 0.2 | `fist_mo_jump_f_loop` |
| `em_jump_in_place` | 1/1 | Signal=em_jump_in_place | one-shot | `fist_mo_jump_f_*` |
| `fist_ba_idle_swim` (`:753`) | 1/1 | Stance=swim & SwimControl ±0.3 & waterLevel ±1000 | `SwimIdle`, loop | `fist_ba_idle_swim` |
| `fist_mo_swim_f` (`:1137`) | 1/1 | Stance=swim, PseudoSpeed 0.2…2, SwimControlY 0.4…10, waterLevel ≥ −1 | `SwimMovement`, transition 0.5 | `fist_mo_swim_f` |
| `fist_mo_swim_f_deep` | 1/1 | same, waterLevel ≤ −1.01 | `SwimMovement`, transition 0.5 | `fist_mo_swim_f_deep` |

Other swim states are `fist_mo_swim_{b,bl,br,l,r,fl,fr,up,down}` (+`_deep` variants, `:1150–1230`), same 0.5 s blend.
Transitions: `Move Start → walk|run` (cost 100, 0 s), `IDLETORUN → RUN` cost 10 (preferred) vs `→ RUNTOIDLE` cost 100, `RUN → RUNTOIDLE` 100, `RUN → Move End` 100, `jump_start → loop|land` 100 (`horse_full_body.xml:1517–1781`). Nearly all links have `transitionTime=0` (the freefall/jump-change links use 0.3); the per-state blends come from the template `transition_time` above.

## 4. Rider graph states that follow the mount

Base selection: `InHorse` = `Vehicle=horse` **and** `VehicleSeat=0` (`player_full_body.xml:1545`). `Horse Start` sits under `Main_Hub`; its four outgoing links select the entry side (`:1460`):

| state (line) | Template / seat | clip | event |
|---|---|---|---|
| `Horse_Enter_l` (`:1517`, extends `VehicleTransitionIn`) | `vehicle_transition`, seat=1 | `fist_ac_horse_l_geton` | Event onExit `BindHorseStirrup` |
| `Horse_Enter_r` (`:1521`) | seat=2 | `fist_ac_horse_r_geton` | same |
| `Horse_Enter_b` (`:1509`) | seat=3 | `fist_ac_horse_b_geton` | same |
| `Horse_Enter_f` (`:1513`) | seat=4 | `fist_ac_horse_f_geton` | same |

Seat numbers are exactly the `CSMountMate` reason byte (1 left, 2 right, 3 back, 4 front) documented in `mounts.md` §2. Clips are 1.00 s and carry root motion from ±1/±2 m and +1.888 m up to the saddle (`mounts.md` §2, `caf.py`).

Driving states (all in the `InHorse` subtree):

| state (line) | SelectWhen | Template/clip | blend/loop |
|---|---|---|---|
| `Horse_DriverIdles_NullNode` (`:1476`) | — | default hub, no clip | — |
| `Horse_DriverIdle` (`:1464`) | Action=ac_cooking & PseudoSpeed 0…0.1 & VehicleSeat 0…10 | `InVehicle` `horse_ba_relaxed_idle` | loop; `SetInput time=3 → Signal horse_ba_relaxed_idle_rand` → `horse_ba_relaxed_idle_rand_1..5` one-shots (`:11574–11626`) |
| `Horse_DriverIdleToRun` (`:1473`) | — | `idle_to_movement_horse`, `_FIST_MO_RIDER_HORSE_IDLETORUN`, `leave_at_stop=1` | transition 0.2 |
| `Horse_DriverWalk` (`:1490`) | Action=ac_cooking & PseudoSpeed 0.15…0.6 & VehicleSeat 0…10 | `InVehicle` `horse_mo_normal_walk_f` | loop |
| `Horse_DriverRun` (`:1479`) | Action=ac_cooking & PseudoSpeed 0.7…2.0 & VehicleSeat 0…10 | `InVehicle` `_FIST_MO_RIDER_HORSE_RUN` | loop; XL14 tree of **horse-prefixed** clips (`horse_mo_normal_run_*`), see `fist_mo_rider_horse_run.lmg` |
| `Horse_DriverRunToIdle` (`:1487`) | — | `in_vehicle_idle_one_shot` `horse_mo_relaxed_runtoidle_f` | transition 0.2 |
| `Horse Move to Jump` (`:1457`) | — | hub, H=3 | links into jump |
| `Horse_Driver_Jump_Start` (`:1502`) | Action=jumpMP01 & VehicleSeat 0…10 | `JumpLoop` `horse_mo_jump_f_start` | loop |
| `Horse_Driver_Jump_Land` (`:1498`) | — | `JumpEnd` `horse_mo_jump_f_land`, transition 0.2 | one-shot |
| `Horse End` (`:1454`) | — | `vehicle_exit` | exit |

Dismount uses `Action=jump_horse`: `jump_horse_start` (`fist_ac_horse_l_getoff_start`, JumpMain) → `jump_horse_loop` (`_loop`, JumpLoop) → `jump_horse_end` (`_end`, JumpEnd), `player_full_body.xml:11733–11747`.

**Rider→mount coupling:** x2game `FUN_390dcc80` (0x390dcc80), called each frame from `X2::ActorUnitModel::Update` (`FUN_390dd080`, 0x390dd080), reads the **mount's** graph `PseudoSpeed` and `RequestedTurnSpeedLZ`, forces PseudoSpeed to 0 if the mount `Stance` is `swim`, then writes both into the rider's graph (`out1.txt`, function start). No packets are involved; the rider's animation state is a function of the mount's graph. Position follows through `Unit::AttachTo` and the bone binding (`mounts.md` §2; horse CDF lines 8–9). The attached entity is rebound to `bone_spine_driver`/`bone_spine_passenger` (`model_attach_point_strings` 1/2, `models 450`, `model_bindings` owner 450 attach 1/2; `horse_brown_spot.cdf:8–9`).

## 5. Gait speeds and their derivation

`game_stances` row for actor model 206, stance 2 (`relaxed`): `normal_speed=1.5`, `max_speed=9.0`, `ai_move_speed_slow/walk/run/sprint = 1.8/1.8/9.0/9.0`; swim stance 3: `normal=1.0, max=1.8`, AI run/sprint 5.4.
`actor_models(206)`: `game_sprint_multiplier=1.5`, `game_strafe_multiplier=0.133`, `game_backward_multiplier=0.2`, `game_walk_multiplier=0.2`, `game_walk_strafe_multiplier=0.2`, `game_walk_backward_multiplier=0.2`, `game_forward_multiplier=1.0`, `game_forward_diagonal_multiplier=1.0`, `game_backward_diagonal_multiplier=0.2`, `turn_speed=360`, `game_jump_height=1.3`, `swim_height=0.3`.

| gait | formula (client) | m/s | PseudoSpeed if v/9 | expected graph state |
|---|---|---|---|---|
| walk (W + `toggle_walk`) | 9.0 × 0.2 | 1.8 | 0.20 | WALK (band 0.11–0.7) |
| run | 9.0 × 1.0 | 9.0 | 1.00 | RUN (0.8–3.0) |
| sprint (`lshift`) | 9.0 × 1.5 | 13.5 | 1.50 | RUN, sprint tier of XL14 |
| strafe (Q/E) | 9.0 × 0.133 | 1.20 | 0.133 | WALK l/r clips (inferred) |
| backward (S) | 9.0 × 0.2 | 1.8 | 0.20 | WALK b clips (inferred) |
| walk+strafe / walk+back | 1.8 × 0.2 | 0.36 | 0.04 | below WALK band; controller may clamp — unresolved |
| swim | stance max 1.8 | ≤1.8 | ≤0.2 | swim states |

`PseudoSpeed = horizontal speed / game_stances.max_speed` is a **data fit** (walk/run/sprint land exactly on the graph's bands and on the rider's 0.15/0.7 splits); the producer function itself was not located (see §7). The direction multipliers are read from `actor_models`; whether walk-mode multipliers stack on `max_speed` or on the walk base (and how a 0.36 m/s walk-strafe avoids the idle state) is not resolved.

Keys (`game/config/defaultprofile.xml`): `moveforward=w`, `moveback=s`, `turnleft=a`, `turnright=d`, **`moveleft=q`, `moveright=e`**, `jump=space`, `sprint=lshift`. A/D turn; Q/E strafe. Clips for strafe are the l/r entries of the same walk/run trees (see §3), i.e. no dedicated strafe clip set for horses.

## 6. Movement code (Part B)

**Local rider/mount.** The client moves the mate, not the player: `CSMoveUnit` carries the mate object id and a UnitMoveType (server `on_mate_unit_move`, `mounts.md` §4). The mount’s entity is the moved one; the rider entity itself receives no movement input — its graph is driven by the mount. While mounted the player’s own movement controller still produces the input, but the client drives the mount and the mount’s graph feeds the rider (§4).

**Acceleration/deceleration — how `game_inertia=4.0` / `game_inertia_accel=11.0` are applied.**
1. DB → model desc: the actor-model loader puts these columns on the model object; `FUN_390cb440` (0x390cb440) exposes them as script vars `inertia` (from desc +0x3c) and `inertiaAccel` (+0x40): `out9.txt:289–300`, `(*obj+0x14)("inertia", …)`, `("inertiaAccel", …)`.
2. Model object → actor params: `FUN_3911a5a0` (0x3911a5a0) reads script vars `inertia`, `inertiaAccel`, `jumpHeight` and stores them in the actor/CPlayer param block at +0x468, +0x46c, +0x470 (xrefs 0x3911af5d / 0x3911afa7 / 0x3911aff1, `out16.txt`).
3. Actor params → engine object: `FUN_391347b0` (0x391347b0) calls the object at `[actor+0x24]` (getter vtable +0x70, setter +0x74) with a params block whose inertia/accel slots come from +0x468/+0x46c (`out10.txt`); `FUN_3912a220`/`FUN_39127b30` do the same for other actor classes. `CActor::UpdateLivingDynamics` (`FUN_39117a90`, xref 0x39117e40) also passes the inertia value into the animated-character movement request each frame.
So the numbers are **entity dynamics parameters consumed by the engine/physics layer**, not a client-side lerp. The 1.2 `cryphysics.dll` is VMProtect-packed and was not inspected, so whether `inertia` is a time constant and `inertiaAccel` a maximum acceleration (the natural reading) is inferred.

**Turn.** `actor_models.turn_speed=360.0` (per model) is the yaw-rate cap; the graph receives `RequestedTurnSpeedLZ` (−50…50) and `FIST_MO_[STANCE]_TURN` plays the TSTP turn tree while idle. Rider gets the mount’s requested turn speed via `FUN_390dcc80`. Exact use of the 360 value is inferred (data only).

**Jump.** Graph: `Action=jumpMP01` → start/loop/land; `em_jump_in_place`; freefall `Action=freefall`. Physics: `game_jump_height=1.3 m`, `physics_living_gravity=20.0 m/s²` (actor_models 206), `physics_living_max_vel_ground=30`. No dedicated jump packet was found; the jump is part of the normal client movement/animation stream (inferred from the absence of any jump opcode and the CSMoveUnit-only model). For Bevy the physically consistent initial vertical speed is `sqrt(2·20·1.3)=7.21 m/s`, but the actual impulse application was not located.

**Slopes/falls.** `physics_living_min_slide_angle=45`, `max_climb_angle=50`, `min_fall_angle=50`, `slope_alignment=t`, gravity 20, air resistance 0.29, air control 0.9, impulse-recover 1.0. Fall damage (`CActor::UpdateFallDamage`, 0x39116f00) computes impact speed as `sqrt(40.0 · drop)` (constant 0x3999f668 = 40.0 = 2g), clamps it against the current fall speed, and compares `speed·0.1` to the game-rule threshold (`out6.txt`). No mount-specific fall path.

**Swim.** Stance becomes `swim`; the graph switches to `fist_ba_idle_swim` / `fist_mo_swim_*` with 0.5 s blends and `_deep` variants below −1 m. `waterLevel`, `SwimControl*` are the inputs. `FUN_390dcc80` sets the rider’s PseudoSpeed to 0 when the mount’s Stance is `swim`, so the rider sits idle on a swimming mount. Max swim speed from the stance row is 1.8 m/s (normal 1.0).

**Attachment / rider position.** `Unit::AttachTo` (`FUN_390b3f10`) → `On Attaching` (`FUN_390eb520`) sets rider graph `Vehicle` = mount pose name and `VehicleSeat` = reason (2/3/4 stay, else 1); pose names from `FUN_39571f10` (1 horse, 2 lion, 3 bear, 4 wildboar, 5 submarine, 6 smallboat, 7 coupleduckship, 8 speedcar, 9 elephant, 10 pangolin). `FUN_390e96a0` binds the rider entity to the mount skeleton bone (`mounts.md` §2). Every frame after `BindHorseStirrup`, `FUN_390cd890` snaps the mount’s `bone_spine_stirrups_l_01/r_01` to the rider’s toes (stirrup IK). Model 450 has no `horse_rein` rows.

**Other players’ mounts.** Observed mounts are ordinary units: their movement samples go through `X2::NetActorMovementController::UpdateLivingEntity` (`FUN_390a30e0`, 0x390a30e0). It clamps the sample dt to 0.033 s (`min(dt, 0.033)`, constants 0x399a3040=0.033, 0x3999c488=1.0), derives velocity = Δposition/dt, and runs a smoothing filter `FUN_39097dc0` (0x39097dc0) with its response parameter clamped to 0.01…10.0; its denominator is `1 + x + 0.48x² + 0.235x³` with `x = 2·dt/response` (constants 0x399a2998=0.48, 0x399a2994=0.235). Mode 0 snaps, mode 1 smooths, Mode 2 takes the position carried in the network sample, mode 3 extrapolates with the stored velocity (`param_1+0x370`). The result is submitted as the animated-character movement request (vtable +0x60, flags `local_10c=4`). The rider of a remote mount is attached client-side and is not separately interpolated. `X2::NetActorMovementController::Update` is `FUN_390a3bb0`.

**Camera.** `models(450)`: `camera_distance=15.0`, `camera_distance_for_wide_angle=12.0`, `play_mount_animation=f` (loaded by `FUN_396c7090`, 0x396c7090, columns 2/3 of the `models` SELECT). The camera manager (`X2::GameClient::CameraManager::Update`, `FUN_3916aab0`, 0x3916aab0) dispatches six camera modes (FUN_39169860/391632d0/3916a260/39164100/39166390). The model carries a `camera` bone in `horse.chr` (extracted bone table). **Inferred:** mounted play stays in the normal unit third-person mode with these distances and may anchor on the `camera` bone; no mounted-specific camera branch was found.

## 7. What is still inferred

1. **PseudoSpeed producer**: not located. The only by-name writers found are the debug binding (`FUN_3906d740`, 0x3906d740, exposes `inputPseudoSpeed`/`inputDesiredSpeed`) and the rider copy (`FUN_390dcc80`). The formula `speed/max_speed` is a data fit; the writer is probably in the big `CPlayer` update (`FUN_3906a7f0`, 0x3906a7f0) using cached AG input ids.
2. **Inertia semantics**: values are passed to physics (`pe` params via vtable +0x74, flag 0x18), but the 1.2 physics DLL could not be read; exact time-constant/acceleration meaning is inferred.
3. **Jump impulse**: height 1.3 and gravity 20 are data; the impulse call was not found; no dedicated jump opcode was found (movement is CSMoveUnit).
4. **Turn 360**: applied as a yaw cap is inferred; the code use was not traced.
5. **Camera**: mounted camera mode and use of the `camera` bone / `camera_distance` were not proven; the values and the six-mode dispatcher are confirmed.
6. **XL14 tier selection** (run vs sprint rows) is inferred from the clip order and PseudoSpeed 1.5; the compiled blend-space weighting is in `cryanimation.dll` and was not decoded.
7. **Rider idle condition quirk**: `Horse_DriverIdle/Walk/Run` all carry `Action=ac_cooking` in `SelectWhen` (`player_full_body.xml:1465/1480/1491`); this looks like a copied condition. Whether it is evaluated as “not cooking” or ignored is not resolved.
8. **Transition choice** between links of equal cost (all 100) is engine-side cost/random (`forceFollowChance`), not reproduced here.
9. **`normal_speed=1.5`** is not used by the measured walk (1.8); its role (AI normal speed vs. animation reference) is unresolved.

## 8. Source index

- Graphs: `pak/game/animations/graphs/horse_full_body.{xml,ag}`, `player_full_body.{xml,ag}`; LMGs `pak/game/animations/animals/horse/lmg_files/*.lmg`, `pak/game/animations/nuian/male/lmg_files/fist_mo_rider_horse_*.lmg`; CALs `pak/game/objects/characters/global_horse.cal`, `global_player.cal`; CDFs `horse_brown_spot.cdf`, `nu_m.cdf`; `horse.chr`.
- DB `D:/aaemu-live-bevy-srv/rs-game/Data/compact.sqlite3`: `actor_models.id=206`, `models.id=450`, `game_stances.actor_model_id=206`, `model_bindings.owner_id=450`, `model_attach_point_strings.id=1..3`.
- Ghidra: `proj-aa12u` decompiles in `gh/out1..out16.txt` (x2game); `proj-aa12-engine` crygame/cryaction in `D:/aa-re/client12/engine/out/*.c`; `crygame.dll` extra scratch decompile `gh/crygame_move.txt`; constants `gh/dump1..3.txt`.
- Prior confirmed game-level details (mount command, approach, attach, get-on root motion, UI) are cited from `mounts.md` and were not re-derived here.

