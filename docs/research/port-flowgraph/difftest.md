# Differential tests for the flow-graph / cutscene port

The harness is the shared one: `aadriver.dll` runs inside the hidden 1.2 client and exposes
`call` (address, this, args), memory read/write, allocation and the module bases. The recipes
below are ordered leaves-first and state exactly what is compared. Every golden is a JSON
`{fn, this, args, ret, memory:[[addr,len,hex],…], log:[]}`; CI replays the Rust port against the
golden, the oracle run regenerates it from the original.

## 0. Calling convention notes

* x2game/engine functions are `__thiscall`: the object is `ECX` and Ghidra prints it as the first
  parameter. The driver's `call` needs a `--this <addr>` mode; if it only supports
  cdecl/stdcall, pass `this` as the first arg and set `ECX` with a small stub (the existing
  `feat/driver-call` branch already does this for `aaemu-physics`).
* Return values: `bool` is `AL`, `u8`, `u16`, `u32`, `f32`/`f64` are as declared in
  `functions.md`. Integer-returning functions may leave garbage in the upper bytes; compare only
  the documented width.
* Strings are `char*` (`std::string` only in engine internals; never pass a `std::string` by
  value to the original). For a name argument, allocate a NUL-terminated buffer in the client
  process and pass its address.
* Every test starts from the same client state: login → world select → enter `main_world` (or the
  smallest instance world), wait for the level load and the first `CFlowSystem::Update`, then
  freeze input. The movie system (`DAT_3a5bb1d8+0x3c`) and the camera actor (`DAT_39afe2b8`)
  exist only after the world screen; assert non-null before the first call.

## 1. Pure data leaves

### 1.1 Quest-camera desc lookup (`FUN_398562e0`, x2game `0x398562e0`, 62 bytes)

```
inputs:  id in eax
setup:   enter a world so the DB is loaded (the 51 rows are loaded at startup)
run:     5c: ret = FUN_398562e0(id)
compare: ret (desc pointer) and, when non-null, the 0x3c bytes at ret
ids:     10, 11, 12, 17, 0, 999999, 0x3ff
```

`0x3ff` must return 0 (the sentinel relies on that). Re-run after `FUN_39892880` has loaded the
table; the pointer may differ between runs, so compare **contents**, not addresses.

### 1.2 Doodad graph row lookup (`FUN_3962ccc0`, 50 bytes)

`this` is the range-vector object at the doodad manager + the `doodad_func_play_flow_graphs`
sub-object (the loader writes `client+0x82c8/+0x82cc`). Input ids from the DB
(`SELECT id FROM doodad_func_play_flow_graphs`), output desc pointer; compare the three dwords.

### 1.3 Node lookup by name

`IFlowSystem vtable+0x20` (`GetFlowNodeTypeId(name)`) is not a `FUN_` but is called from
`FUN_39095cd0` with `"X2:SkillController"` (`0x39ae4f5c`) and from `FUN_305525f0` for every XML
`Class`. Golden: the u16 returned for every class name in `nodes.md`; it must be stable across
runs and equal to the id used by the Rust registry. This also validates the name table at
`0x30777330`/`0x39ac3d68` without decoding its internal layout.

## 2. Graph construction and static structure

### 2.1 XML init (`FUN_305525f0`, CryAction `0x305525f0`, 1,166 bytes)

Pick an existing preloaded asset (no need to fabricate XML):

```
setup:  enter the world; preload is run by FUN_30558a70 at startup
graph:  FUN_30556f40(&out, "game/libs/flownodes/weather/sunny.xml")
run:    re-init not needed; read graph->ports at graph+0x70
golden: for i in 0..nodeCount-1: {class id (port+0x18?), flags (port+0x1e),
        entity id (port+0x18), edgeCount (port+0x1c), edge targets}
```

