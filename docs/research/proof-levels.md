# Proof levels of the ported 1.2 functions

> **Restored 2026-10-27 from `archive/fork-main-2026-10-27`, UNVERIFIED.** The client was restarted from NickMesser's `main`; nothing described here is wired into the game. Addresses, call chains and offsets must be re-checked against the decompile ([R]/[I]/[?]) before use. A 2026-10-27 audit found 151 of the 269 EXACT/TOLERANCE labels in `proof-levels.*` weak or wrong (report: `D:\aaemu-client\handoff\2026-10-27\audit\report-proofs.md`).

Audit of branch `docs/proof-levels` (client `main` at `6ca78df4`) over
`crates/aa-client/src/port/**` and `crates/aa-physics`. The generated data is in
[proof-levels.csv](proof-levels.csv) and [proof-levels-table.md](proof-levels-table.md);
the rows that need a decision are in [proof-levels-review.md](proof-levels-review.md).
Regenerate with `python tools/port_audit/proof_levels.py` (no cargo, no build).

**Rule:** a slice is complete only when every function on its chain is **EXACT** or
**TOLERANCE** (with a stated reason, e.g. x87 80-bit intermediates) **and** wired into
the game. A low level anywhere on the chain keeps the slice open.

## Levels

| level | meaning |
|---|---|
| **EXACT** | a live golden from the hidden original compared bit-for-bit (`f32`/`f64` bits) or byte-for-byte |
| **TOLERANCE** | a live golden compared within a stated tolerance (the table records it) |
| **DECOMPILE** | ported from the decompile, no live comparison in a test (unit tests only or none) |
| **APPROX** | approximate port or stand-in: measured rule, guessed constant, caller-supplied imitation of an un-ported original |
| **REFERENCED** | an address named in a port comment; this tree does not implement it (not a proof target) |
| **GOLDEN-ONLY** | a golden names a data/table/stub address with no Rust function |

`+UNWIRED` means the function is ported at that level but nothing under
`crates/aa-client/src/ingame/` or `hud/` calls it (directly or through the port call
graph). `wired` in the CSV is `direct`, `transitive`, `type-only` or `no`.
`type-only` is **not** wired: a type from the same file is used, but the function
itself is never called. Only `direct` and `transitive` count as wired for the rule.
The direct check requires a free/qualified call, or a method call on a receiver of
the port type; common method names (`insert`, `add`, `update`) are not trusted on
their own.

The audit reads addresses from doc comments and `fun_xxxxxxxx` names, including the
bare `3155c710` form. `0x`-prefixed decompile offsets inside function bodies are not
treated as function starts (they are call-site offsets); goldens that name a table
or stub base keep that address as `GOLDEN-ONLY`.

## Totals

| level | rows |
|---|---:|
| EXACT | 232 |
| TOLERANCE | 37 |
| DECOMPILE | 213 |
| APPROX | 3 |
| REFERENCED | 352 |
| GOLDEN-ONLY | 2 |
| **total** | **839** |

839 addresses are catalogued. 485 are ported functions in this
tree; 62 of them are EXACT/TOLERANCE **and** wired, so they satisfy the rule.
352 are reference-only addresses of original functions that this
tree does not implement. 2 are data/table/stub addresses that
only a golden names.

## Per slice

| slice | addresses | ported | EXACT | TOLERANCE | DECOMPILE | APPROX | REFERENCED | GOLDEN-ONLY | wired EXACT/TOL | unwired | done |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| F1/S5 (`movement`) | 86 | 67 | 41 | 11 | 15 | 0 | 19 | 0 | 2 | 64 | **2** |
| F2 (`camera`) | 21 | 10 | 5 | 1 | 4 | 0 | 11 | 0 | 0 | 10 | **0** |
| F3/F4 (`anim`) | 125 | 81 | 28 | 18 | 35 | 0 | 44 | 0 | 0 | 81 | **0** |
| E2 (`cryanim`) | 31 | 20 | 12 | 2 | 6 | 0 | 11 | 0 | 2 | 18 | **2** |
| E6 (`combat_anim`) | 56 | 43 | 11 | 0 | 32 | 0 | 13 | 0 | 11 | 8 | **11** |
| S1 (`combat`) | 124 | 64 | 42 | 1 | 20 | 1 | 60 | 0 | 12 | 42 | **12** |
| S2 (`interaction`) | 197 | 78 | 15 | 2 | 61 | 0 | 117 | 2 | 5 | 66 | **5** |
| S3 (`vehicles`) | 45 | 35 | 23 | 0 | 12 | 0 | 10 | 0 | 0 | 33 | **0** |
| S4 (`housing`) | 104 | 66 | 38 | 1 | 27 | 0 | 38 | 0 | 27 | 20 | **27** |
| F7 (`remote`) | 8 | 5 | 2 | 0 | 1 | 2 | 3 | 0 | 2 | 0 | **2** |
| S8 (`formulas`) | 41 | 15 | 15 | 0 | 0 | 0 | 26 | 0 | 0 | 15 | **0** |
| E9 (`world`) | 1 | 1 | 0 | 1 | 0 | 0 | 0 | 0 | 1 | 0 | **1** |
| F6 (`physics`) | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | **0** |
| E1 frame loop | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | **0** |
| E3/E4 character | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | **0** |
| S6 flow graphs | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | **0** |
| S7 portals | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | **0** |

`unwired` counts ported rows whose level carries `+UNWIRED`. `done` is the only
column that satisfies the rule. `crates/aa-physics` is absent in this worktree, so
the physics slice has no rows here; the script reports it and skips it.

### F3 animation graph and F4 mount flow (the `anim` directory)

The `anim` slice contains two chains; the address rows are split here by file.

| chain | addresses | ported | EXACT | TOLERANCE | DECOMPILE | APPROX | REFERENCED | unwired | done |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| F3 | 77 | 55 | 23 | 2 | 30 | 0 | 22 | 55 | **0** |
| F4 | 48 | 26 | 5 | 16 | 5 | 0 | 22 | 26 | **0** |

F4 is the mount/summon/dismount flow (`mount.rs`, `side.rs`, `summon.rs`, `camera.rs`,
`rider.rs`); F3 is the graph runtime (`.ag` reader, state machine, LMG/blend, desc,
inputs, command writer).

## Non-EXACT functions by slice

Each row below is not done under the rule. EXACT rows are omitted unless they are
unwired. A TOLERANCE row states its tolerance; a DECOMPILE row states whether a test
exists; an APPROX row states the stand-in. Reference-only addresses are not functions
of this port: their counts are in the table above and every one is listed in the CSV
with its file and line.

### F1/S5: `movement`

86 addresses, 67 ported, 2 done, 64 ported but unwired.

#### TOLERANCE (11)

| address | Rust function | source | proof / why |
|---|---|---|---|
| `3912ac50` | apply (crates/aa-client/src/port/movement/quadruped.rs:471) | crates/aa-client/src/port/movement/quadruped.rs:471 | tolerance: 1e-5 * scale; goldens: quad.json; tests: quad_apply_matches_goldens; quad_dispatch_matches_goldens; quad_glide_matches_goldens; quad_glide_straight_matches_goldens |
| `3912af10` | fixed_flight (crates/aa-client/src/port/movement/quadruped.rs:330) | crates/aa-client/src/port/movement/quadruped.rs:330 | tolerance: 1e-6 * want.abs().max(1.0); goldens: flight_live.json; tests: special_flight_matches_live |
| `3912b090` | input_flight (crates/aa-client/src/port/movement/quadruped.rs:344) | crates/aa-client/src/port/movement/quadruped.rs:344 | tolerance: 1e-6 * want.abs().max(1.0); goldens: flight_live.json; tests: special_flight_matches_live |
| `3912b280` | glide_only (crates/aa-client/src/port/movement/quadruped.rs:1017); glide_straight (crates/aa-client/src/port/movement/quadruped.rs:375) | crates/aa-client/src/port/movement/quadruped.rs:375 | tolerance: 1e-5 * scale; goldens: quad.json; tests: quad_apply_matches_goldens; quad_dispatch_matches_goldens; quad_glide_matches_goldens; quad_glide_straight_matches_goldens |
| `3912d860` | glide (crates/aa-client/src/port/movement/quadruped.rs:389) | crates/aa-client/src/port/movement/quadruped.rs:389 | tolerance: 1e-5 * scale; goldens: quad.json; tests: quad_apply_matches_goldens; quad_dispatch_matches_goldens; quad_glide_matches_goldens; quad_glide_straight_matches_goldens |
| `3912dd40` | dispatch (crates/aa-client/src/port/movement/quadruped.rs:502) | crates/aa-client/src/port/movement/quadruped.rs:502 | tolerance: 1e-5 * scale; tol * scale; goldens: phase_c.json; quad.json; tests: phase_c_mover_matches_goldens; phase_c_quadruped_ctors_match_goldens; phase_c_rotation_bind_matches_goldens; phase_c_rotation_clamp_matches_goldens; phase_c_rotation_ctor_matches_goldens; phase_c_rotation_ladder_and_dispatch_match_goldens; phase_c_rotation_normal_matches_goldens; quad_apply_matches_goldens; quad_dispatch_matches_goldens;... |
| `3913dfd0` | bind (crates/aa-client/src/port/movement/rotation.rs:491) | crates/aa-client/src/port/movement/rotation.rs:491 | tolerance: 1e-5 * scale; tol * scale; goldens: phase_c.json; tests: phase_c_mover_matches_goldens; phase_c_quadruped_ctors_match_goldens; phase_c_rotation_bind_matches_goldens; phase_c_rotation_clamp_matches_goldens; phase_c_rotation_ctor_matches_goldens; phase_c_rotation_ladder_and_dispatch_match_goldens; phase_c_rotation_normal_matches_goldens |
| `3913e6a0` | process_ladder (crates/aa-client/src/port/movement/rotation.rs:90) | crates/aa-client/src/port/movement/rotation.rs:90 | tolerance: 1e-5 * scale; tol * scale; goldens: phase_c.json; tests: phase_c_mover_matches_goldens; phase_c_quadruped_ctors_match_goldens; phase_c_rotation_bind_matches_goldens; phase_c_rotation_clamp_matches_goldens; phase_c_rotation_ctor_matches_goldens; phase_c_rotation_ladder_and_dispatch_match_goldens; phase_c_rotation_normal_matches_goldens |
| `3913e8d0` | clamp_angles (crates/aa-client/src/port/movement/rotation.rs:271) | crates/aa-client/src/port/movement/rotation.rs:271 | tolerance: 1e-5 * scale; tol * scale; goldens: phase_c.json; tests: phase_c_mover_matches_goldens; phase_c_quadruped_ctors_match_goldens; phase_c_rotation_bind_matches_goldens; phase_c_rotation_clamp_matches_goldens; phase_c_rotation_ctor_matches_goldens; phase_c_rotation_ladder_and_dispatch_match_goldens; phase_c_rotation_normal_matches_goldens |
| `3913ff90` | process_normal (crates/aa-client/src/port/movement/rotation.rs:120) | crates/aa-client/src/port/movement/rotation.rs:120 | tolerance: 1e-5 * scale; tol * scale; goldens: phase_c.json; tests: phase_c_mover_matches_goldens; phase_c_quadruped_ctors_match_goldens; phase_c_rotation_bind_matches_goldens; phase_c_rotation_clamp_matches_goldens; phase_c_rotation_ctor_matches_goldens; phase_c_rotation_ladder_and_dispatch_match_goldens; phase_c_rotation_normal_matches_goldens |
| `391407b0` | dispatch (crates/aa-client/src/port/movement/rotation.rs:97) | crates/aa-client/src/port/movement/rotation.rs:97 | tolerance: 1e-5 * scale; tol * scale; goldens: phase_c.json; quad.json; tests: phase_c_mover_matches_goldens; phase_c_quadruped_ctors_match_goldens; phase_c_rotation_bind_matches_goldens; phase_c_rotation_clamp_matches_goldens; phase_c_rotation_ctor_matches_goldens; phase_c_rotation_ladder_and_dispatch_match_goldens; phase_c_rotation_normal_matches_goldens; quad_apply_matches_goldens; quad_dispatch_matches_goldens;... |

#### DECOMPILE (15)

| address | Rust function | source | proof / why |
|---|---|---|---|
| `390a8dc0` | build_move_to_point (crates/aa-client/src/port/movement/click_move.rs:176) | crates/aa-client/src/port/movement/click_move.rs:176 | ported from the decompile; no live golden found |
| `390b3750` | unit_chain (crates/aa-client/src/port/movement/request.rs:207) | crates/aa-client/src/port/movement/request.rs:207 | ported from the decompile; no live golden found |
| `391119f0` | blend_pair (crates/aa-client/src/port/movement/direction.rs:81) | crates/aa-client/src/port/movement/direction.rs:81 | ported from the decompile; no live golden found |
| `3911d6c0` | set_ride_stance (crates/aa-client/src/port/movement/glide.rs:249) | crates/aa-client/src/port/movement/glide.rs:249 | ported from the decompile; no live golden found |
| `391219c0` | set_glider_attached (crates/aa-client/src/port/movement/glide.rs:267) | crates/aa-client/src/port/movement/glide.rs:267 | ported from the decompile; no live golden found |
| `3912c640` | process (crates/aa-client/src/port/movement/mover.rs:102) | crates/aa-client/src/port/movement/mover.rs:102 | ported from the decompile; no live golden found |
| `3912e0c0` | process (crates/aa-client/src/port/movement/mover.rs:131) | crates/aa-client/src/port/movement/mover.rs:131 | ported from the decompile; no live golden found |
| `3912e480` | scope_copy_transform (crates/aa-client/src/port/movement/quad_scope.rs:460); scope_setup (crates/aa-client/src/port/movement/quad_scope.rs:531) | crates/aa-client/src/port/movement/quad_scope.rs:460 | ported from the decompile; no live golden found |
| `39135ed0` | process_frame_rotation (crates/aa-client/src/port/movement/rotation.rs:463) | crates/aa-client/src/port/movement/rotation.rs:463 | ported from the decompile; no live golden found |
| `3913ded0` | new (crates/aa-client/src/port/movement/rotation.rs:73) | crates/aa-client/src/port/movement/rotation.rs:73 | ported from the decompile; no live golden found |
| `39402b30` | reset (crates/aa-client/src/port/movement/input_state.rs:195) | crates/aa-client/src/port/movement/input_state.rs:195 | ported from the decompile; no live golden found |
| `39402c50` | smooth_velocity (crates/aa-client/src/port/movement/input_state.rs:249) | crates/aa-client/src/port/movement/input_state.rs:249 | ported from the decompile; no live golden found |
| `394037d0` | movement_type (crates/aa-client/src/port/movement/update.rs:153) | crates/aa-client/src/port/movement/update.rs:153 | blocked: self.cond_39354600, |
| `39403a90` | can_turn (crates/aa-client/src/port/movement/input_state.rs:383) | crates/aa-client/src/port/movement/input_state.rs:383 | ported from the decompile; no live golden found |
| `3a6ab5f3` | process (crates/aa-client/src/port/movement/mover.rs:58) | crates/aa-client/src/port/movement/mover.rs:58 | ported from the decompile; no live golden found |

#### EXACT+UNWIRED (39)

