# Remote-unit smoothing (F7): 1.2 `NetActorMovementController`

The port lives in `crates/aa-client/src/port/remote/` and is used for every remote unit when
`--ported-remote-smoothing` is passed. Without the flag remote units keep the older buffered
`remote::Track` playback.

## What was ported

| 1.2 function | RVA | port |
|---|---|---|
| `X2::NetActorMovementController::Update` | `0xB3BB0` | `RemoteController::update` |
| `UpdateLivingEntity` | `0xB30E0` | `update_living_entity` (dt clamp, delta velocity, mode dispatch, turn filter, pose) |
| `PrePhysicsUpdate` | `0xBF040` | `pre_physics_update`, `attach` (`+0x40c` hold, `+0x410` speed ramp, water 2.0) |
| `FUN_39097dc0` f32 SmoothCD | `0xA7DC0` | `smooth_cd_f32` |
| `FUN_390a0ad0` f64 SmoothCD | `0xB0AD0` | `smooth_cd_f64` (turn/facing vector) |
| 5 s timeout / 20 m snap | `0xB2420` / `0xB0040` | `port::vehicles::{net_update, within_snap_distance}` |

`sample_dt` clamps the frame step to `max(min(dt, 0.033), 0.001)`; the turn/position response is
the `DAT_39ae4e8c` getter clamped to `0.01..10.0`. The mode is `DAT_39ae4e80`: snap (0), cached
(follow, 1), carried (2), extrapolate (3). The live getter returns **3**, so the receive block
passes `SampleMode::Extrapolate` and the sample's packet velocity (1/1024 m/s, `+0x370`).
`RemoteSample` carries position, velocity, facing, mode, airborne and combat; `RemotePose`
carries the applied position, filtered yaw, horizontal speed/heading, airborne/combat and the
animation-speed multiplier.

Mode 3 does not step the drawn position to every packet. `OnOneUnitMovement` closes over
`FUN_390b2cf0`, which for mode 3 queues the sample in `FUN_390ad070`; the actor model consumes
that queue and plays the sample forward by its stored velocity. That layer is not ported, so
`RemoteController` extrapolates the sample by `sample.vel * age` (`PREDICT_MAX = 0.3 s`) and
follows it with `MODE3_FOLLOW_RESPONSE = 0.15`; the mode-3 turn uses
`MODE3_TURN_RESPONSE = 0.3` because the original's drawn player rotates slower than the NPC
0.1 s filter. Mode 1 keeps the getter response. `delta_velocity` masks each stored coordinate
with `0x7fffffff` like `FUN_390a30e0`, so a negative coordinate still contributes.

The controller's `+0x410` multiplier is wired into remote animation: `drive_locomotion` multiplies
the locomotion speed of a unit that has a `RemoteUnit` by `RemoteUnit::anim_speed()` (1.0
normally, 2.0 in water, 0.5 for 0.2 s after attach). `--remote-csv` records the controller's
target, filter velocity and `anim_speed` per frame.

## Live evidence

**Pure leaves.** `tools/remote-smooth/live_pure.py` calls the real `FUN_39097dc0` and
`FUN_390a0ad0` in the hidden client and writes `crates/aa-client/src/port/remote/goldens/`
(`smooth_f32.json`, `smooth_f64.json`): 20 live cases each; the Rust functions match every
input bit-for-bit.

**Controller.** `crates/aa-client/src/port/remote/tests.rs` replays the live f32 filter through
`RemoteController` and covers the 20 m snap, the 5 s timeout, the turn filter and the animation
ramp (10 tests, all passing with the rest of the client suite: 1370 passed / 80 ignored).

**Live controller object.** `tools/remote-smooth/live_capture.py` scanned the hidden observer's
memory for the controller vtable (`x2game+0x9B3308`): 16 active objects near the test village
(`live/candidates.json`), all on `EmptyUnitModel`-style NPC actors. The player/mount pose goes
through `FUN_390af740 -> FUN_390eed80` instead (the mount probe was negative:
`live/mount_walk.json`, commit `ebee3923`).

**Getters.** Probing `DAT_39ae4e80`/`DAT_39ae4e8c` in the live client gives a movement mode of
**3** (extrapolate branch) and a response of **0.1 s**. The mode is constant in this zone while
walking and standing.

## Two-observer player comparison (valid) 

`tools/remote-smooth/live_compare.py` runs at the same time on the move-golden lane:

* original `rmsmooth1/Rmsmootho` (hidden) watching the mover with an in-client Lua timer;
* Bevy `movegold2/Movegoldb` (headless, `--ported-remote-smoothing`) recording `--remote-csv`;
* Bevy `rmsmooth2/Rmsmoothm` (headless) walking the same scripted path.

Both observers watch the same moving unit and the same server packets. Result in
`live/compare.json` (4.08 s moving window, original 21.6 Hz, Bevy 54.9 Hz; the lane was
loaded, so both clocks are below the step-A run):

| metric | step A (raw packets) | step B (this pass) | original 1.2 |
|---|---|---|---|
| drawn vs server path, mean / p95 (m) | 0.856 / 0.973 | 0.869 / 0.943 | 0.834 / 1.407 |
| ours vs original drawn, mean / p95 (m) | 0.613 / 1.266 | 0.473 / 0.899 | — |
| yaw error ours vs original, mean / p95 (deg) | 6.36 / 19.94 | 2.04 / 5.91 | — |
| our yaw vs path, mean / p95 (deg) | 5.16 / 14.90 | 11.18 / 30.78 | 13.19 / 35.48 |
| speed, mean / sd (m/s) | 5.200 / 1.126 | 5.19 / 0.83 | 5.07 / 1.32 |
| acceleration 60 Hz, mean / p95 (m/s²) | 35.54 / 70.55 | 4.09 / 14.16 | 2.89 / 7.56 |
| acceleration 20 Hz, mean / p95 (m/s²) | — | 2.93 / 11.85 | 2.76 / 5.40 |
| yaw rate, mean / sd-ratio (rad/s) | 0.543 / 1.652 | 0.543 / 1.419 | 0.552 / 1.426 |
| turning rate, mean (rad/s) | 1.263 | 0.881 | 0.904 |

