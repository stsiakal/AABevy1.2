# Port map: the 1.2 frame loop, timer and per-frame update order

> **Restored 2026-10-27 from `archive/fork-main-2026-10-27`, UNVERIFIED.** The client was restarted from NickMesser's `main`; nothing described here is wired into the game. Addresses, call chains and offsets must be re-checked against the decompile ([R]/[I]/[?]) before use. A 2026-10-27 audit found 151 of the 269 EXACT/TOLERANCE labels in `proof-levels.*` weak or wrong (report: `D:\aaemu-client\handoff\2026-10-27\audit\report-proofs.md`).

Address map of the 1.2 client's main loop, timing and update order, for a function-by-function
Rust port and for differential tests against the running hidden client (`aadriver.dll`).
Resolves roadmap v2 **E1 / F5** (tick rate and movement-send cadence).

## 0. Sources, conventions and licensing

### 0.1 Licensing

Derived **only** from the shipped 1.2 binaries and their decompiles. The published CRYENGINE
source was not used, quoted or consulted. Names come from the binaries' own RTTI/profiler/log
strings or from cvars; behaviour with no name in the binary is marked *(inferred)*. Port from
this document and the decompiles, never from the CRYENGINE source.

### 0.2 Sources

| source | path | base |
|---|---|---|
| CrySystem decompile | `D:/aa-re/client12/engine3/out/crysystem.dll.c` (+ `funcs.csv`, `strings.txt`, `exports.txt`) | `0x36500000` |
| CryScriptSystem (in progress) | `D:/aa-re/client12/engine3/out/cryscriptsystem.dll.c` | `0x36500000`-ish, check |
| CryAction decompile | `D:/aa-re/client12/engine/out/cryaction.dll.c` | `0x304f0000` |
| CryEntitySystem decompile | `D:/aa-re/client12/engine/out/cryentitysystem.dll.c` | `0x32000000` |
| CryAISystem decompile | `D:/aa-re/client12/engine2/out/cryaisystem.dll.c` | `0x31000000` |
| CryAnimation decompile | `D:/aa-re/client12/engine/out/cryanimation.dll.c` | strings only this pass |
| CryGame decompile | `D:/aa-re/client12/engine/out/crygame.dll.c` | strings only this pass |
| x2game decompile | `C:/AAEMU_DISK/aa-re/client12/out-x2game/x2game.dll.c` (+ `funcs.csv`, `calls.tsv`, `strings-xrefs.tsv`) | `0x38FF0000` |
| x2game image | `D:/aa-re/client12/x2game_unpacked_data.dll` | `0x38FF0000` |
| CryPhysics x86 1.2 | `D:/aa-re/cryphysics/x86_12/out/cryphysics.c`, `cryphysics_unpacked.dll` | `0x34FF0000` |
| physics port notes | `C:/Users/stsiakal/AAEmu/rust/crates/aaemu-physics/src/physvars.rs`, `README.md` | — |
| existing maps | `docs/research/port-movement/MAP.md`, `port-mount-anim/MAP.md`, `port-vehicles/MAP.md`, `port-flowgraph/MAP.md`, `port-program.md` | — |

Evidence markers: **[R]** read from the binary/decompile, **[I]** inference, **[?]** open.

### 0.3 Corrections to existing maps

* **`port-flowgraph/MAP.md` §3.2:** `gEnv[5]` is **ITimer**, not the game framework
  (`DAT_307a36b4[5]->vtable+0x30` is `CTimer::GetFrameTime`; the vtable at `0x3660c060` is
  `CTimer`). The game framework is reached from x2game: `CGame+0x18`.
* The game (IGame) is **`gEnv[1]` / `CSystem+0x2c`** (`ISystem vtable+0x168 = SetGame`,
  setter `FUN_3654d690`, called from the CGame constructor). CSystem::Update therefore calls
  `CGame::PrePhysicsUpdate` itself, not CryAction.
* Entity `PrePhysicsUpdate` receives the **frame time** (`timer->GetFrameTime`), not a physics
  substep dt.

## 1. The call chain

### 1.1 Outer loop (x2game, the application loop)

`X2Startup` object vtable is `0x399e6a38` (installed by `FUN_39509910`): slot 0 `Init`
`FUN_3950d5e0`, slot 4 `Shutdown` `FUN_39509940`, slot 8 `FUN_395099c0`, slot 12
`FUN_3950c3c0`, slot 24 `Run` `FUN_3950c4f0` **[R]**.

```
Run                                  x2game 0x3950c4f0  (452 B)
├─ PeekMessageA loop; DispatchMessageA; WM_QUIT -> return   (pump #1)
├─ startup->Update(1, 0)             x2game 0x395099c0  (194 B)
│  ├─ gEnv[0x16]->vtable+8          (frame budget/profile begin) [I]
│  ├─ CGame::Update(1, 0)            x2game vtable+0x1dc, body 0x390f6a4b
│  └─ gEnv[0x16]->vtable+0xc        (frame budget/profile end) [I]
├─ if (result == 0) break
└─ FUN_39509be0                     (keep window on screen, 0x39509be0) [R]
```

`CGame` is the global `DAT_3a5bb1e8`; it was created by `CreateGame(framework)` in
`FUN_39509870` (98 B). `CreateGame` then calls `CGame::Init(framework)` (vtable slot 0).
CryAction is loaded by `FUN_39509730` (`LoadLibraryA("cryaction.dll")`,
`CreateGameFramework`, `framework->Init`).

### 1.2 CGame::Update (x2game, once per outer iteration)

Body at `0x390f6a4b` (not present in `x2game.dll.c`, it sits in the obfuscated tail;
read from the image with capstone). Vtable slot `+0x1dc` (trampoline `0x390f6a20` ->
`0x39000b12`), profiler string `CGame::Update` `0x399a6d1c` **[R]**.