Because the object has multiple interface vtables, use `graph+0x70` and the 0x20 stride from
`DoUpdate` (`0x3054eb40`). Compare against the Rust parser's expected graph for the same file.
Repeat for `rainy.xml` (3 nodes), `interaction_c.xml` (Math:Random, X2:PlayDoodadParticleEffect)
and a level `entities.xml` graph (e.g. `main_world/cells/019_023/client/entities.xml`).

If a synthetic XML is needed, the driver must find the engine's `LoadXmlFromString` (not mapped
here); the simpler alternative is a loose file under the client `game/` directory loaded as an
override, which the port's test build can add without touching the pak.

### 2.2 RegisterType / type id (`FUN_30559170`, CryAction `0x30559170`, 557 bytes)

```
this: the IFlowSystem
args: name = "Test:Node" + factory pointer (a Rust-side stub is not valid in the client;
      use an existing factory from a known node, e.g. the one returned by vtable+0x20? no)
```

The function needs a real `CFlowNodeFactory` object; the safest test is **not** to call it with
fabricated factories but to call `GetFlowNodeTypeId` for the existing names (1.3). The
registration path is stateful and only interesting for the max-4095/override logs; add a later
test that registers a name twice through a factory cloned from an existing type and compares the
returned ids / the `already registered. Overriding.` log.

## 3. Graph evaluation

### 3.1 `CFlowGraphBase::DoUpdate` (`0x3054eb40`, 984 bytes)

Use a real graph with a deterministic event, not a random/time one:

* `game/libs/flownodes/weather/sunny.xml`: `X2:Weather` → `Time:Delay(delay=1)` → `Weather:Stop`.
  With the graph's time frozen (call `DoUpdate` with a controlled dt? `Delay` reads the frame
  timer; the driver can set `DAT_...` time or run the client paused), the first `DoUpdate(0)`
  must leave the queue empty; after the delay elapses the `OnEnded` port fires exactly once.
* A better fixture is a test graph whose nodes are pure: `Start → Math:SetNumber → Logic:Any →
  Time:Delay → Math:SetNumber`. The driver can build it as a loose asset (above) or start from
  `game/libs/flownodes/skill/example.xml` and only use the `X2:SkillController`/`Vec3` paths.
* Golden: the ordered list of `(nodeId, portId, value)` triples that `DoUpdate` passes to
  `ProcessEvent` plus the final queue head (`graph+0x54/+0x6c`). Capture them by reading the
  port array before/after, or by a small detour on the node vtable `+0x18` (the driver can
  write a breakpoint-style hook). The Rust port must produce the same order and the same
  max-loop behaviour for a cycle (0x100 guard).

### 3.2 `CFlowSystem::Update` (`0x30556640`, 671 bytes)

```
setup: graph registered for update (DoUpdate ran at least once)
run: call FUN_30556640(system)
golden: the sequence of graph->Update calls (vtable+0x3c) and the three stat counters
        DAT_3077d774/d778/d77c after the frame
```

Compare the counters with a graph that activates exactly N nodes; the numbers are in
`UpdateStats` (`0x30549cc0`, 347 bytes).

## 4. Entity binding (`FUN_30546380`, 596 bytes)

Construct a graph with a single `Entity:GetPos` node and `GraphEntity="0"`, set the owner entity
to a known NPC, then:

```
inputs: port index of the entity input, graph, target entity
run:    FUN_30546380
compare: resolved entity id; special 0xffffffff / 0xfffffffe map to graph+0xd8/+0xdc
```

Repeat with a mounted player (forwarding entity) and with an entity that has no forwarding
entity; the function must return the same id the node would receive. This is also the test for
the port's `GraphEntity` index handling.

## 5. Critical nodes

### 5.1 `Camera:OverrideFOV` (x2game `0x3914b3f0` / `0x3914ad30` / `0x3914ab90`)

