# CryAnimation step D: the rider's own clips, facing and seat

Addresses are `cryanimation.dll` (preferred base `0x31500000`) and `x2game.dll`
(`0x38ff0000`) VAs. Live evidence: hidden 1.2 client through
`tools/cryanim-d/capture.py` (read-only `read`/`call`/`alloc`; scenario idle, gallop,
Q+W, E+W, A+W, stop, jump). Raw capture: `data/extract/anim-d/anim_d.json`
(git-ignored): 282 frame samples, 46 pure leaf calls.

## 1. Rider graph, states and clips

The rider's full-body graph is `game/animations/graphs/player_full_body.xml`
(`CAnimationGraph`, loaded from `player_full_body`). Mounted states:

| state | select | template | animation |
|---|---|---|---|
| `Horse_DriverIdle` | PseudoSpeed 0..0.1, VehicleSeat 0..10 | `InVehicle` | `horse_ba_relaxed_idle` |
| `Horse_DriverWalk` | PseudoSpeed 0.15..0.6 | `InVehicle` | `horse_mo_normal_walk_f` |
| `Horse_DriverRun` | PseudoSpeed 0.7..2 | `InVehicle` | `_FIST_MO_RIDER_HORSE_RUN` (XL14 LMG) |
| `Horse_DriverIdleToRun` | transition | `idle_to_movement_horse` | `_FIST_MO_RIDER_HORSE_IDLETORUN` (M2I8 LMG) |
| `Horse_DriverRunToIdle` | transition | `in_vehicle_idle_one_shot` | `horse_mo_relaxed_runtoidle_f` |
| `Horse_Driver_Jump_Start` | Action=jumpMP01 | `JumpLoop` | `horse_mo_jump_f_start` |
| `Horse_Driver_Jump_Land` | transition | `JumpEnd` | `horse_mo_jump_f_land` |
| `Horse_Enter_{l,r,b,f}` | VehicleSeat 1/2/3/4 | `vehicle_transition` | `fist_ac_horse_{l,r,b,f}_geton` |

`InVehicle` uses `TransitionTime 0.2`, `LoopAnimation 1`, `stickyOutTime 0.3`,
`speedMultiplier 1`. The run LMG asset is `animations/<race>/<sex>/lmg_files/fist_mo_rider_horse_run.lmg`
(`XL14`, 32 examples). `global_player.cal` maps the forward examples
`horse_mo_normal_run_f/fl/fr` to the **same** rider clip
`horse/horse_mo_normal_run_f.caf`; only `_l`, `_r`, `_b`, `_lturn`, `_rturn`, uphill and
downhill are distinct files. Therefore Q/E does **not** select a different pose clip from
straight running; it selects a different LMG example id with the same CAF and a different
weight/direction.

Live full-run frames (`Horse_DriverRun`, rider clip time equals mount clip time):

| input | original state | entity yaw - mount yaw | rider LMG weights | direction |
|---|---|---:|---|---|
| W | `Horse_DriverRun` | 0.1 deg | `[1,0,...]` (f) | (0,1) |
| Q+W | `Horse_DriverRun` | **+45.0 deg** | `[0,1,0,...]` (fl) | (-0.7071,0.7071) |
| E+W | `Horse_DriverRun` | **-40.8 / -38.4 deg** (settling to -45) | `[0,0,0,0,0,0,0,1,...]` (fr) | (0.7071,0.7071) |
| A+W | `Horse_DriverRun` | 0.0 deg (both yaw) | index 13/14 (downhill/lturn) | ~ (0,1) |

Counts: 20 full-run gallop rows, 8 Q+W full-run rows, 9 E+W full-run rows, 8 A+W rows,
17 idle rows, 9 stop rows. The `A+W` weights sit on the turn branch (index 8/13/14);
the `Q/E` weights sit on the 8-way directions 1 and 7.

## 2. Which clips our client plays

Our client already binds the **rider's** `.cal` (`Character.assets.cal` of the rider), so
`horse_mo_normal_run_f` resolves to `animations/<race>/<sex>/horse/horse_mo_normal_run_f.caf`
(the humanoid rider clip: 81 tracks, 79 bound to the 166-bone player skeleton; the mount's
`fist_mo_normal_run_f.caf` binds only 27 bones). The earlier 103.8 deg pelvis result was not a
horse clip: it was the rider entity keeping the mount yaw while Q/E should turn it, plus the
raw seat-bone rotation being applied as a yaw.

The missing input is the rider's own facing: in 1.2 the rider faces the **movement direction**
while the horse keeps its heading (`FUN_390dcc80` copies only PseudoSpeed / Stance /
RequestedTurnSpeedLZ; the rider entity yaw is its own). `Q+W` gives `Motion.local_dir = (-0.707,0.707)`,
hence +45 deg; `E+W` gives -45 deg. Our client had `rider facing = mount facing` (0 deg), which is
the 45 deg gap in the feel recorder.

### Wiring (step D, first commit)