| address | Rust function | source | proof / why |
|---|---|---|---|
| `39093ef0` | inverse3x3 (crates/aa-client/src/port/movement/rotation.rs:417) | crates/aa-client/src/port/movement/rotation.rs:417 | exact, but not wired; goldens: inverse3x3.json; tests: inverse3x3_matches_goldens |
| `39095e70` | set_move_target (crates/aa-client/src/port/movement/click_move.rs:37) | crates/aa-client/src/port/movement/click_move.rs:37 | exact, but not wired; goldens: click_move_live.json; tests: click_move_target_matches_live |
| `39097bd0` | quaternion_to_euler (crates/aa-client/src/port/movement/click_move.rs:142) | crates/aa-client/src/port/movement/click_move.rs:142 | exact, but not wired; goldens: quat_euler_live.json; tests: quaternion_to_euler_matches_live |
| `390a58b0` | queue_quaternion (crates/aa-client/src/port/movement/click_move.rs:62) | crates/aa-client/src/port/movement/click_move.rs:62 | exact, but not wired; goldens: queue_quat_live.json; tests: click_queue_quaternion_matches_live |
| `390b2af0` | init_path_controller (crates/aa-client/src/port/movement/click_move.rs:284) | crates/aa-client/src/port/movement/click_move.rs:284 | exact, but not wired; goldens: path_controller_live.json; tests: path_controller_matches_live |
| `390ca3d0` | path_controller_for (crates/aa-client/src/port/movement/click_move.rs:317) | crates/aa-client/src/port/movement/click_move.rs:317 | exact, but not wired; goldens: path_controller_live.json; tests: path_controller_matches_live |
| `391130c0` | set_move_target (crates/aa-client/src/port/movement/click_move.rs:37) | crates/aa-client/src/port/movement/click_move.rs:37 | exact, but not wired; goldens: click_move_live.json; tests: click_move_target_matches_live |
| `39116f00` | update_fall_damage (crates/aa-client/src/port/movement/fall.rs:125) | crates/aa-client/src/port/movement/fall.rs:125 | exact, but not wired; goldens: fall_live.json; tests: fall_damage_matches_live; fall_start_matches_live; fall_tracker_matches_live |
| `39117590` | track_fall (crates/aa-client/src/port/movement/fall.rs:94) | crates/aa-client/src/port/movement/fall.rs:94 | exact, but not wired; goldens: fall_live.json; tests: fall_damage_matches_live; fall_start_matches_live; fall_tracker_matches_live |
| `39118650` | update_movement_request (crates/aa-client/src/port/movement/request.rs:277) | crates/aa-client/src/port/movement/request.rs:277 | exact, but not wired; goldens: request_live.json; tests: update_movement_request_matches_live_goldens |
| `3911c780` | update_fall_start (crates/aa-client/src/port/movement/fall.rs:170) | crates/aa-client/src/port/movement/fall.rs:170 | exact, but not wired; goldens: fall_live.json; tests: fall_damage_matches_live; fall_start_matches_live; fall_tracker_matches_live |
| `3911c8a0` | update_fall_up (crates/aa-client/src/port/movement/fall.rs:213) | crates/aa-client/src/port/movement/fall.rs:213 | exact, but not wired; goldens: fall_state_live.json; tests: fall_state_transitions_match_live |
| `3911c970` | select_water_state (crates/aa-client/src/port/movement/fall.rs:247) | crates/aa-client/src/port/movement/fall.rs:247 | exact, but not wired; goldens: fall_state_live.json; tests: fall_state_transitions_match_live |
| `3911fc80` | normalized_for_start (crates/aa-client/src/port/movement/glide.rs:130); set_glide (crates/aa-client/src/port/movement/glide.rs:195) | crates/aa-client/src/port/movement/glide.rs:130 | exact, but not wired; goldens: glide_live.json; tests: glide_from_skill_controller_matches_live; glide_start_and_stop_match_live |
| `3911feb0` | update_sliding_timer (crates/aa-client/src/port/movement/glide.rs:288) | crates/aa-client/src/port/movement/glide.rs:288 | exact, but not wired; goldens: sliding_timer_live.json; tests: sliding_timer_matches_live_snapshots |
| `39120300` | update_swim_stats (crates/aa-client/src/port/movement/swim.rs:134) | crates/aa-client/src/port/movement/swim.rs:134 | exact, but not wired; goldens: swim_live.json; tests: swim_stats_matches_live_goldens |
| `39121c10` | handle_glider_event (crates/aa-client/src/port/movement/glide.rs:273) | crates/aa-client/src/port/movement/glide.rs:273 | exact, but not wired; goldens: glider_cleanup_live.json; tests: glider_cleanup_event_matches_live |
| `3912bfb0` | final_pass (crates/aa-client/src/port/movement/quadruped.rs:573) | crates/aa-client/src/port/movement/quadruped.rs:573 | exact, but not wired; goldens: final_pass.json; tests: quad_final_pass_matches_goldens |
| `3912c770` | mounted (crates/aa-client/src/port/movement/quadruped.rs:928) | crates/aa-client/src/port/movement/quadruped.rs:928 | exact, but not wired; goldens: mounted.json; tests: quad_mounted_matches_goldens |
| `3912ce00` | swim_control (crates/aa-client/src/port/movement/swim_control.rs:105) | crates/aa-client/src/port/movement/swim_control.rs:105 | exact, but not wired; goldens: swim_control_live.json; tests: swim_control_matches_live |
| `3912e540` | quat_cross_plus (crates/aa-client/src/port/movement/quad_scope.rs:116); scope_ground_align (crates/aa-client/src/port/movement/quad_scope.rs:146) | crates/aa-client/src/port/movement/quad_scope.rs:116 | exact, but not wired; goldens: scope.json; tests: scope_c_ground_and_pose_match_goldens |
| `3912ebc0` | quat_cross_plus (crates/aa-client/src/port/movement/quad_scope.rs:116); scope_pose (crates/aa-client/src/port/movement/quad_scope.rs:296) | crates/aa-client/src/port/movement/quad_scope.rs:116 | exact, but not wired; goldens: scope.json; tests: scope_c_ground_and_pose_match_goldens |
| `3912f600` | scope_copy_transform (crates/aa-client/src/port/movement/quad_scope.rs:460); scope_setup (crates/aa-client/src/port/movement/quad_scope.rs:531) | crates/aa-client/src/port/movement/quad_scope.rs:460 | exact, but not wired; goldens: scope_setup.json; tests: scope_c_setup_matches_goldens |
| `39135fe0` | process_frame_movement (crates/aa-client/src/port/movement/mover.rs:188) | crates/aa-client/src/port/movement/mover.rs:188 | exact, but not wired; goldens: wrappers.json; tests: wrapper_frame_movement_matches_goldens; wrapper_frame_rotation_matches_goldens |
| `3915c700` | mouse_yaw (crates/aa-client/src/port/movement/mouse.rs:29) | crates/aa-client/src/port/movement/mouse.rs:29 | exact, but not wired; goldens: mouse_yaw.json; tests: mouse_yaw_matches_goldens |
| `3915c7d0` | mouse_pitch (crates/aa-client/src/port/movement/mouse.rs:34) | crates/aa-client/src/port/movement/mouse.rs:34 | exact, but not wired; goldens: mouse_pitch.json; tests: mouse_pitch_matches_goldens |
| `3922aed0` | clamp (crates/aa-client/src/port/movement/clamp.rs:13) | crates/aa-client/src/port/movement/clamp.rs:13 | exact, but not wired; goldens: clamp.json; tests: clamp_matches_goldens |
| `3930b050` | breath_correct (crates/aa-client/src/port/movement/breath.rs:96) | crates/aa-client/src/port/movement/breath.rs:96 | exact, but not wired; goldens: breath_helpers_live.json; tests: breath_helpers_match_live |
| `3930b5b0` | swim_jump_gate (crates/aa-client/src/port/movement/breath.rs:104) | crates/aa-client/src/port/movement/breath.rs:104 | exact, but not wired; goldens: breath_helpers_live.json; tests: breath_helpers_match_live |
| `3930b600` | breath_start (crates/aa-client/src/port/movement/breath.rs:75) | crates/aa-client/src/port/movement/breath.rs:75 | exact, but not wired; goldens: breath_helpers_live.json; tests: breath_helpers_match_live |
| `3930b660` | get_breath_time (crates/aa-client/src/port/movement/breath.rs:49) | crates/aa-client/src/port/movement/breath.rs:49 | exact, but not wired; goldens: breath_live.json; tests: breath_time_matches_live |
| `39357920` | from_skill_controller (crates/aa-client/src/port/movement/glide.rs:102) | crates/aa-client/src/port/movement/glide.rs:102 | exact, but not wired; goldens: glide_live.json; tests: glide_from_skill_controller_matches_live; glide_start_and_stop_match_live |
| `393ba480` | build_move_to_point (crates/aa-client/src/port/movement/click_move.rs:176) | crates/aa-client/src/port/movement/click_move.rs:176 | exact, but not wired; goldens: move_to_point_live.json; tests: move_to_point_request_matches_live |
| `39402590` | combine (crates/aa-client/src/port/movement/input.rs:51) | crates/aa-client/src/port/movement/input.rs:51 | exact, but not wired; goldens: combine.json; tests: combine_matches_goldens |
| `39402690` | prefer_axes (crates/aa-client/src/port/movement/input.rs:133) | crates/aa-client/src/port/movement/input.rs:133 | exact, but not wired; goldens: prefer_axes.json; tests: prefer_axes_matches_goldens |
| `39402e00` | turn_delta (crates/aa-client/src/port/movement/turn.rs:75) | crates/aa-client/src/port/movement/turn.rs:75 | exact, but not wired; goldens: turn_delta.json; tests: turn_delta_matches_goldens |
| `39402f60` | ladder_filter (crates/aa-client/src/port/movement/input.rs:185) | crates/aa-client/src/port/movement/input.rs:185 | exact, but not wired; goldens: ladder_filter.json; tests: ladder_filter_matches_goldens |
| `39404d50` | update_movement (crates/aa-client/src/port/movement/update.rs:214) | crates/aa-client/src/port/movement/update.rs:214 | exact, but not wired; goldens: phase_b.json; tests: phase_b_movement_type_matches_goldens; phase_b_reset_matches_goldens; phase_b_sequences_match_goldens; phase_b_smoothing_matches_goldens |
| `734c6580` | libm_sse2_asinf (crates/aa-client/src/port/movement/asinf.rs:50) | crates/aa-client/src/port/movement/asinf.rs:50 | exact, but not wired; goldens: asinf.json; tests: asinf_matches_goldens |

19 reference-only addresses are listed in the CSV; they are original
functions named by comments that this tree does not implement.

### F2: `camera`

21 addresses, 10 ported, 0 done, 10 ported but unwired.

#### TOLERANCE (1)

| address | Rust function | source | proof / why |
|---|---|---|---|
| `39169860` | compose_basis (crates/aa-client/src/port/camera/basis.rs:39); mode1_step (crates/aa-client/src/port/camera/mode1.rs:112) | crates/aa-client/src/port/camera/basis.rs:39 | tolerance: 1e-5; goldens: basis.json; mode1.json; tests: basis_matches_goldens; mode1_matches_goldens |

#### DECOMPILE (4)

| address | Rust function | source | proof / why |
|---|---|---|---|
| `3915b1c0` | zoom_speed (crates/aa-client/src/port/camera/zoom.rs:33) | crates/aa-client/src/port/camera/zoom.rs:33 | ported from the decompile; no live golden found |
| `3915c040` | mouse_look (crates/aa-client/src/port/camera/look.rs:116) | crates/aa-client/src/port/camera/look.rs:116 | ported from the decompile; no live golden found |
| `3915d370` | spring_target_valid (crates/aa-client/src/port/camera/spring.rs:33) | crates/aa-client/src/port/camera/spring.rs:33 | ported from the decompile; no live golden found |
| `3916aab0` | dispatch_mode (crates/aa-client/src/port/camera/mode1.rs:230) | crates/aa-client/src/port/camera/mode1.rs:230 | ported from the decompile; no live golden found |

#### EXACT+UNWIRED (5)

| address | Rust function | source | proof / why |
|---|---|---|---|
| `3915c2b0` | apply_zoom (crates/aa-client/src/port/camera/zoom.rs:74) | crates/aa-client/src/port/camera/zoom.rs:74 | exact, but not wired; goldens: zoom.json; tests: zoom_matches_goldens |
| `3915ccf0` | offset_quaternion (crates/aa-client/src/port/camera/basis.rs:13) | crates/aa-client/src/port/camera/basis.rs:13 | exact, but not wired; goldens: basis.json; tests: basis_matches_goldens |
| `3915d6f0` | spring_step (crates/aa-client/src/port/camera/spring.rs:67) | crates/aa-client/src/port/camera/spring.rs:67 | exact, but not wired; goldens: spring.json; tests: spring_matches_goldens |
| `39165420` | collision_step (crates/aa-client/src/port/camera/collision.rs:78) | crates/aa-client/src/port/camera/collision.rs:78 | exact, but not wired; goldens: collision.json; tests: collision_matches_goldens |
| `391694c0` | fade_step (crates/aa-client/src/port/camera/fade.rs:64) | crates/aa-client/src/port/camera/fade.rs:64 | exact, but not wired; goldens: fade.json; tests: fade_matches_goldens |

11 reference-only addresses are listed in the CSV; they are original
functions named by comments that this tree does not implement.

### F3/F4: `anim`

125 addresses, 81 ported, 0 done, 81 ported but unwired.

#### TOLERANCE (18)

