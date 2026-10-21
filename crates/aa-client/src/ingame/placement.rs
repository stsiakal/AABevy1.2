//! Planting: using a seed, sapling or livestock item starts a placement with a ghost of the doodad's start phase
//! under the cursor; a left click sends `CSCreateDoodad`, Escape / right click cancels.
//!
//! ## Which doodad an item plants
//!
//! `item_spawn_doodads` is the item -> doodad template link the client database carries (347 rows in the 1.2 data):
//! `item 15646` ("볍씨", rice seed) -> doodad `2246` (rice), `item 14899` (spruce sapling) -> `367`, `13725` (fig
//! sapling) -> `380`. The planting skills are shared by a whole class of seeds (10953 "large vegetation planting",
//! 25536 "herb planting") and their `InteractionEffect` only names a work item, not a doodad, so the seed's own row in
//! `item_spawn_doodads` is the only mapping. The doodad's start phase is the first `doodad_func_groups` row with
//! `doodad_func_group_kind_id = 1` (the server's `GetFuncGroupId`, falling back to the first normal group); its model
//! comes from the same `doodad_func_groups.model` lookup as [`super::doodad_visual`].
//!
//! ## Ghost and validity
//!
//! The ghost follows the cursor's ground hit ([`super::ground_target`]'s ray, the world collision and the terrain),
//! snaps to the floor height and applies the template's auto-z offset when it has one. The server database has no
//! auto-z columns on `doodad_almighties` (only `housings` has them), so the data path is ready but plants use none.
//!
//! The retail 1.2 capture (2026-10-21) shows the placement exactly: the ghost is the doodad template's plain model
//! (`doodad_almighties.model`; a sapling shows the full-grown spruce, not its start-phase sprout), surrounded by a
//! ring of leaf petals (`leaf_03.dds`) that are yellow on a valid spot and red on an invalid one, with a mixed ring
//! when part of the footprint touches something; the cursor is the client's white `housing_position.dds` / red
//! `housing_invalid.dds` icon; a red ground arrow points from the player to the spot; errors print at the top centre
//! ("Can't be installed here.", "Too close to other objects.", "Can't place this for N second(s).").
//!
//! Validity, for now: within the item's use range of the player, standing on ground (or on water when the template's
//! phases have a `DoodadFuncWaterVolume`), not on a road / stone / metal surface, and not inside another placed
//! doodad's footprint (`doodad_almighties.sim_radius`). The server stays authoritative: nothing is consumed locally,
//! and the doodad appears with the server's echo.

use std::collections::HashMap;

use aa_cry::scene::{CryAssets, ModelOptions, spawn_loaded};
use aa_net::farm::CreateDoodad;
use aa_net::state::{Doodads, Inventory, Skills};
use aa_net::{Command, NetCommand, Pos};
use aa_world::coords::cry_rot_to_bevy;
use aa_world::collide::SurfaceKind;
use aa_world::{TerrainQuery, WaterQuery, WorldCollision, cry_to_bevy};
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use rusqlite::{Connection, OpenFlags};

use super::camera::OrbitCamera;
use super::data::Db;
use super::ground_target::spot_under;
use super::player::{LocalPlayer, Mover};
use crate::Screen;
use crate::hud::{HudKeyboardFocus, HudPointerOverUi};

/// Step of the rotation keys, radians (15 degrees; Shift: 90 degrees).
pub const ROT_STEP: f32 = std::f32::consts::PI / 12.0;
/// Fallback use range of an item whose skill has none (the planting skills have 2 m).
const DEFAULT_RANGE: f32 = 2.0;
/// How far above the ground the floor is searched for, metres.
const REACH_UP: f32 = 4.0;

/// A template an item plants: the doodad, its start phase model, use range and footprint.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plantable {
    pub item: u32,
    pub doodad: u32,
    /// `items.use_skill_id` of the seed.
    pub use_skill: u32,
    /// `skills.max_range` of that skill (metres).
    pub use_range: f32,
    /// `skills.cooldown_time` / `cooldown_tag_id` of the use skill (a received `SCCooldowns` entry wins).
    pub cooldown_ms: u32,
    pub cooldown_tag: u32,
    /// The doodad's start phase (`doodad_func_groups.id`).
    pub start_group: u32,
    /// The start phase model as a pak path, when it can be found.
    pub model: Option<String>,
    /// `doodad_almighties.sim_radius` in metres.
    pub footprint: f32,
    /// The template's phases include a `DoodadFuncWaterVolume`: it must be placed on water.
    pub requires_water: bool,
    pub grounding: Grounding,
}

/// The offset fields a placement template may carry. The 1.2 server database has none on doodads (only `housings`),
/// so plants keep the defaults; the maths is here for the house placement of a later phase.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Grounding {
    /// Apply [`Grounding::offset`] to the ground point.
    pub auto_z: bool,
    /// Offset in game axes (x, y, z), metres.
    pub offset: Vec3,
}

/// The plants of the database, by item template and by doodad template.
#[derive(Resource, Default)]
pub struct Catalog {
    by_item: HashMap<u32, Plantable>,
    /// `doodad_almighties.id` -> `sim_radius` (metres).
    radius: HashMap<u32, f32>,
}

impl Catalog {
    /// Loads `item_spawn_doodads` and the footprints from the game database; `None` when it cannot be opened.
    pub fn load(db: &Db) -> Option<Catalog> {
        let path = std::env::var("AA_DB").unwrap_or_else(|_| aa_anim::db::DEFAULT_DB.to_string());
        Catalog::load_path(&path, Some(db))
    }

