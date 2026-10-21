//! Placement: using a seed, sapling or livestock item starts a placement with a ghost of the doodad's start phase
//! under the cursor; a left click sends `CSCreateDoodad`, Escape / right click cancels. A house / farm design item
//! (`item_housings`) uses the same session: its ghost is the housing model, the plot is an axis-aligned square of
//! `garden_radius`, and a valid left click asks `CSConstructHouseTax` for the original "Start Construction" window
//! (see `hud/houses.rs`) instead of creating anything itself.
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
use aa_net::housing::{ConstructHouseTax, HouseCommand};
use aa_net::state::{Doodads, Houses, Inventory, Skills};
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
use super::houses::HousingAreas;
use super::player::{LocalPlayer, Mover};
use super::virtual_input::VirtualInput;
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
    /// A design item (`item_housings`): the placement is a house plot, not a doodad.
    pub house: Option<HouseDesign>,
}

/// A house / farm design (`item_housings.item_id` -> `housings.id`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HouseDesign {
    /// `housings.id`, the design id the server calls `design_id`.
    pub design: u32,
    pub name: String,
    /// `housings.garden_radius`: the half extent of the axis-aligned plot square (m).
    pub garden_radius: f32,
    pub heavy_tax: bool,
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
    /// `housings.id` (design) -> plot radius and name.
    houses: HashMap<u32, HouseDesign>,
    /// `housing_areas.id` the server knows (the client's `housing_area.xml` tags).
    area_ids: std::collections::HashSet<u32>,
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
                    house: None,
                });
            }
        }
        // Design items: `item_housings.item_id` -> `housings` (the server's design and plot radius).
        {
            let mut q = conn
                .prepare(
                    "SELECT ih.item_id, h.id, COALESCE(l.en_us, h.name), h.garden_radius, h.main_model_id, \
                     i.use_skill_id, s.cooldown_time, s.cooldown_tag_id, \
                     CASE WHEN h.auto_z IN ('t', 'T', 1) THEN 1 ELSE 0 END, \
                     h.auto_z_offset_x, h.auto_z_offset_y, h.auto_z_offset_z, \
                     CASE WHEN h.heavy_tax IN ('t', 'T', 1) THEN 1 ELSE 0 END \
                     FROM item_housings ih JOIN housings h ON h.id = ih.design_id \
                     LEFT JOIN items i ON i.id = ih.item_id \
                     LEFT JOIN skills s ON s.id = i.use_skill_id \
                     LEFT JOIN localized_texts l ON l.tbl_name = 'housings' AND l.tbl_column_name = 'name' AND l.idx = h.id",
                )
                .ok()?;
            let mut rows = q.query([]).ok()?;
            while let Ok(Some(r)) = rows.next() {
                let item: u32 = r.get::<_, Option<i64>>(0).unwrap_or_default().unwrap_or(0).max(0) as u32;
                let design: u32 = r.get::<_, Option<i64>>(1).unwrap_or_default().unwrap_or(0).max(0) as u32;
                if item == 0 || design == 0 {
                    continue;
                }
                let name: String = r.get::<_, Option<String>>(2).unwrap_or_default().unwrap_or_default();
                let garden: f32 = r.get::<_, Option<f64>>(3).unwrap_or_default().unwrap_or(0.0) as f32;
                let model_id: u32 = r.get::<_, Option<i64>>(4).unwrap_or_default().unwrap_or(0).max(0) as u32;
                let use_skill: u32 = r.get::<_, Option<i64>>(5).unwrap_or_default().unwrap_or(0).max(0) as u32;
                let cooldown_ms: u32 = r.get::<_, Option<i64>>(6).unwrap_or_default().unwrap_or(0).max(0) as u32;
                let cooldown_tag: u32 = r.get::<_, Option<i64>>(7).unwrap_or_default().unwrap_or(0).max(0) as u32;
                let auto_z: bool = r.get::<_, Option<i64>>(8).unwrap_or_default().unwrap_or(0) != 0;
                let (ox, oy, oz) = (
                    r.get::<_, Option<f64>>(9).unwrap_or_default().unwrap_or(0.0) as f32,
                    r.get::<_, Option<f64>>(10).unwrap_or_default().unwrap_or(0.0) as f32,
                    r.get::<_, Option<f64>>(11).unwrap_or_default().unwrap_or(0.0) as f32,
                );
                let heavy_tax: bool = r.get::<_, Option<i64>>(12).unwrap_or_default().unwrap_or(0) != 0;
                let house = HouseDesign { design, name: name.clone(), garden_radius: garden, heavy_tax };
                catalog.houses.insert(design, house.clone());
                catalog.by_item.entry(item).or_insert(Plantable {
                    item,
                    doodad: 0,
                    use_skill,
                    // the server does not range-check a house placement: the plot is valid anywhere in the
                    // housing area
                    use_range: 0.0,
                    cooldown_ms,
                    cooldown_tag,
                    start_group: 0,
                    model: db.and_then(|db| db.actor_model_file(model_id)),
                    footprint: garden,
                    requires_water: false,
                    grounding: Grounding { auto_z, offset: Vec3::new(ox, oy, oz) },
                    house: Some(house),
                });
            }
        }
        // The housing polygons' `value1` tags must be rows the server knows, or `validate_house` refuses them.
        if let Ok(mut q) = conn.prepare("SELECT id FROM housing_areas")
            && let Ok(mut rows) = q.query([])
        {
            while let Ok(Some(r)) = rows.next() {
                if let Ok(id) = r.get::<_, i64>(0) {
                    catalog.area_ids.insert(id.max(0) as u32);
                }
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

    /// The plot half extent of a housing design (m).
    pub fn house_radius(&self, design: u32) -> f32 {
        self.houses.get(&design).map_or(0.0, |h| h.garden_radius)
    }

    /// The housing area tags the server's `housing_areas` table knows.
    pub fn area_known(&self, id: u32) -> bool {
        self.area_ids.contains(&id)
    }

    #[cfg(test)]
    fn insert(&mut self, plant: Plantable) {
        self.radius.insert(plant.doodad, plant.footprint);
        if let Some(house) = &plant.house {
            self.houses.insert(house.design, house.clone());
        }
        self.by_item.insert(plant.item, plant);
    }
}

/// A planting item was used (the HUD sends this instead of a skill press).
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlantPressed {
    pub item_id: u64,
    pub item_template: u32,
}

/// A house / farm design item was used (`item_housings`): the HUD sends this instead of its use skill.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HousePressed {
    pub item_id: u64,
    pub item_template: u32,
}

