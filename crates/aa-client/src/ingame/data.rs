//! Game database lookups the in-game world needs: which model an NPC / doodad template draws.
//!
//! The database is the server's plain SQLite copy (`compact.sqlite3`, read only). Results are
//! cached: a town has a few dozen distinct templates and hundreds of instances.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex};

use aa_anim::GameDb;
use aa_net::model::Equipment;
use bevy::prelude::*;
use rusqlite::{Connection, OpenFlags, params};

pub(crate) use super::combat::skills::{Tool, tool_from_conn};

/// `slaves.slave_kind_id` of a cannon.
pub const SLAVE_KIND_CANNON: u32 = 8;

/// The database handles (a connection is `Send` but not `Sync`, hence the mutexes). Cloning shares them, so worker
/// threads can use the same lookups and caches.
#[derive(Resource, Clone)]
pub struct Db {
    pub game: Arc<Mutex<GameDb>>,
    lookups: Arc<Mutex<Lookups>>,
    /// The file the lookups were opened from (the indexed copy when it could be built).
    path: String,
    /// `slave_attach_points.json` next to the original database (the server's copy of the client data).
    attach_points_path: Option<String>,
}

struct Lookups {
    conn: Connection,
    actor_files: HashMap<u32, Option<String>>,
    doodad_models: HashMap<u32, Option<String>>,
    /// `(template, func group)` -> the phase model.
    doodad_phase_models: HashMap<(u32, u32), Option<String>>,
    /// `doodad_func_groups.id` -> the phase's `DoodadFuncClimb` type (`doodad_func_climbs.climb_type_id`).
    ladder_kinds: HashMap<u32, Option<u8>>,
    doodad_visuals: HashMap<(u32, u32), Option<String>>,
    doodad_growths: HashMap<u32, Option<DoodadGrowth>>,
    npc_scale: HashMap<u32, f32>,
    weapons: HashMap<u32, Option<Weapon>>,
    backpacks: HashMap<u32, Option<Weapon>>,
    gliders: HashMap<u32, Option<GliderItem>>,
    anim_actions: HashMap<u32, Option<String>>,
    anim_action_tools: HashMap<u32, [Option<Tool>; 2]>,
    mount_poses: HashMap<u32, u32>,
    mount_moves: HashMap<u32, MountMove>,
    vehicle_bodies: HashMap<u32, VehicleBody>,
    land_vehicles: HashMap<u32, bool>,
    ship_bodies: HashMap<u32, ShipBody>,
    ship_floors: HashMap<u32, Option<Arc<ShipFloor>>>,
    ships: HashMap<u32, bool>,
    ship_variants: HashMap<u32, ShipVariants>,
    ship_equip_slots: HashMap<u32, Vec<ShipEquipSlot>>,
    ship_doodad_bindings: HashMap<u32, Vec<ShipDoodadBinding>>,
    doodad_ship_parts: HashMap<u32, Vec<ShipPart>>,
    item_slave_doodads: HashMap<u32, Option<u32>>,
    /// `slave_attach_points.json` by model id, read once (empty when the file is absent).
    ship_attach_points: std::sync::OnceLock<HashMap<u32, HashMap<u8, ShipAttach>>>,
    /// `slaves.slave_kind_id` by slave template.
    slave_kinds: HashMap<u32, u32>,
    seats: HashMap<(u32, u8), Seat>,
}

/// A land vehicle/slave body from `vehicle_models` (the row `models.sub_id` names when the model
/// is a `VehicleModel`): native forward speed and angular velocity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VehicleBody {
    pub speed: f32,
    /// Radians per second (`vehicle_models.angVel`).
    pub turn: f32,
    /// Number of wheel entries the native `VehicleMove` body carries: the wheel bones of the body
    /// skeleton (4 for the Farm Wagon); the client sends this many wheel speeds (zero is valid).
    pub wheels: u8,
}

impl Default for VehicleBody {
    fn default() -> Self {
        Self { speed: 8.0, turn: 1.0, wheels: 2 }
    }
}

/// The wheel `.cgf` files of a land vehicle (`vehicle_models.wheel` / `wheel2`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VehicleWheelModels {
    pub wheel: Option<String>,
    pub wheel2: Option<String>,
}

/// A ship hull from `ship_models` (the row `models.sub_id` names when the model is a
/// `ShipModel`). The server owns the hull physics; these numbers are the authored top speed,
/// reverse speed, rudder rate and the hull box used to keep a standing passenger on deck.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShipBody {
    /// `ship_models.velocity` (m/s).
    pub speed: f32,
    /// `ship_models.reverse_velocity` (m/s).
    pub reverse: f32,
    /// `ship_models.steer_vel`.
    pub turn: f32,
    /// Half extents of the hull box in model space: x lateral, y fore/aft.
    pub half: [f32; 2],
    /// Driver deck height above the hull root (`slave_attach_points.json` point 1 Z).
    pub deck: f32,
}

impl Default for ShipBody {
    fn default() -> Self {
        Self { speed: 13.0, reverse: 5.0, turn: 1.0, half: [3.0, 10.0], deck: 1.5 }
    }
}

/// A seat in a slave's model space (`slave_attach_points.json`): x lateral, y fore/aft, z up,
/// and the attachment yaw offset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seat {
    pub local: [f32; 3],
    pub yaw: f32,
}

impl Default for Seat {
    fn default() -> Self {
        Self { local: [0.0, 0.0, 1.1], yaw: 0.0 }
    }
}

/// How a ridden mate moves: the actor model's base stance speeds and body (the same `game_stances`
/// rows the server's `Npc.BaseMoveSpeed` reads), plus its jump/gravity and turn rate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MountMove {
    pub walk: f32,
    pub run: f32,
    pub radius: f32,
    pub height: f32,
    pub jump_height: f32,
    pub gravity: f32,
    /// Radians per second (`actor_models.turn_speed` is degrees per second).
    pub turn: f32,
    /// `actor_models.game_inertia`: how fast the speed falls to the wanted one (1/s). Inferred semantics: Cry's `inertia` is the
    /// response while slowing down, `game_inertia_accel` while speeding up. A horse has 4 / 11, most actors 20 / 11.
    pub inertia: f32,
    /// `actor_models.game_inertia_accel` (1/s).
    pub accel: f32,
}

impl Default for MountMove {
    fn default() -> Self {
        Self { walk: 1.8, run: 5.4, radius: 0.6, height: 1.8, jump_height: 1.3, gravity: 25.0, turn: 360.0_f32.to_radians(), inertia: 20.0, accel: 11.0 }
    }
}

/// What the glider backpack item `item` is for the flight: its use skill and the model it flies with.
#[derive(Clone, Debug, PartialEq)]
pub struct GliderItem {
    /// `items.use_skill_id`: the skill that puts the glider's buff on the wearer.
    pub use_skill: u32,
    /// The flying model (`item_backpacks.asset_id`: a `.chr` or `.cdf`), as a pak path.
    pub model: Option<String>,
}

/// How a growing doodad phase scales (`doodad_func_growths`): from `start_scale` to `end_scale` over `delay_ms`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DoodadGrowth {
    pub delay_ms: u32,
    /// Per mille in the database, here as a factor (600 -> 0.6).
    pub start_scale: f32,
    pub end_scale: f32,
}

impl DoodadGrowth {
    /// The total duration used to interpolate the scale. On a phase change the server's `time_left` is the whole
    /// remaining time of the new phase (it already applied its growth rate and climate); a doodad first seen mid-phase
    /// keeps the database `delay` when that is longer, so the visible size matches the part of the phase already
    /// elapsed. Without an announced `time_left` (0) the database delay is the fallback.
    pub fn total_ms(&self, time_left_ms: f32, first_seen: bool) -> f32 {
        if time_left_ms <= 0.0 {
            return self.delay_ms as f32;
        }
        if first_seen {
            time_left_ms.max(self.delay_ms as f32)
        } else {
            time_left_ms
        }
    }

    /// The scale `remaining_ms` before the end of a phase of `total_ms`: `start_scale` with the full time left,
    /// `end_scale` at 0.
    pub fn scale_at(&self, remaining_ms: f32, total_ms: f32) -> f32 {
        if total_ms <= 0.0 {
            return self.end_scale;
        }
        let t = (1.0 - remaining_ms / total_ms).clamp(0.0, 1.0);
        self.start_scale + (self.end_scale - self.start_scale) * t
    }
}

/// How a weapon item is drawn (`item_weapons` + its `item_assets` row).
#[derive(Clone, Debug)]
pub struct Weapon {
    pub path: String,
    /// `holdables.code`: `bow`, `shield`, `normal_piercing_sod`, ...
    pub code: String,
    /// Offset in the attachment bone's frame (m) and rotation (degrees about x, y, z).
    pub offset: [f32; 3],
    pub rotation: [f32; 3],
    /// Scale while it hangs on the body.
    pub worn_scale: f32,
    /// Scale while it is in the hand.
    pub drawn_scale: f32,
    /// Held with both hands (`holdables.slot_type_id` 16: the two handed swords, axes, hammers, spears and staves).
    pub two_handed: bool,
    /// A staff (`holdables.pose_id` 4): held two handed but fights and casts in the one handed stance (the `staff_` clips are
    /// the one handed ones).
    pub staff: bool,
    /// `item_assets.allow_mirror`: the off hand copy is mirrored (x2game flips `pos_x`, `rot_y` and `rot_z`).
    pub mirror: bool,
}

/// Columns the in-game lookups filter or join on. The server's database has no indexes beyond the
/// primary keys, so `item -> armour meshes` (three joined tables) takes about 30 ms a call; a
/// private indexed copy makes it a lookup.
const INDEXES: &[(&str, &str)] = &[
    ("item_armors", "item_id"),
    ("item_armors", "asset_id"),
    ("item_armor_assets", "armor_asset_id"),
    ("item_armor_assets", "asset_id"),
    ("item_assets", "id"),
    ("item_assets", "model_id"),
    ("item_body_parts", "item_id"),
    ("item_body_parts", "model_id, slot_type_id"),
    ("hair_colors", "id"),
    ("hair_colors", "model_id, asset_id"),
    ("skin_colors", "id"),
    ("skin_colors", "model_id"),
    ("dyeable_items", "item_id"),
    ("item_dyeings", "item_id"),
    ("dyeing_colors", "id"),
    ("total_character_customs", "id"),
    ("total_character_customs", "model_id, owner_type_id"),
    ("face_decal_assets", "id"),
    ("face_normal_maps", "id"),
    ("models", "id"),
    ("actor_models", "id"),
    ("npcs", "id"),
    ("doodad_almighties", "id"),
    ("characters", "model_id"),
    ("equip_pack_cloths", "id"),
    ("item_weapons", "item_id"),
    ("item_tools", "item_id"),
    ("holdables", "id"),
    ("anim_actions", "id"),
];

/// The database path to use: a copy of the server's database with the [`INDEXES`] added, kept under
/// `data/extract/aa-client/` and rebuilt when the original changes. Falls back to the original.
fn indexed_database(original: &str) -> String {
    let build = || -> Option<String> {
        let meta = std::fs::metadata(original).ok()?;
        let stamp = format!("{} {:?}", meta.len(), meta.modified().ok()?);
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/extract/aa-client");
        std::fs::create_dir_all(&dir).ok()?;
        let target = dir.join("compact-indexed.sqlite3");
        let marker = dir.join("compact-indexed.stamp");
        if target.exists() && std::fs::read_to_string(&marker).ok().as_deref() == Some(stamp.as_str()) {
            return Some(target.to_string_lossy().into_owned());
        }
        info!("building the indexed game database copy {} (once)", target.display());
        let tmp = dir.join(format!("compact-indexed.{}.tmp", std::process::id()));
        std::fs::copy(original, &tmp).ok()?;
        {
            let conn = Connection::open(&tmp).ok()?;
            for (i, (table, columns)) in INDEXES.iter().enumerate() {
                // tables or columns the database lacks are skipped
                let _ = conn.execute_batch(&format!("CREATE INDEX IF NOT EXISTS aa_client_idx_{i} ON {table}({columns})"));
            }
            let _ = conn.execute_batch("ANALYZE");
        }
        std::fs::rename(&tmp, &target).ok()?;
        std::fs::write(&marker, stamp).ok()?;
        Some(target.to_string_lossy().into_owned())
    };
    build().unwrap_or_else(|| original.to_string())
}

