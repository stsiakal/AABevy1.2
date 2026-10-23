# S3 function inventory (vehicles and ships)

Single list of every function in scope. `size` is `funcs.csv` (bytes); `—` = not listed there
(the 8 unlisted leaves are marked). `in`/`out` list callers/callees **limited to addresses that
appear in this inventory** (full lists in `calls.tsv`). `pure` means the result depends only on
arguments and the referenced constant data (no game object, no global mutation) — these are the
first difftest targets. "this" labels name the function's `this` object as used by the decompile
(see `structs.md` for the offset tables).

Addresses are x2game VAs at base `0x38FF0000`.

Legend for `this`: **C** = local vehicle movement controller (0x1a0), **M** = unit-model subobject,
**F** = full unit-model block (M-8), **N** = NetActorMovementController, **A** = actor,
**G** = GameClient, **R** = `vehicle_models` row, **S** = move struct (0x130).

---

## 1. Inventory

### 1.1 Local vehicle movement controller (area A)

| addr | size | name/role | this | in | out | fields / globals | kind | difftest |
|---|---|---|---|---|---|---|---|---|
| 390c1b80 | 273 | ActivateLocalVehicleController(bool) | M | — (vtable 0x399a5bf8+0) | 390abe10 | reads M+0x29c, M+0x2a0; logs the activation | stateful | phase 5: hook on boarding |
| 390abe10 | 373 | controller ctor | C | 390c1b80 | 39098450, 390a8dc0 | C+8 = owner (M-8); C+0x15c..+0x184 init; C+0x188 pose cache | stateful (allocation) | phase 4: compare field init on a synthetic alloc |
| 390acee0 | 85 | controller step | C | 390acee0 vtable +8 | 0x3909c660, 0x390a3de0, 0x390ac300, thunk 0x3a9c0607 | reads owner F+0x2a0 (wheeled), entity desc +0x34+5 | stateful | phase 4: 3 branch dispatches |
| 3909c660 | 1837 | wheeled-vehicle physics drive | C | 390acee0 | 39094100, 39096780, 39097a20, 39098250 | pose cache C+0x188/+0x190/+0x198; warp log over 128 m (0x399a2f7c); action struct | stateful | phase 4: physics action args + pose readback |
| 390a3de0 | 1175 | kinematic integrator | C | 390acee0 | — | C+0x16c/+0x170/+0x174 ang vel, C+0x178/+0x17c/+0x180 lin vel; dt from G[5]->+0x30 | stateful | phase 4: pose after N steps from a fixed state |
| 390ac300 | 29 | CryPhysics generic action | C | 390acee0 | 3a9c33fa | — | stateful | phase 4: assert the thunk call args |
| 390967c0 | 231 | velocity filter (vtable +0x10) | C | — | — | C+0x15c, C+0x174; 0x399a2d5c (0.0873), 0x3999cffc (2) | pure | phase 0/4: 3-vector in/out |
| 390964e0 | 264 | controller reset | C | vtable +4 | — | zeroes C+0x15c..+0x184 | stateful | phase 4: field compare |
| 390abf90 | 137 | controller dtor | C | vtable +0 | — | releases owner refs | stateful | not ported (Rust Drop) |
| 39094330 | — (16) | controller vtable +0xc: pose getter | C | vtable +0xc | — | reads C+0 maybe | pure | phase 4: disassemble + port by behaviour |
| 39094340 | 27 | small leaf (controller/actor helper) | — | — | — | — | pure | phase 0 |

### 1.2 Vehicle/ship unit model: construction and shared update

