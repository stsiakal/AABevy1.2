# Port map: the remaining engine-core pieces (roadmap v2, E8)

> **Restored 2026-10-27 from `archive/fork-main-2026-10-27`, UNVERIFIED.** The client was restarted from NickMesser's `main`; nothing described here is wired into the game. Addresses, call chains and offsets must be re-checked against the decompile ([R]/[I]/[?]) before use. A 2026-10-27 audit found 151 of the 269 EXACT/TOLERANCE labels in `proof-levels.*` weak or wrong (report: `D:\aaemu-client\handoff\2026-10-27\audit\report-proofs.md`).

Function-by-function map of the small engine pieces the survey left as "to port":
`CExactPositioning`, the breakable-object path, the Lua `Movie` bind, CrySoundSystem
occlusion/obstruction, and xlcommon name/auction policy + hash variants.

## 0. Sources, conventions and scope

### 0.1 Licensing

Everything here is derived **only** from the shipped 1.2 binaries and their decompiles. The
published CRYENGINE source was never used, quoted or consulted. Class and method names in
`CClass::Method` form come from the binaries' own RTTI, profiler strings, log/assert strings or
cvar names; names with no string in the 1.2 binary are marked *(inferred)*. 10.x builds were
used for **names only** (`CExactPositioning::vftable`, `CBreakableManager` RTTI, `UI...`), never
as a behavioural source.

Evidence markers: **[R]** read from the binary/decompile, **[A]** read from a shipped asset
(pak), **[I]** inference, **[?]** open.

### 0.2 Sources

| source | path / base |
|---|---|
| 1.2 x2game decompile | `C:/AAEMU_DISK/aa-re/client12/out-x2game/x2game.dll.c`, base `0x38FF0000` |
| 1.2 x2game tables | `.../out-x2game/{funcs.csv,calls.tsv,strings.txt,strings-xrefs.tsv,imports.txt}` |
| 1.2 cryaction | `D:/aa-re/client12/engine/out/cryaction.dll.{c,funcs.csv,strings.txt}`, base `0x304F0000` |
| 1.2 cryentitysystem | `D:/aa-re/client12/engine/out/cryentitysystem.dll.{c,funcs.csv,strings.txt}`, base `0x32000000` |
| 1.2 cryscriptsystem | `D:/aa-re/client12/engine3/out/cryscriptsystem.dll.{c,funcs.csv,strings.txt}`, base `0x35500000` |
| 1.2 crysoundsystem | `D:/aa-re/client12/engine2/out/crysoundsystem.dll.{c,funcs.csv,strings.txt}`, base `0x36000000` |
| 1.2 xlcommon | `D:/aa-re/client12/engine2/out/xlcommon.dll.{c,funcs.csv,strings.txt,exports.txt}`, base `0x33000000` |
| 1.2 images (vtables/tables) | `D:/aa-re/client12/engine/{cryaction.dll,cryentitysystem.dll}`, `.../engine2/{crysoundsystem.dll,xlcommon.dll}` |
| shipped assets | `D:/AA1.2/game_pak` through the TSV index (`wt/pr/data/pak12.tsv`, `tools/pakx.py`) |
| existing maps | `docs/research/port-program.md`, `port-movement/MAP.md`, `port-mount-anim/MAP.md`, `port-engine-survey/{core,content}.md` |

Decompile line numbers are intentionally not quoted as the primary key (the files are huge and
regenerated); addresses and sizes come from `funcs.csv`, which is stable.

### 0.3 Phase relevance (roadmap priorities)

| piece | survey phase | runtime use found in 1.2? | phase-1 (riding/on-foot) impact |
|---|---|---|---|
| `CExactPositioning` | P1/P3 (sit/use) | **no evidence of use** (§1.2) | none; mount/sit paths do not enter it |
| breakables | P2/P3 | wired, but **no placed breakables** in the world data (§2.2) | none |
| `Movie` Lua bind | P5 | **used** by 15 shipped scripts (§3.2) | none (login/cutscene flow) |
| sound occlusion | P5/P2 | **used**; `<Sound Obstruction>` and `s_Obstruction*` are live (§4.2) | none for feel; audio only |
| xlcommon name/auction | P3/P5 | **used** at 6 x2game call sites (§5.2) | none |
| xlcommon hashes | P5 | CRC32 every name/id; MD5 + SHA1-HMAC in x2game (§5.6) | none |

**Bottom line for phase 1:** this whole bucket is **0 functions / 0 KB of P1-critical work**.
The riding/on-foot chain in `port-movement` and `port-mount-anim` does not call any of these
engine pieces. The first two items are instead *dormancy questions*; the live call-site probes
below decide whether they ever become P3 work.

### 0.4 Measured scope

| area | measured 1.2 bodies | size | survey estimate |
|---|---:|---:|---:|
| `CExactPositioning` class (0x30644400-0x3064b496) | 21 | 28.6 KB | 7.2 KB / 3 bodies |
| — excluding the two debug-draw bodies (0x30644710, 0x306457c0) | 19 | 23.5 KB | — |
| breakable key path (5 bodies) | 5 | 21.0 KB | 19.0 KB (2 bodies) |
| `Movie` bind (class proper) | 18 | 2.1 KB | 3.9 KB / 21 fns (incl. shared ScriptBind helpers) |
| sound occlusion (key algorithm bodies) | 12 | 7.3 KB | 20.8 KB / 154 fns |
| xlcommon name/auction policy | 7 | 1.9 KB | 1.8 KB |
| xlcommon hash family (0x33009d70-0x3300e600) | 61 | 20.2 KB | 21.8 KB / 57 fns |

The survey's `CExactPositioning` 7.2 KB / 3 bodies counts only the three named methods
(`CalculateTarget`, `CheckTargetMovement`, `RecalculateTriggering`). The class as compiled in 1.2
also contains `SetTriggerState` (9,719 B), the two update bodies and the constructor/copy helpers,
so the measured class is 28.6 KB / 21 bodies (23.5 KB excluding debug draw). Sizes here are the
`funcs.csv` sizes including exception/security glue.

### 0.5 Layout

| file | content |
|---|---|
| `MAP.md` (this) | call chains, function tables, structs, parameters, harness recipes, port order, sizing |

Asset evidence (sitting actiongraph, breakable entity/Lua, animation-graph XMLs, scripts) is
extracted read-only under `data/extract/engine-rest/` (git-ignored) with
`tools/pakx.py <pak> <index.tsv> <outdir> <regex>`; no game asset is committed here.

---

## 1. `CExactPositioning` (cryaction) — compiled but dormant in 1.2

### 1.1 What it is

