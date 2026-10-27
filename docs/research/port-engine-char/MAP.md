# Port map: character and entity glue (CryAction CAnimatedCharacter, CryGame actor, CryEntitySystem attachment)

> **Restored 2026-10-27 from `archive/fork-main-2026-10-27`, UNVERIFIED.** The client was restarted from NickMesser's `main`; nothing described here is wired into the game. Addresses, call chains and offsets must be re-checked against the decompile ([R]/[I]/[?]) before use. A 2026-10-27 audit found 151 of the 269 EXACT/TOLERANCE labels in `proof-levels.*` weak or wrong (report: `D:\aaemu-client\handoff\2026-10-27\audit\report-proofs.md`).

E3 of roadmap v2 phase 0. Read-only map of the engine-side links between x2game's movement port
(`docs/research/port-movement/MAP.md`) and the visible character/mount. Every address below is from
the 1.2 decompiles; the published CRYENGINE source was not used, quoted or consulted. Names in
`CClass` form that the binaries themselves print (profiler strings, log strings) are marked [R];
behaviour with no name in the binary is *(inferred)*. **[R]** = read from binary/decompile,
**[I]** = inference, **[?]** = open.

## 0. Sources, conventions, scope

### 0.1 Sources

| source | path |
|---|---|
| CryAction 1.2 | `D:/aa-re/client12/engine/out/cryaction.dll.{c,funcs.csv,strings.txt,exports.txt,imports.txt}` |
| CryEntitySystem 1.2 | `D:/aa-re/client12/engine/out/cryentitysystem.dll.*` |
| CryGame 1.2 | `D:/aa-re/client12/engine/out/crygame.dll.*` |
| CryAnimation 1.2 | `D:/aa-re/client12/engine/out/cryanimation.dll.*` |
| raw PE (vtables, constants) | `D:/aa-re/client12/engine/{cryaction,cryentitysystem,crygame}.dll` |
| x2game hand-over sites | `C:/AAEMU_DISK/aa-re/client12/out-x2game/x2game.dll.c` |
| existing maps | `docs/research/port-movement/MAP.md`, `port-mount-anim/MAP.md`, `port-vehicles/MAP.md`, `port-program.md` |
| physics | `C:/Users/stsiakal/AAEmu/rust/crates/aaemu-physics`, `C:/Users/stsiakal/AAEmu/rust/docs/physics/cryphysics/entities.md` |

Image bases: cryaction `0x304F0000`, cryentitysystem `0x32000000`, crygame `0x39000000`,
x2game `0x38FF0000`. `funcs.csv` gives the function extents used for the sizes below.

**Readable vs runtime data.** cryaction/cryentitysystem/crygame are packed: `.rdata` is readable in
the file but most of `.data` (where the class vtables live) is BSS in the file. Function-pointer
tables that are in `.rdata` were read directly from the PE (see §2.2); `.data` vtables must be
dumped from a live/dumped image (unknown U3).

### 0.2 Phase-1 priorities (marked everywhere they appear)

* **P1** — why the rider does not turn with the horse when strafing with Q/E while moving forward.
* **P2** — how the visible character/mount location is smoothed against the physics location.

Functions that matter for P1/P2 are marked **[P1]** / **[P2]** in §2–§5.

### 0.3 Module split in one line

x2game owns the input, the movement request, the actor's rotation controller and the rider/mount
*bookkeeping*; CryAction owns the animated character (animation location vs entity location, root
motion, render location); CryEntitySystem owns the transform graph (entity → parent → attached
children) and the per-frame event dispatch; CryGame owns the actor wrapper that drives the animated
character's graph state and movement-control mode.

---

## 1. The call chain

### 1.1 Entity frame (CryEntitySystem) — the outer loop

`CEntity::PrePhysicsUpdate` **FUN_32010250** (449 B) and `CEntity::Update` **FUN_3200ff70**
(723 B) are the two per-entity dispatchers:

```
CEntity::PrePhysicsUpdate(dt)                       cryentitysystem 0x32010250
├─ event = {id = 0x1c (ENTITY_EVENT_PREPHYSICSUPDATE), param = dt}
├─ send event to proxies at this+0xb4 (up to 3 inline, count this+0xb8)
├─ if (flags & 0x100000) for each child in this+0xa0 vector:
│     CEntity::PrePhysicsUpdate(child, dt)          recursive, parent first
└─ (CryAction CGameObject forwards the event to extensions, see §1.2)

CEntity::Update(param)                              cryentitysystem 0x3200ff70
├─ sends the 6-dword context to each proxy's vtable+0x10 (proxy Update)
├─ timers (flags field +8 bits 10..13) decrement per call
├─ if (flags & 0x100000) for each child: CEntity::Update(child, same context)
└─ (extensions receive it through the same proxy/event path)
```

`CEntity::Update` does **not** touch transforms; transforms are recomputed by `InvalidateTM`
(§2.3). The child recursion only affects *update order*: a child entity is updated immediately
after its parent, inside the parent's Update/PrePhysics call, before the world proceeds to the next
root entity [R]. This is the engine-side answer to "update order of parent and child".

### 1.2 Extension/event forwarding (CryAction CGameObject)

CryAction's game object receives the entity event and forwards it to each registered extension's
`ProcessEvent`. `CAnimatedCharacter::ProcessEvent` **FUN_30663ac0** handles [R]:

| event id | meaning | action |
|---|---|---|
| `0x1c` | `ENTITY_EVENT_PREPHYSICSUPDATE` | cache `ICharacterInstance*` (`entity->vtable+0x168(0)`), rebuild sub-objects at `AC+0x73c/+0x740`, call `CAnimatedCharacter::PrePhysicsUpdate` **FUN_306634a0** |
| `0x2a` | (event 42) | forwards to `AC vtable+0x9c(param_2[1], 3, 0)` [I: collider-mode request, string `AnimatedCharacter:ColliderMode`] |
| `0` | per-frame update event | full anim/entity location update; sends the render location to the character instance (see §3.4) |

There is no static caller of **FUN_306634a0** outside `ProcessEvent`; there is no static caller of
`CAnimatedCharacter::Update` **FUN_306637e0** or `ProcessEvent` at all inside cryaction: both are
called through the extension vtable (unknown U3 for the exact slot).

### 1.3 Animated-character frame (CryAction) — the two entry points

```
CAnimatedCharacter::PrePhysicsUpdate                  cryaction 0x306634a0 (830 B)  [P1][P2]
├─ FUN_3066bc10 PreAnimationUpdate                   (2432 B)
│  ├─ FUN_30665500 AcquireRequestedBehaviourMovement (780 B)
│  ├─ FUN_30665830 SetAnimGraphSpeedInputs           (2564 B, already ported)
│  ├─ FUN_30668f40 PostProcessingUpdateFootIK        (7988 B)
│  └─ FUN_30673340 CalculateDesiredLocationAndVelocity (4021 B)  [P2]
├─ FUN_30674550 CalculateParamsForCurrentMotions     (667 B)
├─ read entity pos (vtable+0x98) / rot (vtable+0xa0)
├─ FUN_306629f0 (hand entity transform to the anim object)      [P1]
├─ FUN_3066f090 UpdateCurAnimLocation                (4526 B)  [P2]
├─ store entity rot at AC+0x214
├─ FUN_3066c9e0 CalculateAndRequestPhysicalEntityMovement (1402 B) [P2]
├─ FUN_3066cf60 UpdatePhysicalColliderMode            [P2]
└─ FUN_30666470 UpdatePhysicsInertia                 (567 B)

CAnimatedCharacter::Update                            cryaction 0x306637e0 (487 B)
├─ FUN_306666b0 GetCurrentEntityLocation             (1972 B)  [P1][P2]
├─ FUN_3066b2b0 ClampAnimLocation                    (2011 B)  [P2]
├─ FUN_3066ea40 CalculateAnimRenderLocation          (1605 B)  [P2]
└─ if (AC+0x738 != 0):
     AC+0x738->vtable+0x12c(...)
     AC+0x738->vtable+0x168(AC+0x214, &renderQuatT, 0, dist, 0x666)
```

