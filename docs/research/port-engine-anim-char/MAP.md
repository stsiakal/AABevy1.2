# CryAnimation character map (E4): pose/physics, skin/attachments, facial, loading

> **Restored 2026-10-27 from `archive/fork-main-2026-10-27`, UNVERIFIED.** The client was restarted from NickMesser's `main`; nothing described here is wired into the game. Addresses, call chains and offsets must be re-checked against the decompile ([R]/[I]/[?]) before use. A 2026-10-27 audit found 151 of the 269 EXACT/TOLERANCE labels in `proof-levels.*` weak or wrong (report: `D:\aaemu-client\handoff\2026-10-27\audit\report-proofs.md`).

Status: research map for roadmap v2 phase 0, map **E4**. Branch `map/engine-anim-char`, from client
`main` `9d56d970`. Read-only research: every statement below comes from the shipped 1.2 binaries and
their decompiles, the shipped game data, or the server database.

Scope: the CryAnimation areas that decide how characters, NPCs and mounts look and move **beyond the
riding path already mapped in E2**:

* pose and physics sync: character physical proxy build, pose <-> physics sync, ragdoll handover,
  hit reactions, look/aim pose;
* skin and attachments: subskin/equipment attachments, attachment classes and types, bone/face
  attachments, CPU skinning, morph targets, cloth/hair (as far as they exist);
* facial animation: `CFacialInstance`/`CFacialModel`/`CFacialAnimSequence`, CAL/FXL/FSQ assets,
  expressions, lip sync and eyes;
* model/animset loading only as far as runtime behaviour depends on it (CHR/CDF build, compiled
  physical bones/proxies, morph targets, animation-set lookup).

Not redone here:

| map | owns |
|---|---|
| [`../port-engine-anim/MAP.md`](../port-engine-anim/MAP.md) (**E2**) | graph command -> FIFO -> clip/LMG sample -> posed skeleton, transitions/layers, root motion, rider seat attachment, generic IK, `SkeletonPostProcess` top level |
| [`../port-mount-anim/MAP.md`](../port-mount-anim/MAP.md) | x2game animation graph, guards, LMG capabilities, rider feed `390dcc80` |
| [`../port-engine-char/MAP.md`](../port-engine-char/MAP.md) | CryEntitySystem transform graph, CryAction `CAnimatedCharacter`, rider/mount entity parenting |
| [`../port-movement/MAP.md`](../port-movement/MAP.md) | x2game movement and camera |

E2 is the direct predecessor: its `CSkeletonPose::SkeletonPostProcess 3154b4b0` is the caller of the
physics-sync dispatcher mapped here (§3), and its attachment manager map (§7) is extended here (§4).

---

## 0. Sources and conventions

All addresses are virtual addresses at the **preferred image base**:

| module | base | decompile inputs |
|---|---|---|
| `x2game.dll` | `0x38ff0000` | `C:/AAEMU_DISK/aa-re/client12/out-x2game/x2game.dll.c`, `.funcs.csv`, `.calls.tsv`, `.strings.txt`, `.strings-xrefs.tsv` |
| `cryanimation.dll` | `0x31500000` | `D:/aa-re/client12/engine/out/cryanimation.dll.c`, `.funcs.csv`, `.strings.txt` |
| `cryaction.dll` | `0x304f0000` | `D:/aa-re/client12/engine/out/cryaction.dll.c`, `.funcs.csv`, `.strings.txt` |
| `cryentitysystem.dll` | `0x30400000` | `D:/aa-re/client12/engine/out/cryentitysystem.dll.c` |
| `crysystem.dll` | `0x36500000` | `D:/aa-re/client12/engine3/out/crysystem.dll.c` (checked; no E4 code path enters it directly, only cvar/file APIs already owned by E1/E3) |
| `cry3dengine.dll` | `0x30000000` | `D:/aa-re/client12/engine2/out/cry3dengine.dll.c` (only to prove cloth is not there) |

Sizes are `funcs.csv` `size` bytes unless marked *(observed)*. At runtime use `module+rva`
(rva = VA - preferred base) with `aaemu-clientdriver call/read/alloc`. `__thiscall` is MSVC x86
`__fastcall` with an unused EDX.

**Licensing.** Derived only from the shipped binaries, their decompiles, `.data`, the shipped Lua
bytecode and game data. The published CRYENGINE source was never used, quoted or consulted. Names in
quotes/`CClass::Method` form are the binaries' own profiler/log/assert strings; *(inferred)* means the
class name or field role is inferred from layout/behaviour only.

Tags: **[R]** read directly in the decompile/bytes; **[I]** inferred; **[?]** unresolved.
Phase tags: **P1** riding/on-foot feel, **P2** combat (hit/aim), **P5** character creation/cutscenes.
The P1 links are marked in every table and are mapped in full first.

`cryanimation.dll` has exactly one export, `CreateCharManager`; every other reachable entry is an
`ICharacterManager` / `ICharacterInstance` / `ISkeletonAnim` / `ISkeletonPose` virtual at
`gEnv+0x7c` (E2 §1). The x2game addresses below are therefore the only static handovers; the
cryanimation functions themselves mostly have no static caller.

---

## 1. The one-screen call chain

### 1.1 Physics: proxy build and per-frame sync (P1)

```
character load (§6)
  CharacterManager::CreateCHRInstance            315d3c90   (x2game -> ICharacterManager)
    CryCHRLoader::LoadNewCHR                     315f1510
      LoadFromContents                           315f1260/315f1283
        CryCHRLoader build core                  315ed8a0   (14,072 B, DECOMPILE FAILED)
          compiled bones / physical proxies / morph targets

first physicalize (entity or CryAction requests it)
  CSkeletonPose::CreateCharacterPhysics           315362a0   P1
    IPhysicalWorld::CreatePhysicalEntity(6, ...)          (crysystem/physics)
    pose->vt[0x84] = CSkeletonPose::BuildPhysicalEntity 3153b5b0  P1
      compiled per-bone proxies -> part list pose+0x19c (stride 0x40), count pose+0x190
      BuildPickingProxy                           31535d50   P1
      CModelSkeleton::BuildAuxPhysicsData         3161da30   P1 (rope bones)

every frame (inside E2's SkeletonPostProcess 3154b4b0)
  CSkeletonPose::SynchronizeWithPhysics            31543a90   P1 dispatcher
    CSkeletonAnimTask::Prepare -> SynchronizeWithPhysicsPrepare 31535430  P1
    Physics_SynchronizeToEntity                    315391e0   P1 (physics -> pose, dead/ragdoll)
    Physics_SynchronizeToAux                       315347d0   P1 (aux rope/ragdoll parts)
    SynchronizeWithPhysicsPost                     31538880   P1 (pose -> attachment bones)
  pose callbacks                                   3154dc70

ragdoll handover (x2game, no cryanimation ragdoll state machine)
  ActorUnitModel/Actor ragdollize                 391199a0   P2
    IEntity::Physicalize(vt+0x104), PE type 6
  actor state string "alive"/"ragdoll"/"unragdoll" 39141e70   P2
  actor state read                                 391430b0   P2
  get-up / unragdoll helper                        39119400   P2
  can-ragdoll check                                39111d20   P2
```

### 1.2 Skin, equipment and attachments (P1)

```
equipment/customize/model change (x2game)
  CharacterManager::CreateCDFSkinInstance           315d6080
  CharacterManager::CreateCGAInstance               315cfd00
    CCharInstance (slave) -> CAttachmentSkin::VerifySkeleton 31508830  P1
    CAttachmentManager::AttachObjectToBone          315078e0 / 31507b00  P1

every frame
  CSkeletonAnimTask::Begin -> CAttachmentManager::UpdateAnimBones 3150df50   (E2)
  SkeletonPostProcess 3154b4b0
    CSkinInstance::UpdateAttachedObjects            315253f0   P1
      CAttachmentManager::UpdateLocationAttachments  3150f340   P1 (E2 §7)
        kind 1 (character/subskin): 3-bone matrix blend + CAttachmentSkin::SkinningAttSW 315095a0
        kind 2 (entity): rider/child entity transform (E2 §7.3)
        kind 0/4/CA_FACE/CA_CAMERA: generic bone/camera path
    CSkinningTransformationsComputationTask         315251e0/31525f40  P1 (skin matrix buffer)
```

### 1.3 Facial (P5, minor P2)

```
x2game anim event "facial_expression" (SPHERE/quest/bubble chat facial column)
  FUN_39138a40                                     (x2game)  P5
    -> character facial interface -> CFacialInstance::PlaySequence 31598330
         CFacialInstance::LoadSequence            315985d0   (name -> .fsq path via character CAL)
           CFacialAnimation::LoadSequence         3157bb00   (name + ".fsq")
             CCalManager / CCalParser             315c0ba0 / 315c2c50 / 315c48f0 / 315c5240
         layer slot 0..5, active = lowest non-empty layer
per frame
  CFacialInstance::Update                          31597f80
    CFacialAnimationContext::Update                3157a800   P5
      sample FSQ channels -> FXL effectors -> morph weights + bone rotations
      eyes: CFacialModel eye/look bones (§5.5)
    CPhonemesLibrary                               3159dbc0   (lip sync)
    SkeletonAnimationEntry from the FSQ           (body animation, consumer unresolved [?])
```

### 1.4 Loading runtime dependency (P1)

```
CharacterManager::CreateInstance                  315d6820
  LoadCharacterDefinition                         315d5010  (CDF: <Model File=... Material=...>)
  FetchModel                                      315cfbd0  (model cache)
  CreateCHRInstance/CGAInstance/CDFSkinInstance   (above)
    CryCHRLoader::LoadNewCHR                      315f1510
      ".cal" of the character (global_player.cal, global_horse.cal, per-monster .cal)
      LoadFromContents -> build core 315ed8a0 (failed decompile)
      LoadFacialLibrary / LoadFaceLib              315ec600 / 315ec9f0
      LoadModelAnimations / LoadExtraResources     315f0fb0 / 315f1110
CharacterManager::Update                           315d3360
  UpdateStreaming / UpdateGarbageCollection / UpdateRendererFrame
```

The x2game handovers are input/selection only. All clip timing, posing, proxy assembly, attachment
transforms and facial evaluation are cryanimation.

---

## 2. Object model and inventory

### 2.1 What E2 already covers

E2 maps the playback object model (`CCharInstance`, `CSkeletonAnim`, per-layer FIFO, command buffer,
`CSkeletonPose` top level) and its 74-function key set (71.4 KB). This document adds fields to the same
objects and does not repeat E2's offsets. Read E2 §1.2 before this section.

### 2.2 Address bands (measured from `cryanimation.dll.funcs.csv`)

| band | f | KB | contents | E4 treatment |
|---|---:|---:|---|---|
| `0x31501000-0x3150c000` | 91 | 44.1 | early `CSkinInstance`, `CAttachment` classes, morph/attachment edge cases | **P1** key set mapped in full; GPU morph copy skipped |
| `0x3150c000-0x31528000` | 483 | 107.0 | `CCharInstance`, attachment manager, skinning task, char glue | E2 phase-1 band; E4 maps the attachment/skin subset |
| `0x31528000-0x3154d400` | 246 | 137.8 | `CSkeletonPose`: physics proxies, sync, bbox, post-process | **P1/P2** key set mapped in full |
| `0x3154d400-0x31580000` | 587 | 197.2 | E2 playback + facial lower part | E2 for playback; **facial starts at `0x3157a4f0`** |
| `0x3157a4f0-0x3159e000` | 790 | 137.2 | facial effectors/instances/model/sequences/look/eyes/lip sync | **P5** mapped; P2 look/eye subset |
| `0x315a4000-0x315c6920` | 202 | 122.7 | CAL parser/manager, facial effector libraries, face makers init | **P5**, plus P1 loader dependency |
| `0x315c6920-0x315fd910` | 806 | 213.0 | `CharacterManager`, CHR/CGA/CDF streaming, `CryCHRLoader` | **P1** handover/lookup mapped; stream tasks replaced |
| `0x315fd910-0x31626526` | 556 | 153.8 | `CCharacterModel`, `CModelMesh/Skeleton`, `CAnimationSet/Manager` | **P1** runtime-relevant subset mapped |
| `0x31626526-0x3165cde0` | 977 | 202.6 | CAF/DBH/DBA animation assets, threads/contexts | E2 dependency; not E4 |
| `0x3165cde0-0x31673a00` | 2,474 | 67.7 | mesh merge, templates, utility tail | face maker (`0x316641f0-0x31665760`, 10 f / 4.1 KB) only |

**Facial band correction.** The survey (`../port-engine-survey/content.md`) puts facial at
`0x31580000-0x3159e000` + `0x315a4000-0x315c6920` and counts 863 f / 237.4 KB. The real facial block
starts at `0x3157a4f0` (`CFacialAnimationContext` ctor/vtable). The extra
`0x3157a4f0-0x31580000` slice is 129 f / 22.5 KB and sits inside E2's coarse
`0x31558000-0x31580000` "desc/LMG" phase-1 range, but E2 did not map it. Including it and the face
maker tail, the facial area is **1,002 f / 264.1 KB**.

E4 area total (excluding the animation-asset band owned by E2): **2,701 f / 812.8 KB**. Adding the
asset band that E2 consumes gives 3,678 f / 1,015.4 KB; the survey's 988.8 KB is the same set with
its facial boundary and face-maker rounding.

### 2.3 `CSkeletonPose` (physics-relevant fields) [R]

`CSkeletonPose` is at `char+0x254` (E2). E4 adds:

| offset | meaning | used by |
|---|---|---|
| `+0x004` | owning `CCharInstance*` | all physics/sync |
| `+0x008` | skeleton bone data (model `CModelSkeleton` bone array) | `BuildPhysicalEntity`, `UpdateBBox` |
| `+0x014` | physical-part pointer array (one entry per bone, `0x10` stride in the map) | `BuildPhysicalEntity`, `Physics_SynchronizeToEntity` |
| `+0x018` | pose matrix block consumed/generated by sync | `SynchronizeWithPhysicsPost` |
| `+0x044..+0x04c` | pose root position / scale cache | `CreateCharacterPhysics`, `SyncPost` |
| `+0x06c` | model/skeleton interface | E2, `Prepare` |
| `+0x148` | main `IPhysicalEntity*` | create/sync/destroy |
| `+0x14c` | entity passed to `CreateCharacterPhysics` | create |
| `+0x150` | aux physical entity array (rope/ragdoll parts) | `Physics_SynchronizeToAux` |
| `+0x190` | physical part count | build/sync |
| `+0x194` | per-character scale/param passed to `BuildPhysicalEntity` | build |
| `+0x19c` | part records, **stride 0x40** | build/sync/aux |
| `+0x92c`, `+0x93c`, `+0x944`, `+0x948` | physics params (mass/scale/inertia caches) | create/sync |
| `+0x980` | sync flags: bit0 = proxies built; bit2 = wants physical entity; bits 4/5 = sync state | create/sync |
| `+0x9b4` | owning `CSkeletonAnim*` | E2 |

The magic at `model+0x48` distinguishes skeletons: `0x11223344` = CHR (skinned character),
`0x55aa55aa` = non-CHR/legacy (world/stat object). `BuildPhysicalEntity` and both sync functions
branch on it.

### 2.4 `CAttachment` / `CAttachmentManager` (P1) [R]

The manager hangs at `char+0x30` (E2). Live attachment objects are the pointer vector at
`manager+0x08/+0x0c` (E2). E4 additionally verified the compiled per-bone attachment/bone data used by
`UpdateLocationAttachments` (`3150f340`): it is at **`model+0x658/+0x65c`, stride 0xf0**, indexed by
`model+0x50`. `manager+0x2c` is the skeleton, `manager+0x30` the owning character. Do not confuse the
two vectors: the manager vector is per instance, the model vector is per compiled model.

`CAttachment` (one record):

| offset | meaning |
|---|---|
| `+0x00..+0x1c` | local rotation quaternion / position (also used as bone-space transform) |
| `+0x1c` | **class**: 0 = `CA_BONE`, 1 = `CA_FACE`, 2 = `CA_SKIN`, 4 = `CA_CAMERA` (`31508c50`) |
| `+0x20` | flags |
| `+0x24` | `AName` (attachment name) |
| `+0x28` | `BoneName` pointer |
| `+0x2c` | `Binding`/material binding |
| `+0x30` | `Material` |
| `+0x34/0x38/0x3c` | `HingeIdx` / `HingeLimit` / `HingeDamping` |
| `+0x40` | `AlignBoneAttachment` mode |
| `+0x44` | attachment type: 3 = `entity`, 5 = `effect` |
| `+0x48` | `scale` |
| `+0xb8` | **object kind** (argument of `AttachObjectToBone`): 0 = named bone, 1 = character/subskin, 2 = entity, 4 = aligned |
| `+0xc4` | resolved bone id (`-1` when not found) |
| `+0xcc` | `m_HideInMainPass` |
| `+0xd4` | alignment type (1 = align to bone matrix, 2 = zero local) |

