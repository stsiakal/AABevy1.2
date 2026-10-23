# Movement / camera function inventory (1.2 x2game)

Companion to [`MAP.md`](MAP.md). All addresses are x2game VAs at base `0x38FF0000` unless the
module column says otherwise. Sizes are from `funcs.csv`. `C:` = callers, `E:` = callees
(from `calls.tsv`; `jmp`-thunk targets resolved with capstone). Purity: **pure** = outputs are a
function of the listed inputs/struct fields only (safe for synthetic differential calls);
**state** = reads/writes live game state (needs a live object or a captured snapshot).

## A. Bindings, action map, input manager

| addr | size | function / role | C | E | fields / globals | purity |
|---|---|---|---|---|---|---|
| `0x394005f0` | 211 | register the built-in actions in the fixed order (`FUN_394005f0`) | 39400e50, 39400e80, 39401a50 | 393ffc20 | string constants 0x399d5ac0 etc. | state (writes the action map) |
| `0x393f8d20` | 70 | action name -> action index (0x68-entry table at `0x39ac8e68`) | 393f8d70, 393f8db0, 393f8df0, 393ffc20 | `XlCompareString` | table `0x39ac8e68` (dump-verified) | **pure** |
| `0x39406540` | 264 | `X2::GameClient::InputManager::Update`; calls UpdateMovement on `[this+4]` | 39171590 | 39404d50, 39402b30, 39360020, 390b3750 | `this` = {paused byte, inline InputState at +4}, `DAT_3a5bb1d8` (GameClient) | state |
| `0x39404d50` | 5971 | UpdateMovement: whole input -> MovementRequest | 39406540 | 39 calls: 39360020, 390b3750, 392aa250, 39424580, 39403a90, 39402590, 39402f60, 39402690, 39402e00, 39402c50, 394037d0, 394039c0, 39402fe0, 390b66c0, 390b67e0 | `InputState` (§structs), request at `[ebp-0x130]`; flags 0x1000/0x800/0x2000; cvars via `DAT_39aefb98` | state |
| `0x39402590` | 246 | action/axis combiner; turn axes 4/5 passed by pointer, extra flags via ESI/EDI | 39404d50 | 39424580 | live axes 0..3, turn axes 4/5, mode/can-turn/prefer bytes | **pure given the live input map** |
| `0x39402690` | 371 | opposite-axis preference + press timers (axis 0..3 -> `+0x9c..0xa8`) | 39404d50 | 39424580 | `this+0x9c..0xa8`, live game time via `game->vtable+0x2c` | state (small) |
| `0x39402e00` | 349 | turn delta: keyboard or mouse (see MAP §3) | 39404d50 | 3915c700, 394fab30, 3922aed0 | `this+0x38` (mount ramp), `unit+0x1c1`, `unit+0x174`; cvar `0x308`; π `0x399d5ea0`; 1/3 `0x399a1c40`; dt cap `0x399a3040` | state (accumulator) |
| `0x3915c700` | 193 | mouse yaw delta (`rotateyaw`) | 39402e00, 39169860 | 39424580 | axis 8; `0x3999cce8=10`, `0x399b050c=2π`, `0x399b0724=-0.002`, `0x3999c47c=0.5`, dt cap | **pure** (cvar/axis) |
| `0x3915c7d0` | 193 | mouse pitch delta (`rotatepitch`) | 39169860 | 39424580 | axis 9; same constants | **pure** |
| `0x39402b30` | 283 | InputState reset (clears the whole state, called when InputManager is paused) | 39406540 | - | clears +0x0c..+0xc0; +0x78=0xff, +0x50=1.0, +0x34=30.0 (`0x3999f7c0`) | **pure** reset |
| `0x39402c50` | 427 | velocity smoothing/acceleration into `this+0x24` | 39404d50 | - | `this+0x24..0x2c` (velocity), `this+0x34` (accel, default 30.0), `0x3999cf90=0.01`, `0x399c4e20=0.55`, dt cap | state |
| `0x39402f60` | 126 | ladder-axis filter by `unit+0x23c`; unit in EAX, axes/flags in ESI/EDI | 39404d50 | - | kinds {4,8} set the flag and zero the axis; {6,7,10} set the flag only | **pure** |
| `0x39402fe0` | 198 | external move-to velocity | 39404d50 | 390b3850, 390b3750 | `game->vtable[0x204]` (path), +0x14a.. | state |
| `0x39403330` | 107 | "is this the local unit / can turn" | 39403a90, 39404d50 | 39360020, 390b3750 | `DAT_39b02294` (world), game vtable +4 | state |
| `0x394037d0` | 119 | movement type: 1 default, 0/5/6 from state (meaning [?]) | 39404d50 | 390b3750, 39354600 | `this+0x3a78`, `+0x3b00` | state |
| `0x394039c0` | 200 | new-movement / free-look gate | 39404d50 | 39360020, 390b37a0, 390b3750, 39402560, 39402810 | game vtable +0x200/+0x194/+0x8c | state |
| `0x39403a90` | 87 | local-unit guard for the request | 39404d50 | 39403330 | `DAT_39b02294+0x10c` | state |
| `0x392aa250` | 130 | input-mode table lookup (`DAT_39d2ff30[i]`) | 39404d50, 39169860 | - | `0x39d2ff2c`, `0x39d2ff10`, `0x39d2ff30` | **pure** (read) |
| `0x3921ad60` | 6 | constant-0 gate in the dump | 39404d50 | - | returns 0 | **pure** |
| `0x39055fa0` | 37 | UI/menu lock check | 39404d50 | - | `DAT_3a5bb1d8+0x3c` vtable +0x84 | state |
| `0x3931e2f0` | 113 | resolve a game view/object | 39404d50 | - | `+0x480` flag | state |
| `0x390b66c0` | 276 | walk-action test 1 | 39404d50 | - | walk bit 0x1000 | state |
| `0x390b67e0` | 210 | walk-action test 2 | 39404d50 | - | walk bit 0x1000 | state |
| `0x390b37a0` | 36 | `actor+0x3a70/3a71/3a7b` all clear | 394039c0 | - | - | **pure** |

