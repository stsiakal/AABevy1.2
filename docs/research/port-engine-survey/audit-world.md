# Audit E9: Cry3DEngine "covered" areas against the 1.2 decompile

Status: read-only audit for roadmap v2 phase 0 (audit E9). Branch `map/audit-world`, worktree
`D:/aaemu-client/wt/map5`, from client `main` `40897af6`. **No crate code was changed.** The
published CRYENGINE source was not used, quoted or consulted; class and method names come from the
binaries' own profiler strings, cvars and RTTI.

The survey (`docs/research/port-engine-survey/content.md` §10) marks **1,783.0 KB / 4,707
functions** of `cry3dengine` as "covered by `aa-world` + `aa-cry`/`aa-fx`". Covered means an
existing crate implements the area. This file checks the **game-relevant (non-rendering) parts**
against the 1.2 decompile and lists every behavioural difference found, in the priority order of
the task: terrain height/normal, vegetation/brush collision, areas/volumes, water, time of day,
streaming/LOD visibility.

## 0. Sources, conventions, how to read this file

### 0.1 Sources

| source | path | use |
|---|---|---|
| 1.2 Cry3DEngine decompile | `D:/aa-re/client12/engine2/out/cry3dengine.dll.c` | behaviour, band boundaries, callers |
| 1.2 Cry3DEngine functions/strings | `.../cry3dengine.dll.{funcs.csv,strings.txt}` | sizes, profiler names, cvar names |
| 1.2 Cry3DEngine image | `D:/AA1.2/bin32/cry3dengine.dll`, base `0x30000000` | vtable decode, constants, 16-byte stubs the decompiler dropped |
| 1.2 CryEntitySystem decompile | `D:/aa-re/client12/engine/out/cryentitysystem.dll.c` | entity view distance (streaming visibility) |
| 1.2 x2game decompile | `C:/AAEMU_DISK/aa-re/client12/out-x2game/x2game.dll.c` (+ `funcs.csv`, `strings-xrefs.tsv`) | call sites, prefab loaders |
| crate code | `crates/aa-world`, `crates/aa-cry`, `crates/aa-fx`, `crates/aa-client` | what is implemented |
| existing maps | `docs/research/port-movement/MAP.md`, `port-mount-anim/MAP.md`, `port-vehicles/MAP.md`, `docs/research/time-of-day.md`, `docs/aa-world.md` | linked, not repeated |

Evidence markers: **[R]** read in the binary/decompile, **[I]** inference (stated as such),
**[?]** open. Addresses are virtual addresses at the DLL's preferred base; function sizes are
`funcs.csv` `start..end` (LTCG-linked, so a named function can absorb adjacent inlined bodies).

### 0.2 The engine interface, corrected

The survey lists `gEnv` `p3DEngine` at **`+0x54`**. That is **not** the 3D engine in the 1.2
`x2game.dll`: every terrain/water call is made through **`*(gEnv + 0x2C)`**. Evidence:

* 30 call sites of the form `(**(code **)(**(int **)(DAT_3a5bb1d8 + 0x2c) + 0x1fc))(x, y, 0)` —
  the wrapper returns `FUN_300C91B0`, the height query (§1.1).
* 15 sites of `...+ 0x110` with a `float pos[3]` first argument, returning a float compared with the
  terrain height: the water level (§4). `DAT_3a5bb1d8` is the `SSystemGlobalEnvironment` used by
  the whole game (`+0x40` entity system, `+0x14` timer, `+0x2c` 3D engine).
* The `C3DEngine` vtable at image VA `0x263718` (305 slots, dumped from the `.rdata` entries; the
  decompiler does not print it) contains `C3DEngine::Update`, `GetWaterLevel`,
  `GetTerrainElevation` (slot 127 = `+0x1FC`, `FUN_30121A70`), `GetTerrainSurfaceNormal`
  (slot 130 = `+0x208`, `FUN_30124960`), `GetBottomLevel` (`+0x10C`), `GetVisAreaFromPos`
  (`+0x30C`) and the vegetation erase functions (`+0x464/0x468`). Slot numbering below is
  `offset/4` from this table.

**Discovering a new engine query:** find the `I3DEngine` slot in the table above, then its wrapper
`FUN_30xxxxxx`; wrappers are 5–120 B and only forward to the internal `CTerrain`/`CVisArea`/
`CObjManager` function. A cheap harness can call any slot as
`(*(code**)(i3d + off))(...)` with `i3d = *(void**)(gEnv + 0x2C)`.

In the C snippets below `vt(i3d, off)` means "load the 32-bit function pointer at
`i3d + off`"; the calls are written as MSVC `__thiscall` function pointers (the interface `this`
in `ECX`, the remaining arguments on the stack). If the driver compiles the calls as plain
`cdecl`, push the `this` first and call the raw address instead.

### 0.3 Where x2game hands over to the engine (the links that matter for phase 1)

| game use | x2game function(s) | engine call | crate replacement | verdict |
|---|---|---|---|---|
| ground under a point, for snapping, swimming, effects, camera | `FUN_39113B50` "actor swim bottom level" (283 B), `FUN_39088410` "RayWorldIntersection(Game)" (2,812 B, "wind.tornado.water"), `FUN_39106A90` (874 B, entity placement in the prefab band), `FUN_3910C200` (1,205 B, entity placement helper) | `gEnv+0x2C` slot `+0x1FC` `GetTerrainElevation(x,y,0)` (30 sites) | `aa_world::TerrainQuery::height_at` (`crates/aa-world/src/terrain_query.rs:140`) through `aa-client` `player.rs:372`, `mount.rs:1031`, `camera.rs:705` | height **same** on the finest grid; normal **differs** (§1.3) |
| water surface under a point (swim, wade, boats, housing) | same functions | slot `+0x110` `GetWaterLevel(pos,0,0,0,0)` (15 sites) | `aa_world::WaterQuery::water_at` (`crates/aa-world/src/water_query.rs:187`) | records **same**; query **differs** (waves, floor, stream) (§4) |
| ground normal for vegetation/decals/actor alignment | `C3DEngine` slot `+0x208`; `CVegetation::CalcMatrix` calls it directly | `GetTerrainSurfaceNormal` | `TerrainQuery::normal_at` (`terrain_query.rs:152`), `CellHeights::mesh_normal` (`heightmap.rs:188`) | **differs** in sampling (§1.3) |
| brush / vegetation collision | none in x2game; engine `CStatObj::Physicalize`, `CBrush`, `CVegetation` | CryPhysics static entities | `aa_world::WorldCollision` (`collide.rs`) using `aa_cry::physics` blobs | data **same**, runtime **differs** (custom triangles vs CryPhysics) (§2) |
| vis areas / interior/outdoor flags / fog volumes | `VisAreaPrefabObject::Load` (`FUN_39110770`), `WaterVolumePrefabObject::Load` (`FUN_3910CD10`) | `CVisAreaManager` + `I3DEngine::CreateWaterVolume` (`+0x2F4`) | `aa-world`: visareas only as interior brushes; water volumes from `object.dat` | vis areas **not implemented** (§3); water records same, query differs (§4) |
| time of day | server packets -> game -> `SetTimeOfDay` (`FUN_39488A00`) -> engine manager | `CTimeOfDay::Update` `0x300A4C10`, `SetTime` `0x300A46B0` | `aa_world::env` (`sample` at `env.rs:462`, `spline_hour` at `env.rs:612`) | same warp/sun; layers blended (`sample_blended`, §5) |
| who is visible: units/doodads/static objects | x2game creates entities; engine culls by entity view distance | `CEntity::GetViewDistance` `0x3207DAC0` (cryentitysystem); `CObjManager` view ratios | units hard-capped at 140/160 m (`crates/aa-client/src/ingame/units.rs:40`), static objects by compiled `maxViewDist` (`objects.rs:107`) | static objects same, **entities differ** (§6) |

The phase-1 (on-foot/mounted feel) **critical** links are the first four rows: terrain height and
normal (used by the mover, camera and object alignment), brush/vegetation collision (what the mover
slides on) and water (swim/wade/mount depth). The last three rows matter for world flow and for what
the player sees around them, but do not change the local movement response.

---

## 1. Terrain height and normal queries

### 1.1 Original functions

