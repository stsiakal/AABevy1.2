# Port map: the ArcheAge 1.2 client flow graphs, cutscenes and quest cameras (S6)

Address map for a function-by-function Rust port of the 1.2 client's flow-graph runtime
(`CFlowSystem`/`CFlowGraph`/`CFlowNode`, CryAction), the TrackView/movie cutscene player
(`crymovie`), the client cinematic camera actor, the quest camera, the flow-graph skill
controller (kind 8) and the flow-graph-driven doodad/animated-prop paths. It is written for
the port harness that calls the original functions inside the running hidden 1.2 client
(`aadriver.dll`).

## 0. Sources, conventions and licensing

### 0.1 Licensing

Everything here is derived **only** from the shipped 1.2 binaries and their decompiles. The
published CRYENGINE source was not used, quoted or consulted (same rule as
`aaemu-physics/CRYPHYSICS-MAP.md`, `port-combat/MAP.md` and the other `port-*` maps). Names in
`CFlow*`/`CMovie*` form come from the binaries' own RTTI-ish class-name strings, log/profiler/assert
strings, the `.data` node-name table, or the named 10.x build (structure/naming only). Behaviour
without a name in the binary is marked *(inferred)*. Port from this document and the decompiles,
never from the CRYENGINE source.

### 0.2 Sources

| source | path |
|---|---|
| 1.2 x2game decompile | `C:/AAEMU_DISK/aa-re/client12/out-x2game/x2game.dll.c`, `funcs.csv`, `calls.tsv`, `strings-xrefs.tsv` |
| 1.2 x2game image/data | `D:/aa-re/client12/x2game_unpacked.dll` (base `0x38FF0000`), `x2game_unpacked_data.dll` |
| 1.2 CryAction (flow graph runtime) | `D:/aa-re/client12/engine/cryaction.dll` + `engine/out/cryaction.dll.c`, `.funcs.csv`, `.strings.txt` |
| 1.2 CryMovie (TrackView) | `D:/aa-re/client12/engine2/crymovie.dll` + `engine2/out/crymovie.dll.c`, `.funcs.csv`, `.strings.txt` |
| 1.2 CryEntitySystem (entity properties) | `D:/aa-re/client12/engine/out/cryentitysystem.dll.c` |
| 10.x named build (names/structure only) | `D:/aa-re/zone/out/cryaction_dedicate.dll.c`, `C:/Users/stsiakal/AAEmu/REVERSING.md` |
| client pak / assets | `D:/AA1.2/game_pak` via `tools/pakx.py`; index `data/pak12.tsv` in the extraction worktree |
| existing notes | `docs/research/re-skills.md`, `re-quests.md`, `re-movement.md`, `remaining.md` §2.4, `port-movement/`, `port-mount-anim/`, `port-interaction/` |
| UI API surface | `data/ui-api.json` (x2ui Lua bindings) |
| server protocol names | `C:/Users/stsiakal/AAEmu/rust/crates/aaemu-game/src/opcodes.rs` (opcode ids only) |

x2game VAs use the preferred base `0x38FF0000`; CryAction's preferred base is `0x304F0000` and
CryMovie's is `0x34000000`. Function entry addresses are `FUN_xxxxxxxx` as named by Ghidra.
Evidence markers: **[R]** read from the binary/decompile, **[I]** inference, **[?]** open.

### 0.3 Layout of this folder

| file | content |
|---|---|
| `MAP.md` (this) | the call chains, the asset format, port order, unknowns |
| [`functions.md`](functions.md) | every function in scope: address, size, callers/callees, fields, purity |
| [`nodes.md`](nodes.md) | the node types found in 1.2 content, counts, host module, implementation addresses |
| [`structs.md`](structs.md) | Rust struct layouts/offsets (graph, port, edge, camera actor, descs, sequence) |
| [`cutscenes.md`](cutscenes.md) | cutscene, quest camera, skill controller kind 8 and doodad graph playback detail |
| [`difftest.md`](difftest.md) | in-process differential recipes per function |

### 0.4 Scope size

`functions.md` documents **124 function entries / 60,986 bytes** by `funcs.csv` (x2game 59 entries,
CryAction 51, CryMovie 14). The asset inventory covers **1,529 graphs / 10,295 nodes / 9,793 edges**
from 2,092 XML files and **255 sequence names / 10,476 tracks**. The node inventory in
`nodes.md` lists all **107 class names** that 1.2 content actually instantiates.

---

## 1. The one-page model

1. **A graph is an XML asset.** It is either embedded in an entity (`<Entity><FlowGraph>…</FlowGraph>`)
   in a level `cells/*/client/entities.xml`, a prefab, or a standalone `<Graph>` under
   `game/libs/flownodes/` or `game/libs/materialeffects/flowgraphs/`. The XML carries node class
   names (`Class="Time:Delay"`), port names on the edges (`portIn`/`portOut`), an optional
   `GraphEntity` index and a `MultiPlayer` mode. **[R]** loader `FUN_305525f0`
   (`cryaction`), format section 2.
2. **The host game registers node types.** CryAction registers the engine nodes and the
   ArcheAge `X2:Weather`/`X2:OceanWave`/`X2:SkillController`/`X2:PlayDoodadParticleEffect` nodes;
   x2game registers its own game/camera nodes (`Game:ActorSensor`, `Camera:OverrideFOV`,
   `Game:PlayerStaging`, `Game:PlayerLink`, `CrysisFX:ScreenFader`, the Crysis hit/explosion
   nodes, …). `CFlowSystem::RegisterType` (`0x30559170`) maps a name to a type id and a factory.
   **[R]**
3. **A graph instance is created from the XML** (`CFlowGraph` ctor `0x30551a00`; node factory
   `0x30552000`; XML init `0x305525f0`). Every node becomes a 0x20-byte port slot; every edge is
   compiled into a linked list of ports. The graph keeps the name, its owner entity, the two
   activation queues and the node objects. **[R]**