impl Db {
    pub fn open() -> Option<Db> {
        let original = std::env::var("AA_DB").unwrap_or_else(|_| aa_anim::db::DEFAULT_DB.to_string());
        let path = indexed_database(&original);
        let attach_points_path = std::env::var("AA_SLAVE_ATTACH_POINTS").ok().filter(|p| std::path::Path::new(p).exists()).or_else(|| {
            std::path::Path::new(&original)
                .parent()
                .map(|dir| dir.join("slave_attach_points.json"))
                .filter(|p| p.exists())
                .map(|p| p.to_string_lossy().into_owned())
        });
        let game = match GameDb::open_path(&path) {
            Ok(g) => g,
            Err(e) => {
                error!("game database: {e}");
                return None;
            }
        };
        let conn = match Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI) {
            Ok(c) => c,
            Err(e) => {
                error!("game database {path}: {e}");
                return None;
            }
        };
        Some(Db {
            game: Arc::new(Mutex::new(game)),
            path: path.clone(),
            attach_points_path,
            lookups: Arc::new(Mutex::new(Lookups {
                conn,
                actor_files: HashMap::new(),
                doodad_models: HashMap::new(),
                doodad_phase_models: HashMap::new(),
                ladder_kinds: HashMap::new(),
                doodad_visuals: HashMap::new(),
                doodad_growths: HashMap::new(),
                npc_scale: HashMap::new(),
                weapons: HashMap::new(),
                backpacks: HashMap::new(),
                gliders: HashMap::new(),
                anim_actions: HashMap::new(),
                anim_action_tools: HashMap::new(),
                mount_poses: HashMap::new(),
                mount_moves: HashMap::new(),
                vehicle_bodies: HashMap::new(),
                land_vehicles: HashMap::new(),
                ship_bodies: HashMap::new(),
                ship_floors: HashMap::new(),
                ships: HashMap::new(),
                ship_variants: HashMap::new(),
                ship_equip_slots: HashMap::new(),
                ship_doodad_bindings: HashMap::new(),
                doodad_ship_parts: HashMap::new(),
                item_slave_doodads: HashMap::new(),
                ship_attach_points: std::sync::OnceLock::new(),
                slave_kinds: HashMap::new(),
                seats: HashMap::new(),
            })),
        })
    }

    /// The database file the lookups use: the indexed copy when it could be built, else `AA_DB` /
    /// [`aa_anim::db::DEFAULT_DB`]. Everything that reads game tables goes through this one file.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The model file (`.cdf` / `.chr`) of an actor model (`models.id`), lower case, `/` separators.
    pub fn actor_model_file(&self, model_id: u32) -> Option<String> {
        let mut l = self.lookups.lock().unwrap();
        if let Some(f) = l.actor_files.get(&model_id) {
            return f.clone();
        }
        let file: Option<String> = l
            .conn
            .query_row(
                "SELECT a.model_file FROM models m JOIN actor_models a ON a.id = m.sub_id AND m.sub_type = 'ActorModel' WHERE m.id = ?1",
                params![model_id],
                |r| r.get::<_, Option<String>>(0),
            )
            .ok()
            .flatten()
            .map(|f| norm(&f))
            .filter(|f| !f.is_empty())
            .or_else(|| {
                // a prefab model (the training dummies, 46 NPCs): the `object_Model` of the prefab's animated object
                let prefab: Option<String> = l
                    .conn
                    .query_row(
                        "SELECT e.file_path FROM models m JOIN prefab_elements e ON e.prefab_model_id = m.sub_id AND m.sub_type = 'PrefabModel' WHERE m.id = ?1 ORDER BY e.state_id LIMIT 1",
                        params![model_id],
                        |r| r.get::<_, Option<String>>(0),
                    )
                    .ok()
                    .flatten();
                prefab.and_then(|p| prefab_model_file(&p))
            })
            .or_else(|| {
                // a land vehicle (the Farm Wagon, 92 slave models): `vehicle_models.normal`, usually
                // a `prefab://` element whose animated object is the body `.chr`
                let raw: Option<String> = l
                    .conn
                    .query_row(
                        "SELECT v.normal FROM models m JOIN vehicle_models v ON v.id = m.sub_id AND m.sub_type = 'VehicleModel' WHERE m.id = ?1",
                        params![model_id],
                        |r| r.get::<_, Option<String>>(0),
                    )
                    .ok()
                    .flatten();
                raw.as_deref().and_then(model_file)
            })
            .or_else(|| {
                // a ship (`ShipModel`, 26 slave models): the hull brush of the `ship_models.normal` prefab
                let parts = ship_parts_of(&l.conn, model_id);
                parts.iter().find(|p| p.is_hull()).or(parts.first()).map(|p| p.path.clone())
            });
        l.actor_files.insert(model_id, file.clone());
        file
    }

    /// The brushes of a housing prefab model (a design's finished house or farm, a construction step): every `Brush` of
    /// every state-1 (alive) element of `prefab_elements` (a house is the wall, roof and floor prefabs, each with its
    /// decorations), each at its place in the prefab. The `models` row's first element alone is only one piece. Empty
    /// for a model that is no housing prefab.
    pub fn housing_parts(&self, model_id: u32) -> Vec<ShipPart> {
        let l = self.lookups.lock().unwrap();
        let is_housing: bool = l
            .conn
            .query_row(
                "SELECT 1 FROM models WHERE id = ?1 AND sub_type = 'PrefabModel' AND (id IN (SELECT main_model_id FROM housings) OR id IN (SELECT model_id FROM housing_build_steps))",
                params![model_id],
                |_| Ok(()),
            )
            .is_ok();
        if !is_housing {
            return Vec::new();
        }
        let Ok(mut q) = l.conn.prepare(
            "SELECT e.file_path FROM models m JOIN prefab_elements e ON e.prefab_model_id = m.sub_id AND m.sub_type = 'PrefabModel'              WHERE m.id = ?1 AND e.state_id = (SELECT MIN(e2.state_id) FROM prefab_elements e2 WHERE e2.prefab_model_id = m.sub_id) ORDER BY e.id",
        ) else {
            return Vec::new();
        };
        let paths: Vec<String> = q
            .query_map(params![model_id], |r| r.get::<_, Option<String>>(0))
            .map(|rows| rows.flatten().flatten().map(|p| p.trim().to_string()).collect())
            .unwrap_or_default();
        paths.iter().flat_map(|p| prefab_brushes(p)).collect()
    }

    /// The brushes a ship model is drawn from (hull, rudder, cannons, cargo), empty for a model that is not a ship.
    #[cfg(test)]
    pub fn ship_parts(&self, model_id: u32) -> Vec<ShipPart> {
        let l = self.lookups.lock().unwrap();
        ship_parts_of(&l.conn, model_id)
    }

    /// The damage-state prefabs of a ship model (`ship_models`).
    pub fn ship_variants(&self, model_id: u32) -> ShipVariants {
        let mut l = self.lookups.lock().unwrap();
        if let Some(v) = l.ship_variants.get(&model_id) {
            return v.clone();
        }
        let variants = l
            .conn
            .query_row(
                "SELECT h.normal, h.damaged25, h.damaged50, h.damaged75, h.dead \
                 FROM models m JOIN ship_models h ON h.id = m.sub_id AND m.sub_type = 'ShipModel' WHERE m.id = ?1",
                params![model_id],
                |r| {
                    let p = |i: usize| r.get::<_, Option<String>>(i).map(|v| v.map(|p| p.trim().to_string()).filter(|p| !p.is_empty()));
                    Ok(ShipVariants { normal: p(0)?, damaged25: p(1)?, damaged50: p(2)?, damaged75: p(3)?, dead: p(4)? })
                },
            )
            .unwrap_or_default();
        l.ship_variants.insert(model_id, variants.clone());
        variants
    }

    /// The equipment slots a slave template declares (`slave_equip_slots`), 0-based as the wire.
    pub fn ship_equip_slots(&self, template_id: u32) -> Vec<ShipEquipSlot> {
        let mut l = self.lookups.lock().unwrap();
        if let Some(s) = l.ship_equip_slots.get(&template_id) {
            return s.clone();
        }
        let mut out = Vec::new();
        if let Ok(mut st) = l.conn.prepare(
            "SELECT attach_point_id, equip_slot_id FROM slave_equip_slots WHERE slave_id = ?1 ORDER BY equip_slot_id, attach_point_id",
        ) {
            let rows = st.query_map(params![template_id], |r| {
                Ok(ShipEquipSlot { attach: r.get::<_, i64>(0)? as u8, slot: r.get::<_, i64>(1)? as u8 })
            });
            if let Ok(rows) = rows {
                out.extend(rows.filter_map(Result::ok));
            }
        }
        l.ship_equip_slots.insert(template_id, out.clone());
        out
    }

    /// The default doodads a slave model carries (`slave_doodad_bindings`).
    pub fn ship_doodad_bindings(&self, template_id: u32) -> Vec<ShipDoodadBinding> {
        let mut l = self.lookups.lock().unwrap();
        if let Some(b) = l.ship_doodad_bindings.get(&template_id) {
            return b.clone();
        }
        let mut out = Vec::new();
        if let Ok(mut st) = l.conn.prepare(
            "SELECT attach_point_id, doodad_id, scale FROM slave_doodad_bindings \
             WHERE owner_id = ?1 AND lower(owner_type) = 'slave' ORDER BY attach_point_id, id",
        ) {
            let rows = st.query_map(params![template_id], |r| {
                Ok(ShipDoodadBinding {
                    attach: r.get::<_, i64>(0)? as u8,
                    doodad: r.get::<_, i64>(1)? as u32,
                    scale: r.get::<_, Option<f64>>(2)?.unwrap_or(1.0) as f32,
                })
            });
            if let Ok(rows) = rows {
                out.extend(rows.filter_map(Result::ok).filter(|b| b.doodad != 0));
            }
        }
        l.ship_doodad_bindings.insert(template_id, out.clone());
        out
    }

    /// The doodad an item installs in a slave slot (`item_slave_equipments.doodad_id`).
    pub fn item_slave_doodad(&self, item_id: u32) -> Option<u32> {
        let mut l = self.lookups.lock().unwrap();
        if let Some(d) = l.item_slave_doodads.get(&item_id) {
            return *d;
        }
        let doodad = l
            .conn
            .query_row(
                "SELECT doodad_id FROM item_slave_equipments WHERE item_id = ?1 AND doodad_id > 0 ORDER BY id LIMIT 1",
                params![item_id],
                |r| r.get::<_, i64>(0),
            )
            .ok()
            .map(|d| d as u32);
        l.item_slave_doodads.insert(item_id, doodad);
        doodad
    }

    /// The model brushes of a ship doodad (`doodad_almighties.model`): a prefab's objects or a single
    /// `.cgf` / `.chr`; empty for an unknown doodad.
    pub fn ship_doodad_parts(&self, doodad_id: u32) -> Vec<ShipPart> {
        let mut l = self.lookups.lock().unwrap();
        if let Some(p) = l.doodad_ship_parts.get(&doodad_id) {
            return p.clone();
        }
        let parts = ship_doodad_parts_of(&l.conn, doodad_id);
        l.doodad_ship_parts.insert(doodad_id, parts.clone());
        parts
    }

    /// The attach point `point` of a ship model (`slave_attach_points.json`), cry space and yaw in radians.
    pub fn ship_attach_point(&self, model_id: u32, point: u8) -> Option<ShipAttach> {
        let l = self.lookups.lock().unwrap();
        let all = l.ship_attach_points.get_or_init(|| {
            self.attach_points_path.as_deref().map(load_attach_points).unwrap_or_default()
        });
        all.get(&model_id).and_then(|m| m.get(&point)).copied()
    }

    /// The brushes and particle effects a ship draws in `state` with its installed parts: the variant prefab
    /// plus one doodad per equipped slot (or the default binding at points no slot owns).
    pub fn ship_visual(&self, template_id: u32, model_id: u32, state: u8, equipped: &Equipment) -> ShipVisualBuild {
        let variants = self.ship_variants(model_id);
        let path = variants.path(state).map(str::to_string);
        let mut parts = path.as_deref().map(prefab_brushes).unwrap_or_default();
        let effects = path.as_deref().map(prefab_effects_of).unwrap_or_default();
        // The dead prefab is the whole sunken hull (`galleon_dead.cgf` + its water): it carries no
        // equipment, so a wreck does not keep its masts and sails.
        if state != 6 {
            let slots = self.ship_equip_slots(template_id);
            let bindings = self.ship_doodad_bindings(template_id);
            let doodads = resolve_ship_doodads(&slots, &bindings, equipped, |item| self.item_slave_doodad(item));
            for doodad in doodads {
                let Some(at) = self.ship_attach_point(model_id, doodad.attach) else { continue };
                let local = self.ship_doodad_parts(doodad.doodad);
                parts.extend(place_ship_doodad(local, at, doodad.scale));
            }
        }
        ShipVisualBuild { parts, effects }
    }

    /// The parts of a transfer's `vehicle_models.normal` prefab (an airship's body, wings and propellers):
    /// every static `Brush` and every `AnimObject` at its place on the model. Empty when the model is not a
    /// prefab (a single `.cgf` / `.chr` body is drawn as one creature).
    pub fn transfer_parts(&self, model_id: u32) -> Vec<ShipPart> {
        let l = self.lookups.lock().unwrap();
        let raw: Option<String> = l
            .conn
            .query_row(
                "SELECT v.normal FROM models m JOIN vehicle_models v ON v.id = m.sub_id AND m.sub_type = 'VehicleModel' WHERE m.id = ?1",
                params![model_id],
                |r| r.get::<_, Option<String>>(0),
            )
            .ok()
            .flatten();
        raw.as_deref().filter(|p| p.trim().to_ascii_lowercase().starts_with("prefab://")).map(prefab_parts).unwrap_or_default()
    }

    /// `transfers.wait_time` in seconds: how long the transfer waits at a stop when no per-leg time is set.
    pub fn transfer_wait(&self, transfer_id: u32) -> Option<f32> {
        let l = self.lookups.lock().unwrap();
        l.conn
            .query_row("SELECT wait_time FROM transfers WHERE id = ?1", params![transfer_id], |r| r.get::<_, Option<f64>>(0))
            .ok()
            .flatten()
            .map(|v| v as f32)
    }

    /// `transfer_paths.wait_time_start` / `wait_time_end` of one named route of a transfer.
    pub fn transfer_waits(&self, transfer_id: u32, path_name: &str) -> Option<(f32, f32)> {
        let l = self.lookups.lock().unwrap();
        l.conn
            .query_row(
                "SELECT COALESCE(wait_time_start, 0), COALESCE(wait_time_end, 0) FROM transfer_paths WHERE owner_id = ?1 AND path_name = ?2 LIMIT 1",
                params![transfer_id, path_name],
                |r| Ok((r.get::<_, f64>(0)? as f32, r.get::<_, f64>(1)? as f32)),
            )
            .ok()
    }

    /// The clip (`.cal` entry name, lower case) of an animation action (`anim_actions.anim_name`): the idle pose of an NPC.
    pub fn anim_action_clip(&self, id: u32) -> Option<String> {
        let mut l = self.lookups.lock().unwrap();
        if let Some(c) = l.anim_actions.get(&id) {
            return c.clone();
        }
        let clip: Option<String> = l
            .conn
            .query_row("SELECT anim_name FROM anim_actions WHERE id = ?1", params![id], |r| r.get::<_, Option<String>>(0))
            .ok()
            .flatten()
            .map(|n| n.to_ascii_lowercase())
            .filter(|n| !n.is_empty() && n != "none");
        l.anim_actions.insert(id, clip.clone());
        clip
    }

    /// The tool models an NPC pose holds (`anim_actions.mainhand_tool_id` / `offhand_tool_id`): main hand first.
    pub fn anim_action_tools(&self, anim_action: u32) -> [Option<Tool>; 2] {
        let mut l = self.lookups.lock().unwrap();
        if let Some(t) = l.anim_action_tools.get(&anim_action) {
            return t.clone();
        }
        let ids: Option<(u32, u32)> = l
            .conn
            .query_row(
                "SELECT COALESCE(mainhand_tool_id, 0), COALESCE(offhand_tool_id, 0) FROM anim_actions WHERE id = ?1",
                params![anim_action],
                |r| Ok((r.get::<_, i64>(0)? as u32, r.get::<_, i64>(1)? as u32)),
            )
            .ok();
        let tools = match ids {
            Some((main, off)) => [tool_from_conn(&l.conn, main), tool_from_conn(&l.conn, off)],
            None => [None, None],
        };
        l.anim_action_tools.insert(anim_action, tools.clone());
        tools
    }

    /// What a model is ridden like (`models.mount_pose_id`: 1 horse, 2 lion, 4 wildboar, 10 pangolin; 0 for none).
    pub fn mount_pose(&self, model_id: u32) -> u32 {
        let mut l = self.lookups.lock().unwrap();
        if let Some(p) = l.mount_poses.get(&model_id) {
            return *p;
        }
        let pose: u32 = l
            .conn
            .query_row("SELECT mount_pose_id FROM models WHERE id = ?1", params![model_id], |r| r.get::<_, Option<u32>>(0))
            .ok()
            .flatten()
            .unwrap_or(0);
        l.mount_poses.insert(model_id, pose);
        pose
    }

    /// The actor model's base stance speeds/body a driven mate moves with. The first stance row
    /// (`stance_id = 1`) is the normal one the server's `Npc.BaseMoveSpeed` uses outside battle.
    pub fn mount_move(&self, model_id: u32) -> MountMove {
        let mut l = self.lookups.lock().unwrap();
        if let Some(m) = l.mount_moves.get(&model_id) {
            return *m;
        }
        let d = MountMove::default();
        let out = l
            .conn
            .query_row(
                "SELECT g.ai_move_speed_walk, g.ai_move_speed_run, a.radius, a.height, \
                        a.game_jump_height, a.physics_living_gravity, a.turn_speed, \
                        a.game_inertia, a.game_inertia_accel \
                 FROM models m \
                 JOIN actor_models a ON a.id = m.sub_id AND m.sub_type = 'ActorModel' \
                 JOIN game_stances g ON g.actor_model_id = a.id \
                 WHERE m.id = ?1 AND g.stance_id = 1 LIMIT 1",
                params![model_id],
                |r| {
                    let n = |i: usize, default: f32| -> f32 {
                        r.get::<_, Option<f64>>(i).ok().flatten().map_or(default, |v| v as f32)
                    };
                    let turn = n(6, 360.0);
                    Ok(MountMove {
                        walk: n(0, d.walk).max(0.1),
                        run: n(1, d.run).max(0.1),
                        radius: n(2, d.radius).max(0.1),
                        height: n(3, d.height).max(0.1),
                        jump_height: n(4, d.jump_height).max(0.0),
                        gravity: n(5, d.gravity).max(1.0),
                        turn: if turn > 0.0 { turn.to_radians() } else { d.turn },
                        inertia: n(7, d.inertia).max(0.1),
                        accel: n(8, d.accel).max(0.1),
                    })
                },
            )
            .unwrap_or(d);
        l.mount_moves.insert(model_id, out);
        out
    }

    /// Where the attach point `point` (`model_attach_point_strings`, `$cannon0`) of a slave model is, in the body's frame: the
    /// helper of that name in the model's prefab (`vehicle_models.normal`). The server announces a child doodad (the Farm
    /// Wagon's cargo crates) at the parent's origin; the client puts it on the point.
    pub fn slave_attach_offset(&self, model_id: u32, point: u8) -> Option<Vec3> {
        let l = self.lookups.lock().unwrap();
        let helper: String = l.conn.query_row("SELECT prefab FROM model_attach_point_strings WHERE id = ?1", params![point], |r| r.get(0)).ok()?;
        let prefab: String = l
            .conn
            .query_row(
                "SELECT v.normal FROM models m JOIN vehicle_models v ON v.id = m.sub_id AND m.sub_type = 'VehicleModel' WHERE m.id = ?1",
                params![model_id],
                |r| r.get(0),
            )
            .ok()?;
        drop(l);
        prefab_helper_pos(&prefab, helper.trim_start_matches('$'))
    }

    /// The wheel models of a land vehicle (`vehicle_models.wheel` for the bones `wheel0..9`, `wheel2` for `wheel_type20..9`):
    /// pak paths of `.cgf` files (the columns are model paths, not counts).
    pub fn vehicle_wheel_models(&self, model_id: u32) -> VehicleWheelModels {
        let l = self.lookups.lock().unwrap();
        l.conn
            .query_row(
                "SELECT v.wheel, v.wheel2 FROM models m JOIN vehicle_models v ON v.id = m.sub_id AND m.sub_type = 'VehicleModel' WHERE m.id = ?1 LIMIT 1",
                params![model_id],
                |r| {
                    let path = |i: usize| r.get::<_, Option<String>>(i).ok().flatten().as_deref().and_then(model_file);
                    Ok(VehicleWheelModels { wheel: path(0), wheel2: path(1) })
                },
            )
            .unwrap_or_default()
    }

    /// The `vehicle_models` row a slave model draws (`models.sub_type = 'VehicleModel'`).
    pub fn vehicle_body(&self, model_id: u32) -> VehicleBody {
        let l = self.lookups.lock().unwrap();
        if let Some(v) = l.vehicle_bodies.get(&model_id) {
            return *v;
        }
        drop(l);
        // `vehicle_models.wheel` is a wheel MODEL path, not a count: the wheel entries of `VehicleMove` are the wheel bones of
        // the body (`wheelN`, `wheel_typeN`), 4 for the Farm Wagon.
        let wheels = self
            .actor_model_file(model_id)
            .and_then(|file| aa_cry::skeleton::Skeleton::load(&file).ok())
            .map(|skeleton| super::wheels::count_wheel_bones(skeleton.bones.iter().map(|b| b.name.as_str())).min(10) as u8);
        let mut l = self.lookups.lock().unwrap();
        let d = VehicleBody { wheels: wheels.unwrap_or(VehicleBody::default().wheels), ..VehicleBody::default() };
        let out = l
            .conn
            .query_row(
                "SELECT v.velocity, v.angVel FROM models m \
                 JOIN vehicle_models v ON v.id = m.sub_id AND m.sub_type = 'VehicleModel' \
                 WHERE m.id = ?1 LIMIT 1",
                params![model_id],
                |r| {
                    let n = |i: usize, default: f32| -> f32 {
                        r.get::<_, Option<f64>>(i).ok().flatten().map_or(default, |v| v as f32)
                    };
                    Ok(VehicleBody {
                        speed: n(0, d.speed).max(0.1),
                        turn: n(1, d.turn).abs().max(0.05),
                        wheels: d.wheels,
                    })
                },
            )
            .unwrap_or(d);
        l.vehicle_bodies.insert(model_id, out);
        out
    }

    /// The `ship_models` row a slave model draws (`models.sub_type = 'ShipModel'`).
    pub fn ship_body(&self, model_id: u32) -> ShipBody {
        let mut l = self.lookups.lock().unwrap();
        if let Some(v) = l.ship_bodies.get(&model_id) {
            return *v;
        }
        let d = ShipBody::default();
        let mut out = l
            .conn
            .query_row(
                "SELECT h.velocity, h.reverse_velocity, h.steer_vel, h.mass_box_size_x, h.mass_box_size_y \
                 FROM models m JOIN ship_models h ON h.id = m.sub_id AND m.sub_type = 'ShipModel' \
                 WHERE m.id = ?1 LIMIT 1",
                params![model_id],
                |r| {
                    let n = |i: usize, default: f32| -> f32 {
                        r.get::<_, Option<f64>>(i).ok().flatten().map_or(default, |v| v as f32)
                    };
                    Ok(ShipBody {
                        speed: n(0, d.speed).max(0.1),
                        reverse: n(1, d.reverse).abs().max(0.1),
                        turn: n(2, d.turn).abs().max(0.05),
                        half: [(n(3, d.half[0] * 2.0) * 0.5).max(0.5), (n(4, d.half[1] * 2.0) * 0.5).max(1.0)],
                        deck: d.deck,
                    })
                },
            )
            .unwrap_or(d);
        out.deck = ship_seat(model_id, 1).local[2];
        l.ship_bodies.insert(model_id, out);
        out
    }

    /// The walkable surface of a ship's hull (its body brush's upward-facing triangles, in the ship's cry frame), loaded
    /// once; `None` for a model without a readable hull.
    pub fn ship_floor(&self, model_id: u32) -> Option<Arc<ShipFloor>> {
        let mut l = self.lookups.lock().unwrap();
        if let Some(v) = l.ship_floors.get(&model_id) {
            return v.clone();
        }
        let parts = ship_parts_of(&l.conn, model_id);
        let out = parts.iter().find(|p| p.is_hull()).and_then(|hull| {
            let model = aa_cry::mesh::Model::load(&hull.path).ok()?;
            let floor = ShipFloor::from_model(&model, hull);
            (!floor.is_empty()).then(|| Arc::new(floor))
        });
        l.ship_floors.insert(model_id, out.clone());
        out
    }

    /// The seat of a slave model at an attach point. Ships use the authored point from
    /// `slave_attach_points.json` (the server data the client's UI is built from); land vehicles
    /// keep the native root seat until their model attach oracle exists.
    pub fn seat(&self, model_id: u32, point: u8) -> Seat {
        let seat_point = if point <= 1 { 1 } else { 2 };
        let mut l = self.lookups.lock().unwrap();
        if let Some(s) = l.seats.get(&(model_id, seat_point)) {
            return *s;
        }
        let out = ship_seat(model_id, seat_point);
        l.seats.insert((model_id, seat_point), out);
        out
    }

    /// Whether a slave model is a `ShipModel` rather than a land vehicle / siege model.
    pub fn is_ship(&self, model_id: u32) -> bool {
        let mut l = self.lookups.lock().unwrap();
        if let Some(v) = l.ships.get(&model_id) {
            return *v;
        }
        let out = l
            .conn
            .query_row("SELECT 1 FROM models WHERE id = ?1 AND sub_type = 'ShipModel'", params![model_id], |_| Ok(()))
            .is_ok();
        l.ships.insert(model_id, out);
        out
    }

    /// Whether a slave template is a cannon / harpoon gun (`slaves.slave_kind_id` 8, "slave equipment"): the
    /// child slaves a ship carries at its cannon attach points, manned from the driver seat.
    pub fn is_cannon(&self, template_id: u32) -> bool {
        let mut l = self.lookups.lock().unwrap();
        if let Some(v) = l.slave_kinds.get(&template_id) {
            return *v == SLAVE_KIND_CANNON;
        }
        let kind = l
            .conn
            .query_row("SELECT coalesce(slave_kind_id, 0) FROM slaves WHERE id = ?1", params![template_id], |r| r.get::<_, u32>(0))
            .unwrap_or(0);
        l.slave_kinds.insert(template_id, kind);
        kind == SLAVE_KIND_CANNON
    }

    /// Whether a slave model is a land vehicle (`models.sub_type = 'VehicleModel'`) rather than a
    /// ship/siege model.
    pub fn is_land_vehicle(&self, model_id: u32) -> bool {
        let mut l = self.lookups.lock().unwrap();
        if let Some(v) = l.land_vehicles.get(&model_id) {
            return *v;
        }
        let out = l
            .conn
            .query_row("SELECT 1 FROM models WHERE id = ?1 AND sub_type = 'VehicleModel'", params![model_id], |_| Ok(()))
            .is_ok();
        l.land_vehicles.insert(model_id, out);
        out
    }

    /// `npcs.scale` of an NPC template (1 when unknown).
    pub fn npc_scale(&self, template_id: u32) -> f32 {
        let mut l = self.lookups.lock().unwrap();
        if let Some(s) = l.npc_scale.get(&template_id) {
            return *s;
        }
        let scale: f32 = l
            .conn
            .query_row("SELECT scale FROM npcs WHERE id = ?1", params![template_id], |r| r.get::<_, Option<f64>>(0))
            .ok()
            .flatten()
            .map(|s| s as f32)
            .filter(|s| *s > 0.0)
            .unwrap_or(1.0);
        l.npc_scale.insert(template_id, scale);
        scale
    }

    /// The model and attachment data of a weapon / shield item template.
    pub fn weapon(&self, item_id: u32) -> Option<Weapon> {
        let mut l = self.lookups.lock().unwrap();
        if let Some(w) = l.weapons.get(&item_id) {
            return w.clone();
        }
        let weapon = l
            .conn
            .query_row(
                "SELECT ia.path, h.code, ia.attachment_offset_pos_x, ia.attachment_offset_pos_y, ia.attachment_offset_pos_z,                  ia.attachment_offset_rot_x, ia.attachment_offset_rot_y, ia.attachment_offset_rot_z, w.worn_scale, w.drawn_scale, h.slot_type_id, h.pose_id, ia.allow_mirror                  FROM item_weapons w JOIN item_assets ia ON ia.id = w.asset_id JOIN holdables h ON h.id = w.holdable_id WHERE w.item_id = ?1",
                params![item_id],
                |r| {
                    let f = |i: usize| -> rusqlite::Result<f32> { Ok(r.get::<_, Option<f64>>(i)?.unwrap_or(0.0) as f32) };
                    Ok(Weapon {
                        path: norm(&r.get::<_, String>(0)?),
                        code: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                        offset: [f(2)?, f(3)?, f(4)?],
                        rotation: [f(5)?, f(6)?, f(7)?],
                        worn_scale: f(8)?,
                        drawn_scale: f(9)?,
                        two_handed: r.get::<_, Option<i64>>(10)?.unwrap_or(0) == 16,
                        staff: r.get::<_, Option<i64>>(11)?.unwrap_or(0) == 4,
                        mirror: matches!(r.get::<_, Option<String>>(12)?.as_deref(), Some("t" | "T" | "1")),
                    })
                },
            )
            .ok();
        l.weapons.insert(item_id, weapon.clone());
        weapon
    }

    /// The model a backpack / glider item template wears on the back (`item_backpacks`): its stowed look when it has one
    /// (`asset2_id`: a glider's folded `_back.cgf`), else `asset_id`. [`Weapon::code`] is `backpack`. A glider without a stowed look
    /// (`glider_bx.chr`, 5 items) is only its flying model, a 6 m canopy: nothing is drawn for it.
    pub fn backpack(&self, item_id: u32) -> Option<Weapon> {
        let mut l = self.lookups.lock().unwrap();
        if let Some(w) = l.backpacks.get(&item_id) {
            return w.clone();
        }
        let backpack = l
            .conn
            .query_row(
                "SELECT ia.path, ia.attachment_offset_pos_x, ia.attachment_offset_pos_y, ia.attachment_offset_pos_z, ia.attachment_offset_rot_x, ia.attachment_offset_rot_y, ia.attachment_offset_rot_z
                 FROM item_backpacks b JOIN item_assets ia ON ia.id = CASE WHEN b.asset2_id > 0 THEN b.asset2_id ELSE b.asset_id END WHERE b.item_id = ?1 AND NOT (b.backpack_type_id = 2 AND b.asset2_id = 0)",
                params![item_id],
                |r| {
                    let f = |i: usize| -> rusqlite::Result<f32> { Ok(r.get::<_, Option<f64>>(i)?.unwrap_or(0.0) as f32) };
                    Ok(Weapon {
                        path: norm(&r.get::<_, String>(0)?),
                        code: "backpack".into(),
                        offset: [f(1)?, f(2)?, f(3)?],
                        rotation: [f(4)?, f(5)?, f(6)?],
                        worn_scale: 1.0,
                        drawn_scale: 1.0,
                        two_handed: false,
                        staff: false,
                        mirror: false,
                    })
                },
            )
            .ok();
        l.backpacks.insert(item_id, backpack.clone());
        backpack
    }

    /// The glider a backpack item is (`backpack_type_id` 2): its use skill and flying model; `None` for other items.
    pub fn glider(&self, item_id: u32) -> Option<GliderItem> {
        let mut l = self.lookups.lock().unwrap();
        if let Some(g) = l.gliders.get(&item_id) {
            return g.clone();
        }
        let glider = l
            .conn
            .query_row(
                "SELECT i.use_skill_id, ia.path FROM item_backpacks b JOIN items i ON i.id = b.item_id LEFT JOIN item_assets ia ON ia.id = b.asset_id                  WHERE b.item_id = ?1 AND b.backpack_type_id = 2",
                params![item_id],
                |r| Ok(GliderItem { use_skill: r.get::<_, Option<i64>>(0)?.unwrap_or(0).max(0) as u32, model: r.get::<_, Option<String>>(1)?.map(|p| norm(&p)) }),
            )
            .ok();
        l.gliders.insert(item_id, glider.clone());
        glider
    }

    /// The model a doodad template is drawn with (`doodad_almighties.model`: `cgf://`, `vegetation://`, `cga://` or
    /// `prefab://`).
    pub fn doodad_model(&self, template_id: u32) -> Option<String> {
        let mut l = self.lookups.lock().unwrap();
        if let Some(m) = l.doodad_models.get(&template_id) {
            return m.clone();
        }
        let model = almighty_model(&l.conn, template_id);
        l.doodad_models.insert(template_id, model.clone());
        model
    }

    /// The model of a doodad phase (`doodad_func_groups.model`) as a pak path; an empty (or unusable) phase model
    /// falls back to the template's [`Db::doodad_model`].
    pub fn doodad_phase_model(&self, template_id: u32, func_group_id: u32) -> Option<String> {
        let key = (template_id, func_group_id);
        let raw = {
            let l = self.lookups.lock().unwrap();
            if let Some(m) = l.doodad_phase_models.get(&key) {
                return m.clone();
            }
            l.conn
                .query_row(
                    "SELECT model FROM doodad_func_groups WHERE id = ?1 AND doodad_almighty_id = ?2",
                    params![func_group_id, template_id],
                    |r| r.get::<_, Option<String>>(0),
                )
                .ok()
                .flatten()
        };
        let model = raw.as_deref().and_then(model_file).or_else(|| self.doodad_model(template_id));
        self.lookups.lock().unwrap().doodad_phase_models.insert(key, model.clone());
        model
    }

    /// The climb kind of a doodad phase (`doodad_funcs`' `DoodadFuncClimb` ->
    /// `doodad_func_climbs.climb_type_id`), or `None` when the phase is not a ladder. The kinds are the
    /// client's (`FUN_39132be0`): 1/6 ladder, 2 rope, 3 wood, 4 net, 5 hanging rope, 7 mast,
    /// 8 guardrail, 9 45-degree ladder, 10 big tree.
    pub fn ladder_kind(&self, func_group_id: u32) -> Option<u8> {
        let mut l = self.lookups.lock().unwrap();
        if let Some(kind) = l.ladder_kinds.get(&func_group_id) {
            return *kind;
        }
        let kind: Option<u8> = l
            .conn
            .query_row(
                "SELECT c.climb_type_id FROM doodad_funcs f JOIN doodad_func_climbs c ON c.id = f.actual_func_id \
                 WHERE f.doodad_func_group_id = ?1 AND f.actual_func_type = 'DoodadFuncClimb' ORDER BY f.id LIMIT 1",
                params![func_group_id],
                |r| Ok(r.get::<_, Option<i64>>(0)?.unwrap_or(0).clamp(0, 255) as u8),
            )
            .ok()
            .filter(|kind| *kind != 0);
        l.ladder_kinds.insert(func_group_id, kind);
        kind
    }

    /// The complete phase geometry reference. Prefabs retain their identity and all parts.
    pub fn doodad_phase_visual(&self, template_id: u32, func_group_id: u32) -> Option<String> {
        let key = (template_id, func_group_id);
        let mut l = self.lookups.lock().unwrap();
        if let Some(reference) = l.doodad_visuals.get(&key) { return reference.clone(); }
        let phase = l.conn.query_row("SELECT model FROM doodad_func_groups WHERE id=?1 AND doodad_almighty_id=?2", params![func_group_id,template_id], |r| r.get::<_,Option<String>>(0)).ok().flatten();
        let base = l.conn.query_row("SELECT model FROM doodad_almighties WHERE id=?1",params![template_id],|r|r.get::<_,Option<String>>(0)).ok().flatten();
        let normalize = |raw: &str| {
            let raw = raw.trim().replace('\\',"/");
            if raw.to_ascii_lowercase().starts_with("prefab://") {
                let (file,name) = raw["prefab://".len()..].rsplit_once('/')?;
                let reference = format!("prefab://{}/{}",norm(file),name);
                (!super::housing::prefab::load(&reference).is_empty()).then_some(reference)
            } else { model_file(&raw) }
        };
        let reference = phase.as_deref().and_then(normalize).or_else(||base.as_deref().and_then(normalize));
        l.doodad_visuals.insert(key,reference.clone());
        reference
    }

    /// The doodad template a planting item places (`item_spawn_doodads`: seeds, saplings, livestock, ladders ...), `None` for
    /// every other item.
    pub fn item_doodad(&self, item_template: u32) -> Option<u32> {
        let l = self.lookups.lock().unwrap();
        l.conn
            .query_row("SELECT doodad_id FROM item_spawn_doodads WHERE item_id = ?1 ORDER BY id LIMIT 1", params![item_template], |r| r.get::<_, i64>(0))
            .ok()
            .map(|d| d as u32)
    }

    /// The model a new doodad of `template_id` has (the model of its start phase, group kind 1) and the room it keeps
    /// around it (`sim_radius`, metres).
    pub fn doodad_start(&self, template_id: u32) -> (Option<String>, f32) {
        let (group, radius) = {
            let l = self.lookups.lock().unwrap();
            let group = l
                .conn
                .query_row("SELECT id FROM doodad_func_groups WHERE doodad_almighty_id = ?1 AND doodad_func_group_kind_id = 1 ORDER BY id LIMIT 1", params![template_id], |r| r.get::<_, i64>(0))
                .ok();
            let radius = l.conn.query_row("SELECT sim_radius FROM doodad_almighties WHERE id = ?1", params![template_id], |r| r.get::<_, Option<i64>>(0)).ok().flatten().unwrap_or(0);
            (group, radius as f32 / 1000.0)
        };
        let model = group.map_or_else(|| self.doodad_model(template_id), |g| self.doodad_phase_model(template_id, g as u32));
        (model, radius)
    }

    /// How a phase grows (`doodad_phase_funcs` -> `doodad_func_growths`), `None` when it does not.
    pub fn doodad_growth(&self, func_group_id: u32) -> Option<DoodadGrowth> {
        let mut l = self.lookups.lock().unwrap();
        if let Some(g) = l.doodad_growths.get(&func_group_id) {
            return *g;
        }
        let growth: Option<DoodadGrowth> = l
            .conn
            .query_row(
                "SELECT g.delay, g.start_scale, g.end_scale
                 FROM doodad_phase_funcs p JOIN doodad_func_growths g ON g.id = p.actual_func_id
                 WHERE p.doodad_func_group_id = ?1 AND p.actual_func_type = 'DoodadFuncGrowth'
                 ORDER BY p.id LIMIT 1",
                params![func_group_id],
                |r| {
                    Ok(DoodadGrowth {
                        delay_ms: r.get::<_, Option<i64>>(0)?.unwrap_or(0).max(0) as u32,
                        start_scale: r.get::<_, Option<i64>>(1)?.unwrap_or(1000) as f32 / 1000.0,
                        end_scale: r.get::<_, Option<i64>>(2)?.unwrap_or(1000) as f32 / 1000.0,
                    })
                },
            )
            .ok();
        l.doodad_growths.insert(func_group_id, growth);
        growth
    }
}