| function | module | addr | size | callers | callees / reads | pure? |
|---|---|---|---|---|---|---|
| `I3DEngine::GetTerrainElevation` wrapper | cry3dengine | `0x30121A70` | 69 B | vtable slot 127 `+0x1FC`; 30 x2game sites | calls `FUN_300C91B0(x,y,flag,0)` (4 stack args, no `this`); returns 0 if `DAT_302DCD2C` (terrain global) is null | pure (terrain cache read) |
| `CTerrain` height sample | cry3dengine | `0x300C91B0` | 665 B | 29 sites (wrappers, normals, vegetation, decals, placement) | `FUN_300C8B60` (4 samples), `FUN_301EA870` when the 3rd arg is set; globals `DAT_302DCD2C`, `DAT_302DCD3C+0x5C30` | stateful only through the loaded terrain |
| `CHeightMap` corner samples | cry3dengine | `0x300C8B60` | 984 B | 3 | reads node `+0`, `+1`, `+2`, `+4` (size u16), `+5` (1/unit), `+0x12` (LOD shift); returns 4 heights | pure given the node |
| `I3DEngine::GetTerrainSurfaceNormal` wrapper | cry3dengine | `0x30124960` | 123 B | vtable slot 130 `+0x208`; `CVegetation::CalcMatrix` | `vt+0x20C` (= heightmap unit size, 2) `* 0.5` -> radius 1.0; calls `FUN_300C23D0(out, pos, 1.0, 0)` | pure |
| `CTerrain::GetTerrainSurfaceNormal` | cry3dengine | `0x300C23D0` | 699 B | 8 | 4 x `FUN_300C91B0` at `pos ± (r + 0.05)`; `DAT_3024C838` = 0.05 | pure |
| `CTerrain::GetTerrainSurfaceNormal_Int` | cry3dengine | `0x300C21C0` | 520 B | 2 | `FUN_300CABC0` twice per axis (sector height), `DAT_30290660` = 2 (m/sample) | pure |
| `I3DEngine::GetHeightMapUnitSize` | cry3dengine | `0x30123530` | 5 B | vtable slot 131 `+0x20C` | returns global `DAT_30290660` = 2 | pure |
| `CHeightMap::RayTrace` | cry3dengine | `0x300C9520` | 3,062 B | — | heightfield stepping, holes/voxels | pure |
| `CHeightMap::IsPointUnderGround` | cry3dengine | `0x300CA340` | 625 B | — | height sample vs z | pure |

Numeric parameters from the image: heightmap unit **2 m**, sector **64 m** (`DAT_30290670` = 32
units), 8x8-cell heightmap window **8192 m** (`DAT_30290668`; the cell itself is 1024 m), normal
epsilon **0.05 m**, public normal
radius **1.05 m**, height query scale **0.5** (`DAT_30290664`).

**Height semantics [R].** `FUN_300C91B0` reads the four heights of the 2 m quad and interpolates
linearly over the two triangles split at `tx + ty = 1`: for `tx+ty <= 1` it returns
`(1-tx-ty)*a + tx*b + ty*d`; otherwise `(1-ty)*b + (tx+ty-1)*c + (1-tx)*d`. That is exactly the
finest mesh the renderer draws (south-east to north-west diagonal) and the crate's
`CellHeights::mesh_height` (`crates/aa-world/src/heightmap.rs:183-192`). It returns **0.0**, not
an error, when the terrain is not loaded, the position is outside the 8192 m window, or the sector
node has no data (`DAT_302DCD2C == 0`). It does **not** test holes: heights exist over holes.

**Normal semantics [R].** The public normal is not the face normal of the triangle. It samples
four heights at `(x-d, y-d)`, `(x, y+d)`, `(x+d, y)` and `(x+d, y+d)` with `d = 1.0 + 0.05`
(`FUN_30124960` takes the heightmap unit 2 and multiplies by 0.5, then `FUN_300C23D0` adds 0.05),
and combines them into `(2d(H1-H4-H3+H2), 2d(H3-H2-H4+H1), 8d²)`, normalised with an SSE rsqrt +
Newton step (verified from the disassembly, not only the decompiler). The sample set is not a
symmetric square: it lacks `(x-d, y+d)` and uses `(x, y+d)` twice in the differencing. For a plane
`h = ax + by` this gives the direction `(-(3a+b), -(a+3b), 4)` instead of `(-a, -b, 1)`, i.e. it is
close to, but not, the true slope on a slope. `GetTerrainSurfaceNormal_Int` is the
central-difference variant used internally: `(h(x+1)-h(x-1)) / (2 * unit)` and
`(h(y+1)-h(y-1)) / (2 * unit)`, with `unit = 2 m`.

### 1.2 Crate implementation

| what | where | used by |
|---|---|---|
| height over the finest 2 m mesh, no holes | `TerrainQuery::height_at` `crates/aa-world/src/terrain_query.rs:140` -> `CellHeights::mesh_height` `heightmap.rs:183` | player, mount, camera, combat, HUD |
| physical ground (holes excluded) | `TerrainQuery::ground_at` `terrain_query.rs:162` | `WorldCollision` |
| normal: face normal of the same triangle | `TerrainQuery::normal_at` `terrain_query.rs:152` -> `CellHeights::mesh_normal` `heightmap.rs:188` | player/mount/camera/placement |
| central-difference normal (smoother, for rendering) | `CellHeights::normal` / `normal_step` `heightmap.rs:195-209` | terrain shading, not gameplay |
| outside the map / not loaded | returns `Option::None` (`terrain_query.rs:131-136`) | callers treat `None` as "no terrain" |
| cache | 48 cells LRU, 1.3 MB each, background prefetch (`terrain_query.rs:86-196`) | - |

The crate's own oracle test (`terrain_query.rs:266-283`) compares eight native `300C91B0` outputs
(including over holes) to 1e-4 m, and `heightmap.rs:276-299` compares the loader/sample/hole
signature against the executed original `300DB920`/`300C8B60`/`300CAB30`. Height is therefore
**verified**, not just plausible.

### 1.3 Verdict: height **same**, normal **differs**

* Height (finest LOD): **same**. Same triangle split, same samples, same 0.0-vs-`None` difference
  only at the API edge. The original can return a *coarser* height if the sector's heightmap
  geometry has been released (`CTerrainNode::ReleaseHeightMapGeometry`, `CheckNodeGeomUnload`) —
  x2game's gameplay queries run near the actor, where LOD0 is resident, so this does not affect
  movement [I].
* Outside the map: original returns **0.0**; crate returns `None`. All crate callers handle
  `None`; no gameplay difference found. But a differential test that compares raw floats must
  special-case it.
* Normal: **differs**. `normal_at` is the triangle's face normal (piecewise constant per 2 m
  triangle), while the original public normal uses four samples over a 2.1 m square and is
  therefore smoother across triangle edges and has a different value on any non-planar quad. On a
  plane the original formula is *not* exactly the plane normal either (the sample set is not
  symmetric); the magnitude of the difference on the spawn/road slopes was not measured. This
  feeds the player/mount ground alignment, the camera's ground tilt and vegetation tilt (§2).
* Normal `_Int`: the crate's `CellHeights::normal_step(ix, iy, 1)` (`heightmap.rs:200-209`) is the
  same central difference at ±1 sample; it is used for shading only. If a gameplay caller needs
  the native *internal* normal, call `normal_step` rather than `normal_at`.
* **Port status:** `TerrainQuery::normal_at` (and therefore `ground_at`) now implements
  `FUN_30124960` -> `FUN_300C23D0` exactly (1.05 m four-tap, native cross product and rsqrt +
  Newton normaliser). Twenty live cases are in `crates/aa-world/tools/terrain_normal_golden.json`
  (worst component error 1.9e-5). Note the terrain queries take engine-space coordinates: after a
  `ChangeOrigin` (SeamlessSystem, whole sectors) the absolute game point is the engine point plus
  the sector shift; the capture records `origin_shift` for this reason.

### 1.4 How to call the originals in-process (hidden 1.2 client)

All calls go through the 1.2 `x2game.dll` global `gEnv` at VA `0x3A5BB1D8` (preferred base
`0x38FF0000`): `i3d = *(uint8_t**)(*(uintptr_t*)0x3A5BB1D8 + 0x2C)`.

```c
// height (slot 0x1FC). Returns a float; 0.0 outside the map / before terrain load.
float z = ((float (__cdecl*)(float,float,int))vt(i3d,0x1FC))(x, y, 0);

// normal (slot 0x208): out first, then the position components by value.
float n[3];
((float* (__thiscall*)(void*, float*, float, float, float))vt(i3d,0x208))(i3d, n, x, y, z);

// raw height sample (FUN_300C91B0): x, y, flags, 0; it reads the terrain global itself.
float z2 = ((float (__cdecl*)(float,float,int,int))0x300C91B0)(x, y, 0, 0);
```

Read-only, no allocation, safe to call every frame if the driver is injected into a client that
has finished loading a world. For a differential run, sample a grid over a cell (2 m spacing plus
random fractional points) and compare `height_at`, then `normal_at` at 16 m spacing; report the
distribution, not a single point. Guard the call while `DAT_302DCD2C == 0` (login/character
select).

### 1.5 Port order (terrain)

1. `height_at` grid test (already done in-tree) — keep as the regression baseline.
2. Add a `normal_at_native` (four-sample, radius 1.05) next to the face normal and use it where
   the original did: vegetation align (§2), decals, camera owner-follow. A/B the mounted camera
   feel before switching the player controller.
3. `GetBottomLevel` (`0x301270F0`, 556 B) and `CHeightMap::RayTrace` (`0x300C9520`) are the only
   remaining terrain leaves with gameplay callers (camera/placement); map them when a camera ray
   or a placement uses them.

