# Port survey: 1.2 engine modules (roadmap v2, survey B)

Status: read-only breadth survey, roadmap v2 phase 0. Branch `map/engine-survey-b`.
This file classifies **every function area** of the assigned modules so the owner can see what is
left to map and port. It is a survey, not a map: it links the existing deep maps instead of redoing
them. The published CRYENGINE source was not used, quoted or consulted.

## 0. Method

* Sources: `D:/aa-re/client12/engine/out/` (cryaction, cryanimation, cryentitysystem, crygame),
  `engine2/out/` (cry3dengine, cryaisystem, cryfont, cryinput, crymovie, cryrenderd3d10,
  crysoundsystem, x2common, x2ui, xlcommon, xlleveldb), `engine3/out/` (crysystem,
  cryscriptsystem) and `C:/AAEMU_DISK/aa-re/client12/out-x2game/` (x2game call sites, strings,
  `strings-xrefs.tsv`, `calls.tsv`).
* **Never a whole `.c`.** Areas come from the binaries' own strings: profiler registrations
  (`CClass::Method`), `X2Log`/assert/file-name strings, class names, cvar names, chunk names,
  exports/imports, and narrow greps at the addresses named by `funcs.csv`. Function counts and KB
  are the `funcs.csv` `size` sums inside the stated address/area bounds.
* Runtime-use evidence is the x2game side: direct xlcommon/xlleveldb imports, `gEnv` interface
  slots (E1: `p3DEngine` `+0x54`, sound `+0x58`, music `+0x5c`, movie `+0x64`, AI `+0x68`,
  character manager `+0x7c`), cvar/command strings referenced by x2game functions
  (`strings-xrefs.tsv`), and SQL/table strings the client actually queries.
* Tags: **[R]** read in the binary/decompile, **[I]** inference, **[?]** open. "Band" counts are
  address-bound estimates because 1.2 is LTCG-linked: classes are mostly contiguous but some
  functions (notably the animation command evaluators) are reordered into neighbouring bands.
* Existing maps and notes linked, not repeated:
  `docs/research/port-engine-anim/MAP.md` (E2 animation), `port-engine-char/MAP.md` (E3
  character/entity glue), `port-engine-loop/MAP.md` (E1 frame loop), the `port-*/MAP.md` x2game
  maps, `docs/aa-ui.md` (CryScriptSystem argument conversion) and `docs/aa-world.md`
  (CryEntitySystem parsers). CryPhysics is fully ported in `aaemu-physics`.

Relevance tags: **P1** movement/riding, **P2** combat, **P3** interaction/housing, **P4** vehicles,
**P5** world flow/cutscenes/UI/audio.

---

## Survey summary

| Module | Whole | To port (new) | Covered / replaced | Out of scope (rendering) | Skip |
|---|---:|---:|---:|---:|---:|
| cryanimation (beyond E2) | 1,382.9 KB / 7,163 | 988.8 KB / 3,539 (E2 already maps 341.6 KB / 1,197) | — | — | 67.7 KB / 2,474 |
| crymovie | 268.0 KB / 1,628 | 231.9 KB / 1,226 | 28.4 KB / 249 (Rust XML) | — | 7.6 KB / 153 |
| cryaisystem | 1,642.5 KB / 7,155 | 365.0 KB / 1,200 (upper bound) | — | — | 1,277.5 KB / 5,955 |
| crysoundsystem | 574.4 KB / 3,426 | 20.8 KB / 154 (occlusion) | 468.6 KB / 2,002 (`aa-audio`) | — | 85.0 KB / 1,270 |
| cryfont | 144.7 KB / 1,275 | 0 | 67.6 KB / 686 (`aa-ui`) | — | 77.1 KB / 589 |
| x2ui | 226.6 KB / 936 | 0 | 60.2 KB / 349 (`aa-ui`) | — | 166.4 KB / 587 |
| x2common | 29.8 KB / 341 | 0 | 25.8 KB / 274 (Rust/`aa-*`) | — | 4.0 KB / 67 |
| xlcommon | 316.4 KB / 1,760 | 21.8 KB / 57 (name policy/hashes) | 120.9 KB / 861 (Rust/`aa-*`) | — | 173.7 KB / 842 |
| xlleveldb | 235.9 KB / 1,795 | 0 | 2.5 KB / 12 (`aa-client`) | — | 233.4 KB / 1,783 |
| cry3dengine | 2,221.3 KB / 10,100 | 0 | 1,783.0 KB / 4,707 (`aa-world` + `aa-cry`/`aa-fx`) | 285.9 KB / 615 | 152.4 KB / 4,778 |
| cryrenderd3d10 | 2,670.3 KB / 12,525 | 0 | — | 2,670.3 KB / 12,525 | — |
| **total newly to port (excluding E2)** | — | **1,628.3 KB / 6,176** | — | — | — |

The single biggest new port is cryanimation beyond E2: facial/face-lib/CAL (237.4 KB), character
loading/streaming (213.1 KB), CAF/DBH animation assets + thread/context (202.6 KB), model/animset/
mesh glue (153.8 KB), pose/physics-sync helpers (137.8 KB) and skin/attachment helpers (44.1 KB).
crymovie is the next P5 block (231.9 KB). cryaisystem's 365 KB is an upper bound for a client-side
anchor/AI-point subset; most of that module is server-side behaviour. Everything else is already
covered, replaced by Bevy/Rust, or rendering.

---

## 1. cryanimation (beyond E2): facial, IK, physics, LOD, loading

Whole module: **7,163 f / 1,382.9 KB** (image base `0x31500000`, one export:
`CreateCharManager`; everything else is reached through the `ICharacterManager` /
`ICharacterInstance` / `ISkeletonAnim` interfaces at `gEnv+0x7c`). E2 already owns the playback
core and gives the sizing boundary: phase-1 union **1,197 f / 341.6 KB**, remainder **5,966 f /
1,041.3 KB** (`port-engine-anim/MAP.md` §9.7). The rows below classify that remainder. E2's key
functions are inside these bands, so the first task is always "port E2's key set"; the rest is new.

