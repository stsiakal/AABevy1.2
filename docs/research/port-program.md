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

| # | System | Status |
|---|---|---|
| F1 | Movement controller: input, turn, move request, per-frame update, physics move, CSMoveUnit; on foot, mounted, swimming, ladder | mapping (`port/movement-map`) |
| F2 | Third-person camera: follow, keyboard-turn yaw, pitch, zoom, collision, modes | mapping (with F1) |
| F3 | Animation graph runtime: inputs, state selection, transitions, blends, layers | mapping (`port/mount-anim-map`) |
| F4 | Summon / mount / dismount flow, interaction prompt, rider/mount reactions | mapping (`port/mount-anim-map`) |
| F5 | Tick rate and movement send cadence (remaining.md 2.2) | after F1 |
| F6 | `aaemu-physics` as the client mover and query service (2.3) | after F1 |
| F7 | Remote-unit smoothing (interpolation/extrapolation of other units) | after F1 |
| S1 | Client combat: cast/queue, GCD and cooldown producers, targeting, tab target, target-of-target, force mode | queued |
| S2 | Doodad and NPC interaction rules: range gates, interaction selection, gathering/farming, coffers, phases | queued |
| S3 | Vehicles and ships: wagon/cart driving, helm, boarding, cannons, hanging/supports | queued (needs F6) |
| S4 | Housing and decoration placement: snapping, wheel step, validity checks | queued |
| S5 | Gliders, swimming/diving details, falling and fall damage, click-to-move, follow | queued |
| S6 | Flow graphs and cutscenes: quest camera, cinematic actors, animated entities, skill controller kind 8 (2.4) | queued |
| S7 | Portals, teleports, return points, navigation (`H5`) | queued |

At most three map workers run at once, plus implementation workers in their own worktrees. Shared struct layouts (unit/actor
object, mate, controller, movement request) live in one module so later systems reuse them.
