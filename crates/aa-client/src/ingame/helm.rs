//! The steering gear of a land vehicle (the Farm Wagon, `docs/research/re-wagons.md` 2): the helm model sits on the front axle
//! assembly next to the rider's grip bones `$IK_Grip_L/R`, and `Bone_spindle` (`use_center_spindle`: `UpdateCenterSpindle`,
//! `FUN_390cffd0`) turns with the smoothed steering. The steering is read from the heading's rate of turn, which is the same for
//! the driven vehicle and for the ones the server moves.

use aa_anim::Animator;
use aa_cry::scene::{CryAssets, ModelOptions, Shading, spawn_loaded};
use bevy::prelude::*;

use super::data::Db;
use super::units::RemoteUnit;

/// The bone that turns with the steering.
const SPINDLE: &str = "Bone_spindle";
/// The bone the helm is put on: `$helms` when the skeleton has it, else the bone the rider's grip bones hang on (the
/// wagon's skeleton has no `$helms`).
const HELM_BONES: [&str; 2] = ["$helms", "Bone_Front_01"];
/// The grip bones of the rider's hands; the helm sits between them.
const GRIPS: [&str; 2] = ["$IK_Grip_L", "$IK_Grip_R"];
/// The wheel angle at full lock (rad); the original turns by about half a turn scale (`DAT_3999edbc`), inferred.
const FULL_LOCK: f32 = 0.5;
/// How fast the smoothed steering follows its target (1/s): `steer += (target - steer) * min(dt * rate, 1)`.
const FOLLOW_RATE: f32 = 8.0;

/// The helm model of a body: `<name>_body.chr` is accompanied by `<name>_hlems.cgf` (the data has no link, the model of
/// `$helms` is by file name).
pub fn helm_model(body: &str) -> Option<String> {
    let stem = body.rsplit_once("_body.")?.0;
    Some(format!("{stem}_hlems.cgf"))
}

/// A vehicle with a steering spindle.
#[derive(Component)]
pub struct Steering {
    spindle: Entity,
    /// The spindle's own rotation in the bind pose; the steering turns about the vertical (cry z) on top of it.
    base: Quat,
    /// Radians per second of full steering (`vehicle_models.angVel`).
    turn: f32,
    last_facing: Option<f32>,
    /// The smoothed steering, -1 (right) .. 1 (left).
    pub value: f32,
}

/// The steering the heading's rate of turn asks for, -1..1 (left positive).
pub fn steering_target(delta_facing: f32, dt: f32, turn_rate: f32) -> f32 {
    if dt <= 0.0 || turn_rate <= 0.0 {
        return 0.0;
    }
    (delta_facing / dt / turn_rate).clamp(-1.0, 1.0)
}

/// The spindle's rotation at steering `value` (-1..1) on top of `base`.
pub fn spindle_rotation(base: Quat, value: f32) -> Quat {
    base * Quat::from_rotation_z(value * FULL_LOCK)
}

pub fn plugin(app: &mut App) {
    app.add_systems(PostUpdate, (dress_steering, steer).chain().after(aa_anim::animator::animate).before(TransformSystems::Propagate));
}

fn dress_steering(mut commands: Commands, mut cry: CryAssets, db: Option<Res<Db>>, units: Query<(Entity, &RemoteUnit, &Animator), Without<Steering>>, transforms: Query<&Transform>) {
    let Some(db) = db else { return };
    for (entity, unit, animator) in &units {
        if !db.is_land_vehicle(unit.model_id) {
            continue;
        }
        let bone = |name: &str| animator.rig.skeleton.bones.iter().position(|b| b.name.eq_ignore_ascii_case(name)).and_then(|i| animator.bones.get(i).copied());
        let Some(spindle) = bone(SPINDLE) else {
            commands.entity(entity).insert(Steering { spindle: Entity::PLACEHOLDER, base: Quat::IDENTITY, turn: 0.0, last_facing: None, value: 0.0 });
            continue;
        };
        let base = transforms.get(spindle).map_or(Quat::IDENTITY, |t| t.rotation);
        if let Some(file) = db.actor_model_file(unit.model_id) {
            attach_helm(&mut commands, &mut cry, animator, &file);
        }
        commands.entity(entity).insert(Steering { spindle, base, turn: db.vehicle_body(unit.model_id).turn, last_facing: None, value: 0.0 });
    }
}

/// The bind pose between the two grip bones (their parent's frame): the position the helm is held at.
fn grips_midpoint(animator: &Animator) -> Option<Transform> {
    let bind = |name: &str| animator.rig.skeleton.bones.iter().find(|b| b.name.eq_ignore_ascii_case(name)).map(|b| b.bind_local);
    let (l, r) = (bind(GRIPS[0])?, bind(GRIPS[1])?);
    Some(Transform::from_translation(Vec3::from((l.translation + r.translation) * 0.5)))
}

