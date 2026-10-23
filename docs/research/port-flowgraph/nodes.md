# Node inventory: every flow-graph class 1.2 content instantiates

Method. The 1.2 pak was scanned for every XML file that can contain a graph: level
`cells/*/client/entities.xml` (1,294 files), `game/prefabs/*.xml` (235),
`game/worlds/*/cinemas/*.xml` (513 - only the `_cinema_only_objs.xml` files carry graphs),
`game/libs/flownodes/*.xml` (14) and `game/libs/materialeffects/flowgraphs/*.xml`. Every
`<Node Class="...">` inside a `<FlowGraph>` or `<Graph>` block was counted. Result: **1,529 graphs,
10,295 nodes, 9,793 edges, 107 class names**. The one extra name over the first pass is
`Game:ActorCheckHealth` (1 node in a material-effect graph), a Crysis leftover, not an
ArcheAge gameplay node.

Host attribution. `x2game` means a class whose `GetConfiguration`/vtable lives in `x2game.dll`
(the five game/camera nodes plus the Crysis/PostFX leftovers); `CryAction` means the engine
node table at `0x30777330`; `CryAction (X2)` means the ArcheAge nodes implemented inside
CryAction (`X2:*` string table at `0x3072a4d8`); `CryAction (Game)` means the
`Game:LocalPlayer`/`FindDoodadPos`/`IsPlayerInRange` nodes. Evidence: class-name strings and
config function addresses are cited in the section after the table.

## All classes by usage