---

## 2. Vegetation and brush collision data

### 2.1 Original functions

| function | module | addr | size | callers / callees | fields read/written | pure? |
|---|---|---|---|---|---|---|
| `CObjManager::_LoadBrush` (profiler `_LoadBrush`) | cry3dengine | `0x301F2CA0` | 685 B | `CObjManager::ProcessObjectsStreaming` | 128 B brush record: bbox, `maxViewDist` @28, flags @32, view/lod ratios @36-37, matrix @67, material @115, model @123; creates `CBrush`, adds to octree | stateful (allocates) |
| `CObjManager::_LoadVegetation` (profiler `_LoadVegetation`) | cry3dengine | `0x301F5A90` | 919 B | same | 64 B record: pos @39, scale @51 (clamped 0.1..5.0), group @55, angles @57-60, colour @61-63, flags @32; creates `CVegetation` | stateful |
| `CVegetation::CalcMatrix` | cry3dengine | `0x300F4390` | 1,171 B | vegetation loader, `OnRenderNodeBecomeVisible`, `ChangeOrigin`, dynamic add | group `+0x28` bit 8 (= the XML `bAlignToTerrain` [I]); instance `+0x12` yaw byte, `+0x13` scale, `+0xF/0x10/0x11` position; calls `GetTerrainSurfaceNormal(pos, bboxRadius)` | pure |
| `CVegetation::CalcTerrainAdaption` | cry3dengine | `0x300F13D0` | 865 B | vegetation loader | samples 4 heights around the instance, packs 4 heights into `CVegetation+0x15` for the terrain colour/detail shader | pure |
| `CVegetation::Dephysicalize` | cry3dengine | `0x300F22F0` | 489 B | object destroy/unload | clears flags `+0x38` & ~0x18; destroys the physics entity through `IPhysicalWorld` | stateful |
| `CVegetation::Physicalize` | cry3dengine | string `0x3025FEEA` (function body not recovered) | — | loader / visibility | inferred: creates a static physics entity from the group's stat obj with the instance matrix | stateful |
| `CStatObj::Physicalize` | cry3dengine | `0x30030F20` | 2,175 B | `IStatObj` vtable slot at `0x30255EC4` | CGF physics blob, `nPhysicalizeType`, flags, surface ids; creates the `IPhysicalEntity` (static/box/trimesh) | stateful |
| `CBrush::OnRenderNodeBecomeVisible` | cry3dengine | `0x300FFD60` | 1,334 B | render node birth | render node only, no physics | stateful |
| `CBrush::ChangeOrigin` | cry3dengine | `0x300FED00` | 839 B | seamless origin move | shifts the brush and its physics | stateful |
| `C3DEngine::EraseVegetations` / `Cancel…` | cry3dengine | `0x301422C0` / `0x301418A0` | 568 / 267 B | x2game `HousingBase::EraseVegetation` (`0x3908E040`) through vtable `+0x464/0x468` | removes instances inside a radius from the octree/renderer | stateful |
| `CObjManager::ProcessObjectsStreaming` | cry3dengine | `0x3011DF10` | 1,798 B | `CObjManager::UpdateStreaming` | creates/streams brushes, vegetation, decals, water in priority order | stateful |
| `CObjManager::UpdateObjectsStreamingPriority` | cry3dengine | `0x301188D0` | 2,572 B | `CObjManager::Update` | per-object distance, `e_view_dist_ratio*`, `e_stream_for_physics` | stateful |

The collision geometry itself is CryPhysics `MeshPhysicsData` geometry parsed by `CStatObj::Physicalize`
(CGF blob types 0 box, 1 trimesh, 4 sphere, 5 cylinder, 6 capsule) and put into the physics world as
static entities. There is **no** separate "collision data" file: brush and vegetation records point at
the same `.cgf` the renderer uses. `_LoadBrush`/`_LoadVegetation` only place the node; the physics
entity is created lazily with the model stream. Cvars: `e_vegetation_create_collision_only`
(`+0x5A0`, read in `_LoadVegetation`), `e_stream_for_physics` (`+0x2E0`, default 1),
`e_stream_for_visuals` (`+0x2E4`, default 1), `e_stream_cgf`.

### 2.2 Crate implementation

| what | where |
|---|---|
| brush record: matrix, bbox, model/material, flags, `maxViewDist`, `lodRatio` | `crates/aa-world/src/placement.rs:376-402`; `brush.dat` reader `placement.rs:424` |
| vegetation record: pos, scale, group, 3 angles, flags, `maxViewDist` | `placement.rs:404-420`; `vegetation.dat` + sector files `placement.rs:426-450` |
| vegetation group XML: model, material, sprites, shadow, brightness, bending | `crates/aa-world/src/vegetation.rs:85-123` (**no** `bAlignToTerrain`, `bRandomRotation`, `bHideability`, `bPickable`, `bAffectedByVoxels`) |
| instance transform: serialized Euler ZYX × scale (clamped 0.1..5) × position | `placement.rs:101-111`, collision copy `collide/instances.rs:76-87` |
| physics blob parse: box/trimesh/sphere/cylinder/capsule, surfaces | `crates/aa-cry/src/physics.rs:1-135`, `collision_mesh` `physics.rs:308` |
| brush/vegetation collision build per cell; skip `no_collide` sub-materials; fallback off | `crates/aa-world/src/collide.rs:1035-1092`, `:158-166` |
| collision queries (tri-soup): `stand`, `cast`, `sweep_sphere`, `push_out`, `slide` | `collide.rs:583`, `:654`, `:706`, `:787`, `:865` |
| collision coverage: cells within 150 m built, dropped past 400 m | `collide.rs:158-159`, `:449` |
| no vegetation erase (housing) | no match for `EraseVegetations` under `crates/` |

### 2.3 Verdict: data **same**, runtime **differs**, vegetation alignment **differs**

* **Record parsers and physics data: same.** The crate reads the same `brush.dat`/`vegetation.dat`/
  sector files and the same CGF `MeshPhysicsData` blobs, honours the scale clamp (0.1..5.0), the
  `no_collide` sub-materials and the "no physics geometry ⇒ no collision" rule. The vegetation group
  table lacks only fields that do not change collision existence.
* **Runtime: differs.** The original hands every brush and physicalized vegetation instance to
  CryPhysics as a static entity and lets the character controller/`MoveEntity` work against it. The
  crate instead keeps a triangle soup per 1024 m cell (`collide.rs`), converts box/sphere/cylinder/
  capsule proxies to triangle meshes (12 triangles for a box, a 12-sided prism for a cylinder;
  `aa-cry/src/physics.rs:26-28`, `:98-135`) and runs its own `stand`/`slide`/`push_out`/`sweep`
  code. Consequences for the mover: no exact analytic box/cylinder contacts, no CryPhysics
  continuous sweep, no solver/impulse response, different contact normals on curved proxies.
  `aaemu-physics` is ported (task statement) but is **not used by `aa-client`'s mover**.
* **Collision coverage: differs.** The original streams/physicalizes objects by camera priority
  over the object streaming range; the crate builds collision for 150 m around the focus and drops
  it past 400 m (`CollisionConfig`, `collide.rs:158-159`, `follow` `:449`). Any collision ray
  farther than ~150 m (camera pull-in on a far wall, a long ranged attack, a placement probe)
  sees terrain only. For on-foot/riding feel this is invisible while the mount stays near its
  collision cells; it is a real difference for long camera arms and combat.
* **`bAlignToTerrain`: differs.** 737 of the 2,745 main-world vegetation groups set
  `bAlignToTerrain="1"` (checked in the shipped `vegetation.xml`). `CVegetation::CalcMatrix`
  (`0x300F4390`) re-orients the instance to `GetTerrainSurfaceNormal` when that group bit is set;
  `aa-world` ignores the attribute and uses only the serialized Euler angles
  (`placement.rs:101-111`). Trees/bushes on slopes are therefore upright in the crate but tilted in
  the original, and their collision proxies tilt with them. `bRandomRotation`/`bPickable` are
  ignored too; `bPickable` affects interaction rays, not the mover.
* **Housing erase: not implemented.** The original removes vegetation in a radius when a house is
  built (`C3DEngine::EraseVegetations`, called by `X2::HousingBase::EraseVegetation`); the crate
  keeps the plants (and their collision) inside placed houses. P3, not phase 1.
* **Vegetation `bHideability`/`bAffectedByVoxels`: ignored.** Agriculture/voxel interactions, P3.

### 2.4 How to call the originals in-process

* `CStatObj::Physicalize` is not directly callable without a loaded stat obj: call the `IStatObj`
  vtable slot at `0x30255EC4` with `(this, SPhysGeomInfo*, bApplyJoint, nPhysicalizeType)`.