The three enums (class `+0x1c`, object kind `+0xb8`, attachment type `+0x44`) are distinct; do not
collapse them in the port. Which combination the equipment generator uses for armor parts is still
unresolved (§10), but the subskin path (kind 1 + `CA_SKIN`) is the one that blends three bones and is
required for visible equipment [I].

### 2.5 Facial objects (P5) [R]

`CFacialInstance` (attached to the master `CCharInstance`):

| offset | meaning |
|---|---|
| `+0x00/+0x04` | vtable/refcount |
| `+0x0c..+0x3c` | 6 layer slots, stride 8: sequence pointer, byte flag, byte loop flag |
| `+0x3c` | active layer index, `-1` = none |
| `+0x40` | play parameter passed to the ctor |
| `+0x44` | master `CCharInstance*` |
| `+0x48` | model/owner handle passed to `FUN_3159a540` [I] |
| `+0x4c` | `CFacialModel*` |
| `+0x50` | `CFacialAnimationContext*` (allocated `0x70`) |
| `+0x9c` | per-instance flags |
| `+0xa4` | refcount/vtable block |

`CFacialAnimationContext`:

| offset | meaning |
|---|---|
| `+0x010/+0x014` | active facial-effect vector, **stride 0x30** |
| `+0x044` | owning `CFacialInstance*` |
| `+0x048` | facial model pointer |
| `+0x050/+0x054` | active sequence vector, stride 0x30 |
| `+0x068` | "changed this frame" flag |
| `+0x06c` | bone-rotation-smoothing countdown |
| effect `+0x004` | source effector pointer |
| effect `+0x008` | effect type (0/1/2) |
| effect `+0x00c` | blend weight |
| effect `+0x014/+0x018` | current/target weight |
| effect `+0x020` | interpolation state |
| effect `+0x028` | flags (bit0 = sticky) |

`CFacialAnimSequence` (`31583180` ctor):

| offset | meaning |
|---|---|
| `+0x00c` | flags |
| `+0x014` | joystick library name (`Game/Animations/pc_joy.joy`) |
| `+0x018/+0x01c` | `SkeletonAnimationEntry` vector, stride 0x10: name, start, end |
| `+0x028/+0x02c` | facial channel vector |
| `+0x038/+0x03c` | `StartTime` / `EndTime` |
| `+0x040/+0x044` | sound-entry vector, stride 0x14: sound, sentence, start time |
| `+0x094/+0x0ac/+0x0c4` | camera path positions / orientations / FOV |
| `+0x09c` | camera path object |

`CFacialModel` (`3159b440` ctor; fields confirmed by `3159aa40`, `315b32d0`, `315b37c0`):

| offset | meaning |
|---|---|
| `+0x004/+0x008` | left/right eyeball attachment object (resolved by name) |
| `+0x01c` | facial effector library (`CFacialEffectorsLibrary`) |
| `+0x034` | effector count |
| `+0x040` | owning character/model |
| `+0x06c..+0x094` | per-frame look/eye state block, copied to `+0x098..+0x0e4` by `315b32d0` |
| `+0x0a0..+0x0b8` | look target/rotation state (floats, see §3.6) |
| `+0x098..+0x0e4` | current eye/look bone-id and blend block |
| `+0x0f0` | look-bone chain (`u16` indices) |
| `+0x0f4` | look-IK target chain (`u16` indices) |

`CModelSkeleton` ([R] from `3161ce60`):

* bone array at `model+0x0c`, count from the vector at `-4` masked `0x3fffffff`, **stride 0x120**;
* bone name at `bone+0x04`, child count `+0x18`, child index array `+0x10`, proxy pointer `+0x2c`,
  bone mass/`mass` property at `bone+0x08`;
* the bone-id map at `model+0x28..+0x96` resolves fixed names: `Bip01`, `Bip01 Pelvis`, `Spine`,
  `Spine1..3`, `Neck`, `Head`, `eye_left_bone`, `eye_right_bone`, `L/R UpperArm`, `L/R Thigh`,
  `L/R Calf`, `L/R Foot`, `L/R Heel`, `L/R Toe0`, `L/R Toe0Nub`, `item_side_l/r`, `item_back_l/c/r`,
  `item_staff_l`, `item_lute_r`, `item_bow_l`, `item_arm_l`, `item_hand_l/r`, `item_glider`, `camera`,
  `Bip01 Hit`, `driver`, `L/RThighTwist`, `bone_L/R Toe0`, `Bone_Swim`;
* all bone names beginning with `Bip01 Hit` are additionally collected into a vector at
  `model+0xac..+0xb0` (hit-bone chain; consumer unknown, §10).

### 2.6 What the shipped data says (P1/P5) [R]

From `data/pak12.tsv` and the pak:

* **989 `.cal`** and **894 `.fsq`** files; **9 `.fxl`** facial effector libraries, **4 `.joy`**
  joystick layouts.
* `global_player.cal` includes `facial_player.cal` (`$Include`) and lists LMG/CAF locomotion groups.
* `facial_player.cal` is a flat `name = facial/<name>.fsq` list (295 lines), e.g.
  `fist_em_bow = facial/fist_em_bow.fsq`.
* face CALs are `$facelib = animations/pc_expressions.fxl` (46-byte file;
  `$FaceLib` accepted case-insensitively).
* FSQ is XML `FacialSequence` with `Flags`, `StartTime`, `EndTime`, `Joysticks`, 8 top-level channel
  groups (`eye`, `eye_ball`, `eye_brow`, `head`, `mouth_aeiou`, `mouth_emotion`, `nose`, `tongue`),
  `Spline Keys="time:value:flags,..."`, one `SkeletonAnimationEntry`, and empty camera paths.
* FXL is `ExpressionLibrary` -> `AllEffectors` with `Effector Type=group|expression|morph|bone|attachment`
  and `SubCtrl` submixes. Bone effectors carry `Attachment`, `PosOffset`, `RotOffset`; attachment
  effectors are rejected by the engine ("Attachment Effectors not longer supported").
* character composition is CDF-per-part: a CDF is `<CharacterDefinition><Model File="...chr"
  Material="...mtl"/></CharacterDefinition>`; head, hair and every armor part are separate CHRs.
  Example: `nu_m_face00.cdf` -> `nu_m_face01.chr` + `nu_m_face00.mtl`; hair has per-color material
  variants (`*_hcNN.mtl`), not a cloth/hair simulation.

---

## 3. Pose and physics sync (P1/P2)

This is the area that decides whether an on-foot character, an NPC corpse or a mount stands on the
ground, collides, dies into a ragdoll, and whether the visible skin follows that physics. Movement's
capsule is E1/E3; CryAnimation owns the **articulated character proxy** and its sync.

### 3.1 Call chain

```
CryAction / x2game physicalize request
  -> CSkeletonPose::CreateCharacterPhysics                 315362a0   build entity + parts
       -> pose vt[0x84] = BuildPhysicalEntity              3153b5b0
            compiled physical bones/proxies -> part records
            BuildPickingProxy                              31535d50
       (loader time) CModelSkeleton::BuildAuxPhysicsData   3161da30   rope aux parts
per frame (from E2 SkeletonPostProcess 3154b4b0)
  -> CSkeletonPose::SynchronizeWithPhysics                 31543a90
       alive path: FUN_31543950                            31543950
         -> FUN_3153a150  (entity -> pose, type 6)
         -> Physics_SynchronizeToEntity                    315391e0
         -> Physics_SynchronizeToAux                       315347d0
         -> FUN_31542370 / FUN_315373e0
       dead path: Physics_SynchronizeToEntity              315391e0   (ragdoll -> pose)
  -> SynchronizeWithPhysicsPost                            31538880   pose -> attachment bones
  -> SSkeletonPose::UpdateBBox                             31549f80
```

### 3.2 Object and data model

`CSkeletonPose` fields are in §2.3. The physical data itself comes from the model:

* `model+0x180` = model container used everywhere (`*(int*)(char+0x180)`);
* `model+0x48` magic: `0x11223344` = CHR, `0x55aa55aa` = non-CHR skeleton;
* per-bone compiled proxy pointer at `bone+0x2c`, bone stride **0x120**;
* per-bone mass at `bone+0x08` (read as the `"mass"` property in the proxy builder);
* pose part records at `pose+0x19c`, stride **0x40**, count `pose+0x190`;
* pose bone matrices at `pose+0x18`, stride **0x1c** (7 floats: quat x/y/z/w + pos x/y/z).

**Proxy-to-bone assignment is by CRC32 of the lower-case bone name, not by index** [R]. In
`BuildPhysicalEntity` the engine lazily computes CRCs for these names and compares them with the model
bone names:

```
Bip01 Pelvis, Bip01 Spine, Bip01 Spine1..3, Bip01 Head,
Bip01 L/R Thigh, Bip01 L/R Calf, Bip01 L/R Forearm
```

This matters for the port: any bone-name change (importer or retarget) breaks the proxy mapping.

### 3.3 Proxy build

| function | addr | size | callers | callees | state / offsets | P1 |
|---|---:|---:|---|---|---|---|
| `CSkeletonPose::CreateCharacterPhysics` | `315362a0` | 1,660 | virtual (`ISkeletonPose`), entity physicalize path | physics world `CreatePhysicalEntity`, `3152ca70`, `31533900`, `31543e20/3db0`, pose vt[0x84] | stateful; reads/writes `pose+0x148`, `+0x14c`, `+0x150`, `+0x190`, `+0x194`, `+0x19c`, `+0x92c`, `+0x93c`, `+0x944`, `+0x948`, `+0x980`; scale from `param_6[0], [4], [8]` and `char+0x298`; scale snap tolerance 0.01 | yes |
| `CSkeletonPose::BuildPhysicalEntity` | `3153b5b0` | 8,036 | `CreateCharacterPhysics` via pose vt[0x84] | `315460a0`, `31544240`, `315d8eb0` (CRC), `31535d50`, `315081f0`, physics entity vt | stateful; bone array `pose+0x8`, stride 0x120; proxy `bone+0x2c`, mass `bone+0x8`; parts `pose+0x19c`/`+0x190`; `pose+0x148`, `+0x980` bit0; validates orthonormal/inertia/volume; `model+0x48` magic branch | yes |
| `CSkeletonPose::BuildPickingProxy` | `31535d50` | 511 | `BuildPhysicalEntity` | render mesh `GetGeometry(0x1001)`, entity `AddGeometry(vt+0x2c)` | stateful; adds picking geometry ids 0x1f4+i; flags `0xffbfffff`, `0x80000000`; threshold `DAT_31676938` = 1e-4 | yes |
| `CModelSkeleton::BuildAuxPhysicsData` | `3161da30` | 2,370 | loader/build-model (no static caller) | `315d8f40`; bone arrays | stateful; scans bones named `rope*`, builds aux descriptors; no cloth/hair | yes (ropes) |
| `SSkeletonPose::UpdateBBox` | `31549f80` | 5,413 | pose/render frame (virtual) | `315bdc20`, `315bcec0`, `315be0d0` | stateful; model `+0x14` bone array; bbox from bones if no bbox chunk; `param_1` = dt (0 = force no interpolation) | yes |

`CreateCharacterPhysics` payload observed [R]:

* physical entity type **6** (`PE_ARTICULATED`); the next argument is the constant `5.0` (`_DAT_31674f50`, role unresolved [?]);
* flags set to `0xffbfffff` (all bits except `0x00400000`), `0x80000000` for some limits;
* `pose+0x980` bit 2 gates creation; on destroy, bits `0xbe` are cleared and `pose+0x93c = -1.0`;
* `pose+0x148` entity, `pose+0x150` aux entity array, `pose+0x190` part count;
* the scale is the length of the matrix's first basis column divided by `char+0x298`, snapped to 1.0
  when `|scale-1| < 0.01`.

`BuildPhysicalEntity` validation (strings, all `[R]`):

* `chr has non-orthonormal physical proxy. file(%s). bone(%s)`
* `chr has invalid inertia(I:%f, %f, %f) physical proxy. file(%s). bone(%s)`
* `chr has too small(V:%f) physical proxy. file(%s). bone(%s)`
* `chr Physical Bone Error : too many root bones(%d). file(%s)`
* `CSkeletonPose::BuildPhysicalEntity() failed to add geometry. jointNo(%d), jointName(%s)`

The heavier proxy warnings (`physical proxy is relatively heavy`, `too many faces`, `created physical
trimesh`, `too small physical proxy ... at joint`) are emitted by the mesh-build side
(`FUN_31608010`), not by `BuildPhysicalEntity`; they matter for the loader (§6), not per frame.

The numeric guards are `1e-6` (volume/epsilon), `FLT_EPSILON = 1.1920929e-7`, `±1e10` (limits), and
`3.0 / 0.3 / 36.0 / 40.0` for the proxy inertia and box checks (addresses
`0x31676c10..0x31676c20`). Treat these as compatibility constants, not tunables.

### 3.4 Per-frame sync

| function | addr | size | callers | callees | state / offsets | P1 |
|---|---:|---:|---|---|---|---|
| `CSkeletonPose::SynchronizeWithPhysics` | `31543a90` | 537 | E2 `SkeletonPostProcess` | `31543950`, pose vt[0xbc]/vt[0x120]/vt[0xd8], char flags | stateful; only for `model+0x48 == 0x11223344`; sets/clears `char+0x1ec` bit 2; `pose+0x148`, `+0x190`; profiler `CSkeletonPose::SynchronizeWithPhysics` | yes |
| `SynchronizeWithPhysics` helper | `31543950` | 33 | `31543a90` | `3153a150`, `315373e0`, `315391e0`, `315347d0`, `31542370` | stateful; clears `pose+0x980` bit 4 and `pose+0x44/+0x4c`; toggles bit 5; global `DAT_316c9290` enable, `DAT_316c9388` aux enable | yes |
| `SynchronizeWithPhysicsPrepare` | `31535430` | 220 | `CSkeletonAnimTask::Prepare 3154e8a0` (E2) | `31547780` | stateful; copies the task's pose/locator input; profiler label | yes |
| `SynchronizeWithPhysicsPost` | `31538880` | 2,394 | E2 `SkeletonPostProcess` | `3151c9e0`, `31544110`, `31508570` | stateful; reads params `0xf`/`0x80000000`; entity type 6 & flag 2; writes attachment bone transforms via `char+0x30`; `pose+0x150` aux triggers `3151c9e0` | yes |
| `Physics_SynchronizeToEntity` | `315391e0` | 3,920 | `31543950`, `3153b380`, `3153b430` | `31508570`, `3151e700`, `315460a0`, `31544110`, `31543fd0`, physics entity vt | stateful; entity part transforms -> pose/attachments; bone stride 0x1c; part stride 0x40; `pose+0x14/+0x148/+0x190/+0x19c`; char+0x30 | yes |
| `Physics_SynchronizeToAux` | `315347d0` | 3,156 | `31543950` | `315460a0`, `3151e700`, `31543e20` | stateful; aux entity array `pose+0x150`, parts `pose+0x19c`; degree/radian conversion 57.29578, ±90/180, 1e-3 | yes |
| `Physics_SynchronizeToAux` SPU variants | `31546120/31546690/31547090/31547870` | 1,389/2,361/1,771/5,683 | job/SPU path | - | same maths, platform-specific | skip |

Details that matter for a faithful port:

* **Alive path** (`pose vt[0x120]+5 == 0`): if the animation is dirty
  (`CSkeletonAnim+0x458 != 0`) or aux parts exist, `SynchronizeWithPhysicsPost` first pulls the
  physics pose into the attachment/bone block (`3151c9e0`), then copies the animated pose into the
  physics parts. This is the normal standing/walking character: physics is passive.
* **Dead/ragdoll path** (`pose vt[0x120]+5 != 0`): the alive flag is false, so the same functions
  reverse the direction: physics part matrices are converted to bone matrices and written back into
  the pose by `Physics_SynchronizeToEntity`/`Physics_SynchronizeToAux`. This is the only ragdoll
  smarts inside cryanimation.
* The sync is gated by `DAT_316c9290` (global) and the aux path by `DAT_316c9388`. Both are runtime
  globals, not cvars.
