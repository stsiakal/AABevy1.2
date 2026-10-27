# Port map: the ArcheAge 1.2 cutscene player (CryMovie / TrackView)

> **Restored 2026-10-27 from `archive/fork-main-2026-10-27`, UNVERIFIED.** The client was restarted from NickMesser's `main`; nothing described here is wired into the game. Addresses, call chains and offsets must be re-checked against the decompile ([R]/[I]/[?]) before use. A 2026-10-27 audit found 151 of the 269 EXACT/TOLERANCE labels in `proof-levels.*` weak or wrong (report: `D:\aaemu-client\handoff\2026-10-27\audit\report-proofs.md`).

Function-level map of the 1.2 client's `crymovie.dll` — the TrackView sequence player behind every
cinema — plus its hand-over from `x2game` and the `Movie` Lua table in `cryscriptsystem`. This is
roadmap v2 map **E5**. Phase relevance: **P5** (world flow / cutscenes); no phase-1 (riding /
on-foot) function is in this module.

Companion maps (do not redo):

| map | covers |
|---|---|
| [`../port-flowgraph/MAP.md`](../port-flowgraph/MAP.md) | x2game flow graphs, the x2game side of cutscenes, quest camera, camera actor |
| [`../port-flowgraph/cutscenes.md`](../port-flowgraph/cutscenes.md) | `SCPlaySequence` handler, `TryStartCinema`, `OnStopMovie`, cinema UI, quest camera, sequence-XML track-id table |
| [`../port-flowgraph/structs.md`](../port-flowgraph/structs.md) | x2game camera actor, `IMovieSystem` vtable (several entries are corrected below) |
| [`../port-movement/MAP.md`](../port-movement/MAP.md) | local player/mount movement and normal camera |
| [`../port-program.md`](../port-program.md) | roadmap v2 and phase/status table |

## 0. Sources, conventions and licensing

### 0.1 Licensing

Everything here is derived **only** from the shipped 1.2 binaries and their decompiles, the
shipped pak content and the shipped `.alb` scripts. The published CRYENGINE source was not used,
quoted or consulted. Class names in `CClass` form come from the binaries' own RTTI, profiler
strings, log/assert strings, cvar names or the class-name strings in the module; behaviour that has
no name in the binary is marked *(inferred)*. Port from this document and the decompiles, never
from the CRYENGINE source. 10.x builds are used for names only.

### 0.2 Sources

| source | path |
|---|---|
| 1.2 crymovie decompile | `D:/aa-re/client12/engine2/out/crymovie.dll.c` (1.72 MB) |
| 1.2 crymovie functions/calls/strings | `D:/aa-re/client12/engine2/out/crymovie.dll.{funcs.csv,strings.txt,imports.txt,exports.txt}` |
| 1.2 crymovie image | `D:/aa-re/client12/engine2/crymovie.dll`, image base `0x34000000` |
| 1.2 cryscriptsystem decompile | `D:/aa-re/client12/engine3/out/cryscriptsystem.dll.c` (being decompiled; `crysystem.dll.c` also exists) |
| 1.2 x2game decompile | `C:/AAEMU_DISK/aa-re/client12/out-x2game/x2game.dll.c` |
| 1.2 x2game image | `D:/aa-re/client12/x2game_unpacked_data.dll`, base `0x38FF0000` |
| cinema content | 513 `game/worlds/*/cinemas/*.xml` from the pak (258 sequences + 255 companion files), extracted read-only to `data/extract/port-engine-movie/` (git-ignored) |
| Lua scripts | 961 `game/scriptsbin/**/*.alb` from the pak (extracted read-only to the same folder) |

Addresses are virtual addresses at the module's preferred base. Function entry addresses are
`FUN_xxxxxxxx` as named by Ghidra; a few vtable targets that Ghidra did not split into functions
are given as raw addresses and were disassembled with capstone for this map. Sizes are from
`funcs.csv` (code bytes).

Evidence markers: **[R]** read from the binary/decompile/content, **[I]** inference, **[?]** open.

### 0.3 Layout of this folder

This pass is a single file (the module is one coherent subsystem). It is organised so a port can
stop after any section:

| section | content |
|---|---|
| 1 | the call chain from `SCPlaySequence` to the visible result, phase relevance |
| 2 | object model and vtables (`IMovieSystem`, `ISequence`, nodes, tracks) |
| 3 | the sequence XML asset (audited against all 258 shipped sequences) |
| 4 | playback: load, play, update, stop, skip, time of day |
| 5 | track semantics (camera, animation, sound, event, screen, …) |
| 6 | entity binding and hidden cinematic actors |
| 7 | hand-over to x2game and the `Movie` Lua binding |
| 8 | in-process harness (driver call/read/alloc) and difftest recipes |
| 9 | port order (testable leaves first) |
| 10 | numeric parameters |
| 11 | unknowns |
| 12 | sizing (function count / KB for phase 1 and the rest) |
| 13 | key-function table (module, address, size, callers/callees, fields, pure/stateful) |

### 0.4 Scope size

Whole module: **1,628 functions / 268.0 KB** of code (`funcs.csv`). The survey's to-port subset is
1,226 f / 231.9 KB; the XML-value-parser band (249 f / 28.4 KB) is replaced by a Rust XML reader,
and 153 f / 7.6 KB are editor/container/CRT code. The cinema-XML audit in §3 removes another
111 f / 20.9 KB of track/node classes that no shipped sequence uses. See §12.

---

## 1. The call chain

### 1.1 Server packet → sequence playback

```
server SCPlaySequence (opcode 0x13b)
└─ x2game X2::GameClient::OnPlaySequence                 0x391e4250 (129 B)
   └─ X2::GameClient::TryStartCinema                      0x391e2a70 (524 B)
      ├─ movie = *(IMovieSystem**)(gameFramework + 0x3c)
      ├─ seq   = movie->vtable+0x18(name, world)           CreateSequence      0x3400e4e0
      ├─ if (!movie->vtable+0x70(seq))                     IsPlaying(seq)      0x3400cef0
      │  ├─ seq->vtable+0xb4()                             ISequence::Load     0x34002620
      │  ├─ send CSStartedCinemaPacket 0xcf                 FUN_39428370(0xcf)
      │  ├─ movie->vtable+0x34(seq, listener)               AddMovieListener    0x3400f700
      │  ├─ FUN_3901b930(seq)                              add x2game listener 0x3901bcd0
      │  ├─ if (seq flags & 0x4000)                        fade path / movie->+0x94
      │  └─ movie->vtable+0x4c(seq,0,0,black,black,black)   PlaySequence       0x3400e200
      └─ (the fade path may stop all sequences first)
```

`TryStartCinemaNoFade` `0x39407ff0` is the same start logic without the fade argument; it always
takes the fade path when `flags & 0x4000` is set. The `0x4000` sequence flag means "fade"; the
colour argument is the black constant `DAT_399b9e38`. Details of the x2game side are in
[`cutscenes.md`](../port-flowgraph/cutscenes.md) §1.2.

**Correction to `cutscenes.md` §1.2:** the `seq->vtable+0xb4()` call in `TryStartCinema` is
`ISequence::Load` (`0x34002620`), not "stop". `IMovieSystem+0xb4` is a different function (it is
the paused getter, `0x340207f0`). Stop-sequence is `IMovieSystem+0x54` (`0x3400f6a0`) /
`+0x5c` (`0x3400f6c0`, fade).

### 1.2 One engine frame while a cinema plays

```
engine frame
└─ IMovieSystem::Update(dt)                               0x3400dcc0 (812 B)
   ├─ if (movie+0x59) return                              paused
   ├─ read frame time from gEnv->pTimer (+0x14)->vtable+0x24
   ├─ for each entry of the active list (movie+0x3c):
   │  ├─ entry.time += dt
   │  ├─ if (entry.time >= entry.endTime):
   │  │  ├─ flags & 4  -> entry.time = entry.loopStart (loop)
   │  │  ├─ flags & 2  -> hold at end, still update
   │  │  └─ else      -> append to stop list, do not update
   │  └─ seq->vtable+0x70(&ctx)                           ISequence::Update   0x34001eb0
   │     ├─ seq.time = ctx.time
   │     ├─ engine pause / ui_draw_level handling
   │     ├─ per node:
   │     │  ├─ node->vtable+0x114()                       entity (or 0)
   │     │  ├─ FUN_34047890(entity, ctx)                  animation update (first entity node)
   │     │  ├─ node->vtable+0xd4(ctx)                     ApplyTracks
   │     │  └─ node->vtable+0xd8(ctx)                     link/attachment update
   │     └─ listener notify when finished
   └─ stop every sequence collected in the stop list via movie->vtable+0x54
```

Per-node `ApplyTracks`:

| node type | `+0xd4` | `+0xd8` |
|---|---|---|
| 1 entity (`CAnimEntityNode`) | `0x34042fc0` transform/anim/sound/event/equip/alpha | `0x34040470` link apply |
| 2 scene (`CSceneNode`) | `0x34048150` camera slot/event/console/sound/AVI/UI | `0x3402ab50` no-op |
| 3 camera (`CCameraNode`) | `0x34027d00` camera tracks + shake/focus | `0x3403dfb0` link-position update |

### 1.3 Visible result

* Camera: scene node camera track selects a camera node by name; the camera node evaluates
  `Fov`/`Position`/`Rotation`/`Shake`/`FocusObject`/`Alpha` and pushes the result into one of three
  camera slots (`IMovieSystem+0xd8`, `0x3400bff0`). `IMovieSystem+0xd4` (`0x3400bfd0`) forwards the
  slots to the camera listener registered with `IMovieSystem+0x04` (`0x340207a0`). In the shipped
  client that listener is the x2game camera actor (`FUN_3916bbd0`, see
  [`cutscenes.md`](../port-flowgraph/cutscenes.md) §1.5).
* Entities: `CAnimEntityNode::ApplyTracks` writes position/rotation/scale to the bound entity,
  sets visibility/alpha, sends entity events, starts/stops sounds, plays cutscene animations, and
  applies link/equip.
* Screen: scene node AVI tracks (`ParamId 110`) are played by the movie system's two AVI channels
  and drawn by `IMovieSystem+0x104` (`0x3400d010`); UI/subtitle tracks (`120-123`) are drawn by
  `IMovieSystem+0x108` (`0x3400c170`); text is set through `IMovieSystem+0x110`
  (`0x3400c420`).
* Game side: the x2game listener (`0x3901bcd0`) receives start/stop notifications and drives the
  quest-cinema windows, fades, resurrection restore and login-screen flow; `OnStopMovie`
  `0x39407e40` matches the 11 login/opening names. These are already mapped in
  [`cutscenes.md`](../port-flowgraph/cutscenes.md) §1.6.

### 1.4 Phase relevance

**P5 only.** CryMovie contains nothing that affects riding or on-foot feel. The only P1-adjacent
link is indirect: the x2game camera actor that registers with `IMovieSystem+0x04` is the same object
that implements the quest camera (`FUN_3916bbd0` / `FUN_3916b440`), and that is mapped in
[`port-flowgraph`](../port-flowgraph/MAP.md). Do not start this map before P1/P2/P3.

---

## 2. Object model and vtables

### 2.1 `IMovieSystem` (the movie-system object)

Created by the export `CreateMovieSystem` `0x340053b0` (96 B):

```
CreateMovieSystem(ISystem* sys):
    gEnv = sys->vtable+0x14()                    // GetGlobalEnvironment
    console = sys->vtable+0x288()
    console->vtable+0x18(DAT_34052aac)           // console setup
    obj = operator new(0x21f0)                   // FUN_340048c0
    return CMovieSystem::ctor(obj, sys)          // 0x3400f0c0, 1501 B
```

`crysystem` calls this once at startup (`CSystem::InitMovieSystem`, `0x36552ee0` region) and stores
the result at `CSystem+0x64` (`param_1[0x19]`). Inside crymovie the global environment pointer is
`DAT_34052ab0` and the movie system is at **`gEnv+0x3c`** (`CmdPlaySequence` `0x3400bbf0`). Note:
the survey's E1 table lists movie at `gEnv+0x64`; the `+0x64` value is `CSystem`'s field. Use the
offsets actually read by this module (§10.1).

Object size **0x21f0 bytes**. Fields touched by this module (offset, meaning, evidence):

| offset | type | meaning | evidence |
|---:|---|---|---|
| +0x00 | vtable | `0x3404d1a8` | ctor |
| +0x04 | `ISystem*` | owner system (ctor arg) | ctor |
| +0x08 | `IMovieListener*` | camera/sink listener; set by vtable+0x04 (`0x340207a0`) | `PlaySequence`, `StopSequence`, `ApplyCameraSlots` |
| +0x0c | `void*` | delegated object (getter/setter via vtable+0xc8/0xcc) | `0x3400bf10`, `0x34020800` |
| +0x10/+0x14 | `u64` | last frame time (CTimeValue) | `Update` early-out |
| +0x18 | `int` | time scale? init 1 | ctor |
| +0x1c/+0x20 | vector | sequence registry (`ISequence*`, 4-byte stride) | `FindSequence`, `GetSequence`, ctor |
| +0x2c/+0x30 | map | node registry keyed by node id (`+0x30` is the map sentinel/root; value `node*`); `FindCameraNode` scans it by name | `GetNodeById` `0x3400d8b0`, `FindCameraNode` `0x3400cd20` |
| +0x3c/+0x40 | list + count | active sequences (entry 0x20 B) | `Update`, `PlaySequence`, ctor |
| +0x48/+0x4c | map | listener lists keyed by `ISequence*` | `AddMovieListener` `0x3400f700`, `Notify` `0x3400e080` |
| +0x50 | `int` | init 0 | ctor |
| +0x58 | `u16` | pause flags (low byte used by +0xb0/+0xb4) | `0x340207e0/f0` |
| +0x59 | `u8` | paused (Update early-out) | `Update`, `Pause` `0x3400c530` |
| +0x5a | `u8` | init 1; gates listener calls | `PlaySequence`, `StopSequence` |
| +0x5c | `int` | time scale, init 1 | ctor |
| +0x60 | `u8` | fade enabled, init 1 | `StopSequence` fade path |
| +0x64 | 3×0x28 | camera slots `{id, dirty, pos?, quat?, fov?, …}` (first slot at `+0x64`, dirty byte at `+0x68`) | `SetCameraSlot` `0x3400bff0`, `GetCameraSlotPtr` `0x34020810` |
| +0x124 | 4×`void*` | video/UI texture handles (AVI + UI) | `0x3400c0c0/e0/130` |
| +0x130 | `u8` | generic flag | `0x340207c0/d0` |
| +0x134 | `ISequence*` | "last sequence" (vtable+0x90/0x94) | `0x3400bcd0`, `0x3400bd20` |
| +0x138 | `f32` | time of day saved at PlaySequence | `0x3400c5c0`, PlaySequence |
| +0x13c | `void*` | cinema-object loader (created by vtable+0x130) | `0x3400ca70` |
| +0xe4/+0x104 | 2×0x20 | AVI channel objects (getter `+0xe4`, data setter `+0xe8`) | ctor, `Pause`/`Resume` |
| +0x140 | 4×0x82c | UI/subtitle channels | ctor, `0x3400c420`, `0x3400c170` |

