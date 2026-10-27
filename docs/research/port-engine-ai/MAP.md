# Port map: CryAISystem in the ArcheAge 1.2 client (E7)

> **Restored 2026-10-27 from `archive/fork-main-2026-10-27`, UNVERIFIED.** The client was restarted from NickMesser's `main`; nothing described here is wired into the game. Addresses, call chains and offsets must be re-checked against the decompile ([R]/[I]/[?]) before use. A 2026-10-27 audit found 151 of the 269 EXACT/TOLERANCE labels in `proof-levels.*` weak or wrong (report: `D:\aaemu-client\handoff\2026-10-27\audit\report-proofs.md`).

What of `CryAISystem.dll` the 1.2 client really runs, mapped to the function level, with an
in-process oracle plan and a size estimate for a Rust port. This map also records the x2game
hand-over points that reach the AI system, because the engine interface alone does not show why
the client touches it.

## 0. Sources, conventions, licensing

### 0.1 Licensing

Everything here is derived **only** from the shipped 1.2 binaries and their decompiles. The
published CRYENGINE source was not used, quoted or consulted. Class names in `CClass` form come
from the binaries' own profiler registrations, RTTI, log/assert strings or cvar names; behaviour
that has no name in the binary is marked *(inferred)*. Port from this document and the decompiles,
never from the CRYENGINE source. 10.x builds were not needed for this map.

### 0.2 Sources

| source | path |
|---|---|
| 1.2 CryAISystem decompile | `D:/aa-re/client12/engine2/out/cryaisystem.dll.c` (image base `0x31000000`) |
| 1.2 CryAISystem functions/imports/strings | `D:/aa-re/client12/engine2/out/cryaisystem.dll.{funcs.csv,imports.txt,strings.txt}` |
| 1.2 CrySystem decompile | `D:/aa-re/client12/engine3/out/crysystem.dll.c` (base `0x36500000`) |
| 1.2 CryEntitySystem decompile | `D:/aa-re/client12/engine/out/cryentitysystem.dll.c` (base `0x32000000`) |
| 1.2 CryAction decompile | `D:/aa-re/client12/engine/out/cryaction.dll.c` (base `0x304f0000`) |
| 1.2 x2game decompile | `C:/AAEMU_DISK/aa-re/client12/out-x2game/x2game.dll.c` (base `0x38ff0000`) |
| x2game calls/strings/xrefs | `D:/aa-re/client12/out-x2game/{calls.tsv,strings.txt,strings-xrefs.tsv}` |
| client Lua bytecode | `game/scriptsbin/**.alb` in `D:/AA1.2/game_pak`, disassembled with `cargo run -p aa-lua --bin aa-lua -- dis <path>` |
| real-client runtime log | `D:/AA1.2/ailog.log`, `D:/AA1.2/aisignals.csv` |
| existing maps | `port-engine-loop/MAP.md` (E1 frame loop, physics cap), `port-movement/MAP.md` (P1 controller), `port-engine-survey/content.md` §3 (survey upper bound) |

Evidence markers: **[R]** read in the binary/decompile, **[I]** inference, **[?]** open.

### 0.3 The one offset trap: `gEnv+0x40`, not `gEnv+0x68`

E1 lists `CSystem+0x68 = IAISystem`. That is the field of `CSystem`. `ISystem::GetGlobalEnvironment`
(vtable `+0x14`, crysystem `0x3654d3d0`: `lea eax,[ecx+0x28]; ret`) returns a pointer to the inline
`SSystemGlobalEnvironment` at `CSystem+0x28`. Therefore:

```
gEnv  = *(void**)(crysystem_module_base + (0x366381b4 - 0x36500000));
AI    = *(void**)(gEnv + 0x40);          // == CSystem+0x68
```

x2game stores the same pointer in `DAT_3a5bb1d8`; CryEntitySystem in `DAT_320da0a4`; CryAction in
`DAT_307a36b4`. Every `DAT_xxx + 0x40` in this document is the AI interface pointer, not a log
pointer. The AI system's own logging methods at vtable `+0x228/+0x22c/+0x238` are what turn those
`<CAIHandler>` strings into `AI:` log lines.

### 0.4 Terms used below

* **AI object** — `CAIObject`/`CAIActor`-family instance owned by `CAISystem`; the base object is
  `0xc0` bytes, created by `CAISystem::CreateAIObject` (vtable `+0x44`, `0x31062120`).
* **anchor** — an entity registered with the AI system by `AI.RegisterWithAI`; the client's anchor
  scripts pass types `1001..1006` (`AIAnchorTable`), so they fall into the default `CAIObject` case.
