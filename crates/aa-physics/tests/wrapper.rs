//! The wrapper reproduces the CryPhysics goldens and steps only in whole fixed ticks.

use aa_physics::{Body, BodyState, FIXED_DT, ORACLE_DT, PhysicsWorld};

const IDENTITY: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

/// The 1.2/x64 golden rest pose of `box_on_ground` (0x3EF62457, 0.480746).
const REST_Z: f32 = f32::from_bits(0x3ef6_2457);
/// The golden float height of `box_float_300kg` (0x400DBD88, 2.214693).
const FLOAT_Z: f32 = f32::from_bits(0x400d_bd88);

/// `box_on_ground.scn` through the wrapper: ground top at 0, 10 kg 1 m cube dropped from z = 5.
fn drop_scene() -> (PhysicsWorld, Body) {
    let mut w = PhysicsWorld::new([0.0, 0.0, -100.0], 64, 4.0);
    w.add_static_box([32.0, 32.0, -0.5], IDENTITY, [50.0, 50.0, 0.5])
        .expect("ground geometry");
    let body = w
        .add_rigid_box([32.0, 32.0, 5.0], IDENTITY, [0.5, 0.5, 0.5], 10.0)
        .expect("box geometry");
    (w, body)
}

#[test]
fn same_scene_stepped_twice_is_bit_identical() {
    let (mut a, ba) = drop_scene();
    let (mut b, bb) = drop_scene();
    for frame in 0..150 {
        let dt = match frame % 3 {
            0 => 0.004,
            1 => 0.013,
            _ => 0.031,
        };
        assert_eq!(a.advance(dt), b.advance(dt), "frame {frame}");
        assert_eq!(
            bits(a.body_state(ba)),
            bits(b.body_state(bb)),
            "frame {frame}"
        );
    }
}

fn bits(s: BodyState) -> ([u32; 3], [u32; 4], [u32; 3], [u32; 3], bool) {
    (
        s.pos.map(f32::to_bits),
        s.rot.map(f32::to_bits),
        s.lin_vel.map(f32::to_bits),
        s.ang_vel.map(f32::to_bits),
        s.awake,
    )
}

#[test]
fn golden_box_on_ground_rest_z() {
    let mut w = PhysicsWorld::with_fixed_dt([0.0, 0.0, -100.0], 64, 4.0, ORACLE_DT);
    w.add_static_box([32.0, 32.0, -0.5], IDENTITY, [50.0, 50.0, 0.5])
        .expect("ground geometry");
    let body = w
        .add_rigid_box([32.0, 32.0, 5.0], IDENTITY, [0.5, 0.5, 0.5], 10.0)
        .expect("box geometry");
    // The scenario's `step n=90`; the box has been asleep since step 85.
    for _ in 0..90 {
        assert_eq!(w.advance(ORACLE_DT), 1);
    }
    let s = w.body_state(body);
    assert!(!s.awake, "the box sleeps at rest");
    assert!(
        (s.pos[2] - REST_Z).abs() <= 1e-6,
        "rest z {} != {REST_Z}",
        s.pos[2]
    );
}

#[test]
fn golden_box_float_300kg_float_z() {
    let mut w = PhysicsWorld::with_fixed_dt([0.0, 0.0, -100.0], 64, 4.0, ORACLE_DT);
    w.add_static_box([32.0, 32.0, -20.0], IDENTITY, [50.0, 50.0, 0.5])
        .expect("ground geometry");
    let body = w
        .add_rigid_box([32.0, 32.0, 5.0], IDENTITY, [0.5, 0.5, 0.5], 300.0)
        .expect("box geometry");
    w.set_ocean(2.0);
    w.set_body_buoyancy(body, 1000.0, 1000.0);
    // The scenario's `step n=300`.
    for _ in 0..300 {
        w.advance(ORACLE_DT);
    }
    let s = w.body_state(body);
    assert!(!s.awake, "the box sleeps at the surface");
    assert!(
        (s.pos[2] - FLOAT_Z).abs() <= 1e-6,
        "float z {} != {FLOAT_Z}",
        s.pos[2]
    );
}

#[test]
fn accumulator_runs_only_whole_fixed_steps() {
    let mut w = PhysicsWorld::new([0.0, 0.0, -100.0], 64, 4.0);
    assert_eq!(w.advance(FIXED_DT), 1);
    let mut total = 1;
    for (dt, want) in [
        (0.03, 0),
        (0.02, 1),
        (0.01, 0),
        (0.02, 1),
        (0.0, 0),
        (0.1, 2),
        (0.02, 1),
        (1.0, 25),
    ] {
        assert_eq!(w.advance(dt), want, "dt {dt}");
        total += want;
    }
    assert_eq!(total, 31);
    // A stall is clamped to MAX_FRAME_DT instead of replaying the whole pause.
    assert_eq!(w.advance(5.0), 25);
    assert_eq!(w.advance(-0.04), 0);
    assert_eq!(w.advance(f32::NAN), 0);
    assert_eq!(w.advance(f32::INFINITY), 0);
}
