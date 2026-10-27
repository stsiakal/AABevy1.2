# CryAnimation step B: playback in the client and the update path

This note records what step B changed, how it was proven against the hidden 1.2 client, and what
is still blocked. Addresses are `cryanimation.dll` (preferred base `0x31500000`) and `x2game.dll`
(`0x38ff0000`) VAs. The step-A leaves and their goldens are described in `MAP.md` sections 3, 5,
7 and 9 and in `crates/aa-client/src/port/cryanim/`.

## 1. Wiring into the client

`crates/aa-client/src/ingame/anim_clock.rs` drives the base locomotion clip of the local player and
every mount through the ported clock:

* `FUN_315557d0` computes the segment duration (`+0x24`) and phase step (`+0x28`) from
  `char+0x238` (scaled frame delta) and the layer speed (`CSkeletonAnim+layer*0x30+0xe0`),
* `FUN_31555240` advances the entry phase (`+0x0c` previous, `+0x10` current) with 1.2's wrap and
  once flags.

The phase step is converted back to seconds for `aa_anim::Animator` (`external_advance`, keyed by
the clip data pointer so a clip another system started in the meantime is not affected). The
client's ground-speed match (`Motion::speed / clip ground speed`) is used as the graph playback
speed (`entry+0x14`) until the locomotion graph itself is ported. `--legacy-anim-playback` keeps the
old ground-matched advance for comparison.

`aa_anim::Animator` gained `external_advance: Option<(usize, f32)>` and `Playing::clip_key()`; the
fade, the layers and one-shot clips stay real-time.

### Recorder (mounted scenario, lane `anim-golden`)

| run | original stride / bob | Bevy stride / bob |
|---|---|---|
| before (docs baseline, `mounted-20261025-160629`) | 572.5 ms / 0.274 m | 575.0 ms / 0.291 m |
| after, graph rate forced to 1.0 (`mounted-20261025-190913-anim-b`) | 572.5 ms / 0.271 m | 511.1 ms / 0.285 m |
| after, graph rate = the client's ground-speed match (`mounted-20261025-191327-anim-b2`) | 575.0 ms / 0.271 m | 575.0 ms / 0.291 m |

The 511 ms run is the proof that the ported clock is the one advancing the clip: one authored CAF
cycle plays in 511 ms instead of the ground-matched 575 ms. The 1.2 original's effective gallop rate
is ~0.89 of the authored length, i.e. it is not 1.0 either; with `entry+0x14` unknown until the
graph is read live, the client keeps its ground-speed approximation and reproduces the previous
timing exactly (575 ms).

## 2. Ported functions

| function | address | size | port | live proof |
|---|---|---:|---|---|
| `ProcessAnimationUpdate` | `31550610` | 500 B | `cryanim/process.rs::fun_31550610` | 36 snapshot pairs (`update_b.json`) |
| `ProcessAnimations` | `31550b90` | 1,336 B | `cryanim/process.rs::fun_31550b90` | 21 processing snapshots (`update_b.json`) |

Snapshots come from 2,548 samples of the hidden client (on foot idle/run/stop, mounted
idle/gallop/stop; player and horse), captured by `tools/cryanim-b/capture.py` through
`ReadProcessMemory` while the client advanced real frames. A pair is kept only when one game frame
passed between the samples (`dphase * duration == char+0x238`, within 2 ms), the FIFO size did not
change (an insertion is `StartAnimationById`), and the sample was taken while the character was not
already processing.

The tests compare every 0x88-byte CAF entry (times, weights, `+0x24/+0x28`, flags) with a float
tolerance for the sampled frame delta, and every layer header. A CAF entry beside an LMG entry
proves its time/speed but not the transition weights, because the LMG per-example speed
(`FUN_31556540` and its leaves) and its share of the normalization are not ported.

### Decompile/disassembly corrections found while porting

* The dirty test is `entry+0x54 & 0x400`, but `ProcessAnimationUpdate` also has a stack argument
  that forces the pass (the `in_stack_00000040` of the decompile). The live client passes 1 while
  playing, which is why the entries advance without a 0x400 bit. The port exposes it as
  `ProcessInputs::force`.
* The layer speed is `header+0x14` (absolute `+0xe0`), not `+0x20`.
* `LayerBlendManager` returns immediately for layer 0; only layers 1..15 are ramped.
* The blend validation treats an entry as valid when `entry->asset != 0` (LMG) or the clip resolves;
  the earlier step-A helper had the condition inverted.
* `CAttachmentManager` is **embedded** at `char+0x30` (its vtable is the first dword and the object
  vector is at `manager+0x08/+0x0c`), not a pointer stored at `char+0x30`. A live `char+0x30`
  read is the manager vtable (`0x31675274`), which made an earlier attachment capture read garbage.

## 3. Blocked

| function | address | blocker |
|---|---|---|
| `StartAnimationById` | `3155b350` | the FIFO insert is `FUN_3155ae00` (1,347 B, instantiate/streaming/refcount), not ported; 9 live insertion pairs exist (idle->run, stop) but no port yet |
| `UpdateLocationAttachments` | `3150f340` | 12,725 B, multiple attachment types and pose-matrix paths; the type-0 output block (`object+0x50..0xa0`) and the type-2 block (`object+0x230..`) are identified, the line-by-line port is not done |
| x2game seat bind | `390e96a0` | 4,268 B; attach-point offsets (+-1, +-2) and the matrix composition are mapped in `port-engine-char/MAP.md`, but the live seat matrix golden (mount seat bone, rider local, attachment output) is not captured yet |
| LMG per-example speed | `31556540` -> `31565d20` / `31561820` / `31561a80` | outside step A/B; LMG entries keep their previous `+0x28` in the port, so their time is reported, not reproduced |

The live attachment evidence gathered so far (embedded manager, object vector, the changing output
blocks per type) is in the capture script and in this note; the seat proof is the next step.
