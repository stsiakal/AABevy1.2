# aa-physics: the client's CryPhysics world

`crates/aa-physics` owns one `aaemu_physics::world::World` for the client. `aaemu-physics` is the
server's bit-faithful Rust port of `cryphysics_dedicate.dll` (x64 10.0.2.13); the 1.2 client's
`cryphysics.dll` is the same engine generation: 15 oracle scenarios are bit-identical and the rest
differ by 1-6 ulp (`rust/docs/physics/CRYPHYSICS-MAP.md` §9b and §10 in the server repository).

The dependency is pinned by revision in the root `Cargo.toml`:

```toml
aaemu-physics = { git = "https://github.com/AAEmu/AAEmu-1.2-Rust.git", rev = "b87b6554630e87e142d5e68b76f8c400dc6bb02c" }
```

Cargo finds the package by name in the server workspace (`rust/crates/aaemu-physics`); it pulls in
no other server crate (`cargo tree -p aaemu-physics` prints only itself). To bump: change the rev,
then `cargo update -p aaemu-physics`.

## API (phase 0)

```rust
use aa_physics::{FIXED_DT, PhysicsWorld};

let mut w = PhysicsWorld::new([0.0, 0.0, -100.0], 64, 4.0);
w.add_static_box([32.0, 32.0, -0.5], [0.0, 0.0, 0.0, 1.0], [50.0, 50.0, 0.5]);
let body = w.add_rigid_box([32.0, 32.0, 5.0], [0.0, 0.0, 0.0, 1.0], [0.5, 0.5, 0.5], 10.0).unwrap();
w.set_ocean(0.0);                       // global plane at z = 0, density/resistance 1000
w.set_body_buoyancy(body, 1000.0, 1000.0);
w.advance(frame_time);                  // whole FIXED_DT steps only
let state = w.body_state(body);         // pos, rot (x,y,z,w), lin_vel, ang_vel, awake
```

`BodyState` uses plain arrays, so the crate has no Bevy dependency. Nothing is wired into
`aa-client` yet: this is the phase-0 link (handoff 2026-10-27, research/client-physics-plan §4).

## Rules

* **One thread per world.** The port keeps `thread_local!` caches (geometry/collision scratch) and
  per-caller `World::tls_thread` state. Create, step and query a world on one thread. In Bevy a
  world is a resource stepped in `FixedUpdate`, never touched from parallel systems; a dedicated
  owner thread is also fine.
* **Fixed steps only.** `FIXED_DT = 1/25 s`, the AAEmu server's physics tick
  (`target_physics_tps = 25` in `aaemu-game/src/systems/ai/physics/mod.rs`; `physics/cry.rs` steps
  `World::step(0.04)`). `advance(dt)` accumulates frame time and runs whole steps only. A frame
  longer than `MAX_FRAME_DT` (1 s, 25 steps) is clamped: the excess is dropped, never replayed.
* `with_fixed_dt` exists for replaying the engine's oracle scenarios, which tick at `ORACLE_DT =
  1/30 s` (`0x3d088889`), and for callers that already own another fixed clock. Gameplay uses
  `new`.
* **1.2 defaults.** `add_rigid_box` sets rigid damping to 0 explicitly. The 1.2 ctor leaves
  `E+480` (damping) at 0 while the x64 10.x server ctor uses 0.1 (`CRYPHYSICS-MAP.md` §9b,
  "Buoyancy"); free-fall damping stays 0.1. The port's own rigid ctor already matches 1.2, but
  the wrapper pins it so a later port change cannot silently alter client behaviour.
* Terrain, brushes and water bodies from the client's own pak are not added yet. Heightfield
  terrain, cell residency and CGF physics blobs belong to the phase-2 world builder
  (handoff research, client-physics-plan §3).

## Licence

The client is `GPL-3.0-or-later` since 29cb0262; `aaemu-physics` is `GPL-3.0-or-later` (the server
workspace), so linking is compatible. `aaemu-physics` is derived only from the shipped binaries
(decompiles and disassembly), not from the published CRYENGINE source.

## 1.2 vs 10.x differences to respect

* `pe_params` id 26 does not exist in 1.2; never read it on the client path.
* Car params: the x64-only engine-brake (`+132`) and rear-steer (`+136`) fields are absent in 1.2
  (1.2 car params are 124 B, and the wheel block moves from x64 `+112..183` to 1.2 `+88..159`).
  Leave those fields at their 1.2 defaults in the car slice.
* The 1.2 world is x86 and its structs have different offsets from the x64 port (for example
  `pe_params_pos` is 56 B, the physics-vars block is 540 B). Use the typed Rust API; never feed
  1.2 binary byte images to the port.
* Buoyancy: the 1.2 `global_buoyancy` record is 76 B, not 80 B; the oracle repacks it. Exact-match
  goldens only exist for the documented scenarios; expect 1-6 ulp elsewhere and use tolerances,
  not bit-lockstep.
* The port is the **dedicated server** build; the 1.2 client is the non-dedicated build
  (BACKLOG C8c). The oracle covers the client `cryphysics.dll` too (server PR #14/#16).
* The 1.2 mesh route (`CreateMesh` slot 11) is not in the client oracle; the port's
  `LoadPhysGeometry` is the 10.x route. Mesh collision for cars needs its own check in phase 2.

## Pending live checks

Values below are from the research/servers and are **not yet verified against a live 1.2 client**:

* The 1.2 client's actual `TimeStep` interval while a cart or car is driven. No capture exists; the
  25 Hz here is the AAEmu server's tick. A capture must confirm it, or the client tick must change.
* The cart's tilt mechanism (CryPhysics entity vs a controller ground trace) is unproven.
* The client's `vehicle_models` field to `pe_params_car` / `pe_params_wheel` mapping is inferred.
* Rigid damping 0 is proven by `box_on_ground` / `box_float_300kg` (bit-identical to the 1.2
  oracle), but not yet observed on a live driven vehicle.
