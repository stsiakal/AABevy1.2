# CryAnimation playback map (ArcheAge 1.2 client): graph state -> posed skeleton

> **Restored 2026-10-27 from `archive/fork-main-2026-10-27`, UNVERIFIED.** The client was restarted from NickMesser's `main`; nothing described here is wired into the game. Addresses, call chains and offsets must be re-checked against the decompile ([R]/[I]/[?]) before use. A 2026-10-27 audit found 151 of the 269 EXACT/TOLERANCE labels in `proof-levels.*` weak or wrong (report: `D:\aaemu-client\handoff\2026-10-27\audit\report-proofs.md`).

Status: research map for a line-by-line Rust port and for differential tests that call the original
functions inside the running 1.2 client. Branch `map/engine-anim` (roadmap v2 phase 0, map **E2**).

Scope: how a selected animation state becomes a posed skeleton each frame, starting at the x2game
call sites that hand over to the engine:

* clip time advance and playback-speed scaling (how `PseudoSpeed` and the graph's speed inputs reach
  the clip rate),
* locomotion groups / blend spaces and blend weights,
* transitions and cross-fades,
* layers (full body, upper body, additive) and their bone masks,
* root motion / locomotion locator and whether it is applied to the entity,
* skeleton update order,
* attachments (rider seat bone, weapons, `$IK_Grip` helpers),
* IK and post-process (foot/stirrup IK, look IK, grip IK),
* phase-1 focus: the horse gallop and the mounted rider.

The animation-graph side (CryAction inputs, guards, state selection, transitions, LMG capabilities,
the command-buffer writer) is already mapped in `docs/research/port-mount-anim/MAP.md` and
`D-graph-runtime.md`; this document does not redo it. It starts where that map stops: the
CryAnimation `CSkeletonAnim` / `CSkeletonPose` side that turns the graph's animation commands into
a skeleton. Movement-request construction is in `docs/research/port-movement/MAP.md`. Vehicle
`$IK_Grip` use is in `docs/research/port-vehicles/MAP.md`.

All addresses are virtual addresses at the **preferred image base**:

| module | base | decompile |
|---|---|---|
| `x2game.dll` | `0x38ff0000` | `D:/aa-re/client12/out-x2game/x2game.dll.c`, `.funcs.csv`, `.calls.tsv`, `.strings.txt` |
| `cryaction.dll` | `0x304f0000` | `D:/aa-re/client12/engine/out/cryaction.dll.c`, `.funcs.csv` |
| `cryanimation.dll` | `0x31500000` | `D:/aa-re/client12/engine/out/cryanimation.dll.c`, `.funcs.csv`, `.strings.txt` |
| `cryentitysystem.dll` | `0x30400000` region | `D:/aa-re/client12/engine/out/cryentitysystem.dll.c` |

At runtime the module is rebased; use `module_name+rva` (rva = VA - preferred base) with
`aaemu-clientdriver call/read`. Sizes are the `funcs.csv` `size` field unless a body was split or
not exported, in which case the observed body size is marked *(no entry)*.

**Licensing.** Everything here was derived from the shipped binaries, their decompiles, their
`.data`, the shipped Lua bytecode and the shipped game data. The published CRYENGINE source was not
used, quoted or consulted. Names in quotes or `CClass` form come from the binaries' own
profiler/log/assert strings; names marked *(inferred)* describe observed behaviour only. Same rule
as `port-mount-anim/MAP.md` and `aaemu-physics/CRYPHYSICS-MAP.md`.

Tags: **[R]** read directly in the decompile/bytes; **[I]** inferred from data or from an unread
branch; **[?]** unresolved.

---

## 0. The one-screen call chain

```
x2game                                              cryaction / cryanimation
-------                                              ------------------------
X2::ActorUnitModel::Update 390dd080
  -> rider copy 390dcc80  (graph inputs PseudoSpeed, Stance, ...)
  -> graph setters 39153b60 / 39153ff0 / 390c47f0
                                                     CAnimatedCharacter::SetAnimGraphSpeedInputs
                                                     CAnimationGraphState::Update
                                                       -> state / transition / LMG desc
                                                       -> ISkeletonAnim::StartAnimationById
                                                          (SAnimation: id, layer, weight, speed)
                                                     CSkeletonAnim::ProcessAnimations 31550b90
                                                       -> ProcessAnimationUpdate 31550610
                                                            BlendManager 31556910 (weights, time)
                                                            LayerBlendManager 31555070
                                                            SyncRiderAnimation 3154dd70
                                                            ParseLocator 3154fda0
                                                       -> CSkeletonAnimTask::Begin/Execute
                                                            BuildCommandBuffer 31557cb0
                                                              LMG expand 31556e90
                                                            CommandExecutor 315a1250
                                                              clip sample 315a2160
                                                              LMG sample 315a3100
                                                              animation-driven IK 315a0630
                                                     CSkeletonAnim::FinishAnimationComputations 31551350
                                                     CSkeletonPose::SkeletonPostProcess 3154b4b0
                                                       final bone matrices, IK, physics sync
                                                     CSkinInstance::UpdateAttachedObjects 315253f0
                                                       -> CAttachmentManager::UpdateLocationAttachments
                                                          (rider entity on the seat bone)
                                                     skinning tasks / renderer
```

The x2game calls are input and entity/attachment glue; **all clip timing, blending and posing is
cryanimation**. The graph never advances clip time itself: it selects a state and writes an
`SAnimation` command. CryAnimation owns the per-frame `currentTime += speed` loop.

---

## 1. Handover and object model

### 1.1 x2game -> entity -> character

The x2game unit model is `X2::ActorUnitModel` (vtable/type strings in `x2game`); it owns the entity,
not the character instance. The engine interfaces are reached through entity virtuals:

| call | observed site | result |
|---|---|---|
| `entity->vt[0x168](0)` | `FUN_390e96a0` | `ICharacterInstance*` (character slot 0) |
| `ICharacterInstance->vt[0x20]()` | `FUN_390e96a0` | pose/attachment interface (used for bone transforms) |
| `ISkeletonPose->vt[0x1c](boneName)` | `FUN_390e96a0` | bone id/handle for the seat bone |
| `ISkeletonPose->vt[0x34](matrix)` | `FUN_390e96a0` | store a bone/attachment matrix |
| `ISkeletonPose->vt[0x94](x)` / `vt[0x48](0)` | `FUN_390e96a0` | attachment/bone auxiliary state |
| `entity->vt[0x3c](pose, 2)` | `FUN_390e96a0` | bind the child (rider) object to the mount character |
| `ISkeletonPose->vt[0x168](0, seat)` | `FUN_390e96a0` | resolve seat bone index |

These are **observed vtable offsets** [R]; the semantic names are [I] from the arguments and from
the engine side (`CAttachmentManager`, `CSkinInstance`). They are the only direct x2game->CryAnimation
links in the mount path.

The per-frame x2game handover is not a direct call into cryanimation:

* `X2::ActorUnitModel::Update FUN_390dd080` (632 B) runs the model sub-updates and calls the rider copy
  `FUN_390dcc80` (see `port-mount-anim/MAP.md` C). That writes the rider graph's `PseudoSpeed`,
  `Stance` and `RequestedTurnSpeedLZ`; it does not touch `CSkeletonAnim`.
* Graph key setters `FUN_39153b60` (`Vehicle`, `VehicleSeat`), `FUN_39153ff0` (`Action=jumpMP01/02`),
  `FUN_390c47f0` (death force) and the skill `match_animation` path write `CAnimationGraph` inputs.
  The graph state machine then produces the `SAnimation` command for cryanimation.
* The seat attach `FUN_390e96a0` (4,268 B) computes the mount-bone -> rider offset matrix and calls the
  engine attach. After that the engine updates the rider each frame through the attachment manager,
  not through x2game.

### 1.2 CryAnimation objects

`CCharInstance` (the `ICharacterInstance` implementation) is the stable object. The animation and pose
objects hang off it:

| offset | object / field |
|---|---|
| `char+0x030` | `CAttachmentManager` (attached objects, bone matrices) |
| `char+0x078` | current bone pose array, 7 floats per bone `{quat x,y,z,w, pos x,y,z}` (0x1c stride) |
| `char+0x084` | pose bone count |
| `char+0x11c` / `char+0x120` | animation-driven IK (ADIK) target list / bone target list |
| `char+0x180` | animation-set container; `+0x5d4` inside it is the `CAnimationSet` used by `FUN_3160cc50` |
| `char+0x1ec` | char flags (bit 1 set when an attached object updated) |
| `char+0x238` | **scaled frame delta** = `char+0x278 * char+0x240` (float) |
| `char+0x23c` | accumulated scaled time |
| `char+0x240` | raw frame delta from the engine timer (`ITimer::GetFrameTime`) |
| `char+0x244` | dirty flags; bit 0 = animation queue changed |
| `char+0x254` | `CSkeletonPose` interface pointer |
| `char+0x278` | `fAnimSpeedScale` (default 1.0; model property `fAnimSpeedScale`) |
| `char+0x29c` | last animation transform/position block copied by `StartAnimationProcessing` |