## B. Actor / camera/unit accessors (tiny, used everywhere)

| addr | size | role | C | E | fields | purity |
|---|---|---|---|---|---|---|
| `0x390b3750` | 7 | `actor->GameClient` = `*(actor+0x3be8)` | many | - | actor+0x3be8 | **pure** |
| `0x390b3850` | 20 | view position: `GameClient vtable[0x228]() + 0x10` | 394046f0, 39169860, 3915dd40, 39402fe0 | - | GameClient vtable +0x228 | state (but read-only) |
| `0x390b3870` | 16 | view quaternion: `GameClient vtable[0x228]()` | 394046f0, 39169860 | - | same | state (read-only) |
| `0x390b3ad0` | 16 | controlled actor/character accessor | 390aa730, 3916a260 | - | - | state |
| `0x39402560` | 43 | current "game view" unit | 394039c0 | 390b3750 | - | state |
| `0x39402810` | 75 | Lua `HoldPosition` == "1" | 394039c0, 39404d50 | - | unit vtable +0xd8/+0x4c/+0x80 | state |
| `0x39402880` | 4 | HoldPosition byte of the camera target | 39169860 | - | target+0x40 | **pure** |

## C. Turn / locomotion assets

| addr | size | role | C | E | globals | purity |
|---|---|---|---|---|---|---|
| `0x39112690` | 1615 | direction-speed factor for the 8 input sectors (walk set from flag 0x1000); `__thiscall(Actor*, float* dir)` | 3912b330 | - | movement block `+0x43c..+0x44c`, `+0x454..+0x464`; `0x3999cf90` | **pure** (given actor) |
| `0x3912b330` | 3199 | animated-character movement vector (graph `jumpVec`, `RequestedMoveDir4LH`) | 3912dd40 | 39111d60, 391136c0, 39112690, 390b3750 | actor+0x40/+0x48, request quat | state |
| `0x3911a5a0` | 3917 | parse Lua `gameParams.stance` block into `actor+0x3ec..0x498` | 39133650 | memset, 39001290, `strcpy_s` | stance, maxSpeed, heightCollider/Pivot, size, useCapsule, offsets, view limits | state (writes actor) |
| `0x3911b950` | 624 | copy the DB `actor_models` movement record into `actor+0x3f0..0x494` | 391320d0 | `XlCopyString` | multipliers `+0x438..0x464`, inertia `+0x468/+0x46c`, jump `+0x470`, swim `+0x494` | state |
| `0x3911a2b0` | 467 | `CActor::UpdateStance`; physics action `{id=8}` (awake) via phys vtable +0x28 | 3911bcb0, 3911d970, 3911feb0 | 39115820, 39112560 | `entity = actor+0xc`, `actor+0x21c` stance, phys slot 10 | state |
| `0x39153ff0` | 99 | jump animation (`jumpMP01`/`jumpMP02`/`em_jump_in_place`) | 390ccab0 | - | `this+0xc` graph, `this+4+0x40` flags bit 0x1000 | state |
| `0x390dcc80` | 1011 | copy mount graph `PseudoSpeed`/`RequestedTurnSpeedLZ` to the rider graph | 390dd080 | graph API | mount `Stance` (`swim` -> 0) | state |
| `0x390dd080` | 632 | `X2::ActorUnitModel::Update` | many | 390dcc80 | - | state |