`CExactPositioning` is the CryAction component that aligns a character to a target
position/orientation while a transition animation plays (sit on a chair, use a smart object,
enter a vehicle seat). It is owned by `CAnimationGraphState` (`graph+0x7c8`), created lazily by
`CAnimationGraphState::CreateExactPositioning` (`0x30630030`, a vtable slot of the graph), and
driven from the graph update: it proposes a state to the graph (`+0x73e` "EP decides the current
animation state"), watches the selected animation state, raycasts/pathfinds to the target, and
when it decides the entity is close enough either starts the animation or teleports the entity
(`ag_safeExactPositioning`).

The profiler names in the 1.2 binary are `CExactPositioning::SetTriggerState`,
`CExactPositioning::CalculateTarget`, `CExactPositioning::CheckTargetMovement`,
`CExactPositioning::RecalculateTriggering` [R: `cryaction.dll.strings.txt` 0x30746184,
0x307461f8, 0x30746300, 0x30746350]. The class RTTI name is present in 10.x builds
(`.?AVCExactPositioning@@`) and is used here **for the name only**.

### 1.2 Runtime use in 1.2: not used (evidence)

| check | result |
|---|---|
| x2game strings/imports/cvars for exact positioning | none. The only x2game "Exact" strings are `X2::ActorUnitModel::GetExactAABB` and auction `exactMatch` [R] |
| all 961 shipped Lua 5.1 scripts (`game/scriptsbin/**/*.alb`) | `ExactPos` 0, `StartExact` 0; `Movie` appears in 15 (contrast, §3) [A] |
| all 210 shipped animation-graph / template / actiongraph XMLs | `exactpos`/`exact pos` 0 [A] |
| the shipped sitting actiongraph | `game/libs/actiongraphs/sit_down_fg.xml` uses `AI:AIAnimAction` with `animaction_animtype="fist_ac_sit_down"` and a `SmartObjectHelper`; it never asks for exact positioning [A] |
| the only engine references to EP | the class itself, its cvars, and the CAnimationGraphState hooks listed in §1.3. `CreateExactPositioning` has exactly one data reference in the image: the CAnimationGraphState vtable at `0x3074595c` (slot +0x84 of the table at `0x307458d8`) [R] |
| the only other "exact pos" consumer in the client | cryaisystem cvar `ai_UseObjectPosWithExactPos` (AI UseObject goal), but client AI goal pipes are not driven by AA (survey §3) [R] |

**Verdict.** The code is compiled, its vtable slot exists, and the graph would run it if some
caller invoked `CAnimationGraphState` slot `+0x84`. No 1.2 caller was found: x2game has no
string, no import and no Lua API for it, no shipped graph configures it, and the actual sitting
flow is an actiongraph animation. Treat EP as **dormant**. Do not port it for P1/P3 until a live
probe shows `CExactPositioning::SetTriggerState` running (recipe in §1.7). If a later AA system
(e.g. a server-driven "use object" action) starts calling it, the port order in §1.8 applies.

### 1.3 Call chain (when enabled)

```
CAnimationGraphState::Update                         cryaction 0x30636a00
├─ if graph+0x7c8 != 0
│  ├─ EP::Update                                     0x3064ae10  (time, wait, target trajectory)
│  └─ (per state-update iteration)
│     └─ EP::UpdateStateMachine                      0x3064b030
│        ├─ SetTriggerState(4) when state 3 and +0xe4 > 3
│        ├─ SetTriggerState(0) when target point is null
│        ├─ UpdateTriggerPoint                       0x30649460
│        ├─ RecalculateTriggering                    0x30649e90
│        ├─ CalculateTarget(1) when state 6 and +0x3c == 1
│        ├─ MoveToTarget                             0x3064ab30
│        └─ SetTriggerState(6) / CalculateTarget(0)
│
├─ CAnimationGraphState::Update_EphemeralState        0x30634b70
│  └─ EP::PollResult                                 0x30644400
├─ CAnimationGraphState::PerformQuery                 0x306359e0
│  └─ EP::FillState                                  0x30644520
└─ CAnimationGraphState query-result handler          0x3062fc20
   └─ if result flags & 4: copy EP trigger ring to graph and set graph+0x73e = 1

CAnimationGraphState::CreateExactPositioning          0x30630030  (vtable +0x84)
├─ alloc 0x328, EP::ctor                             0x306459b0
└─ EP::StartParams                                   0x30643e80
   (target state id, start query, finish query, SActorTargetParams)
```

The graph-side hooks are at fixed offsets in `CAnimationGraphState`: `+0x7c8` EP pointer,
`+0x73e` "EP overrides trigger history", `+0x708` current update callback, `+0x13c` query-id
counter, `+0x26/+0x28` current/target state ids, `+0x274` `IEntity*`.

### 1.4 Function table

`EP` = the object at `CAnimationGraphState+0x7c8` (size 0x328). All addresses are cryaction
VAs. "State" means it mutates EP/graph; "read" means it only reads.

| address | size | name / role | callers → callees | fields / globals | kind |
|---|---:|---|---|---|---|
| `0x30644400` | 55 | `PollResult` — latch the graph query id when the EP state is Running and the animation state matches | `0x30634b70` → `graph+0xe8` virtual | `EP+0x08,+0x5c,+0x90,+0x0e`; graph `+0x26` | state |
| `0x30644440` | 220 | `FindMatchingAnimState` *(inferred)* — walk the entity's animation states, match `+0x0e`, accept states with `>3` variants and bit 1 set | `0x30644520` → entity anim interface | `EP+0x0e`; returned state flags | read |
| `0x30644520` | 238 | `FillState` — copy EP state/trigger flags into a query result (state id, first/last trigger) | `0x306359e0` → `0x30644440` | `EP+0x08,+0x0e,+0x5c..+0x5e`, graph `+0x26` | read |
| `0x30644610` | 81 | `PackTriggerHistory` — copy the 24-entry trigger ring into a packed buffer | `0x30645bc0` | `EP+0x5c..+0x86` | pure |
| `0x30644670` | 160 | `CopyStateSnapshot` — copy EP state + trigger ring into the graph's saved state | `0x30645c60`, `0x3064ab30`, `0x3064ad80` | whole EP prefix; `EP+0x15..0x87` | state |
| `0x30644710` | 4241 | **debug draw only** — `ExactPos:` text, distances, target, trigger list; called only from the graph debug renderer | `0x30636a00` (debug branch) → `FUN_30503b90` draw | `EP+0x18..0x3c,+0x2b0,+0x2e4..+0x31c,+0x14a` | debug |
| `0x306457c0` | 324 | **debug draw only** — trigger transition list | `0x30636a00` (debug branch) | `EP+0x5c..+0x86` | debug |
| `0x30645910` | 154 | `InitTargetParams` — clear/init the `SActorTargetParams`-like block (+0x00..+0x88) | `0x30645c60` | `EP+0x00..+0x88` | pure |
| `0x306459b0` | 528 | **ctor** — vtable `0x30745e84`, owner at +4, state 0, default params | `0x30630030` | `EP+0x00..+0x328` | state |
| `0x30645bc0` | 159 | `CopyParams` — copy the target-params block into/out of the saved state | `0x3064ab30`, `0x3064ad80` | `EP+0x00..+0x88` | state |
| `0x30645c60` | 9719 | **`SetTriggerState(state)`** — the 0..6 state machine: consider/wait/prepare/run/complete, pathfind request, animation state selection, timers; profiler marker | EP internal (`0x30649460`, `0x30649e90`, `0x3064ab30`, `0x3064ad80`, `0x3064ae10`, `0x3064b030`) | `EP+0x08,+0x0c,+0x0e,+0x18..0x3c,+0x90,+0x94,+0x98,+0x104,+0x2e4..+0x31c`; cvars `ag_debugExactPos`, `ag_safeExactPositioning`, `ag_log` | state |
| `0x30648280` | 1915 | **`CalculateTarget(success)`** — completion: log, optional safe teleport (`SetWorldTM`), apply target position/orientation, set flags | `0x30649e90`, `0x3064b030` | `EP+0x18..0x38,+0x104,+0x2e4..+0x310`; `ag_safeExactPositioning` | state |
| `0x30648a00` | 2642 | `ComputeTriggerPoint` *(inferred)* — turn the entity/animation state into a target trigger point/orientation, with path/collision checks | `0x30649e90` (2 sites), `0x3064b030` → `0x30642830`, `0x30643ff0`, `0x30644090` | target block +0x18..0x3c, +0x290..0x2b0 | state/read |
| `0x30649460` | 1067 | `UpdateTriggerPoint` — diff trigger point and inputs; "Trigger point changed"/"Inputs changed"; start queries; `SetTriggerState(0/1)` | `0x3064b030` → `0x30645c60`, `0x30632670` | `EP+0x8,+0x10a..0x14a,+0x18c..0x20c,+0x2b8..0x320`; graph `+0x13c` | state |
| `0x30649890` | 31 | `OnGraphStateChanged(state)` — cancel EP if the animation state id is at/past the target and state < 5 | `0x3062ec10` (graph) | `EP+0x320,+0x08` | state |
| `0x306498b0` | 1489 | **`CheckTargetMovement`** — validate the target/trigger movement, build the ray/path query, profiler marker | `0x3062ebe0` (graph wrapper) | input pos/dir, target block, `EP+0x2e4..` | state |
| `0x30649e90` | 3223 | **`RecalculateTriggering`** — recompute trigger states from the graph's trigger ring, pathfind when needed, call `CalculateTarget`/`SetTriggerState` | `0x3064ad80`, `0x3064b030`, `0x3064ab30` → `0x30630c20`, `0x30648a00`, `0x30642830`, `0x30645c60`, `0x30648280` | `EP+0x8,+0xc,+0xe,+0x18..0x38,+0x2e4..+0x320`; graph `+0x2e..+0x34` | state |
| `0x3064ab30` | 582 | `MoveToTarget` — if considering and within `+0x24` metres, snapshot, recalc and advance; timers/transition counts | `0x3064b030` → `0x30645bc0`, `0x30645c60`, `0x30649e90`, `0x30644670` | `EP+0x8,+0x18..0x20,+0x24,+0x10,+0xe4,+0x104,+0x304` | state |
| `0x3064ad80` | 134 | `ResetAndRecalculate` — reset params/trigger state and call `RecalculateTriggering` | `0x3064ae10`, `0x3064b030` | `EP+0x8,+0xc,+0x8e,+0x108` | state |
| `0x3064ae10` | 538 | **`Update`** — advance clocks, wait timer, animation-duration estimate, call target-trajectory `0x30642a10`; per-frame | `0x30636a00` | `EP+0x08,+0x3c,+0x94,+0x98,+0x104`; time source `0x30791aa0+0x30` | state |
| `0x3064b030` | 1122 | **`UpdateStateMachine`** — state transitions, completion, call `UpdateTriggerPoint`/`RecalculateTriggering`/`CalculateTarget`/`MoveToTarget`; per-frame | `0x30636a00` | `EP+0x08,+0x3c,+0x108,+0x109,+0xe4,+0x2e4..` | state |

Shared helpers called by EP (outside the EP address range but part of the same feature):

| address | size | role | notes |
|---|---:|---|---|
| `0x30642820` | 10 | clear one dword at `+0x48` | target-params init |
| `0x30642830` | 362 | build `SActorTargetParams` from pos/dir/quat/time/radius | writes 0x00..+0x60 of the target block |
| `0x306429a0` | 104 | clamp/smooth a target distance | used by `0x3064b030` |
| `0x30642a10` | 2429 | target-trajectory step (approach marker, debug draw, collision-free interpolation) | called by EP::Update |
| `0x30643400` | 2657 | debug draw of the target/velocity/trajectory | debug only |
| `0x30643e80` | 185 | `StartParams` — store target state id (`+0x320`), start/finish query pointers, target block, set flag `+0x109` | called from CreateExactPositioning |
| `0x30643f70` | 32 | `IsIdle` — state 0 and disabled timer expired | called from graph Update |
| `0x30643ff0` | 160 | copy a 3-float + 3-float target | EP helpers |
| `0x30644090` | 184 | copy/lerp a target | EP helpers |

**State machine (`SetTriggerState`, 0x30645c60).** The debug labels give the names; the case
bodies give the transitions [R/I]:

| state | label | entered when | work in the case |
|---:|---|---|---|
| 0 | disabled/off | trigger point changed and EP not running; graph state change cancels | reset target params, copy the saved snapshot, clear query ids |
| 1 | Considering | new trigger point/inputs (from `UpdateTriggerPoint`) | clear target-satisfied flags, wait timer and trigger flags |
| 2 | Waiting for | target selected but not yet ready | same clears as 1; waits for animation/target |
| 3 | Preparing for | preparation radius/time reached | sample timing `+0x308/+0x304`, start the trajectory `0x30643390`, set flags |
| 4 | About to start | preparation done (`+0xe4 > 3`) | same, plus the start-query / animation start |
| 5 | Running | transition started | set target params, clear flags, run completion timer `+0x3c` |
| 6 | Completing | animation finished | same as 5, then `CalculateTarget(1)`; back to 0 |

The transition conditions are timing- and distance-based (`+0x24` arrival distance, `+0x28`
angle, `+0x104` wait, `+0xe4` frame count, `+0x2fc/+0x300/+0x304/+0x308` sampled times). Port the
enum and these timers first; the rest of the 9.7 KB is exception/query plumbing.

### 1.5 Struct layout (as read by the functions)

`CExactPositioning`, 0x328 bytes:

| offset | type | meaning (inferred from use) |
|---|---|---|
| `+0x00` | ptr | vtable (`0x30745e84`) |
| `+0x04` | ptr | owner `CAnimationGraphState*` |
| `+0x08` | i32 | state: 0 off, 1 considering, 2 waiting, 3 preparing, 4 about to start, 5 running, 6 completing |
| `+0x0c` | i16 | target animation state id |
| `+0x0e` | i16 | current animation state id |
| `+0x10` | u32 | flags: bit 0 target valid, bit 3 ?, bit 5 safe-teleport applied |
| `+0x18/+0x1c/+0x20` | f32×3 | target position |
| `+0x24` | f32 | arrival distance (`<=` starts transition; default in code path) |
| `+0x28` | f32 | max angle (radians) |
| `+0x2c..+0x38` | f32×4 | target orientation quaternion |
| `+0x3c` | f32 | remaining time / animation duration |
| `+0x40..+0x48` | f32×3 | target velocity |
| `+0x5c` | u8 | number of trigger ids in the ring |
| `+0x5d` | u8 | ring start |
| `+0x5e..+0x85` | u16×24 | trigger animation state ids |
| `+0x86,+0x87` | u8 | trigger flags |
| `+0x8e` | u8 | target trigger satisfied |
| `+0x8f` | u8 | target trigger reached/cached |
| `+0x90` | u32 | start query id |
| `+0x94` | f32 | "disabled for" timer (seconds) |
| `+0x98` | f32 | elapsed time in state |
| `+0xd4` | f32 | movement correction |
| `+0xe4` | i32 | transition/frame counter |
| `+0x104` | f32 | wait timer |
| `+0x108` | u8 | recalc inputs pending |
| `+0x109` | u8 | start query pending |
| `+0x10a..+0x149` | u32×16 | input snapshot |
| `+0x14a..+0x189` | u16×32 | trigger state ids (graph copy) |
| `+0x18c..+0x20b` | u32×32 | query ids |
| `+0x20c..+0x28b` | u32×32 | previous inputs |
| `+0x290..+0x298` | f32×3 | path distance |
| `+0x29c..+0x2a8` | f32×4 | path orientation |
| `+0x2b0` | i64 | path time (ticks; scaled by `_DAT_3071fc3c`) |
| `+0x2b8..+0x2e3` | — | new target (pos +0x2b8, dir +0x2c4, quat +0x2d0, state/flags +0x2e0) |
| `+0x2e4..+0x30f` | — | active target (same layout shifted +0x2c) |
| `+0x310` | ptr | start query object |
| `+0x314` | ptr | finish query object |
| `+0x318` | u32 | start query id |
| `+0x31c` | u32 | finish query id |
| `+0x320` | i32 | target animation state id (start threshold) |

### 1.6 Cvars and numeric parameters

| cvar | storage in cvars struct | default | effect |
|---|---|---:|---|
| `ag_debugExactPos` | `DAT_3078d700+0x18` | 0 | debug draw + verbose logs |
| `ag_ep_correctMovement` | `+0x20` | 1 | position correction (read by graph, not by the EP bodies here) |
| `ag_ep_showPath` | `+0x24` | 0 | show path |
| `ag_safeExactPositioning` | `+0x68` | 1 | teleport entity to the requested pose when EP thinks it is done |
| `ag_log` | `+0x28` | 0 | enables the "Exact positioning: pathfind %s -> %s" log |

Numerics read from the code: arrival distance `EP+0x24`, max angle `EP+0x28`, wait
`EP+0x104`, disable period `EP+0x94`, path-time scale `_DAT_3071fc3c` (1/1000 s ticks),
`DAT_30722fac` (0.01, "within" epsilon), `DAT_30745e60` (disable threshold). The exact default
values of `+0x24/+0x28/+0x104` are set by `StartParams`/`SetTriggerState` from the caller's
`SActorTargetParams`, not constants.

### 1.7 In-process harness (hidden original)

There are three practical probes, in increasing invasiveness:

1. **Dormancy probe (cheap).** In the hidden client, `SetBreakpoint(0x30645c60)` (or patch the
   first byte to `0xC3` for a one-shot log) and play for 10 minutes on foot, mounted, sitting,
   using doodads and a chair. If it never fires while `ag_debugExactPos 1` produces no overlay,
   EP is confirmed dormant.
2. **Call the real starter.** Get the `CAnimationGraphState*` for an entity (the graph state is
   reachable from the entity's animated-character interface; `0x30630030` is vtable slot
   `+0x84`). Call
   `graph->vtable[0x84/4](graph, SActorTargetParams*, targetStateId, startQuery, finishQuery)`.
   `SActorTargetParams` is the 0x30-byte block copied by `0x30643e80` from its `param_2`:
   pos (3 f32), dir (3 f32), quat (4 f32), state/flags (2 bytes), padded. `startQuery` /
   `finishQuery` are the graph query objects returned by the graph query API; passing `0` is
   safe (`0x30643e80` accepts them and EP will create its own query when it has an owner).
   Allocate 0x328 bytes for the EP (`0x306ea940(0x328)` is the game allocator) and pass the
   graph as owner.
3. **Read after a live trigger.** Read `EP+0x08` (state), `EP+0x2e4` (active target pos),
   `EP+0x2fc` (quat x / timer), `EP+0x90` (query id) and `graph+0x73e` (EP overrides triggers).
   A valid run moves `+0x08` 0→1→2→3→4→5→6 and ends with the entity at `+0x18..+0x20`.

There is no driver export for EP; it must be called through the graph vtable or hooked. Do not
bother with the debug-draw bodies unless reproducing `ag_debugExactPos`.

### 1.8 Port order (only if a live probe proves use)

1. State enum + trigger ring (`+0x5c..+0x87`), `PackTriggerHistory`, `CopyStateSnapshot` — pure
   data, unit-testable against dumps (0.4 KB).
2. `StartParams` + `InitTargetParams` + target block layout (pure, 0.9 KB).
3. `CalculateTarget` (teleport/orientation end state) — testable with a fake graph/entity
   (1.9 KB).
4. `CheckTargetMovement` + `ComputeTriggerPoint` (ray/path inputs; needs `aaemu-physics` raycast)
   (4.1 KB).
5. `UpdateTriggerPoint` + `RecalculateTriggering` (5.4 KB).
6. `MoveToTarget` + `Update` + `UpdateStateMachine` (2.2 KB).
7. `SetTriggerState` last (9.7 KB, most of the complexity; port after the state machine is
   understood from live traces).
8. `OnGraphStateChanged`/`PollResult`/`FillState` glue (0.3 KB).
Skip both debug-draw bodies (4.6 KB) until a debug overlay is needed.

### 1.9 Unknowns

* **U1 [?]** Is `CAnimationGraphState` slot `+0x84` ever called by x2game through the entity
  animated-character interface? Static evidence says no; a live breakpoint settles it.
* **U2 [?]** `SActorTargetParams` field names/units; the 0x30-byte block is inferred from
  `0x30643e80`. The caller's exact struct (probably `SActorTargetParams` in CryAction) is not
  named in 1.2 strings.
* **U3 [?]** `startQuery`/`finishQuery` are graph query objects; their creation API and lifetime
  are not mapped here (they are in the animation-graph query subsystem, `port-mount-anim` scope).
* **U4 [?]** The `+0x24` arrival distance default and `+0x28` angle default come from the action
  that starts EP, not from a cvar; no 1.2 action uses them.


---

## 2. Breakable objects (cryaction + cryentitysystem) — wired, but nothing is placed

### 2.1 What it is

CryEngine's breakable-object feature destroys a static/physics object into physical part
entities when a strong enough impact or a script event hits it. Three layers are involved:

* **cryentitysystem** owns the geometry/splitting: `CBreakableManager::BreakIntoPieces`
  (`0x320b1100`, 11,903 B) turns a breakable object + part connection into new part entities and
  physical bodies. The same module parses the `mat_breakable` surface flag (`0x3204f030` call at
  `0x3204b...`, string `mat_breakable` [R]).
* **cryaction** owns the game collision callback: `CActionGame::OnCollisionLogged_Breakable`
  (`0x306e2c10`, 7,487 B), invoked for **every** collision from
  `CActionGame::OnCollisionLogged` (`0x306e5050`, 595 B) after the material-FX and AI callbacks.
  It filters the collision for breakable entities/vegetation/2D chunks, builds break events and
  schedules piece spawns.
* **x2game** owns the game-side notification: four event handlers
  `OnBreakableCreatePartEvent` / `OnBreakableUpdateMeshEvent` for units and doodads
  (`0x393b6360`, `0x393b6430`, `0x393b6510`, `0x393b65e0`), registered through the four
  wrappers `0x3908a530/0x3908a560/0x3908a590/0x3908a5c0` in the event table at
  `x2game .rdata 0x399a64dc..0x399a64e8`. They look up the unit/doodad and forward the part id to
  its game object (`+0x1ac` / `+0x80`).

### 2.2 Runtime use in 1.2: code path live, world data empty

| check | result |
|---|---|
| x2game event handlers | present and registered (addresses above); they are real forwarding code, not stubs [R] |
| x2game breakable strings | only the four event/log strings; no `BreakableObjects` level serialization [R] |
| shipped entity class | `game/entities/breakableobject.ent` + `game/scripts/entities/physics/breakableobject.lua` + `scriptsbin/.../breakableobject.alb` exist, with `nBreakableType`, `PhysicsBreakable` properties [A] |
| shipped art | `breakable_objects.xml` particle library (glass/ice shatter), `*_break.cgf`, `*_breakup` prefabs; `mat_breakable` parsed by the entity system [A][R] |
| **all 1,294 world cells** `client/entities.xml` | **0** occurrences of `BreakableObject` or `Breakable` [A, scanned] |
| **all 1,294 world cells** `client/leveldata.xml` | **0** occurrences of `Breakable` (so no `BreakableObjects` section) [A, scanned] |
| prefabs sampled (`sol_house_break.xml`, `wienhouse_tir60_breakup.xml`) | use `Brush`/`Comment` objects only; no `BreakableObject` entity [A] |

**Verdict.** The engine path is reachable and x2game would forward the events, but the shipped
1.2 world does not place any breakable object through the two data paths the client loads. The
`*_break` CGFs and `breakable_objects.xml` are used by *doodad* break/shatter phases (server
drives the phase; the client plays a particle/model swap), not by `CBreakableManager`. Unless a
live cell (or a server-spawned client entity) creates a breakable at runtime, the 19 KB of
manager/callback code is **dormant**. This is the opposite of the survey's "to port if breakable
doodads are in scope": AA's breakable doodads are not this system.

A live confirmation is cheap: break a wooden crate/ice doodad and watch
`CBreakableManager::BreakIntoPieces` (`0x320b1100`) or the x2game `OnBreakable*` log lines. If
neither fires, skip the feature.

### 2.3 Call chain

```
physics/entity collision
└─ CActionGame::OnCollisionLogged                        cryaction 0x306e5050
   ├─ listener map (material FX, AI, breakable, ...)
   ├─ OnCollisionLogged_MaterialFX                        0x306e1f60? (E3 scope)
   ├─ OnCollisionLogged_NotifyAI                          0x306e?? (E3 scope)
   └─ OnCollisionLogged_Breakable                         0x306e2c10
      ├─ entity system: resolve entity A/B by id
      ├─ material/impact filter (mat_breakable, impulse, normal, mass)
      ├─ break-event queue  "BreakEvents"                 0x306e06d0
      │  ├─ "BrokenEntParts"   0x306e08e0
      │  ├─ "BrokenVegParts"   0x306e0b00
      │  └─ "Broken2dChunkIds" 0x305d5120
      ├─ FUN_306dcff0  (spawn/defer break parts)
      └─ FUN_306e1220  (build 0xAC-byte break event) -> FUN_306e2c10 re-entry per event

CBreakableManager (cryentitysystem)
├─ physics/virtual trigger  FUN_320b3f90                   0x320b3f90
│  └─ CBreakableManager::BreakIntoPieces                   0x320b1100
└─ impact/queue path        FUN_320b4130                   0x320b4130
   └─ CBreakableManager::BreakIntoPieces                   0x320b1100

x2game event sink
└─ OnBreakableCreatePartEvent / OnBreakableUpdateMeshEvent
   ├─ unit  : 0x393b6360 / 0x393b6430 -> unit vtable +0x1ac(part)
   └─ doodad: 0x393b6510 / 0x393b65e0 -> doodad vtable +0x80(part)
```

### 2.4 Function table

| address | size | name / role | callers | callees | fields / globals | kind |
|---|---:|---|---|---|---|---|
| `0x306e5050` | 595 | `CActionGame::OnCollisionLogged` — iterate the listener map, then call the breakable/material/AI callbacks | engine collision event | listener fn ptrs, `0x306e2c10`, `0x306de2f0`, `0x306d5e50` | `param+0x18/+0x1c` entity types, `+0x10/+0x14` entities | state |
| `0x306e2c10` | 7487 | `CActionGame::OnCollisionLogged_Breakable` — collision filter + break events + deferred parts; profiler marker `CActionGame::OnCollisionLogged_Breakable` | `0x306e5050` | `0x306dcff0`, `0x306e1220`, queue helpers, entity system | collision info `+0x24..+0x74`: normal, velocity, mass, threshold; entity ids `+0x64/+0x66/+0x8c...`; global cvar block `DAT_307a0f80` | state |
| `0x320b1100` | 11,903 | `CBreakableManager::BreakIntoPieces` — split a breakable object into part entities/physics bodies; profiler marker `CBreakableManager::BreakIntoPieces` | `0x320b3f90`, `0x320b4130` (vtable/physics) | entity system create entity, physics create/destroy, geometry split, particle/event dispatch | `this` manager; `param_1` entity id; `param_2` breakable geometry/object; `param_3` part id; `param_4` material; `param_5` transform/impact (0x30-byte blocks); `param_7` flags | state |
| `0x320b3f90` | 402 | physics break event handler — resolves entity/part transforms (two 0x30 blocks) and calls `BreakIntoPieces` | virtual/physics event | `0x320b1100` | entity `+0x140` transform, `+0x174` geometry, `+0x168` material; flag word `+0xb4` | state |
| `0x320b4130` | 1132 | impact/queue handler — computes impulse/part data and calls `BreakIntoPieces` | `0x320b...` queue (caller at 131941) | `0x320b1100` | large local impact struct; queued part list | state |
| `0x320b45a0` | 1267 | breakable-manager support (unmapped body) | manager internal | — | — | state |
| `0x320b4aa0` | 5669 | breakable-manager support/tail (unmapped body) | manager internal | — | — | state |
| `0x320af080-0x320b0ff0` | ~4.2 KB / 21 bodies | breakable-object/part helpers (geometry, transforms, material, list) | manager internal | — | — | mixed |
| `0x393b6360` | 178 | x2game unit `OnBreakableCreatePartEvent` — notify unit (+0x1ac) with part id | event table wrapper `0x3908a530` | unit lookup `0x39360020`, `0x390b3750` | unit object `+0xc` name, part id | state |
| `0x393b6430` | 166 | x2game unit `OnBreakableUpdateMeshEvent` | `0x3908a560` | same, flag 0 | same | state |
| `0x393b6510` | 208 | x2game doodad `OnBreakableCreatePartEvent` — doodad (+0x80) | `0x3908a590` | doodad lookup `0x39035110` | doodad obj id | state |
| `0x393b65e0` | 200 | x2game doodad `OnBreakableUpdateMeshEvent` | `0x3908a5c0` | same, flag 0 | doodad obj id | state |

### 2.5 Parameters and thresholds

`OnCollisionLogged_Breakable` works on the engine `SEntityCollision`-like struct:
`+0x3c..+0x44` relative velocity/impulse, `+0x54` impulse threshold, `+0x74` secondary threshold,
`+0x24..+0x2c` contact position, `+0x64/+0x66` entity ids, `+0x8/+0xc` entity pointers. It rejects
the break when the impact normal points away, the entity is not breakable, or the accumulated
impulse is below `+0x54`; it also handles vegetation and "2D chunk" (decal/plane) breaks, and
defers parts whose entity is not yet streamed (`BrokenEntParts`, `BrokenVegParts`,
`Broken2dChunkIds`). The manager itself has no cvar; piece count/mass come from the breakable
geometry.

### 2.6 In-process harness

1. **Dormancy probe.** Break a crate/ice/window doodad in a live cell with a hook on
   `0x320b1100` and on the x2game log function around the `OnBreakable*` strings. Read the
   manager's `this` (first arg), entity id, part id, and the 0x30-byte transform block. If the
   hook never fires across several destructible doodads, the feature is dead in 1.2.
2. **Call `BreakIntoPieces` directly.** `this` is the entity system's breakable manager (reachable
   from the `IEntitySystem` interface; the manager vtable contains `0x320b1100`). It expects a
   live `IEntity` with breakable geometry (`+0x140` transform, `+0x174` geometry, `+0x168`
   material) and a part id; allocating a synthetic entity is not useful. Use a real breakable
   entity from `game/entities/breakableobject.ent` spawned through the entity system, then call
   the function twice (create/update) and count new entities/physical bodies.