* **AI point** — a `CGraph` node created from an `AIPoint` prefab by x2game (type 4, or 1..5 from
  the prefab's anchor type); it is world placement data, not a goal/behaviour.
* **navigation modifier** — a named navigation shape/area created from an `AINavModifier` prefab
  through `CAISystem` vtable `+0xbc` and removed with `+0xc0`.
* **AI listener** — an entity proxy that registers a sphere/type mask in the AI event-listener
  vector (`CAISystem+0x1358`) through vtable `+0xb4`.

---

## 1. Answer: what the client really runs

The 1.2 client runs **only the bookkeeping shell** of CryAISystem:

1. **Lifecycle.** `CrySystem` loads `CryAISystem.dll`, calls `CreateAISystem`, and later
   `AI->vtable+4` (`CAISystem::Init`). `CAISystem::Reset` is called from `Init`.
2. **A per-frame `Update` entry, twice gated.** `CSystem::Update` calls `AI->vtable+0x14` once per
   frame, but only if `sys_AI != 0`, not paused/a cutscene, a loading object returns 0, and inside
   `CAISystem::Update` only if `ai_SystemUpdate != 0` and the init/enable flags are set. x2game sets
   `ai_SystemUpdate = 0` in its loading path.
3. **The physics frame-time cap.** `AI->vtable+0x30` (`GetMaxStep`) returns **0.1 s** (constant
   `0x311b6484`) because the optional delegate `CAISystem+0x13c` is never assigned. This is the
   only AI link that changes on-foot/riding feel (E1 §1.5).
4. **Anchors.** Entity Lua (`AIAnchor`, `AIAlertness`, `TagPoint`, `AIReinforcementSpot`) calls
   `AI.RegisterWithAI`; x2game forwards to the entity AI proxy. The proxy-side engine path is
   inferred to allocate a base `CAIObject` for types `1001..1006` (U3), but the exact proxy call is
   not proven; the AI event-listener and graph placement parts are proven.
5. **AI event areas.** Some entity proxies register themselves as AI event listeners with a
   position, radius and event-type mask; `CAISystem::NotifyAIEventListeners` fans AI events out to
   them.
6. **AI points and navigation modifiers.** The x2game prefab classes create `CGraph` nodes
   (AIPoint) and named navigation shapes (AINavModifier) at world placement time, through
   `CAISystem+0x1a4` (the graph) and `CAISystem+0xbc/+0xc0`.
7. **Logging.** x2game's own `CAIHandler`/`CScriptBind_AI` uses the AI logging slots
   (`+0x228/+0x22c/+0x238`) for `<CAIHandler>`/`<CScriptBind_AI>` diagnostics.

It does **not** run, on the client:

* **puppets / AI characters / goal pipes** — `BasicAI`'s client `OnInit` only calls `OnReset`; the
  `RegisterAI` call exists only in `OnPropertyChange` and in the server `OnInit` branch (Lua
  disassembly, §8.1). `ProcessAiPhase()` is explicitly unsupported (`ScriptBindUnit::ProcessAiPhase()
  we don't support this for client!`, x2game string/`0x...`).
* **perception of the local player** — no AI actor/puppet exists, so `VisCheckBroadPhase` and the
  visibility/SOM code have an empty agent set.
* **pathfinding for click-to-move / auto-walk** — the local player is moved by the physics-based
  controller (`port-movement`), not by `CAISystem`. `click_to_move` is a UI option; the only
  client-side path-request entry (`RequestPathToPipeUser`, vtable `+0x2e8`) is gated by
  `IsServer` (`+0x2f8`), which is **false** on the client (field `CAISystem+0x344 = 0`).
* **smart objects** — `CAISystem::InitSmartObjects` is vtable `+0x8` and has **no caller** in any of
  the four engine modules or x2game; `/Libs/SmartObjects.xml` is loaded only from it. No x2game
  string mentions smart objects.
* **navigation-data loading** — `CAISystem::LoadNavigationData` is vtable `+0x134` and also has no
  caller. The graph the client fills is therefore the empty `CGraph` populated by AIPoint prefabs,
  not a navmesh loaded from disk.

Runtime corroboration: a real client session (`D:/AA1.2/ailog.log`, 6 lines) contains only
`<RESETTING AI SYSTEM>` and the `aiTick` header; `aisignals.csv` contains only its header. There are
no AI ticks, events, names or path records in the recorded session.

The survey's ~1,200 f / 365 KB "client anchor/AI-point subset" is an **upper bound by address band**,
not the live set. The code actually reached at runtime is **82 functions / 54.6 KB** including the
x2game hand-over; the live CryAISystem part alone is **56 f / 39.0 KB** (and the phase-1 part is
**5 f / 8.1 KB**). See §14.

---

## 2. The call chain

### 2.1 Startup

```
CSystem::Init                     crysystem 0x36556770
├─ FUN_3654a830(...,"InitAISystem",...)                     profiler scope
├─ FUN_36553010(this)            InitAISystem, 208 B
│  ├─ FUN_36545120(this,"CryAISystem.dll",0)               load library -> CSystem+0x8c8
│  ├─ GetProcAddress("CreateAISystem")
│  ├─ CreateAISystem(this)       cryaisystem 0x31096fc0, 266 B
│  │  ├─ gEnv = ISystem->GetGlobalEnvironment()             -> DAT_311fb438
│  │  ├─ ISystem->GetCryPak(), pak->...                    (profiler/CRC setup)
│  │  ├─ RegOpenKeyExA("Software\\Crytek\\AIConfig")       DevMode -> DAT_311fb43e
│  │  ├─ FUN_31037e30(this)                                cvar/console bootstrap
│  │  └─ CAISystem ctor (0x1380 bytes) -> DAT_311fb410     returns CAISystem* in EAX
│  └─ CSystem+0x68 = returned CAISystem*
└─ later in the same Init:
   AI->vtable+4()                 CAISystem::Init 0x31050df0, 287 B
      ├─ vtable+0x84(0)           CAISystem::Reset 0x31089300, 3256 B
      ├─ AI+0x381 = timer->Get... ; AI+0x3f8 = dt
      └─ AI+0xe00 = 1             "initialized" byte
```

`CAISystem` ctor (`0x3108de00`, 3148 B) sets `+0xe00 = 0`, `+0x137c = 1` (enabled),
`+0x344 = 0` (not server), builds the empty nav-graph wrapper at `+0x340` and the object/listener
containers. `GetMaxStep`'s delegate `+0x13c` is never written, so it stays null.

### 2.2 One frame

```
CSystem::Update                   crysystem 0x365459b0 (mapped in E1)
├─ ... input / entity pre-physics / render begin ...
├─ physics block (only one TimeStep per frame)
│  ├─ maxStep = AI ? AI->vtable+0x30() : 0.25f       <-- 0.1 s, E1 §1.5
│  ├─ step = min(maxStep, frameTime); world->vtable+0x84(step,0x19f)
│  ├─ world->vtable+0x134()
│  └─ if (flags allow && sys_AI != 0 && this+0x18b0->vtable+8() == 0)
│        timer->vtable+0x30(); timer->vtable+0x24(&t,0);
│        AI->vtable+0x14()                            <-- CAISystem::Update
│           ├─ if AI+0x380 (initialized) == 0 or AI+0x137c (enabled) == 0: return
│           ├─ if ai_SystemUpdate == 0 (cvar at AI+0xc8): return
│           ├─ UpdatePathFinder        0x3106dbf0, 5626 B  (queue empty)
│           ├─ VisCheckBroadPhase      0x310911b0, 1638 B  (no agents)
│           ├─ UpdateExpensiveAccessoryQuota 0x31092490, 1969 B
│           ├─ UpdateAuxSignalsMap     0x31093100,  402 B
│           └─ time-sliced AI-object / graph / area passes
└─ post-physics: entity system, movie, 3D engine terrain, sound, music, network
```

The heavy branches of `Update` (pathfinder beautification, visibility rays, goal pipes, group
tactics) only do work when participants exist. The client has none (no puppets, no path requests),
so a Rust port can keep the entry, the two gates and the anchor/graph maintenance and stub the
empty workers.

### 2.3 x2game hand-over to the AI system

```
Lua entity script (AIAnchor.OnInit / OnPropertyChange, AIAlertness, TagPoint)
└─ AI.RegisterWithAI(id, anchorType)
   x2game CScriptBind_AI::RegisterWithAI        0x390dd300, 1739 B
   ├─ entity->GetAI() (proxy)                   entity vtable+0xe4
   ├─ proxy->+0x78(6,0)                         clear previous anchor type
   ├─ read Properties/aicharacter_character     script tables
   ├─ proxy->+0xa4(entity)                      register entity with AI proxy
   └─ engine (through the proxy):
      ├─ CAISystem::RegisterAIObject (?)        vtable+0x18c, 0x31051260, 885 B
      ├─ FUN_31044870                           143 B, ensure AI object for entity
      └─ CAISystem::CreateAIObject               vtable+0x44, 0x31062120, 835 B
         -> base CAIObject (0xc0 B) for anchor types 1001..1006
            inserted in the AI map (CAISystem+0x1c)

AIPoint prefab (world placement)
└─ AIPointPrefabObject::CreateNode              x2game 0x390ff290, 190 B
   ├─ graph = AI->vtable+0x1a4()                 CAISystem::GetNavigationGraph
   ├─ (*graph+0x1c)(oldNode,1)                   remove old node
   ├─ node = (*graph)(4,&pos,0)                  CGraph::CreateNode
   ├─ obj  = (*graph+4)(node)                    CGraph::GetNodePtr
   └─ (*graph+0x40)(node,type)                   node type 1..5
   per-frame update 0x39106a90, 874 B
   ├─ (*graph+0x7c)(&pos)                        point-in-graph test
   ├─ AI->vtable+0x1a8(&pos,&navType,4)          CAISystem::CheckNavigationType
   ├─ (*graph+0x38)(node,navType)                set nav type
   ├─ (*graph+0x1c)(node,0); (*graph+8)(node,&pos)
   └─ connect children through graph +0x14

AINavModifier prefab
└─ AINavModifierPrefabObject loader              x2game 0x3910d230, 812 B
   ├─ AI->vtable+0xc0(name)                      remove old shape
   └─ AI->vtable+0xbc(&descriptor)               create shape (0x31082980, 3502 B)
   destructor 0x3910c0e0 -> AI->vtable+0xc0(name) for both name fields

Entity proxy with an AI event area
├─ AI->vtable+0xb4(listener,pos,radius,6)        cryentitysystem 0x320a37d0 / 0x320a9cf0
└─ AI->vtable+0xb8(listener)                     cryentitysystem 0x320a9860
```

The visible result of all of this is **not** player movement: it is world AI-placement data plus a
few script/UI events. The only P1-visible result is the 0.1 s physics cap.

### 2.4 Where CryAction fits

CryAction contains a second copy of the same `<CAIHandler>`/AI script layer. Its 485 references to
`gEnv+0x40` are all inside that layer (methods `0x3050f4b0`…`0x305af380` and the AI bind helpers)
and use slots such as `+0x100,+0x15c,+0x1d4,+0x1d8,+0x21c,+0x24c,+0x254,+0x25c,+0x260,+0x264,
+0x27c,+0x288,+0x28c,+0x290,+0x294,+0x298,+0x2a4…+0x2b0,+0x2c4,+0x2d4,+0x2e4`. AA's client
uses x2game as its game module; x2game ships its own `CScriptBind_AI`/`CAIHandler` (same table
names, same `<CScriptBind_AI>`/`<CAIHandler>` tags). No x2game code calls into CryAction's AI
handler, and the AA Lua scripts resolve `AI.*` through the x2game bind. CryAction's AI surface is
therefore **dormant on the client** [I, strong: duplicate implementation, no import, no log lines].
If a loader ever activates it, the slots above are the extra interface surface; it is not part of
the client port.

---

## 3. Interface map (CAISystem vtable `0x311c4248`)

Object size `0x1380`, ctor `0x3108de00`, global pointer `DAT_311fb410`, gEnv `DAT_311fb438`.
"Live" means the client reaches it (directly or from entity Lua); "dormant" means the code exists
but no client caller was found.

| slot | function | size | what it does | client users | status |
|---|---|---:|---|---|---|
| `+0x00` | `0x3108ea50` | 33 | deleting dtor | CrySystem unload | live |
| `+0x04` | `0x31050df0` `CAISystem::Init` | 287 | reset, timer dt, `+0xe00=1` | CSystem::Init | live |
| `+0x08` | `0x31073d40` `InitSmartObjects` | 244 | loads `/Libs/SmartObjects.xml` + action graphs | none | **dormant** |
| `+0x14` | `0x31093d10` `CAISystem::Update` | 4579 | per-frame AI bookkeeping | CSystem::Update | live (gated) |
| `+0x18` | `0x3108dc00` | 16 | set `+0x137c` enabled flag | none (ctor sets 1) | dormant setter |
| `+0x1c` | `0x3108f870` | 19 | `return *(cvar at +0x1d8)->GetIValue() != 0`; cvar is `ai_UpdateAllAlways` | x2game `0x39023f00` | live read |
| `+0x30` | `0x31042b00` `GetMaxStep` | 29 | `+0x13c ? +0x13c->vtable+0xc() : 0.1f` | CSystem physics | **live, P1** |
| `+0x44` | `0x31062120` `CreateAIObject` | 835 | create base/actor/puppet object, register in map | anchor registration | live |
| `+0x48` | `0x31042b70`→`FUN_3113b710` | 15+361 | load named dynamic goal pipe (`DynamicGoalPipes`, `DynPipe_%d`, `.lua`) | x2game pet manual `0x3933e630` | live content load |
| `+0x5c` | `0x31056da0` | 438 | find AI object within a sphere (pos/type/radius/height/mask) | x2game anchor-skill controller `0x390ad7e0` | live |
| `+0x74`/`+0x78` | `0x31042c00`/`0x31042c70` | 36 / 14 | call CAISystem's own vtable `+0x318`/`+0x31c` (AI map lookup); `+0x74` returns `obj+0x880` | cryentitysystem `0x3200fdf0` | live |
| `+0x7c` | `0x31055b30` | 114 | remove empty AI group (logs if non-empty) | cryentitysystem `0x3200fdf0` | live |
| `+0x84` | `0x31089300` `CAISystem::Reset` | 3256 | clear maps/lists/timers, call subsystem resets | `CAISystem::Init` | live at init |
| `+0xb4` | `0x31060650` | 170 | add/update record `{listener,pos,radius,typeMask}` in `+0x1358` vector | cryentitysystem entity proxies | live |
| `+0xb8` | `0x31053670` | 161 | remove listener record (swap-with-last) | cryentitysystem entity proxies | live |
| `+0xbc` | `0x31082980` `CreateNavigationShape` | 3502 | create named area/shape from a descriptor | x2game AINavModifier `0x3910c200` | live at placement |
| `+0xc0` | `0x3107c2b0` `RemoveNavigationShape` | 2929 | remove named shape | x2game AINavModifier dtor/loader | live at placement |
| `+0x12c` | `0x3106f290` `CreateGoalPipe` | 5410 | create a goal pipe | x2game CAIHandler, when a puppet exists | dormant (no puppets) |
| `+0x134` | `0x31088290` `LoadNavigationData` | 1157 | load nav graph file | none | **dormant** |
| `+0x18c` | `0x31051260` | 885 | register entity's AI actor/formation (`CreateAIObject(0x5f)`) | entity AI proxy | live? [I] |
| `+0x1a4` | `0x31042b50` | 9 | return `CGraph*` (`**(CAISystem+0x340)`) | x2game AIPoint `0x390ff290`,`0x39106a90`; anchor skill | live |
| `+0x1a8` | `0x31051b10` `CheckNavigationType` | 1580 | classify a position's navigation type | x2game AIPoint update `0x39106a90` | live |
| `+0x1d0` | `0x310388e0` | 1503 | memory statistics for `IMemoryStatistics` | crysystem memory stats | live at unload |
| `+0x228` | `0x31042fa0` | 90 | AI error log (`vsprintf` into 0x1020 buffer, `AI: %s`) | x2game/CryAction CAIHandler | live (diagnostics) |
| `+0x22c` | `0x31043000` | 90 | AI warning log | x2game `RegisterWithAI`, entity AI | live |
| `+0x238` | `0x31043120` | 90 | AI message log | CryAction/X2 AI bind | live |
| `+0x260` | `0x31063850` | 136 | set AI object status by entity/name (`"Frozen"`) | x2game `0x39126e40`, `0x39129310`, `0x39132710` | live? [I] |
| `+0x290` | `0x31043470` | 19 | increment AI stat/ref counter, return value | CryAction (dormant) | dormant |
| `+0x294` | `0x3104aa60` | 73 | allocate a 0x30-byte AI event object (vtable `0x311b9d18`) | x2game `0x39023ce0`, `0x390276e0`, `0x3933cb60`, `0x3933ebe0` | live with aggro UI |
| `+0x298` | `0x31043490` | 23 | release object (`vtable+0xc(1)`) | x2game event path | live |
| `+0x2e4` | `0x31043410` | 5 | jump to `0x310960f0` (query helper) | CryAction | dormant |
| `+0x2e8` | `0x31069090` `RequestPathToPipeUser` | 713 | request a path for a pipe user | x2game skill controller `0x39095e00` **only if IsServer** | dormant on client |
| `+0x2f0` | `0x3104e410` | 199 | `GetAIMovementSpeeds(movementParams, scale, out[7])` | x2game `ParseTables` `0x390f5b20` | live table parse |
| `+0x2f4` | `0x3108dd50` | 6 | `return &DAT_311e3090` (default movement speeds, 7 floats) | x2game `ParseTables` | live table parse |
| `+0x2f8` | `0x31043420` `IsServer` | 13 | `return CAISystem+0x344 == 1` | x2game `0x39095e00` | live read (false) |
| `+0x318`/`+0x31c` | internal | — | AI map lookup by entity/group id (called by the `+0x74`/`+0x78` wrappers); `+0x31c` is `0x31055af0`, 55 B | x2game/entity system | live |

Call-site audit (all decompiled modules scanned for `gEnv+0x40` and `CSystem+0x68`):

| caller module | AI vtable slots called | notes |
|---|---|---|
| crysystem | `+0x04` (Init), `+0x14` (Update), `+0x30` (GetMaxStep), `+0x1d0` (memory stats) | the only engine module that drives the AI lifecycle/update |
| x2game | `+0x1c`, `+0x48`, `+0x5c`, `+0xbc`, `+0xc0`, `+0x12c`, `+0x1a4`, `+0x1a8`, `+0x228`, `+0x22c`, `+0x260`, `+0x294`, `+0x298`, `+0x2e8`, `+0x2f0`, `+0x2f4`, `+0x2f8` | all other x2game `+0x40` traffic is log/text |
| cryentitysystem | `+0x78`, `+0x7c`, `+0xb4`, `+0xb8` | entity AI-group/listener glue |
| cryaction | many (`+0x100`, `+0x15c`, `+0x1d4`, `+0x1d8`, `+0x21c`, `+0x24c`…`+0x2e4`, logging) | **only inside CAIHandler / its AI bind**, dormant in AA (§2.4) |
| cryaisystem itself | `+0x44` + internal vtable `+0x318/+0x31c`, graph/shape APIs | object creation/lookup internals |

No other slot is called from the client's live path. That is the complete interface surface the
port has to match.

Note: `+0x44`'s decompiled switch creates `CAIActor` (type 5/0x50, 4000 B), `CAIPlayer`-like
objects (0x96/0x97 0x880 B, 100 0xdd0 B), `0x5f` 0x928 B, and the default `CAIObject` 0xc0 B.
Anchor types `1001..1006` take the default case. `CreateAIObject` checks `+0x137c` (enabled) and
`+0xe00` (initialized) before allocating.

---

## 4. Lifecycle and update functions

### 4.1 Start-up / shutdown

| function | module / address / size | callers → callees | fields, globals, cvars | pure/stateful | oracle |
|---|---|---|---|---|---|
| `InitAISystem` | crysystem `0x36553010`, 208 B | `CSystem::Init` `0x36556770` → `FUN_3654a830` (profiler), `FUN_36545120("CryAISystem.dll")`, `GetProcAddress("CreateAISystem")` | writes `CSystem+0x8c8` (HMODULE), `CSystem+0x68` (AI*); logs `Did not find AI System library` / `Cannot instantiate AISystem class` | stateful | call once with `CSystem*`; read back `*(void**)(sys+0x68)` and compare with `AI` |
| `CreateAISystem` | cryaisystem `0x31096fc0`, 266 B | crysystem InitAISystem → `ISystem::GetGlobalEnvironment`, `ISystem+0x288` (CryPak), `FUN_31037e30`, `CAISystem ctor` | `DAT_311fb438`=gEnv, `DAT_311fb410`=AI, `DAT_311fb43e`=AIConfig DevMode; allocates `0x1380` | stateful | `AI = call(rebase(0x31096fc0), ISystem*)`; returns the same pointer as `*(gEnv+0x40)` |
| `CAISystem ctor` | `0x3108de00`, 3148 B | CreateAISystem → many container ctors, `FUN_310472c0` (cvar registration), `FUN_3100fda0` | clears `+0xe00`; sets `+0x137c=1`, `+0x344=0`; `+0x340` nav-graph wrapper; `+0x1c` object map; `+0x1358/+0x135c` listener vector; `+0xc8` cvar pointer (later); `+0x13c=0` (max-step delegate) | stateful | read fields after boot: `u8[+0x137c]=1`, `u8[+0xe00]=1` after Init, `u32[+0x344]=0`, `u32[+0x13c]=0` |
| `CAISystem::Init` | `0x31050df0`, 287 B | `CSystem::Init` via vtable `+0x4` → `vtable+0x84(0)` (Reset), timer/cvar helpers | sets `+0xe00=1`; `+0x381`; `+0x3f8=0`; logs `[AISYSTEM] Initialization started/finished` | stateful | call `AI->vtable+4` once (or read `+0xe00`) |
| `CAISystem::Reset` | `0x31089300`, 3256 B | `CAISystem::Init` (`+0x84(0)`), later internal resets | clears maps/lists/timers, resets graph/areas; checks `+0x137c` and `+0xe00`; reads `+0x4ae/+0x4af` lists | stateful | call `AI->vtable+0x84(0)` and diff the map/list heads |
| `CAISystem dtor` | `0x3108ea50`, 33 B | CrySystem module unload | vtable `0x311c4248` | stateful | not needed for a Rust port |

**Cvar registration** (`FUN_310472c0`, `0x310472c0`, 9195 B, called from the ctor) registers the
whole AI cvar set. The ones on the client's live path are:

| cvar | default | where read | effect |
|---|---:|---|---|
| `ai_SystemUpdate` | 1 | `CAISystem+0xc8` (ICVar pointer), read at the head of `Update` | 0 disables the whole AI update (x2game sets 0 while loading) |
| `ai_UpdateAllAlways` | 0 | `CAISystem+0x1d8`, read by vtable `+0x1c` | queried by x2game's alertness update |
| `ai_IgnorePlayer` | 0 | ICVar in x2game `0x3915b440` | set from a loading/flag path; no AI actor consumes it on the client |
| `ai_DefaultWalkability` | 1 | `CAISystem+0x330` with change callback `0x31042ba0` | selects X2/Cry walkability defaults; only matters when a path/walkability query exists |
| `ai_doNotLoadNavigationData` | 0 | `CAISystem+...` | would gate nav-data loading; `LoadNavigationData` itself has no caller |
| `ai_LimitPhysicsRequestPerFrame` | 2000 | graph/physics request path | not reached on the client |
| `ai_UpdateInterval` | 0.1 s | AI object update scheduler | time-slice quantum |
| `sys_AI` | 1 | crysystem `DAT_3663b420` | outer gate in `CSystem::Update` |

### 4.2 `CAISystem::Update` (`0x31093d10`, 4579 B)

Caller: `CSystem::Update` physics block (once per frame, both sync and async branches). Entry checks:

1. `CAISystem+0x380` byte (`+0xe00`) must be non-zero (set by `Init`);
2. `CAISystem+0x4df` byte (`+0x137c`) must be non-zero (set by ctor, never cleared on client);
3. `ai_SystemUpdate` value (ICVar at `CAISystem+0xc8`) must be non-zero;
4. a frame-time/timer accumulator at `CAISystem+0x3f6..` and `+0x3f8`.

Then it runs, in order [R for the call order, I for the role of unnamed helpers]:

| step | function | size | notes |
|---|---|---:|---|
| event/area list compaction | inline at `0x31093d10+…` | — | vector at `+0x4ae/+0x4af`, 0x20-byte records |
| nav/area managers | `FUN_31051020`, `FUN_31093550`, `FUN_31093580`, `FUN_31072240`, `FUN_310eec70`, `FUN_310eee40`, `FUN_310593f0` | — | only when `+0x3e1 != 0` and object map non-empty |
| object/list timer sweep | inline | — | list at `+0xda/+0xdb` (8-byte entries with float timeout) |
| **`UpdatePathFinder`** | `0x3106dbf0`, 5626 B | profiler `CAISystem::UpdatePathFinder`; path request queues; no requests on the client |
| **`VisCheckBroadPhase`** | `0x310911b0`, 1638 B | profiler `CAISystem::VisCheckBroadPhase`; no AI agents on the client |
| **`UpdateExpensiveAccessoryQuota`** | `0x31092490`, 1969 B | profiler name; accessory/visibility quota; empty on client |
| **`UpdateAuxSignalsMap`** | `0x31093100`, 402 B | profiler name; auxiliary AI signal map |
| AI-object time slice | inline | — | list at `+0x34e/+0x34f`; each entry is scheduled through `FUN_31091820` and updated through object vtable calls. With only anchor `CAIObject`s (no `CAIPuppet`) this is position/flag bookkeeping, not behaviour |
| graph reload/generation | `FUN_31016c70` etc. | — | only when a graph change is pending; no client caller changes the graph except AIPoint prefabs |
| profiler/status | `FUN_31001290`, `DAT_311fb400` bits | — | once-per-name profiler registration |

The four bold entries are reached **every frame** but are effectively idle on the client. A Rust port
should keep the gate + entry names and stub the queues; there is no need for the pathfinder or
perception core in phase 1.

The AI object update slice is the only branch that can touch the registered anchors. It does not
create a visible change on its own.

### 4.3 Frame-time cap (`GetMaxStep`)

| function | module / address / size | callers | fields | pure? | oracle |
|---|---|---|---|---:|---|
| `CAISystem::GetMaxStep` | cryaisystem `0x31042b00`, 29 B (gap function, not in `funcs.csv`) | crysystem `CSystem::Update` physics block | reads `CAISystem+0x13c`; if null returns constant `0x311b6484 = 0.1f`; otherwise `delegate->vtable+0xc()` | **pure** | call `AI->vtable+0x30()` in a stub; expect `0.100000f`; read `*(float*)rebase(0x311b6484)` |

`CAISystem+0x13c` is never written by the ctor or by any engine/x2game code in the decompiles
(`[R]`), so the constant path is the only one in the shipping client. This is the **only** AI value
that influences on-foot/riding physics (`dt = min(frameTime, 0.1)`), and E1 already uses it.

### 4.4 Query/gate helpers

| function | address / size | semantics | client use |
|---|---|---:|---|
| `IsUpdateAllAlways` | `0x3108f870`, 19 B | `return *(cvar CAISystem+0x1d8)->GetIValue() != 0` | x2game `0x39023f00` (alertness update range) |
| `IsServer` | `0x31043420`, 13 B | `return *(u32*)(CAISystem+0x344) == 1`; ctor writes 0 | x2game `0x39095e00`; false on client, so the AI skill/path path is skipped |
| `SetEnabled` | `0x3108dc00`, 16 B | `*(u8*)(CAISystem+0x137c) = arg` | no caller; ctor already sets 1 |
| `GetMemoryStatistics` | `0x310388e0`, 1503 B | fills `IMemoryStatistics` | crysystem memory report at unload |
| `InitSmartObjects` | `0x31073d40`, 244 B | loads `/Libs/SmartObjects.xml`, `Libs/ActionGraphs` | **no caller** (vtable `+0x8`) |
| `LoadNavigationData` | `0x31088290`, 1157 B | level nav-graph load | **no caller** (vtable `+0x134`) |

---

## 5. AI objects, anchors and event listeners

### 5.1 Object creation and lookup

| function | address / size | callers → callees | fields / offsets | pure/stateful | oracle |
|---|---|---|---:|---|---|
| `CreateAIObject` | `0x31062120`, 835 B (vtable `+0x44`) | `FUN_31044870`, `FUN_310515e0`, `FUN_31044870` from registration; entity AI proxy | checks `u8[+0x137c]` (enabled) and `u8[+0xe00]` (init); `switch(type)`; allocates `0xc0` (base), `0x880`, `0x928`, `0xdd0`, `4000`; sets `CAISystem+0x378` object-count estimate; inserts into map via `FUN_3105db20`/`FUN_31046bb0` | stateful | call with type `1001`: returns a `0xc0`-byte `CAIObject`; read `u16[obj+0x52]==1001` |
| `CAIObject ctor` | `0x3103a1e0`, 339 B | `CreateAIObject` default case / `FUN_3103a1e0` | vtable `0x311b8d90`; zeroes `+0x0..+0x70`; sets `+0x52=0x100`, flags `+0x44=0` | stateful | direct call on a `0xc0` buffer; check vtable |
| `SetType` | `0x31039240`, 15 B (CAIObject vtable `+0x64`) | `CreateAIObject` | writes `u16[obj+0x52]` | stateful | call `obj->vtable+0x64(1001)` |
| `SetEntity` | `0x31039260`, 16 B (CAIObject vtable `+0x118`) | `CreateAIObject` | writes `void*[obj+0xa4]` | stateful | call with a dummy entity; read back |
| `GetPos` | `0x31008f20`, 13 B (CAIObject vtable `+0x3c`) | `GetAIObjectInSphere` caller, anchor skill | sets `u8[obj+0x44]=1`; returns `obj+0x5c` (Vec3) | mostly pure | call on an object and read 3 floats |
| `GetEntity` | `0x31008ef0`, 7 B | engine/script | returns `void*[obj+0xa4]` | pure | read |
| map lookup wrapper | `0x31042c00`, 36 B (vtable `+0x74`) | entity system, x2game | calls internal `+0x318(id)`; returns `obj+0x880` or null | stateful read | call with an entity id |
| map lookup wrapper | `0x31042c70`, 14 B (vtable `+0x78`) | entity system `0x3200fdf0` | tail-calls internal `+0x31c(id)` | stateful read | call with a group/entity id |
| internal map lookup | `0x31055af0`, 55 B (vtable `+0x31c`) | wrappers | searches the id→object map; returns stored value or null | stateful read | find an anchor's id |
| `RemoveAIGroup` | `0x31055b30`, 114 B (vtable `+0x7c`) | entity system `0x3200fdf0` | removes an empty group, logs `Non-empty AI group(%d) can't be removed!` | stateful | remove a dummy group |
| `GetAIObjectInSphere` | `0x31056da0`, 438 B (vtable `+0x5c`) | x2game anchor-skill controller `0x390ad7e0` | iterates `CAISystem+0x1c`; filters type/radius/height; returns `IAIObject*` | stateful read | create two anchors at known positions, query the midpoint |
| `RegisterAIObject` (name inferred) | `0x31051260`, 885 B (vtable `+0x18c`) | entity AI proxy [I] | entity → id, `MoveAICommandSetFrom`, formation group, creates `0x5f` object when missing | stateful | register a dummy entity and inspect the object map |
| ensure-object helper | `0x31044870`, 143 B | `0x31051260` | if entity id has no AI object and entity flag set: `CreateAIObject(0x5f)` + `FUN_31017530` | stateful | call with an entity |
| `CreateAIObject` variant | `0x310515e0`, 56 B | internal | looks up object, creates `0x5f` if entity has an AI proxy | stateful | — |
| `SetAIObjectStatus` | `0x31063850`, 136 B (vtable `+0x260`) | x2game `0x39126e40`, `0x39129310`, `0x39132710` | if enabled and `+0xff4 != 0`: `FUN_3116d390(entity/status)` + `FUN_3116aef0(name)` + `FUN_310605d0` | stateful | call with a known anchor id and `"Frozen"`; read the status store |

The `0x5f` object class (`0x928` bytes, vtable `0x311ca790`) is the AI actor used for entities that
need an AI presence. Anchor types `1001..1006` do **not** become `0x5f`; they are base `CAIObject`s.
No `CAIPuppet`/goal-pipe object is created by the client's entity scripts.

### 5.2 AI event listeners (the anchor/area service)

Entity proxies that want AI proximity events register a 0x18-byte record:

```
struct SAIEventListenerRecord {      // CAISystem+0x1358, 0x18 bytes, vector end +0x135c
    void*  listener;                 // +0x00, object whose vtable[0] is the callback
    Vec3   pos;                      // +0x04
    float  radius;                   // +0x10
    uint32 typeMask;                 // +0x14
};
```

| function | address / size | callers → callees | fields | oracle |
|---|---|---|---:|---|
| `AddAIEventListener` | `0x31060650`, 170 B (vtable `+0xb4`) | cryentitysystem `0x320a37d0`, `0x320a9cf0` | update or append record at `+0x1358`; grows through `FUN_3105d7a0` | call with a dummy listener + pos/radius/mask, then read `[AI+0x1358]` |
| `RemoveAIEventListener` | `0x31053670`, 161 B (vtable `+0xb8`) | cryentitysystem `0x320a9860`, `0x320a37d0(0)` | find by listener pointer, swap with last record, shrink `+0x135c` | add then remove, verify count |
| `NotifyAIEventListeners` | `0x31045b90`, 370 B | cryaisystem internal (`0x310…`, events 1 and 4) | for each record: `(mask & type) != 0` and `dist² < (radius+extra)²` → `listener->vtable[0](type,pos,radius,…)` | register a listener, call `NotifyAIEventListeners(4,&pos,radius,0,0)` and count callbacks |

The event type values seen on the client path are 1 (sound/ai signal) and 4 (visibility/signal);
the entity-system callers pass mask `6` (= 2|4). This is the mechanism by which AIAnchor/AIAlertness
areas learn about AI events. It is data-only without puppets, but it is live and must be kept in a
faithful port of the anchor feature.

### 5.3 AI event objects (`+0x294` / `+0x298`)

`CAISystem+0x294` (`0x3104aa60`, 73 B) allocates a `0x30`-byte object with vtable `0x311b9d18`,
zeroes its Vec3 + counters and returns it. `+0x298` (`0x31043490`, 23 B) releases any object through
`vtable+0xc(1)`. The x2game aggro/alert UI path creates such an object, fills `+0x1c` with an entity
id and passes it to a Lua `OnAggroTargetChanged`/`OnAlertTargetChanged` callback; it then releases
it. This is **not** an AI goal/event queue — it is a small script-bind payload.

### 5.4 Logging slots

`CAISystem+0x228` (`0x31042fa0`), `+0x22c` (`0x31043000`) and `+0x238` (`0x31043120`) are 90-byte
wrappers: `strncpy(tag)` + `vsprintf_s` into a global `0x1020` buffer (`DAT_311efe20`) + call the AI
log sink. They are the reason x2game's `<CAIHandler>`/`<CScriptBind_AI>` strings appear in
`ailog.log`; they do not affect gameplay.

---

## 6. Navigation graph and navigation shapes

### 6.1 The graph the client really has

`CAISystem+0x340` is the `AINavGraph` wrapper created in the ctor (`0x311225d0`). Its first pointer
is the `CGraph` object (vtable `0x311ca468`, ctor `0x310f6190`, 846 B). `CAISystem::GetNavigationGraph`
(vtable `+0x1a4`, `0x31042b50`, 9 B) returns `**(CAISystem+0x340)` = that `CGraph*`.

`CGraph` stores 1-based node ids in 128-node blocks; `GetNode(id)` (`0x310ed150`, 68 B) computes
`block[(id-1)>>7]`, then `data + ((id-1)&0x7f) * stride`. Node fields used by the client:
`+0x2c` nav type, `+0x30` node type, `+0x18..+0x20` the position written by `SetNodePos`,
`+0x34..+0x3c` position B, `+0x40..+0x48` position A, `+0x14` link/obstacle field. Invalid id `0` returns the sentinel block,
not null.

| method | address / size | slot | semantics | client use |
|---|---|---:|---|---|
| ctor | `0x310f6190`, 846 B | — | creates empty graph, node allocator, link maps | ctor via `AINavGraph` |
| `CreateNode` | `0x310f3d90`, 135 B | `+0x00` | `(type,pos,flags) -> id`; allocates via `FUN_310f70d0` | AIPoint `0x390ff290` |
| `GetNodePtr` | `0x310ed150`, 68 B | `+0x04` | id → node pointer | AIPoint |
| `SetNodePos` | `0x310f3e20`, 207 B | `+0x08` | writes node `+0x18..+0x20`, invalidates grid | AIPoint update |
| `Connect` | `0x310f09c0`, 1278 B | `+0x14` | link two nodes with radius/direction | AIPoint children |
| `RemoveNode` | `0x310f4480`, 1038 B | `+0x1c` | unlink + free node | AIPoint update |
| node field get/set | `0x310e6730`/`0x310e6740`/`0x310e6750`/`0x310e6760` | `+0x34..+0x40` | read/write `+0x2c`, `+0x30` | AIPoint |
| pos get/set | `0x310e6770`/`0x310e6790`/`0x310e67b0`/`0x310e67d0` | `+0x5c..+0x68` | read/write `+0x40` and `+0x34` Vec3s | AIPoint |
| point-in-graph | `0x310e61f0`, 74 B | `+0x7c` | bounding-box test for a position | AIPoint update |
| node allocator | `0x310f70d0`, 725 B | — | 128-node blocks, zeroes stride data | CreateNode |

`CGraph` also has the full nav-mesh/path API (A*, link costs, grid, 3D volume regions). None of it
is called on the client: no nav graph file is loaded (vtable `+0x134` has no caller) and no path
request is issued (`RequestPathToPipeUser` is gated by `IsServer`). Those are the bulk of the
survey's 137.6 KB navigation core and are **not** needed for the client.

### 6.2 Navigation type and shapes

| function | address / size | slot | semantics | client use |
|---|---|---:|---|---|
| `CheckNavigationType` | `0x31051b10`, 1580 B | `+0x1a8` | `(pos, outType*, flags)`; profiler-registered `CAISystem::CheckNavigationType`; asks the graph/areas for the nav type at a position | AIPoint update `0x39106a90` |
| `CreateNavigationShape` | `0x31082980`, 3502 B | `+0xbc` | builds a named shape from a descriptor (position, points, type, radius/height); messages `CAISystem::CreateNavigationShape: … already exists` | AINavModifier loader `0x3910c200` |
| `RemoveNavigationShape` | `0x3107c2b0`, 2929 B | `+0xc0` | removes a named shape; parses the name buffer | AINavModifier dtor/loader |

The descriptor passed by x2game (`local_ac` in `0x3910c200`) has a name pointer at `+0x3c`, a point
array/count and type/height fields; the exact layout is in §7.3. The engine keeps shapes in named
lists (`Forbidden area`, `Forbidden boundary`, `NPC Roaming Area`, `Designer path`, `Occlusion
plane`, `Navigation modifier`; strings at `0x311b…`/`0x311c…`). Walkability queries
(`CheckWalkability*`) use them, but the client only creates them.

---

## 7. x2game and entity-system hand-over

### 7.1 `AI.RegisterWithAI` (Lua → engine)

| function | module / address / size | callers | AI interface used | notes |
|---|---|---|---:|---|
| `CScriptBind_AI::RegisterWithAI` | x2game `0x390dd300`, 1739 B | Lua `AI.RegisterWithAI` (wrappers `0x390dd9d0`, `0x390ddc20`) | none directly; drives the entity AI proxy, which calls `CAISystem+0x18c` / `+0x44` | logs `RegisterWithAI: no puppet for %s (Id %i).` when the entity has no AI proxy; clears the old anchor type through `proxy+0x78(6,0)`; reads the entity `Properties`/`AIAnchorTable`; sets position/rotation; returns 1 |
| Lua wrappers | x2game `0x390dd9d0` (38 B), `0x390ddc20` (77 B) | Lua VM | — | thin argument marshalling |

Anchor types come from `game/scriptsbin/ai/anchor.alb` (`AIAnchorTable = {AI_ANCHOR_1..6 =
1001..1006}`). They are carried as the AI type into the entity proxy; if the proxy forwards that
type to `CreateAIObject`, the `default` case allocates the `0xc0`-byte base object (U3). `AIAnchor.OnInit` and `OnPropertyChange` always call `Register`
(the `OnInit` path has no `Server` guard), so anchors are the one AI object kind that is genuinely
created on the client.

### 7.2 `AIPoint` prefab (CGraph nodes)

| function | module / address / size | callers | AI interface used | notes |
|---|---|---|---:|---|
| ctor | x2game `0x3910c050`, 128 B | prefab allocator | — | vtable `0x399ab378`; fields `+0xa0` node index, `+0xa4` node handle, `+0xc4` anchor type |
| `AIPointPrefabObject::CreateNode` | `0x390ff290`, 190 B | prefab load/update | `GetNavigationGraph` `+0x1a4` | remove old node `(*graph+0x1c)(old,1)`; `node=(*graph)(4,&pos,0)`; `obj=(*graph+4)(node)`; type 1..5 from `+0xc4` through `(*graph+0x40)`; log `AIPointPrefabObject::CreateNode() node index %d` |
| per-frame update | `0x39106a90`, 874 B | AIPoint vtable `+0x4c` (entity/prefab update) | `+0x1a4`, `+0x1a8` | compute world pos from entity matrix; `(*graph+0x7c)(&pos)` bounds test; `CheckNavigationType(&pos,&type,4)`; `(*graph+0x38)(node,type)`; remove/reinsert node (`+0x1c`, `+8`); update links/children with graph `+0x14/+0x28/+0x34/+0x60/+0x10` |
| prefab path type | x2game `0x391114f0`, 661 B | prefab loader | — | logs `invalid prefab path type(%d)` |

Visible result: AI-point graph nodes exist at the authored positions. The server uses the same
world data for NPC navigation; the client never queries them for the local player.

### 7.3 `AINavigationModifier` prefab (navigation shapes)

| function | module / address / size | callers | AI interface used | notes |
|---|---|---|---:|---|
| prefab loader | x2game `0x3910d230`, 812 B | world prefab load | — | reads `prefab_ai_nav_mod_%d` / `prefab_ai_forbidden_%d`; builds a descriptor |
| create shape | `0x3910c200`, 1222 B | loader | `+0xc0` (remove old by name), `+0xbc` (create) | descriptor has name at `+0x3c`, point count/array, type/height; logs `AINavModifierPrefabObject::Load() total %ld points loaded` |
| destroy | `0x3910c0e0`, 287 B | entity destruction | `+0xc0` twice (two name fields) | removes both shape variants; no server check |

### 7.4 Anchor skill controller (`FindObjectOfType`-style)

`0x390ad7e0` (604 B, x2game) is the anchor skill controller update. For each authored anchor in
its list `+0x5`/`+0x6` it computes a radius and calls `AI->vtable+0x5c(pos, radius, 0, …, 0, 0)`
(`GetAIObjectInSphere`), then reads the returned object's position through `obj->vtable+0x3c`
(`GetPos`). It then sets up a client skill effect/animation. This is the one live gameplay use of
AI objects outside the world graph, and it is why anchors must exist even on the client.

### 7.5 AI-character entities and `CAIHandler`

| function | module / address / size | notes |
|---|---|---:|
| entity/handler factory | x2game `0x39103c80`, 535 B | creates a client entity through `gEnv+0x44` (entity system; `+0x2c`/`+0x100`), allocates the `0x54`-byte AI-handler object via `0x39002d10`, sets its script table |
| AI-handler ctor | x2game `0x39002d10`, 83 B | vtable `0x3999c750`; zero fields |
| AI-handler dtor | `0x39003640`/`0x39003000` | releases script tables |
| character wrapper ctor | `0x3910d560`, 380 B | registers entity listeners for events `0xd`/`0x2c` (aggro/alert); vtable `0x399ab40c` |

`CAIHandler` loads `aicharacter_character`, `aibehavior_behaviour` and `AVAILABLE` tables and owns the
per-entity AI script object. On the client its `BasicAI.OnInit` only calls `OnReset`; it does not
call `RegisterAI`, so this factory creates entity/script state but **no puppet**.

### 7.6 Movement-speed tables and `ParseTables`

`CScriptBind_AI::ParseTables` (`0x390f5b20`, 3791 B) reads the Lua `AIMovementSpeeds` table
(`Relaxed/Combat/Stealth/Crouch/Prone/Swim/Fly` × `Slow/Walk/Run/Sprint`), fills the actor movement
struct at `+0x2b8..+0x2d0`, and uses:

* `AI->vtable+0x2f4` (`GetDefaultMovementSpeeds`, `0x3108dd50`) → `&DAT_311e3090` (7 floats);
* `AI->vtable+0x2f0` (`GetAIMovementSpeeds`, `0x3104e410`) → 7 floats from the actor's
  `SAIMovementParams` (`+0x30` mode, `+0x34` animation/movement set at `+0x118`), scaled by a
  passed factor.

The parsed values feed NPC movement/animation if an actor exists. With no client puppets they are
loaded but not applied to the local player. `pathType`, `usePathfinder` and `pathFindPrediction`
are parsed into the same struct (`0x390f5b20`, offsets `+0xe1`, `+0x120`); they have no client path
request to consume them.

### 7.7 AI event/aggro UI and cvar toggles

| function | address / size | AI interface used | notes |
|---|---|---:|---|
| `OnAggroTargetChanged` emitter | x2game `0x39023ce0`, 121 B | `+0x294` (create event payload) | reads entity id, `FUN_390b3750`, writes callback payload `+0x1c`, calls `0x39023c30(0,"OnAggroTargetChanged",payload)` |
| `OnNoAggroTarget` | `0x39023ce0` tail | — | calls `0x39023c30(0,"OnNoAggroTarget",0)` |
| script event dispatch | `0x39023c30`, 101 B | `+0x12c` (goal pipe) when a puppet exists, `+0x298` (release) otherwise | with no puppet it only releases the payload |
| alert target emitter | `0x390276e0` | `+0x294` | same pattern, `OnAlertTargetChanged` |
| `GO_TO_DEAD` goal | `0x39024540`, 132 B | `+0x12c` | creates `GO_TO_DEAD` goal pipe; needs a puppet, so dormant on client |
| `ai_IgnorePlayer` setter | `0x3915b440`, 98 B | console ICVar | sets cvar via `GetCVar("ai_ignoreplayer")` (case-insensitive hash) |
| loading `ai_SystemUpdate=0` | `0x3915b960`, 1014 B | console ICVar | when load flag `param_3 & 0x80000000`, `GetCVar("ai_systemupdate")->SetValue(0)`; no matching set-to-1 was found in x2game |

The console lookup is case-insensitive (`IConsole::GetCVar`, crysystem `0x36576170`, hashes with
`XlHashStringI`), so x2game's lowercase names hit `ai_SystemUpdate` and `ai_IgnorePlayer`.

### 7.8 Entity-system listener glue

| function | module / address / size | AI call | trigger |
|---|---|---|---:|
| add listener | cryentitysystem `0x320a37d0`, 116 B | `AI+0xb4(this,this+0x24,*(this+0x94),6)` | entity proxy enabled/created |
| add listener | `0x320a9cf0`, 113 B | same | entity proxy position set/created |
| remove listener | `0x320a9860`, 490 B | `AI+0xb8(this)` when `u8[this+0x172]` | entity proxy destruction/disable |
| remove AI group | `0x3200fdf0`, 162 B | `AI+0x78(id)`, `AI+0x7c(id)` | entity removal from an AI group |

These are live for every entity that has an AI event proxy (anchor/alertness/tag-point entities).
They are the engine-side half of the anchor service.

---

## 8. Evidence for what is **not** run

### 8.1 Client Lua never registers a puppet

`game/scriptsbin/entities/ai/shared/basicai.alb` was disassembled (Lua 5.1, stock opcodes) and
selected the client branch explicitly:

```
[283] GETGLOBAL "BasicAI"; GETTABLE "Server"; CLOSURE F4; SETTABLE "OnInit"
[292] GETGLOBAL "BasicAI"; GETTABLE "Client"; CLOSURE F5; SETTABLE "OnInit"   <- client override
F4 (Server.OnInit): Log("$8%s.BasicAI.Server:OnInit()"); self:OnReset()
F5 (Client.OnInit): self:OnReset()
F0 (OnPropertyChange): self:RegisterAI()       <- only here, not on spawn
```

`AI.RegisterWithAI` appears only in the anchor-style entity scripts:

| script | where |
|---|---|
| `ai/anchor.alb` | `AIAnchorTable = {AI_ANCHOR_1..6 = 1001..1006}`; `Register` → `AI.RegisterWithAI(id, type)`; `OnInit` → `Register` |
| `entities/ai/aianchor.alb` | `Register` → `AI.RegisterWithAI(self.id, AIAnchorTable[type])`; `OnInit`/`OnPropertyChange`/`OnLoad` → `Register` |
| `entities/ai/aialertness.alb` | `RegisterWithAI` |
| `entities/ai/tagpoint.alb` | `RegisterWithAI` |
| `entities/ai/aireinforcementspot.alb` | `RegisterWithAI` |
| `entities/actor/basicactor.alb`, `player.alb`, `entities/ai/civilian_x.alb`, `orc_x.alb`, `npc.alb`, `prefab_unit.alb` | **no** `RegisterAI`/`RegisterWithAI` at all |

So the client creates **anchor/area AI objects**, never the NPC actor puppets that would execute
goals, perception or pathfinding.

### 8.2 The AI phase API is explicitly unsupported

x2game `FUN_394b6900` (`ScriptBindUnit::ProcessAiPhase`, 0x394b6900):

```
X2Log(0, "ScriptBindUnit::ProcessAiPhase() we don't support this for client!");
```

`EndAiSkillLists` has the same message. These are the Lua hooks the server uses to run AI phases;
the client stubs them out.

### 8.3 The update is disabled during loading

x2game `FUN_3915b960` (loading-screen update, 1014 B) does:

```
if ((char)flags < 0) {                       // param_3 & 0x80000000
    cvar = IConsole::GetCVar(console, 0, "ai_systemupdate");
    if (cvar) cvar->SetValue(0);
}
```

`GetCVar` is case-insensitive (`XlHashStringI`), so this hits `ai_SystemUpdate`. No x2game or engine
code found by string/byte scan writes it back to 1. Independently, the outer `sys_AI` cvar and the
`this+0x18b0` object gate the call in `CSystem::Update` (E1 §1.5).

### 8.4 Real-client AI log is empty

`D:/AA1.2/ailog.log` from a real 1.2 client session contains exactly:

```
<RESETTING AI SYSTEM>
aiTick:startTime:<entity> ai_event    details
```

and `D:/AA1.2/aisignals.csv` contains only `Function,Time,Page faults,Entity`. No `CAISystem`,
`CAIHandler`, `AIPoint`, `NavigationModifier` or `aiTick` records. This is consistent with the
static picture: the system resets, then has no actors/events to process.

### 8.5 No client nav-data or smart-object load

* `CAISystem::LoadNavigationData` (`0x31088290`, slot `+0x134`): one data reference (the vtable),
  no code caller in crysystem/cryentitysystem/cryaction/x2game.
* `CAISystem::InitSmartObjects` (`0x31073d40`, slot `+0x8`): one data reference (the vtable), no
  code caller.
* No x2game string contains `SmartObjects.xml`, `Libs/ActionGraphs`, `navmesh`, `.ngm` or a
  navigation-data file name.

The `CGraph` the client sees is the ctor's empty graph, then AIPoint-prefab nodes.

### 8.6 No click-to-move / auto-walk path query

* Player movement is the physics/controller path in `port-movement`; the only `click_to_move`
  strings in x2game are the UI option (`OPTION_ITEM_CLICK_TO_MOVE`, `x2_effects.ui.click_to_move`)
  and the `click_to_move` console variable. No `CGraph`/`CAISystem` path call is made for it.
* The only client-side path request entry, `CAISystem::RequestPathToPipeUser` (slot `+0x2e8`,
  `0x31069090`), is called from x2game `0x39095e00` **after** `if (AI->vtable+0x2f8() == 0) { disable
  skill; return; }`. `+0x2f8` is `IsServer`, and the client ctor writes `CAISystem+0x344 = 0`, so the
  request is never issued.

### 8.7 CryAction's AI surface is dormant

See §2.4. CryAction's `CAIHandler` is a second copy of the same script layer; its AI vtable calls
have no x2game/engine caller and AA's Lua uses the x2game bind. If it were running, `ailog.log`
would contain its `<CAIHandler>` messages; it does not.

---

## 9. Phase relevance

| phase | what of the AI system matters | why |
|---|---|---|
| **P1** movement/riding | `GetMaxStep` = 0.1 s, and the fact that AI never moves or queries the player | the physics step cap is the only AI value on the feel path (E1 §1.5). Anchors/graphs do not touch the local controller. |
| **P2** combat | anchor skill controller (`GetAIObjectInSphere`), `SetAIObjectStatus("Frozen")`, AI event payloads for aggro/alert UI | client-side skill/anchor effects and target alerts; no damage or goals are computed by AI |
| **P3** interaction/housing | AI event listener areas (anchor/alertness/tag-point), AI points and navigation modifiers as world data | placement data can be inspected; server owns the actual NPC behaviour |
| **P4** vehicles | none | no AI use in the vehicle path |
| **P5** world flow/UI | `CAIHandler` entity/script state, AI log strings, AI event areas in cutscenes/cinematics | script state and diagnostics only |

**The P1 port decision is simple:** keep an AI object with `GetMaxStep() == 0.1f` (or just feed 0.1
to the physics step), keep the `sys_AI`/`ai_SystemUpdate` gates for ordering parity, and do not port
any AI behaviour. Everything else in this map is P2/P3/P5 and can be deferred.

---

## 10. In-process oracle recipes (aadriver)

The hidden client's globals are absolute VAs in each module; rebase by the module base. Module
bases: crysystem `0x36500000`, cryaisystem `0x31000000`, cryentitysystem `0x32000000`, x2game
`0x38ff0000`. CryAISystem is loaded at runtime; use its actual module base rather than assuming
the preferred base if ASLR is off/on in the harness.

### 10.1 Find the AI object

```
gEnv = *(void**)(crysystem_base + (0x366381b4 - 0x36500000));
AI   = *(void**)((char*)gEnv + 0x40);
assert(*(void**)AI == (void*)(ai_base + (0x311c4248 - 0x31000000)));  // CAISystem vtable
```

Cross-check: `AI` equals the pointer returned by `CreateAISystem` (`0x31096fc0`), and
`*(void**)(gEnv+0x40) == *(void**)(crysystem_base + (CSystem+0x68))`.

### 10.2 Frame-time cap

```
typedef float (__thiscall *GetMaxStepFn)(void*);
float v = ((GetMaxStepFn)(ai_base + (0x31042b00 - 0x31000000)))(AI);
assert(v == 0.1f);                    // constant at 0x311b6484
assert(*(void**)((char*)AI + 0x13c) == nullptr);
```

This is safe (pure, no writes).

### 10.3 State flags and cvars

```
u8  enabled     = *(u8*)((char*)AI + 0x137c);   // 1 after ctor
u8  initialized = *(u8*)((char*)AI + 0xe00);    // 1 after CAISystem::Init
u32 isServer    = *(u32*)((char*)AI + 0x344);   // 0 on client
void* aiUpdateCvar = *(void**)((char*)AI + 0xc8);
int  aiSystemUpdate = (**(int(__thiscall**)(void*))(*(void**)aiUpdateCvar + 8))(aiUpdateCvar);
void* updateAllCvar = *(void**)((char*)AI + 0x1d8);   // ai_UpdateAllAlways
```

Read `ai_SystemUpdate` before and after the x2game loading path (`0x3915b960`) to confirm the
disable; read `ailog.log` for the reset line.

### 10.4 One AI update

```
typedef void (__thiscall *UpdateFn)(void*);
((UpdateFn)(ai_base + (0x31093d10 - 0x31000000)))(AI);
```

It is safe in a scratch harness with anchors only (no physics/render dependencies beyond the AI
subsystems). Compare the count of objects/events before/after. Do not call it while the real client
is in a loading screen (it early-outs anyway).

### 10.5 AI event listeners

Build a dummy listener whose first vtable slot increments a counter:

```
struct Dummy { void** vt; int hits; };
void __cdecl cb(uint type, Vec3* pos, float r, uint p5, uint p6) { ... }
static void* vt[] = { (void*)cb };
Dummy d{vt,0};
AI->vtable[0xb4/4](AI, &d, &pos, 5.0f, 6);      // AddAIEventListener
// trigger through a real event path or call NotifyAIEventListeners directly:
((void(__thiscall*)(void*,uint,Vec3*,float,uint,uint))
    (ai_base+(0x31045b90-0x31000000)))(AI, 4, &pos, 0.0f, 0, 0);
assert(d.hits == 1);
AI->vtable[0xb8/4](AI, &d);                     // RemoveAIEventListener
```

Inspect the vector directly while validating: `begin=*(void**)(AI+0x1358)`,
`end=*(void**)(AI+0x135c)`, `count=(end-begin)/0x18`.

### 10.6 Anchor object

Spawn an `AIAnchor` entity (or call the x2game `RegisterWithAI` Lua binding) and then:

```
begin/end = *(void**)(AI+0x1358), *(void**)(AI+0x135c);  // listener records
// AI object map: read the anchor id from the entity, use +0x31c lookup
obj = AI->vtable[0x31c/4](AI, anchorId);                 // internal map lookup
assert(*(u16*)((char*)obj + 0x52) >= 1001);
float* p = (float*)((char*)obj + 0x5c);                  // GetPos result
```

Use `GetAIObjectInSphere` (vtable `+0x5c`) around the authored anchor position to prove the object
is findable. The returned object's `GetPos` (vtable `+0x3c`) should match the entity position.

### 10.7 CGraph node round-trip

```
CGraph* g = **(CGraph***)((char*)AI + 0x340);     // GetNavigationGraph (+0x1a4)
typedef int (__thiscall *CreateNodeFn)(CGraph*, int type, Vec3* pos, uint flags);
int id = ((CreateNodeFn)g->vt[0])(g, 4, &pos, 0); // 1-based id
void* node = ((GetNodeFn)g->vt[1])(g, id);
assert(*(u32*)((char*)node + 0x30) == 4);         // node type
((SetPosFn)g->vt[2])(g, id, &pos2);
memcpy(&out, (char*)node + 0x18, 12);             // position written by SetNodePos
((RemoveNodeFn)g->vt[0x1c/4])(g, id, 0);
```

`CheckNavigationType` (`AI->vtable+0x1a8`, `0x31051b10`) takes `(pos, outType*, 4)` and returns 0/1;
compare against a known AIPoint position. The full vtable order is in §6.1.

### 10.8 Navigation-shape round-trip

`AINavModifier` is the easiest trigger: load a prefab with a nav modifier, then inspect the AI's
named shape lists through the engine (no public getter is used by x2game). A direct call to
`CreateNavigationShape` needs the descriptor built by `0x3910c200`; the field map is in §7.3. Use
the prefab path first; only call `+0xbc/+0xc0` directly after dumping a real descriptor from a
breakpoint on `0x3910c200`.

### 10.9 Movement-speed table

```
float* def = (float*)((GetDefFn)(ai_base+(0x3108dd50-0x31000000))());
// def[0..6] are the seven default values at 0x311e3090
float out[7];
bool ok = ((GetSpeedsFn)(ai_base+(0x3104e410-0x31000000)))(movementParams, 1.0f, out);
```

`movementParams` is the actor's `SAIMovementParams` (`+0x30` mode 1, `+0x34` animation/movement
set); dump it from a real NPC actor if a full oracle is wanted.