    fn load_path(path: &str, db: Option<&Db>) -> Option<Catalog> {
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI).ok()?;
        let mut catalog = Catalog::default();
        {
            let mut q = conn
                .prepare(
                    "SELECT d.item_id, d.doodad_id, i.use_skill_id, s.max_range, a.sim_radius, \
                     (SELECT g.id FROM doodad_func_groups g WHERE g.doodad_almighty_id = d.doodad_id \
                      ORDER BY CASE g.doodad_func_group_kind_id WHEN 1 THEN 0 ELSE 1 END, g.id LIMIT 1), \
                     EXISTS(SELECT 1 FROM doodad_phase_funcs p JOIN doodad_func_groups g ON g.id = p.doodad_func_group_id \
                      WHERE g.doodad_almighty_id = d.doodad_id AND p.actual_func_type = 'DoodadFuncWaterVolume'), \
                     s.cooldown_time, s.cooldown_tag_id \
                     FROM item_spawn_doodads d \
                     LEFT JOIN items i ON i.id = d.item_id \
                     LEFT JOIN skills s ON s.id = i.use_skill_id \
                     LEFT JOIN doodad_almighties a ON a.id = d.doodad_id",
                )
                .ok()?;
            let mut rows = q.query([]).ok()?;
            while let Ok(Some(r)) = rows.next() {
                let item: u32 = r.get::<_, Option<i64>>(0).unwrap_or_default().unwrap_or(0).max(0) as u32;
                let doodad: u32 = r.get::<_, Option<i64>>(1).unwrap_or_default().unwrap_or(0).max(0) as u32;
                if item == 0 || doodad == 0 {
                    continue;
                }
                let use_skill: u32 = r.get::<_, Option<i64>>(2).unwrap_or_default().unwrap_or(0).max(0) as u32;
                let range: f32 = r.get::<_, Option<f64>>(3).unwrap_or_default().unwrap_or(0.0) as f32;
                let radius: f32 = r.get::<_, Option<i64>>(4).unwrap_or_default().unwrap_or(0).max(0) as f32 / 1000.0;
                let start_group: u32 = r.get::<_, Option<i64>>(5).unwrap_or_default().unwrap_or(0).max(0) as u32;
                let water: bool = r.get::<_, Option<i64>>(6).unwrap_or_default().unwrap_or(0) != 0;
                let cooldown_ms: u32 = r.get::<_, Option<i64>>(7).unwrap_or_default().unwrap_or(0).max(0) as u32;
                let cooldown_tag: u32 = r.get::<_, Option<i64>>(8).unwrap_or_default().unwrap_or(0).max(0) as u32;
                catalog.radius.insert(doodad, radius);
                catalog.by_item.entry(item).or_insert(Plantable {
                    item,
                    doodad,
                    use_skill,
                    use_range: if range > 0.0 { range } else { DEFAULT_RANGE },
                    cooldown_ms,
                    cooldown_tag,
                    start_group,
                    // the ghost is the template's own model, not the start phase: the retail capture shows the
                    // full-grown spruce for a sapling (see the module documentation); an empty template model falls
                    // back to the start phase
                    model: db.and_then(|db| match db.doodad_model(doodad) {
                        Some(model) => Some(model),
                        None if start_group > 0 => db.doodad_phase_model(doodad, start_group),
                        None => None,
                    }),
                    footprint: radius,
                    requires_water: water,
                    grounding: Grounding::default(),
                });
            }
        }
        Some(catalog)
    }

    pub fn plantable(&self, item: u32) -> Option<&Plantable> {
        self.by_item.get(&item)
    }

    /// The footprint radius of a placed doodad template (m), 0 when the data has none.
    pub fn footprint(&self, doodad: u32) -> f32 {
        self.radius.get(&doodad).copied().unwrap_or(0.0)
    }

    #[cfg(test)]
    fn insert(&mut self, plant: Plantable) {
        self.radius.insert(plant.doodad, plant.footprint);
        self.by_item.insert(plant.item, plant);
    }
}

/// A planting item was used (the HUD sends this instead of a skill press).
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlantPressed {
    pub item_id: u64,
    pub item_template: u32,
}

/// Why a spot cannot take a plant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Invalid {
    NoGround,
    OutOfRange,
    NotWater,
    Underwater,
    Footprint,
    BadSurface,
}

impl Invalid {
    pub fn text(self) -> &'static str {
        match self {
            Self::NoGround => "no ground under the cursor",
            Self::OutOfRange => "out of the item's use range",
            Self::NotWater => "this plant needs water",
            Self::Underwater => "this plant cannot stand in water",
            Self::Footprint => "another doodad is in the way",
            Self::BadSurface => "the surface cannot be planted on",
        }
    }
}

/// The placement being aimed: the seed, its doodad, the spot under the cursor and the ghost.
pub struct Session {
    pub item_id: u64,
    pub plant: Plantable,
    /// Game coordinates of the spot; `None` while the cursor is over no ground.
    pub spot: Option<Vec3>,
    pub yaw: f32,
    pub scale: f32,
    pub invalid: Option<Invalid>,
    /// The ghost root, its model and the tinted part materials.
    root: Option<Entity>,
    model_failed: bool,
    /// The state changed since the last log (the spot or its validity).
    changed: bool,
}

impl Session {
    pub fn new(item_id: u64, plant: Plantable, yaw: f32) -> Session {
        Session {
            item_id,
            plant,
            spot: None,
            yaw,
            scale: 1.0,
            invalid: Some(Invalid::NoGround),
            root: None,
            model_failed: false,
            changed: true,
        }
    }

    pub fn valid(&self) -> bool {
        self.invalid.is_none() && self.spot.is_some()
    }

    /// The request a confirmation sends; `None` while the spot is not valid.
    pub fn request(&self) -> Option<CreateDoodad> {
        if !self.valid() {
            return None;
        }
        let spot = self.spot?;
        Some(CreateDoodad {
            id: self.plant.doodad,
            pos: Pos::new(spot.x, spot.y, spot.z),
            z_rot: self.yaw,
            scale: self.scale,
            item_id: self.item_id,
        })
    }
}

#[derive(Resource, Default)]
pub struct Placement {
    pub session: Option<Session>,
}

#[derive(Component)]
struct Ghost;

/// One of the two petal-ring meshes.
#[derive(Component)]
struct PlacementArrow;

#[derive(Component)]
struct PlacementCursorIcon;

/// The assets of the retail placement look, loaded once.
#[derive(Resource, Default)]
struct Visuals {
    ready: bool,
    failed: bool,
    yellow: Option<Handle<StandardMaterial>>,
    red: Option<Handle<StandardMaterial>>,
    arrow_image: Option<Handle<Image>>,
    cursor_mesh: Option<Handle<Mesh>>,
    cursor_good: Option<Handle<StandardMaterial>>,
    cursor_bad: Option<Handle<StandardMaterial>>,
    ring: RingParts,
    arrow: Option<Entity>,
}

/// A ring mesh entity and the geometry signature last uploaded.
struct RingPart {
    entity: Entity,
    mesh: Handle<Mesh>,
    signature: u64,
}

#[derive(Default)]
struct RingParts {
    yellow: Option<RingPart>,
    red: Option<RingPart>,
}

pub fn plugin(app: &mut App) {
    app.add_message::<PlantPressed>()
        .init_resource::<Placement>()
        .init_resource::<Visuals>()
        .insert_resource(Catalog::default())
        .add_systems(Startup, load_catalog)
        .add_systems(PreUpdate, input.after(bevy::input::InputSystems).run_if(in_state(Screen::InGame)))
        .add_systems(Update, (start, track, cursor_icon, log_server).chain().run_if(in_state(Screen::InGame)))
        .add_systems(OnExit(Screen::InGame), leave);
}

fn load_catalog(mut catalog: ResMut<Catalog>, db: Option<Res<Db>>) {
    let Some(db) = db else { return };
    match Catalog::load(&db) {
        Some(loaded) => {
            info!("planting: {} seed / sapling items in item_spawn_doodads", loaded.by_item.len());
            *catalog = loaded;
        }
        None => warn!("planting: cannot read item_spawn_doodads"),
    }
}

/// The ground height under `(x, y)`: a world brush, else the terrain, else `fallback`.
pub fn ground_at(collision: Option<&WorldCollision>, terrain: Option<&TerrainQuery>, x: f32, y: f32, from_z: f32, fallback: f32) -> f32 {
    collision
        .and_then(|c| c.ground_below(x, y, from_z + REACH_UP, 0.0))
        .map(|g| g.height)
        .or_else(|| terrain.and_then(|t| t.height_at(x, y)))
        .unwrap_or(fallback)
}