3. **Read the x2game event.** Hook `0x393b6510` and dump the doodad id + part id; compare with the
   `OnBreakableCreatePartEvent(). doodadId(%d)` log line.

### 2.7 Port order (only if a live cell proves use)

1. Breakable material flag (`mat_breakable`) in the material parser (already in `aa-cry`? verify).
2. Entity/part resolution + transform copy (`0x320b3f90`, pure given entities).
3. `BreakIntoPieces` geometry split + part entity creation (largest, needs `aa-world` entity
   spawn and `aaemu-physics` bodies).
4. Collision filter/queue in `OnCollisionLogged_Breakable` (pure given the collision struct).
5. x2game event forwarding (unit/doodad `+0x1ac/+0x80`), only if the game object layer consumes
   part ids.
Skip the 2D chunk/vegetation halves until those object classes are in scope.

### 2.8 Unknowns

* **U5 [?]** Does any *server-spawned* client entity use `BreakableObject` at runtime? The static
  world does not. A live breakable doodad settles it.
* **U6 [?]** `CBreakableManager` object layout and the meaning of `BreakIntoPieces`' 8 parameters;
  the transform block is inferred from `0x320b3f90`, not from a named struct.
* **U7 [?]** `FUN_320b45a0` / `0x320b4aa0` (6.8 KB) are unmapped support; needed only if the
  manager is ported.