| addr | size | name/role | this | in | out | fields / globals | kind | difftest |
|---|---|---|---|---|---|---|---|---|
| 390e8330 | 1116 | unit-model base ctor | F | 390e95a0 | 390b2260, 390e28f0, 393f38b0 | sets base vtables; F+0x2a8 = N; creates N | stateful | phase 3 (live model) |
| 390e95a0 | 108 | model factory | — | — | 390e8330 | alloc 0x3a8, returns F+8 | stateful | not ported (Rust `new`) |
| 390e28f0 | 1063 | embedded entity/actor base ctor | F / F+8 (base ctor) | 390e8330 | — | vtable 0x399a52b8 etc. | stateful | phase 3 |
| 390d5f20 | 357 | prefab brush loader | M | 390e8b30, 390f0510 | 390c15e0, 391117b0 | log "loading prefab for unit model. unitName(%s), unitId(%u), path(%s)" | stateful | phase 3 |
| 390e69d0 | 3773 | post-load members/parts | M | 390e8b30, 390f0510, 390eb7d0 | 390cf010, 390d30c0, 390dea00, 3963b390 …, 3910c740 | creates attachments, doodad slots | stateful | phase 3 |
| 390e1aa0 | 1909 | bind helpers/attach points | M | 390eab80 | 390bf0f0, 3963b490 | bone/helper lookup | stateful | phase 3 |
| 390eb7d0 | 714 | `X2::PrefabUnitModel::Update` | M | 390ebaa0, 390ebc50 | 390eb610, 390e69d0 | base update; damage check | stateful | phase 2 |
| 390eb610 | 268 | `PrefabUnitModel::UpdateModelChange` | M | 390eb7d0 | 390e7b20, 3989f8b0 | M+0x1f0 state compare | stateful | phase 2 |
| 390e7b20 | 74 | switch prefab variant | M | 390eb610 | 390e7910 | log "prefab unit model : switching to(%d)" | stateful | phase 2 |
| 390e8b30 | 569 | `ShipUnitModel::Init` | M | vtable slot (ship) | 390d5f20, 390e69d0, 3989f8b0, 397a7810 | sub-type 4; M+0x1f0; vtable +0xd0/+0x244/+0x50/+0xf4 | stateful | phase 3 |
| 390f0510 | 732 | `VehicleUnitModel::Init` | M | vtable slot (vehicle) | 390d5f20, 390e69d0, 390eed80, 3989f8b0 | sub-type 3; M+0x298 = row+0x72; M+0x1f0 | stateful | phase 3 |
| 390ebaa0 | 429 | `VehicleUnitModel::Update` | M | vtable slot 0 | 390eb7d0, 390cffd0, 390d6b10, 390da800, 390dac80, 390d0800, 390d14b0, 390db140, 390c6780, 390c6e40, 390cf180 | order in MAP §2.7 | stateful | phase 3 |
| 390ebc50 | 325 | `ShipUnitModel::Update` | M | vtable slot 0 | 390eb7d0, 390c22c0, 390d1c60, 390c7410, 390d2990, 390c78d0, 390c7bf0, 390c7640, 390db700 | order in MAP §3.4 | stateful | phase 3 |

### 1.3 Vehicle model (area A)

| addr | size | name/role | this | in | out | fields / globals | kind | difftest |
|---|---|---|---|---|---|---|---|---|
| 390cf180 | 3455 | Physicalize (wheeled descriptor) | M | 390ebaa0, 390eab80, 390d1b10, 390d1b50, 390df3c0 | 39098250, 390cf010 | see MAP §2.5; logs "failed to physicalize vehicle : no entity", "AWD…", "wheel radius is invalid" | stateful | phase 1/3: descriptor fields |
| 390e7b70 | 1979 | wheel builder | M | 390eab80 | 390c8e20, 3910ade0, 397a6c00 | bone names `wheel%d`, `wheel_type2%d`; row+0x78/+0x7c; radius ½ bbox | stateful | phase 1/3: wheel list from a synthetic skeleton |
| 390d6b10 | 15584 | `VehicleUnitModel::UpdateWheels` | M | 390ebaa0 | 39007240, 39086370, 3908d0a0, 390bf0f0, 39101300 | M+0x2ac wheel vector; ray flags 0x101/0x8008f; helpers `surspension_front/rear` | stateful | phase 3 (split) |
| 390cffd0 | 2082 | `UpdateCenterSpindle` | M | 390ebaa0 | 390c1cc0, 3922af40 | `Bone_spindle`; M+0x3f8; wheel entries; 0x3999ccf0 dt cap | stateful | phase 3 |
| 390d0800 | 2016 | `UpdateDriver` | M | 390ebaa0 | 390c9f40 | `$IK_Grip_L/R`, M+0x3fc; row+0x00/+0x74; 0x3999edbc (3.0) | stateful | phase 3 |
| 390d14b0 | 1510 | `UpdateGunTurret` | M | 390ebaa0, 390d6a10 | — | M+0x2cc/+0x2d0 aim; row+0x58..+0x6c; `bone_yaw`/`bone_pitch` | stateful | phase 3: bone quats |
| 390da800 | 1137 | steering anim | M | 390ebaa0 | 390ff930 | `fist_ac_steering[_backward]` | stateful | phase 3 |
| 390dac80 | 1205 | push anim | M | 390ebaa0 | 3922b900 | `fist_ac_push_f/_b` | stateful | phase 3 |
| 390db140 | 716 | `UpdatePlank` | M | 390ebaa0 | 3901e180 | `spread`/`fold` | stateful | phase 3 |
| 390c6780 | 1723 | `VehicleUnitModel::UpdateSound` | M | 390ebaa0 | 393f1470, 393f14a0, 393f1bd0, 393f4220 | engine loop | stateful | phase 3 |
| 390c6e40 | 439 | wheel surface effects | M | 390ebaa0 | 390bf210 | M+0x2ac, physics | stateful | phase 3 |
| 390c6080 | 361 | current "control unit" helper | — | 390eab80 | 390fdd50 | — | stateful | phase 3 |
| 390c1cc0 | 123 | spindle pose notify | M | 390cffd0 | — | — | stateful | phase 3 |
| 390d0fe0 | 1223 | wheel/pose init helper | M | 390eab80 | 3922b910 | — | stateful | phase 3 |
| 390d6860 | 424 | wheel/anim init helper | M | 390eab80 | 390ff930 | — | stateful | phase 3 |
| 390e48a0 | 622 | attach doodad to model | M | — | 390e3690, 393b0110 | log "doodad with no entity can't be attached to vehicle/ship(%s)" | stateful | phase 5 |
| 390e4b10 | 406 | attach doodad (variant) | M | — | 390c8db0, 390e3690, 393b0110 | same log | stateful | phase 5 |
| 3910ade0 | 224 | wheel object create | — | 390e7b70 | 3908d0a0, 390fe5c0, 391056a0, 3910a440 | cgf loader | stateful | phase 1 |
| 391047b0 | 497 | fx lookup by name | — | 390e8d70 | 39104630 | `ripple_effect%i` | stateful | phase 3 |
| 390e8d70 | 196 | ripple object init | M | 390eaf40 | 391047b0 | 5 ripple objects | stateful | phase 3 |