### 10.10 What not to call blindly

`CAISystem::Update` and `NotifyAIEventListeners` are safe with no actors. `CreateNavigationShape`
and `RemoveNavigationShape` mutate global area lists and can affect later walkability queries; do
them in a disposable process. `CreateGoalPipe`/`RequestPathToPipeUser` need a pipe user and can
touch the pathfinder; they are not part of the client path.

---

## 11. Port order (testable leaves first)

1. **AI stub + max step + gates** (P1). A struct with `enabled/initialized/isServer`,
   `ai_SystemUpdate`, `ai_UpdateAllAlways`, and `max_step() -> 0.1f`. Unit test the constant and the
   `min(frameTime, maxStep)` use in the physics step.
2. **CAISystem field layout** for the flags and the nav-graph wrapper (`+0xe00`, `+0x137c`,
   `+0x344`, `+0x340`, `+0x1c`, `+0x1358/0x135c`). Read-only tests against the hidden client's
   memory (oracle §10.3).
3. **Base `CAIObject`** (`0xc0` bytes): type `+0x52`, position `+0x5c`, entity `+0xa4`; create
   default; set type/entity; get position. Test with anchor types `1001..1006`.
4. **AI object map**: id lookup `+0x318/+0x31c`, insert/remove, group removal; use it for
   `GetAIObjectInSphere`. Test two anchors at known positions.