`AC+0x738` is the **ICharacterInstance** (`entity->vtable+0x168(0)`), not a movement controller
[R: `ProcessEvent` caches it there]. The cryanimation vtable (base `0x31574554`-style table read
from `.rdata`, first entry `0x31521110`) pins the slots: `ICharacterInstance+0x14` returns
`[this+0x248]` (ISkeletonAnim), `+0x18` returns `[this+0x254]` (skeleton pose), `+0x34` is
`CCharInstance::StartAnimationProcessing` (`0x3151b220`), `+0x168` forwards its 5 arguments to
`ISkeletonAnim+0xb4`, and `+0x12c` is only a float setter at `ICharacterInstance+0x47c` (not a
per-frame hook). The render location therefore ends in the skeleton-animation object, which is
E2's territory. `AC+0x24` is the animation graph state used for input queries
(`QueryInput("Action")`, `"heightOffset"`, `"DesiredTurnSpeed"`, the `RequestedMoveSpeed*` table at
`0x30778a60`).

### 1.4 CryGame actor wrapper

CryGame's `IActor` keeps the animated character at **actor+0x464** (`param_1[0x119]` in the
decompile) [R, `FUN_390287a0`]. Setup (`FUN_390287a0`, 361 B) does:

```
AC = entity->QueryExtension("AnimatedCharacter")
actor+0x464 = AC
actor->vtable+0x1e0( AC->vtable+0x50() )     // AC+0x50 returns the anim graph state [R]
```

`CPlayer::Update` **FUN_39050170** (4843 B) drives the graph through
`AC+0x94(state, 1)` with states 0/1/5 and reads the entity through `AC+0x50` (graph state). The
actor-side teardown `FUN_3902e...` uses:

| AC slot | role |
|---|---|
| `+0x50` | `IAnimationGraphState*` (queries/sets `"Action"`, `"waterLevel"`) [R] |
| `+0x60` | release/reset call on destroy [R] |
| `+0x6c` / `+0x70` | get/set a 10-dword movement state block [R] |
| `+0x8c` | movement-control mode getter (returns 4/5) [R] |
| `+0x94` | movement-control mode setter `(mode, bool)` [R] |
| `+0xb8` | two-float graph input helper [R] |

`CActor::Update` **FUN_3902eb60** (2148 B) contains the `"%s has no AnimatedCharacter!"` check and
then calls `actor vtable+0x274` (movement/rotation update) and reads the camera DAT_3923282c.

`CPlayer::PrePhysicsUpdate` is a **gap function** in the crygame decompile (funcs.csv jumps from
`0x3905220d` to `0x39052fb0`), found by its profiler string `0x392091bc`: **0x39052210**, ~3488 B.
It is the player-side pre-physics glue and works only through the animated character:

```
CPlayer::PrePhysicsUpdate(this /*actor*/, dt)           crygame 0x39052210
├─ if (this+0x464 == 0) skip / fall back                // animated-character ptr
├─ flags = AC->vtable+0x6c()                            // get 10-dword movement state
│  flags &= 0xffffff38; if (walk) flags |= 0x1000
│  AC->vtable+0x70(flags)                               // set movement state
├─ graph = AC->vtable+0x50()                            // IAnimationGraphState
│  graph->vtable+0x9c(bool)                             // set an input (walk/run/…)
└─ (falls through to the main pre-physics body)
```