```
setup: allocate 0xc bytes; this = node; set vtable 0x399ae30c, +8 = 0
run:   ProcessEvent(port 1 (CONCAT22(0x101, nodeId)), float value=45.0)
       read cl_fov via DAT_3a5bb1d8+0x1c vtable+0x60 (GetCVar("cl_fov"))
compare: +8 == previous cl_fov, then cl_fov == 45.0
run:   ProcessEvent(port 3 (CONCAT22(0x103, nodeId)), 0)  (reset)
compare: +8 == 0, cl_fov restored
serialise: call 0x3914ab90(node, write, xml) and compare "m_storedFOV"/"curFOV"
```

This is the smallest full node test and covers `GetConfiguration`, `ProcessEvent`, `Serialize`
and a real engine side effect. Use the client's own `cl_fov` and restore it after the test.

### 5.2 `Cinema:PlaySequence` (`0x305ce390` config, `0x305ce210` event)

```
setup: allocate 0x30? node; this = node; +0xc = movie system; +0x28 = 0
run:   ProcessEvent(Trigger) with seq_Sequence_File = "zq_01_wing"
compare: node+0xc != 0, node+0x28 == 1, movie->vtable+0x70(seq) == true
run:   ProcessEvent(Stop)
compare: node+0x28 == 0, node+0xc == 0 after the release
```

The sequence must be loaded (the loader `0x34002620` runs on first use); run in a world whose
`Game/worlds/main_world/cinemas/zq_01_wing.xml` exists (the client pak has it). Compare the
returned `IMovieSequence*` **behaviour** (is-playing, time) rather than the pointer.

### 5.3 `X2UI:ScreenFader` (`0x39416800` config + class vtable)

Follow the same pattern: set `FadeOutTime`, activate `FadeOut`, call the node update with a
controlled dt (the fader is driven by the UI update), read `CurColor`/`FadedOut`. The x2game
class vtable `0x399ae858` has the event and serialise slots; the exact slot numbers are in
`structs.md` §3. If the visual overlay is not deterministic headless, compare the node fields
and the `color` left in the node.

### 5.4 Camera actor / quest camera (`FUN_39164ff0`, `0x3916b440`)

```
setup: world loaded; actor = DAT_39afe2b8 (non-null); pick an NPC with a model in
       model_quest_cameras (query the DB for a model_id that has a camera)
run:   FUN_3915b160(actor, npcUnitId)
       FUN_39164ff0(actor, 1, 0x3ff)
       FUN_3916b440(actor, npcUnit, cameraId, 0x3ff)
compare: actor+0x1a8 == 1, actor+0x1ac == 0x3ff,
         actor+0x1c8..+0x1d0 == desc+4..+0xc of the model's camera,
         actor+0x1d4..+0x1dc == desc+0x20..+0x28,
         actor+0x1e0 == desc+0x18,
         unit+0x3c5a == desc+0x1d for the player and the NPC
run:   FUN_39164ff0(actor, 0, 0)
compare: actor+0x1a8 == 0, +0x1b0/+0x1b4 cleared, camera type restored to 3
```

If `FUN_3916b440` requires a live sequence camera, call it with `param_3 == actor+0x1ac` (the
early-return guard) and `param_4 = 0x3ff`. The pure offset-copy part is then testable without a
movie. The NPC must exist (spawn a quest NPC or use the local player's model if it is in the
mapping; the player model usually is not — use an NPC).

### 5.5 Skill controller kind 8 (`FUN_390ca700` case 8, `0x390959f0`, `0x39095cd0`)

```
setup: world loaded, local player actor, flow system ready
desc:  the skill_controllers row 7907 (kind 8, str_value1 = test1.xml); if the asset is absent
       in the pak (it is: only example.xml ships), use 23969/7907's str_value1 or create a
       loose graph with an X2:SkillController node and a Time:Delay
run:   FUN_390ca700(actor, desc, target, tag=1)
compare: returned controller != 0; controller+0x13c graph != 0; for each X2:SkillController
         node the ports 1..10 were activated with 1, value1..9
run:   FUN_39095cd0(controller)
compare: port 0x101 activated with 1; graph released; controller vtable back to base
```