/// The water surface over `(x, y, ground)`, when there is any.
pub fn water_surface(water: Option<&WaterQuery>, x: f32, y: f32, ground: f32) -> Option<f32> {
    water.and_then(|w| w.water_at(Vec3::new(x, y, ground)).map(|(h, _)| h))
}

/// The placement point of a ground hit: the floor, or the water surface when the plant needs water, then the
/// template's auto-z offset when it is set.
pub fn grounded(hit: Vec3, ground: f32, water: Option<f32>, plant: &Plantable) -> Vec3 {
    let z = if plant.requires_water { water.unwrap_or(ground) } else { ground };
    let mut p = Vec3::new(hit.x, hit.y, z);
    if plant.grounding.auto_z {
        p += plant.grounding.offset;
    }
    p
}

/// Validity of `spot` for `plant` from `player`: use range, ground / water, surface and other doodads' footprints.
pub fn check(
    plant: &Plantable,
    spot: Vec3,
    player: Vec3,
    doodads: &Doodads,
    catalog: &Catalog,
    water: Option<f32>,
    surface: Option<SurfaceKind>,
) -> Result<(), Invalid> {
    if spot.distance(player) > plant.use_range {
        return Err(Invalid::OutOfRange);
    }
    if matches!(surface, Some(SurfaceKind::Stone | SurfaceKind::Metal)) {
        return Err(Invalid::BadSurface);
    }
    match (plant.requires_water, water) {
        (true, None) => return Err(Invalid::NotWater),
        (false, Some(h)) if h > spot.z + 0.05 => return Err(Invalid::Underwater),
        _ => {}
    }
    for doodad in doodads.by_id.values() {
        // only doodads a player or a house put down have a footprint we know: the static world is full of
        // vegetation whose simulated radius would block every open spot
        if doodad.owner_type != 254 && doodad.owner_type != 3 {
            continue;
        }
        let other = catalog.footprint(doodad.template_id);
        if plant.footprint <= 0.0 || other <= 0.0 {
            continue;
        }
        let d = Vec2::new(doodad.pos.x - spot.x, doodad.pos.y - spot.y).length();
        if d < plant.footprint + other {
            return Err(Invalid::Footprint);
        }
    }
    Ok(())
}

/// Ends the placement: the ghost goes with it.
fn end_session(commands: &mut Commands, placement: &mut Placement) {
    if let Some(session) = placement.session.take()
        && let Some(root) = session.root
    {
        commands.entity(root).despawn();
    }
}

/// Starts a placement when the HUD reports a seed use.
fn start(
    mut commands: Commands,
    mut pressed: MessageReader<PlantPressed>,
    catalog: Res<Catalog>,
    mut placement: ResMut<Placement>,
    player: Query<&Mover, With<LocalPlayer>>,
    camera: Query<&OrbitCamera>,
) {
    for press in pressed.read() {
        let Some(plant) = catalog.plantable(press.item_template).cloned() else {
            warn!("planting: item {} (template {}) is not in item_spawn_doodads", press.item_id, press.item_template);
            continue;
        };
        end_session(&mut commands, &mut placement);
        // the retail client's first yaw is the camera heading (two captures: 4.2020082 and 3.166932 rad)
        let yaw = camera.single().map(|c| c.yaw).or_else(|_| player.single().map(|m| m.facing)).unwrap_or(0.0);
        info!(
            "planting: {} (item {}) places doodad {} (phase {}, model {:?}, range {:.1} m, footprint {:.2} m)",
            press.item_template, press.item_id, plant.doodad, plant.start_group, plant.model, plant.use_range, plant.footprint
        );
        placement.session = Some(Session::new(press.item_id, plant, yaw));
    }
}

/// Loads the ghost model and keeps it on the cursor's spot, with the retail ring and ground arrow.
#[allow(clippy::too_many_arguments)]
fn track(
    mut commands: Commands,
    mut cry: CryAssets,
    mut placement: ResMut<Placement>,
    catalog: Res<Catalog>,
    player: Query<&Mover, With<LocalPlayer>>,
    doodads: Res<Doodads>,
    over_ui: Res<HudPointerOverUi>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    collision: Option<Res<WorldCollision>>,
    terrain: Option<Res<TerrainQuery>>,
    water: Option<Res<WaterQuery>>,
    mut visuals: ResMut<Visuals>,
) {
    ensure_visuals(&mut cry.images, &mut cry.meshes, &mut cry.materials, &mut visuals);
    let Some(session) = placement.session.as_mut() else { return };
    let root = ensure_ghost(&mut commands, &mut cry, session);
    let Some(mover) = player.single().ok() else { return };
    let cursor = if over_ui.0 { None } else { windows.single().ok().and_then(|w| w.cursor_position()) };
    let hit = cursor.and_then(|c| {
        let (camera, transform) = cameras.iter().find(|(c, _)| c.is_active)?;
        spot_under(camera, transform, c, collision.as_deref(), terrain.as_deref())
    });
    let Some(hit) = hit else {
        session.spot = None;
        session.invalid = Some(Invalid::NoGround);
        if let Some(root) = root {
            commands.entity(root).insert(Visibility::Hidden);
        }
        hide_ring(&mut commands, &visuals);
        return;
    };
    let ground = ground_at(collision.as_deref(), terrain.as_deref(), hit.x, hit.y, hit.z, hit.z);
    let wet = water_surface(water.as_deref(), hit.x, hit.y, ground);
    let surface = collision.as_deref().and_then(|c| c.surface_below(hit.x, hit.y, ground + 0.3, 0.0)).map(|s| s.surface.kind());
    let spot = grounded(hit, ground, wet, &session.plant);
    let before = (session.spot, session.invalid);
    session.spot = Some(spot);
    session.invalid = check(&session.plant, spot, mover.pos, &doodads, &catalog, wet, surface).err();
    let mixed = ring_blocked(&session.plant, spot, &doodads, &catalog).is_some();
    if mixed && session.invalid.is_none() {
        session.invalid = Some(Invalid::Footprint);
    }
    if (session.spot, session.invalid) != before {
        session.changed = true;
    }
    if let Some(root) = root {
        let cry_q = cry_rot_to_bevy(Quat::from_xyzw(0.0, 0.0, (session.yaw * 0.5).sin(), (session.yaw * 0.5).cos()));
        commands.entity(root).insert((
            Transform {
                translation: cry_to_bevy(spot),
                rotation: cry_q,
                scale: Vec3::splat(session.scale),
            },
            Visibility::Visible,
        ));
    }
    if session.changed {
        session.changed = false;
        info!(
            "planting: spot ({:.1}, {:.1}, {:.1}) {}",
            spot.x,
            spot.y,
            spot.z,
            session.invalid.map_or("is valid", Invalid::text)
        );
    }
    update_ring(&mut commands, &mut cry.meshes, &mut visuals, session, &doodads, &catalog, collision.as_deref(), terrain.as_deref());
}