5. **AI event listener vector**: add/remove/notify with the exact 0x18-byte record and the
   `(mask & type)` + squared-distance test. Unit-test with a dummy vtable callback.
6. **`CGraph` minimal**: empty graph, 1-based ids, 128-node blocks, CreateNode/GetNodePtr/
   SetNodePos/RemoveNode/Connect/point-in-bounds and the `+0x34..+0x68` field accessors. Test node
   ids and positions against the oracle.
7. **AIPoint prefab path**: `CreateNode` (type 4) and per-frame update with
   `CheckNavigationType`; keep a stub for nav type until the area classes are ported.
8. **Navigation shapes**: named `CAIShape` storage + `CreateNavigationShape`/`Remove...` with the
   descriptor from §7.3; only needed for AINavModifier/walkability parity.
9. **Movement-speed tables**: default 7 floats + `GetAIMovementSpeeds` from an actor movement
   struct; parse the Lua `AIMovementSpeeds` table if NPC animation needs it.
10. **x2game hand-over**: `RegisterWithAI`, AIPoint/nav-modifier prefab classes, anchor skill
    controller, AI event/aggro payloads. These are glue; implement them after the engine primitives
    have tests.
11. **Idle update entries**: `UpdatePathFinder`, `VisCheckBroadPhase`,
    `UpdateExpensiveAccessoryQuota`, `UpdateAuxSignalsMap` as no-op stubs that keep the frame order
    and profiler names. Do not port their bodies.
