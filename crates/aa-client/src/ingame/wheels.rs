//! The wheels of a land vehicle (the Farm Wagon): the original's `X2::VehicleUnitModel` loops the bones `wheel0..9` (model
//! `vehicle_models.wheel`) and `wheel_type20..9` (model `wheel2`) of the body `.chr`, puts a wheel object on each and spins it
//! (`docs/research/re-wagons.md` 2). The wheel radius is half the model's extent (`FUN_390d6b10`), so the spin that matches a
//! distance rolled is `distance / radius`.

use aa_anim::Animator;
use aa_cry::scene::{CryAssets, ModelOptions, Shading, spawn_loaded};
use bevy::prelude::*;

use super::data::{Db, VehicleWheelModels};
use super::player::forward;
use super::units::RemoteUnit;

/// Which wheel model a bone takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WheelKind {
    /// `wheel0..9`: `vehicle_models.wheel`.
    Primary,
    /// `wheel_type20..9`: `vehicle_models.wheel2`.
    Secondary,
}

/// The wheel kind of a bone name (`wheelN` / `wheel_type2N`, N a digit), case-insensitive as the client's lookups are.
pub fn wheel_kind(bone: &str) -> Option<WheelKind> {
    let lower = bone.to_ascii_lowercase();
    let digit = |rest: &str| rest.len() == 1 && rest.as_bytes()[0].is_ascii_digit();
    if let Some(rest) = lower.strip_prefix("wheel_type2") {
        return digit(rest).then_some(WheelKind::Secondary);
    }
    lower.strip_prefix("wheel").filter(|r| digit(r)).map(|_| WheelKind::Primary)
}

/// How many wheel bones a skeleton has (what `VehicleMove.wheelCount` carries).
pub fn count_wheel_bones<'a>(bones: impl IntoIterator<Item = &'a str>) -> usize {
    bones.into_iter().filter(|b| wheel_kind(b).is_some()).count()
}

/// The wheel radius from a model's bounding boxes: half the biggest extent (the diameter of a wheel).
pub fn wheel_radius(boxes: impl IntoIterator<Item = (Vec3, Vec3)>) -> f32 {
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for (a, b) in boxes {
        lo = lo.min(a);
        hi = hi.max(b);
    }
    if lo.x > hi.x { 0.0 } else { (hi - lo).max_element() * 0.5 }
}

/// The wheels of a vehicle were put on its bones (or there are none), and how far it has rolled.
#[derive(Component, Default)]
pub struct VehicleWheels {
    /// The radius of the wheels (m), 0 when none was loaded.
    pub radius: f32,
    /// Number of wheels attached.
    pub count: usize,
    /// Distance rolled (m), wrapped to keep the angles small.
    pub rolled: f32,
    last: Option<Vec3>,
}

/// A wheel's anchor under its bone: it carries the spin, the model is its child.
#[derive(Component)]
pub struct Wheel {
    pub owner: Entity,
    /// Half the wheel model's extent (m): its own, the two wheel models differ in size.
    pub radius: f32,
}

/// The axis the wheels turn about in their bone's frame (the axle).
const AXLE: Vec3 = Vec3::X;

pub fn plugin(app: &mut App) {
    app.add_systems(PostUpdate, (dress_wheels, spin_wheels).chain().before(TransformSystems::Propagate));
}

/// Puts the wheel models on the wheel bones of a land vehicle once.
fn dress_wheels(mut commands: Commands, mut cry: CryAssets, db: Option<Res<Db>>, units: Query<(Entity, &RemoteUnit, &Animator), Without<VehicleWheels>>) {
    let Some(db) = db else { return };
    for (entity, unit, animator) in &units {
        if !db.is_land_vehicle(unit.model_id) {
            continue;
        }
        let wheels = attach_wheels(&mut commands, &mut cry, entity, animator, &db.vehicle_wheel_models(unit.model_id));
        commands.entity(entity).insert(wheels);
    }
}

/// Spawns a wheel model under every wheel bone of `animator`'s skeleton.
pub fn attach_wheels(commands: &mut Commands, cry: &mut CryAssets, owner: Entity, animator: &Animator, models: &VehicleWheelModels) -> VehicleWheels {
    let options = ModelOptions { cry_space: true, shading: Shading::Full, ..default() };
    let mut wheels = VehicleWheels::default();
    for (i, bone) in animator.rig.skeleton.bones.iter().enumerate() {
        let Some(kind) = wheel_kind(&bone.name) else { continue };
        let Some(path) = (match kind {
            WheelKind::Primary => models.wheel.as_deref(),
            WheelKind::Secondary => models.wheel2.as_deref(),
        }) else {
            continue;
        };
        let Some(&bone_entity) = animator.bones.get(i) else { continue };
        let Ok(loaded) = cry.load_model(path, &options) else {
            warn!("vehicle wheel {path}: cannot be loaded");
            continue;
        };
        let radius = wheel_radius(loaded.model.meshes.iter().map(|m| (m.bbox_min, m.bbox_max)));
        if wheels.radius == 0.0 {
            wheels.radius = radius;
        }
        let anchor = commands.spawn((Name::new(format!("wheel {}", bone.name)), Wheel { owner, radius }, Transform::default(), Visibility::default(), ChildOf(bone_entity))).id();
        let model = spawn_loaded(commands, path, &loaded, &options);
        commands.entity(model).insert(ChildOf(anchor));
        wheels.count += 1;
    }
    wheels
}

