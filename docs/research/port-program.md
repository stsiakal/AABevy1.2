# Port programme: system by system from the 1.2 decompile

> **Restored 2026-10-27 from `archive/fork-main-2026-10-27`, UNVERIFIED.** The client was restarted from NickMesser's `main`; nothing described here is wired into the game. Addresses, call chains and offsets must be re-checked against the decompile ([R]/[I]/[?]) before use. A 2026-10-27 audit found 151 of the 269 EXACT/TOLERANCE labels in `proof-levels.*` weak or wrong (report: `D:\aaemu-client\handoff\2026-10-27\audit\report-proofs.md`).

Decided by the owner on 2026-10-23. Client logic that decides how the game feels and behaves is ported **line by line** from the
decompiled 1.2 binaries into the Bevy client. Each ported function is proven equal to the original by calling the original
function inside the running hidden 1.2 client. Making the code legally cleaner (rewriting it from the proven behaviour) is a
later, separate step.

## Rules

- **Parity is judged by Nick's ledger (owner, 2026-10-27).** The goal is that the game behaves like the original; the
  measure is [`../aa-client-parity.md`](../aa-client-parity.md): a row is done only when RE spec, Implemented and
  Oracle-verified are all done and the owner has played it. Every step names its row, starts from a scenario captured
  from the original (`reference/`, extended per `reference/README.md`) with a check that fails on the current code, and
  updates the area's `re-<area>.md` (structure in [`README.md`](README.md)) and the row plus a dated checkpoint in the
  ledger. This file and [`proof-levels.md`](proof-levels.md) remain the function-level record; function goldens prove
  ported functions but never replace the scenario check. Breadth: all ledger rows are in scope, not only the slices below.
- **Sources:** the shipped 1.2 binaries and their decompiles (`x2game`, the CryAction/CryAnimation/CryGame/CryInput engine DLLs,
  `cryphysics`), the `.data` memory dump for constants, and the 10.x builds for names only. The published CRYENGINE source is
  never used, quoted or consulted (same rule as `aaemu-physics`, see its `CRYPHYSICS-MAP.md`).
- **Traces are a final check, not a source.** Numbers measured from the hidden original (window-message input, packet yaw) are
  never shipped without decompile evidence; the 12 deg/s mount turn of `fix/mount-nick-base` was such an artifact.