| Area (band) | f | KB | What it does | Runtime use (evidence) | Parity | Approach |
|---|---:|---:|---|---|---|---|
| Facial animation, face libs, CAL, face makers, morph targets (`0x31580000-0x3159e000`, `0x315a4000-0x315c6920`) | 863 | 237.4 | `CFacialAnimation*`, `CFacialInstance`/`Model`/`AnimSequence`, `.cal` facial libraries, `$FaceLib`, per-race `*_targets.xml` face makers, morph targets and facial sequence streaming | **Used.** x2game cvars `ca_UseFacialAnimation`/`ca_FacialAnimationRadius` (xrefs `FUN_3915b960`, `FUN_39160ba0`), `facial_expression` string, `custom_face_presets` and `face_*_maps` SQL, `facial` columns in chat-bubble tables | P5 (character creation, NPC chat/portraits); P2 minor | Port line by line into `aa-anim` (sequences/instances) and `aa-cry` (`.cal`, `.fsq`, morph chunk streams). Keep the face-maker XML + DB preset path; do not port the 2000s facial GPU buffer code (renderer). |
| Character loading / streaming / CharacterManager / CHR loader (`0x315c6920-0x315fd910`) | 806 | 213.1 | `CharacterManager` (instances, streaming, precache, GC), `CHRStreamTask`/`CGAStreamTask`/`CDFStreamTask`, `CryCHRLoader::LoadNewCHR/FaceLib/FacialLibrary/ExtraResources`, CHR/CGA/CDF/DBH material and XML streaming, `BuildModel` physics/LOD/morph | **Used.** every visible character/mount/doodad-creature enters through the character manager (E3 §1.1); `CHR PreCaching`, `ca_dbh_level`, `ca_UseFileAfterDBH`, stream-task profiler strings; x2game model swaps on equipment/mount/customize | P1, P5 | Port the parse/build path into `aa-cry`/`aa-anim`; replace stream worker tasks with Bevy async loading. Skip retail precache heuristics and duplicate CRC/resource tables. |
| Animation assets: CAF/DBH, animation set/thread/context (`0x31626526-0x3165cde0`) | 977 | 202.6 | `GlobalAnimationHeaderCAF::LoadCAF/LoadCAFfromDBH/DoesExistCAF`, `.dbh` databases, `CControllerPQLog` clip curves, `CAnimationThreadTask`, `CAnimationContext`, `CAnimationSet` lookup | **Used.** feeds E2's clip sampling; `Failed To Load Animation DBH`, `Illegal File Type for .caf/.dbh`, `ca_dbh_level`, `ca_UseFileAfterDBH`; `preloadlmg.dbh` | P1 | Port CAF/DBH/DBA parsing and clip curves into `aa-anim`; E2 already maps the consumers. Replace `CAnimationThreadTask`/SPU with Bevy tasks; no reason to reproduce the job system. |
| Model / animation-set / skeleton-mesh glue (`0x315fd910-0x31626526`) | 556 | 153.8 | `CCharacterModel`, `CAnimationSet`, `CModelMesh`, `CModelSkeleton`, `CAnimationManager`; model descriptors, bone/mesh mapping, default-anim tables | **Used.** `actor_models.model_file`/`animation_graph`, character CDF/CHR assembly (E3), customization | P1, P5 | Port descriptors and assembly into `aa-anim`/`aa-cry`. The global `CAnimationManager` singleton can be simplified to asset caches. Drop editor/debug dump paths. |
| Skeleton pose helpers, post-process and character physics sync (`0x31528000-0x3154d400`) | 246 | 137.8 | pose helpers around `CSkeletonPose`, `SSkeletonPose::UpdateBBox`, `SkeletonPostProcess`, `CreateCharacterPhysics`, `BuildPhysicalEntity`, `SynchronizeWithPhysics*`, SPU variants | **Used.** E2 §6/§8/§9.6 maps the post-process; `chr Physical Bone Error`, `physical proxy` asserts; char physics proxies collide with the world every frame | P1, P2 | Already mapped by E2 for pose; port proxy build/sync with `aaemu-physics`. Drop SPU variants. Ragdoll itself is **not here**: x2game issues `RagDollize`/`TurnRagdoll`/`unragdoll` (`391462a0`, `39141e70`, `391430b0`) against CryPhysics (already ported). |
| Skin / morph / attachment / effect helpers (`0x31501000-0x3150c000`) | 91 | 44.1 | `CSkinInstance` early methods, attachment manager edge cases, `CSkeletonEffectManager`, `MorphCopyToGPU`, `UpdateAttachedObjects` | **Used.** E2 §7 maps attachments; `$IK_Grip`/seat bones; morph copy is visible on customized faces | P1, P2 | Covered by E2 attachments + `aa-anim`; GPU skinning/morph copy is rendering (Nick). Port only CPU-side attachment updates. |
| Mesh merge / template / utility tail (`0x3165cde0-0x31673a00`) | 2,474 | 67.7 | `CharRenderMeshMerger`, 2,400+ tiny template/thunk bodies, allocators/debug helpers | Indirect. `ca_MergeMaxNumLods` is read by x2game (`FUN_3902a8a0`) but only affects render-mesh merging; `DoodadManager::UpdateLod` is x2game/world code | none (perf) | Skip. Bevy/the renderer owns mesh merging and small-object overhead. |

**Cross-cutting: LOD.** LOD is not a separate band; it is spread over loader/pose/skin rows.
Internal cvars: `e_lod_max/min/ratio/skin_ratio/lods`, `ca_ChrBaseLOD`, `ca_MaxFaceLOD`,
`ca_FaceBaseLOD`, `ca_useBoneLOD`, `ca_MergeMaxNumLods`, `ca_LodDist*`, `ca_LodCount*`,
`ca_UseAssetDefinedLod`, `ca_LodSkipTask*`. x2game exposes `option_character_lod` and doodad LOD.
For Bevy parity, LOD selection is a perf feature: port the bone/LOD masks only when character
rendering needs them; otherwise let the renderer/Bevy choose. Do not port the LOD task-skipping
heuristics.

**Ragdoll/physics.** `cryanimation.dll` has **no** ragdoll strings or solver. It builds character
physics proxies (capsules/boxes, `BuildPhysicalEntity`) and synchronizes the pose with them. The
ragdoll state machine and `_ragdoll.cdf` live in x2game; the solver is CryPhysics, already ported.
So for P2: port proxy build + sync (row 5), reuse `aaemu-physics`; nothing ragdoll-specific here.

**IK.** E2 §8 already maps animation-driven IK, look/aim IK, foot/stirrup IK and the pose-modifier
chain. The only new IK-adjacent strings in the remainder are face/eye targets (`EyeLeft`,
`EyeRight`, `LHand_IKTarget`) and `LoadTestIKs` in the loader (editor test path). Port the facial
eye/look targets with the facial row; skip `LoadTestIKs`.

**Runtime call path (one line).** `gEnv+0x7c` -> `ICharacterManager` (`CreateCharManager`) ->
`CharacterManager::CreateInstance/CreateCHRInstance` -> `CryCHRLoader` -> `CCharacterModel` /
`CAnimationSet` -> `CSkeletonPose` (E2) -> attachments/skin. Facial is a second instance
(`CFacialInstance`) attached to the same master character.

**cryanimation totals**

| bucket | f | KB |
|---|---:|---:|
| Already mapped/scoped by E2 (phase-1 union; 74 key functions = 71.4 KB) | 1,197 | 341.6 |
| New: to port line by line (facial 237.4, loading 213.1, assets 202.6, model 153.8, pose/physics 137.8, skin/attach 44.1) | 3,539 | 988.8 |
| Skip (mesh merge/templates/utility; GPU skinning is rendering/out of scope) | 2,474 | 67.7 |
| **rest beyond E2, measured** | **6,013** | **1,056.5** |
| **whole module** | **7,163** | **1,382.9** |

The 47-function difference against E2's 5,966/1,041.3 remainder is E2 band-boundary rounding; the
whole-module total matches exactly. Of the 988.8 KB to port, the P1/P2 subset (loading, assets,
model/animset, pose/physics, skin/attach) is ~**751 KB**; facial is **237 KB** and is P5-first.

---

## 2. crymovie (track view / cutscenes)

Whole module: **1,628 f / 268.0 KB**, image base `0x34000000`, one export `CreateMovieSystem`.
Reached at `gEnv+0x64` (E1) and by the console command `CmdPlaySequence`; x2game plays quest and
login cinemas (`try start Cinema : %s`, `OnPlaySequence`, `PlayCinema`, `GetMainQuestCutSceneReplayList`,
`cinema_effects` SQL). The pak carries **517** `game/worlds/*/cinemas/*.xml` files, so this is real
P5 content, not an editor leftover.