Reading: the jitter is gone from the steady walk (accel mean 4.1 vs 35.5 in step A); the
remaining p95 is the start transient in the first ~1 s of the window (last-3-s p95 7.56 at
60 Hz, equal to the original's full-window p95). The turn error, yaw rate and path error now
match the original's drawn player; the
acceleration p95 is still ~1.9x the original at 60 Hz and ~2.2x at the common 20 Hz rate.
The original is not running the NPC controller for a remote **player** here — its player pose
path is `FUN_390eed80` — so this table compares the two clients' drawn behaviour, not the same
internal chain.

### Why it jittered

1. **Mode-3 sample playback was missing (dominant).** The original picks mode 3 and queues
   samples; the queue plays each sample forward with its stored velocity before the actor model
   applies it. The port passed the raw packet position every frame, so the critically damped
   filter saw a staircase target. Replaying the recorded targets through the controller
   reproduces the step-A accel p95 (76 m/s²) exactly, and feeding the same targets through the
   played-forward mode-3 path brings it to ~14.
2. **`delta_velocity` used the sign, not the magnitude.** `FUN_390a30e0` masks `+0x4b0..+0x4b8`
   with `0x7fffffff`; the port tested `> 0.0`, so any negative coordinate produced no step
   velocity. Fixed.
3. **The frame clock.** `UpdateLivingEntity` reads `CTimer::GetFrameTime` (the 15-sample moving
   average from `FUN_3656e7e0`, capped at 0.25 s), then clamps to 0.033 s. The Bevy hook sees
   the raw render delta (mean 29.8 ms, sd 12.6 ms, 123 ms max on this lane). Applying the
   CTimer average to the controller alone is **not** a fix: the sample queue and the CSV clock
   stay on the real app clock, and the mismatch adds sampling kinks (measured p95 150 m/s² in
   the replay). The port keeps the real app clock for sample age and uses the original's
   `sample_dt` cap; the p95 improvement comes from playing the sample forward.
4. **The receive timestamp.** `OnOneUnitMovement` stamps `+0x130` with the timer at packet
   dispatch. The hook stamps with `Time::elapsed_secs` in the same Update frame that reads the
   event, so the difference is sub-frame; no separate bug was found.
5. **The turn filter.** The 0.1 s NPC response makes our yaw track the packet path much more
   closely than the original's drawn player (5.2° vs 13.2° mean vs path), which showed up as a
   6-20° cross-client yaw error in step A. The mode-3 turn now uses 0.3 s, matching the
   original's yaw rate (0.88 vs 0.90 rad/s) and cutting the cross error to 2.0°/5.9°.
6. **The sampling rate.** `movement_metrics` resamples to a 60 Hz game-time grid; a second
   `movement_common` block resamples both observers to 20 Hz. The Bevy observer runs at 54.9 Hz
   vs the original's 21.6 Hz on this loaded lane, so the 60 Hz p95 keeps more high-frequency
   detail (14.2 vs 7.6 m/s²); at 20 Hz it is 11.9 vs 5.4.

The ported path stays **opt-in**. Position and turn now match the original's drawn player, but
the acceleration p95 is still ~2x (the start transient), so it is not proposed as the default.

## NPC end-to-end attempt (blocked)

To exercise the real controller chain the watched unit must be an NPC. Probes showed:

* the village controller objects belong to the NPCs (`Burin`, `Lacia`, `Naida`, ...; `prev_pos`
  matched the NPC position within 0.03 m) and their target/velocity fields move;
* `/moveto go loka_level_perform_06` on `Burin` moved it (2.18 m net in 8 s) and its controller
  target followed, but repeated runs left it out of the observer's stream;
* `/spawn npc 6396` (daru tamer) + `client_target` works (target name checked, controller matched
  within 0.06 m), but `/moveto go`, `/ai load_path` + `/ai set_behavior FollowPath` do not move the
  freshly spawned NPC on this lane; no naturally wandering NPC exists at the test spot.

The harness `tools/remote-smooth/live_npc_compare.py` is in place (spawn, target by native unit
id, controller sampling over `ReadProcessMemory`, in-client Lua drawn timer, Bevy observer), but
this lane blocks the drive step. The controller chain itself is covered by the 40 live-call
goldens, the controller replay test and the live controller scan above.

The original client selects mode 3 (extrapolate) in this build, and the port now does too. The
SmoothCD leaves and turn filter remain proven against the live functions; `SampleMode::Smooth`
is still available for mode-1 clients. The mode-3 follow/turn responses are marked as the
stand-in for the actor-model queue layer that is not ported.

## Reproduce

```bash
# pure leaves + controller tests
CARGO_TARGET_DIR=D:/aaemu-target/bevy-pr cargo test -p aa-client --bin aa-client
# rebuild the client for the flag
CARGO_TARGET_DIR=D:/aaemu-target/bevy-pr cargo build -p aa-client --bin aa-client
# lane stack (once)
bash /d/aaemu-wt/move-golden/Scripts/live-stack.sh start
# player two-observer run -> live/compare.json
python tools/remote-smooth/live_compare.py
# NPC attempt (needs a drivable NPC; currently blocked as above)
python tools/remote-smooth/live_npc_compare.py
```

Raw captures from the runs are left in `live/` untracked (they are large); `compare.json` and the
goldens are committed.
