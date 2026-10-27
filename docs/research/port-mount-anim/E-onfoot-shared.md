# E. On-foot equivalents that share the mount machinery

Scope: only the on-foot states/inputs that go through the same code as A-D. This is not a second
movement map; the movement-request side is `docs/research/port-movement/MAP.md` (other branch).

The important point for the port: the animation graph runtime is **not** mount-specific. The
rider and the player use the same `CAnimationGraphState`, the same input encoding/guard/state
selection, the same `CAnimatedCharacter` speed-input producer, the same jump/death functions and
the same cryanimation command writer. The differences are graph data (`player_full_body.xml` vs
`horse_full_body.xml`) and the `Vehicle`/`VehicleSeat` inputs.

---

## 1. Shared functions

| function | address | size | role for both |
|---|---|---:|---|
| `CAnimatedCharacter::SetAnimGraphSpeedInputs` | `30665830` | 9,646 | writes the movement graph inputs (direction key, speed floats, `RequestedTurnSpeedLZ`) for every actor |
| `CAnimatedCharacter::Update` | `306637e0` | 487 | frame entry; graph update via the entity/character interfaces |
| `CAnimationGraphState::SetInput` | `30632810` | 345 | raw/encoded input write |
| `CAnimationGraphState::Update` | `30639000` | 1,718 | state machine tick |
| `PerformQuery` / `BasicQuery` / `CStateIndex::Query` | `306359e0` / `30630c20` / `3065e050` | 2,257 / 345 / 502 | state selection |
| `EvaluateGuards` | `305f7a80` | 356 | guard test |
| `Update_Initial/Steady/Transition/Ephemeral` | `30636640` / `306357f0` / `30634d50` / `30634b70` | 537 / 483 / 1,053 / 480 | state machine |
| `EnterState` / `BuildStateFactories` / `ForceTeleport` | `306352a0` / `30631520` / `30636480` | 746 / 483 / 440 | factory/enter/force |
| `CAnimationSet::GetAnimationDesc` | `31558f80` | 304 | desc per layer |
| `UpdateAnimationLayer` | `31558550` | 1,187 | per-layer desc data (13 layers) |
| LMG command writer | `31556e90` | 2,488 | clip/LMG weights -> command buffer |
| jump input | `39153ff0` | 99 | `Action=jumpMP01/02` or `Signal=em_jump_in_place` |
| death force | `390c47f0` | 557 | `death_animation` / swim variants |
| rider copy | `390dcc80` | 1,011 | only mount-specific, but it uses the same input API |

On foot the player graph is `player_full_body.xml` (model 1-41, actor_models
`animation_graph`), with `player_upper_body.xml` as the second layer; the mount graph is
`horse_full_body.xml` (actor model 206). Both have the same input names and the same
`MovementControlMethodH/V` semantics (`re-mounts-full.md` §2-§4).

---

## 2. Inputs shared by the on-foot states

The player graph's states use the same inputs as the horse:

| input | on-foot use | mount/rider use |
|---|---|---|
| `PseudoSpeed` | idle 0..0.1, walk 0.11..0.7, run 0.8..3, sprint >1 | horse walk/run; rider `Horse_DriverWalk` 0.15..0.6, run 0.7..2.0 (copied from the mount) |
| `RequestedTurnSpeedLZ` | turn-in-place + run turn blend | same; rider copies the mount's value |
| `Action` | `jumpMP01/02`, `all_re_combat_hit`, skills | same; rider `Horse_Driver_Jump_*`, mount jump |
| `Signal` | `idle_rand`, `fall`, `em_jump_in_place` | same; mount idle rand |
| `RequestedMoveDir4LH` / `ActualMoveDir4LH` | 8-way direction substitutes `[dir]` | same; `Desired8Dir` for non-unit parents |
| `Stance` | relaxed/combat/stealth/swim | mount relaxed/combat/swim |
| `Vehicle` / `VehicleSeat` | `none`/unused | `horse` + seat 1..4 |

The player graph's on-foot states: `Move Start` -> `FIST_MO_[STANCE]_WALK` / `Run`; `Idle Start`
/ `Idle End`; `FIST_MO_[STANCE]_TURN` (TSTP); `FIST_MO_IDLETORUN` / `RUNTOIDLE`; jump
`jump_[dir]_start/loop/land`; `all_re_combat_hit`. The horse graph's equivalents are the same
templates with the horse clips (`re-mounts-full.md` §3).

`SetAnimGraphSpeedInputs` details (shared):

- `param_1+0x24` = graph state interface; `+0x6d0..+0x6de` = cached float input ids;
  `+0x6e0` = `RequestedTurnSpeedLZ` id; `+0x6d6` = key input id.
- `param_2` = a 7-float movement block (quaternion + velocity/speed), `param_3` = the entity
  rotation quaternion, `param_4` = dt, `param_5` = profile 0/9.
- It computes the local direction/speeds, writes the key and floats, then computes
  `RequestedTurnSpeedLZ = (quaternion delta / dt) * rad2deg` and writes `+0x6e0`.
- The "simplified" path (`+0x110 < +0x124 + 3`) writes zeros and `s` once; the complex path
  writes the full direction/speed set. Both profiles share the same code; the profile only
  selects a different input-id bank and flag byte.

Test: call `30665830` `thiscall` `this=character` with a scratch `param_2`/`param_3` and `f:dt`,
then read the graph inputs through `GetInputId`/`GetInputFloat` (`+0x60`/`+0x5c`). Run the same
values for the player and the horse; the port must reproduce all input values and the turn rate.

---

## 3. On-foot jump, turn, start/stop

- Jump: `FUN_39153ff0` writes `Action=jumpMP01` (or `jumpMP02` with walk flag 0x1000) or
  `Signal=em_jump_in_place`; the on-foot player graph selects `jump_[dir]_start/loop/land`
  (`player_full_body.xml`, `JumpMain/JumpLoop/JumpEnd`), the same pipeline as the rider's horse
  jump. Physics: `game_jump_height 1.3`, gravity 25 (player) / 20 (horse).
- Turn: `RequestedTurnSpeedLZ` from `SetAnimGraphSpeedInputs`; the graph's `TSTP` state blends
  `turn_l/idle_stop/turn_r`; `actor_models.turn_speed 360` is the model cap.
- Start/stop: `PseudoSpeed` bands 0.11..0.7 (walk), 0.8..3 (run), with `IDLETORUN` /
  `RUNTOIDLE` states and the `leave_at_stop`/`leave_actual_changed` flags. The same state machine
  and the same LMG writer are used on foot and mounted; only the clip names differ
  (player `fist_mo_*` vs horse `fist_mo_*` and rider `horse_mo_*`).
- Idle randoms: the same `SetInput time=N -> Signal idle_rand` template, with
  `fist_ba_<stance>_idle_rand_1..7` additive clips; the timer is graph-template logic (D §3.5).

---

## 4. Why E comes after A-D in the port

The on-foot path is the cleanest way to validate the shared runtime without a mount:

1. run the graph runtime with `player_full_body.xml` and fixed input vectors;
2. compare `PerformQuery`/state ids/guards against the original;
3. compare `SetAnimGraphSpeedInputs` values for walk/run/stop/turn/jump;
4. compare the command buffer and clip weights for the on-foot LMGs (`XL15`, `S8_1`, `MI01`,
   `MI02`, `M2I8`);
5. only then plug the mount/rider graphs (`XL14`, `horse S8_1/TSTP/ISTP/M2I8`) and the rider copy.

Every shared function in §1 is already in the D table with its size and test recipe; E adds no
new functions, only the data set and the input-value fixtures.
