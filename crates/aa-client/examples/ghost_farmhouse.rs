//! The placement ghost of a housing design on flat ground, without a server:
//! `cargo run -p aa-client --example ghost_farmhouse -- --design 171 [--lift 1.0] --shot shots/ghost_farmhouse.png`
//! The ghost is every brush of the design's prefab (`Db::housing_parts`), as `placement::spawn_ghost_model` draws it.
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
use bevy::light::CascadeShadowConfigBuilder;
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

#[derive(Resource)]
struct Args {
    design: u32,
    lift: f32,
    yaw: f32,
}

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

fn main() {
    let args = Args {
        design: arg("--design").and_then(|v| v.parse().ok()).unwrap_or(171),
        lift: arg("--lift").and_then(|v| v.parse().ok()).unwrap_or(0.0),
        yaw: arg("--yaw").and_then(|v| v.parse().ok()).unwrap_or(35.0),
    };
    App::new()
        .add_plugins((aa_dev::default_plugins("ghost farmhouse"), aa_dev::DevPlugin, CryPlugin))
        .insert_resource(args)
        .insert_resource(ClearColor(Color::srgb(0.32, 0.38, 0.46)))
        .insert_resource(GlobalAmbientLight { color: Color::srgb(0.75, 0.8, 0.9), brightness: 900.0, ..default() })
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, mut cry: CryAssets, args: Res<Args>) {
    let db = ingame::data::Db::open().expect("game database");
    let catalog = ingame::placement::Catalog::load(&db).expect("catalog");
    let plant = catalog.plantable_for_design(args.design).expect("an item for the design").clone();
    info!("design {}: {} parts, model {:?}, grounding {:?}", args.design, plant.parts.len(), plant.model, plant.grounding);
    let root = commands.spawn((Transform::from_xyz(0.0, args.lift, 0.0), Visibility::Visible)).id();
    let options = ModelOptions::default();
    let mut hi = 1.0f32;
    if plant.parts.is_empty()
        && let Some(path) = &plant.model
        && let Ok(loaded) = cry.load_model(path, &options)
    {
        let m = spawn_loaded(&mut commands, path, &loaded, &options);
        commands.entity(m).insert(ChildOf(root));
    }
    for part in &plant.parts {
        let Ok(loaded) = cry.load_model(&part.path, &options) else { continue };
        if let Some((_, b)) = loaded.model.bounds() {
            hi = hi.max(b.z * part.scale.z + part.pos.z);
        }
        let m = spawn_loaded(&mut commands, &part.path, &loaded, &options);
        commands.entity(m).insert((ingame::units::ship_part_transform(part), ChildOf(root)));
    }
    let r = plant.footprint.max(4.0);
    commands.spawn((
        Mesh3d(cry.meshes.add(Plane3d::default().mesh().size(r * 8.0, r * 8.0))),
        MeshMaterial3d(cry.materials.add(StandardMaterial { base_color: Color::srgb(0.2, 0.3, 0.15), perceptual_roughness: 1.0, ..default() })),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
    commands.spawn((
        DirectionalLight { illuminance: 9000.0, shadow_maps_enabled: true, ..default() },
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, args.yaw.to_radians() - 0.7, -0.75, 0.0)),
        CascadeShadowConfigBuilder { maximum_distance: 120.0, first_cascade_far_bound: 20.0, ..default() }.build(),
    ));
    let yaw = args.yaw.to_radians();
    let dist = (hi * 2.4 + 8.0).max(14.0);
    let eye = Vec3::new(yaw.sin() * dist, hi * 0.6 + 3.0, yaw.cos() * dist);
    commands.spawn((Camera3d::default(), Transform::from_translation(eye).looking_at(Vec3::new(0.0, hi * 0.4, 0.0), Vec3::Y)));
}
