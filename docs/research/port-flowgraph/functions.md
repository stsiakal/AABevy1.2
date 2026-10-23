# Flow graph / cutscene functions (1.2)

Every entry below is a function the port has to reproduce or call. Addresses are VAs in the
shipped 1.2 DLLs (x2game base `0x38FF0000`, CryAction `0x304F0000`, CryMovie `0x34000000`);
sizes are `funcs.csv` byte counts. "callers/callees" are function entries (x2game call sites were
mapped from `calls.tsv` to their containing function). "—" means the call is virtual, through a
vtable or a callback pointer, so it has no direct caller in the decompile. Purity is about the
function, not about the whole subsystem.

The CryAction and CryMovie entries cite the decompile file that contains them; x2game entries
cite `x2game.dll.c`. Struct offsets are in `structs.md`, the per-class node map in `nodes.md`.

## Summary

| module | entries | bytes |
|---|---:|---:|
| CryAction (`cryaction.dll.c`) | 51 | 22,678 |
| CryMovie (`crymovie.dll.c`) | 14 | 5,369 |
| x2game (`x2game.dll.c`) | 59 | 32,939 |
| **total** | **124** | **60,986** |

## CryAction: the flow-graph runtime

These are the functions an interpreter must mirror. Node classes themselves are listed in
`nodes.md`.

### CryAction flow-graph runtime (1.2 `cryaction.dll`, base `0x304F0000`)