---

## 3. `Movie` Lua bind (cryscriptsystem) — used by 1.2 scripts

### 3.1 What it is

The 1.2 CryScriptSystem registers a Lua global table `Movie` with seven methods. The bind is a
thin adapter over `IMovieSystem` (obtained from `ISystem+0xe0`; the survey's `gEnv+0x64` movie
slot is the same object):

| Lua name | C++ action |
|---|---|
| `Movie.PlaySequence(sSequenceName)` | `IMovieSystem+0x50(name, 0, 0, defaults)` |
| `Movie.StopSequence(sSequenceName)` | `IMovieSystem+0x58(name)` |
| `Movie.StopAllSequences()` | `IMovieSystem+0x60()` |
| `Movie.AbortAllSequences()` | `IMovieSystem+0x68()` |
| `Movie.PauseSequences()` | `IMovieSystem+0xb8()` |
| `Movie.ResumeSequences()` | `IMovieSystem+0xbc()` |
| `Movie.GetNumOfPlayingCutscenes()` | `IMovieSystem+0x6c()` → number (float/int conversion) |

The constructor also calls an internal `ScriptBind` helper (`0x35515210`, "SetGlobalName") and
registers the table only when the script system is not in a restricted mode
(`system->+8 != 3`).

### 3.2 Runtime use in 1.2: used