/// The ghost entity of the session, loaded once; `None` when the doodad has no model.
fn ensure_ghost(commands: &mut Commands, cry: &mut CryAssets, session: &mut Session) -> Option<Entity> {
    if let Some(root) = session.root {
        return Some(root);
    }
    if session.model_failed {
        return None;
    }
    let Some(path) = session.plant.model.clone() else {
        session.model_failed = true;
        warn!("planting: doodad {} has no model to ghost", session.plant.doodad);
        return None;
    };
    let options = ModelOptions::default();
    let loaded = match cry.load_model(&path, &options) {
        Ok(loaded) => loaded,
        Err(e) => {
            session.model_failed = true;
            warn!("planting: cannot load ghost model {path}: {e}");
            return None;
        }
    };
    let root = commands
        .spawn((Name::new("planting ghost"), Ghost, Transform::IDENTITY, Visibility::Visible))
        .id();
    let model = spawn_loaded(commands, &path, &loaded, &options);
    commands.entity(model).insert((ChildOf(root), Visibility::Visible, NoFrustumCulling));
    let parts: usize = loaded.parts.iter().map(|(_, parts)| parts.len()).sum();
    info!("planting: ghost {path} loaded ({parts} parts)");
    session.root = Some(root);
    Some(root)
}

/// The petals of a ring that touch a placed doodad (the mixed red / yellow ring), when any does.
fn ring_blocked(plant: &Plantable, spot: Vec3, doodads: &Doodads, catalog: &Catalog) -> Option<usize> {
    let (radius, count) = ring_shape(plant);
    let mut n = 0;
    for i in 0..count {
        let ang = i as f32 / count as f32 * std::f32::consts::TAU;
        let p = Vec2::new(spot.x, spot.y) + Vec2::new(ang.cos(), ang.sin()) * radius;
        if petal_blocked(plant, p, doodads, catalog) {
            n += 1;
        }
    }
    (n > 0).then_some(n)
}

fn ring_shape(plant: &Plantable) -> (f32, usize) {
    let radius = (plant.footprint.max(0.3) + 0.35).clamp(0.7, 4.0);
    let count = (14.0 + radius * 6.0).clamp(14.0, 40.0) as usize;
    (radius, count)
}

fn petal_blocked(plant: &Plantable, p: Vec2, doodads: &Doodads, catalog: &Catalog) -> bool {
    let _ = plant;
    for doodad in doodads.by_id.values() {
        if doodad.owner_type != 254 && doodad.owner_type != 3 {
            continue;
        }
        let other = catalog.footprint(doodad.template_id);
        if other <= 0.0 {
            continue;
        }
        if Vec2::new(doodad.pos.x, doodad.pos.y).distance(p) < other + 0.12 {
            return true;
        }
    }
    false
}

/// Builds the two petal lists (yellow valid, red blocked) around `spot`, in Bevy coordinates.
fn ring_quads(
    plant: &Plantable,
    spot: Vec3,
    doodads: &Doodads,
    catalog: &Catalog,
    floor: impl Fn(Vec2) -> f32,
    all_red: bool,
) -> (Vec<Quad>, Vec<Quad>) {
    let (radius, count) = ring_shape(plant);
    let half_len = 0.10 + radius * 0.10;
    let half_w = half_len * 0.52;
    let mut yellow = Vec::with_capacity(count);
    let mut red = Vec::with_capacity(count);
    for i in 0..count {
        let jitter = (((i * 37) % 101) as f32 / 101.0 - 0.5) * 0.22;
        let ang = i as f32 / count as f32 * std::f32::consts::TAU + jitter;
        let dir = Vec2::new(ang.cos(), ang.sin());
        let side = Vec2::new(-dir.y, dir.x);
        let centre = Vec2::new(spot.x, spot.y) + dir * radius;
        let z = floor(centre) + 0.045;
        let game = Vec3::new(centre.x, centre.y, z);
        let p = [
            game - (side * half_w + dir * half_len).extend(0.0),
            game + (side * half_w - dir * half_len).extend(0.0),
            game + (side * half_w + dir * half_len).extend(0.0),
            game + (dir * half_len - side * half_w).extend(0.0),
        ];
        let p = p.map(cry_to_bevy);
        let quad = Quad { p, uv: [[0.5, 0.5], [1.0, 0.5], [1.0, 1.0], [0.5, 1.0]] };
        if all_red || petal_blocked(plant, centre, doodads, catalog) {
            red.push(quad);
        } else {
            yellow.push(quad);
        }
    }
    (yellow, red)
}

fn quads_signature(quads: &[Quad]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for q in quads {
        for p in q.p {
            ((p.x * 100.0).round() as i32).hash(&mut h);
            ((p.y * 100.0).round() as i32).hash(&mut h);
            ((p.z * 100.0).round() as i32).hash(&mut h);
        }
    }
    h.finish()
}

/// Uploads a non-empty petal mesh and shows its entity; an empty list only hides the entity. The mesh is not
/// touched (and not reallocated) while the geometry signature is unchanged.
fn show_ring_part(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    part: &mut Option<RingPart>,
    material: &Handle<StandardMaterial>,
    name: &'static str,
    quads: &[Quad],
) {
    if quads.is_empty() {
        if let Some(p) = part {
            commands.entity(p.entity).insert(Visibility::Hidden);
        }
        return;
    }
    let signature = quads_signature(quads);
    match part {
        Some(p) => {
            if p.signature != signature {
                if let Some(mesh) = mesh_from_quads(quads)
                    && let Some(mut m) = meshes.get_mut(&p.mesh)
                {
                    *m = mesh;
                }
                p.signature = signature;
            }
            commands.entity(p.entity).insert(Visibility::Visible);
        }
        None => {
            let Some(mesh) = mesh_from_quads(quads) else { return };
            let mesh = meshes.add(mesh);
            let entity = commands
                .spawn((
                    Name::new(name),
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::IDENTITY,
                    Visibility::Inherited,
                    NoFrustumCulling,
                    NotShadowCaster,
                    NotShadowReceiver,
                ))
                .id();
            *part = Some(RingPart { entity, mesh, signature });
        }
    }
}

fn mesh_from_quads(quads: &[Quad]) -> Option<Mesh> {
    if quads.is_empty() {
        return None;
    }
    let mut positions = Vec::with_capacity(quads.len() * 4);
    let mut uvs = Vec::with_capacity(quads.len() * 4);
    let mut indices = Vec::with_capacity(quads.len() * 6);
    for q in quads {
        let base = positions.len() as u32;
        positions.extend_from_slice(&q.p);
        uvs.extend_from_slice(&q.uv);
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    Some(
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; positions.len()])
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
            .with_inserted_indices(Indices::U32(indices)),
    )
}

/// Rebuilds the two petal meshes when the cursor, the footprint or the blocked petals changed; the CPU work is
/// cheap but no mesh is reallocated or uploaded while nothing changes.
#[allow(clippy::too_many_arguments)]
fn update_ring(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    visuals: &mut Visuals,
    session: &Session,
    doodads: &Doodads,
    catalog: &Catalog,
    collision: Option<&WorldCollision>,
    terrain: Option<&TerrainQuery>,
) {
    if !visuals.ready {
        return;
    }
    let Some(spot) = session.spot else {
        hide_ring(commands, visuals);
        return;
    };
    let floor = |p: Vec2| ground_at(collision, terrain, p.x, p.y, spot.z, spot.z);
    let all_red = matches!(session.invalid, Some(i) if i != Invalid::Footprint);
    let (yellow, red) = ring_quads(&session.plant, spot, doodads, catalog, floor, all_red);
    let (Some(yellow_mat), Some(red_mat)) = (visuals.yellow.clone(), visuals.red.clone()) else { return };
    show_ring_part(commands, meshes, &mut visuals.ring.yellow, &yellow_mat, "planting ring (valid)", &yellow);
    show_ring_part(commands, meshes, &mut visuals.ring.red, &red_mat, "planting ring (blocked)", &red);
}