### 1.4 Ship model (area B)

| addr | size | name/role | this | in | out | fields / globals | kind | difftest |
|---|---|---|---|---|---|---|---|---|
| 390c22c0 | 579 | wind-area update | M | 390ebc50 | — | weather mgr 0x39ae6c8c, 3D engine +0x370 | stateful | phase 2: no-wind path |
| 390d1c60 | 3225 | `UpdateSail` | M | 390ebc50 | 390fdd50, 39695a70 | M+0x2a4 sail vector, M+0x2d0 flags, +0x348/+0x34c/+0x350; wind 0x39ae6c94 | stateful | phase 3 |
| 390c7410 | 550 | `UpdateRudder` | M | 390ebc50 | 3903bd80 | M+0x35c steer; child at M+0x2bc | stateful | phase 3: child quat |
| 390d2990 | 1031 | `UpdateSteeringWheel` | M | 390ebc50 | 390bf0f0, 390d2900, 39695a70 | attach 0x23; M+0x35c/+0x360/+0x364; 45.0, 7.0 | stateful | phase 3: wheel angle/quantisation |
| 390db700 | 531 | `UpdateThrottleAnim` | M | 390ebc50 | 3901e180 | M+0x358/+0x39c/+0x3a0; flags 0x5082/0x508a | stateful | phase 3: clip/rate choice |
| 390c78d0 | 790 | `UpdateExhaustEffect` | M | 390ebc50 | 390fed20, 39695a70 | M+0x2c0 exhaust, M+0x2f0 bubbles | stateful | phase 3: emitter params |
| 390c7640 | 656 | `UpdateWaterRipple` | M | 390ebc50 | — | M+0x2e0 points (0xc), 3D engine +0x2e0 | stateful | phase 3 |
| 390c7bf0 | 628 | `UpdateSound` | M | 390ebc50 | 393f14a0, 393f1bd0, 393f4220 | water loop, state 5/6 stop | stateful | phase 3 |
| 390e8e40 | 904 | attach-point state sink | M | vtable +0x30 | 390db5e0 | point 0x23 -> +0x35c..+0x374; points 0x15/0x16 -> sail entry | stateful | phase 2: exact floats |
| 390c2510 | 226 | sail slot attach | M | vtable +0x38 | — | entry +0xc doodad, +0x46 `bone_yaw` | stateful | phase 2 |
| 390db920 | 142 | sail slot detach | M | vtable +0x34 | 390db5e0 | clears +0xc | stateful | phase 2 |
| 390db5e0 | 285 | sail array compact | M | 390db920, 390e8e40, 390de480, 390df450 | — | — | stateful | phase 2: array order |
| 390c2150 | 263 | vehicle type-name string | M | vtable +0x58 | 39695a70 | `smallboat`/`soloship`/`coupleduckship`/`boardship` | pure-ish (data) | phase 0/3 |
| 390c2260 | 89 | model stop helper | M | vtable +0x40 | — | resets controller | stateful | phase 3 |
| 390d1b10 | 53 | plank/attach toggle A | M | vtable +0x30? | 390cf180 | — | stateful | phase 3 |
| 390d1b50 | 53 | plank/attach toggle B | M | vtable +0x38? | 390cf180 | — | stateful | phase 3 |
| 390df450 | 301 | exhaust slot add | M | vtable +0x28 | 390db5e0, 390dea70 | — | stateful | phase 3 |
| 390de480 | 50 | exhaust slot remove | M | vtable +0x2c | 390db5e0, 390ddc70 | — | stateful | phase 3 |
| 390df3c0 | 134 | ripple slot add | M | vtable +0x28 (vehicle) | 390cf180, 390dea70 | — | stateful | phase 3 |
| 390de3d0 | 56 | ripple slot remove | M | vtable +0x2c (vehicle) | 390ddc70 | — | stateful | phase 3 |
| 390e3530 | 36 | vtable slot (ship) | M | vtable +0x14 | 390cf010, 390df580 | — | stateful | phase 3 |
| 390eb720 | 160 | vtable slot (ship +0x54) | M | vtable +0x54 | 390eaf40, 390c20e0, 390df580 | — | stateful | phase 3 |
| 390be050 | 52 | vtable slot (ship +0x5c) | M | vtable +0x5c | 3910a950 | — | stateful | phase 3 |
| 390bb090 | 50 | vtable slot (ship/vehicle +0x18) | M | vtable | — | — | stateful | phase 3 |
| 390bb0d0 | 66 | vtable slot (+0x24) | M | vtable | — | — | stateful | phase 3 |
| 390e3050 | — | vtable slot (+0x20) | M | vtable | — | thunk, not in funcs.csv | stateful | phase 3 (disassemble) |
| 390e3070 | — | vtable slot (vehicle +0x34) | M | vtable | — | thunk | stateful | phase 3 |
| 390e3080 | — | vtable slot (+0x44) | M | vtable | — | thunk | stateful | phase 3 |
| 390e87e0 | — | vtable slot (ship +0x4c) | M | vtable | — | thunk | stateful | phase 3 |
| 390e87f0 | — | vtable slot (ship +0x50) | M | vtable | — | thunk | stateful | phase 3 |
| 390cb1c0 | — | vtable slot (+0x3c) | M | vtable | — | thunk | stateful | phase 3 |

