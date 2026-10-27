# F. Live capture: step 6 mount flow and step 7 summon/dismiss

The step 6 port was decompile+unit-test gated. This document records the live proof against the
**original hidden 1.2 client** (build 208022) driven through its own Lua bindings, and the step 7
summon/dismiss path. No scratch controller drives the flow: the horse is summoned with item 8159
through `X2Bag:UseItemByType`, mounted with `X2Mate:MountMate()`, dismounted with
`X2Mate:UnMountMate()`, and dismissed with `X2Mate:DismissPet()`. Only the read-only geometry
function `FUN_3909ae90` is called directly (with the live controller and live mate) to record
the entry side the original computes.

Addresses are x2game VAs at the preferred base `0x38ff0000`. The lane is `D:/aaemu-wt/anim-golden`
(login `127.0.0.1:26237`, branch `test/anim-golden`); the client is launched hidden with
`aaemu-clientdriver mcp`, a temporary git-ignored `--password-file`, and no OS-level input.

## 1. Resolved pointer chain

The previous blocker was the wrong game-client global. The movement/combat ports use:

| object | expression | live value (one session) |
|---|---|---|
| GameClient | `*(x2game+0x9b2294)` (`DAT_39b02294`) | `0x3b910314` |
| local ClientUnit | `[gameClient+0x10c]` | `0x524ab6dc` |
| local model actor | `[unit+0x3be8]` (`FUN_390b3750`) | `0x435f9d14` (`X2::ActorUnitModel` vtable `0x399a4f68`) |
| local character id | `[[gameClient+0x40]+0x18]` | `2` |
| mate map | `FUN_3933f5e0(ECX=0x3a168304)` | record vtable `0x399cfb18` |
| mate record | result of the map lookup | `+0x14` tl, `+0x18` item db id, `+0x20` unit id |
| mate unit | `FUN_39360020(mate+0x20)` | ClientUnit, kind 5, tl 75, owner 2 |
| mount controller | `[local model+0x58]` | vtable `0x399a30b8` |
| unmount controller | `[local model+0x58]` | vtable `0x399a3108` |

The unit kind at `ClientUnit+0x410` is **5 for the mate and 0 for the local player** (the player's
"kind 1" checks in the controller are the actor-model virtual `vt[4]`, not this net field). The
mate's `ClientUnit+0x3be8` model has `+0x1fc = -1` (the hidden client's actor id), the same for the
local player.

## 2. Controller layout correction (live bytes)

`FUN_390a5f00` is called by `FUN_390ca4d0(local model, mate unit id, point)`. Its `+0x14` is the
**local player model actor**, not the mate; `+0x18` is the **mate object id**:

```
+0x00 vtable 0x399a30b8
+0x04 u32 millisecond timestamp (vt[0x10] clock)   e.g. 21189
+0x08 0
+0x14 local player X2::ActorUnitModel
+0x18 mate object id (e.g. 34779)
+0x1c finished byte, +0x1d arrived byte
+0x20..0x28 approach offset (length 1 sides / 2 back-front)
+0x2c point (1 driver / 2 passenger), +0x30 sent byte
+0x34 f32 expiry on the seconds clock (vt[0x0c])  e.g. 31.189
```

The mount update (`FUN_390aaf30`) compares the seconds clock with `+0x34`. The unmount controller
(`FUN_390a6110`) stores its start as an **integer millisecond** at `+0x20`, reason `+0x24`, flags
`+0x28/+0x29`: live rows show `+0x20 = 36807`, reason 9, `+0x29 = 1`, hand-off at >100 ms and
finish strictly after 500 ms. This two-clock split is now in `port/anim/mount.rs`.

`FUN_390aa730` tests **`player.z - target.z`** against `-1.5..2.5` (the decompile's
`local_50 - local_5c`), not `target.z - player.z`; the port was corrected and the live arrival
ticks match.

## 3. Case table

Captured by `rust/tools/capture_anim_mount.py` and `capture_anim_mount_extra.py`; compacted by
`curate_mount.py`/`curate_mount_extra.py` into
`crates/aa-client/src/port/anim/goldens/mount_live.json`,
`mount_extra_live.json` and `camera_live.json`.