Order of calls (addresses = callee entry):

```
CGame::Update(this, haveFocus, flags)
 1. profiler begin (gEnv+0x97/0x98)
 2. save/clear per-frame accumulators this+0xc..+0x14 -> +0x64..+0x6c
 3. pSystem->vtable+0xb8()                IsDedicatedServer? -> optional client setup
 4. build two debug viewport structs (300x180, 1800x600)
 5. if (gEnv+0x94) GameClient::PreUpdate        0x39171470 (277 B)
 6. framework->vtable+0x20(1, flags)            CCryAction::PreUpdate 0x306f61b0
 7. framework->vtable+0x38() -> bool; if false: 0x39149980
 8. 0x39027df0 (if global set), 0x391739e0 (if gEnv+0x94)
 9. if pSystem->GetSystemMode() != 2:
      this+0x38 -> 0x393f4d90
      this+0x3c -> 0x39052ea0
      this+0x40 -> 0x3907b3c0
      this+0x44 -> 0x390844a0
10. if [0x3a5a4da8] in (1,2): 0x3916ac70
11. if (this+0x50): 0x3902b260
12. framework->vtable+0x24(1, flags)          CCryAction debug pass 0x306e9710
13. framework->vtable+0x28(1, flags)          CCryAction::PostUpdate 0x306f5280
14. if (gEnv+0x94) GameClient::PostUpdate      0x391716b0 (324 B)
15. game-time delta = timer->vtable+0x10(0) - previous (ms); stores [0x39af06dc]
16. client-state branches ([0x3a5a4da8] == 3/4/6): network/login/heartbeat
17. profiler end
```

`this+0x18` = `IGameFramework*` (set by `CGame::Init`). `this+4` = secondary listener
interface registered with the framework as `"Game"` via `framework->vtable+0x114` =
`FUN_306f66e0` (738 B) **[R]**.

### 1.3 CCryAction::PreUpdate / PostUpdate (CryAction game framework)

CCryAction / `IGameFramework` vtable (primary) is **`0x3074fa60`** **[R]**:

| slot | function | size | meaning |
|---|---|---|---|
| `+0x14` | `FUN_306f7200` | 1743 | `CCryAction::Init` (loads crysystem if needed, fills gEnv) |
| `+0x18` | `FUN_306efe10` | 821 | `CCryAction::CompleteInit` |
| `+0x20` | `FUN_306f61b0` | 681 | **`CCryAction::PreUpdate`** |
| `+0x24` | `FUN_306e9710` | 226 | debug/draw pass (calls +0x38, listener +8, two debug viewports) |
| `+0x28` | `FUN_306f5280` | 891 | **`CCryAction::PostUpdate`** |
| `+0x2c` | `FUN_306e95d0` | — | `Shutdown` |
| `+0x114` | `FUN_306f66e0` | 738 | add framework listener |
| `+0x120` | `FUN_306e9c20` | 40 | set level/name string (CGame::Init passes a string) |

`CCryAction::PreUpdate(this, flags, paused)`:

```
pSystem->vtable+0x24()                        CSystem::RenderBegin 0x3655ff70 (317 B)
dt = pSystem->GetITimer()->vtable+0x30(0)     CTimer::GetFrameTime
pSystem->vtable+0x1c(flags, paused)           CSystem::Update 0x365459b0 (4469 B)  <-- engine frame
send event 0xb to listeners (FUN_306f5ed0)
if (!paused && this+0x12f) FUN_306c8430(dt)   music logic [I]
if (this+0x134) FUN_3067f310(dt)              game-rules update [I]
if (this+0x135) (**(...))(dt)                 listener callback [I]
if (this+0x14d) this[0x14d]->vtable+0x14
if (this+0x14c) this[0x14c]->vtable+0x44
if (this+0x136) this[0x136]->vtable+0x10
```

`CCryAction::PostUpdate(this, flags, paused)`:

```
dt = pSystem->GetITimer()->vtable+0x30(0)
if (!paused) { this+0x139->vtable+8(dt); countdown this+0x162 -= dt (clamp 0) }
if (gEnv[0x15]) gEnv[0x15]->vtable+0xa0()
pSystem->vtable+0x28()                        CSystem::Render 0x36561650 (1389 B)
"CCryAction_PostUpdate_Listener":
  EnterCriticalSection; FUN_306f4690(this+0x155); for listeners: call vtable+0xc(dt)
  this->vtable+0x168(); gEnv[0x15]->vtable+0xb4()
pSystem->vtable+0x2c(1)                       CSystem::RenderEnd 0x36561bd0 (385 B)
gEnv[0xb]->vtable+0x48c()
```

So the **render** happens inside `CCryAction::PostUpdate`, after all game/physics updates.

### 1.4 CSystem::Update (crysystem, the engine frame)

`FUN_365459b0`, `0x365459b0`-`0x36546b24`, 4469 B. Called from `CCryAction::PreUpdate` as
`ISystem vtable+0x1c(flags, paused)`. `this` is `CSystem`; `gEnv` is the inline struct at
`CSystem+0x28` **[R]**.

Member map used below (**all [R]** unless noted):