* 15 shipped `.alb` scripts contain the string `Movie`; **5 call `PlaySequence`**:
  `entities/triggers/{cinematictrigger,multipletrigger,delaytrigger,proximitytrigger}.alb` and
  `x2ui/hud/gm_console_view.alb`; `x2ui/centermessage/effect_message.alb` calls
  `GetNumOfPlayingCutscenes` [A: scan of all 961 `game/scriptsbin/**/*.alb`].
* The login stage scripts (`x2ui/loginstage/login/login.alb`, `login_view.alb`, locale files)
  reference `Movie` [A].
* x2game has a `StartLoginMovie`/`StopLoginMovie` console command pair and an `OnPlaySequence`
  event (`0x391e4250`; event string `OnPlaySequence` 0x399b9e60) [R].
* The pak ships cinematic trigger entity/script and 20+ `cinemas/*.xml` sequences including
  `login2/cinemas/01_world_select_e1.xml` [A].
* `crymovie.dll` is loaded and `gEnv+0x64` is non-null in the shipped client (survey §2).

So unlike EP and breakables this bind is on a real, exercised path: login cutscene, quest
cinematic triggers, GM console. The movie backend itself (232 KB, `crymovie`) is P5 and is mapped
separately by the survey; this section covers only the Lua contract.

### 3.3 Call chain

```
Lua script (login / cinematic trigger / GM console)
└─ Movie.PlaySequence("name")
   └─ ScriptBind_Movie::PlaySequence                 cryscriptsystem 0x35514be0
      ├─ pSystem->+0x14c (editor/loading gate)       ISystem
      └─ gEnv->pMovieSystem (ISystem+0xe0)
         └─ IMovieSystem::PlaySequence(name,0,0,def,def,def)   +0x50
            └─ crymovie sequence scheduler → camera/entity tracks → visible cutscene
```

`GetNumOfPlayingCutscenes` is used by the center-message UI to suppress messages while a movie
plays; it is the cheapest live probe.

### 3.4 Function table

| address | size | name / role | callers | callees | fields | kind |
|---|---:|---|---|---|---|---|
| `0x35514be0` | 100 | `PlaySequence` — gate + `IMovieSystem::PlaySequence` | Lua dispatch | system `+0x14c`, movie `+0x50` | bind `+0x48` script system, `+0x50` ISystem, `+0x54` movie system | state |
| `0x35514ad0` | 37 | `StopSequence(name)` | Lua dispatch | movie `+0x58` | same | state |
| `0x35514b00` | 33 | `StopAllSequences()` | Lua dispatch | movie `+0x60` | same | state |
| `0x35514b30` | 75 | `AbortAllSequences()` — extra system gate | Lua dispatch | system `+0x14c`, movie `+0x68` | same | state |
| `0x35514b80` | 36 | `PauseSequences()` | Lua dispatch | movie `+0xb8` | same | state |
| `0x35514bb0` | 36 | `ResumeSequences()` | Lua dispatch | movie `+0xbc` | same | state |
| `0x35514c70` | 216 | `GetNumOfPlayingCutscenes()` — numeric return | Lua dispatch | movie `+0x6c` | same | read |
| `0x35514d50` | 834 | **ctor / bind registration** — stores script system + movie system, creates the `Movie` table and registers the 7 methods + helpers | ScriptBind factory | `0x35515210`, ScriptBind `+0x50` | `+0x04` name[0x40], `+0x44` table, `+0x48` script system, `+0x50` ISystem, `+0x54` movie system, `+0x4c` int | state |
| `0x355150a0` | 44 | re-init with a new script system | factory | — | `+0x48/+0x44/+0x4c` | state |
| `0x355150d0` | 26 | release table | dtor | — | `+0x44` | state |
| `0x355150f0` | 118 | read numeric arg helper | Lua dispatch | — | stack args | read |
| `0x35515170` | 125 | read numeric arg helper (type 5) | Lua dispatch | — | stack args | read |
| `0x355151f0` | 32 | base dtor | factory | — | vtable `0x35570b34` | state |
| `0x35515210` | 143 | set the global table name (`strcpy_s` + script `+0x64`) | ctor | script system | `+0x04` name | state |
| `0x355152a0` | 88 | register one function in the table | ctor | ScriptBind `+0x50` | `+0x44` table | state |
| `0x35515300` | 29 | integer-arg helper | ctor | Lua arg API | — | read |
| `0x35515320` | 55 | deleting dtor | factory | — | — | state |
| `0x35514c50` | 32 | small base dtor used by the arg wrapper | — | — | — | state |

Sizes are `funcs.csv`; total 18 bodies / 2,059 B. The survey's 3.9 KB / 21 fns adds three
shared `ScriptBind` helpers (`0x355149c0`, `0x35514a20`, `0x35514aa0`, 243 B) and generic
registration glue. The Movie-specific logic is tiny; the work is the backend.

### 3.5 Parameters

* `sSequenceName` is the sequence name as loaded from `cinemas/*.xml` (e.g.
  `01_world_select_e1`). It is passed as `const char*` directly; no prefix is added.
* `PlaySequence` passes `IEntity* = 0`, `flags = 0`, and three default args
  (`DAT_35570b48`) whose values are zero/identity from the bind's point of view; the exact
  `IMovieSystem::PlaySequence` signature is `(const char*, IEntity*, int, float, float, ...)` in
  the 1.2 interface (slots are read from the calls, names from the 10.x build; behaviour is
  1.2-only here).
* Return values: only `GetNumOfPlayingCutscenes` returns a number (float marshalled back as
  Lua number); the rest return nothing.

### 3.6 In-process harness

1. **Cheapest live check.** In the hidden client execute
   `Movie.GetNumOfPlayingCutscenes()` from the Lua console (or `gm_console_view`); 0 while idle,
   >0 during the login/world-select movie.
2. **Play a shipped sequence.** `Movie.PlaySequence("01_world_select_e1")` (or a quest cinema
   name) and read `GetNumOfPlayingCutscenes` before/after; the visible camera/cutscene is the
   oracle. `Movie.StopSequence(name)` and `StopAllSequences` must return to 0.
3. **Direct C++ call.** Read `gEnv+0x64` (`IMovieSystem*`) and call
   `*(int(__thiscall**)(void*,const char*,void*,int,float,float,float))(*(int*)movie+0x50)`
   with `(movie,name,0,0,0,0,0)`. This bypasses Lua and is the cleanest differential harness;
   allocate nothing, the sequence object is owned by the movie system.
4. **Read the bind.** `ScriptBind_Movie` object layout: `+0x04` table name (`"Movie"`), `+0x44`
   Lua table, `+0x48` `IScriptSystem*`, `+0x50` `ISystem*`, `+0x54` `IMovieSystem*`. A driver
   can read these after script-system init to assert the table exists.

### 3.7 Port order

1. `Movie` Lua table + 7 method bindings (`aa-lua`, no backend): make them safe no-ops that
   record the call (0.3 KB of Rust).
2. `GetNumOfPlayingCutscenes` once a cutscene scheduler exists (return the scheduler's active
   count).
3. `PlaySequence`/`StopSequence`/`StopAll`/`AbortAll` over the scheduler.
4. `Pause`/`Resume` (trivial once sequences have a paused flag).
5. The crymovie backend itself is P5 and separate (~232 KB, survey §2); do **not** port the
   whole track system for the login movie if a simpler pre-rendered/video path suffices.

### 3.8 Unknowns

* **U8 [?]** The five default arguments in `PlaySequence` (`DAT_35570b48`) are 0 in 1.2; their
  formal names (`startTime`, `endTime`, `flags`, ...) come from the interface layout, not from
  1.2 strings.
* **U9 [?]** `ISystem+0x14c` is an editor/loading gate; its exact predicate is not named in 1.2.
  If it returns 0, `PlaySequence` silently does nothing (the script path still succeeds).
* **U10 [?]** Which movie files the sequences reference (Bink/USM vs engine tracks) and whether
  the login path uses `StartLoginMovie` rather than Lua. `aa-client` can choose a video player
  independently of the Lua contract.

---

## 4. CrySoundSystem occlusion / obstruction — used, approximated by `aa-audio`

### 4.1 What it is

Every 3D sound in CryEngine can be occluded by geometry between the listener and the source.
The 1.2 sound system has two mechanisms:

* **Obstruction** (this section): a per-sound set of rays from the listener/source to the sound
  position; the hit surfaces' "pierecability" plus visarea/portal steps produce an obstruction
  value in `[0,1]`. It is re-evaluated every `s_ObstructionUpdate` seconds and applied either as
  an FMOD DSP parameter (mode 1) or as a volume factor (mode 2).