| Area (band) | f | KB | What it does | Runtime use (evidence) | Parity | Approach |
|---|---:|---:|---|---|---|---|
| Sequence object, XML attributes, serialization (`0x34001000-0x34003ab0`) | 59 | 10.0 | `Scene`/`Nodes`/`SequenceData` model, `StartTime`/`EndTime`/`Flags`, time-of-day/music fields, `Serialize : %s`, origin-transform recording | **Used.** sequence XML -> cinema playback; editor record paths also live here | P5 | Port the data model + reader; drop `RecordOrigin*`/editor save. |
| XML value parser + cinema loader (`0x34003ab0-0x3400bbf0`) | 249 | 28.4 | `anim_sequence_*.xml` / `Game/worlds/%s/cinemas/%s.xml` loading, `*_cinema_only_objs.xml`, typed value/`TrackType` parsing, `GeomType` debug parsing | **Used.** every cinema is loaded from these paths | P5 | Replace with Rust `quick-xml`/`serde`; keep the XML schema and the two-file (sequence + only-objs) convention. Do not port the generic C++ string/number parsers. |
| Movie-system core (`0x3400bbf0-0x34015740`) | 184 | 34.1 | `CMovieSystem`: sequence registry, `PlaySequence`/stop/update, listeners, seek (`seek : %.2f`), `end cinema`, console commands/cvars (`mov_ps`, `mov_back_time`, `mov_effect`, `mov_loading`, `mov_NoCutscenes`), time-of-day | **Used.** `s_CinemaVolume`, quest cinema windows, unit-state rejection during cutscene, cinema volume triggers | P5, P3 | Port line by line as a small cutscene scheduler (aa-client or a new `aa-movie`); cvars become settings. |
| Node model, track-type registry and generic track classes A (`0x34015740-0x34020b60`) | 177 | 43.1 | node casting/creation from XML (`cast Node`, `create Node`), `TrackType`, `Flags`, param ids, keyframe containers, animation/expression/sound/camera track bases | **Used.** the 517 cinemas are built from these node/track types | P5 | Port the node/track framework and the track types the shipped cinemas use; drop editor-only node types after a data audit. |
| Track parameter/keyframe types (`0x34020b60-0x3402a360`) | 122 | 32.0 | per-track params: `overridetimes`, `length`, `command`, `equipSlot`, `event`, `blendin`, `boneName`, `relative`, `volramp_time`, `RampDown`, `angle`, `filename`, `subtitleNum`, `subtitle` | **Used.** camera/anim/sound/expression/event/subtitle tracks | P5, P3 | Port the used parameter set into the same data structs as the XML reader. |
| Track classes/evaluators B + camera/shake/focus (`0x3402a360-0x3403c3d0`, `0x34046a80-0x3404b990`) | 392 + 303 | 68.3 + 15.6 | remaining per-type tracks, camera FOV (`FOV`), camera shake (`ShakeA/BEnabled`, noise/frequency/amplitude), `LookAt`/focus, material/shader keys (`Ambient`/`Diffuse`/`Specular`/`Emission`/`Shininess`/`Opacity`, shader params 1-9), event track | **Used** for camera/anim/sound/event/expression; material/shader keys are unproven in shipped cinemas (strings only) | P5 | Port camera, shake/focus, event, visibility, equip and animation tracks; drive the Bevy camera. Skip shader/material tracks unless the cinema-XML audit finds them. |
| Entity/doodad/animation binding and cutscene animation (`0x3403c3d0-0x34046a80`) | 142 | 36.4 | `CAnimEntityNode`, dynamic entity/unit/doodad creation (`prefab_unit`, `prefab_doodad`, `DynamicType`), GUID/link updates, visibility/disable/activate, `LookAt`, cutscene animation IDs/FIFO (`ca_ignoreCutSceneAnim`, `[cut scene error] ...`), deferred animations | **Used.** cinemas spawn and animate units/doodads; x2game defers unit-state packets while a cutscene plays | P5 | Port line by line; bind to `aa-world` entities/animation via `aa-anim`; reuse `aa-fx` for cinema effects. |
| Editor/recording, containers, CRT/debug tail | included above | ~0 | `RecordOrigin*`, editor XML path, generic `map/set/vector` and CRT thunks | Editor/tool; not needed by the shipped client | none | Skip (Rust std/renderer replaces). |

**Sizing note.** The band table sums to 268.0 KB. The generic XML-value parser (28.4 KB) is replaced,
not ported. Track material/shader keys are the only major uncertainty: the strings exist, but a
cinema-XML track audit is needed before deciding whether to drop them.

**crymovie totals**

| bucket | f | KB |
|---|---:|---:|
| To port (sequence model, movie core, node/track framework, used tracks, camera, entity binding) | 1,226 | 231.9 |
| Covered by Rust XML parsing / Bevy camera+camera-shake (no line port) | 249 | 28.4 |
| Skip (editor recording/save, C++ containers/CRT, unproven material/shader tracks) | 153 | 7.6 |
| **whole module** | **1,628** | **268.0** |

---

## 3. cryaisystem (does the client use any of it?)

Whole module: **7,155 f / 1,642.5 KB**, image base `0x31000000`, one export `CreateAISystem`.
CrySystem initializes it and the frame loop reads `gEnv+0x68` (E1 §1.5): the AI object exists and
is updated every frame. But the **game logic is server-authoritative**: x2game states
`ScriptBindUnit::ProcessAiPhase() we don't support this for client!` and only registers units as
puppets (`RegisterWithAI: no puppet for %s`, `FUN_390dd300`) so they can be anchored/animated.
The client-side AI uses are narrow: AI-point prefabs and navigation modifiers in the world,
`ai_systemupdate` toggling during loading, AI movement parameters (`AIMovementSpeeds`, `pathType`,
`usePathfinder`, `pathFindPrediction`) and X2 walkability (`ai_DefaultWalkability`,
`1=Use ours(X2) walkability`). Combat goals, leader tactics, cover and nav-mesh pathfinding are
not driven by the client.

| Area (band) | f | KB | What it does | Runtime use (evidence) | Parity | Approach |
|---|---:|---:|---|---|---|---|
| Core AI actors/groups/players/actions/tactics (`0x31001000-0x31045700`) | 983 | 260.6 | `CAIActor`, `CAIGroup`, `CAIPlayer`, `CAIActionManager`, `CObjectTracker`, `CHumanGroupTactic`, `CAIHideObject`, `CAILightManager`, `CAStarSolver`, group/leader scaffolding | **Narrow.** Units are registered as puppets (x2game `RegisterWithAI`); `ProcessAiPhase` is explicitly unsupported on the client; `ai_systemupdate` is toggled during loading. Tactics/cover/leader actions have no x2game call site | P3/P5 (anchors) | Port only the actor/puppet **anchor** and registration subset; skip tactics, group combat, leader actions, hide/cover, light manager. `aa-world` already owns placement. |
| CAISystem core: update, events, walkability, shapes (`0x31045700-0x310b2130`) | 969 | 410.8 | `CAISystem::Update`, event queue, `CheckWalkability*`, `ai_DefaultWalkability` (X2 vs Cry mode), AI shapes/areas, AI point management, debug cvars | **Partly.** `ai_systemupdate` cvar (x2game xref `FUN_3915b960`/`39160ba0`), `ai_ignoreplayer`, AI movement params loaded by `FUN_390f5b20`, walkability option string | P1 (walkability), P3 (AI points) | Port the system bootstrap, walkability query and AI-point/nav-modifier management if local movement needs them; skip event processing for combat/sound/bullet/collision and the debug recording buffer. |
| Goal pipes, goal ops, flight/volume/waypoint/triangular/road nav regions, leader actions (`0x310b2130-0x31119f10`) | 976 | 393.1 | `CPipeUser`/`CGoalPipe`/`COP*` behavior execution; `CFlightNavRegion`, `CVolumeNavRegion`, `CWaypoint*NavRegion`, `CTriangularNavRegion`, `CRoadNavRegion`, `CGraph`, human-group tactics | **Not on the client.** No x2game call sites or cvars for goal pipes, cover, tactics, flight/volume nav or combat goals; AI phase execution is server-side | none | Skip. The server owns NPC decisions and movement; do not port a behavior tree. |
| Navigation / pathfinding core (`0x31119f10-0x3113ded0`) | 380 | 137.6 | `CNavigation`, `CNavPath`, `CPathfindingManager`, `CPathFollower`, `CPathObstacles`, `CAStarSolver`, `CStandardHeuristic`, triangulation | **Unclear/limited.** `usePathfinder`/`pathFindPrediction` are loaded as actor movement params (`FUN_390f5b20`), but the client does not path NPCs; player movement is physics-based | none (initial) | Skip initially. If local prediction ever needs it, port only `CNavPath`/walkability query, not the full nav-mesh stack. |
| Puppet/pipe-user glue + road nav (`0x3113ded0-0x31165890`) | 420 | 149.9 | `CPuppet`, `CPipeUser`, `CRoadNavRegion`, `CAIQuadTree`, `CAIShapeContainer`; puppet state/selection, road navigation | **Narrow.** Puppet objects exist (RegisterWithAI) but run no goals; `ProcessAiPhase` unsupported | P5 (NPC anchoring) | Port the puppet registration/position/rotation sync surface used by the unit proxy; skip goal selection and road nav. |
| Smart objects, AI points, waypoint nav, tail (`0x31165890-0x311b5d30`) | 3,426 | 290.3 | `CSmartObjects`, `CTrackPattern`, `CWaypointHumanNavRegion`/3D surface, `CVolumeNavRegion`, `CVertexList`, AI-point checks (`AIPoint is not inside Navigation Modifier`), plus containers/templates | **Used at placement time.** x2game loads AIPoint prefabs (`AIPointPrefabObject::CreateNode`, `ai point prefab loaded : type(%d)`, `prefab_ai_nav_mod_%d`, `AIPoint`) and AI nav modifiers | P3, P5 | Port AIPoint/nav-modifier parsing and validation; skip smart-object behavior, track patterns and waypoint nav regions. The static placement itself belongs to `aa-world`. |
| Debug/editor/test cvars (`ai_DebugDraw*`, `ai_CheckGoalpipes`, `ai_event_debug`, `ai_skill_debug`, editor-only limits) | included above | — | debug draw, state dumps, editor-only limits | Editor/tool; no shipping call site | none | Skip. |