## D. Local per-frame movement

| addr | size | role | C | E | fields | purity |
|---|---|---|---|---|---|---|
| `0x39171590` | 288 | `X2::GameClient::PrePhysicsUpdate`; drives InputManager + peers | 390f5360 | 39406540, 3935f3b0, 393527f0 | `DAT_39aff901` enabled | state |
| `0x39133280` | 267 | `X2::CPlayer::PrePhysicsUpdate` | vtable | 39117a90, 39118650, 39113f60 | `this+0x24` actor, `this+0xc` entity | state |
| `0x39118650` | 1965 | `CActor::UpdateMovementRequest`: read input request + living status, update actor | 39133280 | input thunks, 390862c0, atan2, phys thunks | `this+0xb` input, `this+3` entity, `+0x212..0x216` cached quat, `+0xb4..0xb7` yaw quat, `+0x1a4/+0x1a8` vtable | state |
| `0x39113f60` | 29 | post-request hook (`game_vtable[0x18C]` if `actor+0x38`) | 39127480, 39129800, 39133280 | 390b3750 | actor+0x38 | state |
| `0x39135fe0` | 378 | `X2::CPlayer::ProcessFrameMovement` wrapper | vtable (U7) | 3a6ab5f3, 3a98bb00, 3a9918bb | stack `CPlayerMovement` at `[ebp-0xe4]`, args (actor, dt) | state |
| `0x39135ed0` | 264 | `X2::CPlayer::ProcessFrameMovementRotation` wrapper | vtable (U7) | 3913ded0, 391407b0, 3913dfd0 | stack `CPlayerRotation` | state |
| `0x3a6ab5f3` | 338 | `CPlayerMovement::Process` real body (reached via `jmp 0x3913a620`) | 39135fe0 | entity vtable +0x98 (tail jump) | `CPlayerMovement` layout (`structs.md` §5), actor+0x3ec/0x50/0x2b0/0x2d0/0x960/0x2e0/0x96c/0x9dc/0x284 | state |
| `0x3913ded0` | 252 | `CPlayerRotation` ctor/bind | 39135ed0 | - | `CPlayerRotation` layout (`structs.md` §6) | **pure** ctor |
| `0x3913dfd0` | 980 | bind rotation to actor transforms (quat norm, actor+0x950..) | 39135ed0 | - | actor+0x950/0x954/0x2b0/0x2d0/0x340 | state |
| `0x391407b0` | 1682 | rotation dispatch (normal/roll/ladder/swim) | 39135ed0 | 3913f020, 3913f710, 3913e6a0, 3913e780, 3913e8d0, 3913ff90 | actor+4, +0xa8, +0x171, +0x18b | state |
| `0x3913ff90` | 2067 | `CPlayerRotation::ProcessNormal` | 391407b0 | asinf | quats at `this+0x20..0x2c`, `+0x40..0x4c`, `+0x80..0x88`; `0x3999cf98=deg2rad` | state |
| `0x3913e8d0` | 1867 | `CPlayerRotation::ClampAngles` | 391407b0 | 39093ef0, asinf/atan2 | `this+0x14/+0x1c`, `0x399a3158=0.9` | state |
| `0x3913e780` | 322 | normal roll (no x2game string name) | 391407b0 | 3913e3b0 | - | state |
| `0x3913e6a0` | 221 | `CPlayerRotation::ProcessLadderRotation` | 391407b0 | - | copies source quaternion | state |
| `0x3913f020` | 1768 | rotation path (air/zero-g; `this+0x34>0`, `0x399a2f24=-0.1`) | 391407b0 | atan2/asinf/powf, 3913e3b0 | `DAT_39aefb98+0x1c4/0x1c8/0x1cc/0x1d8` | state |
| `0x3913f710` | 2174 | rotation path (ladder/other; calls 3913e3b0) | 391407b0 | atan2/asinf, 3913e3b0 | constants `0x399ad690..0x399ad698` | state |
| `0x3913e3b0` | 752 | build a quaternion/matrix from euler angles | 3913e780, 3913f020, 3913f710 | - | - | **pure** |
| `0x3912dd40` | 895 | Quadruped movement vector/controller | 39129800 | 3912af10, 3912c770, 3912d860, 3912b280, 3912ce00, 3912b330, 3912b090, 3912bfb0 | controller `+0x1c/+0x20/+0x24`, actor+0x2e0 | state |
| `0x3912ac50` | 703 | apply quadruped movement to actor fields | 39129800 | 39131ea0, 39131f10 | `+0x60..0x7c`, actor+0x2e0/0x96c/0x974/0x9dc | state |
| `0x39129800` | 1927 | `Quadruped::PrePhysicsUpdate` | vtable | 39117a90, 3912e0c0, 3912f600, 3912e1a0, 3912c640, 3912dd40, 3912ac50, 39129170, 39113f60 | actor+0x24/0x3..., controllers | state |
| `0x3912c640` | 297 | quadruped controller setup A | 39129800 | `CryRandomFloat` | actor+0x3ec/0x50/0x118/0x2e0/0x918 | state |
| `0x3912e0c0` | 216 | quadruped controller setup B | 39129800 | - | actor+0x2b0..0x358/0x118 | state |
| `0x3912e1a0` | 723 | apply controller quats to the actor | 39129800 | - | actor+0x2d0..0x358 | state |
| `0x3912f600` | 1792 | quadruped setup C | 39129800 | 3912e480, 3912e540, 3912ebc0 | - | state |
| `0x39129170` | 74 | copy a 0x2c-byte movement struct | 39129800 | - | - | **pure** |
| `0x3912e480` | 180 | copy controller transform | 3912f600 | - | - | **pure** |
| `0x3912e540` | 1653 | quadruped terrain/ground alignment | 3912f600 | - | - | state |
| `0x3912ebc0` | 2616 | quadruped pose/lean | 3912f600 | - | - | state |