* Collision existence for a brush can be tested through `IPhysicalWorld` (`gEnv+0x38`) after the
  cell streams: `GetEntitiesInBox` (`vt+0x2C`, see `RayWorldIntersection(Game)` `FUN_39088410`).
  A harness that wants to compare what the mover hits should spawn an `IPhysicalWorld` box query
  around a known brush instead of poking `CBrush` internals.
* `CVegetation::CalcMatrix` (`0x300F4390`) is pure and callable with a live `CVegetation` pointer:
  `this = instance`, args `(float out[12], uint flags*)`. The instance is reachable from a
  `RayWorldIntersection`/`GetEntitiesInBox` result only indirectly; easier to call it on a
  `CVegetation` found by iterating the object octree (not exposed). Mark as inspection-only.

### 2.5 Port order (collision)

1. Keep `aa-cry::physics` as the data oracle (already ported); add a test that compares the
   triangle count and bounding box of every brush/vegetation proxy against the original
   `CStatObj::Physicalize` output for one cell.
2. If phase-1 feel needs exactness, move the mover onto `aaemu-physics` (heightfield + static
   entities + character controller) instead of extending the tri-soup. That is the only way to
   reproduce box/cylinder contacts and continuous sweeps; the data is already parsed.
3. Independent of (2): honour `bAlignToTerrain` in the vegetation matrix, because it changes both
   the visual and the proxy of 27 % of the groups.
4. Add vegetation erase when housing lands (P3).

---

## 3. Area and volume triggers and shapes

### 3.1 Original functions

| function | module | addr | size | what |
|---|---|---|---|---|
| `CVisAreaManager::LoadCell` | cry3dengine | `0x3017A770` | 2,408 B | reads the cell's visarea table (areas, portals, occlusion volumes), builds the area tree |
| `CVisArea::LoadCell` | cry3dengine | `0x30170CE0` | 1,141 B | one area: shape points, flags (sky/light/fog/outdoor), portals, `VisArea` name |
| `CVisAreaManager::CreateVisArea` | cry3dengine | `0x301712C0` | 89 B | game prefab creation (`I3DEngine` slot `+0x2E4`) |
| `C3DEngine::DeleteVisArea` | cry3dengine | `0x30127510` | 556 B | slot `+0x2E8` |
| `CVisAreaManager::GetVisAreaFromPos` | cry3dengine | `0x301747E0` | 246 B | slot `+0x30C`; dispatches to `FUN_30171540` |
| point-in-area (AABB + child tree + `CVisArea` vtable `+0x38`) | cry3dengine | `0x30171540` | 182 B | recursive test |
| `CVisAreaManager::IntersectsVisAreas` | cry3dengine | `0x301748E0` | 254 B | slot `+0x310`; `FUN_30171600` |
| box-vs-area AABB test | cry3dengine | `0x30171600` | 249 B | recursive test |
| `CVisAreaManager::ClipOutsideVisAreas` | cry3dengine | `0x301749E0` | 377 B | clip a box/segment to area |
| `CVisAreaManager::SetEntityArea` | cry3dengine | `0x30172E50` | 761 B | sets `IEntity` area pointer when it crosses a portal |
| `CVisAreaManager::CheckVis` / `IsOccludedByOcclVolumes` | cry3dengine | `0x30175410` / `0x30175AD0` | 1,726 / 373 B | renderer visibility |
| `X2::VisAreaPrefabObject::Load` | x2game | `0x39110770` | 967 B | `<VisArea>` prefab -> `I3DEngine::CreateVisArea` (`+0x2E4`) |
| `X2::WaterVolumePrefabObject::Load` | x2game | `0x3910CD10` | 1,191 B | `<Water Volume>` prefab -> `+0x2F4` CreateWaterVolume; fields `VolumeDepth`, `StreamSpeed/Dir`, `FogDensity/ColorMultiplier`, `UScale/VScale`, `NoFishing`, `Points` |
| ToD polygon areas (`Area` Group 14) | x2game entity system | native `0x3208F090` / `0x32094360` (cited by crate) | — | polygon prism point test used by `aa-world` |
| sound areas (`AreaShape`, Group 0) | x2game entity system / cell XML | — | — | `entities.xml` outlines, sound entity binding |

Runtime use: vis areas are **created** by the game from prefabs and by the cell streamer; they are
not queried by x2game's gameplay code (the 3D-engine `+0x30C` query has no x2game call site). The
engine uses them for indoor/outdoor flags, portal occlusion and area light/fog. The game gets the
area pointer through the entity (`SetEntityArea`) and entity scripts can read the area name/flags.

### 3.2 Crate implementation

| what | where |
|---|---|
| `visareas.dat` parsing: only the interior brushes/vegetation/decals/roads/water/voxels | `crates/aa-world/src/placement/visareas.rs:1-60` (explicitly "Area/portal/occlusion records are consumed ... Unsupported render kinds are skipped") |
| no area/portal/occlusion query | no `GetVisAreaFromPos`, `IntersectsVisAreas`, `SetEntityArea` equivalent under `crates/` |
| ToD polygon areas (Group 14), native point-in-polygon + height/falloff/priority | `crates/aa-world/src/area_tod.rs:39-88`, parser `:90-180` |
| sound areas (`AreaShape`): polygon prism, hours, radius, reverb read-only | `crates/aa-world/src/objects/sounds.rs:1-35`, parser `:137-173` |
| `OutdoorOnly` sound flag | treated as outdoor everywhere (`objects/sounds.rs:20-24`: "the client has no indoor test") |

### 3.3 Verdict: ToD **same**, sound **differs in one flag**, vis areas **not implemented**

* **Vis areas (shapes, portals, indoor/outdoor, area fog/sky flags): not implemented.** The crate
  parses the file only to extract the placements inside it; the area records themselves are skipped
  (`placement/visareas.rs:4-6`). There is no point/box query, no portal graph, no entity area
  assignment, and no indoor test. This is P3/P5 (interiors, weather, housing), not phase 1.
* **ToD polygon areas: same**, with a native point-in-polygon (`CalcPointWithin`) and the nested
  `TimeOfDay` curves; priorities and falloff are honoured (`area_tod.rs`).
* **Sound areas: same except `OutdoorOnly`.** The original uses the vis-area indoor flag for
  `OutdoorOnly`; the crate has no indoor test and plays these events everywhere. P3/P5.
* **Water volumes** are the third area system; they are covered in §4.

### 3.4 How to call the originals in-process

```c
// point -> area (returns CVisArea* or 0); slot +0x30C is a 34 B wrapper.
void* area = ((void* (__cdecl*)(float*))vt(i3d,0x30C))(pos);   // pos = float[3]

// box -> any area; slot +0x310 takes a pointer to a 6-float AABB (min, max).
bool hit = ((bool (__cdecl*)(float*))vt(i3d,0x310))(aabb);

// create/delete a game vis area (prefab path)
void* va = ((void* (__cdecl*)())vt(i3d,0x2E4))();
((void (__cdecl*)(void*))vt(i3d,0x2E8))(va);
```

The manager must have a loaded cell (`DAT_302DCD34 != 0`); otherwise the wrappers return 0. A
differential harness can walk the player through a doorway in a capital interior and compare the
indoor flag every 0.1 m; there is no crate side to compare yet.

### 3.5 Port order (areas)

1. ToD areas are done; no action.
2. When interiors matter (P3): parse and keep `visareas.dat` area/portal records, port
   `GetVisAreaFromPos` (`0x301747E0` + `0x30171540`) and `SetEntityArea` (`0x30172E50`) first;
   portals/culling are rendering and can stay with Nick.
3. Add the indoor test to sound areas (`OutdoorOnly`) once (2) exists.

---

## 4. Water level queries

### 4.1 Original functions

| function | module | addr | size | what |
|---|---|---|---|---|
| `C3DEngine::GetWaterLevel` | cry3dengine | `0x3012B2E0` | 1,644 B | `I3DEngine` slot `+0x110`; returns the highest water surface at `pos`; optional out = water body id |
| `GetOceanWaterLevel[UpdateLevel]` / `[...WaveCalculation]` | cry3dengine | `0x30127320` | 487 B | slot `+0x114`; caches the terrain header ocean level (`DAT_302DCD2C+0xDF0`), adds `FUN_300E3250` waves |
| ocean wave height | cry3dengine | `0x300E3250` | 1,981 B | 4-direction wave sum, amplitude from globals `DAT_302DCD3C+0x304/0x308`, time |
| `CObjManager::_LoadWaterVolume` | cry3dengine | `0x301F6AB0` | 3,007 B | reads a 119 B object.dat type-11 record; creates `CWaterVolumeRenderNode` (`0x300E7E80`, 952 B); separate render and physics point arrays |
| `I3DEngine::CreateWaterVolume` | cry3dengine | `0x30124450` | 918 B | slot `+0x2F4`; material, fog colour, depth, no-fishing |
| `I3DEngine::SetWaterVolume` | cry3dengine | `0x30127940` / `0x30124850` | 412 / 225 B | slots `+0x2F8` / `+0x2FC` |
| `C3DEngine::ProcessOcean` / `CTerrain::UpdateOcean` | cry3dengine | `0x30150970` / `0x300CF4D0` | 967 / 284 B | wave clock and ocean render update |
| `X2::WaterVolumePrefabObject::Load` | x2game | `0x3910CD10` | 1,191 B | editor prefab `<Water Volume>`: `VolumeDepth`, `StreamSpeed`, `StreamDirX/Y/Z`, `FogDensity`, `FogColorMultiplier`, `UScale/VScale`, `NoFishing`, `Points` -> `+0x2F4` |

