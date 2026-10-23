//! A ship slave model as the client spawns it: its damage-state prefab plus the doodads its equipment installs.
//! `SHIP_MODEL=<models.id>` (default 393, the cutter), `SHIP_TEMPLATE=<slaves.id>` (default 21, the cutter hull),
//! `SHIP_STATE=1|2|3|4|6`, `SHIP_EQUIP=slot:item,slot:item` (e.g. `2:29726,3:28852`), `SHIP_VIEW=side|front|top`,
//! `SHIP_OLD=1` for the transform the client used before the hull's cry-space fix (the "before" picture).
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
        .add_plugins((aa_dev::default_plugins("aa-client: ship hull"), aa_dev::DevPlugin, CryPlugin, aa_fx::FxPlugin))
        .insert_resource(ClearColor(Color::srgb(0.55, 0.7, 0.85)))
        .insert_resource(GlobalAmbientLight { brightness: 900., ..default() })
        .add_systems(Startup, (setup, sea))
        .run();
}

fn setup(mut commands: Commands, mut cry: CryAssets) {
    let model: u32 = std::env::var("SHIP_MODEL").ok().and_then(|v| v.parse().ok()).unwrap_or(393);
    let template: u32 = std::env::var("SHIP_TEMPLATE").ok().and_then(|v| v.parse().ok()).unwrap_or(21);
    let state: u8 = std::env::var("SHIP_STATE").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    let old = std::env::var("SHIP_OLD").is_ok();
    let view = std::env::var("SHIP_VIEW").unwrap_or_else(|_| "side".into());
    let db = ingame::data::Db::open().expect("game database");
    let mut equipment = aa_net::model::Equipment::default();
    if let Ok(spec) = std::env::var("SHIP_EQUIP") {
        for pair in spec.split(',').filter(|p| !p.trim().is_empty()) {
            let Some((slot, item)) = pair.split_once(':') else { continue };
            let (Ok(slot), Ok(item)) = (slot.trim().parse::<usize>(), item.trim().parse::<u32>()) else { continue };
            if let Some(s) = equipment.slots.get_mut(slot) {
                *s = aa_net::model::Item::template_only(item);
            }
        }
    }
    let build = db.ship_visual(template, model, state, &equipment);
    let parts = build.parts;
    info!("ship model {model} template {template} state {state}: {} brushes, {} effects", parts.len(), build.effects.len());
    if std::env::var("SHIP_DEBUG").is_ok() {
        let options = ModelOptions::default();
        for part in &parts {
            match cry.load_model(&part.path, &options) {
                Ok(l) => info!("part {} nodes {:?} lod0 {:?} parts {:?} meshes {:?} material {:?}", part.path, l.model.nodes.iter().map(|n| (&n.name, &n.kind, n.mesh, n.parent)).collect::<Vec<_>>(), (0..l.model.nodes.len()).filter(|i| l.model.is_lod0_render_node(*i)).collect::<Vec<_>>(), l.parts.iter().map(|(n, p)| (*n, p.len())).collect::<Vec<_>>(), l.model.meshes.iter().map(|m| (m.subsets.len(), m.indices.len(), m.skin.is_some())).collect::<Vec<_>>(), l.material_path),
                Err(e) => info!("part {} error {e}", part.path),
            }
        }
    }
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
    for e in build.effects {
        let effect = aa_fx::Effect { scale: e.scale, speed_scale: e.speed_scale, count_scale: e.count_scale, prime: e.prime, no_body: true, ..aa_fx::Effect::new(e.effect.clone()) };
        commands.spawn((effect, Transform { translation: cry_to_bevy(e.pos), rotation: cry_rot_to_bevy(e.rot), ..default() }, ChildOf(root)));
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