### 1.5 Movement apply and interpolation (area D)

| addr | size | name/role | this | in | out | fields / globals | kind | difftest |
|---|---|---|---|---|---|---|---|---|
| 391d4e70 | 187 | OnOneUnitMovement (packet entry) | — | packet table | 390b3750, 39350160, 3961de30 | unit +0x10, move +0x18; `G+4` mode | stateful | phase 4: hook |
| 391e55c0 | 263 | SCUnitMovement list handler | G | 391e6350 | 39024610, 391b9fc0 | G+0x117c queue, G+0x1180 count (cap 0x1000) | stateful | phase 4: queue bytes |
| 391d4d60 | 244 | queue flush | G | 391e27f0 | 390b3750, 39350160, 3961de30 | pops and applies each move | stateful | phase 4 |
| 39024610 | 270 | move-struct copy | S | 39025b10, 390b2f30, 391e55c0, 394046f0 | — | copies 0x130 bytes field-wise | pure | phase 0: byte compare |
| 390b2260 | 275 | NetActorMovementController ctor | N | 390e8330 | 390afe10 | N+4 owner; vtable 0x399a3368 | stateful | phase 4 |
| 390b2420 | 406 | controller interpolate/update | N | vtable +8 | 390b0040, 390be0b0, 390be0d0 | N+0x4c timeout (5000 ms) | stateful | phase 4: timeout behaviour |
| 390b0040 | 1198 | interpolated apply | N | 390b2420 | 390ad150, 390ae4a0, 390be0b0, 390be0d0, 390be0f0, 390be100 | N+0xfc/0x104/0x10a/0x10c/0x1b8/0x1bc; model setters | stateful | phase 4: output pose/fields |
| 390ad150 | 372 | ground align A | N | 390b0040 | 390a0fc0, 390a16b0 | — | stateful | phase 4 |
| 390ae2c0 | 411 | ground align B | N | 390af740 | 390a1050, 390a1430 | — | stateful | phase 4 |
| 390afe10 | 352 | controller reset | N | 390b2260, 390aff70 | 390a0fc0 | clears N+0x130..+0x1bc | stateful | phase 4: field compare |
| 390aff70 | 201 | reset from entity | N | vtable +4 | 390afe10, 3961ddd0 | N+0x150/+0x168 | stateful | phase 4 |
| 390af740 | 1732 | unit movement apply | N | vtable 0x399a3348+0x10 | 39098250, 390ae230, 390ae2c0, 390c1e80, 390c1ff0, 390c70f0, 390db410, 390eed80 | model +0x330; per-wheel actions; wheel cap 18 | stateful | phase 4 |
| 390af650 | 236 | stop/reset apply | N | 390af740 (bypass) | 390af560, 390c1ff0, 390db410 | zeroes wheels/throttle | stateful | phase 4 |
| 390af550 | — (unlisted) | N base dtor/helper | N | 390af650 | — | — | stateful | not ported (Rust Drop) |
| 390eed80 | 6024 | generic pose apply (12 args) | M/N | 390af740, 390f0510 | 39097dc0, 390c70f0, 390eda60 | smooth pos/quat; 0.05; call 0x390eda60 | stateful | phase 4: pose after one call |
| 390c70f0 | 423 | vehicle pose apply | M | 390af740, 390eed80 | 3961ddd0 | writes entity pose | stateful | phase 4 |
| 390b2380 | 146 | N dtor | N | vtable +0 | — | — | stateful | not ported |
| 390b21c0 | 146 | other base ctor/dtor | — | — | — | — | stateful | not ported |

