//! A thin client-side owner of one [`aaemu_physics::world::World`]: the CryPhysics engine the
//! server already uses, with the rules a renderer needs on top.
//!
//! `aaemu-physics` is the server's bit-faithful port of `cryphysics_dedicate.dll` (x64
//! 10.0.2.13); the 1.2 client's `cryphysics.dll` is the same engine generation
//! (the server repository's `rust/docs/physics/CRYPHYSICS-MAP.md` §9b: 15 oracle scenarios are
//! bit-identical, the rest differ by 1-6 ulp). This crate adds:
//!
//! * **Fixed steps.** The server's ship physics ticks at `target_physics_tps = 25`
//!   (`aaemu-game/src/systems/ai/physics/mod.rs`, and `physics/cry.rs` calls `World::step(0.04)`),
//!   so [`PhysicsWorld::advance`] turns variable frame times into whole [`FIXED_DT`] steps and
//!   nothing else. The simulation therefore does not depend on the frame rate.
//! * **One thread.** The port uses `thread_local!` caches (geometry/collision scratch) and a
//!   per-caller `World::tls_thread`; a world must be created, stepped and queried on one thread
//!   (in Bevy: a resource stepped in `FixedUpdate`, never touched from parallel systems).
//! * **1.2 defaults.** The 1.2 rigid ctor leaves its damping (`E+480`) at 0 (the 10.x server
//!   ctor uses 0.1; §9b "Buoyancy" of the server's CRYPHYSICS-MAP), so
//!   [`PhysicsWorld::add_rigid_box`] pins 0 explicitly. A scenario that wants damping has to set
//!   it.
//!
//! The crate has no Bevy dependency; wiring it into a plugin is client work. `docs/aa-physics.md`
//! records the rest of the engine differences a later slice must respect.

use aaemu_physics::entities::interface::set::set_params_simulation;
use aaemu_physics::geometry::primitives::{BoxPrim, Primitive};
use aaemu_physics::math::{Quat, Vec3};
use aaemu_physics::params::ParamsSimulation;
use aaemu_physics::world::api::PartDesc;
use aaemu_physics::world::{EntIdx, World};

/// One step of the fixed clock: the server's `target_physics_tps = 25` (0.04 s).
pub const FIXED_DT: f32 = 1.0 / 25.0;

/// The oracle scenario tick (`tests/scenarios`, `step n= dt=h:3d088889`), used to replay the
/// CryPhysics goldens; the 1.2 client oracle runs the same scenarios at this tick.
pub const ORACLE_DT: f32 = f32::from_bits(0x3d08_8889);

/// The longest frame time [`PhysicsWorld::advance`] accepts at once (25 steps); a longer stall
/// is dropped instead of replayed, so a debugger pause cannot freeze the caller in a catch-up
/// loop.
pub const MAX_FRAME_DT: f32 = 1.0;

/// The global water plane's density and resistance in the server's collision world
/// (`PhysWorld::new` calls `set_ocean(ocean_level, 1000.0, 1000.0)`).
const OCEAN_DENSITY: f32 = 1000.0;
const OCEAN_RESISTANCE: f32 = 1000.0;

/// A handle to an entity of a [`PhysicsWorld`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Body(EntIdx);

/// A body's pose and velocity after the last step (`pe_status_pos` + `pe_status_dynamics`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyState {
    /// World position (z up), metres.
    pub pos: [f32; 3],
    /// Orientation as `(x, y, z, w)`, the engine's quaternion order.
    pub rot: [f32; 4],
    /// Linear velocity, m/s.
    pub lin_vel: [f32; 3],
    /// Angular velocity, rad/s.
    pub ang_vel: [f32; 3],
    /// `false` once the engine put the body to sleep (its velocities read as zero).
    pub awake: bool,
}

/// One physics world and its fixed-step accumulator.
pub struct PhysicsWorld {
    world: World,
    fixed_dt: f32,
    accumulator: f64,
}

impl PhysicsWorld {
    /// An empty world with a `cells × cells` entity grid of `cell_size` metres starting at
    /// `origin` (z up), stepped at the server's [`FIXED_DT`]. The grid bounds the broad phase:
    /// put everything simulated inside it.
    #[must_use]
    pub fn new(origin: [f32; 3], cells: i32, cell_size: f32) -> Self {
        Self::with_fixed_dt(origin, cells, cell_size, FIXED_DT)
    }