fn hide_ring(commands: &mut Commands, visuals: &Visuals) {
    for part in [visuals.ring.yellow.as_ref(), visuals.ring.red.as_ref()].into_iter().flatten() {
        commands.entity(part.entity).insert(Visibility::Hidden);
    }
    if let Some(arrow) = visuals.arrow {
        commands.entity(arrow).insert(Visibility::Hidden);
    }
}

/// A textured quad in world space.
#[derive(Clone)]
struct Quad {
    p: [Vec3; 4],
    uv: [[f32; 2]; 4],
}

/// Loads the original placement textures (`leaf_03.dds`, the road arrow, the housing cursors) once.
fn ensure_visuals(images: &mut Assets<Image>, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, visuals: &mut Visuals) {
    if visuals.ready || visuals.failed {
        return;
    }
    let mut build = || -> Option<()> {
        let leaf = dds_image("textures/p_effects/leaf_03.dds")?;
        let leaf = images.add(leaf);
        let leaf_material = |color: Color| StandardMaterial {
            base_color: color,
            base_color_texture: Some(leaf.clone()),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            depth_bias: 10.0,
            ..default()
        };
        visuals.yellow = Some(materials.add(leaf_material(Color::srgba(1.0, 0.82, 0.18, 0.95))));
        visuals.red = Some(materials.add(leaf_material(Color::srgba(0.82, 0.09, 0.05, 0.95))));

        let arrow = dds_image("textures/decal/roadsign/arrow.dds")?;
        visuals.arrow_image = Some(images.add(arrow));

        let good = dds_image("ui/cursor/housing_position.dds")?;
        let bad = dds_image("ui/cursor/housing_invalid.dds")?;
        let good = images.add(good);
        let bad = images.add(bad);
        let cursor_material = |image: &Handle<Image>| StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(image.clone()),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            ..default()
        };
        visuals.cursor_good = Some(materials.add(cursor_material(&good)));
        visuals.cursor_bad = Some(materials.add(cursor_material(&bad)));
        visuals.cursor_mesh = Some(meshes.add(Mesh::from(Rectangle::new(1.0, 1.0))));
        Some(())
    };
    if build().is_none() {
        visuals.failed = true;
        warn!("planting: cannot read the placement textures (leaf_03 / arrow / housing cursors)");
        return;
    }
    visuals.ready = true;
}
fn dds_image(path: &str) -> Option<Image> {
    let bytes = aa_cry::read(path).ok()?;
    let dds = aa_cry::texture::Dds::parse(&bytes).ok()?;
    let (w, h) = (dds.width, dds.height);
    let mut image = Image::new(
        Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        TextureDimension::D2,
        dds.decode_rgba8(0, 0),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = bevy::image::ImageSampler::linear();
    Some(image)
}

/// Hides the OS cursor while placing and draws the client's placement cursor at the pointer (the white crosshair
/// `housing_position` / red cross `housing_invalid` of the 1.2 client).
fn cursor_icon(
    mut commands: Commands,
    placement: Res<Placement>,
    mut visuals: ResMut<Visuals>,
    mut windows: Query<(Entity, &mut Window, &mut bevy::window::CursorOptions), With<bevy::window::PrimaryWindow>>,
    cameras: Query<(Entity, &Camera, &Projection), With<Camera3d>>,
    ui_cameras: Query<Entity, With<aa_ui::render::UiCamera>>,
    mut icon: Query<(&mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>), With<PlacementCursorIcon>>,
) {
    let Ok((window_entity, window, mut cursor_options)) = windows.single_mut() else { return };
    let placing = placement.session.is_some();
    let hide_os = placing && visuals.cursor_mesh.is_some();
    if cursor_options.visible == hide_os {
        cursor_options.visible = !hide_os;
    }
    let _ = window_entity;
    if !placing {
        if let Ok((_, mut v, _)) = icon.single_mut() {
            *v = Visibility::Hidden;
        }
        if let Some(arrow) = visuals.arrow {
            commands.entity(arrow).insert(Visibility::Hidden);
        }
        return;
    }
    let Some((camera_entity, camera, projection)) = cameras.iter().find(|(_, c, _)| c.is_active) else { return };
    // the placement-mode arrow: a screen-space HUD element at the bottom centre (the retail arrow is native; the
    // road arrow texture is the closest original asset and stays documented as a placeholder)
    if let Some(image) = visuals.arrow_image.clone() {
        match visuals.arrow {
            Some(arrow) => {
                commands.entity(arrow).insert(Visibility::Inherited);
            }
            None => {
                let target = ui_cameras.single().ok().unwrap_or(camera_entity);
                let arrow = commands
                    .spawn((
                        Name::new("planting arrow"),
                        PlacementArrow,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(50.0),
                            bottom: Val::Px(112.0),
                            width: Val::Px(150.0),
                            height: Val::Px(60.0),
                            ..default()
                        },
                        UiTransform::from_translation(Val2::new(Val::Px(-75.0), Val::Px(0.0))),
                        ImageNode { image, color: Color::srgba(0.78, 0.07, 0.04, 0.9), flip_x: true, ..default() },
                        UiTargetCamera(target),
                        Visibility::Inherited,
                        GlobalZIndex(100),
                    ))
                    .id();
                visuals.arrow = Some(arrow);
            }
        }
    }
    let Some(cursor) = window.cursor_position() else {
        if let Ok((_, mut v, _)) = icon.single_mut() {
            *v = Visibility::Hidden;
        }
        return;
    };
    let Some(material) = (if placement.session.as_ref().is_some_and(|s| s.invalid.is_none()) { visuals.cursor_good.clone() } else { visuals.cursor_bad.clone() }) else { return };
    let Some(mesh) = visuals.cursor_mesh.clone() else { return };
    let size_px = 32.0;
    let view = camera.logical_viewport_size().unwrap_or(Vec2::new(1920.0, 1080.0));
    let fov = match projection {
        Projection::Perspective(p) => p.fov,
        _ => 1.0,
    };
    let dist = 1.0;
    let half_h = (fov * 0.5).tan() * dist;
    let half_w = half_h * view.x / view.y;
    let scale = size_px / view.y * 2.0 * half_h;
    let x = ((cursor.x / view.x) * 2.0 - 1.0) * half_w;
    let y = (1.0 - (cursor.y / view.y) * 2.0) * half_h;
    match icon.single_mut() {
        Ok((mut transform, mut visibility, mut mat)) => {
            transform.translation = Vec3::new(x, y, -dist);
            transform.scale = Vec3::splat(scale);
            *visibility = Visibility::Inherited;
            if mat.0 != material {
                mat.0 = material;
            }
        }
        Err(_) => {
            commands.spawn((
                Name::new("planting cursor"),
                PlacementCursorIcon,
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::from_translation(Vec3::new(x, y, -dist)).with_scale(Vec3::splat(scale)),
                Visibility::Inherited,
                NoFrustumCulling,
                NotShadowCaster,
                NotShadowReceiver,
                ChildOf(camera_entity),
            ));
        }
    }
}
/// Mouse wheel rotates; Alt finely, Shift 15 degrees, Ctrl keeps the camera zoom; Escape / right click cancels and a
/// left click confirms. An invalid click prints the retail error and sends nothing.
#[allow(clippy::too_many_arguments)]
fn input(
    mut commands: Commands,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    over_ui: Res<HudPointerOverUi>,
    focus: Res<HudKeyboardFocus>,
    mut placement: ResMut<Placement>,
    skills: Res<Skills>,
    rt: Option<NonSend<aa_ui::UiRuntime>>,
    mut net: MessageWriter<NetCommand>,
) {
    let Some(session) = placement.session.as_mut() else { return };
    if focus.0 {
        return;
    }
    // picking an item up onto the cursor ends the placement mode (the retail client does the same)
    if rt.as_ref().is_some_and(|rt| aa_ui::lua::drag::holding(&rt.lua)) {
        info!("planting: an item is on the cursor, placement ends");
        end_session(&mut commands, &mut placement);
        return;
    }
    let mut cancel = keys.just_pressed(KeyCode::Escape);
    if cancel {
        keys.clear_just_pressed(KeyCode::Escape);
    }
    let (shift, alt, ctrl) = (
        keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]),
        keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]),
        keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]),
    );
    if !ctrl {
        let step = if shift {
            ROT_STEP
        } else if alt {
            1f32.to_radians()
        } else {
            ROT_STEP * 2.0 / 3.0
        };
        let mut turn = 0.0;
        for ev in wheel.read() {
            if ev.y != 0.0 {
                turn += step * ev.y.signum();
            }
        }
        if turn != 0.0 {
            session.yaw = wrap_angle(session.yaw + turn);
        }
    }
    if mouse.just_pressed(MouseButton::Right) {
        mouse.clear_just_pressed(MouseButton::Right);
        cancel = true;
    }
    if mouse.just_pressed(MouseButton::Left) {
        mouse.clear_just_pressed(MouseButton::Left);
        if !over_ui.0 {
            let cooldown = skills
                .cooldown_left(session.plant.use_skill, session.plant.cooldown_tag, session.plant.cooldown_ms)
                .filter(|left| !left.is_zero())
                .map(|left| left.as_secs().max(1));
            if let Some(seconds) = cooldown {
                notify(rt.as_deref(), format!("Can't place this for {seconds} second(s)."));
            } else if let Some(reason) = session.invalid {
                match error_message(reason) {
                    Some(text) => notify(rt.as_deref(), text.into()),
                    None => info!("planting: not confirmed: {}", reason.text()),
                }
            } else if let Some(request) = session.request() {
                let spot = request.pos;
                info!(
                    "planting: confirm doodad {} (item {}) at ({:.2}, {:.2}, {:.2}), yaw {:.2}, scale {:.2}",
                    request.id, request.item_id, spot.x, spot.y, spot.z, request.z_rot, request.scale
                );
                net.write(NetCommand(Command::CreateDoodad(request)));
                cancel = true;
            }
        }
    }
    if cancel {
        info!("planting: cancelled");
        end_session(&mut commands, &mut placement);
    }
}