- **Owner is the retail oracle** for anything the code does not settle.
- **Zone-side server behaviour** (NPC AI, pathing, aggro, spawners, physics/navigation) has no 1.2 source at all; the 10.x zone binary and its decrypted server database (`D:\aa-re\db-references\10.0-server\`) are the retail evidence there (NickMesser takes zone data from 10.x for this reason).
- **Every function cites its address** (`FUN_xxxxxxxx`, module, image base) in a doc comment, so the later clean-room pass can
  find every ported function.
- **Proof levels:** a slice is complete only when every function on its chain is **EXACT** or **TOLERANCE** (with a stated
  reason, e.g. x87 80-bit intermediates) **and** wired into the game. The level of every ported address is in
  [`proof-levels.md`](proof-levels.md); a low level anywhere on the chain keeps the slice open.

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

## Roadmap v2 (2026-10-25): experience slices, engine included

Why the change: porting x2game system by system produced exact functions, but riding still did not feel like the original
(owner test, 2026-10-25). The links that decide the feel live in the CryEngine DLLs, which were never mapped: animation
playback, rider attachment, character update and the frame order and timer. Wiring into the game also came last. From now on
work goes by **player experience**, with every link from input to screen in scope (x2game and the engine DLLs), wired into the
game as it is ported, and judged frame by frame against the original plus the owner's feel test.

| Phase | Scope | Done when |
|---|---|---|
| 0. Foundations | Decompile `crysystem` and `cryscriptsystem` (`ghidra/run_engine3.cmd`, out `D:\aa-re\client12\engine3\out\`). Maps: **E1** frame loop, timer and per-frame update order (CrySystem, CryAction game loop, x2game, physics step, animation, camera, render; resolves F5); **E2** CryAnimation playback (clip time, speed scaling, locomotion blending, root motion, layers, attachments, IK); **E3** character and entity glue (CryAction `CAnimatedCharacter`, movement controller, entity attachment/parenting for riders and seats, CryGame player update). **Feel recorder**: both clients run the same scripted scenario; per-frame dumps of unit, mount, rider bone and camera at 60 fps; curves plus a side-by-side video | maps reviewed; recorder works on the current build |
| 1. Movement and riding | F1-F7 plus E1-E3: input, movement, CryPhysics mover (F6, wired), animation playback, rider attachment, camera, frame timing, remote smoothing (F7); then gliders, falling, swimming details (S5) | recorder curves line up; owner says it feels the same |
| 2. Combat | S1 plus engine casting/hit animation links (layers, upper body), cooldown/GCD producers, targeting | same |
| 3. Interaction and housing | S2, S4 (resume `port/interaction-b`, `wire/housing-a`), construction, farms, gathering | same |
| 4. Vehicles and ships | S3 (resume `port/vehicles-d`), CryPhysics vehicle driving, ship visual chain, transfers | same |
| 5. World flow | S7 portals/teleports, S6 flow graphs and cutscenes (CryMovie), S8 formulas | same |

Phase 0 progress: **E2 mapped** ([`port-engine-anim/`](port-engine-anim/MAP.md), 8 commits to `00fe9fbf`): stride time = CAF segment duration / (`fAnimSpeedScale` x command speed x entry scale); `PseudoSpeed` selects the state/LMG, not the clip rate. Phase 1 needs ~1,197 CryAnimation functions / 342 KB (minimal playback + seat set ~110-130 / 100-120 KB); the rest of the module is ~5,966 / 1,041 KB. Rider seat: `SyncRiderAnimation` `3154dd70`, attachments `3150f340`, x2game seat bind `390e96a0`.
**E1 mapped** ([`port-engine-loop/`](port-engine-loop/MAP.md), `0b463b66`..`b2981f55`; resolves F5): one timer tick and **one physics `TimeStep` per rendered frame** with the smoothed/scaled frame time capped at 0.1 s (no fixed step, no substeps, no catch-up); movement is sampled before `IInput::Update`, camera/units/rider feed run after physics and before render, the `CSMoveUnit` pump after render; on-foot sends are change-driven with a minimum interval, mounted sends periodic (intervals still unknown). Our client steps physics at a fixed 30 Hz: a known difference.
**E3 mapped** ([`port-engine-char/`](port-engine-char/MAP.md), `7b9e90c4`..`a4501e81`): the rider is re-derived from the mount every frame (parent world x local in `InvalidateTM`, plus the parent's rotation/position delta in `GetCurrentEntityLocation`), so the rider always turns with the horse; three locations (physics, animation + root motion, render) with a 2.0 m snap and a state-dependent blend. Phase 1 needs ~35 functions / ~44 KB here.
**Engine survey** ([`port-engine-survey/`](port-engine-survey/core.md), `core.md` + `content.md`): every 1.2 engine module classified by runtime use and phase. New line-by-line work beyond E1-E3: CryAnimation beyond E2 ~3,540 fn / 989 KB (facial 237, loading/streaming 213, CAF/DBH assets 203, model/animset 154, pose/physics sync 138, skin/attach 44 KB), CryMovie ~1,230 fn / 232 KB (cutscenes, P5), CryAISystem client subset <=1,200 fn / 365 KB (upper bound), core modules ~55 fn / 30 KB (`CExactPositioning`, breakables, Movie Lua bind), sound occlusion 21 KB, xlcommon name/auction policy 22 KB. CryGame is the unused Crysis sample game (x2game is the game DLL); rendering (renderer, most of Cry3DEngine) is out of scope. "Covered" in the survey means an existing crate implements the area (aa-world, aa-ui, aa-audio, aa-cry, aa-fx), **not** that it is proven equal to the original; those areas still need the side-by-side check of their phase.
**E5 mapped** ([`port-engine-movie/`](port-engine-movie/MAP.md), 16 commits to `e867aff4`): CryMovie for the cutscenes; phase 1 needs none of it; phase 5 needs ~1,115 functions / 211 KB plus the `Movie` Lua bind.
**E6 mapped** ([`port-engine-combat/`](port-engine-combat/MAP.md), 13 commits): the x2game skill-animation scheduler (`PlayAnim` `39157680`, controller tick `39156ac0`), weapon swap at the animation's first scheduler tick, fire time from the CAF `combat_sync` event, additive hit reactions on layer 3, combat stance flips after 10 s. Phase 1 shares 69 functions / 18 KB; phase 2 adds ~454 / 94 KB.
**E8 mapped** ([`port-engine-core-rest/`](port-engine-core-rest/MAP.md)): `CExactPositioning` and breakables are wired in the engine but **dormant in 1.2** (no data or call site uses them); live are the `Movie` Lua bind (2 KB), sound occlusion (7 KB) and the xlcommon name/auction policy (2 KB) plus CRC32/MD5/SHA1-HMAC (9 KB).
**E9 audit** ([`port-engine-survey/audit-world.md`](port-engine-survey/audit-world.md)): against Cry3DEngine, `aa-world` terrain height is the same (1e-4 m); **differences**: the terrain normal (face normal vs the native 4-sample normal at 1.05 m), runtime collision (custom triangle soup vs CryPhysics, 150 m coverage, `bAlignToTerrain` ignored), water queries (waves, floor, streams), entity visibility caps (140/160 m), vis areas not implemented. The survey's `gEnv+0x54` for the 3D engine is wrong for 1.2: x2game uses `*(gEnv+0x2C)`. Phase 1 needs 26 functions / 29 KB here.
**E7 mapped** ([`port-engine-ai/`](port-engine-ai/MAP.md), 8 commits to `bcb4d9e9`): the 1.2 client runs only CryAISystem bookkeeping (init, a gated update, the 0.1 s physics cap `GetMaxStep` `31042b00`, anchors/AI-point graph nodes, nav-modifier shapes, logging): no puppets, perception, pathfinding or smart objects. Live part ~56 functions / 39 KB (82 / 55 KB with the hand-over); the survey's 1,200-function upper bound drops to that.
**E4 mapped** ([`port-engine-anim-char/`](port-engine-anim-char/MAP.md), 12 commits to `18f7b366`): pose/physics sync, skinning and attachments, facial animation, character loading. Phase 1 needs 74 functions / 118 KB (character physics proxy, pose<->physics, pose->attachments, skin matrices); the CHR build core `315ed8a0` (14 KB) **failed to decompile** and needs a re-run. The rest is ~2,630 functions / 695 KB (facial 264, loading 182, model 145, pose 86, skin 18 KB). Hit reactions are game-side, not an engine system.
**Feel recorder done** ([`feel-recorder.md`](feel-recorder.md), `tools/feel-recorder/`, merged `40897af6`, `f123e1cd`): one command runs the same scenario in the hidden original and in Bevy (`--bevy-exe` for any prebuilt branch) and writes aligned per-frame CSVs, overlay plots, a frame strip and metrics. First mounted run (original vs Bevy main): gallop 9.09 vs 9.02 m/s, stride 574 vs 575 ms, rider bob 0.27 vs 0.29 m; turn ~77 vs ~378 deg/s; on Q+W/E+W the original's **horse keeps its heading and the rider turns +/-45 deg** (drawn pelvis vs horse root +46.5 deg mean), Bevy keeps both at 0. **Phase 0 is complete**: every engine map (E1-E9), both surveys and the recorder are on main.
**Phase 1 CryAnimation step A** (`port/cryanim`, `01ef1941`..`0e2fedc2`, not wired yet): time advance `31555240`, CAF speed formula `315557d0`, transition ramp `31554ba0`, normalize `315547a0`, segment average `31554e80` (incl. the original's non-contiguous range quirk), layer ramp `31555070`, LMG expander `31556e90`, command writer `31557af0`/`31557cb0` **live-proven** (20-24 cases each; `MIN_SEGMENT_DURATION` = 1/30 read live); blend manager `31556910` proven by components only (a direct call wedges the original); `SyncRiderAnimation` `3154dd70` partly; blocked for direct calls: `StartAnimationById`, `ProcessAnimationUpdate`, `ProcessAnimations`, `UpdateLocationAttachments`, the x2game seat bind.
**Phase 1 CryAnimation step B** (`fe04405f`..`d77dcc3f`): `ingame/anim_clock.rs` drives the local player's and mounts' locomotion clip through the ported time advance and speed formula (`--legacy-anim-playback` keeps the old path); `ProcessAnimationUpdate` `31550610` (36 snapshot pairs) and `ProcessAnimations` `31550b90` (21) ported and proven from read-only live snapshots; found: the original's effective gallop graph rate is ~0.89 (stride 511 ms at rate 1.0 vs 575 ms), so the ground-speed rate stays until the graph rate is read live; still blocked: `StartAnimationById` (at `3155ae00` instantiate/streaming), `UpdateLocationAttachments`, the x2game seat bind.
**Phase 1 world queries** (`82377c7d`..`417ab030`, wired): terrain surface normal `30124960`->`300C23D0` (20 live, worst 1.9e-5), water level `3012B2E0` + ocean `30127320` (20 live; flat exact, wave band via the renderer's swell since the live wave is a GPU texture), entity view distance `3207DAC0` (20 cases run on the shipped function under emulation with live cvars); the fixed 140/160 m caps are gone (units ~180 m at scale 1, doodads min 64 m); live actor `ViewDistRatio` flags not readable.
**Phase 1 CryAnimation step C** (`4b4e8bbb`..`610c469e`): the gallop rate comes from the LMG speed cap `asset+0x23c` x layer speed (`315557d0`, `31554660`, `315546a0`; 27 mount + 25 rider live frames): 0.9375 mount, 0.7928 rider; stride now 568.8 ms vs the original's 568.0 ms. Seat composition `world = model x local` (`390e96a0`/`3150f340`, 38 live snapshots, <1e-3 m) wired for the rider's translation; `StartAnimationById` FIFO insertion and pending queue (`3155b350`/`3155ae00`, 30 live pairs). **Finding:** our client plays the horse's run clip on the rider; the original uses the rider's own clips, so applying the full seat rotation made the rider's pelvis yaw worse (rms 3.3 -> 103.8 deg) and was reverted: the rider's clip selection is the next fix. Open: full attachment type dispatch, the start descriptor mapping, LMG per-example speed leaves.
**Phase 1 character physics (E4 part)** (`port/engine-anim-char-a`, `4a7d02f0`..`d978924f`, on the owner riding-build chain, not on main yet; not wired): pose-matrix inverse `31544110` (24 live), skin matrices `31525f40` (20), bone map `3161ce60` (24), part defaults `3151e700` (20), proxy ancestor walk `315460a0` (20 + 24) **live-proven**; bbox `31549f80`, sync dispatcher `31543a90`, proxy build `3153b5b0`, create physics `315362a0`, physics->pose `315391e0`, aux sync `315347d0` and the CHR build core `315ed8a0` ported from the decompile with unit tests only: a standing or riding character builds no physics parts in the original (only ragdoll/dead paths do), so they have no live cases yet. **Phase 1 CryAnimation step D** (`83cd4e45`..`b64ea95b`): XL14 per-example LMG weights `3155c710` + `315697b0` **live-proven** (32 run descriptors, straight/Q+W/E+W/A+W, within 1e-5) so the rider plays the original's rider clips; the rider feed `390dcc80` confirmed to copy only PseudoSpeed/Stance/TurnSpeed (282 frames), the rider's yaw is its own; the +-45 deg lean is a measured rule in `ingame/mount_anim.rs` (30 live frames within 1 deg): on the riding-build chain the ported request-direction version from `port/engine-char-a` takes precedence when the two meet. Recorder: stride 575/575 ms, bob 0.280/0.292 m, Q+W 44.5/45.2 deg. Open: the saddle pitch/roll needs the bind-removed seat offset; walk/jump rider states have no live window yet. Physics world step B (`wire/physics-b`, same chain): houses 20/20, fences 18/20, slaves 19/20 approaches stop within 3 m of the original (mean 0.7-1.9 m), water level exact, glide through the living entity (scripted test only); **close, not exact**.

Out of scope as before: rendering (NickMesser's area), the network protocol (`aa-net`, byte-exact already), server behaviour.
Workers: at most two building the client at once; mapping workers are read-only; a fresh worker session per step.

**Phase 2 combat, step 1** (2026-10-26, merges `c89f6597`, `3aa0ac4c`): engine links ported in `port/combat_anim/` (PlayAnim `39157680`, controller tick `39156ac0`, CAF times `39156e50`, hit timing `39157130`, additive miss `39155de0`, SetActiveWeapon `39360920`, stance `39355ee0`, impulse block `3915afb0`/`3915ac50`, skill camera `3915af30`; 20-22 live calls each, not wired yet; event pump `39155660`, projectile manager `393f0a50`, ballistic `393ee930` decompile-only). Wired behind `--legacy-combat`: UseSkill front end and refusal names (`393989e0`, `39897990`), GCD/cooldown (`39899df0`/`39899f30`/`3989a290`), tab/click/target set (`394270a0`), force attack, combat byte/first hit/engage, buff rows; live cast 18132 confirmed by the lane server. Open: aa-net target 0 and the 40-byte position body, item/mount caster forms, U3 `unit_reqs` kinds, target-of-target source.

**Phase 1 F7 remote smoothing** (2026-10-26, merge `a132a895`, opt-in `--ported-remote-smoothing`): SmoothCD follow filter and sample step (40 live goldens), NetActorMovementController state machine, `+0x410` into the remote clip speed. Two-observer live run (`remote-smoothing/live/compare.json`): drawn position vs the original 0.61 m mean / 1.27 m p95, turn error 6.4 deg mean / 19.9 deg p95, **acceleration p95 70.6 vs 7.4 m/s^2** (our follow jitters), so the default stays `remote::Track`. Open: the jitter, the NPC controller chain live (NPCs on the lane would not follow paths).

**Phase 2 combat, step 2** (2026-10-26, merge `8f80d428`, default on, `--legacy-combat-anim` to revert): the ported skill-animation scheduler drives cast (`PlayAnim`), fire/plot/stop, CAF `combat_sync` fire times, victim hit timing and additive reactions (layer 3), additive miss, weapon swap at the first tick, the 10 s stance, big-hit impulse and the skill camera. Bevy live (skill 18132 Triple Slash): CAF fire 0.192/0.166/0.299 s, hit reaction fire 0.070 s (original 0.0701 s), stance flip 10.021 s after the last action (original timeout 10,000 ms). Original-side cast of the same skill blocked on the lane (target out of range); compared with the original's live goldens. Held: `fix/combat-net-a` (target 0, item/mount caster forms, level `unit_reqs`): its 1.2 position body (f32, from the original serializer) disagrees with the server's i64 fixed-point reader; reproduction with the original pending.

**F7 step B** (2026-10-26, merge `16acbebe`, still opt-in): the live sample getter returns mode 3, whose samples are queued (`FUN_390ad070`) and played forward with their packet velocity before `FUN_390a30e0` reads them; the port had fed raw packet positions (the jitter). Also `delta_velocity` uses the masked magnitude, and mode 3 turns with a 0.3 s response. Live: accel p95 70.6 -> 14.2 m/s^2 (original 7.6; steady walk equal), yaw error vs the original 6.4 -> 2.0 deg mean, position 0.61 -> 0.47 m. Open: the actor-model queue consumer after `FUN_390ad070` (start transient).

**Correction (2026-10-26, `31500099`):** the `front_forms.json` `unit_position_*` bodies are crafted-block artifacts (the obj id and floats sit in the halves of i64 fields). A real 1.2 ground cast sends `i64 x, i64 y (x*4096 << 32), f32 z, f32 rot, bc, bc, flag`: the server's reader is right.

**Phase 3 interaction, step B** (2026-10-26, merge `cc94a4a2`, default on, `--legacy-interaction` to revert): `StartNpcInteraction` `FUN_393be280` (224 live rows), `CancelNpcInteraction` `FUN_393be180` (decompile only); wired: the F/G/H/J bar from the ported candidate builders (ascending, de-duplicated), 2.1 m doodad / 3.0-3.5 m NPC surface range with the LOS ray, the NPC 500 ms rate limit, target-change cancel, 3 m gate + auto-walk and routing to the service/quest windows. Live: merchant bar `[12087]` and store window match the original; the doodad comparison was inconclusive (different instances). Open: `FUN_393bfc70` talk animation/camera, kind 3/6 vehicle/house handlers, greeting-only NPCs (no client data source), doodad bar side by side on one instance.

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
| F1 | Movement controller: input, turn, move request, per-frame update, physics move, CSMoveUnit; on foot, mounted, swimming, ladder | [`port-movement/`](port-movement/MAP.md), 101 fn / 91 KB | **player movement complete and bit-exact**: phases A/B, rotation, ClampAngles, CRT asinf/atan2/acosf/sinf, ground and jump, swim stats `FUN_39120300` (24 live: surface, dive, ascend, deep water), frame wrappers, UpdateMovementRequest orchestration `FUN_39118650` (20 live, call order exact) (`0280f9b4`, `69370644`); horse: mounted path, final pass, scope C, glides bit-exact; **open: mount swim control `0x3912ce00` (only called for a swimming ridden mate/boat) and the scope C attached-entity branch** |
| F2 | Third-person camera: follow, keyboard-turn yaw, pitch, zoom, collision, modes | in [`port-movement/`](port-movement/MAP.md) section 4 | mode 1 chain **bit-exact** (`941495fc`): mouse look/pitch, zoom settle (cvars accel 5.0, power 1.8, snap 1.0), offset quaternion, spring arm, collision math (24 live each), mode 1 step (inline yaw <= 2 ulp); fade decision only decompile-modelled (hidden client has no view matrix); modes 2-6 and FOV open |
| F3 | Animation graph runtime: inputs, state selection, transitions, blends, layers | [`port-mount-anim/`](port-mount-anim/MAP.md) D, 51 fn / 76 KB | step 1 **bit-exact**; step 2 `.ag` reader matches all 17 live graphs; step 3 state machine reproduces live sequences; steps 4-5 **bit-exact**: all blend bodies TSTP/S8_1/XL14/XL15/ISTP/M2I8/MI01/MI02/S8SW, per-layer desc, GetAnimationDesc + reset/finalize, command writer, rider copy, speed inputs `FUN_30665830` (40 walking + 48 moving-horse frames) (`9d047aaf`); tie-break matches every live sequence (constructed tie not captured); step 6-7 mount flow and summon/dismiss **proven live** (see F4) |
| F4 | Summon / mount / dismount flow, interaction prompt, rider/mount reactions | [`port-mount-anim/`](port-mount-anim/MAP.md) A-C, 72 fn / 41 KB | **ported and proven live** (`e37e1f4b`): MountHorse/UnMountHorse controllers, mount update (210 ticks), auto-walk, the 3 m send with 2 s expiry, entry side (all four reasons seen), dismount placement, seat attach/detach, summon (Mate record, ClientUnit, SPAWN_PET) and dismiss (CSRemoveMate 0xa4), the mode-bar gate; open: camera matrix slot, auto-dismiss timers |
| F5 | Tick rate and movement send cadence (remaining.md 2.2) | movement U5, vehicles send pump | open |
| F6 | `aaemu-physics` as the client mover and query service (2.3) | - | open |
| F7 | Remote-unit smoothing (interpolation/extrapolation of other units) | movement section 6 + [`port-vehicles/`](port-vehicles/MAP.md) D | mapped |
| S1 | Client combat: cast/queue, GCD and cooldown producers, targeting, tab target, target-of-target, force mode | [`port-combat/`](port-combat/MAP.md), 151 fn / 83 KB | step 1 **bit-exact** (~760 goldens); steps 2-3 with 20 live cases per function; skill-use front end exact (CSStartSkill 13-byte body); step 4 (`661a8ef1`, `8c4ef7a8`): requirement evaluators, 36 kinds / 620 live cases; tab target collect/select/entry, target setter, click select, combat and force bytes, force-attack sender (0x4f), first hit, engage, buff append with 20 live cases each; CSStartSkill forms for unit, doodad and position targets (25 bodies); **damage/heal dispatch, aggro/party and buff apply only synthetic** (crafted records crash the hidden client); key-to-slot dispatch (U8) open; step 5 (`3c299894`): action-bar key -> slot -> UseSkill `FUN_39209d80` (20 live key chords) and the item caster CSStartSkill form (1 real item use) proven; step 6 (`4fa5ca55`): **damage dispatch (53), heal dispatch (26), aggro link (33), party tracker (68), buff apply (35) proven with live events**; limits: the party row is injected (normal parties leave the raid-party map empty), buff UI-event and big-hit/screen-shake branches never fired live |
| S2 | Doodad and NPC interaction rules: range gates, interaction selection, gathering/farming, coffers, phases, loot | [`port-interaction/`](port-interaction/MAP.md), 220 fn / 57 KB | phase A+B (`25172a09`): action table and dispatch (216 live), do_interaction stubs, permission switch (72), range gates (96), candidate builders (30 live: doodads and NPCs) pass; candidate set only 18 cases, doodad func-type switch only 4 live rows, the bar model only decompile-derived (live bar mutation crashes the original); NPC/doodad start, phases, coffers, pick/click, loot not yet ported |
| S3 | Vehicles and ships: wagon/cart driving, helm, boarding, cannons, transfers | [`port-vehicles/`](port-vehicles/MAP.md), 191 fn / 115 KB | phase A **bit-exact** (`9276f79f`, `ec3a7e03`): position codec, velocity codecs 1/60, 1/30, 1/50, delta i8, quaternion i16 and 3xi8 read/write, the move serializer for types 0-5 (write log + read struct), move copy, frame builder, velocity filter, damage state (20-31 live cases each); open: vehicle models, visual chains, movement apply, flow (summon, bind, transfers); phase B (`a375e9fe`): wheeled descriptor, controller activation/ctor/reset, kinematic integrator, non-foot send pump (100 ms max cadence, no change suppression), ship helm sink, type-2/type-4 move fills (throttle +0x108 127/0, steering +0x109 0/-127), 20 live cases each; phase C (`1fbe1363`, `b7d8ffae`): fixed→world codec (scale 2^-44, wire step 2^-9 m), wagon actor apply (U1), net move mark, basis multiply, rudder child basis, receive/queue (0x1000 limit), 5000 ms timeout and 400 m² snap, wheeled step call order; the drive block +0x15c..+0x184 is filled by CryPhysics, not x2game (U4); open: slerp body `0x390eed80`, helm wheel `0x390d2990`, turret `0x390d14b0`, wheel-bone builder `0x390e7b70` (need a live ship skeleton) |
| S4 | Housing and decoration placement, construction display, seeds | [`port-housing/`](port-housing/MAP.md), 197 fn / 84 KB | phase A **bit-exact** (error keys, maths, wheel step, packet builders); phase B **bit-exact** (`9172f7ac`, `265aee27`, `447c8e7d`): collision dispatch with OBB/OBB and AABB/OBB SAT, footprint and mesh walk, the two-click state machine (left click, right click, per frame), quote 0x56 and create 0x57 decisions incl. the 5 s limit, cancel, house-map find and overlap (20-48 live cases each); phase C (`8c03af7f`, `e312622a`): area/category check `0x39333e20` **bit-exact** (185 live cases), group rule `0x39897280` exact through it; U14 resolved (check runs on the zone-local position, `local = cell*4096 + Pos`); U1/U2/U3 constants (5.0°, yaw 0, terrain step 2.0, grid 8192, origin 15360,19456); phase D (`b5f70fdc`, `f84945a3`, `f58e4be0`): decoration cursor ctor, prechecks, validation and confirm (P3) and seed start, checks, height, farm/area and confirm (P4) **live-proven** (20-50 live cases each); the live cases corrected three phase-C transcriptions (deco inside/outside/wall flag bytes, seed height snaps to the box minimum at <=6 m, deco confirm checks the cursor error before the item pair); not observable hidden: seed guard 0x163 (dispatcher 0xce fires first); **wired into the game** (`wire/housing-a`, `85d71edf`, `7ee909d9`, `d7374cc6`, `f462ca3a`, `bf2d4c63`): in-game house, decoration and seed placement run on `port::housing` (5 deg wheel, ported refusal keys), `--legacy-housing` keeps the old path; live check sent the same wire layouts with the ported positions; step E (`40537352`..`8f156c2a`): placement screen ray `394298c0` (24 live), plot half-grid `39897170` (29), builder start gate `39017620` (20) and item dispatcher `394eaac0` (24) **live-proven** and wired, grow-stone plot border and red/green invalid tint drawn; still open: the translucent house ghost model itself (only the border shows), start-gate/routing refusal codes decompile-only, a fence-box live anomaly (48 m vs 6) documented; open: over-world per-frame branches, notch amplitude, dot spacing |
| S5 | Gliders, swimming/diving details, falling and fall damage, click-to-move, jump | [`port-movement-extra/`](port-movement-extra/MAP.md), 75 fn / 65 KB | step A **bit-exact** (`cf29060b`..`76dc09e2`, not wired yet): fall start `3911c780`, fall tracker `39117590`, fall damage `39116f00`, glide block `39357920`, glider start/stop `3911fc80`, glider cleanup event, move-target setter `39095e70`, breath `3930b660`, fall-up/water selectors, sliding timer, fixed/input flight `3912af10`/`3912b090` (20-26 live cases each; one fixed-flight case 1 ulp); step B **bit-exact** (`60a22e73`..`e4850e4d`): swim control `3912ce00` (20 on foot + 20 mounted), dive start/end `3930b600`, breath correction `3930b050`, swim-jump gate `3930b5b0`, click-move queue quaternion `390a58b0`, quaternion to Euler `39097bd0`, move-to-point request `393ba480` (all 0x9C bytes), path controller `390ca3d0`/`390b2af0`; open: the click handler `390a6af0` (needs a camera-ray recorder), the water-landing effect `39117650`; not wired yet |
| S6 | Flow graphs and cutscenes: quest camera, cinematic actors, animated entities, skill controller kind 8 | [`port-flowgraph/`](port-flowgraph/MAP.md), 124 fn / 61 KB | mapped |
| S7 | Portals, teleports, return points, navigation | [`port-portals/`](port-portals/MAP.md), 118 fn / 50 KB | mapped |
| S8 | Client formulas and stats: formula evaluator, stat calculation, tooltip numbers, client mirrors of world rules (shareable with the server) | [`port-formulas/`](port-formulas/MAP.md), 188 fn / 76 KB | mapped (`62661df9`); step A **bit-exact** (`ce445d15`..`5e950045`): the expression engine (name lookups, 4-state lexer, recursive-descent parser, builtin dispatch, node evaluators types 0-22), 811 live cases (tree shapes and f32 bits); type 23 (table path) ported but never emitted by the parser, 0 live cases; next: B stat calculation and UnitInfo |

### Proof levels per slice (2026-10-26)

Audit of `crates/aa-client/src/port/**` at `6ca78df4`; full per-address tables and reasons in
[`proof-levels.md`](proof-levels.md) (regenerated by `python tools/port_audit/proof_levels.py`). `done` = EXACT or
TOLERANCE and wired; `ported` is the number of ported addresses, excluding reference-only comments.

| slice | ported | EXACT | TOLERANCE | DECOMPILE | APPROX | done | what keeps it open |
|---|---:|---:|---:|---:|---:|---:|---|
| F1/S5 movement | 67 | 41 | 11 | 15 | 0 | 2 | nearly everything is unwired; 15 decompile-only |
| F2 camera | 10 | 5 | 1 | 4 | 0 | 0 | no camera function is wired; fade/zoom/look decompile-only |
| F3 anim graph | 55 | 23 | 2 | 30 | 0 | 0 | test-only; 30 decompile-only |
| F4 mount flow | 26 | 5 | 16 | 5 | 0 | 0 | test-only; 5 decompile-only |
| E2 CryAnimation | 20 | 12 | 2 | 6 | 0 | 2 | only clip time/speed wired; the playback chain is PNW |
| E6 combat anim | 43 | 11 | 0 | 32 | 0 | 11 | 32 decompile-only; scheduler wired behind `--legacy-combat-anim` |
| S1 combat | 64 | 42 | 1 | 20 | 1 | 12 | 20 decompile, latest damage/heal/aggro rows only type-only |
| S2 interaction | 78 | 15 | 2 | 61 | 0 | 5 | 61 decompile-only; live captures without tests |
| S3 vehicles | 35 | 23 | 0 | 12 | 0 | 0 | only the codec is exact; no driving chain wired |
| S4 housing | 66 | 38 | 1 | 27 | 0 | 27 | 27 decompile (refusal keys, packet ctors) |
| F7 remote | 5 | 2 | 0 | 1 | 2 | 2 | mode-3 follow is a stand-in |
| S8 formulas | 15 | 15 | 0 | 0 | 0 | 0 | all exact but unwired |
| E9 world | 1 | 0 | 1 | 0 | 0 | 1 | done |
| F6 physics | 0 | 0 | 0 | 0 | 0 | 0 | `crates/aa-physics` absent in this worktree |
| E1 loop, E3/E4 character, S6 flow, S7 portals | 0 | 0 | 0 | 0 | 0 | 0 | no `port/**` address rows yet |

Totals: 485 ported addresses, 62 done, 352 reference-only addresses of functions this tree does not implement.
The riding cross-check (the `fix/ride-6` inventory) is in [`proof-levels.md`](proof-levels.md#riding-slice-cross-check-fix-ride-6).

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
| Action-name lookup | byte-exact (case-sensitive), not case-insensitive | interaction phase A, 112 live cases |
| Interaction bar | F/G/H/J = actions 31-34, up to 10 shown, 2.1 m doodad / 3-3.5 m NPC, LOS | [`port-interaction/`](port-interaction/MAP.md) |
| NPC talk | 500 ms rate limit, 3 m gate + auto-walk, 17 service tags | [`port-interaction/`](port-interaction/MAP.md) |
| Local cast start | only on `SCSkillStarted`, never predicted | [`port-combat/`](port-combat/MAP.md) |
| Tab targeting chain | `FUN_39368630` -> `FUN_39368520` -> `FUN_393672c0`; hostile flag bit 0x100; a separate 10-entry previous-target list | combat step 4, 20 live cases each |
| Force attack | sender `FUN_39390ec0`, opcode 0x4f with an inverted byte, gate `FUN_39390f20` | combat step 4 |
| Formula evaluation | always dynamic from the DB text; the static mirrors only feed a startup verifier; unit-formula variable kind 1 is the character race | formulas map |
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
| Swim control `FUN_3912ce00` | only called from `Quadruped::PrePhysicsUpdate`: it is the ridden mount's swimming, not the player's; player swimming is `UpdateSwimStats` | movement port, live call graph |
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
| M2I8/MI01/MI02/S8SW share the S8_1 blend body (merge `0eab0343`) | they do not: separate wrappers `FUN_3156c190/c1d0/c210/d7a0`, now ported bit-exact (24 cases each) | animation `9d047aaf` |
| Big-hit threshold 1.0 | 0.0f | combat step 1 goldens |
| `0x115` is a portal packet | not portal related | [`port-portals/`](port-portals/MAP.md) U-E1 |

### Suspects (need a capture before any server change)

| Suspect | What differs | Next check |
|---|---|---|
| `CSUsePortal` fields | client writes `tid` u32, `type` u32, `role` u8; server parses `objId` + `onlyMyPortal` | capture from the original in the portals port (U-A2) |
| `CSDeletePortal` fields | client writes only `tid` u32 | capture a private and a district delete (U-A3) |
| Quest 251 reward count | Classic 3.5 gave 2x item 18791, 1.2 data says 1x | Classic is 3.5; likely a version change |