`CSkeletonAnim` (`this` in the functions below) points to its owning character at **`+0x498`**. Its
layout is the animation-FIFO owner:

| offset | meaning |
|---|---|
| `+0xcc` | 16 layer headers, stride 0x30: FIFO vector (`start`, `end`, `cap`) + current weight `+0x18` (= absolute `+0xe4`), ramp value `+0x1c` (= `+0xe8`), layer speed `+0x20` (= `+0xec`) |
| `+0x458` | dirty bit set by the command writer / layer update |
| `+0x45c` | additive/upper-body flag read by command writer (bit 0) |
| `+0x460` | per-layer "layer enabled" bitfield (u16) |
| `+0x464` | locator/time bias added to the animation time |
| `+0x46c` | flags: bit 0 = no command writer, bit 1 = no `StartAnimationById`, bit 2 = additive? |
| `+0x474` | total motion weight (sum of entry weights) |
| `+0x478..+0x494` | weighted motion accumulator (speed, turn, slope, strafe x/y); fed by `BuildCommandBuffer` |
| `+0x498` | owner `CCharInstance*` |
| `+0x49c` | animation-set/model interface used by the command writer |
| `+0x4a0` | **rider skeleton** (`CSkeletonAnim*` of the mounted rider) |
| `+0x4a4` | "processing started this frame" flag |
| `+0x4a6` | "new animation incoming" flag |
| `+0x4a7` | "locator parsed" flag |
| `+0x4cc` | pending animation queue (used before `ProcessAnimations` starts) |
| `+0x64c` | locator translation x/y/z (output of `ParseLocator`) |
| `+0x658` | locator rotation quaternion x/y/z/w |
| `+0x764` | per-bone layer mask (u16 per bone; bit N = bone is affected by layer N) |

`CSkeletonPose` (`char+0x254`) owns the final bone matrices and post-process: `+0x04` is the owning
character, `+0x09b4` the skeleton-anim object, `+0x6c` the skeleton/model, and `+0x9b4` etc. are used
by `SkeletonPostProcess`.

`CAttachmentManager` (at `char+0x30`): `+0x08/+0x0c` attachment object vector, `+0x2c` skeleton,
`+0xac/+0xb0` animated-bone index list, `+0xa9` dirty flag. `CSkinInstance` wraps it; the `CSkinInstance`
at `char+0x00`? is the object whose `UpdateAttachedObjects` calls
`UpdateLocationAttachments` first (see §7).

### 1.3 Per-frame entry points

| function | address | size | role |
|---|---|---:|---|
| `CharacterManager::Update` | `315d3360` | 1,002 | global per-frame animation-manager tick (timer, animation headers, streaming, GC) |
| `CAnimationContext::Update` | `3165ac00` | 120 | iterates registered per-character update callbacks |
| `CSkeletonAnim::ProcessAnimations` | `31550b90` | 1,336 | per-character animation tick; must be called once per frame |
| `CSkeletonAnim::ProcessAnimationUpdate` | `31550610` | 500 | advances FIFO time/weights, rider sync, locator |
| `CSkeletonAnim::FinishAnimationComputations` | `31551350` | 282 | waits the pose task, clears the busy flag, publishes locator |
| `CSkeletonPose::SkeletonPostProcess` | `3154b4b0` | 7,920 | final pose, post-process IK, physics sync, attachments |
| `CSkinInstance::UpdateAttachedObjects` | `315253f0` | 570 | updates attached objects with the new bone matrices |

`ProcessAnimations`, `FinishAnimationComputations`, `StartAnimationById`, `SkeletonPostProcess` and the
attachment methods have **no direct static callers** in the decompile (0 internal xrefs); they are
`ICharacterInstance` / `ISkeletonAnim` / `ISkeletonPose` virtuals called by the entity/animated-character
layer in cryaction (`CAnimatedCharacter::Update`). This is why the engine-side call order has to be
taken from the functions themselves and from the task/profiler names, not from a static call graph.

The per-character update order (cryanimation side) is:

1. graph update writes animation commands (`StartAnimationById` -> FIFO entry),
2. `ProcessAnimations` -> `ProcessAnimationUpdate` (per-layer BlendManager, LayerBlendManager, rider
   sync, `ParseLocator`),
3. `CSkeletonAnimTask::Begin` -> `Execute` (command buffer + pose evaluation + animation-driven IK),
4. `FinishAnimationComputations` (task wait, `SyncData`, clear busy),
5. `SkeletonPostProcess` (absolute matrices, post-process IK, physics sync),
6. `CSkinInstance::UpdateAttachedObjects` (attachments, rider),
7. skinning tasks / renderer.

The exact interleaving with the x2game unit update, physics step and camera is E1 (frame loop), not
this document.

---

## 2. From graph command to FIFO entry

### 2.1 `SAnimation` command shape

CryAction's graph/LMG output is a per-layer animation command. The command struct consumed by
`StartAnimationById` (`param_2` there) is copied field for field by `FUN_315523b0` and has this
observable layout (dwords):

| offset | field | used as |
|---|---|---|
| `+0x00` | asset / LMG pointer or 0 | `entry+0x00`; 0 = single clip, else LMG asset |
| `+0x04` | u16 flags / u16 | `entry+0x04/+0x06` flags |
| `+0x08` | u16 anim id + u16 segment | `entry+0x08` (clip id), `entry+0x0a` (segment) |
| `+0x0c` | previous clip time | `entry+0x0c` |
| `+0x10` | current clip time | `entry+0x10` |
| `+0x14` | **playback speed** | `entry+0x14`; multiplies the clip rate |
| `+0x18` | transition weight | `entry+0x18`; ramps 0..1 |
| `+0x1c` | normalized blend weight | `entry+0x1c` |
| `+0x20` | playback/speed cap | `entry+0x20` |
| `+0x24` | segment duration | `entry+0x24` |
| `+0x28` | per-frame phase step | `entry+0x28` |
| `+0x2c` | u16/u16 | validity / loop counters |
| `+0x30` | u16/u16 | segment index / loop count |
| `+0x34` | u16 | remove flag |
| `+0x36` | u8/u8 | segment id / "wrapped" |
| `+0x38` | u16/u16 | loop / max loops |
| `+0x3c` | transition/events block | 0x4a bytes, refcounted asset at +0x44 |
| `+0x44` | per-entry playback scale | read as float by `FUN_315557d0` (**see [?] below**) |
| `+0x54` | flags | bit 0x8000 = locator, 0x400 = time-dirty, ... |
| `+0x5c..+0x78` | motion block | weighted speed/turn/slope/strafe for `BuildCommandBuffer` |
| `+0x85` | additive/attack-speed flag | selects `char+0x468` scalar in the speed formula |

The 0x3c block is copied by `FUN_315523b0` (163 B), the full 0x88 entry by `FUN_31552950` (180 B).
The `+0x44` float vs refcounted-pointer collision between the `SAnimation` copy and the FIFO entry is
the one layout ambiguity in this area [?]; every speed formula below uses the FIFO-entry reading.

### 2.2 `CSkeletonAnim::StartAnimationById 3155b350` (1,617 B)

`ISkeletonAnim` entry. Callers: no static xref (virtual from the graph/animated-character layer).
Callees: `FUN_315523b0` (copy `SAnimation`), `FUN_3155ae00` (instantiate), `FUN_315445c0` (unlock the
animation set), `FUN_3160cc50` (resolve animation id -> asset), and the animation-asset streaming
warnings.

Reads/writes:
* `param_1` = animation id (`short`, resolved through `CAnimationSet` at `*(char+0x180)+0x5d4`).
* `param_2[4]` = layer 0..15; `param_2[5]` = layer speed -> `char+layer*0x30+0xec`; `*param_2` -> layer
  weight `char+layer*0x30+0xe8`.
* `param_2[6] & 0x800` = "scale the previous entries' transition durations by 0.5" (start-of-blend
  flag); `& 0x200` clears the entry's weight.
* `char+0x4a6 = 0` (new animation incoming).
* Rejects invalid layer, missing animation set, type-2 (LMG) assets without the needed bits, and
  updates the animation-asset refcount/streaming state.

Stateful. Test: call with a live `CSkeletonAnim`, a known clip id and a scratch `SAnimation`; compare
the queue entry bytes and `char+layer*0x30+0xe8/+0xec` before/after. The pure `FUN_315523b0`
copy can be diffed separately with `alloc`/`write`.

`FUN_3155ae00` (1,347 B) is the instantiate path:
* validates the layer queue length (< 0x10 entries),
* creates the entry from `param_8` (`FUN_31552760` -> `FUN_31552950`),
* sets the layer's FIFO (`FUN_3155ab20` / `FUN_3155bbb0`) or queues into `char+0xc8+...` while
  `ProcessAnimations` has not started,
* increments clip reference counts (`GlobalAnimationHeaderCAF+0x56/+0x58`) and streaming state.