### 1.6 Move packet codecs and senders (areas A–D contract)

| addr | size | name/role | this | in | out | fields / globals | kind | difftest |
|---|---|---|---|---|---|---|---|---|
| 397c0470 | 3410 | move-struct serializer/deserializer (types 0–5) | S | 397c52f0, 397c5e70, 397c5f60 | 3961e2c0, 397ad5a0, 397ad740, 397ad8c0, 397ada60, 397adc00, 397adca0, 397ae020 | layout in `packets.md` | **pure** (archive+struct only) | **phase 0, first** |
| 397c52f0 | 93 | single-move packet shell | — | — | 397c0470 | reads/adds `unitId` | pure | phase 0 |
| 397c5e70 | 233 | move-list packet | — | 391e3660, 391e36f0 | 397c0470 | count +0xc, entries +0x18 stride 0x130 | pure | phase 0 |
| 397c5f60 | 42 | move packet (unitId only) | — | 391cea00, 391cea80 | 397c0470 | — | pure | phase 0 |
| 3961e2c0 | 972 | position codec (9-byte 24-bit) | out | 397c0470 | — | keys `ix0..iz2`; fixed>>3 / ×8; z: 2^-22×cell−100 | **pure** | phase 0 |
| 397ad5a0 | 406 | 3×i16 codec (type 1 scale 1/60) | — | 397c0470 | — | `0x39a12664` = 1/60 | pure | phase 0 |
| 397ad8c0 | 406 | 3×i16 codec (types 2/3/5, 1/30) | — | 397c0470 | — | `0x39a12668` = 1/30, clamp ±1 | pure | phase 0 |
| 397ada60 | 406 | 3×i16 codec (type 0) | — | 397c0470 | — | 1/50 path | pure | phase 0 |
| 397adc00 | 149 | quaternion normalise + i16 | — | 397c0470 | — | w = sqrt(1−x²−y²−z²) | pure | phase 0 |
| 397ad740 | 376 | 3×u8 codec (delta movement) | — | 397c0470 | — | 1/127 (`0x399a28dc`) | pure | phase 0 |
| 397ae020 | 602 | quaternion writer (inverse of 397adc00) | — | 397c0470 | — | ×32767, w sign flip | pure | phase 0 |
| 394045f0 | 243 | vehicle/ship send pump | A | 394064b0 | 390b3750 | opcode 0x89; timer; move at [esp-0x12c] | stateful | phase 4: hooked bytes |
| 394046f0 | 1624 | on-foot send pump | A | 394064b0 | 39024610, 39186810 | same shell, type 1 | stateful | phase 4 (F1) |
| 394064b0 | 139 | choose sender | A | 39406650, 39406740 | 394045f0, 394046f0 | GameClient mode | stateful | phase 4 |
| 39406650 | 228 | per-unit timer | — | 39406740 | 394064b0 | +1 accumulated dt | stateful | phase 4 |
| 39406740 | 307 | movement pump | G | 391716b0 | 39406650, 394064b0 | 5 extra slots + main | stateful | phase 4 |
| 39404d50 | 5971 | per-unit input update (F1) | — | 39406540 | (F1) | axes, turn, request | stateful | out of S3 (F1) |

### 1.7 Attach, seats, binding