## E. Swim and ladder

| addr | size | role | C | E | fields | purity |
|---|---|---|---|---|---|---|
| `0x39120300` | 2072 | `CActor::UpdateSwimStats` (water level, in-water, breath events) | 39133280? | 3912ce00 | actor+0x84/0x1f/0x20 | state |
| `0x3912ce00` | 2648 | swim control -> graph `SwimControlX/Y/Z` | 3912dd40 | - | controller +0x1c.., cvars `0x399a24cc` etc. | state |
| `0x391388f0` | 323 | `Player::GrabOnLadder` | 390c1330 | 39137470 | actor+0x238/0x23c/0x244/0x1db | state |
| `0x39137470` | 4319 | per-frame ladder transform/IK | many | - | ladder entity, actor+0x238..0x280; log `my ladder(%u) is missing` | state |
| `0x39135570` | 1732 | `Player::LeaveLadder` (modes 2-7) | many | - | actor+0x1db/0x1dd | state |
| `0x39132be0` | 1044 | ladder kind -> graph `Vehicle` names + jump animation | 391388f0? | - | actor+0x23c, +0x9a2 | state |

## F. CSMoveUnit send

| addr | size | role | C | E | fields | purity |
|---|---|---|---|---|---|---|
| `0x391716b0` | 324 | `Client Game PostUpdate`; calls the movement pump | 390f5360 | 39406740, 39424410, ... | - | state |
| `0x39406740` | 307 | movement pump: local + 5 mates | 391716b0 | 39406650, 394064b0 | 5 slots, id at `+0x194+0x90*i`, flags at `+0x218+0x90*i` | state |
| `0x39406650` | 228 | accumulate timer, dispatch send, clear auto-run bits | 39406740 | 394064b0, 39360020 | `state+4` timer, `state+0x21` flags, `state+0x2d` mate | state |
| `0x394064b0` | 139 | choose on-foot/mount writer | 39406650, 39406740 | 394046f0, 394045f0, 393d57b0 | game vtable +0x8c/+4 | state |
| `0x394046f0` | 1624 | on-foot CSMoveUnit builder (opcode 0x89) | 394064b0 | 390b3750, 390b3850, 390b3870, 39024610, 39186810, 390862f0 | InputState `+0x44..0x80` last-sent; `0x3999cf90=0.01`, `0x3999cf30=0.001` | state |
| `0x394045f0` | 243 | non-player/vehicle CSMoveUnit builder (opcode 0x89; used for mounts) | 394064b0 | 390b3750, 3ac1058c | unit+0x410, unit+0x3af0 | state |
| `0x39024610` | 270 | copy the 0x128-byte movement block into the packet | 394046f0, 390b2f30, 391e55c0, 39025b10 | 39023f80 | 0x128 bytes | **pure** |
| `0x39186810` | 5 | `jmp 0x3ac1058c` (send; funcs.csv name `thunk_FUN_3ac1058c`) | 394046f0 | serialiser | - | state |
| `0x390a8dc0` | 245 | MovementRequest ctor/reset (0x9C bytes; called at 0x39405fdd) | 39404d50, many | - | `-1` at +0x28/+0x2c/+0x6c/+0x78/+0x84/+0x94, 0.5 at +0x74, 1.0 at +0x5c, 0 at +0x98 | **pure** init |
| `0x390b2f30` | 445 | copy/keep last unit movement + timing | many | 39024610 | `+0x130` last time, `+0x1c0/+0x1c4` deltas, `+0x150..0x174` last block | state |