`FUN_3151b220 CCharInstance::StartAnimationProcessing` (535 B) is the other public entry (used by the
entity side for a pre-built command block):
* copies `param_1[0..6]` to `char+0x29c..0x2b4` (last animation transform),
* `param_1[0x11]` -> `+0x4e8` of the object returned by `char vt[0x1b4]`; `param_1[0x12]` -> `char+0x2b8`,
* calls `FUN_3151b150` (201 B), `FUN_3151a1c0` (787 B) and `FUN_3151e930` (339 B) to publish the
  transform and call the animation interface.

The setter/getter pair for the global clip-rate scale is `FUN_31520270` (20 B, writes
`char+0x278`) / `FUN_31520290` (7 B, reads it); the model property `fAnimSpeedScale` is bound to
`char+0x278` in `FUN_3151aab0` (property loader). Default is 1.0.

---

## 3. Clip time advance and playback-speed scaling

### 3.1 `ProcessAnimationUpdate 31550610` (500 B)

Called once per frame from `ProcessAnimations`, after the graph has written the frame's commands.
It scans all 16 layer FIFOs for the dirty bit `entry+0x54 & 0x400`. If any entry is dirty:

1. For each non-empty layer `i`: `BlendManager(this, char->+0x238 * entry[5], layer, i)` where
   `entry[5]` is `+0x14` (see 3.2) and `char+0x238` is the scaled frame delta.
2. `LayerBlendManager(this, char->+0x238, i)` for every layer (layer-weight ramp).
3. `SyncRiderAnimation(this)` (mount only).
4. Optional debug FIFO dump (`DAT_316c9248` / `char+0x45d`).
5. Clears `char+0x244` bit 0, sets `char+0x4a7 = 1`, calls `ParseLocator` (root motion).

The per-entry dirty flag `+0x54 & 0x400` is set by `StartAnimationById`/the graph command; if no entry
is dirty the whole advance is skipped. This is a cheap "animation unchanged" fast path, not a pause:
`ProcessAnimations` still checks the task/pose state afterwards.

### 3.2 Per-entry speed formula

`BlendManager 31556910` (822 B) gathers up to 8 entries from the layer FIFO, validates their assets
(`FUN_31555000`: clip loaded / LMG valid), then runs:

| callee | address | size | role |
|---|---|---:|---|
| `FUN_31554ba0` | `31554ba0` | 726 | ramps transition weight `entry+0x18` -> 1 over `entry+0x3c` |
| `FUN_315547a0` | `315547a0` | 924 | cross-fade chain + normalizes `entry+0x1c` to sum 1 |
| `FUN_31556540` | `31556540` | 829 | per-entry playback speed / LMG-specific speed policy |
| `FUN_315557d0` | `315557d0` | 525 | computes `entry+0x24` and `entry+0x28` |
| `FUN_31554e80` | `31554e80` | 362 | averages per-segment travel speed into `entry+0x28` |
| `FUN_315559e0` | `315559e0` | 180 | calls `FUN_31555240` (time advance) and `FUN_315554d0` (events) per entry |

`FUN_315557d0` for a **single clip**:

```
segmentDuration = (clip+0x60[seg] - clip+0x5c[seg]) * clip+0x30     // start/end lane times * time scale
entry[+0x24]    = max(segmentDuration, minClipDuration)            // minClipDuration = _DAT_31677660
fVar5           = (entry[+0x85] ? char+0x468 : 1.0)
entry[+0x20]    = fVar5 * entry[+0x44]
entry[+0x28]    = (fVar5 * entry[+0x44] * speedIn) / entry[+0x24]
```

where `speedIn` is the `param_3` passed from `FUN_31556540`, which is
`char+0x238 * entry[+0x14]` (scaled frame delta times the command's playback speed). Therefore:

```
phase step per frame = dt * fAnimSpeedScale * entry[+0x14] * entry[+0x44] / segmentDuration
one segment pass     = segmentDuration / (fAnimSpeedScale * entry[+0x14] * entry[+0x44])
```

With defaults `fAnimSpeedScale = 1`, `entry[+0x14] = 1`, `entry[+0x44] = 1`, a clip segment plays in
exactly its CAF length. This is the answer to "why one stride takes the time it does": **the stride is
the clip's segment length divided by the graph-supplied playback speed and the per-entry scale; the
graph's `PseudoSpeed` enters only through the state/LMG selection and the command's `+0x14` speed, not
through a global multiplier.** The global `fAnimSpeedScale` is a model property (slow-motion/death),
not the mount's speed.

`FUN_31556540` applies LMG-specific overrides before the formula:
* `MI01`/`MI02` special cases at the end/start of a loop choose the next example index and store it at
  LMG `+0x184`/`+0x188`/`+0x18c` (random idle/step variation).
* LMG `S8SW` uses `DAT_31677bf8` as a minimum play speed.
* For LMG entries (`*entry != 0`) the example speeds from the caps (`+0x23c` play-speed scale and the
  per-example `+0x44` values) are multiplied into `entry+0x28`; the exact per-blend-type arithmetic is
  the LMG weight work already mapped in D §4 and is not repeated here.

### 3.3 Time advance `FUN_31555240` (654 B)

Per entry, per frame:

```
entry[+0x0c] = entry[+0x10]                 // previous time
entry[+0x10] = entry[+0x10] + entry[+0x28]  // speed is a normalized phase step, not a dt
clamp entry[+0x10] to [0, DAT_31677bf0]     // animation length / wrap limit
```

* Negative speed plays backwards; the loop flags at `entry+0x54` choose once/clamp/loop.
* On wrap: increments `entry+0x2e` (loop count) and, for LMG, advances the per-example loop counters
  in the LMG asset (`+0x54 + i*2`); when the count reaches the example limit the segment index wraps
  and `entry+0x37` is set (event re-arm).
* If `entry+0x1c` (blend weight) is 0, marks `entry+0x34 = 1` (remove after this frame).
* `DAT_316768ac` clamps the maximum time step (a very long frame cannot skip a whole loop), and
  `DAT_31677bf4` clamps the minimum (0).

`FUN_315559e0` also calls `FUN_315554d0` (510 B) for animation events: it tests the clip's event time
list (`clip+0x50` for CAF, LMG table `+0x18`) against the `[previous, current)` interval and appends up
to 5 events to `char+0x4f4` (`char+0x648` count). This is the footstep/effect/IK event channel.

### 3.4 The horse gallop, concretely

The mount's gallop is an `XL14` locomotion group (`fist_mo_normal_run.lmg`), 16 examples (8 directions,
turn and slope/sprint tiers; D §4.4). The graph's `PseudoSpeed` selects the run state; the LMG weights
select a blend of the examples from the character's actual velocity (`FUN_31558550` case 2: speed =
`|velocity|` scaled by the model scale). The command's `+0x14` speed is the graph's playback scale; the
example's CAF start/end times give `segmentDuration`. The visible stride time is therefore set by:

1. the CAF clip's authored cycle length (seconds) and its `+0x30` time scale,
2. the selected example blend (a slower example has a longer cycle),
3. `entry[+0x14]` (graph speed) and `entry[+0x44]` (per-entry scale),
4. `fAnimSpeedScale` (normally 1).

There is no separate "stride length / velocity" calculation in CryAnimation; the animation is the
clock. Movement speed is produced by the x2game movement request/physics, and animation follows it
via the LMG speed inputs. This is why a mismatch in the graph's `PseudoSpeed` or in the LMG example
selection shows up as foot sliding, not as a changed movement speed.

**Unknowns:** the exact `entry[+0x14]` value written by the graph for run/sprint (needs a live FIFO
read of a galloping horse); the exact `+0x44` provenance; the numeric CAF lengths (data, not
decompile). The `PseudoSpeed` producer itself is still open in `port-mount-anim/MAP.md` §6.4.

---

## 4. Transitions, blend weights and layers

### 4.1 Transition-weight ramp `FUN_31554ba0` (726 B)

Called once per entry by `BlendManager`. For each entry:

```
duration = entry[+0x3c]              // transition time from the graph command
step     = (flag ? char+0x240 : speedIn) / max(duration, epsilon)
entry[+0x18] = clamp(entry[+0x18] + step, 0, 1)
```

* `speedIn` = `char+0x238 * entry[+0x14]` when the "raw time" flag (`char+0x46c bit 1`) is clear;
  otherwise the raw `char+0x240` delta is used (so the cross-fade is wall-clock, not speed-scaled).
* If `speedIn == 0` and `duration == 0`, the weight is set to 1 immediately.
* The weight is the cross-fade alpha, not the final blend weight.

`FUN_315547a0` (924 B) turns the per-entry alphas into final blend weights:

1. `entry[0][+0x1c] = 1`;
2. each following entry's alpha is multiplied by the running complement `(1 - previous weight)`
   (an ordered cross-fade chain, so entry *i* only gets what the earlier entries left);
3. the alpha is shaped by a smooth `x/(1 + 0.5 x^2)` style curve (constants `DAT_316745a4`,
   `DAT_31674584`), clamped to `[0,1]`;
4. all entry weights are normalized so they sum to 1.

`FUN_31554e80` (362 B) then groups entries by segment id `+0x36` and replaces each group's
`+0x28` (travel speed / phase step) with the weight-averaged group value. Segment ids are derived
from the `+0x54 & 8` flag: flagged entries start a new segment, unflagged entries inherit the current
segment's id. This is how a multi-example LMG keeps all examples of one gait on the same phase.

