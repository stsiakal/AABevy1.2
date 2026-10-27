# CryAnimation step C: the gallop rate, the rider seat, the FIFO insert

Addresses are `cryanimation.dll` (preferred base `0x31500000`) and `x2game.dll`
(`0x38ff0000`) VAs. Live evidence comes from the hidden 1.2 client through
`tools/cryanim-c/capture.py` (read-only `read`/`call`/`alloc`; no writes to the client).

## 1. The gallop rate

`FUN_315557d0` builds the FIFO entry from the resolved clip:

* CAF: `duration = (end[segment] - start[segment]) * header+0x30`; `step =
  entry+0x44 * speedIn / duration` (already ported in step A).
* LMG: `entry+0x20 = *(f32 *)(asset + 0x23c) * entry+0x44` when the layer argument is 0;
  the later LMG leaves scale `entry+0x28` by the per-example blend.

The layer speed is `header+0x14` (absolute `CSkeletonAnim + layer*0x30 + 0xe0`, written by
`FUN_31554660`). It was **1.0** in every captured frame; `entry+0x44` was 1.0 too. The
graph `PseudoSpeed` in the gallop is 1.0. So the whole playback multiplier is the LMG asset
`+0x23c`:

| actor | live asset `+0x23c` | steady live rate `step*duration/dt` | cases |
|---|---:|---:|---:|
| mount (horse run LMG) | 0.93752074 | 0.93752074 | 27 |
| rider (`fist_mo_rider_horse_run.lmg`) | 0.79284537 | 0.79284537 | 25 |

The authored `horse_mo_normal_run_f.caf` is 16 frames at 30 fps (0.5333 s). The mount rate
0.93752074 is exactly `0.5 / 0.5333333`, the LMG phase rate; the rider's rate is its LMG
blend. The step-B ~0.89 was the ratio between the CAF length and the LMG phase, not a
different graph input.

`port::cryanim::layer::fun_31554660` / `fun_315546a0` are proven by 20 + 20 direct calls on
a scratch `CSkeletonAnim` (`goldens/layer_setter.json`). `horse_gallop_rate` is proven against
`goldens/gallop_rate.json` (27 mount + 25 rider steady frames).

### Wiring and the recorder

`ingame/anim_clock.rs` applies `horse_gallop_rate` instead of the ground-speed match for the
mate and the local rider on the horse-run clip. The ported clock still uses the CAF segment
length, so the external advance is `dt * rate * layer speed`.

Mounted recorder, lane `anim-golden`, hidden original + prebuilt Bevy
(`mounted-20261025-230325-anim-c`): the original's rider-seat bob peaks average **567.95 ms**
over the gallop (13 fps sampling); Bevy peaks at **568.75 ms** (60 fps), i.e. the LMG rate
0.9375. The FFT metric bins both runs near 565/575 ms; the peak series is the finer number.
Bob amplitude: original 0.262 m, Bevy 0.292 m (the rider animation offset; the seat matrix is
the task-2 item).

## 2. The rider seat

`CAttachmentManager` is embedded at `char+0x30`. While riding, the mount manager holds a
type-2 object whose `+0x230` block is the seat `QuatT` pair: the model-space seat transform
at `+0x230` and the world transform at `+0x24c`. `x2game FUN_390e96a0` composes the mount
model transform with the seat bone's model-space transform:

```
world_quat = model_quat * local_quat
world_pos  = model_pos + model_quat * local_pos
```

Idle snapshot: model `(quat -0.0022,0,0,1; pos 4368.0,4350.4141,229.4372)`, local
`(quat -0.0022,0,0,1; pos 0.005,-0.0223,1.8786)` -> world
`(4368.0049,4350.3916,231.3158)`, matching the live block exactly. During the gallop the
local transform moves with the seat bone (`(-0.0043,-0.8938,1.6717)`), which is why the
rider must follow the animated bone, not a static attach point.

The composition is ported and live-gated (`port::cryanim::attach`, 38 stationary idle/stop
frames, `goldens/seat_world.json`). The playable client keeps the seat-bone **translation**
(the existing `seat_now` path). Applying the seat's full rotation to the rider entity was
measured in the recorder and rejected: the pelvis yaw rms went from 3.3 deg to 103.8 deg,
because this client plays the horse run CAF on the rider, which is not authored to compensate
the saddle pitch. The full matrix needs the rider graph/clip set first (blocked, reported).

## 3. StartAnimationById

`StartAnimationById 3155b350` guards and the `FUN_3155ae00` queue decision (16-entry limit,
dirty flag, last-entry dedupe with the desc `+0x18 & 0x80` exception, processing -> pending
queue, `+0x32` marker) are ported in `port::cryanim::start`. 30 live layer-0 FIFO growth pairs
from `cryanim_c.json` gate the insertion index, the new entry's `+0x32`, the count and the
duplicate rejection (`goldens/insertions.json`).

Blocked: the caller's descriptor field mapping. `FUN_31552950` copies the first 0x3c bytes of
the descriptor and embeds `FUN_315523b0(desc+0x3c)`, but the live FIFO entry's `+0x10` is a
phase while `StartAnimationById` reads the descriptor's `+0x10` as the layer index; the
descriptor is not the FIFO entry. The full field-level proof needs a live capture of the
descriptor passed by the graph (or a direct call on a cloned `CSkeletonAnim`), which is
reported as the remaining blocker.