12. **Skip** goal pipes, perception, full pathfinding, nav regions, tactics, smart objects, debug
    drawing, and the CryAction CAIHandler.

---

## 12. Numeric parameters and cvars

| parameter | value | source |
|---|---:|---|
| AI physics max step | **0.1 s** (`0x311b6484`) | `CAISystem::GetMaxStep`, E1 |
| no-AI fallback max step | 0.25 s | crysystem physics block |
| physics step flags | `0x19f` | E1 |
| physics max world step / granularity | 0.2 s / 1e-4 s | `aaemu-physics`, E1 |
| AI object sizes | base `0xc0`; actor `4000`; `0x5f` `0x928`; 100 `0xdd0`; `0x96/0x97` `0x880` | `CreateAIObject` |
| anchor types | `1001..1006` | `ai/anchor.alb` |
| AI event record | `0x18` bytes `{listener,pos.xyz,radius,mask}` | `AddAIEventListener` |
| entity listener mask | `6` (2\|4) | `0x320a37d0`, `0x320a9cf0` |
| AI event types seen | `1` (sound/signal), `4` (visibility/signal) | `NotifyAIEventListeners` callers |
| AI event payload object | `0x30` bytes, vtable `0x311b9d18` | `+0x294` |
| CGraph node id | 1-based; `0` = invalid; 128-node blocks | `CreateNode`/`GetNode` |
| CGraph node fields | `+0x2c` navType, `+0x30` nodeType, `+0x34` posB, `+0x40` posA, `+0x14` link | §6.1 |
| default movement speeds | 7 floats at `0x311e3090` | `+0x2f4` |
| AI stances × speeds | 7 × 4 (Relaxed/Combat/Stealth/Crouch/Prone/Swim/Fly × Slow/Walk/Run/Sprint) | `ParseTables` |
| `ai_UpdateInterval` | 0.1 s | cvar registration |
| `ai_SystemUpdate` | 1 | cvar registration |
| `ai_UpdateAllAlways` | 0 | cvar registration |
| `ai_IgnorePlayer` | 0 | cvar registration |
| `ai_DefaultWalkability` | 1 (1 = X2, 2 = Cry) | cvar registration |
| `ai_doNotLoadNavigationData` | 0 | cvar registration |
| `ai_LimitPhysicsRequestPerFrame` | 2000 | cvar registration |
| `sys_AI` | 1 | crysystem `DAT_3663b420` |