| CSystem offset | object | evidence |
|---|---|---|
| `+0x2c` | `IGame*` (gEnv+4) | `ISystem vtable+0x168 = SetGame`, `FUN_3654d690`; CGame ctor calls it |
| `+0x30` | `INetwork` | `InitNetwork` |
| `+0x34` | `IRenderer` | `InitRenderer` |
| `+0x38` | `IInput` | `InitInput` (`CreateInput`) |
| `+0x44` | `IConsole` | console update |
| `+0x48/0x4c/0x50` | script systems | `InitScriptSystem` (3 instances) |
| `+0x54` | `I3DEngine` | `Init3DEngine` (`CreateCry3DEngine`) |
| `+0x58` | sound system | `InitSoundSystem` |
| `+0x5c` | `IMusicSystem` | sound init |
| `+0x60` | `IPhysicalWorld` | `InitPhysics` (`CreatePhysicalWorld`) |
| `+0x64` | `IMovieSystem` | `InitMovieSystem` |
| `+0x68` | `IAISystem` | `InitAISystem` |
| `+0x6c` | `IEntitySystem` | `InitEntitySystem` |
| `+0x78` | `ILog` | `CSystem::Init` |
| `+0x7c` | `ICharacterManager` | `InitAnimationSystem` |
| `+0x8ec` | `IStreamEngine` | stream update |
| `+0x118` | `ITimer` | `CTimer` ctor `FUN_3656f7d0(param_1+0x46)` |
| `+0x1954` | `sys_physics_CPU` (int cvar) | "turn on/off physics thread" |
| `+0x2194/0x21a4` | physics job / busy flag | physics thread helpers |
| `+0x21f0` | async physics interpolation dt | async path |

Order inside `CSystem::Update`:

```
 1. profiler; frame counter this+0x2210++; log flush; stream engine update
 2. early-outs (minimized / loading)
 3. PeekMessageA/DispatchMessageA loop (window this+0x19a4)        (pump #2)
 4. timer->vtable+8()                      CTimer::UpdateOnFrameStart   (dt produced)
 5. this+0x54 (I3DEngine)->vtable+0xb0     engine begin-frame [I]
 6. this+0xac->vtable+0x80                 unknown subsystem [?]
 7. this+0x2c (IGame)->vtable+0x1e0()      CGame::PrePhysicsUpdate
 8. this+0x2200 helper; script systems 0x48/0x4c/0x50 ->vtable+0xc
 9. input this+0x38 ->vtable+0x2c          IInput::Update (after movement sampling!)
10. console this+0x44 ->vtable+0x7c
11. if (!paused):
      sound this+0x58 ->vtable+0x98, then +4
      entity system this+0x6c ->vtable+0x10   IEntitySystem::PrePhysicsUpdate
12. FUN_3656a9b0(this+0x2224)               unknown [?]
13. PHYSICS BLOCK (this+0x60)               see 1.5
14. post physics: timer->GetFrameTime; movie this+0x64 ->vtable+0x9c;
      entity system this+0x6c ->vtable+0x18 = IEntitySystem::Update 0x32020ab0;
      I3DEngine->vtable+0x414 -> object->vtable+0xc (terrain/scene update) [I];
      sound this+0x58 ->vtable+4; music this+0x5c ->vtable+8;
      network this+0x30 ->vtable+0x38
15. return (bool)
```

`IEntitySystem::PrePhysicsUpdate` = `FUN_3201c890` (506 B, cryentitysystem) **[R]**:
`dt = gEnv[5]->GetFrameTime(0)`; iterate entities, skip logic-disabled/deferred ones, call
`CEntity::PrePhysicsUpdate(entity, dt)`. `CEntity::PrePhysicsUpdate` = `FUN_32010250`
(449 B) builds event `{type=0x1c, ..., dt}` and sends it to every entity proxy
(`proxy->vtable+0x14`). For x2game players this reaches `X2::CPlayer::PrePhysicsUpdate`
`0x39133280` and from there `CActor::UpdateMovementRequest` (port-movement §1.1).
`CAnimatedCharacter::PrePhysicsUpdate` `0x306634a0` (830 B, cryaction) is also an entity-proxy
receiver of the same event.

### 1.5 Physics block (same CSystem::Update, lines 56768-56890)

```
if (sys_physics_CPU > 0)  FUN_365446e0: world->vtable+0x88(); job->vtable+0x1c(job)
else                      FUN_36544730: wait for async physics to finish

world = this+0x60;  vars = world->vtable+0x54()      GetPhysVars (world+0x48)
if (async not active) {
    maxStep = this+0x68 ? AI->vtable+0x30() : 0.25f
    t0      = timer->vtable+0x0c(0)                  GetCurTime (discarded)
    world->vtable+0x8c()                             GetLastDt (discarded)
    realDt  = timer->vtable+0x30(0)                  GetFrameTime
    if (this+0xbe) realDt = timer->vtable+0x34(1)    GetRealFrameTime
    world->vtable+0xb0(1)                            flush queued RWI
    do {                                             (local_1c == 1: AT MOST ONE STEP)
        step = min(maxStep, realDt); realDt -= step
        if (paused || flags&2 || !DAT_3663b424 || cutscene)
             if ((++counter & 0x1f)==0) world->vtable+0x84(0.0f, 0x19f)   TimeStep
        else {
             world->vtable+0x90()                     GetLastStepICount
             world->vtable+0x8c()                     GetLastDt
             vars[0x190] = 1.0f; vars[0x148] = 0
             world->vtable+0x84(step, 0x19f)          TimeStep(step, 0x19f)
        }
        world->vtable+0x134()                        post-step update
    } while (realDt > 1e-4)                          (never: one iteration)
} else { ... async interpolation path ... }
world->vtable+0x164(&params)                         SetParams (pe_params type 0x16)
```

**F5 answer for physics:** exactly **one** `CPhysicalWorld::TimeStep` per frame in the normal
path (`local_1c` is initialised to 1 and decremented before the body), fed
`min(GetFrameTime(), maxStep)` where `maxStep` is `IAISystem vtable+0x30()`. CryAISystem's
slot `+0x30` = `0x31042b00`: it returns `this+0x13c->vtable+0xc()` or, with no delegate, the
constant `0x311b6484` = **0.1 s** **[R]**. If no AI system exists, the fallback is **0.25 s**.
So in the normal client the physics step is capped at **0.1 s** and there is **no catch-up**
(the leftover `realDt` is discarded because the loop body runs once). The timer's
`GetFrameTime()` is the *smoothed, time-scaled* frame time (§2). `TimeStep` itself further
clamps to `max_world_step = 0.2 s` and quantises to `time_granularity = 1e-4 s`
(aaemu-physics `physvars.rs`), both above the 0.1 cap. Physics is **not** run in fixed
substeps; the "fixed" option exists only via `fixed_time_step`/`p_fixed_timestep`.