| address | Rust function | source | proof / why |
|---|---|---|---|
| `3155c710` | fun_3155c710 (crates/aa-client/src/port/anim/lmg.rs:659) | crates/aa-client/src/port/anim/lmg.rs:659 | tolerance: 1e-4; 1e-5; goldens: lmg_xl14_weights.json; tests: xl14_weights_match_live_capture |
| `31565d20` | fun_3155c710 (crates/aa-client/src/port/anim/lmg.rs:659) | crates/aa-client/src/port/anim/lmg.rs:659 | tolerance: 1e-4; 1e-5; goldens: lmg_xl14_weights.json; tests: xl14_weights_match_live_capture |
| `390a2e90` | unmount_placement (crates/aa-client/src/port/anim/mount.rs:392) | crates/aa-client/src/port/anim/mount.rs:392 | tolerance: 0.09; 1e-4; goldens: mount_live.json; tests: live_attach_and_detach_bookkeeping_match; live_dismiss_sequences_match; live_mount_auto_walk_and_send_match; live_mount_controller_matches_fun_390a5f00; live_summon_matches_mate_record_and_unit_bind; live_unmount_controller_matches_fun_390a6110_and_390aaff0 |
| `390a6110` | unmount_controller_init (crates/aa-client/src/port/anim/mount.rs:333) | crates/aa-client/src/port/anim/mount.rs:333 | tolerance: 0.09; 1e-4; goldens: mount_live.json; tests: live_attach_and_detach_bookkeeping_match; live_dismiss_sequences_match; live_mount_auto_walk_and_send_match; live_mount_controller_matches_fun_390a5f00; live_summon_matches_mate_record_and_unit_bind; live_unmount_controller_matches_fun_390a6110_and_390aaff0 |
| `390aa730` | auto_walk (crates/aa-client/src/port/anim/mount.rs:219) | crates/aa-client/src/port/anim/mount.rs:219 | tolerance: 0.09; 1e-4; goldens: mount_live.json; tests: live_attach_and_detach_bookkeeping_match; live_dismiss_sequences_match; live_mount_auto_walk_and_send_match; live_mount_controller_matches_fun_390a5f00; live_summon_matches_mate_record_and_unit_bind; live_unmount_controller_matches_fun_390a6110_and_390aaff0 |
| `390aaff0` | unmount_controller_update (crates/aa-client/src/port/anim/mount.rs:356) | crates/aa-client/src/port/anim/mount.rs:356 | tolerance: 0.09; 1e-4; goldens: mount_live.json; tests: live_attach_and_detach_bookkeeping_match; live_dismiss_sequences_match; live_mount_auto_walk_and_send_match; live_mount_controller_matches_fun_390a5f00; live_summon_matches_mate_record_and_unit_bind; live_unmount_controller_matches_fun_390a6110_and_390aaff0 |
| `390b4000` | detach_from (crates/aa-client/src/port/anim/mount.rs:454) | crates/aa-client/src/port/anim/mount.rs:454 | tolerance: 0.09; 1e-4; goldens: mount_live.json; tests: live_attach_and_detach_bookkeeping_match; live_dismiss_sequences_match; live_mount_auto_walk_and_send_match; live_mount_controller_matches_fun_390a5f00; live_summon_matches_mate_record_and_unit_bind; live_unmount_controller_matches_fun_390a6110_and_390aaff0 |
| `390dd080` | actor_unit_model_update (crates/aa-client/src/port/anim/rider.rs:30); camera_distance (crates/aa-client/src/port/anim/camera.rs:26) | crates/aa-client/src/port/anim/camera.rs:26 | tolerance: 1e-6; goldens: camera_live.json; rider_copy.json; tests: live_camera_distance_write_matches_fun_390dd080; rider_copy_matches_live_mount |
| `390eb520` | reason_to_vehicle_seat (crates/aa-client/src/port/anim/mount.rs:79) | crates/aa-client/src/port/anim/mount.rs:79 | tolerance: 0.09; 1e-4; goldens: mount_live.json; tests: live_attach_and_detach_bookkeeping_match; live_dismiss_sequences_match; live_mount_auto_walk_and_send_match; live_mount_controller_matches_fun_390a5f00; live_summon_matches_mate_record_and_unit_bind; live_unmount_controller_matches_fun_390a6110_and_390aaff0 |
| `39153b60` | vehicle_keys (crates/aa-client/src/port/anim/mount.rs:68) | crates/aa-client/src/port/anim/mount.rs:68 | tolerance: 0.09; 1e-4; goldens: mount_live.json; tests: live_attach_and_detach_bookkeeping_match; live_dismiss_sequences_match; live_mount_auto_walk_and_send_match; live_mount_controller_matches_fun_390a5f00; live_summon_matches_mate_record_and_unit_bind; live_unmount_controller_matches_fun_390a6110_and_390aaff0 |
| `3933ac10` | from_spawned (crates/aa-client/src/port/anim/summon.rs:89) | crates/aa-client/src/port/anim/summon.rs:89 | tolerance: 0.09; 1e-4; goldens: mount_live.json; tests: live_attach_and_detach_bookkeeping_match; live_dismiss_sequences_match; live_mount_auto_walk_and_send_match; live_mount_controller_matches_fun_390a5f00; live_summon_matches_mate_record_and_unit_bind; live_unmount_controller_matches_fun_390a6110_and_390aaff0 |
| `3933b1b0` | mode_bar_command_unmount (crates/aa-client/src/port/anim/summon.rs:217); own_mate_unit (crates/aa-client/src/port/anim/summon.rs:146) | crates/aa-client/src/port/anim/summon.rs:146 | tolerance: 0.09; 1e-4; goldens: mount_extra_live.json; mount_live.json; tests: live_attach_and_detach_bookkeeping_match; live_detach_geometry_is_placed_to_a_side; live_dismiss_sequences_match; live_mode_bar_suppresses_own_mount; live_mount_auto_walk_and_send_match; live_mount_controller_matches_fun_390a5f00; live_summon_matches_mate_record_and_unit_bind; live_unmount_controller_matches_fun_390a6110_and_390aaff0 |
| `3933e3a0` | bind_unit (crates/aa-client/src/port/anim/summon.rs:104) | crates/aa-client/src/port/anim/summon.rs:104 | tolerance: 0.09; 1e-4; goldens: mount_live.json; tests: live_attach_and_detach_bookkeeping_match; live_dismiss_sequences_match; live_mount_auto_walk_and_send_match; live_mount_controller_matches_fun_390a5f00; live_summon_matches_mate_record_and_unit_bind; live_unmount_controller_matches_fun_390a6110_and_390aaff0 |
| `3933f0e0` | unit_state_bind (crates/aa-client/src/port/anim/summon.rs:151) | crates/aa-client/src/port/anim/summon.rs:151 | tolerance: 0.09; 1e-4; goldens: mount_live.json; tests: live_attach_and_detach_bookkeeping_match; live_dismiss_sequences_match; live_mount_auto_walk_and_send_match; live_mount_controller_matches_fun_390a5f00; live_summon_matches_mate_record_and_unit_bind; live_unmount_controller_matches_fun_390a6110_and_390aaff0 |
| `3933fd00` | from_spawned (crates/aa-client/src/port/anim/summon.rs:89) | crates/aa-client/src/port/anim/summon.rs:89 | tolerance: 0.09; 1e-4; goldens: mount_live.json; tests: live_attach_and_detach_bookkeeping_match; live_dismiss_sequences_match; live_mount_auto_walk_and_send_match; live_mount_controller_matches_fun_390a5f00; live_summon_matches_mate_record_and_unit_bind; live_unmount_controller_matches_fun_390a6110_and_390aaff0 |
| `39362070` | dismiss_plan (crates/aa-client/src/port/anim/summon.rs:181); request_dismiss (crates/aa-client/src/port/anim/summon.rs:114) | crates/aa-client/src/port/anim/summon.rs:114 | tolerance: 0.09; 1e-4; goldens: mount_extra_live.json; mount_live.json; tests: live_attach_and_detach_bookkeeping_match; live_detach_geometry_is_placed_to_a_side; live_dismiss_sequences_match; live_mode_bar_suppresses_own_mount; live_mount_auto_walk_and_send_match; live_mount_controller_matches_fun_390a5f00; live_summon_matches_mate_record_and_unit_bind; live_unmount_controller_matches_fun_390a6110_and_390aaff0 |
| `3936b000` | unit_state_bind (crates/aa-client/src/port/anim/summon.rs:151) | crates/aa-client/src/port/anim/summon.rs:151 | tolerance: 0.09; 1e-4; goldens: mount_live.json; tests: live_attach_and_detach_bookkeeping_match; live_dismiss_sequences_match; live_mount_auto_walk_and_send_match; live_mount_controller_matches_fun_390a5f00; live_summon_matches_mate_record_and_unit_bind; live_unmount_controller_matches_fun_390a6110_and_390aaff0 |
| `3936b580` | unit_state_bind (crates/aa-client/src/port/anim/summon.rs:151) | crates/aa-client/src/port/anim/summon.rs:151 | tolerance: 0.09; 1e-4; goldens: mount_live.json; tests: live_attach_and_detach_bookkeeping_match; live_dismiss_sequences_match; live_mount_auto_walk_and_send_match; live_mount_controller_matches_fun_390a5f00; live_summon_matches_mate_record_and_unit_bind; live_unmount_controller_matches_fun_390a6110_and_390aaff0 |

#### DECOMPILE (35)

| address | Rust function | source | proof / why |
|---|---|---|---|
| `305047b0` | string (crates/aa-client/src/port/anim/ag.rs:69) | crates/aa-client/src/port/anim/ag.rs:69 | ported from the decompile; no live golden found |
| `3060f730` | load_template_lib (crates/aa-client/src/port/anim/graph.rs:527) | crates/aa-client/src/port/anim/graph.rs:527 | ported from the decompile; no live golden found |
| `30630c20` | basic_query (crates/aa-client/src/port/anim/machine.rs:219) | crates/aa-client/src/port/anim/machine.rs:219 | ported from the decompile; no live golden found |
| `30632810` | write (crates/aa-client/src/port/anim/machine.rs:188) | crates/aa-client/src/port/anim/machine.rs:188 | ported from the decompile; no live golden found |
| `30634b70` | update_ephemeral (crates/aa-client/src/port/anim/machine.rs:376) | crates/aa-client/src/port/anim/machine.rs:376 | ported from the decompile; no live golden found |
| `30634d50` | update_transition (crates/aa-client/src/port/anim/machine.rs:360) | crates/aa-client/src/port/anim/machine.rs:360 | ported from the decompile; no live golden found |
| `306357f0` | update_steady (crates/aa-client/src/port/anim/machine.rs:332) | crates/aa-client/src/port/anim/machine.rs:332 | ported from the decompile; no live golden found |
| `306359e0` | perform_query (crates/aa-client/src/port/anim/machine.rs:248) | crates/aa-client/src/port/anim/machine.rs:248 | ported from the decompile; no live golden found |
| `30636480` | force_state (crates/aa-client/src/port/anim/machine.rs:267) | crates/aa-client/src/port/anim/machine.rs:267 | ported from the decompile; no live golden found |
| `30636640` | update_initial (crates/aa-client/src/port/anim/machine.rs:313) | crates/aa-client/src/port/anim/machine.rs:313 | ported from the decompile; no live golden found |
| `30639000` | update (crates/aa-client/src/port/anim/machine.rs:290) | crates/aa-client/src/port/anim/machine.rs:290 | ported from the decompile; no live golden found |
| `3063e0f0` | set_key (crates/aa-client/src/port/anim/machine.rs:130) | crates/aa-client/src/port/anim/machine.rs:130 | ported from the decompile; no live golden found |
| `3063e370` | set_float (crates/aa-client/src/port/anim/machine.rs:164) | crates/aa-client/src/port/anim/machine.rs:164 | ported from the decompile; no live golden found |
| `3063e470` | set_int (crates/aa-client/src/port/anim/machine.rs:147) | crates/aa-client/src/port/anim/machine.rs:147 | ported from the decompile; no live golden found |
| `30655570` | canonical_names (crates/aa-client/src/port/anim/graph.rs:927) | crates/aa-client/src/port/anim/graph.rs:927 | ported from the decompile; no live golden found |
| `30655c40` | gapfill (crates/aa-client/src/port/anim/graph.rs:1448) | crates/aa-client/src/port/anim/graph.rs:1448 | ported from the decompile; no live golden found |
| `30655e40` | normalize (crates/aa-client/src/port/anim/graph.rs:1415) | crates/aa-client/src/port/anim/graph.rs:1415 | ported from the decompile; no live golden found |
| `306560c0` | clip (crates/aa-client/src/port/anim/graph.rs:1439) | crates/aa-client/src/port/anim/graph.rs:1439 | ported from the decompile; no live golden found |
| `3065e050` | basic_query (crates/aa-client/src/port/anim/machine.rs:219); query (crates/aa-client/src/port/anim/graph.rs:200) | crates/aa-client/src/port/anim/graph.rs:200 | ported from the decompile; no live golden found |
| `30665830` | fun_30665830 (crates/aa-client/src/port/anim/speed.rs:68) | crates/aa-client/src/port/anim/speed.rs:68 | ported from the decompile; no live golden found |
| `315582d0` | fun_315582d0 (crates/aa-client/src/port/anim/desc.rs:72) | crates/aa-client/src/port/anim/desc.rs:72 | ported from the decompile; no live golden found |
| `31558cd0` | fun_31558cd0 (crates/aa-client/src/port/anim/desc.rs:176) | crates/aa-client/src/port/anim/desc.rs:176 | ported from the decompile; no live golden found |
| `31558eb0` | fun_31558eb0 (crates/aa-client/src/port/anim/desc.rs:155) | crates/aa-client/src/port/anim/desc.rs:155 | ported from the decompile; no live golden found |
| `31569030` | barycentric (crates/aa-client/src/port/anim/lmg.rs:160) | crates/aa-client/src/port/anim/lmg.rs:160 | ported from the decompile; no live golden found |
| `315697b0` | fun_315697b0 (crates/aa-client/src/port/anim/lmg.rs:615) | crates/aa-client/src/port/anim/lmg.rs:615 | ported from the decompile; no live golden found |
| `31569a20` | fun_31569a20 (crates/aa-client/src/port/anim/lmg.rs:180) | crates/aa-client/src/port/anim/lmg.rs:180 | ported from the decompile; no live golden found |
| `31569f80` | fun_31569f80 (crates/aa-client/src/port/anim/lmg.rs:877); rsqrt_ss (crates/aa-client/src/port/anim/lmg.rs:523) | crates/aa-client/src/port/anim/lmg.rs:523 | ported from the decompile; no live golden found |
| `3156b020` | fun_315697b0 (crates/aa-client/src/port/anim/lmg.rs:615); fun_3156b020 (crates/aa-client/src/port/anim/lmg.rs:552) | crates/aa-client/src/port/anim/lmg.rs:552 | ported from the decompile; no live golden found |
| `31574380` | fun_31574380 (crates/aa-client/src/port/anim/lmg.rs:798) | crates/aa-client/src/port/anim/lmg.rs:798 | ported from the decompile; no live golden found |
| `3160cc50` | entry (crates/aa-client/src/port/anim/lmg.rs:94) | crates/aa-client/src/port/anim/lmg.rs:94 | ported from the decompile; no live golden found |
| `3909b070` | mount_send (crates/aa-client/src/port/anim/mount.rs:273) | crates/aa-client/src/port/anim/mount.rs:273 | ported from the decompile; no live golden found |
| `3909b4a0` | dismount_place (crates/aa-client/src/port/anim/mount.rs:471) | crates/aa-client/src/port/anim/mount.rs:471 | /// keeps the saddle transform. `collision` receives the candidate and returns blocked. |
| `390a5f00` | mount_controller_init (crates/aa-client/src/port/anim/mount.rs:154) | crates/aa-client/src/port/anim/mount.rs:154 | /// `+0x1c = 1`; a blocked ray fires `mate_cannot_mount_blocked` (0x6b) and also finishes. |
| `390aaf30` | mount_controller_update (crates/aa-client/src/port/anim/mount.rs:186) | crates/aa-client/src/port/anim/mount.rs:186 | ported from the decompile; no live golden found |
| `39571f10` | pose_name (crates/aa-client/src/port/anim/mount.rs:42) | crates/aa-client/src/port/anim/mount.rs:42 | ported from the decompile; no live golden found |

#### EXACT+UNWIRED (28)

| address | Rust function | source | proof / why |
|---|---|---|---|
| `305f7a80` | evaluate_guards (crates/aa-client/src/port/anim/guards.rs:14) | crates/aa-client/src/port/anim/guards.rs:14 | exact, but not wired; goldens: evaluate_guards.json; tests: evaluate_guards_matches_goldens |
| `30604b10` | encode_int_str (crates/aa-client/src/port/anim/inputs.rs:36) | crates/aa-client/src/port/anim/inputs.rs:36 | exact, but not wired; goldens: inputs.json; tests: float_encode_matches_goldens; integer_encode_matches_goldens; key_encode_matches_goldens |
| `30604c50` | encode_int (crates/aa-client/src/port/anim/inputs.rs:25); set_int (crates/aa-client/src/port/anim/machine.rs:147) | crates/aa-client/src/port/anim/inputs.rs:25 | exact, but not wired; goldens: inputs.json; tests: float_encode_matches_goldens; integer_encode_matches_goldens; key_encode_matches_goldens |
| `30605180` | encode_float_str (crates/aa-client/src/port/anim/inputs.rs:69) | crates/aa-client/src/port/anim/inputs.rs:69 | exact, but not wired; goldens: inputs.json; tests: float_encode_matches_goldens; integer_encode_matches_goldens; key_encode_matches_goldens |
| `306052d0` | encode_float (crates/aa-client/src/port/anim/inputs.rs:47); set_float (crates/aa-client/src/port/anim/machine.rs:164) | crates/aa-client/src/port/anim/inputs.rs:47 | exact, but not wired; goldens: inputs.json; tests: float_encode_matches_goldens; integer_encode_matches_goldens; key_encode_matches_goldens |
| `30611470` | encode_key_index (crates/aa-client/src/port/anim/inputs.rs:99) | crates/aa-client/src/port/anim/inputs.rs:99 | exact, but not wired; goldens: inputs.json; tests: float_encode_matches_goldens; integer_encode_matches_goldens; key_encode_matches_goldens |
| `306115d0` | encode_key (crates/aa-client/src/port/anim/inputs.rs:85) | crates/aa-client/src/port/anim/inputs.rs:85 | exact, but not wired; goldens: inputs.json; tests: float_encode_matches_goldens; integer_encode_matches_goldens; key_encode_matches_goldens |
| `306261f0` | ag_path (crates/aa-client/src/port/anim/ag.rs:100); load_graph_from_pak (crates/aa-client/src/port/anim/graph.rs:510) | crates/aa-client/src/port/anim/ag.rs:100 | exact, but not wired; goldens: live_state_machine.json; tests: live_state_machine_sequence_reproduces; state_index_tie_break_matches_live |
| `31556e90` | fun_31556e90 (crates/aa-client/src/port/anim/command.rs:84) | crates/aa-client/src/port/anim/command.rs:84 | exact, but not wired; goldens: anim_commands.json; tests: command_writer_matches_live_bytes; lmg_expander_matches_live_original |
| `31558550` | fun_31558550 (crates/aa-client/src/port/anim/lmg.rs:359) | crates/aa-client/src/port/anim/lmg.rs:359 | exact, but not wired; goldens: lmg_layers.json; tests: lmg_layers_match_live_caps |
| `31558f80` | fun_31558f80 (crates/aa-client/src/port/anim/desc.rs:230) | crates/aa-client/src/port/anim/desc.rs:230 | exact, but not wired; goldens: anim_desc.json; tests: get_animation_desc_matches_live |
| `31568fc0` | fourcc_case (crates/aa-client/src/port/anim/fourcc.rs:74) | crates/aa-client/src/port/anim/fourcc.rs:74 | exact, but not wired; goldens: fourcc.json; tests: fourcc_matches_goldens |
| `3156bf40` | fun_3156bf40 (crates/aa-client/src/port/anim/lmg.rs:282) | crates/aa-client/src/port/anim/lmg.rs:282 | exact, but not wired; goldens: lmg_weights.json; tests: lmg_shared_blend_and_tstp_match_goldens |
| `3156c190` | fun_3156c190 (crates/aa-client/src/port/anim/lmg.rs:1033) | crates/aa-client/src/port/anim/lmg.rs:1033 | exact, but not wired; goldens: lmg_wrappers.json; tests: lmg_wrappers_match_live |
| `3156c1d0` | fun_3156c190 (crates/aa-client/src/port/anim/lmg.rs:1033) | crates/aa-client/src/port/anim/lmg.rs:1033 | exact, but not wired; goldens: lmg_wrappers.json; tests: lmg_wrappers_match_live |
| `3156c210` | fun_3156c190 (crates/aa-client/src/port/anim/lmg.rs:1033) | crates/aa-client/src/port/anim/lmg.rs:1033 | exact, but not wired; goldens: lmg_wrappers.json; tests: lmg_wrappers_match_live |
| `3156cfd0` | fun_3156cfd0 (crates/aa-client/src/port/anim/lmg.rs:986) | crates/aa-client/src/port/anim/lmg.rs:986 | exact, but not wired; goldens: lmg_xl_istp.json; tests: lmg_xl_istp_match_goldens |
| `3156d760` | fun_3156d760 (crates/aa-client/src/port/anim/lmg.rs:1044) | crates/aa-client/src/port/anim/lmg.rs:1044 | exact, but not wired; goldens: lmg_s8_fallback.json; lmg_weights.json; tests: lmg_s8_fallback_matches_goldens; lmg_shared_blend_and_tstp_match_goldens |
| `3156d7a0` | fun_3156d7a0 (crates/aa-client/src/port/anim/lmg.rs:1054) | crates/aa-client/src/port/anim/lmg.rs:1054 | exact, but not wired; goldens: lmg_wrappers.json; tests: lmg_wrappers_match_live |
| `3156e950` | fun_3156e950 (crates/aa-client/src/port/anim/lmg.rs:693) | crates/aa-client/src/port/anim/lmg.rs:693 | exact, but not wired; goldens: lmg_xl_istp.json; tests: lmg_xl_istp_match_goldens |
| `3156ef10` | fun_3156ef10 (crates/aa-client/src/port/anim/lmg.rs:785) | crates/aa-client/src/port/anim/lmg.rs:785 | exact, but not wired; goldens: lmg_xl_istp.json; tests: lmg_xl_istp_match_goldens |
| `315714b0` | get_lmg_capabilities (crates/aa-client/src/port/anim/desc.rs:213); init_lmg (crates/aa-client/src/port/anim/lmg.rs:42) | crates/aa-client/src/port/anim/desc.rs:213 | exact, but not wired; goldens: lmg_layers.json; lmg_s8_fallback.json; lmg_weights.json; lmg_wrappers.json; lmg_xl_istp.json; tests: lmg_layers_match_live_caps; lmg_s8_fallback_matches_goldens; lmg_shared_blend_and_tstp_match_goldens; lmg_wrappers_match_live; lmg_xl_istp_match_goldens |
| `31574e70` | fun_3156c190 (crates/aa-client/src/port/anim/lmg.rs:1033); fun_31574e70 (crates/aa-client/src/port/anim/lmg.rs:225) | crates/aa-client/src/port/anim/lmg.rs:225 | exact, but not wired; goldens: lmg_wrappers.json; tests: lmg_wrappers_match_live |
| `3909ae90` | entry_side (crates/aa-client/src/port/anim/side.rs:79) | crates/aa-client/src/port/anim/side.rs:79 | exact, but not wired; goldens: side.json; tests: approach_side_matches_goldens; entry_side_matches_goldens; side_placement_first_candidate_matches_goldens |
| `3909b1a0` | dismount_candidates (crates/aa-client/src/port/anim/mount.rs:402); side_placement (crates/aa-client/src/port/anim/side.rs:133) | crates/aa-client/src/port/anim/mount.rs:402 | exact, but not wired; goldens: mount_extra_live.json; side.json; tests: approach_side_matches_goldens; entry_side_matches_goldens; live_detach_geometry_is_placed_to_a_side; live_dismiss_sequences_match; live_mode_bar_suppresses_own_mount; side_placement_first_candidate_matches_goldens |
| `390a2c90` | approach_side (crates/aa-client/src/port/anim/side.rs:105) | crates/aa-client/src/port/anim/side.rs:105 | exact, but not wired; goldens: side.json; tests: approach_side_matches_goldens; entry_side_matches_goldens; side_placement_first_candidate_matches_goldens |
| `392045f0` | mode_bar_command_unmount (crates/aa-client/src/port/anim/summon.rs:217) | crates/aa-client/src/port/anim/summon.rs:217 | exact, but not wired; goldens: mount_extra_live.json; tests: live_detach_geometry_is_placed_to_a_side; live_dismiss_sequences_match; live_mode_bar_suppresses_own_mount |
| `393642f0` | pet_bar_mount_point (crates/aa-client/src/port/anim/summon.rs:222) | crates/aa-client/src/port/anim/summon.rs:222 | exact, but not wired; goldens: mount_extra_live.json; tests: live_detach_geometry_is_placed_to_a_side; live_dismiss_sequences_match; live_mode_bar_suppresses_own_mount |