`IMovieSystem` vtable `0x3404d1a8` (all entries read from the image; roles from call sites):

| off | address | role |
|---:|---|---|
| +0x00 | `0x340209a0` | deleting destructor |
| +0x04 | `0x340207a0` | **SetCameraActor/Listener(void\*)** → `movie+8` |
| +0x08 | `0x34029190` | default return |
| +0x0c | `0x3400e6c0` | **CreateNode(type,id,name)** |
| +0x10 | `0x3400e9b0` | **CreateTrack(typeId)** |
| +0x14 | `0x3400e920` | ReplaceNode(id,newId) |
| +0x18 | `0x3400e4e0` | **CreateSequence(name,world)** (allocates 0x74, registers) |
| +0x1c | `0x3400e580` | LoadSequenceFromXmlNode(xmlNode) |
| +0x20 | `0x3400ee40` | DeleteSequence(seq) |
| +0x24 | `0x3400cb50` | **FindSequence(name)** (case-insensitive) |
| +0x28 | `0x3400cba0` | GetSequence(index) |
| +0x2c | `0x3400c630` | FindSequenceByPredicate (uses seq vtable+0xbc) |
| +0x30 | `0x3400cc10` | GetSequences(filter) |
| +0x34 | `0x3400f700` | **AddMovieListener(seq,listener)** |
| +0x38 | `0x3400f780` | RemoveMovieListener(seq) |
| +0x3c | `0x3400ebb0` | sequence index/name helper |
| +0x40 | `0x340207b0` | GetEnv → `movie+4` |
| +0x44 | `0x3400e660` | stop all + clear registry |
| +0x48 | `0x3400e1a0` | stop all + clear active list |
| +0x4c | `0x3400e200` | **PlaySequence(seq, start, flags, fade, startTime, endTime)** |
| +0x50 | `0x3400bd30` | **PlaySequenceByName(name, …)** |
| +0x54 | `0x3400f6a0` | **StopSequence(seq)** (no fade) |
| +0x58 | `0x3400ce90` | **StopSequenceByName(name)** |
| +0x5c | `0x3400f6c0` | StopSequence(seq, fade) |
| +0x60 | `0x3400e160` | **StopAllSequences** |
| +0x64 | `0x3400ce50` | StopSequencesWithFlag8 |
| +0x68 | `0x3400dba0` | StopSequencesWithoutFlag0x800 (used by AbortAll) |
| +0x6c | `0x34020860` | **GetActiveSequenceCount** (`movie+0x40`) |
| +0x70 | `0x3400cef0` | **IsPlaying(seq)** |
| +0x74 | `0x3400bda0` | **IsPlayingAny** (any seq flag 8) |
| +0x78 | `0x340207c0` | GetFlag130 |
| +0x7c | `0x340207d0` | SetFlag130 |
| +0x80 | `0x3400c680` | reserved-name test (`nu_002`,`no_004`) |
| +0x84 | `0x3400bdf0` | IsPlayingBlocking (flags 8/0x20/0x40000) |
| +0x88 | `0x3400da50` | sequence/event helper |
| +0x8c | `0x3400cf20` | sequence/event helper |
| +0x90 | `0x3400bcd0` | play last sequence (`movie+0x134`) |
| +0x94 | `0x3400bd20` | SetLastSequence(ptr) → `movie+0x134` |
| +0x98 | `0x3400cf80` | notify/seek helper |
| +0x9c | `0x3400dcc0` | **Update(dt)** |
| +0xa0 | `0x3400dff0` | per-frame node/registry helper |
| +0xa4 | `0x3400d8b0` | **GetNodeById(id)** |
| +0xa8 | `0x3400cd20` | **FindCameraNode(name)** |
| +0xac | `0x3400cdc0` | FindNodeByParamId(id) (map `+0x30`) |
| +0xb0 | `0x340207e0` | SetPaused(bool) → `movie+0x58` |
| +0xb4 | `0x340207f0` | **GetPaused()** — *not* StopSequence |
| +0xb8 | `0x3400c530` | **Pause** (pauses AVI channels; Update early-outs) |
| +0xbc | `0x3400d4c0` | **Resume** |
| +0xc0 | `0x3400d590` | resume helper |
| +0xc4 | `0x3400d5f0` | resume helper |
| +0xc8 | `0x34020800` | SetInt `movie+0xc` |
| +0xcc | `0x3400bf10` | GetInt from `movie+0xc` |
| +0xd0 | `0x3400c790` | save sequence data to XML |
| +0xd4 | `0x3400bfd0` | **ApplyCameraSlots** → listener `+0x14` |
| +0xd8 | `0x3400bff0` | **SetCameraSlot(index, desc)** |
| +0xdc | `0x3400bf90` | GetCameraSlotIndex(nodeId) |
| +0xe0 | `0x34020810` | GetCameraSlotPtr(index) → `movie+0x64+index*0x28` |
| +0xe4 | `0x34020830` | GetAviChannelPtr(index) → `movie+0xe4+index*0x20` |
| +0xe8 | `0x3400db50` | SetAviChannelData(index, data) (writes `movie+0xe4+index*0x20`) |
| +0xec | `0x3400c0c0` | GetUiTexture(index) |
| +0xf0 | `0x3400c0e0` | LoadUiTexture(index, filename) |
| +0xf4 | `0x3400c130` | ReleaseUiTexture(index) |
| +0xf8 | `0x3400bf30` | GetSequenceTime? (delegates `+0xc`) |
| +0xfc | `0x3400c730` | UI helper |
| +0x100 | `0x3400c9f0` | UI helper |
| +0x104 | `0x3400d010` | **RenderAVI/UI** (draws the 4 screen channels) |
| +0x108 | `0x3400c170` | **RenderUiOverlays** |
| +0x10c | `0x3400c380` | UI helper |
| +0x110 | `0x3400c420` | **SetUiText(channel, text)** |
| +0x114 | `0x3400c510` | ClearUiText (delegates listener) |
| +0x118 | `0x3400d660` | UI helper |
| +0x11c | `0x3400d6b0` | UI helper |
| +0x120 | `0x3400c600` | SetInt `movie+0x12c` |
| +0x124 | `0x3400d730` | music/console helper |
| +0x128 | `0x3400d790` | music/console helper |
| +0x12c | `0x3400c5c0` | **SetMovieStartTimeOfDay** (restores `movie+0x138`) |
| +0x130 | `0x3400ca70` | **LoadCinemaObjects(filename)** ("" unloads) |
| +0x134 | `0x34020850` | GetInt `movie+0x130` |
| +0x138 | `0x3400c5f0` | Get `mov_effect` cvar |
| +0x13c | `0x3400d7e0` | music helper |
| +0x140 | `0x3400e080` | **NotifyListeners(seq, event, param)** |
| +0x144 | `0x3400be70` | camera-pos helper |
| +0x148 | `0x3400bec0` | GetCameraPos? (delegates `+0xc`) |
| +0x14c | `0x3400c6d0` | GetSequenceCameraPos? (delegates `+0xc`) |

**Corrections to `port-flowgraph/structs.md` §7:** `+0x54` is StopSequence (the map's `+0x54`
`0x3400f6a0` is right); `+0x70` is IsPlaying(seq) and `+0x74` IsPlayingAny (right); **`+0x18` is
CreateSequence(name,world), not FindSequence** (FindSequence is `+0x24`); **`+0x2c` is a
find-sequence-by-predicate helper, not GetSequenceFlags** (sequence flags are `ISequence+0x2c`,
i.e. `seq->vtable+0x2c`); **`+0xb4` is GetPaused, not StopSequence**; **`+0xac` is
FindNodeByParamId, not IsPaused** (pause is `+0xb0`/`+0xb4`). The `+0x118` row in that table is a
sequence-vtable offset, not movie-system.

### 2.2 `ISequence` (one cinema)

Allocated 0x74 bytes by `IMovieSystem+0x18` (`0x3400e4e0`), ctor `0x340028a0` (369 B), vtable
`0x3404c490`. Loaded from the two XML files by `ISequence::Load` (`0x34002620`). Fields:

| offset | type | meaning | evidence |
|---:|---|---|---|
| +0x00 | vtable | `0x3404c490` | ctor |
| +0x04 | `int` | refcount | ctor / AddRef |
| +0x08 | `void*` | reserved | ctor |
| +0x0c | `void*` | reserved | ctor |
| +0x10/+0x14 | `u64` | reserved / time | ctor |
| +0x18 | `f32` | reserved (init 1.0) | ctor |
| +0x1c/+0x20 | `void*` | reserved | ctor |
| +0x24/+0x28 | `u8` | +0x28 = loaded flag | `LoadXML` |
| +0x2c | `IXmlNode*` | companion `<Objects>` node (`_cinema_only_objs.xml`) | `Load`, `Unload` |
| +0x30/+0x34 | array | node array, 8-byte entries `{node*, trackContainer*}` | `AddNode`, `LoadXML` |
| +0x38 | `int` | node count helper | `0x340010f0` |
| +0x40 | `std::string` | sequence name | ctor, LoadXML name check |
| +0x44 | `std::string` | world name | ctor, `Load` path build |
| +0x48 | `std::string` | TimeOfDayName | ctor, LoadXML |
| +0x4c | `std::string` | MusicPatternName | ctor, LoadXML |
| +0x50/+0x54 | `f32` | StartTime / EndTime | `SetTimeRange` `0x340020d0` |
| +0x58 | `f32` | current time | `Sequence::Update` |
| +0x5c | `u32` | Flags | `0x34001010/0x34001000` |
| +0x60 | `u16` | state | ctor |
| +0x64 | `u32` | start offset passed to PlaySequence | `0x34001020`, PlaySequence |
| +0x68 | `CSceneNode*` | implicit Scene node | `0x340011a0` |
| +0x6c | `IMovieSystem*` | owner | ctor |
| +0x70 | `u16` | state | ctor |
| +0x71 | `u8` | playing/active | `Unload` |

`ISequence` vtable `0x3404c490` (the entries the client actually calls):

| off | address | role |
|---:|---|---|
| +0x00 | `0x34003f90` | deleting destructor |
| +0x04 | `0x340030d0` | AddRef |
| +0x08 | `0x34003fc0` | Release |
| +0x0c | `0x34001c70` | SetName |
| +0x10 | `0x340010c0` | GetName |
| +0x14 | `0x34003c90` | GetWorld |
| +0x18 | `0x34001c80` | SetTimeOfDayName |
| +0x1c | `0x34001c90` | SetMusicPatternName |
| +0x20 | `0x340010d0` | GetTimeOfDayName |
| +0x24 | `0x340010e0` | GetMusicPatternName |
| +0x28 | `0x34001000` | SetFlags |
| +0x2c | `0x34001010` | **GetFlags** |
| +0x30 | `0x34001020` | SetStartOffset (`seq+0x64`) |
| +0x34 | `0x34001030` | IsPaused? (Update reads it) |
| +0x38 | `0x340010f0` | GetNodeCount |
| +0x3c | `0x34001110` | FindNodeByName |
| +0x40 | `0x34001040` | FindNodeByParamId? |
| +0x44 | `0x34001100` | GetNode(index) |
| +0x48 | `0x34001180` | GetTrackContainer(index) |
| +0x4c | `0x34002d60` | AddNode |
| +0x50 | `0x34002c40` | RemoveNode |
| +0x54 | `0x34002eb0` | node helper |
| +0x58 | `0x340011a0` | CreateSceneNode |
| +0x5c | `0x34002d00` | node helper |
| +0x60 | `0x34001880` | **Unload/Deactivate** (removes companion GUIDs, stops music) |
| +0x64 | `0x34002340` | serialize/helper |
| +0x68 | `0x34002570` | serialize/helper |
| +0x6c | `0x34001e40` | ResetLinks |
| +0x70 | `0x34001eb0` | **Update(SAnimContext\*)** |
| +0x74 | `0x34001ab0` | activate/reset |
| +0x78 | `0x340020d0` | **SetTimeRange(start,end)** |
| +0x7c | `0x34003cd0` | GetTimeRange |
| +0x80 | `0x34001d20` | reset/disable tracks |
| +0x84 | `0x34001de0` | helper |
| +0x88 | `0x34001e10` | helper |
| +0x8c | `0x34001ca0` | IsActive? |
| +0x90 | `0x34001ce0` | stop all tracks |
| +0x94 | `0x34002130` | **ScaleTime** (rescales every key) |
| +0x98 | `0x34001250` | **LoadXML(xml, loadFlag, objectsNode)** |
| +0xa8 | `0x34003d30` | GetRecordOrigin? |
| +0xac | `0x34003ca0` | **IsLoaded** |
| +0xb0 | `0x34003cb0` | SetLoaded |
| +0xb4 | `0x34002620` | **Load()** (builds the two paths and calls LoadXML) |
| +0xb8 | `0x34003d70` | **GetObjectsXmlNode** (AddRef) |
| +0xbc | `0x34003cc0` | predicate used by movie+0x2c |
| +0xc0 | `0x34003ff0` | editor/record helper |

The vtable continues past +0xc0 with per-node/track helpers (`0x34029d10`…); they are not needed
for playback. The first `+0x00..+0xc0` is the port contract.

### 2.3 Node classes

`IMovieSystem+0x0c` (`0x3400e6c0`, 583 B) creates a node by the `Type` attribute:

| Type | class | ctor | size | vtable | shipped use |
|---:|---|---|---:|---|---|
| 1 | `CAnimEntityNode` | `0x34043c60` | 0x1a0 | `0x3404edd0` | **used** (2,309 nodes) |
| 2 | `CSceneNode` | `0x34048aa0` | 0x100 | `0x3404f0e8` | **used** (258 nodes) |
| 3 | `CCameraNode` | `0x34028f80` | 0x248 | `0x3404d9c8` | **used** (405 nodes) |
| 4 | value/event node | `0x3403cfb0` | 0x30 | `0x3404e7e0` | **unused** |
| 5 | value node | `0x34049450` | 0x30 | `0x3404f210` | **unused** |
| 6 | material/shader node | `0x34046e50` | 0x2c | `0x3404efc0` | **unused** |

Node base ctor `0x3402a130` (119 B) sets the common part: vtable, refcount, name (`+0xc`), track
container pointer (`+0x10`, getter `0x34015570` = `node+0x10`). Entity node fields read/written by
playback:

| offset | type | meaning |
|---:|---|---|
| +0x08 | `u32` | flags |
| +0x0c | `std::string` | node name |
| +0x10 | `CTrackContainer*` | track container |
| +0x30/+0x34 | `u64` | `EntityGUID` |
| +0x40 | `int` | DynamicType (0 = NpcType, 1 = GroupUnitText) |
| +0x44 | `std::string` | GroupUnitText / dynamic name |
| +0x48 | `int` | NpcType |
| +0x4c | `std::string` | entity class name |
| +0x50 | `u32` | entity id / 0xffffffff sentinel |
| +0x54 | `void*` | 0x15f0-byte dynamic entity state block |
| +0x68 | `IMovieSystem*` | owner |
| +0x70 | `f32` | last applied time |
| +0x78/+0x80/+0x88 | `f64`×3 | position |
| +0x90 | `f32`×4 | rotation quaternion |
| +0xa0 | `f32`×3 | scale |
| +0xac | `u8` | active |
| +0xb4 | `u8` | visible |
| +0xb8..+0xc4 | `int` | last event/anim ids |
| +0xcc | `u8` | ? |
| +0xd0..+0x164 | 9×0x10 | per-sound-channel slots (sound handle, filename, length) |
| +0x170 | `std::string` | entity name |
| +0x174 | `int` | ? |
| +0x178 | `u8` | ? |
| +0x17c | `int` | ? |
| +0x180 | `int` | ? |
| +0x184 | `f32` | current alpha |
| +0x198 | `int[9]` | disable-track update counters |
| +0x1a0 | `void*` | (camera node) movie system |

Camera node adds `+0x1a4` FOV, `+0x1a8` shake-enabled, `+0x1ac..+0x1fc` shake A/B params
(amplitude/frequency/noise/time offset), `+0x240` last focus key, `+0x244` focus target. Scene node
adds `+0x30` movie system, `+0x38..+0x40` last camera keys, `+0x44` last event key, `+0x48` last
console key, `+0x4c` last music key, `+0x50` last sequence key, `+0x58..+0x60` last AVI/UI keys,
`+0x70..+0x88` sound channels.

Node vtable contract used by playback (`+0x00..+0x118`):

| off | entity | scene | camera | role |
|---:|---|---|---|---|
| +0x14 | `0x340292e0` | `0x340292e0` | `0x340292e0` | GetName |
| +0x18 | `0x34046980` | `0x34026a90` | `0x3403c5e0` | SetSequence/owner? |
| +0x30 | `0x3403d1f0` | `0x34046fe0` | `0x34027340` | reset channels (1,2,4) |
| +0x70 | `0x3403d220` | `0x34008860` | `0x3403d220` | GetMovieSystem? |
| +0x74 | `0x340291b0` | `0x340291b0` | `0x340291b0` | GetOwner |
| +0x78 | `0x34040d30` | `0x34047ae0` | `0x34027370` | Reset(disable) |
| +0x7c | `0x3403ee00` | `0x34047cc0` | `0x3403ee00` | reset |
| +0x84 | `0x3403d3a0` | `0x3402a080` | `0x3403d3a0` | activate |
| +0x88 | `0x3403dfb0` | `0x3402ab60` | `0x3403dfb0` | link/position |
| +0x8c | `0x3403d3c0` | `0x3402ab90` | `0x3403d3c0` | ? |
| +0x90 | `0x3403e1e0` | `0x34030cb0` | `0x3403e1e0` | offset transform |
| +0x98 | `0x3403d710` | `0x34030cb0` | `0x3403d710` | set position |
| +0x9c | `0x3403d7b0` | `0x34030cb0` | `0x3403d7b0` | set rotation |
| +0xa0 | `0x3403d4d0` | `0x34030cb0` | `0x3403d4d0` | set scale |
| +0xd0 | `0x340403f0` | `0x34047320` | `0x340403f0` | look-at |
| +0xd4 | `0x34042fc0` | `0x34048150` | `0x34027d00` | **ApplyTracks** |
| +0xd8 | `0x34040470` | `0x3402ab50` | `0x34040470` | link apply |
| +0xdc | `0x34015570` | `0x34015570` | `0x34015570` | **GetTrackContainer** (`node+0x10`) |
| +0xe0 | `0x3403d550` | `0x3402a040` | `0x3403d550` | set track container |
| +0x104 | `0x34040610` | `0x34029f90` | `0x34027980` | **LoadNodeXml** (attrs) |
| +0x108 | `0x34043a70` | `0x340471c0` | `0x34043a70` | ResetTracks |
| +0x110 | `0x34029180` | `0x34029180` | `0x34029180` | default ret |
| +0x114 | `0x34008860` | `0x34029180` | `0x34008860` | GetEntity (0 for scene) |
| +0x118 | `0x3402ab50` | — | `0x3402ab50` | no-op |

### 2.4 Track classes

The node's track container is at `node+0x10` (`0x34015570`). Its `+0x30` method is the track
loader `0x3402a360` (a generic `{paramId, track*}` array, 8-byte entries). `+0x1c` is the track
factory; the factory is `IMovieSystem+0x10` (`0x3400e9b0`) for the movie system and the
node-type switch in `0x3402a6a0` for the concrete class.

`0x3402a6a0` (983 B) maps a track **type id** to a class. Type ids are *not* ParamIds: the node
registration table maps `name → ParamId → typeId`:

```
entity node (Type 1): FUN_34040b90
  Position 1→0x12, Rotation 2→0x13, Scale 3→1, Visibility 5→3, Event 4→4,
  Sound1..9 7,8,9,124..129→7, Animation1..3 10,11,12→6,
  Expression1..3 14,15,16→8, Facial Sequence 20→0xb, LookAt 21→0xc,
  Link 29→0xf, Equip 30→0x10, Alpha 32→0

scene node (Type 2): FUN_340489d0
  Camera 6→5, Camera2 101→5, Camera3 102→5, Event 4→4,
  Sound1..9 7..9,124..129→7, Sequence 13→13, Console 17→9, Music 18→10,
  Play Origin 31→5, AVI 110→17, AVI2 111→17, UI 120..123→17

camera node (Type 3): FUN_34028eb0
  Position 1→0x12, Rotation 2→0x13, Fov 0→0, Event 4→4,
  FocusObject 100→5, Shake 26→0xe, Link 29→0xf, Alpha 32→0
```

| typeId | class (role) | ctor | obj size | vtable | key size | evaluate | load | save |
|---:|---|---|---:|---|---:|---|---|---|
| 0 | float key track (`Fov`,`Alpha`) | `0x34015510` | 0x14 | `0x3404ce20` | 0x28 | `0x34017f00` | `0x34015740` | `0x34015ad0` |
| 1 | vec3 key track (`Scale`) | `0x340151a0` | 0x1c | `0x3404cf10` | 0x40 | — | `0x34018010` | `0x340183c0` |
| 2 | key container helper | `0x3401df90` | 0x20 | — | — | — | — | — |
| 3 | visibility track | `0x3403c5a0` | 0x30 | `0x3404dfb0` | 8 | node inline | `0x3402c480` | `0x3402c6f0` |
| 4 | event track | `0x3403c560` | 0x30 | `0x3404df10` | 0x8c | `0x34044750` | `0x3402ee40` | `0x34034420` |
| 5 | camera/focus track (`Camera`,`FocusObject`) | `0x3403c5f0` | 0x30 | `0x3404e050` | 0xb0 | `0x34029380` | `0x3402d6a0` | `0x3402d910` |
| 6 | animation track | `0x3403c680` | 0x30 | `0x3404e190` | 0x5c | `0x34041aa0` | `0x3402b0d0` | `0x3402b330` |
| 7 | sound track | `0x3403c630` | 0x30 | `0x3404e0f0` | 0xc4 | `0x340443f0` | `0x3402fa00` | `0x3402fc70` |
| 8 | expression track | `0x3403c6d0` | 0x30 | `0x3404e230` | 0x98 | `0x3403d8f0` | `0x34030fc0` | `0x34031230` |
| 9 | console track | `0x3403c720` | 0x30 | `0x3404e2d0` | 0x48 | `0x34048d50` | `0x34031c60` | `0x34031ec0` |
| 10 | music track | `0x3403c770` | 0x30 | `0x3404e370` | 0x74 | `0x34048f00` | `0x34033080` | `0x340332e0` |
| 0xb | facial-sequence track | `0x3403c7c0` | 0x30 | `0x3404e410` | 0x8c | `0x34042e00` | `0x3402ee40` | `0x34034420` |
| 0xc | look-at track | `0x3403c810` | 0x30 | `0x3404e4b0` | 0x94 | `0x3403ffc0` | `0x340353c0` | `0x34035630` |
| 0xd | sequence track | `0x3403c860` | 0x30 | `0x3404e550` | 0x98 | — | `0x34030fc0` | `0x34031230` |
| 0xe | shake track | `0x34015280` | 0x20 | `0x3404d058` | 0x4c | camera inline | `0x3401ca60` | `0x3401ce00` |
| 0xf | link track | `0x3403c8b0` | 0x30 | `0x3404e5f0` | 0x140 | `0x34042680` | `0x34037480` | `0x340376f0` |
| 0x10 | equip track | `0x3403c900` | 0x30 | `0x3404e690` | 0x14 | `0x34044a90` | `0x34038eb0` | `0x34039110` |
| 0x11 | screen track (`AVI`,`UI`) | `0x3403c950` | 0x30 | `0x3404e730` | 0xb8 | `0x34049230` | `0x3403a360` | `0x3403a5d0` |
| 0x12 | position track | `0x3403ce40` | 0x60 | `0x3404d790` | 0x68 | node inline | `0x3401a3b0` | `0x3401a7a0` |
| 0x13 | rotation track | `0x3403ced0` | 0x40 | `0x3404d828` | 0x4c | node inline | `0x3401e410` | `0x3401e7f0` |

Track object layout (observed): `+0x08` key array begin, `+0x0c` end, `+0x20` last key index,
`+0x24` sort-dirty flag, `+0x28` last evaluated time, `+0x2c` track Flags
(bit 1 = loop, bit 2 = hold; the shipped content uses Flags 0, 2 and 4). The generic track base
vtable (first 0xa0 bytes of each class vtable) provides AddRef/Release, GetKey at `+0x20`,
SetTimeRange at `+0x44`, SortKeys at `+0x3c`, key count at `+0x70`, Load at `+0x90`, Save at
`+0x94`.

---

## 3. The sequence asset (audited against all shipped cinemas)

### 3.1 Files and paths

`ISequence::Load` `0x34002620` (625 B) builds two paths, using the sequence's world name
(`seq+0x44`, set by `CreateSequence(name, world)`) or, if empty, the current level name:

```
Game/worlds/<world>/cinemas/<name>_cinema_only_objs.xml     // companion entities
Game/worlds/<world>/cinemas/<name>.xml                      // the Sequence
Game/worlds/<world>/cinemas/editor/<name>...                // when gEnv+0x2a (editor)
```

The companion file is opened first and its root (`<Objects>`) is stored at `seq+0x2c`; if the
file is missing, a new empty `<Objects>` node is created and `NumObjects=0`. Then the sequence
file is opened and parsed. Failure logs `Failed to load sequence xml file '%s'`
(`0x3404c39c`). The sequence name from the file name is compared with the `Name` attribute; a
mismatch logs `Sequence name mismatch! filename=%s , attribute=` (`0x3404c2b0`) and, in a debug
build, shows a message box.

The pak ships **513** `game/worlds/*/cinemas/*.xml`: **258 sequences** + **255 companion files**,
in 11 worlds (main_world 130, login2 105, instance_prologue 12, instance_library 4, the remaining
7 instances 1 each). Companion files contain **2,530** `<Entity>` definitions. Total sequence
duration is **10,321 s** (0.1 s … 287 s).

### 3.2 Sequence schema

Every shipped sequence has exactly these root attributes (order in file is fixed but irrelevant):

```xml
<Sequence Name="zq_01_wing" MusicPatternName="" TimeOfDayName="" Flags="24"
          StartTime="0" EndTime="12" RecordOriginQuat="1,0,0,0" RecordOriginTrans="0,0,0">
 <Nodes>
  <Node Id="0" Type="2" Name="Scene"> ... </Node>
  <Node Id="-1312470402" Type="1" Name="AnimObject_18" EntityGUID="4bb62830b1c54a7e"> ... </Node>
  <Node Id="-705063836" Type="1" Name="player" EntityGUID="4a6b1c83d5f99464"
        DynamicType="1" GroupUnitText="player"
        Pos="22130.305,8510.4268,478.35461" Rotate="0.57357621,0,0,0.81915218"
        Scale="1.0000001,1.0000001,1"/>
  <Node Id="..." Type="3" Name="Camera_1" EntityGUID="..." FOV="60" ShakeAEnabled="0" ... />
 </Nodes>
</Sequence>
```

* `StartTime` is **0 in all 258 sequences**. `EndTime` is the playback length.
* `TimeOfDayName` and `MusicPatternName` are **empty in all 258 sequences** — the sequence-level
  time-of-day/music features are not used by shipped content. Time of day is changed by a
  console-command track instead (§5.7).
* `RecordOriginQuat`/`RecordOriginTrans` are editor recording data, always present, never read by
  playback. Skip.
* The implicit `Scene` node has `Id="0"`, `Type="2"`, `Name="Scene"`. Every sequence has exactly
  one. The loader creates it on demand (`0x340011a0`) and the first node with `Id=0` is merged
  into it.
* Node `Id` values are editor ids (often negative, hashed); they are only used for
  `IMovieSystem+0xa4 GetNodeById` and link resolution. The playback links by **name**.

Node attribute counts (2,972 nodes):

| attribute | count | meaning |
|---|---:|---|
| `Id`, `Type`, `Name` | 2,972 each | every node |
| `EntityGUID` | 2,714 | all type 1 + type 3 nodes |
| `DynamicType` | 473 | type 1 only; 0 = NpcType, 1 = GroupUnitText |
| `NpcType` | 345 | DynamicType 0 |
| `GroupUnitText` | 128 | DynamicType 1 |
| `Pos`, `Rotate`, `Scale` | 128 each | dynamic type 1 only |
| `FOV` + 16 shake attrs | 405 each | every type 3 camera node |

So the 2,309 entity nodes are **1,836 static level entities** (GUID only) + **345 NPCs** (NpcType)
+ **128 group units** (GroupUnitText). Node types 4/5/6 never appear.

### 3.3 Track and key inventory

10,476 `<Track>` elements; **8,968** have at least one key (22,580 keys total). An empty track is
a disabled track: the player creates it but it evaluates to nothing.