* `pose+0x980` bit 0 is set by `BuildPhysicalEntity` from `param_5 > 0`; bit 2 is "wants physical
  entity"; bits 4/5 are sync-in-progress/aux state. Do not serialize the raw flag byte.

### 3.5 Ragdoll handover (x2game -> CryPhysics) (P2)

Correction to the survey's call list: **`FUN_391462a0` is not `RagDollize`**. It is a flow-node /
actor-action registration function (it registers strings `DumpActorInfo`, `Revive`, `Kill`,
`RagDollize`, `SetStats` with callbacks). The real handover is:

| function | addr | size | role | evidence |
|---|---:|---:|---|---|
| ragdollize | `391199a0` | 1,289 | builds `SPhysicalizeParams` (type 6, mass, flags `0xffbfffff`, `0x80000000`), calls entity `Physicalize` (vt+0x104), optional IK setup, `"FallAndPlay"` | `Tried ragdollizing player with 0 mass.`; mass from actor+0x530; default mass `_DAT_399abd8c` |
| actor state setter | `39141e70` | 255 | maps strings `alive`(1), `unragdoll`(0), `ragdoll`(2), `spectator`(6), `sleep`(3), `frozen`(4) and calls actor physics state `vt+0x48(8, state, 0)` | `_stricmp` per string; `FUN_39111d20` can-ragdoll gate |
| actor state getter | `391430b0` | 277 | reads `vt+0x4c(8)` and emits `profileChanged` with `alive`/`unragdoll`/`ragdoll`/`frozen`/`spectator` | string table `0x399abb...` |
| unragdoll / get-up | `39119400` | 1,429 | restores capsule/IK targets, re-physicalizes, clears state | calls actor+0xc entity `vt+0x104`/`+0x108`, IK setup |
| can ragdoll | `39111d20` | 38 | false in menu/dead/`actor+0x150` conditions | reads `DAT_3a5bb1d8+0x96`, actor+0x81c, +0x150 |
| state string -> profile | `39111ad0` | 53 | `alive`/`ragdoll` profile emit | `profileChanged` |

What is **not** in cryanimation: a ragdoll state machine, constraint setup, or `_ragdoll.cdf`. The
`_ragdoll.cdf` string belongs to the character customizer's cape/customize path (`FUN_39316fd0`,
profiler `X2::GameClient::CharacterCustomizer::AdjustCapeAsset`), not to death ragdoll. The solver is
CryPhysics (`aaemu-physics`). The port therefore needs:

1. the compiled proxies from `BuildPhysicalEntity` (this map);
2. the exchange in `Physics_SynchronizeToEntity/Aux` (this map);
3. the x2game state/impulse policy (P2, server-driven in AAEmu).

### 3.6 Hit reactions (P2)

There is **no hit-reaction animation system in cryanimation or cryaction**. Searches for
`hit_reaction`, `HitReaction`, `stagger`, `flinch`, `knockdown` over both string tables return nothing.
What exists:

* `Bip01 Hit` and every `Bip01 Hit*` bone are collected by the model bone map
  (`3161ce60`, §2.5) into `model+0xac..+0xb0`; the consumer of that vector is not in cryanimation
  (likely exposed to game code through the model interface) [?].
* `actor_models.use_ragdoll`, `use_ragdoll_hit`, `use_ragdoll_knock_down`, `ragdoll_hit_bone` are
  database/config fields (x2game cvar/config registration `390f7b40`/`390fac10`); the client loads
  them but there is no engine code that plays a hit animation.
* Skills/damage use `damage_effects.hit_anim_timing_id`, `skills.start_anim_id/fire_anim_id`, and
  `buffs.anim_start_id/anim_end_id` to select animations through the normal graph (E2). The visible
  hit reaction is therefore game-side animation selection + an optional CryPhysics impulse/ragdoll.
* `AnimationPoseModifier_Recoil` (`315bcbb0` factory, object `315bcbd0`, 0x30 bytes) exists but no
  ArcheAge code path using it was found; it belongs to CryGame's weapon recoil, not AA combat [I].

Port verdict: do not build a hit-reaction engine. Keep the `Bip01 Hit` bone ids available and let the
skill/animation layer choose clips; use `aaemu-physics` for impulses and the P2 ragdoll handover.

### 3.7 Look/aim pose (P2)

The pose has three independent contributions; do not merge them:

1. **Aim pose** = normal upper-body LMG blending (E2 §4/§5) driven by graph inputs.
2. **Look IK** = a post-process on the bone chain, driven by the game target direction.
3. **Eye look** = facial model eye bones (`eye_left_bone`/`eye_right_bone`), §5.5.

x2game handover:

| function | addr | size | role | evidence |
|---|---:|---:|---|---|
| `CPlayer::SetIK` | `39133390` | 701 | enabled byte at `param_2+0x24` gates the call; target dir at `param_2+0x18`; sets the `UsingLookIK` graph var, then calls animated-char vt+0x48 (SetLookIK, name inferred) with `(relaxed, blend, dir, 0, 1)` | profiler `CPlayer::SetIK`, `CPlayer::SetIK - local player` |
| graph variable resolution | `39133650` | 964 | resolves `LookIkBlend` and `BowLookIkBlend` sub-variables `spine1/2/3`, `neck`, `head` into actor+0x90c..0x930 | `LookIkBlend`, `BowLookIkBlend` strings |
| player graph var ids | `39131fc0` | 161 | caches ids for `Action`, `Item1/2`, `UsingLookIK`, `Vehicle`, `VehicleSeat` | string table |
| other actor graph var ids | `39128ab0` | 140 | same for non-player actors (no `VehicleSeat`) | string table |
| `UsingLookIK` | `39131fc0`/`39128ab0` | 161/140 | graph bool that gates the look IK contribution | graph input |

`actor_models` supplies the blend weights (server DB):
`game_look_ik_blend_head/neck/spine1/2/3` and `game_bow_look_ik_blend_head/neck/spine1/2/3`.
They are loaded by `39736340` and selected by the engine's look chain, not hard-coded.

Engine side:

| function | addr | size | callers | callees | state / offsets | P2 |
|---|---:|---:|---|---|---|---|
| `CSkeletonPose::SetPostProcessQuat` | `3152e450` | 425 | game/look IK | `31518f50`, `31505c60` | stateful; `pose+0x124` = post-process records, **stride 0x20**: quaternion `+0x0..0xc`, extra `+0x10`, bone id `+0x1c`; update or append | yes |
| `CLookAt::CFactory` / `CPoseBlenderLook` | `315b2f80`/`315b6220` | 6/6 | pose-modifier chain | - | factory thunks; modifier object constructors at `315b2fc0`/`315b6270` | yes |
| `CPoseBlenderAim` / LookAt factory | `315b3040`/`315b2f80` | 45/6 | `PoseModifiersPrepare 3154e3a0` (E2) | `315b31b0`, `3152cae0`, `315b2fc0` | builds modifier objects at model+0x98 / model look slots | yes |
| look IK solver (`CFacialModel` inferred) | `315b37c0` | 992 | virtual (facial model/post-process) | `315b2c50`, `31505780`, `31505d90`, renderer debug vt | stateful; `model+0xf0` look-bone chain, `+0xf4` look targets, `+0x6c` flag, `+0x80` threshold, `+0x99` flag, `+0x9c` (source bone), `+0xa0..0xb8` look state, `+0xc4` blend; constants ±0.09 rad deadband (`0x3167ac84`/`0x31677ee4`); validates "No look-bone specified" / "No LookIK-Setup" | yes |
| post-process application | `31599dc0`/`31599e80` | 181/1,263 | E2 `SkeletonPostProcess` | - | copies pose subtree and rotates bones under the changed parent; E2 §8.3 | yes |
| `AnimationPoseModifier_Recoil` | `315bcbb0`/`315bcbd0` | 6/~/0x30 | unused by AA [?] | - | defaults at +4 = 100.0, +0x10 = 0.8, +0x14 = 3 (axis) | P2 optional |

Cvars: `ca_UseLookIK`, `ca_DrawLookIK`, `ca_UseAimIK`, `ca_UseIMG_AIM`, `ca_UseAimIKRefPose`,
`ca_DrawAimIKVEGrid`, `ca_DrawAimPoses`, `ca_DebugADIKTargets` (E2 §8.3 has the full list). None of
them is read by x2game; the game drives the target and the actor_models weights.

### 3.8 What phase 1 actually needs here

P1 needs the proxy build and both sync directions, because the rider is a character on a moving mount
and the mount itself has an articulated proxy. P1 does **not** need look IK, aim pose, recoil or
`Bip01 Hit`. Port order in §7.

### 3.9 Phase-1 critical functions (summary)

| function | addr | size | unknown |
|---|---:|---:|---|
| `BuildPhysicalEntity` | `3153b5b0` | 8,036 | exact compiled-proxy chunk layout; CRC bone table; mass distribution |
| `CreateCharacterPhysics` | `315362a0` | 1,660 | meaning of `param_1/param_5`; entity-creation flags |
| `SynchronizeWithPhysics` | `31543a90` | 537 | global gates `DAT_316c9290/9388` |
| `SynchronizeWithPhysicsPrepare` | `31535430` | 220 | helper `31547780` |
| `Physics_SynchronizeToEntity` | `315391e0` | 3,920 | part->bone matrix convention |
| `Physics_SynchronizeToAux` | `315347d0` | 3,156 | aux part layout |
| `SynchronizeWithPhysicsPost` | `31538880` | 2,394 | attachment bone scale/blend formula |
| `UpdateBBox` | `31549f80` | 5,413 | bbox chunk transform |
| `BuildAuxPhysicsData` | `3161da30` | 2,370 | rope bone grammar |
| `BuildPickingProxy` | `31535d50` | 511 | picking geometry flags |

---

## 4. Skin, equipment and attachments (P1/P5)

This is how a character looks: body + head + hair + every armor part are separate CHRs, bound to the
same skeleton and blended either by the GPU (normal) or by the CPU subskin path (fallback/attached
objects). E2 §7 maps the rider seat and the generic attachment update; this section extends it to
equipment, face attachments, morph targets, and the cloth/hair question.

### 4.1 Call chain

```
model/equipment/customize change (x2game)
  CharacterManager::CreateCDFSkinInstance            315d6080
  CharacterManager::CreateCGAInstance                315cfd00
    child CCharInstance (slave skin)
      CAttachmentSkin::VerifySkeleton                31508830
        all slave bone names must exist in the master skeleton
      CAttachmentManager::AttachObjectToBone         315078e0 / 31507b00
        kind 0/1/2/4, bone id, local transform, alignment
per frame
  CSkeletonAnimTask::Begin -> UpdateAnimBones        3150df50  (E2)
    builds the per-bone attachment-use bitmask (256 bits)
  CSkeletonPose::SkeletonPostProcess
    CSkinInstance::UpdateAttachedObjects             315253f0
      CAttachmentManager::UpdateLocationAttachments   3150f340  (E2)
        kind 1: 3-bone skinning matrices -> CAttachmentSkin::SkinningAttSW 315095a0
        kind 2: entity transform (rider / child entity)
        kind 0/4/CA_FACE/CA_CAMERA: bone/camera transform
    CSkinningTransformationsComputationTask          315251e0 Begin
                                                     31525f40 Execute
                                                     31525310 Wait
      pose bones + animated-bone list -> 0x40-stride skin matrices
```

### 4.2 Attachment object model

`CAttachment` (base) ctor `31519670` (404 B) confirms the 0xf0-byte object; the fields are in §2.4.
`CAttachmentManager` is at `char+0x30`; live object vector at `manager+0x08/+0x0c`; animated-bone list
at `manager+0xac/+0xb0`; dirty byte `manager+0xa9`; skeleton `manager+0x2c`; owning char
`manager+0x30`. The per-bone compiled attachment/bone data is `model+0x658/+0x65c` (stride 0xf0).

`AttachObjectToBone` overloads:

| function | addr | size | behaviour |
|---|---:|---:|---|
| `AttachObjectToBone` (string) | `315078e0` | 531 | `param_1` = kind, `param_2` = bone name; resolves bone id via `char+0x254` pose vt[0x18]; logs the long warning when the bone is absent; stores `+0xb8` kind, `+0xc4` bone id, `+0xd4` alignment; zeroes local transform for kinds 1/2/4 |
| `AttachObjectToBone` (id) | `31507b00` | 427 | same with a pre-resolved bone id |
| `CAttachmentSkin::VerifySkeleton` | `31508830` | 232 | slave bone count <= master; every slave name found in master (`FUN_3161c290`); logs `Bone-names of SLAVE-skeleton ... don't match` / `SLAVE-skeleton needs to have less bone` |
| `CAttachment::Serialize` | `31508c50` | 690 | writes `AName`, class `CA_BONE/CA_FACE/CA_SKIN/CA_CAMERA`, `AttachmentType` (`entity`/`effect`), `Rotation`, `Position`, `AlignBoneAttachment`, `BoneName`, `Binding`, `Flags`, `Material`, `Hinge*`, `scale` |

Kinds (`+0xb8`) and classes (`+0x1c`) are separate enums; §2.4 has the table. The **kind 1** path is
the only one with per-vertex skinning; it is the one used for a child character/subskin. In
`UpdateLocationAttachments` kind 1 reads the two/three bone ids at the skinning record (`+0xbc` and
the next two `u16`), copies three 0x40-byte skinning matrices and blends vertices; kind 2 uses the
entity transform; other kinds use the generic bone matrix.

### 4.3 Per-frame attachment and skinning functions

| function | addr | size | callers | callees | state / offsets | P1 |
|---|---:|---:|---|---|---|---|
| `CAttachmentManager::UpdateAnimBones` | `3150df50` | 1,884 | E2 `CSkeletonAnimTask::Begin` | `31527310`, `31508f10` | stateful; dirty `+0xa9`; 256-bit bone-use mask built in 4 qwords; `+0x2c` skeleton, `+0xac/+0xb0` animated bones | yes |
| `CAttachmentManager::UpdateLocationAttachments` | `3150f340` | 12,725 | `CSkinInstance::UpdateAttachedObjects` | `315095a0`, `31518a90`, `31518a20`, `31519140` | stateful; live objects `manager+0x08/+0x0c`; compiled data `model+0x658/+0x65c` stride **0xf0**, index `model+0x50`; `manager+0x2c` skeleton, `+0x30` char; kind dispatch at object+0xb8; skinning matrices at skin-data+0xec, stride 0x40; `ca_DrawFaceAttachments`, `ca_useAttachmentItemEffect`, `ca_MergeAttachmentMeshes` | yes |
| `CSkinInstance::UpdateAttachedObjects` | `315253f0` | 570 | skin/entity frame | `3150f340`; object vt+0x4/0x8/0xc/0x10/0x40 | stateful; manager at `+0x30`; attached-object vector `+0x38/+0x3c`; if object type 3 sets `char+0x1ec` bit 1; type 2 uses the passed matrix | yes |
| `CSkinInstance::UpdateAttachedObjectsFast` | `31525630` | 485 | skin/entity frame | `3150f340` | stateful; root-bone-only fast path (E2 §7.2) | yes |
| `CAttachmentSkin::SkinningAttSW` | `315095a0` | 3,519 | `UpdateLocationAttachments` | `FUN_31519140` etc. | stateful; CPU 3-bone skinning of an attached subskin; vertex loop, 3 weights, 0x40-stride matrices; scratch 2042 floats | yes (fallback) |
| `CSkinningTransformationsComputationTask::Begin` | `315251e0` | 294 | char/skin frame | `FUN_3164b640` | stateful; task+0x6c = model+0x180+0x60 skin data; +0x70 pose skeleton; +0x74 char+0x30; +0x78 source pose; +0x80/+0x84 inputs; +0x88 scale | yes |
| `CSkinningTransformationsComputationTask::Execute` | `31525f40` | 2,598 | task/job | reads task fields; 0x1c pose stride, 0x40 output stride | stateful; builds skin matrices from pose bones + `manager+0xac/+0xb0` animated bones; reads task+0x6c/+0x70/+0x74/+0x78/+0x84/+0x88 | yes |
| `CSkinningTransformationsComputationTask::Wait` | `31525310` | 214 | char/skin frame | `FUN_3164b4c0` | stateful task wait | yes |
| `CCharInstance::BeginSkinningTransformationsComputation` | `3151a7d0` | 386 | char frame | `31525310`, `315251e0` | stateful; 3 task slots at `char+0x4c8 + (slot%3)*0x9c`; current slot `char+0x6a4`; source/dest matrices `char+0x494 + slot*0x10`; scale `char+0x298` | yes |
| `CCharInstance::WaitSkinningTransformationsComputation` | `3151a960` | 325 | char frame | `31525310` | stateful; waits the slot task and returns source/previous matrix ids | yes |
| `CCharInstance::MergeAttachmentRenderMeshes` | `3151adc0` | 407 | mesh merge path | `3165e450`, `3165e590`, `3165f160` | stateful; merges attached render meshes into the master skin mesh; profiler label only; not simulation | P2 optional |
| `CSkinInstance::Release` | `31524cf0` | 90 | destruction | - | stateful; releases skin/attach data | yes |
| `CAttachmentManager::DrawAttachments` | `31512500` | 4,658 | renderer | draw calls | stateful; render-side, not simulation | skip (renderer) |
| `CAttachmentManager::SkinningAttSW` | `315095a0` | 3,519 | see above | - | same as CPU subskin | yes |