    /// [`PhysicsWorld::new`] with another fixed tick. Gameplay uses [`FIXED_DT`]; this exists for
    /// replaying the engine's oracle scenarios, which step at [`ORACLE_DT`], and for callers that
    /// already run another fixed clock (e.g. 1/60 s).
    #[must_use]
    pub fn with_fixed_dt(origin: [f32; 3], cells: i32, cell_size: f32, fixed_dt: f32) -> Self {
        assert!(
            fixed_dt.is_finite() && fixed_dt > 0.0,
            "fixed dt must be finite and positive"
        );
        Self {
            world: World::with_grid(Vec3::from_array(origin), cells, cell_size),
            fixed_dt,
            accumulator: 0.0,
        }
    }

    /// Make the global water plane sit at `level` with the server's density and resistance
    /// (1000 / 1000). Bodies float once [`PhysicsWorld::set_body_buoyancy`] gives them a
    /// density/resistance; rigid bodies default to the engine's 0.001 multipliers.
    pub fn set_ocean(&mut self, level: f32) {
        self.world.set_ocean(level, OCEAN_DENSITY, OCEAN_RESISTANCE);
    }

    /// A body's buoyancy response relative to the ocean's (`pe_params_buoyancy` on the entity).
    pub fn set_body_buoyancy(&mut self, body: Body, density: f32, resistance: f32) {
        self.world.set_body_buoyancy(body.0, density, resistance);
    }

    /// A static (immovable) box: `pos` is the centre, `rot` `(x, y, z, w)`, `half_extents` the
    /// half-size on each axis. Returns `None` when the geometry cannot be registered.
    pub fn add_static_box(
        &mut self,
        pos: [f32; 3],
        rot: [f32; 4],
        half_extents: [f32; 3],
    ) -> Option<Body> {
        let geom = self.world.geometry(&box_prim(half_extents))?;
        self.world
            .add_static(
                Vec3::from_array(pos),
                Quat::from_array(rot),
                &[PartDesc::new(geom, 0.0)],
                -1,
            )
            .map(Body)
    }

    /// A dynamic box (rigid body) with the 1.2 client's rigid defaults.
    ///
    /// `mass` is the whole body's mass in kg. Damping is set to 0 explicitly (the 1.2 ctor's
    /// value; see the module docs), free-fall damping keeps the engine's 0.1.
    pub fn add_rigid_box(
        &mut self,
        pos: [f32; 3],
        rot: [f32; 4],
        half_extents: [f32; 3],
        mass: f32,
    ) -> Option<Body> {
        let geom = self.world.geometry(&box_prim(half_extents))?;
        let e = self.world.add_rigid(
            Vec3::from_array(pos),
            Quat::from_array(rot),
            &[PartDesc::new(geom, mass)],
            -1,
        )?;
        set_params_simulation(
            &mut self.world,
            e,
            &ParamsSimulation {
                damping: 0.0,
                ..ParamsSimulation::default()
            },
        );
        Some(Body(e))
    }

    /// Adds `frame_dt` seconds to the accumulator and runs every whole fixed step it holds;
    /// returns how many steps ran. Non-finite or negative deltas are ignored, and a delta longer
    /// than [`MAX_FRAME_DT`] is clamped (the excess is dropped).
    pub fn advance(&mut self, frame_dt: f32) -> u32 {
        if !frame_dt.is_finite() || frame_dt <= 0.0 {
            return 0;
        }
        self.accumulator += f64::from(frame_dt.min(MAX_FRAME_DT));
        let step = f64::from(self.fixed_dt);
        let mut steps = 0;
        while self.accumulator >= step {
            self.world.step(self.fixed_dt);
            self.accumulator -= step;
            steps += 1;
        }
        steps
    }

    /// The body's pose and velocity now.
    #[must_use]
    pub fn body_state(&self, body: Body) -> BodyState {
        let s = self.world.body_state(body.0);
        BodyState {
            pos: s.pos.to_array(),
            rot: s.q.to_array(),
            lin_vel: s.v.to_array(),
            ang_vel: s.w.to_array(),
            awake: s.awake,
        }
    }
}

fn box_prim(half_extents: [f32; 3]) -> Primitive {
    Primitive::Box(BoxPrim {
        size: Vec3::from_array(half_extents),
        ..BoxPrim::default()
    })
}