The exact graph input set by `+0x9c` and the meaning of the `0x1000` walk bit must be confirmed
live (the bit is the same walk flag as x2game's request, `port-movement` §2.6); the function body
is not in the decompile (unknown U9).

### 1.5 x2game hand-over

The x2game sites and the movement request itself are already mapped in
`port-movement/MAP.md`. The engine-facing links found there are:

* `CPlayerMovement::Process` **0x3a6ab5f3** ends with
  `(**(code **)(**(int **)(actor + 0xc) + 0x98))()`; with the entity vtable mapped in §2.1 this is
  **IEntity::GetPos**, not an "apply movement" call. The actual apply is through the entity
  transform/`SetPosRotScale` + `InvalidateTM` and through the animated character's movement request
  (§3.5).
* `CActor` accesses the physical entity through `entity` APIs; the physical living entity `Action`
  (slot 10) path is mapped in `aaemu-physics`.
* Rider/mount bookkeeping is `X2::Unit::AttachTo` **0x390b3f10** (225 B) → `X2::UnitModel` virtual
  `+0x278` (child, **0x390cbe30**) / `+0x288` (parent, **0x390c0ff0**) via `FUN_390e16b0` (94 B),
  see §4.

---

## 2. CryEntitySystem: the transform graph

### 2.1 CEntity layout used by the glue [R from the constructor **FUN_32013bc0** and slots]

| offset | field |
|---|---|
| `+0x08` | flags (`0x40` = proxy array is heap, `0x100000` = has attached children, `0x80000` = ?) |
| `+0x0c` | entity class / archetype |
| `+0x18` | ? |
| `+0x20`,`+0x24` | entity class + script info |
| `+0x28..0x30` | local position (x,y,z) |
| `+0x34..0x40` | local rotation quaternion |
| `+0x44..0x4c` | scale (x,y,z) |
| `+0x50..0x7c` | world `Matrix34` cache (row-major, translation at +0x5c/+0x6c/+0x7c) |
| `+0x80..0x8c` | local rotation quaternion copy (getter side) |
| `+0x90..0x98` | world position copy (getter side) |
| `+0xa0` | parent-link / children-list node: `{begin,end,cap,?,parent}`; `node+0x10` = parent entity |
| `+0xb4` | proxy array (up to 3 inline if `flags&0x40` clear), `+0xb8` = count |
| `+0xd0`,`+0xd8`,`+0xdc` | archetype params |
| `+0xe0` | byte, from entity-system default |

The `IEntity` vtable is the **primary** one assigned at `CEntity+0` in the constructor
(`&PTR_LAB_320c41d0` in the decompile). Slots verified from the raw `.rdata` table and by
disassembling the stubs:

| slot | function | role | evidence |
|---|---|---|---|
| `+0x3c` | `FUN_32014700` (437 B) | **AttachChild(child, flags)** | `"Trying to attaching Entity %s to itself"`, event 6 |
| `+0x48` | `FUN_3200f6f0` | child count | returns `(node[1]-node[0])>>2` |
| `+0x4c` | `FUN_3200f710` | GetChild(index) | vector index |
| `+0x50` | `0x32016b20` | **GetParent** | `[ecx+0xa0]->+0x10` |
| `+0x54` | `0x32016b40` | GetRoot | loops GetParent |
| `+0x58` | `FUN_32010420` (822 B) | **SetWorldTM** (keep-world: parent⁻¹ × world → local) | reads parent via +0x50 and parent world via +0x60 |
| `+0x5c` | `FUN_32010760` (72 B) | **SetLocalTM** (fast path, then InvalidateTM) | writes +0x28/+0x34 |
| `+0x60` | `0x3200e160` | **GetWorldTM** = `&this[0x50]` | `lea eax,[ecx+0x50]; ret` |
| `+0x64` | `0x3200e170` | GetLocalTM = `&this[0x80]` | `lea eax,[ecx+0x80]; ret` |
| `+0x7c` | `0x32016b70` | GetLocalPos = `&this[0x28]` | `lea eax,[ecx+0x28]; ret` |
| `+0x84` | `0x32016b80` | GetLocalRotation = `&this[0x34]` | `lea eax,[ecx+0x34]; ret` |
| `+0x8c` | `0x32016b90` | GetScale = `&this[0x44]` | `lea eax,[ecx+0x44]; ret` |
| `+0x94` | `FUN_320107b0` (721 B) | **SetPosRotScale** (threshold 1e-4 position / 1e-6 rotation) | writes +0x28/+0x34/+0x44 then InvalidateTM |
| `+0x98` | `0x32016ba0` | **GetPos** = `&this[0x90]` | used by CAnimatedCharacter and x2game |
| `+0xa0` | `0x32016bb0` | **GetRotation (quat)** = `&this[0x80]` | used by CAnimatedCharacter and x2game |
| `+0x108` | `FUN_3200e530` | GetProxy/GetPhysics accessor (returns proxy at +0xb4) | CAnimatedCharacter treats the result as an entity-like object and calls slot +0x24 |
| `+0x168` | `FUN_3200eb40` | **GetCharacter(slot)** → `ICharacterInstance*` | iterates the render proxy; CAnimatedCharacter caches it at +0x738 |
| `+0x1a4` | `0x32012f80` (runtime-sized) | **InvalidateTM(flags)** | profiler string `CEntity::InvalidateTM` at `0x320c43f8` |

`FUN_320148c0` (116 B) is the child-count/detach helper used after `AttachChild` bookkeeping.

### 2.2 CEntity::AttachChild [R] — `FUN_32014700` (437 B)

```
this = parent, child = arg0, flags = arg1
if parent == child: log "Trying to attaching Entity %s to itself"; return
ensure both nodes at entity+0xa0 exist (0x14-byte node, FUN_320146b0)
if (flags & 1) copy child world TM (vtable+0x60) -> keep pose
add child to parent's node vector (FUN_320248d0)
if (child had a parent) child->vtable+0x44(0,0)   // clear old local/parent link
child->node+0x10 = parent
if (flags & 2) parent->flags |= 0x100000; child->flags |= 0x180000; return
if (flags & 1) child->vtable+0x58(saved world TM)  // SetWorldTM (parent⁻¹*child)
else           child->vtable+0x1a4(0x20)          // InvalidateTM(0x20)
event = {id = 6 (ENTITY_EVENT_ATTACH), child id}
parent->vtable+0xb8(event)                        // SendEvent
```

Consequences for the port:

* A child's world TM is **always** `parentWorld × childLocal` (§2.3). Any parent rotation — e.g. the
  mount's body yaw — is inherited by the child, at every InvalidateTM.
* `flags & 2` marks the parent `0x100000`, which is the exact bit `CEntity::Update` /
  `CEntity::PrePhysicsUpdate` test before recursing into the children vector. So a flag-2
  attachment also changes update order (child runs inside the parent's update, not in the world
  list).

### 2.3 CEntity::InvalidateTM [R] — `0x32012f80` (decompiler does not start a function here; read by disassembly)

```
build local Matrix34 at this+0x50 from +0x28(p)/+0x34(q)/+0x44(s)   (call 0x32012d20)
save local quat (+0x34..0x40) and pos (+0x28..0x30) on stack
parent = this[0xa0]->+0x10
if (parent && !(flags & 0x80000)):
    world = parentWorld(+0x50) × local(+0x50)
    +0x50..0x7c = world
for each child in this[0xa0] vector:
    child->vtable+0x1a4(flags | 0x20)          // recursive InvalidateTM
```

The matrix math is a 3x4 row-major multiply (parent rows as scalars against local rows) plus
translation. This is the **only** place attached children get their world transform from the
parent; there is no per-frame per-bone composition in CEntity. (Bone-level parenting is a
character-instance attachment inside CryAnimation, not an entity transform; see §4.3.)

### 2.4 Position getter vs local fields

`GetPos`/`GetRotation` return the *copies* at +0x90/+0x80, which are refreshed by `SetPosRotScale`
and `InvalidateTM`. Game code that reads the entity transform every frame therefore reads the
world position (+0x90) and the local quaternion (+0x80). For a parented entity the quaternion is
still the **local** quaternion; only the matrix at +0x50 is world. This matters for P1: a rider's
`GetRotation` is its local rotation relative to the mount, not its world yaw.

---

## 3. CryAction CAnimatedCharacter

### 3.1 Construction [R]

Factory `FUN_30629f60` allocates **0x940 bytes** and calls the constructor `FUN_30664370`, which
assigns `*this = &PTR_FUN_30746730` (primary IAnimatedCharacter/extension vtable, in `.data`, BSS in
the file) and a secondary table at `this+4 = &PTR_LAB_3074671c`. `PostInit` **FUN_30661a20**
initialises the fields; `Init`/`PostInit` profiler strings are at `0x30746ab0` / `0x3074689c`.

### 3.2 Field map (byte offsets) [R]

| offset | type | meaning |
|---|---|---|
| `+0x0c` | `IEntity*` | the character's entity |
| `+0x24` | `IAnimationGraphState*` | graph state queried for `Action`, `heightOffset`, `DesiredTurnSpeed`, move-speed table |
| `+0x130..0x158` | block | requested movement / parameters (written by `FUN_30662490`) |
| `+0x160` | `IActor*` [I] | game actor looked up from the entity id; `+0xec()` returns a **linked/parent entity id** (used with `IEntitySystem::vtable+0x3c`), `+0xa8()` a mode byte, `+0xc4()` a byte used at setup. If it were the actor's own entity id the parent-delta block would be dead code, so it is the mount/vehicle link [I] |
| `+0x180` | double | frame delta (dt) |
| `+0x190..0x1a8` | QuatT | **animation location**: quat x,y,z,w at +0x190/+0x194/+0x198/+0x19c, position at +0x1a0/+0x1a4/+0x1a8 |
| `+0x1c8..0x1e0` | block | current animation motion/root-motion delta |
| `+0x214..0x220` | quat | current **entity** rotation snapshot |
| `+0x224..0x22c` | vec3 | current **entity** position snapshot |
| `+0x230..0x23c` | quat | previous entity rotation |
| `+0x240..0x248` | vec3 | previous entity position |
| `+0x268` | int | entity id the animation location is bound to |
| `+0x26c..0x2b8` | block | entity-motion compensation: cached position/rotation deltas, `+0x2b0..0x2b8` rotation offset |
| `+0x2c0` | `IEntity*` | cached **parent/linked** entity (see P1) |
| `+0x2c4..0x2d0` | quat | cached parent rotation |
| `+0x2d4..0x2dc` | vec3 | cached parent position |
| `+0x2e0..0x2ec` | quat | parent-relative orientation accumulator |
| `+0x2f0..0x2f8` | vec3 | parent-relative position accumulator |
| `+0x2fc..0x314` | block | zeroed each GetCurrentEntityLocation (per-frame motion) |
| `+0x6d0..0x6fa` | u16[21] | graph input ids (`RequestedMoveSpeedLX` … `0x30778ab8`) |
| `+0x725`,`+0x727`,`+0x728`,`+0x744`,`+0x746` | byte | pause/update gates |
| `+0x734` | int | state snapshot from character instance `vtable+0x8c` |
| `+0x738` | `ICharacterInstance*` | cached character instance |
| `+0x73c` | `*` | character sub-object (`ICharacterInstance+0x14`), animation sink queried at `+0xa8` |
| `+0x740` | `*` | character sub-object (`ICharacterInstance+0x18`), receives `heightOffset` at `+0x44` |
| `+0x744` | byte | "hold/snap" flag used by ProcessEvent |
| `+0x7d0`,`+0x8d4..0x8d6`,`+0x8d8`,`+0x938` | bytes/dword | misc flags; `+0x938` = last time handed to the anim object |

### 3.3 Function table (CryAction) [R]

| function | address | size | callers → callees | role |
|---|---|---|---|---|
| `CAnimatedCharacter::PostInit` | `0x30661a20` | (huge) | factory | defaults; graph input ids; actor/entity lookup |
| `CAnimatedCharacter::~/release` | `0x30661570` | 44 | vtable | releases sub-objects at +4 |
| `CAnimatedCharacter::QueryComplete` | `0x306614c0` | 94 | vtable | stance query callback |
| `CAnimatedCharacter::ProcessEvent` | `0x30663ac0` | 1747 | extension vtable | event 0x1c → PrePhysics; event 0 → location update |
| `CAnimatedCharacter::PrePhysicsUpdate` | `0x306634a0` | 830 | ProcessEvent | per-frame pre-physics pipeline |
| `CAnimatedCharacter::Update` | `0x306637e0` | 487 | extension vtable | render location → character instance |
| `CAnimatedCharacter::AcquireRequestedBehaviourMovement` | `0x30665500` | 780 | PreAnimationUpdate | movement request acquisition |
| `CAnimatedCharacter::SetAnimGraphSpeedInputs` | `0x30665830` | 2564 | PreAnimationUpdate | **already ported** (`port-mount-anim`) |
| `CAnimatedCharacter::UpdatePhysicsInertia` | `0x30666470` | 567 | PrePhysicsUpdate | physics inertia smoothing |
| `CAnimatedCharacter::GetCurrentEntityLocation` | `0x306666b0` | 1972 | Update | entity/parent delta, P1/P2 |
| `FUN_30666e70` | `0x30666e70` | 1414 | ClampAnimLocation | blend two QuatT |
| `FUN_30668c30` | `0x30668c30` | 774 | ClampAnimLocation | angular clamp between entity and anim rotation |
| `CAnimatedCharacter::PostProcessingUpdateFootIK` | `0x30668f40` | 7988 | PreAnimationUpdate | foot IK |
| `FUN_3066ae80` | `0x3066ae80` | 1063 | CalculateAndRequest… | local anim rotation for physics |
| `CAnimatedCharacter::ClampAnimLocation` | `0x3066b2b0` | 2011 | Update, ProcessEvent | blends anim loc to entity loc; skipped with a parent |
| `CAnimatedCharacter::PreAnimationUpdate` | `0x3066bc10` | 2432 | PrePhysicsUpdate | acquisition + speed inputs + IK + desired velocity |
| `CAnimatedCharacter::CalculateAndRequestPhysicalEntityMovement` | `0x3066c9e0` | 1402 | PrePhysicsUpdate | combines anim/entity rotation and requests movement |
| `CAnimatedCharacter::UpdatePhysicalColliderMode` | `0x3066cf60` | 3891 | PrePhysicsUpdate | **not** the movement apply (profiler string `0x30747780`); collider-mode request |
| `CAnimatedCharacter::RequestPhysicalEntityMovement` | `0x307c70b0` (runtime `.data`) | 131? | CalculateAndRequest… | profiler string `0x307473b4` has no xref in decompile → the body is the packed/runtime part; called as `thunk_FUN_307c70b0(&animQuat, &localAnim)` |
| `FUN_30662490` | `0x30662490` | 361 | game/actor interface | **SMovementRequest setter** (see §3.11) |
| `FUN_3066db00` | `0x3066db00` | 3891 | CalculateAnimRenderLocation | procedural leaning |
| `CAnimatedCharacter::CalculateAnimRenderLocation` | `0x3066ea40` | 1605 | Update | final visible QuatT |
| `CAnimatedCharacter::UpdateCurAnimLocation` | `0x3066f090` | 4526 | PrePhysicsUpdate | root motion vs entity delta, P2 |
| `CAnimatedCharacter::MergeMCM` | `0x306713f0` | 1165 | ProcessEvent, FUN_3066ae80 | merge horizontal/vertical MCM |
| `CAnimatedCharacter::UpdateMCMComponent` | `0x306718d0` | 659 | FUN_30671b70 | debug/console path |
| `CAnimatedCharacter::CalculateDesiredLocationAndVelocity` | `0x30673340` | 4021 | PreAnimationUpdate | desired location/velocity from request |
| `CAnimatedCharacter::CalculateParamsForCurrentMotions` | `0x30674550` | 667 | PrePhysicsUpdate | params from current motions |
| `CAnimatedCharacter::ValidateAnimGraphPathNode` | `0x30674a20` | 1063 | debug command `FUN_30600300` | not on the runtime path |
| `FUN_30670540` | `0x30670540` | small | many | quaternion multiply helper (QuatT) |
| `FUN_306706d0` | `0x306706d0` | small | many | quaternion delta/relative helper |

The three "MCM" functions (`MergeMCM`, `UpdateMCMComponent`, `ClampLocations` string
`0x307473eb`) implement the animation motion-controllable-moment blend; the error strings
(`MergeMCM() - Horizontal & Vertical MCM %s not implemented!`) show the horizontal/vertical are
expected to be merged by the game, not by this code.

### 3.4 ProcessEvent event 0 — the per-frame location path [R]

```
oldQuat  = AC+0x214 (cached), oldPos = AC+0x224
if (event.flags & 4) oldQuat = entity->GetRotation()      (vtable+0xa0)
if (event.flags & 2) oldPos  = entity->GetPos()           (vtable+0x98)
if (flags & 8) AC+0x204 = entity->vtable+0x8c()
if (AC+0x744 == 0):
    FUN_306706d0(local, &AC+0x214, &oldQuat)  -> AC+0x2fc..+0x24c (relative transform)
    AC+0x214 = oldQuat; AC+0x224 = oldPos
    if (|oldPos - AC+0x1a0| <= 2.0):            // DAT_30720208 = 2.0
         AC+0x1a0.. = MergeMCM(entityLoc, animLoc, 1)
    else:
         AC+0x1a0.. = entityLoc                 // snap anim location to entity
    ClampAnimLocation()
else:
    AC+0x214 = oldQuat; AC+0x224 = oldPos
    AC+0x1a0.. = oldQuat/oldPos                 // hard snap both
    if (AC+0x738):
        character->vtable+0x168(&AC+0x214, &loc, 0, distance*factor, 0xbeef)
FUN_306649a0()
```

Numeric: threshold `2.0` (`DAT_30720208`), time factor `0.001`
(`DAT_30720214`), `57.29578` deg/rad (`DAT_3072458c`), `1/60` (`DAT_3074682c`), `0.999`
(`DAT_30745e20`). The last factor converts the squared distance to a "speed" argument for the
character instance call [I].

### 3.5 CalculateAndRequestPhysicalEntityMovement [P2] — `FUN_3066c9e0` (1402 B)

```
animQuat = FUN_3066ae80()                    // local anim rotation
q        = FUN_30670540(&out, &AC+0x214, animQuat)   // entity rot * anim rot
normalize(q)
if (debug global DAT_3078d700+0xb4 == 2): draw debug axes through DAT_307a36b4[3]
thunk_FUN_307c70b0(&q, &animQuat)            // tail call; decompiler cannot recover it
```

`thunk_FUN_307c70b0` is at `0x307c70b0`, inside the runtime-created part of cryaction (`.data`
BSS, not in the file), and its decompile is a 3-line SEH stub. The physical request itself is
therefore **not yet resolved (U1)**; the most likely consumer is the movement controller exposed
through the actor/animated-character interface (§3.6), not a literal `Action(action_move)`.

### 3.6 Movement control modes and the visible/physics split [R, P2]

`CAnimatedCharacter::Update` reads the character's movement state and applies the render location:

```
GetCurrentEntityLocation()
if (game mode != 2 && !AC+0x744):
    ClampAnimLocation()                      // only when entity has NO parent
    renderQuatT = CalculateAnimRenderLocation()
    if (AC+0x738):
        AC+0x738->vtable+0x12c(local_14)     // ICharacterInstance+0x47c = float (IK/lean weight)
        AC+0x738->vtable+0x168(&AC+0x214, &renderQuatT, 0, dist, 0x666)
                                             // -> ISkeletonAnim+0xb4(quat, QuatT, 0, dist, flags)
```

So the **visible** pose is the render location computed from the animation location, not the
entity position. The **entity** position is the physics/living-entity result. The two are coupled
only by CPU-side blending/clamping (ClampAnimLocation, MergeMCM, UpdateCurAnimLocation) and by the
parent delta in GetCurrentEntityLocation; there is no render-side interpolation (the render proxy
gets a complete QuatT once per frame). The blend weights are state-dependent: ClampAnimLocation
early-outs for a parented entity and for the "hold" flags, otherwise it slerps the anim rotation
toward the entity rotation; state 2 uses a time-scaled slerp (`dt * k`, `k` in the runtime `.data`,
unknown U2), other states snap.

### 3.7 GetCurrentEntityLocation [P1][P2] — `FUN_306666b0` (1972 B)

```
prev = cur (AC+0x214/+0x224 -> AC+0x230/+0x240)
cur  = entity->GetPos()/GetRotation()          // vtable+0x98 / +0xa0
FUN_306706d0(&AC+0x2e0, &prev, &cur)           // parent-relative accumulator = f(prev,cur)
if (AC+0x160 and own entity->vtable+0x108 and actor->vtable+0xec() -> id):
    parent = entitySystem->GetEntity(id)       // vtable+0x3c
    if (parent && parent->vtable+0x108):
        get status id 2 from own physics (IEntity+0x108 object, slot +0x24)
        if (parent == AC+0x2c0):               // parent unchanged since last frame
            deltaPos = statusPos - AC+0x2d4
            rotate deltaPos by cur quat, scale by 1.0, add to AC+0x2f0
            deltaQuat = f(parentQuat, curQuat), multiply into AC+0x2e0
        AC+0x2c0 = parent
        AC+0x2c4..+0x2d0 = parent->GetRotation()
        AC+0x2d4..+0x2dc = statusPos (or parent pos)
AC+0x2fc..+0x314 = 0
```

This is the piece that keeps an attached character aligned with a moving parent: when the parent
(the mount/vehicle the actor is linked to) rotates or moves, the delta is folded into
`AC+0x2e0/+0x2f0`, and those accumulators are then used by `UpdateCurAnimLocation` /
`CalculateAnimRenderLocation`. **P1 answer (decompile-level):** the rider's visible orientation
does not come from a one-off seat offset; it is re-derived each frame from the **parent entity's
rotation delta** in GetCurrentEntityLocation, and from the entity transform inheritance of
`CEntity::InvalidateTM` if the rider entity is an attached child. A port that parents the rider
with a fixed local yaw, or that only copies the seat position, will not turn with the mount body.

### 3.8 UpdateCurAnimLocation [P2] — `FUN_3066f090` (4526 B)

```
AC+0x1d4 = 1.0; AC+0x1c8..+0x1e0 = 0
if (!AC+0x725):
    if (AC+0x73c):
        rootMotion = AC+0x73c->vtable+0xa8()   // 7 floats: quat + translation
        rotate rootMotion by AC+0x194.. (current anim quat)
        AC+0x1c8..+0x1e0 = rootMotion
    blend with 4 history vectors at AC+0x900/+0x90c/+0x918/+0x924 via FUN_30670320
    animLoc = FUN_30670540(&out, &AC+0x194, &AC+0x1c8)
    if (AC+0x160): id = actor->vtable+0xec(); entity = GetEntity(id)
        read entity pos/rot
        if (id == AC+0x268 && cached deltas != 0):
            compensate the animation root motion by the entity delta
            (quaternion rotate + translate), smooth with cos/sin of angle/2
            AC+0x194..+0x1a8 += compensation
        if (|rotation delta|^2 > 10000.0) reset the offset  // DAT_30721ac0
        AC+0x268 = id; cache entity pos/rot in AC+0x288..+0x2a0
```

Numeric: snap threshold 10000.0 (squared, i.e. 100 m), history blend step `0.1`
(`DAT_30720e2c`), normalisation constants 1.5 / 0.5 (Newton rsqrt), `0.9999` in ClampAnimLocation's
quaternion dot clamp (`DAT_3073b3b0`).

### 3.9 CalculateAnimRenderLocation [P2] — `FUN_3066ea40` (1605 B)

Returns a 4x4 QuatT:

```
base = animLoc(+0x190) composed with entity rot(+0x214) and motion(+0x1a4)
if (state != 3 and !AC+0x744):
    leaning = FUN_3066db00()
    if (AC+0x1cd flag & 2 == 0):
        result = leaning * base; translation += leaning
    else result = base
else result = animLoc
```

State 3 is excluded from procedural leaning (the same `vtable+0xac` state getter used elsewhere).
The exact lean curve is in FUN_3066db00 (3891 B) and is not on the P1/P2 critical path beyond the
composition above.

### 3.10 ClampAnimLocation [P2] — `FUN_3066b2b0` (2011 B)

Early-outs when `entity->GetParent() != 0` (i.e. **never for an attached rider**), when
`AC+0x725` (hold) is set, when a query is in progress, or when `AC+0x744` is set. Otherwise:

* `FUN_30668c30(entityRot, animRot)` computes the angular error.
* state 5 (swim): time-scaled weight from dt (+0x180) and speed params (+0x188, +0x1cc,
  +0xbe); state 2 uses a `dt * k` slerp; all other states set blend = 1.0 (snap). The state
  numbers come from the `vtable+0xb0` getter and are not yet named (U2/U8).
* writes the blended quat/pos back to AC+0x190.. / +0x210 (blend factor).

### 3.11 How requested movement and animation are combined [R]

There are two different request structures:

* **x2game `MovementRequest`** (0x9c bytes) — built by `UpdateMovement` and consumed by the x2game
  actor (`port-movement/MAP.md` §2.6). It never reaches CAnimatedCharacter directly.
* **engine `SMovementRequest`** (11 dwords) — copied by **FUN_30662490** into `AC+0x130..0x158`
  and stamped with the current time at `AC+0x120`. This is the setter that the CryGame actor (or
  the animated-character interface) calls after translating the game request.

`SMovementRequest` fields as read by `AcquireRequestedBehaviourMovement` **FUN_30665500**:

| AC offset | field | use |
|---|---|---|
| `+0x130` | type | switch 0 none, 1 movement, 2/3/4 movement+stance (3 sets `+0x724`), 5 timed movement, 6 rotation (sets `+0x720=2`), 7 rotation (`+0x720=1`) |
| `+0x134/+0x138` | vec2 / direction | copied to `AC+0x710` |
| `+0x13c` | float | copied to `AC+0x718` |
| `+0x140..0x148` | vec3 | copied to `AC+0x700..0x708` |
| `+0x14c` | float | copied to `AC+0x70c` |
| `+0x150` | byte | flag |
| `+0x154` | int | stance id |
| `+0x158` | byte | jump/flag |

`AcquireRequestedBehaviourMovement` expires the request when `AC+0x120 < AC+0x110` (the current
frame time), zeroes the derived block on expiry, and scales the three direction components
(`AC+0x710..0x718`) by the frame dt (`AC+0x180`). A query-in-progress flag
(`FUN_306702d0() == true`) clears the derived movement and forces type 1 / mode 0.

`PreAnimationUpdate` then uses it as:

```
FUN_30665500  AcquireRequestedBehaviourMovement(dt)
FUN_30665360(dt)                                     // request time advance
FUN_30665830(AC+0x700, AC+0x214, dt, 0)              // graph speed inputs, profile 0, entity rot
FUN_30665830(AC+0x2e0, AC+0x230, AC+0x188, 9)        // graph speed inputs, profile 9, parent-relative rot
FUN_30666240(dt)
… graph input writes ("Action", "Desired…" strings at 0x30747118..0x3074716c) …
FUN_30668f40  PostProcessingUpdateFootIK()
FUN_30673340  CalculateDesiredLocationAndVelocity()
```

The **profile-9** call is the second view of the movement: `AC+0x2e0` is the parent-relative
orientation accumulator maintained by `GetCurrentEntityLocation` (§3.7). This is how a mounted
rider's graph receives "how am I turned relative to my mount" separately from the world/entity
rotation fed by profile 0. Together with the state machine (`port-mount-anim`), this is the whole
"animation + requested movement" combination on the engine side.

The request types (0..7) are named only by their indices in the binary; the mapping from game
inputs (idle/walk/run/turn/strafe/jump) to type/mode is **unknown U8** and should be settled by
hooking `FUN_30662490` during a live mounted strafe.

---

## 4. Rider ↔ mount attachment

### 4.1 x2game bookkeeping [R]

`X2::Unit::AttachTo` **0x390b3f10** sets `unit+0x3af0` (parent id) and `unit+0x3af4` (attach
point), then calls `FUN_390e16b0(childModel, parentModel, point, flags)` (94 B):

```
childModel+0x11c = parentModel
childModel+0x12c = point
childModel vtable+0x278(point)        -> FUN_390cbe30
parentModel vtable+0x288(child, point, flags) -> FUN_390c0ff0
```

`FUN_390cbe30` propagates the point to the model's attachment list and to
`*(model+0x1f0)`'s entity (`entity->vtable+0xf8(0)` then `+0x6c(point)`). `FUN_390c0ff0` does the
parent side (gets the actor/model, resets the old entity link, looks up the attachment object and
calls `childModel->vtable+0x130`). Detach is `X2::UnitModel::DetachAll` **FUN_390dcaa0** →
`FUN_390dca30` (own +0x27c/+0x284, parent +0x280/+0x28c, `FUN_390d50e0` list erase).

The actual seat pose in the original is produced on the model side; the rider graph copy
`FUN_390dcc80` (already ported, `anim/rider.rs`) copies only `PseudoSpeed`/`RequestedTurnSpeedLZ`
between the graphs and is **not** the transform link.

`FUN_390e96a0` also computes the **seat local offset** from the attach point before calling the
attachment object [R, `.data` values read from `x2game_unpacked_data.dll`]:

| attach point | offset passed as `local_28` | constant |
|---|---|---|
| 1 | x = -1.0 | `0x3999cc1c` |
| 2 | x = +1.0 | `_DAT_3999c488` = 1.0 |
| 3 | y = -2.0 | `0x399a2eb4` |
| 4 | y = +2.0 | `0x3999cffc` |

Conventions in the same function: `0x3999c47c` = 0.5 and `0x3999c478` = 1.5 (Newton rsqrt
constants), `0x3999c480` = 1e-7 (threshold), `0x3999cc20` = 0.81, `DAT_3999cc10` = 0.0. The point
is also stored at `unitModel+0x12c` and propagated to the game proxy (`vtable+0x6c(point)`), so the
game side knows which seat the model is on.

### 4.2 Engine entity parenting [R]

`CEntity::AttachChild` (§2.2) + `InvalidateTM` (§2.3) is the only entity-level transform
inheritance in the engine: child world = parent world × child local. The attachment retains
`local = parent⁻¹ × childWorld` at attach time (flags & 1), or resets it. The child is updated
inside the parent's Update/PrePhysics when the parent has flag 0x100000 (flags & 2 at attach).

### 4.3 Bone/seat attachments [R, partial]

CryAction's CAnimatedCharacter calls the character instance's attachment/animation sub-objects at
`+0x73c`/`+0x740`; the seat bone itself (`bone_spine_driver` / `bone_spine_passenger`, from
`model_attach_point_strings`) is resolved inside the mount's character instance by x2game's attach
setup (`FUN_390e96a0`, 1011 B: reads the mount bone local matrix, composes the seat offset, calls
`FUN_390c32d0`, then the attachment object's `+0x94/+0x34/+0x48`). CryAnimation's `IAttachment`
update is the layer that makes a bone-attached object follow the bone every frame; it is **not yet
mapped here** (E2 owns CryAnimation playback/attachments; unknown U4).

**CryAnimation rider layer sync [R, P1].** `CSkeletonAnim::ProcessAnimationUpdate`
**FUN_31550610** calls `CSkeletonAnim::SyncRiderAnimation` **FUN_3154dd70** (699 B) whenever a layer
has flag `0x400`. That function walks the rider's animation layers, finds the matching animation
clip (CRC via `FUN_3160cc50`) in the character instance stored at `param_1[0x128]` (the mount/master
character), and copies the layer time, weight and the +0x144 blend block from the mount's layer to
the rider's. So the rider's pose timing follows the mount's clip; the graph-side copy
(`FUN_390dcc80`) and this skeleton-side copy are the two animation couplings. Neither writes the
rider's world transform — orientation still comes from §3.7 / entity parenting.

### 4.3b Vehicle seats

The same attach path is used for wagons and ships (`port-vehicles/MAP.md` §2.3): the wagon has one
seat (attach point 1, `anim_action` 12), ships/cannons add passenger/gunner points, and the client
parents the rider/child by the seat. The engine side is identical — `Unit::AttachTo` sets
`unit+0x3af0/+0x3af4`, the model attach virtuals run, and the child's transform/orientation comes
from the same parent-delta/entity-inheritance mechanisms. No separate engine seat class was found;
ship deck pose is the parent transform (`port-vehicles` U9). Vehicle attach points 0x15/0x16 (sails)
and 0x23 (helm) are doodad attachments handled by the ship model, not entity seats.

### 4.4 Why the rider does not turn on Q/E in our client [P1]

Decompile-level findings that constrain the fix:

1. `CEntity::InvalidateTM` composes `parentWorld × local` for every attached child. If the rider is
   an entity child of the mount, the mount's body yaw propagates to the rider **only if** the port
   recomputes the rider's world from the parent on every mount transform change (i.e. calls the
   equivalent of InvalidateTM after the mount's `SetPosRotScale`), not once at attach.
2. `CAnimatedCharacter::GetCurrentEntityLocation` additionally folds the **parent entity's
   rotation/position delta** into the animation-location accumulators `AC+0x2e0/+0x2f0` (field
   `AC+0x2c0` = cached parent). This is the engine's own way of keeping the visible rider aligned
   with a moving mount even when the entity transform does not inherit it.
3. `ClampAnimLocation` explicitly early-outs when `entity->GetParent() != 0`, i.e. the engine does
   **not** blend the attached rider's anim rotation toward its own entity rotation. A port that
   clamps/slerps the rider's yaw to its own entity yaw while mounted will therefore fight the
   mount's rotation — this matches the reported Q/E strafe bug.
4. The rider graph copy (`FUN_390dcc80`) does not carry orientation; only PseudoSpeed / turn speed.
   Orientation must come from (1) and/or (2), not from the animation graph.
5. The x2game rotation controller (`CPlayerRotation`) runs on the **mount** (the controlled unit is
   the mate, `port-movement/MAP.md` §1.2); the rider's own entity yaw is not the source of truth
   while mounted.

**Most likely cause in our client:** the rider is parented with a fixed local yaw/position, or the
parent transform is applied once at attach; the original re-derives the rider from the mount each
frame. P1 needs a differential capture of the original (rider world yaw vs mount world yaw during a
Q/E strafe) to choose between the entity-parenting path and the CAnimatedCharacter parent-delta
path; both are mapped above. **Unknown U5:** whether the 1.2 rider entity is attached with
`AttachChild` (flag 2) or only tracked by the animation layer. Live breakpoints: `CEntity::AttachChild`
`0x32014700`, `CEntity::InvalidateTM` `0x32012f80`, `GetCurrentEntityLocation` `0x306666b0` during a
mount.

---

## 5. Position smoothing (visible vs physics) [P2] — summary

The original has **three** locations, all on the CPU:

| # | location | source | updated by |
|---|---|---|---|
| 1 | physics/entity | CryPhysics living entity → CEntity SetPosRotScale | physics step + x2game movement request |
| 2 | animation | root motion from the anim graph + entity delta compensation | `UpdateCurAnimLocation` `0x3066f090` |
| 3 | render | animation location + leaning, composed with entity rotation | `CalculateAnimRenderLocation` `0x3066ea40` |

Each frame: `CEntity::Update` → extension event 0 → `ProcessEvent` folds entity deltas into the
animation location (snap if > 2.0 m), `CAnimatedCharacter::PrePhysicsUpdate` recomputes the
desired velocity/movement and the animation location, and `CAnimatedCharacter::Update` sends the
render QuatT to the character instance with flag `0x666`. There is **no render-side interpolation**:
smoothing is the anim-location clamp/blend, whose weight is state-dependent and whose constants
partly live in runtime `.data` (unknown U2). For a parented rider `ClampAnimLocation` is skipped,
so the parent-delta path (§3.7) is the smoothing/alignment mechanism.

Practical port rules:

* Keep physics position and visible position separate; never set the visible mesh from the
  physics transform directly.
* Drive the visible position from root motion plus the entity delta compensation; snap when the
  entity moves more than 2.0 m in one frame.
* While mounted, do not clamp the rider's yaw to its own entity yaw; apply the mount's rotation
  delta instead.

---

## 6. Port order (testable leaves first)

Phase A — pure math (no world state):

1. `FUN_30670540` quaternion multiply and `FUN_306706d0` relative transform (small, pure).
2. `CEntity::InvalidateTM` matrix composition `0x32012f80` as a pure `world = parent * local`
   function plus a transform-tree walk (test: two entities, known transforms).
3. `FUN_30668c30` angular clamp, `FUN_30666e70` QuatT blend (pure).

Phase B — entity graph (stateful, testable in isolation):

4. `CEntity::AttachChild` `0x32014700`, child vector, flags 1/2 and the 0x100000 recursion bit.
5. `CEntity::SetPosRotScale` `0x320107b0` / `SetWorldTM` `0x32010420` / `SetLocalTM` `0x32010760`
   (threshold 1e-4 / 1e-6, keep-world composition).
6. `CEntity::Update` `0x3200ff70` and `PrePhysicsUpdate` `0x32010250` order (parent → children).

Phase C — animated character (needs a character/anim graph stub):

7. `GetCurrentEntityLocation` `0x306666b0` (P1/P2).
8. `ClampAnimLocation` `0x3066b2b0` + `MergeMCM` `0x306713f0` (P2).
9. `UpdateCurAnimLocation` `0x3066f090` (P2).
10. `CalculateAnimRenderLocation` `0x3066ea40` (P2).
11. `ProcessEvent` `0x30663ac0` and the two frame entries (`0x306634a0`, `0x306637e0`).
12. `CalculateAndRequestPhysicalEntityMovement` `0x3066c9e0` once U1 is settled.

Phase D — integration:

13. CryGame actor glue: `FUN_390287a0`, `CPlayer::Update` `0x39050170`, `CActor::Update`
    `0x3902eb60` (graph-state slots +0x50/+0x94/+0x6c/+0x70).
14. Rider attach/detach: x2game `0x390b3f10`, `0x390e16b0`, `0x390cbe30`, `0x390c0ff0`,
    `0x390dcaa0`; decide U5 with a live capture.

### 6.1 Callers/callees and purity (phase-1 functions)

| function | callers | callees | pure/stateful |
|---|---|---|---|
| `CEntity::Update` `0x3200ff70` | `CEntitySystem::Update` [I] `0x3201f7f0` | `0x3200e180`, profiler block `0x320164a0`; recursive self | stateful (timers, event dispatch) |
| `CEntity::PrePhysicsUpdate` `0x32010250` | `CEntitySystem::PrePhysicsUpdate` `0x3201c890` | profiler block; recursive self | stateful |
| `CEntity::AttachChild` `0x32014700` | `IEntity` vtable `+0x3c` (script `AttachChild`); no static caller | `0x320146b0` node alloc, `0x320248d0` list add, `0x32015ac0` log, `entity+0xb8` event | stateful |
| `CEntity::InvalidateTM` `0x32012f80` | own vtable `+0x1a4` (from SetPosRotScale, AttachChild) | `0x32012d20` matrix build; recursive child `+0x1a4` | stateful (writes world TM) |
| `CEntity::SetPosRotScale` `0x320107b0` | vtable `+0x94`; helper `0x32016cc0` (`+0x90`); no static caller | profiler block; own `+0x1a4` | stateful |
| `CEntity::SetWorldTM` `0x32010420` | vtable `+0x58` | parent `+0x50`/`+0x60`, `0x32015b30` inverse, own `+0x5c` | stateful |
| `CEntity::SetLocalTM` `0x32010760` | vtable `+0x5c` | `0x32004cd0`, own `+0x94` | stateful |
| `CEntity::GetCharacter` `0x3200eb40` | vtable `+0x168` | `0x3207aef0` (render proxy) | pure given entity |
| `CAnimatedCharacter::PrePhysicsUpdate` `0x306634a0` | `ProcessEvent` `0x30663ac0` | `0x3066bc10`, `0x306629f0`, `0x3066f090`, `0x3066c9e0`, `0x3066cf60`, `0x30666470`, `0x30674550`, `0x30671b70` | stateful |
| `CAnimatedCharacter::Update` `0x306637e0` | extension vtable (no static caller) | `0x306666b0`, `0x3066b2b0`, `0x3066ea40`, `0x30671e70`, `0x30671cc0` | stateful |
| `CAnimatedCharacter::ProcessEvent` `0x30663ac0` | extension event dispatch | `0x306634a0`, `0x3066b2b0`, `0x306713f0`, `0x30670540`, `0x306706d0` | stateful |
| `GetCurrentEntityLocation` `0x306666b0` | `Update` | `0x30661640`, `0x306706d0`, `0x306fd1ec` | stateful |
| `ClampAnimLocation` `0x3066b2b0` | `Update`, `ProcessEvent` | `0x30668c30`, `0x306702d0`, `0x306fd3ba` | stateful |
| `UpdateCurAnimLocation` `0x3066f090` | `PrePhysicsUpdate` | `0x3066c590`, `0x306702d0`, `0x30670320`, `0x30670540` | stateful |
| `CalculateAnimRenderLocation` `0x3066ea40` | `Update` | `0x3066db00`, `0x306702d0` | stateful (reads AC, writes out) |
| `CalculateAndRequestPhysicalEntityMovement` `0x3066c9e0` | `PrePhysicsUpdate` | `0x3066ae80`, `0x30670540`, `0x307c70b0` (runtime) | stateful |
| `PreAnimationUpdate` `0x3066bc10` | `PrePhysicsUpdate` | `0x30665500`, `0x30665830`, `0x30668f40`, `0x30673340`, `0x30665360`, `0x30666240`, `0x30664ec0`, `0x30660d50` | stateful |
| `AcquireRequestedBehaviourMovement` `0x30665500` | `PreAnimationUpdate` | `0x306702d0` | stateful (reads request block) |
| `FUN_30662490` request setter | game/actor interface (no static caller in cryaction) | `0x30660e00` | stateful |
| `CPlayer::PrePhysicsUpdate` `0x39052210` | vtable/extension (gap) | AC `+0x50/+0x6c/+0x70` | stateful |
| `CPlayer::Update` `0x39050170` | game object update | profiler, AC `+0x94/+0x50` | stateful |

### 6.2 Calling the original in-process (driver harness)

Conventions: MSVC x86 `__thiscall` = `__fastcall` with a dummy EDX; `float10` returns are plain
`float`; `QuatT`/`Matrix34` are 16/12 floats in a caller-owned buffer.

| target | how to call |
|---|---|
| pure helpers `0x30670540`, `0x306706d0`, `0x30668c30`, `0x30666e70` | allocate a struct in the driver, call with pointers, compare floats |
| `CEntity::AttachChild` `0x32014700` | `entitySystem->GetEntity(id)` for parent/child; call (thiscall parent, child, flags 1/2/0); then read `child+0x50` (world TM), `child+0xa0->+0x10` (parent), `parent+8` flag 0x100000 |
| `CEntity::InvalidateTM` `0x32012f80` | call (thiscall entity, flags); compare `entity+0x50`; repeat with a child attached to test recursion |
| `CEntity::SetPosRotScale` `0x320107b0` / `SetWorldTM` `0x32010420` / `SetLocalTM` `0x32010760` | pass the live entity + a 12/16-float buffer at a chosen address; read +0x28/+0x34/+0x50 after the call |
| `CAnimatedCharacter::*` | live AC = `*(void**)(actor + 0x464)` (actor from `FUN_39360020(playerId)` in x2game); pass ECX=AC; read AC fields by offset from §3.2. For the render-location call the character instance is `AC+0x738` |
| `GetCurrentEntityLocation` `0x306666b0` | call on the live AC while mounted and while on foot; dump `AC+0x214/+0x224` (entity), `AC+0x2c0` (parent), `AC+0x2e0/+0x2f0` (accumulators) each frame |
| `UpdateCurAnimLocation` `0x3066f090` | call on the live AC and diff `AC+0x194..+0x1a8` (anim location) and `AC+0x268` (bound entity id) |
| `CPlayer::PrePhysicsUpdate` `0x39052210` | hook the actor's extension vtable slot, or call with ECX=actor; record the 10-dword block from AC `+0x6c` before/after |
| seat attach | hook `0x390e16b0` and `0x390e96a0` (x2game) and record `childModel+0x11c`/`+0x12c`, the attach point, and the model/entity world transforms |

The hidden client must be driven with a scripted walk → strafe (Q/E) → turn → mount/dismount
sequence; dump at 60 fps: `entity+0x50` (12 floats), `entity+0x28/+0x34`, AC `+0x214/+0x224`,
`+0x2c0`, `+0x2e0/+0x2f0`, `+0x194`, the rider/mount bone matrices, and the camera. That is the
feel-recorder input for P1/P2.

---

## 7. Numeric parameters

| value | number | source |
|---|---|---|
| anim-location snap distance | 2.0 (plain distance) | cryaction `DAT_30720208` `0x30720208` |
| quaternion dot snap (ClampAnimLocation) | 0.9999 | `DAT_3073b3b0` |
| rotation-offset reset | 10000.0 squared (100 m) | `DAT_30721ac0` |
| history blend step | 0.1 | `DAT_30720e2c` |
| deg→rad | 57.29578 | `DAT_3072458c` |
| 1/60 | 0.0166667 | `DAT_3074682c` |
| 0.999 | 0.9990000 | `DAT_30745e20` |
| 0.001 | 0.001 | `DAT_30720214` |
| quaternion normalise helpers | 1.5, 0.5 | `DAT_3072020c`, `DAT_3071fac4` |
| 1.0 | 1.0 | `_DAT_3071f868` |
| SetPosRotScale move threshold | 1e-4 (position), 1e-6 (rotation) | `DAT_320c3ee0` / `DAT_320c3edc`, used in `0x320107b0` |
| rider graph copy | PseudoSpeed, RequestedTurnSpeedLZ | `FUN_390dcc80` (already ported) |

---

## 8. Unknowns

1. **U1 — where `CalculateAndRequestPhysicalEntityMovement` actually requests movement.**
   `thunk_FUN_307c70b0` (`0x307c70b0`) is in the runtime-created `.data` part of cryaction and its
   decompile is a stub. Settle with a live breakpoint on `CLivingEntity::Action` (cryphysics
   `0x35030E90`) and on `0x3066c9e0` during a live walk; record the action id and the `dt`/jump
   fields. This is the last missing link between the x2game request and the physics living entity.
2. **U2 — ClampAnimLocation blend constants.** The time-scaled slerp factors are in BSS
   (`DAT_30747538`, `DAT_30747540` read 0 from the file) or are cvars. Dump them live, or record
   the original's blend factor for states 2/5.
3. **U3 — CAnimatedCharacter/IAnimatedCharacter vtable.** The class vtable is assigned from
   `&PTR_FUN_30746730` in `.data` (BSS in the file). The `.rdata` table at `0x255b1c` was read
   entry by entry and is almost certainly the interface/extension table, but the exact slot of
   `Update`/`ProcessEvent`/`GetAnimationGraphState` must be confirmed with a live vtable dump. The
   crygame calls already pin the semantics of +0x50/+0x6c/+0x70/+0x8c/+0x94/+0xb8.
4. **U4 — CryAnimation bone attachment update.** `IAttachment`/`IAttachmentManager` and the
   `bone_spine_*` seat update live in cryanimation/E2; E2 owns the mapping. E3 only records that
   CAnimatedCharacter hands transforms to `ICharacterInstance` slots +0x12c/+0x168 and that x2game
   sets up the seat attachment object in `FUN_390e96a0`.
5. **U5 — is the rider an entity child of the mount?** Decisive for P1. Live breakpoints:
   `CEntity::AttachChild` `0x32014700` at mount, `CEntity::InvalidateTM` `0x32012f80`, and
   `GetCurrentEntityLocation` `0x306666b0`; record whether the rider entity's parent is the mount
   and whether `AC+0x2c0` tracks the mount.
6. **U6 — `IEntity+0x108` semantics.** It behaves like a physical-entity/proxy accessor in
   CAnimatedCharacter (slot +0x24 receives a status id), but the CEntity primary vtable at +0x108
   is a proxy getter (`FUN_3200e530`). Confirm whether the game passes an adjusted interface
   pointer or whether the returned proxy forwards the status call to its physical entity.
7. **U7 — extension order.** CEntity dispatches to extensions in registration order; the exact
   order of `AnimatedCharacter` vs x2game's actor/movement extension decides whether the input
   reaches the actor before CAnimatedCharacter reads the entity transform. Read from the entity
   archetype / a live hook of `CGameObject::SendEvent`.

---

## 9. Size of the work

Sizes from `funcs.csv` (bytes of decompiled functions; the decompile text is roughly 30–50 bytes
per source line, so KB below is decompile *bytes*, matching the existing maps).

**(a) Phase 1 needs (P1/P2 + the minimal glue):**

| group | functions | bytes |
|---|---|---|
| CEntity graph (attach, InvalidateTM, SetPos/SetWorld/SetLocal, Update, PrePhysics) | 8 | ~3,265 |
| CAnimatedCharacter location (GetCurrentEntityLocation, ClampAnimLocation, MergeMCM, UpdateCurAnimLocation, CalculateAnimRenderLocation, ProcessEvent, PrePhysics, Update, CalculateAndRequest, PreAnimationUpdate, CalculateDesired…) | 12 | ~24,500 |
| helpers (quat mul/delta, angular clamp, QuatT blend) | 4 | ~3,400 |
| CryGame actor glue (`FUN_390287a0`, `CPlayer::Update`, `CActor::Update`) | 3 | ~7,350 |
| x2game rider attach/detach (already partly mapped) | 6 | ~2,600 |
| **total** | **~35** | **~44,000 B (~44 KB)** |

**(b) The rest of E3 (full char/entity glue):**

| group | functions | bytes |
|---|---|---|
| CAnimatedCharacter IK/leaning/params/debug (PostProcessingUpdateFootIK, CalculateParamsForCurrentMotions, CalculateProceduralLeaning, ValidateAnimGraphPathNode, MCM debug, Init/PostInit) | ~10 | ~20,000 |
| CryGame actor/vehicle/animation glue beyond the P1/P2 path (SpawnParticleEffect, stance, stats, CPlayer/CActor setup/teardown) | ~12 | ~15,000 |
| entity system beyond the graph (proxy dispatch, entity events, collider modes) | ~15 | ~12,000 |
| **total** | **~37** | **~47,000 B (~47 KB)** |

Grand total E3: **~72 functions, ~84-88 KB** of decompile. The phase-1 slice is roughly half of it and
is the part that must be wired before the riding feel test.

---

## 10. Status

* Mapped in full: CEntity attach/invalidate/update order; CAnimatedCharacter frame entry points,
  layout, location pipeline, parent-delta mechanism; CryGame actor↔animated-character slots; x2game
  rider attach bookkeeping.
* Still open: U1 (physics request sink), U2 (blend constants), U3 (vtable slots), U4 (CryAnimation
  attachment update, E2), U5 (rider entity parentage), U6 (IEntity+0x108), U7 (extension order).
* Phase-1 critical: **P1** = §3.7 + §4.4 (GetCurrentEntityLocation parent delta / entity
  parenting); **P2** = §3.5–3.10 (root motion vs entity, ClampAnimLocation, CalculateAnimRenderLocation).