---

## 13. Unknowns

* **U1 — `AINavGraph`/`CGraph` wrapper.** `CAISystem+0x340` is an `AINavGraph` (ctor `0x311225d0`,
  713 B), whose first field is the `CGraph*`; the double indirection in `+0x1a4` is clear,
  but the wrapper's other fields (`+0x128` container, `+0x1c`) and the exact ownership/lifetime are
  not fully mapped.
* **U2 — proxy → engine registration slot.** `0x31051260` (vtable `+0x18c`) creates an `0x5f`
  object and handles formations; it is *probably* the engine side of `RegisterWithAI`, but the
  entity-proxy vtable method that calls it was not dumped. Verify by breaking on `+0x44` during an
  `AIAnchor.OnInit`.
* **U3 — anchor object creation path.** `CreateAIObject`'s default case and the `1001..1006` types fit
  the anchor scripts, but the exact proxy call that passes the anchor type (`proxy+0xa4`?) needs a
  live trace. If anchors never allocate a `CAIObject`, only the listener record and the AI graph
  matter, and the port gets smaller.
* **U4 — `CAISystem+0x344` (IsServer).** ctor writes 0; who writes 1 on a server build is unknown.
  On the client it is 0, which is all the port needs.
* **U5 — `CSystem+0x18b0` gate object** in the AI-update condition (E1 also leaves it open). It
  returns 0 outside loading; its class is not named.
