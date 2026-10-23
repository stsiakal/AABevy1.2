# Cutscenes, quest camera, skill controller kind 8 and graph-driven props

This note is the behaviour side of the map: what the client does with the assets, in the order
it does it, with the address of every decision point. `MAP.md` has the summary; `structs.md`
has the layouts; `functions.md` has the function table.

## 1. Cutscene playback (CryMovie TrackView)

### 1.1 The packet

The server starts a cutscene with `SCPlaySequence` (0x13b in `aaemu-game/opcodes.rs`). The
client handler is `OnPlaySequence` `0x391e4250` (129 bytes):

```
FUN_390115a0("OnPlaySequence", 2, ...)          // profiler scope
FUN_39325120(*(u32*)(packet + 0x2c))            // DAT_3a167e80 = id
FUN_391e2a70(packet + 0xc, &DAT_3999c708, 0)    // TryStartCinema(name, world="", fade=0)
FUN_390116d0()
```

`packet+0xc` is the sequence name, `packet+0x2c` an id stored globally. The handler is registered
through the opcode table at `0x382882` (`FUN_391c5790(FUN_391e4250, 0)`); the opcode id itself is
from the server port. **[R]** for the handler and the call; the 0x13b id is **[R]** in our
server's `opcodes.rs`, not read from the client table in this pass.

### 1.2 `TryStartCinema` (`0x391e2a70`, 524 bytes: name, world, fade)

```
movie = *(IMovieSystem**)(DAT_3a5bb1d8 + 0x3c)          // game framework's movie system
if (!movie) { log "Failed to get movie system..." ; return 0 }
seq = movie->vtable+0x18(name, world)                    // FindSequence(name, world)
if (!seq) return 0
if (!movie->vtable+0x70(seq)) {                          // only when not already playing
    seq->vtable+0xb4()                                  // stop (no-op/cleanup)
    flags = seq->vtable+0x2c()                          // get sequence flags
    movie->vtable+0x34(seq, &PTR_PTR_39ac30a0)          // AddMovieListener
    FUN_3901b930(seq)                                   // adds the cinema listener for the player
    if (flags & 0x4000) {                               // fade variant
        if (fade) { movie->vtable+0x94(seq); fade setup }
        else      { fade setup }
    }
    movie->vtable+0x4c(seq, 0, 0, DAT_399b9e38, DAT_399b9e38, DAT_399b9e38)   // play
}
```

`TryStartCinemaNoFade` `0x39407ff0` (name, world) is the same start logic without a fade argument: it always takes
the fade path when the sequence flag `0x4000` is set, and otherwise plays with `+0x4c`. The movie system object is at game framework `+0x3c`; the object's vtable offsets
are listed in `structs.md` §7. The `0x4000` sequence flag means "fade" and the colour argument is
the black constant `DAT_399b9e38`. **[R]**

### 1.3 Loading the assets (the sequence XML loader `0x34002620`, 625 bytes)

```
world = current level name (level system vtable+0x24)
if (!sequenceObject) {
    open Game/worlds/<world>/cinemas/<name>_cinema_only_objs.xml
    if (!found) use <sequence>/Objects and NumObjects=0
}
open Game/worlds/<world>/cinemas/<name>.xml
if (!found) { log "Failed to load sequence xml file '%s'"; stop }
else sequence->vtable+0x98(xmlNode)        // parse
```

The `_cinema_only_objs.xml` companion supplies the props and story NPCs of the scene; the
`<name>.xml` is the `Sequence` itself. The sequence-name/attribute mismatch is rejected with
`Sequence name mistmatch!` (`0x3404c28c`) or
`Sequence name mismatch! filename=%s , attribute=` (`0x3404c2b0`). **[R]**

### 1.4 What the sequence drives

`CMovieSystem::Update` `0x3400dcc0` advances the active sequences and calls the sequence's
`vtable+0x70` per track node. Tracks (ParamId) from the 255 1.2 sequences:

| id | what it drives | implementation target |
|---:|---|---|
| 1/2/3 | entity position / rotation / scale | entity transform |
| 4 | graph event on the node's entity | `IEntity::SendEvent` (the same `Event` names as the entity's graph) |
| 5 | visibility (hide/show) | entity render flags |
| 6 | active camera on the Scene node | camera source entity |
| 7/8/9 | sound / voice / music | sound system |
| 10 | animation (`anim`, `length`, `loop`) | character animation, `AnimObject` |
| 17 | console command (`e_time_of_day …`, `ca_FacialAnimationRadius`) | console |
| 29 | attachment/bone transform | attachment |
| 30 | equipment (`equipSlot`, `itemType`) | equipment |
| 100 | DoF focus target | camera DoF |
| 110 | AVI screen | renderer / UI |
| 120 | subtitle | UI |
| 124–128 | ambience/music/voice buses | sound |