`CSkinningTransformationsComputationTask::Execute` is the bridge the renderer consumes: it walks the
pose bone array (0x1c stride), converts each bone to a 0x40-byte matrix, applies the inverse scale
`task+0x88`, and writes the animated-bone subset. A Bevy port can either reproduce this buffer or
upload the pose directly; the differential harness should compare the 0x40-stride buffer, because the
attachment/CPU paths read it.

### 4.4 Bone map, face attachments and eyes

`CModelSkeleton` (`3161ce60`, 1,284 B) fills the fixed bone-id map from §2.5. It is the source for:

* `driver` / seat bone (rider path, E2);
* `item_*` bones used by equipment attachments (`item_side_l/r`, `item_hand_l/r`, `item_back_*`,
  `item_bow_l`, `item_arm_l`, `item_glider`, `item_staff_l`, `item_lute_r`);
* `camera` (camera attachment);
* `Bip01 Hit*` (hit chain);
* `eye_left_bone` / `eye_right_bone` (facial eyes/look);
* `Bone_Swim` and toe/heel/twist bones for procedural posture.

Face attachments:

* class `CA_FACE` (`+0x1c == 1`) is serialized as `CA_FACE`; `ca_DrawFaceAttachments` (default 1)
  gates drawing of skin/face attachment objects; the cvar description says "if this is 0, will not
  draw the skin attachments objects".
* `CFacialModel` resolves the two eyeball attachments by name (`315b32d0`, 344 B) into
  `model+0x4`/`model+0x8`; if the name is missing it logs
  `CryAnimation: Left/Right Eyeball attachments not defined or wrong name: %s` and stores 0.
* The eye/eyelid bone ids are `eye_left_bone`, `eye_right_bone`, and per-eye lid names
  `Eye_left/right_upper_left`, `..._middle_left`, `..._lower_left`, `..._lower_middle`,
  `..._lower_right`, `..._middle_right`, `..._upper_right`, `..._upper_middle`
  (`0x31678740-0x316788d0`). Eye look is §3.7; eyelid/morph animation is §5.

### 4.5 Morph targets

* CHR chunks: `ChunkType_MeshMorphTarget` (`0x316803a0`), `ChunkType_CompiledMorphTargets`
  (`0x316804b8`), descriptors `MESHMORPHTARGET_CHUNK_DESC_0001` (`numMorphVertices`),
  `COMPILED_MORPHTARGETS_CHUNK_DESC_0800` (`numMorphTargets`), `SMeshMorphTargetVertex`,
  `SMeshMorphTargetHeader`.
* CPU/API: `CSkinInstance::MorphCopyToGPU`, `MorphTargetCopy`,
  `AddMorphTargetsToVertexBuffer_Verified` (`FUN_31502200`, 4,853 B, merged bodies). These are GPU
  upload/vertex-buffer code; the survey classifies them as rendering. The port needs the CPU-side
  morph weights and target IDs, not the 2010s GPU buffer management.
* `ca_UseMorph` (default 1) gates the morph skinning step; `ca_UseMorph` description says the morph
  step is part of overall skinning during rendering.
* `SetMorphTime: Morph Target "%s" Not Found for character "%s"` is the only named game-facing morph
  setter string; the exact API function was not resolved (it is inside the morph block, not a
  standalone export) [?].
* Facial morph effectors in FXL use the same names as the CHR morph targets
  (`brow_inner_raise_l`, `eye_close_l`, `mouth_LipClose`, ...). The mapping is name-based [I];
  `!NEGATIVE MORPH TARGET ID!` / `!MORPH TARGET ID OUT OF RANGE!` are the guards.
* Shared morph targets: `CharacterManager::AddSharedMorphTargets 315cec90` (781 B),
  `ApplySharedMorphTargets 315cce20` (383 B), `PreCacheResources 315d0450` (766 B),
  `/objects/characters/preloadmorphlist.txt`.

### 4.6 Cloth and hair: what actually exists

**Hair is not simulated.** It is a separate CHR/CDF attached to the head (`item_*` / head bones),
with per-color material variants (`*_hcNN.mtl`). The only engine-side work is the normal attachment
and skinning path above. Hair selection/color is game data (`hair_colors`, `custom_hair_textures`,
`character_customizing_hair_assets`, `equip_pack_body_parts`), not animation.

**Cloth has cvars but no solver code path found in this build.** `cryanimation.dll` registers:

```
cloth_max_timestep (default 0), cloth_max_safe_step (0), cloth_stiffness (0),
cloth_thickness (0), cloth_friction (0), cloth_stiffness_norm (0),
cloth_stiffness_tang (0), cloth_damping (0), cloth_air_resistance (0),
cloth_mass_decay (1.0), ca_cloth_vars_reset (2)
```

but every `cloth_` reference in the decompile is the cvar registration (`FUN_3164b810`); there is no
`CCloth`/solver/vtable string, no cloth chunk in the CHR chunk list, and the only cloth-related string
outside is `cloth_mass_decay_attached_scale` in `crysystem` and `Deformable StatObj ClothTangents` in
`cry3dengine` (a vertex-format name). The 1.2 assets show no cloth chunks. Treat cloth as disabled in
1.2; do not build a solver for parity. If a live test ever shows a moving cape, it is the cape CHR's
skin attachment, not cloth.

### 4.7 What phase 1 needs

P1 needs:

1. the bone map (`3161ce60`) so seat/weapon/equipment bones resolve;
2. `AttachObjectToBone` and `VerifySkeleton`;
3. the attachment update (`3150f340`) for kind 2 (rider entity) and kind 1 (equipment subskin);
4. the skinning task buffer (`315251e0/31525f40`) for the visible rider.

P1 does **not** need morph upload, face attachments, hair simulation or cloth. `ca_DrawFaceAttachments`
and `ca_UseMorph` can be defaults until the character-creation work (P5).

---

## 5. Facial animation (P5, minor P2)

Facial animation is a second animation system attached to the same character. It does not replace the
body graph: it layers morph-target weights and a few bone rotations (head/eyes) on top of the pose E2
produced. The visible result is the 1.2 face: expression emotes, NPC chat expressions, eye look/blink,
lip sync, and character-creation morphs.

### 5.1 Data formats (shipped) [R]

| file | count in pak | format | role |
|---|---:|---|---|
| `.cal` | 989 | text `name = path` lines, `$Include`, `$FaceLib`, `#setting` | animation library; `facial_player.cal` lists FSQ sequences, face CALs list the FXL |
| `.fsq` | 894 | XML `FacialSequence` | one facial sequence: channels + splines, body `SkeletonAnimationEntry`, sound/camera entries |
| `.fxl` | 9 | XML `ExpressionLibrary` | facial effectors (group/expression/morph/bone), submixes |
| `.joy` | 4 | XML `Joysticks` | character-creation UI joystick layout |
| `*_targets.xml` | 12 (race/gender) | XML `FaceMaker` | customization targets (morph name, range, apply mode) |

The CAL chain is:

```
global_player.cal
  $Include = objects/characters/facial_player.cal
    fist_em_bow = facial/fist_em_bow.fsq
    npc_question = facial/npc_question.fsq
    ...
  <face>.cal
    $facelib = animations/pc_expressions.fxl
```

`$Include`, `$FaceLib`/`$facelib` (case-insensitive), `$filepath`, `#BaseModelHeight`,
`#AnimModelHeight`, `#LMGPlaySpeedScale`, `#ParseSubFolders` are parsed by `CCalParser`/`CAL` helpers
(`315c2c50`, `315c48f0`, `315c5240`). Unknown lines are skipped. Duplicate includes log
`Duplicate .cal file included`.

### 5.2 Object model and update order

```
CFacialInstance (6 layers)
  PlaySequence(name, layer)            31598330
    -> LoadSequence                   315985d0   resolve name via master char animation set
         -> CFacialAnimation::LoadSequence 3157bb00   name + ".fsq"
              -> CCalManager/CCalParser 315c0ba0/315c2c50
  Update                              31597f80   per frame
    -> UpdateCurrentSequence          31597df0   active = lowest non-empty layer
         -> context Start/Stop          3157b330 / 3157ad80
    -> CFacialAnimationContext::Update 3157a800
         effect list (0x30) -> weights
         apply to model/pose via CFacialModel::FillInfoMapFromFaceState 3159aa40
    -> eyes/look                    315b37c0 (P2), eyelid/eye morphs (P5)
    -> phonemes                     3159dbc0
```

Layers are 0..5. `PlaySequence` stores `(sequence, flag, loop)` in the slot and makes the lowest
non-empty layer active; layers 4 and 5 force the loop/active flag. E2 already uses the same
per-character frame; facial update must run after the body pose, because bone effectors write into the
pose.

### 5.3 `CFacialInstance` functions

| function | addr | size | callers | callees | state / offsets |
|---|---:|---:|---|---|---|
| ctor | `31598f10` | 668 | character creation/attach | `FUN_3159a540`, context ctor | stateful; `+0x0c..0x3c` 6 slots; `+0x40` play param; `+0x44` master char; `+0x48` owner handle; `+0x4c` model; `+0x50` context (0x70 bytes); `+0x9c`, `+0xa4` |
| `UpdateCurrentSequence` | `31597df0` | 387 | `Update` | `3157ad80`, `3157b330`, `31579620` | stateful; selects first non-empty slot; `+0x3c` active; logs start/stop at `DAT_316c9254 != 0` |
| `Update` | `31597f80` | 399 | character frame | `3157af90`, `315799a0`, `31599260`, `31597df0` | stateful; advances context; when a sequence finishes, clears the slot and activates the next |
| `PlaySequence` | `31598330` | 359 | game/facial interface | `31597df0` | stateful; `(seq, layer, flag, loop)`; layer 4/5 loop special-cased |
| `StopSequence` | `315984a0` | 298 | game/facial interface | `31597df0`, `3157af90` | stateful; finds the slot by sequence pointer |
| `LoadSequence` | `315985d0` | 667 | PlaySequence/game | `char+0x44` animation set vt+0x6c, facial system vt+0x1c/0x20 | stateful; path length <= 0x400; errors `not listed in CAL`, `no animation set`, `no master character instance`; `ca_stream_facial` selects sync vs stream |
| `Reset` | `315989f0` | 268 | detach/teleport | context Reset | stateful; clears all layers |
| `CFacialAnimation::LoadSequence` | `3157bb00` | 1,105 | `LoadSequence` | `3157d560`, `3157d940`, `CCalManager`, file system | stateful; appends `.fsq` (`DAT_31678b8c` = `fsq`), streams if `ca_stream_facial`, evicts a cached sequence when `ca_FacialSequenceMaxCount` is exceeded, then retries |
| `CFacialAnimationContext::Update` | `3157a800` | 1,407 | `CFacialInstance::Update` | `31579300`, `3157a3d0`, `31579880`, `31579c00`, `3157cd70`, `3157e280`, `31579730` | stateful; timer from `DAT_316c2224[5]`; effects stride 0x30; active sequences at `+0x50/+0x54`; changed `+0x68`; smoothing `+0x6c`; update cadence gated by `ca_FacialAnimationFramerate` (20) and `ca_FacialAnimationRadius` (5.0) [I] |
| context ctor | `3157a4f0` | 100 | instance ctor | - | stateful; effect/sequence vectors, `+0x44` owner, `+0x48` model, `+0x68/+0x6c` |
| effect begin/stop | `3157ad80` | 192 | UpdateCurrentSequence | - | stateful; logs sequence start/stop |
| effect start | `3157b330` | 499 | UpdateCurrentSequence | `3157af90` etc. | stateful; adds sequence effects at layer time |
| context reset / smoothing | `31579620` | 14 | UpdateCurrentSequence | - | pure setter |
| sequence playing test | `315799a0` | 63 | `Update` | - | pure-ish; returns finished flag |

### 5.4 Effectors, FXL and CAL

| function | addr | size | role |
|---|---:|---:|---|
| `CFacialEffectorsLibrary` ctor | `31593f20` | 390 | library object; effectors map |
| `AddEffector` | `315940b0` | 347 | parse/lookup by name; duplicate logs `Effector with name %s already exist` |
| library destructor | `315959d0` | 57 | release |
| `CCalManager::Update` | `315c0ba0` | 495 | per-frame CAL load/stream queue |
| `CCalParser::Parse` | `315c2c50` | 2,857 | line parser (`$Include`, `$FaceLib`, settings, `name = path`) |
| CAL key/value parser | `315c48f0` | 2,374 | `#filepath`, `#BaseModelHeight`, `#AnimModelHeight`, `#LMGPlaySpeedScale`, `#ParseSubFolders` |
| CAL register | `315c5240` | 5,790 | register animation/sequence entries into the character animation set |
| `CryCHRLoader::LoadFacialLibrary` | `315ec600` | 388 | load the FXL via the facial system |
| `CryCHRLoader::LoadFaceLib` | `315ec9f0` | 647 | load a face lib; error `Facial Effector Library %s not found (when loading %s.cal)` |
| `Init Face Makers` | `315cc500` | 763 | load the 12 `*_targets.xml` files; logs `Init Face Makers - Done!` |
| `X2FaceMaker` parser/apply | `316641f0`, `316646d0`, `31664b40`, `31664c00`, `31664f20`, `31664ff0`, `31665190`, `31665390`, `31665750`, `31665760` | 1,174 / 864 / 188 / 291 / 204 / 410 / 495 / 574 / 7 / 34 | `FaceMaker` XML -> target list; errors listed in strings (`Targets node is not exist`, `Asset Path is empty`, `invalid apply method`, `index already exist`, ...) |

FXL semantics [R from `pc_expressions.fxl`]:

* `Effector Type="group"` = folder; `Type="expression"` = submix; `Type="morph"` = one morph target;
  `Type="bone"` = bone rotation with `Attachment`, `PosOffset`, `RotOffset`; `Type="attachment"` is
  rejected (`Error: Attachment Effectors not longer supported!`).
* `SubCtrl Type="1" Weight="..." Keys="..." Effector="..."` maps an input channel/key range onto a
  child effector. Keys use the same `time:value:flag` grammar as FSQ splines.
* Bone effectors in the shipped library: `head_x` (`Bip01 Head`, rot `0.87266463` rad = 50° about X),
  `head_y` (25° about Y), `head_z` (40° about Z).
* Morph names are the CNT/CHR morph-target names (`brow_inner_raise_l`, `eye_close_l`,
  `mouth_LipClose`, `smile_teeth`, ...), so the FXL and the CHR must be paired by name [I].

`CFacialModel::FillInfoMapFromFaceState 3159aa40` is the application step: it takes the per-effector
weight vector (`param_2+0xc`, count; bone weights at `+0x1c`), the effector library (`model+0x1c`),
and writes bone rotations/morph weights. `model+0x20` is the per-effector face-bone index (`-1` =
none) and `model+0x30` the 8-byte bone bind/rotation table. Bone effectors (type 3) go through
`31599c60` and `3159c570`; morph effectors set the morph-target weight. `DAT_31676938` = **1e-4** is
the weight cutoff.

### 5.5 Eyes, blink and look

* Eye bones: `eye_left_bone`, `eye_right_bone` (model bone map `3161ce60`); eyeball attachments are
  resolved by `CFacialModel` init `315b32d0` (344 B) into `model+0x4`/`model+0x8`; missing names log
  `Left/Right Eyeball attachments not defined or wrong name: %s`.