**Answer in one line.** Yes, but only as a **placement/anchor service**: `CreateAISystem`, puppet
registration, AI points/nav modifiers, walkability, and the `ai_systemupdate` toggle. The 5,000+
functions of goal pipes, nav regions, pathfinding, combat tactics and debug drawing are server-side
or dormant in this client.

**cryaisystem totals**

| bucket | f | KB |
|---|---:|---:|
| To port (AI bootstrap + puppet/anchor + AI points/nav modifiers + walkability, upper bound) | 1,200 | 365.0 |
| Covered by the server/`aa-net` or `aa-world` (NPC decisions, movement, placement) | — | — |
| Skip (goal pipes, goal ops, nav regions, pathfinding, leader/tactics, debug/editor) | 5,955 | 1,277.5 |
| **whole module** | **7,155** | **1,642.5** |

The split is band-based; the exact client subset needs a call-site audit of the `RegisterWithAI` /
AIPoint paths before porting. Even at the upper bound, cryaisystem is the lowest parity value per KB
of the modules in this survey.

---

## 4. crysoundsystem

Whole module: **3,426 f / 574.4 KB**, image base `0x36000000`, one export `CreateSoundSystem`.
This is the FMODEx 4.x wrapper behind `gEnv+0x58` (sound) and `gEnv+0x5c` (music). The client uses
it continuously: x2game has its own `X2Sound`/`X2::GameClient::MusicManager`/`SoundMoodManager`
layers, `option_sound`, `s_MusicVolume`/`s_GameMusicVolume`, `i_soundeffects`,
`i_staticfiresounds`, `<Sound Obstruction>`, `OnPlaySound`/`OnDoodadSound`/`OnSoundAreaEvent`,
and paths such as `sounds/x2physics:foley:dive_in`. The audio **content** path is already mapped in
`docs/research/audio.md` and implemented in `crates/aa-audio` (FSB/FEV, Vorbis/MPEG/CELT, events,
mixing, music, reverb, moods). This module is therefore mostly *already covered by `aa-audio`*, not
a new line-by-line port.

| Area (band) | f | KB | What it does | Runtime use (evidence) | Parity | Approach |
|---|---:|---:|---|---|---|---|
| Core sound system, FMOD audio device, allocator (`0x36001000-0x360218c0`) | 631 | 120.8 | `CSoundSystem`/`SoundSystem` bootstrap, `CAudioDeviceFmodEx400`, channel groups, `CSoundAllocator`, FMOD system/device setup, listener | **Used.** `gEnv+0x58` is updated every frame and torn down at shutdown; `option_sound`; x2game plays UI/game events | P5 (audio) | Already covered by `aa-audio` (`engine.rs`/`playback.rs`). Do not wrap FMOD. |
| Sound moods / mixing / DSP categories (`0x360218c0-0x3602a600`) | 131 | 33.6 | `MoodManager`/`CMoodManager`/`CSoundMoods`, `Libs/SoundMoods/SoundMoods.xml`, DSP low-pass/EQ per category, `s_SoundMoods`, `s_SoundMoodsDSP` | **Used.** x2game `SoundMoodManager`/`CSoundMoods::Update`; zone combat/underwater/death moods; `sound_mood_combat_enable` | P5 | Already covered by `aa-audio` moods + `audio.md` §6 (soundmoods.xml, 31 categories). Add missing category mappings only. |
| Sound objects, assets and buffers (`0x36035db0-0x36045440`) | 272 | 56.8 | `CSound`, `CSoundAssetManager`, `CSoundBuffer*` (FMOD event/micro/network), bank decoder, streaming calls | **Used.** all event playback and music/voice streaming | P5 | Already covered by `aa-audio` `cache.rs`/`library.rs`/`stream.rs`/`fsb5.rs`/`fev.rs`. |
| `CSoundSystem` implementation: listener, 3D, virtualization, occlusion (`0x36045440-0x36061000`) | 300 | 108.3 | play/stop, channel stealing and virtual voices, 3D position/velocity, `RecomputeSoundOcclusion`, `s_Obstruction*`, `s_PlaybackFilter`, pause/resume, debug info | **Used.** 3D sounds and occlusion are audible in the client; `<Sound Obstruction>` and `s_Obstruction*` are live x2game strings | P5, P2 (sound cues) | Mostly covered by `aa-audio`; **port the occlusion/obstruction algorithm** (geometry + visibility, `s_Obstruction*`), which `aa-audio` currently approximates. |
| Music system (`0x36061ac0-0x360749a0`) | 280 | 72.7 | `CMusicSystem`/`Musicsystem`, themes/moods/patterns, `music/*.xml`, `s_MusicVolume`, `s_GameMusicVolume`, streaming, speaker volumes | **Used.** x2game `MusicManager::Update`, `PlayMusic`, login/zone music | P5 | Already covered by `aa-audio` music playback + `aa-client` `sound/`; `audio.md` §6 maps the data. |
| Reverb / EAX (`0x360749a0-0x3607d000`) | 246 | 32.7 | `CReverbManager`, `CReverbManagerEAX`, reverb presets, `s_ReverbType`, `s_GameReverbManagerPause`, ReverbVolume handling | **Used.** 587 `ReverbVolume` entities in world data (`audio.md` §2) | P5 | Covered by `aa-audio` `reverb.rs` presets; skip the EAX device path (FMOD software reverb only). |
| Sound-system misc (`0x3602a600-0x36035db0`) | 142 | 43.7 | helpers, category/name lookup, event property get/set, voice pools | **Used** indirectly by every event | P5 | Covered by `aa-audio`; no direct port. |
| Occlusion/obstruction, debug/profiling, containers, tail (`0x3607d000-0x36099800`) | 1,420 | 103.9 | occlusion raycasts and geometry (overlaps row 4), `s_soundInfo`/`s_DebugMusic`/`s_MusicProfiling`, stats, templates, CRT thunks | **Occlusion used**; debug/profiling is tooling | P5 (occlusion), none (debug) | Port only occlusion if not fully covered in row 4 (estimated ~154 f / ~21 KB); skip debug/profiling/editor helpers and CRT/template bodies. |

**crysoundsystem totals**