## G. Camera

| addr | size | role | C | E | fields / cvars | purity |
|---|---|---|---|---|---|---|
| `0x391739e0` | 705 | `Client Game Update` (calls camera first) | engine | 3916ac70, 3942c640, ... | - | state |
| `0x3916ac70` | 16 | camera update guard (`DAT_39afe2b8 != 0`) | 391739e0 | 3916aab0 | camera manager global `0x39afe2b8` | state |
| `0x3916aab0` | 421 | `CameraManager::Update`: dt cap + mode switch | 3916ac70 | 39169860, 391632d0, 3916a260, 39164100, 39166390, 3915e2c0 | mode at `+4`, dt cap `0x399a3040` | state |
| `0x39169860` | 2558 | mode 1 unit/third-person camera | 3916aab0 | 39360020, 390b3750, 390b3870, 3915c700, 3915c7d0, 3915c040, 3915c2b0, 3915d6f0, 39165420, 39161690, 391694c0, `atan2` | camera offsets in MAP §4.2; cvars `39afe28c/290/24c`; target id `+0x134` | state |
| `0x3915c040` | 609 | apply mouse yaw/pitch + free-look clamp | 39169860 | - | `+0x84/+0x88/+0x8C/+0x90`; `0x3999ccf4=5`, `0x3999ccd0=0.3`, `0x399b0508=π`, `0x399aa990=1/180` | state |
| `0x3915c2b0` | 649 | zoom settle (`drawn` -> `wanted`) | 39169860 | 3915b1c0, `_CIpow` | `+0x20/+0x24/+0x34/+0x48`; cvars `39afe1cc/24c/204/208` | state |
| `0x3915d6f0` | 1352 | camera position (inferred: target + pivot + distance) | 39169860 | 3915d370 | `+0x68..0x7c`, `+0x268..0x270` | state |
| `0x3915dd40` | 1405 | final camera transform (inferred) | 39169860, 39166390 | 390b3850, 390b3750, 39111c10 | camera output `+0x3c..0x44` | state |
| `0x39161690` | 3476 | FOV/aspect/projection basis (inferred) | 39169860 | `CryRandomFloat`, `atan2` | `+0x1f8/+0x1fc`, `+0x2a0..0x2c8` | state |
| `0x39165420` | 3937 | `ProcessUnitCamActualDist` (collision) | 39169860 | ray/primitive, 39164b80, 39164600, 3915be40, `powf` | `0x107/0x2000f` masks | state |
| `0x391694c0` | 919 | `ProcessUnitCamFadeOut` | 39169860, 3916a260 | 39360020, 3915d240, 391678e0, 39354560 | unit id, radius | state |
| `0x391632d0` | 3622 | mode 2 free camera | 3916aab0 | 3921aa60, 392aa250, 39424580, 390b3750, 39162d00 | `np_*` input, `fr_*` cvars | state |
| `0x3916a260` | 2123 | mode 3 (unnamed; target camera inferred) | 3916aab0 | 39360020, 39163270, 390b3ad0, 390c0cf0, 3915f440 | `+0x1b8..0x1e4`, `+0x2ac` | state |
| `0x39164100` | 1052 | mode 4 | 3916aab0 | `tanf`, 3915f440, 3908ceb0 | - | state |
| `0x39166390` | 2898 | mode 6 | 3916aab0 | 3921aa60, 39424580, 39360020, 3915dd40 | - | state |
| `0x3915e7a0` | 398 | camera cvar registration | init | - | `camera_max_dist` (+ default `0x3999f928=18.0`), `camera_max_dist_debug`, `uc_set_*`, `cam_blur_amount` | **pure** init |
| `0x3915b1b0` | 12 | read `dt`/time helper | 3915c2b0 | - | - | **pure** |
| `0x3915b1c0` | 174 | zoom curve helper (used by 3915c2b0) | 3915c2b0 | - | - | **pure** |

