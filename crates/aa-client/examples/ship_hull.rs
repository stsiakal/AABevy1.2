//! A ship slave model's prefab brushes (hull, rudder, cannons, cargo) as the client spawns them, seen from the side.
//! `SHIP_MODEL=<models.id>` (default 393, the cutter), `SHIP_VIEW=side|front|top`, `SHIP_OLD=1` for the transform the client
//! used before the hull's cry-space fix (the "before" picture).
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

use aa_cry::scene::{CryAssets, CryPlugin, ModelOptions, spawn_loaded};
use aa_world::coords::{cry_rot_to_bevy, cry_to_bevy};
use bevy::prelude::*;

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
        .add_plugins((aa_dev::default_plugins("aa-client: ship hull"), aa_dev::DevPlugin, CryPlugin))
        .insert_resource(ClearColor(Color::srgb(0.55, 0.7, 0.85)))
        .insert_resource(GlobalAmbientLight { brightness: 900., ..default() })
        .add_systems(Startup, (setup, sea))
        .run();
}

fn setup(mut commands: Commands, mut cry: CryAssets) {
    let model: u32 = std::env::var("SHIP_MODEL").ok().and_then(|v| v.parse().ok()).unwrap_or(393);
    let old = std::env::var("SHIP_OLD").is_ok();
    let view = std::env::var("SHIP_VIEW").unwrap_or_else(|_| "side".into());
    let db = ingame::data::Db::open().expect("game database");
    let parts = db.ship_parts(model);
    info!("ship model {model}: {} brushes", parts.len());
    let root = commands.spawn((Transform::default(), Visibility::default())).id();
    if old {
        let options = ModelOptions::default();
        for part in &parts {
            let Ok(loaded) = cry.load_model(&part.path, &options) else { continue };
            let m = spawn_loaded(&mut commands, &part.path, &loaded, &options);
            let local = Transform { translation: cry_to_bevy(part.pos), rotation: cry_rot_to_bevy(part.rot), scale: Vec3::new(part.scale.x, part.scale.z, part.scale.y) };
            commands.entity(m).insert((local, ChildOf(root)));
        }
    } else {
        ingame::units::spawn_ship_parts(&mut commands, &mut cry, &parts, root);
    }
    commands.spawn((DirectionalLight { illuminance: 12000., shadow_maps_enabled: false, ..default() }, Transform::from_xyz(20., 40., 30.).looking_at(Vec3::ZERO, Vec3::Y)));
    let (eye, at) = match view.as_str() {
        "front" => (Vec3::new(0., 8., -60.), Vec3::new(0., 6., 0.)),
        "top" => (Vec3::new(0.1, 70., 0.), Vec3::ZERO),
        _ => (Vec3::new(40., 8., 0.), Vec3::new(0., 5., 0.)),
    };
    commands.spawn((Camera3d::default(), Projection::Perspective(PerspectiveProjection { fov: 50f32.to_radians(), far: 2000., ..default() }), Transform::from_translation(eye).looking_at(at, if view == "top" { Vec3::NEG_Z } else { Vec3::Y })));
}

fn sea(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.spawn((Mesh3d(meshes.add(Plane3d::default().mesh().size(400., 400.))), MeshMaterial3d(materials.add(Color::srgb(0.1, 0.3, 0.6))), Transform::from_xyz(0., -0.05, 0.)));
}