/// `doodad_almighties.model` as a pak path.
fn almighty_model(conn: &Connection, template_id: u32) -> Option<String> {
    conn.query_row("SELECT model FROM doodad_almighties WHERE id = ?1", params![template_id], |r| r.get::<_, Option<String>>(0))
        .ok()
        .flatten()
        .and_then(|m| model_file(&m))
}

/// A database model reference as a pak path: `cgf://Game/x.cgf`, `vegetation://x.cgf`, `cga://x.cga` lose their
/// scheme (and `game/`), a `prefab://` path becomes the `object_Model` of its prefab. Empty and unusable references are
/// `None`.
fn model_file(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if raw.to_ascii_lowercase().starts_with("prefab://") {
        return prefab_model_file(&format!("prefab://{}", &raw["prefab://".len()..]));
    }
    let mut path = norm(raw);
    for scheme in ["cgf://", "cga://", "vegetation://"] {
        if let Some(rest) = path.strip_prefix(scheme) {
            path = rest.to_string();
            break;
        }
    }
    let path = path.strip_prefix("game/").unwrap_or(&path).to_string();
    path.contains('.').then_some(path)
}

impl Db {
    /// The particle effects of a prefab model without a model of its own (the summoned portal gates, `models` 308 / 667:
    /// an invisible proxy brush and `ParticleEffect` entities): its first state's (`prefab_elements.state_id` 1, alive)
    /// effects, positions relative to the unit. Sounds (`x2_sounds.*`) are left out.
    pub fn prefab_effects(&self, model_id: u32) -> Vec<aa_world::placement::PlacedEffect> {
        let l = self.lookups.lock().unwrap();
        let path: Option<String> = l
            .conn
            .query_row(
                "SELECT e.file_path FROM models m JOIN prefab_elements e ON e.prefab_model_id = m.sub_id AND m.sub_type = 'PrefabModel' WHERE m.id = ?1 ORDER BY e.state_id LIMIT 1",
                params![model_id],
                |r| r.get::<_, Option<String>>(0),
            )
            .ok()
            .flatten();
        drop(l);
        path.map(|p| prefab_effects_of(&p)).unwrap_or_default()
    }
}