Because `skill_controllers.id=7907` points at `\game\libs\flownodes\skill\test1.xml` which is
**not in the 1.2 pak**, the test must supply a loose graph or use a synthetic desc whose
`str_value1` points at `example.xml` (which has two `X2:SkillController` nodes and the same
port contract). Mark this in the golden as a test fixture, not a shipped asset.

### 5.6 Doodad flow graph

```
setup: a doodad with an embedded graph and a DoodadFuncPlayFlowGraph phase func
run:   change the doodad phase (the normal interaction path)
compare: the graph's event_on_phase_change/visible nodes fire, the doodad particle/anim plays
```

This is an integration test; the pure leaves are the desc loader `0x3966fbd0` and the lookup
`0x3962ccc0`.

### 5.7 Screen fader state machine (`FUN_393f6fc0`)

```
setup: allocate the fader object (the UI owns it; use the real one and save/restore fields)
run:   set state 1 with inTime=1.0, call with dt=0.25 x 4
compare: state 2 after 1 s, alpha = 1.0, then state 3 -> 0 after the out time
```

The renderer side effect cannot be compared headless; compare the state/alpha and, if a
screenshot harness is available, the pixel at the centre of the frame.

## 6. Movie system

### 6.1 `IsPlaying` / `PlaySequence` (`0x3400bda0`, `0x3400bd30`, `0x3400e200`)

```
setup: world loaded, movie system = *(DAT_3a5bb1d8+0x3c)
run:   PlaySequence("zq_01_wing") via vtable+0x4c (or 0x3400bd30)
compare: IsPlaying (vtable+0x74) == true; sequence time (vtable+0x118) advances by dt
        within 1 frame tolerance; Stop (vtable+0xb4) makes IsPlaying false
```

Do not compare raw pointers. The Rust `SequencePlayer` gets the same inputs (name, dt) and must
produce the same `IsPlaying` and the same active-camera name at each time sample. Golden times:
0, 6.49, 6.51, 12.0 (zq_01_wing switches camera at 6.5 and ends at 12).

### 6.2 Sequence XML loader (`0x34002620`)

`this` is a `CSequence` (ctor `0x340028a0`), not the movie system. The driver can allocate the
object, set the name at +0x40, call the ctor, then call `0x34002620`; compare the loaded counts
(`NumObjects`), the name, and whether the companion file path resolved. Repeat for
`zq_01_wing` (companion present) and a name without a companion (fallback to `<Objects>`).

### 6.3 `OnUnitState` (`0x391d4c30`)

```
setup: movie playing vs stopped
run:   FUN_391d4c30(0, fakePacket) with a unit-state payload
compare: with the movie playing the unit state is not applied and the log string is emitted;
         with it stopped the state is applied (read the unit's state fields)
```

This is a log/behaviour pair; capture the `UnitStatePacket rejected during cutscene playing.`
line from the client log (the harness already has the log) plus the unit field diff.

## 7. Golden format and CI

```
{ "fn": "FUN_398562e0", "module": "x2game", "this": null,
  "args": ["u32:10"], "ret": "0x...", "ret_kind": "u32",
  "mem": [["0x...", 60, "hex..."], ...], "log": [], "notes": "..." }
```

* `mem` blocks are content-compared with a normalisation: pointers zeroed, padding ignored.
* Logs are normalised (timestamps/addresses masked) and compared as an ordered subsequence.
* The CI run loads the goldens and drives the Rust port; any mismatch fails with the first
  differing `(offset, expected, actual)`.
* Tests that need a live world are marked `"world": "main_world"` and run only in the oracle
  phase; pure leaves run without a client.

## 8. What not to test through the driver yet

* `CFlowSystem::Update` and `DoUpdate` on random/time graphs: the outcome depends on the frame
  timer and the level's graph set. Use the controlled fixtures above.
* `CFlowSystem::RegisterType` with fabricated factories: the factory object layout is not
  decoded; use `GetFlowNodeTypeId` instead until the factory vtable is mapped.
* Any renderer-side fader/AVI/letterbox pixels: compare state, not pixels, until a screenshot
  pipeline is available.
