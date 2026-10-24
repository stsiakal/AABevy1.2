# Port programme: system by system from the 1.2 decompile

Decided by the owner on 2026-10-23. Client logic that decides how the game feels and behaves is ported **line by line** from the
decompiled 1.2 binaries into the Bevy client. Each ported function is proven equal to the original by calling the original
function inside the running hidden 1.2 client. Making the code legally cleaner (rewriting it from the proven behaviour) is a
later, separate step.

## Rules

- **Sources:** the shipped 1.2 binaries and their decompiles (`x2game`, the CryAction/CryAnimation/CryGame/CryInput engine DLLs,
  `cryphysics`), the `.data` memory dump for constants, and the 10.x builds for names only. The published CRYENGINE source is
  never used, quoted or consulted (same rule as `aaemu-physics`, see its `CRYPHYSICS-MAP.md`).
- **Traces are a final check, not a source.** Numbers measured from the hidden original (window-message input, packet yaw) are
  never shipped without decompile evidence; the 12 deg/s mount turn of `fix/mount-nick-base` was such an artifact.
- **Owner is the retail oracle** for anything the code does not settle.
- **Every function cites its address** (`FUN_xxxxxxxx`, module, image base) in a doc comment, so the later clean-room pass can
  find every ported function.

## Pipeline per system

1. **Map** (research, read-only): the call chain from input or packet to the visible result, every function with address, size,
   callers/callees, the struct offsets it reads/writes, globals/cvars/data tables, pure or stateful, and how to call it
   in-process. Output `docs/research/port-<system>/MAP.md`.
2. **Review** by the orchestrator: addresses spot-checked against the decompile, scope and order agreed.
3. **Harness tests:** for each function, a differential test that calls the original through the driver's `call` command
   (`aaemu-clientdriver`, branch `feat/driver-call`) with chosen inputs and compares with the Rust port. Pure leaves first.
4. **Port** line by line into the Bevy client, leaves first, replacing the guessed code. Unit tests from the harness goldens so
   CI runs without the original client.
5. **Side-by-side check** of the whole system in the oracle suite, then the **owner's test**.
6. Merge to client main; mark the parity row's RE spec / Implemented / Oracle-verified fields.

## Scope: what is ported and what is not