| address | size | role | callers | callees | fields / globals | purity |
|---|---:|---|---|---|---|---|
| `0x30546380` | 596 | CFlowGraph entity resolver (GetFlowgraphForwardingEntity) | — | — | port flags 0x1000/0x2000; graph+0xd8/+0xdc | stateful |
| `0x30549cc0` | 347 | CFlowSystem::UpdateStats | — | — | DAT_3077d774/d778/d77c counters | stateful |
| `0x3054eb40` | 984 | CFlowGraphBase::DoUpdate | — | — | graph+0x54/+0x56/+0x6c/+0x70/+0x124/+0x130 | stateful |
| `0x30551a00` | 1030 | CFlowGraph ctor | — | — | graph+0..+0x13c | stateful |
| `0x30552000` | 1048 | CFlowGraph::CreateNode | — | — | node type/factory; 0x20-byte port slot | stateful |
| `0x305525f0` | 1166 | CFlowGraph XML init (SerializeXML) | — | — | Nodes/Edges/Node/Edge | stateful |
| `0x30553360` | 35 | CFlowGraph::Init(bool) -> XML init | — | — | -- | stateful |
| `0x30553b70` | 629 | CFlowGraphBase::PerformActivation (variant) | — | — | port list; firstEdge | stateful |
| `0x30556640` | 671 | CFlowSystem::Update | — | — | system+0x34..0x38 graphs; +0x70 debug | stateful |
| `0x30556f40` | 240 | get a preloaded graph by name | — | — | preload map | pure-read |
| `0x30558a70` | 993 | preload /libs/flownodes/preload.txt | — | — | file system | stateful |
| `0x30559170` | 557 | CFlowSystem::RegisterType(name, factory, category) | — | — | type array +0x24; max 4095 | stateful |
| `0x3055b0a0` | 143 | CFlowBaseNode::GetPortBool | — | — | -- | pure-read |
| `0x3055bed0` | 194 | CFlowBaseNode::GetPortString | — | — | -- | pure-read |
| `0x3055bfa0` | 201 | CFlowNode_PlayDoodadParticleEffect | — | — | doodad entity + particle name | stateful |
| `0x3055c070` | 672 | flowgraph node: teardown / end | — | — | -- | stateful |
| `0x3055c310` | 279 | flowgraph node: end variant | — | — | -- | stateful |
| `0x30561910` | 421 | CFlowCompositeNodeFactory | — | — | -- | stateful |
| `0x305637d0` | 348 | System:ConsoleVariable GetConfiguration | — | — | -- | pure-init |
| `0x305684d0` | 670 | Image:DepthOfField GetConfiguration | — | — | -- | pure-init |
| `0x3056dac0` | 159 | CFlowNode_EntitySetProperty | — | — | -- | stateful |
| `0x30573ce0` | 265 | Entity:TimeOfDayArea GetConfiguration | — | — | -- | pure-init |
| `0x305742a0` | 222 | Entity:BroadcastEvent GetConfiguration | — | — | -- | pure-init |
| `0x30574d40` | 210 | CFlowEntityClass registration | — | — | -- | stateful |
| `0x30578680` | 175 | CFlowActorSetHealth | — | — | -- | stateful |
| `0x30578e30` | 260 | Game:LocalPlayer GetConfiguration | — | — | -- | pure-init |
| `0x30578f40` | 438 | Game:IsPlayerInRange GetConfiguration | — | — | -- | pure-init |
| `0x30579900` | 373 | Game:FindDoodadPos GetConfiguration | — | — | -- | pure-init |
| `0x3057a9c0` | 375 | HUD:DisplayTimedDebugMessage GetConfiguration | — | — | -- | pure-init |
| `0x3057eb40` | 400 | Logic:Any GetConfiguration | — | — | -- | pure-init |
| `0x3057ef70` | 422 | Logic:All GetConfiguration | — | — | -- | pure-init |
| `0x305871d0` | 376 | Math:Equal GetConfiguration | — | — | -- | pure-init |
| `0x30587350` | 376 | Math:Less GetConfiguration | — | — | -- | pure-init |
| `0x30587e70` | 289 | Math:ToBoolean GetConfiguration | — | — | -- | pure-init |
| `0x305890f0` | 280 | Vec3:ScaleVec3 GetConfiguration | — | — | -- | pure-init |
| `0x305915d0` | 192 | Animations:PlayAnimation config (AI variant?) | — | — | -- | pure-init |
| `0x305935a0` | 137 | CFlowNode_AIBase::RegisterEvents | — | — | -- | stateful |
| `0x305b6580` | 320 | Physics:ActionImpulse GetConfiguration | — | — | -- | pure-init |
| `0x305b8e50` | 434 | CFlowScriptedNodeFactory | — | — | SimpleScript .node files | stateful |
| `0x305c04b0` | 519 | Time:TimeOfDay GetConfiguration | — | — | -- | pure-init |
| `0x305c06c0` | 384 | Time:Time GetConfiguration | — | — | -- | pure-init |
| `0x305c0840` | 311 | Time:TimeOfDayTrigger GetConfiguration | — | — | -- | pure-init |
| `0x305c4480` | 402 | Movement:MoveEntity (between) GetConfiguration | — | — | -- | pure-init |
| `0x305c5d50` | 490 | Movement:RotateEntity GetConfiguration | — | — | -- | pure-init |
| `0x305c67d0` | 388 | Movement:RotateTo GetConfiguration | — | — | -- | pure-init |
| `0x305c77e0` | 469 | Movement:MoveEntityTo GetConfiguration | — | — | -- | pure-init |
| `0x305ccf40` | 522 | Animations:PlayAnimation GetConfiguration | — | — | Start/Stop/anim_Animation/BlendInTime/loop/... | pure-init |
| `0x305ce210` | 372 | Cinema:PlaySequence ProcessEvent | — | — | node+0xc IMovieSequence*, +0x28 m_bPlaying | stateful |
| `0x305ce390` | 618 | Cinema:PlaySequence GetConfiguration | — | — | seq_Sequence_File/Trigger/Stop/... | pure-init |
| `0x305cf5d0` | 584 | Sound:PlaySoundEvent GetConfiguration | — | — | -- | pure-init |
| `0x30683c40` | 692 | MaterialFX:HUDStartFX GetConfiguration | — | — | -- | pure-init |

### CryMovie TrackView player (1.2 `crymovie.dll`, base `0x34000000`)