* Eyelid morphs/expressions: `eye_close_l/r`, `eye_raise_l/r`, `smile_l/r`, `L_close_suprise`,
  `R_close_suprise`, `e_smile`, `eye_rotate_l/r/up/down`, `eye_brow_*` (FXL and FSQ channel names).
  The model has 8 lid bone names per eye (`Eye_left/right_upper_left`, `..._middle_left`,
  `..._lower_left`, `..._lower_middle`, `..._lower_right`, `..._middle_right`,
  `..._upper_right`, `..._upper_middle`) for procedural eye/lid motion.
* `ca_eyes_procedural` (default 1), `ca_DebugFacialEyes` (0), `ca_DebugFacial` (0),
  `ca_lipsync_debug` gate eye/lip diagnostics. The procedural eye update is inside the facial model
  block; `315b3ba0` (9,049 B) is the debug/eye draw path (`EyeRight: %f %f %f %f m_IKBlend: %f`).
* Look IK for the eyes is the same `315b37c0` solver used for head/neck (P2); `ca_UseLookIK` gates it.
  There is **no x2game blink code**: the `SetNotifyBlinkingFreq` string belongs to a UI tab widget
  (`FUN_3925a5c0`), not to facial animation. Blink is engine/FXL/FSQ-driven.

### 5.6 `CFacialAnimSequence` / FSQ channels

`CFacialAnimSequence` fields are in §2.5; `Serialize` `315827c0` (2,421 B) writes/reads:

| XML node | sequence offset | notes |
|---|---|---|
| `Flags`, `StartTime`, `EndTime` | `+0x0c`, `+0x38`, `+0x3c` | |
| `Joysticks` | `+0x14` | `Game/Animations/pc_joy.joy` |
| `Channel`/`Spline` | `+0x28/+0x2c` | nested channels, keys `time:value:flags` |
| `SoundEntry`/`Sentence` | `+0x40/+0x44`, stride 0x14 | sound name, sentence, start time |
| `SkeletonAnimationEntry` | `+0x18/+0x1c`, stride 0x10 | `AnimationName`, `StartTime`, `EndTime` |
| `CameraPathPositions/Orientations/FOV` | `+0x94/+0xac/+0xc4` | empty in all sampled FSQs |

Top-level FSQ channel groups (all sampled files share this layout): `eye`, `eye_ball`, `eye_brow`,
`head`, `mouth_aeiou`, `mouth_emotion`, `nose`, `tongue`. The sampled `npc_question.fsq` has 47 splines.

`SkeletonAnimationEntry` is how an emote like `fist_em_bow` also plays a body animation
(`fist_em_bow` CAF in the same sequence) [R data]. The cryanimation decompile stores/serializes the
entry but has **no static caller** that turns it into `StartAnimationById`; it is exposed through the
sequence interface and started by the game/actor layer [?]. The port can play the named CAF directly
from the FSQ entry when it starts a facial sequence.

Sound entries are similarly stored and exposed; the actual sound playback is the audio layer.

### 5.7 Lip sync and phonemes

* FSQ carries authored `mouth_aeiou` (a/e/i/o/u) and `mouth_emotion` channels. This is the visible
  lip animation for the shipped emotes/NPC lines; no runtime TTS is needed.
* `CFacialAnimSequence` has a `BakedLipSync` flag (`0x3167944c`); `ca_lipsync_phoneme_offset` (20 ms),
  `ca_lipsync_phoneme_crossfade` (70 ms), `ca_lipsync_phoneme_strength` (1.0),
  `ca_lipsync_vertex_drag` gate runtime lip sync.
* `CPhonemesLibrary` (`3159d8e0` ctor 537 B, `3159dbc0` deserialize 2,555 B) parses a `Phonemes`
  node. No `.phn`/phoneme file was found in the pak and no FXL contains phonemes; treat this as a
  cutscene/TTS path that is unused for normal AA gameplay until proven otherwise [?].

### 5.8 x2game / database handover (P5)

* `sphere_chat_bubbles.facial`, `bubble_chats.facial`, `quest_chat_bubbles.facial` are **TEXT**
  sequence names, e.g. `npc_question`, `npc_angry`, `npc_sad` (server DB, read-only). They match the
  `facial_player.cal` entries.
* x2game anim-event handler `FUN_39138a40` (1,189 B) handles the event name `facial_expression`:
  splits the payload into four delimiter-separated tokens, parses three as int/float/float, resolves
  the actor's facial interface, and calls its play method on the facial instance with a constant 1.0
  and one parsed float. The sequence name is the DB `facial` value carried by the event; the other
  two parsed values are discarded by the decompile, so the argument order and layer/blend mapping are
  unresolved [?].
* the actor model / NPC setup and the `actor_models.face_target_instantly` DB column (loaded by
  `39736340`) select instant vs blended facial target application; the name implies the behaviour [I].
* Character creation probes 12 `*_targets.xml` paths; **9 are shipped** (dwarf and warborn female are missing from the pak). The nuian male sample has 81 targets,
  `custom_face_presets`, `total_character_customs`, `face_decal_assets`,
  `face_diffuse_maps`/`face_normal_maps`/`face_eyelash_maps`. The engine side is the FXL morph
  effector path above; decals/textures are renderer-side.

### 5.9 Phase relevance

Facial is **P5-first**: it matters for NPC dialogue, quest emotes, character creation and cutscenes.
For P2 it matters only in that the eye/head look IK (`315b37c0`) and the eyelid/eye bone map are shared
with combat aim; these can be stubbed until combat needs them. For P1 the rider's face can be neutral:
the horse gallop and rider seat do not depend on facial state. `ca_UseFacialAnimation` defaults to 1,
but a correct P1 client may run with the facial instance idle (no sequences) without visual
difference on the mount.

### 5.10 Key unknowns

1. `FUN_315ed8a0` (loader core) and the exact CHR morph-target/chunk layout (blocks a full morph port).
2. Effector-index -> morph-target-id mapping in `CFacialModel` (`model+0x20/+0x30` arrays); whether it
   is built from names or from CHR chunk ids.
3. The runtime consumer of `SkeletonAnimationEntry` (body clip from an FSQ).
4. The `facial_expression` event argument order and how the sequence name reaches the interface.
5. Whether `CFacialModel` look-bone chain (`+0xf0/+0xf4`) comes from CHR params, a UDP, or a fixed
   Bip01 chain.
6. The exact eye procedural algorithm (blink/saccade) and its cvars.

---

## 6. Model / animset loading runtime dependency (P1/P5)