| Kind | Examples | Approach |
|---|---|---|
| Client game logic | movement, turning, camera, mounts, summon/mount/dismount flow, animation graph, interaction rules, client combat timing, targeting, remote-unit smoothing, vehicle/ship driving | **this programme** |
| Physics | living entity, vehicles, ships | already ported bit-exact (`aaemu-physics`); wire it into the client (remaining.md 2.3) |
| UI | windows, HUD, chat, bags | the original Lua UI scripts already run; port only the native `X2*` functions behind them when a behaviour differs |
| Network protocol | packets | `aa-net`, byte-exact from the protocol; unchanged |
| Rendering | shaders, lighting, sky, wind, sun shafts | not ported (Bevy renderer; world rendering is NickMesser's area) |
| Server behaviour | quest logic, loot, AI | the server emulator, not the client |

## Order

Foundations first, because most systems sit on them (`remaining.md` section 2):

| # | System | Map | Status |
|---|---|---|---|
| F1 | Movement controller: input, turn, move request, per-frame update, physics move, CSMoveUnit; on foot, mounted, swimming, ladder | [`port-movement/`](port-movement/MAP.md), 101 fn / 91 KB | phases A/B **bit-exact**; phase C: ProcessNormal, ClampAngles (+3x3 inverse), CRT asinf/atan2, quadruped dispatch/apply, glides, **mounted horse (50 cases), final pass (24), ground movement and jump `FUN_3912b330` (38), frame movement and rotation wrappers (20 each) all bit-exact** (`e0cca6fe`, `bbada035`); **open: swim control `0x3912ce00` (needs the engine ray/water-current interface), swim stats `0x39120300`, scope C `0x3912f600` (needs CRT acosf/sinf), UpdateMovementRequest `0x39118650` (orchestration)** |
| F2 | Third-person camera: follow, keyboard-turn yaw, pitch, zoom, collision, modes | in [`port-movement/`](port-movement/MAP.md) section 4 | mapped |
| F3 | Animation graph runtime: inputs, state selection, transitions, blends, layers | [`port-mount-anim/`](port-mount-anim/MAP.md) D, 51 fn / 76 KB | step 1 **bit-exact** (~400 goldens); step 2 loader: the compiled `.ag` reader (`CAnimationGraph::Serialize` `FUN_306152b0`, version 0x31) loads all 17 shipped graphs and matches the live tables (player_full_body 2999 states / 7999 transitions with ranges, horse 223/455, creature graphs), playerupperbody XML expands to the live 13,753 transitions (`21baa2b9`); step 3 state machine with the full `CStateIndex` **reproduces live sequences** recorded from the original: player idle-walk-run-stop, turns, jump, swim, mount/dismount, horse gaits; open: equal-priority tie-break (port keeps the larger id, matches all captures), playerupperbody template guards, factory-node payloads, ephemeral multi-hop order (synthetic only); step 4-5 (`53f4bd20`, `5142a03e`): TSTP blend caps (24), the shared S8_1/M2I8/MI01/MI02/S8SW blend body (48), per-layer desc data (624), rider copy from the mount `FUN_390dcc80`/`FUN_390dd080` (35) **bit-exact**; open: XL14/XL15/ISTP blend weights (atan2 operand pairs), GetAnimationDesc and the command writer, speed inputs `FUN_30665830` |
| F4 | Summon / mount / dismount flow, interaction prompt, rider/mount reactions | [`port-mount-anim/`](port-mount-anim/MAP.md) A-C, 72 fn / 41 KB | mapped |
| F5 | Tick rate and movement send cadence (remaining.md 2.2) | movement U5, vehicles send pump | open |
| F6 | `aaemu-physics` as the client mover and query service (2.3) | - | open |
| F7 | Remote-unit smoothing (interpolation/extrapolation of other units) | movement section 6 + [`port-vehicles/`](port-vehicles/MAP.md) D | mapped |
| S1 | Client combat: cast/queue, GCD and cooldown producers, targeting, tab target, target-of-target, force mode | [`port-combat/`](port-combat/MAP.md), 151 fn / 83 KB | step 1 **bit-exact** (~760 goldens); steps 2-3 with 20 live cases per function; skill-use front end exact (CSStartSkill 13-byte body); step 4 (`661a8ef1`, `8c4ef7a8`): requirement evaluators, 36 kinds / 620 live cases; tab target collect/select/entry, target setter, click select, combat and force bytes, force-attack sender (0x4f), first hit, engage, buff append with 20 live cases each; CSStartSkill forms for unit, doodad and position targets (25 bodies); **damage/heal dispatch, aggro/party and buff apply only synthetic** (crafted records crash the hidden client); key-to-slot dispatch (U8) open |
| S2 | Doodad and NPC interaction rules: range gates, interaction selection, gathering/farming, coffers, phases, loot | [`port-interaction/`](port-interaction/MAP.md), 220 fn / 57 KB | mapped |
| S3 | Vehicles and ships: wagon/cart driving, helm, boarding, cannons, transfers | [`port-vehicles/`](port-vehicles/MAP.md), 191 fn / 115 KB | mapped |
| S4 | Housing and decoration placement, construction display, seeds | [`port-housing/`](port-housing/MAP.md), 197 fn / 84 KB | mapped |
| S5 | Gliders, swimming/diving details, falling and fall damage, click-to-move, jump | [`port-movement-extra/`](port-movement-extra/MAP.md), 75 fn / 65 KB | mapped |
| S6 | Flow graphs and cutscenes: quest camera, cinematic actors, animated entities, skill controller kind 8 | [`port-flowgraph/`](port-flowgraph/MAP.md), 124 fn / 61 KB | mapped |
| S7 | Portals, teleports, return points, navigation | [`port-portals/`](port-portals/MAP.md), 118 fn / 50 KB | mapped |

At most three map workers run at once, plus implementation workers in their own worktrees. Shared struct layouts (unit/actor
object, mate, controller, movement request) live in one module so later systems reuse them.

## Findings catalogue

Every confirmed finding, correction and suspect from the maps and ports, newest first. Details and addresses are in the linked
map; constants also go into [`re-constants.md`](re-constants.md). A suspect stays a suspect until a capture from the original
client confirms it.

### Confirmed values (decompile + `.data` dump, or bit-exact goldens)

| Finding | Value | Evidence |
|---|---|---|
| Keyboard turn on foot | axis x min(dt, 0.033) x `keyboard_rotate_speed` (0.5) x pi = 90 deg/s | `FUN_39402e00`, `.data 0x3999c47c`; 30 goldens bit-exact |
| Turn ramp while **gliding** (state `+0x1c1 == 4`) | accumulator 0..3 s x `unit+0x174` x 1/3, reset on release | `FUN_39402e00`, `0x399d5ed0`=3.0, `0x399a1c40`=1/3; live read: 4 only while gliding |
| Horse keyboard turn | the on-foot branch, 90 deg/s (horse reads `+0x1c1` 0, `+0x174` 1.0) | live driver read, movement phase B |
| Mouse turn fold | `clamp(axis x -0.002, -c, c)` | movement U12, goldens bit-exact |
| Global cooldown | 1,000,000 / (1000 + attribute 0x4a), floor -999, unsigned/wrapping; custom GCD x1000/(1000+attr); the stack argument is the unit | `FUN_39899f30`, `FUN_39899df0`; 160 goldens bit-exact |
| Cooldown manager | inline at `unit+0x2c48`, 150 entries, size 0xE28; `SCCooldowns` rows `(id,total,remaining)` | combat step 1, 115 goldens |
| Animation graph input quantisation | floats quantised to 1/1024; guard packing and op semantics, `forceGuard`/`mixinFilter`, inheritance masking | animation step 1, cryaction `EvaluateGuards`/`EncodeInput` goldens |
| Animation graph source | the client loads the compiled `.ag`, which is one data revision older than the `.xml` next to it; parity needs the `.ag` | animation step 2, live dumps |
| Get-on seat | back -> seat 2, front/sides -> seat 1 | `FUN_3909ae90` goldens + decompile |
| Big-hit threshold | `0.0f` | `FUN_39391950`, 111 goldens |
| Interaction ranges | 3 m units/houses (3.5 m mounted), 4 m bind, surface gap not origin distance | `FUN_39093860` -> `FUN_3989bf90` -> `FUN_398b1ab0`; client `75d7ca7` |
| Placement wheel step | 1 deg x `builder_rotate_angle` (5.0) per notch | [`port-housing/`](port-housing/MAP.md); name from 10.x, value in 1.2 |
| Plot dots | rotate with the model, 6 m spread | [`port-housing/`](port-housing/MAP.md) |
| Interaction bar | F/G/H/J = actions 31-34, up to 10 shown, 2.1 m doodad / 3-3.5 m NPC, LOS | [`port-interaction/`](port-interaction/MAP.md) |
| NPC talk | 500 ms rate limit, 3 m gate + auto-walk, 17 service tags | [`port-interaction/`](port-interaction/MAP.md) |
| Local cast start | only on `SCSkillStarted`, never predicted | [`port-combat/`](port-combat/MAP.md) |
| Tab targeting chain | `FUN_39368630` -> `FUN_39368520` -> `FUN_393672c0`; hostile flag bit 0x100; a separate 10-entry previous-target list | combat step 4, 20 live cases each |
| Force attack | sender `FUN_39390ec0`, opcode 0x4f with an inverted byte, gate `FUN_39390f20` | combat step 4 |
| Client skill prechecks | the 1.2 client checks **no** range, LOS, facing, resources, mount or swim before sending `CSStartSkill`; those are server-side | combat step 3, `FUN_393989e0` path, 18 live cases |
| Cast cancel by movement | server-driven (`SCCastingStopped`); no input-layer caller of `FUN_39395b50`; no queue margin, 1000 ms client-only grace | combat step 3 (U7, U9) |
| Combo entries | keyed by the **next** skill id; expiry inserts the next skill's cooldown (18 s seen live) | combat step 3 |
| Remote smoothing | 5 s timeout, snap beyond 400 m^2 | `FUN_390b2420`; [`port-vehicles/`](port-vehicles/MAP.md) D |
| Ships | no client buoyancy; pose from server type-3 movement | [`port-vehicles/`](port-vehicles/MAP.md) |
| Summon | no spawn animation or fade; idle pose when the model stream completes; whistle fx only | [`port-mount-anim/`](port-mount-anim/MAP.md) A |
| Own-mount prompt | no F entry for one's own mount; mounting is the pet bar `pet_mount.dds`; F/G/H/J is the dynamic interaction bar | [`port-mount-anim/`](port-mount-anim/MAP.md) B |
| Portal gates | auto-entered within range of NPCs 3891/5476: 1 s scan, 10 s reuse, buffs 1128/1910 block | [`port-portals/`](port-portals/MAP.md) |
| Same-zone teleport | never shows the loading art; only `SCLoadInstance` does | [`port-portals/`](port-portals/MAP.md) |
| Flow graph content | 1,529 graphs, 10,295 nodes, 107 node classes, 255 cinema sequences | [`port-flowgraph/`](port-flowgraph/MAP.md) |
| Jump | two-stage up projection with an f64 up vector; jump height read from `[actor+0x3ec]+0x84`; asymmetric extra-impulse blend | `FUN_3912b330`, 38 live cases bit-exact |
| Fall damage | glide and swim exempt | [`port-movement-extra/`](port-movement-extra/MAP.md) |

### Corrections to earlier notes or code

| Earlier claim | Correction | Source |
|---|---|---|
| Mount turn 12 deg/s in place, 6.7 running (`fix/mount-nick-base`) | trace artifact; the real formula is the ramp above, not a constant | movement map section 3 |
| `unit+0x1c1 == 4` means mounted | **confirmed live: 4 is the glide state**; a horse uses the 90 deg/s on-foot turn | [`port-movement-extra/`](port-movement-extra/MAP.md) 0.4 |
| `command_unmount` is on the F bar | it is on the mode action bar (R/T/Y/U), only for a ridden mount that is not one's own mate | [`port-mount-anim/`](port-mount-anim/MAP.md) B |
| Plot dots 3 m apart | 6 m | [`port-housing/`](port-housing/MAP.md) |
| Ladder axis pairing, combiner order | corrected per goldens | movement phase A, `MAP.md` 1.4 / 2.3 |
| Big-hit threshold 1.0 | 0.0f | combat step 1 goldens |
| `0x115` is a portal packet | not portal related | [`port-portals/`](port-portals/MAP.md) U-E1 |

### Suspects (need a capture before any server change)

| Suspect | What differs | Next check |
|---|---|---|
| `CSUsePortal` fields | client writes `tid` u32, `type` u32, `role` u8; server parses `objId` + `onlyMyPortal` | capture from the original in the portals port (U-A2) |
| `CSDeletePortal` fields | client writes only `tid` u32 | capture a private and a district delete (U-A3) |
| Quest 251 reward count | Classic 3.5 gave 2x item 18791, 1.2 data says 1x | Classic is 3.5; likely a version change |