## H. Physics interface (cryphysics 1.2 x86, base 0x34FF0000)

| addr | size | role | notes |
|---|---|---|---|
| `0x35030E90` | - | `CLivingEntity__Action` (vtable slot 10) | action id 1 = `action_move`, 24 B: +4 vec3 dir, +16 int iJump, +20 float dt cap |
| `0x3502B250` | - | `action_move` validator | rejects non-finite dir, |dir|² > 1e6, dt cap outside [0,2] |
| `0x35030500` | - | `CLivingEntity__GetStatus` | returns pos/vel/velRequested/ground |
| `0x3502F5D0` | - | `CLivingEntity__SetParams` | `pe_params_pos` etc. |
| `0x3502C3F0` | - | living dtor | - |

Port target: `aaemu-physics` already implements this living entity; the x2game side only needs
to hand it the same dir/iJump/dt-cap values (see MAP §5.4, unknown U1).

## I. Remote smoothing (later phase)

| addr | size | role | C |
|---|---|---|---|
| `0x390a3bb0` | 558 | `X2::NetActorMovementController::Update` | vtable |
| `0x390a30e0` | 2759 | `UpdateLivingEntity` (interpolation) | 390a3bb0 |
| `0x390af040` | 1281 | `PrePhysicsUpdate` | vtable |
| `0x391e6350` | 108 | `OnUnitMovements` | network |
| `0x391e55c0` | 263 | parse a movement batch, `too many unit movements..` | 391e6350 |
| `0x391d4e70` | 187 | `OnOneUnitMovement` | network |
| `0x391db9d0` | 140 | `OnGimmickMovement` | network |

Controller fields (from `0x390a3bb0`): `+4` unit pointer, `+0xcd` active, `+0x418/+0x420/+0x428`
previous delta, `+0x430/+0x438/+0x440` target delta, `+0x1b8/+0x1c0/+0x1c4` timing.
