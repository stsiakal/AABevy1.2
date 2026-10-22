//! The Farm Wagon (slave model 1008) as the client draws it: the body `.chr` of its prefab with the wheel models on the
//! `wheel0/1` and `wheel_type20/21` bones. `WAGON_MODEL=<models.id>` (default 1008), `WAGON_VIEW=side|front|top|back`,
//! `WAGON_SPIN=<rad>` rolls the wheels by that angle, `WAGON_EYE=x,y,z` / `WAGON_AT=x,y,z` place the camera.
//! `cargo run -p aa-client --example wagon -- --offscreen-shot shots/wagon.png --mute`
#![allow(dead_code)]
#[path = "../src/script/mod.rs"]
mod script;
#[path = "../src/hud/mod.rs"]
mod hud;
#[path = "../src/ingame/mod.rs"]
mod ingame;
#[path = "../src/login/mod.rs"]
mod login;
#[path = "../src/settings.rs"]
mod settings;
#[path = "../src/sound/mod.rs"]
mod sound;

use aa_anim::{AnimPlugin, Animator, ClipLibrary};
use aa_cry::scene::{CryAssets, CryPlugin};
use bevy::prelude::*;
use ingame::wheels::{VehicleWheels, Wheel};

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Screen {
    #[default]
    WorldSelect,
    Login,
    CharacterSelect,
    CharacterCreate,
    Loading,
    InGame,
}

fn main() {
    App::new()
        .add_plugins((aa_dev::default_plugins("aa-client: wagon"), aa_dev::DevPlugin, CryPlugin, AnimPlugin))
        .insert_resource(ClearColor(Color::srgb(0.55, 0.7, 0.85)))
        .insert_resource(GlobalAmbientLight { brightness: 900., ..default() })
        .add_systems(Startup, setup)
        .add_systems(Update, (dress, spin, report))
        .run();
}

fn model() -> u32 {
    std::env::var("WAGON_MODEL").ok().and_then(|v| v.parse().ok()).unwrap_or(1008)
}

fn setup(mut commands: Commands, mut cry: CryAssets, clips: Res<ClipLibrary>) {
    let db = ingame::data::Db::open().expect("game database");
    let file = db.actor_model_file(model()).expect("the body file");
    info!("wagon model {} body {file}", model());
    let root = ingame::creature::spawn(&mut commands, &mut cry, &clips, &file, None, Transform::default()).expect("the body spawns");
    commands.entity(root).insert(Name::new("wagon"));
    commands.spawn((DirectionalLight { illuminance: 12000., shadow_maps_enabled: false, ..default() }, Transform::from_xyz(8., 14., 10.).looking_at(Vec3::ZERO, Vec3::Y)));
    let view = std::env::var("WAGON_VIEW").unwrap_or_else(|_| "side".into());
    let (eye, up) = match view.as_str() {
        "front" => (Vec3::new(0., 2.5, 8.), Vec3::Y),
        "back" => (Vec3::new(0., 2.5, -8.), Vec3::Y),
        "top" => (Vec3::new(0.1, 10., 0.), Vec3::NEG_Z),
        _ => (Vec3::new(8., 2.5, 0.), Vec3::Y),
    };
    let vec3 = |name: &str| std::env::var(name).ok().and_then(|v| { let p: Vec<f32> = v.split(',').filter_map(|x| x.parse().ok()).collect(); (p.len() == 3).then(|| Vec3::new(p[0], p[1], p[2])) });
    let (eye, at) = (vec3("WAGON_EYE").unwrap_or(eye), vec3("WAGON_AT").unwrap_or(Vec3::new(0., 1.2, 0.)));
    commands.spawn((Camera3d::default(), Projection::Perspective(PerspectiveProjection { fov: 50f32.to_radians(), ..default() }), Transform::from_translation(eye).looking_at(at, up)));
}

#[allow(clippy::type_complexity)]
fn dress(mut commands: Commands, mut cry: CryAssets, bodies: Query<(Entity, &Animator), (Without<VehicleWheels>, Without<Wheel>)>, mut once: Local<Option<ingame::data::Db>>) {
    let db = once.get_or_insert_with(|| ingame::data::Db::open().expect("game database"));
    for (entity, animator) in &bodies {
        let wheels = ingame::wheels::attach_wheels(&mut commands, &mut cry, entity, animator, &db.vehicle_wheel_models(model()));
        ingame::helm::attach_helm(&mut commands, &mut cry, animator, &db.actor_model_file(model()).expect("the body file"));
        info!("wheels: {} attached, radius {:.3}", wheels.count, wheels.radius);
        commands.entity(entity).insert(wheels);
    }
}

fn spin(mut wheels: Query<&mut Transform, With<Wheel>>) {
    let angle: f32 = std::env::var("WAGON_SPIN").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    for mut t in &mut wheels {
        t.rotation = Quat::from_axis_angle(Vec3::X, angle);
    }
}

fn report(bodies: Query<&Animator, With<VehicleWheels>>, xforms: Query<&GlobalTransform>, mut n: Local<u32>) {
    *n += 1;
    if *n != 30 {
        return;
    }
    for animator in &bodies {
        for (i, b) in animator.rig.skeleton.bones.iter().enumerate() {
            if let Ok(g) = animator.bones.get(i).map(|e| xforms.get(*e)).unwrap() {
                info!("bone {} at {:?}", b.name, g.translation());
            }
        }
    }
}