### 1.6 What runs in one frame, in order, with the dt each step sees

| # | step | dt |
|---|---|---|
| 1 | startup pumps messages | — |
| 2 | `GameClient::PreUpdate` (`GameClient::PreUpdate` x2game 0x39171470) | — |
| 3 | `CCryAction::PreUpdate` -> `CSystem::RenderBegin` | — |
| 4 | `CTimer::UpdateOnFrameStart` | computes raw QPC dt; fixed/smoothing/clamp -> frame time |
| 5 | `CGame::PrePhysicsUpdate` -> `GameClient::PrePhysicsUpdate` -> input sampling + `UpdateMovement` | per-unit update, dt source TBD (movement uses its own clamp, see U5) |
| 6 | `IInput::Update` (engine input, after the movement sample) | frame time |
| 7 | `IEntitySystem::PrePhysicsUpdate` -> per-entity `PrePhysicsUpdate` | `timer->GetFrameTime()` (smoothed) |
| 8 | physics `TimeStep` | `min(GetFrameTime(), AI maxStep)`; AI default **0.1 s**, fallback 0.25 s; internally `min(dt, 0.2)`, quantised `1e-4` |
| 9 | `CSystem` post-physics: movie, entity systems, terrain, sound, music, network | frame time |
| 10 | x2game subsystems (`this+0x38..0x50`), client-state updates | — |
| 11 | `CCryAction::PostUpdate` -> `CSystem::Render` -> listeners -> `CSystem::RenderEnd` | `timer->GetFrameTime()` |
| 12 | `GameClient::PostUpdate` (movement send pump + camera?) | — |
| 13 | game clock delta (`timer->GetTime(0)`, ms); state machine | `timer->GetTime(0)` ms |