Record fields used by the crate are exactly those of `_LoadWaterVolume` (`0x301F6AB0`): bbox,
`maxViewDist` @28, type @39 (2 = area/lake, 3 = river), material @51, fog density/colour @55-67,
plane normal/origin @71-95, UV/river U @87-99, depth @107, point counts @103/@115, then the
render and physics outlines. The engine query path is: `GetWaterLevel` starts from the ocean level
(slot `+0x114`, waves added) and then asks the physics interface (`DAT_302DCD28`, [I]; the same object is `gEnv+0x38` elsewhere) for water planes; each
horizontal plane gives its surface height and the highest wins. The ocean level is cached from the
terrain header; a missing terrain (`DAT_302DCD2C == 0`) gives `-1000000.0` for a position-less
query and 0 for a position query.

### 4.2 Crate implementation

| what | where |
|---|---|
| object.dat type-11 parser, same fields incl. type, fog, plane, depth, render/physics point counts | `crates/aa-world/src/placement.rs:185-262` |
| `WaterQuery` (clonable `Arc`, 64-cell cache) | `crates/aa-world/src/water_query.rs:108-237` |
| highest volume surface whose outline contains the point, fog, kind | `water_query.rs:192-207` |
| floor test (`surface - depth`) excludes caves under a lake | `water_query.rs:71-107`, `:200` |
| sea = world.xml `oceanLevel` where terrain is below it (or no terrain in a map cell) | `water_query.rs:211-218`, `:147`; `world.rs:72` |
| waves: rendered only (`ocean_wave` swell) | `crates/aa-world/src/water_material.rs:107-109`, `:627` |
| river UV flow for rendering | `crates/aa-world/src/water/authored.rs:9-45` |
| no `NoFishing`, no `StreamSpeed/StreamDir` in the query or the game | no match under `crates/` |

### 4.3 Verdict: data **same**, query **differs** in waves/floor/z, extras **not implemented**

* **Record data: same.** The crate parses the same `object.dat` type-11 records (and its field
  offsets are documented against native `301F6AB0`), including the separate render/physics outlines
  the original keeps (`placement.rs:236-262`). This is the data the in-game client streams; the
  x2game `<Water Volume>` prefab loader is an additional path for prefab entities, not the cell
  source.
* **Surface height: differs by the wave.** `GetOceanWaterLevel` adds the animated ocean wave height
  (`FUN_300E3250`) at the query position; `aa-world::WaterQuery::sea_at` returns the flat
  `oceanLevel` (`water_query.rs:211-218`) while the renderer animates the same sea
  (`water_material.rs:107`). Swimming/wading depth and boat buoyancy therefore differ by the swell
  amplitude, which is time- and position-dependent. Phase 4 for boats, minor for the phase-1
  swim/wade.