| class | nodes | host | families |
|---|---:|---|---|
| `Time:TimeOfDayTrigger` | 1498 | CryAction | prefabs=1400, level entities=90, libs/flownodes=8 |
| `Animations:PlayAnimation` | 996 | CryAction | prefabs=774, level entities=222 |
| `Logic:Any` | 947 | CryAction | prefabs=738, level entities=174, libs/flownodes=18, materialeffects=17 |
| `Start` | 627 | CryAction | prefabs=472, level entities=144, libs/flownodes=9, materialeffects=1, cinemas=1 |
| `Math:Less` | 512 | CryAction | prefabs=406, level entities=97, libs/flownodes=8, cinemas=1 |
| `Math:SetNumber` | 393 | CryAction | prefabs=368, level entities=12, materialeffects=9, libs/flownodes=4 |
| `Entity:MaterialParam` | 380 | CryAction | prefabs=368, level entities=10, libs/flownodes=2 |
| `entity:SimpleLight` | 361 | CryAction | prefabs=330, level entities=30, libs/flownodes=1 |
| `Time:Delay` | 332 | CryAction | prefabs=208, level entities=61, materialeffects=52, libs/flownodes=9, cinemas=2 |
| `Time:RandomDelay` | 296 | CryAction | prefabs=246, level entities=49, libs/flownodes=1 |
| `entity:ParticleEffect` | 289 | CryAction | prefabs=266, level entities=16, libs/flownodes=7 |
| `Vec3:FromVec3` | 246 | CryAction | prefabs=160, level entities=80, cinemas=6 |
| `Entity:GetPos` | 245 | CryAction | prefabs=162, level entities=82, libs/flownodes=1 |
| `Animations:StopAnimation` | 229 | CryAction | prefabs=174, level entities=55 |
| `entity:AreaTrigger` | 220 | CryAction | cinemas=219, level entities=1 |
| `Movement:RotateEntityTo` | 209 | CryAction | prefabs=168, level entities=40, libs/flownodes=1 |
| `Time:TimeOfDay` | 200 | CryAction | prefabs=162, level entities=33, libs/flownodes=5 |
| `Logic:All` | 194 | CryAction | prefabs=162, level entities=28, libs/flownodes=4 |
| `entity:AnimObject` | 172 | CryAction | prefabs=168, level entities=4 |
| `Interpol:Float` | 161 | CryAction | materialeffects=99, cinemas=39, prefabs=12, libs/flownodes=6, level entities=5 |
| `Movement:MoveEntityTo` | 157 | CryAction | prefabs=116, level entities=41 |
| `X2UI:ScreenFader` | 153 | x2game | cinemas=153 |
| `Image:DepthOfField` | 129 | CryAction | cinemas=129 |
| `Math:Sub` | 123 | CryAction | prefabs=80, level entities=40, cinemas=3 |
| `Entity:BroadcastEvent` | 122 | CryAction | level entities=122 |
| `Logic:NOT` | 120 | CryAction | prefabs=80, level entities=40 |
| `Logic:OR` | 120 | CryAction | prefabs=80, level entities=40 |
| `Time:Timer` | 120 | CryAction | prefabs=80, level entities=40 |
| `Math:FromBoolean` | 52 | CryAction | level entities=45, prefabs=6, materialeffects=1 |
| `Mission:GameToken` | 46 | CryAction | level entities=46 |
| `Image:ColorCorrection` | 39 | CryAction | materialeffects=25, cinemas=14 |
| `MaterialFX:HUDEndFX` | 37 | CryAction | materialeffects=37 |
| `MaterialFX:HUDStartFX` | 37 | CryAction | materialeffects=37 |
| `Entity:EntityPos` | 30 | CryAction | prefabs=14, level entities=7, libs/flownodes=4, materialeffects=3, cinemas=2 |
| `Math:ToBoolean` | 28 | CryAction | prefabs=26, materialeffects=1, cinemas=1 |
| `Image:FilterBlur` | 27 | CryAction | cinemas=14, materialeffects=13 |
| `Game:LocalPlayer` | 26 | CryAction (Game) | level entities=11, prefabs=8, materialeffects=5, libs/flownodes=2 |
| `Movement:RotateEntity` | 26 | CryAction | prefabs=26 |
| `Image:FilterRadialBlur` | 25 | CryAction | cinemas=18, materialeffects=7 |
| `CrysisFX:ScreenFader` | 21 | x2game | materialeffects=21 |
| `Camera:ViewShake` | 20 | CryAction | materialeffects=10, level entities=7, prefabs=2, libs/flownodes=1 |
| `Input:Key` | 20 | CryAction | materialeffects=17, libs/flownodes=2, cinemas=1 |
| `Logic:OnChange` | 20 | CryAction | prefabs=20 |
| `Vec3:AddVec3` | 14 | CryAction | level entities=8, prefabs=4, libs/flownodes=2 |
| `Vec3:SubVec3` | 14 | CryAction | prefabs=8, libs/flownodes=3, level entities=3 |
| `_comment` | 13 | CryAction | libs/flownodes=7, level entities=5, materialeffects=1 |
| `Sound:PlaySoundEvent` | 11 | CryAction | level entities=7, prefabs=2, libs/flownodes=1, materialeffects=1 |
| `Game:IsPlayerInRange` | 10 | CryAction (Game) | level entities=7, prefabs=2, libs/flownodes=1 |
| `Image:VolumetricScattering` | 10 | CryAction | libs/flownodes=4, level entities=4, cinemas=2 |
| `Entity:MaterialLayer` | 8 | CryAction | prefabs=8 |
| `Entity:TimeOfDayArea` | 8 | CryAction | libs/flownodes=4, level entities=4 |
| `Math:Mul` | 8 | CryAction | prefabs=4, cinemas=4 |
| `Physics:ActionImpulse` | 8 | CryAction | libs/flownodes=4, prefabs=2, level entities=2 |
| `Time:Time` | 8 | CryAction | prefabs=4, libs/flownodes=3, materialeffects=1 |
| `entity:Light` | 8 | CryAction | libs/flownodes=4, level entities=4 |
| `Image:ChromaShift` | 7 | CryAction | materialeffects=6, cinemas=1 |
| `Logic:RandomTrigger` | 7 | CryAction | prefabs=6, level entities=1 |
| `Condition` | 6 | CryAction | prefabs=4, libs/flownodes=1, level entities=1 |
| `Image:FilterSharpen` | 6 | CryAction | materialeffects=6 |
| `Movement:RotateTo` | 6 | CryAction | prefabs=6 |
| `Vec3:ClampVec3` | 6 | CryAction | libs/flownodes=2, prefabs=2, level entities=2 |
| `Vec3:MulVec3` | 6 | CryAction | prefabs=4, libs/flownodes=2 |
| `X2:PlayDoodadParticleEffect` | 6 | CryAction (X2) | prefabs=6 |
| `entity:ProximityTrigger` | 6 | CryAction | cinemas=6 |
| `Game:FindDoodadPos` | 5 | CryAction (Game) | prefabs=4, libs/flownodes=1 |
| `Math:DirToAngles` | 5 | CryAction | prefabs=4, libs/flownodes=1 |
| `Math:Random` | 5 | CryAction | level entities=3, prefabs=2 |
| `System:ConsoleVariable` | 5 | CryAction | libs/flownodes=2, level entities=2, materialeffects=1 |
| `Vec3:NormalizeVec3` | 5 | CryAction | prefabs=2, level entities=2, libs/flownodes=1 |
| `Vec3:ScaleVec3` | 5 | CryAction | prefabs=2, level entities=2, libs/flownodes=1 |
| `entity:Rain` | 5 | CryAction | prefabs=4, libs/flownodes=1 |
| `Environment:Rain` | 4 | CryAction | libs/flownodes=2, level entities=2 |
| `Image:AlienInterference` | 4 | CryAction | materialeffects=4 |
| `Image:EffectWaterFlow` | 4 | CryAction | cinemas=4 |
| `Math:Add` | 4 | CryAction | prefabs=2, cinemas=2 |
| `Movement:ScaleEntityTo` | 4 | CryAction | prefabs=4 |
| `Camera:CameraShake` | 3 | CryAction | materialeffects=3 |
| `CrysisFX:PostFXWaterDroplets` | 3 | x2game | materialeffects=3 |
| `HUD:DisplayTimedDebugMessage` | 3 | CryAction | libs/flownodes=2, cinemas=1 |
| `Logic:RandomSelect` | 3 | CryAction | materialeffects=3 |
| `Math:Abs` | 3 | CryAction | cinemas=3 |
| `Math:Equal` | 3 | CryAction | prefabs=2, materialeffects=1 |
| `Vec3:MagnitudeVec3` | 3 | CryAction | prefabs=2, level entities=1 |
| `CrysisFX:PostFXGlobal` | 2 | x2game | materialeffects=2 |
| `Engine:MaterialParam` | 2 | CryAction | prefabs=2 |
| `Entity:SetProperty` | 2 | CryAction | libs/flownodes=1, level entities=1 |
| `Image:EffectBloodSplats` | 2 | CryAction | materialeffects=2 |
| `Image:EffectCondensation` | 2 | CryAction | materialeffects=2 |
| `Image:RainDrops` | 2 | CryAction | prefabs=2 |
| `Logic:Condition` | 2 | CryAction | prefabs=2 |
| `Math:Sqrt` | 2 | CryAction | cinemas=2 |
| `Vec3:SetVec3` | 2 | CryAction | prefabs=2 |
| `Vec3:ToVec3` | 2 | CryAction | prefabs=2 |
| `X2:SkillController` | 2 | CryAction (X2) | libs/flownodes=2 |
| `X2:Weather` | 2 | CryAction (X2) | libs/flownodes=2 |
| `entity:AmbientVolume` | 2 | CryAction | level entities=2 |
| `entity:RelicEffect` | 2 | CryAction | libs/flownodes=1, level entities=1 |
| `Cinema:PlaySequence` | 1 | CryAction | libs/flownodes=1 |
| `Game:ActorCheckHealth` | 1 | CryAction | materialeffects=1 |
| `HUD:DisplayInstructionMessage` | 1 | CryAction | materialeffects=1 |
| `Image:EffectFrost` | 1 | CryAction | materialeffects=1 |
| `Image:FilterGrain` | 1 | CryAction | cinemas=1 |
| `Interpol:Int` | 1 | CryAction | cinemas=1 |
| `Logic:Indexer` | 1 | CryAction | cinemas=1 |
| `Logic:Multiplexer` | 1 | CryAction | cinemas=1 |
| `Physics:Dynamics` | 1 | CryAction | libs/flownodes=1 |
| `entity:RandomSoundVolume` | 1 | CryAction | level entities=1 |