| ParamId | role | tracks | with keys | key attributes used by content |
|---:|---|---:|---:|---|
| 0 | `Fov` (camera node) | 405 | 383 | `time`, `value` |
| 1 | `Position` | 2,714 | 2,452 | `time`, `value`; rare `cont/tens/bias/easefrom/easeto` |
| 2 | `Rotation` | 2,714 | 2,402 | `time`, `value` (quat); rare tangents |
| 3 | `Scale` | 91 | 90 | `time`, `value` (vec3) |
| 4 | `Event` | 2,309 | 1,513 | `time`, `event`, `anim`, `length`, `loop`, `blend`, `speed`, `inplace`, `node`, `boneName`, `offset`, `rot`, `value`, `start` |
| 5 | `Visibility` | 500 | 499 | `time` (key presence toggles) |
| 6 | `Camera` (scene) | 152 | 61 | `time`, `node`, `angle`, `interpolation`, `offset`, `renderRect` |
| 7/8/9 | `Sound1..3` | 156/26/16 | all | `filename`, `volume`, `pan`, `duration`, `desc`, `InRadius`, `OutRadius`, `Stream`, `Is3D`, `Loop`, `Voice`, `LipSync` |
| 10 | `Animation1` | 665 | 660 | `anim`, `length`, `loop`, `speed`, `blend`, `inplace`, `start` |
| 17 | `Console` | 3 | 3 | `command` (`e_time_of_day 03`) |
| 20 | `Facial Sequence` | 50 | 50 | `node` (facial sequence name) |
| 21 | `LookAt` | 1 | 0 | — (disabled) |
| 26 | `Shake` | 62 | 59 | `value` = 4 floats |
| 29 | `Link` | 406 | 402 | `node`, `boneName`, `offset`, `rot` |
| 30 | `Equip` | 25 | 21 | `equipSlot`, `itemType`, `value` |
| 32 | `Alpha` | 32 | 29 | `value` |
| 100 | `FocusObject` | 4 | 4 | `node`, `angle`, `interpolation`, `offset`, `renderRect` |
| 110 | `AVI` | 112 | 112 | `node` (avi path), `subtitleNum`, `renderRect`, `color`, `cutRect`, `outline`, `shadow`, `align` |
| 120 | `UI` (subtitle) | 12 | 12 | same as AVI; `node` is the subtitle text |
| 124-128 | `Sound4..8` | 7/5/5/3/1 | all | same as Sound1-3 |

Track `Flags` are `0` (10,414), `2` (59) and `4` (3). Only animation (52), facial (3), position
(2), link (1), visibility (1) and one camera track use a non-zero flag. Bit 2 is loop-related
(`track+0x2c & 6` in the key lookup), bit 4 the hold/one-shot variant.

**Track types that exist in the class registry but are not used by any shipped sequence:**
`Expression1..3` (ParamId 14-16), `Music` (18), `Sequence` (13), `Play Origin` (31), `Camera2/3`
(101/102), `AVI2` (111), `UI2..4` (121-123), and all material/shader tracks (node types 4/5/6).
They can be skipped (see §9/§12). `LookAt` is present but has no keys, so it is effectively
disabled too.

### 3.4 Sequence flags in shipped content

Bit histogram over 258 sequences:

| bit | count | known/observed meaning |
|---:|---:|---|
| 0x2 | 0 | — |
| 0x4 | 1 | loop (Update resets to loop start) |
| 0x8 | 42 | blocking cinematic: `IsPlayingAny`, `Update` stop path, x2game unit-state rejection |
| 0x10 | 160 | hide level/UI: `ui_draw_level=0` while the engine is paused (line 762) |
| 0x20 | 16 | blocking test in `IsPlayingBlocking` (`0x3400bdf0`) |
| 0x40 | 1 | unknown |
| 0x100 | 1 | unknown |
| 0x200 | 1 | unknown |
| 0x800 | 17 | persistent: not stopped by `StopSequencesWithoutFlag0x800` (AbortAll) |
| 0x4000 | 114 | fade (x2game `TryStartCinema`, `IMovieSystem+0x94` path) |
| 0x8000 | 15 | unknown; login/opening sequences |
| 0x20000 | 124 | unknown; quest cinemas |
| 0x40000 | 118 | blocking test in `IsPlayingBlocking` |

Common values: `0` (81), `24` = 0x18 (23), `16` = 0x10 (5), `34816` = 0x8800 (14),
`34820` = 0x8804 (1), `409616` = 0x64010 (104), `409656` = 0x64038 (5), `393232` = 0x60010 (4),
`393272` = 0x60038 (2), `131096` = 0x20018 (3), `131128` = 0x20038 (4). The login/opening set is
`00_login_s` 0x8804, `01..07` 0x8800, `08_intro` 0x800, `09_ci` 0x810, `10_opening` 0x10,
`op_*` 0x60010.

---

## 4. Playback

### 4.1 Creating and loading a sequence

`IMovieSystem+0x18` `0x3400e4e0` (159 B):

```
seq = new(0x74) CSequence(this, name)        // ctor 0x340028a0
seq->SetWorld(world)                          // seq+0x44
add seq to the registry at movie+0x1c
return seq
```

`IMovieSystem+0x1c` `0x3400e580` (218 B) is the XML-node path used by the editor/savegame: it reads
the `Name` attribute, creates a sequence, parses the XML node with `seq->+0x98`, replaces an
existing sequence of the same name in the registry, and returns the new one.

`ISequence::Load` `0x34002620` (625 B) loads the two files as in §3.1. It is called:

* from x2game `TryStartCinema` (`0x391e2a70`) and `TryStartCinemaNoFade` (`0x39407ff0`);
* from `PlaySequence` itself when `seq->+0xac()` (`IsLoaded`) is false.

It calls `seq->+0x98(xml, 1, &objectsNode)` (`0x34001250`, 1259 B). The load branch:

1. reads the root attributes `Name`, `TimeOfDayName`, `MusicPatternName`, `Flags`, `StartTime`,
   `EndTime`, `RecordOrigin*`;
2. checks the name against `seq+0x40`, logs a mismatch;
3. sets `seq+0x48`/`+0x4c` (time-of-day/music names) and `seq+0x50`/`+0x54` (time range);
4. iterates `<Nodes>` children. For each child: if `Id=0`, use/create the implicit Scene node
   (`0x340011a0`); otherwise read `Type` (int) and `Name`, then call
   `IMovieSystem+0x0c(type, id, name)` (`0x3400e6c0`);
5. calls the node's `+0x104` to read node attributes (entity GUID/dynamic type/camera attrs);
6. gets the node's track container via `seq->+0x48(index)` and calls the container's `+0x30`
   (`0x3402a360`) to load every `<Track>`;
7. sets `seq+0x28 = 1` (loaded).

Track loading (`0x3402a360`, 488 B): for each `<Track>` child, read `ParamId`, look up the type id
through the node's `+0xec` (`0x34029e70`, name→typeId from the registration table), create the
track through the container's `+0x1c` (`0x3402a6a0`), then call the track's `+0x90` loader. The
track loaders read `Flags`, `StartTime`, `EndTime` and each `<Key>` (see §5 for the per-type key
fields). Empty tracks are created and remain valid but never fire.

### 4.2 PlaySequence `0x3400e200` (727 B)

Signature (from call sites):

```
PlaySequence(ISequence* seq, u32 startOffset, u8 fade, f32 fadeIn, f32 startTime, f32 endTime)
```

Steps:

1. if `movie->+0x70(seq)` (`IsPlaying`) is true, return.
2. read the current time-of-day from `gEnv->p3DEngine` (`gEnv+0x2c`) `+0x414` and store it in
   `movie+0x138`; this is the value `mov_back_time` restores.
3. if `seq->+0xac()` is false, call `seq->+0xb4()` (`Load`).
4. if `seq->GetFlags() & 0x8` and `mov_NoCutscenes` (`DAT_34052fc0`) is set, return.
   Inside that flag-8 branch, if `mov_loading` (`DAT_340520d0`) is 1/2 and the sequence has flag
   `0x2000`, show the loading bar (`movie->+0x7c(1)`, `movie->+0x140(seq,0,0)`).
5. read the time range (`seq->+0x7c`) when the start/end arguments are the sentinel
   `DAT_3404cb28` (0.0); `startTime`/`endTime` default to the sequence's own range.
6. `seq->+0x24()` (reset), `seq->+0x60()` (activate/reset tracks), `seq->+0x30(startOffset)`
   (`seq+0x64`); the context struct is filled with `{seq, time=startTime, dt=0, endTime, fade}`.
7. if `movie+8` (camera/listener sink) is set, call its `+0x04(seq, flags)`.
8. add a 0x20-byte entry to the active list at `movie+0x3c`:
   `{seq, ?, loopStart=startTime, endTime, time=startTime, paused=0, ?, lastFrameTime}`.
9. `movie->+0x140(seq, 1, 0xffffffff)` — notify listeners "started".
10. if `gEnv+100` (music) is set, call its `+0x08()`.

`startOffset` is stored at `seq+0x64` and is passed to `seq->+0x30`; the shipped call sites always
pass 0. `fade` is 0 from `TryStartCinema`, 1 for the fade path. The three colour arguments are the
black constant `DAT_399b9e38` in x2game.

### 4.3 Update `0x3400dcc0` (812 B)

1. if `movie+0x59` is set (paused), return.
2. read the frame start time from `gEnv->pTimer` (`gEnv+0x14`) `+0x24`; if the time and frame
   counter are unchanged and `gEnv+0x2a` (dedicated/editor?) is clear, return.
3. iterate the active list (`movie+0x3c`):
   * if the sequence has flag `0x8` and `mov_NoCutscenes` is set, append it to the stop list and
     break out of the loop;
   * if the game is dedicated (`gEnv+0x2a`) and the sequence has flag `0x8`, do not accumulate
     time;
   * otherwise `entry.time += dt`; if the frame time changed, update `entry.lastFrameTime`;
   * if `entry.time >= entry.endTime` and `time != endTime`:
     - flags `& 4` → `entry.time = entry.loopStart` (loop);
     - flags `& 2` → hold, still update;
     - else → append to the stop list and skip the sequence update;
   * call `seq->+0x70(&ctx)` with `ctx = {time, dt, ?, ?, seq}`.
4. for every sequence in the stop list, call `movie->+0x54(seq)` (`StopSequence`).

The active entry layout (observed in Update/PlaySequence) is 0x20 bytes:

| entry offset | meaning |
|---:|---|
| +0x00/+0x04 | list links |
| +0x08 | `ISequence*` |
| +0x0c | loop start (StartTime) |
| +0x10 | end time |
| +0x14 | current time |
| +0x18 | paused/held flag |
| +0x1c | ? |
| +0x20/+0x24 | last frame time (u64) |
### 4.4 Stop, unload, skip

`StopSequence` `0x3400ee90` (556 B):

1. find the sequence in the active list; if absent, return 0.
2. unlink the entry and release it.
3. if `movie+0x60` (fade enabled):
   - `movie+300` is 1 → `seq->+0x7c()` (get time range), `seq->+0x70()` (update);
   - `movie+300` is 2 → same then `seq->+0x64()`.
4. `seq->+0x30()` (clear start offset), `seq->+0x10()` (release?), log `end cinema : %s`
   (`0x3404d2f8`).
5. if the active list is now empty: a console-side object (`gEnv+0x1c` vtable `+0x60`, then
   `+0x14(1)`) is notified, `movie+8` listener `+0x08(seq, flags)` is called, and
   `FUN_3400c960` (music/audio cleanup) runs.
6. if the sequence was in the fade path, `movie->+0x140(seq, 2 or 3, -1)` — notify listeners
   "stopped" (2) or "stopped with fade" (3).
7. `FUN_3400eda0` removes the sequence from the registry if it was a temporary load.

`ISequence::Unload` `0x34001880` (556 B, vtable `+0x60`) runs when the sequence is deactivated:

* for every node: `+0x78(1)` (reset/disable), `+0x108(0)` (reset tracks), `+0xe0(0)` (clear track
  container);
* `FUN_34001790` removes the companion entities: iterate the `<Objects>` node, read each
  `EntityGuid`, find the entity (`gEnv->pEntitySystem` `+0xa4`) and remove it (`+0x4c`);
* build `"anim_sequence_" + seq->name` and call the sound system (`gEnv+0x30`) `+0x3c(str,1)` to
  stop the sequence music;
* `movie->+0x130("")` unloads the companion object file;
* clear `seq+0x71` (playing) and `seq+0x60`.

`StopAllSequences` `0x3400e160` (49 B) stops every active sequence. `StopSequencesWithoutFlag0x800`
`0x3400dba0` (274 B) is the "abort all" used by `Movie.AbortAllSequences`: it collects active
sequences whose flags do **not** have `0x800` and stops them. `StopSequenceByName`
`0x3400ce90` (81 B) finds a sequence by name and calls `+0x54`.

**Skipping.** There is no skip key or skip function inside crymovie. The exposed stop paths are
`IMovieSystem+0x54/0x58/0x5c/0x60`, and the `Movie` Lua table binds them (§7.3). The x2game quest
cinema window and `OnStopMovie` `0x39407e40` handle the game-side consequences (restore
resurrection state, end the login screen, etc.); that is mapped in
[`cutscenes.md`](../port-flowgraph/cutscenes.md) §1.6. Whether the 1.2 UI actually binds a skip
key was not traced here — mark **[?]**.

### 4.5 Pause / resume

`Pause` `0x3400c530` (71 B) sets `movie+0x59 = 1` and calls the two AVI channel objects'
`+0x18(1)` (pause). `Update` early-outs while `movie+0x59` is set, so sequence time freezes too.
`Resume` `0x3400d4c0` (200 B) clears the flag and resumes the AVI channels. The Lua names are
`Movie.PauseSequences` / `Movie.ResumeSequences`, but no shipped `.alb` uses them (§7.3).

### 4.6 Console commands and cvars

Registered by the `CMovieSystem` ctor `0x3400f0c0` (tail, `0x3400f4d0..0x3400f620`):

| name | type | default | help |
|---|---|---:|---|
| `mov_ps` | command `0x3400bbf0` | — | `Play Sequence`; args `<sequence> [world]` |
| `mov_back_time` | command `0x3400bbc0` | — | `Set Movie Start Time Of Day`; if `IsPlayingAny`, `movie+0x12c()` restores `movie+0x138` |
| `mov_effect` | cvar `DAT_340520d4` | 1 | `(0 = off, 1 = on)`; gates the AVI seek/effect path (`seek : %.2f`) |
| `mov_loading` | cvar `DAT_340520d0` | 1 | `loading bar show option (0 = off, 1 = sequence set, 2 = on)` |
| `mov_NoCutscenes` | cvar `DAT_34052fc0` | 0 | `Disable playing of Cut-Scenes`; blocks flag-8 cinemas and the flag-8 update |

`mov_ps` `0x3400bbf0` (222 B): `movie+0x18(name, world)` (create) then
`movie+0x4c(seq,0,0,black...)` (play). It does not load explicitly; PlaySequence loads if needed.
`mov_back_time` `0x3400bbc0` (40 B, not split by Ghidra): if `movie->+0x74()` (`IsPlayingAny`)
then `movie->+0x12c()` (`SetMovieStartTimeOfDay`). Both are debug commands; shipped scripts use
`Movie.PlaySequence` instead.

