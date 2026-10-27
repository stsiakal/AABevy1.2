# Feel recorder: the original 1.2 client and Bevy, per frame

> **Restored 2026-10-27 from `archive/fork-main-2026-10-27`, UNVERIFIED.** The client was restarted from NickMesser's `main`; nothing described here is wired into the game. Addresses, call chains and offsets must be re-checked against the decompile ([R]/[I]/[?]) before use. A 2026-10-27 audit found 151 of the 269 EXACT/TOLERANCE labels in `proof-levels.*` weak or wrong (report: `D:\aaemu-client\handoff\2026-10-27\audit\report-proofs.md`).

The acceptance tool for every experience slice. One command runs the same scripted scenario in the
hidden original client and in the Bevy client, records the same channels from both, joins the
server's movement packets, and writes aligned CSVs, overlay plots and a side-by-side frame strip.

```bash
python tools/feel-recorder/run.py --scenario mounted
```

That is the whole flow: it builds `aa-client` (into `D:/aaemu-target/bevy-pr`), starts the
`move-golden` lane stack with the movement field trace on, launches the original (hidden) and Bevy
(`--headless --mute`, offscreen frames), drives the scenario through each client's own input path,
then writes everything under `shots/feel-recorder/<scenario>-<stamp>/`. The password is copied to a
temporary file in that output directory and deleted at the end. No OS-level input is used.

Useful flags: `--scenario on_foot|mounted|all`, `--no-build`, `--reuse-stack`, `--stop-stack`,
`--only original|bevy`, `--no-frames`, `--no-plots`, `--frame-interval 1.0` (original screenshots,
seconds), `--bevy-frame-every 30` (game frames), `--keep-clients` (debugging).

To compare with a binary built in another branch or worktree, pass `--bevy-exe PATH` (and optionally
`--bevy-label NAME`, default `bevy`). A given `--bevy-exe` is always treated as prebuilt: the
recorder does **not** build in that case, so it never writes another worktree's target directory.
The label is used for the plot/strip column and, when it is not `bevy`, for the output directory
suffix (`mounted-<stamp>-<label>`).

## Prerequisites

| thing | path / value |
|---|---|
| original client | `D:/AA1.2/bin32/archeage.exe` (read-only) |
| driver | `D:/aaemu-target/move-golden/debug/aaemu-clientdriver.exe` + `aadriver/aadriver.dll` |
| lane stack | `D:/aaemu-wt/move-golden` (`Scripts/live-stack.sh`), live dir `D:/aaemu-live-move-golden` |
| login | `127.0.0.1:28237` |
| Bevy target | `D:/aaemu-target/bevy-pr/debug/aa-client.exe` (or any `--bevy-exe`) |
| password source | `D:/aaemu-live-move-golden/cap-pw.txt` (copied then deleted) |
| accounts | original `movegold3` / `Movegoldc`, Bevy `movegold2` / `Movegoldb` |

The command restarts the lane stack unless `--reuse-stack` is given; it leaves it running unless
`--stop-stack` is given. Both characters must exist in that lane's database.

## What each client records

One row per game frame. Bevy's clock is pinned to `--feel-fps 60` (`TimeUpdateStrategy`), so its
CSV has even 16.67 ms rows even if rendering is slower; the original samples on a 16 ms Lua timer
(observed ~45 fps in the mounted run).

| column | original 1.2 | Bevy (`crates/aa-client/src/ingame/feel.rs`) |
|---|---|---|
| `player_x/y/z`, `player_yaw`, `player_vx/vy/vz` | `GetWorldPos` / `GetAngles` / `GetVelocity` on the player entity | `Mover` position/facing/velocity |
| `mount_x/y/z`, `mount_yaw`, `mount_pitch` | same calls on the nearest horse (`FEEL_HE`) | mount `RemoteUnit` position/facing, packet pitch |
| `mount_vx/vy/vz` | horse `GetVelocity` | not recorded (empty; plots derive speed from position) |
| `rider_x/y/z`, `rider_yaw` | `GetBonePos("Bip01 Pelvis")`, player actor yaw | rider pelvis bone `GlobalTransform`, `Mover.facing` |
| `rider_bone_yaw` | `GetBoneDir("Bip01 Pelvis")` (the drawn pelvis) | pelvis bone world rotation, local +Y in cry axes |
| `mount_bone_yaw` | `GetBoneDir("Bip01")` (the horse root bone) | mount root bone (`Rig::root`) world rotation, local +Y in cry axes |
| `cam_x/y/z`, `cam_dx/dy/dz` | in-process CameraManager read: eye `+0x64`, view target `+0x3c`, direction = target - eye | camera `GlobalTransform` translation/forward, converted to cry axes |
| `cam_yaw`, `cam_pitch` | derived from the camera direction | same derivation (not the orbit internals) |
| `player_state`, `mount_state`, `rider_state` | `actor:GetCurrentAnimationState()` | `Animator` current clip stem |
| `player_clip_ms`, `mount_clip_ms`, `rider_clip_ms` | `GetAnimationTime(0, 0) * 1000` (CryEntity binding) | `Animator` clip time |
| `server_x/y/z`, `server_mount_*` | last `CSMoveUnitPacket` at or before the row's wall time, from the lane server log (`fields=trace`) | same join |
| `frame`, `t_ms` | engine frame id; scenario clock | game frame; scenario clock |