/// Rolls the wheels by the distance the vehicle moved along its heading.
fn spin_wheels(mut vehicles: Query<(Entity, &RemoteUnit, &mut VehicleWheels)>, mut wheels: Query<(&Wheel, &mut Transform)>) {
    for (entity, unit, mut state) in &mut vehicles {
        if state.count == 0 || state.radius <= 0.0 {
            continue;
        }
        let here = unit.pos;
        let step = state.last.map_or(0.0, |last| {
            let d = Vec2::new(here.x - last.x, here.y - last.y);
            if d.length() > 5.0 { 0.0 } else { d.dot(forward(unit.facing)) }
        });
        state.last = Some(here);
        state.rolled = (state.rolled + step).rem_euclid(1000.0);
        for (wheel, mut transform) in &mut wheels {
            if wheel.owner == entity && wheel.radius > 0.0 {
                transform.rotation = Quat::from_axis_angle(AXLE, (state.rolled / wheel.radius).rem_euclid(std::f32::consts::TAU));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bone_names_pick_the_wheel_model() {
        assert_eq!(wheel_kind("wheel0"), Some(WheelKind::Primary));
        assert_eq!(wheel_kind("Wheel9"), Some(WheelKind::Primary));
        assert_eq!(wheel_kind("wheel_type20"), Some(WheelKind::Secondary));
        assert_eq!(wheel_kind("wheel_type21"), Some(WheelKind::Secondary));
        for other in ["wheel", "wheel10", "wheel_type2", "wheel_type210", "Bone_spindle", "susp_hub_0", "wheels0"] {
            assert_eq!(wheel_kind(other), None, "{other}");
        }
    }

    #[test]
    fn the_wagon_skeleton_has_four_wheel_bones() {
        let bones = ["$driver", "Bone_spindle", "wheel0", "wheel1", "wheel_type20", "wheel_type21", "bone_susdummy"];
        assert_eq!(count_wheel_bones(bones), 4);
    }

    #[test]
    fn the_radius_is_half_the_biggest_extent() {
        let boxes = [(Vec3::new(-0.1, -0.5, -0.5), Vec3::new(0.1, 0.5, 0.5))];
        assert!((wheel_radius(boxes) - 0.5).abs() < 1e-6);
        assert_eq!(wheel_radius([]), 0.0);
    }

    #[test]
    fn wheels_roll_with_the_distance_along_the_heading() {
        let mut app = App::new();
        app.add_systems(Update, spin_wheels);
        let mut unit = RemoteUnit::test_character(1);
        unit.pos = Vec3::ZERO;
        let vehicle = app.world_mut().spawn((unit, VehicleWheels { radius: 0.5, count: 1, ..default() })).id();
        let wheel = app.world_mut().spawn((Wheel { owner: vehicle, radius: 0.5 }, Transform::default())).id();
        let small = app.world_mut().spawn((Wheel { owner: vehicle, radius: 0.25 }, Transform::default())).id();
        app.update();
        // 1 m straight ahead (facing 0 looks along +y) rolls 1 / 0.5 rad, the smaller wheel twice as far
        app.world_mut().get_mut::<RemoteUnit>(vehicle).unwrap().pos = Vec3::new(0.0, 1.0, 0.0);
        app.update();
        let angle = |app: &App, e: Entity| app.world().get::<Transform>(e).unwrap().rotation.to_axis_angle().1;
        assert!((angle(&app, wheel) - 2.0).abs() < 1e-4);
        assert!((angle(&app, small) - 4.0).abs() < 1e-4);
        let axis = app.world().get::<Transform>(wheel).unwrap().rotation.to_axis_angle().0;
        assert!(axis.abs_diff_eq(AXLE, 1e-4));
        // backing up rolls back
        app.world_mut().get_mut::<RemoteUnit>(vehicle).unwrap().pos = Vec3::new(0.0, 0.0, 0.0);
        app.update();
        assert!(app.world().get::<VehicleWheels>(vehicle).unwrap().rolled.abs() < 1e-5);
    }
}