* **Occlusion culling** (rendering/visarea) is a different feature and is out of scope here.

The system is driven automatically: no script has to enable it. x2game only exposes the entity
type name `<Sound Obstruction>` (`0x39008330`, class id 5) and reads the `<Sound Obstruction>`
sound-area node; it plays sounds and moves the listener through the engine sound interface
(`gEnv+0x58`), never through an occlusion-specific call. The cvars are engine-side [R/A].

### 4.2 Runtime use in 1.2: used

| check | result |
|---|---|
| `<Sound Obstruction>` entity/area type | present in x2game `FUN_39008330` (case 5) [R] |
| cvars | `s_Obstruction`, `s_ObstructionMaxPierecability`, `s_ObstructionMaxRadius`, `s_ObstructionVisArea`, `s_ObstructionUpdate`, `s_ObstructionAccuracy`, `s_ObstructionMaxValue`, `s_DrawObstruction` all registered with help text (0x360a44xx..) [R] |
| raycasts | `RayWorldIntersection(Sound)` string and two call sites in the sound object update (0x36032d40) [R] |
| application | `FMOD::Event::set3DOcclusion` thunk `0x3607ccb6` [R] |
| 3D sounds | used everywhere (footsteps, combat, doodads, ambience); `aa-audio` already plays them [survey §5] |
| occlusion strength in 1.2 | **not measured**; the feature is compiled and on the per-sound path, but a quiet zone with `s_Obstruction 0` vs `1` was not captured. Add an audible A/B to the P5 audio pass. |

The survey's statement "`aa-audio` currently approximates" is accurate: port the algorithm for
parity, but this is P5 and does not affect riding feel.

### 4.3 Call chain

```
sound play (x2game/engine) -> CSoundObject created
  fields: position +0x3d..+0x3f, radius +0x90, flags +0x24, ray state +0x184
  per frame / s_ObstructionUpdate:
  ├─ CSoundObject::UpdateObstructionRays                 0x36032d40
  │  ├─ build 1..5 rays (centre + 4 offsets) in +0x184 (0x30 each)
  │  ├─ FUN_36038b70 RayWorldIntersection(Sound)         0x36038b70
  │  │  flags 0x307 (static|terrain|...), 0x800, skip list, 4 hits/ray
  │  └─ (mode 1) one long ray, (mode 2) 5-ray set
  ├─ CSoundObject::AccumulateObstruction                 0x36032840
  │  pierecability / accuracy / distance weighting into +0x214/+0x218/+0x21c/+0x220
  ├─ CSoundObject::UpdateObstruction                     0x360337c0
  │  picks max/accumulated value, distance attenuation, visarea steps, mode 1 vs 2
  ├─ CSoundObject::EffectiveVolume                       0x360310c0
  └─ FMOD::Event::set3DOcclusion                         0x3607ccb6
     or volume multiplier

listener moves / changes visarea:
  CSoundSystem::SetListener                              0x360456d0
  └─ if listener area changed:
     CSoundSystem::RecomputeSoundOcclusion               0x36045440
     ├─ per sound: read cached obstruction               0x360449f0
     ├─ reapply 0x36045060 / 0x36045220
     └─ reset visarea obstruction state
```

### 4.4 Function table

| address | size | name / role | callers | callees | fields / cvars | kind |
|---|---:|---|---|---|---|---|
| `0x36032d40` | 2667 | **`UpdateObstructionRays`** — timer, ray set, `RayWorldIntersection`, mode 1 vs 2 | CSoundObject update (0x36033xxx) | `0x36032840`, `0x36038b70` | sound `+0x24` flags, `+0x90` radius, `+0x184` ray block, `+0x20d/0x20e/0x20f`; cvar mode `DAT_360cd334+0xc4`, radius `+0xc4?` | state |
| `0x36032840` | 1264 | **`AccumulateObstruction`** — clear hit flags, weight by pierecability/accuracy, accumulate max + sum | `0x36032d40` | math helpers | ray block `+0x184`: count, per-ray `+0x1c` hit, `+0x24` obstruction, `+0x2c` distance, `+0x31` valid; accum `+0x214..+0x220`; cvars `s_ObstructionMaxPierecability`, `s_ObstructionAccuracy` | state |
| `0x360337c0` | 672 | **`UpdateObstruction`** — final value from max/sum, distance attenuation (`s_ObstructionMaxRadius`), mode 2 volume math | `0x36033xxx` update | `0x360310c0`, `0x3607ce00` | `+0x6c` cache, `+0x144` applied, `+0x184+0x214/0x21c`, `+0x58` volume; cvar `s_ObstructionMaxValue` | state |
| `0x360310c0` | 184 | `EffectiveVolume` — `volume * +0x78 * filter` plus distance/filter factors | `0x360337c0` | — | `+0x24` flags, `+0x20` type, `+0x78` volume, `+0xe4` | pure |
| `0x36038b70` | 146 | **`RayWorldIntersection` wrapper** — packs a 0x40-byte `ray_hit` query and calls `IPhysicalWorld+0xac` | `0x36032d40` | physics | start/end, flags, skip list, 5 hits | state (physics) |
| `0x360449f0` | 241 | `GetCachedObstruction` — returns the sound's cached obstruction, distance-corrected | `0x36044af0`, `0x360460c0`, `0x360463b0`, `0x36059387` | — | sound `+0x5` interface, `+0x3d..+0x3f`, `+0x78` flags; listener `+0xf0/+0xf4` | read |
| `0x36044af0` | 116 | apply cached obstruction + pick max | `0x36045060` | `0x360449f0` | `CSoundSystem+0x4370/+0x4374` | state |
| `0x36045060` | 161 | apply obstruction for a sound by entity/listener position | sound update `0x3603...` (site 45240) | `0x36044af0` | listener entity, `+0x4370` | state |
| `0x36045220` | 533 | **`ApplySoundObstruction`** — if sound has flag 0x4000, resolve its area/portal and set obstruction | CSoundSystem update/visarea (3 sites: 43283, 59997, 60246/60463) | `0x36030fc0` | sound `+0x24` (0x4000), `+0xf8..`, listener list `+0x42b0/+0x42b4` | state |
| `0x36045440` | 642 | **`CSoundSystem::RecomputeSoundOcclusion`** — listener visarea changed: recompute/clear cached obstruction for all sounds | listener update | `0x36030fc0`, sound interface `+0x44/+0x48` | sounds `+0x250/+0x254`, `+0x260/+0x264`, `+0x42b0/+0x42b4`, `+0x42c4`, `+0x4396` | state |
| `0x360456d0` | 843 | `SetListener` — listener position/orientation, detection of visarea changes, drives recompute | game/engine | `0x36051230`, `0x360389c0`, sound interface `+0x2c/+0x30` | listener `+0x3d..+0x3f`, `+0xc`, `+0x90` flags; `+0x4394` | state |
| `0x3607ccb6` | 6 | FMOD `Event::set3DOcclusion(occlusion, obstruction)` thunk | FMOD wrapper | FMOD | — | state |

The 154-function / 20.8 KB survey bucket also includes debug stats (`s_DrawObstruction`, ray
drawing), the `SSoundInfo` dumps and FMOD property plumbing; only the 12 bodies above implement
the algorithm.

### 4.5 Cvars and numeric parameters

| cvar | range / help text (shown value) | use in the algorithm |
|---|---|---|
| `s_Obstruction` | `0..2`; 1 = DSP based, 2 = volume based | mode select (`DAT_360cd334+0xc4`) |
| `s_ObstructionMaxPierecability` | `[0..]`, help shows 100; "accumulated pierecability of 50 gets half obstruction" | divisor of accumulated material pierecability |
| `s_ObstructionMaxRadius` | `[0..]`, help shows 500 m; "a sound with a radius of 500 m is not affected" | distance fade of obstruction |
| `s_ObstructionVisArea` | `[0..1]`, help shows 0.15 | extra obstruction per visarea/portal step |
| `s_ObstructionUpdate` | `[0..]` seconds | re-evaluation interval |
| `s_ObstructionAccuracy` | `[1..5]` | number of rays / sample accuracy |
| `s_ObstructionMaxValue` | `[0..1]` | clamp of the final obstruction |
| `s_DrawObstruction` | `0/1` | debug draw of the ray ball |

The defaults are from the cvar help text (`crysoundsystem.dll.strings.txt` 0x360a5c38..0x360a60xx);
the actual storage defaults were not read back (the cvars are registered through thunks that do
not embed a float default in the decompile). Verify with `s_Obstruction` in a live console. The
per-sound constants (`DAT_3609f...`) are unit conversions (1.0, 0.5, epsilon), not tunables.

### 4.6 In-process harness

1. **Cvar A/B (audible).** `s_Obstruction 0` then `2` (or `1`) next to a waterfall/behind a wall;
   the difference is the feature. `s_DrawObstruction 1` draws the source ball and rays, which
   confirms the ray set is built.
2. **Hook the raycast.** Hook `0x36038b70` (`RayWorldIntersection(Sound)`), play a 3D sound, and
   dump the start/end points, flags (`0x307`), skip list and ray count. Expected: 1 ray for mode
   1 and 2, 5 for mode 2 with accuracy 5.
3. **Read the sound object.** `sound+0x184` is the ray block: `u32 count` at +0, then 0x30-byte
   rays (`start +0x04`, `dir +0x10`, `length +0x1c`?, `hit +0x1c`, `obstruction +0x24`,
   `distance +0x2c`, `valid +0x31`). Read `+0x144` (obstruction applied), `+0x214/+0x21c`
   (accumulators), `+0x58` (volume) before/after moving behind cover.