* **Floor/depth: differs.** The original wrapper returns the plane's surface for any point under it
  (the depth is a rendering/volume property, not tested by `GetWaterLevel`; the physics query may
  still filter by the volume's bounds); the crate suppresses a
  volume when `pos.z < surface - depth` (`water_query.rs:200`), so a cave under a lake is dry.
  This is the documented intent of the crate; it is not what the original query does. Low phase-1
  impact (few caves under water), P3 for housing.
* **`NoFishing` and `StreamSpeed/Dir`: not implemented.** These are prefab fields (`0x3910CD10`).
  `NoFishing` gates the fishing skill; `StreamSpeed/Dir` drive the water volume's flow. The crate
  has neither. P3 (fishing) / P4 (boats), not movement on foot.
* **Water volume set: same source, not bit-compared.** Both use the cell's object.dat; no count
  comparison was done. A harness can count type-11 records per cell on both sides easily.
* **Ocean level source: same value, different file.** The original caches the terrain header ocean
  level; the crate reads `world.xml` `oceanLevel`. They are 100 m in `main_world`; for a world
  where they disagree the crate would differ. Not verified for every world.
* **Port status:** `WaterQuery` now implements the `GetWaterLevel` selection (highest surface,
  sea included, no floor/z test) and the `GetOceanWaterLevel` flag/band rules (flag 0 wave always,
  flag 1 only within `DAT_3024C924` = 5 m; `CActor::UpdateSwimStats` passes flag 1). Twenty live
  cases are in `crates/aa-world/tools/water_level_golden.json`: the twelve outside the band are
  exact (100.0 m) and prove the wrapper and band; the eight in-band ones record the live wave.
  That wave is **not** the analytical branch of `FUN_300E3250`: the live client takes the other
  branch (`DAT_302DCD40+0x180 != 0`), which bilinearly samples a 64x64 displacement texture the
  ocean renderer generates on the GPU (`COcean::Update`, vt `+0x278` upload) - renderer state, not
  data. The query therefore adds the swell the crate renders (`water.wesl`, same `Time` clock);
  the residual against the original inside the band is the live FFT amplitude (up to 0.58 m in
  the capture), as recorded there.

### 4.4 How to call the original in-process

```c
// water surface at pos (slot 0x110). Extra args: flags, ?, ?, int* waterBodyOut (may be null).
float pos[3] = {x, y, z};
float level = ((float (__thiscall*)(void*, float*, int, int, int, int*))
               vt(i3d,0x110))(i3d, pos, 0, 0, 0, 0);

// ocean only (slot 0x114): 0 = smooth cached level, 1 = with waves.
float ocean = ((float (__thiscall*)(void*, float[3], int))vt(i3d,0x114))(i3d, pos, 1);
```

Compare on a grid in the spawn pond and in the sea, at several hours (the wave clock is global);
the crate side is `WaterQuery::water_at`. Report the difference in metres and whether it crosses
the swim threshold, not the raw float. Valid only after terrain and the cell are streamed.

### 4.5 Port order (water)

1. Keep the parser; add a per-cell count/height comparison against `_LoadWaterVolume` (cheap).
2. If swimming feel matters before boats: add the same wave function to `sea_at` (the renderer
   already has the swell; reuse its phase/amplitude) so query and visual surface agree.
3. Leave `NoFishing`/stream for fishing/boats (P3/P4).


---

## 5. Time of day and game time

### 5.1 Original functions

| function | module | addr | size | what |
|---|---|---|---|---|
| `TimeOfDayManager::SetTime` | cry3dengine | `0x300A46B0` | 132 B | stores the raw hour (`this+0x24`, engine `+0x694` = `e_time_of_day`); warps it once per layer (`vt+0x38` = `0x300A49D0`) and calls layer `SetTime` + `Update` |
| time warp / inverse | cry3dengine | `0x300A49D0` / `0x300A48C0` | 286 / 270 B | quadratic per half day moving `DawnTime`/`DuskTime` onto 6:00/18:00 |
| per-layer `SetTime` | cry3dengine | `0x300A4170` | 30 B | stores the warped hour, `Update` |
| `CTimeOfDay::Update` | cry3dengine | `0x300A4C10` | 704 B | reads the layer splines at `time / 24`; updates sun/ambient/fog colours and multipliers |
| `TimeOfDayManager::Tick` | cry3dengine | `0x300A4ED0` | 316 B | advances `time += speed * dt`, wraps to `[start, end]`, calls `SetTime` |
| `TimeOfDayManager::UpdateTimeOfDays` | cry3dengine | `0x300A5010` | 871 B | blends up to 10 zone layers, normalises their weights |
| `TimeOfDayManager::UpdateEngineMoonParameters` | cry3dengine | `0x300A5380` | 672 B | moon light and back light |
| `TimeOfDayManager::UpdateEngineBackLightParameters` | cry3dengine | `0x300A5630` | 424 B | back light |
| `TimeOfDayManager::UpdateEngine` | cry3dengine | `0x300A5DD0` | 9,139 B | pushes the result to sky/fog/sun/shadows |
| `X2::...::SetTimeOfDay` | x2game | `0x39488A00` | 7,011 B | game/UI reaction to the server clock (large handler, not a single setter) |

Numeric parameters: `e_time_of_day` (raw hour), `e_time_of_day_speed`, `e_time_of_day_engine_update`,
`e_time_of_day_debug`; `DawnTime`/`DuskTime` from `lighting.xml` (`main_world` 119 / 599 minutes = 2:00
/ 22:00). The raw hour is used by the sun path and the HUD; only the spline lookup is warped.

### 5.2 Crate implementation

| what | where |
|---|---|
| raw hour source: server `SCTimeOfDay`/`SCDetailedTimeOfDay`, extrapolated locally | `crates/aa-net/src/state/world.rs:11-30, 66-69`, `game.rs:742-749, 1280-1286` |
| hour copied to the world clock each frame | `crates/aa-client/src/ingame/mod.rs:234-245` |
| warp and sun path | `crates/aa-world/src/env.rs:462-478` (`sample` uses `spline_hour`), `:612-639`, `sun_direction` `:648-659` |
| 1–10 zone layer blend, normalized weights, plus Group-14 ToD areas | `crates/aa-world/src/env.rs:472-490`, `sky.rs:385-416` |
| clock speed/wrap (unused while the server drives the hour) | `sky.rs:39-51`, `:525-528` |
| HUD clock = raw hour | `crates/aa-client/src/hud/api.rs:845-851, 972-973` |

### 5.3 Verdict: **same for gameplay time**, minor differences

* **Raw hour and HUD:** same. Both use the server hour; the warp is applied only to the spline lookup and
  the sun path uses the raw hour (verified against the Royal Palace captures, `time-of-day.md`).
* **Clock progression: differs when disconnected.** `TimeOfDayManager::Tick` (`0x300A4ED0`) advances the
  hour itself by `e_time_of_day_speed`; the client sets `WorldClock.speed = 0` and copies the
  server-extrapolated hour every frame. While the server pushes time this is identical; if the server
  stalls, the original keeps moving and the crate freezes. No gameplay difference in a normal session.
* **Layer blending:** both blend up to 10 zone layers by weight (`sample_blended` matches
  `UpdateTimeOfDays` `0x300A5010`); the crate adds ToD polygon areas. No difference found for the
  spawn/zone-179 case.
* **Sun/moon:** the sun formula is the engine's (`sun_direction`, cross-checked at 9.56 h and 10 h);
  the moon is `+12 h` of the same path while the original has separate moon/back-light splines. Visual
  only, P5.
* **What time affects for gameplay:** NPC and doodad schedules, sound `daylight`, and the HUD all read
  the same raw hour in both. No difference found.

### 5.4 How to call the original in-process

```c
// manager = *(void**)(cry3dengine_base + 0x2DCD1C)   // VA 0x302DCD1C at base 0x30000000
// set the raw hour (slot +0x14); force=0. This is what the server packet handler ends up doing.
((void (__thiscall*)(void*, float, int))vt(manager,0x14))(manager, hour, 0);
uint8_t* gEnv = *(uint8_t**)0x3A5BB1D8;
float now = *(float*)(*(uint8_t**)(gEnv + 0x2C) + 0x694);   // e_time_of_day
```

A harness can sweep `hour` 0..24 in 0.25 h steps and compare `e_time_of_day` (trivial) and the sun
direction (`CTimeOfDay::Update` output) with `aa_world::env::sun_direction`; the spline colours need
the zone's `lighting.xml` and are rendering.

### 5.5 Port order

Nothing to port for phase 1. Keep `spline_hour`/`sun_direction` as the tested leaves; add the moon
spline only when night lighting is polished (P5).


---

## 6. Streaming distances and LOD switches that change what the player sees

This area mixes three systems that are easy to confuse:

1. **static world objects** streamed by `CObjManager` (brushes, vegetation, decals, water) — the
   crate reproduces their native view-distance/LOD formulas;
2. **entities** (server units, doodads, vehicles) culled by `CEntity::GetViewDistance` — the crate
   uses fixed radii;
3. **cells/terrain** streamed by the seamless system — the crate uses a 3000 m radius and its own
   terrain LOD thresholds.

### 6.1 Original functions and numbers

| function | module | addr | size | what |
|---|---|---|---|---|
| `CObjManager::UpdateObjectsStreamingPriority` | cry3dengine | `0x301188D0` | 2,572 B | per-object distance/priority; decides what to stream next |
| `CObjManager::ProcessObjectsStreaming` / `…_Finish` | cry3dengine | `0x3011DF10` / `0x3011F4B0` | 1,798 / 1,152 B | creates brushes, vegetation, decals, water from the cell stream |
| `CObjManager::UpdateStreaming` / `Update` | cry3dengine | `0x3010D650` | 2,154 B | per-frame streaming driver |
| `CObjManager::StartStreamVegetation` | cry3dengine | `0x3010D3F0` | 348 B | vegetation sector stream start |
| `CVegetation::Render` | cry3dengine | `0x300F36E0` | 2,608 B | mesh/sprite switch and view-distance test |
| vegetation sprite switch distance | cry3dengine | `0x300F0C10` / `0x300F0C30` | 30 / 297 B | `max(18 × radius × fSize × max(fSpriteDistRatio, 0.01) × 0.8 × max(width/1024, 1), 8 m)` × scale |
| `CStatObj`/render-node view distance | cry3dengine | `0x30019FF0` | 342 B | `halfDiagonal × e_view_dist_ratio × per-object%` clamped to `e_view_dist_min` |
| `CStatObj::GetLod` / `GetLodFromScale` | cry3dengine | `0x3003FBA0` / `0x30040090` | 377 / 399 B | authored LOD selection by scale and distance |
| `C3DEngine::SetStatInstGroup` | cry3dengine | `0x30130DB0` | 981 B | vegetation group view distance / sprites / shadows |
| `CEntity::GetViewDistance` | cryentitysystem | `0x3207DAC0` | 1,037 B | entity cull distance (players/AI/vehicles/doodads) |

Cvar defaults in the 1.2 client (read from the image): `e_max_view_dst` **2000 m**,
`e_view_dist_ratio` **60**, `e_view_dist_ratio_detail` **30**, `e_view_dist_ratio_vegetation`
**30**, `e_view_dist_custom_ratio` **100** ("special marked objects (Players,AI,Vehicles)"),
`e_view_dist_min` **0**, `e_view_dist_doodad_min` **64 m**, `e_lod_min` 0, `e_lod_max` **6**,
`e_lod_ratio` **6.0**, `e_lod_skin_ratio` 1.0.

`CEntity::GetViewDistance` (`0x3207DAC0`, inferred name from the cvar block it registers):
`halfDiagonal × e_view_dist_ratio × ratio/100`, where `ratio` is the entity's `ViewDistRatio` byte
(`0xFF` sentinel -> 1000); for entities flagged 0x400 ("special": players, AI, vehicles) it uses
`maxBBoxExtent × e_view_dist_custom_ratio × ratio/100` instead; the result is raised to
`e_view_dist_doodad_min` (64 m) for doodads, then clamped by the far plane. The game's prefab
loaders set `ViewDistRatio` from the prefab attribute, default **30.0** (x2game data dll
`0x3999F7C0`) — so a prefab entity is normally culled at `18–30 × its size`, at least 64 m. The
value used for **player/mount actors** was not located in x2game [?]; a 1 m character would be
visible at roughly 30 m with ratio 30 or 60 m with ratio 100.

### 6.2 Crate implementation

| what | where |
|---|---|
| cell stream radius 3000 m / unload 3600 m; brush cap 2000 m; small vegetation 220 m; trees 1000 m; water 1400 m | `crates/aa-world/src/stream.rs:96-124` |
| terrain LOD distances 350 / 750 / 1500 / 2600 m, 12 % hysteresis; LOD0 is always the height query | `crates/aa-world/src/terrain.rs:20-44`, `terrain_query.rs:140-169` |
| brush view distance = compiled `maxViewDist × 1.5` (60/40), capped by `brush_distance`; near/far switch at 1000 m | `crates/aa-world/src/objects.rs:85-135`, `distance.rs:1-60` |
| vegetation: compiled distance for small plants, ×1.5 for trees, cap `tree_distance`; native sprite formula | `objects.rs:122-135`, `objects/sprites.rs:1-40` |
| brush LOD request: native `300FFC60`/`30112240`, `e_lod_max` 6 | `objects/lod.rs:1-120` |
| graphics options map `option_view_distance`/`option_view_dist_ratio`/`…vegetation` | `crates/aa-client/src/graphics.rs:108-125` |
| remote units: drawn within 140 m, dropped at 175 m; animated within 60 m; shadows within 45 m | `crates/aa-client/src/ingame/units.rs:40-58`, `:612`, `:648` |
| remote doodads: drawn within 160 m, dropped at 200 m | `units.rs:40-41`, `:648` |
| static entity slots / animated props in cells: 140 / 380 m bands | `docs/aa-world.md:2386-2395` |
| collision coverage 150 m (see §2) | `crates/aa-world/src/collide.rs:158-159` |

### 6.3 Verdict: static brushes/vegetation **same**, terrain LOD **differs (rendering)**, entities **differ**

* **Static brushes: same.** The crate uses exactly the native formula `radius × e_view_dist_ratio ×
  viewDistRatio/100` with the exported 40 → client 60 scale (1.5) and the 2000 m far plane, and
  reproduces the native LOD request (`source_lod` in `objects/lod.rs` cites `300FFC60`/`30112240`).
  The defaults (group 4) match; other graphics groups are mapped by `graphics.rs`.
* **Vegetation: same, with a crate-only 1000 m tree cap.** Small plants keep their compiled distance
  (deliberate, documented), trees are scaled 1.5 and capped at 1000 m (`tree_distance`). The native
  cap is the far plane (2000 m), so a tree whose scaled distance exceeds 1000 m disappears earlier
  in the crate. Trees are rarely authored beyond ~700 m (`docs/aa-world.md`), so the visible
  difference is small.
* **Terrain LOD: differs, rendering-only for phase 1.** The crate picks 5 patch levels at 350/750/
  1500/2600 m with 12 % hysteresis (`terrain.rs:30`); the original computes sector LOD from
  `e_terrain_lod_ratio` and screen error. The crate's height/normal queries always use LOD0, while
  the original query uses whatever heightmap geometry is resident. Near the player (LOD0) they
  agree; at 400+ m a remote unit may be drawn on a coarser mesh than it stands on. This is the one
  terrain "LOD switch that changes gameplay visibility"; low phase-1 impact because the player and
  the mount are always in LOD0.
* **Entities (server units, doodads, vehicles): differs.** The original culls each entity by its
  bbox and `ViewDistRatio` (special-marked actors use the 100 custom ratio); the crate hard-caps
  units at 140 m and doodads at 160 m and animates only within 60 m. Depending on the actor's
  `ViewDistRatio` (not located, [?]) the original may cull players around 30–120 m or keep them to
  the far plane. The mechanism is different even where the numbers happen to be close: a large
  mount or a wagon gets a longer native distance, a small critter a shorter one; the crate uses one
  radius for all. This is a phase-1 visibility difference (other riders/players at >140 m vanish),
  but its size depends on the unknown actor ratio.
* **Entity LOD / animation LOD:** the crate animates at 30/15 Hz beyond 25/45 m (`units.rs:56-58`)
  and hides remote unit shadows beyond 45 m. The original uses animation LOD tasks
  (`ca_Lod*`, `ca_ChrBaseLOD`) and shadow view ratios. Perf-only for feel, not gameplay.
* **Collision streaming (150 m)** is a separate visibility limit: beyond it the world has terrain
  only for the mover/camera (§2.3).
* **Port status:** `aa-client` now computes the distance of each remote unit and doodad with
  `FUN_3207DAC0` (`cryentitysystem.dll`, image base `0x32000000`; the function is virtual on the
  render proxy, the audit calls it `CEntity::GetViewDistance`): a unit is a special actor (flag
  `0x400`) at `ViewDistRatio` 100 over a humanoid bbox at its scale (180 m at scale 1, more for a
  large mount or wagon); a doodad is an ordinary entity at `ViewDistRatio` 30 held by
  `e_view_dist_doodad_min` 64 m. The five cvars are the live values (0/64/60/30/100). Twenty cases
  executed from the shipped function itself (Unicorn; `tools/native_view_distance_oracle.py` ->
  `crates/aa-client/src/port/world/goldens/view_distance.json`) are the golden. The exact actor
  `ViewDistRatio` and the `0x400` flag could not be read from a live entity (the entity system
  returns a script/render proxy object this build, not the `CEntity`/`CRenderProxy` with the
  fields); the policy is the one recommended in §6.5 and is applied per unit scale.

### 6.4 How to call the originals in-process

* Static view distance: `CStatObj`/render-node function `0x30019FF0` is a leaf; calling it needs a
  live render node. Easier to compare the crate's compiled `maxViewDist × 1.5` with the value the
  original stores in the render node (render-node `+0x2C`) after streaming a cell — read it with
  the object manager, no call.
* Entity view distance: `0x3207DAC0` takes an `IEntity*`; iterate `IPhysicalWorld`/`IEntitySystem`
  entities around the player and call it. `CEntity` layout fields used: bbox via `vt+0x6C`, flags
  `+0x25` bit 0x400, `ViewDistRatio` byte `+0x15`.
* Streaming priority: not a clean leaf (needs a loaded level and the object manager). Use the
  render-node `maxViewDist` read above instead.
* A practical differential for units: spawn the real client with a second character at 50, 100,
  150, 200, 400 m and record whether it is rendered (screenshot) — no code call needed. The crate
  side answers `UNIT_RADIUS`/`DRAW_RADIUS` directly.

### 6.5 Port order (streaming)

1. No port needed for brushes/vegetation; keep the native LOD formula as the oracle test.
2. Decide the remote-unit policy with a real-client capture at known distances; if parity matters,
   replace the 140/160 constants with `CEntity::GetViewDistance` using the actor/prefab
   `ViewDistRatio` (read from the entity template, default 30) and the 100/60 ratios. This is the
   only phase-1 visibility change with a user-visible effect.
3. Terrain LOD thresholds are rendering; leave them with `aa-world`'s renderer owner.

---

## 7. Ranked differences, phase 1 first

### 7.1 Phase-1 critical functions in full

| function | module | addr | size | callers | callees | struct fields / globals | pure? | in-process call |
|---|---|---:|---:|---|---|---|---|---|
| `GetTerrainElevation` | cry3dengine | `0x30121A70` | 69 B | `I3DEngine` slot `+0x1FC` (30 x2game sites) | `FUN_300C91B0` | reads `DAT_302DCD2C` (terrain), `DAT_3029066*` | yes | `vt(i3d,0x1FC)(x,y,0)` |
| height sample | cry3dengine | `0x300C91B0` | 665 B | 29 | `FUN_300C8B60` ×4, `FUN_301EA870` (flag) | terrain node `+0,+1,+2,+4,+5,+0x12`; voxel `DAT_302DCD3C+0x5C30` | resident state | direct `__thiscall (terrain, x, y, 0)` |
| heightmap corners | cry3dengine | `0x300C8B60` | 984 B | 3 | — | node as above | yes | not needed (covered by 300C91B0) |
| `GetTerrainSurfaceNormal` | cry3dengine | `0x30124960` | 123 B | slot `+0x208`; `CVegetation::CalcMatrix` | `vt+0x20C` (unit=2), `FUN_300C23D0` | radius = 2 × 0.5; `DAT_3024C838` = 0.05 | yes | `vt(i3d,0x208)(out, x,y,z)` |
| terrain normal | cry3dengine | `0x300C23D0` | 699 B | 8 | `FUN_300C91B0` ×4 | globals 0.05 / 1.5 / 0.5 | yes | via `0x30124960` |
| `GetWaterLevel` | cry3dengine | `0x3012B2E0` | 1,644 B | slot `+0x110` (15 x2game sites) | `vt+0x114`, physics `vt+0x158` | pos, optional water-body out, terrain +0xDF0 | stateful | `vt(i3d,0x110)(pos,0,0,0,0)` |
| ocean level | cry3dengine | `0x30127320` | 487 B | `0x3012B2E0`, slot `+0x114` | `FUN_300E3250` | `DAT_302F7090`, terrain `+0xDF0`, `DAT_302DCD74` | cached | `vt(i3d,0x114)(pos, 1)` |
| ocean waves | cry3dengine | `0x300E3250` | 1,981 B | `0x30127320` | — | `DAT_302DCD3C+0x304/0x308`, wave clock | time state | not a leaf (needs ocean state) |
| `CStatObj::Physicalize` | cry3dengine | `0x30030F20` | 2,175 B | `IStatObj` vtable `0x30255EC4` | physics `gEnv+0x38` | CGF physics blob, flags `+0x38`, surfaces | stateful | vtable call with stat obj |
| `CVegetation::CalcMatrix` | cry3dengine | `0x300F4390` | 1,171 B | vegetation loader | `FUN_300C23D0` when aligning | group `+0x28` bit 8; instance `+0x12/+0x13/+0xF..0x11` | yes | `(instance, out[12], flags*)` |
| `CEntity::GetViewDistance` | cryentitysystem | `0x3207DAC0` | 1,037 B | virtual | — | bbox `vt+0x6C`, flags `+0x25`, ratio `+0x15` | yes | `(entity)` |

### 7.2 Ranked differences (phase 1 first)

1. **P1 — collision response is a reimplementation, not CryPhysics.** The mover slides on a
   triangle soup; box/cylinder/capsule proxies are tessellated and there is no continuous body
   sweep or solver (`§2.3`). On-foot and mounted contact with walls, stairs, rocks, bridges and
   tree trunks can differ in contact point/normal and in step-up/penetration behaviour. The data
   (`aa-cry::physics`) is already correct; the gap is the runtime.
2. **P1 — ground normal used for alignment differs.** The crate returns the triangle face normal;
   the original returns the four-sample `GetTerrainSurfaceNormal` (radius 1.05 m, a slightly
   asymmetric sample set). This feeds camera ground tilt, character/mount alignment and
   vegetation tilt (`§1.3`). Height itself is verified identical.
3. **P1 (medium) — who is visible around the rider.** Units are hard-capped at 140 m (drawn),
   60 m (animated); doodads at 160 m; the original computes each entity's distance from its bbox
   and `ViewDistRatio` with separate ratios for actors (100) and ordinary objects (60). The
   direction of the difference depends on the actor ratio, which is not located [?] (`§6.3`).
4. **P1 (low) — collision coverage radius 150 m.** Beyond it the mover/camera rays see terrain only;
   the original physicalizes streamed objects over the object streaming range (`§2.3`). Matters for
   long camera arms, long shots and placement, not for normal riding.
5. **P1 (low) — water query is flat and has a floor test.** The original adds the animated ocean
   wave to the query and returns a volume plane at any depth; the crate returns a flat sea above a
   floor (`§4.3`). Swim/wade depth and boat surface can differ; on-foot riding near a shore mostly
   unaffected.
6. **P1 (low, visual + proxy) — `bAlignToTerrain` ignored for vegetation.** 737/2,745 groups tilt to
   the terrain normal in the original; the crate keeps them upright (`§2.3`).
7. **P3 — vis areas/portals are not implemented** (no indoor/outdoor, area flags), which also
   leaves sound `OutdoorOnly` wrong (`§3.3`).
8. **P3 — housing does not erase vegetation** (`C3DEngine::EraseVegetations`).
9. **P3 — `NoFishing` / `StreamSpeed` / `StreamDir` missing** in the water volume data.
10. **P5 — terrain LOD thresholds are crate-specific** (350/750/1500/2600 m vs the native
    screen-error ratio); rendering-only near the player (`§6.3`).
11. **P5 — animation LOD and shadow radii are fixed** (30/15 Hz, 45 m); performance policy, not
    gameplay.
12. **P5 — the world clock freezes if the server stops** instead of advancing at
    `e_time_of_day_speed` (`§5.3`).

### 7.3 Unknowns (ordered by phase-1 risk)

* **Actor `ViewDistRatio` and the 0x400 "special" flag for players/mounts** — decides whether the
  140 m unit cap is too short or too long. Harness: read `CEntity+0x15` and `+0x25` for a spawned
  player in the hidden client, or capture the real client at 50–400 m.
* **`CVegetation::Physicalize` body** — the profiler string exists (`0x3025FEEA`) but no decompiled
  function and no xref; the physics creation may be inlined into the loader. Its exact flags
  (static vs box, collision-only) are inferred from `CStatObj::Physicalize` + `Dephysicalize`.
* **`GetBottomLevel` (`0x301270F0`) semantics** — read only the prologue; it is used by x2game's
  camera/placement but not by the six audited areas. Map it before touching the camera arm.
* **Terrain LOD resident level during a height query** — the original can return a coarser sample
  after `ReleaseHeightMapGeometry`; the crate always LOD0. Not measured at distance.
* **Water query floor behaviour** — whether the physics water planes are bounded in Z by the volume
  (our reading of `0x3012B2E0` says no Z test); verify with a cave-under-lake position.
* **Ocean level agreement between `world.xml` and the heightmap header** for every world; only
  `main_world` (100 m) was checked.


---

## 8. Size of the work

Sizes are `funcs.csv` `size` sums for the functions named in this audit (not whole address bands);
"KB" = 1,024 bytes. "Map in full" is the decompile that has to be read to be sure of the area;
"parity delta" is the part that is actually different and would be ported/fixed.

### (a) What phase 1 needs (movement / riding)

| group | functions | bytes | KB |
|---|---:|---:|---:|
| terrain height + normal (incl. `RayTrace`, `IsPointUnderGround`, `GetBottomLevel`) | 10 | 7,308 | 7.1 |
| brush/vegetation collision data + runtime | 10 | 12,847 | 12.5 |
| water query + waves + volume loader | 5 | 8,071 | 7.9 |
| **cry3dengine total** | **25** | **28,226** | **27.6** |
| `CEntity::GetViewDistance` (cryentitysystem) | 1 | 1,037 | 1.0 |
| **phase-1 total to map in full** | **26** | **29,263** | **28.6** |

The unknown `CVegetation::Physicalize` body (profiler string only, no decompiled function) is not
in the count; estimate 0.5–1 KB. Actual **parity delta** inside that set: `0x30124960` + `0x300C23D0` (822 B), the `bAlignToTerrain`
branch of `0x300F4390` (whole function 1,171 B), `0x30127320` + `0x300E3250` (2,468 B) and
`0x3207DAC0` (1,037 B) = **5,498 B / 5.4 KB / 6 functions**, plus the runtime collision integration
which is work in `aa-client`/`aaemu-physics`, not decompile. Height, brush/vegetation data, the
water volume parser and the static-object view distances need no new port.

### (b) The rest of the six audited areas

| group | functions | bytes | KB |
|---|---:|---:|---:|
| vis areas / portals / area queries | 13 | 9,480 | 9.3 |
| streaming / LOD visibility (CObjManager, vegetation, stat LOD) | 13 | 9,928 | 9.7 |
| time of day manager | 10 | 12,844 | 12.5 |
| **cry3dengine subtotal** | **36** | **32,252** | **31.5** |
| x2game prefab loaders (`WaterVolume`, `VisArea`, `HousingBase::EraseVegetation`) | 3 | 3,050 | 3.0 |
| **rest total** | **39** | **35,302** | **34.5** |

Of that rest, the **parity delta** is small: the vis-area query/creation set (13 funcs, 9.3 KB) is
*wanted only for P3/P5*; the TOD set needs nothing for gameplay; the streaming set is mostly
"already matched" (only the entity radius policy is a real change). The housing vegetation erase
needs `0x301422C0` + `0x301418A0` (835 B) + the x2game caller (892 B).

### Against the survey's 1,783 KB "covered" claim

| bucket | KB | share of 1,783 KB |
|---|---:|---:|
| six audited gameplay areas, all functions named here | 60.1 | 3.4 % |
| plus x2game prefab loaders | 3.0 | 0.2 % |
| **audited gameplay subset** | **63.1** | **3.5 %** |
| not re-audited here: mesh/material/CGF (aa-cry), particles/decals (aa-fx), terrain rendering, sky rendering, voxel/visarea culling | ~1,720 | 96.5 % |

So the "1,783 KB covered" number is not wrong as a *coverage* claim, but only ~63 KB of it is
gameplay logic of the kind this audit checks, and the real differences are concentrated in
**six functions / ~5.4 KB** (normal sampling, vegetation alignment, water waves, entity view
distance) plus the architectural choice of using a custom triangle collision runtime instead of
the already-ported CryPhysics.

### Verdict summary by area

| area | data | query/logic | phase |
|---|---|---|---|
| terrain height | same (verified to 1e-4 m) | same on LOD0; `None` vs 0 at the API edge | P1 |
| terrain normal | same samples available | **differs** (face normal vs 4-sample 1.05 m normal) | P1 |
| vegetation/brush collision | same records and physics blobs | **differs** (custom tri-soup vs CryPhysics; 150 m coverage; `bAlignToTerrain` ignored) | P1 |
| areas/volumes | visareas parsed only for contents; ToD/sound areas parsed | **vis areas not implemented**; ToD same; sound `OutdoorOnly` wrong | P3/P5 |
| water | same object.dat records | **differs** (waves, floor, NoFishing/stream) | P1 low / P3 / P4 |
| time of day | same server hour, same warp/sun | same (clock freeze when disconnected; moon visual) | P5 |
| streaming/LOD | same for brushes/vegetation/static brushes | **entities differ** (fixed 140/160 m caps); terrain LOD crate-specific | P1 medium / P5 |


### 0.4 One-line call chains from x2game to the visible/gameplay result

```
ground:   x2game CActor/placement/swim -> gEnv+0x2C (I3DEngine) -> vt+0x1FC -> CTerrain:300C91B0
          -> 4 x CHeightMap:300C8B60 -> height -> mover/camera/placement
normal:   CVegetation::CalcMatrix / decal / alignment -> vt+0x208 -> CTerrain:300C23D0
          -> 4 heights at 1.05 m -> normal -> object orientation / camera tilt
collide:  streamer -> CObjManager::_LoadBrush 301F2CA0 / _LoadVegetation 301F5A90
          -> CStatObj::Physicalize 30030F20 -> CryPhysics static entity -> mover contacts
areas:    VisAreaPrefabObject::Load 39110770 -> I3DEngine vt+0x2E4 -> CVisAreaManager::CreateVisArea 301712C0
          -> GetVisAreaFromPos 301747E0 / SetEntityArea 30172E50 -> indoor flag, fog, sound
water:    _LoadWaterVolume 301F6AB0 -> CWaterVolumeRenderNode; GetWaterLevel 3012B2E0
          -> GetOceanWaterLevel 30127320 + waves 300E3250 + physics planes -> swim/wade/boat height
time:     server packet -> SetTimeOfDay 39488A00 -> TimeOfDayManager::SetTime 300A46B0 -> warp 300A49D0
          -> CTimeOfDay::Update 300A4C10 -> sky/sun/fog/HUD
visible:  server entity -> CEntity -> CEntity::GetViewDistance 3207DAC0 -> culler -> drawn/culled
```