BlendManager caps the active set at **8 entries** (`param_2 = min(count, 8)`). Entries with weight 0
are marked `+0x34 = 1` by `FUN_31555240` and removed in `SyncData` after the frame.

### 4.2 Layer-weight ramp `FUN_31555070` (449 B)

`LayerBlendManager` runs per layer after the per-entry weights. The layer header is at
`char + layer*0x30 + 0xcc` (same address as the FIFO vector header):

* `+0xe4` (header `+0x18`) = **current layer weight**;
* `+0xe8` = desired/ramp value written by `StartAnimationById` from the command's layer weight, and
  used as the ramp divisor in this function;
* `+0xec` = layer speed multiplier used by the command writer.

```
w    = current (+0xe4)
rate = max(+0xe8, minRamp)          // minRamp = DAT_31675118
w   += (direction ? +1 : -1) * dt / rate
clamp w to [0,1]; write back to +0xe4
```

* `direction` is `char+0x460 & (1 << layer)`; set by `StartAnimationById` when the layer is enabled.
* If `+0xe8` is 0 or negative, the raw `char+0x240` delta is used for `dt`.
* If the layer is being disabled and reaches 0, the first entry is marked `+0x34 = 1`.
* Layer 0 is the full-body layer; layers 1..12 are upper-body/additive/extra (13 desc blocks at
  `char+0x19c`, stride 0x3c; 16 FIFO slots at `char+0xcc`, stride 0x30).

### 4.3 Building the command buffer `FUN_31557cb0` (1,278 B)

`CSkeletonAnimTask::Execute` calls this to translate the FIFO into the flat command buffer consumed
by `FUN_315a1250`.

* Buffer: 0x800 bytes at `param_2`; header/cursor at `+0x800`, command count at `+0x804`, command
  payload starts at `+0x808`.
* Layer 0 first: writes command `0` (clear) with the layer/flags byte, then per entry:
  * simple clip: `FUN_31557af0` -> command `2` (0x1c);
  * LMG: `FUN_31557860` -> command `3` (0x18, with bone mask) or `5` (0x10), and the LMG expander
    `FUN_31556e90` for the weighted examples;
  then command `9` (normalize) and command `0xb` (event/aux).
* Layers 1..15: same per-entry expansion, with the layer index in the high nibble of the command
  flags byte (`cmd[+1] = layer << 4 | flags`).
* Accumulates `entry[+0x5c..+0x78] * entry[+0x1c]` into `char+0x474..+0x494` (motion feedback).
* Writes `char+0x458 = 1` (dirty) and `char+0x4a7` handling.
* Overflow logs `CryAnimation: CommandBuffer overflow!`; the writer checks the remaining bytes before
  every command.

`FUN_31556e90` (2,488 B) is the LMG expander. For one LMG entry it:
* resolves the LMG asset and validates the weight sum (`CryAnimation: LMG weights don't sum up to 1`);
* groups equal animation ids and sums their weights;
* emits command `2` for a single example or command `3`/`4` with the example list;
* writes `cmd[+0xc]` weight, `cmd[+0x10]` blend time, `cmd[+0x14]` playback, and for command 3 the
  per-bone layer mask at `cmd+0x18`:
  `cmd[+0x18+i] = (char+0x764[i] >> layer) << 7`.
* Sets `char+0x458 |= 1`.

The per-blend-type weight formulas (XL14, S8_1, TSTP, ISTP, ...) are D §4.2/§6; E2 only needs the
command layout and the fact that their result is the list of example weights consumed here.

### 4.4 Command evaluators `FUN_315a1250` (928 B)

The executor dispatches by command byte. Sizes and meanings established from the bodies:

| cmd | bytes | evaluator | meaning |
|---:|---:|---|---|
| 0 | 4 | `FUN_315a17e0` | clear target pose range for the layer |
| 1 | 4 | `FUN_315a1830` | clear one bone (7 floats) |
| 2 | 0x1c | `FUN_315a2160` | sample one clip and blend into the layer pose |
| 3 | 0x18 | `FUN_315a3100` | sample an LMG example list / secondary clip, with bone mask |
| 4 | 0x118 | `FUN_315a3100` | sample a full LMG blend block (up to 40 weights) |
| 5 | 0x10 | `FUN_315a1cc0` | resolve/set animation-set asset |
| 6 | 0xc | `FUN_315a1870` | resolve animation id |
| 7 | 8 | `FUN_315a1b90` | additive: `dst += src * weight` per bone |
| 8 | 8 | `FUN_315a19b0` | scale bone translations |
| 9 | 8 | `FUN_315a18c0` | normalize quaternions in the bone range |
| 0xa | 8 | inline | debug/telemetry only |
| 0xb | 8 | `FUN_315a1cf0` | external pose/event callback |
| 0xd | 8 | `FUN_315a1a30` | pelvis height offset |
| 0xe | 8 | `FUN_315a1e50` | pelvis yaw/roll rotation |
| 0xf | 4 | `FUN_315a1b50` | copy one bone range |

`FUN_315a2160` (3,969 B) does the actual clip sample:
* resolves the clip header (`FUN_3160cc50` -> `GlobalAnimationHeaderCAF`),
* computes the sample time `t = start + (end-start) * clamp01(cmd[+4])`,
* for each bone in the clip's bone list (`param_1[0x14]`, count `param_1[0x15]`) copies the rest
  pose from `char+0x78` and calls `FUN_315a1fd0` (72 lines) to evaluate the bone at time `t`,
* blends the result into the destination pose with `cmd[+0xc]` weight; additive commands accumulate
  into the additive buffer (flag bit 3) instead of replacing,
* applies root-motion/attack-speed tweaks for the pelvis and the `Bip01` hierarchy.

`FUN_315a3100` (1,750 B) is the LMG evaluator: it samples each example with `FUN_315a1fd0` and
accumulates them by the command's weights; the bone-mask byte at `cmd+0x18` is tested per bone when
`cmd[+2] != 0`, which is the upper-body/layer mask. `param_1[0x14]`/`[0x15]` is the command's bone
list, `cmd[+8]` the animation id, `cmd[+4]` the additive/source mode.

### 4.5 Layers and masking

* `CAnimatedCharacter::Init` (CryAction, D §5) creates up to 8 graph layers; the default graphs are
  `player_full_body` and `player_upper_body`.
* CryAnimation has **16 FIFO layers** (`char+0xcc`, stride 0x30) and **13 desc layers**
  (`char+0x19c`, stride 0x3c). The graph layer index is the command's high nibble; 0 is full body.
* The bone mask is `char+0x764`: one `u16` per bone, bit *n* = this bone is touched by layer *n*.
  It is filled from the model's animation-layer setup; `FUN_31556e90`/`FUN_31557860` serialize it as
  one byte per bone into command 3. `FUN_315a3100` skips a bone when its mask byte is 0. This is the
  upper-body lower-body masking policy that D §5 left open: **it is a per-bone layer bitmask, not a
  hard-coded bone-name list**.