/// Puts the helm model of the body `file` on the front axle bone of `animator`'s skeleton.
pub fn attach_helm(commands: &mut Commands, cry: &mut CryAssets, animator: &Animator, file: &str) {
    let Some(path) = helm_model(file) else { return };
    debug!("helm {path} exists {}", aa_pak::Pak::global().exists(&path));
    if !aa_pak::Pak::global().exists(&path) {
        return;
    }
    let Some(anchor) = HELM_BONES.iter().find_map(|name| animator.rig.skeleton.bones.iter().position(|b| b.name.eq_ignore_ascii_case(name)).and_then(|i| animator.bones.get(i).copied())) else { return };
    // `$helms` itself: its own place. Otherwise the middle of the grips, in the frame of their parent bone.
    let local = if animator.rig.skeleton.bones.iter().any(|b| b.name.eq_ignore_ascii_case("$helms")) { Transform::default() } else { grips_midpoint(animator).unwrap_or_default() };
    let options = ModelOptions { cry_space: true, shading: Shading::Full, ..default() };
    if let Ok(loaded) = cry.load_model(&path, &options) {
        debug!("helm bboxes {:?} anchor {anchor:?}", loaded.model.meshes.iter().map(|m| (m.bbox_min, m.bbox_max)).collect::<Vec<_>>());
        let model = spawn_loaded(commands, &path, &loaded, &options);
        commands.entity(model).insert((local, ChildOf(anchor)));
    } else {
        debug!("helm {path} cannot be loaded");
    }
}

fn steer(time: Res<Time>, mut vehicles: Query<(&RemoteUnit, &mut Steering)>, mut transforms: Query<&mut Transform>) {
    let dt = time.delta_secs();
    for (unit, mut steering) in &mut vehicles {
        if steering.spindle == Entity::PLACEHOLDER {
            continue;
        }
        let delta = steering.last_facing.map_or(0.0, |last| {
            let d = (unit.facing - last + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
            d
        });
        steering.last_facing = Some(unit.facing);
        if dt > 0.0 {
            let target = steering_target(delta, dt, steering.turn);
            steering.value += (target - steering.value) * (dt * FOLLOW_RATE).min(1.0);
        }
        if let Ok(mut t) = transforms.get_mut(steering.spindle) {
            t.rotation = spindle_rotation(steering.base, steering.value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_helm_model_is_found_by_the_body_name() {
        assert_eq!(helm_model("objects/env/06_unit/05_carriage/transfers_trailer_a_body.chr").as_deref(), Some("objects/env/06_unit/05_carriage/transfers_trailer_a_hlems.cgf"));
        assert_eq!(helm_model("objects/x/horse.cdf"), None);
    }

    #[test]
    fn steering_follows_the_rate_of_turn_and_saturates() {
        assert_eq!(steering_target(0.0, 0.1, 0.4), 0.0);
        // 0.02 rad in 0.1 s is 0.2 rad/s: half of the wagon's 0.4 rad/s
        assert!((steering_target(0.02, 0.1, 0.4) - 0.5).abs() < 1e-6);
        assert_eq!(steering_target(1.0, 0.1, 0.4), 1.0);
        assert_eq!(steering_target(-1.0, 0.1, 0.4), -1.0);
        assert_eq!(steering_target(0.1, 0.0, 0.4), 0.0);
    }

    #[test]
    fn the_spindle_turns_about_the_vertical_on_its_bind_rotation() {
        let base = Quat::from_rotation_x(0.3);
        assert!(spindle_rotation(base, 0.0).abs_diff_eq(base, 1e-6));
        let full = spindle_rotation(base, 1.0);
        let (axis, angle) = (base.inverse() * full).to_axis_angle();
        assert!((angle - FULL_LOCK).abs() < 1e-5 && axis.abs_diff_eq(Vec3::Z, 1e-5));
    }

    #[test]
    fn the_smoothed_steering_turns_the_spindle_while_the_vehicle_turns() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_millis(100))).add_systems(Update, steer);
        let base = Quat::from_rotation_x(0.3);
        let spindle = app.world_mut().spawn(Transform::from_rotation(base)).id();
        let vehicle = app.world_mut().spawn((RemoteUnit::test_character(1), Steering { spindle, base, turn: 0.4, last_facing: None, value: 0.0 })).id();
        app.update();
        app.update();
        // turning left at the full rate: 0.04 rad per 0.1 s
        for i in 1..=30 {
            app.world_mut().get_mut::<RemoteUnit>(vehicle).unwrap().facing = 0.04 * i as f32;
            app.update();
        }
        let value = app.world().get::<Steering>(vehicle).unwrap().value;
        assert!(value > 0.9, "{value}");
        assert!(!app.world().get::<Transform>(spindle).unwrap().rotation.abs_diff_eq(base, 0.05), "the spindle turned");
        // straight again: it comes back
        for _ in 0..40 {
            app.update();
        }
        assert!(app.world().get::<Steering>(vehicle).unwrap().value.abs() < 0.05);
    }
}