| addr | size | name/role | this | in | out | fields / globals | kind | difftest |
|---|---|---|---|---|---|---|---|---|
| 393be280 | 606 | target interaction | — | 393b1de0, 3947db70 | 393bf270, 393bf310 | interaction id, target | stateful | phase 5 |
| 393bf270 | 151 | interaction range gate | — | 393be280 | 39093860, 390a6af0 | 0x39ac79fc (9.0), 0x39ac7a00 (12.25) | stateful | phase 5: distance table |
| 393bf310 | 485 | interaction dispatch | — | 393be280 | 393565c0, 39362f50, 39365230, 393beb30 | interaction id switch | stateful | phase 5 |
| 39362f50 | 390 | slave bind path | — | 393bf310 | 393464a0 | calls 0x39346290 first | stateful | phase 5 |
| 39346290 | 517 | bind precheck | — | 393464a0 | 39093860, 39360020, 39390900 | seat `+0x410 == 2`; dead +0x3c26; busy +0x3a70..7b; 0x39ac7408 (16.0) | stateful | phase 5: error code per input |
| 393464a0 | 103 | `CSBindSlave` 0x31 | — | 39362f50 | 39186810, 39346290 | log "requesting bind slave(slaveId:%u)" | stateful | phase 5: packet bytes |
| 39345aa0 | 335 | `CSDiscardSlave` 0x32 | — | 39349f00, 394aa090 | 39186810, 39186810 | speed² > 25.0 (0x3999f85c) refused | stateful | phase 5 |
| 39345d00 | 192 | own seat state + event 0x61 | — | 391d4760 | 392045f0, 39424240 | `+0x10=&DAT_3a168a98`, +0x14 slave obj, +0x18 slave id | stateful | phase 5 |
| 39345ea0 | 109 | escape slave | — | 391d48a0 | — | repositions | stateful | phase 5 |
| 39362770 | 300 | attach handler | — | 391d49d0, 39364d40 | 390b3f10, 392045f0, 39424240 | local player + state 2 + point != 1 -> event 0x61 | stateful | phase 5 |
| 390b3f10 | 225 | `X2::Unit::AttachTo` | A | 39362770 | 390e16b0 | log "Unit::AttachTo: id=%u cur=%u new=%u"; +0x3af0/+0x3af4 | stateful | phase 5 |
| 390e16b0 | 94 | `GameClient::AttachEntity` | G | 390b3f10 | 390dea00 | G+0x11c parent, G+0x12c point; entity +0x278, parent +0x288 | stateful | phase 5 |
| 390dea00 | 109 | attachment list add | G | 390e16b0, 390e69d0 | 390d30c0 | list at G+0x120, node {next,prev,entity,point} | stateful | phase 5: list contents |
| 390c9f40 | 42 | attachment lookup by point | G | many | — | returns node entity | pure-ish (reads list) | phase 5 |
| 39093860 | 90 | surface-gap query | — | many | 3989bf90 | min gap over shapes | stateful (physics) | phase 5 / physics oracle |
| 3989bf90 | 101 | min shape gap | — | 39093860 | 398b1ab0 | sphere-sphere etc. | pure-ish (given shapes) | physics port |
| 398b1ab0 | 64 | shape distance helper | — | 3989bf90 | — | — | pure | physics port |
| 398b1af0 | 92 | sphere-sphere gap | — | — | — | centre distance − radii | **pure** | phase 0 (already in aaemu-physics) |

### 1.8 Summon and placement

| addr | size | name/role | this | in | out | fields / globals | kind | difftest |
|---|---|---|---|---|---|---|---|---|
| 3934a3e0 | 332 | `SlaveLocator::Update` | L | 391739e0 | 39346580, 39349ad0, 39349b70 | state `*param` vs 0x3a168a64; mode 1/2; 0x3a5a4ea0 | stateful | phase 5 |
| 39349ad0 | 146 | cursor-ray placement | L | 3934a3e0 | 393488d0, 39348bf0, 394298c0 | ground ray | stateful | phase 5 |
| 39349b70 | 112 | wheel-rotate placement | L | 3934a3e0 | 39346f10, 393488d0, 39348bf0 | mouse axis 8 | stateful | phase 5 |
| 393488d0 | 792 | land ghost validity | — | 39349ad0/39349b70 | 391040b0 | OBB, flags 0xd | stateful (physics) | phase 5 / physics oracle |
| 39348bf0 | 869 | boat ghost validity | — | 39349ad0/39349b70 | 391040b0 | 6400.0 (80 m), flags 0x10d | stateful | phase 5 |
| 39346580 | 145 | invalid-material swap | — | 3934a3e0 | 390ff5b0, 391040b0 | `materials/special/housing_build_invalid` | stateful | phase 5 |
| 3934a6c0 | 289 | `OnSlaveCreated` | — | 391d45d0 | 39347d90, 39349020, 39424220 | fire 0x66; spawn fx | stateful | phase 5 |
| 391d45d0 | 139 | SCSlaveCreated handler | — | packet table | 3934a6c0 | — | stateful | phase 5 |
| 391d4760 | 160 | SCSlaveBound handler | — | packet table | 39345d00, 39354280 | — | stateful | phase 5 |
| 391d49d0 | 155 | SCUnitAttached handler | — | packet table | 39362770 | log + child/parent/payload | stateful | phase 5 |