* Additive layers use command flag bit 4 (`char+0x45c`), command 7, and a separate additive pose
  buffer (the evaluator's scratch+0 vs scratch+0xc selection via command flag bit 3).

---

## 5. Root motion and the locomotion locator

CryAnimation does not move the entity. It produces two motion outputs:

### 5.1 Motion feedback `char+0x474..+0x494`

`FUN_31557cb0` sums each entry's motion block (`entry+0x5c..+0x78`, 8 floats) weighted by the entry's
blend weight `+0x1c`:
* `char+0x474` = total weight,
* `+0x478/+0x480/+0x488` = weighted speed/turn/slope/strafe components,
* `+0x490/+0x494` = extra components.
The debug string names them `speed: %+.2f turn: %+.2f slope: %+.2f strafe:(%+.2f %+.2f)`
(`0x31677ca0`). CryAction reads this block through `CAnimatedCharacter::CalculateParamsForCurrentMotions`
/ `AcquireRequestedBehaviourMovement` to feed leaning, inertia and motion parameters. It is not a
world-space displacement.

### 5.2 Locator `char+0x64c..+0x664`

`CSkeletonAnim::ParseLocator 3154fda0` (1,337 B) runs after the per-entry time update:

1. resets `char+0x64c..+0x664` (translation x/y/z at +0x64c, quaternion x/y/z/w at +0x658; w
   initialized to 1.0);
2. if `char+0x45c` is clear, stops here (no locator);
3. takes the first N (<= `param_1`) entries and calls `FUN_3154e030` (535 B) for each to extract a
   locator sample: `{clip id, sample time, weight, flags}`. For a simple clip the sample time is
   `start + (end-start) * clip.timeScale * current/scale`; for an LMG it emits one sample per example
   with the example's weight;
4. normalizes the sample weights, calls `FUN_3154e9f0` (4,354 B) to blend the samples and
   `FUN_31554560` (192 B) to resolve the final turn/height;
5. writes translation and quaternion. `FUN_3154e030` only emits a sample if the entry flags
   `+0x54 & 0x8000` ("locator") are set.

`FUN_3154e9f0` classifies the motion (turn-in-place, move, slope, height) and composes the weighted
quaternions; its full body is not decoded here (4.3 KB) but its inputs/outputs are known. `ParseLocator`
is the sole producer of `char+0x64c`.

### 5.3 Is it applied to the entity?

For the horse and rider: **no** [I, backed by x2game movement]. The mount's world position/orientation
comes from the server `SCUnitState` and the x2game movement request/physics; the animation locator is
skeletal only. `port-mount-anim` §B already states "the clip root motion is skeletal only". The x2game
string `allowEntityClampingByAnimation` exists for the entity-side option, but no mount/riding path sets
it in the code read. For on-foot, the movement request is also authoritative; the locator feeds
CryAction's `ClampAnimLocation` / `CalculateAnimRenderLocation` as a clamp/lean input, not as a
position update [I]. This must be confirmed with the E3 (`CAnimatedCharacter`) map.

Consequence for the port: do **not** add clip root motion to the mount or player transform. Use the
locator only if E3 shows a clamp/lean consumer; the phase-1 gallop does not need it.

---

## 6. Skeleton update order

The task pipeline is explicit in the profiler names:

| step | function | address | size | what it does |
|---|---|---|---:|---|
| begin | `CSkeletonAnimTask::Begin` | `3154fbf0` | 427 | `CAttachmentManager::UpdateAnimBones` on `char+0x30`; copy `SAnimation`/locator input into the task; `PoseModifiersPrepare` |
| prepare | `CSkeletonAnimTask::Prepare` | `3154e8a0` | 325 | copies the task pose/locator input; sets the running flag |
| synchronize | `CSkeletonAnimTask::Synchronize` | `3154d820` | 233 | task-side sync (job/task wait) |
| wait | `CSkeletonAnimTask::Wait` | `3154e250` | 228 | joins the task; called from `FinishAnimationComputations` |
| execute | `CSkeletonAnimTask::Execute` | `315502e0` | 542 | `BuildCommandBuffer 31557cb0`; `FUN_315a1250` command executor; marks task done |
| finish | `CSkeletonAnim::FinishAnimationComputations` | `31551350` | 282 | wait, clear `char+0x4a4`, `SyncData`, clear task slots |
| sync | `CSkeletonAnim::SyncData` | `315510d0` | 623 | `SyncProcessAnimations`; remove expired entries; publish pending queue; update time accumulator |
| post (char) | `CCharInstance::SkeletonPostProcess` | `31520860` | 1,644 | char-level post process: publishes the animation transform (`FUN_3151e930`), updates skeleton effects (`CSkeletonEffectManager::Update 31523f80`) |
| post (pose) | `CSkeletonPose::SkeletonPostProcess` | `3154b4b0` | 7,920 | absolute bone matrices, post-process IK, physics sync, attachment prep |

There are two different `SkeletonPostProcess` methods: `CCharInstance::SkeletonPostProcess 31520860`
(char/entity glue and effects) and `CSkeletonPose::SkeletonPostProcess 3154b4b0` (the final pose).
Do not merge them in the port.

`PoseModifiersPrepare 3154e3a0` (783 B) builds the pose-modifier chain: `AnimationPoseModifier_LimbIk`
and `AnimationPoseModifier_PoseBlenderAim` are the named modifiers (`cryanimation` strings 0x316766b4
and 0x3167ab60). The modifiers are applied to the final pose, after the clip/LMG sample and before
skinning.

`FUN_315a1250` (928 B) after the command loop:
* runs `FUN_315bdc20`, `FUN_315bcec0`, `FUN_315be0d0` (pose/skinning transform scheduling; the same
  helpers are used by `SkeletonPostProcess`),
* copies the layer pose to the final matrix block,
* calls `ProcessAnimationDrivenIk 315a0630` (3,092 B),
* optional `FUN_315a00a0` debug dump (`DAT_316c924c`).

`SkeletonPostProcess 3154b4b0` is the final pose:
* takes the local pose + entity transform, produces absolute bone matrices,
* runs physics synchronisation (`CSkeletonPose::SynchronizeWithPhysics`, SPU variants) when the skeleton
  has a physical entity,
* applies the post-process quaternion / limb IK (`FUN_31599dc0`, `FUN_31599e80`) when the pose has a
  post-process setup,
* calls the pose callbacks (`FUN_3154dc70`); the result is then consumed by
  `CSkinInstance::UpdateAttachedObjects` (attachments) and the skinning/render task.

Skinning then runs on the task pipeline (`CSkinningTransformationsComputationTask::Begin/Wait/Execute`,
`315251e0/31525310/31525f40`) and `CSkinInstance::UpdateAttachedObjects` updates attachments with the
new matrices. `CharacterManager::SyncAllAnimations 315ca330` (290 B) flushes the per-frame animation
list and clears the done flags; it is the manager-side counterpart of the per-character finish.

---

## 7. Attachments: rider seat, weapons, `$IK_Grip`

### 7.1 Attachment objects

`CAttachmentManager` (at `char+0x30`) owns the attached objects. Two `AttachObjectToBone`
overloads exist:

| function | address | size | notes |
|---|---|---:|---|
| `AttachObjectToBone` (object overload) | `315078e0` | 531 | stores type `this+0xb8`, bone id `this+0xc4 = -1`, local offset/rotation at `+0x0c..+0x1c`, alignment type at `+0xd4`; resolves the bone via `char+0x254` pose `vt[0x18]`; missing bone logs the "AttachObjectToBone is called for bone ..." warning |
| `AttachObjectToBone` (name overload) | `31507b00` | 427 | same, resolving the bone name first |

Attachment types observed [I]: 0 = static object, 1 = character, 2 = entity (the rider is type 2), 4 =
aligned character. The object vector is `manager+0x08..+0x0c`; the animated-bone list is
`manager+0xac/+0xb0`, and `manager+0xa9` is the dirty flag.

### 7.2 Per-frame update order

| function | address | size | role |
|---|---|---:|---|
| `CAttachmentManager::UpdateAnimBones` | `3150df50` | 1,884 | called from `CSkeletonAnimTask::Begin`; advances animated attachments and builds the bone-usage bitmask |
| `CAttachmentManager::UpdateLocationAttachments` | `3150f340` | 12,725 | computes each attached object's world matrix from its bone; called from `CSkinInstance::UpdateAttachedObjects` |
| `CSkinInstance::UpdateAttachedObjects` | `315253f0` | 570 | calls `UpdateLocationAttachments`, then each object's `vt[+8]`/`vt[+0xc]` with the new matrix |
| `CSkinInstance::UpdateAttachedObjectsFast` | `31525630` | 485 | fast path when only the root bone moved |

`UpdateAnimBones` runs **before** the pose task (on the previous frame's bone matrices) [I] and
`UpdateLocationAttachments` runs **after** `SkeletonPostProcess`, so an attached object lags the
skeleton by no more than the task latency; both are in the same frame in the normal case. The
attachment manager copies the bone's world matrix multiplied by the stored local offset to the child.

### 7.3 The mounted rider

The rider is an **entity attachment of type 2** on the mount character. The seat bind is set up by
x2game `FUN_390e96a0` (4,268 B), which:

1. resolves the mount `ICharacterInstance` and its pose interface (vtable offsets in §1.1);
2. resolves the seat bone id (`bone_spine_driver` for the driver seat / `bone_spine_passenger` for
   the passenger seat, from `model_attach_point_strings`, mount map §B);
3. composes the seat offset (entry-side lean/offset math in `FUN_390c32d0`, `FUN_390bf0f0`) and
   calls the pose interface to store the bone/attachment matrix;
4. binds the rider child object (`entity vt[0x3c](pose, 2)`).

After the bind there is **no per-frame x2game seat code**. The mount's attachment manager updates the
rider entity transform from the seat bone every frame; the rider's own `CCharInstance` then evaluates
its own graph/clips at that transform. The rider graph is a full separate `player_full_body` graph
with `Vehicle`/`VehicleSeat` inputs (mount map §B). This is why the rider's pose on the mount is the
combination of:

* the seat bone matrix (mount animation),
* the rider entity transform (attachment manager),
* the rider's own animation (rider graph + rider clips + LMG).

The engine-side rider coupling `SyncRiderAnimation 3154dd70` (699 B) additionally copies the rider's
FIFO timing/weight data into the mount's matching entries (same animation-set id at asset `+0x40`):
`entry[+0x20/+0x24/+0x10]` and the LMG block `0xa4..0x144` are copied from rider to mount. This keeps
the rider's legs and the horse's gait in phase when both use the same locomotion asset family.

### 7.4 Weapons and other attachments

Weapons/shields use the same `CAttachmentManager`; their bindings are item/model data (see
`docs/research/equipment-attach.md`). `UpdateLocationAttachments` handles the per-type transform
(static object, character, entity), including animated weapon bones. The animation side only copies
the bone matrix; it does not decide which weapon is equipped.

### 7.5 `$IK_Grip`

`$IK_Grip_L` / `$IK_Grip_R` are **model bones**, not a CryAnimation feature. They are used by the
x2game vehicle code (`UpdateDriver 390d0800`, 2,016 B; see `port-vehicles/MAP.md`) to IK the driver's
hands to the helm/steering wheel. There is no `$IK_Grip` string in `cryanimation.dll`, and no grip code
in the horse/rider path. A horse saddle has no grip bones. Do not add grip IK to the mount port.

---

## 8. IK and post-process

### 8.1 Animation-driven IK (ADIK) `FUN_315a0630` (3,092 B)

Runs at the end of the command executor, on the final layer pose. It consumes the model's ADIK setup:

* `char+0x11c` = target list, `char+0x120` = bone target list (stride 0x18 and 0x28 respectively).
  Each record carries source bone, target bone, a tag and a blend value.
* Tag `0x4b494232` (`"2BIK"`) -> two-bone IK (`FUN_315b77a0`);
  `0x4b494233` (`"3BIK"`) -> three-bone IK (`FUN_315b8f20`);
  `0x58444343` (`"CCDX"`) -> CCD chain (`FUN_315b6340`).
* The result is written back into the pose bone array; children are re-composed when a parent changes.
* Debug string `LHand_IKTarget: name: ...` and cvar `ca_DebugADIKTargets` / `ca_ShowADIKTargets`.

This is the general "animation-driven IK" used by CHR params and test IK (`CryCHRLoader::LoadTestIKs`);
it is not the horse stirrup solution. It can target any named bone, so a modded rig could use it for
hands/feet, but the shipped horse/rider rigs do not define stirrup targets.

### 8.2 Pose modifiers `FUN_3154e3a0` (783 B)

`PoseModifiersPrepare` builds the modifier chain from the model:
`AnimationPoseModifier_LimbIk` (0x316766b4) and `AnimationPoseModifier_PoseBlenderAim` (0x3167ab60).
The modifiers are applied after clip/LMG blending and before the absolute-matrix pass. Aim/look IK is
the `PoseBlenderAim` modifier; limb IK is the procedural two-bone helper.

### 8.3 Look IK

`CSkeletonPose::SetPostProcessQuat 3152e450` (425 B) stores a post-process quaternion and blend weight
in the pose. `SkeletonPostProcess` applies it through `FUN_31599dc0` (181 B) / `FUN_31599e80` (1,263 B),
which copy the pose subtree and rotate the bones below the changed parent. Cvars:
`ca_UseLookIK`, `ca_DrawLookIK`, `ca_UseAimIK`, `ca_UseIMG_AIM`, `ca_UseAimIKRefPose`,
`ca_DrawAimIKVEGrid`, `ca_DrawAimPoses` (strings `0x31684518..0x31684ea8`). Missing setup logs
`CryAnimation: No LookIK-Setup for model: %s`.

This is generic engine look/aim, driven by the game/entity (target look-at). It is not part of the
gallop or the rider seat; it can be ported later for combat/camera look.

### 8.4 Foot IK / stirrups

CryAnimation contains the foot-IK machinery but no stirrup-specific code:

* cvars `ca_LockFeetWithIK`, `ca_FootAnchoring`, `ca_DebugFootPlants`, `ca_DrawFootPlants`
  (`0x31684358..0x31684ba0`);
* foot-plant chunk `ChunkType_FootPlantInfo` / `FOOTPLANT_INFO` (`0x316805a8`, `0x31681338`);
* the actual solve is inside `CSkeletonPose::SkeletonPostProcess` (7,920 B), which was not decoded
  line by line [R for the entry/cvars, ? for the full foot solve].

There is **no "stirrup" string** in `cryanimation.dll` or `x2game.dll`. The rider's feet in the stirrups
are authored in the rider clips (`fist_mo_rider_horse_idle/walk/run/...` and the `fist_ac_horse_*`
get-on/off clips) and positioned by the seat attachment; no engine IK holds them there. This is
consistent with the rider LMG list in D §4.4 and the mount map's get-on/off clips.

### 8.5 What phase 1 needs

For the gallop and the mounted rider, only these post-process pieces matter:
* final absolute matrices and skinning (`SkeletonPostProcess`, skinning task),
* attachments (seat bone -> rider entity),
* rider sync (`SyncRiderAnimation`).

Foot IK, look IK, aim IK and ADIK can be stubbed as no-ops until E3/combat needs them, provided the
rider rig's clips keep the feet in the stirrups. That is the phase-1 trade-off to validate against the
owner's capture.

---

## 9. Phase 1: the gallop and the mounted rider

### 9.1 Horse gallop, frame by frame

| # | link | where | why it matters |
|---:|---|---|---|
| 1 | graph `PseudoSpeed`/`Stance`/`RequestedTurnSpeedLZ` from the rider copy | `x2game FUN_390dcc80`, `cryaction FUN_30665830` | selects Run and the LMG blend position; wrong input = wrong gait |
| 2 | graph state -> LMG desc + weights | `cryaction CAnimationGraphState::Update`, `cryanimation FUN_31558f80`, `FUN_31558550`, `FUN_31556e90` | chooses the example set and the per-example weights |
| 3 | layer speed spans from actual velocity | `FUN_31558550` case 2 | maps movement speed to LMG blend, not to clip rate directly |
| 4 | command `+0x14` playback speed + `+0x44` scale | `FUN_3155b350`/`FUN_3155ae00` | sets the clip rate divisor |
| 5 | per-frame phase step | `FUN_315557d0` -> `FUN_31555240` | `time += dt*scale*speed/segmentDuration`; this is the stride clock |
| 6 | clip sample + blend | `FUN_315a2160`, `FUN_315a3100` | visible leg/body pose at that phase |
| 7 | final pose + skinning | `FUN_3154b4b0`, skinning task | what is drawn |
| 8 | rider seat + rider graph | `FUN_3150f340`, `FUN_3154dd70`, rider graph | rider follows the saddle and keeps its own gait phase |

**Most important links for feel:** 4, 5, 6, 8. A correct state machine with a wrong `entry+0x14` or a
wrong segment duration gives the "floaty/wrong stride" feel; a correct stride with a stale seat matrix
gives a rider that slides on the saddle.

### 9.2 Mounted rider, frame by frame

1. Mount `ProcessAnimations` advances the mount FIFO and calls `SyncRiderAnimation`; if the rider is
   mounted (`mountAnim+0x4a0 != 0`) and has a matching animation set, the mount copies the rider's
   current time, weights and LMG block (`+0x10`, `+0x20`, `+0x24`, LMG `0xa4..0x144`).
2. `CSkeletonAnimTask::Begin` -> `CAttachmentManager::UpdateAnimBones` (previous pose).
3. `Execute` -> pose of the mount, including the seat bone `bone_spine_driver`.
4. `FinishAnimationComputations` -> `SyncData`.
5. `SkeletonPostProcess` -> absolute mount bone matrices.
6. `CSkinInstance::UpdateAttachedObjects` -> `UpdateLocationAttachments` writes the seat-bone matrix to
   the rider entity (type 2 attachment).
7. The rider's own `CCharInstance`/graph/pose pipeline runs with that entity transform and its own
   clips (`fist_mo_rider_horse_*`), so the legs/hips are authored on the saddle.
8. No stirrup IK, no grip IK, no root motion on either entity.

The rider graph itself is updated by the ordinary player graph state; the mount->rider graph input
copy (`FUN_390dcc80`) only feeds `PseudoSpeed`/`Stance`/`RequestedTurnSpeedLZ`. The seat matrix is the
only positional coupling.

### 9.3 Port order (testable leaves first)

1. **Pure leaves (no live actor):**
   * time advance `FUN_31555240` (scratch 0x88 entry),
   * speed formula `FUN_315557d0` (scratch entry + a live/synthetic character animation set),
   * transition ramp `FUN_31554ba0`, normalize `FUN_315547a0`, layer ramp `FUN_31555070`,
   * `SAnimation` copy `FUN_315523b0` / `FUN_31552950`,
   * locator sample extraction `FUN_3154e030`, angle resolve `FUN_31554560`,
   * command dispatch `FUN_315a1250` with a hand-built command buffer and scratch pose.
2. **Command buffer + evaluators:** `FUN_31557cb0`, `FUN_31557af0`, `FUN_31557860`,
   `FUN_31556e90`, then `FUN_315a2160` / `FUN_315a3100` against a known CAF/LMG.
3. **FIFO manager:** `ProcessAnimations/ProcessAnimationUpdate`, `BlendManager`,
   `LayerBlendManager`, `SyncRiderAnimation`, `ParseLocator` against a live character.
4. **Task + pose:** `CSkeletonAnimTask` Begin/Execute/Wait, `SkeletonPostProcess`, skinning task
   scheduling.
5. **Attachments:** `AttachObjectToBone`, `UpdateAnimBones`, `UpdateLocationAttachments`,
   `CSkinInstance::UpdateAttachedObjects`; then the x2game seat bind and a live mounted rider.
6. **IK:** start with no-op pose modifiers; add ADIK `FUN_315a0630`, then look/aim, then foot IK.
7. **Rider sync / seat golden:** compare rider bone world matrices and horse stride phase against the
   hidden client (the F-live-capture harness in `port-mount-anim/F-live-capture.md`).

Each step's gate: byte-identical FIFO entry, command buffer, pose array or bone matrix for the
captured inputs; differences go to §11.

### 9.4 Differential harness recipes

Use `aaemu-clientdriver` (`call`, `read`, `alloc`, `write`, `dump`; see `port-mount-anim/MAP.md` §3).
x86 `__thiscall` = `this=` plus stack args. All addresses are RVAs from `cryanimation+0x…`.

```text
# 1. pure time advance: FUN_31555240(this, entry*, layer, count, index)
alloc 0x88 -> E
write E <entry bytes>                    # set +0x10 current, +0x28 speed, +0x54 flags
call cryanimation.dll+0x55240 thiscall this=<CSkeletonAnim> p:E 0 1 0
read E 0x88                              # compare +0x0c, +0x10, +0x34

# 2. speed formula: FUN_315557d0(this, entry*, count, speedIn, layer)
call cryanimation.dll+0x557d0 thiscall this=<CSkeletonAnim> p:E 1 f:0.016 0
read E 0x88                              # +0x24 segment duration, +0x28 phase step

# 3. transition ramp: FUN_31554ba0(fifoHeader*, count, speedIn, rawFlag, rawDt)
#    fifoHeader = &firstEntry (vector start/end/cap)
call cryanimation.dll+0x54ba0 thiscall this=<fifoHeader> 1 f:0.016 0 f:0.016
read E 0x88                              # +0x18 weight

# 4. normalize: FUN_315547a0(fifoHeader*, count)
call cryanimation.dll+0x547a0 thiscall this=<fifoHeader> 1
read E 0x88                              # +0x1c weight

# 5. layer ramp: FUN_31555070(this, dt, layer)
call cryanimation.dll+0x55070 thiscall this=<CSkeletonAnim> f:0.016 1
read <char+layer*0x30+0xe4> 0x0c

# 6. command executor: FUN_315a1250(buffer*) (__fastcall, one arg)
alloc 0x880 -> B
write B <buffer header + commands>       # +0x800 cursor, +0x804 count, +0x808 data
call cryanimation.dll+0xa1250 cdecl p:B
read B 0x880

# 7. clip sample: FUN_315a2160(this=cmd*, buffer+0x808, scratchBuf)
call cryanimation.dll+0xa2160 thiscall this=<cmdPtr> p:<buffer+0x808> p:<scratch>

# 8. locator: FUN_3154fda0(this, count)
call cryanimation.dll+0x4fda0 thiscall this=<CSkeletonAnim> 1
read <this+0x64c> 0x1c
```

For a live `CSkeletonAnim`, use the local player/horse and the pointer recovered through the actor
graph-input holder (the same route as `port-mount-anim` F-live-capture). Restore any mutated FIFO
entry, or clone a layer into scratch memory, because the animation tick will otherwise keep the
modified state.

### 9.5 Numeric parameters and constants

| symbol / offset | value / role | source |
|---|---|---|
| `char+0x278` | `fAnimSpeedScale`, default **1.0** | `FUN_31520270`/`FUN_31520290`, property loader `FUN_3151aab0` |
| `char+0x240` | raw `ITimer::GetFrameTime()` seconds | `SkeletonPostProcess`, `StartAnimationProcessing` |
| `char+0x238` | `+0x278 * +0x240` (scaled delta) | same |
| `char+0x468` | additive/attack speed scalar used when `entry+0x85` | `FUN_315557d0` |
| `_DAT_31677660` | minimum segment duration (value not in the text decompile) | `FUN_315557d0` |
| `DAT_31677bf0` | maximum clip time / wrap limit (value ?) | `FUN_31555240` |
| `DAT_316768ac` | maximum per-frame time step clamp (value ?) | `FUN_31555240` |
| `DAT_316745a0` | 1.0 | many uses |
| `DAT_316745a8` | 0.0 | many uses |
| `DAT_31674584` | 0.5 | halving / quaternion normalization |
| `DAT_316745e0` | 0x7fffffff (abs mask) | time/weight math |
| `DAT_316746f4` | time/weight epsilon (value ~0.001) | weight/segment comparisons |
| FIFO entries blended | max **8** per layer | `FUN_31556910` |
| FIFO layers | **16** (`char+0xcc`, stride 0x30) | `ProcessAnimationUpdate` |
| desc layers | **13** (`char+0x19c`, stride 0x3c) | `FUN_31558f80` |
| event queue | max **5** events (`char+0x4f4`) | `FUN_315554d0` |
| command buffer | **0x800** bytes, cursor `+0x800`, count `+0x804`, data `+0x808` | `FUN_31557cb0`/`FUN_315a1250` |
| command sizes | 0:4, 1:4, 2:0x1c, 3:0x18, 4:0x118, 5:0x10, 6:0xc, 7/8/9/a/b/d/e:8, f:4 | `FUN_315a1250` switch |
| FIFO entry stride | **0x88** | `FUN_3155bbb0` |
| CAF header stride | **0xdc** (`DAT_316c2254+0xb4`) | `FUN_3160cc50` users |
| LMG header stride | **0x48** (`DAT_316c2254+0xb8`) | LMG loader/D |

Exact values of the `DAT_*` constants need the `.data` dump; only the roles are proven by the
decompile. Do not hard-code a guessed epsilon or max time without that read.

### 9.6 Key function table

All functions are in `cryanimation.dll` unless the caller column says otherwise (x2game/entity).
Static callers are listed when the decompile has an internal xref; "virtual" means the function is
an interface method with no static caller. "State" = pure (no object mutation) or stateful with the main offsets. Sizes are
`funcs.csv`. The last column gives the cheapest in-process differential check.

| function | addr | size | caller(s) | callees | state / offsets | diff recipe |
|---|---:|---:|---|---|---|---|
| `CSkeletonAnim::StartAnimationById` | `3155b350` | 1,617 | virtual (`ISkeletonAnim`) | `315523b0`, `3155ae00`, `315445c0`, `3160cc50` | stateful; `char+layer*0x30+0xe8/+0xec`, `char+0x4a6` | live call + queue dump |
| `CCharInstance::StartAnimationProcessing` | `3151b220` | 535 | virtual (entity) | `3151b150`, `3151a1c0`, `3151e930` | stateful; `char+0x29c..0x2b8`, `+0x24c` | live call + `read char+0x29c` |
| `SetAnimationSpeed` | `31520270` | 20 | virtual/property | - | pure setter; `char+0x278` | `call` + `read` |
| `GetAnimationSpeed` | `31520290` | 7 | virtual/property | - | pure getter; `char+0x278` | `call`, compare EAX/ST0 |
| `CSkeletonAnim::ProcessAnimations` | `31550b90` | 1,336 | virtual (`ISkeletonAnim`) | `31550610`, `3154fbf0` | stateful; `+0x4a4`, `+0x4a7` | live, once per frame |
| `ProcessAnimationUpdate` | `31550610` | 500 | `31550b90` | `31556910`, `31555070`, `3154dd70`, `3154fda0` | stateful; per-layer FIFOs, `+0x64c` | live FIFO dump |
| `BlendManager` | `31556910` | 822 | `31550610` | `31554ba0`, `315547a0`, `31556540`, `31554e80`, `315559e0` | stateful; entry weights/times | scratch entry + fixed dt |
| transition ramp | `31554ba0` | 726 | `31556910` | - | pure on entry; `+0x18`, `+0x3c` | scratch entry |
| weight normalize | `315547a0` | 924 | `31556910` | - | pure on entry; `+0x1c` | 2-8 scratch entries |
| speed formula | `315557d0` | 525 | `31556540` | - | pure on entry; `+0x24`, `+0x28`, `+0x44` | scratch entry + live char |
| time advance | `31555240` | 654 | `315559e0` | - | pure on entry; `+0x0c`, `+0x10`, `+0x28` | scratch entry |
| animation events | `315554d0` | 510 | `315559e0` | - | stateful; `char+0x4f4` event array, `+0x648` count | scratch entry, compare event bytes |
| layer ramp | `31555070` | 449 | `31550610` | - | stateful; `char+layer*0x30+0xe4/e8/ec` | live/scratch layer header |
| `SyncRiderAnimation` | `3154dd70` | 699 | `31550610` | `31551f50` | stateful; `+0x4a0`, rider FIFO | live mount+rider |
| `ParseLocator` | `3154fda0` | 1,337 | `31550610` | `3154e030`, `3154e9f0`, `31554560` | stateful; `+0x64c..0x664` | live FIFO, read locator |
| locator extraction | `3154e030` | 535 | `3154fda0` | `3160cc50` | pure-ish clip; clip `+0x5c/+0x60/+0x30` | scratch clip + entry |
| `CSkeletonAnimTask::Begin` | `3154fbf0` | 427 | `31550b90` | `3150df50`, `3154e8a0` | stateful task; `task+0x6c/+0x70` | live |
| `CSkeletonAnimTask::Execute` | `315502e0` | 542 | task/job | `31557cb0`, `315a1250` | stateful task | live |
| command writer | `31557cb0` | 1,278 | `315502e0` | `31557af0`, `31557860`, `31556e90` | stateful; buffer, `char+0x474` | live/scratch FIFO |
| LMG expander | `31556e90` | 2,488 | `31557af0` | `3160cc50`, caps | stateful; buffer | scratch LMG + desc |
| command executor | `315a1250` | 928 | `315502e0` | 15 evaluators, `315a0630` | stateful; buffer, pose | hand-built buffer |
| clip sample | `315a2160` | 3,969 | `315a1250` | `315a1fd0` | stateful; `char+0x78` pose | scratch command + clip |
| LMG sample | `315a3100` | 1,750 | `315a1250` | `315a1fd0` | stateful; pose, bone mask | scratch LMG command |
| animation-driven IK | `315a0630` | 3,092 | `315a1250` | `315b77a0`, `315b8f20`, `315b6340` | stateful; `char+0x11c/0x120` | live pose (ADIK model) |
| `FinishAnimationComputations` | `31551350` | 282 | virtual | `3154e250`, `315510d0` | stateful; `+0x4a4` | live |
| `SyncData` | `315510d0` | 623 | `31551350` | `3154db10`, `3155ab20` | stateful; FIFO/pending | live |
| `SkeletonPostProcess` | `3154b4b0` | 7,920 | virtual (`ISkeletonPose`) | physics sync, `31599dc0/e80`, `3154dc70` | stateful; final matrices | live bone dump |
| `AttachObjectToBone` | `315078e0` | 531 | x2/entity attach | - | stateful; `this+0xb8/0xc4/0xd4` | scratch object + bone name |
| `CAttachmentManager::UpdateAnimBones` | `3150df50` | 1,884 | `3154fbf0` | `31527310`, `31508f10` | stateful; `+0xac/+0xb0`, `+0xa9` | live |
| `UpdateLocationAttachments` | `3150f340` | 12,725 | `315253f0` | `315095a0`, `31518a90`, ... | stateful; attachment matrices | live rider seat |
| `CSkinInstance::UpdateAttachedObjects` | `315253f0` | 570 | skinning/entity | `3150f340` | stateful; `+0x30` manager | live |
| `SetPostProcessQuat` | `3152e450` | 425 | game/look IK | - | stateful; pose quat | scratch pose |
| `CharacterManager::Update` | `315d3360` | 1,002 | engine frame | `3165ac00`, `315d0ee0`, ... | stateful global | live/once |
| `SyncAllAnimations` | `315ca330` | 290 | `CharacterManager` | - | stateful list | live |

All `FUN_315*` names are Ghidra addresses; the class/method names come from the profiler strings
listed in §1.3 and §6. Rows marked pure still read global animation/asset tables, so a scratch call
needs the live `CAnimationSet` pointer for that character.

### 9.7 Sizing

Counted from `cryanimation.dll.funcs.csv` (bodies as decompiled, not source lines):

| scope | functions | bytes | KB |
|---|---:|---:|---:|
| **whole cryanimation** | 7,163 | 1,416,108 | 1,382.9 |
| this map's key function set (74 functions) | 74 | 73,077 | 71.4 |
| phase-1 realistic scope: attach/skin/char `0x3150c000-0x31528000` | 483 | 109,520 | 107.0 |
| phase-1: skeleton anim `0x3154d400-0x3155c000` | 193 | 58,501 | 57.1 |
| phase-1: desc/LMG `0x31558000-0x31580000` | 441 | 158,992 | 155.3 |
| phase-1: command eval/IK `0x3159e000-0x315a4000` | 80 | 22,765 | 22.2 |
| **phase-1 total (union of the four ranges)** | **1,197** | **349,778** | **341.6** |
| **rest of cryanimation** | **5,966** | **1,066,330** | **1,041.3** |

The desc/LMG range includes the blend-type weight functions already mapped in D (`port-mount-anim`);
the **new** work for E2 is approximately 500-700 functions / 130-180 KB. Phase 1 does not need the
facial, streaming, CHR-loader, morph and SPU code that makes up most of the rest.

If the port uses the existing D graph runtime and only needs playback + posing + rider seat, the
minimal new set is the 74 key functions (71.4 KB) plus the task/attachment glue (~40-60 functions,
~30-40 KB), i.e. **~110-130 functions / ~100-120 KB**.

### 9.8 What is proven vs inferred

Proven (decompile/bytes/data):
- the x2game->entity->character interface offsets in `FUN_390e96a0` and the seat bind;
- `CSkeletonAnim` owner `+0x498`, rider `+0x4a0`, FIFO 16x0x30, entry 0x88, command buffer 0x800;
- `char+0x238 = fAnimSpeedScale * frameTime`, default 1.0;
- time advance `time += entry[+0x28]`, phase step formula, 8-entry blend cap, 13 desc layers;
- transition/layer weight ramps and normalization;
- command dispatch (all 15 command ids) and the clip/LMG evaluators' entry points;
- upper-body masking is the `char+0x764` per-bone layer bitmask serialized at `cmd+0x18`;
- rider sync copies rider FIFO timing/LMG data into the mount;
- seat attachment is a type-2 entity attachment updated by `UpdateLocationAttachments`;
- no `$IK_Grip`/stirrup code in the horse/rider path; `$IK_Grip_L/R` belong to x2game vehicles;
- ADIK, look/aim IK and foot-IK cvars/entry points.

Inferred / unresolved: §10.

---

## 10. Unknowns (ranked)

1. **`PseudoSpeed` producer and the exact graph speed -> `entry+0x14` mapping.** `port-mount-anim` §6.4
   still has this open. E2 proves the playback side; a live FIFO read of a galloping horse is needed to
   close the loop. This is the highest-value unknown for feel.
2. **`entry+0x44` provenance.** `FUN_315557d0` reads it as a float playback scale; the `SAnimation`
   copy has a refcounted pointer at the same offset in its own layout. The FIFO entry is the reading
   used by every formula here; confirm with a live entry dump.
3. **Exact CAF/LMG segment lengths.** The stride seconds depend on the authored clip times (`+0x5c/+0x60`
   lanes, `+0x30` scale). They are data, not decompile; the port should read them from the same CAF/LMG
   files, not hard-code.
4. **Foot IK solve inside `SkeletonPostProcess`.** The 7,920 B body was not decoded line by line; only
   the cvars, foot-plant chunk and the post-process quaternion helpers are proven. Need a separate
   pass if foot anchoring is visible on the horse/rider.
5. **Look/aim IK modifier math.** `AnimationPoseModifier_PoseBlenderAim` and `LimbIk` are named, but
   their pose math is not decoded. Not needed for phase 1 if the rider clips hold the feet.
6. **Rider sync authority.** `SyncRiderAnimation` copies rider -> mount matching entries. Why the
   mount takes the rider's time (rather than the reverse) and what happens without a matching asset is
   [?]. It only runs when `mountAnim+0x4a0 != 0` and the asset id matches.
7. **Attachment frame latency.** `UpdateAnimBones` runs in `Begin` (previous pose) and
   `UpdateLocationAttachments` after `SkeletonPostProcess` (current pose). Confirm whether the rider
   seat uses the current or previous frame's bone in the hidden client.
8. **Root motion application.** E2 produces the locator and motion feedback; E3 must prove whether
   `CAnimatedCharacter::ClampAnimLocation`/`CalculateAnimRenderLocation` ever applies it to the entity.
   For the horse/rider the current evidence says no.
9. **Task threading.** The task Begin/Execute/Wait order is from profiler names and the task object; the
   job-system interleaving is E1.
10. **Command 0xb target.** `FUN_315a1cf0` calls a vtable on `this+4` with a pose/event struct; the
    consumer (animation event sink / effect spawn) is [?]. It is likely the animation-event/sound/fx
    bridge, not pose.
11. **Layer count mapping.** 8 graph layers (CryAction) vs 16 FIFO/13 desc (CryAnimation). Which graph
    layer maps to which FIFO slot is data (`AnimationLayer1..8` properties + model setup); the default
    full-body/upper-body are known, the rest [?].
12. **Numeric constants.** `_DAT_31677660`, `DAT_31677bf0`, `DAT_316768ac`, the exact `DAT_316746f4`
    epsilon: values need the `.data` dump.

---

## 11. Files and cross-references

- `docs/research/port-mount-anim/MAP.md` (roadmap parent; graph runtime and mount flow)
- `docs/research/port-mount-anim/D-graph-runtime.md` (graph states, LMG caps, command writer)
- `docs/research/port-movement/MAP.md` (movement request; why animation does not move the mount)
- `docs/research/port-vehicles/MAP.md` (`$IK_Grip_L/R`, `UpdateDriver`)
- `docs/research/port-program.md` (roadmap v2, phase 0 E2)
- `docs/research/equipment-attach.md` (weapon attachment data)
- `docs/research/re-mounts-full.md` (horse/rider clip names, get-on/off, saddle)

Function tables in this document use the `funcs.csv` sizes; bodies marked *(no entry)* are split or
unexported and are described by their observed block size.

---