/// The red error text at the top centre, as `SYSMSG` (the original `centermessage` script).
fn notify(rt: Option<&aa_ui::UiRuntime>, text: String) {
    info!("planting: {text}");
    if let Some(rt) = rt {
        rt.fire_event("SYSMSG", text);
    }
}

/// The retail client's placement errors (the 1.2 capture; no `locale` key was found in the UI scripts, so the
/// English of the capture is the fallback).
fn error_message(reason: Invalid) -> Option<&'static str> {
    match reason {
        Invalid::Footprint => Some("Too close to other objects."),
        Invalid::OutOfRange => None,
        _ => Some("Can't be installed here."),
    }
}

fn wrap_angle(mut a: f32) -> f32 {
    while a > std::f32::consts::PI {
        a -= std::f32::consts::TAU;
    }
    while a < -std::f32::consts::PI {
        a += std::f32::consts::TAU;
    }
    a
}

/// The server's answer: the doodad it created (the item task is the other half of the echo).
fn log_server(mut events: MessageReader<aa_net::NetEvent>) {
    for aa_net::NetEvent(event) in events.read() {
        if let aa_net::Event::DoodadsSpawned(doodads) = event {
            for d in doodads {
                debug!("planting: server created doodad {} (template {}) at ({:.1}, {:.1}, {:.1})", d.obj_id, d.template_id, d.pos.x, d.pos.y, d.pos.z);
            }
        }
    }
}

fn leave(mut placement: ResMut<Placement>, ghosts: Query<Entity, With<Ghost>>, mut visuals: ResMut<Visuals>, mut windows: Query<&mut bevy::window::CursorOptions>, mut commands: Commands) {
    placement.session = None;
    for e in &ghosts {
        commands.entity(e).despawn();
    }
    hide_ring(&mut commands, &visuals);
    if let Some(arrow) = visuals.arrow.take() {
        commands.entity(arrow).despawn();
    }
    for mut cursor in &mut windows {
        cursor.visible = true;
    }
}