The sequence console track (ParamId 17) executes arbitrary console commands; the only shipped key
is `e_time_of_day 03` (3 sequences). This is the only time-of-day change in cinemas; the sequence
root's `TimeOfDayName` is always empty.

---

## 5. Track semantics

This section is the behaviour spec for the port: what each shipped track reads, what it writes, and
the exact functions. Every track is evaluated only while its `[StartTime,EndTime]` (track-level, or
the sequence range when absent) contains the sequence time; keys are `time`-sorted.

### 5.1 Entity transform (`ParamId 1/2/3/32/29`)

`CAnimEntityNode::ApplyTracks` `0x34042fc0` (1,902 B) runs before the per-track switch:

```
posTrack  = container->GetTrack(1)
rotTrack  = container->GetTrack(2)
scaleTrack= container->GetTrack(3)
FUN_3403e3a0(node, posTrack, rotTrack, scaleTrack, ctx, entity)   // 0x3403e3a0
node+0x70 = ctx.time
linkTrack = container->GetTrack(0x1d)
if (linkTrack && FUN_34042680(node, linkTrack, ctx))
    FUN_3403e3a0(...)                                             // re-apply after link
for i in 0..2:
    animTrack = container->GetTrack(10+i)
    if (animTrack) FUN_34041aa0(node, animTrack, ctx, i, false)   // 0x34041aa0
```

`FUN_3403e3a0` (2,433 B) samples position/rotation/scale and writes them to the bound entity:
`IEntity` transform (SetPos/Rot/Scale through the entity transform API). Position/rotation keys are `vec3`/`quat`; scale is `vec3`. The key interpolation is the
float-key algorithm (`0x34017dc0`, key size 0x28, cubic Hermite with `bias/cont/tens/easefrom/
easeto`), applied component-wise; quaternion uses slerp in the rotation key container
(`0x3401e...`). Position and rotation tracks have **no** explicit interpolation in almost all keys
(2,452/2,402 keys with only `time`+`value`), so linear/step behaviour is enough for the shipped
content; still port the tangent fields for the 97/82 `cont/tens` keys.

`Alpha` (`ParamId 32`) is evaluated per-key (`container->GetTrack(32)->+0x58(time,&out)`) and
written through `IEntity+0x6c` and the renderer (`gEnv->pRenderer` `+0x18c`), cached in
`node+0x184`.

`Link` (`ParamId 29`) is applied by `FUN_34040470` (122 B) / `FUN_34042680`; it attaches the node's
entity to another node's entity at `boneName` with `offset`/`rot`. The link target is resolved by
name (`IMovieSystem+0xa8`/sequence node lookup). On unload, `FUN_34001e40` resets links.

### 5.2 Camera selection (scene `ParamId 6/101/102`)

`CSceneNode::ApplyTracks` `0x34048150` (1,970 B) collects tracks 6, 101, 102 into three slots and
for each:

```
keyIndex = FUN_34029380(track, time, &key)        // camera key evaluator, key size 0xb0
if (keyIndex != last[slot])
    FUN_34047530(scene, &key, ctx, slot)          // 0x34047530
last[slot] = keyIndex
```

`FUN_34047530` (325 B):

1. `cameraNode = IMovieSystem+0xa8(key.name)` — find a type-3 camera node by name
   (`0x3400cd20`).
2. fill a 0x34-byte camera descriptor from the key (`offset`, `renderRect`, `angle`,
   `interpolation`) and from the camera node (`FOV`, `alpha`, `shake`).
3. if the camera node's camera type (`+0x18`) is 3, read its `+0xcc(0)` (FOV) and `+0xcc(0x20)`
   (alpha) and scale them; this is the "quest camera/fixed" mode (`cameraType == 3`).
4. `movie->+0xd8(&desc, slot)` — store into camera slot 0/1/2 (`0x3400bff0`).
5. `movie->+0xd4(slot)` is called separately (from the movie system frame path) to tell the camera
   listener which slot changed (`0x3400bfd0`); the listener reads the slot through `+0xe0`.

Only `ParamId 6` (slot 0) is used by shipped content; `Camera2/3` are absent. The camera node's
own `Position`/`Rotation`/`Fov` tracks are applied in `CCameraNode::ApplyTracks` `0x34027d00`
(not split by Ghidra; disassembled for this map, `0x34027d00-0x34028eaa` = 4,522 B):

* `Fov` (ParamId 0): `track->+0x6c(time,&fov)`; writes the camera descriptor FOV and the camera
  actor's FOV through the movie-system listener.
* `Alpha` (ParamId 32): same, cached in `node+0x184`.
* `Position`/`Rotation` (1/2): evaluated by `FUN_34025480`/`FUN_34025a40` and combined with the
  link track (`UpdateLinkPos` `0x3403dfb0`).
* `Shake` (ParamId 26): key value is 4 floats; `FUN_3403...` writes the shake A/B amplitude/
  frequency state at `node+0x1a4..+0x1fc`; if changed, calls the node's `+0x14` callback.
* `FocusObject` (ParamId 100): `FUN_34029380` evaluates the target node name; if changed,
  `FUN_34027680` sets the focus; the camera math then builds a look-at rotation and calls
  `container->+0x80(0x8000, &dir)`.
* The camera apply ends by calling `trackContainer->+0x80(0x8000, &lookAt)` (focus/rotation flag)
  and `movie->+0xdc`/`+0xd4` for the slot.

Shake numeric parameters are **node attributes**, not key values: `FOV`, `ShakeAEnabled`,
`AmplitudeA/AMult`, `FrequencyA/AMult`, `NoiseAAmpMult/NoiseAFreqMult`, `TimeOffsetA`, and the B
set. The evaluator uses the timer (`gEnv+0x14`) and a cubic noise function `0x34027270`; the exact
constants are in §10.3. The shipped content has 405 camera nodes with all 16 attributes, 62 Shake
tracks (59 with keys) and 4 FocusObject tracks.
### 5.3 Animation (`ParamId 10/11/12`)