44 reference-only addresses are listed in the CSV; they are original
functions named by comments that this tree does not implement.

### E2: `cryanim`

31 addresses, 20 ported, 2 done, 18 ported but unwired.

#### TOLERANCE (2)

| address | Rust function | source | proof / why |
|---|---|---|---|
| `31550610` | fun_31550610 (crates/aa-client/src/port/cryanim/process.rs:109) | crates/aa-client/src/port/cryanim/process.rs:109 | tolerance: 1e-5; tol * x.abs().max(1.0); goldens: update_b.json; tests: process_animation_update_replays_live_frames; process_animations_replays_live_processing_frames |
| `31550b90` | fun_31550b90 (crates/aa-client/src/port/cryanim/process.rs:232) | crates/aa-client/src/port/cryanim/process.rs:232 | tolerance: 1e-5; tol * x.abs().max(1.0); goldens: update_b.json; tests: process_animation_update_replays_live_frames; process_animations_replays_live_processing_frames |

#### DECOMPILE (6)

| address | Rust function | source | proof / why |
|---|---|---|---|
| `31551f50` | copy_rider_asset_block (crates/aa-client/src/port/cryanim/rider.rs:33) | crates/aa-client/src/port/cryanim/rider.rs:33 | ported from the decompile; no live golden found |
| `315559e0` | fun_315559e0 (crates/aa-client/src/port/cryanim/blend.rs:171) | crates/aa-client/src/port/cryanim/blend.rs:171 | ported from the decompile; no live golden found |
| `31556910` | fun_31556910 (crates/aa-client/src/port/cryanim/blend.rs:212) | crates/aa-client/src/port/cryanim/blend.rs:212 | ported from the decompile; no live golden found |
| `31557860` | fun_31557860 (crates/aa-client/src/port/cryanim/command.rs:130) | crates/aa-client/src/port/cryanim/command.rs:130 | ported from the decompile; no live golden found |
| `31557af0` | fun_31557af0 (crates/aa-client/src/port/cryanim/command.rs:76) | crates/aa-client/src/port/cryanim/command.rs:76 | ported from the decompile; no live golden found |
| `31559430` | fun_31556910 (crates/aa-client/src/port/cryanim/blend.rs:212) | crates/aa-client/src/port/cryanim/blend.rs:212 | ported from the decompile; no live golden found |

#### EXACT+UNWIRED (10)

| address | Rust function | source | proof / why |
|---|---|---|---|
| `3154dd70` | copy_rider_asset_block (crates/aa-client/src/port/cryanim/rider.rs:33); fun_3154dd70 (crates/aa-client/src/port/cryanim/rider.rs:19) | crates/aa-client/src/port/cryanim/rider.rs:19 | exact, but not wired; goldens: rider_live.json; tests: rider_sync_noop_matches_live_original |
| `31554660` | fun_31554660 (crates/aa-client/src/port/cryanim/layer.rs:35) | crates/aa-client/src/port/cryanim/layer.rs:35 | exact, but not wired; goldens: layer_setter.json; tests: layer_command_multiplier_matches_live_original; layer_update_multiplier_matches_live_original |
| `315546a0` | fun_315546a0 (crates/aa-client/src/port/cryanim/layer.rs:40) | crates/aa-client/src/port/cryanim/layer.rs:40 | exact, but not wired; goldens: layer_setter.json; tests: layer_command_multiplier_matches_live_original; layer_update_multiplier_matches_live_original |
| `315547a0` | fun_315547a0 (crates/aa-client/src/port/cryanim/blend.rs:57); normalize_curve (crates/aa-client/src/port/cryanim/blend.rs:49) | crates/aa-client/src/port/cryanim/blend.rs:49 | exact, but not wired; goldens: playback_live.json; tests: command_writer_matches_live_original; layer_ramp_matches_live_original; normalize_matches_live_original; segment_average_matches_live_original; speed_formula_matches_live_original; time_advance_matches_live_original; transition_ramp_matches_live_original |
| `31554ba0` | fun_31554ba0 (crates/aa-client/src/port/cryanim/blend.rs:21) | crates/aa-client/src/port/cryanim/blend.rs:21 | exact, but not wired; goldens: playback_live.json; tests: command_writer_matches_live_original; layer_ramp_matches_live_original; normalize_matches_live_original; segment_average_matches_live_original; speed_formula_matches_live_original; time_advance_matches_live_original; transition_ramp_matches_live_original |
| `31554e80` | fun_31554e80 (crates/aa-client/src/port/cryanim/blend.rs:87) | crates/aa-client/src/port/cryanim/blend.rs:87 | exact, but not wired; goldens: playback_live.json; tests: command_writer_matches_live_original; layer_ramp_matches_live_original; normalize_matches_live_original; segment_average_matches_live_original; speed_formula_matches_live_original; time_advance_matches_live_original; transition_ramp_matches_live_original |
| `31555070` | fun_31555070 (crates/aa-client/src/port/cryanim/blend.rs:136) | crates/aa-client/src/port/cryanim/blend.rs:136 | exact, but not wired; goldens: playback_live.json; tests: command_writer_matches_live_original; layer_ramp_matches_live_original; normalize_matches_live_original; segment_average_matches_live_original; speed_formula_matches_live_original; time_advance_matches_live_original; transition_ramp_matches_live_original |
| `31557cb0` | fun_31557cb0 (crates/aa-client/src/port/cryanim/command.rs:167) | crates/aa-client/src/port/cryanim/command.rs:167 | exact, but not wired; goldens: command_live.json; playback_live.json; tests: command_writer_matches_live_original; layer_ramp_matches_live_original; normalize_matches_live_original; segment_average_matches_live_original; speed_formula_matches_live_original; time_advance_matches_live_original; transition_ramp_matches_live_original |
| `3155ae00` | fun_3155ae00 (crates/aa-client/src/port/cryanim/start.rs:106); fun_3155b350 (crates/aa-client/src/port/cryanim/start.rs:111); insert (crates/aa-client/src/port/cryanim/start.rs:65) | crates/aa-client/src/port/cryanim/start.rs:65 | exact, but not wired; goldens: insertions.json; tests: insertion_pairs_replay_live_fifo_growth |
| `3155b350` | fun_3155b350 (crates/aa-client/src/port/cryanim/start.rs:111) | crates/aa-client/src/port/cryanim/start.rs:111 | exact, but not wired; goldens: insertions.json; tests: insertion_pairs_replay_live_fifo_growth |

11 reference-only addresses are listed in the CSV; they are original
functions named by comments that this tree does not implement.

### E6: `combat_anim`

56 addresses, 43 ported, 11 done, 8 ported but unwired.

#### DECOMPILE (32)

| address | Rust function | source | proof / why |
|---|---|---|---|
| `390bb4e0` | fun_390bb4e0 (crates/aa-client/src/port/combat_anim/hit.rs:19) | crates/aa-client/src/port/combat_anim/hit.rs:19 | ported from the decompile; no live golden found |
| `39153af0` | front_back_variant (crates/aa-client/src/port/combat_anim/entry.rs:188) | crates/aa-client/src/port/combat_anim/entry.rs:188 | ported from the decompile; no live golden found |
| `39153ef0` | transition_kind (crates/aa-client/src/port/combat_anim/entry.rs:154) | crates/aa-client/src/port/combat_anim/entry.rs:154 | ported from the decompile; no live golden found |
| `39153f80` | transition_speed (crates/aa-client/src/port/combat_anim/entry.rs:169) | crates/aa-client/src/port/combat_anim/entry.rs:169 | ported from the decompile; no live golden found |
| `39154550` | layer_ramp (crates/aa-client/src/port/combat_anim/scheduler.rs:547) | crates/aa-client/src/port/combat_anim/scheduler.rs:547 | ported from the decompile; no live golden found |
| `391545e0` | attack_speed_keys (crates/aa-client/src/port/combat_anim/scheduler.rs:518) | crates/aa-client/src/port/combat_anim/scheduler.rs:518 | ported from the decompile; no live golden found |
| `39154a50` | new (crates/aa-client/src/port/combat_anim/entry.rs:242); reaction_entry (crates/aa-client/src/port/combat_anim/hit.rs:90) | crates/aa-client/src/port/combat_anim/entry.rs:242 | ported from the decompile; no live golden found |
| `39154ad0` | build_action (crates/aa-client/src/port/combat_anim/scheduler.rs:563) | crates/aa-client/src/port/combat_anim/scheduler.rs:563 | ported from the decompile; no live golden found |
| `39154ea0` | idle_fixup (crates/aa-client/src/port/combat_anim/scheduler.rs:530) | crates/aa-client/src/port/combat_anim/scheduler.rs:530 | ported from the decompile; no live golden found |
| `39155560` | pick_current (crates/aa-client/src/port/combat_anim/scheduler.rs:350) | crates/aa-client/src/port/combat_anim/scheduler.rs:350 | ported from the decompile; no live golden found |
| `391555d0` | push_action_id (crates/aa-client/src/port/combat_anim/scheduler.rs:483) | crates/aa-client/src/port/combat_anim/scheduler.rs:483 | ported from the decompile; no live golden found |
| `39155660` | event_pump (crates/aa-client/src/port/combat_anim/events.rs:49) | crates/aa-client/src/port/combat_anim/events.rs:49 | ported from the decompile; no live golden found |
| `39155930` | end_events (crates/aa-client/src/port/combat_anim/events.rs:84) | crates/aa-client/src/port/combat_anim/events.rs:84 | ported from the decompile; no live golden found |
| `39155b70` | clear_list (crates/aa-client/src/port/combat_anim/scheduler.rs:330) | crates/aa-client/src/port/combat_anim/scheduler.rs:330 | ported from the decompile; no live golden found |
| `39155d50` | by_name (crates/aa-client/src/port/combat_anim/entry.rs:140) | crates/aa-client/src/port/combat_anim/entry.rs:140 | ported from the decompile; no live golden found |
| `39156160` | force_apply (crates/aa-client/src/port/combat_anim/scheduler.rs:499) | crates/aa-client/src/port/combat_anim/scheduler.rs:499 | ported from the decompile; no live golden found |
| `39156310` | by_id (crates/aa-client/src/port/combat_anim/entry.rs:135); front_back_variant (crates/aa-client/src/port/combat_anim/entry.rs:188) | crates/aa-client/src/port/combat_anim/entry.rs:135 | ported from the decompile; no live golden found |
| `391564e0` | overlay (crates/aa-client/src/port/combat_anim/entry.rs:148) | crates/aa-client/src/port/combat_anim/entry.rs:148 | ported from the decompile; no live golden found |
| `39156760` | stop_anim (crates/aa-client/src/port/combat_anim/scheduler.rs:309) | crates/aa-client/src/port/combat_anim/scheduler.rs:309 | ported from the decompile; no live golden found |
| `39156e00` | push_entry (crates/aa-client/src/port/combat_anim/scheduler.rs:248) | crates/aa-client/src/port/combat_anim/scheduler.rs:248 | ported from the decompile; no live golden found |
| `391572f0` | overlay_play (crates/aa-client/src/port/combat_anim/scheduler.rs:273) | crates/aa-client/src/port/combat_anim/scheduler.rs:273 | ported from the decompile; no live golden found |
| `3915b040` | fun_3915b040 (crates/aa-client/src/port/combat_anim/camera.rs:117) | crates/aa-client/src/port/combat_anim/camera.rs:117 | ported from the decompile; no live golden found |
| `39354840` | detach_attachment (crates/aa-client/src/port/combat_anim/weapon.rs:136) | crates/aa-client/src/port/combat_anim/weapon.rs:136 | ported from the decompile; no live golden found |
| `3935a690` | weapon_class (crates/aa-client/src/port/combat_anim/weapon.rs:53) | crates/aa-client/src/port/combat_anim/weapon.rs:53 | ported from the decompile; no live golden found |
| `3935a780` | build_visual_slots (crates/aa-client/src/port/combat_anim/weapon.rs:112) | crates/aa-client/src/port/combat_anim/weapon.rs:112 | ported from the decompile; no live golden found |
| `39360780` | update_unit_weapon_visuals (crates/aa-client/src/port/combat_anim/weapon.rs:101) | crates/aa-client/src/port/combat_anim/weapon.rs:101 | ported from the decompile; no live golden found |
| `393609a0` | change_unit_weapon_visual (crates/aa-client/src/port/combat_anim/weapon.rs:124); detach_attachment (crates/aa-client/src/port/combat_anim/weapon.rs:136) | crates/aa-client/src/port/combat_anim/weapon.rs:124 | ported from the decompile; no live golden found |
| `393953a0` | instance_ref (crates/aa-client/src/port/combat_anim/scheduler.rs:572) | crates/aa-client/src/port/combat_anim/scheduler.rs:572 | ported from the decompile; no live golden found |
| `39395440` | release_instance (crates/aa-client/src/port/combat_anim/scheduler.rs:575) | crates/aa-client/src/port/combat_anim/scheduler.rs:575 | ported from the decompile; no live golden found |
| `393ee930` | fun_393ee930 (crates/aa-client/src/port/combat_anim/projectile.rs:80) | crates/aa-client/src/port/combat_anim/projectile.rs:80 | ported from the decompile; no live golden found |
| `393eeb10` | fun_393eeb10 (crates/aa-client/src/port/combat_anim/projectile.rs:128) | crates/aa-client/src/port/combat_anim/projectile.rs:128 | ported from the decompile; no live golden found |
| `393f0a50` | fun_393f0a50 (crates/aa-client/src/port/combat_anim/projectile.rs:58) | crates/aa-client/src/port/combat_anim/projectile.rs:58 | ported from the decompile; no live golden found |