4. **Call the system recompute.** From the client's sound system pointer (`gEnv+0x58`), call
   `vtable[0x??]`/`0x36045440(soundSystem, 1, 1, 0)` after teleporting the listener; then read
   each sound's cached value. No allocation; the system owns all sounds.

The `aa-audio` port does not need the FMOD calls: port `0x36032d40`/`0x36032840`/`0x360337c0`
with `aaemu-physics` raycasts and apply the result to the mixer gain (or a filter) in `aa-audio`.

### 4.7 Port order

1. Ray set + `RayWorldIntersection` adapter (pure given positions/physics; 2.7 KB).
2. `AccumulateObstruction` (pure, 1.3 KB) — unit-test with synthetic hits.
3. `UpdateObstruction` + `EffectiveVolume` (0.8 KB) — final value and distance fade.
4. Cvar plumbing + `RecomputeSoundOcclusion`/`SetListener` (1.5 KB) — needs visarea changes.
5. Apply to `aa-audio` gain/filter; compare with `s_Obstruction` A/B.
Skip FMOD-specific property calls, debug drawing and `SSoundInfo` dumps.

### 4.8 Unknowns

* **U11 [?]** Actual cvar defaults; read them live (`s_Obstruction` etc. print their values).
* **U12 [?]** The exact ray offsets/shortening (`DAT_3609eff4`, `0x360a1030`, `+0x94`) and the
  meaning of sound flag `0x4000` (visarea-aware obstruction). The code is readable but the
  constants need a live dump.
* **U13 [?]** Whether `s_Obstruction` is 1 or 2 in the shipped config; the help text presents both,
  and the mode changes the ray count and the application (DSP vs volume).
* **U14 [?]** `CSoundSystem` and `CSoundObject` field names/layouts; offsets above are inferred
  from the 1.2 bodies.

---

## 5. xlcommon: name/auction policy and hash variants

### 5.1 What it is

`xlcommon.dll` is the shared utility library. Two pieces matter to the client's visible rules:

* **Name policy** — the exact allowed-character/length rules for player-visible names
  (character, summon/pet, faction/guild, family title, chat tab, portal). x2game imports
  `XlIsValidName`, `XlIsValidAuctionSearchText` and `XlGetNamePolicyInfo`, and exposes the
  policy to the Lua UI as `local_min`, `local_max`, `eng_min`, `eng_max`, `limit_space_cnt`,
  `is_allow_space`, `is_allow_mix_lang`, `is_allow_mix_case`, `is_allow_special_letter`
  (`FUN_394c8ca0`).
* **Hash variants** — CRC32 (ubiquitous), MD5 (content cache key) and SHA1/SHA1-HMAC (auth
  digest). The other hash families (CityHash, Murmur1/2/3, lookup3, FNV, sumhash) are compiled
  in but not used by x2game; only `murmur3_32` is used by the renderer/3D engine.

### 5.2 Runtime use in 1.2: both used

`XlIsValidName` call sites in x2game [R: `calls.tsv`/decompile]:

| x2game site | policy type | inferred enum (from the Lua registration at `FUN_394c8e20`) | context |
|---|---:|---|---|
| `0x39416c20` (window proc) | 0 | `VNT_CHAR` | character-create / rename window; error text "wrong name format..." |
| `0x39069860`, `0x394aa0c0` | 1 | `VNT_SUMMONS` | summon/pet name; UI error id `0x150` |
| `0x392f8bb0`, `0x392f9020` | 2 | `VNT_FACTION` | faction/guild name; error ids `0x128/0x129` |
| `0x39458e40` | 4 | `VNT_CHAT_TAB` | chat tab name |
| `0x394cace0` | 5 | `VNT_PORTAL` | portal name (the Lua registration uses `"VNT_PORTAL"`) |
| `0x394c8ca0` | 0..5 | `XlGetNamePolicyInfo` | exposes the whole table to Lua |
| `0x398a6260` | — | `XlIsValidAuctionSearchText` | auction search string (min/max lengths at `DAT_39a2040c/14`) |

`ValidNameType` values are not named in 1.2 xlcommon strings, but the six names are registered
by x2game for the Lua `ValidName` table in this order: `VNT_CHAR`, `VNT_SUMMONS`, `VNT_FACTION`,
`VNT_FAMILY_TITLE`, `VNT_CHAT_TAB`, `VNT_PORTAL` (values 0..5 in that order, [I] from the
`XlIsValidName` type 5 == portal use). Type 3 (`VNT_FAMILY_TITLE`) has no direct x2game call in
the scanned sites; the UI still queries its policy.

### 5.3 Call chain

```
UI / character create / guild / chat tab / portal
└─ x2game wrapper (e.g. 0x394cace0 for portal)
   ├─ XlIsValidName(text, type)                    xlcommon 0x330095c0
   │  ├─ reject leading '.' 
   │  ├─ locale = XlGetLocale()                    xlcommon 0x33019920
   │  ├─ locale 2,6,7 -> 0x330091a0                (wide-char validator)
   │  ├─ locale 5     -> 0x33009320                (Hangul/Latin validator)
   │  └─ default      -> 0x33008f80                (Latin/ASCII validator)
   │     └─ policy = XlGetNamePolicyInfo(type)     0x33009650
   │        table = 0x3305db78 + (type + locale*6)*24
   └─ result -> UI error text or accept

Auction search
└─ 0x398a6260 -> XlIsValidAuctionSearchText         0x33009630
   └─ FUN_33009550 (length min/max + first char not space + MultiByteToWideChar)

Lua UI policy query
└─ 0x394c8ca0 -> XlGetNamePolicyInfo(type) -> Lua globals
```

### 5.4 The policy table (exact, recovered from the image)

`NamePolicyInfo` is 24 bytes: `i32 local_min, i32 local_max, i32 eng_min, i32 eng_max,
i32 limit_space_cnt, u32 flags`. Flags: bit 0 = allow space, bit 1 = allow mixed languages,
bit 2 = allow mixed case, bit 4 = allow special letters. The table has 8 locales × 6 types.
Rows (locale, type): `(local_min, local_max, eng_min, eng_max, space_cnt, flags)`:

```
locale 0: 0 (2,12,3,12,0,0)  1 (2,12,3,12,0,0)  2 (2,21,3,21,0,5)  3 (1,12,1,12,0,4)  4 (1,5,1,5,0,21)  5 (1,12,1,12,0,21)
locale 1: 0 (2,12,3,12,0,0)  1 (2,12,3,12,0,0)  2 (2,21,3,21,0,5)  3 (1,12,1,12,0,4)  4 (1,5,1,5,0,21)  5 (1,12,1,12,0,5)
locale 2: 0 (2,26,2,26,0,0)  1 (2,26,2,26,0,0)  2 (2,32,3,32,0,5)  3 (2,26,2,26,0,4)  4 (2,10,2,10,0,5)  5 (3,32,3,32,0,5)
locale 3: 0 (2,12,1,12,0,5)  1 (1,12,1,12,0,5)  2 (1,21,1,21,0,5)  3 (1,12,1,12,0,5)  4 (1,5,1,5,0,21)  5 (1,12,1,12,0,5)
locale 4: 0 (2,12,3,12,0,0)  1 (2,12,3,12,0,0)  2 (2,21,3,21,0,5)  3 (1,12,1,12,0,4)  4 (1,5,1,5,0,21)  5 (1,12,1,12,0,5)
locale 5: 0 (2,18,2,18,0,0)  1 (2,18,2,18,1,5)  2 (2,21,2,21,1,5)  3 (2,18,2,18,0,0)  4 (2,10,2,10,1,5)  5 (2,21,2,21,1,5)
locale 6: 0 (2,26,2,26,0,0)  1 (2,26,2,26,0,0)  2 (2,32,3,32,0,5)  3 (2,26,2,26,0,4)  4 (2,10,2,10,0,5)  5 (3,32,3,32,0,5)
locale 7: 0 (2,26,2,26,0,0)  1 (2,26,2,26,0,0)  2 (2,32,3,32,0,5)  3 (2,26,2,26,0,4)  4 (2,10,2,10,0,5)  5 (3,32,3,32,0,5)
```

This is a direct dump of the 1.2 image at `0x3305db78`; it reproduces the UI-visible limits
exactly. The client's active locale is `XlGetLocale()`; read it live rather than assuming.

### 5.5 Function table

| address | size | name / role | callers | callees | fields | kind |
|---|---:|---|---|---|---|---|
| `0x330095c0` | 76 | `XlIsValidName(text,type)` — locale dispatch + leading-dot reject | x2game 6 sites | `XlGetLocale`, `0x33008f80/0x330091a0/0x33009320` | `type` in EAX, locale | pure |
| `0x33009630` | 20 | `XlIsValidAuctionSearchText(text,min,max)` — delegates to `0x33009550` | x2game `0x398a6260` | `0x33009550` | two `NameLen{min,max}` | pure |
| `0x33009650` | 42 | `XlGetNamePolicyInfo(type)` — table lookup, clamps locale/type | x2game `0x394c8ca0` | — | `table + (type+locale*6)*24` | pure |
| `0x33008f80` | 543 | Latin/ASCII validator (locales 0,1,3,4) — two paths: Latin-first vs wide char; checks min/max, spaces, case and special letters | `XlIsValidName` | `XlIsEngAlpha`, `XlIsSpace`, `XlIsUpper`, `0x33008df0`, `MultiByteToWideChar` | policy row | pure |
| `0x330091a0` | 373 | wide-char validator (locales 2,6,7) | `XlIsValidName` | same + `0x33008eb0` | policy row | pure |
| `0x33009320` | 519 | Hangul/Latin validator (locale 5) | `XlIsValidName` | `XlIsEngAlpha`, `0x33008eb0` | policy row | pure |
| `0x33009550` | 97 | auction text: length in `[min,max]`, first char not space, valid MBCS | `XlIsValidAuctionSearchText` | `XlStringLen`, `MultiByteToWideChar` | `NameLen` | pure |
| `0x33008df0` | 149 | Unicode char-class helper (ranges) | `0x33008f80` | — | — | pure |
| `0x33008eb0` | 95 | wide-char helper for `0x330091a0/0x33009320` | validators | `XlGetChar`, `MultiByteToWideChar` | — | pure |
| `0x33019920` | 6 | `XlGetLocale()` | validators/policy | — | global locale | read |

