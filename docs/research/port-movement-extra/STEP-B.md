# Movement extras step B: swim, click-to-move, breath, water landing

Branch `port/move-extra-b`, port code in `crates/aa-client/src/port/movement/`.
All addresses are VAs in `x2game.dll` at the preferred base `0x38ff0000`.

## Ported and live-proven

| function | address | port | live cases | result |
|---|---|---|---|---|
| `FUN_3912ce00` swim control | 0x3912ce00 | `swim_control.rs` / `swim_control` | 20 foot + 20 mount | bit-exact velocity x/y/z, blend, deep latch, mode; graph values from the documented tail |
| `FUN_3930b600` dive start/end | 0x3930b600 | `breath.rs` / `breath_start` | 20 | bit-exact start/capacity writes |
| `FUN_3930b050` breath correction | 0x3930b050 | `breath.rs` / `breath_correct` | 20 | bit-exact wrapping `now - capacity + breath` |
| `FUN_3930b5b0` swim-jump gate | 0x3930b5b0 | `breath.rs` / `swim_jump_gate` | 20 | exact around the 0.2 threshold |
| `FUN_390a58b0` queue quaternion | 0x390a58b0 | `click_move.rs` / `queue_quaternion` | 20 | bit-exact quaternion + flags through a scratch queue vtable |
| `FUN_39097bd0` quaternion -> Euler | 0x39097bd0 | `click_move.rs` / `quaternion_to_euler` | 20 | bit-exact yaw/pitch/roll |
| `FUN_393ba480` move-to-point | 0x393ba480 | `click_move.rs` / `build_move_to_point` | 20 | all 0x9C request bytes exact |
| `FUN_390ca3d0` + `FUN_390b2af0` path controller | 0x390ca3d0 / 0x390b2af0 | `click_move.rs` / `path_controller_for`, `init_path_controller` | 20 | return/null, reuse flag and deterministic controller fields exact |

The live harness is `tools/move-extra-b/` (`drive.py`, `relaunch.py`, one capture script per
function). Goldens are the `goldens/*_live.json` files next to the port.

## Notes from the decompile

* Swim control mode is written as `mov dword ptr [esi+0x60], 3` (the C decompile renders the
  constant badly). The zero-G blend exponent is the double `1.0` at `0x3999cc10`, so the `pow`
  call is the identity; `+0x18` is clamped with its sign (`-v`, not `abs(v)`).
* `FUN_390a58b0` computes a determinant and calls `pow`, but the result is dead; only the two
  row normalisations and the quaternion branch matter.
* `FUN_393ba480` copies `-1` to request `+0x2c` and `+0x94`; the generic `MovementRequest`
  default leaves those zero, so the builder sets them explicitly.
* `FUN_390b2af0` leaves `+0x3c` and `+0x44..+0x6c` uninitialised (stack copy); the live golden
  excludes those words. The rest of the 0x1a0-byte object is deterministic.

## Blocked

* `FUN_390a6af0` (0x390a6af0) click handler: not ported. It is a 1,848-byte engine-global
  decision tree (game client singleton, camera ray, hierarchy checks). The queue object,
  its quaternion, the move-target setter (step A) and the path controller it calls are all
  ported and live-proven; the remaining handler needs a live recorder around the camera
  raycast before it can be ported without inventing inputs.
* `FUN_39117650` (0x39117650) water-landing effect: not ported. It depends on the live water
  volume / entity ray queries and writes graph `"landfall"`/`"speed"` inputs plus sound and
  camera-shake side effects. A direct synthetic call needs a fake physics entity and water
  manager; the effect arithmetic alone was not committed without that proof.