E4 does not re-map the asset formats (CAF/DBH/DBA are E2's dependency). It maps only what the runtime
contract needs: how a character instance is created, which CHR chunks must be parsed for the physics
and attachment paths above to work, and how the animation set is looked up.

### 6.1 Call chain

```
xg2ame / entity requests a character by file
  CharacterManager::CreateInstance                  315d6820
    init engine globals (renderer/physics/system/debug)
    extension dispatch:
      .cga -> CreateCGAInstance                     315cfd00
      .chr -> CreateCHRInstance                     315d3c90
      .cdf -> LoadCharacterDefinition               315d5010
    model cache FetchModel                          315cfbd0
    allocate CCharInstance (0x230) + CSkinInstance (0x6a8)
      FUN_31529030 / FUN_3151dfd0
  CHR path:
    CryCHRLoader::LoadNewCHR                        315f1510
      "<character>.cal" -> CCalManager (CCalParser)
      LoadFromContents                              315f1260/315f1283
        build core                                  315ed8a0  (14,072 B, DECOMPILE FAILED)
        LoadFacialLibrary / LoadFaceLib             315ec600 / 315ec9f0
        LoadModelAnimations / LoadExtraResources    315f0fb0 / 315f1110
per frame
  CharacterManager::Update                          315d3360
    CCalManager::Update, CAnimationManager::UnloadUnusedAnim,
    CAnimationContext::Update, instance list, streaming, GC
```

### 6.2 CharacterManager functions

| function | addr | size | callers | callees | state / offsets | P1 |
|---|---:|---:|---|---|---|---|
| `CreateInstance` | `315d6820` | 571 | entity/game `ICharacterManager` | `CreateCGAInstance`, `CreateCHRInstance`, `LoadCharacterDefinition`, `FetchModel` | stateful global; initializes `DAT_316c2240/2248/224c/2244`; path normalization `\`->`/`; extension dispatch | yes |
| `CreateCHRInstance` | `315d3c90` | 1,302 | `CreateInstance` | `FUN_315cabe0`, `FetchModel`, `FUN_31529030`, `FUN_3151dfd0` | stateful; allocates CCharInstance `0x230` and CSkinInstance `0x6a8`; uses resolved file name; flags `0xdeadbeef` | yes |
| `CreateCGAInstance` | `315cfd00` | 876 | `CreateInstance` | model cache, CGA instance | stateful; animated stat object path (world doodads/vegetation) | P3 |
| `CreateCDFSkinInstance` | `315d6080` | 590 | equipment/customize | model cache, skin instance | stateful; CDF (file + material) path for equipment/hair parts | yes |
| `FetchModel` | `315cfbd0` | 294 | all create paths | model cache/hash | stateful; returns existing CCharacterModel or 0 | yes |
| `LoadCharacterDefinition` | `315d5010` | 4,195 | `.cdf` path | XML parser, `FetchModel`, create | stateful; parses `<CharacterDefinition><Model File=... Material=.../></CharacterDefinition>` | yes |
| `SyncAllAnimations` | `315ca330` | 290 | manager update | - | stateful; flushes per-frame instance list and done flags (E2) | yes |
| `ApplySharedMorphTargets` | `315cce20` | 383 | loader/manager | morph target table | stateful; applies shared morph target | P5 |
| `AddSharedMorphTargets` | `315cec90` | 781 | loader | file system, morph table | stateful; loads `preloadmorphlist.txt` entries | P5 |
| `PreCacheResources` | `315d4710` | 241 | startup | - | stateful; precaches a model/asset | P5 |
| `ReleaseCustomizedMesh` | `315d3750` | 145 | customize | - | stateful; releases customized mesh | P5 |
| `UpdateStreaming` | `315d0ee0` | 1,702 | manager update | stream tasks | stateful; CHR/CGA/CDF stream-task pump | replace |
| `UpdateGarbageCollection` | `315cda80` | 997 | manager update | erase list | stateful; unloads unused models | replace |
| `CleanEraseList` | `315cd920` | 335 | GC | - | stateful | replace |
| `UnregisterModel` | `315cd650` | 308 | GC | - | stateful | replace |
| `UpdateRendererFrame` | `315cafb0` | 414 | render frame | - | stateful | replace |
| `Update` | `315d3360` | 1,002 | engine frame | `FUN_315c0ba0`, `FUN_3161ffb0`, `FUN_315ca520`, instance list | stateful; refreshes engine globals, timer, CAL manager, animation manager, instances | yes |

The manager is `DAT_316c2254`; E4 confirmed `+0x154` = `CCalManager`, `+0xa0` = `CAnimationManager`
(from `CharacterManager::Update`). The renderer interfaces are cached in `DAT_316c2240` (renderer),
`DAT_316c2248` (physical world), `DAT_316c224c` (system/file), `DAT_316c2244` (debug draw).

### 6.3 CHR/CDF build

| function | addr | size | role |
|---|---:|---:|---|
| `CryCHRLoader::LoadNewCHR` | `315f1510` | 691 | opens `<character>.cal` via CCalManager, calls `315ec790`, builds `.cal`/CAL cache, then `LoadFromContents` |
| `CryCHRLoader::LoadFromContents` | `315f1260` | 35 (+ split `315f1283` 649) | calls the build core `315ed8a0`, then `315f1110` extra resources and `316079d0` mesh release |
| CHR build core | `315ed8a0` | **14,072** | **DECOMPILE FAILED**; reads CHR chunks, builds CCharacterModel/CModelSkeleton/CModelMesh, compiled physical bones/proxies, morph targets, facial model; the single largest unknown |
| `CryCHRLoader` precheck | `315ec790` | 597 | validates/loads the model/entry |
| `CryCHRLoader::LoadModelAnimations` | `315f0fb0` | 350 | registers the CAL's CAF/LMG entries with the animation set |
| `CryCHRLoader::LoadExtraResources` | `315f1110` | 328 | loads materials/extra resources after the build |
| `CryCHRLoader::LoadFacialLibrary` | `315ec600` | 388 | loads the FXL library named by the face CAL |
| `CryCHRLoader::LoadFaceLib` | `315ec9f0` | 647 | loads a face lib into the facial system; error strings in §5.4 |
| `CCharacterModel` layout writer | `316078c0` | 264 | serializes/validates the section layout: `+0x60` `ModelSkeleton`, `+0x5d4` `AnimationSets`, `+0x658` `CModelMesh` section (0xf0 records), `+0x684` `FacialModel` |
| `CCharacterModel` mesh release | `316079d0` | 112 | releases the CModelMesh vector (0xf0-stride records) |
| `CCharacterModel` dtor | `31609c70` | 821 | releases all subobjects |
| `CModelSkeleton` ctor | `3160b9a0` | 811 | skeleton subobject; bone vector at `+0x0c`; many side vectors |
| `CModelMesh` ctor | `3160bde0` | 1,376 | mesh subobject (0xf0-ish record family); skinning/morph pointers |
| `CModelMesh::SkinningExtSW` | `316172a0` | 851 | software skinning fallback |
| `CModelMesh::GetSingleSkinnedVertex` | `31617600` | 2,275 | software single-vertex skin query (debug/CPU) |

Compiled CHR chunks that matter for runtime [R from the chunk-name table]:

| chunk | runtime use |
|---|---|
| `ChunkType_CompiledBones`, `ChunkType_BoneNameList`, `ChunkType_BoneInitialPos` | bone array (0x120 stride), names, rest pose |
| `ChunkType_CompiledPhysicalBones` | per-bone physical parameters (`mass`, bone flags) |
| `ChunkType_CompiledPhysicalProxies` | `BuildPhysicalEntity` proxy geometry, CRC bone match |
| `ChunkType_CompiledMorphTargets`, `ChunkType_MeshMorphTarget` | facial/customization morph targets |
| `ChunkType_FootPlantInfo` | foot-plant/foot-IK data (P2) |
| `ChunkType_BonesBoxes` | per-bone bounds |
| `ChunkType_BoneMesh`, `ChunkType_BoneAnim`, `ChunkType_BoneLightBinding` | render/attachment bindings |

The compiled physical proxies are the only chunks that must be byte-exact for P1: their layout drives
`BuildPhysicalEntity` (part count/records, mass, bone CRC match). Everything else can be converted to
Bevy-native structures as long as the contracts hold (bone order/stride 0x120, pose stride 0x1c,
animation set names).

### 6.4 Animation set / CAF / LMG lookup

| function | addr | size | role |
|---|---:|---:|---|
| `CAnimationSet::GetModelAnimationHeader` | `3160cc50` | 329 | name -> animation header (used by `StartAnimationById`/E2) |
| `CAnimationSet::LoadFileCAF` | `31610390` | 924 | load/register a CAF header |
| `CAnimationSet::LoadFileLMG` | `3160f710` | 865 | load/register an LMG |
| `CAnimationSet::VerifyLMGs` | `3160db30` | 240 | validation |
| `CAnimationSet::VerifyLocomotionGroup` | `3160c880` | 715 | validation of locomotion groups |
| `CAnimationManager::LoadAnimationTCB` | `31621c90` | 877 | TCB controller clips |
| `CAnimationManager::GetAnimEventInstances` | `3161f950` | 1,098 | animation event instances (sound/fx) |
| `CAnimationManager::UnloadUnusedAnim` | `3161ffb0` | 521 | unload policy (`ca_UnloadAnimTime`) |

For the port, the animation set is a name -> clip-index map plus the CAF/DBH data E2 samples. The
`GetModelAnimationHeader` function is the only lookup the playback path actually calls; the
`Verify*`/`Unload*` functions are validation/eviction policy and can be replaced by Bevy asset
lifetimes. `CAnimationSet` is at `CCharacterModel+0x5d4`; `char+0x180` is the `CCharacterModel` container
used by the pose/skin path.

### 6.5 What must be ported vs replaced

| concern | must port | can replace |
|---|---|---|
| file identity | CHR/CDF/CAF/LMG/FXL/FSQ path resolution, case-insensitive names | async loading, cache eviction, precache heuristics |
| skeleton | bone order/names, stride 0x120, rest pose, bone id map | bone LOD, joint masking |
| physics | compiled physical bones/proxies, proxy CRC bone match, part records | stream tasks, SPU/job variants |
| attachments | compiled attachment/bone data (0xf0), live attachment objects, subskin bone names | render-mesh merging (`ca_MergeAttachmentMeshes`), draw path |
| animation set | CAF/LMG/DBH headers and name lookup | thread/task framework, `CAnimationThreadTask` |
| morph/facial | morph target data, FXL/FSQ/CAL parsing, effector mapping | GPU morph upload, streaming cache |
| LOD | bone/LOD masks only if parity needs them | `ca_Lod*`, `ca_ChrBaseLOD`, task-skipping |

### 6.6 Phase relevance

P1 needs the CHR build for the mount and rider: bone names (seat/`driver`, `item_*`), compiled
physical proxies, and the character model/mesh split. It does **not** need `CAnimationManager`
eviction, LOD, precache, streaming tasks, shared morph targets, or the facial model. The failed
`315ed8a0` decompile is the main P1 risk: it is exactly the build stage that populates the proxy and
bone arrays. Until it is recovered from assembly or a live memory read, the port should implement
CHR parsing against the chunk specs already used by `aa-cry` and validate by comparing the live
`model+0x60` skeleton and `pose+0x19c` part records.

---

## 7. Port order (testable leaves first)

The order below is dependency order, not priority order. Each step has a differential check that can
run against the original in-process before the next step depends on it. **P1** marks the riding/on-foot
critical path; P2/P5 can be stubbed behind the same interfaces.

### Step 0 - data-only parsers (pure; no engine calls)

| task | input | check |
|---|---|---|
| CHR chunk parser (compiled bones/names/initial pos/physical bones/proxies/morph/footplant) | `objects/characters/**/*.chr` | compare bone count/names/order, proxy count and per-bone CRC match with live `model+0x60` |
| CDF parser | `**/*.cdf` | compare `File`/`Material` pairs with live model paths |
| CAL parser (`$Include`, `$FaceLib`, settings, `name = path`) | `global_*.cal`, `facial_*.cal`, per-face CAL | compare resolved entry count/names with live animation set |
| FXL parser (effector tree, SubCtrl, bone/morph/expression) | 9 `.fxl` | compare effector count/names/types with live library (`model+0x1c`, `+0x34`) |
| FSQ parser (channels/splines/skeleton/sound entries) | 894 `.fsq` | compare channel names/key counts with live sequence object |
| face-maker target parser | 9 shipped `*_targets.xml` | compare target index/name/morph/apply/min/max with live face maker |
| bone-id map | `CModelSkeleton` bone names | compare `+0x28..+0x96` ids and `Bip01 Hit*` vector with live model |

These are pure and can be unit-tested against extracted files; no process risk. **P1** needs CHR, CDF,
CAL and the bone map. FXL/FSQ/targets are **P5**.

### Step 1 - pure math leaves (P1)

| function | why first |
|---|---|
| `FUN_31544110` (298 B) matrix conversion | used by `SynchronizeWithPhysicsPost`; feed known quat/pos, compare 4x4 |
| `FUN_3151e700` (224 B) matrix helper | used by `Physics_SynchronizeToEntity/Aux` |
| `FUN_315460a0` (108 B) per-bone/part value | used by proxy build and sync |
| `FUN_31508570` (696 B) attachment bone scale/blend | used by both sync directions; easiest isolated check |
| `FUN_31508f10` (129 B) bone bitmask recursion | attachment bone-use mask |

Call them with scratch 4x4/quat buffers; they do not need a live world.

### Step 2 - proxy assembly (P1)

| function | check |
|---|---|
| `CSkeletonPose::BuildPhysicalEntity 3153b5b0` | on a live character: compare `pose+0x190` count and `pose+0x19c` records (stride 0x40) before/after; verify part->bone map |
| `CSkeletonPose::BuildPickingProxy 31535d50` | compare geometry id count/ids (0x1f4+) |
| `CModelSkeleton::BuildAuxPhysicsData 3161da30` | compare `pose+0x150` aux count for a rope/cape model |
| `CSkeletonPose::CreateCharacterPhysics 315362a0` | create/destroy cycle; compare `pose+0x148`, `+0x980` bits, `+0x944/+0x948` |

This is the first step that needs a live physics world. Use a live mount/player and read, never
destroy the live entity without restoring it.

### Step 3 - sync (P1)

| function | check |
|---|---|
| `SSkeletonPose::UpdateBBox 31549f80` | call on a live pose with dt 0; compare model bbox against a manual bone-bounds calculation |
| `SynchronizeWithPhysicsPrepare 31535430` | call from a scratch task at prepare time; compare copied input |
| `Physics_SynchronizeToEntity 315391e0` | living character: pose -> physics; dead/ragdoll: physics -> pose. Compare bone matrices (0x1c stride) |
| `Physics_SynchronizeToAux 315347d0` | rope/aux model; compare aux part transforms |
| `SynchronizeWithPhysicsPost 31538880` | compare attachment bone scale/blend (`31508570` input/output) |
| `SynchronizeWithPhysics 31543a90` | compare `char+0x1ec` bit 2 and `pose+0x980` flags after alive/dead frames |

The differential oracle is the final pose matrix block (`pose+0x18`, 0x1c per bone) plus the
attachment manager matrices. Run the same frame in the original and the port with identical input
pose/physics state.

### Step 4 - attachments and skinning (P1)

| function | check |
|---|---|
| `CModelSkeleton` bone map `3161ce60` | ids for `driver`, `item_*`, `eye_*`, `Bip01 Hit*` |
| `AttachObjectToBone 315078e0/31507b00` | scratch/live attachment object: `+0xb8/+0xc4/+0xd4` and local matrix |
| `CAttachmentSkin::VerifySkeleton 31508830` | equip a part with one renamed bone: both reject; correct part: both accept |
| `CAttachmentManager::UpdateAnimBones 3150df50` | compare the 256-bit bone-use mask for a known equipment set |
| `UpdateLocationAttachments 3150f340` | compare per-object world matrices and subskin vertex transforms |
| `CSkinInstance::UpdateAttachedObjects 315253f0` | compare child entity transforms (rider seat is E2) |
| `CSkinningTransformationsComputationTask 315251e0/31525f40` | compare 0x40-stride skin matrix buffer |

`ca_MergeAttachmentMeshes`, `ca_DrawFaceAttachments`, `ca_useAttachmentItemEffect` should be read
once and applied as rendering policy, not re-tested every frame.

### Step 5 - loader integration (P1, highest risk)

| function | check |
|---|---|
| `CharacterManager::CreateInstance 315d6820` | call with a path and entity; compare returned `CCharInstance` and the model cache hit |
| `CreateCHRInstance 315d3c90` | compare `CCharInstance 0x230` + `CSkinInstance 0x6a8` allocation and resolved path |
| `LoadNewCHR 315f1510` | compare CAL entry count and loaded model pointer |
| `LoadFromContents 315f1260/315f1283` + core `315ed8a0` | compare skeleton bones, mesh, physical proxies, facial model; **blocked by failed decompile** |
| `LoadCharacterDefinition 315d5010` | compare CDF File/Material and model pointer |
| CAnimationSet lookup `3160cc50` | compare name -> CAF header for a known animation |

The port can proceed with an `aa-cry` CHR parser while the failed core is recovered. Validate the
first mount/player by checking the same invariants the proxy/sync steps use.

### Step 6 - P2 (combat, death, aim)

1. Ragdoll handover: x2game `391199a0` + state functions; compare `pose+0x150`, `+0x980`, and the
   physics entity type 6.
2. Look IK: `CPlayer::SetIK 39133390`, `CSkeletonPose::SetPostProcessQuat 3152e450`,
   `CFacialModel` look solver `315b37c0`; compare `pose+0x124` records and final head/spine bones.
3. Hit chain: expose `model+0xac` ids; no engine animation to port.
4. Foot IK/ground alignment: `AnimationPoseModifier_GroundAlignment`, `FeetPoseStore/Restore`
   (E2 §8.4 has the cvars; solvers are inside `SkeletonPostProcess`).

### Step 7 - P5 (facial and creation)

1. FXL library load: `31593f20/315940b0/315ec600/315ec9f0`; compare effectors.
2. FSQ load: `315c0ba0/315c2c50/315c48f0/315c5240`, `3157bb00`; compare sequence channels/keys.
3. Instance: `31598f10/31598330/315984a0/315985d0/31597df0/31597f80`; compare layer slots and
   context effect weights (`+0x10` stride 0x30).
4. Apply: `3159aa40`, morph path `31502200` (CPU weights only), eyes `315b32d0/315b3ba0`.
5. CAL/FXL/FSQ tests from Step 0 become the golden data.

### Step 8 - P3/P4 and skip

* CGA instances (`CreateCGAInstance 315cfd00`) for animated doodads.
* Camera/effect attachments (`CA_CAMERA`, attachment type 5).
* Rope aux physics (`BuildAuxPhysicsData`).
* Cloth solver: **do not port** until a live test contradicts §4.6.

### Dependency graph (one line)

```
parsers -> bone map -> math leaves -> proxy build -> sync -> attachments/skin -> loader -> P2 -> P5
```

---

## 8. Numeric parameters and defaults

All values are from the 1.2 binaries' `.data`/immediates or the shipped data; DB values are the
server SQLite copy.

### 8.1 Pose and physics

| value | number | source |
|---|---|---|
| skeleton bone stride | 0x120 (288 B) | `3161ce60`, `3153b5b0` |
| pose bone stride | 0x1c (7 floats) | E2, `31538880/315391e0` |
| physical part stride | 0x40 (64 B) | `315362a0`, `3153b5b0`, `315391e0` |
| post-process quat record stride | 0x20 (32 B) | `3152e450` |
| attachment object / compiled record stride | 0xf0 (240 B) | `31519670`, `3150f340` |
| skin matrix stride | 0x40 (64 B) | `31525f40`, `3150f340` |
| skinning task slot stride / count | 0x9c / 3 | `3151a7d0`, `char+0x4c8` |
| physical entity type | 6 (`PE_ARTICULATED`) | `315362a0`, `391199a0` |
| physical flags | `0xffbfffff`, `0x80000000` | `315362a0`, `391199a0` |
| scale snap tolerance | 0.01 | `315362a0` |
| proxy epsilon / FLT_EPSILON | 1e-6 / 1.1920929e-7 | `3153b5b0` |
| proxy bounds | ±1e10 | `3153b5b0` |
| proxy box constants | 3.0 / 0.3 / 36.0 / 40.0 | `0x31676c10..0x31676c20` |
| bbox limits | ±9,999,999.0 | `31549f80` (`0x31674d60/64`) |
| root bone limit | logged when too many | `3153b5b0` |
| pose flags | bit0 proxies built, bit2 wants entity, bits4/5 sync state | `315362a0/31543a90/31543950` |
| char flags | bit1 attachment updated, bit2 physics sync | `315253f0`, `31543a90` |

### 8.2 Attachments

| value | number | source |
|---|---|---|
| live attachment / bone-use mask | 256 bits (4×u64) | `3150df50` |
| subskin bones per vertex | 3 | `3150f340`, `315095a0` |
| `ca_MergeAttachmentMeshes` | 1 | cvar registration |
| `ca_DrawFaceAttachments` | 1 | cvar registration |
| `ca_useAttachmentItemEffect` | 1 | cvar registration |
| `ca_UseMorph` | 1 | cvar registration |
| `ca_UseDecals` | 0 | cvar registration |
| morph weight cutoff | 1e-4 | `3159aa40` |

### 8.3 Facial

| value | number | source |
|---|---|---|
| `ca_UseFacialAnimation` | 1 | cvar registration `0x316822b8` |
| `ca_FacialAnimationRadius` | 5.0 | `_DAT_31674f50` |
| `ca_FacialAnimationFramerate` | 20 | immediate `0x14` |
| `ca_FacialSequenceMaxCount` | 20 | immediate `0x14` |
| `ca_stream_facial` | 1 | cvar registration |
| `ca_eyes_procedural` | 1 | cvar registration |
| `ca_lipsync_phoneme_offset` | 20 ms | immediate `0x14` |
| `ca_lipsync_phoneme_crossfade` | 70 ms | immediate `0x46` |
| `ca_lipsync_phoneme_strength` | 1.0 | `0x3f800000` |
| facial layers | 6 (0..5) | `31598f10`, `31598330` |
| effect record stride | 0x30 | `3157a800` |
| sequence record stride | 0x30 | `3157a800` |
| FSQ name buffer max | 0x400 | `315985d0` |
| weight cutoff | 1e-4 | `3159aa40` |
| head bone effector rotations | 50° / 25° / 40° | `pc_expressions.fxl` (`0.87266463/0.43633232/0.69813168`) |
| eyelid bones per eye | 8 | `0x31678760..0x316788d0` |
| FXL bone attachment | `Bip01 Head` | `pc_expressions.fxl` |
| facial sequence extension | `.fsq` | `DAT_31678b8c` |

### 8.4 Cloth cvars (registered, likely inactive)

| cvar | default | note |
|---|---:|---|
| `ca_cloth_vars_reset` | 2 | 1 = load from next char, 2+ = ignore |
| `cloth_max_timestep` | 0 | |
| `cloth_max_safe_step` | 0 | |
| `cloth_stiffness` | 0 | |
| `cloth_thickness` | 0 | |
| `cloth_friction` | 0 | |
| `cloth_stiffness_norm` | 0 | |
| `cloth_stiffness_tang` | 0 | |
| `cloth_damping` | 0 | |
| `cloth_air_resistance` | 0 | |
| `cloth_mass_decay` | 1.0 | only non-zero default |

### 8.5 Data-scale

| value | number | source |
|---|---:|---|
| `.cal` / `.fsq` / `.fxl` / `.joy` files | 989 / 894 / 9 / 4 | `pak12.tsv` |
| face target XML (shipped/probed) | 9 / 12 | `pak12.tsv`, `315cc500` |
| nuian male targets | 81 | `nu_m_targets.xml` |
| FSQ top-level channel groups | 8 | sampled FSQs |
| `npc_question` splines | 47 | sampled FSQ |
| CCharInstance / CSkinInstance allocation | 0x230 / 0x6a8 | `315d3c90` |
| CCharacterModel serialized size | 0x690 | `316078c0` |
| character path buffer | 260 chars | `315d3c90` |

---

## 9. Driver harness recipes (in-process differential checks)

Use the current `aaemu-clientdriver/native/out/aadriver.dll` from `docs/research/port-movement/MAP.md`
§7.2. All CryAnimation calls are `__thiscall` (MSVC x86 `__fastcall` with a dummy EDX). Floats are
plain ABI floats; x87 `float10` returns are `float` at the call boundary.

### 9.1 Live pointers

```
char   = entity->GetCharacter(slot)        (entity vt+0x168; E3)
pose   = read32(char + 0x254)              (CSkeletonPose*)
model  = read32(char + 0x180)              (CCharacterModel*)
skel   = model + 0x60                      (inline CModelSkeleton)
mgr    = read32(char + 0x30)               (CAttachmentManager*)
anim   = read32(pose + 0x9b4)              (CSkeletonAnim*)
```

`CFacialInstance*` has no public getter in the decompile. Find it by scanning the character object
for a pointer whose `+0x44 == char` and whose vtable matches `CFacialInstance::vftable`, or by
breaking on `CFacialInstance::Update 31597f80` and reading ECX during a known emote.

### 9.2 Pure and model-state calls

| target | call | read / compare |
|---|---|---|
| bone map | `call cry 0x3161ce60(skel)` | `read(skel+0x28,0x70)` ids; `read(skel+0xac,8)` hit vector; compare names via `read(bone+4)` |
| bone map without engine call | parse CHR names ourselves | same ids; proves the name mapping |
| matrix leaf | `call cry 0x31544110(dst, src)` | compare 16 floats |
| quota helper | `call cry 0x315460a0(pose, bone, part)` | compare float |
| attachment bone blend | `call cry 0x31508570(mgr, bone, part, matrix, scale)` | compare attachment matrix |
| FXL load | `call cry 0x315ec9f0(faceLibPath, calPath, 1)` | `read(model+0x684,4)` facial model; `read(fac+0x1c,4)` library; effector count `+0x34` |
| CAL load | `call cry 0x315c0ba0(calManager)` after requesting the CAL | animation-set entry count/names |

### 9.3 Physics proxy build (P1, destructive)

`CreateCharacterPhysics 315362a0` destroys the old entity. Use a **disposable NPC/mount**, not the
player, and keep the original entity pointer/params so the live character can be restored or the
process exited afterwards.

```
pose  = read32(char+0x254)
old   = read32(pose+0x148)
call cry 0x315362a0(pose, entity, 0, 0, 0, 0, matrix16)
count = read32(pose+0x190)
parts = read(pose+0x19c, count*0x40)
```

Compare `count` and the per-part record's bone map/mass/transform against the port. Call
`BuildPhysicalEntity` directly only if a fresh `IPhysicalEntity` can be obtained from the live world;
otherwise treat `CreateCharacterPhysics` as the entry point and snapshot the old entity.

`BuildPickingProxy 31535d50` is non-destructive; call it on a live entity and read the geometry ids
returned through the entity `AddGeometry` calls (hook the call count).

### 9.4 Sync (P1)

The sync functions mutate the live pose. Take a full pose snapshot first:

```
bones = read(pose+0x18, boneCount*0x1c)
call cry 0x31543a90(pose, pose+0x18, 0)      # SynchronizeWithPhysics dispatcher
bones2 = read(pose+0x18, boneCount*0x1c)
```

For a ragdoll oracle, use a disposable corpse and compare `Physics_SynchronizeToEntity 315391e0`
output with `aaemu-physics`'s part transforms. For the alive path, compare
`SynchronizeWithPhysicsPost 31538880` attachment matrices (`31508570` inputs) instead of the body pose
(the body pose is animated, not physics-driven).

### 9.5 Attachments (P1)

Non-destructive test on a live equipped character:

```
mgr = read32(char+0x30)
obj = read32(read32(mgr+8))                  # first live attachment object
call cry 0x315078e0(obj, kind, boneNamePtr)   # re-bind to a known bone
read(obj+0xb8,4)  # kind
read(obj+0xc4,4)  # bone id
read(obj+0x0,0x1c) # local transform
call cry 0x3150f340(mgr, poseMatrix, 0)       # update all attachments
```

Bind to an existing bone (`Bip01 Spine`) so the test cannot corrupt the model; verify the warning path
with a deliberately missing bone name on a scratch copy. For subskin, equip a known armor part and
compare `UpdateLocationAttachments` output for kind 1.

`VerifySkeleton 31508830` is safe: call on a live part character and a live master character; compare
the returned boolean and the log string. Rename one bone in a test asset to prove the reject path.

### 9.6 Facial (P5)

Find a live `CFacialInstance*` (9.1), then:

```
seq = call cry 0x315985d0(inst, "npc_question", 0, 1)   # LoadSequence(name, arg, force)
call cry 0x31598330(inst, seq, 0, 0, 0)                 # PlaySequence(seq, layer, flag, loop)
call cry 0x31597f80(inst, 0, 0)                         # Update
ctx = read32(inst+0x50)
eff = read(ctx+0x10, (read32(ctx+0x14)-read32(ctx+0x10)))  # 0x30 records
```

Read each effect's `+0x18` weight and the model's morph/bone outputs; compare with the port after
loading the same FSQ/FXL. `CFacialAnimationContext::Update 3157a800` is safe on an NPC; it only
mutates the facial state.

For CAL/FXL parsing, call `LoadFaceLib 315ec9f0` on a disposable NPC with the same file paths and
compare the effector names/types; no need to touch the live player's face.

### 9.7 Loader (P1)

`CharacterManager::CreateInstance` is a free function:

```
char = call cry 0x315d6820("objects/characters/nuian/male/nu_m_face00.cdf", entity, 0, &out)
```

Compare the returned `CCharInstance`, `char+0x180` model, `model+0x60` skeleton, and proxy/skeleton
counts. Use a disposable entity. `CreateCHRInstance 315d3c90` and `LoadNewCHR 315f1510` are the
alternate entry points when the caller already has a model/entity.

### 9.8 Ragdoll and hit policy (P2)

Call `x2game 0x391199a0` on a disposable actor with the same params as the
live ragdollize event; read `pose+0x150` (aux array) and `pose+0x980` flags. Restore with the
`unragdoll` path `x2game 0x39141e70` and verify the state round-trip. Do not call it on
the player during a capture.

### 9.9 Look IK (P2)

Capture a live `param_2` for `CPlayer::SetIK 39133390` (the structure at `param_2`: enable at
`+0x24`, target dir at `+0x18`; the rest is copied from the live player), then call it on a
disposable player/NPC. Read `pose+0x124` (post-process records) and the final head/spine bones; compare
with the port. The original call also sets the `UsingLookIK` graph variable through the actor's graph
interface, so compare that variable too.

### 9.10 Safety

* Never destroy the live player's physics entity without a restore path.
* Prefer NPCs/mounts and `--shot`/`--exit-after`; close Bevy windows when done.
* Do not kill `archeage.exe`, `aaemu-*.exe`, `cargo` or `clippy-driver` (shared box rules).
* If a call faults, let the SEH-protected trampoline report it; do not patch the process globally.

---

## 10. Unknowns (ranked)

### 10.1 Proven (decompile/bytes/data)

* the proxy build/sync call chain and every entry point in §3;
* pose/part/bone/attachment/record strides (0x1c, 0x40, 0x120, 0xf0, 0x20);
* physical entity type 6, flags `0xffbfffff`/`0x80000000`, scale snap 0.01;
* the bone-name -> CRC proxy match for the nine standard body bones;
* the ragdoll handover is x2game `391199a0` + state functions, not `391462a0`, and not cryanimation;
* no hit-reaction system in cryanimation/cryaction; `Bip01 Hit*` is only collected;
* attachment classes/types/enums and the kind-1 3-bone subskin math;
* the face/bone map (`Bip01*`, `eye_*`, `item_*`, `driver`, `camera`, `Bip01 Hit*`);
* facial object layout, 6 layers, CAL/FXL/FSQ formats and the shipped counts;
* 9 FXL, 894 FSQ, 9 face-target XML, 4 JOY, 989 CAL;
* x2game `facial_expression` is the facial handover; DB facial columns are sequence names;
* cloth has cvars but no solver code/chunk in this build;
* hair is a CHR/CDF attachment, not a simulation.

### 10.2 Inferred / unresolved

1. **`FUN_315ed8a0` (CHR build core, 14,072 B) failed to decompile.** It is the single largest P1
   risk; it builds the model, skeleton, mesh, compiled proxies, morph and facial model. Recover from
   assembly (capstone) or live memory reads.
2. **Compiled physical proxy chunk layout.** `BuildPhysicalEntity` consumes it but the exact
   per-bone/per-part record fields, mass distribution and the CRC table are only partially decoded.
3. **Attachment record semantics.** The live manager vector (`manager+0x08`) and the compiled
   `container+0x658` 0xf0-stride records are distinct; which one equipment/hair uses for kind 1 vs
   `CA_SKIN`, and how the record 0xb8 kind is chosen, needs a live equipment sweep.
4. **Sync semantics.** `DAT_316c9290`/`DAT_316c9388` gates, the part->bone matrix convention in
   `Physics_SynchronizeToEntity`, and the exact meaning of `pose+0x980` bits 4/5.
5. **Facial effector -> morph id mapping.** `model+0x20/+0x30` is a face-bone table; where the
   morph-target id per effector is stored and whether it is name- or index-based is unresolved.
6. **FSQ `SkeletonAnimationEntry` consumer.** The sequence stores it; no static caller starts the
   named body CAF. The game/actor layer must do it; the interface is not decoded.
7. **`facial_expression` event argument order.** The handler parses four tokens and uses only one
   float in the call; how the sequence name/layer/loop reach the instance is unclear.
8. **Eye procedural algorithm.** `ca_eyes_procedural` exists and the eye/lid bones are mapped, but
   blink/saccade/eye-IK maths is not isolated (inside `315b37c0`/`315b3ba0`).
9. **Look IK chain source/blend.** `model+0xf0/+0xf4` look-bone/target arrays and the
   `actor_models.game_*_look_ik_blend_*` weights; exact head/neck/spine mix not decoded.
10. **Ragdoll policy ownership.** x2game sets state/impulse; AAEmu server decides when. Which parts
    of `391199a0`/`39119400` the port should keep vs move server-side is a design decision.
11. **Cloth.** Cvars exist but no solver/chunk; a live cape test is needed to prove they are dead.
12. **Hit reactions.** `Bip01 Hit*` consumer and the intended game-side hit-anim selection are not
    in the engine; the port must implement them in the skill/animation layer.
13. **LOD and foot IK.** `SkeletonPostProcess` contains foot IK/procedural posture but was not
    decoded line by line (E2 §8.4); LOD masks are spread over loader/skin.
14. **CGA path.** `CreateCGAInstance` is only handover-level; CGA model build has the same failed-core
    risk.

---

## 11. Size of the work

### 11.1 What E4 actually covers (measured)

| area | band / definition | f | KB | mapped in full (this map + E2) | P1 subset | remainder |
|---|---|---:|---:|---:|---:|---:|
| pose/physics | `0x31528000-0x3154d400` | 246 | 137.8 | 22 (20 new + 2 E2) / 53.7 | 21 / 51.4 | 224 / 84.1 |
| skin/attachments | `0x31501000-0x3150c000` + attach subset | 91 | 44.1 | 18 (14 new + 4 E2) / 25.7 | 18 / 25.7 | 73 / 18.4 |
| facial | `0x3157a4f0-0x3159e000` + `0x315a4000-0x315c6920` + face maker | 1,002 | 264.1 | 47 (37 facial + 10 face maker) / 43.5 | 0 | 955 / 220.6 |
| loading | `0x315c6920-0x315fd910` | 806 | 213.0 | 24 / 31.1 | 24 / 31.1 | 782 / 181.9 |
| model/animset runtime | `0x315fd910-0x31626526` | 556 | 153.8 | 11 / 9.3 | 11 / 9.3 | 545 / 144.5 |
| **E4 total** | — | **2,701** | **812.8** | **122 / 163.2** | **74 / 117.5** | **2,579 / 649.6** |

`mapped in full` means the function was read and its address/size/callers/offsets/state are in this
map (or E2's key table for the six overlapping functions). The remainder is classified by band and
by replacement strategy, not read line by line.

### 11.2 Reconciliation with the survey

* survey E4-ish port set: facial 237.4 + loading 213.1 + model 153.8 + pose 137.8 + skin 44.1 =
  **786.2 KB**;
* this map's corrected facial boundary adds 22.5 KB (`0x3157a4f0-0x31580000`) and 4.1 KB face maker,
  giving **812.8 KB**;
* the survey's **988.8 KB** additionally includes the animation-asset band
  (`0x31626526-0x3165cde0`, 977 f / 202.6 KB) that E2's playback consumes but does not own as a port;
  812.8 + 202.6 = **1,015.4 KB**.
* E2's 341.6 KB phase-1 union includes 129 facial functions / 22.5 KB only because of coarse address
  bands; E2 did not map them. The E4 key set does not double-count: the six E2 functions it reuses
  (`3154b4b0`, `3152e450`, `3150df50`, `3150f340`, `315253f0`, `31525630`) are explicitly counted
  once in the E4 key set.

### 11.3 What phase 1 needs vs the rest

| scope | functions | KB | note |
|---|---:|---:|---|
| **P1 surface (port or replace)** | **74** | **117.5** | pose/physics 21, skin/attach 18, loading 24, model 11 |
| of which replaced, not ported | 4 | 3.8 | streaming/GC/apply-shared-morph in the P1 key set (see §6.5) |
| **of which ported/reimplemented** | **70** | **113.7** | includes the failed 14 KB loader core; without it ~100 KB of readable code |
| P1 optional (ropes, picking, SPU entity sync, fast paths, CGA) | ~16 | ~12 | can be stubbed for the first playable; includes `31546690` (2.3 KB) |
| P2 (combat/aim/death) | ~30-40 | ~30-55 | estimate; ragdoll handover, look IK, foot IK/ground alignment, recoil, hit-chain ids |
| P5 (facial + creation) | ~1,000 | ~264 | 47 key functions / 43.5 KB mapped; the rest is channel/spline/interpolator/asset code |
| remainder that is mostly replace | 1,400 | 344.8 | loading 782/181.9 + model 545/144.5 + skin 73/18.4 |
| remainder pose (SPU/utility + foot IK) | 224 | 84.1 | split between replace and P2 |
| remainder facial (P5) | 955 | 220.6 | channel/spline/interpolator/asset code |

The strongest statement for planning: **the P1 surface is 74 functions / 117.5 KB**, of which
~3.8 KB is replacement and ~14 KB is currently undecompiled, leaving ~100 KB of readable
port/reimplement work.

**Everything outside phase 1 (the "rest"):** 2,701 - 74 = **2,627 functions / 695.3 KB**. Of that,
48 functions / 45.7 KB are already read in this map (the 47 facial/face-maker functions plus the
optional SPU sync `31546690`); 2,579 functions / 649.6 KB are classified by band but not read line by
line. The rest breaks down as facial 1,002/264.1, loading 782/181.9, model 545/144.5, pose 225/86.4,
skin 73/18.4. The facial block alone is the P5 port; loading/model are mostly replaced by Bevy asset
handling, and the pose rest is SPU/utility plus foot IK (P2).

### 11.4 Phase-1 critical functions (address, size, main unknown)

| function | addr | size | unknown / risk |
|---|---:|---:|---|
| CHR build core | `315ed8a0` | 14,072 | **decompile failed**; proxy/morph/model build |
| `CSkeletonPose::BuildPhysicalEntity` | `3153b5b0` | 8,036 | compiled proxy layout, CRC table, mass |
| `CSkeletonPose::CreateCharacterPhysics` | `315362a0` | 1,660 | entity params, scale |
| `CSkeletonPose::SynchronizeWithPhysics` | `31543a90` | 537 | global gates, flags |
| `CSkeletonPose::SynchronizeWithPhysicsPost` | `31538880` | 2,394 | attachment bone blend |
| `Physics_SynchronizeToEntity` | `315391e0` | 3,920 | part->bone matrix |
| `Physics_SynchronizeToAux` | `315347d0` | 3,156 | aux part layout |
| `SSkeletonPose::UpdateBBox` | `31549f80` | 5,413 | bbox chunk |
| `CModelSkeleton::BuildAuxPhysicsData` | `3161da30` | 2,370 | rope grammar |
| `CSkeletonPose::BuildPickingProxy` | `31535d50` | 511 | picking geometry |
| `CModelSkeleton` bone map | `3161ce60` | 1,284 | name/id map, hit vector |
| `AttachObjectToBone` | `315078e0` | 531 | kind/align semantics |
| `CAttachmentSkin::VerifySkeleton` | `31508830` | 232 | bone-name match |
| `CAttachmentManager::UpdateLocationAttachments` | `3150f340` | 12,725 | compiled record 0xf0 |
| `CAttachmentSkin::SkinningAttSW` | `315095a0` | 3,519 | CPU 3-bone path |
| `CSkinningTransformationsComputationTask::Begin` | `315251e0` | 294 | task fields |
| `CSkinningTransformationsComputationTask::Execute` | `31525f40` | 2,598 | 0x40 skin matrix buffer |
| `CharacterManager::CreateInstance` | `315d6820` | 571 | extension dispatch |
| `CharacterManager::CreateCHRInstance` | `315d3c90` | 1,302 | 0x230/0x6a8 composition |
| `CryCHRLoader::LoadNewCHR` | `315f1510` | 691 | CAL + build handoff |

---

## 12. Files, data and cross-references

### 12.1 Decompiles used

* `D:/aa-re/client12/engine/out/cryanimation.dll.c` / `.funcs.csv` / `.strings.txt`
* `D:/aa-re/client12/engine/out/cryaction.dll.c` / `.strings.txt`
* `D:/aa-re/client12/engine/out/cryentitysystem.dll.c`
* `D:/aa-re/client12/engine2/out/cry3dengine.dll.c` / `.strings.txt` (cloth negative evidence)
* `D:/aa-re/client12/engine3/out/crysystem.dll.c` / `.strings.txt` (checked; no E4 path)
* `C:/AAEMU_DISK/aa-re/client12/out-x2game/x2game.dll.c` / `.funcs.csv` / `.strings.txt` /
  `.strings-xrefs.tsv`
* shipped binaries for constants: `D:/AA1.2/bin32/cryanimation.dll`, `D:/AA1.2/bin32/x2game.dll`

### 12.2 Game data

* `D:/AA1.2/game_pak` via `tools/aa-pak.exe`
* `game/objects/characters/**/*.cal|cdf|chr`
* `game/objects/characters/facial_player.cal`
* `game/objects/characters/*/face/*.cal`, `*_targets.xml`
* `game/animations/**/*.fsq`, `.fxl`, `.joy`
* `D:/aaemu-live/rs-game/Data/compact.sqlite3`: `actor_models`, `sphere_chat_bubbles`,
  `bubble_chats`, `quest_chat_bubbles`, `custom_face_presets`, `total_character_customs`,
  `face_*` maps, `hair_colors`, `equip_pack_*`, `item_body_parts`

### 12.3 Related maps

* [`../port-engine-anim/MAP.md`](../port-engine-anim/MAP.md) - playback, layers, IK, rider seat, key set
* [`../port-mount-anim/MAP.md`](../port-mount-anim/MAP.md) - graph, guards, LMG, rider feed
* [`../port-engine-char/MAP.md`](../port-engine-char/MAP.md) - entity transform graph, CAnimatedCharacter
* [`../port-movement/MAP.md`](../port-movement/MAP.md) - movement/camera and driver conventions
* [`../port-vehicles/MAP.md`](../port-vehicles/MAP.md) - `$IK_Grip`/vehicle seats
* [`../re-mounts.md`](../re-mounts.md), [`../re-mounts-full.md`](../re-mounts-full.md) - mount data
* [`../equipment-attach.md`](../equipment-attach.md) - weapon/shield attachments
* `C:/Users/stsiakal/AAEmu/rust/crates/aaemu-physics` - CryPhysics port (ragdoll solver)

### 12.4 Status

Mapped in full: §3 pose/physics, §4 skin/attachments, §5 facial runtime/data (key functions), §6
loading handovers, §7 port order, §8 numeric parameters, §9 harness, §10 unknowns, §11 sizing.
Not line-by-line: the `315ed8a0` loader core (failed decompile), SPU/job variants, LOD heuristics,
renderer draw/GPU morph paths, the full channel/spline/interpolator tail of the facial block, and the
cloth cvars (no code).

---

## Appendix A: mapped function index

`S` = stateful (mutates an object/global), `P` = pure (outputs depend only on arguments; may read
immutable asset/global tables). All functions are `cryanimation.dll` unless marked `x2`. Addresses
are VAs. "live" = the harness in §9; "scratch" = arguments can be Rust-owned buffers. Offsets are the
main reads/writes, not an exhaustive struct dump.

### A.1 Pose and physics (22)

| VA | B | callers | callees | P/S | reads/writes | harness |
|---|---:|---|---|---|---|---|
| `3152aa00` | 350 | `3154e8a0` | - | S | pose locator/writeable input | live task prepare, read pose |
| `315347d0` | 3,156 | `31543950` | `315460a0`, `3151e700`, `31543e20` | S | `pose+0x148/150/190/19c`, bone 0x1c | live ragdoll/aux |
| `31535430` | 220 | `3154e8a0` | `31547780` | S | task/pose input copy | live task prepare |
| `31535d50` | 511 | `3153b5b0` | render mesh `GetGeometry`, entity `AddGeometry` | S | entity geometry ids 0x1f4+ | live entity, count geometry |
| `315362a0` | 1,660 | virtual | world create, `3152ca70`, `31543e20/3db0`, pose vt[0x84] | S | `pose+0x148/14c/150/190/194/19c/92c/93c/944/948/980`, `char+0x298` | disposable NPC, restore |
| `31538880` | 2,394 | `3154b4b0` | `3151c9e0`, `31544110`, `31508570` | S | `pose+0x150`, `char+0x30`, attachment matrices | live frame, compare |
| `315391e0` | 3,920 | `31543950`, `3153b380`, `3153b430` | `31508570`, `3151e700`, `315460a0`, `31544110`, `31543fd0` | S | entity parts -> pose bones 0x1c; `pose+0x14/148/190/19c` | dead/ragdoll live |
| `3153b5b0` | 8,036 | `315362a0` vt[0x84] | `315460a0`, `31544240`, `315d8eb0`, `31535d50`, `315081f0` | S | bone 0x120, proxy `bone+0x2c`, mass `bone+0x8`, parts `pose+0x19c` | live proxy build |
| `31543a90` | 537 | `3154b4b0` | `31543950`, pose vt | S | `model+0x48`, `char+0x1ec` bit2, `pose+0x148/190/980` | live frame |
| `31543950` | 33 | `31543a90` | `3153a150`, `315373e0`, `315391e0`, `315347d0`, `31542370` | S | `pose+0x44/4c/980`; `DAT_316c9290/9388` | live frame |
| `31549f80` | 5,413 | virtual | `315bdc20`, `315bcec0`, `315be0d0` | S | pose bones 0x1c, model bbox | live pose dt 0 |
| `31508570` | 696 | `31538880`, `315391e0` | - | S | attachment bone matrix/scale | scratch matrix |
| `3151c9e0` | 2,233 | `31538880` | - | S | `char+0x18`, pose/attachment state | live frame |
| `31544110` | 298 | `31538880`, `315391e0` | - | P | 4x4/quat conversion | scratch 4x4 |
| `31542370` | 5,578 | `31543950` | - | S | pose/attachment post sync | live frame |
| `315373e0` | 5,257 | `31543950` | - | S | entity/aux prep, dt `char+0x240` | live frame |
| `3153a150` | 1,340 | `31543950` | - | S | entity type 6 -> pose | live frame |
| `31546690` | 2,361 | `3153b380`, `3153b430` | `315391e0` | S | SPU variant of entity sync | skip / live if SPU off |
| `3154dc70` | 242 | `3154b4b0` | pose callbacks | S | pose callback list | live frame |
| `3161da30` | 2,370 | build core `315ed8a0` | `315d8f40` | S | bones `rope*`, aux list `pose+0x150` | live rope/cape |
| `3154b4b0` | 7,920 | virtual (E2) | §3 | S | final pose, calls sync | live bone dump (E2) |
| `3152e450` | 425 | look IK (E2) | `31518f50` | S | `pose+0x124` records 0x20 | live look target |

### A.2 Skin and attachments (18)

| VA | B | callers | callees | P/S | reads/writes | harness |
|---|---:|---|---|---|---|---|
| `315078e0` | 531 | game/entity attach | pose vt[0x18] | S | `obj+0xb8/c4/d4`, local 0x0..0x1c | live attachment object |
| `31507b00` | 427 | game/entity attach | - | S | same, id already resolved | live |
| `31508830` | 232 | equipment attach | `3161c290` | P/S | slave/master bone vectors `+0x180` | live part/master |
| `31508c50` | 690 | serialize | - | S | object fields 0x0..0x48, class 0x1c | scratch/live |
| `315095a0` | 3,519 | `3150f340` | `31519140` | S | vertex/skin buffers, 3 weights | live equipped part |
| `31508f10` | 129 | `3150df50` | recursive self | P/S | 256-bit mask, bone tree `+0x10/+0x18` | scratch bone tree |
| `31519140` | 486 | `3150f340` | - | S | subskin vertex buffer | live part |
| `31518a90` | 105 | `3150f340` | - | S | attachment destination buffer | live |
| `31518a20` | 103 | `3150f340` | - | S | attachment destination buffer | live |
| `315251e0` | 294 | `3151a7d0` | `3164b640` | S | task `+0x6c/70/74/78/80/84/88` | scratch task |
| `31525310` | 214 | `3151a7d0/1a960` | `3164b4c0` | S | task wait | scratch task |
| `31525f40` | 2,598 | task/job | - | S | pose 0x1c -> skin 0x40; `+0x6c/70/74/78/84/88` | scratch/live compare |
| `31524cf0` | 90 | destruction | - | S | skin release | live teardown |
| `3161ce60` | 1,284 | model build | - | S | bone 0x120, ids `+0x28..0x96`, hit vec `+0xac/b0` | live model |
| `3150df50` | 1,884 | `3154fbf0` (E2) | `31527310`, `31508f10` | S | `mgr+0xa9/ac/b0`, 256-bit mask | live equipment |
| `3150f340` | 12,725 | `315253f0` | `315095a0`, `31518a90/18a20/19140` | S | live `mgr+0x08`, records `+0x658` 0xf0, kind `+0xb8` | live ride/equipment |
| `315253f0` | 570 | skin frame | `3150f340` | S | skin `+0x30`, object vec `+0x38/3c`, `char+0x1ec` | live |
| `31525630` | 485 | skin frame | `3150f340` | S | root fast path | live |

### A.3 Facial and face maker (47)

| VA | B | callers | callees | P/S | reads/writes | harness |
|---|---:|---|---|---|---|---|
| `31598f10` | 668 | instance creation | `3159a540`, `3157a4f0` | S | `+0x0c..0xa4` | live/scratch instance |
| `31597df0` | 387 | `31597f80`, `31598330` | `3157ad80`, `3157b330`, `31579620` | S | `+0x3c`, 6 slots | live facial |
| `31597f80` | 399 | char frame | `3157af90`, `315799a0`, `31599260`, `31597df0` | S | `+0x50` context | live facial |
| `31598330` | 359 | game/facial interface | `31597df0` | S | slots `+0x0c..0x3c` | live NPC |
| `315984a0` | 298 | game/facial interface | `31597df0`, `3157af90` | S | slots | live NPC |
| `315985d0` | 667 | `31598330`/game | `31522f70`, facial system vt | S | `+0x44` master; name 0x400 | live NPC |
| `315989f0` | 268 | detach/teleport | context reset | S | all layers | live NPC |
| `3157a4f0` | 100 | `31598f10` | - | S | `+0x10/50` vectors, `+0x44/48` | scratch context |
| `3157a800` | 1,407 | `31597f80` | `31579300`, `3157a3d0`, `31579880`, `31579c00`, `3157cd70`, `3157e280`, `31579730` | S | effects 0x30; `+0x50/68/6c`; timer | live facial |
| `3157ad80` | 192 | `31597df0` | - | S | stop sequence effects | live |
| `3157b330` | 499 | `31597df0` | `3157af90` | S | start sequence effects | live |
| `31579620` | 14 | `31597df0` | - | P | smoothing set | scratch |
| `315799a0` | 63 | `31597f80` | - | P | playing/finished | scratch |
| `3157af90` | 651 | `31597f80`, `315984a0` | - | S | context time/sequence | live |
| `3157bb00` | 1,105 | `315985d0` | `3157d560`, `3157d940`, CCalManager | S | name + `.fsq`; cache | live file |
| `315827c0` | 2,421 | serialization | `3157fba0`, `3159dbc0` | S | sequence fields | scratch/live |
| `31583180` | 347 | sequence creation | - | S | sequence layout | scratch |
| `315805c0` | 69 | channel creation | - | S | channel object | scratch |
| `31593f20` | 390 | library creation | - | S | effector map | scratch |
| `315940b0` | 347 | library load | - | S | `+0x34` count | live library |
| `315959d0` | 57 | destruction | - | S | release | live teardown |
| `3159b440` | 123 | model build | `3159b0e0` | S | `+0x40` owner | live model |
| `3159aa40` | 1,668 | context apply | `3159c7b0`, `315931d0`, `31599c60`, `3159c570` | S | weights `+0x1c`; `model+0x20/30` | live facial |
| `315b32d0` | 344 | model init | eyeball lookup | S | `model+0x4/8`, `+0x6c..0xe4` | live model |
| `315b37c0` | 992 | virtual | `315b2c50`, `31505780`, `31505d90` | S | `model+0xf0/f4`, `+0xa0..0xc4` | live look target |
| `315b3ba0` | 9,049 | debug draw | renderer debug | S | eye debug; skip | renderer only |
| `3159d8e0` | 537 | phoneme lib | - | S | phoneme map | scratch |
| `3159dbc0` | 2,555 | sequence deserialize | - | S | `Phonemes` node | live file |
| `315c0ba0` | 495 | manager update | - | S | CAL queue | live |
| `315c2c50` | 2,857 | CAL manager | `315c48f0/5240` | S | CAL lines | live file |
| `315c48f0` | 2,374 | CAL parser | tokenizer | S | `#settings` | live file |
| `315c5240` | 5,790 | CAL parser | animation set | S | register entries | live file |
| `315cc500` | 763 | loader init | `315cb280`, `31664f20` | S | 12 target XMLs | live init |
| `3157cd70` | 243 | `3157a800` | - | S | remove effect | scratch/live |
| `3157c410` | 1,323 | context/sequence | - | S | channel eval | scratch spline |
| `31579730` | 325 | `3157a800` | - | S | apply model/pose | live |
| `31516b20` | 113 | debug | - | P/S | debug string | n/a |
| `316641f0` | 1,174 | face maker | parser | S | targets | live/scratch XML |
| `316646d0` | 864 | face maker | parser | S | target apply | live/scratch |
| `31664b40` | 188 | face maker | - | S | target lookup | scratch |
| `31664c00` | 291 | face maker | - | S | target add | scratch |
| `31664f20` | 204 | `315cc500` | - | S | face maker object | scratch |
| `31664ff0` | 410 | face maker | - | S | target validation | scratch |
| `31665190` | 495 | face maker | - | S | asset/target check | scratch |
| `31665390` | 574 | face maker | - | S | failed-load log | scratch |
| `31665750` | 7 | face maker | - | P | getter | scratch |
| `31665760` | 34 | face maker | - | P | getter | scratch |

### A.4 Loading and model/animset (35)

| VA | B | callers | callees | P/S | reads/writes | harness |
|---|---:|---|---|---|---|---|
| `315d3360` | 1,002 | engine frame | `315c0ba0`, `3161ffb0`, `315ca520`, instance list | S | manager `DAT_316c2254`, globals | live tick |
| `315d0ee0` | 1,702 | manager update | stream tasks | S | stream queues | replace |
| `315cda80` | 997 | manager update | erase list | S | model refs | replace |
| `315d6820` | 571 | `ICharacterManager` | `315cfd00/3c90/5010`, `315cfbd0` | S | path, extension, engine globals | disposable create |
| `315d3c90` | 1,302 | create | `315cabe0`, `315cfbd0`, `31529030`, `3151dfd0` | S | `CCharInstance 0x230`, `CSkinInstance 0x6a8` | disposable create |
| `315cfd00` | 876 | create | model cache | S | CGA instance | P3 |
| `315d6080` | 590 | equipment | model cache | S | CDF skin instance | live equip |
| `315cfbd0` | 294 | create paths | cache/hash | S | model pointer | live |
| `315d5010` | 4,195 | `.cdf` | XML, `315cfbd0`, create | S | CDF File/Material | disposable CDF |
| `315cce20` | 383 | loader | morph table | S | shared morph | live |
| `315cec90` | 781 | loader | file system | S | `preloadmorphlist.txt` | replace |
| `315d0450` | 766 | startup | - | S | precache list | replace |
| `315ca330` | 290 | manager | - | S | instance done flags | live tick |
| `315f1510` | 691 | create CHR | CCalManager, `315ec790`, `315f1260` | S | `.cal`, loader | live NPC |
| `315f1260` | 35 | loader | `315ed8a0`, `315f1110`, `316079d0` | S | file list | live |
| `315f1283` | 649 | split body of `315f1260` | core | S | file list | live |
| `315ed8a0` | **14,072** | `315ce770`, `315f1260` | unknown | S | CHR chunks -> model | **DECOMPILE FAILED** |
| `315ec790` | 597 | `315f1510` | - | S | model/entry validation | live |
| `315ec600` | 388 | `315f1260` | facial system | S | FXL path | live file |
| `315ec9f0` | 647 | `315f1260` | facial system | S | face lib path | live file |
| `315f0fb0` | 350 | `315f1260` | animation set | S | CAL animation list | live |
| `315f1110` | 328 | `315f1260` | file system | S | materials/extra | live |
| `316078c0` | 264 | model build | `3160a3a0`, `3161d3e0`, `3160f3c0`, `3159a5a0` | S | `+0x60/5d4/658/684` sections | live model |
| `316079d0` | 112 | loader | `3160acf0` | S | CModelMesh records 0xf0 | live teardown |
| `3160cc50` | 329 | playback | animation set | P/S | name -> CAF header | live |
| `31610390` | 924 | CAL register | file/CAF | S | CAF header | live file |
| `3160f710` | 865 | CAL register | file/LMG | S | LMG | live file |
| `3160db30` | 240 | validation | - | P/S | LMG list | scratch |
| `3160c880` | 715 | validation | - | P/S | locomotion group | scratch |
| `31621c90` | 877 | animation manager | file/TCB | S | TCB clip | live |
| `3161f950` | 1,098 | event query | - | P/S | event instances | live |
| `3161ffb0` | 521 | manager update | - | S | unload policy | replace |
| `316172a0` | 851 | CPU skin | - | P | vertex/bone data | scratch |
| `31617600` | 2,275 | CPU skin query | - | P | single vertex | scratch |
| `31609c70` | 821 | destruction | - | S | release model subobjects | live teardown |