The `Scene` node (`Type="2"`) holds the camera-selection track; the `Camera` nodes (`Type="3"`)
hold the FOV/shake parameters; the entity nodes (`Type="1"`) hold the transforms/animation.
**[R]** for the track ids/keys; **[I]** for the named behaviour where the binary has only the
numeric ParamId (the mapping is consistent with every key's attribute set).

### 1.5 The cinematic camera actor

`FUN_3916bbd0` (158 bytes) creates `DAT_39afe2b8` (0x368 bytes, ctor `FUN_3916ad80`, vtable
`0x399b0a90`) once, then registers it with the movie system `vtable+0x04`. If the movie system is
missing it logs `Failed to get movie system. Maybe camera system will not work correctly.`
(`0x399b0c08`). The camera actor is the object that implements both sequence cameras and the
quest camera (`FUN_3916b440`, 1,936 bytes). Camera type 3 is the fixed/quest mode
(`FUN_39161540` logs `changedCameraType : %d -> %d`). **[R]**

### 1.6 Stop and skip

`OnStopMovie` `0x39407e40` is the movie listener stop callback. It matches the first 7 characters
of the movie name against the 11-entry table at `0x39ac9150`:

```
0 00_login_s              6 06_character_customize_s
1 01_world_select_e1      7 07_character_customize_e
2 02_world_select_e2      8 08_intro
3 03_character_select_s   9 09_ci
4 04_character_select_e  10 10_opening
5 05_character_create_s  11 none
```

Index 2/4 ends the login/world-select sequence; 9 finishes the cinema UI and restores the
resurrection state; 10 ends the opening movie and either finishes the world entry or restarts
the login screen. Everything else just logs `Stop movie : %s`. **[R]**

There is **no client skip function** with a name; the normal stop goes through
`IMovieSystem::vtable+0xb4` (`0x340207f0`, a thunk) or `StopSequence` (`0x3400ee90`). Which key
invokes it was not traced; see `MAP.md` §10. The UI side is `quest_cinema_wnd`
(`0x399bb510`) / `quest_cinema_fade_wnd` (`0x399bb524`), with the fade window also driven by
`FUN_393f6fc0` (screen state machine below).

### 1.7 The screen fader / letterbox (`FUN_393f6fc0`, 956 bytes)

`FUN_393f6fc0(obj, dt)` is a 4-state machine on `obj+8`: 0 idle, 1 fade-in, 2 hold, 3 fade-out.
`obj+0x14/+0x18` are the target rect, `obj+0x1c/+0x20` the in/out durations, `obj+0x3c` the
colour/alpha scale and `obj+0x34` a group index. It draws a textured/coloured quad through the
renderer's `vtable+0xf4` and then serves the 8 (0..7) subtitle/AVI channels, each with its own
0x20c-byte buffer. Unknown states log `Unknown cinematic screen state(%d)` (`0x399d5527`). **[R]**
The port can replace the renderer calls with the Bevy UI overlay; the state machine and the
timings are the reusable part.

## 2. Quest camera (dialogue camera)

### 2.1 Data

| table | rows | use |
|---|---:|---|
| `quest_cameras` | 51 | one 0x3c-byte desc per camera |
| `model_quest_cameras` | 100 | NPC model → camera |
| `quest_contexts` | 4,876 | 4,869 rows have `use_quest_camera='t'` |
| `cinemas` | 149 | quest cinemas (`replay='t'` for 93) |
| `quest_components` | 17,851 | 111 rows with a `cinema_id` (111 distinct ids) |
| `quest_act_obj_cinemas` | 12 | quest objective "watch cinema" |
| `cinema_captions` / `cinema_subtitles` / `cinema_effects` | 97 / 71 / 27 | subtitle and screen-effect tables |

All 51 cameras have `invisible='t'`, `interpolate='f'`; 43 have `dof='t'` and `nv_dof='t'`.
**Quest camera is the normal dialogue camera in 1.2, not an exception.** **[R]** (server copy
`D:/aaemu-live/rs-game/Data/compact.sqlite3`, same rows as the client's encrypted DB for these
tables).

### 2.2 The chain

```
X2Quest:SetUseQuestCamera(bool)          Lua binder 0x394a6e50
  → FUN_3949b270                         (49 bytes)
      DAT_39afe2a0->vtable+0x14(enable ? 0x3ff : 0)

X2Quest:UseQuestCamera()                 Lua binder 0x394a6e50
  → FUN_3949dab0                         (212 bytes)
      DAT_39afe2a0->vtable+8() == 0x3ff

X2Quest:GetMainQuestCutSceneReplayList() → FUN_3949db90 (241 bytes)
      builds a Lua table from the loaded cinemas where replay='t'

NPC talk start                            FUN_39379c70 (423 bytes)
  FUN_39856420(unit) / FUN_398563a0(npc)  // two desc lookups
  if npcDesc+0x34 (has quest camera)
      FUN_3915b160(npc)                   // camera actor +0x1b0 = npc
      FUN_391673a0(1, cameraId)           // 24-byte wrapper
          FUN_39164ff0(actor, 1, cameraId) // 1,061 bytes
```

`FUN_39164ff0` sets actor+0x1a8 = enable, +0x1ac = camera id. With the `0x3ff` sentinel the
later desc lookup fails by design and the camera actor falls back to the NPC model mapping.

### 2.3 The camera actor update (`FUN_3916b440`)

```
if (param_3 != actor->cameraId) return
if (param_4 == DAT_39ac3f60) return
desc = FUN_398562e0(param_4);                     // quest_cameras by id
if (!desc && localPlayer->vtable+4() == 1) {
    localPlayer->vtable+8(&targetModel);
    FUN_398426c0(&table);                         // model_quest_cameras
    for (row : table) if (row.model_id == targetModel) { desc = FUN_398562e0(row.camera_id); break; }
}
if (actor->questCameraEnabled && desc) {
    if (actor->cameraType != 3) { FUN_39161540(3); ... }
    actor->speed = 1.0 / ...;
    copy desc offsets to actor+0x1c8..+0x1e0;
    compute the camera position from the target's world TM + camera/npc offsets;
    FUN_39354520(desc->invisible);                // unit+0x3c5a (hide)
    FUN_39354520(desc->invisible);                // second unit (target when available)
    actor->... = blend;
    FUN_3915a9e0(...);                            // camera/shake
}
```

`FUN_39354520(unit, flag)` writes `unit+0x3c5a`; the same byte is the stealth "hidden" flag
(`re-skills.md` §5), so the dialogue camera and stealth share the hide path. `FUN_3915beb0`
restores the previous camera when leaving type 3. The exact unit for the second
`FUN_39354520` call is `[?]` when `FUN_3933f5e0()`/`FUN_39360020` returns null.

### 2.4 What to implement in Bevy

1. A `QuestCamera` resource holding the 51 descs and the 100 model mappings (loaded from the
   client DB; the existing `aa-client` DB code already loads other tables).
2. `SetUseQuestCamera(bool)` → mode "quest camera"; `true` = use the model mapping, `false` =
   restore the normal third-person camera.
3. On NPC dialogue start, set the camera target to the NPC, select the model's camera, apply
   `npc_offset`/`camera_offset`/`fov`, hide the player and NPC when `invisible`, and blend only
   if `interpolate` (all 1.2 rows are false, so it is a hard cut).
4. On dialogue end call the equivalent of `FUN_3915beb0` / `FUN_39161540(previousType)`, unhide
   the units and restore the camera.

## 3. Skill controller kind 8

The only 1.2 row: `skill_controllers.id=7907`, `kind_id=8`, `str_value1='\\game\\libs\\flownodes\\skill\\test1.xml'`,
`value1..9=0`, `value10=1`, used by `skills.id=23969`
(`test_flowgraph_skillcontroller`). No shipped player skill uses the real thing; the mechanism is
therefore a clean unit-test target rather than a content requirement.

Creation (`FUN_390ca700` case 8, 1,610 bytes):

```
copy desc.str_value1 (desc+0x18) into a 0x100 buffer
copy desc.value1..9 (desc+0x40 .. +0x5c) into locals
new 0x144 bytes → FUN_390959f0(actor, graphName)
```

Constructor `FUN_390959f0` (651 bytes):

```
copy the 0x49-dword (0x124-byte) state block into controller+0x18
graph = flowSystem->vtable+0x10()
graph->vtable+0x24(owner)
graph->vtable+0x40(name, 1)     // init
graph->vtable+0x48()            // activate
for each node of type "X2:SkillController":
    graph->vtable[0](nodeId | 1<<16, value=1)      // port 1
    for i in 1..9: graph->vtable[0](nodeId | (i+1)<<16, value_i)
```

End `FUN_39095cd0` (269 bytes): for each `X2:SkillController` node sends port `0x101` with 1 and
releases the graph. The node itself does nothing; the graph around it reacts to the port values.
`re-skills.md` §1.2/1.3 has the actor/tag rules (who may start a controller; kinds 1/2/3/4/6/7
are local-player only, kinds 5/8 run on any actor). **[R]**

## 4. Doodad flow graphs

`DoodadFuncPlayFlowGraph` is phase-func type **0x17**. Its desc is three ints
(`doodad_func_play_flow_graphs`: `id`, `event_on_phase_change_id`, `event_on_visible_id`), loaded
by `FUN_3966fbd0` (401 bytes) into client+0x82c8 (ids) and +0x82cc (pairs); lookup
`FUN_3962ccc0` (50 bytes). The graph is the **doodad entity's own embedded graph**; the client
activates the node ids on phase change and on becoming visible. No graph name is stored in the
doodad row. **[R]**

`X2:PlayDoodadParticleEffect` (`0x3055bfa0`, 201 bytes) is a different node: it takes a doodad
entity and a particle name (from a `ParticleEffect` entity's `Properties`) and starts/stops it.
Used 6× in `game/prefabs/interaction_c.xml`. **[R]**

## 5. Animated props

* `AnimObject` entities (172 graph nodes, plus `Type="1"` sequence nodes) are normal entities with
  `object_Model` `.chr`, `Animation`, `bLoop`, `Speed`, `bPhysicalize` properties. Their graph
  uses `Animations:PlayAnimation` (config `0x305ccf40`) and `Animations:StopAnimation`.
* The sequence player also drives `AnimObject`/character animation directly through ParamId 10
  tracks (`anim`, `length`, `loop`), 665 in the 255 cinemas.
* `entity:AnimObject` is an entity-class proxy node; it does not reimplement the animation, it
  exposes the entity's properties/events. Port it as a thin adapter over the existing
  `aa-anim`/`aa-world` actor animation, not as new logic.
* The 1.2 content uses 996 `Animations:PlayAnimation` nodes, mostly in `prefabs/common.xml`
  (movement/doors) and level entities (props); the cinema companion files use the sequence
  ParamId 10 path instead. **[R]**

## 6. What the protocol adds

* `SCPlaySequence` 0x13b drives `OnPlaySequence` (cinematics).
* `SCUnitState` is dropped during a cutscene (`OnUnitState` `0x391d4c30`,
  `UnitStatePacket rejected during cutscene playing. unit id(%u)`). The port must do the same or
  remote actors will jitter under the sequence.
* The quest side has `CSStartedCinemaPacket` 0x0cf / `CSCompletedCinemaPacket` 0x0ce and the
  directing-mode permission 0x0d0 on the server; the client-side senders were **not located** in
  this pass. `quest_act_obj_cinemas` needs the server to see "cinema started/ended"; mark the
  client sender as an unknown until a live packet capture of a quest cinema settles it.

## 7. Reference content for the first port

| asset | why |
|---|---|
| `game/worlds/main_world/cinemas/zq_01_wing.xml` + `_cinema_only_objs.xml` | small: 12 s, Scene + 2 cameras + 1 AnimObject + 3 sounds; the quest-wing cutscene |
| `game/worlds/main_world/cinemas/sl_el_001.xml` | 85.7 s, AVI screen + subtitle track + AreaTrigger events; tests the Scene/AVI/subtitle path |
| `game/libs/flownodes/weather/sunny.xml` / `rainy.xml` | 7/8 nodes, `X2:Weather` + `Time:Delay`; tests a standalone graph |
| `game/prefabs/interaction_c.xml` | `X2:PlayDoodadParticleEffect` (6×), `Math:Random`; doodad graph |
| `game/prefabs/common.xml` | the biggest graph set: `Animations:PlayAnimation`, movement and logic; the animation/prop path |
| `game/libs/flownodes/skill/example.xml` | `X2:SkillController` + Physics/Vec3/Time; the kind-8 node in isolation |
| `game/worlds/instance_library/cinemas/id_303_01.xml` | ProximityTrigger + `Image:VolumetricScattering` + camera DoF; tests the trigger/prop interactions |