/// The scripted `plant <item template> <dx> <dy> [yaw]`: the same spot maths and validity as the ghost, without a
/// cursor, then the confirmation command.
#[allow(clippy::too_many_arguments)]
pub fn plant_now(
    catalog: &Catalog,
    doodads: &Doodads,
    inventory: &Inventory,
    collision: Option<&WorldCollision>,
    terrain: Option<&TerrainQuery>,
    water: Option<&WaterQuery>,
    player: Vec3,
    facing: f32,
    item: u32,
    dx: f32,
    dy: f32,
    yaw: Option<f32>,
) -> Result<CreateDoodad, String> {
    let plant = catalog.plantable(item).ok_or_else(|| format!("item {item} is not in item_spawn_doodads"))?;
    let (_, item_instance) = inventory.find_template(item).ok_or_else(|| format!("no item {item} in the bag"))?;
    let x = player.x + dx;
    let y = player.y + dy;
    let ground = ground_at(collision, terrain, x, y, player.z, player.z);
    let wet = water_surface(water, x, y, ground);
    let surface = collision.and_then(|c| c.surface_below(x, y, ground + 0.3, 0.0)).map(|s| s.surface.kind());
    let spot = grounded(Vec3::new(x, y, ground), ground, wet, plant);
    check(plant, spot, player, doodads, catalog, wet, surface).map_err(|why| why.text().to_string())?;
    Ok(CreateDoodad {
        id: plant.doodad,
        pos: Pos::new(spot.x, spot.y, spot.z),
        z_rot: yaw.unwrap_or(facing),
        scale: 1.0,
        item_id: item_instance.id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plant() -> Plantable {
        Plantable {
            item: 15646,
            doodad: 2246,
            use_skill: 25536,
            use_range: 2.0,
            cooldown_ms: 0,
            cooldown_tag: 0,
            start_group: 4366,
            model: Some("objects/env/05_nature/00_common/common_plants_rice01.cgf".into()),
            footprint: 0.5,
            requires_water: false,
            grounding: Grounding::default(),
        }
    }

    fn catalog() -> Catalog {
        let mut c = Catalog::default();
        c.insert(plant());
        c
    }

    #[test]
    fn the_catalog_maps_an_item_to_its_doodad_template() {
        let c = catalog();
        let p = c.plantable(15646).expect("rice seed");
        assert_eq!((p.doodad, p.start_group), (2246, 4366));
        assert_eq!(c.footprint(2246), 0.5);
        assert_eq!(c.plantable(1), None);
    }

    #[test]
    fn grounding_follows_the_floor_and_applies_the_auto_z_offset_only_when_set() {
        let hit = Vec3::new(100.0, 200.0, 12.0);
        let flat = plant();
        assert_eq!(grounded(hit, 10.0, None, &flat), Vec3::new(100.0, 200.0, 10.0));
        let mut offset = flat.clone();
        offset.grounding = Grounding { auto_z: true, offset: Vec3::new(1.0, -2.0, 0.5) };
        assert_eq!(grounded(hit, 10.0, None, &offset), Vec3::new(101.0, 198.0, 10.5));
        let mut water_plant = flat.clone();
        water_plant.requires_water = true;
        assert_eq!(grounded(hit, 10.0, Some(12.5), &water_plant).z, 12.5, "the plant sits on the water surface");
    }

    #[test]
    fn validity_checks_range_water_and_the_footprints_of_other_doodads() {
        let c = catalog();
        let p = plant();
        let doodads = Doodads::default();
        let me = Vec3::new(100.0, 200.0, 10.0);
        let near = Vec3::new(101.0, 200.0, 10.0);
        assert_eq!(check(&p, near, me, &doodads, &c, None, None), Ok(()));
        assert_eq!(check(&p, Vec3::new(103.0, 200.0, 10.0), me, &doodads, &c, None, None), Err(Invalid::OutOfRange));
        assert_eq!(check(&p, near, me, &doodads, &c, Some(12.0), None), Err(Invalid::Underwater));
        let mut wet = p.clone();
        wet.requires_water = true;
        assert_eq!(check(&wet, near, me, &doodads, &c, None, None), Err(Invalid::NotWater));
        assert_eq!(check(&wet, near, me, &doodads, &c, Some(10.5), None), Ok(()));

        let mut others = Doodads::default();
        let occupied = aa_net::model::Doodad {
            obj_id: 7,
            template_id: 2246,
            owner_obj_id: 0,
            parent_obj_id: 0,
            attach_point: 0,
            pos: Pos::new(101.4, 200.0, 10.0),
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: 1.0,
            has_loot_item: false,
            func_group_id: 4366,
            owner_id: 0,
            ucc_id: 0,
            item_template_id: 0,
            type2: 0,
            time_left: 0,
            plant_time: 0,
            quest_glow: 0,
            family: 0,
            puzzle_group: 0,
            owner_type: 254,
            owner_db_id: 0,
            data: 0,
        };
        others.by_id.insert(7, occupied);
        assert_eq!(check(&p, Vec3::new(100.5, 200.0, 10.0), me, &others, &c, None, None), Err(Invalid::Footprint));
        assert_eq!(check(&p, Vec3::new(100.0, 200.0, 10.0), me, &others, &c, None, None), Ok(()), "1.4 m is outside 1.0 m of footprints");
        // a static world doodad (the system's own) does not block the player's planting
        let mut system = Doodads::default();
        let mut world = others.by_id[&7].clone();
        world.owner_type = 255;
        system.by_id.insert(8, world);
        assert_eq!(check(&p, Vec3::new(100.5, 200.0, 10.0), me, &system, &c, None, None), Ok(()));
    }

    #[test]
    fn the_confirmation_is_a_create_doodad_with_the_planted_item() {
        let p = plant();
        let mut session = Session::new(77, p, 1.5);
        assert_eq!(session.request(), None, "no spot yet");
        session.spot = Some(Vec3::new(1000.5, 2000.25, 100.0));
        session.invalid = None;
        let request = session.request().unwrap();
        assert_eq!(
            request,
            CreateDoodad { id: 2246, pos: Pos::new(1000.5, 2000.25, 100.0), z_rot: 1.5, scale: 1.0, item_id: 77 }
        );
        let mut w = aa_net::wire::Writer::new();
        request.write(&mut w);
        let mut expected = aa_net::wire::Writer::new();
        expected
            .raw(&2246u32.to_le_bytes())
            .raw(&aa_net::wire::coord_to_long(1000.5).to_le_bytes())
            .raw(&aa_net::wire::coord_to_long(2000.25).to_le_bytes())
            .raw(&100.0f32.to_le_bytes())
            .raw(&1.5f32.to_le_bytes())
            .raw(&1.0f32.to_le_bytes())
            .raw(&77u64.to_le_bytes());
        assert_eq!(w.finish(), expected.finish());
    }

    #[test]
    fn a_road_surface_is_not_plantable_and_the_retail_errors_match_the_capture() {
        let c = catalog();
        let p = plant();
        let doodads = Doodads::default();
        let me = Vec3::new(100.0, 200.0, 10.0);
        let spot = Vec3::new(101.0, 200.0, 10.0);
        assert_eq!(check(&p, spot, me, &doodads, &c, None, None), Ok(()));
        assert_eq!(check(&p, spot, me, &doodads, &c, None, Some(SurfaceKind::Stone)), Err(Invalid::BadSurface));
        assert_eq!(check(&p, spot, me, &doodads, &c, None, Some(SurfaceKind::Metal)), Err(Invalid::BadSurface));
        assert_eq!(check(&p, spot, me, &doodads, &c, None, Some(SurfaceKind::Grass)), Ok(()));
        assert_eq!(error_message(Invalid::BadSurface), Some("Can't be installed here."));
        assert_eq!(error_message(Invalid::Footprint), Some("Too close to other objects."));
        assert_eq!(error_message(Invalid::OutOfRange), None);
    }

    #[test]
    fn the_ring_follows_the_footprint_and_a_blocked_petal_splits_the_ring() {
        let rice = ring_shape(&plant());
        let mut tree = plant();
        tree.footprint = 2.0;
        let tree = ring_shape(&tree);
        assert!(tree.0 > rice.0 && tree.1 > rice.1, "a tree ring is larger: {rice:?} vs {tree:?}");
        let c = catalog();
        let mut doodads = Doodads::default();
        let mut other = aa_net::model::Doodad {
            obj_id: 9,
            template_id: 2246,
            owner_obj_id: 0,
            parent_obj_id: 0,
            attach_point: 0,
            pos: Pos::new(101.2, 200.0, 10.0),
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: 1.0,
            has_loot_item: false,
            func_group_id: 4366,
            owner_id: 0,
            ucc_id: 0,
            item_template_id: 0,
            type2: 0,
            time_left: 0,
            plant_time: 0,
            quest_glow: 0,
            family: 0,
            puzzle_group: 0,
            owner_type: 254,
            owner_db_id: 0,
            data: 0,
        };
        doodads.by_id.insert(9, other.clone());
        // the centre is outside the other's footprint but the near petals touch it
        assert_eq!(check(&plant(), Vec3::new(100.0, 200.0, 10.0), Vec3::new(100.0, 200.0, 10.0), &doodads, &c, None, None), Ok(()));
        assert!(ring_blocked(&plant(), Vec3::new(100.0, 200.0, 10.0), &doodads, &c).is_some());
        other.pos = Pos::new(140.0, 200.0, 10.0);
        doodads.by_id.insert(9, other);
        assert!(ring_blocked(&plant(), Vec3::new(100.0, 200.0, 10.0), &doodads, &c).is_none());
    }

    #[test]
    fn the_initial_yaw_is_wrapped_like_a_heading() {
        assert!((wrap_angle(0.0) - 0.0).abs() < 1e-6);
        assert!((wrap_angle(std::f32::consts::TAU + 1.0) - 1.0).abs() < 1e-5);
        assert!((wrap_angle(-std::f32::consts::TAU - 1.0) + 1.0).abs() < 1e-5);
    }

    #[test]
    fn the_ring_geometry_is_non_empty_and_in_bevy_space() {
        let p = plant();
        let c = catalog();
        let doodads = Doodads::default();
        let spot = Vec3::new(100.0, 200.0, 10.0);
        let (yellow, red) = ring_quads(&p, spot, &doodads, &c, |_| 10.0, false);
        assert!(!yellow.is_empty(), "a valid footprint has a petal ring");
        assert!(red.is_empty());
        for q in &yellow {
            for v in q.p {
                assert!((v.y - 10.045).abs() < 1e-3, "Bevy Y is the ground height: {v:?}");
                assert!((v.x - 100.0).abs() < 2.0 && (v.z + 200.0).abs() < 2.0, "game y -> Bevy -z: {v:?}");
            }
        }
        let (yellow_red, red_all) = ring_quads(&p, spot, &doodads, &c, |_| 10.0, true);
        assert!(yellow_red.is_empty() && !red_all.is_empty(), "an invalid spot rings red");
        let moved = Vec3::new(100.2, 200.0, 10.0);
        let (moved_quads, _) = ring_quads(&p, moved, &doodads, &c, |_| 10.0, false);
        assert_ne!(quads_signature(&yellow), quads_signature(&moved_quads), "a new spot changes the upload signature");
        assert_eq!(quads_signature(&yellow), quads_signature(&yellow.clone()));
    }

    #[test]
    fn the_real_database_maps_seeds_and_saplings() {
        let Ok(path) = std::env::var("AA_DB") else { return };
        if !std::path::Path::new(&path).exists() {
            return;
        }
        let Some(db) = Db::open() else { return };
        let catalog = Catalog::load(&db).expect("catalog");
        let rice = catalog.plantable(15646).expect("rice seed in item_spawn_doodads");
        assert_eq!((rice.doodad, rice.start_group, rice.use_skill), (2246, 4366, 25536));
        assert_eq!(rice.use_range, 2.0);
        assert!(rice.model.as_deref().is_some_and(|m| m.ends_with("common_plants_rice01.cgf")));
        let spruce = catalog.plantable(14899).expect("spruce sapling");
        assert_eq!((spruce.doodad, spruce.start_group), (367, 2631));
        assert_eq!(catalog.footprint(367), 2.0);
        // the capture shows the full-grown spruce, not the start-phase `grow03` sprout: the template's own model
        assert!(spruce.model.as_deref().is_some_and(|m| m.ends_with("sub_tree_bigfir02.cgf")), "{:?}", spruce.model);
        assert_ne!(spruce.model.as_deref(), Some("objects/env/05_nature/04_subpol/sub_tree_bigfir01_grow03.cgf"));
        let _ = db;
    }

    #[test]
    fn the_placement_and_farm_plugins_validate_and_tick() {
        // The binary's full plugin set needs a window and the renderer; this smoke App keeps the real placement
        // and farm plugins (and every resource their systems read) so Bevy validates every system parameter and
        // the startup/update schedules, which is what the B0002 conflict needed.
        use bevy::asset::AssetPlugin;
        use bevy::state::app::StatesPlugin;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::input::InputPlugin, StatesPlugin))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<bevy::image::Image>()
            .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
            .add_plugins(aa_cry::CryPlugin)
            .add_plugins(bevy::camera::visibility::VisibilityPlugin)
            .init_state::<crate::Screen>()
            .add_plugins(plugin)
            .add_plugins(crate::hud::farm::plugin)
            .add_message::<aa_net::NetCommand>()
            .add_message::<aa_net::NetEvent>()
            .init_resource::<aa_net::Farms>()
            .init_resource::<aa_net::state::Doodads>()
            .init_resource::<aa_net::state::Inventory>()
            .init_resource::<aa_net::state::Skills>()
            .init_resource::<crate::hud::HudPointerOverUi>()
            .init_resource::<crate::hud::HudKeyboardFocus>()
            .init_resource::<crate::hud::HudBuilt>()
            .insert_non_send(crate::hud::api::HudBridge(std::rc::Rc::new(std::cell::RefCell::new(
                crate::hud::api::HudData::default(),
            ))));
        let plant = plant();
        app.insert_resource(Placement { session: Some(Session::new(1, plant, 0.0)) });
        app.world_mut().resource_mut::<NextState<crate::Screen>>().set(crate::Screen::InGame);
        for _ in 0..4 {
            app.update();
        }
        assert!(app.world().resource::<Placement>().session.is_some(), "placement survives the ticks");
        if aa_cry::pak_available() {
            let roots: Vec<Entity> = {
                let world = app.world_mut();
                let mut q = world.query_filtered::<Entity, With<Ghost>>();
                q.iter(world).collect()
            };
            assert_eq!(roots.len(), 1, "one ghost root");
            assert_eq!(app.world().get::<Visibility>(roots[0]), Some(&Visibility::Visible), "the ghost root is visible");
            let visible_parts = {
                let world = app.world_mut();
                let mut q = world.query_filtered::<&Visibility, With<Mesh3d>>();
                q.iter(world).filter(|v| **v != Visibility::Hidden).count()
            };
            assert!(visible_parts > 0, "the ghost model's meshes inherit the visible root");
        }
    }

    #[test]
    fn the_ghost_model_loads_with_mesh_parts() {
        if !aa_cry::pak_available() {
            return;
        }
        use bevy::asset::AssetPlugin;
        #[derive(Resource, Default)]
        struct Parts(usize);
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<bevy::image::Image>()
            .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
            .add_plugins(aa_cry::CryPlugin)
            .init_resource::<Parts>()
            .add_systems(Startup, |mut cry: CryAssets, mut out: ResMut<Parts>| {
                if let Ok(loaded) = cry.load_model("objects/env/05_nature/00_common/common_plants_rice01.cgf", &ModelOptions::default()) {
                    out.0 = loaded.parts.iter().map(|(_, parts)| parts.len()).sum();
                }
            });
        app.update();
        assert!(app.world().resource::<Parts>().0 > 0, "the template model has meshes to show as a ghost");
    }
}