### 5.6 Hash variants

| family | addresses (size) | used by x2game? | client use / action |
|---|---|---:|---|
| CRC32 | `reflect 0x3300ada0 (57)`, `init_CRC32_Table 0x3300ade0 (313)`, `Crc32Gen ctor 0x3300af20 (19)`, `Init 0x3300af40 (22)`, `GetCRC32Lowercase 0x33025670 (69)` / `0x33026d80 (79)`, `GetCRC32 0x33025d80 (50)` / `0x33026d30 (62)` | **yes, 34 sites** | string ids, animation/attribute names, sound names (`0x39111840` repeatedly), material/surface keys. Port with `crc32fast` if not already. |
| MD5 | `MD5Init 0x3300b460 (40)`, `MD5Transform 0x3300b490 (1626)`, `MD5Update 0x3300baf0 (191)`, `MD5Final 0x3300bbb0 (144)`, `md5 0x3300bc40 (84)` | **yes** (`0x398b0d80`) | content/cache key over a linked list of buffers; 16-byte digest at object `+0x24`. A Rust `md5` crate is byte-equivalent. |
| SHA1 | `sha1_starts 0x3300ca70 (53)`, `Sha1Transform 0x3300cab0 (5447)`, `update 0x3300e000 (173)`, `finish 0x3300e0b0 (303)`, `sha1 0x3300e1e0 (106)`, `sha1_hmac 0x3300e4d0 (77)` | **yes** (`sha1_hmac` at `0x3961bc50`) | 16-byte key + 20-byte message → 20-byte digest; NUL bytes are replaced by `~index` (`0x3961bc50`). This is an auth/session digest, not the account SHA-256 in `aa-net`. Port only if client-side auth replay needs it. |
| murmur3_32 | `0x3300c570 (165)` (+ 128-bit variants) | no (renderer uses it) | cry3dengine/cryrenderd3d10 import `murmur3_32`; not a client rule. Skip. |
| Murmur1/2/64A/64B/2A/Neutral2/Aligned2 | `0x3300bca0-0x3300c350` (~2.9 KB) | no | no x2game call site. Skip. |
| CityHash64/128 | `0x33009d70 (1032)`, `0x3300a180 (122)`, `0x3300a200 (751)`, `0x3300a4f0 (1925)`, `0x3300ac80 (228)`, `0x3300ad70 (39)` | no | no x2game call site; used by tools/other modules if any. Skip. |
| sumhash/sumhash32/DoNothingHash/FNV/Crap8/lookup3 | `0x3300afa0-0x3300b45c` (~1.2 KB) | no | no client call site. Skip. |

The 21.8 KB / 57-function survey bucket is the whole 0x33009d70-0x3300e600 band. Only **CRC32
(8 bodies, 0.7 KB), MD5 (5, 2.1 KB), SHA1/HMAC (7, 6.2 KB)** matter to the 1.2 client; the rest
is dead weight for this project.

### 5.7 In-process harness

1. **Policy (pure).** Call `XlIsValidName("abc",0)` etc. for a matrix of strings per type and
   read the boolean; compare with the table above. No state, no allocation. For locale-specific
   rows call `XlSetLocale`/`XlGetLocale` (if a setter exists in the driver build) or run under
   the shipped locale.
2. **Policy table.** Call `XlGetNamePolicyInfo(type)` and read 24 bytes; diff against the dump
   above for all 6 types.
3. **Auction text.** Call `XlIsValidAuctionSearchText(text,&min,&max)` with empty, 1-char,
   max+1-char and leading-space strings; expected false/true/false/false.
4. **CRC32.** Call `Crc32Gen::GetCRC32(gen,"string",len,seed)` and compare with a Rust
   `crc32fast`/table implementation. `GetCRC32Lowercase` first lowercases ASCII.
5. **MD5/SHA1-HMAC.** Known-answer tests against RFC 1321 / RFC 2202; then call the x2game
   wrappers (`0x398b0d80`, `0x3961bc50`) with synthetic buffers to confirm the final truncation
   (16-byte digest; 20-byte HMAC with `~index` NUL substitution).

### 5.8 Port order

1. Name policy: table + `XlGetNamePolicyInfo` + `XlIsValidName` (pure Rust, unit-test the table;
   ~0.5 KB).
2. Validators for the Latin path (`0x33008f80`), then wide/Hangul paths only if the client
   supports those locales.
3. Auction text validator (0.2 KB).
4. CRC32 only if a crate is not already used for names/ids (check `aa-*` first).
5. MD5/SHA1-HMAC only if the client replays auth/cache logic; `aa-net` already uses SHA-256 for
   account login, which is a different step.

### 5.9 Unknowns

* **U15 [?]** `ValidNameType` numeric order (0..5) is inferred from the six `VNT_*` names and the
  portal type-5 call; verify with a live `XlGetNamePolicyInfo` dump per type.
* **U16 [?]** `XlGetLocale()` mapping (0..7 → language names); not needed for the table port,
  but needed to pick the active row. `XlGetLocaleString` can name them at runtime.
* **U17 [?]** The exact character classes accepted by `0x33008df0` / `0x33008eb0` (Unicode
  ranges) need a table dump; the policy lengths/flags are exact, the per-character predicates
  are not yet transcribed.
* **U18 [?]** `sha1_hmac` key/message roles: the call passes `(out, 0x10, key, 0x14, msg)`, i.e.
  16-byte key and 20-byte message; the caller at `0x3961bc50` is the only x2game use. Confirm
  which protocol step it feeds before porting.

---

## 6. Port order and sizing

### 6.1 Recommended order

1. **xlcommon name policy** (pure, tiny, testable with the exact table in §5.4) — do it whenever
   character create/rename/guild/portal UI is touched.
2. **Movie Lua table** (no backend): register the 7 methods with a recorder/no-op, then wire to a
   cutscene scheduler when P5 starts. This unblocks the shipped login/trigger scripts.
3. **Sound occlusion** only during the P5 audio pass; the ray/accumulator bodies are pure and
   testable against synthetic hits before the mixer work.
4. **CRC32/MD5/SHA1-HMAC** only if a concrete consumer needs them (CRC32 first; MD5/SHA1 are
   auth/cache, and `aa-net` already covers the account SHA-256 step).
5. **CExactPositioning** — only after a live probe proves `SetTriggerState` runs. The first three
   ported pieces should be the trigger ring, `StartParams`, and `CalculateTarget`.
6. **Breakables** — only after a live probe proves `BreakIntoPieces` or the x2game
   `OnBreakable*` events fire. Port the manager only together with entity spawn and physics
   pieces.

### 6.2 Sizing

(a) **What phase 1 (riding/on-foot feel) needs: 0 functions / 0 KB.**
None of these pieces is on the input → movement request → actor update → rotation → physics
chain, the mount/rider graph, or the camera. The only theoretical link was EP for sitting/mount
alignment, and §1.2 shows the 1.2 mount/sit paths do not use it.

(b) **The rest** (measured `funcs.csv` bodies; debug bodies called out):

| piece | bodies | KB | status / note |
|---|---:|---:|---|
| EP core (19) | 19 | 23.5 | dormant; port only after a live probe |
| EP debug draw (2) | 2 | 4.6 | skip unless reproducing `ag_debugExactPos` |
| breakable key path (cryaction callback + manager key) | 5 | 21.0 | dormant (no placed breakables); probe first |
| breakable manager support / tails | 39 + 2 | 14.7 | skip unless the manager is ported |
| `Movie` bind (18) | 18 | 2.1 | live; only P5 backend remains |
| `Movie` shared ScriptBind helpers (3) | 3 | 0.3 | reuse `aa-lua` bind plumbing |
| sound occlusion key algorithm | 12 | 7.3 | live; P5; the survey's full bucket is 154 fns / 20.8 KB including debug/FMOD |
| xlcommon name/auction policy | 7 | 1.9 | live; P3/P5; pure |
| xlcommon hashes used by x2game (CRC32+MD5+SHA1/HMAC) | 20 | 9.0 | live where the consumer exists; whole band is 61 fns / 20.2 KB |
| **total, everything measured here** | **~127** | **~84** | — |
| **total, live and recommended (Movie + sound + policy + used hashes)** | **~60** | **~21** | — |
| **total, dormant (EP + breakables), only after positive probes** | **~67** | **~64** | — |

The survey's "to port" total of ~30.2 KB / ~55 fns is a **lower bound**: measured EP alone is
23.5 KB of live logic plus 4.6 KB debug, and the breakable path is 36.5 KB including the manager
support. The difference does not change the phase-1 answer (zero), but it does change the cost of
a "port everything" decision.

### 6.3 What "done" means for this bucket

* EP: a live probe shows whether it is dormant; if dormant, document it and close the item.
* Breakables: same; no world data places breakables, so the expected outcome is "close as
  unused" unless a live destructible doodad fires the manager.
* Movie: the Lua contract exists and the 7 methods are available; the backend is tracked with
  the P5 cutscene work.
* Sound: the occlusion algorithm is ported into `aa-audio` or explicitly accepted as an
  approximation; the cvar A/B is recorded.
* xlcommon: the policy table and auction rule are ported and unit-tested; only the hashes with a
  real consumer are ported.