* **U6 — re-enable after loading.** Only `ai_SystemUpdate = 0` was found in x2game. Whether a
  console/config/level path writes 1 back (or whether the loading call only runs in a benchmark
  path) is open; `ailog.log` suggests no AI work in the recorded session.
* **U7 — navigation-shape descriptor.** The name at `+0x3c`, points and type/height are known from
  `0x3910c200`, but the full `SAINavigationShape` layout and the shape classes behind
  `CreateNavigationShape` are not mapped. Build a live descriptor dump before porting §6.2.
* **U8 — prefab presence.** `AIPoint`/`AINavigationModifier` loaders exist in x2game, but a scan of
  the 1.2 world/prefab data for their actual placement count was not done (the pak index has no
  `aipoint` filename; placement is inside level/prefab XML). If a map has none, the graph service is
  dead there too.
* **U9 — CryAction CAIHandler.** The "dormant" conclusion is from implementation duplication and
  absence of callers/logs; a live breakpoint on cryaction `0x3050f4b0` would settle it.
* **U10 — entity listener class.** The cryentitysystem proxy at vtable `0x320ceb70` calls
  `+0xb4`; the class name and the entity classes that use it (anchor/alertness/tag point?) are not
  resolved.
* **U11 — what the anchor object does in `Update`.** Base `CAIObject`s are in the time-sliced map;
  the exact update callback for type `1001..1006` and whether it has visible effects is unknown.
