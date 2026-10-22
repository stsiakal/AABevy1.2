//! The live placement path, without a server: `Db` is inserted while the plugin is built (exactly like
//! `InGamePlugin`), the placement plugin's Startup builds the `Catalog`, a `HousePressed` opens the real
//! session and `track`/`spawn_ghost_model` draws the ghost with its plot dots and housing-area posts.
//!
//! `cargo run -p aa-client --example ghost_live -- --offscreen-shot shots/ghost_live.png --shot-after 8 --mute`
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
use aa_cry::scene::{CryAssets, CryPlugin};
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
    /// `item_housings.item_id` of the design (21165 = the Thatched Farmhouse, design 171).
    item: u32,
    x: f32,
    y: f32,
    yaw: f32,
}

#[derive(Resource)]
struct Spot {
    game: Vec3,
    yaw: f32,
}

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

/// The database at plugin-build time and the placement plugin after it, as `InGamePlugin` does it.
struct GameStartup;

impl Plugin for GameStartup {
    fn build(&self, app: &mut App) {
        if let Some(db) = ingame::data::Db::open() {
            app.insert_resource(db);
        }
        app.add_plugins(ingame::placement::plugin);
    }
}

fn main() {
    let args = Args {
        item: arg("--item").and_then(|v| v.parse().ok()).unwrap_or(21165),
        x: arg("--x").and_then(|v| v.parse().ok()).unwrap_or(10609.98),
        y: arg("--y").and_then(|v| v.parse().ok()).unwrap_or(15544.68),
        yaw: arg("--yaw").and_then(|v| v.parse().ok()).unwrap_or(35.0),
    };
    App::new()
        .add_plugins((aa_dev::default_plugins("ghost live"), aa_dev::DevPlugin, CryPlugin))
        .insert_resource(args)
        .insert_resource(ClearColor(Color::srgb(0.32, 0.38, 0.46)))
        .insert_resource(GlobalAmbientLight { color: Color::srgb(0.75, 0.8, 0.9), brightness: 900.0, ..default() })
        .insert_state(Screen::InGame)
        .add_message::<aa_net::NetCommand>()
        .add_message::<aa_net::NetEvent>()
        .init_resource::<aa_net::Farms>()
        .init_resource::<aa_net::Houses>()
        .init_resource::<aa_net::state::Doodads>()
        .init_resource::<aa_net::state::Inventory>()
        .init_resource::<aa_net::state::Skills>()
        .init_resource::<hud::HudPointerOverUi>()
        .init_resource::<hud::HudKeyboardFocus>()
        .init_resource::<ingame::virtual_input::VirtualInput>()
        .add_plugins(GameStartup)
        .add_systems(Startup, setup)
        .add_systems(PreUpdate, start_house)
        .add_systems(PostUpdate, aim_house)
        .run();
}

fn setup(mut commands: Commands, mut cry: CryAssets, args: Res<Args>) {
    let areas = ingame::houses::HousingAreas::load();
    let (x, y) = if areas.shape_at(args.x, args.y).is_some() {
        (args.x, args.y)
    } else {
        areas
            .shapes
            .first()
            .map(|s| ((s.bounds[0] + s.bounds[2]) * 0.5, (s.bounds[1] + s.bounds[3]) * 0.5))
            .unwrap_or((args.x, args.y))
    };
    info!("ghost live: spot ({x:.1}, {y:.1}) in {} housing areas", areas.shapes.len());
    commands.insert_resource(areas);
    commands.insert_resource(Spot { game: Vec3::new(x, y, 0.0), yaw: args.yaw.to_radians() });
    commands.spawn((
        ingame::player::LocalPlayer { obj_id: 1 },
        ingame::player::Mover {
            pos: Vec3::new(x, y, 0.0),
            facing: args.yaw.to_radians(),
            vz: 0.0,
            grounded: true,
            velocity: Vec3::ZERO,
            swim: None,
            wading: 0.0,
        },
    ));
    let center = Vec3::new(x, 0.0, -y);
    commands.spawn((
        Mesh3d(cry.meshes.add(Plane3d::default().mesh().size(160.0, 160.0))),
        MeshMaterial3d(cry.materials.add(StandardMaterial {
            base_color: Color::srgb(0.20, 0.30, 0.15),
            perceptual_roughness: 1.0,
            ..default()
        })),
        Transform::from_translation(center),
    ));
    commands.spawn((
        DirectionalLight { illuminance: 9000.0, shadow_maps_enabled: true, ..default() },
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, args.yaw.to_radians() - 0.7, -0.75, 0.0)),
        CascadeShadowConfigBuilder { maximum_distance: 120.0, first_cascade_far_bound: 20.0, ..default() }.build(),
    ));
    let yaw = args.yaw.to_radians();
    let dist = 24.0;
    let eye = center + Vec3::new(yaw.sin() * dist, 10.0, yaw.cos() * dist);
    commands.spawn((Camera3d::default(), Transform::from_translation(eye).looking_at(center + Vec3::Y * 3.0, Vec3::Y)));
}

/// The HUD's house item press, once the Startup catalog has the design.
fn start_house(mut pressed: MessageWriter<ingame::placement::HousePressed>, catalog: Res<ingame::placement::Catalog>, args: Res<Args>, mut started: Local<bool>) {
    if *started || catalog.plantable(args.item).is_none() {
        return;
    }
    pressed.write(ingame::placement::HousePressed { item_id: 1, item_template: args.item });
    *started = true;
}

/// The scripted `house_place` spot: the session keeps it, `track` shows the ghost there.
fn aim_house(mut placement: ResMut<ingame::placement::Placement>, spot: Res<Spot>, mut aimed: Local<bool>) {
    let Some(session) = placement.session.as_mut() else { return };
    if *aimed {
        return;
    }
    session.spot = Some(spot.game);
    session.invalid = None;
    session.scripted = true;
    session.yaw = spot.yaw;
    *aimed = true;
}