| bucket | f | KB |
|---|---:|---:|
| Already covered by `aa-audio` (device/buffers/events/music/moods/reverb/misc) | 2,002 | 468.6 |
| To port: sound occlusion/obstruction only | 154 | 20.8 |
| Skip (debug/profiling, EAX device path, containers/CRT) | 1,270 | 85.0 |
| **whole module** | **3,426** | **574.4** |

`aa-audio` is a reimplementation, not a line port: it decodes the same FSB/FEV data and reproduces
the mixer/music/mood behaviour. The only fidelity gap this survey finds is geometry/event occlusion;
everything else is covered or tooling.

---

## 5. cryfont

Whole module: **1,275 f / 144.7 KB**, image base `0x32500000`, one export
`CreateCryFontInterface`. CrySystem loads it dynamically (`GetProcAddress("CreateCryFontInterface")`,
`InitFont`, `CryFont.dll` strings in crysystem) so the engine font interface exists. It is **not**
the game's UI font path: x2game has its own `X2 Font System` (`InitFTFontSystem`, `LoadFTFont`,
`FontFileBufferManager`, `game/fonts/fonts.g`, `ui/font/*.ttf`) and X2UI/`aa-ui` read `fonts.g`
directly. CryFont serves engine/CryAction/CryMovie debug text and overlays. `aa-ui` already
implements the game text path (fonts.g chains, glyph atlas, kerning, layout), so there is nothing
to port here.

| Area (band) | f | KB | What it does | Runtime use (evidence) | Parity | Approach |
|---|---:|---:|---|---|---|---|
| CryFont interface, font objects, texture/atlas, effects, console commands (`0x32501000-0x32508000`) | 273 | 27.8 | `CreateCryFontInterface`, font/face objects, glyph atlas/texture, smooth/supersample/effect flags, `r_DumpFontNames`/`r_DumpFontTexture` | **Engine only.** Loaded and initialized by CrySystem; x2game game text uses its own X2 Font System, not this interface | none (UI tooling) | Covered by `aa-ui` `text.rs` (fonts.g, atlas, kerning, layout). Do not port. |
| XML font descriptor parser (`0x32508000-0x32512000`) | 413 | 39.8 | font XML/.g descriptors, value/geometry types, font name/effect attributes | **Engine only**, same as above | none | Covered by `aa-ui` (`fonts.g` parsing already done); no port. |
| Bundled FreeType core, drivers and tail (`0x32512000-0x32525d98`) | 589 | 77.1 | statically linked FreeType: TrueType/Type1/BDF/PS names, autohinter, glyph loader, rasterizer, memory/stream glue | **Engine only**; `aa-ui` and Bevy render text with Rust rasterizers | none | Skip. Use the existing `aa-ui` font stack / a Rust rasterizer; never port FreeType. |

**cryfont totals**

| bucket | f | KB |
|---|---:|---:|
| Covered by `aa-ui` text (interface + descriptor parsing) | 686 | 67.6 |
| Skip (bundled FreeType, debug texture dumps) | 589 | 77.1 |
| To port | 0 | 0 |
| **whole module** | **1,275** | **144.7** |

---

## 6. x2ui

Whole module: **936 f / 226.6 KB**, image base `0x10000000`. Exports `InitDesktop`,
`CleanupDesktop`, `CreateWidget`, `CreateTopLevelWidget`, `DeleteTopLevelWidget`, `FindWidget`,
`Show`, `RegisterWidgetEventHook`, `HandleScriptEvent`, `ListboxWidget::AppendItem/GetItem/
GetSelectedValue`, `Color4::Create`. x2game loads it from `FUN_3921db60` (`InitDesktop` xref
`399bf7a0`), runs `scripts/x2ui/x2ui.lua`, loads the top-level `ui/world.ui`, and exposes
`ScriptBindUI::CreateWidget`; the shipped UI is Lua-driven. The widget stack has **two layers**:
this DLL owns the desktop/top-level/`.ui`-data widgets, while the in-game widget manager and Lua
vocabulary live in x2game. Both are mapped in `docs/research/re-uitoolkit.md`; `crates/aa-ui`
reimplements the in-game stack and runs the original scripts. This survey only classifies the DLL.

| Area (band) | f | KB | What it does | Runtime use (evidence) | Parity | Approach |
|---|---:|---:|---|---|---|---|
| Desktop, top-level widgets, widget factory, `.ui` property parser (`0x10001000-0x10011000`) | 349 | 60.2 | `InitDesktop`/`CreateTopLevelWidget`/`CreateWidget`/`FindWidget`, widget types window/label/button/editbox/scrollbar/listbox/world, descriptor keys (`bounds`, `three_part`/`nine_part`, `font_path`/`font_size`, `align`, `padding`, `scrollbar`, `editbox`, `listbox`, `movable`, `resizable`, `dont_pick`), Lua `CreateFrame`/`UIParent`, listbox items | **Used.** x2game calls `InitDesktop`, loads `ui/world.ui` and `scripts/x2ui/x2ui.lua`; `UIC_*` windows and `X2UI:ScreenFader`; all screens | P5 (UI) | Already mapped by `re-uitoolkit.md` §2 and covered by `aa-ui` + `aa-lua`. No line port; only listbox/desktop edge cases remain. |
| Bundled FreeType (`0x10011000-0x10041000`) | 430 | 153.8 | statically linked FreeType (TrueType/Type1/CID/BDF, psnames, autohinter, rasterizer) for the `.ui` font paths | **Used indirectly** for text; replaced by the Rust stack | none | Skip. `aa-ui` `text.rs` rasterizes the same game fonts with a Rust rasterizer. |
| zlib/containers/CRT tail (`0x10041000-0x10046050`) | 156 | 12.5 | embedded inflate, `map/set/vector`, CRT glue | Tooling/infrastructure | none | Skip; Rust `flate2`/std replaces it. |

**x2ui totals**

| bucket | f | KB |
|---|---:|---:|
| Already mapped by `re-uitoolkit.md` + covered by `aa-ui`/`aa-lua` (widget/desktop/property layer) | 349 | 60.2 |
| Skip (bundled FreeType, zlib/containers/CRT) | 587 | 166.4 |
| To port | 0 | 0 |
| **whole module** | **936** | **226.6** |

---

## 7. x2common

Whole module: **341 f / 29.8 KB**, image base `0x10000000`. A small utility library exported by
`x2common.dll` and **also statically linked into x2game** (`Data::` appears 578 times in the
x2game decompile). x2ui imports 18 of its functions. It contains no game logic: `Data` (the
XML/`.g`/`.ui`-style tree parser and serializer), string/path/file/dir/memory-map helpers,
formatting/logging/string tables, perf zones/watches, pools/quick-sort/streams, UTF-8/char
helpers, and Win32 threads/mutex/semaphore/task queue.