13 reference-only addresses are listed in the CSV; they are original
functions named by comments that this tree does not implement.

### S1: `combat`

124 addresses, 64 ported, 12 done, 42 ported but unwired.

#### TOLERANCE (1)

| address | Rust function | source | proof / why |
|---|---|---|---|
| `393672c0` | collect_candidates (crates/aa-client/src/port/combat/targeting.rs:195) | crates/aa-client/src/port/combat/targeting.rs:195 | tolerance: 1e-3; goldens: targeting_collect.json; tests: targeting_candidates_match_goldens |

#### APPROX (1)

| address | Rust function | source | proof / why |
|---|---|---|---|
| `393989e0` | key_skill (crates/aa-client/src/port/combat/bar.rs:105); use_skill (crates/aa-client/src/port/combat/front.rs:413) | crates/aa-client/src/port/combat/bar.rs:105 | the request body is a caller-supplied predicate standing in for the un-ported split tail (port/combat/front.rs); goldens: front.json; live_keys.json; tests: action_bar_keys_match_the_live_dispatch; front_matches_live_goldens |

#### DECOMPILE (20)

| address | Rust function | source | proof / why |
|---|---|---|---|
| `39361bf0` | tab_select (crates/aa-client/src/port/combat/targeting.rs:333) | crates/aa-client/src/port/combat/targeting.rs:333 | ported from the decompile; no live golden found |
| `39363ec0` | record (crates/aa-client/src/port/combat/targeting.rs:129) | crates/aa-client/src/port/combat/targeting.rs:129 | ported from the decompile; no live golden found |
| `39363f60` | previous (crates/aa-client/src/port/combat/targeting.rs:141) | crates/aa-client/src/port/combat/targeting.rs:141 | ported from the decompile; no live golden found |
| `3938c2e0` | unit_casting_info (crates/aa-client/src/port/combat/cast.rs:102) | crates/aa-client/src/port/combat/cast.rs:102 | ported from the decompile; no live golden found |
| `39390900` | packed (crates/aa-client/src/port/combat/front.rs:191) | crates/aa-client/src/port/combat/front.rs:191 | ported from the decompile; no live golden found |
| `39390f20` | can_toggle_force_attack (crates/aa-client/src/port/combat/targeting.rs:516) | crates/aa-client/src/port/combat/targeting.rs:516 | ported from the decompile; no live golden found |
| `393925c0` | target_override (crates/aa-client/src/port/combat/front.rs:364) | crates/aa-client/src/port/combat/front.rs:364 | ported from the decompile; no live golden found |
| `393be4e0` | auto_attack_gate (crates/aa-client/src/port/combat/front.rs:323) | crates/aa-client/src/port/combat/front.rs:323 | ported from the decompile; no live golden found |
| `393fc4a0` | bar_key_slot (crates/aa-client/src/port/combat/bar.rs:89) | crates/aa-client/src/port/combat/bar.rs:89 | ported from the decompile; no live golden found |
| `39897990` | result_name (crates/aa-client/src/port/combat/front.rs:37) | crates/aa-client/src/port/combat/front.rs:37 | ported from the decompile; no live golden found |
| `39899d10` | reset (crates/aa-client/src/port/combat/cooldown.rs:138) | crates/aa-client/src/port/combat/cooldown.rs:138 | ported from the decompile; no live golden found |
| `39899d40` | append (crates/aa-client/src/port/combat/cooldown.rs:149) | crates/aa-client/src/port/combat/cooldown.rs:149 | ported from the decompile; no live golden found |
| `39899e50` | tick (crates/aa-client/src/port/combat/cooldown.rs:194) | crates/aa-client/src/port/combat/cooldown.rs:194 | ported from the decompile; no live golden found |
| `39899ec0` | clear_skill (crates/aa-client/src/port/combat/cooldown.rs:207) | crates/aa-client/src/port/combat/cooldown.rs:207 | ported from the decompile; no live golden found |
| `39899f00` | clear_tag (crates/aa-client/src/port/combat/cooldown.rs:215) | crates/aa-client/src/port/combat/cooldown.rs:215 | ported from the decompile; no live golden found |
| `39899f90` | insert (crates/aa-client/src/port/combat/cooldown.rs:228) | crates/aa-client/src/port/combat/cooldown.rs:228 | ported from the decompile; no live golden found |
| `3989a290` | query (crates/aa-client/src/port/combat/cooldown.rs:260) | crates/aa-client/src/port/combat/cooldown.rs:260 | ported from the decompile; no live golden found |
| `3989a3b0` | is_on_cooldown (crates/aa-client/src/port/combat/cooldown.rs:288) | crates/aa-client/src/port/combat/cooldown.rs:288 | ported from the decompile; no live golden found |
| `3989a3e0` | global_pair (crates/aa-client/src/port/combat/cooldown.rs:293) | crates/aa-client/src/port/combat/cooldown.rs:293 | ported from the decompile; no live golden found |
| `3989a400` | state (crates/aa-client/src/port/combat/cooldown.rs:298) | crates/aa-client/src/port/combat/cooldown.rs:298 | ported from the decompile; no live golden found |

#### EXACT+UNWIRED (31)

| address | Rust function | source | proof / why |
|---|---|---|---|
| `390b3710` | clear (crates/aa-client/src/port/combat/targeting.rs:458) | crates/aa-client/src/port/combat/targeting.rs:458 | exact, but not wired; goldens: combat_state.json; tests: combat_state_matches_goldens |
| `3938a3c0` | apply_buff (crates/aa-client/src/port/combat/presentation.rs:499) | crates/aa-client/src/port/combat/presentation.rs:499 | exact, but not wired; goldens: buff_apply.json; tests: buff_apply_matches_live_goldens |
| `393917f0` | reaction (crates/aa-client/src/port/combat/reaction.rs:43) | crates/aa-client/src/port/combat/reaction.rs:43 | exact, but not wired; goldens: reaction.json; tests: reaction_matches_goldens |
| `39391950` | big_hit (crates/aa-client/src/port/combat/reaction.rs:89) | crates/aa-client/src/port/combat/reaction.rs:89 | exact, but not wired; goldens: bighit.json; tests: big_hit_matches_goldens |
| `39391da0` | caster_eq (crates/aa-client/src/port/combat/equality.rs:56) | crates/aa-client/src/port/combat/equality.rs:56 | exact, but not wired; goldens: equality.json; tests: equality_matches_goldens |
| `39391e30` | target_eq (crates/aa-client/src/port/combat/equality.rs:83) | crates/aa-client/src/port/combat/equality.rs:83 | exact, but not wired; goldens: equality.json; tests: equality_matches_goldens |
| `39393f30` | party_last_hitter (crates/aa-client/src/port/combat/presentation.rs:392) | crates/aa-client/src/port/combat/presentation.rs:392 | exact, but not wired; goldens: party_tracker.json; tests: party_tracker_matches_live_goldens |
| `39395510` | cast_progress (crates/aa-client/src/port/combat/cast.rs:171) | crates/aa-client/src/port/combat/cast.rs:171 | exact, but not wired; goldens: cast.json; tests: cast_queries_match_goldens |
| `39395760` | casting_delayed (crates/aa-client/src/port/combat/timeline.rs:448) | crates/aa-client/src/port/combat/timeline.rs:448 | exact, but not wired; goldens: timeline.json; timeline_wide.json; tests: timeline_matches_goldens; wide_timeline_matches_live_goldens |
| `39395b50` | stop_casting (crates/aa-client/src/port/combat/timeline.rs:400) | crates/aa-client/src/port/combat/timeline.rs:400 | exact, but not wired; goldens: timeline.json; timeline_wide.json; tests: timeline_matches_goldens; wide_timeline_matches_live_goldens |
| `39395d90` | can_cast (crates/aa-client/src/port/combat/cast.rs:223) | crates/aa-client/src/port/combat/cast.rs:223 | exact, but not wired; goldens: cast.json; tests: cast_queries_match_goldens |
| `39395e40` | can_channel (crates/aa-client/src/port/combat/cast.rs:238) | crates/aa-client/src/port/combat/cast.rs:238 | exact, but not wired; goldens: cast.json; tests: cast_queries_match_goldens |
| `39396000` | casting_stopped (crates/aa-client/src/port/combat/timeline.rs:425) | crates/aa-client/src/port/combat/timeline.rs:425 | exact, but not wired; goldens: timeline.json; timeline_wide.json; tests: timeline_matches_goldens; wide_timeline_matches_live_goldens |
| `393968b0` | aggro_link (crates/aa-client/src/port/combat/presentation.rs:359) | crates/aa-client/src/port/combat/presentation.rs:359 | exact, but not wired; goldens: aggro_link.json; tests: aggro_link_matches_live_goldens |
| `39397ff0` | end_skill (crates/aa-client/src/port/combat/timeline.rs:366) | crates/aa-client/src/port/combat/timeline.rs:366 | exact, but not wired; goldens: timeline.json; timeline_wide.json; tests: timeline_matches_goldens; wide_timeline_matches_live_goldens |
| `39398410` | stop_skill (crates/aa-client/src/port/combat/timeline.rs:378) | crates/aa-client/src/port/combat/timeline.rs:378 | exact, but not wired; goldens: stop.json; timeline.json; tests: stop_matches_live_goldens; timeline_matches_goldens |
| `39398c80` | damage_presentation (crates/aa-client/src/port/combat/presentation.rs:171) | crates/aa-client/src/port/combat/presentation.rs:171 | exact, but not wired; goldens: damage_dispatch.json; tests: damage_dispatch_matches_live_goldens |
| `393992d0` | heal_presentation (crates/aa-client/src/port/combat/presentation.rs:258) | crates/aa-client/src/port/combat/presentation.rs:258 | exact, but not wired; goldens: heal_dispatch.json; tests: heal_dispatch_matches_live_goldens |
| `3939a810` | start_skill (crates/aa-client/src/port/combat/timeline.rs:240) | crates/aa-client/src/port/combat/timeline.rs:240 | exact, but not wired; goldens: timeline.json; timeline_wide.json; tests: timeline_matches_goldens; wide_timeline_matches_live_goldens |
| `3939b010` | fire_skill (crates/aa-client/src/port/combat/timeline.rs:310) | crates/aa-client/src/port/combat/timeline.rs:310 | exact, but not wired; goldens: timeline.json; timeline_wide.json; tests: timeline_matches_goldens; wide_timeline_matches_live_goldens |
| `3939bfd0` | update_client_only (crates/aa-client/src/port/combat/timeline.rs:602); update_client_only_with (crates/aa-client/src/port/combat/timeline.rs:557) | crates/aa-client/src/port/combat/timeline.rs:557 | exact, but not wired; goldens: client_only.json; tests: client_only_matches_live_goldens |
| `3939cb80` | combo_update (crates/aa-client/src/port/combat/timeline.rs:525) | crates/aa-client/src/port/combat/timeline.rs:525 | exact, but not wired; goldens: combo.json; tests: combo_add_matches_live_goldens; combo_update_matches_live_goldens |
| `3939ce90` | combo_add (crates/aa-client/src/port/combat/timeline.rs:465) | crates/aa-client/src/port/combat/timeline.rs:465 | exact, but not wired; goldens: combo.json; tests: combo_add_matches_live_goldens; combo_update_matches_live_goldens |
| `393f61c0` | slot_cooldowns (crates/aa-client/src/port/combat/slot.rs:56) | crates/aa-client/src/port/combat/slot.rs:56 | exact, but not wired; goldens: slot.json; tests: slot_matches_goldens |
| `3943f310` | get_action_cooldown (crates/aa-client/src/port/combat/slot.rs:92) | crates/aa-client/src/port/combat/slot.rs:92 | exact, but not wired; goldens: slot.json; tests: slot_matches_goldens |
| `3943f420` | get_action_cooldown_ex (crates/aa-client/src/port/combat/slot.rs:103) | crates/aa-client/src/port/combat/slot.rs:103 | exact, but not wired; goldens: slot.json; tests: slot_matches_goldens |
| `3945fcd0` | parse_token (crates/aa-client/src/port/combat/token.rs:95) | crates/aa-client/src/port/combat/token.rs:95 | exact, but not wired; goldens: token.json; tests: token_matches_goldens |
| `394bcf60` | unit_casting_info (crates/aa-client/src/port/combat/cast.rs:102) | crates/aa-client/src/port/combat/cast.rs:102 | exact, but not wired; goldens: cast.json; tests: cast_queries_match_goldens |
| `394bd830` | unit_combat_state (crates/aa-client/src/port/combat/cast.rs:250) | crates/aa-client/src/port/combat/cast.rs:250 | exact, but not wired; goldens: cast.json; tests: cast_queries_match_goldens |
| `39899d00` | clear_global (crates/aa-client/src/port/combat/cooldown.rs:131) | crates/aa-client/src/port/combat/cooldown.rs:131 | exact, but not wired; goldens: cooldown.json; tests: cooldown_matches_goldens |
| `3989a450` | remaining (crates/aa-client/src/port/combat/cooldown.rs:311) | crates/aa-client/src/port/combat/cooldown.rs:311 | exact, but not wired; goldens: cooldown.json; tests: cooldown_matches_goldens |

60 reference-only addresses are listed in the CSV; they are original
functions named by comments that this tree does not implement.

### S2: `interaction`

197 addresses, 78 ported, 5 done, 66 ported but unwired.

#### TOLERANCE (2)

| address | Rust function | source | proof / why |
|---|---|---|---|
| `39093800` | range_gate (crates/aa-client/src/port/interaction/range.rs:72) | crates/aa-client/src/port/interaction/range.rs:72 | tolerance: 1e-5; goldens: range.json; tests: range_gates_match_live_distances |
| `39093860` | range_gate_squared (crates/aa-client/src/port/interaction/range.rs:84) | crates/aa-client/src/port/interaction/range.rs:84 | tolerance: 1e-5; goldens: range.json; tests: range_gates_match_live_distances |

#### DECOMPILE (61)