4. **Every frame the game framework calls `CFlowSystem::Update`** (`0x30556640`), which walks the
   graphs registered for update and calls `CFlowGraphBase::DoUpdate` (`0x3054eb40`) on each.
   `DoUpdate` drains the two activation queues: for each activated port it calls the target node's
   `ProcessEvent(portId, value)` and follows the port's edge list, obeying the 0x100 max-loop
   guard. **[R]**
5. **Entity binding** is by graph entity index or by GUID. A port flagged `0x1000`/`0x2000` carries
   an entity id (or `0xffffffff`/`0xfffffffe` for the graph's two special entities); the graph
   resolves it through `FUN_30546380` (`GetFlowgraphForwardingEntity`) before the node sees it.
   **[R]**
6. **Cinematics are not flow graphs.** A cutscene is a CryMovie `Sequence` XML
   (`game/worlds/<world>/cinemas/<name>.xml`) plus its companion entity file
   `<name>_cinema_only_objs.xml`. The sequence XML loader (`0x34002620`) loads both, with the
   paths hard-coded in the binary. `SCPlaySequence` (0x13b) → `OnPlaySequence` (`0x391e4250`) →
   `TryStartCinema(name, world, fade)` (`0x391e2a70`) → movie system `PlaySequence` (`vtable+0x4c` = `0x3400e200`).
   The sequence drives camera tracks, entity position/rotation/animation, sounds, subtitles, AVI
   screens and graph events. **[R]**
7. **Quest cameras are a dialogue feature, not a movie one.** 4,869 of the 4,876 `quest_contexts`
   rows have `use_quest_camera='t'`; the actual camera is selected per NPC **model** through
   `model_quest_cameras` and its 0x3c-byte `quest_cameras` row (offset, FOV, DoF, invisible).
   Lua `X2Quest:SetUseQuestCamera(true)` writes the sentinel camera id `0x3ff`; the cinematic
   camera actor (`DAT_39afe2b8`) then uses the model mapping and switches the camera to type 3.
   **[R]** (DB stats from the server copy, same rows as the client for these tables).
8. **Skill controller kind 8 is a flow graph per skill.** `skill_controllers.kind_id = 8`,
   `str_value1` = graph asset name; `FUN_390ca700` case 8 copies the name and `value1..9` and calls
   the controller ctor `0x390959f0`, which loads the graph, binds the owner entity and pushes the
   nine values into the `X2:SkillController` node's ports. End sends port `0x101`. Only one 1.2
   row uses it (`skill 23969`, `\game\libs\flownodes\skill\test1.xml`), so content coverage is
   small but the mechanism is a clean test target. **[R]**
9. **Doodads can play their own graph.** `DoodadFuncPlayFlowGraph` (phase-func type 0x17) has a
   desc `{event_on_phase_change_id, event_on_visible_id}`; the doodad entity's embedded graph is
   fired at those node ids. `X2:PlayDoodadParticleEffect` is a separate node used by prefab graphs.
   **[R]**
10. **Animated props** are ordinary entities (`AnimObject`) with an embedded graph; the node that
    drives them is `Animations:PlayAnimation` (config `0x305ccf40`) or the entity class's own
    animation properties. **[R]**

---

## 2. Flow-graph assets: format and where they come from

### 2.1 Counts (1.2 content)

Scanned: all level `entities.xml` (1,294 files, 21 MB), all `game/prefabs/*.xml` (235), all
`game/worlds/*/cinemas/*.xml` (513), `game/libs/flownodes/*.xml` (14) and
`game/libs/materialeffects/flowgraphs/*.xml`. Counting `<Node Class="…">` inside
`<FlowGraph>`/`<Graph>` blocks:

| family | graphs | share |
|---|---:|---|
| prefabs | 1,056 | 69 % |
| cinemas (companion object files) | 226 | 15 % |
| level entities | 198 | 13 % |
| material effects | 37 | 2.4 % |
| `libs/flownodes` | 12 | 0.8 % |
| **total** | **1,529** | |

10,295 nodes / 9,793 edges; 107 distinct class names. Multiplayer split: 1,423 `ClientOnly`, 101
`ServerOnly` (all Crysis-era material-effect leftovers) and 5 with no attribute. Node `flags`:
8,483 are 0 and 1,812 are 1 (loader treats a non-zero flag as an input-port default/negation — see
§3.4). About 2,585 nodes carry `GraphEntity="0"`.

### 2.2 Two XML shapes

**Embedded in an entity** (level cells, prefabs, cinema companion files):

```xml
<Entity … EntityGuid="4d590051bb455e63" EntityClass="ParticleEffect">
  <FlowGraph Description="" Group="housing_light" enabled="1" MultiPlayer="ClientOnly">
    <Nodes>
      <Node Id="1" Class="Time:TimeOfDayTrigger" pos="409,226,0" flags="0">
        <Inputs Active="1" Time="20"/>
      </Node>
      …
    </Nodes>
    <Edges>
      <Edge nodeIn="12" nodeOut="34" portIn="in1" portOut="out" enabled="1"/>
    </Edges>
  </FlowGraph>
</Entity>
```

**Standalone** (`game/libs/flownodes/*.xml`, material effects):

```xml
<Graph Description="" Group="fade" enabled="1" MultiPlayer="ClientOnly">
  <Nodes>…</Nodes><Edges>…</Edges>
</Graph>
```

Loader evidence: `FUN_305525f0` reads `Nodes` (`0x30728d44`), `Edges` (`0x30728d38`), `Node` /
`Class` / `Id` / `GraphEntity`, `Edge` / `nodeIn` / `nodeOut` / `portIn` / `portOut` /
`enabled`; errors `No [Nodes] XML child` (`0x30729130`), `No [Edges] XML child`
(`0x30729050`), `Failed to create node '%s' of type '%s' - FLOWGRAPH DISCARDED`
(`0x307290a4`), `Can't link edge <%s,%s> to <%s,%s> - FLOWGRAPH DISCARDED` (`0x30728ff4`),
`Did not load all edges (%d/%d edges) - FLOWGRAPH DISCARDED` (`0x30728fb8`),
`Did not load all nodes (%d/%d nodes) - FLOWGRAPH DISCARDED` (`0x30729068`). `_comment` nodes are
skipped (compare in `FUN_305525f0`). **[R]**

The standalone `game/libs/flownodes/camerashake.node` is a one-line
`<SimpleScript name="Camera:CameraShake" script="Scripts/FlowNodes/CameraShake.lua"/>`; the client
also has `game/scriptsbin/flownodes/camerashake.alb`. Scripted node factories are
`FUN_305b8e50` (`CFlowScriptedNodeFactory`) and `FUN_30561910` (`CFlowCompositeNodeFactory`). **[R]**

### 2.3 Where graphs are referenced

| reference | how | port target |
|---|---|---|
| level entity | `<Entity><FlowGraph>`: graph belongs to that entity; `GraphEntity="0"` on a node binds the entity input to the owner | `CFlowGraph` owner entity |
| prefab entity | same, inside `game/prefabs/*.xml`; spawned with the prefab | owner entity |
| doodad phase func | type 0x17 row `{event_on_phase_change_id, event_on_visible_id}`; fires the doodad entity's graph at those node ids | `FUN_3966fbd0`, `FUN_3962ccc0` |
| skill controller | `skill_controllers.kind_id=8`, `str_value1` = graph name (usually `\game\libs\flownodes\skill\<name>.xml`) | `FUN_390ca700` case 8 → `0x390959f0` |
| material effect | `game/libs/materialeffects/flowgraphs/*.xml` loaded by the MaterialFX manager (`CMaterialEffects::LoadFlowGraphLibs` strings `0x3074821c`/`0x30748970`); mostly Crysis leftovers, `ServerOnly` | not S6-critical |
| weather | `game/libs/flownodes/weather/sunny.xml`, `rainy.xml`, `snow.xml`; `X2:Weather` node | world ambience |
| preload list | `/libs/flownodes/preload.txt` read by `FUN_30558a70` before levels | global |

There is **no quest→graph reference in the client**: quest cinematics use the movie system, not
FlowGraph. The one gameplay graph that quests/skills reach is the skill controller above.

### 2.4 The node-name / type registry

The host DLLs keep the class name table in `.data`:

* CryAction: names at `0x30777330…` (`Movement:RotateTo`, `Environment:ComputeLighting`,
  `Entity:EntitiesInRange`, `X2:PlayDoodadParticleEffect`, `X2:SkillController`, `X2:Weather`,
  `X2:OceanWave`, `Camera:Camera`, `Camera:ViewShake`, `Image:*`, `Entity:*`, `Time:*`, `Logic:*`,
  `Math:*`, `AI:*`, `_comment`, `Start`, …), each 16-byte entry with a class-name pointer and a
  category dword (0/1/2). **[R]**
* x2game: names at `0x39ac3d68…` (`Game:ActorSensor`, `Game:DifficultyLevel`,
  `Camera:OverrideFOV`, `Game:PlayerStaging`, `Game:PlayerLink`, `CrysisFX:ScreenFader`,
  `Crysis:HitInfo`, `Crysis:ExplosionInfo`, `Crysis:FreezeEntity`, …) with the class factory
  vtables in `.rdata` (`0x399ae210`, `0x399ae2ac`, `0x399ae2fc`, `0x399ae858`, …). **[R]**
* `CFlowSystem::RegisterType` (`0x30559170`) allocates type ids (max 4095; error `CFlowSystem::RegisterType: Reached maximum amount of NodeTypes: Limit=%d`, `0x307297f8`)
  and stores `{factory, id, short id per category}`. Categories 0/1/2 are the "short id" buckets
  used by `CFlowGraph::CreateNode`; x2game's name remains in the type name for
  `GetFlowNodeTypeId` (`IFlowSystem vtable+0x20`, used by `FUN_39095cd0`). **[R]**

**Only five node classes are registered by x2game** for S6 purposes: `Game:ActorSensor`,
`Camera:OverrideFOV`, `Game:PlayerStaging`, `Game:PlayerLink`, `X2UI:ScreenFader` (plus the
Crysis hit/explosion/freeze and PostFX nodes which 1.2 content does not use). `X2:SkillController`,
`X2:Weather`, `X2:OceanWave`, `X2:PlayDoodadParticleEffect`, `Game:LocalPlayer`,
`Game:FindDoodadPos`, `Game:IsPlayerInRange` are CryAction nodes. **[R]**

---

## 3. The flow-graph runtime (CryAction)

### 3.1 Objects and ownership

```
CGame / CGameFramework
  └─ IFlowSystem                 (game framework[0x19] = 0x64, vtable below)
       ├─ type registry          name → {id, factory, category}
       ├─ preloaded graphs       name → CFlowGraph*      (FUN_30558a70/FUN_30556f40)
       └─ graph list             graphs updated each frame
            └─ CFlowGraphBase (CFlowGraph)
                 ├─ ports[]      0x20-byte entries, each with a list of edges
                 ├─ node objects  created by the type factory, 0x0c…0x144 bytes
                 └─ activation queues (two linked lists of port indices)
```

`IFlowSystem` vtable offsets observed in x2game: `+0x08` release, `+0x10` create graph,
`+0x20` `GetFlowNodeTypeId(name)`, `+0x24` get type name / default id, `+0xac` load/preload XML,
`+0x1a4` (on the **game framework**, `DAT_307a36b4`) preload flow graph from file. The exact
vtable is in CryAction; the port needs only these operations. **[R]** from the call sites in
`0x39095cd0`, `0x30558a70`, `0x30556f40`.

### 3.2 The update order

`CFlowSystem::Update` (`0x30556640`):

```
if (!flowSystem->enabled) return
for (inspector in system->inspectors) inspector->Begin(system)
if (flowSystem->graphs) {
    if (system->debugFlag) {
        for each object in the graph's pending list: object->vtable+8()
        for each graph in the system list: for each object: object->vtable+0xc()
    }
    CFlowSystem::UpdateStats()                 // 0x30549cc0
    if (system->debugFlag) {
        for each graph in the system list: for each object: object->vtable+0xc()
    }
}
```

The update starts by reading the frame time from the game framework (`DAT_307a36b4[5]->vtable+0x30`)
and pushing it into the engine timer through `FUN_306998a0(dt)` (`0x30556640`). The graph list is
`system+0x34..0x38`; per graph the "pending" list is `graph+0x114..0x118`.
The graph itself is updated through its vtable `+0x3c` → `CFlowGraphBase::DoUpdate`
(`0x3054eb40`, 984 bytes). The per-frame graph chain is `graph+0x130` and is reset in
`CFlowSystem::UpdateStats`. `UpdateStats` also logs `Number of Flow Graphs Updated: %d`,
`Number of Flow Graph Nodes Updated: %d`, `Number of Flow Graph Nodes Activated: %d`
(`0x30728ad3`, `0x30728aa3`, `0x30728afc`). **[R]**

`CFlowGraphBase::DoUpdate(graph, event)` drains the two queues `graph+0x54` (input) and
`graph+0x6c` (update). The port slot is `graph+0x70 + index*0x20`:

| slot offset | meaning |
|---|---|
| +0x00 | first edge / low-bit flags |
| +0x08 | value/variant array base (the `+ flagBit*8` and edge-loop reads) |
| +0x10 | node object (the class instance) |
| +0x18 | entity id when `+0x1e & 0x1000` |
| +0x1c | edge count (byte) |
| +0x1e | flags: `0x1000` = entity-bound, `0x2000` = entity forwarding, `0x4000` = (entity resolved) |

For every activated port: resolve the entity (if `0x1000`, `FUN_30546380`), call
`node->ProcessEvent(port, &value)` (vtable `+0x18`), clear the edge "active" bits, then walk the
edge list. `graph+0x20` is the graph name (`%s` in the max-loop error
`[flow] CFlowGraphBase::DoUpdate: %s -> Reached maxLoops %d during event %d`, `0x30728d68`). The
loop guard is 0x100; on event 3 it calls `DoUpdate(0xc)` recursively. **[R]**

### 3.3 Graph creation and XML init

* `CFlowGraph` ctor `0x30551a00` (1,030 bytes): sets the interface vtables at +0…+0x1c, allocates
  the port vector (+0x70) and the name string (+0x20), sets both queues empty
  (`+0x54 = 0xfffefffe`, `+0x6c = 0xfffe`), `+0xb2 = 0x101`, `+0xd8/+0xdc` special entity ids,
  `+0x124` entity system, `+0x130` update chain. **[R]**
* Node creation `FUN_30552000(graph, nodeType, class, config)` (1,048 bytes) calls the type
  factory, allocates the 0x20-byte port entry, copies the input defaults and the
  `GraphEntity` index. **[R]**
* XML init `FUN_305525f0(graph, xmlNode)` (1,166 bytes): resolves each `Class` through the host's
  `GetFlowNodeTypeId`, skips `_comment`, creates nodes, then resolves each edge's `portIn`/`portOut`
  names and links. `FUN_30553360(graph, bool)` calls it from the graph's `Init`. **[R]**
* Preload `FUN_30558a70` (993 bytes): reads `/libs/flownodes/preload.txt`, loads each line through
  the file system, creates the graph object, stores it in the preloaded map; errors
  `failed to preload flow graph from file : %s` (`0x30729738`). `FUN_30556f40` (240 bytes) returns
  a preloaded graph by name and logs `requesting non-preloaded flow graph xml : %s`
  (`0x307295d0`) otherwise. **[R]**

### 3.4 Port values and activation encoding

`CFlowBaseNode::GetPortBool` (`0x3055b0a0`) and `GetPortString` (`0x3055bed0`) are the leaves a
port needs; the rest of the family (int/float/vec3/entity) is in the same cluster
(`0x3055a930…0x3055c0xx`). The port id passed to `ProcessEvent` is a 32-bit value:
`nodeId | (portIndex << 16) | (edgeIndex << 24)`; x2game's helpers show the layout directly:
`CONCAT22(0x103, nodeId)` (output event on port 3), `CONCAT22(0x10a, nodeId)` (a value port),
`CONCAT13(1, CONCAT12(portByte, nodeId))` (activate input `portByte`). **[R]** from
`0x3914a400…0x3914a590` and `0x3914a5f0`.

Port values are 8-byte tagged variants: the low dword is the tag (0 = bool/int, 1 = float?, …;
the `CFlowBaseNode` accessors compare it) and the upper 4 bytes are the payload. The port slot
stores the first edge pointer and the edge stores the target port index; the decompiled
`FUN_3054eb40` shows the linked-list walk and the `& 0xfffffffd` flag clearing. **[R]** —
see `structs.md` §3 for the exact field offsets we can rely on.

### 3.5 Graph mode, flags and entity binding

`enabled` (graph byte +0xb0) gates updates. `MultiPlayer="ClientOnly"`/`"ServerOnly"` is a graph
attribute; the client creates only `ClientOnly` graphs (the 101 `ServerOnly` graphs in the pak are
all Crysis material effects). `GraphEntity="0"` on a node sets the input port to the graph's
owner entity; `GraphEntity="1"` etc. address the graph's entity list. `FUN_30546380` (596 bytes)
resolves a `0x1000`-flagged port: `0xffffffff` → graph+0xd8, `0xfffffffe` → graph+0xdc, otherwise
`IEntitySystem::GetEntity(entityId)`; if the entity has a `GetFlowgraphForwardingEntity`
forwarding entity (e.g. a vehicle seat / rider) it is substituted. The graph uses the result for
the node's `entityId` input. **[R]**

---

## 4. The node set actually used by 1.2 content

`nodes.md` has the full table. Summary by host:

| host | classes | nodes | notes |
|---|---:|---:|---|
| CryAction engine nodes | 97 | 10,065 | time-of-day/lighting/ambient volumes, entity + animation + logic + math + image + movement, `Start` |
| CryAction X2/AA nodes | 6 | 51 | `X2:SkillController` (2), `X2:Weather` (2), `X2:PlayDoodadParticleEffect` (6), `Game:LocalPlayer` (25), `Game:IsPlayerInRange` (10), `Game:FindDoodadPos` (5); `X2:OceanWave` has 0 shipped uses |
| x2game game/camera nodes | 4 used | 179 | `X2UI:ScreenFader` 153, `CrysisFX:ScreenFader` 21, `CrysisFX:PostFXWaterDroplets` 3, `CrysisFX:PostFXGlobal` 2; `Game:ActorSensor`, `Camera:OverrideFOV`, `Game:PlayerStaging` and `Game:PlayerLink` are unused by 1.2 content |

Important negative result: **no 1.2 shipped asset instantiates `Game:ActorSensor`,
`Camera:OverrideFOV`, `Game:PlayerStaging` or `Game:PlayerLink`**; they exist for the editor and
for 10.x content. The x2game nodes that 1.2 does use are the two screen faders and two PostFX
nodes (179 nodes, all in cinemas/effects). Only `X2UI:ScreenFader` (153 nodes), `Cinema:PlaySequence` (1 node), `X2:PlayDoodadParticleEffect` (6),
`X2:Weather` (2) and the engine nodes are reached by 1.2 content; `X2:SkillController` appears only
in the `libs/flownodes/skill/example.xml` test asset. This keeps the first port small.

### 4.1 Critical nodes and addresses

| class | config (GetConfiguration) | ProcessEvent | extra |
|---|---|---|---|
| `Cinema:PlaySequence` | `0x305ce390` | `0x305ce210` | serialise `0x305ce0d0`; plays a TrackView sequence; `m_bPlaying` at node+0x28, `IMovieSequence*` at +0xc |
| `Animations:PlayAnimation` | `0x305ccf40` | in the anim class vtable `0x30787xxx` cluster | ports Start/Stop/anim_Animation/BlendInTime/loop/ForceUpdate/PauseAnimGraph/ControlMovement |
| `Image:DepthOfField` | `0x305684d0` | in cluster | the cinema DoF node (`Image:DepthOfField` 129×) |
| `X2UI:ScreenFader` (x2game) | `0x39416800` | in class vtable | fade/screen overlay |
| `CrysisFX:ScreenFader` (x2game) | `0x3914cd30` | vtable family around `0x3914c160` (not decoded) | the older sky-blue fader |
| `entity:AreaTrigger` | entity-class proxy | entity class | the cinema `_cinema_only_objs` trigger |
| `entity:AnimObject` | entity-class proxy | entity class | animated props |
| `Game:LocalPlayer` | `0x30578e30` | class vtable | outputs the local player entity id |
| `Game:IsPlayerInRange` | `0x30578f40` | `0x30578f40` | |
| `Game:FindDoodadPos` | `0x30579900` | cluster | |
| `Time:TimeOfDayTrigger` | `0x305c0840` | cluster | 1,498 uses, mostly light switches |
| `Time:TimeOfDay` | `0x305c04b0` | | set/get time and speed |
| `Time:Time` | `0x305c06c0` | | elapsed time |
| `Time:Delay`/`RandomDelay` | in cluster 0x305c0xxx | | |
| `Logic:Any` / `Logic:All` | `0x3057eb40` / `0x3057ef70` | | |
| `Math:Less` / `Math:Equal` | `0x30587350` / `0x305871d0` | | |
| `Math:ToBoolean` | `0x30587e70` | | |
| `Vec3:ScaleVec3` | `0x305890f0` | | |
| `Movement:MoveEntityTo` | `0x305c77e0` | | |
| `Movement:RotateEntity` | `0x305c5d50` | | |
| `Movement:RotateTo` | `0x305c67d0` | | |
| `Physics:ActionImpulse` | `0x305b6580` | | |
| `Camera:ViewShake` | class vtable (not resolved) | | used 20× in cinemas |
| `Camera:CameraShake` | scripted `.node` → `Scripts/FlowNodes/CameraShake.lua` | | |
| `X2:PlayDoodadParticleEffect` | PlayDoodadParticleEffect cluster | `0x3055bfa0` | |
| `X2:SkillController` | (passive; receives port values) | — | the controller sends the events |

The full per-class count list and the exact per-family split are in `nodes.md`. Addresses marked
"—" or "cluster" are open (see §9): CryAction's node classes are contiguous `.text` clusters
whose config is identified by the description string, but the class vtables are in `.rdata` and
the per-method slots were not resolved for every class in this pass.

---

## 5. Cutscenes (TrackView)

### 5.1 Asset and path rules

the sequence XML loader `FUN_34002620` (625 bytes) builds two paths, using the world name
from the current level and the sequence name:

```
Game/worlds/%s/cinemas/%s_cinema_only_objs.xml
Game/worlds/%s/cinemas/%s.xml
```

(editor variants add `/editor/`). If the companion file is missing, it falls back to an `<Objects>`
node from the sequence file itself and sets `NumObjects=0`. Errors:
`Failed to load sequence xml file '%s'` (`0x3404c39c`), `Sequence name mistmatch!` /
`Sequence name mismatch! filename=%s , attribute=` (`0x3404c28c`/`0x3404c2b0`). **[R]**

### 5.2 Sequence XML

`<Sequence Name=… TimeOfDayName=… Flags=… StartTime=… EndTime=…>` with `<Nodes>`:

* `<Node Id="…" Type="1" Name="…" EntityGUID="…" DynamicType="…" GroupUnitText="player" Pos="…"
  Rotate="…" Scale="…">` — entity/actor
* `Type="2" Name="Scene"` — the scene node (AVI, subtitles, camera selection, sounds)
* `Type="3" Name="Camera_1" FOV="60" ShakeAEnabled="…" …>` — a camera source with FOV and two
  shake oscillators

Each node carries `<Track ParamId="…">` with `<Key time="…" …/>`. The ParamId semantics (from
`crymovie`'s track classes and the key attributes in all 255 sequence names):

| ParamId | meaning | key attributes |
|---:|---|---|
| 0 | camera parameters (FOV/shake) | `value` (fov), shake fields |
| 1 | position | `value="x,y,z"` |
| 2 | rotation | `value="qx,qy,qz,qw"` |
| 3 | scale | `value="x,y,z"`, `tens` |
| 4 | flow-graph event on the node's entity | `event="Disable/Enable/…"` |
| 5 | visibility | no attributes (hide/show at time) |
| 6 | active camera on the Scene node | `node`, `angle`, `interpolation`, `offset`, `renderRect` |
| 7 | sound / dialogue | `filename`, `volume`, `pan`, `duration`, `desc`, `InRadius`, `OutRadius`, `Stream`, `Is3D`, `Loop`, `Voice`, `LipSync` |
| 8 | voice (lipsync) | same |
| 9 | music | same |
| 10 | animation | `anim`, `length`, `loop` |
| 17 | console command | `command` |
| 20 | character animation-state name | `node` |
| 26 | 4-float parameter | `value` |
| 29 | attachment / IK target | `node`, `boneName`, `offset`, `rot` |
| 30 | equipment | `equipSlot`, `itemType` |
| 32 | boolean parameter | `value` |
| 100 | camera focus/DoF target | `node`, `angle`, `offset` |
| 110 | AVI screen | `node` (avi path), `subtitleNum`, `renderRect`, `color`, `cutRect`, `outline`, `shadow`, `align` |
| 120 | subtitle | `node`, `subtitleNum`, `renderRect`, `color`, `cutRect`, `outline`, `shadow`, `align` |
| 124–128 | ambience / music / voice buses | sound attributes |

**[R]** track ids and key attributes from the 255 sequences; **[I]** the human names where the
binary only has numeric ParamIds (crymovie's track classes use the same ids in
`0x340149xx…0x340170xx`; the mapping is validated by every key's attribute set).

### 5.3 Playback and stop

* `SCPlaySequence` 0x13b → `OnPlaySequence` (`0x391e4250`) → `TryStartCinema` (`0x391e2a70`).
  `TryStartCinema` calls `movieSystem->+0x18(name, world)`; when `+0x70` says it is not already
  playing, it reads `+0x2c` (sequence flags; `0x4000` = fade), adds the listener (`+0x34`), then
  plays with `+0x4c` or the fade path; it also updates the `quest_cinema_wnd`/`quest_cinema_fade_wnd` UI.
  **[R]**
* `TryStartCinemaNoFade` `0x39407ff0` (name, world) is the same start logic without a fade
  argument; it always takes the fade path when the sequence flag `0x4000` is set. Called from 10
  sites (login, movie, quest).
* `OnStopMovie` `0x39407e40` handles the listener stop: it matches the 7-character movie prefix
  against `00_login_s`, `01_world_select_e1`, `02_world_select_e2`, `03_character_select_s`,
  `04_character_select_e`, `05_character_create_s`, `06_character_customize_s`,
  `07_character_customize_e`, `08_intro`, `09_ci`, `10_opening`, and finishes the login/world
  sequence or the cinema UI accordingly. The table is at `0x39ac9150`. **[R]**
* `CMovieSystem::Update` `0x3400dcc0` walks the active sequence list (`+0x3c`), advances time,
  evaluates tracks and dispatches callbacks through the sequence's vtable (`+0x70` per node).
  `PlaySequence` (`0x3400e200`) sets it up; `IsPlaying` (`0x3400bda0`, 68 bytes) is the
  "any sequence has flag 8" test used by `OnUnitState`. `AddMovieListener` is `0x3400f700`.
  The full movie-system vtable offsets are in `structs.md` §7.
* `UseQuestCamera`/`SetUseQuestCamera` are **not** part of the movie system: they are Lua quest
  natives (`0x3949dab0` / `0x3949b270`), see §6.

### 5.4 Cinema actors and hiding

* Companion `_cinema_only_objs.xml` entities are spawned only for the sequence (e.g.
  `instance_prologue/cinemas/id_300_21_cinema_only_objs.xml`, 125 KB of props). Actors in the
  main level are referenced by `EntityGUID`; the sequence node's name is resolved by
  `FUN_34027680` (`Failed to find object(entity, unit, ...)!!! its name is '%s'`, `0x3404dae8`).
* `DynamicType="1"` + `GroupUnitText="player"` marks the local player (128 nodes in 255
  sequences). Other dynamic nodes (`pc`, `ch_01`…) are story NPCs; their models are spawned from
  the companion file.
* During a cutscene the client rejects `SCUnitState` updates (`OnUnitState` `0x391d4c30`, see
  §3.5) so server movement cannot fight the sequence.
* Camera work is the cinematic camera actor (`DAT_39afe2b8`), created by `FUN_3916bbd0` and
  registered with the movie system through `vtable+0x04`. Its update `FUN_3916b440` also serves
  quest cameras (§6). The sequence camera node itself comes from the `Type="3"` nodes with the
  `FOV`/shake attributes and the Scene node's ParamId 6 track. **[R]**
* `quest_cinema_wnd` and `quest_cinema_fade_wnd` (strings `0x399bb510`/`0x399bb524`) are the UI
  windows; `UIC_QUEST_CINEMA_FADE_WND` (`0x399da000`) is the fade state exposed to Lua. The
  screen fade/letterbox state machine is `FUN_393f6fc0` (956 bytes) — see `cutscenes.md` §3.
* Skip: the client does not expose a special skip function; pressing the UI/cancel key goes
  through the normal input path and calls the movie system's stop (`vtable+0xb4` = `0x340207f0`,
  a thunk) or `StopSequence`; `SetUseQuestCamera(false)` is the quest-camera counterpart.
  **[I]** — the exact key binding was not traced. See §9.

---

## 6. Quest camera

Data (server DB, same rows as the client for these tables):

| table | rows | fields used by the client |
|---|---:|---|
| `quest_cameras` | 51 | `id, camera_offset_x/y/z, comment, dof, fov, interpolate, invisible, npc_offset_x/y/z, nv_bokeh_size, nv_dof, nv_intensity, nv_luminance` |
| `model_quest_cameras` | 100 | `id, model_id, quest_camera_id` |
| `quest_contexts` | 4,876 | `use_quest_camera` = `t` for 4,869 |

All 51 1.2 quest cameras have `invisible='t'` and `interpolate='f'`; 43 have `dof='t'`.

Call chain:

```
Lua X2Quest:SetUseQuestCamera(enable)                     0x3949b270
  └─ DAT_39afe2a0->vtable+0x14(enable ? 0x3ff : 0)        // 0x3ff = "use the target model's camera"
Lua X2Quest:UseQuestCamera()                              0x3949dab0  (vtable+8 == 0x3ff)
NPC talk start (FUN_39379c70)                             enable quest camera for the NPC
  ├─ FUN_3915b160(npc)                                    camera actor target = NPC
  └─ FUN_391673a0(1, cameraId) → FUN_39164ff0             camera actor +0x1a8 = 1, +0x1ac = id
cinematic camera actor update FUN_3916b440
  ├─ desc = FUN_398562e0(id)                              quest_cameras id lookup
  ├─ if no desc and local player has a target:
  │     FUN_398426c0() → model_quest_cameras rows
  │     find model_id == target model → FUN_398562e0(quest_camera_id)
  ├─ if +0x1a8 and desc: set camera type 3 (FUN_39161540)
  ├─ copy desc +4..+0xc to actor +0x1c8..+0x1d0 (camera offset)
  ├─ copy desc +0x20..+0x28 to actor +0x1d4..+0x1dc (npc offset)
  ├─ copy desc +0x18 fov (actor +0x1e0) and blend position/rotation
  └─ FUN_39354520(desc+0x1d invisible) on the player and the NPC (unit+0x3c5a)
```

`FUN_39164ff0` (1,061 bytes) is the enable/disable state machine; `FUN_391673a0` (24 bytes) is the
thin wrapper used by `LeaveQuestDirectingMode` (`0x3949b1c0`) and the talk path. The desc layout
is in `structs.md` §6. **[R]** for the call chain and offsets; **[I]** for the exact unit that
receives `invisible` when the second lookup returns null.

What the Bevy port must reproduce: a camera mode 3 that positions the camera at
`npc_world * npc_offset + camera_offset` with the desc's FOV, blends only if `interpolate='t'`,
hides the dialogue units when `invisible='t'`, applies the DoF parameters (or at least the
NV DoF fields) and restores the previous camera on disable.

---

## 7. Skill controller kind 8

`skill_controllers` row (SELECT order, 24 dwords; `re-skills.md` §1.1):

| index | column | used at |
|---:|---|---|
| 4 | `kind_id` | switch in `FUN_390ca700` |
| 5 | `start_anim_id` | after creation |
| 6 | `str_value1` | graph asset name (kind 8) |
| 9..23 | `value1..15` | controller parameters |

Kind 8 path (`FUN_390ca700` case 8, `0x390ca700` + case at `0x390cad…`):

```
copy desc.str_value1 into a 0x100-byte buffer
copy desc.value1..9
allocate 0x144 bytes
FUN_390959f0(actor, graphName)                   // 651 bytes, ctor
   graph = flowSystem(DAT_3a5bb1d8[0x19])->vtable+0x10()
   graph->vtable+0x24(owner entity / name)
   graph->vtable+0x40(name, 1)                   // load/init
   graph->vtable+0x48()                          // start
   for each node of type X2:SkillController:
       activate port 1 with value 1
       activate ports 2..10 with value1..9
FUN_39095cd0(controller)                         // end: port 0x101 with 1, then release
```

The `X2:SkillController` node itself is passive: it is the graph's data sink for the controller
values and the source of the `0x101` end event. `nodeId`/port encoding is the same as §3.4.
**[R]** from `FUN_390ca700`, `FUN_390959f0`, `FUN_39095cd0`, `FUN_3909a820`.

## 8. Doodads and animated props

* **DoodadFuncPlayFlowGraph** is phase-func type 0x17. Loader `FUN_3966fbd0` (401 bytes) reads
  `doodad_func_play_flow_graphs(id, event_on_phase_change_id, event_on_visible_id)`; lookup
  `FUN_3962ccc0` (50 bytes). The doodad entity's embedded graph is activated at those node ids on
  phase change and on becoming visible. **[R]**
* **`X2:PlayDoodadParticleEffect`** (`0x3055bfa0`, 201 bytes) is a CryAction node that takes a
  doodad entity and a particle name and starts/stops it; the 1.2 content uses it 6× in
  `game/prefabs/interaction_c.xml`. **[R]**
* **Animated props** are `AnimObject` entities in the level/prefab. Their graph uses
  `Animations:PlayAnimation`/`StopAnimation` (996/229 uses) and `Entity:*`/`Movement:*` nodes.
  Entity class properties (`Animation`, `bLoop`, `Speed`, `bPhysicalize`) are in the entity XML; the
  entity-system class reads them (`cryentitysystem` `0x104089` reads `HiddenInGame`). **[R]**
* `AnimObject` also appears as `Type="1"` sequence nodes with ParamId 10 animation tracks
  (665 tracks in the 255 cinemas), so the same asset plays through the movie player. **[R]**

---

## 9. Port order (testable leaves first)

1. **Pure data**: `quest_cameras`/`model_quest_cameras` desc layout (`0x39892880`, `0x39892b70`),
   `skill_controllers` row layout (`0x3971e060`) and the `doodad_func_play_flow_graphs`
   layout (`0x3966fbd0`). Unit tests against the DB rows; no client state.
2. **Range lookups**: `FUN_398562e0` (quest camera by id), `FUN_3962ccc0` (doodad graph ids),
   `FUN_398426c0` (model camera table). Differential calls with known ids.
3. **Graph XML parser + node registry**: port `FUN_305525f0` on an XML string and compare the
   `(nodeId, class, portIn/out, edge)` structure and the failure logs for the 1,529 assets.
   `CFlowSystem::RegisterType` (`0x30559170`) is the stateful part; test name→id stability.
4. **Graph update**: `CFlowGraphBase::DoUpdate` (`0x3054eb40`) on a hand-built graph
   (`Start → Time:Delay → Logic:Any`) and compare port activation order and the node
   `ProcessEvent` calls; then `CFlowSystem::Update` (`0x30556640`).
5. **Entity binding**: `FUN_30546380` with entity ids and a forwarding entity (mount/rider).
6. **Critical nodes**: `Cinema:PlaySequence` (`0x305ce210`/`0x305ce390`), `X2UI:ScreenFader`
   (`0x39416800`), `entity:AreaTrigger`, `Animations:PlayAnimation` (`0x305ccf40`), `X2:SkillController`.
7. **Movie player**: the sequence XML loader (`0x34002620`) → sequence graph/track model; `PlaySequence`
   (`0x3400e200`), `Update` (`0x3400dcc0`), `IsPlaying` (`0x3400bda0`); validate on
   `zq_01_wing` (12 s, 2 cameras, 1 AnimObject, 3 sounds) and `sl_el_001` (85.7 s AVI + subtitle).
8. **Quest camera state machine**: `FUN_39164ff0` (`SetQuestCamera`) and the camera actor update
   `FUN_3916b440`; call with a known `quest_cameras` row and compare the derived camera TM.
9. **Skill controller kind 8**: `FUN_390959f0` on the test graph + `FUN_390ca700` case 8; compare
   the port activations.
10. **Doodad graph + prop animation**: `FUN_3966fbd0` + the doodad phase path, and an
    `AnimObject` graph through the movie player (ParamId 10).

## 10. Unknowns

1. **Per-node ProcessEvent addresses for the ~70 CryAction classes not listed in §4.1.** The config
   functions are identifiable by their description strings; the class vtables are in `.rdata`
   (`0x3077xxxx`), but this pass did not resolve every vtable slot. The port can start with the
   critical subset and add classes as content needs them.
2. **The registry table structure** at `0x30777330` (CryAction) and `0x39ac3d68` (x2game):
   16-byte entries with a name pointer and a category, but the exact pointer/field meaning and
   the type-id ordering are not decoded. `RegisterType` returns the id; the table is the name
   side. Needed only for byte-identical node ids, not for behaviour.
3. **How `MultiPlayer="ServerOnly"` is rejected on the client** (all such assets are Crysis
   material effects; no 1.2 gameplay graph is ServerOnly).
4. **The exact unit that receives `invisible` in `FUN_3916b440`** (player, NPC, or both) and the
   `interpolate=false` path: the code copies the flag into unit+0x3c5a via `FUN_39354520`, but the
   second lookup can return null.
5. **The quest-camera owner object `DAT_39afe2a0`**: only four references in x2game; which class
   creates it (and its vtable `+0x08`/`+0x14` implementations) is not decoded. The setter/getter
   behaviour is clear from the call sites.
6. **`param_4` of `FUN_3916b440`**: evidenced as a quest-camera id (looked up by
   `FUN_398562e0`, with the model fallback), but the caller is `FUN_3916bc70` from the sequence
   camera callback `FUN_393758d0`; the exact track field that supplies it needs one in-process
   trace.
7. **Skip binding**: no explicit skip native was found; the stop path is the movie system stop
   (`vtable+0xb4`) / `StopSequence`, but the key that triggers it is not traced.
8. **Sequence ParamId 0/3/5/17/20/26/29/30/32 exact engine semantics** (inferred from key
   attributes; the port needs the crymovie track classes at `0x340149xx…0x340170xx` read in
   detail).
9. **`Cinema:PlaySequence`'s graph class id / registration**: config `0x305ce390` and
   ProcessEvent `0x305ce210` are solid; the class name string is at `0x3073cb5c`.
10. **Which 1.2 quests actually trigger `SetUseQuestCamera`**: the DB says 4,869 contexts, but the
    Lua quest scripts only show `UseQuestCamera`; a live trace of a quest talk is needed to see the
    sequence of `SetUseQuestCamera`/`EnterQuestDirectingMode` calls.