`ingame/mount_anim.rs`:
* `rider_lean(local_dir)` = `atan2(-x, y)` clamped to +-90 deg (0 when not moving);
* `approach_lean` settles it at `RIDER_LEAN_RATE` 12/s.

`ingame/mount.rs`: `Rider`/`LocalRider` carry `lean`; both `pose_riders` and
`pose_local_riders` read the mount's `Motion.local_dir`, smooth the lean and set
`facing = mount.facing + lean` for the entity, `Mover` and `Transform`. The seat **position**
stays the animated seat-bone translation (the specified step-C path). Proven by the live
capture above (17 Q/E full-run rows: +45.0 / -40.8 deg) and the unit test
`the_rider_leans_into_the_movement_direction`.

## 3. Decompile leaves for the LMG selection / rate

| function | address | module | role |
|---|---|---|---|
| `FUN_31565d20` | `31565d20` | cryanimation | per-entry LMG weight dispatch (clears `desc+0xa4..`, switch by fourcc) |
| `FUN_3155c710` | `3155c710` | cryanimation | XL14 weight body (`FUN_3156b020` + `FUN_315697b0`, then tier split `desc+0x144`) |
| `FUN_315697b0` | `315697b0` | cryanimation | XL14 8-way / turn / uphill sub-weight fill |
| `FUN_3156b020` | `3156b020` | cryanimation | 8 compass weights from `desc+0x148/+0x14c` (already ported in `port::anim::lmg`) |
| `FUN_31558cd0` | `31558cd0` | cryanimation | desc direction/tier fields from the layer phases (ported in `port::anim::desc`) |
| `FUN_31561820` | `31561820` | cryanimation | LMG weight validator |
| `FUN_31561a80` | `31561a80` | cryanimation | per-example LMG playback speed (rider rate) |

The live calls in `anim_d.json` reuse the captured XL14 descriptor and call
`FUN_31565d20`/`FUN_3155c710` on a scratch copy (46 cases). The captured descriptor's own
weights are the ground truth for the port (weights sum to 1, e.g. Q+W `[0,1,...]`,
A+W `[0,0.012,0,0,0,0,0,0,0,0,0,0,0,0.606,0.333,0,0,0.001,0,...]`).

`fun_315697b0` (address `315697b0`) and `fun_3155c710` (address `3155c710`) are ported in
`port::anim::lmg` and gated by `goldens/lmg_xl14_weights.json`: 32 distinct live 32-example
rider/mount run descriptors (straight, Q+W, E+W, A+W), every weight within 1e-5 and each live
block summing to 1. The rider facing rule is gated by `goldens/rider_lean.json`: 30 live
mounted frames (7 Q+W, 4 E+W, 19 straight) where `rider_lean(dir)` matches the original's
`player_yaw - mount_yaw` within 1 deg.

### Full seat rotation

The rider entity rotation is `world = mount model * rider local yaw`: `placement(pos, facing)`
with `facing = mount.facing + lean`. Applying the mount seat bone's **raw** world rotation
(the reverted step-C attempt) or its model-space delta from bind was measured again in the
recorder and rejected: the pelvis yaw rms went from 3.3/4.2 deg to 81 deg, because the bone
carries the horse rig's bind axis. The live attachment's stored seat `QuatT` is the identity in
idle exactly because that bind axis is removed; the remaining local yaw is the rider's own
facing, which is what is applied here.


## 4. Recorder numbers (lane `anim-golden`, hidden original vs this branch)

Run `shots/feel-recorder/mounted-20261026-004310-anim-d-lean` (Bevy exe
`D:/aaemu-target/bevy-docs/debug/aa-client.exe`, label `anim-d-lean`). Scenarios and
alignment are the standard `mounted` scenario; windows below select the plateau rows only
(`player_yaw - mount_yaw` inside +-40..50 deg, and the same sign for the bone).

| metric | original 1.2 | Bevy |
|---|---:|---:|
| stride period | 575.0 ms | 575.0 ms |
| rider bob amplitude | 0.280 m | 0.292 m |
| Q+W entity yaw / pelvis-vs-root | 44.5 / 45.0 deg mean (45.0 max) | 45.2 / 45.4 deg mean (49.6 / 48.8 max) |
| E+W entity yaw / pelvis-vs-root | 44.1 / 48.6 deg mean (74.5 max) | 45.2 / 47.8 deg mean (57.2 max) |
| gallop mean speed | 9.81 m/s | 9.02 m/s |
| original frame rate | 17.5 fps (261 rows) | 60 fps (1007 rows) |

The original's 17.5 fps run makes the exact plateau windows coarse; the entity plateau itself
is +45.0 / -45.0 in both clients and the drawn pelvis is within 1 deg of the original's mean.
The `rider_lean` golden (30 frames) and the recorder confirm the same rule; the raw seat-bone
rotation was left out after the measured 81 deg rms regression (section 3).

Acceptance on this branch: `cargo test -p aa-client --bin aa-client` 1377 passed, 80 ignored;
`cargo clippy -p aa-client --bin aa-client` no errors.