| Area (band) | f | KB | What it does | Runtime use (evidence) | Parity | Approach |
|---|---:|---:|---|---|---|---|
| `Data` parser/serializer (`0x10001000-0x10003ca0`) | 65 | 9.7 | attribute tree, `Load`/`LoadFromFile`/`LoadFromString`, `Attribute`/`IntAttribute`/`FloatAttribute`/`BoolAttribute`/`StrAttribute`, `ChildAt`/`ChildWithValue`/`NextWithValue`, `Save`, comments/meta | **Used.** `.g`/`.ui`/cell XML data throughout x2game/x2ui; the same format feeds the UI font and layout files | P5 (UI/data) | Covered by existing parsers: `aa-ui` (`fonts.g`, `.ui`/layout data) and `aa-world`/`aa-cry` (`Data`-style XML). No new line port. |
| Path/file/dir/memory-map (`0x10003ca0-0x10004d70`) | 28 | 3.8 | `NormalizePath`, `ConcatPath`, `MakeRelPath`, `Extension`, `Dirname`, `Basename`, dir enumeration, `OpenMemoryMap`/`OpenAsyncMemoryMap` | **Used.** every file lookup goes through pak-relative paths; x2game/x2ui import many of these | P5 | Rust `std::path` + `aa-pak` cover it; no port. |
| Format/log/string table (`0x10004d70-0x100057c0`) | 11 | 2.2 | `PrintFormat`, `DefaultLogHandler`, `LogMsg`, `LoadStringTable`/`LocalString`, localization tables | **Used.** logs and localized UI strings | P5 | Covered by `tracing`/`log` + `aa-ui` locale; no port. |
| Perf zones/watches/timers (`0x100057c0-0x10005f00`) | 17 | 1.6 | `CreatePerfZone`, `BeginPerfBlock`, `InitWatches`, `StartWatch`, `FrameEnd`, `MsLastWindowAvg`, `ReadPerfTimer` | **Developer tooling.** x2game uses it for frame stats | none | Covered by `aa-dev`/tracing; skip the original reporting. |
| Pools/quick-sort/streams/strings (`0x10005f00-0x10006610`) | 39 | 1.3 | `InitPool`/`PoolAlloc`/`PoolFree`, `QuickSort`, `OpenFileInput`/`OpenMemoryInput`, `String` class | Infrastructure | none | Rust std/`aa-pak`; skip. |
| String/char/UTF-8 utils (`0x10006610-0x10007b10`) | 67 | 4.5 | `StrByteLen`, `CompareString`, `HashString`, `MultiToWide`/`WideToMulti`, `IsValidUtf8`, `ToInt`/`ToFloat`, hex helpers | **Used.** every name/path/attribute conversion | P5 | `std`/`encoding_rs`/`aa-ui` locale; no port. |
| System/threads/task queue/timer (`0x10007b10-0x10008a00`) | 46 | 2.6 | `CreateThread`, mutex/semaphore, `CreateTaskQueue`/`Schedule`, `ReadTimer`, `ExecCommand`, command line | Infrastructure | none | Bevy tasks/std threads; skip. |
| Tail/CRT/debug (`0x10008a00-0x1000ab20`) | 67 | 4.0 | CRT init, assertion/exception plumbing, manifest padding | Build/CRT | none | Skip. |

**x2common totals**

| bucket | f | KB |
|---|---:|---:|
| Covered by Rust std + `aa-ui`/`aa-world`/`aa-pak` parsers and helpers | 274 | 25.8 |
| Skip (perf reporting, pools/CRT/threads/assert plumbing) | 67 | 4.0 |
| To port | 0 | 0 |
| **whole module** | **341** | **29.8** |

---

## 8. xlcommon

Whole module: **1,760 f / 316.4 KB**, image base `0x33000000`, 1,890 export ordinals. This is the
client's foundation library: `Data` tree/parser, strings/UTF/locale, paths/files/memory maps,
hashes/checksums/crypto, random, pools/streams, threads/sync/timers, logging, **Breakpad crash
handling**, and a statically linked **libcurl 7.21.2** (HTTP/FTP/SMTP/LDAP/TFTP). x2game imports
many `Xl*` functions directly (and links the same code in places); x2ui imports 18. It is mostly
replaced by Rust std/crates, with only name-policy and exact checksum compatibility worth a port.

| Area (band) | f | KB | What it does | Runtime use (evidence) | Parity | Approach |
|---|---:|---:|---|---|---|---|
| Bit ops (`0x33001000-0x33002050`) | 22 | 3.0 | shift/rotate/window/print-bits helpers | Tooling/debug | none | Skip; Rust shifts. |
| `Data` tree parser/serializer (`0x33002050-0x33006530`) | 131 | 14.8 | `Data` attributes, children, values, load/save, pretty print | **Used.** same `.g`/`.ui`/XML data as x2common; x2game has 578 `Data::` calls | P5 | Covered by the same Rust parsers as x2common (`aa-ui`/`aa-world`). |
| File/dir/memory-map/time (`0x33006530-0x33008d70`) | 66 | 8.4 | `XlOpenFile/ReadFile/WriteFile`, dirs, junctions, file times, shared memory/memory maps | **Used.** every asset/file access | P5 | Rust `std::fs` + `aa-pak`; no port. |
| Name policy + base64 (`0x33008d70-0x330095c0`) | 7 | 1.8 | `XlIsValidName`, `XlIsValidAuctionSearchText`, `XlGetNamePolicyInfo`, `encode_base64`/`decode_base64` | **Used.** character naming and auction search validation; x2game imports these directly | P3, P5 | **Port line by line** (tiny): exact allowed-character/length rules and base64. |
| Hashes/checksums/crypto A (`0x330095c0-0x33014790`) | 74 | 39.1 | CityHash64/128, CRC32 table/generator, sumhash, Murmur 1/2/3, FNV, lookup3, MD5, SHA-1, SHA-256 crypt/password | **Used** for cache/file checks, names, auth helpers (`sha1_hmac`, `sha256_password`) | P5 (auth/data) | Use Rust crates for general hashing; **port exact CRC32/CityHash/Murmur variants only if a wire or file format requires the same value**. |
| Logging + memory manager (`0x330148b0-0x33016d70`) | 78 | 7.8 | `X2Log`/`X2Warning`/`X2Error`, file log handler, `CryMalloc`/`CryRealloc`/stats/heap queries | **Used.** every module logs through it | none | Covered by `tracing`/`log`; allocator stats are tooling. |
| Random (`0x33016d70-0x330177b0`) | 28 | 2.4 | `XLRandom`, `CSysPseudoRandGen`, Mersenne Twister, `CryRandom*` | Used by effects/gameplay jitter | P5 | Rust `rand`; port only if a deterministic replay/capture needs the exact stream. |
| Path utils (`0x330177b0-0x33018bb0`) | 44 | 4.6 | `XlMakeRelPath`, `XlExtension`, `XlSplitPath`, `XlNormalizePath`, save-game dirs | **Used.** asset/config paths | P5 | Rust `std::path`; no port. |
| Ping/pool/streams/locale/string (`0x33018bb0-0x3301c900`) | 187 | 12.8 | `XlPing` (server list probe), pools, memory streams, locale/codepage, `XlString*`, `XlFindString*`, `XlFormatString`, number conversion | **Used.** strings/paths/UI data; `XlPing` for server latency | P5 | Covered by `aa-net` (ping), `aa-ui` (locale/strings), Rust; port none except the name-policy row. |
| Threads/sync/time (`0x3301c900-0x3301e340`) | 68 | 5.8 | mutex/semaphore/event, thread affinity/priority, timers, clipboard | Infrastructure | none | Bevy/std; skip. |
| UTF-8/printf (`0x3301e340-0x330224d0`) | 59 | 12.8 | UTF-8/16/32 conversion, escape/printf helpers | **Used.** chat/UI text | P5 | `encoding_rs`/`std::fmt`; no port. |
| Crypto/CRC/Rijndael/Whirlpool (`0x33025400-0x3302cd50`) | 136 | 24.6 | `Crc32Gen`, `CSysPseudoRandGen`, AES/Rijndael, Whirlpool, SHA/CRC wrappers | Likely auth/cache; exact use needs an x2game call-site audit | P5 (auth) | Use Rust crates; port a variant only if the server/wire format requires byte-identical output. |
| Breakpad crash handler (`0x3302cd50-0x33033f10`) | 156 | 24.7 | `CreateDump`, callstack, exception handler, `.dmp`/`.crash`/ArcheAge.log, dbghelp | **Used** on crash; not gameplay | none | Skip; Rust panic hook/tracing if wanted. |
| libcurl 7.21.2 (`0x33033f10-0x3305c6ca`) | 664 | 146.0 | HTTP/HTTPS/FTP/SMTP/LDAP/TFTP/RTSP client, cookies, auth, `CLIENT libcurl 7.21.2` | **Used** by web/store/member features (x2game has `http://%s/store/buy`, member.xlgames.com URLs) and possibly crash upload; not core gameplay | none (P5 web) | Skip; use `reqwest` if a web feature is ever ported. The game login/game protocol is `aa-net`, not libcurl. |

