# Which per-frame update functions run for the local player in 1.2 (W0)

Recon, 2026-10-27. Fresh capture from the original 1.2 client (r208022,
`x2game.dll` TimeDateStamp `0x543CB835`, SizeOfImage `0x1CB0A00`) on the
`move-golden` lane, slot `wp6` (`wp6a` / `MgWp6a`), hidden, against the lane's
Rust server.

Evidence:

* `tools/onfoot_update_probe.py` - the capture script (committed).
* `tools/evidence/onfoot_update/report.txt` - generated per-scenario report.
* `tools/evidence/onfoot_update/summary.json` - generated counts, positions and checks.
* `tools/evidence/onfoot_update/per-frame-calls.csv` - generated per-update-pass rows.
* `tools/evidence/onfoot_update/driver-trace.patch` - the read-only trace capability,
  lane commit `a61813286` on `recon/onfoot-trace`.
* Raw run: `data/extract/onfoot-update/onfoot-update-20261027-r9/`
  (git-ignored: traces, samples, exact commands, `ACTUAL_RUN_REPORT.md`).
  Earlier fresh runs r4, r6 and r7 in the same directory supplied the
  conditional-`3912b330` and mounted-9/10 observations; the final r9 run is the
  one reproduced by the committed probe.

The driver DLL used for the run is SHA256
`a6ba1835b302961d082b878a1d3a574762b813a25e63cc5ea499598db798d9e7`
(built by `native/build.bat`, which runs the stand-alone thunk selftest).
The client binaries are `archeage.exe` SHA256
`a6fbd9ff58bf749fd995489f34acb01e1bc84d4d5a533721b0c017562766fc2a`
and `x2game.dll` SHA256
`3821c34366ff8b6f7aed95ba53d12216a5c31cb7c3407703d28eea639a621205`.

## Method

The probe launches the hidden original through `aaemu-clientdriver`, enters the
world, and installs a read-only entry detour on 13 `x2game.dll` functions. Each
detour saves flags/registers, increments an in-DLL counter, records
`(function, ECX)` and then executes the relocated original prologue before
jumping back. No game state is written; the synthetic selftest in
`native/trace-test.cpp` covers a 5-byte `mov` prologue and a 9-byte SEH prologue.

One trace row is flushed at the start of the **System** Lua state's
`CScriptSystem::Update`. For each actor, every instrumented function appears
once per row (plus the state branch), and the client's own Present counter
(`aaemu-clientdriver ping frames=`) matches the rows: rows/render = 0.99-1.07
in the final run, the excess coming from the `clear`/`dump` boundaries.
`System.GetFrameID()` advanced about twice per row in the same run, so it is
not the row unit.

The local actor is identified exactly, without guessing:

* Lua `PE.__this` (the player entity userdata) equals `[actor+0xc]` for the
  candidate CPlayer object;
* its entity position at `entity+0x28` matches `PE:GetWorldPos()`.

Live vtables (read with the driver and, for the controller,
`ReadProcessMemory`):

* CPlayer object vtable pointer `0x399AD450` (a window into the class's vtable;
  the first entry is `39131F80`). Relative slots: `+0x28` ->
  `391387C0`, `+0x1a4` -> `39135ED0`, `+0x1a8` -> `39135FE0`,
  `+0x1bc` -> `39133280`.
* Quadruped object vtable `0x399AC918`; `+0x28` -> `3911FEB0`,
  `+0x1bc` -> `39129800`.
* `[actor+0x24]` is the movement-controller object; its vtable is
  `0x30746730` in `cryaction.dll` (base `0x304F0000`, size `0x548A00`). None of
  the 13 traced functions is in that vtable, so the actor movement functions are
  actor vtable slots called by `CActor::UpdateMovementRequest`, not controller
  slots.

Because the Quadruped dispatcher is called with a stack-local context in ECX,
its entries are attributed by nesting inside the preceding
`Quadruped::PrePhysicsUpdate` call rather than by ECX.

## Answer: on foot

Measured per update pass (the row unit), in order, **exactly once each**:

| # | function | RVA | decompile | call site |
|---|---|---|---|---|
| 1 | `CPlayer::PrePhysicsUpdate` | `0x143280` | 233948 | vtable `+0x1bc` |
| 2 | `CActor::UpdateMovementRequest` | `0x128650` | 219255 | called at 233987 |
| 3 | `CPlayer::ProcessFrameMovementRotation` | `0x145ED0` | 235447 | vtable `+0x1a4`, call at 219374 |
| 4 | `CPlayer::ProcessFrameMovement` | `0x145FE0` | 235491 | vtable `+0x1a8`, call at 219454 |
| 5 | `FUN_39113f60` | `0x123F60` | 216241 | called at 233988 |
| 6 | `FUN_391387c0` (`CPlayer::Update` wrapper) | `0x1487C0` | 236825 | vtable `+0x28` |
| 7 | `CActor::Update` | `0x12FEB0` | 224683 | called at 236838 |
| 8 | `CActor::UpdateStats` | `0x12EFF0` | 223817 | called at 224840 |

The flat recorded order in every standing, forward, Q/E strafe, A/D turn and
jump row was `1,4,6,7,5,12,8,9` (function ids as in the probe). 3 and 4 are
nested inside 2; 7 and 8 are nested inside 6.

Only in the fall state, after 8:

| # | function | RVA | decompile | condition |
|---|---|---|---|---|
| 9 | `FUN_3911c780` | `0x12C780` | 222108 | `param_1[0x14] == 3` at 223961 |
| 10 | `FUN_39117590` | `0x127590` | 218718 | called by 9 at 222142 |

Fall phase (GM `/move` +25 m): 72 of 98 update passes had
`1,4,6,7,5,12,8,9,10,11`; the other 26 had no 9/10. Nothing else was ever
recorded for the local actor. The standing and running jumps did **not** enter
this branch, even when the running jump rose (0.92 m peak in the final run;
5.37 m in the earlier r4 run). So 9/10 are not the generic jump/airborne pair.

Never run for the local player on foot in any phase:

* `Quadruped::PrePhysicsUpdate` (`0x139800`);
* `FUN_3912dd40` (`0x13DD40`);
* `FUN_3912b330` (`0x13B330`, the quadruped ground mover).

They do run in the same rows for other actors (NPC/other Quadrupeds), but the
local CPlayer never enters them (final-run check: no 0/2/3 in any on-foot local
row).

## Answer: mounted (contrast)

While mounted the local actor is still the same CPlayer class, and its rider
chain continues:

* every pass: `1,4,6,7,5,12,8,9`;
* the 9/10 pair was present on every mounted pass in the final run
  (60/60, 73/73, 47/47 and 59/59 in stand, forward, turn and jump); the earlier
  r7 raw run had only a subset of passes (14/24, 5/29, 12/21, 19/27), so the
  `[actor+0x14] == 3` branch is state-dependent. Its exact semantics (fall vs.
  mounted/airborne) remain open.

The mount is a separate Quadruped actor (found by the horse entity's
`__this`). Its per-pass chain is:

| # | function | final run |
|---|---|---|
| 1 | `Quadruped::PrePhysicsUpdate` (`0x139800`) | every pass |
| 2 | `FUN_3912dd40` (`0x13DD40`) | every pass |
| 3 | `FUN_3912b330` (`0x13B330`) | **conditional**: 0/59 passes in the final run; 3 frames in each of the r4 and r6 runs, during the mounted jump |
| 4 | `FUN_39113f60` (`0x123F60`) | every pass |

The recorded mount order was `0,2,5` in the final run (0,2,3,5 for the 3 jump
frames in r4/r6). The dispatcher `FUN_3912dd40` calls the ground mover only on one branch;
the mounted actor spends most passes on another branch. The mount does not run
the CPlayer chain (1/4/6/7/12/8/9), so rider and mount are two independent
actors.

## Decompile and vtable evidence

Call edges (all in `D:/aa-re/client12/out-x2game/x2game.dll.c`):

* 233987/233988: `CPlayer::PrePhysicsUpdate` -> `CActor::UpdateMovementRequest`
  and `FUN_39113f60`.
* 219374/219454: `CActor::UpdateMovementRequest` -> actor vtable `+0x1a4`
  (`ProcessFrameMovementRotation`) and `+0x1a8` (`ProcessFrameMovement`).