### 1.9 Transfer manager (area C)

| addr | size | name/role | this | in | out | fields / globals | kind | difftest |
|---|---|---|---|---|---|---|---|---|
| 393539e0 | 608 | `TransferManager::LoadPaths` | TM | 39172420, 39172b10 | 393536c0, 39351b20, 39352470, 39353200, 39352f80, 39352bb0 | `transfer_path.xml`; profiler name | stateful (file io) | phase 5 |
| 393536c0 | 795 | path XML parse | — | 393539e0, 39353c50 | — | `<Transfer Name Type><Point Pos>` | stateful | phase 5: parsed points from a fixture |
| 39351b20 | 168 | spline point add | — | 393539e0 | 39351990 | — | stateful | phase 5 |
| 39352470 | 39 | transfer node find | — | 393539e0 | 39350310 | — | stateful | phase 5 |
| 39353200 | 168 | spline build | — | 393539e0 | 39352740 | — | stateful | phase 5 |
| 39352f80 | 357 | transfer registry insert | — | 393539e0 | 3934aae0, 39351a20, 39352980 | — | stateful | phase 5 |
| 39352bb0 | 73 | registry remove | — | 393532c0, 393539e0 | — | — | stateful | phase 5 |
| 393540a0 | 336 | `TransferManager::Update` | TM | 391739e0 | 393524a0, 39353c50 | per-transfer progress | stateful | phase 5 |
| 393524a0 | 390 | rider/transfer sync | T | 393540a0, 39352630 | 3934a9c0, 3934dd90, 39351bd0 | unit `+0x410 == 4`; T+0x180 = 1 | stateful | phase 5 |
| 39351bd0 | 2132 | transfer rider/seat state apply | T | 393524a0 | 39007240, 390087d0, 390b3670, 390b3ad0, 3934dd90, 39360020 | per-rider point/pose; reads the local player transform | stateful | phase 5: rider pose per point |
| 39353c50 | 821 | path refresh around player | TM | 393540a0 | 393536c0 | — | stateful | phase 5 |
| 393565c0 | 86 | `CSBoardingTransfer` 0x67 | — | 393bf310 | 39186810, 393546b0, 397ab770 | `{tl u16, ap u8}` | stateful | phase 5: packet bytes |
| 393546b0 | 64 | first free point 2..7 | G | 393565c0 | 390c9f40 | GameClient mode 3 | stateful | phase 5 |
| 39440d30 | 76 | `X2.UnBoardingTransfer` | — | Lua native | 39186810, 397ab770 | same opcode, ap = 0 | stateful | phase 5 |
| 39350160 | 115 | transfer move history push | — | 391d4d60, 391d4e70 | 3934ce00 | 100-entry list | stateful | phase 5 |

### 1.10 UI / Lua getters and events