`cam_yaw` is `atan2(dx, dy)` and `cam_pitch` is `atan2(dz, hypot(dx, dy))` for both clients, so the
columns mean the same thing even though Bevy's orbit camera stores yaw/pitch differently.

### Alignment

* Original `t_ms = 0` is the first Lua row after `_G.FEEL_RUN`; Bevy `t_ms = 0` is the `feel START`
  script mark. Rows before Bevy's start mark are dropped.
* The original's `GetWorldPos` is **zone-local**: on this map it is offset by about
  `(+6144, +11265)` from the server/Bevy world. The harness shifts player, mount, pelvis and camera
  x/y by the median of `server - local` over the run (the median rejects the one-packet lag at the
  start and around speed changes). The applied offset is logged.
* Server rows are joined by wall clock; `aligned/server.csv` keeps the raw packet stream
  (`wall_ms,obj,x,y,z,yaw`), while the two aligned CSVs repeat the last packet for the driven
  object and the mount in each frame row.
* Bone yaw is `atan2(-x, y)` of the bone's world **local +Y** (GetBoneDir in the original), in the
  same convention as the entity yaw. Each client's offset is calibrated on the first mounted row
  (scenario start, yaw 0); this removes only the bone's bind-pose axis, not a per-frame turn.

## Scenarios

`tools/feel-recorder/scenarios/mounted.json` (scenario b): summon and mount the horse, gallop
`W` 0-6 s, `Q+W` 6.5-8.5 s, `E+W` 9-11 s, turn `A` 11.5-13.5 s, stop. Start
`(10512, 15585, 232)`, yaw 0.

`tools/feel-recorder/scenarios/on_foot.json` (scenario a): run `W` 5 s, strafe `Q` then `E`, turn
`A`, jump. Same start position and facing.

The original is driven through the driver (`client_input`, Lua UI calls); Bevy through its script
runner (`keydown`/`keyup`). The original hides the tutorial in Lua; the Bevy script does the same.

## Output

```
shots/feel-recorder/mounted-<stamp>/
  aligned/original.csv, bevy.csv, server.csv
  plots/                     all channels + overview.png + rider_bob_centered.png
  original/raw_rows.txt      raw Lua rows
  original/camera.csv        wall_ms,x,y,z,dx,dy,dz (50 Hz in-process read)
  original/o_*.png           hidden-window captures
  bevy/feel.csv              raw 60 Hz Bevy rows
  bevy/feel.csv.marks.csv    SETUP/START/END
  bevy/frames/b_*.png        offscreen frames every --bevy-frame-every game frames
  strip.png                  original column vs Bevy column at 10 scenario times
  metrics.json               stride/bob/rider-yaw numbers per client
  bevy-script.txt, bevy.log
```

### Yaw provenance

* `player_yaw` / `mount_yaw` are the **entity** yaws: the original compares
  `player:GetAngles().z` with `horse:GetAngles().z`; Bevy compares `Mover.facing` with the mount
  `RemoteUnit.facing`. These are facing states, not necessarily the drawn pose.
* `rider_bone_yaw` / `mount_bone_yaw` are the **drawn** yaws: the original compares the world
  direction of `GetBoneDir("Bip01 Pelvis")` (rider) with `GetBoneDir("Bip01")` (horse root); Bevy
  compares the world rotation of the rider's pelvis bone with the mount rig's root bone
  (`Rig::root`), taking each bone's local +Y in cry axes. Both use `atan2(-x, y)` and are
  calibrated once on the first mounted row (yaw 0) to remove the bind-pose axis offset.
* The `rider_bone_yaw` is therefore the visual answer to "what is drawn", and the entity columns
  remain available to see whether the client's facing state matches.

## Scenario (b) numbers, final run

`shots/feel-recorder/mounted-20261025-160629` (original 772 rows @44.8 fps, Bevy 1010 rows @60 fps):