| function | live cases | result |
|---|---:|---|
| `FUN_390a5f00` MountHorse constructor | 20 controllers | `+0x04` ms, `+0x14` local model, `+0x18` mate id, `+0x2c` point 1, `+0x34` expiry; offset matches the live approach vector |
| `FUN_390aaf30` mount update | 210 ticks | movement axes never cancelled in these cases; expiry never hit; no false finish |
| `FUN_390aa730` auto-walk | 210 ticks / 20 arrivals | computed arrival is within one 12 ms sample of the live `+0x1d`; the z band sign fix is confirmed |
| `FUN_3909b070` send | 210 ticks / 12 observed send transitions | sent byte and 2 s expiry match; the 0xa7 packet is armed exactly when distance² < 9 |
| `FUN_3909ae90` entry side | 20 controllers | reasons **1, 2, 3 and 4** all observed (`reason_start`); the four approach-offset lengths 1/1/2/2 line up with the side/back/front bands |
| `FUN_390a6110` UnMountHorse constructor | 19 + 3 controllers | reason 9 and 10; start ms, `+0x28/+0x29` flags and placement match |
| `FUN_390aaff0` unmount update | 257 ticks | hand-off first at >100 ms, finished strictly after 500 ms (reason 9) / 1000 ms (reason 10) |
| `FUN_390a2e90` / `FUN_3909b1a0` placement | 22 dismounts | live `+0x29 = 1` on every side placement; the first (left) candidate's horizontal position is the observed landing side |
| `FUN_390eb520` / `FUN_39153b60` attach | 20 mounts | child parent id = mate id, seat 1; reason 2/3/4 map to VehicleSeat 2/3/4, else 1 |
| `FUN_390b3f10` / `FUN_390e16b0` / `FUN_390b4000` | 20 attach + 19 detach | parent/seat written on attach, cleared to `0xffffffff`/0 on detach |
| out-of-range refusal | 1 genuine refusal + 3 near-range starts | `CanMount() == false`, `MountMate()` starts no controller |

`reason_start` histogram: `{1:7, 2:7, 3:3, 4:3}` (plus 4 out-of-range cases). The live send
reason histogram is `{1:10, 2:7, 3:3}`; no send happened for the two shortest front approaches
before the server attach, which is expected because the controller ends on attach.

## 4. Step 7: summon and dismiss

All with the real packets from the lane server (`SCMateSpawned` 0x125, ordinary `SCUnitState`,
`SCUnitAttached`, `SCUnitDetached`) and the real UI:

* `Mate` record: vtable `0x399cfb18`, `+0x14` tl 75, `+0x18` item db id 16777232,
  `+0x20` unit id, `+0x38` state 4 after `Mate::SetUnit`, `+0xac` filled slots 5,
  `+0xbc` dismiss pending 0.
* `ClientUnit`: kind 5, tl 75, owner 2 (local character id), model id 450
  (`animals/horse/brown_spot`), created at the server transform; `InitModel` is the only model
  creation call in the path.
* `SPAWN_PET` (0x110): before summon `X2Mate:IsPlayerPetExists()` is false and `petFrame` is not
  visible; after the bind it is true, `X2Mate:PetState() == "passive"` and `petFrame:IsVisible()`
  is true.
* **Dismiss while mounted**: `DismissPet` -> `CSUnMountMate` reason 10 (no controller at that
  moment); the server detaches; `On Detaching` starts `UnMountHorseController(10)` with
  `param_5 = 1` (live `+0x29 = 1`, hand-off at >100 ms, finished after 1000 ms); the client then
  sends `CSRemoveMate` 0xa4 and the Mate record is gone (`mate_map = None`). `Mate+0xbc` stays 0.
* **Dismiss while not mounted**: `DismissPet` starts `UnMountHorseController(10)` with
  `param_5 = 0`, sets `Mate+0xbc = 1`, fires `DISMISS_PET` 0x111; after 1000 ms it sends
  `CSRemoveMate` 0xa4 and removes the record. No hand-off (no placement request in this path).
* **Mode bar**: `client_mode_actions` is empty before and while riding the own mate; the own kind-5
  unit owned by the local character suppresses `command_unmount` (`FUN_3933b1b0` true). The
  pet-bar mount point is 1 for the owner.
* Controller types read via the live vtable: mount `vt+8 = 8`, vtable `0x399a30b8`.

## 5. Camera write `FUN_390dd080`

The hidden local actor's `+0x1fc` is `-1`, so the original skips the block. Writing a valid id
(`0x12345678`) into the live model and swapping the camera interface's `vt[0x1c]` for a scratch
recorder produced the write request:

```
buffer: [6, 0x12345678, 0x80000000, ...]   (type, actor id, ...)
distance at +0x2c: 1.0873116
```

The provider objects hold fov `2.0` (`0x40000000`) and secondary `50.0` (`0x42480000`); the ported
`camera_distance` reproduces the captured float for the recovered camera distance. The camera
matrix slot `(*DAT_3a5bb1d8)->vt[0x1bc]` is 0 in the hidden client, so the matrix itself could not
be recorded and the comparison uses the write's distance with the provider values. The write with
a valid id is proven; the exact camera matrix frame remains a live-open item.

## 6. Scripts and reproduction

In the capture lane (`D:/aaemu-wt/anim-golden`):

```
python rust/tools/capture_anim_mount.py --password-file <temp> --attempts 24 --out mount_live_raw.json
python rust/tools/capture_anim_mount_extra.py <temp> mount_extra_raw.json
python rust/tools/capture_anim_camera.py <temp> camera_raw.json
python rust/tools/curate_mount.py mount_live_raw.json <docs>/.../mount_live.json
```

The account is `animgolden` (GM, character 2, item 8159 in the bag). The password file is a
temporary git-ignored copy (`data/extract/anim-golden/pw.txt`) and is deleted after the runs; the
lane stacks and the client are stopped at the end. The client runs hidden (`--hidden`) and all
input goes through the MCP `client_input`/`client_lua`/`client_use_item` tools on the client main
thread.