| address | Rust function | source | proof / why |
|---|---|---|---|
| `3902e190` | coffer_interaction_body (crates/aa-client/src/port/interaction/doodad.rs:327) | crates/aa-client/src/port/interaction/doodad.rs:327 | ported from the decompile; no live golden found |
| `390f3780` | default_interaction (crates/aa-client/src/port/interaction/click.rs:74) | crates/aa-client/src/port/interaction/click.rs:74 | live capture but no test loads click_live.json; ported from the decompile; no live golden found; goldens: click_live.json |
| `390f4960` | click (crates/aa-client/src/port/interaction/click.rs:187) | crates/aa-client/src/port/interaction/click.rs:187 | live capture but no test loads click_live.json; ported from the decompile; no live golden found; goldens: click_live.json |
| `3921ad60` | refresh_actions (crates/aa-client/src/port/interaction/bar.rs:102) | crates/aa-client/src/port/interaction/bar.rs:102 | ported from the decompile; no live golden found |
| `3932d3a0` | build_slave_candidates (crates/aa-client/src/port/interaction/builders.rs:343) | crates/aa-client/src/port/interaction/builders.rs:343 | ported from the decompile; no live golden found |
| `393b1da0` | skill_at (crates/aa-client/src/port/interaction/bar.rs:155) | crates/aa-client/src/port/interaction/bar.rs:155 | ported from the decompile; no live golden found |
| `393b1de0` | execute_dynamic_action (crates/aa-client/src/port/interaction/action.rs:201) | crates/aa-client/src/port/interaction/action.rs:201 | ported from the decompile; no live golden found |
| `393b1eb0` | show (crates/aa-client/src/port/interaction/bar.rs:128) | crates/aa-client/src/port/interaction/bar.rs:128 | ported from the decompile; no live golden found |
| `393b23d0` | new (crates/aa-client/src/port/interaction/bar.rs:58) | crates/aa-client/src/port/interaction/bar.rs:58 | ported from the decompile; no live golden found |
| `393b2490` | set_unit (crates/aa-client/src/port/interaction/bar.rs:76) | crates/aa-client/src/port/interaction/bar.rs:76 | ported from the decompile; no live golden found |
| `393b24e0` | set_doodad (crates/aa-client/src/port/interaction/bar.rs:86) | crates/aa-client/src/port/interaction/bar.rs:86 | ported from the decompile; no live golden found |
| `393b2530` | reset (crates/aa-client/src/port/interaction/bar.rs:63) | crates/aa-client/src/port/interaction/bar.rs:63 | ported from the decompile; no live golden found |
| `393b25a0` | refresh_actions (crates/aa-client/src/port/interaction/bar.rs:102) | crates/aa-client/src/port/interaction/bar.rs:102 | live capture but no test loads barlive.json; ported from the decompile; no live golden found; goldens: barlive.json |
| `393b2760` | update (crates/aa-client/src/port/interaction/bar.rs:118) | crates/aa-client/src/port/interaction/bar.rs:118 | live capture but no test loads barlive.json; ported from the decompile; no live golden found; goldens: barlive.json |
| `393b2c70` | rate_limit_ok (crates/aa-client/src/port/interaction/npc.rs:49) | crates/aa-client/src/port/interaction/npc.rs:49 | live capture but no test loads npc_rate.json; ported from the decompile; no live golden found; goldens: npc_rate.json |
| `393b3d50` | doodad_precheck (crates/aa-client/src/port/interaction/doodad.rs:94) | crates/aa-client/src/port/interaction/doodad.rs:94 | ported from the decompile; no live golden found |
| `393b3ec0` | source_loot (crates/aa-client/src/port/interaction/loot.rs:294); start_doodad_interaction (crates/aa-client/src/port/interaction/doodad.rs:205) | crates/aa-client/src/port/interaction/doodad.rs:205 | ported from the decompile; no live golden found |
| `393b79a0` | doodad_default_skill (crates/aa-client/src/port/interaction/click.rs:239) | crates/aa-client/src/port/interaction/click.rs:239 | ported from the decompile; no live golden found |
| `393b7aa0` | close_current_state (crates/aa-client/src/port/interaction/doodad.rs:415) | crates/aa-client/src/port/interaction/doodad.rs:415 | ported from the decompile; no live golden found |
| `393b7b10` | change_state (crates/aa-client/src/port/interaction/doodad.rs:391) | crates/aa-client/src/port/interaction/doodad.rs:391 | ported from the decompile; no live golden found |
| `393b7b50` | cancel_state (crates/aa-client/src/port/interaction/doodad.rs:403) | crates/aa-client/src/port/interaction/doodad.rs:403 | ported from the decompile; no live golden found |
| `393b8240` | change_doodad_phase (crates/aa-client/src/port/interaction/doodad.rs:344) | crates/aa-client/src/port/interaction/doodad.rs:344 | ported from the decompile; no live golden found |
| `393bb420` | add_descriptor_services (crates/aa-client/src/port/interaction/builders.rs:191) | crates/aa-client/src/port/interaction/builders.rs:191 | ported from the decompile; no live golden found |
| `393bb910` | add_npc_services (crates/aa-client/src/port/interaction/builders.rs:246) | crates/aa-client/src/port/interaction/builders.rs:246 | ported from the decompile; no live golden found |
| `393bbb80` | build_container_candidates (crates/aa-client/src/port/interaction/builders.rs:388) | crates/aa-client/src/port/interaction/builders.rs:388 | ported from the decompile; no live golden found |
| `393bbc90` | build_mate_candidates (crates/aa-client/src/port/interaction/builders.rs:351) | crates/aa-client/src/port/interaction/builders.rs:351 | ported from the decompile; no live golden found |
| `393bbd70` | build_house_candidates (crates/aa-client/src/port/interaction/builders.rs:377) | crates/aa-client/src/port/interaction/builders.rs:377 | ported from the decompile; no live golden found |
| `393bbf10` | build_slave_candidates (crates/aa-client/src/port/interaction/builders.rs:343) | crates/aa-client/src/port/interaction/builders.rs:343 | ported from the decompile; no live golden found |
| `393bc7e0` | bar_loot (crates/aa-client/src/port/interaction/loot.rs:268) | crates/aa-client/src/port/interaction/loot.rs:268 | live capture but no test loads loot_f.json; ported from the decompile; no live golden found; goldens: loot_f.json |
| `393bc8c0` | source_loot (crates/aa-client/src/port/interaction/loot.rs:294) | crates/aa-client/src/port/interaction/loot.rs:294 | live capture but no test loads loot_f.json; ported from the decompile; no live golden found; goldens: loot_f.json |
| `393bd1b0` | append_source (crates/aa-client/src/port/interaction/bar.rs:96) | crates/aa-client/src/port/interaction/bar.rs:96 | ported from the decompile; no live golden found |
| `393be0e0` | service_access_refusal (crates/aa-client/src/port/interaction/npc.rs:57) | crates/aa-client/src/port/interaction/npc.rs:57 | live capture but no test loads npc_special.json; ported from the decompile; no live golden found; goldens: npc_special.json |
| `393be180` | cancel_npc_interaction (crates/aa-client/src/port/interaction/npc.rs:544) | crates/aa-client/src/port/interaction/npc.rs:544 | ported from the decompile; no live golden found |
| `393be250` | apply_status_changed (crates/aa-client/src/port/interaction/npc.rs:305) | crates/aa-client/src/port/interaction/npc.rs:305 | ported from the decompile; no live golden found |
| `393bed90` | pick_greeting (crates/aa-client/src/port/interaction/npc.rs:290) | crates/aa-client/src/port/interaction/npc.rs:290 | live capture but no test loads npc_greeting.json; ported from the decompile; no live golden found; goldens: npc_greeting.json |
| `393bf270` | npc_range_gate (crates/aa-client/src/port/interaction/range.rs:109) | crates/aa-client/src/port/interaction/range.rs:109 | live capture but no test loads npc_range.json; ported from the decompile; no live golden found; goldens: npc_range.json |
| `393bf500` | quest_route (crates/aa-client/src/port/interaction/npc.rs:265) | crates/aa-client/src/port/interaction/npc.rs:265 | live capture but no test loads npc_quest.json; ported from the decompile; no live golden found; goldens: npc_quest.json |
| `393c01d0` | target_gate (crates/aa-client/src/port/interaction/npc.rs:93) | crates/aa-client/src/port/interaction/npc.rs:93 | if input.blocked_state {; live capture but no test loads npc_gates.json; goldens: npc_gates.json |
| `393c1310` | service_tag (crates/aa-client/src/port/interaction/npc.rs:172) | crates/aa-client/src/port/interaction/npc.rs:172 | ported from the decompile; no live golden found |
| `39424280` | payload (crates/aa-client/src/port/interaction/npc.rs:249) | crates/aa-client/src/port/interaction/npc.rs:249 | ported from the decompile; no live golden found |
| `39428370` | service_access_refusal (crates/aa-client/src/port/interaction/npc.rs:57) | crates/aa-client/src/port/interaction/npc.rs:57 | ported from the decompile; no live golden found |
| `39428d80` | cursor_ray (crates/aa-client/src/port/interaction/pick.rs:66) | crates/aa-client/src/port/interaction/pick.rs:66 | live capture but no test loads pick_ray.json; ported from the decompile; no live golden found; goldens: pick_ray.json |
| `39429d00` | gimmick_interaction (crates/aa-client/src/port/interaction/click.rs:244) | crates/aa-client/src/port/interaction/click.rs:244 | ported from the decompile; no live golden found |
| `3942c640` | update_picking (crates/aa-client/src/port/interaction/pick.rs:159) | crates/aa-client/src/port/interaction/pick.rs:159 | live capture but no test loads pick_update.json; ported from the decompile; no live golden found; goldens: pick_update.json |
| `39442370` | action_count (crates/aa-client/src/port/interaction/bar.rs:150) | crates/aa-client/src/port/interaction/bar.rs:150 | ported from the decompile; no live golden found |
| `39442400` | skill_at_1based (crates/aa-client/src/port/interaction/bar.rs:160) | crates/aa-client/src/port/interaction/bar.rs:160 | ported from the decompile; no live golden found |
| `3947e060` | is_visible (crates/aa-client/src/port/interaction/bar.rs:168) | crates/aa-client/src/port/interaction/bar.rs:168 | ported from the decompile; no live golden found |
| `394d7a40` | build_house_candidates (crates/aa-client/src/port/interaction/builders.rs:377) | crates/aa-client/src/port/interaction/builders.rs:377 | ported from the decompile; no live golden found |
| `394f9360` | row_count (crates/aa-client/src/port/interaction/loot.rs:57) | crates/aa-client/src/port/interaction/loot.rs:57 | ported from the decompile; no live golden found |
| `394f9620` | find_lootable (crates/aa-client/src/port/interaction/loot.rs:62) | crates/aa-client/src/port/interaction/loot.rs:62 | ported from the decompile; no live golden found |
| `394f9790` | begin_take (crates/aa-client/src/port/interaction/loot.rs:119); take_request (crates/aa-client/src/port/interaction/loot.rs:208) | crates/aa-client/src/port/interaction/loot.rs:119 | live capture but no test loads loot_take.json; ported from the decompile; no live golden found; goldens: loot_take.json |
| `394f9c20` | failed (crates/aa-client/src/port/interaction/loot.rs:104) | crates/aa-client/src/port/interaction/loot.rs:104 | ported from the decompile; no live golden found |
| `394fa110` | mark_lootable (crates/aa-client/src/port/interaction/loot.rs:68) | crates/aa-client/src/port/interaction/loot.rs:68 | ported from the decompile; no live golden found |
| `394fa6c0` | took (crates/aa-client/src/port/interaction/loot.rs:85) | crates/aa-client/src/port/interaction/loot.rs:85 | ported from the decompile; no live golden found |
| `394fab30` | close_bag (crates/aa-client/src/port/interaction/loot.rs:222) | crates/aa-client/src/port/interaction/loot.rs:222 | live capture but no test loads loot_close.json; ported from the decompile; no live golden found; goldens: loot_close.json |
| `394faf40` | unmark_lootable (crates/aa-client/src/port/interaction/loot.rs:74) | crates/aa-client/src/port/interaction/loot.rs:74 | ported from the decompile; no live golden found |
| `394fb010` | open_loot_bag (crates/aa-client/src/port/interaction/loot.rs:173) | crates/aa-client/src/port/interaction/loot.rs:173 | live capture but no test loads loot_open.json; ported from the decompile; no live golden found; goldens: loot_open.json |
| `394fb160` | open_loot_bag (crates/aa-client/src/port/interaction/loot.rs:173) | crates/aa-client/src/port/interaction/loot.rs:173 | live capture but no test loads loot_open.json; ported from the decompile; no live golden found; goldens: loot_open.json |
| `397ac0f0` | coffer_interaction_body (crates/aa-client/src/port/interaction/doodad.rs:327) | crates/aa-client/src/port/interaction/doodad.rs:327 | ported from the decompile; no live golden found |
| `3989bf90` | min_surface_gap (crates/aa-client/src/port/interaction/range.rs:57) | crates/aa-client/src/port/interaction/range.rs:57 | ported from the decompile; no live golden found |
| `398b1ab0` | gap (crates/aa-client/src/port/interaction/range.rs:25) | crates/aa-client/src/port/interaction/range.rs:25 | ported from the decompile; no live golden found |

#### EXACT+UNWIRED (10)

| address | Rust function | source | proof / why |
|---|---|---|---|
| `393b2b70` | do_interaction_index (crates/aa-client/src/port/interaction/action.rs:147) | crates/aa-client/src/port/interaction/action.rs:147 | exact, but not wired; goldens: stubs.json; tests: do_interaction_stubs_pass_their_index |
| `393b3300` | check_permission (crates/aa-client/src/port/interaction/permission.rs:59) | crates/aa-client/src/port/interaction/permission.rs:59 | exact, but not wired; goldens: permission.json; tests: permission_switch_matches_live_doodads |
| `393b6e90` | insert_replace (crates/aa-client/src/port/interaction/candidates.rs:40) | crates/aa-client/src/port/interaction/candidates.rs:40 | exact, but not wired; goldens: candidates.json; tests: candidate_set_is_ascending_and_deduplicated |
| `393ba1b0` | insert (crates/aa-client/src/port/interaction/candidates.rs:30) | crates/aa-client/src/port/interaction/candidates.rs:30 | exact, but not wired; goldens: candidates.json; candidates2.json; tests: candidate_set_is_ascending_and_deduplicated |
| `393ba370` | lookup (crates/aa-client/src/port/interaction/candidates.rs:45) | crates/aa-client/src/port/interaction/candidates.rs:45 | exact, but not wired; goldens: candidates.json; candidates2.json; tests: candidate_set_is_ascending_and_deduplicated |
| `393bc260` | decide (crates/aa-client/src/port/interaction/functype.rs:122) | crates/aa-client/src/port/interaction/functype.rs:122 | exact, but not wired; goldens: functype.json; tests: functype_switch_matches_the_live_default_case |
| `393bcf00` | build_unit_candidates (crates/aa-client/src/port/interaction/builders.rs:420) | crates/aa-client/src/port/interaction/builders.rs:420 | exact, but not wired; goldens: builders.json; tests: doodad_builder_candidates_match_the_live_set; npc_builder_reproduces_the_live_greeting_only_npcs |
| `393c0ff0` | insert (crates/aa-client/src/port/interaction/candidates.rs:30) | crates/aa-client/src/port/interaction/candidates.rs:30 | exact, but not wired; goldens: candidates.json; tests: candidate_set_is_ascending_and_deduplicated |
| `393f8d20` | action_index (crates/aa-client/src/port/interaction/action.rs:138) | crates/aa-client/src/port/interaction/action.rs:138 | exact, but not wired; goldens: action_names.json; tests: action_names_match_the_original_table |
| `393f8d70` | action_handler (crates/aa-client/src/port/interaction/action.rs:156) | crates/aa-client/src/port/interaction/action.rs:156 | exact, but not wired; goldens: handlers.json; tests: handler_table_slots_match_the_live_process |

#### GOLDEN-ONLY (2)

| address | Rust function | source | proof / why |
|---|---|---|---|
| `393f8290` | (none) |  | goldens: stubs.json; tests: do_interaction_stubs_pass_their_index |
| `3a59f1c0` | (none) |  | goldens: handlers.json; tests: handler_table_slots_match_the_live_process |

117 reference-only addresses are listed in the CSV; they are original
functions named by comments that this tree does not implement.

### S3: `vehicles`

45 addresses, 35 ported, 0 done, 33 ported but unwired.

#### DECOMPILE (12)

| address | Rust function | source | proof / why |
|---|---|---|---|
| `3903bd80` | mat3x4_mul (crates/aa-client/src/port/vehicles/visual.rs:22) | crates/aa-client/src/port/vehicles/visual.rs:22 | ported from the decompile; no live golden found |
| `3909c660` | wheeled_physics_calls (crates/aa-client/src/port/vehicles/step.rs:27); wheeled_values (crates/aa-client/src/port/vehicles/step.rs:79); wheeled_warp (crates/aa-client/src/port/vehicles/step.rs:50) | crates/aa-client/src/port/vehicles/step.rs:27 | ported from the decompile; no live golden found |
| `390abe10` | new (crates/aa-client/src/port/vehicles/controller.rs:63) | crates/aa-client/src/port/vehicles/controller.rs:63 | ported from the decompile; no live golden found |
| `390ac300` | cry_physics_step (crates/aa-client/src/port/vehicles/step.rs:44) | crates/aa-client/src/port/vehicles/step.rs:44 | ported from the decompile; no live golden found |
| `390acee0` | step_branch (crates/aa-client/src/port/vehicles/controller.rs:124) | crates/aa-client/src/port/vehicles/controller.rs:124 | ported from the decompile; no live golden found |
| `390b0040` | blend_speed (crates/aa-client/src/port/vehicles/timeout.rs:50); clamp_dt (crates/aa-client/src/port/vehicles/timeout.rs:45); within_snap_distance (crates/aa-client/src/port/vehicles/timeout.rs:40) | crates/aa-client/src/port/vehicles/timeout.rs:40 | ported from the decompile; no live golden found |
| `390b2420` | net_update (crates/aa-client/src/port/vehicles/timeout.rs:28) | crates/aa-client/src/port/vehicles/timeout.rs:28 | ported from the decompile; no live golden found |
| `391d4d60` | flush_queue (crates/aa-client/src/port/vehicles/recv.rs:70) | crates/aa-client/src/port/vehicles/recv.rs:70 | ported from the decompile; no live golden found |
| `391e55c0` | queue_admits (crates/aa-client/src/port/vehicles/recv.rs:34); queue_batch (crates/aa-client/src/port/vehicles/recv.rs:47) | crates/aa-client/src/port/vehicles/recv.rs:34 | ported from the decompile; no live golden found |
| `397ad5a0` | round_half_floor_f32 (crates/aa-client/src/port/vehicles/codec.rs:68); write_i16_vec3 (crates/aa-client/src/port/vehicles/codec.rs:113) | crates/aa-client/src/port/vehicles/codec.rs:68 | ported from the decompile; no live golden found |
| `397ad8c0` | round_half_floor_f32 (crates/aa-client/src/port/vehicles/codec.rs:68); write_i16_vec3 (crates/aa-client/src/port/vehicles/codec.rs:113) | crates/aa-client/src/port/vehicles/codec.rs:68 | ported from the decompile; no live golden found |
| `397ada60` | round_half_floor_f32 (crates/aa-client/src/port/vehicles/codec.rs:68); write_i16_vec3 (crates/aa-client/src/port/vehicles/codec.rs:113) | crates/aa-client/src/port/vehicles/codec.rs:68 | ported from the decompile; no live golden found |

#### EXACT+UNWIRED (23)

| address | Rust function | source | proof / why |
|---|---|---|---|
| `39024610` | copy_move (crates/aa-client/src/port/vehicles/move_unit.rs:613) | crates/aa-client/src/port/vehicles/move_unit.rs:613 | exact, but not wired; goldens: move_copy.json; tests: move_copy_matches_goldens |
| `39094330` | empty_stub (crates/aa-client/src/port/vehicles/step.rs:41) | crates/aa-client/src/port/vehicles/step.rs:41 | exact, but not wired; goldens: stubs.json; tests: empty_vtable_leaves |
| `39094340` | copy12 (crates/aa-client/src/port/vehicles/math.rs:4) | crates/aa-client/src/port/vehicles/math.rs:4 | exact, but not wired; goldens: copy12.json; tests: copy12_matches_goldens |
| `390964e0` | reset (crates/aa-client/src/port/vehicles/controller.rs:75) | crates/aa-client/src/port/vehicles/controller.rs:75 | exact, but not wired; goldens: controller.json; tests: controller_activation_matches_goldens; controller_reset_matches_goldens |
| `390967c0` | velocity_filter (crates/aa-client/src/port/vehicles/math.rs:49) | crates/aa-client/src/port/vehicles/math.rs:49 | exact, but not wired; goldens: velocity_filter.json; tests: velocity_filter_matches_goldens |
| `390a3de0` | integrate_kinematic (crates/aa-client/src/port/vehicles/controller.rs:185) | crates/aa-client/src/port/vehicles/controller.rs:185 | exact, but not wired; goldens: integrate.json; tests: integrate_matches_goldens |
| `390af550` | empty_stub (crates/aa-client/src/port/vehicles/step.rs:41) | crates/aa-client/src/port/vehicles/step.rs:41 | exact, but not wired; goldens: stubs.json; tests: empty_vtable_leaves |
| `390b2e30` | move_moving (crates/aa-client/src/port/vehicles/apply.rs:129); wagon_pose_entry (crates/aa-client/src/port/vehicles/apply.rs:108) | crates/aa-client/src/port/vehicles/apply.rs:108 | exact, but not wired; goldens: apply.json; tests: wagon_apply_entry_matches_goldens |
| `390bf0f0` | build_basis (crates/aa-client/src/port/vehicles/math.rs:11) | crates/aa-client/src/port/vehicles/math.rs:11 | exact, but not wired; goldens: basis.json; tests: basis_matches_goldens |
| `390c1b80` | activation (crates/aa-client/src/port/vehicles/controller.rs:100) | crates/aa-client/src/port/vehicles/controller.rs:100 | exact, but not wired; goldens: controller.json; tests: controller_activation_matches_goldens; controller_reset_matches_goldens |
| `390c63c0` | wagon_pose_entry (crates/aa-client/src/port/vehicles/apply.rs:108); wagon_should_apply (crates/aa-client/src/port/vehicles/apply.rs:64) | crates/aa-client/src/port/vehicles/apply.rs:64 | exact, but not wired; goldens: apply.json; tests: wagon_apply_entry_matches_goldens |
| `390c7410` | rudder_transform (crates/aa-client/src/port/vehicles/visual.rs:41) | crates/aa-client/src/port/vehicles/visual.rs:41 | exact, but not wired; goldens: rudder.json; tests: rudder_matches_goldens |
| `390cf180` | build_descriptor (crates/aa-client/src/port/vehicles/model.rs:172); select_drive_radius (crates/aa-client/src/port/vehicles/model.rs:109) | crates/aa-client/src/port/vehicles/model.rs:109 | exact, but not wired; goldens: descriptor.json; tests: descriptor_matches_goldens |
| `390e8e40` | apply_attach (crates/aa-client/src/port/vehicles/helm.rs:86) | crates/aa-client/src/port/vehicles/helm.rs:86 | exact, but not wired; goldens: helm.json; tests: helm_matches_goldens |
| `3938b460` | read_quat_i8 (crates/aa-client/src/port/vehicles/codec.rs:243) | crates/aa-client/src/port/vehicles/codec.rs:243 | exact, but not wired; goldens: quat8_read.json; tests: quat8_read_matches_goldens |
| `394045f0` | local_unit_flag (crates/aa-client/src/port/vehicles/send.rs:86); pump_decision (crates/aa-client/src/port/vehicles/send.rs:51); pump_due (crates/aa-client/src/port/vehicles/send.rs:18) | crates/aa-client/src/port/vehicles/send.rs:18 | exact, but not wired; goldens: pump.json; tests: pump_matches_goldens |
| `3961de90` | fixed_to_world (crates/aa-client/src/port/vehicles/fixed.rs:27) | crates/aa-client/src/port/vehicles/fixed.rs:27 | exact, but not wired; goldens: fixed.json; tests: fixed_world_scale_matches_goldens |
| `3961e2c0` | axis_wire (crates/aa-client/src/port/vehicles/codec.rs:269); position_codec (crates/aa-client/src/port/vehicles/move_unit.rs:335); read_position (crates/aa-client/src/port/vehicles/codec.rs:309); write_position (crates/aa-client/src/port/vehicles/codec.rs:279) | crates/aa-client/src/port/vehicles/codec.rs:269 | exact, but not wired; goldens: position.json; tests: position_matches_goldens |
| `397ad740` | read_delta_i8_vec3 (crates/aa-client/src/port/vehicles/codec.rs:143); round_half_floor_f32 (crates/aa-client/src/port/vehicles/codec.rs:68); write_delta_i8_vec3 (crates/aa-client/src/port/vehicles/codec.rs:133) | crates/aa-client/src/port/vehicles/codec.rs:68 | exact, but not wired; goldens: delta.json; tests: delta_matches_goldens |
| `397adc00` | read_quat_i16 (crates/aa-client/src/port/vehicles/codec.rs:171) | crates/aa-client/src/port/vehicles/codec.rs:171 | exact, but not wired; goldens: quat_read.json; tests: quat_read_matches_goldens |
| `397adca0` | round_half_floor_f64 (crates/aa-client/src/port/vehicles/codec.rs:85); rsqrt_newton (crates/aa-client/src/port/vehicles/codec.rs:50); write_quat_i8 (crates/aa-client/src/port/vehicles/codec.rs:181) | crates/aa-client/src/port/vehicles/codec.rs:50 | exact, but not wired; goldens: quat8_write.json; tests: quat8_write_matches_goldens |
| `397ae020` | round_half_floor_f64 (crates/aa-client/src/port/vehicles/codec.rs:85); rsqrt_newton (crates/aa-client/src/port/vehicles/codec.rs:50); write_quat_i16 (crates/aa-client/src/port/vehicles/codec.rs:152) | crates/aa-client/src/port/vehicles/codec.rs:50 | exact, but not wired; goldens: quat_write.json; tests: quat_write_matches_goldens |
| `397c0470` | read_delta_i8_vec3 (crates/aa-client/src/port/vehicles/codec.rs:143); read_i16_vec3 (crates/aa-client/src/port/vehicles/codec.rs:123); serialize_move (crates/aa-client/src/port/vehicles/move_unit.rs:419) | crates/aa-client/src/port/vehicles/codec.rs:123 | exact, but not wired; goldens: move.json; tests: move_matches_goldens |

10 reference-only addresses are listed in the CSV; they are original
functions named by comments that this tree does not implement.

### S4: `housing`

104 addresses, 66 ported, 27 done, 20 ported but unwired.

#### TOLERANCE (1)

| address | Rust function | source | proof / why |
|---|---|---|---|
| `394298c0` | screen_ray (crates/aa-client/src/port/housing/ray.rs:115); screen_ray_bevy (crates/aa-client/src/port/housing/ray.rs:165) | crates/aa-client/src/port/housing/ray.rs:115 | tolerance: 0.5; 2e-3; goldens: phase_e_ray.json; tests: the_live_unprojection_matches_the_original |

#### DECOMPILE (27)

| address | Rust function | source | proof / why |
|---|---|---|---|
| `3811dff0` | invert_row_major (crates/aa-client/src/port/housing/ray.rs:72) | crates/aa-client/src/port/housing/ray.rs:72 | ported from the decompile; no live golden found |
| `3811f450` | multiply_row_major (crates/aa-client/src/port/housing/ray.rs:88) | crates/aa-client/src/port/housing/ray.rs:88 | ported from the decompile; no live golden found |
| `390367d0` | doodad_overlap (crates/aa-client/src/port/housing/seeds.rs:96) | crates/aa-client/src/port/housing/seeds.rs:96 | ported from the decompile; no live golden found |
| `3903bf60` | new (crates/aa-client/src/port/housing/decor.rs:126) | crates/aa-client/src/port/housing/decor.rs:126 | ported from the decompile; no live golden found |
| `3903da00` | surface_flag (crates/aa-client/src/port/housing/decor.rs:184) | crates/aa-client/src/port/housing/decor.rs:184 | ported from the decompile; no live golden found |
| `3903e020` | cursor_rotate (crates/aa-client/src/port/housing/decor.rs:307) | crates/aa-client/src/port/housing/decor.rs:307 | ported from the decompile; no live golden found |
| `39053360` | can_use_farm (crates/aa-client/src/port/housing/seeds.rs:267); farm_owned (crates/aa-client/src/port/housing/seeds.rs:100) | crates/aa-client/src/port/housing/seeds.rs:100 | ported from the decompile; no live golden found |
| `390533f0` | is_nation_owner (crates/aa-client/src/port/housing/seeds.rs:284) | crates/aa-client/src/port/housing/seeds.rs:284 | ported from the decompile; no live golden found |
| `39326980` | fence_points_box (crates/aa-client/src/port/housing/fence.rs:109); plot_box (crates/aa-client/src/port/housing/fence.rs:82) | crates/aa-client/src/port/housing/fence.rs:82 | ported from the decompile; no live golden found |
| `39326b40` | contains_any (crates/aa-client/src/port/housing/mapwalk.rs:58) | crates/aa-client/src/port/housing/mapwalk.rs:58 | ported from the decompile; no live golden found |
| `3932c120` | cursor_ready (crates/aa-client/src/port/housing/decor.rs:176); has_surface (crates/aa-client/src/port/housing/decor.rs:88); start_deco (crates/aa-client/src/port/housing/decor.rs:158) | crates/aa-client/src/port/housing/decor.rs:88 | ported from the decompile; no live golden found |
| `3932ead0` | fence_points (crates/aa-client/src/port/housing/fence.rs:104) | crates/aa-client/src/port/housing/fence.rs:104 | ported from the decompile; no live golden found |
| `3932fa00` | temp_house (crates/aa-client/src/port/housing/start.rs:85) | crates/aa-client/src/port/housing/start.rs:85 | ported from the decompile; no live golden found |
| `3932fb20` | temp_house (crates/aa-client/src/port/housing/start.rs:85) | crates/aa-client/src/port/housing/start.rs:85 | ported from the decompile; no live golden found |
| `39331f70` | garden_mesh_hit (crates/aa-client/src/port/housing/areas.rs:158) | crates/aa-client/src/port/housing/areas.rs:158 | ported from the decompile; no live golden found |
| `393a7170` | farm_permission (crates/aa-client/src/port/housing/seeds.rs:84) | crates/aa-client/src/port/housing/seeds.rs:84 | ported from the decompile; no live golden found |
| `393a8bd0` | unit_overlap (crates/aa-client/src/port/housing/seeds.rs:98) | crates/aa-client/src/port/housing/seeds.rs:98 | ported from the decompile; no live golden found |
| `396f62d0` | is_nation_owner (crates/aa-client/src/port/housing/seeds.rs:284) | crates/aa-client/src/port/housing/seeds.rs:284 | ported from the decompile; no live golden found |
| `397abc50` | new (crates/aa-client/src/port/housing/packets.rs:34) | crates/aa-client/src/port/housing/packets.rs:34 | ported from the decompile; no live golden found |
| `397ac850` | new (crates/aa-client/src/port/housing/packets.rs:251) | crates/aa-client/src/port/housing/packets.rs:251 | ported from the decompile; no live golden found |
| `397ae590` | new (crates/aa-client/src/port/housing/packets.rs:179) | crates/aa-client/src/port/housing/packets.rs:179 | ported from the decompile; no live golden found |
| `397b09d0` | new (crates/aa-client/src/port/housing/packets.rs:89) | crates/aa-client/src/port/housing/packets.rs:89 | ported from the decompile; no live golden found |
| `3983b280` | placement_height (crates/aa-client/src/port/housing/seeds.rs:92) | crates/aa-client/src/port/housing/seeds.rs:92 | ported from the decompile; no live golden found |
| `3983b2f0` | surface_ok (crates/aa-client/src/port/housing/seeds.rs:88) | crates/aa-client/src/port/housing/seeds.rs:88 | ported from the decompile; no live golden found |
| `3983b320` | surface_ok (crates/aa-client/src/port/housing/seeds.rs:88) | crates/aa-client/src/port/housing/seeds.rs:88 | ported from the decompile; no live golden found |
| `3983b340` | surface_ok (crates/aa-client/src/port/housing/seeds.rs:88) | crates/aa-client/src/port/housing/seeds.rs:88 | ported from the decompile; no live golden found |
| `39897280` | area_allows (crates/aa-client/src/port/housing/areas.rs:152) | crates/aa-client/src/port/housing/areas.rs:152 | ported from the decompile; no live golden found |

#### EXACT+UNWIRED (12)

| address | Rust function | source | proof / why |
|---|---|---|---|
| `39017620` | start_gate (crates/aa-client/src/port/housing/start.rs:53) | crates/aa-client/src/port/housing/start.rs:53 | exact, but not wired; goldens: phase_e_start.json; tests: the_live_gate_returns_match_the_original |
| `3903ba20` | rotation_between (crates/aa-client/src/port/housing/rotation.rs:42) | crates/aa-client/src/port/housing/rotation.rs:42 | exact, but not wired; goldens: rotation.json; tests: rotation_matrix_matches_live_bits |
| `3908c850` | dot_style (crates/aa-client/src/port/housing/category.rs:44) | crates/aa-client/src/port/housing/category.rs:44 | exact, but not wired; goldens: category.json; tests: category_leaves_match_live |
| `393317e0` | set_position (crates/aa-client/src/port/housing/builder.rs:34) | crates/aa-client/src/port/housing/builder.rs:34 | exact, but not wired; goldens: builder.json; tests: builder_elevation_stubs_are_no_ops; builder_state_setters_match_live; wheel_delta_matches_live_bits |
| `393317f0` | set_rotation (crates/aa-client/src/port/housing/builder.rs:39) | crates/aa-client/src/port/housing/builder.rs:39 | exact, but not wired; goldens: builder.json; tests: builder_elevation_stubs_are_no_ops; builder_state_setters_match_live; wheel_delta_matches_live_bits |
| `39331800` | set_elevation (crates/aa-client/src/port/housing/builder.rs:44) | crates/aa-client/src/port/housing/builder.rs:44 | exact, but not wired; goldens: builder.json; tests: builder_elevation_stubs_are_no_ops; builder_state_setters_match_live; wheel_delta_matches_live_bits |
| `39331810` | set_tax_pending (crates/aa-client/src/port/housing/builder.rs:49) | crates/aa-client/src/port/housing/builder.rs:49 | exact, but not wired; goldens: builder.json; tests: builder_elevation_stubs_are_no_ops; builder_state_setters_match_live; wheel_delta_matches_live_bits |
| `39332030` | frame_action (crates/aa-client/src/port/housing/lifecycle.rs:151) | crates/aa-client/src/port/housing/lifecycle.rs:151 | exact, but not wired; goldens: lifecycle.json; tests: cancel_matches_live_effects; create_matches_live_guards_and_rate_limit; frame_dispatch_matches_live_off_world_path; left_click_matches_live_transitions; quote_matches_live_guards; right_click_matches_live_transitions |
| `39336290` | elevation_ignored (crates/aa-client/src/port/housing/builder.rs:55) | crates/aa-client/src/port/housing/builder.rs:55 | exact, but not wired; goldens: builder.json; tests: builder_elevation_stubs_are_no_ops; builder_state_setters_match_live; wheel_delta_matches_live_bits |
| `393365f0` | no_op (crates/aa-client/src/port/housing/builder.rs:61) | crates/aa-client/src/port/housing/builder.rs:61 | exact, but not wired; goldens: builder.json; tests: builder_elevation_stubs_are_no_ops; builder_state_setters_match_live; wheel_delta_matches_live_bits |
| `39337830` | stair_elevation_ignored (crates/aa-client/src/port/housing/builder.rs:58) | crates/aa-client/src/port/housing/builder.rs:58 | exact, but not wired; goldens: builder.json; tests: builder_elevation_stubs_are_no_ops; builder_state_setters_match_live; wheel_delta_matches_live_bits |
| `394eaac0` | builder_for_use_type (crates/aa-client/src/port/housing/routing.rs:60) | crates/aa-client/src/port/housing/routing.rs:60 | exact, but not wired; goldens: phase_e_routing.json; tests: the_live_dispatcher_claims_the_housing_use_types |

38 reference-only addresses are listed in the CSV; they are original
functions named by comments that this tree does not implement.

### F7: `remote`

8 addresses, 5 ported, 2 done, 0 ported but unwired.

#### APPROX (2)

| address | Rust function | source | proof / why |
|---|---|---|---|
| `390a30e0` | delta_velocity (crates/aa-client/src/port/remote/sample.rs:33); sample_dt (crates/aa-client/src/port/remote/sample.rs:21); turn_response (crates/aa-client/src/port/remote/sample.rs:48); update (crates/aa-client/src/port/remote/controller.rs:168); update_living_entity (crates/aa-client/src/port/remote/controller.rs:202) | crates/aa-client/src/port/remote/controller.rs:168 | mode 3 follows the extrapolated sample with a stand-in response; the actor-model queue (FUN_390ad070/FUN_390b2cf0) is not ported (port/remote/mod.rs) \| ported from the decompile; no live golden found |
| `390a3bb0` | update (crates/aa-client/src/port/remote/controller.rs:168) | crates/aa-client/src/port/remote/controller.rs:168 | Update runs the mode-3 stand-in path (port/remote/mod.rs) \| ported from the decompile; no live golden found |

#### DECOMPILE (1)

| address | Rust function | source | proof / why |
|---|---|---|---|
| `390af040` | attach (crates/aa-client/src/port/remote/controller.rs:294); pre_physics_update (crates/aa-client/src/port/remote/controller.rs:280) | crates/aa-client/src/port/remote/controller.rs:280 | ported from the decompile; no live golden found |

3 reference-only addresses are listed in the CSV; they are original
functions named by comments that this tree does not implement.

### S8: `formulas`

41 addresses, 15 ported, 0 done, 15 ported but unwired.

#### EXACT+UNWIRED (15)

| address | Rust function | source | proof / why |
|---|---|---|---|
| `39568140` | variable_index (crates/aa-client/src/port/formulas/lookups.rs:67) | crates/aa-client/src/port/formulas/lookups.rs:67 | exact, but not wired; goldens: lexer_driver.json; lookups.json; nodes.json; tests: evaluator_leaves_match_the_original; lexer_driver_matches_the_original; parser_and_grammar_levels_match_the_original; variable_and_function_lookups_match_the_original |
| `39568180` | function_index (crates/aa-client/src/port/formulas/lookups.rs:75) | crates/aa-client/src/port/formulas/lookups.rs:75 | exact, but not wired; goldens: lexer_driver.json; lookups.json; nodes.json; tests: evaluator_leaves_match_the_original; lexer_driver_matches_the_original; parser_and_grammar_levels_match_the_original; variable_and_function_lookups_match_the_original |
| `398afbf0` | lexer_step0 (crates/aa-client/src/port/formulas/lexer.rs:99) | crates/aa-client/src/port/formulas/lexer.rs:99 | exact, but not wired; goldens: lexer_states.json; tests: evaluator_leaves_match_the_original; lexer_state_functions_match_the_original; parser_and_grammar_levels_match_the_original; variable_and_function_lookups_match_the_original |
| `398afde0` | lexer_step1 (crates/aa-client/src/port/formulas/lexer.rs:161) | crates/aa-client/src/port/formulas/lexer.rs:161 | exact, but not wired; goldens: lexer_states.json; tests: evaluator_leaves_match_the_original; lexer_state_functions_match_the_original; parser_and_grammar_levels_match_the_original; variable_and_function_lookups_match_the_original |
| `398afef0` | lexer_step2 (crates/aa-client/src/port/formulas/lexer.rs:187) | crates/aa-client/src/port/formulas/lexer.rs:187 | exact, but not wired; goldens: lexer_states.json; tests: evaluator_leaves_match_the_original; lexer_state_functions_match_the_original; parser_and_grammar_levels_match_the_original; variable_and_function_lookups_match_the_original |
| `398b0030` | lexer_step3 (crates/aa-client/src/port/formulas/lexer.rs:218) | crates/aa-client/src/port/formulas/lexer.rs:218 | exact, but not wired; goldens: lexer_states.json; tests: evaluator_leaves_match_the_original; lexer_state_functions_match_the_original; parser_and_grammar_levels_match_the_original; variable_and_function_lookups_match_the_original |
| `398b0150` | dispatch (crates/aa-client/src/port/formulas/parser.rs:197) | crates/aa-client/src/port/formulas/parser.rs:197 | exact, but not wired; goldens: parser.json; tests: evaluator_leaves_match_the_original; parser_and_grammar_levels_match_the_original; variable_and_function_lookups_match_the_original |
| `398b0450` | atof (crates/aa-client/src/port/formulas/lexer.rs:254); lexer_step (crates/aa-client/src/port/formulas/lexer.rs:240); next_token (crates/aa-client/src/port/formulas/lexer.rs:284) | crates/aa-client/src/port/formulas/lexer.rs:240 | exact, but not wired; goldens: lexer_driver.json; tests: evaluator_leaves_match_the_original; lexer_driver_matches_the_original; parser_and_grammar_levels_match_the_original; variable_and_function_lookups_match_the_original |
| `398b0560` | parse_arguments (crates/aa-client/src/port/formulas/parser.rs:185) | crates/aa-client/src/port/formulas/parser.rs:185 | exact, but not wired; goldens: parser.json; tests: evaluator_leaves_match_the_original; parser_and_grammar_levels_match_the_original; variable_and_function_lookups_match_the_original |
| `398b0680` | parse_atom (crates/aa-client/src/port/formulas/parser.rs:146) | crates/aa-client/src/port/formulas/parser.rs:146 | exact, but not wired; goldens: parser.json; tests: evaluator_leaves_match_the_original; parser_and_grammar_levels_match_the_original; variable_and_function_lookups_match_the_original |
| `398b0840` | parse_unary (crates/aa-client/src/port/formulas/parser.rs:133) | crates/aa-client/src/port/formulas/parser.rs:133 | exact, but not wired; goldens: parser.json; tests: evaluator_leaves_match_the_original; parser_and_grammar_levels_match_the_original; variable_and_function_lookups_match_the_original |
| `398b08a0` | parse_power (crates/aa-client/src/port/formulas/parser.rs:106) | crates/aa-client/src/port/formulas/parser.rs:106 | exact, but not wired; goldens: parser.json; tests: evaluator_leaves_match_the_original; parser_and_grammar_levels_match_the_original; variable_and_function_lookups_match_the_original |
| `398b09c0` | parse_multiplicative (crates/aa-client/src/port/formulas/parser.rs:87) | crates/aa-client/src/port/formulas/parser.rs:87 | exact, but not wired; goldens: parser.json; tests: evaluator_leaves_match_the_original; parser_and_grammar_levels_match_the_original; variable_and_function_lookups_match_the_original |
| `398b0a70` | parse_additive (crates/aa-client/src/port/formulas/parser.rs:69) | crates/aa-client/src/port/formulas/parser.rs:69 | exact, but not wired; goldens: parser.json; tests: evaluator_leaves_match_the_original; parser_and_grammar_levels_match_the_original; variable_and_function_lookups_match_the_original |
| `398b0b20` | parse (crates/aa-client/src/port/formulas/parser.rs:260); parse_all (crates/aa-client/src/port/formulas/parser.rs:58) | crates/aa-client/src/port/formulas/parser.rs:58 | exact, but not wired; goldens: parser.json; tests: evaluator_leaves_match_the_original; parser_and_grammar_levels_match_the_original; variable_and_function_lookups_match_the_original |

26 reference-only addresses are listed in the CSV; they are original
functions named by comments that this tree does not implement.

### E9: `world`

1 addresses, 1 ported, 1 done, 0 ported but unwired.

#### TOLERANCE (1)

| address | Rust function | source | proof / why |
|---|---|---|---|
| `3207dac0` | get_view_distance (crates/aa-client/src/port/world/view_distance.rs:43) | crates/aa-client/src/port/world/view_distance.rs:43 | tolerance: 1e-4 + want.abs() * 1e-5; goldens: view_distance.json; tests: matches_the_twenty_executed_original_cases |

## Slices with no address rows

These slices are named in the programme but cannot get a proof level from this audit:

| slice | why |
|---|---|
| E1 frame loop | lives in `ingame/frame_order.rs`, `ingame/physics.rs` and the Bevy schedule; no `port/**` address rows |
| E3/E4 character | not ported in this worktree; `port/engine/char*` does not exist here |
| F6 physics | `crates/aa-physics` is absent in this worktree |
| S6 flow graphs | mapped only (`docs/research/port-flowgraph/`) |
| S7 portals | mapped only (`docs/research/port-portals/`) |

They stay open until their functions are ported and carry a level here.

## Riding slice cross-check (`fix/ride-6`)

The riding inventory on `fix/ride-6` (`docs/research/riding-chain-inventory.md`) is the
chain-level view; this audit is the address-level view of `main`. They agree on the
direction, and `main` is behind the riding branch:

- The inventory's **PW** chain (movement request/turn `39404d50`, `39402590`, `39402690`,
  `39402e00`, quadruped dispatch `3912dd40`, camera mode 1 `39169860`, clip time/speed
  `31555240`/`315557d0`) is not the same set that is wired on `main`. On `main`,
  **64 of the 67 ported `port/movement` rows are unwired** (only `blend` and `ground`
  are reached), all `port/anim` rows are test-only, and only `fun_31555240` and
  `fun_315557d0` from `port/cryanim` are reached by `ingame/anim_clock.rs`.