/// The `ParticleEffect` entities (the `<export>` list) of a prefab element path.
fn prefab_effects_of(path: &str) -> Vec<aa_world::placement::PlacedEffect> {
    let Some((file, name)) = path.strip_prefix("prefab://").and_then(|r| r.rsplit_once('/')) else { return Vec::new() };
    let Ok(xml) = aa_pak::Pak::global().read_string(&norm(file)) else { return Vec::new() };
    let Some(from) = xml.find(&format!("<Prefab Name=\"{name}\"")) else { return Vec::new() };
    let tail = &xml[from..];
    let prefab = &tail[..tail.find("</Prefab>").unwrap_or(tail.len())];
    let Some(start) = prefab.find("<export>") else { return Vec::new() };
    let export = &prefab[start..prefab[start..].find("</export>").map_or(prefab.len(), |e| start + e + "</export>".len())];
    aa_world::placement::parse_effect_entities(export).into_iter().filter(|e| !e.effect.to_ascii_lowercase().starts_with("x2_sounds.")).collect()
}

/// One static brush of a ship's prefab (`ship_models.normal`): the hull, rudder, cannons, cargo. Positions are
/// relative to the ship's origin, cry space.
#[derive(Clone, Debug, PartialEq)]
pub struct ShipPart {
    pub name: String,
    /// Pak path of the `.cgf`, lower case.
    pub path: String,
    pub pos: Vec3,
    /// Cry space (`Rotate="w,x,y,z"` in the file).
    pub rot: Quat,
    pub scale: Vec3,
}