`CAnimEntityNode` handles animation tracks before the switch: `FUN_34041aa0` (`0x34041aa0`,
2,466 B) for each of the three animation channels. Key fields: `anim` (name), `length`, `loop`,
`speed`, `blend`, `inplace`, `start`; key size 0x5c. The animation is played on the bound entity's
`ICharacterInstance`/`AnimObject` (the x2game `AnimObject` entity, or the actor's animation). The
node caches the last animation id/FIFO at `+0xb8..+0xc4`, defers animations while the entity is not
ready (`waiting for deferred animations... %s`), and logs `[cut scene error] not match anim
key=%s FIFO=%s`, `[cut scene error] invalid animation ID! name=%s`, `[cut scene error] No animation
exists!`, `no animation in cinema(%s : %s)`, `reset animation time in cinema(%s : %s)`.
`ca_ignoreCutSceneAnim` (cvar) disables the cutscene animation override.

`FUN_34047890` (`0x34047890`) is called once per frame for the first node with an entity; it
updates the deferred animation queue and the entity's animation time. Port this on top of
`aa-anim`'s actor animation; the animation names are the `machinima_*` and `fist_*` clips in the
content. Unlike the normal actor animation graph (mapped in
[`../port-mount-anim/MAP.md`](../port-mount-anim/MAP.md)), cinema animation is driven directly by
the sequence node's animation tracks, not by graph nodes.

### 5.4 Facial sequence (`ParamId 20`)

50 tracks, key `node` = facial sequence name. `FUN_34042e00` (`0x34042e00`) applies it to the
entity's facial animation (`ca_eyes_procedural`/`ca_FacialAnimationRadius` are the related cvars).
Only 3 tracks have a non-zero track flag (2 or 4), i.e. loop/hold. Port after the core animation
path, or stub it if the facial runtime is not ready.

### 5.5 Event (`ParamId 4`)

2,309 tracks (1,513 with keys). `FUN_34044750` (`0x34044750`) evaluates the key at the current time
and returns the key index + event name; `FUN_3403ee10` (`0x3403ee10`) applies it:

```
strcpy(buf, "Event_"); strcat(buf, key.event);
entity->vtable+0x114(buf);            // SendEvent
```

The key also carries animation fields (`anim`, `length`, `loop`, `blend`, `speed`, `inplace`,
`node`, `boneName`, `offset`, `rot`, `value`, `start`) because the same key structure is shared
with the animation/event tracks; for a pure event only `event` matters. The scene node has its own
event track (`ParamId 4` on scene, 1 track) which sends the event to the scene's entity
(`0x34047680` builds `"Event_" + name` and calls the scene's `+0x114`).

### 5.6 Sound (`ParamId 7/8/9/124..129`)

Entity and scene sound tracks use key size 0xc4:
`filename[0x80]`, `volume` (+0x88), `pan` (+0x8c), `InRadius` (+0x90), `OutRadius` (+0x94),
`Stream` (+0x98), `Is3D` (+0x99), `Loop` (+0x9a), `Voice` (+0x9b), `LipSync` (+0x9c), `duration`
(+0xa0), `desc` (+0xa4).

Entity apply (`FUN_340443f0` evaluate + `FUN_3403f110`/`FUN_34047cd0` apply):

* create the sound through `gEnv->pSoundSystem` (`gEnv+0x30`) `+0x4c(filename, flags|0x8000001a)`;
* flags: `Loop` → 1, `Voice` → `|0x200000`; playback flags `0x2000000` (2D) or `0x2000200`
  (voice/lip-sync);
* set volume (`sound+0xb8` vtable `+0xc0`), pan (`+0xf8`), 3D radius (`+0xe8(0xe6)`), and seek
  with `(currentTime-keyTime)*1000` via `+0xe0(ms,1)`;
* if the sound is a voice and not playing, set the `gender` parameter and start it
  (`+0x20(0,1,1,0)`);
* `FUN_3400c580` notifies the movie listener for flag-8 sequences.

Scene apply uses the same key layout and `FUN_34047cd0` (`0x34047cd0`) with 9 sound channels; the
channel index is `FUN_34026c80(paramId)` = `0..8` (`7..9`→0..2, `124..129`→3..8).
### 5.7 Console (`ParamId 17`) and time of day

3 tracks, one key each: `command="e_time_of_day 03"`. `FUN_34048d50` (`0x34048d50`) evaluates the
key; the scene apply executes the command through the console
(`gEnv->pConsole` (`gEnv+0x1c`) `+0x94(command,0,0)`). This is the only time-of-day change in
shipped cinemas. The movie system saves the time of day at playback start (`movie+0x138`) and
`mov_back_time` restores it; there is no automatic restore on stop.

### 5.8 AVI (`ParamId 110`) and UI/subtitle (`ParamId 120..123`)

Screen track keys (size 0xb8): `node` (AVI path for 110, subtitle text for 120-123),
`subtitleNum`, `renderRect` (4 floats), `color` (4 floats), `cutRect`, `outline`, `shadow`,
`align` (default 4).

* Scene apply `0x34048150` evaluates with `FUN_34049230`; for AVI it calls `FUN_34047ff0`, which
  stores the path + rect through `IMovieSystem+0xe8` into the AVI channel at `movie+0xe4+channel*0x20`.
* UI text is set with `movie->+0x110(channel, text)` (`0x3400c420`), which stores the text and
  loads the font/texture (`gEnv->pRenderer` `+0x23c`); `movie->+0x114` clears it.
* `movie->+0x104` (`0x3400d010`) draws the 4 channels each frame: `gEnv->pRenderer` `+0x3c8`
  draws the quad; the `seek : %.2f` log and the AVI seek path are gated by `mov_effect`.
* `movie->+0x108` (`0x3400c170`) draws the subtitle/UI overlays (4 × 0x82c-byte channels at
  `movie+0x140`).

The 1.2 content uses AVI 112 tracks (mostly `game/objects/machinima/avi/*.avi`) and UI 12 tracks
(subtitles). The x2game cinematic screen/fader (`FUN_393f6fc0`, 8 channels) is a separate system
mapped in [`cutscenes.md`](../port-flowgraph/cutscenes.md) §1.7; do not merge the two.

### 5.9 Equip (`ParamId 30`) and visibility (`ParamId 5`)

* Equip: 25 tracks, keys `equipSlot`/`itemType`. `FUN_34044a90` evaluates the key and returns an
  equip id; `FUN_3403d840` applies it to the entity's equipment. 21 tracks have keys.
* Visibility: 500 tracks, 499 with keys. The key has only `time`; presence toggles the entity's
  hidden flag. Entity apply case 5 calls the entity's `+0xc4(hidden)` and caches `node+0xb4`.
  Scene/node visibility uses the same flag; the x2game `FUN_39354520` writes the same
  `unit+0x3c5a` byte as stealth (see [`cutscenes.md`](../port-flowgraph/cutscenes.md) §2.3).

### 5.10 Unused tracks and nodes

Do **not** port (no shipped sequence uses them):

* `Expression1..3` (14-16) — `0x3403c6d0`, `0x3403d8f0`, key 0x98;
* `Music` (18) — `0x3403c770`, `0x34048f00`, key 0x74;
* `Sequence` (13) — `0x3403c860`, key 0x98;
* `Play Origin` (31) — scene track, no keys;
* `Camera2/3` (101/102), `AVI2` (111), `UI2..4` (121-123) — the classes are shared, so keep the
  generic path but no content exercises them;
* `LookAt` (21) — 1 track, 0 keys;
* node types 4/5/6 and the material/shader tracks (`Opacity/Ambient/Diffuse/Specular/Emission/
  Shininess/Shader Param 1..9`, `0x34046e50` and `0x34047100..0x34049a80`) — 53 functions,
  10.7 KB.

`Facial Sequence` (20) is used (50 tracks) but is a thin adapter; port it after the core.

---

## 6. Entity binding and hidden cinematic actors

### 6.1 Static level entities

A type-1 node with `EntityGUID` (no `DynamicType`) binds to an entity that already exists in the
level. `CAnimEntityNode::LoadNodeXml` `0x34040610` reads `EntityGUID` (`+0x30/+0x34`), `Rotate`
(`+0x90`), `Scale` (`+0xa0`); `FUN_34040710` compares the parsed GUID with the previous one and logs
`diff GUID(%s) <%llu -> %llu>` / `diff GUID(%s) <%d -> %d>` on mismatch. The node caches the GUID
and resolves the entity lazily through `gEnv->pEntitySystem` (`gEnv+0x4`) `+0x168(guid)`
(`GetEntityByGuid`), then checks the entity's model/name and logs
`[Movie] Failed to find object(entity, unit, ...)!!! its name is '%s'` or
`CAnimEntityNode::GetEntity() : diff cryEntity(%s)`.

Camera nodes are also bound to a level entity by GUID; the camera actor reads the entity transform
to build the camera pose. In the shipped content all 405 camera nodes have a GUID.

### 6.2 Dynamic entities

A type-1 node with `DynamicType`:

| DynamicType | attribute | binding |
|---:|---|---|
| 0 | `NpcType` (int) | NPC model/prefab: `CAnimEntityNode::Activate` `0x3403da80` resolves the NPC type and creates the entity through `gEnv->pGame` (`gEnv+0x44`) `+0x2c` (spawn) and `+0x170`/`+0x174` (register); the 1.2 server-side `npc`/`models` tables supply the model |
| 1 | `GroupUnitText` (string) | find an existing unit by group text (`gEnv->pGame` `+0x3c` → unit registry, `+0x1b8` name); if found, reuse it; otherwise spawn a `prefab_unit` |

`FUN_34045360` (341 B) parses `DynamicType`/`NpcType`/`GroupUnitText` on load and saves them on save;
unknown values log `Unknown dynamic type!(%d) on LOAD` / `... on SAVE`. The strings `prefab_unit`
and `prefab_doodad` exist in the module but have no direct xref in the decompile; the actual spawn
goes through the game framework entity-class API. `CAnimEntityNode::Activate` also applies the
node's `Pos`/`Rotate`/`Scale` to the spawned entity, sets the entity flags `0x200102`, and calls the
game framework's `+0x170`/`+0x174` to register it. `FUN_34044210` builds the entity descriptor
(model/class/name), and `FUN_3400be50` notifies the game framework.

Dynamic entities are removed on unload through the same GUID/entity removal path as static ones
(`FUN_34001790` / `FUN_3400eda0`).

### 6.3 Companion `_cinema_only_objs.xml` (hidden actors and props)

The companion file is a standard CryEngine entity-layer XML:

```xml
<Objects NumObjects="10">
 <Entity Name="female_test" Pos="..." EntityClass="npc_hariharan_male" EntityId="6626044"
         EntityGuid="4eead2991ea4f195" Layer="Main" NetworkOption="ClientOnly">
   <Properties .../>
 </Entity>
 ...
</Objects>
```

The 255 shipped companion files contain 2,530 entities (NPCs, `AnimObject`s, `ParticleEffect`s,
`ProximityTrigger`s, lights). The sequence loader stores the `<Objects>` node at `seq+0x2c`; on
unload `ISequence::Unload` removes every listed `EntityGuid` from the level
(`FUN_34001790`), so the actors/props exist only for the duration of the cinema.

**Open:** the actual spawn of the companion entities is not in `crymovie`. The movie system can
load/unload a companion file through `IMovieSystem+0x130` (`0x3400ca70`, `LoadCinemaObjects`), which
opens the XML with `gEnv->pSystem` `+0x1a4`, creates an object loader via
`gEnv[0xb]->+0x410` and calls its `+0x2c(xml,1)`; `Unload` calls it with `""`. No shipped caller of
`+0x130` with a non-empty path was found in `x2game` or `crymovie`; either the level system loads
the companion layer, or the game framework calls `+0x130` from a path not in these decompiles.
Mark **[?]** and test with a live quest cinema.

### 6.4 What the port needs

1. A `CinemaEntityBinding` component per type-1 node: `EntityGUID` (static) or
   `DynamicType/NpcType/GroupUnitText` (dynamic), resolved against `aa-world` actors; keep the
   node's `Pos/Rotate/Scale` as the fallback pose.
2. A `CinemaObjects` list from the companion file; on start, spawn/resolve the entities; on stop,
   despawn/remove them. Reuse `aa-world`'s entity layer loader instead of porting the CryEngine
   XML entity format.
3. The 2,530 companion entities are ordinary level entities, so the port should feed them through
   the same spawn path as the level (not a second entity system).
---

## 7. Hand-over to x2game and the `Movie` Lua binding

### 7.1 The x2game start path (link)

The game-side entry points are mapped in
[`port-flowgraph/cutscenes.md`](../port-flowgraph/cutscenes.md) §1.1-1.2 and are not repeated here:

* `SCPlaySequence` (opcode 0x13b) → `OnPlaySequence` `0x391e4250` (129 B) →
  `TryStartCinema` `0x391e2a70` (524 B);
* `TryStartCinemaNoFade` `0x39407ff0` (name, world);
* `OnStopMovie` `0x39407e40` and the 11-name login table at `0x39ac9150`;
* the movie system pointer is the game framework's `+0x3c` (`DAT_3a5bb1d8+0x3c`), the same object
  as `gEnv+0x3c` in crymovie.

What this map adds:

* `TryStartCinema` sends **`CSStartedCinemaPacket` 0xcf** immediately before adding the listener
  (`local_1c = 0xcf`, `FUN_39428370`/`thunk_FUN_3ac1058c`). This is the client sender that
  `cutscenes.md` §6 marked as not located.
* `CSCompletedCinemaPacket` **0xce** is sent from `FUN_39017620` (`FUN_39428370(0xce)`); it is not
  in `OnStopMovie` itself. The exact trigger of `FUN_39017620` was not traced — mark **[?]**.
* `TryStartCinemaNoFade` calls `movie->+0x60()` (`StopAllSequences`) before playing, so a login
  transition never overlaps the previous cinema. `TryStartCinema` does not.
* The normal start listener is the global object at `0x39ac30a0` (vtable `0x3999d09c`, callback
  `0x3901bcd0`); the no-fade start uses a different listener global at `0x39ac935c`. The
  `IMovieListener` contract is §7.2.

### 7.2 `IMovieListener` contract

`IMovieSystem+0x34` (`0x3400f700`, 115 B) registers a listener for one sequence in the map at
`movie+0x48`; `+0x38` (`0x3400f780`, 88 B) removes it. `NotifyListeners` (`0x3400e080`, 219 B)
walks the list and calls the listener's first vtable entry:

```
listener->vtable[0](ISequence* seq, int event, int param)
```

Events emitted by crymovie:

| event | emitted from | meaning |
|---:|---|---|
| 1 | `PlaySequence` `0x3400e200` | started |
| 2 | `StopSequence` `0x3400ee90` | stopped |
| 3 | `StopSequence` fade path | stopped with fade |
| 0 | `PlaySequence` loading-bar path | loading/hold |

The x2game listener `0x3901bcd0` switches on events `3,4,6,7,8,0x15` and drives the quest-cinema
UI/state; events `4..0x15` are not emitted by the crymovie code above, so either they come from a
different notification source or the decompiled switch is shared with another interface. Mark
**[?]**; the port only needs 1/2/3 for the sequence lifecycle and can add game-specific events
later.

The **camera listener** is separate: it is stored at `movie+8` by `IMovieSystem+0x04`
(`0x340207a0`). The movie system calls:

* `listener->+0x04(seq, flags)` when a sequence starts (`PlaySequence`);
* `listener->+0x08(seq, flags)` when the active list becomes empty (`StopSequence`);
* `listener->+0x14(slotIndex)` from `IMovieSystem+0xd4` (`0x3400bfd0`) to tell the camera
  listener which of the three camera slots changed; the listener reads the slot through
  `IMovieSystem+0xe0` (`GetCameraSlotPtr`, `0x34020810`).

In the shipped client that listener is the x2game camera actor (`FUN_3916bbd0`, 158 B, object
0x368 bytes, vtable `0x399b0a90`), which is also the quest-camera actor. Its `+0x14` receives the
camera descriptor built by the camera track; see
[`cutscenes.md`](../port-flowgraph/cutscenes.md) §1.5 and §2.3. The port's Bevy camera should
implement the same three methods.

### 7.3 The `Movie` Lua table (`cryscriptsystem`)

Constructor `FUN_35514d50` (`0x35514d50`, 834 B) creates a Lua table named `"Movie"` on the script
system's state. Object fields: `+0x44` Lua state (`scriptSystem->vtable+0x30(0)`), `+0x48` script
system, `+0x4c` table ref, `+0x50` game framework, `+0x54` movie system
(`gameFramework->vtable+0xe0()`). The table is only built when the script-system mode
(`scriptSystem->vtable+8()`) is not 3.

| Lua name | binder | movie-system call |
|---|---|---|
| `Movie.PlaySequence(sSequenceName)` | `0x35514be0` (100 B) | `IMovieSystem+0x50` `0x3400bd30` PlaySequenceByName(name,0,0,black…) |
| `Movie.StopSequence(sSequenceName)` | `0x35514ad0` (37 B) | `IMovieSystem+0x58` `0x3400ce90` StopSequenceByName(name) |
| `Movie.StopAllSequences()` | `0x35514b00` (33 B) | `IMovieSystem+0x60` `0x3400e160` |
| `Movie.AbortAllSequences()` | `0x35514b30` (75 B) | if gameFramework `+0x14c` then movie `+0x68` `0x3400dba0` (stop non-persistent) |
| `Movie.PauseSequences()` | `0x35514b80` (36 B) | `IMovieSystem+0xb8` `0x3400c530` |
| `Movie.ResumeSequences()` | `0x35514bb0` (36 B) | `IMovieSystem+0xbc` `0x3400d4c0` |
| `Movie.GetNumOfPlayingCutscenes()` | `0x35514c70` (216 B) | `IMovieSystem+0x6c` `0x34020860` (`movie+0x40`) |

`PlaySequence` uses the **by-name** path, which only finds an existing sequence; it does not create
or load one. This is why a script trigger works only if the sequence was already created (e.g. by
`SCPlaySequence` or a level preload). `GetNumOfPlayingCutscenes` returns the active-list count.

Usage audit over all 961 shipped `.alb` scripts (string scan):

| method | files |
|---|---|
| `PlaySequence` | 5: `entities/triggers/cinematictrigger.alb`, `delaytrigger.alb`, `multipletrigger.alb`, `proximitytrigger.alb`, `x2ui/hud/gm_console_view.alb` |
| `GetNumOfPlayingCutscenes` | 1: `x2ui/centermessage/effect_message.alb` |
| `StopSequence`, `StopAllSequences`, `AbortAllSequences`, `PauseSequences`, `ResumeSequences` | **0** |

So the only shipped runtime use of the Lua binding is the entity-trigger path
(`Movie.PlaySequence(name)`) and one HUD effect query; the stop/pause methods are debug/utility
bindings. The port must implement `PlaySequence` and `GetNumOfPlayingCutscenes` for content
parity; the rest can be thin wrappers over the scheduler.

### 7.4 Quest camera, unit-state rejection, login flow (link)

These are **not** CryMovie features and remain in the flowgraph map:

* quest camera (`X2Quest:SetUseQuestCamera`, `FUN_39164ff0`, `FUN_3916b440`) — a dialogue camera
  implemented by the same x2game camera actor, [`cutscenes.md`](../port-flowgraph/cutscenes.md) §2;
* `SCUnitState` rejection while a flag-8 sequence plays (`OnUnitState` `0x391d4c30`) —
  [`cutscenes.md`](../port-flowgraph/cutscenes.md) §6;
* login/world-select/character-select/opening sequences and `OnStopMovie` — same §1.6;
* the cinematic screen fader/letterbox (`FUN_393f6fc0`) — same §1.7.

The port must keep the two camera systems separate: the normal player camera, the quest camera
(dialogue) and the movie camera actor all exist at the same time, and the movie camera only wins
while a sequence with a camera track is active.
---

## 8. In-process harness and difftest

### 8.1 Reaching the objects in the hidden 1.2 client

The hidden client exposes the movie system through two equivalent pointers:

* x2game: `*(IMovieSystem**)(DAT_3a5bb1d8 + 0x3c)` (game framework `+0x3c`);
* crymovie: `*(IMovieSystem**)(DAT_34052ab0 + 0x3c)` (`gEnv+0x3c`), where `DAT_34052ab0` is the
  global environment returned by `ISystem::GetGlobalEnvironment()`.

The object is the 0x21f0-byte `CMovieSystem` at vtable `0x3404d1a8`. All calls are `__thiscall`
(ECX = object, args on the stack, callee pops). `aadriver` can call them by address with the
`__thiscall` convention; no C++ symbol is needed.

### 8.2 Alloc / call / read recipes

| goal | recipe |
|---|---|
| create a sequence | `movie+0x18(name, world)` — allocates 0x74 bytes in the game heap and registers it |
| load a sequence | `seq+0xb4()` — reads the two XML files; allocates nodes (0x1a0/0x100/0x248), track containers and key arrays in the game heap |
| play | `movie+0x4c(seq, 0, 0, black, black, black)` |
| update | `movie+0x9c(dt)`; for determinism call `seq+0x70(&ctx)` directly with a synthetic context |
| stop | `movie+0x54(seq)` (no fade) or `movie+0x5c(seq, fade)` |
| pause/resume | `movie+0xb8()` / `movie+0xbc()` |
| find | `movie+0x24(name)` / `movie+0x28(index)` / `movie+0x6c()` count |
| read a sequence | read 0x74 bytes at the `ISequence*`: name/world/time range/flags/node array |
| read a node | read 0x1a0/0x100/0x248 bytes at the node pointer from `seq+0x30 + i*8` |
| read a track | `node->vtable+0xdc()` returns the track container; container `+0x08`/`+0x0c` are the `{paramId, track*}` array bounds |
| read keys | track `+0x08`/`+0x0c` are the key array bounds; key size from §2.4 |
| read camera slots | `movie+0x64 + slot*0x28`, 0x28 bytes per slot |
| read the active list | `movie+0x3c` (list sentinel), entries 0x20 bytes |
| call a listener | not needed; register a test listener with `movie+0x34(seq, listener)` and read its callbacks |

Allocation is inside the original methods, so the harness never needs to allocate game objects
itself. Free the sequence with `movie+0x20(seq)` (`DeleteSequence`) after the test.

### 8.3 Difftest plan

1. **Asset parity (leaf).** For every one of the 258 sequences, `CreateSequence(name, world)` +
   `Load()` and dump: node count/types/names/GUIDs, track count/ParamId/Flags, key count and key
   fields. Compare with the Rust XML reader. Expected totals: 2,972 nodes, 10,476 tracks, 22,580
   keys, 2,309/405/258 node types.
2. **Evaluator parity.** For each used track type, build or load a sequence with that track, call
   the evaluator at `t = 0, mid, end, before start, after end` and compare with the Rust evaluator.
   Start with the float track (`0x34017dc0`), camera key (`0x34029380`), event (`0x34044750`),
   sound (`0x340443f0`), screen (`0x34049230`), anim (`0x34041aa0`).
3. **Frame parity.** Play `game/worlds/main_world/cinemas/zq_01_wing.xml` (12 s) at a fixed
   `dt=1/60` by calling `seq+0x70` with a synthetic context; sample every frame: camera slot
   descriptor, entity position/rotation/scale, visibility/alpha, sound events. Repeat for
   `sl_el_001.xml` (85.7 s, AVI/subtitle/AreaTrigger) and `id_300_01.xml` (184 s, console/time of
   day).
4. **Lifecycle parity.** Start/stop/start again; stop during fade; pause/resume; loop flag (the
   52 animation tracks with Flags=2); hold flag (Flags=4); `StopAllSequences`;
   `StopSequencesWithoutFlag0x800`.
5. **Camera parity.** Compare the 0x28-byte camera slot and the camera actor pose (position,
   rotation, FOV) against the Bevy camera. Include the shake evaluator (`0x34027270`) at fixed
   timer values.
6. **Audio parity.** Hook `gEnv->pSoundSystem` `+0x4c` and log `(filename, flags)`; compare with
   the port's audio events. Hook `+0x3c` for the sequence music stop.
7. **Lua parity.** From a test script, call `Movie.PlaySequence(name)` and
   `Movie.GetNumOfPlayingCutscenes()`; verify the by-name path and the count.
8. **Companion objects.** For `instance_library/cinemas/id_303_01` (ProximityTrigger +
   ParticleEffect companion), verify the GUIDs are removed on unload.

### 8.4 Harness caveats

* `Update` uses the engine timer and `gEnv+0x2a`; do not drive it in a unit test. Call
  `ISequence::Update` directly with a synthetic `SAnimContext`.
* A sequence loaded with a world that is not the current level still parses (the paths use the
  sequence's world string), but entity binding only works if the level entities exist. Use
  `main_world` for the camera/entity tests.
* The sound system and renderer may be absent in a headless harness; guard `gEnv+0x30`/`+0xc`.
* The camera listener only exists in the game; `movie+8` is null in a bare engine harness.
  Register a test listener with `movie+0x04` to capture the camera slots.
* `LoadCinemaObjects` (`movie+0x130`) needs `gEnv[0xb]` (entity loader); if null, it skips.

---

## 9. Port order (testable leaves first)

Phase 1 (riding/on-foot) needs **nothing** from this map. The P5 order:

1. **Data model + XML reader (Rust).** `Sequence`, `Node` (type 1/2/3), `Track`, `Key`; parse the
   513 files; assert the audit totals in §3. No engine code. This is the first deliverable and
   immediately testable.
2. **Key containers/evaluators.** Float (`0x34017dc0`), vec3, quat, visibility, event, camera,
   sound, anim, screen, equip, link, alpha, position, rotation. Port the interpolation flags
   (`bias/cont/tens/easefrom/easeto`, loop/hold) because 97 position and 82 rotation keys use them.
   Unit-test each against the original evaluator (§8.3 step 2).
3. **Sequence object.** Node registry, find by name/id, time range, flags, load/unload, companion
   `<Objects>` node, `ScaleTime`. Test the 258 loads and the name-mismatch path.
4. **Movie scheduler.** Registry, create/find/delete, play/stop/stop-all/pause/resume, active-list
   time accumulation, loop/hold, listener notify, cvars (`mov_*`). This is the first playable
   skeleton; test with a no-camera sequence.
5. **Scene apply.** Camera slot selection, event, console/time-of-day, sound, AVI/UI/subtitle.
6. **Camera apply + Bevy camera.** FOV, position/rotation, shake, focus/look-at, link; feed the
   camera actor equivalent. This is where the cutscene becomes visible.
7. **Entity apply.** Transform, visibility/alpha, animation (`aa-anim`), sound (`aa-audio`),
   equip, link; dynamic entities (NpcType/GroupUnitText) and companion spawn/despawn (`aa-world`).
8. **Game hand-over.** `SCPlaySequence` → create/load/play, listener, fade, login name table,
   `CSStarted/CompletedCinemaPacket`, `Movie` Lua table, unit-state rejection.
9. **Skip/drop.** Editor save/record, `RecordOrigin*`, material/shader nodes, expression/music/
   sequence/play-origin tracks, `Camera2/3`/`AVI2`/`UI2..4`, generic C++ containers and the XML
   value parser. Keep the data model tolerant of unknown attributes.

Testable leaves at each step: the XML reader and every evaluator are pure functions; the sequence
load and scheduler are stateful but deterministic with a synthetic context; only steps 6-8 need a
live Bevy world.
---

## 10. Numeric parameters

### 10.1 Environment (`gEnv`) offsets read by crymovie

`DAT_34052ab0` is the global environment returned by `ISystem::GetGlobalEnvironment()`. Offsets
actually read by this module (the survey's E1 table is 0x28 higher for several of these; use these
values when driving crymovie):

| offset | object | used for |
|---:|---|---|
| +0x04 | `IEntitySystem` | `+0x168(guid)` find entity, `+0x154` find class, `+0x1b8` name, `+0x170/+0x174` register, `+0x1ec` actor, `+0x250` texture? |
| +0x0c | `IRenderer` | `+0x23c` create texture, `+0x334` set 2D mode, `+0xf4` draw, `+0x3c8` draw AVI/UI quad, `+0x18c` alpha |
| +0x14 | `ITimer` | `+0x24` frame start time, `+0x18` seconds, `+0x10` frame counter |
| +0x1c | `IConsole` | `+0x24` register cvar, `+0x88` register command, `+0x94` execute command, `+0x60` get cvar |
| +0x2a | `u8` | system flag (dedicated/editor) used by Update/Load |
| +0x2c | `I3DEngine` | `+0x414` time-of-day object (`+0x18` get, `+0x14` set), `+0x410` entity loader |
| +0x30 | `ISoundSystem` | `+0x4c` create sound, `+0x3c` stop by name, `+0x5c` set listener |
| +0x3c | `IMovieSystem` | this module |
| +0x44 | `IGame`/game framework | `+0x2c` spawn entity, `+0x3c` unit registry, `+0x94` remove, `+0xa4` find by GUID, `+0x170/+0x174` register |
| +0x64 | cinema music/audio object (identity **[?]**) | `+0x08` on play/stop |
| +0xa8/+0xa9 | `u8` | dedicated / editor flags (used by sound/AVI paths) |

### 10.2 Sequence flags

See §3.4 for the bit histogram. Playback uses:

| bit | effect |
|---:|---|
| 0x2 | hold at end (Update) |
| 0x4 | loop to loopStart (Update) |
| 0x8 | blocking cinematic; `IsPlayingAny`, unit-state rejection, `mov_NoCutscenes` |
| 0x10 | hide level/UI while paused |
| 0x20 | blocking test in `IsPlayingBlocking` |
| 0x800 | persistent; not stopped by AbortAll |
| 0x2000 | loading-bar path when `mov_loading != 0` |
| 0x4000 | fade (x2game start path) |
| 0x40000 | blocking test in `IsPlayingBlocking` |

### 10.3 Camera, shake and focus constants

| address | value | use |
|---|---:|---|
| `0x3404cc70` | 60.0 | default camera FOV (camera ctor `+0x1a4`) |
| `0x3404cc00` | 75.0° (1.30899692 rad) | scene camera default angle |
| `0x3404dc90` | π/180 (0.0174532924) | degrees → radians for FOV/angle |
| `0x3404cc90` | 0.0 | sentinel |
| `0x3404c264` | 1.0 | scale / identity |
| `0x3404c488` | -1.0 | "not set" |
| `0x3404cbf4` | 0.5 | 2D center |
| `0x3404cbf8` | 600.0 | UI/AVI design width |
| `0x3404cbfc` | 800.0 | UI/AVI design height |
| `0x3404cc94` | 16/9 (1.7777778) | aspect for AVI/UI rect |
| `0x3404cc98` | 9/16 (0.5625) | inverse aspect |
| `0x3404cc9c` | 16.0 | AVI width scale |
| `0x3404cca0` | 9.0 | AVI height scale |
| `0x3404cd40` | 1.5 | shake direction scale |
| `0x3404cd44` | 2.0 | shake angle scale |
| `0x3404dc40` | 0.70710677 | 45° / sqrt(2) |
| `0x3404db30` | 1.17549435e-38 (FLT_MIN) | rsqrt epsilon |
| `0x3404db34` | 1e-6 | shake normalization epsilon |
| `0x3404eaf4` | 1000.0 | seconds → milliseconds (sound seek) |
| `0x3404eaf8` | 0.001 | milliseconds → seconds |
| `0x3404d9c0` | 2^23 (8388608) | float→int trick in the noise evaluator |
| `0x34027270` | — | cubic noise evaluator (shake) |

Shake node attributes: `FOV` (60 in all shipped cameras), `ShakeAEnabled`/`ShakeBEnabled`,
`AmplitudeA/B` (vec3, default 1,1,1), `AmplitudeAMult/BMult` (0), `FrequencyA/B` (vec3, default
1,1,1), `FrequencyAMult/BMult` (0), `NoiseAAmpMult/NoiseAFreqMult` (0), `TimeOffsetA/B` (0). The
evaluator scales the A/B amplitude/frequency and adds two cubic-noise terms; the B set is the
secondary shake. The camera FOV is stored in the node and multiplied by π/180 when applied.

### 10.4 Sound flags and key strides

| value | meaning |
|---:|---|
| `0x8000001a` | base create flags |
| `0x1` | loop (key `Loop`) |
| `0x200000` | voice (key `Voice`) |
| `0x2000000` | 2D playback (not voice) |
| `0x2000200` | voice/lip-sync playback |
| `0xe6` | 3D radius parameter id |
| key size 0xc4 | sound track key |
| `(currentTime-keyTime)*1000` | seek position (ms) |

### 10.5 Object sizes and strides

| object | size / stride |
|---|---:|
| `CMovieSystem` | 0x21f0 |
| camera slot | 0x28 (3 slots at `movie+0x64`) |
| AVI channel | 0x20 (2 at `movie+0xe4`) |
| UI/subtitle channel | 0x82c (4 at `movie+0x140`) |
| active-list entry | 0x20 |
| `CSequence` | 0x74 |
| `CAnimEntityNode` | 0x1a0 |
| `CSceneNode` | 0x100 |
| `CCameraNode` | 0x248 |
| node array entry | 8 (`{node*, trackContainer*}`) |
| track array entry | 8 (`{paramId, track*}`) |
| float key | 0x28 |
| vec3/scale key | 0x40 |
| position key | 0x68 |
| rotation key | 0x4c |
| shake key | 0x4c |
| camera/focus key | 0xb0 |
| sound key | 0xc4 |
| animation key | 0x5c |
| event/facial key | 0x8c |
| screen key | 0xb8 |
| console key | 0x48 |
| music key | 0x74 |
| expression/sequence key | 0x98 |
| look-at key | 0x94 |
| link key | 0x140 |
| equip key | 0x14 |
| visibility key | 8 |

### 10.6 Lua binding constants

`Movie.PlaySequence` passes `(name, 0, 0, DAT_35570b48, DAT_35570b48, DAT_35570b48)` where
`DAT_35570b48` is the black colour constant; the movie system uses `IMovieSystem+0x50`. The table is
only created when the script-system mode is not 3. `GetNumOfPlayingCutscenes` returns
`movie+0x40` as a float.

---

## 11. Unknowns

| # | unknown | why it matters | how to settle |
|---:|---|---|---|
| 1 | Who spawns the `_cinema_only_objs.xml` entities | hidden actors/props are a visible part of cinemas | live quest cinema + breakpoint on `IMovieSystem+0x130` and on the level entity loader; check whether `x2game`/level system calls `+0x130` |
| 2 | Meaning of listener events 4..0x15 in `0x3901bcd0` | the x2game listener switches on them, but crymovie only emits 0/1/2/3 | hook `NotifyListeners` and log every event on a live cinema; or map the other listener interface |
| 3 | Skip key/UI path | the roadmap wants "skipping" | search the 1.2 UI `.alb` for `Movie.StopSequence`/`StopAllSequences` (none found in 961 scripts) and the quest-cinema window; test a key press on a live cinema |
| 4 | Exact non-float interpolation (position/rotation tangents) | 97 position / 82 rotation keys use `cont/tens` | disassemble the position/rotation key containers (`0x3401c...`, `0x3401e...`) and diff against a Rust Hermite/slerp |
| 5 | Flags 0x40/0x100/0x200/0x8000/0x20000/0x40000 | sequence behaviour (blocking/loading/login) | correlate with x2game flag tests and live sequences |
| 6 | `gEnv+0x64` object | music/audio stop on play/stop | identify its vtable; likely the music system (the survey's `+0x5c`/`+0x64` labels are inconsistent with this module) |
| 7 | `prefab_unit`/`prefab_doodad` strings | dynamic entity spawn | no direct xref; inspect the game-framework spawn descriptor (`FUN_34044210`) and the entity-class table |
| 8 | `Movie.PlaySequence(name)` for a not-yet-created sequence | the trigger path would fail silently | find the preload path (level load / server) or change the port to create+load on demand |
| 9 | `seq+0x18` / `seq+0x64` | PlaySequence start offset semantics | dump a live sequence during play; correlate with the active entry |
| 10 | node track container allocation site | struct layout completeness | the getter is `node+0x10`; find the writer in the node ctors |
| 11 | `movie+0x2c`/`+0x30` map key/value layouts | node lookup by id/name | dump the maps during load; the accessors are `0x3400d8b0`/`0x3400cd20` |
| 12 | AVI rendering vs the Bevy UI | visual parity for 112 AVI tracks | port `0x3400d010` as a Bevy UI/video overlay; compare `sl_el_001` |
| 13 | `ca_ignoreCutSceneAnim` exact effect | animation override | test with the cvar on/off on a live cinema |
| 14 | camera actor interface `+0x04/+0x08/+0x14` mapping | hand-over contract | map the x2game camera actor vtable `0x399b0a90` entries to the calls |

---

## 12. Sizing

All numbers are code functions / code bytes from `crymovie.dll.funcs.csv`, not source lines. The
survey's band table is reproduced for traceability; the "after audit" column removes the track/node
classes that no shipped sequence uses (§3.3, §5.10).

| bucket | functions | KB |
|---|---:|---:|
| sequence object, XML attributes, serialization (`0x34001000-0x34003ab0`) | 59 | 10.0 |
| XML value parser + cinema loader (`0x34003ab0-0x3400bbf0`) | 249 | 28.4 |
| movie-system core (`0x3400bbf0-0x34015740`) | 184 | 34.1 |
| node model, track-type registry, generic tracks A (`0x34015740-0x34020b60`) | 177 | 43.1 |
| track parameter/keyframe types (`0x34020b60-0x3402a360`) | 122 | 32.0 |
| track classes/evaluators B + camera/shake/focus (`0x3402a360-0x3403c3d0`, `0x34046a80-0x3404b990`) | 392 + 303 | 68.3 + 15.6 |
| entity/doodad/animation binding (`0x3403c3d0-0x34046a80`) | 142 | 36.4 |
| **whole module** | **1,628** | **268.0** |

Derived buckets:

| bucket | functions | KB |
|---|---:|---:|
| **(a) phase 1 (riding/on-foot) needs** | **0** | **0.0** |
| (b) P5 to port (survey) | 1,226 | 231.9 |
| minus unused track/node classes (expression, music, sequence, material, node 4/5/6) | −111 | −20.9 |
| **P5 to port after the cinema-XML audit** | **1,115** | **211.0** |
| plus `Movie` Lua binding in cryscriptsystem (`0x35514d50` ctor + 7 binders) | 8 | 1.4 |
| replaced by a Rust XML reader (not ported) | 249 | 28.4 |
| skip: editor save/record, C++ containers/CRT | 153 | 7.6 |

So the phase-1 budget is **zero**, and the remaining P5 work is about **1,123 functions /
212 KB** of engine code plus the Rust data model/XML reader. The single largest chunk is the
generic key/track framework (43.1 + 32.0 + 68.3 KB); the audit cuts about 21 KB of dead track code.
The 28.4 KB XML-value parser is not ported at all.

The Lua binding is small (8 functions, 1.4 KB) but it is the only shipped script entry point
(`Movie.PlaySequence` from the four trigger scripts), so implement it in the first playable slice.

---

## 13. Key-function table

This table lists every function on the port path (movie system, sequence, node apply, track
framework/evaluators, Lua binding). The remaining functions of the 1,628 are grouped by band in
§12: XML value parsing (replaced), editor save/record, C++ containers/CRT, and the unused track/
node classes.

Columns: address, size, role, callers → callees, fields/globals, pure/stateful. "Stateful" means it
reads or writes module/game state; "pure" means the result depends only on its arguments and the
loaded data (still may read the sequence/track object).

### 13.1 Movie system

| address | size | role | callers → callees | fields / globals | kind |
|---|---:|---|---|---|---|
| `0x340053b0` | 96 | `CreateMovieSystem` export | crysystem `InitMovieSystem` → ctor | `gEnv`; alloc 0x21f0 | stateful |
| `0x3400f0c0` | 1,501 | `CMovieSystem` ctor | `CreateMovieSystem` → cvar/command registration | `movie+0x04..+0x21e8`; `mov_*` | stateful |
| `0x3400dcc0` | 812 | `Update(dt)` | engine frame → seq `+0x70`, `+0x54` | active list `+0x3c`, `+0x59`, `mov_NoCutscenes` | stateful |
| `0x3400e200` | 727 | `PlaySequence` | x2game `TryStartCinema`, `mov_ps` → seq `+0xac/+0xb4/+0x24/+0x60/+0x30/+0x7c`, listener `+0x140` | `movie+0x138`, active list | stateful |
| `0x3400bd30` | 102 | `PlaySequenceByName` | `Movie.PlaySequence` → `+0x24`, `+0x4c` | registry `+0x1c` | stateful |
| `0x3400ee90` | 556 | `StopSequence` | Update stop list, `+0x54` → seq `+0x7c/+0x70/+0x30/+0x10`, listener `+0x140` | active list; log `end cinema` | stateful |
| `0x3400f6a0` | 18 | `StopSequence` no-fade wrapper | movie `+0x54` → `0x3400ee90` | — | stateful |
| `0x3400f6c0` | 64 | `StopSequence` fade | movie `+0x5c` → `0x3400ee90` | `movie+0x60` | stateful |
| `0x3400ce90` | 81 | `StopSequenceByName` | `Movie.StopSequence` → `+0x54` | registry | stateful |
| `0x3400e160` | 49 | `StopAllSequences` | `Movie.StopAllSequences`, x2game | active list | stateful |
| `0x3400dba0` | 274 | stop sequences without flag `0x800` | `Movie.AbortAllSequences` → `+0x5c` | active list | stateful |
| `0x3400c530` | 71 | `Pause` | `Movie.PauseSequences` → AVI channels | `movie+0x59` | stateful |
| `0x3400d4c0` | 200 | `Resume` | `Movie.ResumeSequences` → AVI channels | `movie+0x59` | stateful |
| `0x3400cb50` | 71 | `FindSequence(name)` | `+0x50`, `+0x18` callers → `_stricmp` | registry `+0x1c` | stateful |
| `0x3400e4e0` | 159 | `CreateSequence(name,world)` | `+0x18`, `TryStartCinema` → seq ctor | alloc 0x74; registry | stateful |
| `0x3400ee40` | 68 | `DeleteSequence` | `+0x20` → `+0x54`, `0x3400eda0` | registry | stateful |
| `0x3400f700` | 115 | `AddMovieListener` | `+0x34`, `TryStartCinema` | listener map `+0x48` | stateful |
| `0x3400f780` | 88 | `RemoveMovieListener` | `+0x38` | listener map | stateful |
| `0x3400e080` | 219 | `NotifyListeners` | `+0x140` → listener `vtable[0]` | listener map | stateful |
| `0x3400cef0` | 39 | `IsPlaying(seq)` | `+0x70`, `TryStartCinema` | active list | stateful |
| `0x3400bda0` | 68 | `IsPlayingAny` (flag 8) | `+0x74`, `mov_back_time` | active list | stateful |
| `0x3400bdf0` | 90 | `IsPlayingBlocking` (8/0x20/0x40000) | x2game | registry | stateful |
| `0x340207e0` | 13 | `SetPaused(bool)` | `+0xb0` | `movie+0x58` | stateful |
| `0x340207f0` | 4 | `GetPaused()` | `+0xb4` | `movie+0x58` | pure |
| `0x340207a0` | 13 | `SetCameraActor(ptr)` | `+0x04`, `FUN_3916bbd0` | `movie+8` | stateful |
| `0x3400bfd0` | 24 | `ApplyCameraSlots` | `+0xd4` → listener `+0x14` | `movie+8`, slots | stateful |
| `0x3400bff0` | 201 | `SetCameraSlot(index,desc)` | `+0xd8`, camera switch | slots `+0x6c` | stateful |
| `0x3400cd20` | 149 | `FindCameraNode(name)` | `+0xa8`, camera switch | node map `+0x30` | stateful |
| `0x3400d8b0` | 49 | `GetNodeById(id)` | `+0xa4`, node loader | node map `+0x2c` | stateful |
| `0x3400c5c0` | 48 | `SetMovieStartTimeOfDay` | `+0x12c`, `mov_back_time` → I3DEngine `+0x14` | `movie+0x138` | stateful |
| `0x3400ca70` | 217 | `LoadCinemaObjects(filename)` | `+0x130`, sequence unload → system `+0x1a4`, loader `+0x2c` | `movie+0x13c` | stateful |
| `0x3400d010` | 1,186 | `RenderAVI/UI` | movie frame → renderer `+0x3c8` | AVI/UI channels | stateful |
| `0x3400c170` | 518 | `RenderUiOverlays` | movie frame → renderer `+0xf4` | `movie+0x140` | stateful |
| `0x3400c420` | 237 | `SetUiText(channel,text)` | scene apply → renderer `+0x23c` | `movie+0x140` | stateful |
| `0x3400c510` | 24 | `ClearUiText` | scene apply → listener `+0xc` | `movie+8` | stateful |
| `0x3400bbf0` | 222 | `mov_ps` command | console → `+0x18`, `+0x4c` | `gEnv+0x3c` | stateful |
| `0x3400bbc0` | 40 | `mov_back_time` command | console → `+0x74`, `+0x12c` | `movie+0x138` | stateful |

### 13.2 Sequence

| address | size | role | callers → callees | fields / globals | kind |
|---|---:|---|---|---|---|
| `0x340028a0` | 369 | `CSequence` ctor | `CreateSequence`, `LoadSequenceFromXml` | `seq+0x40..+0x70` | stateful |
| `0x34002a20` | 532 | `CSequence` dtor | Release | node array, objects node | stateful |
| `0x34002620` | 625 | `ISequence::Load` | `TryStartCinema`, `PlaySequence` → system `+0x1a4`, `+0x98` | `seq+0x2c`, `+0x44` | stateful |
| `0x34001250` | 1,259 | `LoadXML` | `Load`, `LoadSequenceFromXml` → `CreateNode`, node `+0x104`, container `+0x30` | `seq+0x28/0x48/0x4c/0x50/0x54` | stateful |
| `0x34001880` | 556 | `Unload` | `+0x60`, stop → `FUN_34001790`, sound `+0x3c`, movie `+0x130` | `seq+0x71`, nodes | stateful |
| `0x34001eb0` | 528 | `Update(ctx)` | movie Update → node `+0x114/+0xd4/+0xd8`, `FUN_34047890` | `seq+0x58` | stateful |
| `0x340020d0` | 87 | `SetTimeRange(start,end)` | LoadXML → node `+0x28` | `seq+0x50/0x54` | stateful |
| `0x34002130` | 240 | `ScaleTime` | editor/load → track key rescale | all keys | stateful |
| `0x340011a0` | 172 | `CreateSceneNode` | LoadXML → `CSceneNode` ctor | `seq+0x68` | stateful |
| `0x34002d60` | 334 | `AddNode` | LoadXML | node array `+0x30` | stateful |
| `0x34001110` | 109 | `FindNodeByName` | link/camera | node array | pure |
| `0x34001100` | 16 | `GetNode(index)` | LoadXML/apply | node array | pure |
| `0x34001180` | 17 | `GetTrackContainer(index)` | LoadXML/apply | node array | pure |
| `0x34001e40` | 108 | `ResetLinks` | Unload/activate | node links | stateful |
| `0x34001790` | 230 | remove companion GUIDs | Unload → entity system `+0xa4/+0x4c` | `seq+0x2c` | stateful |
| `0x34003d70` | 32 | `GetObjectsXmlNode` | game side | `seq+0x2c` | pure |

### 13.3 Nodes and apply

| address | size | role | callers → callees | fields / globals | kind |
|---|---:|---|---|---|---|
| `0x3400e6c0` | 583 | `CreateNode(type,id,name)` | LoadXML → node ctors | movie node maps | stateful |
| `0x3400e9b0` | 462 | `CreateTrack(typeId)` | container load → track ctors | — | stateful |
| `0x3402a130` | 119 | node base ctor | all node ctors | `node+0x08/0x0c/0x10` | stateful |
| `0x34043c60` | 1,023 | `CAnimEntityNode` ctor | `CreateNode` type 1 | `node+0x30..+0x1a0` | stateful |
| `0x34042fc0` | 1,902 | entity `ApplyTracks` | sequence Update `+0xd4` | transform, anim, sound, event, equip, alpha | stateful |
| `0x3403da80` | 1,322 | entity `Activate`/spawn | node `+0x50` | `DynamicType`, `EntityGUID`, entity system/game | stateful |
| `0x34040610` | 250 | entity `LoadNodeXml` | LoadXML `+0x104` | `+0x30/+0x90/+0xa0` | stateful |
| `0x34045360` | 341 | dynamic attrs parse/save | entity load | `+0x40/+0x44/+0x48` | stateful |
| `0x34040470` | 122 | link apply | sequence Update `+0xd8` | `+0xb8` | stateful |
| `0x34041aa0` | 2,466 | animation apply | entity apply | `+0xb8..+0xc4`, anim FIFO | stateful |
| `0x3403ee10` | 711 | event apply | entity apply | `Event_` + entity `+0x114` | stateful |
| `0x34047cd0` | 688 | sound apply | scene/entity apply | 9 sound channels, `gEnv+0x30` | stateful |
| `0x34048150` | 1,970 | scene `ApplyTracks` | sequence Update `+0xd4` | camera slots, event, console, sound, AVI/UI | stateful |
| `0x34027d00` | 4,522 | camera `ApplyTracks` | sequence Update `+0xd4` | FOV, pos/rot, shake, focus, link | stateful |
| `0x34047530` | 325 | camera switch | scene apply | camera node lookup, `movie+0xd8` | stateful |
| `0x3403dfb0` | 560 | camera link update | camera apply | `+0x1a0`, link track | stateful |
### 13.4 Track framework / evaluators

| address | size | role | callers → callees | fields / globals | kind |
|---|---:|---|---|---|---|
| `0x3402a6a0` | 983 | track factory by type id | container `+0x1c` → class ctors | — | stateful |
| `0x3402a360` | 488 | track container load/save | sequence LoadXML `+0x30` → node `+0xec`, factory, track `+0x90` | `{paramId,track*}` array | stateful |
| `0x34015740` | 898 | float track load/save | container load | `time/value/bias/cont/tens/easefrom/easeto` | stateful |
| `0x34017dc0` | 316 | float key evaluator | float track `+0x6c` | key array, loop flag | pure |
| `0x340443f0` | 403 | sound key evaluator | sound apply | 0xc4 keys | pure |
| `0x34029380` | 403 | camera key evaluator | scene/camera apply | 0xb0 keys | pure |
| `0x34049230` | 403 | screen key evaluator | scene apply | 0xb8 keys | pure |
| `0x34044750` | 403 | event key evaluator | entity apply | 0x8c keys | pure |
| `0x34048d50` | 417 | console key evaluator | scene apply | 0x48 keys | pure |
| `0x34048f00` | 385 | music key evaluator (unused) | scene apply | 0x74 keys | pure |
| `0x3403e3a0` | 2,433 | entity transform apply | entity apply | pos/rot/scale, link | stateful |
| `0x34042680` | 1,915 | link evaluator/apply | entity/camera apply | 0x140 keys, bone/offset | stateful |
| `0x34044a90` | 420 | equip evaluator | entity apply | 0x14 keys | pure |
| `0x34047890` | 318 | deferred animation update | sequence Update | anim queue, entity | stateful |
| `0x34042e00` | 437 | facial apply | entity apply | 0x8c keys | stateful |

### 13.5 Lua `Movie` binding (cryscriptsystem)

| address | size | role | callers → callees | fields | kind |
|---|---:|---|---|---|---|
| `0x35514d50` | 834 | `Movie` table ctor | script system init → 7 binders | `+0x44` Lua state, `+0x48` script system, `+0x50` game, `+0x54` movie | stateful |
| `0x35514be0` | 100 | `PlaySequence(name)` | Lua → movie `+0x50` | — | stateful |
| `0x35514ad0` | 37 | `StopSequence(name)` | Lua → movie `+0x58` | — | stateful |
| `0x35514b00` | 33 | `StopAllSequences()` | Lua → movie `+0x60` | — | stateful |
| `0x35514b30` | 75 | `AbortAllSequences()` | Lua → game `+0x14c`, movie `+0x68` | — | stateful |
| `0x35514b80` | 36 | `PauseSequences()` | Lua → movie `+0xb8` | — | stateful |
| `0x35514bb0` | 36 | `ResumeSequences()` | Lua → movie `+0xbc` | — | stateful |
| `0x35514c70` | 216 | `GetNumOfPlayingCutscenes()` | Lua → movie `+0x6c` | — | stateful |

---

## 14. Verification checklist for this map

* [x] Every shipped cinema parsed: 258 sequences, 255 companions, 2,972 nodes, 10,476 tracks,
      22,580 keys.
* [x] Node/track/key type audit against the registration tables and the class registry.
* [x] Movie-system vtable read from the image; the two wrong entries in
      `port-flowgraph/structs.md` §7 corrected.
* [x] `Movie` Lua table and its 961-script usage audited.
* [x] x2game hand-over linked, including the `0xcf`/`0xce` cinema packets.
* [x] Phase-1 impact: none.
* [ ] Live-client difftest (§8) not run in this pass.
* [ ] Companion-object spawner and listener events 4..0x15 still open (§11).