* **U12 — `ai_UpdateAllAlways` read.** The cvar is at `CAISystem+0x1d8`, but `IsUpdateAllAlways`
  uses `ICVar::vtable+8`; the vtable offset is inferred from the console/ICVar pattern, not from a
  named decompile.

---

## 14. Size of the work

Method: counts and sizes from `cryaisystem.dll.funcs.csv` (start/end inclusive), plus the small
gap functions measured with capstone (`GetMaxStep`, `IsServer`, `GetDefaultMovementSpeeds`,
`IsUpdateAllAlways`, `CreateEventObject`, `IncCounter`). The in-scope function set is listed
explicitly in the inventory above, so these are not address-band estimates.

### 14.1 Phase 1 (P1) — AI as far as movement feel needs it

| set | functions | bytes | KiB |
|---|---:|---:|---:|
| literal cap + boot: `CreateAISystem`, `CAISystem::Init`, `GetMaxStep` | 3 | 582 | 0.6 |
| practical P1: the above + ctor (flags/layout) + `Update` gate | 5 | 8,309 | 8.1 |

The rest of the live AI code has no phase-1 effect and can stay stubbed.

### 14.2 (b) The rest of the live client AI (P2/P3/P5)

Live CryAISystem set: **56 f / 39,986 B / 39.0 KiB** (includes the four idle update workers,
9,635 B). Split:

| bucket | f | bytes | KiB |
|---|---:|---:|---:|
| lifecycle/update/gates (including idle workers) | 14 | 22,793 | 22.3 |
| AI object map, anchors, events, logging | 18 | 4,043 | 3.9 |
| goal-pipe content loader (`+0x48`, pet manual) | 2 | 376 | 0.4 |
| CGraph + AIPoint graph primitives | 17 | 4,558 | 4.5 |
| navigation type + shapes | 3 | 8,011 | 7.8 |
| movement speeds | 2 | 205 | 0.2 |
| **live cryaisystem total** | **56** | **39,986** | **39.0** |

Hand-over and glue outside cryaisystem:

| set | functions | bytes | KiB |
|---|---:|---:|---:|
| x2game `RegisterWithAI`, AIPoint/nav-modifier/CAIHandler/anchor-skill/ParseTables/aggro/cvar | 21 | 14,802 | 14.5 |
| CryEntitySystem listener glue | 4 | 881 | 0.9 |
| CrySystem `InitAISystem` | 1 | 208 | 0.2 |
| **grand total live + hand-over** | **82** | **55,877** | **54.6** |

If the port only needs the P1 cap, skip the whole ~54 KB and keep a 3-field stub. If it wants the
anchor/nav world data, the live set is the right scope; the survey's 365 KB is **10× too large**.

### 14.3 Everything else in CryAISystem (not needed by the client)

| set | functions | bytes | KiB |
|---|---:|---:|---:|
| whole module | 7,155 | 1,707,828 | 1,667.8 |
| live (above) | 56 | 39,986 | 39.0 |
| **not ported** | **7,099** | **1,667,842** | **1,628.8** |

The not-ported 1.63 MB contains the goal-pipe/pipe-user system (~393 KB), navigation/pathfinding
core (~137.6 KB), flight/volume/waypoint/road nav regions (~150 KB), actor/group/tactics core
(~260 KB), smart objects/track patterns (~290 KB), debug/draw/editor paths, and all of their
template/thunk tails. None of it is reached by the client's live call graph; the only pieces that
are *called* every frame are the four idle update entry points already counted in the live set.

If a later phase wants full CGraph/AINavGraph parity (for server-style navigation or walkability
queries), the whole `CGraph` band is 227 f / 79,560 B / 77.7 KiB on top of the live 39.0 KiB; that
is the realistic next bucket, still far below 365 KB. The 365 KB survey figure only materialises if
one ports every class in the address bands, including goal pipes and nav regions that have no client
caller.