/// The model variants of a ship (`ship_models.normal` / `damaged25` / `damaged50` / `damaged75` / `dead`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShipVariants {
    pub normal: Option<String>,
    pub damaged25: Option<String>,
    pub damaged50: Option<String>,
    pub damaged75: Option<String>,
    pub dead: Option<String>,
}

impl ShipVariants {
    fn of(&self, state: u8) -> Option<&str> {
        match state {
            1 => self.normal.as_deref(),
            2 => self.damaged25.as_deref(),
            3 => self.damaged50.as_deref(),
            4 => self.damaged75.as_deref(),
            6 => self.dead.as_deref(),
            _ => None,
        }
    }

    /// The prefab of damage state `state`. `FUN_397a7980` walks down from the wanted state until a
    /// variant exists, so a missing `damaged25` falls back to the whole hull.
    pub fn path(&self, state: u8) -> Option<&str> {
        (1..=state.min(6)).rev().find_map(|s| self.of(s)).or(self.normal.as_deref())
    }
}

/// The visual model state of a ship from its health (`FUN_3989f8b0`): 1 normal, 2 damaged25,
/// 3 damaged50, 4 damaged75, 6 dead. Zero health (or an unknown maximum) is the dead hull.
pub fn ship_damage_state(hp: i32, max_hp: i32) -> u8 {
    if hp == 0 || max_hp == 0 {
        return 6;
    }
    let pct = (i64::from(hp) * 100 / i64::from(max_hp)) as i32;
    if pct > 75 {
        1
    } else if pct > 50 {
        2
    } else if pct < 26 {
        4
    } else {
        3
    }
}

/// A declared equipment slot of a slave template (`slave_equip_slots`): the part slot and the
/// attach point its doodad sits on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShipEquipSlot {
    pub attach: u8,
    pub slot: u8,
}

/// A default doodad of a slave model at an attach point (`slave_doodad_bindings`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShipDoodadBinding {
    pub attach: u8,
    pub doodad: u32,
    pub scale: f32,
}

/// A doodad the hull shows at an attach point: an equipped slot's part or a default binding.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShipDoodad {
    pub attach: u8,
    pub doodad: u32,
    pub scale: f32,
}

/// One attach point of a ship model (`slave_attach_points.json`), cry space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShipAttach {
    pub pos: Vec3,
    pub yaw: f32,
}

/// The parts and effects of one ship damage state plus the installed equipment.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShipVisualBuild {
    pub parts: Vec<ShipPart>,
    pub effects: Vec<aa_world::placement::PlacedEffect>,
}

/// The walkable surface of a ship hull: the upward-facing triangles of its body brush in the ship's frame (cry space: x
/// right, y forward, z up, origin at the hull root), bucketed by 1 m cells of the deck plane.
#[derive(Debug, Default)]
pub struct ShipFloor {
    tris: Vec<[Vec3; 3]>,
    cells: HashMap<(i32, i32), Vec<u32>>,
}

/// A triangle whose normal is at least this vertical is a floor (a wall or a hull side is not); the winding of the
/// file's faces is not trusted, so the side does not matter.
const FLOOR_NORMAL_Z: f32 = 0.5;

impl ShipFloor {
    pub fn new(tris: impl IntoIterator<Item = [Vec3; 3]>) -> ShipFloor {
        let mut floor = ShipFloor::default();
        for tri in tris {
            let normal = (tri[1] - tri[0]).cross(tri[2] - tri[0]);
            if normal.length_squared() < 1e-12 || normal.normalize().z.abs() < FLOOR_NORMAL_Z {
                continue;
            }
            let id = floor.tris.len() as u32;
            let (lo, hi) = (tri[0].min(tri[1]).min(tri[2]), tri[0].max(tri[1]).max(tri[2]));
            for cx in lo.x.floor() as i32..=hi.x.floor() as i32 {
                for cy in lo.y.floor() as i32..=hi.y.floor() as i32 {
                    floor.cells.entry((cx, cy)).or_default().push(id);
                }
            }
            floor.tris.push(tri);
        }
        floor
    }

    /// The floor triangles of the hull brush `part` of `model` (render nodes of the highest detail).
    pub fn from_model(model: &aa_cry::mesh::Model, part: &ShipPart) -> ShipFloor {
        let place = |p: Vec3| part.pos + part.rot * (p * part.scale);
        let mut tris = Vec::new();
        for node in model.lod_nodes(0) {
            let Some(mesh) = model.nodes[node].mesh.and_then(|m| model.meshes.get(m)) else { continue };
            let to_model = model.world_transform(node);
            let at = |i: u32| mesh.positions.get(i as usize).map(|p| place(to_model.transform_point3(*p)));
            for tri in mesh.indices.as_chunks::<3>().0 {
                if let (Some(a), Some(b), Some(c)) = (at(tri[0]), at(tri[1]), at(tri[2])) {
                    tris.push([a, b, c]);
                }
            }
        }
        ShipFloor::new(tris)
    }

    pub fn is_empty(&self) -> bool {
        self.tris.is_empty()
    }

    /// The highest floor at `(x, y)` of the ship's frame that is not above `max_z`.
    pub fn height(&self, x: f32, y: f32, max_z: f32) -> Option<f32> {
        let ids = self.cells.get(&(x.floor() as i32, y.floor() as i32))?;
        let mut best: Option<f32> = None;
        for &id in ids {
            let [a, b, c] = self.tris[id as usize];
            // barycentric coordinates of (x, y) in the triangle's projection on the deck plane
            let (v0, v1, v2) = (Vec2::new(b.x - a.x, b.y - a.y), Vec2::new(c.x - a.x, c.y - a.y), Vec2::new(x - a.x, y - a.y));
            let det = v0.x * v1.y - v1.x * v0.y;
            if det.abs() < 1e-9 {
                continue;
            }
            let u = (v2.x * v1.y - v1.x * v2.y) / det;
            let v = (v0.x * v2.y - v2.x * v0.y) / det;
            if u < -1e-4 || v < -1e-4 || u + v > 1.0 + 1e-4 {
                continue;
            }
            let z = a.z + u * (b.z - a.z) + v * (c.z - a.z);
            if z <= max_z && best.is_none_or(|h| z > h) {
                best = Some(z);
            }
        }
        best
    }
}

impl ShipPart {
    /// The ship's body brush (`galleon_body.cgf`, named `body`).
    pub fn is_hull(&self) -> bool {
        self.name == "body" || self.path.ends_with("_body.cgf")
    }
}

/// The static brushes of the prefab at `path` (`prefab://prefabs/galleon.xml/galleon.step_10galleon_normal`).
fn prefab_brushes(path: &str) -> Vec<ShipPart> {
    prefab_part_objects(path, false)
}

/// The brushes and animated objects of a transfer's prefab (the airship's body, wings, propellers and planks).
fn prefab_parts(path: &str) -> Vec<ShipPart> {
    prefab_part_objects(path, true)
}

fn prefab_part_objects(path: &str, anim_objects: bool) -> Vec<ShipPart> {
    let Some((file, name)) = path.strip_prefix("prefab://").and_then(|r| r.rsplit_once('/')) else { return Vec::new() };
    let Ok(xml) = aa_pak::Pak::global().read_string(&norm(file)) else { return Vec::new() };
    // the slave table and the prefab file disagree on case
    let Some(from) = xml.to_ascii_lowercase().find(&format!("<prefab name=\"{}\"", name.to_ascii_lowercase())) else { return Vec::new() };
    let tail = &xml[from..];
    let prefab = &tail[..tail.find("</Prefab>").map_or(tail.len(), |e| e + "</Prefab>".len())];
    parse_prefab_part_objects(prefab, anim_objects)
}

fn parse_prefab_part_objects(prefab: &str, anim_objects: bool) -> Vec<ShipPart> {
    let Ok(doc) = roxmltree::Document::parse(prefab) else { return Vec::new() };
    let floats = |s: Option<&str>| -> Vec<f32> { s.map(|s| s.split(',').filter_map(|v| v.trim().parse().ok()).collect()).unwrap_or_default() };
    let mut out = Vec::new();
    for o in doc.descendants().filter(|n| n.has_tag_name("Object")) {
        let model = match o.attribute("Type") {
            Some("Brush") => o.attribute("Prefab").map(norm).filter(|m| m.ends_with(".cgf")),
            Some("Entity") if anim_objects && o.attribute("EntityClass") == Some("AnimObject") => o
                .children()
                .find(|c| c.has_tag_name("Properties"))
                .and_then(|p| p.attribute("object_Model"))
                .map(norm)
                .filter(|m| m.contains('.')),
            _ => None,
        };
        let Some(model) = model else { continue };
        let pos = match floats(o.attribute("Pos"))[..] {
            [x, y, z] => Vec3::new(x, y, z),
            _ => Vec3::ZERO,
        };
        let rot = match floats(o.attribute("Rotate"))[..] {
            [w, x, y, z] => Quat::from_xyzw(x, y, z, w).normalize(),
            _ => Quat::IDENTITY,
        };
        let scale = match floats(o.attribute("Scale"))[..] {
            [x, y, z] => Vec3::new(x, y, z),
            _ => Vec3::ONE,
        };
        out.push(ShipPart { name: o.attribute("Name").unwrap_or_default().to_string(), path: model, pos, rot, scale });
    }
    out
}

fn ship_parts_of(conn: &Connection, model_id: u32) -> Vec<ShipPart> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT h.normal FROM models m JOIN ship_models h ON h.id = m.sub_id AND m.sub_type = 'ShipModel' WHERE m.id = ?1",
            params![model_id],
            |r| r.get::<_, Option<String>>(0),
        )
        .ok()
        .flatten();
    raw.map(|r| prefab_brushes(r.trim())).unwrap_or_default()
}

/// The doodads a hull shows: one per attach point an equipment slot owns (only when the slot holds
/// a part), else the authored `slave_doodad_bindings` default. An empty slot clears the part, so
/// the equipment really drives the visual.
fn resolve_ship_doodads(
    slots: &[ShipEquipSlot],
    bindings: &[ShipDoodadBinding],
    equipped: &Equipment,
    item_doodad: impl Fn(u32) -> Option<u32>,
) -> Vec<ShipDoodad> {
    let owned: HashSet<u8> = slots.iter().map(|s| s.attach).collect();
    let mut out: BTreeMap<u8, ShipDoodad> = BTreeMap::new();
    for slot in slots {
        let Some(template) = equipped.slots.get(usize::from(slot.slot)).and_then(Option::as_ref).map(|i| i.template_id) else { continue };
        if let Some(doodad) = item_doodad(template) {
            out.insert(slot.attach, ShipDoodad { attach: slot.attach, doodad, scale: 1.0 });
        }
    }
    for binding in bindings {
        if owned.contains(&binding.attach) || out.contains_key(&binding.attach) {
            continue;
        }
        out.insert(binding.attach, ShipDoodad { attach: binding.attach, doodad: binding.doodad, scale: binding.scale });
    }
    out.into_values().collect()
}