**xlcommon totals**

| bucket | f | KB |
|---|---:|---:|
| To port (name policy + base64 + exact hash variants where required) | 57 | 21.8 |
| Covered by Rust std/crates + `aa-net`/`aa-ui`/`aa-pak` | 861 | 120.9 |
| Skip (libcurl, Breakpad, bit ops, CRT/tooling) | 842 | 173.7 |
| **whole module** | **1,760** | **316.4** |

The 146 KB libcurl and 24.7 KB Breakpad are almost half the module and are not parity work. The
only game-visible logic that is not a generic utility is the name/auction policy (1.8 KB).

---

## 9. xlleveldb

Whole module: **1,795 f / 235.9 KB**, image base `0x10000000`, 17 exports: `LevelDbInit`
(two overloads), `LevelDbLoad` (two), `LevelDbSave` (two), `LevelDbDelete` (two),
`LevelDbCleanup` (two) and `LevelDbEnum`. x2game imports eight of them (`XLLEVEDB.DLL` in
`imports.txt`) and uses them as a **local UI-settings key/value store**:
`leveldb initialized (folder: %s)`, `leveldb save failed for ui data : %d`,
`leveldb load failed for ui data : %d`, `leveldb init fail`, `ChatLog`. It is an embedded
**Google LevelDB + Chromium base** build (strings `D:\leveldb\src\...`, `env_chromium.cc`,
`MANIFEST-`, `CURRENT`, `LOG.old`, `VersionEdit`, compaction/table stats). No game logic.

| Area (band) | f | KB | What it does | Runtime use (evidence) | Parity | Approach |
|---|---:|---:|---|---|---|---|
| Public `LevelDb*` API (`0x10001000-0x10001a80`) | 12 | 2.5 | open/init a store, load/save/delete key, enumerate, cleanup | **Used.** x2game saves/loads UI data and chat-log state | P5 | Replace with `aa-client` addon/settings JSON (already exists) or a tiny key-value file; do not port the API. |
| LevelDB store/compaction/version (`0x10001a80-0x1000d000`) | 228 | 42.3 | DBImpl, version set, compaction, memtable/WAL recovery, write batches, iterators, snapshot | **Used indirectly** through the API | none | Skip; `redb`/JSON if an embedded KV store is ever needed. |
| LevelDB env/log/table A (`0x1000d000-0x10016000`) | 200 | 34.1 | env wrapper, log reader/writer, table builder/reader, comparator | **Used indirectly** | none | Skip. |
| LevelDB table/block/format (`0x10016000-0x1001f000`) | 188 | 33.7 | block format, sstable read/write, checksums, version edits | **Used indirectly** | none | Skip. |
| Chromium base (`0x1001f000-0x10046000`) | 609 | 98.6 | `env_chromium.cc`, file_util, platform_file/thread, time, command line, at_exit, histogram, registry, vlog | Infrastructure only | none | Skip; Rust std/`tracing` replaces it. |
| String/NSPR/CRT tail (`0x10046000-0x1004fc00`) | 558 | 24.5 | Chromium string/number/UTF helpers, NSPR `prtime`, CRT/format tail | Infrastructure only | none | Skip. |

**Runtime keys.** The UI data keys are produced by `FUN_398a7fd0` and the values are `Data`
buffers (`Data::SaveToBuffer`, `version` attribute), i.e. the same `.g`-style UI option trees
already parsed by `aa-ui`. `aa-client` currently persists addon settings as JSON under
`%LOCALAPPDATA%/AABevy1.2/addons/settings/`, so the Bevy client does not need LevelDB for parity.

**xlleveldb totals**

| bucket | f | KB |
|---|---:|---:|
| Covered by `aa-client` settings/JSON (public API surface) | 12 | 2.5 |
| Skip (LevelDB engine + Chromium base + CRT tail) | 1,783 | 233.4 |
| To port | 0 | 0 |
| **whole module** | **1,795** | **235.9** |

---

## 10. cry3dengine (coarse: rendering vs game logic)

Whole module: **10,100 f / 2,221.3 KB**, image base `0x30000000`. This is the world/scene engine
behind `gEnv+0x54` (I3DEngine). It mixes **rendering** (Nick's area, out of scope) with the
**world data and queries** already implemented in `aa-world` (terrain, vegetation, brushes,
voxels, vis areas/portals, water, time of day, streaming) and `aa-cry` (CGF/mesh/material
parsing). x2game's game-relevant call sites are the prefab loaders (`VisAreaPrefabObject::Load`,
`WaterVolumePrefabObject::Load`, `AIPoint`/vegetation placement) and terrain queries used by
housing (`house_cannot_locate_terrain_too_low/high`), movement and camera. Internal cvars:
`e_terrain*`, `e_vegetation*`, `e_water*`, `e_shadows*`, `e_time_of_day`, `r_*`, `option_terrain_lod`,
`option_view_dist_ratio_vegetation`, `rwi_ignore_terrain_holes`.