| addr | size | name/role | this | in | out | fields / globals | kind | difftest |
|---|---|---|---|---|---|---|---|---|
| 394aabd0 | 696 | `GetSiegeWeaponSpeed` | — | Lua | 390699b0, 39360020 | parent `+0x410==2`, mode 3/4; signed |v| | stateful | phase 5: value table |
| 394aae90 | 319 | `GetSiegeWeaponTurnSpeed` | — | Lua | 390699b0 | angVel z × 57.2958 | stateful | phase 5 |
| 394ab270 | 373 | `GetVehicleInfo` | — | Lua | 39009e40, 390309f0 | wheeled only | stateful | phase 5 |
| 394aafd0 | 249 | `IsDriver` | — | Lua | 39058bd0, 39345f10 | attach point == 1 | stateful | phase 5 |
| 394aa840 | 282 | `IsMountingShipStucked` | — | Lua | 39345fa0, 3945fcd0 | stuck flag | stateful | phase 5 |
| 394aaad0 | 247 | `DestroyStuckedShip` | — | Lua | 397a6c60, 39397e50 | sends request | stateful | phase 5 |
| 394abd90 | 739 | `GetSlaveEquipSlotInfo` | — | Lua | 39345f10, 39696380 | slot row | stateful | phase 5 |
| 394ac1e0 | 253 | `CanSlaveEquipPickedItem` | — | Lua | 39058bd0, 39424550 | fit test | stateful | phase 5 |
| 394ac320 | 2210 | slave-equipment binding table | — | Lua | — | slot -> native fn | stateful | phase 5 |
| 39204500 | 236 | mode bar fill | — | 392045f0 | 39204320, 39345f10, 39347130 | mount skills + unbind + info | stateful | phase 5 |
| 392045f0 | 263 | mode bar refresh (event 0x5f) | — | 39204700, 39345d00, 39362770 | 39204500, 39424220 | fires MODE_ACTIONS_UPDATE | stateful | phase 5 |
| 392043d0 | — (8) | SHOW_SLAVE_INFO stub (event 0x60) | — | key binding | 39424220 | `push 0x60; call 0x39424220` | pure | phase 5: one-shot |
| 39424220 | 19 | fire Lua event | — | many | — | event id | stateful | phase 5: ids |
| 390699b0 | 113 | round to 0.1 | — | 394aabd0, 394aae90 | — | floor(x*10+0.5)/10 | **pure** | phase 0 |

### 1.11 Shared leaves used by the above

| addr | size | name/role | kind | difftest |
|---|---|---|---|---|
| 39004bb0 | 152 | profiler scope begin (name, id) | stateful | not ported |
| 390115a0 | 297 | profiler scope begin (name, level, freq, flag) | stateful | not ported |
| 390116d0 | 237 | profiler scope end | stateful | not ported |
| 3922aed0 | 39 | clamp(v, lo, hi) | **pure** | phase 0 |
| 390b3750 | 7 | GameClient from actor (`*(this+0x3be8)`) | pure-ish | phase 0/5 |
| 390b3850 | 20 | player forward vector | stateful | F1 |
| 390b3870 | 16 | player right/up vector | stateful | F1 |
| 390b3ad0 | 16 | player angular velocity | stateful | F1 |
| 39098250 | 110 | entity pose/velocity getter | stateful | phase 4 |
| 39097a20 | 226 | physics wheel params | stateful | phase 4 |
| 39098450 | 306 | controller base init | stateful | phase 4 |
| 390a8dc0 | 245 | controller base ctor | stateful | phase 4 |
| 39094100 | 80 | pose copy after physics | stateful | phase 4 |
| 39096780 | 56 | get entity by id | stateful | phase 4 |
| 390c8a40 | 532 | wheel vector realloc | stateful | phase 3 |
| 390c8e20 | 291 | wheel/bone vector grow | stateful | phase 3 |
| 390bf0f0 | 279 | quaternion from axis-angle | **pure** | phase 0 |
| 390bf210 | 289 | quaternion multiply | **pure** | phase 0 |
| 390bf6a0 | 252 | quaternion helper | pure | phase 0 |
| 39101300 | 412 | bone pose helper | stateful | phase 3 |
| 390ae230 | 132 | ground-align apply | stateful | phase 4 |
| 390c1e80 | 92 | reset model pose | stateful | phase 4 |
| 390c1ff0 | 79 | set wheel physics param | stateful | phase 4 |
| 390db410 | 434 | model speed/scale update | stateful | phase 4 |

---

## 2. Counts

* 191 unique entries (182 listed in `funcs.csv`, 9 unlisted leaves).
* **115,163 bytes** of listed code (unlisted leaves are ~40 B together).
* Pure leaves (safe to port and difftest before any world exists), **17**:
  * move codecs/serializer: 0x3961e2c0, 0x397ad5a0, 0x397ad8c0, 0x397ada60, 0x397adc00,
    0x397ad740, 0x397ae020, 0x397c0470, 0x397c52f0, 0x397c5e70, 0x397c5f60 (11);
  * move copy: 0x39024610 (1);
  * math: 0x3922aed0 (clamp), 0x390bf0f0 / 0x390bf210 / 0x390bf6a0 (quaternion helpers),
    0x390699b0 (round 0.1), 0x398b1af0 (sphere-sphere gap, already in aaemu-physics) (6).
* Stateful but testable with a synthetic object: 0x390967c0, 0x390a3de0, 0x390c7410,
  0x390d2990, 0x390e8e40 (with a fake doodad), 0x390db700, 0x390b2420/0x390b0040 (with a fake
  entity), 0x390af740/0x390c70f0.
* Everything else needs a live model/actor/world (phases 3–5).