## Critical classes: config / ProcessEvent / fields

Addresses are CryAction (`0x304F0000` base) or x2game (`0x38FF0000`). "config" is the
`GetConfiguration(SNodeConfig&)` function, identified by its unique description/port strings;
"event" is the `ProcessEvent` entry where this pass resolved it. `fields` are the node-object
offsets the implementation reads/writes.

| class | config | event | fields / notes |
|---|---|---|---|
| `Cinema:PlaySequence` | `0x305ce390` | `0x305ce210` | `+0x0c` `IMovieSequence*`, `+0x28` `m_bPlaying`; ports `seq_Sequence_File`, `Trigger`, `Stop`, `BreakOnStop`, `BlendPosSpeed`, `BlendRotSpeed`, `PerformBlendOut`; description "Plays a Trackview Sequence" (`0x30787bb...` port tables, class name string `0x3073cb5c`) |
| `Animations:PlayAnimation` | `0x305ccf40` | class vtable (`0x30787xxx`) | `anim_Animation` (string), `BlendInTime`, loop, ForceUpdate, PauseAnimGraph, ControlMovement, Start/Stop; the workhorse of animated props (996 uses) |
| `Animations:StopAnimation` | in the same cluster | class vtable | 229 uses |
| `Image:DepthOfField` | `0x305684d0` | cluster | description "Depth of field effect" (`0x3072c0c8`); 129 uses in cinemas |
| `X2UI:ScreenFader` | `0x39416800` | x2game class vtable | description "Controls screen Fade In/Out"; `clr_Color`, `FadeIn`, `FadeOut`, `FadeOutTime`, `FadeInTime`, `FadedOut`, `Complete`; 153 uses |
| `CrysisFX:ScreenFader` | `0x3914cd30` | vtable family around `0x3914c160` (not decoded) | older fader; `FadeGroup` 0..3, `UseCurColor`, `tex_TextureName`; 21 uses |
| `entity:AreaTrigger` | entity-class proxy | entity class | 220 uses; the trigger that starts/ends cinema DoF/fade in `_cinema_only_objs` |
| `entity:AnimObject` | entity-class proxy | entity class | 172 uses; props with `object_Model` `.chr`, `Animation`, physics |
| `entity:ParticleEffect` | entity-class proxy | entity class | 289 uses |
| `entity:SimpleLight` | entity-class proxy | entity class | 361 uses (light switching) |
| `entity:Light` | entity-class proxy | entity class | 8 uses |
| `entity:AmbientVolume` | entity-class proxy | entity class | 2 |
| `entity:ProximityTrigger` | entity-class proxy | entity class | 6 (cinema doors/props) |
| `entity:Rain` | entity-class proxy | entity class | 5 |
| `entity:RandomSoundVolume` | entity-class proxy | entity class | 1 |
| `entity:RelicEffect` | entity-class proxy | entity class | 2 |
| `X2:PlayDoodadParticleEffect` | CryAction class | `0x3055bfa0` | doodad entity + particle name; 6 uses in `interaction_c.xml` |
| `X2:SkillController` | passive | - | receives port values from `0x390959f0`, end event 0x101 from `0x39095cd0` |
| `X2:Weather` | CryAction class | cluster | `game/libs/flownodes/weather/*.xml`, 2 uses |
| `X2:OceanWave` | CryAction class | cluster | 0 shipped uses (editor) |
| `Game:LocalPlayer` | `0x30578e30` | `0x30578e30` cluster | 25 uses; description "Outputs the local players entity id - NOT USABLE FOR MULTIPLAYER" (`0x3072f9c0`) |
| `Game:IsPlayerInRange` | `0x30578f40` | cluster | 10 uses; outputs True/False |
| `Game:FindDoodadPos` | `0x30579900` | cluster | 5 uses; inputs `doodad_type`, `range`; outputs `Pos`, `Found` |
| `Time:TimeOfDayTrigger` | `0x305c0840` | cluster | 1,498 uses; `Active`, `Time`; light lamps |
| `Time:TimeOfDay` | `0x305c04b0` | cluster | 200 uses; set/get time and speed |
| `Time:Time` | `0x305c06c0` | cluster | 7 uses; elapsed time |
| `Time:Delay` | in `0x305c0xxx` | cluster | 332 uses; `delay` input |
| `Time:RandomDelay` | in `0x305c0xxx` | cluster | 296 uses |
| `Time:Timer` | in `0x305c0xxx` | cluster | 120 uses |
| `Logic:Any` | `0x3057eb40` | cluster | 947 uses; description "Port joiner - triggers it's output when any input is triggered" |
| `Logic:All` | `0x3057ef70` | cluster | 194 uses |
| `Logic:NOT`/`OR` | cluster | cluster | 120 each |
| `Logic:OnChange` | cluster | cluster | 20 |
| `Math:Less` | `0x30587350` | cluster | 512 uses; `A < B` |
| `Math:Equal` | `0x305871d0` | cluster | 3 uses |
| `Math:SetNumber` | cluster | cluster | 393 uses |
| `Math:Sub` | cluster | cluster | 123 |
| `Math:FromBoolean` | cluster | cluster | 52 |
| `Math:ToBoolean` | `0x30587e70` | cluster | 28 |
| `Math:Mul`/`Add`/`Abs`/`Sqrt`/`Random`/`DirToAngles` | cluster | cluster | 8/4/3/2/5/5 |
| `Vec3:FromVec3` | cluster | cluster | 246 |
| `Vec3:AddVec3`/`SubVec3`/`ClampVec3`/`MulVec3`/`NormalizeVec3`/`ScaleVec3`/`MagnitudeVec3`/`SetVec3`/`ToVec3` | cluster; `ScaleVec3` config `0x305890f0` | cluster | 14/14/6/6/5/5/3/2/2 |
| `Movement:MoveEntityTo` | `0x305c77e0` | cluster | 157 uses; description "Move an entity to a destination position at a defined speed" |
| `Movement:MoveEntity` | `0x305c4480` | cluster | 6 (move between two positions at a defined speed) |
| `Movement:RotateEntityTo` | in `0x305c4xxx` | cluster | 209 |
| `Movement:RotateEntity` | `0x305c5d50` | cluster | 26 |
| `Movement:RotateTo` | `0x305c67d0` | cluster | 6 ("Rotate at a defined speed") |
| `Movement:ScaleEntityTo` | cluster | cluster | 4 |
| `Physics:ActionImpulse` | `0x305b6580` | cluster | 4; "Applies an impulse on an entity" |
| `Physics:Dynamics` | cluster | cluster | 1 |
| `Camera:ViewShake` | cluster | cluster | 20 uses (cinema camera shake) |
| `Camera:CameraShake` | scripted `.node` | Lua `Scripts/FlowNodes/CameraShake.lua` | 3 uses |
| `Sound:PlaySoundEvent` | `0x305cf5d0` | cluster | 11 |
| `Entity:MaterialParam` | cluster | cluster | 380 (night lamp glow) |
| `Entity:MaterialLayer` | cluster | cluster | 8 |
| `Entity:GetPos` | cluster | cluster | 245 |
| `Entity:EntityPos` | cluster | cluster | 27 |
| `Entity:BroadcastEvent` | `0x305742a0` | cluster | 122 |
| `Entity:SetProperty` | `0x3056dac0` | cluster | 2 |
| `Entity:TimeOfDayArea` | `0x30573ce0` | cluster | 4 |
| `Image:ColorCorrection`/`FilterBlur`/`FilterRadialBlur`/`VolumetricScattering`/`ChromaShift`/`FilterSharpen`/`FilterGrain`/`EffectFrost`/`EffectWaterFlow`/`EffectBloodSplats`/`EffectCondensation`/`RainDrops`/`AlienInterference` | cluster | cluster | 39/27/25/10/7/6/1/1/4/2/2/2/4 |
| `Interpol:Float`/`Int` | cluster | cluster | 160/1 |
| `HUD:DisplayTimedDebugMessage` | `0x3057a9c0` | cluster | 3 |
| `HUD:DisplayInstructionMessage` | cluster | cluster | 1 |
| `System:ConsoleVariable` | `0x305637d0` | cluster | 5 |
| `Mission:GameToken` | cluster | cluster | 46 |
| `Environment:Rain` | cluster | cluster | 4 |
| `Engine:MaterialParam` | cluster | cluster | 2 |
| `MaterialFX:HUDStartFX`/`HUDEndFX` | `0x30683c40` (+32) | cluster | 35/36 |
| `Input:Key` | cluster | cluster | 20 |
| `Start` | special | `0x30559170("Start")` | 617; the graph entry node |
| `Condition` | cluster | cluster | 6 |
| `_comment` | skipped by the loader | - | 9 |