/// Why a spot cannot take a plant or a house.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Invalid {
    NoGround,
    OutOfRange,
    NotWater,
    Underwater,
    Footprint,
    BadSurface,
    OutsideHousingArea,
    Overlap,
    Player,
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
            Self::OutsideHousingArea => "outside a housing area",
            Self::Overlap => "another house is in the way",
            Self::Player => "the player is in the way",
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
    /// The left button was released after the session started: only then may a click confirm (the bag click that
    /// used the seed must not carry into the placement).
    left_armed: bool,
    /// House only: the tax quote asked from the server, its answer, and the original build window.
    pub quote_requested: bool,
    pub quote: Option<ConstructHouseTax>,
    pub window_open: bool,
    /// House only: the create was sent (the ghost goes when the frame is done).
    pub done: bool,
    /// House only: the server's `SCMyHouse` for this build arrived (kept so `finish` does not leave a
    /// stale pending design when the answer beat it to the frame).
    pub created: bool,
    /// A spot set by a script (`house_place`): the cursor may not move it off-screen in a headless run.
    pub scripted: bool,
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
            left_armed: false,
            quote_requested: false,
            quote: None,
            window_open: false,
            done: false,
            created: false,
            scripted: false,
        }
    }

    pub fn valid(&self) -> bool {
        self.invalid.is_none() && self.spot.is_some()
    }

    pub fn is_house(&self) -> bool {
        self.plant.house.is_some()
    }

    pub fn house_design(&self) -> Option<u32> {
        self.plant.house.as_ref().map(|h| h.design)
    }

    /// The request a doodad confirmation sends; `None` for a house or while the spot is not valid.
    pub fn request(&self) -> Option<CreateDoodad> {
        if self.is_house() || !self.valid() {
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

    /// `CSConstructHouseTax`: ask the server for the retail build window's numbers.
    pub fn tax_request(&self) -> Option<HouseCommand> {
        if !self.valid() {
            return None;
        }
        let design = self.house_design()?;
        let spot = self.spot?;
        Some(HouseCommand::ConstructHouseTax { design_id: design, x: spot.x, y: spot.y, z: spot.z })
    }

    /// Why the confirmed house cannot be sent, for the one WARN the Build button logs.
    pub fn build_blocked(&self) -> Option<&'static str> {
        if !self.is_house() {
            return Some("the active placement is not a house");
        }
        if self.done {
            return Some("the build was already confirmed");
        }
        if !self.quote_requested {
            return Some("the placement was not confirmed (no tax quote was requested)");
        }
        if self.spot.is_none() {
            return Some("the confirmed spot is missing");
        }
        if self.quote.is_none() {
            return Some("the server's tax quote has not arrived");
        }
        None
    }

    /// `CSCreateHouse`: the build window's Build button. It uses the confirmed spot, not the live cursor
    /// validity: the pointer may be over the window and the preview locked when this is called.
    pub fn create_house(&self) -> Option<HouseCommand> {
        if self.build_blocked().is_some() {
            return None;
        }
        let design = self.house_design()?;
        let spot = self.spot?;
        let quote = self.quote.as_ref()?;
        Some(HouseCommand::CreateHouse {
            design_id: design,
            x: spot.x,
            y: spot.y,
            z: spot.z,
            z_rot: self.yaw,
            item_id: self.item_id,
            money_amount: quote.total_tax_money_amount,
            ht: 0,
            auto_use_aa_point: false,
        })
    }
}

#[derive(Resource, Default)]
pub struct Placement {
    pub session: Option<Session>,
    /// Design of a house create that was sent and is waiting for the server's `SCMyHouse`; the ghost is
    /// already gone, so this keeps enough identity to close the original build window on the answer.
    pub pending_house: Option<u32>,
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
    stone: Option<Handle<StandardMaterial>>,
    post: Option<Handle<StandardMaterial>>,
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
    /// The house plot's flat grey stones.
    stones: Option<RingPart>,
    /// The housing area's small yellow posts.
    posts: Option<RingPart>,
}

pub fn plugin(app: &mut App) {
    app.add_message::<PlantPressed>()
        .add_message::<HousePressed>()
        .init_resource::<Placement>()
        .init_resource::<Visuals>()
        .insert_resource(Catalog::default())
        .add_systems(Startup, load_catalog)
        .add_systems(PreUpdate, input.after(bevy::input::InputSystems).run_if(in_state(Screen::InGame)))
        .add_systems(Update, (start, finish, track, cursor_icon, log_server).chain().run_if(in_state(Screen::InGame)))
        .add_systems(OnExit(Screen::InGame), leave);
}