| address | size | role | callers | callees | fields / globals | purity |
|---|---:|---|---|---|---|---|
| `0x34002620` | 625 | sequence XML loader (`CSequence::Load` path) | — | — | world/name paths; cinema_only_objs | stateful |
| `0x340028a0` | 369 | CSequence ctor | — | — | +0x40 name, +0x44 file | stateful |
| `0x340053b0` | 96 | CreateMovieSystem | — | — | -- | stateful |
| `0x3400bd30` | 102 | CMovieSystem::PlaySequence(name,...) | — | — | finds by name, vtable+0x4c | stateful |
| `0x3400bda0` | 68 | IMovieSystem vtable+0x74 IsPlaying | — | — | -- | pure-read |
| `0x3400bdf0` | 90 | IMovieSystem vtable+0x84 IsPlayingSequence | — | — | -- | pure-read |
| `0x3400c630` | 67 | IMovieSystem vtable+0x2c GetSequenceFlags | — | — | -- | pure-read |
| `0x3400dcc0` | 812 | CMovieSystem::Update | — | — | active sequence list +0x3c | stateful |
| `0x3400e200` | 727 | IMovieSystem vtable+0x4c PlaySequence | — | — | -- | stateful |
| `0x3400e4e0` | 159 | IMovieSystem vtable+0x18 FindSequence | — | — | -- | pure-read |
| `0x3400ee90` | 556 | end cinema / StopSequence | — | — | -- | stateful |
| `0x3400f0c0` | 1501 | CMovieSystem ctor | — | — | vtable 0x3404d1a8 | stateful |
| `0x3400f700` | 115 | IMovieSystem vtable+0x34 AddMovieListener | — | — | -- | stateful |
| `0x34027680` | 82 | sequence object/entity lookup | — | — | name | pure-read |

### x2game (1.2 `x2game.dll`, base `0x38FF0000`)