/// Places a doodad's own brushes at an attach point: the point's yaw turns the doodad, and its
/// scale applies to the doodad's local offsets and meshes.
fn place_ship_doodad(parts: Vec<ShipPart>, at: ShipAttach, scale: f32) -> Vec<ShipPart> {
    let turn = Quat::from_rotation_z(at.yaw);
    parts
        .into_iter()
        .map(|p| ShipPart {
            name: p.name,
            path: p.path,
            pos: at.pos + turn * (p.pos * scale),
            rot: turn * p.rot,
            scale: p.scale * scale,
        })
        .collect()
}

fn ship_doodad_parts_of(conn: &Connection, doodad_id: u32) -> Vec<ShipPart> {
    let raw: Option<String> = conn
        .query_row("SELECT model FROM doodad_almighties WHERE id = ?1", params![doodad_id], |r| r.get::<_, Option<String>>(0))
        .ok()
        .flatten();
    let Some(raw) = raw.map(|m| m.trim().to_string()).filter(|m| !m.is_empty()) else { return Vec::new() };
    if raw.to_ascii_lowercase().starts_with("prefab://") {
        let parts = prefab_brushes(&raw);
        if !parts.is_empty() {
            return parts;
        }
    }
    model_file(&raw).map(|path| vec![ShipPart { name: "doodad".into(), path, pos: Vec3::ZERO, rot: Quat::IDENTITY, scale: Vec3::ONE }]).unwrap_or_default()
}

/// `slave_attach_points.json` (the server's copy of the client data): model id -> point -> place.
/// The file is JSON with `//` notes and trailing commas; a missing or broken file just leaves the
/// hull without equipment doodads.
fn load_attach_points(path: &str) -> HashMap<u32, HashMap<u8, ShipAttach>> {
    let Ok(text) = std::fs::read_to_string(path) else { return HashMap::new() };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&relaxed_json(&text)) else {
        warn!("slave attach points {path}: not JSON");
        return HashMap::new();
    };
    let mut out = HashMap::new();
    for entry in json.as_array().into_iter().flatten() {
        let Some(model) = entry.get("ModelId").and_then(serde_json::Value::as_u64) else { continue };
        let mut points = HashMap::new();
        for (key, p) in entry.get("AttachPoints").and_then(serde_json::Value::as_object).into_iter().flatten() {
            let Ok(point) = key.parse::<u8>() else { continue };
            let Some(x) = p.get("X").and_then(serde_json::Value::as_f64) else { continue };
            let Some(y) = p.get("Y").and_then(serde_json::Value::as_f64) else { continue };
            let Some(z) = p.get("Z").and_then(serde_json::Value::as_f64) else { continue };
            let yaw = p.get("Yaw").and_then(serde_json::Value::as_f64).unwrap_or(0.0) as f32;
            points.insert(point, ShipAttach { pos: Vec3::new(x as f32, y as f32, z as f32), yaw: yaw.to_radians() });
        }
        out.insert(model as u32, points);
    }
    out
}

/// The JSON dialect of the Cry prefab tools: `//` / `/* */` comments and trailing commas.
fn relaxed_json(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut in_string = false;
    let mut escaped = false;
    while i < chars.len() {
        let c = chars[i];
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if c == '"' {
            in_string = true;
            out.push(c);
            i += 1;
        } else if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(chars.len());
        } else {
            out.push(c);
            i += 1;
        }
    }
    let chars: Vec<char> = out.chars().collect();
    let mut out = String::with_capacity(chars.len());
    for (i, &c) in chars.iter().enumerate() {
        if c == ',' {
            let next = chars[i + 1..].iter().find(|c| !c.is_whitespace());
            if matches!(next, Some(&'}') | Some(&']')) {
                continue;
            }
        }
        out.push(c);
    }
    out
}

/// The model file of a prefab element path (`prefab://prefabs/interaction_e.xml/scarecrow.learning_scarecrow_02`): the
/// prefab's first static `Brush` (`Prefab="...cgf"`, the houses / farms) or, for an effect-only prefab, its first
/// `object_Model`.
fn prefab_model_file(path: &str) -> Option<String> {
    let rest = path.strip_prefix("prefab://")?;
    let (file, name) = rest.rsplit_once('/')?;
    let xml = aa_pak::Pak::global().read_string(&norm(file)).ok()?;
    let from = xml.find(&format!("<Prefab Name=\"{name}\""))?;
    let tail = &xml[from..];
    let tail = &tail[..tail.find("</Prefab>").unwrap_or(tail.len())];
    if let Some(start) = tail.find("object_Model=\"") {
        let start = start + "object_Model=\"".len();
        let model = norm(&tail[start..start + tail[start..].find('"')?]);
        if !model.is_empty() {
            return Some(model);
        }
    }
    // static house/farm prefabs use a Brush instead of an object_Model entity
    let start = tail.find("Type=\"Brush\"")?;
    let start = start + tail[start..].find("Prefab=\"")? + "Prefab=\"".len();
    let model = &tail[start..start + tail[start..].find('"')?];
    let model = norm(model);
    model.ends_with(".cgf").then_some(model)
}

/// The `Pos` of the helper object `name` (a `Comment` such as `cannon0`) in the prefab element `prefab://<file>/<element>`
/// (cry coordinates, in the frame of the prefab's body).
fn prefab_helper_pos(path: &str, helper: &str) -> Option<Vec3> {
    let rest = path.strip_prefix("prefab://")?;
    let (file, name) = rest.rsplit_once('/')?;
    let xml = aa_pak::Pak::global().read_string(&norm(file)).ok()?;
    helper_pos_in(&xml, name, helper)
}

fn helper_pos_in(xml: &str, prefab: &str, helper: &str) -> Option<Vec3> {
    let from = xml.find(&format!("<Prefab Name=\"{prefab}\""))?;
    let tail = &xml[from..];
    let tail = &tail[..tail.find("</Prefab>").unwrap_or(tail.len())];
    let at = tail.find(&format!(" Name=\"{helper}\" Pos=\""))? + format!(" Name=\"{helper}\" Pos=\"").len();
    let raw = &tail[at..at + tail[at..].find('"')?];
    let v: Vec<f32> = raw.split(',').filter_map(|c| c.trim().parse().ok()).collect();
    (v.len() == 3).then(|| Vec3::new(v[0], v[1], v[2]))
}

/// `slave_attach_points.json` seats of the 1.2 ships (model id -> driver and passenger). The
/// server reads the same file; a ship without an authored second seat uses a point behind the
/// helm on the same deck.
fn ship_seat(model_id: u32, point: u8) -> Seat {
    let (driver, passenger) = match model_id {
        129 => ([0.0, -0.055_639_5, 1.377_99], [0.0, -1.778_73, 0.930_638]),
        1288 => ([0.0, 0.405_178, 1.409_84], [0.0, -1.5, 1.0]),
        128 => ([0.0, -2.9, 1.043_76], [0.0, -4.0, 1.0]),
        1205 => ([-0.004_175_19, -6.540_26, 1.558_67], [-0.004, -7.6, 1.6]),
        1383 | 1384 => ([0.000_815_43, -4.696_58, 6.151_29], [0.0, -5.8, 6.2]),
        1249 | 1482 => ([0.0, -12.510_9, 5.880_29], [0.0, -13.6, 5.9]),
        1046 => ([0.0, -20.079_1, 12.310_5], [0.0, -21.2, 12.3]),
        1546 => ([0.0, -24.590_7, 10.107_6], [0.0, -25.7, 10.1]),
        393 | 511 | 523 => ([0.0, -14.696_7, 6.208_24], [0.0, -15.8, 6.2]),
        1287 => ([0.0, 0.405_178, 1.409_84], [0.0, -1.5, 1.0]),
        _ => ([0.0, 0.0, 1.1], [0.0, -1.5, 1.1]),
    };
    let (local, yaw) = if point <= 1 {
        (driver, if matches!(model_id, 129 | 1288 | 1287) { std::f32::consts::PI } else { 0.0 })
    } else {
        (passenger, 0.0)
    };
    Seat { local, yaw }
}