| Area (band) | f | KB | What it does | Runtime use / evidence | Parity | Approach |
|---|---:|---:|---|---|---|---|
| Core init, statics, math/templates before the first class (`0x30001010-0x30010000`) | 117 | 58.7 | DLL/allocator init, globals, small math helpers | Infrastructure | none | Skip. |
| Indexed mesh / light shapes / mesh chunks (`0x30010000-0x3001e440`) | 170 | 54.4 | `CIndexedMesh`, submesh/light-shape chunks, material ids | **Used** through CGF loading | P1/P5 (visible meshes) | Covered by `aa-cry` CGF/mesh parsing; renderer upload is out of scope. |
| Static objects + foliage (`0x3001e440-0x30034680`) | 221 | 83.3 | `CStatObj`, `CStatObjFoliage`, CGF instances, LOD/physics proxies, placement | **Used.** static brush dressing and trees in every cell | P3/P5 | Covered by `aa-world` (placement/collision) + `aa-cry` (CGF). Render LOD out of scope. |
| Engine core + shadows A (`0x30034680-0x30047fc0`) | 138 | 67.7 | `C3DEngine` early frame, `CCoarseShadowManager`, shadow maps | **Used** for rendering only | none | Rendering (Nick), out of scope. |
| Decals (`0x30047fc0-0x30061e20`) | 155 | 98.9 | `CDecalManager`, `CDecal`, projected decals | **Used** for footprints/blood/etc. | P2/P5 (visual) | Data/trigger side can use `aa-fx`/`aa-world`; rendering out of scope. |
| Particles + physics environment (`0x30061e20-0x3009fb10`) | 771 | 239.2 | `CParticleContainer`/`Emitter`/`PartManager`, particle params, `SPhysEnviron`, primitives | **Used** for all effects; physics environment feeds collision | P2/P5 | Covered by `aa-fx` (particle data) and `aaemu-physics` (phys environ); particle rendering out of scope. |
| Sky / time of day / clouds / fog / volumes (`0x3009fb10-0x300c1a00`) | 388 | 128.9 | `TimeOfDayManager`, `CTimeOfDay`, `CSkyLightManager`/`Nishita`, clouds, fog volumes, irradiance volumes | **Used.** server hour -> `e_time_of_day`; zone lighting/sky/fog | P5 | Covered by `aa-world` time-of-day (`docs/research/time-of-day.md`); sky/fog/cloud rendering out of scope. |
| Terrain / heightmap / ocean / water (`0x300c1a00-0x300f13d0`) | 467 | 184.9 | `CTerrain`, `CTerrainNode`, `CHeightMap`, hole/LOD queries, terrain modifications, ocean/water volumes/waves | **Used.** height/hole queries for movement, housing, camera; water level for swimming and housing | P1/P3/P5 | Covered by `aa-world` (`TerrainQuery::height_at`, holes, water/ocean, LOD). No port. |
| Vegetation + brush (`0x300f13d0-0x301092e0`) | 256 | 92.5 | `CVegetation`, `VegetationManager`, `StreamVegetation`, `CBrush`, `StreamBrush` | **Used.** cell vegetation/brushes; housing removes vegetation; brush collision | P3/P5 | Covered by `aa-world` (vegetation groups, brush/object.dat, collision). Render instancing out of scope. |
| Object manager + streaming (`0x301092e0-0x3014d830`) | 737 | 264.3 | `CObjManager`, stat-obj/vegetation/XML/CGF/material stream workers, object octree | **Used.** every cell streams objects around the camera | P1/P5 | Covered by `aa-world` streaming + `aa-cry` loaders. Do not port the worker/task machinery 1:1. |
| Octree / renderer / visibility (`0x3014d830-0x30157cd0`) | 66 | 40.2 | `COctreeNode`, `Renderer`, render lists | **Used** for drawing only | none | Rendering (Nick), out of scope. |
| Deferred loading / physics streamer (`0x30157cd0-0x30170ce0`) | 268 | 87.0 | `DeferredLoading*`, `OutDoorObjectLoadTask`, `CPhysStreamer` | **Used.** streams collision/physics proxies | P1/P3 | Covered by `aa-world` + `aaemu-physics`; replace with Bevy async. |
| Vis areas + occlusion culling (`0x30170ce0-0x30192d40`) | 227 | 133.7 | `CVisAreaManager`/`CVisArea`, portals, `COcclusionCuller`, W/Z-buffer cullers | **Used.** interiors, portals, area sky/light/fog flags; x2game `VisAreaPrefabObject::Load` | P3/P5 | Area/portal data is covered by `aa-world` (`visareas.dat`/portals); culling is rendering, out of scope. |
| Voxels (`0x30192d40-0x301a5490`) | 218 | 71.0 | `CVoxelObject/Volume/Mesh`, compiled voxel base + projected materials | **Used.** housing/terrain voxel objects | P3 | Covered by `aa-world` voxel docs; render path out of scope. |
| Material / CGF loader / render mesh (`0x301a5490-0x301fdb20`) | 829 | 344.9 | `CMatMan`, `CLoaderCGF`, chunk files, `CryGeometryInfo`, `CRenderMeshMerger` | **Used** for every visible model | P1/P5 | Covered by `aa-cry` (CGF/mtl/mesh); render-mesh construction/merging out of scope. |
| Lights / roads / ropes / lightmaps (`0x301fdb20-0x3022b7c0`) | 411 | 178.0 | `CLightEntity`, `CRoadRenderNode`, `CRopeRenderNode`, `CLMSerializationManager`, mesh-subset chunks | **Used** for rendering and world dressing | P5 (visual) | Road/rope data belongs to `aa-world`; light/lightmap rendering out of scope. |
| Template/utility tail (`0x3022b7c0-0x3024bfc0`) | 4,660 | 93.7 | 4,600+ tiny template/thunk bodies | Infrastructure | none | Skip. |

**cry3dengine totals**

| bucket | f | KB |
|---|---:|---:|
| Covered by `aa-world` (terrain, water, vegetation/brush, voxels, vis areas, TOD, streaming) | 2,293 | 875.3 |
| Covered by `aa-cry`/`aa-fx` (mesh/CGF/material, particles, decals, static objects, deferred loading) | 2,414 | 907.7 |
| Rendering out of scope (shadows, octree/render lists, lights, lightmaps; Nick's area) | 615 | 285.9 |
| Skip (init/statics, template tail) | 4,778 | 152.4 |
| **whole module** | **10,100** | **2,221.3** |

No new game-logic port is needed here: the terrain/vegetation/brush/visarea/time-of-day logic is
already in `aa-world`, the mesh/material parsing in `aa-cry`, and the rest is rendering. The only
reason to open this DLL is to confirm a specific query (height, water level, area flags) against
`aa-world`'s implementation.

---

## 11. cryrenderd3d10 (coarse: rendering only)

Whole module: **12,525 f / 2,670.3 KB**, image base `0x38000000`, exports
`PackageRenderConstructor`, `CryModuleGetMemoryInfo` and CRT shims. This is the Direct3D 10/9
renderer behind `gEnv+0x34`: device/context/swap chain, shaders and shader binaries, textures and
DDS images, render meshes/particles/water/imposter render elements, deferred shading, post
effects and shadows, video playback, auxiliary geometry, render thread and stereo. x2game touches
it only through `IRenderer` (resolution, viewport, graphics options, screenshots, debug draw);
there is **no game logic here** — terrain height, vegetation/brush collision, vis areas and time of
day live in cry3dengine/aa-world, and mesh/material parsing in aa-cry.

| Area (band) | f | KB | What it does | Runtime use (evidence) | Parity | Approach |
|---|---:|---:|---|---|---|---|
| Frame/renderer core + render thread (`0x38010000-0x38043000`, `0x38057000-0x380a0000`, `0x38170000-0x381a0000`) | 2,154 | 661.5 | `CD3D9Renderer`/`CRenderer` frame, viewport, render lists, render-objects, `SRenderThread` | **Used** every frame; `gEnv+0x34`, `InitRenderer`, `Render` in E1 | none | Rendering (Nick), out of scope. |
| Deferred shading / post effects / shadows (`0x38043000-0x38057000`) | 148 | 69.4 | `CDeferredShading`, post-process chain, shadow maps | **Used** for the final image | none | Out of scope; Bevy renderer. |
| D3D device/context/buffers (`0x380a0000-0x380b0000`, `0x38220000-0x38280000`) | 1,076 | 417.1 | `D3DDevice`, `D3DDeviceContext`, `CDeviceManager`, `CDevBufferMan`, `CDeviceTexture`, dynamic VB/IB, GPU events | **Used** for all drawing | none | Out of scope. |
| Shaders and shader binaries (`0x380d0000-0x38100000`) | 438 | 185.7 | `CHWShader_D3D`, `CShader`, `CShaderMan`, `CShaderManBin`, `CParserBin`, shader combinations/cache | **Used** for all materials | none | Out of scope; Bevy/`naga` handles shaders. |
| Textures / resources / streaming (`0x38100000-0x38170000`) | 1,236 | 428.6 | `CTexture`, `CBaseResource`, `CTextureStreamCallback`, resource files | **Used.** all DDS/mesh textures | none (data format) | Device/texture upload is out of scope; DDS decode is `aa-cry`/Bevy's image loader, not this device code. |
| Render meshes / geometry / aux / particles / water / video / DDS / shader parser (`0x38001010-0x38010000`, `0x380b0000-0x380d0000`, `0x381a0000-0x38220000`) | 1,279 | 659.8 | `CRenderMesh`, `CREMeshImpl`, imposters, `CREParticle`, `CREWaterOcean`, `CWaterVolume`/`CWaterSim`, `CRenderAuxGeomD3D`, `CVideoPlayer`, `CImageDDSFile`/`CImageExtensionHelper` | **Used** for rendering; video for the rare `.usm`/movie path | none | Out of scope; mesh/material data from `aa-cry`, video skipped unless a shipped cutscene needs it. |
| Template/utility tail (`0x38280000-0x382d2bd0`) | 6,194 | 248.1 | 6,000+ tiny template/thunk/CRT bodies and unassigned helpers | Infrastructure | none | Skip. |

**cryrenderd3d10 totals**

| bucket | f | KB |
|---|---:|---:|
| Rendering, out of scope (Nick's area) | 12,525 | 2,670.3 |
| Covered by `aa-cry`/Bevy (DDS/mesh data formats only; not device code) | — | — |
| To port | 0 | 0 |
| **whole module** | **12,525** | **2,670.3** |

This module is pure rendering; nothing in it changes world state or gameplay. The parity work that
touches its data is texture/DDS decoding (already in `aa-cry`/Bevy) and the shader/material
descriptions that `aa-cry` reads from the pak.