## Classes registered by x2game but not used by 1.2 content

`Game:ActorSensor` (config `0x3914ae50`, node size 0x2c, event `0x3914a310`),
`Camera:OverrideFOV` (config `0x3914b3f0`, event `0x3914ad30`, serialise `0x3914ab90`),
`Game:DifficultyLevel` (config `0x3914b1b0`), `Game:PlayerStaging` (config `0x3914e520`, vtable in the `0x399af2xx` cluster),
`Game:PlayerLink` (config `0x3914e6f0`, same cluster), `Crysis:HitInfo` (`0x3914d610`),
`Crysis:ExplosionInfo` (`0x3914d950`), `Crysis:FreezeEntity` (`0x3914d490`),
`CrysisFX:PostFX*` (`0x3914f100...0x3914f7c0`), `Tornado`/`TornadoWander`
(`0x39084e10`/`0x39085030`), `MP:GameType` (`0x39150560`). They are port targets only when
10.x content or the editor needs them; 1.2 quests/cinematics do not. `Camera:OverrideFOV` is
still worth porting first because it is the cheapest complete "node with an effect on the client
camera" example (the cutscene FOV override).

## Entity-class proxy nodes

The 14 `entity:*` classes are not hand-written node classes in the CryAction table; they are
the engine's entity-class proxy (`CFlowEntityClass`, registration `0x30574d40`, 210 bytes). The
node exposes the entity's own properties (model, animation, particle, light, trigger, sound) as
ports. `CFlowEntityClass::GetConfiguration` walks the entity class's property list and builds
ports dynamically, which is why no fixed config address is listed. For the port, treat them as
thin adapters over the existing entity/doodad code in `aa-world`/`aa-client`, not as separate
behaviour to reverse.