/// Lower case with `/` separators, as the pak wants paths.
pub fn norm(p: &str) -> String {
    p.replace('\\', "/").to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A house prefab's static part is a `Brush` (`Prefab="...cgf"`), not an `object_Model` entity: the single
    /// `object_Model` fallback would choose a sprinkler or nothing.
    #[test]
    fn a_house_prefab_uses_its_brush_model() {
        if !aa_cry::pak_available() {
            return;
        }
        let model = prefab_model_file("prefab://Prefabs/housing_farm.xml/housing_farm.step1").expect("step1 model");
        assert!(model.ends_with("nuia_housing_scarecrow_b.cgf"), "{model}");
        let step0 = prefab_model_file("prefab://Prefabs/housing_farm.xml/housing_farm.step0").expect("step0 model");
        assert!(step0.ends_with("nuia_housing_scarecrow_a.cgf"), "{step0}");
    }

    #[test]
    #[ignore = "requires original catalog and prefab assets"]
    fn placed_instrument_visuals_keep_the_complete_prefab() {
        let db = Db::open().expect("read-only game catalog");
        for (template,group,name) in [(1236,1951,"piano_a"),(6200,16414,"piano_a"),(8220,22863,"piano_b"),(8065,22477,"percussion_a"),(8255,23046,"percussion_a")] {
            let reference = db.doodad_phase_visual(template,group).unwrap();
            assert_eq!(reference,format!("prefab://prefabs/interaction_c.xml/furniture.{name}"));
            assert_eq!(super::super::housing::prefab::load(&reference).len(),2);
            assert_eq!(db.doodad_phase_model(template,group),None,"former first-object_Model lookup cannot draw this prefab");
            assert_eq!(db.doodad_phase_visual(template,group).as_deref(),Some(reference.as_str()),"cached reference");
        }
        // Ordinary plants retain the established phase/fallback model paths.
        assert_eq!(db.doodad_phase_visual(2246,4403),db.doodad_phase_model(2246,4403));
        assert_eq!(db.doodad_phase_visual(2246,10155),db.doodad_phase_model(2246,10155));
    }

    /// The Lilyut horse (model 139) and the donkey (953) as the database has them.
    #[test]
    fn the_database_holds_the_mount_speeds() {
        let Some(db) = Db::open() else { return };
        assert_eq!(db.mount_move(139).run, 9.0);
        assert_eq!(db.mount_move(139).walk, 1.8);
        assert!((db.mount_move(953).run - 7.2).abs() < 1e-5);
    }

    /// The training dummies are prefab models: their model file is the `object_Model` of the prefab.
    #[test]
    fn a_portal_gate_prefab_is_its_particle_effects() {
        let e = prefab_effects_of("prefab://Prefabs/fx.xml/portal_gate_dot");
        let names: Vec<&str> = e.iter().map(|e| e.effect.as_str()).collect();
        assert_eq!(names, ["abillity_skill_table_m.system_skill.portal_gate_dot3_indivisual", "abillity_skill_table_m.system_skill.portal_gate_hit"]);
        assert!((e[0].pos.z - 0.036621094).abs() < 1e-6);
        assert!(!prefab_effects_of("prefab://Prefabs/fx.xml/portal_gate_exit_dot").is_empty());
    }

    /// `item_tools.item_id` -> `item_tools.asset_id` -> the `item_assets` row: the model path and the hand attachment offset.
    #[test]
    fn a_tool_item_resolves_through_item_tools_to_its_asset() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE item_tools(item_id INT, asset_id INT);
             CREATE TABLE item_assets(id INT PRIMARY KEY, path TEXT, attachment_offset_pos_x REAL, attachment_offset_pos_y REAL,
                 attachment_offset_pos_z REAL, attachment_offset_rot_x REAL, attachment_offset_rot_y REAL, attachment_offset_rot_z REAL);",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO item_assets VALUES (289, 'Objects\\Item\\Weapon\\Etc\\Shovel_General\\Shovel_General.cgf', -0.01, 0.16, 0, 10, 88.88, 0)",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO item_tools VALUES (2660, 289)", []).unwrap();
        let tool = tool_from_conn(&conn, 2660).expect("the shovel");
        assert_eq!(tool.path, "objects/item/weapon/etc/shovel_general/shovel_general.cgf");
        assert_eq!(tool.offset, [-0.01, 0.16, 0.0]);
        assert_eq!(tool.rotation, [10.0, 88.88, 0.0]);
        assert_eq!(tool_from_conn(&conn, 0), None, "an empty tool id holds nothing");
        assert_eq!(tool_from_conn(&conn, 1), None, "an item without a tool row holds nothing");
    }

    /// The real row of the uproot shovel: item 2660 -> `item_tools` 9 -> `item_assets` 289.
    #[test]
    fn the_real_tools_come_from_item_tools() {
        let Some(db) = Db::open() else { return };
        let shovel = tool_from_conn(&db.lookups.lock().unwrap().conn, 2660).expect("the shovel");
        assert_eq!(shovel.path, "objects/item/weapon/etc/shovel_general/shovel_general.cgf");
        assert_eq!(shovel.offset, [-0.01, 0.16, 0.0]);
        assert_eq!(shovel.rotation, [10.0, 88.88, 0.0]);
        let pot = tool_from_conn(&db.lookups.lock().unwrap().conn, 14511).expect("the watering pot");
        assert!(pot.path.ends_with("ndeco_quest_pot_a.cgf"), "{}", pot.path);
        // anim_actions 68 `fist_pos_stn_florist_idle` holds the flowerpot in its off hand
        let pose = db.anim_action_tools(68);
        assert_eq!(pose[0], None);
        assert!(pose[1].as_ref().is_some_and(|t| t.path.ends_with("ndeco_housing_flowerpot01.cgf")));
    }

    #[test]
    fn a_prefab_model_has_the_file_of_its_animated_object() {
        if !aa_cry::pak_available() {
            return;
        }
        assert_eq!(prefab_model_file("prefab://prefabs/interaction_e.xml/scarecrow.learning_scarecrow_02").as_deref(), Some("objects/env/01_nuia/001_ndeco/old/learning_ob01_e.chr"));
    }

    /// Model 1008 (`models.sub_type = 'VehicleModel'`, the Farm Wagon): `vehicle_models.normal`
    /// is a prefab whose animated body is `transfers_trailer_a_body.chr`.
    #[test]
    fn a_vehicle_model_resolves_to_its_prefab_body() {
        if !aa_cry::pak_available() {
            return;
        }
        let Some(db) = Db::open() else { return };
        let file = db.actor_model_file(1008).expect("the Farm Wagon body");
        assert!(file.ends_with("transfers_trailer_a_body.chr"), "{file}");
    }

    /// A transfer's prefab is its body brush plus the animated objects (the airship's wings and
    /// propellers): the single-first-model creature path would draw only the wing.
    #[test]
    fn a_transfer_prefab_is_its_body_brush_and_animated_parts() {
        if !aa_cry::pak_available() {
            return;
        }
        let Some(db) = Db::open() else { return };
        let parts = db.transfer_parts(657);
        assert!(parts.iter().any(|p| p.path.ends_with("airship_base.cgf")), "{parts:?}");
        assert!(parts.iter().any(|p| p.path.ends_with("airship_wing.chr")), "{parts:?}");
        assert!(parts.iter().all(|p| aa_pak::Pak::global().exists(&p.path)), "a part is missing from the pak");
        // `carriage_main` is a single body `.cgf`, not a prefab: it keeps the creature path (and its wheel bones)
        assert!(db.transfer_parts(654).is_empty());
    }

    #[test]
    fn a_prefab_part_list_keeps_brushes_and_anim_objects_at_their_places() {
        let prefab = r#"<Prefab Name="a.x"><Objects>
            <Object Type="Brush" Name="body" Pos="1,2,3" Prefab="Objects\Env\a.cgf"/>
            <Object Type="Entity" Name="wing" Pos="4,5,6" EntityClass="AnimObject"><Properties object_Model="game/Objects/Env/wing.chr"/></Object>
            <Object Type="Entity" Name="fx" EntityClass="ParticleEffect"><Properties ParticleEffect="x"/></Object>
        </Objects></Prefab>"#;
        let parts = parse_prefab_part_objects(prefab, true);
        assert_eq!(parts.len(), 2);
        assert_eq!((parts[0].name.as_str(), parts[0].pos), ("body", Vec3::new(1.0, 2.0, 3.0)));
        assert_eq!((parts[1].name.as_str(), parts[1].pos), ("wing", Vec3::new(4.0, 5.0, 6.0)));
        assert!(parts[1].path.ends_with("wing.chr"));
        assert_eq!(parse_prefab_part_objects(prefab, false).len(), 1, "ships keep their brushes only");
    }

    /// The cargo crates of the Farm Wagon hang on `$cannon0..3` (attach points 9..12): the helpers of the prefab.
    #[test]
    fn the_wagons_cargo_points_come_from_its_prefab() {
        if !aa_cry::pak_available() {
            return;
        }
        let Some(db) = Db::open() else { return };
        let p = db.slave_attach_offset(1008, 9).expect("$cannon0");
        assert!((p - Vec3::new(-0.6295166, -1.9423828, 1.1413727)).length() < 1e-4, "{p:?}");
        let p = db.slave_attach_offset(1008, 12).expect("$cannon3");
        assert!((p - Vec3::new(0.6338501, -1.9420166, 1.1414185)).length() < 1e-4, "{p:?}");
        assert_eq!(db.slave_attach_offset(1008, 200), None);
    }

    #[test]
    fn a_prefab_helper_position_is_read_from_the_named_prefab_only() {
        let xml = r#"<PrefabsLibrary><Prefab Name="a.x"><Objects><Object Type="Comment" Id="{1}" Name="cannon0" Pos="1,2,3" /></Objects></Prefab><Prefab Name="a.y"><Objects><Object Type="Comment" Id="{2}" Name="cannon0" Pos="4,5,6" /></Objects></Prefab></PrefabsLibrary>"#;
        assert_eq!(helper_pos_in(xml, "a.y", "cannon0"), Some(Vec3::new(4.0, 5.0, 6.0)));
        assert_eq!(helper_pos_in(xml, "a.x", "cannon0"), Some(Vec3::new(1.0, 2.0, 3.0)));
        assert_eq!(helper_pos_in(xml, "a.x", "cannon9"), None);
    }

    /// `vehicle_models.wheel` is a model path: the Farm Wagon sends one wheel speed per wheel bone (4), not the 2 the
    /// failed integer parse fell back to.
    #[test]
    fn the_wagon_has_four_wheel_entries() {
        if !aa_cry::pak_available() {
            return;
        }
        let Some(db) = Db::open() else { return };
        let body = db.vehicle_body(1008);
        assert_eq!(body.wheels, 4);
        assert!((body.speed - 4.0).abs() < 1e-6 && (body.turn - 0.4).abs() < 1e-6, "{body:?}");
        let wheels = db.vehicle_wheel_models(1008);
        assert!(wheels.wheel.as_deref().is_some_and(|p| p.ends_with("transfers_trailer_a_wheel01.cgf")), "{wheels:?}");
        assert!(wheels.wheel2.as_deref().is_some_and(|p| p.ends_with("transfers_trailer_a_wheel02.cgf")), "{wheels:?}");
    }

    /// Every `ShipModel` draws a hull: model 393 (the galleon / clipper line) resolves to a body file.
    #[test]
    fn a_ship_model_resolves_to_its_hull_body() {
        if !aa_cry::pak_available() {
            return;
        }
        let Some(db) = Db::open() else { return };
        let file = db.actor_model_file(393).expect("the galleon hull");
        assert!(file.ends_with("galleon_body.cgf"), "{file}");
        let parts = db.ship_parts(393);
        assert!(parts.len() > 5 && parts.iter().any(|p| p.path.ends_with("galleon_rudder.cgf")), "{} parts", parts.len());
        assert!(parts.iter().all(|p| aa_pak::Pak::global().exists(&p.path)), "a part is missing from the pak");
    }

    /// `FUN_3989f8b0`: >75% normal, >50% damaged25, >25% damaged50, else damaged75; zero is dead.
    #[test]
    fn a_ships_damage_state_follows_the_original_thresholds() {
        assert_eq!(ship_damage_state(100, 100), 1);
        assert_eq!(ship_damage_state(76, 100), 1);
        assert_eq!(ship_damage_state(75, 100), 2);
        assert_eq!(ship_damage_state(51, 100), 2);
        assert_eq!(ship_damage_state(50, 100), 3);
        assert_eq!(ship_damage_state(26, 100), 3);
        assert_eq!(ship_damage_state(25, 100), 4);
        assert_eq!(ship_damage_state(1, 100), 4);
        assert_eq!(ship_damage_state(0, 100), 6);
        assert_eq!(ship_damage_state(10, 0), 6);
        // integer division as in the original: 2/3 is 66%, not 66.6%
        assert_eq!(ship_damage_state(2, 3), 2);
    }

    /// A variant the hull does not ship walks down to the less damaged prefab (`FUN_397a7980`), so a missing
    /// `damaged25` still shows the hull.
    #[test]
    fn a_missing_damage_variant_falls_back_to_the_whole_hull() {
        let v = ShipVariants { normal: Some("normal".into()), ..default() };
        assert_eq!(v.path(2), Some("normal"));
        assert_eq!(v.path(6), Some("normal"));
        let v = ShipVariants { normal: Some("normal".into()), damaged50: Some("d50".into()), dead: Some("dead".into()), ..default() };
        assert_eq!(v.path(4), Some("d50"));
        assert_eq!(v.path(6), Some("dead"));
        assert_eq!(ShipVariants::default().path(6), None);
    }

    /// Equipment owns its attach points: an installed part shows, an empty slot shows nothing, and the
    /// authored binding only fills points no slot owns.
    #[test]
    fn a_ships_parts_follow_its_equipment_and_default_bindings() {
        let slots = [
            ShipEquipSlot { attach: 9, slot: 3 },
            ShipEquipSlot { attach: 10, slot: 4 },
            ShipEquipSlot { attach: 21, slot: 9 },
        ];
        let bindings = [
            ShipDoodadBinding { attach: 9, doodad: 101, scale: 1.0 },
            ShipDoodadBinding { attach: 21, doodad: 202, scale: 0.5 },
            ShipDoodadBinding { attach: 35, doodad: 303, scale: 1.0 },
        ];
        let mut equipped = Equipment::default();
        equipped.slots[4] = aa_net::model::Item::template_only(500);
        let out = resolve_ship_doodads(&slots, &bindings, &equipped, |item| (item == 500).then_some(777));
        assert_eq!(out, [ShipDoodad { attach: 10, doodad: 777, scale: 1.0 }, ShipDoodad { attach: 35, doodad: 303, scale: 1.0 }]);
        // the helm binding matches the initial helm item, so a hull wearing it never shows both
        let mut helm = Equipment::default();
        helm.slots[2] = aa_net::model::Item::template_only(29726);
        let bindings = [ShipDoodadBinding { attach: 35, doodad: 2374, scale: 1.0 }];
        let slots = [ShipEquipSlot { attach: 35, slot: 2 }];
        let out = resolve_ship_doodads(&slots, &bindings, &helm, |_| Some(2374));
        assert_eq!(out, [ShipDoodad { attach: 35, doodad: 2374, scale: 1.0 }]);
    }

    /// A doodad's own offsets turn and scale with its attach point (the cannon yaw of the cutter is ±90°).
    #[test]
    fn a_ship_doodad_keeps_its_attach_point_and_yaw() {
        let local = ShipPart { name: "brush".into(), path: "a/b.cgf".into(), pos: Vec3::new(1.0, 0.0, 0.0), rot: Quat::IDENTITY, scale: Vec3::ONE };
        let at = ShipAttach { pos: Vec3::new(2.0, 3.0, 4.0), yaw: std::f32::consts::FRAC_PI_2 };
        let out = place_ship_doodad(vec![local], at, 2.0);
        assert_eq!(out.len(), 1);
        assert!((out[0].pos - Vec3::new(2.0, 5.0, 4.0)).length() < 1e-5, "{:?}", out[0].pos);
        assert!((out[0].rot * Vec3::X - Vec3::Y).length() < 1e-5);
        assert_eq!(out[0].scale, Vec3::splat(2.0));
    }

    /// The server's `slave_attach_points.json` uses `//` notes and trailing commas; it must still parse.
    #[test]
    fn ship_attach_points_are_relaxed_json() {
        let text = r#"[{"Name":"x","ModelId":393,"AttachPoints":{"9":{ // Cannon0
            "X":1.0,"Y":2.0,"Z":3.0,"Yaw":90.0,
        },},},]"#;
        let json = relaxed_json(text);
        let value = serde_json::from_str::<serde_json::Value>(&json).expect("relaxed json");
        assert_eq!(value[0]["AttachPoints"]["9"]["Yaw"], 90.0);
    }

    /// The cutter's cannon point 9 is authored in the json (position and ±90° yaw), and the dead hull is
    /// `galleon_dead.cgf` with the original `caraval_water_destroy` water.
    #[test]
    fn the_cutter_has_attach_points_and_a_dead_hull() {
        if !aa_cry::pak_available() {
            return;
        }
        let Some(db) = Db::open() else { return };
        if let Some(at) = db.ship_attach_point(393, 9) {
            assert!((at.pos - Vec3::new(-4.29277, 2.22774, 4.42057)).length() < 1e-4, "{at:?}");
            assert!((at.yaw.to_degrees() - 90.0).abs() < 1e-3, "{at:?}");
        }
        let dead = db.ship_visual(21, 393, 6, &Equipment::default());
        assert!(dead.parts.iter().any(|p| p.path.ends_with("galleon_dead.cgf")), "{:?}", dead.parts.iter().map(|p| &p.path).collect::<Vec<_>>());
        assert!(dead.effects.iter().any(|e| e.effect.ends_with("caraval_water_destroy")), "{:?}", dead.effects);
        // the wreck is the dead prefab alone, not the live hull's masts, sails and cannon doodads
        assert_eq!(dead.parts.len(), 1, "{:?}", dead.parts.iter().map(|p| &p.path).collect::<Vec<_>>());
        let mut equipped = Equipment::default();
        equipped.slots[2] = aa_net::model::Item::template_only(29726);
        assert_eq!(db.ship_visual(75, 1205, 6, &equipped).parts.len(), 1);
        // the same helm on the live merchant hull is the item's doodad at `$helms`
        let live = db.ship_visual(75, 1205, 1, &equipped);
        let helm = live.parts.iter().find(|p| p.path.ends_with("galleon_helms.cgf")).expect("the equipped helm doodad");
        if let Some(at) = db.ship_attach_point(1205, 35) {
            assert!((helm.pos - at.pos).length() < 1e-4, "{helm:?} at {at:?}");
        }
        let bare = db.ship_visual(75, 1205, 1, &Equipment::default());
        assert!(!bare.parts.iter().any(|p| p.path.ends_with("galleon_helms.cgf")));
    }

    /// `Rotate="w,x,y,z"` and the cry -> Bevy conversion of a brush are the ones the world's dummies and effects use: a quarter turn
    /// about up turns the bow (+y) to the left (-x) in both spaces, and the position and scale axes follow `(x, z, -y)` / `(x, z, y)`.
    #[test]
    fn a_prefab_brush_transform_converts_like_the_rest_of_the_client() {
        let xml = r#"<Prefab Name="p"><Object Type="Brush" Name="b" Prefab="a/b.cgf" Pos="1,2,3" Rotate="0.70710678,0,0,0.70710678" Scale="1,2,3"/></Prefab>"#;
        let parts = parse_prefab_part_objects(xml, false);
        assert_eq!(parts.len(), 1);
        let part = &parts[0];
        let bow = part.rot * Vec3::Y;
        assert!((bow - Vec3::NEG_X).length() < 1e-5, "{bow:?}");
        let bevy_bow = aa_world::coords::cry_rot_to_bevy(part.rot) * aa_world::coords::cry_to_bevy(Vec3::Y);
        assert!((bevy_bow - aa_world::coords::cry_to_bevy(Vec3::NEG_X)).length() < 1e-5, "{bevy_bow:?}");
        assert_eq!(aa_world::coords::cry_to_bevy(part.pos), Vec3::new(1.0, 3.0, -2.0));
        assert_eq!(part.scale, Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn a_ship_floor_answers_the_highest_surface_under_the_limit() {
        // a main deck at z 2 (x -4..4, y -10..10) and a poop deck at z 6 (y 10..16), one wall (vertical) and a ceiling at 9
        let quad = |x0: f32, x1: f32, y0: f32, y1: f32, z: f32| [[Vec3::new(x0, y0, z), Vec3::new(x1, y0, z), Vec3::new(x1, y1, z)], [Vec3::new(x0, y0, z), Vec3::new(x1, y1, z), Vec3::new(x0, y1, z)]];
        let wall = [Vec3::new(4.0, -10.0, 2.0), Vec3::new(4.0, 10.0, 2.0), Vec3::new(4.0, 10.0, 5.0)];
        let floor = ShipFloor::new(quad(-4.0, 4.0, -10.0, 10.0, 2.0).into_iter().chain(quad(-4.0, 4.0, 10.0, 16.0, 6.0)).chain([wall]).chain(quad(-4.0, 4.0, -10.0, 10.0, 9.0)));
        assert_eq!(floor.height(0.0, 0.0, 3.0), Some(2.0), "the main deck under the player");
        assert_eq!(floor.height(0.0, 0.0, 20.0), Some(9.0), "the highest surface under a high limit");
        assert_eq!(floor.height(1.0, 12.5, 7.0), Some(6.0), "the poop deck");
        assert_eq!(floor.height(1.0, 12.5, 4.0), None, "a step up of more than the limit is a wall");
        assert_eq!(floor.height(6.0, 0.0, 9.0), None, "beside the hull there is no deck");
    }

    /// The galleon's walkable hull: the helm chair stands on a floor of the same height, the main deck is lower, and
    /// beside the beam there is none.
    #[test]
    fn the_cutters_hull_has_a_deck_under_the_helm_and_none_beside_it() {
        if !aa_cry::pak_available() {
            return;
        }
        let Some(db) = Db::open() else { return };
        let floor = db.ship_floor(393).expect("the galleon hull is readable");
        let chair = db.ship_parts(393).into_iter().find(|p| p.name.starts_with("ndeco_cookstore_chair")).expect("the helm chair brush");
        let at_helm = floor.height(chair.pos.x, chair.pos.y, chair.pos.z + 1.0).expect("a floor under the helm chair");
        assert!((at_helm - chair.pos.z).abs() < 1.0, "floor {at_helm} chair {}", chair.pos.z);
        assert!(floor.height(30.0, 0.0, 10.0).is_none());
    }

    /// The Eznan Cutter (slave 21, model 393): the authored driver seat sits on the drawn deck, next to the helm chair
    /// brush of the hull prefab, so a seated player is within 2 m of the deck and the hull root is not 24 m below it.
    #[test]
    fn the_cutter_seat_is_on_its_drawn_deck() {
        if !aa_cry::pak_available() {
            return;
        }
        let Some(db) = Db::open() else { return };
        let seat = db.seat(393, 1);
        let chair = db.ship_parts(393).into_iter().find(|p| p.name.starts_with("ndeco_cookstore_chair")).expect("the helm chair brush");
        assert!((seat.local[2] - chair.pos.z).abs() < 2.0, "seat {:?} chair {:?}", seat.local, chair.pos);
        assert!(Vec2::new(seat.local[0], seat.local[1]).distance(Vec2::new(chair.pos.x, chair.pos.y)) < 2.0, "seat {:?} chair {:?}", seat.local, chair.pos);
        // the deck the walking player stands on is the same height
        assert!((db.ship_body(393).deck - chair.pos.z).abs() < 2.0);
    }

    /// Leaving the driver seat of a ship with an authored seat puts the player on the hull's deck (`board_deck` takes a floor
    /// within 1.5 m under the feet): the small galleon (slave 22, model 511) once had the default seat 1.1 m above its keel
    /// and the player stood in the water.
    #[test]
    fn the_driver_seat_of_each_authored_ship_is_over_its_deck() {
        if !aa_cry::pak_available() {
            return;
        }
        let Some(db) = Db::open() else { return };
        for model in [393, 511, 523, 1249, 1046, 1482, 1546, 1205] {
            let Some(floor) = db.ship_floor(model) else { continue };
            let seat = db.seat(model, 1).local;
            let deck = floor.height(seat[0], seat[1], seat[2] + 1.5).unwrap_or(f32::NEG_INFINITY);
            assert!(seat[2] - deck <= 1.5 && deck - seat[2] <= 0.0 + 1.5, "model {model}: seat {seat:?} deck {deck}");
        }
    }

    /// The ship slave models draw a hull. Three have no prefab in the 1.2 pak, so the original draws nothing for them
    /// either: 512 (`galleon02.xml` is not shipped) and the sunken wrecks 1386 / 1402 (`boxship_old_normal_6f` / `_2f`
    /// are not in `boxship_old.xml`).
    #[test]
    fn every_ship_model_has_a_hull() {
        if !aa_cry::pak_available() {
            return;
        }
        let Some(db) = Db::open() else { return };
        let ids: Vec<u32> = {
            let l = db.lookups.lock().unwrap();
            let mut st = l.conn.prepare("SELECT id FROM models WHERE sub_type = 'ShipModel'").unwrap();
            st.query_map([], |r| r.get::<_, u32>(0)).unwrap().map(Result::unwrap).collect()
        };
        assert!(!ids.is_empty());
        let mut missing: Vec<u32> = ids.into_iter().filter(|id| db.actor_model_file(*id).is_none()).collect();
        missing.sort_unstable();
        assert_eq!(missing, [512, 1386, 1402], "ship models without a hull");
    }

    #[test]
    fn phase_model_references_become_pak_paths() {
        assert_eq!(model_file("cgf://Game/Objects/Env/X.cgf").as_deref(), Some("objects/env/x.cgf"));
        assert_eq!(model_file("vegetation://objects/env/05_nature/rice01.cgf").as_deref(), Some("objects/env/05_nature/rice01.cgf"));
        assert_eq!(model_file("cga://objects/env/door.cga").as_deref(), Some("objects/env/door.cga"));
        assert_eq!(model_file(" \tvegetation://Objects/Env/rice01.cgf ").as_deref(), Some("objects/env/rice01.cgf"));
        assert_eq!(model_file(""), None);
        assert_eq!(model_file("a://invalid"), None);
    }

    #[test]
    fn prefab_phase_model_is_the_model_of_its_prefab() {
        if !aa_cry::pak_available() {
            return;
        }
        let path = model_file("prefab://prefabs/interaction_e.xml/tools.lever_left").unwrap();
        assert!(path.ends_with(".cgf") || path.ends_with(".chr") || path.ends_with(".cga"), "{path}");
    }

    #[test]
    fn a_phase_change_grows_over_the_announced_time_left() {
        let g = DoodadGrowth { delay_ms: 10_000, start_scale: 0.6, end_scale: 1.0 };
        // the server shortened its delay by the growth rate: the new phase lasts 4,000 ms, not the database's 10,000
        let total = g.total_ms(4_000.0, false);
        assert_eq!(total, 4_000.0);
        assert!((g.scale_at(total, total) - 0.6).abs() < 1e-6);
        assert!((g.scale_at(2_000.0, total) - 0.8).abs() < 1e-6);
        // exactly when the server changes phase
        assert!((g.scale_at(0.0, total) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_doodad_first_seen_mid_phase_uses_the_longer_duration() {
        let g = DoodadGrowth { delay_ms: 10_000, start_scale: 0.6, end_scale: 1.0 };
        // first seen with 4,000 ms left: the database delay is the phase's whole duration, so it is already 60% grown
        let total = g.total_ms(4_000.0, true);
        assert_eq!(total, 10_000.0);
        assert!((g.scale_at(4_000.0, total) - 0.84).abs() < 1e-6);
        // first seen with more time than the database delay (the server slowed the phase): the announced time wins
        assert_eq!(g.total_ms(12_000.0, true), 12_000.0);
        // no time left is known: the database delay is the fallback, and the growth is over
        assert_eq!(g.total_ms(0.0, true), 10_000.0);
        assert!((g.scale_at(0.0, g.total_ms(0.0, false)) - 1.0).abs() < 1e-6);
        // no delay to fall back on: it is over
        assert_eq!(DoodadGrowth { delay_ms: 0, start_scale: 0.5, end_scale: 0.9 }.scale_at(0.0, 0.0), 0.9);
    }

    /// A housing prefab is every brush it lists, each at its own place: the finished Thatched Farmhouse (model 1082) is the
    /// wall, roof and floor prefabs, not the single first brush of the first element.
    #[test]
    fn a_house_model_is_every_brush_of_its_alive_state() {
        let Some(db) = Db::open() else { return };
        let parts = db.housing_parts(1082);
        let has = |name: &str| parts.iter().any(|p| p.path.ends_with(name));
        assert!(has("housing_s_wall01.cgf") && has("housing_s_roof03.cgf") && has("housing_s_floor03.cgf"), "{parts:?}");
        assert!(parts.len() >= 3);
        // the farm designs (scarecrow gardens) are prefabs too; a plain creature model is not a housing prefab
        assert!(!db.housing_parts(987).is_empty());
        assert!(!db.housing_parts(1253).is_empty());
        assert!(db.housing_parts(129).is_empty() && db.housing_parts(0).is_empty());
    }

    fn test_db() -> Option<Db> {
        let path = std::env::var("AA_DB").ok()?;
        std::path::Path::new(&path).exists().then(Db::open).flatten()
    }

    #[test]
    fn phases_and_growths_come_from_the_database() {
        let Some(db) = test_db() else { return };
        let rice = "objects/env/05_nature/00_common/common_plants_rice01.cgf";
        assert_eq!(db.doodad_phase_model(2246, 4403).as_deref(), Some(rice));
        assert_eq!(db.doodad_phase_model(367, 2632).as_deref(), Some("objects/env/05_nature/04_subpol/sub_tree_bigfir01_grow02.cgf"));
        // an empty phase model falls back to the almighty one
        assert_eq!(db.doodad_phase_model(2246, 10155).as_deref(), Some(rice));
        let g = db.doodad_growth(4403).expect("rice phase 4403 grows");
        assert_eq!((g.delay_ms, g.start_scale, g.end_scale), (9_288_000, 0.75, 1.0));
        assert!(db.doodad_growth(10155).is_none());
    }

    #[test]
    fn ship_rows_expose_the_authored_speed_and_seat() {
        let Some(db) = Db::open() else { return };
        let rowboat = db.ship_body(129);
        assert!(db.is_ship(129) && !db.is_land_vehicle(129));
        assert!((rowboat.speed - 6.4).abs() < 0.1, "{rowboat:?}");
        assert!(rowboat.half[0] > 0.0 && rowboat.half[1] > 0.0);
        let driver = db.seat(129, 1);
        assert!(driver.local[2] > 1.3 && driver.yaw.abs() > 3.0);
        assert!(db.seat(129, 2).local[2] < driver.local[2]);
        let merchant = db.ship_body(1205);
        assert!((merchant.speed - 13.0).abs() < 0.1, "{merchant:?}");
        assert!(merchant.deck > 1.5);
    }
}