* 236838: `FUN_391387c0` -> `CActor::Update`.
* 224840: `CActor::Update` -> `CActor::UpdateStats` when `actor+0x1c7 == 0`.
* 223961: `CActor::UpdateStats` -> `FUN_3911c780` when `actor+0x14 == 3`.
* 222142: `FUN_3911c780` -> `FUN_39117590`.
* 229216/229256: `Quadruped::PrePhysicsUpdate` -> `FUN_3912dd40`.
* 231312: `FUN_3912dd40` -> `FUN_3912b330` (one branch).
* 229265: `Quadruped::PrePhysicsUpdate` -> `FUN_39113f60`.

## Proof level

| statement | mark |
|---|---|
| On-foot per-pass order and one call each for 1-12 on the ground/jump | **R** (traced in the final run) |
| 9/10 run per pass in the fall state (72/98) after 8 | **R** |
| 9/10 do not run during standing/running jumps | **R** |
| Local CPlayer never enters 0/2/3 on foot | **R** |
| Mounted rider keeps 1,4,6,7,5,12,8,9 and sometimes adds 9/10 | **R** |
| Mount runs 0,2,5 per pass and 3 only on a conditional branch | **R** (final run plus r4/r6 raw runs) |
| One row = one rendered present (rows/render 0.99-1.07) | **R** |
| Nesting of 3/4 inside 2 and 7/8 inside 6 | **I** (decompile lines above) |
| Names/semantics of 5, 9, 10 | **I** (profiler strings/decompile; W4/W5 will name them from behaviour) |
| Standing jump lift is animation root motion | **I** (plan hypothesis; entity z did not follow in the final run) |
| Walking uses the same chain | **I** (walk key not exposed by the driver; ground movement only changes the speed factor) |
| Exact branch in `FUN_3912dd40` that selects `FUN_3912b330` | **?** |
| Meaning of `actor+0x14 == 3` (fall vs. mounted/airborne) | **?** |
| Full fall trajectory and landing | **?** (GM teleport; W5 needs a real ledge) |
| Mouse-look turning | **?** (A/D keyboard turn traced; mouse turn [I] same chain) |

## What this changes in the plan

* The on-foot movement math is the actor chain
  `PrePhysicsUpdate -> UpdateMovementRequest -> ProcessFrameMovementRotation ->
  ProcessFrameMovement`, then `FUN_39113f60`, then the update pass
  `FUN_391387c0 -> CActor::Update -> CActor::UpdateStats`. The Quadruped chain
  is not on the on-foot path; do not wire the local player through
  `39129800/3912dd40/3912b330`.
* The ground mover `3912b330` is a conditional leaf of `3912dd40`, not a
  per-pass function. W4 (jump) and the mounted work must capture the condition,
  not assume it runs every frame.
* The `3911c780/39117590` pair is a state branch (`actor+0x14 == 3`) reached in
  falls and intermittently while mounted; it is not the jump branch. W4 should
  not use it for jumping; W5 must capture the state transition, landing and the
  real fall trajectory.
* The actor update pass runs once per rendered present (rows/render 0.99-1.07),
  so per-frame counts are one call each as above. `System.GetFrameID` is not a
  frame clock (about 2x the rows in the final run); use the client Present
  counter or the trace row.
* The movement controller object (`actor+0x24`, `cryaction.dll` vtable
  `0x30746730`) is not where these 13 functions live. Resolve the CPlayer vtable
  slots `+0x1a4/+0x1a8/+0x1bc/+0x28` relative to the live vtable pointer
  `0x399AD450`, and the Quadruped vtable `0x399AC918`.
* The rider keeps its CPlayer chain while mounted; the mount is a second actor
  with its own Quadruped chain. Any port that replaces the player controller
  with a mount controller must still run the rider chain (and decide what 9/10
  do for the rider).

## Open items for the following work packages

* W1: the mouse-turn axis unit and the clamp; this probe did not exercise mouse
  turning.
* W4: standing vs. running jump (entity z, root motion, the `jump_in_place`
  branch); the call chain is fixed by this probe.
* W5: a real ledge fall with position, landing and the `actor+0x14` transition;
  the GM teleport started the state but did not integrate the whole drop.
* W6/W7: the packet cadence and rotation chain; the dispatcher's branch
  condition for `3912b330` and the mounted rider's 9/10 state.