/// A house create was sent: put the ghost, ring, stones and arrow away in this frame.
fn finish(mut commands: Commands, mut placement: ResMut<Placement>, visuals: Res<Visuals>) {
    if placement.session.as_ref().is_some_and(|s| s.done) {
        if let Some(session) = placement.session.as_ref()
            && !session.created
            && let Some(design) = session.house_design()
        {
            placement.pending_house = Some(design);
        }
        end_session(&mut commands, &mut placement, &visuals);
    }
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
    if plant.use_range > 0.0 && spot.distance(player) > plant.use_range {
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

/// Validity of a house plot, mirroring `systems/housing/placement.rs`: inside a housing area the server knows,
/// clear of other houses as an axis-aligned square (half extent = each template's `garden_radius`), on land, and
/// not on the player.
pub fn check_house(
    design: &HouseDesign,
    spot: Vec3,
    player: Vec3,
    houses: &Houses,
    catalog: &Catalog,
    areas: Option<&HousingAreas>,
    water: Option<f32>,
) -> Result<(), Invalid> {
    if Vec2::new(spot.x - player.x, spot.y - player.y).length() < 0.6 {
        return Err(Invalid::Player);
    }
    if water.is_some_and(|h| h > spot.z + 0.05) {
        return Err(Invalid::NoGround);
    }
    let Some(areas) = areas else { return Err(Invalid::OutsideHousingArea) };
    let tags = areas.area_at(spot.x, spot.y);
    if tags.is_empty() || !tags.iter().any(|id| catalog.area_known(*id)) {
        return Err(Invalid::OutsideHousingArea);
    }
    let extent = design.garden_radius.max(0.0);
    for house in houses.by_tl_id.values() {
        if house.demolished {
            continue;
        }
        let p = house.snapshot.position();
        let other = catalog.house_radius(house.snapshot.template_id);
        let reach = extent + other;
        if (spot.x - p.x).abs() < reach && (spot.y - p.y).abs() < reach {
            return Err(Invalid::Overlap);
        }
    }
    Ok(())
}

/// Ends the placement: the ghost goes with it.
fn end_session(commands: &mut Commands, placement: &mut Placement, visuals: &Visuals) {
    if let Some(session) = placement.session.take()
        && let Some(root) = session.root
    {
        commands.entity(root).despawn();
    }
    hide_ring(commands, visuals);
    if let Some(arrow) = visuals.arrow {
        commands.entity(arrow).insert(Visibility::Hidden);
    }
}

/// Starts a placement when the HUD reports a seed use.
#[allow(clippy::too_many_arguments)]
fn start(
    mut commands: Commands,
    mut pressed: MessageReader<PlantPressed>,
    mut houses: MessageReader<HousePressed>,
    catalog: Res<Catalog>,
    mut placement: ResMut<Placement>,
    player: Query<&Mover, With<LocalPlayer>>,
    camera: Query<&OrbitCamera>,
    visuals: Res<Visuals>,
    mut virtual_input: ResMut<VirtualInput>,
) {
    let session_for = |item_id: u64, template: u32, kind: &str| -> Option<Session> {
        let Some(plant) = catalog.plantable(template).cloned() else {
            warn!("{kind}: item {item_id} (template {template}) has no placement data");
            return None;
        };
        // the retail client's first yaw is the camera heading (two captures: 4.2020082 and 3.166932 rad)
        let yaw = camera.single().map(|c| c.yaw).or_else(|_| player.single().map(|m| m.facing)).unwrap_or(0.0);
        match &plant.house {
            Some(house) => info!(
                "{kind}: {} (item {}) places house {} \"{}\" (plot {:.1} m, model {:?})",
                template, item_id, house.design, house.name, house.garden_radius, plant.model
            ),
            None => info!(
                "{kind}: {} (item {}) places doodad {} (phase {}, model {:?}, range {:.1} m, footprint {:.2} m)",
                template, item_id, plant.doodad, plant.start_group, plant.model, plant.use_range, plant.footprint
            ),
        }
        Some(Session::new(item_id, plant, yaw))
    };
    for press in pressed.read() {
        if let Some(session) = session_for(press.item_id, press.item_template, "planting") {
            end_session(&mut commands, &mut placement, &visuals);
            virtual_input.clear_edges();
            placement.pending_house = None;
            placement.session = Some(session);
        }
    }
    for press in houses.read() {
        if let Some(session) = session_for(press.item_id, press.item_template, "housing") {
            end_session(&mut commands, &mut placement, &visuals);
            virtual_input.clear_edges();
            placement.pending_house = None;
            placement.session = Some(session);
        }
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
    houses: Res<Houses>,
    areas: Option<Res<HousingAreas>>,
    over_ui: Option<Res<HudPointerOverUi>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    collision: Option<Res<WorldCollision>>,
    terrain: Option<Res<TerrainQuery>>,
    water: Option<Res<WaterQuery>>,
    mut visuals: ResMut<Visuals>,
    virtual_input: Res<VirtualInput>,
) {
    ensure_visuals(&mut cry.images, &mut cry.meshes, &mut cry.materials, &mut visuals);
    let Some(session) = placement.session.as_mut() else { return };
    let root = ensure_ghost(&mut commands, &mut cry, session);
    let Some(mover) = player.single().ok() else { return };
    let raw_cursor = virtual_input.cursor_position(windows.single().ok());
    let over_ui = over_ui.is_some_and(|o| o.0);
    let cursor = if over_ui { None } else { raw_cursor };
    let house = session.is_house();
    // confirmed for the Start Construction window: the spot and yaw stay locked until Cancel. The pointer
    // over the HUD also neither moves nor re-validates a house preview.
    if house_locks_pointer(session, over_ui) {
        show_ghost(&mut commands, root, session);
        update_ring(&mut commands, &mut cry.meshes, &mut visuals, session, &doodads, &houses, &catalog, areas.as_deref(), collision.as_deref(), terrain.as_deref());
        return;
    }
    // a scripted house placement keeps its spot without a pointer (`--offscreen-shot`, headless runs)
    if house && session.scripted && raw_cursor.is_none() {
        show_ghost(&mut commands, root, session);
        update_ring(&mut commands, &mut cry.meshes, &mut visuals, session, &doodads, &houses, &catalog, areas.as_deref(), collision.as_deref(), terrain.as_deref());
        return;
    }
    let hit = cursor.and_then(|c| {
        let (camera, transform) = cameras.iter().find(|(c, _)| c.is_active)?;
        spot_under(camera, transform, c, collision.as_deref(), terrain.as_deref())
    });
    let Some(hit) = hit else {
        // the retail house ghost stays at its last valid spot when the cursor leaves the ground or points at
        // the UI; plants hide instead (their capture shows no ghost over the sky)
        if house && session.spot.is_some() {
            if raw_cursor.is_some() && session.invalid != Some(Invalid::NoGround) {
                session.invalid = Some(Invalid::NoGround);
                let p = session.spot.unwrap_or_default();
                info!("housing: spot ({:.1}, {:.1}, {:.1}) no ground under the cursor", p.x, p.y, p.z);
            }
            show_ghost(&mut commands, root, session);
            update_ring(&mut commands, &mut cry.meshes, &mut visuals, session, &doodads, &houses, &catalog, areas.as_deref(), collision.as_deref(), terrain.as_deref());
            return;
        }
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
    session.invalid = match &session.plant.house {
        Some(design) => check_house(design, spot, mover.pos, &houses, &catalog, areas.as_deref(), wet).err(),
        None => {
            let mut invalid = check(&session.plant, spot, mover.pos, &doodads, &catalog, wet, surface).err();
            if invalid.is_none() && ring_blocked(&session.plant, spot, &doodads, &catalog).is_some() {
                invalid = Some(Invalid::Footprint);
            }
            invalid
        }
    };
    show_ghost(&mut commands, root, session);
    if session.invalid != before.1 {
        info!(
            "{}: spot ({:.1}, {:.1}, {:.1}) {}",
            if house { "housing" } else { "planting" },
            spot.x,
            spot.y,
            spot.z,
            session.invalid.map_or("is valid", Invalid::text)
        );
    } else if session.spot != before.0 {
        debug!(
            "{}: spot ({:.1}, {:.1}, {:.1}) {}",
            if house { "housing" } else { "planting" },
            spot.x,
            spot.y,
            spot.z,
            session.invalid.map_or("is valid", Invalid::text)
        );
    }
    update_ring(&mut commands, &mut cry.meshes, &mut visuals, session, &doodads, &houses, &catalog, areas.as_deref(), collision.as_deref(), terrain.as_deref());
}

/// A house preview stops following the pointer once confirmed (the Start Construction window is open) and
/// whenever the pointer is over the HUD.
fn house_locks_pointer(session: &Session, over_ui: bool) -> bool {
    session.is_house() && (session.quote_requested || over_ui)
}

/// Puts the ghost root at the session's spot (the last valid one for a house over the UI).
fn show_ghost(commands: &mut Commands, root: Option<Entity>, session: &Session) {
    let (Some(root), Some(spot)) = (root, session.spot) else { return };
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
    let (radius, count, _) = ring_shape(plant);
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

/// Ring radius, petal count and petal half-length for a template's footprint. Retail keeps the ring close to the
/// doodad (rice 0.5 m -> a small sparse ring, spruce 2 m -> a proportionate one) and the petals much smaller than
/// the old whole-leaf quads.
fn ring_shape(plant: &Plantable) -> (f32, usize, f32) {
    let radius = (0.12 + plant.footprint).clamp(0.50, 2.6);
    let count = (14.0 + radius * 9.0).clamp(14.0, 36.0) as usize;
    let half_len = 0.04 + radius * 0.045;
    (radius, count, half_len)
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
    let (radius, count, half_len) = ring_shape(plant);
    let half_w = half_len * 0.45;
    let mut yellow = Vec::with_capacity(count);
    let mut red = Vec::with_capacity(count);
    for i in 0..count {
        let jitter = (((i * 37) % 101) as f32 / 101.0 - 0.5) * 0.22;
        let ang = i as f32 / count as f32 * std::f32::consts::TAU + jitter;
        let dir = Vec2::new(ang.cos(), ang.sin());
        let side = Vec2::new(-dir.y, dir.x);
        let radius_jitter = (((i * 53) % 97) as f32 / 97.0 - 0.5) * 0.10;
        let centre = Vec2::new(spot.x, spot.y) + dir * (radius + radius_jitter);
        let z = floor(centre) + 0.09;
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
    houses: &Houses,
    catalog: &Catalog,
    areas: Option<&HousingAreas>,
    collision: Option<&WorldCollision>,
    terrain: Option<&TerrainQuery>,
) {
    let _ = houses;
    if !visuals.ready {
        return;
    }
    let Some(spot) = session.spot else {
        hide_ring(commands, visuals);
        return;
    };
    let floor = |p: Vec2| ground_at(collision, terrain, p.x, p.y, spot.z, spot.z);
    if session.is_house() {
        let stones = stone_quads(&session.plant, spot, session.yaw, floor);
        let posts = areas.map_or_else(Vec::new, |areas| post_quads(areas, spot, floor));
        let (Some(stone_mat), Some(post_mat)) = (visuals.stone.clone(), visuals.post.clone()) else { return };
        let empty: [Quad; 0] = [];
        if let Some(mat) = visuals.yellow.clone() {
            show_ring_part(commands, meshes, &mut visuals.ring.yellow, &mat, "planting ring (valid)", &empty);
        }
        if let Some(mat) = visuals.red.clone() {
            show_ring_part(commands, meshes, &mut visuals.ring.red, &mat, "planting ring (blocked)", &empty);
        }
        show_ring_part(commands, meshes, &mut visuals.ring.stones, &stone_mat, "housing plot stones", &stones);
        show_ring_part(commands, meshes, &mut visuals.ring.posts, &post_mat, "housing area posts", &posts);
        return;
    }
    let all_red = matches!(session.invalid, Some(i) if i != Invalid::Footprint);
    let (yellow, red) = ring_quads(&session.plant, spot, doodads, catalog, floor, all_red);
    let (Some(yellow_mat), Some(red_mat)) = (visuals.yellow.clone(), visuals.red.clone()) else { return };
    show_ring_part(commands, meshes, &mut visuals.ring.yellow, &yellow_mat, "planting ring (valid)", &yellow);
    show_ring_part(commands, meshes, &mut visuals.ring.red, &red_mat, "planting ring (blocked)", &red);
    if let Some(mat) = visuals.stone.clone() {
        show_ring_part(commands, meshes, &mut visuals.ring.stones, &mat, "housing plot stones", &[]);
    }
    if let Some(mat) = visuals.post.clone() {
        show_ring_part(commands, meshes, &mut visuals.ring.posts, &mat, "housing area posts", &[]);
    }
}

/// The flat grey stones along the plot square (half extent = the housing's `garden_radius`), turned with the
/// ghost: the retail capture's boundary.
fn stone_quads(plant: &Plantable, spot: Vec3, yaw: f32, floor: impl Fn(Vec2) -> f32) -> Vec<Quad> {
    let half = plant.footprint.max(0.5);
    let (sin, cos) = yaw.sin_cos();
    let to_world = |u: f32, v: f32| Vec2::new(spot.x + u * cos - v * sin, spot.y + u * sin + v * cos);
    let corners = [(-half, -half), (half, -half), (half, half), (-half, half)];
    let spacing = 0.78;
    let mut out = Vec::new();
    for i in 0..4 {
        let (a, b) = (corners[i], corners[(i + 1) % 4]);
        let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        let n = (len / spacing).ceil().max(1.0) as usize;
        let dir = ((b.0 - a.0) / len, (b.1 - a.1) / len);
        let side = (-dir.1, dir.0);
        for k in 0..n {
            let t = (k as f32 + 0.5) / n as f32;
            let (u, v) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            let jitter = ((k * 37 + i * 11) % 13) as f32 / 13.0 - 0.5;
            let (wu, wv) = (u + side.0 * jitter * 0.06, v + side.1 * jitter * 0.06);
            let (hw, hd) = (0.30f32, 0.20f32);
            let local = [
                (wu - dir.0 * hw - side.0 * hd, wv - dir.1 * hw - side.1 * hd),
                (wu + dir.0 * hw - side.0 * hd, wv + dir.1 * hw - side.1 * hd),
                (wu + dir.0 * hw + side.0 * hd, wv + dir.1 * hw + side.1 * hd),
                (wu - dir.0 * hw + side.0 * hd, wv - dir.1 * hw + side.1 * hd),
            ];
            let p = local.map(|(u, v)| {
                let g = to_world(u, v);
                cry_to_bevy(Vec3::new(g.x, g.y, floor(g) + 0.025))
            });
            out.push(Quad { p, uv: [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]] });
        }
    }
    out
}

/// The small yellow posts along the housing area polygon that contains the spot (the retail boundary markers).
fn post_quads(areas: &HousingAreas, spot: Vec3, floor: impl Fn(Vec2) -> f32) -> Vec<Quad> {
    let Some(shape) = areas.shape_at(spot.x, spot.y) else { return Vec::new() };
    let mut out = Vec::new();
    let spacing = 5.5;
    for i in 0..shape.points.len() {
        let (a, b) = (shape.points[i], shape.points[(i + 1) % shape.points.len()]);
        let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        if len < 0.5 {
            continue;
        }
        let dir = ((b[0] - a[0]) / len, (b[1] - a[1]) / len);
        let n = (len / spacing).ceil().max(1.0) as usize;
        for k in 0..n {
            let t = (k as f32 + 0.5) / n as f32;
            let g = Vec2::new(a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t);
            if g.distance(Vec2::new(spot.x, spot.y)) > 80.0 {
                continue;
            }
            let z = floor(g) + 0.02;
            let base = cry_to_bevy(Vec3::new(g.x, g.y, z));
            let along = Vec3::new(dir.0, 0.0, -dir.1).normalize_or_zero();
            let across = Vec3::new(-dir.1, 0.0, -dir.0).normalize_or_zero();
            let (half_w, height) = (0.045f32, 0.55);
            for side in [along, across] {
                let p = [
                    base - side * half_w,
                    base + side * half_w,
                    base + side * half_w + Vec3::Y * height,
                    base - side * half_w + Vec3::Y * height,
                ];
                out.push(Quad { p, uv: [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]] });
            }
        }
    }
    out
}

fn hide_ring(commands: &mut Commands, visuals: &Visuals) {
    for part in [
        visuals.ring.yellow.as_ref(),
        visuals.ring.red.as_ref(),
        visuals.ring.stones.as_ref(),
        visuals.ring.posts.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
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
            depth_bias: 20.0,
            ..default()
        };
        visuals.yellow = Some(materials.add(leaf_material(Color::srgba(1.0, 0.72, 0.12, 1.0))));
        visuals.red = Some(materials.add(leaf_material(Color::srgba(0.95, 0.10, 0.06, 1.0))));
        // the house plot boundary: the 1.2 capture's flat grey stones and yellow posts
        visuals.stone = Some(materials.add(StandardMaterial {
            base_color: Color::srgba(0.62, 0.61, 0.57, 1.0),
            unlit: true,
            cull_mode: None,
            ..default()
        }));
        visuals.post = Some(materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.83, 0.08, 1.0),
            unlit: true,
            cull_mode: None,
            alpha_mode: AlphaMode::Blend,
            depth_bias: 20.0,
            ..default()
        }));

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
#[allow(clippy::too_many_arguments)]
fn cursor_icon(
    mut commands: Commands,
    placement: Res<Placement>,
    mut visuals: ResMut<Visuals>,
    virtual_input: Res<VirtualInput>,
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
                            left: Val::Vw(62.0),
                            bottom: Val::Vh(9.5),
                            width: Val::Vw(5.0),
                            height: Val::Vh(4.5),
                            overflow: Overflow::visible(),
                            ..default()
                        },
                        UiTransform::from_translation(Val2::new(Val::Vw(-2.5), Val::Vh(0.0))),
                        UiTargetCamera(target),
                        Visibility::Inherited,
                        GlobalZIndex(100),
                    ))
                    .id();
                // the retail arrow has a dark outline; the road texture has none, so a darker copy sits behind it
                commands.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(-2.5),
                        top: Val::Percent(-5.0),
                        width: Val::Percent(105.0),
                        height: Val::Percent(110.0),
                        ..default()
                    },
                    ImageNode {
                        image: image.clone(),
                        color: Color::srgba(0.05, 0.005, 0.005, 1.0),
                        flip_x: true,
                        image_mode: NodeImageMode::Stretch,
                        ..default()
                    },
                    ChildOf(arrow),
                    GlobalZIndex(100),
                ));
                commands.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    ImageNode {
                        image,
                        color: Color::srgba(0.55, 0.015, 0.01, 0.98),
                        flip_x: true,
                        image_mode: NodeImageMode::Stretch,
                        ..default()
                    },
                    ChildOf(arrow),
                    GlobalZIndex(101),
                ));
                visuals.arrow = Some(arrow);
            }
        }
    }
    let Some(cursor) = virtual_input.cursor_position(Some(&window)) else {
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
    over_ui: Option<Res<HudPointerOverUi>>,
    focus: Option<Res<HudKeyboardFocus>>,
    mut placement: ResMut<Placement>,
    visuals: Res<Visuals>,
    mut virtual_input: ResMut<VirtualInput>,
    skills: Res<Skills>,
    rt: Option<NonSend<aa_ui::UiRuntime>>,
    mut net: MessageWriter<NetCommand>,
) {
    let Some(session) = placement.session.as_mut() else { return };
    let over_ui = over_ui.is_some_and(|o| o.0);
    // the tax window owns the pointer and Escape; Cancel closes it and keeps the placement (retail)
    if session.window_open || focus.is_some_and(|f| f.0) {
        return;
    }
    // picking an item up onto the cursor ends the placement mode (the retail client does the same)
    if rt.as_ref().is_some_and(|rt| aa_ui::lua::drag::holding(&rt.lua)) {
        info!("planting: an item is on the cursor, placement ends");
        end_session(&mut commands, &mut placement, &visuals);
        return;
    }
    let virtual_click = virtual_input.take_click();
    let virtual_keys = virtual_input.take_keys();
    let virtual_wheels = virtual_input.take_wheels();
    let mut cancel = keys.just_pressed(KeyCode::Escape) || virtual_keys.contains(&KeyCode::Escape);
    if cancel {
        keys.clear_just_pressed(KeyCode::Escape);
    }
    let (shift, alt, ctrl) = (
        keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]),
        keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]),
        keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]),
    );
    let mut real_wheel = 0.0;
    for ev in wheel.read() {
        real_wheel += wheel_turn(shift, alt, ctrl, ev.y);
    }
    let mut turn = real_wheel;
    if !virtual_wheels.is_empty() {
        turn = 0.0;
        for w in &virtual_wheels {
            turn += wheel_turn(w.shift, w.alt, w.ctrl, w.steps);
        }
    }
    // the confirmed house's yaw is part of the locked placement
    if turn != 0.0 && !session.quote_requested {
        session.yaw = wrap_angle(session.yaw + turn);
    }
    let right_click = mouse.just_pressed(MouseButton::Right) || virtual_click == Some(MouseButton::Right);
    if right_click {
        mouse.clear_just_pressed(MouseButton::Right);
        cancel = true;
    }
    let left_down = mouse.pressed(MouseButton::Left) || virtual_click == Some(MouseButton::Left);
    let left_just = mouse.just_pressed(MouseButton::Left) || virtual_click == Some(MouseButton::Left);
    if left_just && !over_ui {
        mouse.clear_just_pressed(MouseButton::Left);
    }
    if !session.quote_requested && left_confirm(&mut session.left_armed, left_down, left_just, over_ui) {
        let cooldown = skills
            .cooldown_left(session.plant.use_skill, session.plant.cooldown_tag, session.plant.cooldown_ms)
            .filter(|left| !left.is_zero())
            .map(|left| left.as_secs().max(1));
        if let Some(seconds) = cooldown {
            notify(rt.as_deref(), cooldown_message(seconds));
        } else if let Some(reason) = session.invalid {
            match error_message(reason) {
                Some(text) => notify(rt.as_deref(), text.into()),
                None => info!("placement: not confirmed: {}", reason.text()),
            }
        } else if session.is_house() {
            if let Some(request) = session.tax_request() {
                if let HouseCommand::ConstructHouseTax { design_id, x, y, z } = &request {
                    info!("housing: tax quote for design {design_id} at ({x:.2}, {y:.2}, {z:.2})");
                }
                net.write(NetCommand(Command::House(request)));
                session.quote_requested = true;
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
    if cancel {
        info!("{}: cancelled", if session.is_house() { "housing" } else { "planting" });
        end_session(&mut commands, &mut placement, &visuals);
    }
}

/// Arms the left button only after the button that used the seed has been released; a press while the pointer is
/// over the HUD never confirms.
fn left_confirm(armed: &mut bool, pressed: bool, just_pressed: bool, over_ui: bool) -> bool {
    if !*armed {
        if !pressed && !just_pressed {
            *armed = true;
        }
        return false;
    }
    just_pressed && !over_ui
}

/// The mouse wheel's turn for one notch/test: none with Ctrl (camera zoom), 15 degrees with Shift, 1 with Alt.
fn wheel_turn(shift: bool, alt: bool, ctrl: bool, delta: f32) -> f32 {
    if ctrl || delta == 0.0 {
        return 0.0;
    }
    let step = if shift {
        ROT_STEP
    } else if alt {
        1f32.to_radians()
    } else {
        ROT_STEP * 2.0 / 3.0
    };
    step * delta.signum()
}

fn cooldown_message(seconds: u64) -> String {
    format!("Can't place this for {seconds} second(s).")
}

/// The red error text at the top centre, as `SYSMSG` (the original `centermessage` script).
fn notify(rt: Option<&aa_ui::UiRuntime>, text: String) {
    info!("planting: {text}");
    if let Some(rt) = rt {
        rt.fire_event("SYSMSG", text);
    }
}

/// The retail client's placement errors (`ui_texts` 277 "Can't be installed here.", 278 "Too close to other
/// objects."; the cooldown uses 4598). `ui_texts` 277..281 / the UI locale files have no placement range message, so
/// an out-of-range click stays silent, as in the retail capture.
fn error_message(reason: Invalid) -> Option<&'static str> {
    match reason {
        Invalid::Footprint | Invalid::Overlap => Some("Too close to other objects."),
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

/// The scripted `house_place <design item> <dx> <dy> [yaw]`: aim the real PlacementSession at a spot without a
/// cursor (the ghost and boundary stay for a screenshot; the virtual `click left` then asks for the tax quote).
#[allow(clippy::too_many_arguments)]
pub fn house_place_now(
    catalog: &Catalog,
    houses: &Houses,
    areas: Option<&HousingAreas>,
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
) -> Result<Session, String> {
    let plant = catalog.plantable(item).ok_or_else(|| format!("item {item} is not in item_housings"))?;
    let Some(design) = plant.house.as_ref() else {
        return Err(format!("item {item} is not a house design"));
    };
    let (_, item_instance) = inventory.find_template(item).ok_or_else(|| format!("no item {item} in the bag"))?;
    let x = player.x + dx;
    let y = player.y + dy;
    let ground = ground_at(collision, terrain, x, y, player.z, player.z);
    let wet = water_surface(water, x, y, ground);
    let spot = grounded(Vec3::new(x, y, ground), ground, wet, plant);
    let invalid = check_house(design, spot, player, houses, catalog, areas, wet).err();
    let mut session = Session::new(item_instance.id, plant.clone(), yaw.unwrap_or(facing));
    session.scripted = true;
    session.spot = Some(spot);
    session.invalid = invalid;
    Ok(session)
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
            house: None,
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
        assert!((0.55..=0.75).contains(&rice.0), "the retail rice ring is small: {rice:?}");
        assert!(tree.0 > rice.0 && tree.0 <= 2.6, "a tree ring is larger: {rice:?} vs {tree:?}");
        assert!(tree.1 > rice.1 && tree.2 > rice.2, "more and longer petals for a tree: {rice:?} vs {tree:?}");
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
    fn a_bag_click_cannot_confirm_a_new_session() {
        let mut armed = false;
        // the left button was already down when the seed was used (and/or the press started the bag use)
        assert!(!left_confirm(&mut armed, true, true, false));
        assert!(!left_confirm(&mut armed, true, false, false));
        // release, then the first deliberate press confirms
        assert!(!left_confirm(&mut armed, false, false, false));
        assert!(left_confirm(&mut armed, true, true, false));
        // a press over the HUD never confirms
        assert!(!left_confirm(&mut armed, true, true, true));
        assert!(left_confirm(&mut armed, true, true, false));
    }

    #[test]
    fn the_wheel_mapping_keeps_ctrl_for_the_camera() {
        let normal = ROT_STEP * 2.0 / 3.0;
        assert!((wheel_turn(false, false, false, 1.0) - normal).abs() < 1e-6);
        assert!((wheel_turn(false, false, false, -3.0) + normal).abs() < 1e-6);
        assert!((wheel_turn(true, false, false, 1.0) - ROT_STEP).abs() < 1e-6);
        assert!((wheel_turn(false, true, false, 1.0) - 1f32.to_radians()).abs() < 1e-6);
        assert_eq!(wheel_turn(false, false, true, 1.0), 0.0);
        assert_eq!(wheel_turn(true, true, false, 0.0), 0.0);
    }

    #[test]
    fn the_cooldown_error_uses_the_retail_text() {
        assert_eq!(cooldown_message(23), "Can't place this for 23 second(s).");
        assert_eq!(cooldown_message(1), "Can't place this for 1 second(s).");
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
                assert!((v.y - 10.09).abs() < 1e-3, "Bevy Y is the ground height: {v:?}");
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

    fn house_plantable(design: u32, radius: f32) -> Plantable {
        Plantable {
            item: 15596,
            doodad: 0,
            use_skill: 17496,
            use_range: 0.0,
            cooldown_ms: 0,
            cooldown_tag: 0,
            start_group: 0,
            model: Some("objects/housing/hs_common_farm_b/hs_common_farm_b.cgf".into()),
            footprint: radius,
            requires_water: false,
            grounding: Grounding::default(),
            house: Some(HouseDesign {
                design,
                name: "Scarecrow Garden".into(),
                garden_radius: radius,
                heavy_tax: true,
            }),
        }
    }

    fn house_catalog() -> Catalog {
        let mut c = Catalog::default();
        c.insert(house_plantable(267, 4.0));
        c
    }

    #[test]
    fn the_catalog_maps_a_design_item_to_its_housing_and_plot_radius() {
        let path = std::env::temp_dir().join(format!("aa_house_catalog_{}.sqlite3", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE item_spawn_doodads (id INT, item_id INT, doodad_id INT);
                 CREATE TABLE doodad_almighties (id INT, sim_radius INT);
                 CREATE TABLE doodad_func_groups (id INT, doodad_almighty_id INT, doodad_func_group_kind_id INT);
                 CREATE TABLE doodad_phase_funcs (doodad_func_group_id INT, actual_func_type TEXT);
                 CREATE TABLE items (id INT, use_skill_id INT);
                 CREATE TABLE skills (id INT, max_range REAL, cooldown_time INT, cooldown_tag_id INT);
                 CREATE TABLE localized_texts (tbl_name TEXT, idx INT, tbl_column_name TEXT, en_us TEXT);
                 CREATE TABLE item_housings (id INT, item_id INT, design_id INT);
                 CREATE TABLE housings (id INT, name TEXT, main_model_id INT, garden_radius REAL, heavy_tax NUM,
                     auto_z NUM, auto_z_offset_x REAL, auto_z_offset_y REAL, auto_z_offset_z REAL);
                 CREATE TABLE housing_areas (id INT, name TEXT, housing_group_id INT, comments TEXT);
                 INSERT INTO items VALUES (15596, 17496);
                 INSERT INTO skills VALUES (17496, 0.0, 0, 0);
                 INSERT INTO localized_texts VALUES ('housings', 267, 'name', 'Scarecrow Garden');
                 INSERT INTO item_housings VALUES (58, 15596, 267);
                 INSERT INTO housings VALUES (267, 'Korean', 987, 4.0, 't', 't', 0.0, 0.0, 0.0);
                 INSERT INTO housing_areas VALUES (128, 'gwe_housing', 1, '');",
            )
            .unwrap();
        }
        let catalog = Catalog::load_path(path.to_string_lossy().as_ref(), None).expect("catalog");
        let plant = catalog.plantable(15596).expect("design item");
        let house = plant.house.as_ref().expect("house design");
        assert_eq!((house.design, house.garden_radius), (267, 4.0));
        assert_eq!(house.name, "Scarecrow Garden");
        assert!(house.heavy_tax);
        assert_eq!(catalog.house_radius(267), 4.0);
        assert!(catalog.area_known(128) && !catalog.area_known(129));
        assert_eq!(plant.doodad, 0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn the_plot_stones_follow_the_garden_radius_and_the_yaw() {
        let p = house_plantable(267, 4.0);
        let spot = Vec3::new(100.0, 200.0, 10.0);
        let stones = stone_quads(&p, spot, 0.0, |_| 10.0);
        assert!(!stones.is_empty());
        for q in &stones {
            for v in q.p {
                assert!((v.y - 10.025).abs() < 1e-3, "the stones lie on the ground: {v:?}");
                assert!(v.x >= 100.0 - 4.4 && v.x <= 100.0 + 4.4, "inside the plot: {v:?}");
                assert!(v.z >= -200.0 - 4.4 && v.z <= -200.0 + 4.4, "inside the plot: {v:?}");
            }
        }
        let turned = stone_quads(&p, spot, std::f32::consts::FRAC_PI_4, |_| 10.0);
        assert_ne!(quads_signature(&stones), quads_signature(&turned), "rotating the ghost rotates the square");
    }

    #[test]
    fn the_yellow_posts_follow_the_housing_area_polygon() {
        let area = HousingAreas {
            shapes: vec![crate::ingame::houses::HousingShape {
                id: 128,
                bounds: [90.0, 190.0, 110.0, 210.0],
                points: vec![[90.0, 190.0], [110.0, 190.0], [110.0, 210.0], [90.0, 210.0]],
            }],
        };
        let posts = post_quads(&area, Vec3::new(100.0, 200.0, 10.0), |_| 10.0);
        assert!(!posts.is_empty());
        for q in &posts {
            for v in q.p {
                assert!((v.y - 10.02).abs() < 1e-3 || (v.y - 10.57).abs() < 1e-3, "posts stand on the ground: {v:?}");
            }
        }
    }

    #[test]
    fn a_house_plot_needs_a_known_area_and_no_other_house() {
        let cat = {
            let mut c = house_catalog();
            c.area_ids.insert(128);
            c
        };
        let design = cat.plantable(15596).unwrap().house.clone().unwrap();
        let area = HousingAreas {
            shapes: vec![crate::ingame::houses::HousingShape {
                id: 128,
                bounds: [0.0, 0.0, 20.0, 20.0],
                points: vec![[0.0, 0.0], [20.0, 0.0], [20.0, 20.0], [0.0, 20.0]],
            }],
        };
        let houses = Houses::default();
        let me = Vec3::new(100.0, 100.0, 10.0);
        assert_eq!(check_house(&design, Vec3::new(10.0, 10.0, 10.0), me, &houses, &cat, Some(&area), None), Ok(()));
        assert_eq!(
            check_house(&design, Vec3::new(40.0, 10.0, 10.0), me, &houses, &cat, Some(&area), None),
            Err(Invalid::OutsideHousingArea)
        );
        assert_eq!(
            check_house(&design, Vec3::new(10.0, 10.0, 10.0), me, &houses, &cat, None, None),
            Err(Invalid::OutsideHousingArea)
        );
        let mut occupied = Houses::default();
        occupied.by_tl_id.insert(
            7,
            aa_net::housing::House {
                snapshot: aa_net::housing::HouseSnapshot {
                    tl_id: 7,
                    db_id: 1,
                    obj_id: 50,
                    template_id: 267,
                    model_id: 989,
                    co_owner_id: 0,
                    owner_id: 1,
                    owner_name: "Owner".into(),
                    account_id: 1,
                    permission: 0,
                    all_actions: 1,
                    current_actions: 0,
                    tax: 0,
                    x: aa_net::wire::coord_to_long(13.0),
                    y: aa_net::wire::coord_to_long(10.0),
                    z: 10.0,
                    name: "Other".into(),
                    allow_recover: true,
                    sell_price: 0,
                    sell_to_player_id: 0,
                    sell_to_name: String::new(),
                },
                demolished: false,
                build_pay: None,
                tax: None,
                sale: None,
            },
        );
        assert_eq!(
            check_house(&design, Vec3::new(10.0, 10.0, 10.0), me, &occupied, &cat, Some(&area), None),
            Err(Invalid::Overlap)
        );
        assert_eq!(
            check_house(&design, Vec3::new(10.0, 10.0, 10.0), Vec3::new(10.0, 10.0, 10.0), &houses, &cat, Some(&area), None),
            Err(Invalid::Player)
        );
    }

    #[test]
    fn a_house_session_asks_for_the_quote_then_builds_with_the_quote_total() {
        let mut session = Session::new(77, house_plantable(267, 4.0), 0.5);
        session.spot = Some(Vec3::new(1000.5, 2000.25, 100.0));
        session.invalid = None;
        assert!(session.is_house());
        assert_eq!(session.house_design(), Some(267));
        assert!(session.request().is_none(), "a design does not send CSCreateDoodad");
        assert_eq!(
            session.tax_request(),
            Some(HouseCommand::ConstructHouseTax { design_id: 267, x: 1000.5, y: 2000.25, z: 100.0 })
        );
        assert!(session.create_house().is_none(), "no quote yet");
        session.quote = Some(ConstructHouseTax {
            design_id: 267,
            heavy_tax_house_count: 1,
            normal_tax_house_count: 0,
            is_heavy_tax_house: true,
            base_tax_money_amount: 50000,
            deposit_tax_money_amount: 100000,
            total_tax_money_amount: 150000,
        });
        session.quote_requested = true;
        // The pointer moving over the Start Construction window must not invalidate or move the confirmed spot.
        session.spot = Some(Vec3::new(1000.5, 2000.25, 100.0));
        session.invalid = Some(Invalid::NoGround);
        assert_eq!(
            session.create_house(),
            Some(HouseCommand::CreateHouse {
                design_id: 267,
                x: 1000.5,
                y: 2000.25,
                z: 100.0,
                z_rot: 0.5,
                item_id: 77,
                money_amount: 150000,
                ht: 0,
                auto_use_aa_point: false,
            })
        );
        assert_eq!(session.build_blocked(), None);
        session.quote = None;
        assert_eq!(session.build_blocked(), Some("the server's tax quote has not arrived"));
    }

    #[test]
    fn a_house_locks_the_pointer_once_confirmed_and_over_the_hud() {
        let mut session = Session::new(77, house_plantable(267, 4.0), 0.5);
        assert!(!house_locks_pointer(&session, false));
        assert!(house_locks_pointer(&session, true), "the HUD does not move the preview");
        session.quote_requested = true;
        assert!(house_locks_pointer(&session, false), "the quote locks the confirmed spot");
        session.quote_requested = false;
        assert!(!house_locks_pointer(&session, false));
        let plant = Session::new(1, plant(), 0.0);
        assert!(!house_locks_pointer(&plant, true), "planting keeps its existing HUD behaviour");
    }

    #[test]
    fn the_real_database_maps_seeds_and_saplings() {
        let path = std::env::var("AA_DB").unwrap_or_else(|_| aa_anim::db::DEFAULT_DB.to_string());
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
        let farm = catalog.plantable(15596).expect("Scarecrow Garden design");
        let house = farm.house.as_ref().expect("housing design");
        assert_eq!((house.design, house.garden_radius), (267, 4.0));
        assert!(farm.model.as_deref().is_some_and(|m| m.ends_with("nuia_housing_scarecrow_b.cgf")), "{:?}", farm.model);
        let pumpkin = catalog.plantable(15566).expect("Pumpkin-head farm design");
        assert_eq!(pumpkin.house.as_ref().map(|h| (h.design, h.garden_radius)), Some((89, 8.0)));
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
            .add_message::<HousePressed>()
            .init_resource::<aa_net::Farms>()
            .init_resource::<aa_net::Houses>()
            .init_resource::<aa_net::state::Doodads>()
            .init_resource::<aa_net::state::Inventory>()
            .init_resource::<aa_net::state::Skills>()
            .init_resource::<crate::hud::HudPointerOverUi>()
            .init_resource::<crate::hud::HudKeyboardFocus>()
            .init_resource::<crate::hud::HudBuilt>()
            .init_resource::<crate::ingame::virtual_input::VirtualInput>()
            .insert_non_send(crate::hud::api::HudBridge(std::rc::Rc::new(std::cell::RefCell::new(
                crate::hud::api::HudData::default(),
            ))));
        let plant = plant();
        app.insert_resource(Placement { session: Some(Session::new(1, plant, 0.0)), ..Default::default() });
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
            // no window/camera ray: the next tick has no ground and must hide the ghost and the ring
            app.world_mut().spawn((
                LocalPlayer { obj_id: 1 },
                Mover {
                    pos: Vec3::new(10480.0, 15400.0, 210.0),
                    facing: 0.0,
                    vz: 0.0,
                    grounded: true,
                    velocity: Vec3::ZERO,
                    swim: None,
                    wading: 0.0,
                },
            ));
            app.update();
            assert_eq!(app.world().get::<Visibility>(roots[0]), Some(&Visibility::Hidden), "off-ground hides the ghost");
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

    /// The virtual script input (`cursor`, `click`, `wheel`) drives the real `input` system without any OS mouse.
    #[test]
    fn virtual_input_confirms_a_valid_spot_and_refuses_an_invalid_one() {
        #[derive(Resource, Default)]
        struct Sent(Vec<Command>);
        fn collect(mut events: MessageReader<NetCommand>, mut sent: ResMut<Sent>) {
            sent.0.extend(events.read().map(|command| command.0.clone()));
        }
        let mut app = App::new();
        app.add_message::<NetCommand>()
            .add_message::<bevy::input::mouse::MouseWheel>()
            .init_resource::<VirtualInput>()
            .init_resource::<Visuals>()
            .init_resource::<Sent>()
            .init_resource::<Skills>()
            .insert_resource(HudPointerOverUi::default())
            .insert_resource(HudKeyboardFocus::default())
            .insert_resource(ButtonInput::<MouseButton>::default())
            .insert_resource(ButtonInput::<KeyCode>::default())
            .add_systems(Update, (input, collect).chain());
        let mut session = Session::new(77, plant(), 0.0);
        session.spot = Some(Vec3::new(100.0, 200.0, 10.0));
        session.invalid = None;
        app.insert_resource(Placement { session: Some(session), ..Default::default() });
        app.update(); // arm the left button after the seed click
        app.world_mut().resource_mut::<VirtualInput>().queue_click(MouseButton::Left);
        app.update();
        let sent = std::mem::take(&mut app.world_mut().resource_mut::<Sent>().0);
        assert!(matches!(sent.as_slice(), [Command::CreateDoodad(request)] if request.id == 2246), "{sent:?}");
        assert!(app.world().resource::<Placement>().session.is_none(), "a valid confirm ends the mode");

        let mut session = Session::new(77, plant(), 0.0);
        session.spot = Some(Vec3::new(100.0, 200.0, 10.0));
        session.invalid = Some(Invalid::Footprint);
        app.insert_resource(Placement { session: Some(session), ..Default::default() });
        app.update(); // arm again
        app.world_mut().resource_mut::<VirtualInput>().queue_click(MouseButton::Left);
        app.update();
        assert!(app.world().resource::<Sent>().0.is_empty(), "an invalid click sends no packet");
        assert_eq!(error_message(Invalid::Footprint), Some("Too close to other objects."));
        assert!(app.world().resource::<Placement>().session.is_some(), "the mode stays for another spot");
    }
}