| address | size | role | callers | callees | fields / globals | purity |
|---|---:|---|---|---|---|---|
| `0x39055ef0` | 37 | quest component cinema trigger | — | — | component state | stateful |
| `0x39056040` | 197 | delayed quest cinema start | — | — | component+0/+4/+8/+0x28 | stateful |
| `0x39056250` | 75 | start quest cinema during loading | — | — | component name | stateful |
| `0x39084ac0` | 101 | CG2AutoRegFlowNode registration thunk size 0xc | — | — | -- | pure-init |
| `0x390959f0` | 651 | flowgraph skill-controller ctor | `0x390ca700` | `0x3950a1b0` | controller+0x18 0x124-byte state block, +0x13c graph | stateful |
| `0x39095cd0` | 269 | flowgraph skill-controller end events | `0x3909a820` | — | graph [0x4f]; DAT_39ae4f5c | stateful |
| `0x39095e70` | 90 | flowgraph skill-controller set values | `0x390959f0`, `0x39095e00` | — | controller+0x2c/+0x30/+0x34 | stateful |
| `0x39095f40` | 237 | skill-controller helper (role inferred) | — | — | params | stateful |
| `0x390ca700` | 1610 | Actor::StartSkillController kind 1..8 | `0x390ca5f0`, `0x39389440`, `0x39389b70`, `0x39397ff0`, `0x3939b010` | `0x39094b50`, `0x39094cb0`, `0x39095160`, `0x390958f0`, `0x390959f0`, `0x39096b90`, `0x39096e70`, `0x390992c0` … | desc+0x10 kind, desc+0x18 str_value1 | stateful |
| `0x3914a130` | 101 | CG2AutoRegFlowNode registration thunk (PlayerLink cluster) | — | — | -- | pure-init |
| `0x3914a790` | 101 | CG2AutoRegFlowNodeSingleton registration thunk (ctor 0x3914a720) | — | — | -- | pure-init |
| `0x3914a920` | 101 | CG2AutoRegFlowNodeSingleton registration thunk (Difficulty) | — | — | -- | pure-init |
| `0x3914ae50` | 856 | Game:ActorSensor::GetConfiguration | — | — | 0x39afc210/0x39afc130 | pure-init |
| `0x3914b1b0` | 398 | Game:DifficultyLevel::GetConfiguration | — | — | 0x39afc2b0/0x39afc260 | pure-init |
| `0x3914b3f0` | 363 | Camera:OverrideFOV::GetConfiguration | — | — | 0x39afc318/0x39afc2e4 | pure-init |
| `0x3914ba90` | 101 | CG2AutoRegFlowNode registration thunk (OverrideFOV) | — | — | -- | pure-init |
| `0x3914cd30` | 604 | CrysisFX:ScreenFader::GetConfiguration | — | — | 0x39afc3c8/0x39afc388 | pure-init |
| `0x3914cf90` | 101 | CG2AutoRegFlowNode registration thunk (ScreenFader) | — | — | -- | pure-init |
| `0x3914d170` | 101 | CG2AutoRegFlowNode registration thunk (HitInfo) | — | — | -- | pure-init |
| `0x3914d350` | 101 | CG2AutoRegFlowNodeSingleton registration thunk size 0x10 | — | — | -- | pure-init |
| `0x3914e520` | 458 | Game:PlayerStaging::GetConfiguration | — | — | port tables in .data | pure-init |
| `0x3914e6f0` | 441 | Game:PlayerLink::GetConfiguration | — | — | port tables in .data | pure-init |
| `0x3914fed0` | 101 | CG2AutoRegFlowNode registration thunk (PlayerStaging) | — | — | -- | pure-init |
| `0x3914ff40` | 101 | CG2AutoRegFlowNode registration thunk (PlayerLink) | — | — | -- | pure-init |
| `0x3915b160` | 23 | set camera actor target unit | — | — | DAT_39afe2b8+0x1b0 | stateful |
| `0x3915b180` | 13 | query quest-camera mode | — | — | DAT_39afe2b8+0x1a8 | pure-read |
| `0x3915beb0` | 399 | restore normal camera + DoF defaults after quest camera | `0x39161540`, `0x39164ff0` | `0x390b3750`, `0x390b3ad0` | camera actor fields | stateful |
| `0x39161540` | 325 | SetCameraType(type) | `0x39164ff0`, `0x3916b440` | `0x39007240`, `0x3907afc0`, `0x3915beb0`, `0x39360020`, `0x393fe030` | +4 new type, +0x194 old, +0x1a8 quest flag | stateful |
| `0x39164ff0` | 1061 | SetQuestCamera(enable, cameraId) | `0x391673a0` | `0x3907ac50`, `0x390b3750`, `0x390b3ad0`, `0x39163270`, `0x39360020` | actor+0x1a8..+0x2ac | stateful |
| `0x3916aab0` | 421 | X2::GameClient::CameraManager::Update(dt) | `0x3916ac70` | `0x39004bb0`, `0x3915e2c0`, `0x391632d0`, `0x39164100`, `0x39166390`, `0x39169860`, `0x3916a260` | camera manager+4 mode | stateful |
| `0x3916b440` | 1936 | cinematic camera actor update incl. quest camera | `0x3916bc70` | `0x39098170`, `0x390b3750`, `0x390c0cf0`, `0x3915a9e0`, `0x39161540`, `0x39163270`, `0x3916a260`, `0x3933f5e0` … | actor+0x1a8..+0x2ac; quest_cameras desc | stateful |
| `0x3916bbd0` | 158 | create/register the cinematic camera actor | `0x3915e7a0` | `0x3916ad80`, `0x3950a1b0` | DAT_39afe2b8, ctor 0x3916ad80, vtable 0x399b0a90 | stateful |
| `0x391d4c30` | 157 | OnUnitState handler; drops unit state during a cutscene | — | `0x390115a0`, `0x390116d0`, `0x391b90a0` | packet+0x10 unit, packet+0x20b0 state | stateful |
| `0x391e2a70` | 524 | TryStartCinema(name, world, fade) | `0x39056040`, `0x39056250`, `0x391e4250`, `0x39407ff0`, `0x39495550` | `0x39007240`, `0x3901b930`, `0x3901bdd0`, `0x3901bde0`, `0x39186810`, `0x39186da0`, `0x39415d10`, `0x39415ec0` | movie system at DAT_3a5bb1d8+0x3c | stateful |
| `0x391e4250` | 129 | OnPlaySequence packet handler (SCPlaySequence 0x13b) | — | `0x390115a0`, `0x390116d0`, `0x391e2a70`, `0x39325120` | packet+0xc name, packet+0x2c id -> DAT_3a167e80 | stateful |
| `0x39325120` | 13 | store SCPlaySequence sequence/actor id | `0x391e4250` | — | DAT_3a167e80 | stateful |
| `0x39379c70` | 423 | NPC talk start; enable quest camera | `0x3949b2b0`, `0x3949b4e0`, `0x3949b5c0` | `0x3915b160`, `0x3915b180`, `0x391673a0`, `0x39360020`, `0x393700c0`, `0x39370900`, `0x39371970`, `0x393758d0` … | desc lookups; FUN_3915b160; FUN_391673a0(1,id) | stateful |
| `0x393f6fc0` | 956 | cinematic screen fade/letterbox state machine | — | — | +8 state, +0x14..+0x3c rect/alpha | stateful |
| `0x39407e40` | 393 | OnStopMovie listener stop | — | — | name, 11-entry prefix table 0x39ac9150 | stateful |
| `0x39407ff0` | 370 | TryStartCinemaNoFade(name, world) | `0x394085c0`, `0x39408630`, `0x39408750`, `0x39408850`, `0x39408950`, `0x394089d0`, `0x39408a00`, `0x39408ae0` … | — | movie system; world arg | stateful |
| `0x39416380` | 101 | CG2AutoRegFlowNode registration thunk (X2UI:ScreenFader) | — | — | -- | pure-init |
| `0x39495550` | 57 | Lua PlayCinema handler: TryStartCinema(name, "main_world", 0) | — | — | name arg | stateful |
| `0x39499170` | 4875 | Lua client-camera/player binder incl. PlayCinema | — | — | -- | stateful |
| `0x3949b190` | 36 | Lua EnterQuestDirectingMode | — | — | DAT_39d29288; DAT_39ac67dc=7 | stateful |
| `0x3949b1c0` | 36 | Lua LeaveQuestDirectingMode | — | — | FUN_391673a0(0,...) | stateful |
| `0x3949b1f0` | 29 | Lua AcceptDirectingQuest | — | — | FUN_3937caa0(1,-1) | stateful |
| `0x3949b210` | 29 | Lua DeclineDirectingQuest | — | — | FUN_3937caa0(0,-1) | stateful |
| `0x3949b270` | 49 | Lua X2Quest:SetUseQuestCamera | — | — | DAT_39afe2a0 vtable+0x14 | stateful |
| `0x3949d790` | 112 | Lua IsQuestDirectingMode | — | — | FUN_3921ad60 | pure-read |
| `0x3949dab0` | 212 | Lua X2Quest:UseQuestCamera | — | — | DAT_39afe2a0 vtable+8 | pure-read |
| `0x3949db90` | 241 | Lua X2Quest:GetMainQuestCutSceneReplayList | — | — | cinemas replay rows | stateful |
| `0x394a6e50` | 10066 | Lua X2Quest binder | — | — | -- | stateful |
| `0x3962ccc0` | 50 | lookup a doodad flowgraph row | `0x3962cd00`, `0x39675580`, `0x39675770` | — | range table | pure-read |
| `0x3966fbd0` | 401 | LoadDoodadFuncPlayFlowGraphDescs | `0x3960b820` | `0x3953a390`, `0x3955b3d0`, `0x3955fb10` | client+0x82c8/+0x82cc | stateful |
| `0x3971e060` | 773 | LoadSkillControllerDescs | `0x3960b820` | `0x3953a310`, `0x3953a390`, `0x3953a480`, `0x3955b3d0`, `0x3955fb10` | DB map +0x127c; 24-dword rows | stateful |
| `0x398426c0` | 54 | expose model->quest-camera table | `0x3916b440` | — | +0xc464/+0xc470/+0xc474 | pure-read |
| `0x398562e0` | 62 | quest camera desc lookup by id | `0x3916b440` | — | table +0xc3ac | pure-read |
| `0x39892880` | 743 | LoadQuestCameraDescs | `0x3960b820` | `0x3953a310`, `0x3953a480`, `0x3955b3d0`, `0x3955fb10` | client+0xc3a8; 0x3c-byte desc | stateful |
| `0x39892b70` | 416 | LoadModelQuestCameraDescs | `0x3960b820` | `0x3953a390`, `0x3955b3d0`, `0x3955fb10` | client+0xc470/+0xc474 | stateful |