- The inventory marks the mount per-frame update `39129800`, jump, mouse turn,
  accel/inertia, ground probe, move send pump, mount drawn pose, clip choice, pose
  sampling and the AC location chain as **MISS** or **SI**. Those are outside the
  ported-address rows here; the audit cannot give them a level until they are ported.
  The chain is therefore not complete even where this document reports EXACT rows.
- The inventory's **PNW** CryAnimation playback set (transition ramp, normalize, segment
  average, layer ramp, blend manager, LMG expander, command writer, process
  update/animations, start-by-id insertion) matches the audit: 12 EXACT + 2 TOLERANCE
  + 6 DECOMPILE + 11 reference-only rows in `cryanim`, with 18 unwired.
- The seat note is confirmed: `port/cryanim/attach.rs` seat composition is ported,
  but `ingame/mount.rs` uses its own `seat_world`; the ported attach functions stay
  unwired.
- The inventory's frame-loop correction (one `TimeStep` per frame, not 30 Hz) and its
  open flags/units are E1/E3 work, not address rows in `port/`; this audit does not
  raise them to a proof level.

Read the two documents together: this file says what is proven per original function,
the inventory says whether that function is on the riding chain and in what order.

## Manual review

The generator flags **20** rows, not the
352 reference-only addresses. They were read against the tests and
the goldens:

| address | slice | decision |
|---|---|---|
| `390f3780` | S2 | live capture (`click_live.json`) but no Rust test loads it; kept DECOMPILE, the capture is used by the capture tool, not compared in CI |
| `390f4960` | S2 | live capture (`click_live.json`) but no Rust test loads it; kept DECOMPILE, the capture is used by the capture tool, not compared in CI |
| `393b25a0` | S2 | live capture (`barlive.json`) but no Rust test loads it; kept DECOMPILE, the capture is used by the capture tool, not compared in CI |
| `393b2760` | S2 | live capture (`barlive.json`) but no Rust test loads it; kept DECOMPILE, the capture is used by the capture tool, not compared in CI |
| `393b2c70` | S2 | live capture (`npc_rate.json`) but no Rust test loads it; kept DECOMPILE, the capture is used by the capture tool, not compared in CI |
| `393bc7e0` | S2 | live capture (`loot_f.json`) but no Rust test loads it; kept DECOMPILE, the capture is used by the capture tool, not compared in CI |
| `393bc8c0` | S2 | live capture (`loot_f.json`) but no Rust test loads it; kept DECOMPILE, the capture is used by the capture tool, not compared in CI |
| `393be0e0` | S2 | live capture (`npc_special.json`) but no Rust test loads it; kept DECOMPILE, the capture is used by the capture tool, not compared in CI |
| `393bed90` | S2 | live capture (`npc_greeting.json`) but no Rust test loads it; kept DECOMPILE, the capture is used by the capture tool, not compared in CI |
| `393bf270` | S2 | live capture (`npc_range.json`) but no Rust test loads it; kept DECOMPILE, the capture is used by the capture tool, not compared in CI |
| `393bf500` | S2 | live capture (`npc_quest.json`) but no Rust test loads it; kept DECOMPILE, the capture is used by the capture tool, not compared in CI |
| `393c01d0` | S2 | live capture (`npc_gates.json`) but no Rust test loads it; kept DECOMPILE, the capture is used by the capture tool, not compared in CI |
| `393f8290` | S2 | not a function: the four `do_interaction_*` stub trampolines at `0x393f8290..c0`; the port implements their tail `FUN_393b2b70` as `do_interaction_index` |
| `39428d80` | S2 | live capture (`pick_ray.json`) but no Rust test loads it; kept DECOMPILE, the capture is used by the capture tool, not compared in CI |
| `3942c640` | S2 | live capture (`pick_update.json`) but no Rust test loads it; kept DECOMPILE, the capture is used by the capture tool, not compared in CI |
| `394f9790` | S2 | live capture (`loot_take.json`) but no Rust test loads it; kept DECOMPILE, the capture is used by the capture tool, not compared in CI |
| `394fab30` | S2 | live capture (`loot_close.json`) but no Rust test loads it; kept DECOMPILE, the capture is used by the capture tool, not compared in CI |
| `394fb010` | S2 | live capture (`loot_open.json`) but no Rust test loads it; kept DECOMPILE, the capture is used by the capture tool, not compared in CI |
| `394fb160` | S2 | live capture (`loot_open.json`) but no Rust test loads it; kept DECOMPILE, the capture is used by the capture tool, not compared in CI |
| `3a59f1c0` | S2 | not a function: the hotkey callback table at `0x3a59f1c0+0x13c` (data); the port implements the lookup `FUN_393f8d70` as `action_handler` |

No row remains unclassified. The untested interaction goldens are evidence files from
the capture scripts, not a comparison the test suite runs; they cannot raise the level
above DECOMPILE until a test loads them. The two golden-only addresses are data/table
bases, not ported functions. The CSV keeps the flags so a later change is visible.

## How to regenerate

From the repository root, with no game client and no cargo:

```powershell
python tools/port_audit/proof_levels.py
# optional: python tools/port_audit/proof_levels.py --out-dir <dir>
```

It reads the Rust sources, `port/**/goldens/*.json`, the test loaders/assertions and
the `ingame/`/`hud/` call sites. It writes `docs/research/proof-levels.csv`,
`docs/research/proof-levels-table.md` and `docs/research/proof-levels-review.md`.
This document's tables are generated from the CSV with the same classification; edit
the header prose by hand if the rule changes, then regenerate.