**The key phase-1 ordering:** the startup's message pump (#1) runs before the timer tick and
before `GameClient::PreUpdate`; queued `WM_KEYDOWN`/mouse messages are therefore already
dispatched when movement samples input in `GameClient::PrePhysicsUpdate` (step 5). That
sampling happens **before** the engine's own `IInput::Update` (step 6) and **before** the
entity pre-physics pass and the single physics step (7-8). `CSystem::Update`'s own message
pump (#2) runs after the movement sample, so input arriving during the frame is first seen
by the next frame. `GameClient::PreUpdate` (step 2) runs before the timer tick and thus sees
the previous frame's clock; `GameClient::PrePhysicsUpdate` (step 5) runs after it and sees
the new `GetFrameTime()`. The rider/mount pose is computed after the physics step, during the
entity/animation update and before `CSystem::Render`; the movement packet is pumped after the
render in `GameClient::PostUpdate` (step 12).

### 1.7 After physics: units, camera, animation (x2game)

`CGame::Update` calls `Client Game Update` `FUN_391739e0` (705 B, profiler
`"Client Game Update"` `0x399e...`) after `CCryAction::PreUpdate` returned (i.e. after
`CSystem::Update`/physics) and **before** `CCryAction::PostUpdate`/`CSystem::Render`
**[R]** (call site `0x390f6c5f`, guard `DAT_39b02294`).

Inside `Client Game Update` (head of the call list, `C:/AAEMU_DISK/aa-re/client12/out-x2game/x2game.dll.c:276275`):

```
FUN_391e72b0
FUN_3916ac70  -> CameraManager::Update 0x3916aab0 (421 B)
FUN_3942c640
FUN_39016dc0
... 40+ further per-frame subsystem updates ...
FUN_3936a750  X2::GameClient::UpdateUnits (units/models)
```

`CameraManager::Update` (`0x3916aab0`) reads `gEnv[5]->vtable+0x30(0)` = `ITimer::GetFrameTime`,
takes `dt = min(frameTime, 0.033f)` (`DAT_399a3040`), skips when the result equals
`DAT_3999c484` (0.0), and dispatches on `manager+4`: mode 1 `FUN_39169860` (unit/third
person), 2 `FUN_391632d0` (free), 3 `FUN_3916a260`, 4 `FUN_39164100`, 5 no update,
6 `FUN_39166390`. Mode 1 and the clamp are already mapped in
`docs/research/port-movement/MAP.md` §4. This is **phase-1 critical**: camera is computed
after the physics step and before render, with the 0.033 s cap.

`X2::GameClient::UpdateUnits` (`0x3936a750`): `now = timer->GetTime(0)` (ms, timer slot
`+0x10`), `deltaMs = now - last`; iterate the unit list `DAT_3a16999c`, call
`FUN_3936a0b0(deltaMs)`, `FUN_3933f1b0(unit, deltaMs)`, `FUN_3903ae20(deltaMs)` per unit.
The per-unit model update is `X2::ActorUnitModel::Update` `0x390dd080` (vtable
`0x399a51c8` slot 0), called from this unit update; it runs the rider feed
`FUN_390dcc80` (port-mount-anim §B) every frame. So the **rider/mount pose and camera are
both recomputed after physics and before render**, with millisecond game deltas for units
and the clamped frame time for the camera.

Animation inside `CAnimatedCharacter`: the entity pre-physics event `0x1c` reaches
`CAnimatedCharacter::ProcessEvent` `FUN_30663ac0`, which calls
`CAnimatedCharacter::PrePhysicsUpdate` `0x306634a0`; the per-frame
`CAnimatedCharacter::Update` `0x306637e0` (vtable `0x30746730` +0x28) builds the frame
parameters (`FUN_3066ea40`), pushes them to the graph state (`FUN_30671e70`) and updates the
character instance (`FUN_30671cc0`). Its caller is virtual and not statically resolved; it
runs in the same post-physics, pre-render window (U4, internals owned by E2).

## 2. The timer (`CTimer`)

### 2.1 Object and vtable

Timer object at `CSystem+0x118`; vtable **`0x3660c060`** (crysystem) **[R]**:

| slot | function | size | meaning |
|---|---|---|---|
| `+0x04` | `FUN_3656f410` | 102 | `Reset` / init clocks |
| `+0x08` | `FUN_3656ef80` | 1161 | **`UpdateOnFrameStart`** |
| `+0x0c` | `FUN_3656ed30` | 20 | `GetCurTime(param2)` = `samples[param2] * DAT_3660af20` |
| `+0x10` | `FUN_3656f740` | 46 | `GetTime(param2)` = `samples[param2] * 1000 / 1000000` (ms, int64) |
| `+0x2c` | `FUN_3656e760` | — | `GetAsyncCurTime` (float s) |
| `+0x30` | `FUN_3656e6d0` | 37 | **`GetFrameTime(bool)`** -> `this+0x38` (smoothed/scaled) |
| `+0x34` | `FUN_3656e700` | 37 | `GetRealFrameTime(bool)` -> `this+0x40` (raw) |
| `+0x3c` | `FUN_3656e740` | — | `SetTimeScale` -> `this+0x258` |
| `+0x40` | `FUN_3656e950` | — | enable timer |
| `+0x48` | `FUN_3656e7b0` | — | `1 / realFrameTime` |
| `+0x54` | `FUN_3656f500` | 418 | pause/unpause (offsets frozen while paused) |
| `+0x5c` | `FUN_3656f6b0` | — | reset clocks to now |
| `+0x68` | `FUN_3656ea90` | — | store file-time offset |
| `+0x6c` | `FUN_3656eac0` | — | file-time now |

Constructor `FUN_3656f7d0` (699 B) sets `ticksPerSecond = 1,000,000`, time source
`FUN_3656eb20` (`hpc2usec(QPC)`), and the 100-entry frame-history ring at `+0xb0`
(initialised to 1/30 s) **[R]**.

### 2.2 Cvars and defaults (registered in `FUN_3656eb40`, 488 B)

| cvar | type | offset | default | note |
|---|---|---|---|---|
| `fixed_time_step` | float | `+0x24c` | `0.0` | game updated with this fixed frame time; negative = sleep to fixed rate |
| `max_time_step` | float | `+0x250` | **0.25 s** | game systems clamped to this frame time |
| `min_time_step` | float | `+0x254` | **1/150 = 0.0066666668 s** | busy-wait floor on the raw frame time (max ~150 fps) |
| `time_scale` | float | `+0x258` | `1.0` | slow motion |
| `e_time_smoothing` | int | `+0x25c` | `1` | 0 = no smoothing |
| `profile_smooth` | float | `+0x260` | `0.35 s` | profiler only |
| `profile_weighting` | int | `+0x264` | `0` | profiler only |
| `e_timer_debug` | int | `+0x26c` | `0` | logs the `[CTimer]` lines |

### 2.3 `CTimer::UpdateOnFrameStart` (`FUN_3656ef80`) — the dt pipeline

```
raw  = (now_usec - last_usec) / 1,000,000                     // seconds
m_frameTime(+0x38) = raw; m_realFrameTime(+0x40) = raw
if (fixed_time_step != 0):
    if (fixed < 0): CrySleep((abs(fixed) - raw) * 1000 us); remeasure raw
    frame = abs(fixed_time_step)
else:
    while (raw < min_time_step):  spin / SwitchToThread; remeasure raw
    frame = time_scale * min(raw, max_time_step)
if (frame < 0) frame = 0
if (e_time_smoothing > 0):
    frame = FUN_3656e7e0(this, 0.25 s, frame, lastSmoothed(+0x23c))
lastSmoothed = frame
if (frame != real):
    asyncCurTime += (frame - real) * 1e6                  // keep async clock continuous
m_frameTime = frame
frameTicks(+0x3c) = tickDelta * 0.01                       // DAT_36615fe0 (double 0.01)
profiler smoothing FUN_3656ed70
```

Smoothing `FUN_3656e7e0` (347 B): push the sample into the 100-float ring; clamp the sample
to `0.4 s`; `N = round(0.25 / lastSmoothed)` capped at 100 (min 1); return the average of the
last N samples; if the sample is below `1e-4` return it directly. This is a *moving average
whose window length adapts to the previous frame time* (≈ 15 samples at 60 fps, 25 at 150 fps).
`FUN_3656ed70` (300 B) is a separate profiler-only exponential smoothing (`profile_smooth`,
`profile_weighting`), not game dt.

Numeric constants read from the DLL (crysystem VA -> value) **[R]**:
`0x3660c374 = 0.25`; `0x36616350 = 1/150`; `0x3661634c = 1/30`; `0x366160b4 = 0.35`;
`0x36624c34 = 1.0`; `0x36609840 = 1e-4`; `0x36609844 = 1000.0`;
`0x36615fe0 = 0.01` (double); `0x3660b730 = 0.4`; `0x36609820 = 0.5`;
`0x3660aee0 = 0x7fffffff`; `0x3660b6e0 = 0x80000000` (sign bit).

## 3. `CGame::PrePhysicsUpdate` (x2game)

`FUN_390f5360` (223 B), vtable slot `CGame+0x1e0`, profiler `CGame::PrePhysicsUpdate`
**[R]**:

```
if (gEnv+0x25 != 0)                        // "is client"
    GameClient::PrePhysicsUpdate()         x2game 0x39171590 (288 B)
```

`GameClient::PrePhysicsUpdate` (`FUN_39171590`) calls in order:

```
X2::GameClient::InputManager::Update      x2game 0x39406540
FUN_3935f3b0
FUN_393527f0
```

The whole input → `UpdateMovement` → movement request chain is mapped in
`docs/research/port-movement/MAP.md` §1.1 and must not be re-derived here. Note that this
runs from **CSystem::Update step 7**, i.e. before engine input and physics.

### 1.8 Movement send cadence (F5, `CSMoveUnit`)

`Client Game PostUpdate` `0x391716b0` runs at the **end** of `CGame::Update`, i.e. after
`CSystem::Render`. It calls the pump `FUN_39406740` (307 B) once:

```
dt = gEnv[5]->GetFrameTime(0)            // ITimer, smoothed frame time
FUN_39406650(state0, dt)                 // local player
FUN_39406650(state1, dt)                 // primary controlled unit
for 5 more input slots:
    state->accum += dt                   // float at state+4
    mover = FUN_394064b0(unit)           // on foot FUN_394046f0, mounted FUN_394045f0
    consume pending flags 0x1 / 0x2000
```

Each per-unit send state accumulates `dt` at `+0x4`; on a successful send the builder
resets `+0x4 = 0`. Gate constants are `0.001` (`_DAT_3999cf30`) times an integer read from a
cached interface:

* mounted (`FUN_394045f0`): send when `accum >= clockB->vtable+8() * 0.001`; **no change
  suppression** — the packet is rebuilt and sent every interval while mounted.
* on foot (`FUN_394046f0`): send on any movement change; an **unchanged** state is only
  re-sent when `accum > clockA->vtable+8() * 0.001`.
* both clocks are process globals (`DAT_3a59f84c`, `DAT_3a59f850`); neither is assigned in
  any decompiled x2game function (set from the network/startup path), so the interval values
  are still **U5**. Measure inter-packet times in the live client; the interval is in ms
  (scaled by 0.001).

After the pump, `FUN_393d57b0()` / `FUN_390b3750()->vtable+100` flush the network. Because
the pump runs once per rendered frame, the maximum send rate is one `CSMoveUnit` per unit per
frame, gated by the interval above.

**F5 resolution (tick rate and send cadence):** the client has **no fixed simulation tick**.
There is exactly one timer tick and (normally) one physics `TimeStep` per rendered frame; the
step dt is the smoothed, time-scaled frame time capped at **0.1 s** (AI system) and quantised
to 1e-4 s by the physics world. Movement upload is also once per frame at most, from
`Client Game PostUpdate` after rendering; on foot it is change-driven with a minimum resend
interval, mounted it is periodic (`clockB`). The only fixed-step modes are opt-in cvars
(`fixed_time_step`, `p_fixed_timestep`, `p_single_step_mode`).

## 4. Function inventory (E1 scope)

| module | address | size | function | callers / callees | state / notes |
|---|---|---|---|---|---|
| x2game | `0x3950c4f0` | 452 | `X2Startup::Run` | vtable slot 24; calls `0x395099c0`, `FUN_39509be0` | stateful; pump #1 |
| x2game | `0x395099c0` | 194 | `X2Startup::Update` | Run -> this; calls CGame vtable+0x1dc | stateful (reentrancy guard `DAT_3a5bb20b`) |
| x2game | `0x39509870` | 98 | CreateGame wrapper | startup init | calls `CreateGame`, `CGame::Init` |
| x2game | `0x390f31d0` | ~0x140 | CGame constructor | CreateGame; sets vtables, `pSystem->SetGame(this)` | stateful |
| x2game | `0x390f6a4b` | ~0x464 | `CGame::Update` body | slot `+0x1dc`; see §1.2 | stateful; not in `.c` |
| x2game | `0x390f5360` | 223 | `CGame::PrePhysicsUpdate` | slot `+0x1e0`, called by CSystem::Update | stateful |
| x2game | `0x39171470` | 277 | `GameClient::PreUpdate` | CGame::Update; `0x3942cc80`, `0x3917bbf0` | stateful |
| x2game | `0x39171590` | 288 | `GameClient::PrePhysicsUpdate` | CGame::PrePhysicsUpdate; `0x39406540`, `0x3935f3b0`, `0x393527f0` | stateful; **phase-1** |
| x2game | `0x391716b0` | 324 | `Client Game PostUpdate` | CGame::Update; `0x39424410`, `0x39414c00`, `0x3931f1b0`, `0x39406740` (send pump), `0x393d5b30` | stateful; **F5** |
| x2game | `0x39406740` | 307 | movement send pump | PostUpdate; per-unit timer `0x39406650` | stateful; mapped in port-movement |
| x2game | `0x390a3bb0` | 558 | `X2::NetActorMovementController::Update` | vtable `0x399a3314`; calls `UpdateLivingEntity` | remote actors; already in port-movement |
| x2game | `0x390a30e0` | 2759 | `X2::NetActorMovementController::UpdateLivingEntity` | `0x390a3bb0` | remote interpolation/extrapolation; **0.033 s cap** `0x399a3040`, already in port-movement |
| x2game | `0x3916aab0` | 421 | `X2::GameClient::CameraManager::Update` | `0x3916ac70`; timer GetFrameTime; modes | `dt = min(frameTime, 0.033)`; **phase-1** |
| x2game | `0x391739e0` | 705 | `Client Game Update` | CGame::Update step 8; camera + units + 40 subsystems | after physics, before render |
| x2game | `0x3936a750` | — | `X2::GameClient::UpdateUnits` | Client Game Update; timer GetTime(0) ms | units/models/rider feed per frame |
| x2game | `0x390dd080` | — | `X2::ActorUnitModel::Update` | unit update; vtable `0x399a51c8` slot 0 | calls rider feed `0x390dcc80` |
| cryaction | `0x306f61b0` | 681 | `CCryAction::PreUpdate` | framework slot +0x20 | stateful |
| cryaction | `0x306f5280` | 891 | `CCryAction::PostUpdate` | framework slot +0x28 | stateful |
| cryaction | `0x306e9710` | 226 | framework debug pass | slot +0x24 | stateful |
| cryaction | `0x306f7200` | 1743 | `CCryAction::Init` | slot +0x14 | stateful |
| cryaction | `0x306efe10` | 821 | `CCryAction::CompleteInit` | slot +0x18 | stateful |
| crysystem | `0x365459b0` | 4469 | `CSystem::Update` | CCryAction::PreUpdate; see §1.4 | stateful |
| crysystem | `0x3655ff70` | 317 | `CSystem::RenderBegin` | CCryAction::PreUpdate | stateful |
| crysystem | `0x36561650` | 1389 | `CSystem::Render` | CCryAction::PostUpdate | stateful |
| crysystem | `0x36561bd0` | 385 | `CSystem::RenderEnd` | CCryAction::PostUpdate | stateful |
| crysystem | `0x365440b0` | ~193 | render pre-phase helper | CSystem::Update? | stateful |
| crysystem | `0x3656ef80` | 1161 | `CTimer::UpdateOnFrameStart` | timer vtable+8, CSystem::Update | stateful; **phase-1** |
| crysystem | `0x3656e7e0` | 347 | frame-time moving average | UpdateOnFrameStart | stateful (ring) |
| crysystem | `0x3656ed70` | 300 | profiler smoothing | UpdateOnFrameStart | stateful (profiler) |
| crysystem | `0x3656f7d0` | 699 | `CTimer` ctor | CSystem ctor | stateful |
| crysystem | `0x3656eb40` | 488 | timer cvar registration | timer init | stateful |
| crysystem | `0x3656f500` | 418 | pause/unpause | timer vtable+0x54 | stateful |
| crysystem | `0x3656e6d0` / `0x3656e700` | 37 / 37 | GetFrameTime / GetRealFrameTime | everywhere | pure read |
| crysystem | `0x3656f740` | 46 | GetTime ms | CGame::Update | pure read |
| cryentity | `0x3201c890` | 506 | `CEntitySystem::PrePhysicsUpdate` | CSystem::Update (+0x6c -> vtable+0x10) | stateful |
| cryentity | `0x32010250` | 449 | `CEntity::PrePhysicsUpdate` | entity system; sends event 0x1c{dt} | stateful |
| cryentity | `0x32020ab0` | 491 | `CEntitySystem::Update` | entity system post pass [?] | stateful |
| cryaction | `0x306634a0` | 830 | `CAnimatedCharacter::PrePhysicsUpdate` | entity event receiver | stateful |
| cryaction | `0x306637e0` | 487 | `CAnimatedCharacter::Update` | virtual, caller TBD | stateful |
| cryphysics | `0x3509f6a0` | — | `CPhysicalWorld::TimeStep` | CSystem physics block via world+0x84 | stateful; dt clamp 0.2 |
| cryphysics | `0x35089160` | 16 | GetPhysVars (world+0x48) | CSystem physics block | pure read |
| cryphysics | `0x350891a0` / `0x350891b0` | 16 / 16 | GetLastDt / GetLastStepICount | CSystem physics block | pure read |
| cryphysics | `0x351c3f70` | — | flush queued RWI | CSystem physics block | stateful |
| cryphysics | `0x350a2d70` | — | post-step update | world+0x134 after TimeStep | stateful |
| cryphysics | `0x350880c0` | — | world SetParams | world+0x164 | stateful |

## 5. In-process oracle (aadriver) recipes

The hidden 1.2 client's globals/pointers used below are absolute VAs in the module; the driver
must rebase by its module base.

* **Timer:** `CTimer* t = *(void**)(gEnv + 0x14)` (crysystem `gEnv = *(void**)0x366381b4`).
  Read `*(float*)(t+0x38)` (frame time), `*(float*)(t+0x40)` (real), `*(float*)(t+0x23c)`
  (last smoothed), `*(int*)(t+0x270)` (frame counter). Patch `+0x24c..+0x258` directly or via
  the cvars. Call `t->vtable[2]()` (slot +8) in a wrapper to reproduce one timer tick; then read.
* **System update:** `pSystem = *(void**)gEnv`; call `pSystem->vtable[7](flags, paused)`
  (slot +0x1c). Dangerous (does input/render/physics); use only in the recorder harness.
* **Game:** `CGame* game = *(void**)0x3a5bb1e8` (x2game). `game->vtable[119]()` (byte
  `+0x1dc`) = CGame::Update(1,0); `game->vtable[120]` = PrePhysicsUpdate.
  `framework = *(void**)((char*)game + 0x18)`; framework slots +0x20/+0x28 = Pre/PostUpdate.
* **GameClient:** call `0x39171590` / `0x391716b0` directly (no args).
* **Physics:** `world = pSystem+0x60`; `TimeStep(dt, 0x19f)` via `world->vtable+0x84`;
  read `GetLastDt` via `+0x8c`; `GetPhysVars` via `+0x54`.
* **Render:** `pSystem->vtable+0x28()`; RenderBegin/End are `+0x24`/`+0x2c`.
* Record one line per frame with timer values, game time (`timer->GetTime(0)`), the input
  axes (port-movement oracle), the movement packet send timestamps, camera matrix and rider
  bone. Compare against the Rust port at the same tick indices.

## 6. Port order (testable leaves first)

1. `CTimer` pure reads and layout (`GetFrameTime`, `GetRealFrameTime`, `GetTime`) — unit tests
   against dumped timer memory.
2. `CTimer::UpdateOnFrameStart` — feed raw QPC deltas and cvar values; test fixed negative,
   fixed positive, max clamp, min busy-wait, time scale, smoothing window.
3. `CSystem::Update` ordering skeleton with stubs for input/physics/render — assert the call
   order from a scripted hook log.
4. Physics step driver: `dt = min(frameTime, maxStep)`, one step, flags 0x19f; test against
   `aaemu-physics` `World::time_step` with the same dt.
5. `CCryAction::PreUpdate`/`PostUpdate` ordering and render placement.
6. `CGame::Update` and `GameClient::Pre/PostUpdate` (movement send cadence) — link to
   port-movement F5.
7. Entity pre-physics pass and animation-proxy event order.

## 7. Numeric parameters (summary)

* Timer ticks: 1,000,000/s (microseconds); timer history 100 floats.
* `max_time_step` 0.25 s; `min_time_step` 1/150 s; `time_scale` 1.0.
* Frame smoothing: 0.25 s window, sample clamp 0.4 s, adaptive N, history 100.
* Physics: `maxStep` = **0.1 s** (CryAISystem slot +0x30 default) or 0.25 s without AI;
  `max_world_step` 0.2 s;
  `time_granularity` 1e-4 s; `max_substeps` 5; TimeStep flags 0x19f (0x1f visible bits |
  0x180); epsilon 1e-4.
* Entity pre-physics event type 0x1c; physics `pe_params_world` type 0x16.
* Render viewports seen: 300x180 and 1800x600 (debug/console).

## 8. Unknowns / still to map

* **U1 (partly resolved)** `+0xac` is the optional **Seamless System** object (created by
  `FUN_365ee580` when seamless is enabled); `+0x2224` is the `IThreadTaskManager`
  (`FUN_3656a9b0` = `CThreadTaskManager::OnUpdate`). Still open:
  `FUN_36543750(+0x2200)` (thread/budget governor [I]) and `this+0x2209`/`+0x2200`.
* **U2** `CGame::Update` x2game subsystems `this+0x38/0x3c/0x40/0x44/0x50` (names/callers).
* **U3** Camera update: likely inside `GameClient::PostUpdate` (`0x39424410`? `0x3931f1b0`?)
  or in a per-frame x2game camera function; not yet located. Find the writer of the view
  matrix and its position in PostUpdate (phase-1 critical).
* **U4** Animation pose update: `CAnimatedCharacter::Update` (`0x306637e0`) caller is virtual;
  `CSkeletonAnim::ProcessAnimations` (`0x3167a630` string) and `ca_ForceUpdateSkeletons`.
  Determine whether pose processing happens in entity post-update or render, and the dt it
  receives (E2 owns the internals; E1 only needs the slot).
* **U5** Movement dt and send intervals: `GameClient::InputManager::Update` ->
  `UpdateMovement` uses `CTimer::GetAsyncCurTime` (`gEnv[5]->vtable+0x2c`, float seconds) for
  its turn/strafe clock; the exact per-frame dt handed to the turn formulas is still open.
  The two send intervals behind `DAT_3a59f84c`/`DAT_3a59f850` (ms, scaled by 0.001) are open.
  The 0.033 clamp is confirmed at `0x399a3040` and is used by the **remote** actor
  controller (`X2::NetActorMovementController::UpdateLivingEntity` `0x390a30e0`) and by the
  camera. Measure the intervals and the local movement dt live.
* **U6** Exact semantics of world slots `+0x88` (0x35088520, TLS cleanup), `+0x8c`/`+0x90`
  (GetLastDt/GetLastStepICount), `+0xb0` (queued RWI), `+0x134` (post step), `+0x164`
  (SetParams); names inferred.
* **U7** `CSystem::Render` internals: order of 3DEngine/sprite/UI render and where
  `RenderBegin`/`RenderEnd` really belong (observed: Begin in PreUpdate, Render in PostUpdate).
* **U8** Threading: `sys_physics_CPU` path (`FUN_365446e0`/`FUN_36544730`, job queue at
  `+0x2194`, interpolation at `+0x21f0`) — only used when the cvar is non-zero; default 0?
  Confirm default.
* **U9** CryScriptSystem per-frame update (it is one of the three script systems updated at
  `CSystem::Update` step 8); `cryscriptsystem.dll.c` now exists but is still being read.
* **U10** `CEntitySystem` post pass: `this+0x6c -> vtable+0x18` (probably
  `CEntitySystem::PostUpdate`/`Update`); `CEntitySystem::Update` `0x32020ab0` may belong here.
* **U11** `CGame::Update` client-state branches (`[0x3a5a4da8]` 1/2/3/4/6) and heartbeat
  logic; not needed for phase 1 but they affect world/zone transitions.
* **U12** `CTimer::UpdateOnFrameStart` negative-fixed sleep units (`CrySleep` argument is
  microseconds) and the exact meaning of `+0x3c` (tick delta * 0.01).

## 9. Size of the work

Phase-1 (input -> movement -> physics -> rider pose -> camera -> send, including timer and
ordering): **≈ 25 functions, ≈ 12 KB of decompile** (timer 3.4 KB, CSystem::Update 4.5 KB,
CCryAction Pre/Post 1.6 KB, x2game entry points 1.3 KB, entity pre-physics 1.0 KB,
physics driver 1.2 KB), plus the already-ported movement/physics functions.

The rest of E1 (render internals, async physics thread, camera subsystem, animation playback,
client-state machine): **≈ 60-80 functions, ≈ 40-60 KB**, mostly in CSystem::Render, CryAction
listeners, CryAnimation skeleton processing and x2game GameClient. E2/E3 will absorb most of
the animation/entity-glue part.