| metric | original 1.2 | Bevy | note |
|---|---|---|---|
| gallop mean speed | 9.195 m/s | 9.023 m/s | positions, 1.0-5.6 s |
| stride frequency | 1.747 Hz | 1.739 Hz | FFT of rider-seat height |
| stride period | 572.5 ms | 575.0 ms | 1 / frequency |
| rider bob amplitude | 0.274 m | 0.291 m | (max-min)/2 after linear detrend |
| rider bob RMS | 0.174 m | 0.180 m | |
| entity yaw diff, `Q+W` mean / max | +44.5 / +45.0 deg | 0.0 / 0.0 deg | `player_yaw - mount_yaw` |
| **drawn pelvis vs mount root**, `Q+W` mean / max | **+46.5 / +49.9 deg** | **-1.5 / 10.9 deg** | `rider_bone_yaw - mount_bone_yaw` |

The original `Q+W` entity mean varies between runs (35-45 deg) with how much of the 6.5-8.5 s
window the 45 deg plateau covers; the plateau itself and its maximum are stable at ~45 deg. The
pelvis-bone number is independent of that inflection: it confirms the rider's **drawn** pelvis is
turned too, by ~47 deg on average and up to ~50 deg during the strafe (the extra few degrees are
the pelvis animation). Bevy's drawn pelvis stays at ~0 (only the gait sway), so the entity value
was not hiding a turned visual.

The bob metric uses the rider pelvis height **above the mount root**, so terrain and the two
clients' diverging paths cancel out. The absolute `rider_z` channels differ by a constant (the
original's `Bip01 Pelvis` is its actor root/seat; Bevy's is the pelvis bone about 2 m above it).

## Open gaps (visible in the recorder)

1. **Rider yaw vs mount during strafe.** The original rider turns +45 deg on `Q+W` and -45 deg on
   `E+W` while the horse keeps heading 0. This is confirmed by the drawn data, not just the facing
   state: the rider's pelvis bone turns +46.5 deg (max +49.9) against the mount root, while the
   Bevy pelvis bone stays at -1.5 deg (max abs 10.9). The Bevy rider yaw is `Mover.facing`, the
   same value as the mount. Real client behaviour gap.
2. **Turn rate and input timing.** In the same run Bevy's `A` turn accumulates ~13.2 rad in 2 s
   (~378 deg/s) and keeps turning after key-up; the original turns ~116 deg in ~1.5 s (~77 deg/s).
3. **Mounted camera pivot (fixed).** The horse *was* rendered but fell below the frame: the camera
   pivot was the rider seat + the on-foot `PIVOT_HEIGHT`, ~3.6 m above the mount root, looking
   level. The recorder now lowers the pivot to the seated head (`SEATED_PIVOT_HEIGHT` 0.70 m above
   the seat, the original's mounted camera is at `cam_z - seat_z = 0.70 m`) when `LocalRider` is
   present. The strip now shows the horse in both columns.
4. **Different paths after `Q+W`.** Bevy briefly stops (`runtoidle`, speed ~0.5 m/s) when the
   diagonal starts, then runs left; the original keeps galloping through the strafe. By the `A` turn
   the two clients are several metres apart, so absolute position/z plots diverge after ~10 s.
   Scenario (b) numbers are taken before that.
5. **Clip time semantics.** The original now supplies `GetAnimationTime(0, 0)` (seconds), but the
   slot/layer index is not independently verified; it resets plausibly at state changes. The hook
   is `anim_ms()` in `run.py`'s `SAMPLE_LUA`; if a future build lacks the binding the column is
   empty rather than wrong.
6. **Mount velocity** is not in Bevy's raw CSV (`mount_vx/vy/vz` empty). The required mounted
   channels do not include it; `mount_speed.png` falls back to the position derivative.
7. **Camera pitch** at the gallop is about -3.9 deg in the original and 0 deg in Bevy; camera
   distance and height now match (~4 m, seat + 0.7 m). The Bevy camera still does not reproduce
   the original's slight downward aim.

## Checks

```bash
CARGO_TARGET_DIR=D:/aaemu-target/bevy-pr cargo test -p aa-client --bin aa-client
CARGO_TARGET_DIR=D:/aaemu-target/bevy-pr cargo clippy -p aa-client --bin aa-client
```

Last run: 1307 tests passed, clippy finished with warnings only (no errors).

## Files

| file | role |
|---|---|
| `tools/feel-recorder/run.py` | stack, driver/original, Bevy script, server trace, alignment, CLI |
| `tools/feel-recorder/metrics.py` | stride period, bob amplitude/frequency, rider-vs-mount yaw |
| `tools/feel-recorder/plots.py` | one overlay PNG per channel, centred bob, overview |
| `tools/feel-recorder/strip.py` | 10-row original-vs-Bevy frame strip |
| `crates/aa-client/src/ingame/feel.rs` | Bevy per-frame recorder (`--feel-csv`, `--feel-frames`, `feel MARK`) |
