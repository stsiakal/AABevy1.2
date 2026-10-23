//! Ground targeting: the skills that are aimed at a spot of the ground (Meteor Strike, Missile Rain, thrown bombs, shovels).
//!
//! ## How the 1.2 data marks them
//!
//! `skills.target_type_id` is the server's `SkillTargetType` (the same numbers the client reads: `0` self, `4` hostile, `6`
//! position ...), `target_selection_id` its `SkillTargetSelection` (`1` source, `2` target, `4` location):
//!
//! | `target_type_id` | selection | what the client does here |
//! |---|---|---|
//! | `6` Pos, `11` BallisticPos, `15` ArtilleryPos, `13` RelativePos | any but 1 | **ground reticle**: the decal follows the cursor, a left click casts at that spot (`CSStartSkill`, position target) |
//! | `6` Pos | `1` source | **self position**: the area is centred on the caster, no reticle, cast at once with the caster's own position |
//! | `12` SummonPos | 4 | **water reticle** only when the used item summons a ship (`GameData::is_ship_summon`): the spot is the water surface; a mount summon keeps the plain unit cast |
//! | `0` self, `4`, `5` ... `14` SourcePos, `7` Line | | not ground targeted: the plain unit/self cast (`Command::UseSkill`); the server's `initial_target` takes the caster's spot for these |
//!
//! The reticle's size is `skills.target_area_radius` (`target_decal_radius` when a skill has one: seven rows do); the range is
//! `min_range ..= max_range` (3D distance from the caster's feet less the body, as the server's `distance_to(.., include_z)`).
//! The server also checks `target_angle` (the point must lie in front of the caster, `skill_outof_angle`), so the caster is turned
//! towards the spot with a standing movement packet right before the cast.
//!
//! The client's own strings (`x2game.dll`): `InitGroundTargetDecal`, the decal materials `materials/Decals/target/target_correct`
//! (green, the spot is good) and `target_null` (red, tinted copy of the same ring texture `target_correct_df.dds`), and the option
//! `OPTION_ITEM_SMART_GROUND_TARGETING` ("Skill Deployment Location": *activates skills at the location of the mouse cursor when
//! their shortcut key is pressed*; clicking the slot with the mouse still asks for a spot). The decal is the red ring while the spot
//! is out of range.
//!
//! ## Flow
//!
//! The HUD's slot use (`hud/panels.rs`) sends a [`SkillPressed`] instead of the command. [`route`] sends the plain command for a
//! skill without a spot, and for a ground skill it
//! * with the smart option on and the key used (the cursor is not over the HUD): casts at the cursor's spot at once (when the cursor
//!   is not over the ground, at the current target's spot),
//! * else starts aiming ([`Aiming`]): [`reticle`] draws the decal under the cursor, [`aim_input`] takes the left click (cast), the
//!   right click or `Escape` (cancel), and swallows the click so it does not also select the unit under the cursor.
//!
//! Dev flags (seconds count from entering the game): `--ground-press SKILL T` (as the key), `--aim X Y T` (the cursor, logical
//! window pixels), `--aim-world X Y T` (the cursor held on a ground point, game coordinates), `--click T`, `--ground-cancel T`, `--ground-smart 0|1` (the option), `--bar-skill SLOT SKILL` (puts a skill on
//! the action bar, for `--hud-key slotN`).

use std::collections::HashMap;

use aa_net::skill::target_kind;
use aa_net::state::{Target, Units};
use aa_net::{Command, Event, MoveCommand, NetCommand, NetEvent, Pos};
use aa_world::{TerrainQuery, WaterQuery, WorldCollision, bevy_to_cry, cry_to_bevy};
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::window::PrimaryWindow;

mod plant;

use super::player::{LocalPlayer, Mover};
use crate::Screen;
use crate::hud::{HudPointerOverUi, InFlight};

/// The ring texture of both decal materials.
const TEXTURE: &str = "textures/decal/target/target_correct_df.dds";
/// Fraction of the quad the ring's outer edge spans (the corner crosses reach out to 0.9).
const INK: f32 = 0.74;
/// Metres over the ground, and how far above the spot a vertex still finds the floor it follows.
const LIFT: f32 = 0.25;
const REACH_UP: f32 = 3.0;
/// A click that moves more than this many pixels between press and release is a camera drag.
const CLICK_SLOP: f32 = 6.0;
/// The body of the caster the range is measured from (the server subtracts the model size), and the slack the client allows over
/// the red ring before it refuses to send (`hud::usable::SEND_SLACK` is 2 m for units).
const BODY: f32 = 0.5;
const SEND_SLACK: f32 = 1.0;
/// The longest cursor ray, metres.
const REACH: f32 = 400.0;

/// How a skill uses a spot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The player picks the spot (reticle).
    Ground,
    /// The area is centred on the caster: the caster's own spot, at once.
    SelfPos,
    /// A ship summon (`target_type_id` 12, `SummonPos`): the spot must be on the water surface.
    Water,
}

/// The `skills` row of a ground skill.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundSkill {
    pub mode: Mode,
    /// Area radius of the decal (m).
    pub radius: f32,
    pub min_range: f32,
    pub max_range: f32,
}

/// Which mode a `(target_type_id, target_selection_id)` pair is, if any.
pub fn classify(target_type: i64, selection: i64) -> Option<Mode> {
    match (target_type, selection) {
        (6, 1) => Some(Mode::SelfPos),
        (6 | 11 | 13 | 15, _) => Some(Mode::Ground),
        // `SummonPos` is also used by mounts; only a ship item asks for the water reticle
        // (`SkillPressed::water`), so this mode alone does not force placement.
        (12, _) => Some(Mode::Water),
        _ => None,
    }
}

/// How far in front of the player a land vehicle is summoned, measured live: the original's Farm Wagon appeared about 7 m
/// away (`docs/research/re-wagons.md` 6).
pub const VEHICLE_SUMMON_DISTANCE: f32 = 7.0;

/// The items that summon a slave: `item template -> (slave template, a land vehicle)`.
#[derive(Resource, Default)]
pub struct SummonItems(pub HashMap<u32, (u32, bool)>);

fn load_summon_items() -> Option<HashMap<u32, (u32, bool)>> {
    let path = std::env::var("AA_DB").unwrap_or_else(|_| aa_anim::db::DEFAULT_DB.to_string());
    let conn = rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI).ok()?;
    let mut q = conn
        .prepare("SELECT iss.item_id, iss.slave_id, m.sub_type = 'VehicleModel' FROM item_summon_slaves iss JOIN slaves s ON s.id = iss.slave_id JOIN models m ON m.id = s.model_id")
        .ok()?;
    let rows = q.query_map([], |r| Ok((r.get::<_, u32>(0)?, (r.get::<_, u32>(1)?, r.get::<_, bool>(2)?)))).ok()?;
    Some(rows.flatten().collect())
}

/// The ground skills of the database (`skills` rows whose target type takes a position).
#[derive(Resource, Default)]
pub struct GroundSkills(pub HashMap<u32, GroundSkill>);

fn load_skills() -> Option<HashMap<u32, GroundSkill>> {
    use rusqlite::types::ValueRef;
    let path = std::env::var("AA_DB").unwrap_or_else(|_| aa_anim::db::DEFAULT_DB.to_string());
    let conn = rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI).ok()?;
    let mut q = conn
        .prepare("SELECT id, target_type_id, target_selection_id, target_area_radius, target_decal_radius, min_range, max_range FROM skills WHERE target_type_id IN (6, 11, 12, 13, 15)")
        .ok()?;
    let num = |r: &rusqlite::Row, i: usize| -> f32 {
        match r.get_ref(i) {
            Ok(ValueRef::Integer(v)) => v as f32,
            Ok(ValueRef::Real(v)) => v as f32,
            Ok(ValueRef::Text(t)) => std::str::from_utf8(t).ok().and_then(|s| s.parse().ok()).unwrap_or(0.0),
            _ => 0.0,
        }
    };
    let rows = q
        .query_map([], |r| {
            let id = num(r, 0) as u32;
            let (ty, sel) = (num(r, 1) as i64, num(r, 2) as i64);
            let (area, decal) = (num(r, 3), num(r, 4));
            Ok(classify(ty, sel).map(|mode| (id, GroundSkill { mode, radius: if decal > 0.0 { decal } else { area }, min_range: num(r, 5), max_range: num(r, 6) })))
        })
        .ok()?;
    Some(rows.flatten().flatten().collect())
}

/// The player used a skill (or an item with a skill) from the action bar or the bag. Sent by the HUD instead of the command; [`route`]
/// sends the command itself for what has no spot.
#[derive(Message, Clone, Copy, Debug)]
pub struct SkillPressed {
    pub skill: u32,
    /// `(item id, template id)` when an item's skill is used.
    pub item: Option<(u64, u32)>,
    /// The unit target of the plain command (0: none).
    pub target_obj_id: u32,
    /// The skill waits for the global cooldown: once sent, the next press of such a skill waits for the server's answer
    /// ([`InFlight`]).
    pub gcd: bool,
    /// The item summons a ship: place the `SummonPos` target on the water surface.
    pub water: bool,
    /// A slave casts the skill for the player (a manned ship cannon): `(slave object id, mount skill id)`. The cast
    /// is the mate-style `UseMateSkill(At)` and the seated gunner is neither turned nor stopped.
    pub caster: Option<(u32, u32)>,
}

impl SkillPressed {
    /// The command a skill without a spot sends.
    fn plain(&self) -> Command {
        if let Some((mate_obj_id, mount_skill_id)) = self.caster {
            return Command::UseMateSkill { skill_id: self.skill, mate_obj_id, mount_skill_id, target_obj_id: self.target_obj_id };
        }
        match self.item {
            Some((item_id, template_id)) => Command::UseItem { item_id, template_id, skill_id: self.skill, target_obj_id: self.target_obj_id },
            None => Command::UseSkill { skill_id: self.skill, target_obj_id: self.target_obj_id },
        }
    }

    fn at(&self, pos: Pos) -> Command {
        if let Some((mate_obj_id, mount_skill_id)) = self.caster {
            return Command::UseMateSkillAt { skill_id: self.skill, mate_obj_id, mount_skill_id, pos };
        }
        match self.item {
            Some((item_id, template_id)) => Command::UseItemAt { item_id, template_id, skill_id: self.skill, pos },
            None => Command::UseSkillAt { skill_id: self.skill, pos },
        }
    }
}

/// Where a land vehicle is summoned: [`VEHICLE_SUMMON_DISTANCE`] in front of `pos` (game coordinates) along `facing`.
pub fn vehicle_summon_spot(pos: Vec3, facing: f32) -> Vec3 {
    let ahead = crate::ingame::player::forward(facing) * VEHICLE_SUMMON_DISTANCE;
    Vec3::new(pos.x + ahead.x, pos.y + ahead.y, pos.z)
}

/// What the player is aiming.
struct Aim {
    press: SkillPressed,
    skill: GroundSkill,
    /// The spot under the cursor (game coordinates) and whether the caster is within range of it.
    spot: Option<(Vec3, bool)>,
    /// The item that places a doodad is aimed, not a skill ([`plant`]).
    plant: Option<plant::Plant>,
}

#[derive(Resource, Default)]
pub struct Aiming(Option<Aim>);

impl Aiming {
    /// A ground skill is being aimed: the HUD's use buttons (a doodad's bar) must not take the click.
    pub fn active(&self) -> bool {
        self.0.is_some()
    }
}

/// `--ground-press` and friends.
#[derive(Resource, Default)]
struct Script {
    press: Vec<(f32, u32)>,
    aim: Vec<(f32, Vec2)>,
    /// `--aim-world X Y T`: from T on the cursor is held on this ground point (game coordinates).
    aim_world: Vec<(f32, Vec2)>,
    held: Option<Vec2>,
    click: Vec<f32>,
    cancel: Vec<f32>,
    bar: Vec<(u8, u32)>,
    smart: Option<bool>,
    /// The scripted cursor, once `--aim` set it.
    cursor: Option<Vec2>,
    /// A scripted click / cancel waiting for [`aim_input`].
    clicked: bool,
    cancelled: bool,
    /// Seconds since the game screen came up.
    started: Option<f32>,
}

impl Script {
    fn from_args() -> Script {
        let args: Vec<String> = std::env::args().collect();
        let mut s = Script::default();
        let f = |i: usize| args.get(i).and_then(|v| v.parse::<f32>().ok());
        for (i, a) in args.iter().enumerate() {
            match a.as_str() {
                "--ground-press" => {
                    if let (Some(k), Some(t)) = (f(i + 1), f(i + 2)) {
                        s.press.push((t, k as u32));
                    }
                }
                "--aim" => {
                    if let (Some(x), Some(y), Some(t)) = (f(i + 1), f(i + 2), f(i + 3)) {
                        s.aim.push((t, Vec2::new(x, y)));
                    }
                }
                "--aim-world" => {
                    if let (Some(x), Some(y), Some(t)) = (f(i + 1), f(i + 2), f(i + 3)) {
                        s.aim_world.push((t, Vec2::new(x, y)));
                    }
                }
                "--click" if args.get(i + 1).is_some_and(|v| v.parse::<f32>().is_ok()) && args.get(i + 2).is_none_or(|v| v.starts_with("--")) => {
                    if let Some(t) = f(i + 1) {
                        s.click.push(t);
                    }
                }
                "--ground-cancel" => {
                    if let Some(t) = f(i + 1) {
                        s.cancel.push(t);
                    }
                }
                "--ground-smart" => s.smart = args.get(i + 1).map(|v| v != "0"),
                "--bar-skill" => {
                    if let (Some(slot), Some(k)) = (f(i + 1), f(i + 2)) {
                        s.bar.push((slot as u8, k as u32));
                    }
                }
                _ => {}
            }
        }
        s
    }
}

#[derive(Component)]
struct Reticle;

#[derive(Resource, Default)]
struct ReticleAssets {
    mesh: Option<Handle<Mesh>>,
    good: Option<Handle<StandardMaterial>>,
    bad: Option<Handle<StandardMaterial>>,
}

pub fn plugin(app: &mut App) {
    app.add_message::<SkillPressed>()
        .add_message::<crate::hud::PortalBookUsed>()
        .add_message::<crate::hud::MusicItemUsed>()
        .init_resource::<Aiming>()
        .init_resource::<ReticleAssets>()
        .init_resource::<InFlight>()
        .insert_resource(Script::from_args())
        .insert_resource(GroundSkills(load_skills().unwrap_or_default()))
        .insert_resource(SummonItems(load_summon_items().unwrap_or_default()))
        .add_systems(PreUpdate, aim_input.after(bevy::input::InputSystems).run_if(in_state(Screen::InGame)))
        .add_systems(Update, (dev_script, route, track_cursor, plant::validate, reticle, plant::ghost, log_server).chain().run_if(in_state(Screen::InGame)))
        .add_systems(OnExit(Screen::InGame), leave);
}

fn leave(mut commands: Commands, mut aiming: ResMut<Aiming>, reticles: Query<Entity, With<Reticle>>, mut script: ResMut<Script>) {
    aiming.0 = None;
    script.started = None;
    for e in &reticles {
        commands.entity(e).despawn();
    }
}

/// The cursor, in the window's logical pixels: the scripted one, else the mouse.
fn cursor_of(window: Option<&Window>, script: &Script, virtual_input: Option<&crate::ingame::virtual_input::VirtualInput>) -> Option<Vec2> {
    script
        .cursor
        .or_else(|| virtual_input.and_then(|v| v.cursor_position(window)))
        .or_else(|| window.and_then(|w| w.cursor_position()))
}

/// The first spot of the ground (or of an object) under the view ray through `cursor`, in game coordinates.
pub(crate) fn spot_under(camera: &Camera, transform: &GlobalTransform, cursor: Vec2, collision: Option<&WorldCollision>, terrain: Option<&TerrainQuery>) -> Option<Vec3> {
    let ray = camera.viewport_to_world(transform, cursor).ok()?;
    let origin = bevy_to_cry(ray.origin);
    let dir = bevy_to_cry(*ray.direction);
    if let Some(hit) = collision.and_then(|c| c.raycast(origin, dir, REACH)) {
        return Some(hit.point);
    }
    // no brushes loaded: march over the terrain heights
    let terrain = terrain?;
    let (mut prev, mut t) = (0.0f32, 0.0f32);
    while t < REACH {
        t += 2.0;
        let p = origin + dir * t;
        let Some(h) = terrain.height_at(p.x, p.y) else { continue };
        if p.z <= h {
            let (mut lo, mut hi) = (prev, t);
            for _ in 0..12 {
                let mid = 0.5 * (lo + hi);
                let q = origin + dir * mid;
                if terrain.height_at(q.x, q.y).is_some_and(|g| q.z <= g) {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            let q = origin + dir * hi;
            return Some(Vec3::new(q.x, q.y, terrain.height_at(q.x, q.y).unwrap_or(q.z)));
        }
        prev = t;
    }
    None
}

/// Where the ray `origin + dir * t` (game coordinates, `dir` unit) first goes under the water surface within `reach`
/// metres: `surface` gives the surface height over an `(x, y, z)` point, `None` where there is no water. The point is
/// on the surface. A ship summon aims at this, not at the sea bed under it (open sea has no ground within reach).
fn surface_along_ray(origin: Vec3, dir: Vec3, reach: f32, surface: impl Fn(Vec3) -> Option<f32>) -> Option<Vec3> {
    const STEP: f32 = 1.0;
    // the height of the ray above the surface, `None` over dry land
    let above = |t: f32| {
        let p = origin + dir * t;
        surface(p).map(|h| p.z - h)
    };
    let mut prev = (0.0f32, above(0.0));
    let mut t = 0.0;
    while t < reach {
        t = (t + STEP).min(reach);
        let now = above(t);
        match (prev.1, now) {
            // the ray was over the water and is now under its surface (or the camera starts under it)
            (Some(a), Some(b)) if a > 0.0 && b <= 0.0 => {
                let (mut lo, mut hi) = (prev.0, t);
                for _ in 0..16 {
                    let mid = 0.5 * (lo + hi);
                    if above(mid).is_some_and(|d| d <= 0.0) {
                        hi = mid;
                    } else {
                        lo = mid;
                    }
                }
                let q = origin + dir * hi;
                return Some(Vec3::new(q.x, q.y, q.z - above(hi)?));
            }
            _ => {}
        }
        prev = (t, now);
    }
    None
}

/// The spot a ship summon aims at: the water surface the view ray through `cursor` crosses, unless the ground (a shore, a
/// dock) is hit first, then the water surface over that ground.
fn water_spot_under(camera: &Camera, transform: &GlobalTransform, cursor: Vec2, ground: Option<Vec3>, water: Option<&WaterQuery>) -> Option<Vec3> {
    let water = water?;
    let ray = camera.viewport_to_world(transform, cursor).ok()?;
    let (origin, dir) = (bevy_to_cry(ray.origin), bevy_to_cry(*ray.direction));
    let reach = ground.map_or(REACH, |g| origin.distance(g));
    let crossing = surface_along_ray(origin, dir, reach, |p| water.water_at(p).map(|(h, _)| h));
    crossing.or_else(|| water_spot(ground, Some(water)))
}

/// The water surface over the point under the cursor (a ship summon is placed on the surface,
/// not on the sea bed the ground ray hits).
fn water_spot(ground: Option<Vec3>, water: Option<&WaterQuery>) -> Option<Vec3> {
    let spot = ground?;
    // Ask a metre above the hit so a shallow sea bed just below a volume's floor still answers.
    let (surface, _) = water?.water_at(Vec3::new(spot.x, spot.y, spot.z + 1.0))?;
    Some(Vec3::new(spot.x, spot.y, surface))
}

/// Whether `spot` is a valid target for `skill`: in range and, for a ship summon, on water.
fn spot_ok(skill: &GroundSkill, from: Vec3, spot: Vec3, slack: f32, water: Option<&WaterQuery>) -> bool {
    if skill.mode == Mode::Water && water.is_none_or(|w| w.water_at(spot).is_none()) {
        return false;
    }
    in_range(skill, from, spot, slack)
}

/// Whether the caster at `from` reaches `spot`: the server's 3D distance less the body must lie within `min..=max` (`slack` over).
fn in_range(skill: &GroundSkill, from: Vec3, spot: Vec3, slack: f32) -> bool {
    let d = (from.distance(spot) - BODY).max(0.0);
    d >= skill.min_range && (skill.max_range <= 0.0 || d <= skill.max_range + slack)
}

/// The skill's yaw to face `spot` from `from` (0 north, counter-clockwise).
fn yaw_to(from: Vec3, spot: Vec3) -> Option<f32> {
    let d = spot - from;
    (d.truncate().length() > 0.05).then(|| (-d.x).atan2(d.y))
}

/// A skill that waits for the global cooldown went out: the HUD holds back the next one until the server has answered.
fn sent(flight: &mut InFlight, press: &SkillPressed) {
    if press.gcd {
        flight.sent(std::time::Instant::now());
    }
}

/// Turns the caster to the spot (a standing movement packet carries the facing: the server checks `target_angle` against it)
/// and asks for the skill there.
fn cast_at(press: &SkillPressed, spot: Vec3, mover: &mut Mover, net: &mut MessageWriter<NetCommand>, flight: &mut InFlight) {
    let me = mover.pos;
    // a seated gunner aims the cannon the server turns: no facing change and no standing movement packet
    if press.caster.is_none() {
        if let Some(yaw) = yaw_to(mover.pos, spot) {
            mover.facing = yaw;
        }
        net.write(NetCommand(Command::Move(MoveCommand::stopped(Pos::new(me.x, me.y, me.z), mover.facing))));
    }
    net.write(NetCommand(press.at(Pos::new(spot.x, spot.y, spot.z))));
    sent(flight, press);
    info!("ground: cast skill {} at ({:.1}, {:.1}, {:.1}), {:.1} m away, facing {:.2}", press.skill, spot.x, spot.y, spot.z, me.distance(spot), mover.facing);
}

/// Dev flags, on the clock of the game screen.
fn dev_script(
    time: Res<Time>,
    mut script: ResMut<Script>,
    mut pressed: MessageWriter<SkillPressed>,
    mut net: MessageWriter<NetCommand>,
    player: Query<(), With<LocalPlayer>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    terrain: Option<Res<TerrainQuery>>,
) {
    if player.is_empty() {
        return;
    }
    let now = time.elapsed_secs();
    let start = *script.started.get_or_insert(now);
    let t = now - start;
    if !script.bar.is_empty() {
        for (slot, skill) in std::mem::take(&mut script.bar) {
            net.write(NetCommand(Command::SetActionSlot { slot, action: aa_net::items::ActionSlot { kind: aa_net::items::ActionKind::Spell, id: u64::from(skill) } }));
        }
    }
    while let Some(i) = script.press.iter().position(|(at, _)| *at <= t) {
        let (_, skill) = script.press.remove(i);
        info!("ground: scripted press of skill {skill} at {t:.1} s");
        pressed.write(SkillPressed { skill, item: None, target_obj_id: 0, gcd: false, water: false, caster: None });
    }
    while let Some(i) = script.aim.iter().position(|(at, _)| *at <= t) {
        let (_, p) = script.aim.remove(i);
        script.cursor = Some(p);
        script.held = None;
    }
    while let Some(i) = script.aim_world.iter().position(|(at, _)| *at <= t) {
        let (_, p) = script.aim_world.remove(i);
        script.held = Some(p);
    }
    // the cursor follows a held ground point through the camera
    if let (Some(p), Some((camera, transform))) = (script.held, cameras.iter().find(|(c, _)| c.is_active)) {
        let z = terrain.as_ref().and_then(|t| t.height_at(p.x, p.y)).unwrap_or(0.0);
        script.cursor = camera.world_to_viewport(transform, cry_to_bevy(Vec3::new(p.x, p.y, z))).ok();
    }
    while let Some(i) = script.click.iter().position(|at| *at <= t) {
        script.click.remove(i);
        script.clicked = true;
    }
    while let Some(i) = script.cancel.iter().position(|at| *at <= t) {
        script.cancel.remove(i);
        script.cancelled = true;
    }
}

/// The smart ground targeting option (`OPTION_ITEM_SMART_GROUND_TARGETING`).
fn smart_option(rt: Option<&aa_ui::UiRuntime>, script: &Script) -> bool {
    script.smart.unwrap_or_else(|| {
        rt.and_then(|rt| rt.lua.load("return X2Option:GetOptionItemValue(OPTION_ITEM_SMART_GROUND_TARGETING)").eval::<Option<f32>>().ok().flatten()).is_some_and(|v| v != 0.0)
    })
}

/// The slot use of the HUD: plain skills go out as before, ground skills aim or cast at the cursor.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn route(
    mut pressed: MessageReader<SkillPressed>,
    skills: Res<GroundSkills>,
    mut aiming: ResMut<Aiming>,
    over_ui: Res<HudPointerOverUi>,
    script: Res<Script>,
    rt: Option<NonSend<aa_ui::UiRuntime>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    collision: Option<Res<WorldCollision>>,
    (virtual_input, terrain, water, summons, slaves): (
        Option<Res<crate::ingame::virtual_input::VirtualInput>>,
        Option<Res<TerrainQuery>>,
        Option<Res<WaterQuery>>,
        Option<Res<SummonItems>>,
        Option<Res<aa_net::state::Slaves>>,
    ),
    target: Res<Target>,
    units: Res<Units>,
    mut player: Query<&mut Mover, With<LocalPlayer>>,
    mut net: MessageWriter<NetCommand>,
    mut flight: ResMut<InFlight>,
    (keys, mut book, mut music, emotes, db): (Res<ButtonInput<KeyCode>>, MessageWriter<crate::hud::PortalBookUsed>, MessageWriter<crate::hud::MusicItemUsed>, Option<Res<super::emote::Emotes>>, Option<Res<super::data::Db>>),
) {
    for press in pressed.read() {
        // Native394eaac0 opens paper locally or looks up a stored score; neither casts here.
        if let Some((item_id, 27902 | 28051)) = press.item {
            music.write(crate::hud::MusicItemUsed(item_id));
            continue;
        }
        // the emote skills of the skill book (`/sad`, `/clap` ...) are expressed emotions: the server announces them to everyone in range
        if press.item.is_none()
            && let Some(emotion_id) = emotes.as_ref().and_then(|e| e.skill(press.skill))
        {
            net.write(NetCommand(Command::ExpressEmotion { emotion_id, target_obj_id: press.target_obj_id }));
            continue;
        }
        // the Teleport Book opens the book; its skill is cast from there (`X2Warp:OpenPortal`)
        if press.item.is_some() && press.skill == crate::hud::OPEN_PORTAL_SKILL {
            let record = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
            book.write(crate::hud::PortalBookUsed { item: press.item, record });
            continue;
        }
        // a land vehicle's summon item: used again while the vehicle is out it recalls it (`CSDespawnSlave`), else it is placed in
        // front of the player (both measured live on the original)
        if let Some((_, template)) = press.item
            && let Some(&(slave_template, true)) = summons.as_ref().and_then(|s| s.0.get(&template))
        {
            let own = slaves.as_ref().and_then(|s| s.own.as_ref()).filter(|o| units.get(o.obj_id).is_some_and(|u| matches!(u.kind, aa_net::model::UnitKind::Slave { template_id, .. } if template_id == slave_template)));
            if let Some(own) = own {
                net.write(NetCommand(Command::Slave(aa_net::slave_control::SlaveCommand::Despawn { obj_id: own.obj_id })));
                info!("ground: summon item {template} used while its vehicle {} is out: CSDespawnSlave", own.obj_id);
                continue;
            }
            if let Ok(mover) = player.single() {
                let spot = vehicle_summon_spot(mover.pos, mover.facing);
                net.write(NetCommand(press.at(Pos::new(spot.x, spot.y, spot.z))));
                sent(&mut flight, press);
                info!("ground: land vehicle summon placed {:.1} m ahead at ({:.1}, {:.1}, {:.1})", VEHICLE_SUMMON_DISTANCE, spot.x, spot.y, spot.z);
                continue;
            }
        }
        // a house design enters the builder (`housing::placement`), it is not cast
        if press.item.is_some_and(|(_, template)| super::housing::claims_item(template)) {
            continue;
        }
        // seeds, saplings and the other items that place a doodad are aimed at the ground (`plant.rs`); the same item again lets go
        if let Some((skill, plant)) = press.item.and(db.as_deref()).and_then(|db| plant::begin(press, db)) {
            let again = aiming.0.as_ref().is_some_and(|a| a.press.item == press.item);
            aiming.0 = (!again).then_some(Aim { press: *press, skill, spot: None, plant: Some(plant) });
            continue;
        }
        let Some(skill) = skills.0.get(&press.skill).copied() else {
            net.write(NetCommand(press.plain()));
            sent(&mut flight, press);
            continue;
        };
        let Ok(mut mover) = player.single_mut() else { continue };
        if skill.mode == Mode::SelfPos {
            let me = mover.pos;
            net.write(NetCommand(press.at(Pos::new(me.x, me.y, me.z))));
            sent(&mut flight, press);
            info!("ground: skill {} is centred on the caster: cast at ({:.1}, {:.1}, {:.1})", press.skill, me.x, me.y, me.z);
            continue;
        }
        let cursor = cursor_of(windows.single().ok(), &script, virtual_input.as_deref());
        let spot_at_cursor = || {
            let (camera, transform) = cameras.iter().find(|(c, _)| c.is_active)?;
            spot_under(camera, transform, cursor?, collision.as_deref(), terrain.as_deref())
        };
        // A `SummonPos` skill is placed only when the used item summons a ship; a mount summon
        // keeps the plain owner-relative cast.
        if skill.mode == Mode::Water && !press.water {
            net.write(NetCommand(press.plain()));
            continue;
        }
        // the same key again while aiming: cast where the cursor is
        let again = aiming.0.as_ref().is_some_and(|a| a.press.skill == press.skill && a.press.item == press.item);
        let key = !over_ui.0;
        if key && (again || smart_option(rt.as_deref(), &script)) {
            let ground = spot_at_cursor();
            let spot = if skill.mode == Mode::Water {
                let aimed = cameras.iter().find(|(c, _)| c.is_active).zip(cursor).and_then(|((camera, transform), cursor)| water_spot_under(camera, transform, cursor, ground, water.as_deref()));
                aimed.or_else(|| water_spot(ground, water.as_deref()))
            } else {
                ground
            }
            .or_else(|| target.obj_id.and_then(|id| units.get(id)).map(|u| Vec3::new(u.pos.x, u.pos.y, u.pos.z)));
            if let Some(spot) = spot {
                if spot_ok(&skill, mover.pos, spot, SEND_SLACK, water.as_deref()) {
                    cast_at(press, spot, &mut mover, &mut net, &mut flight);
                } else if skill.mode == Mode::Water {
                    info!("ground: ship summon at ({:.1}, {:.1}, {:.1}) is not on water or is out of range: not sent", spot.x, spot.y, spot.z);
                } else {
                    info!("ground: skill {} is out of range of ({:.1}, {:.1}, {:.1}): not sent", press.skill, spot.x, spot.y, spot.z);
                }
                aiming.0 = None;
                continue;
            }
        }
        if again {
            aiming.0 = None;
            continue;
        }
        info!("ground: aiming skill {} (radius {:.1} m, range {:.0}..{:.0} m)", press.skill, skill.radius, skill.min_range, skill.max_range);
        aiming.0 = Some(Aim { press: *press, skill, spot: None, plant: None });
    }
}

/// Keeps the spot under the cursor and whether it is in range.
#[allow(clippy::too_many_arguments)]
fn track_cursor(
    mut aiming: ResMut<Aiming>,
    over_ui: Res<HudPointerOverUi>,
    script: Res<Script>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    collision: Option<Res<WorldCollision>>,
    terrain: Option<Res<TerrainQuery>>,
    virtual_input: Option<Res<crate::ingame::virtual_input::VirtualInput>>,
    water: Option<Res<WaterQuery>>,
    player: Query<&Mover, With<LocalPlayer>>,
    mut last: Local<Option<Vec3>>,
) {
    let Some(aim) = aiming.0.as_mut() else {
        *last = None;
        return;
    };
    let (Ok(mover), Some((camera, transform))) = (player.single(), cameras.iter().find(|(c, _)| c.is_active)) else { return };
    let cursor = if over_ui.0 && script.cursor.is_none() { None } else { cursor_of(windows.single().ok(), &script, virtual_input.as_deref()) };
    let ground = cursor.and_then(|c| spot_under(camera, transform, c, collision.as_deref(), terrain.as_deref()));
    let spot = if aim.skill.mode == Mode::Water {
        cursor.and_then(|c| water_spot_under(camera, transform, c, ground, water.as_deref())).or_else(|| water_spot(ground, water.as_deref()))
    } else {
        ground
    };
    aim.spot = spot.map(|spot| (spot, spot_ok(&aim.skill, mover.pos, spot, 0.0, water.as_deref())));
    if let Some((spot, ok)) = aim.spot
        && last.is_none_or(|l| l.distance(spot) > 0.5)
    {
        *last = Some(spot);
        info!(
            "ground: cursor {:?}: spot ({:.1}, {:.1}, {:.1}), {:.1} m from the caster at ({:.1}, {:.1}, {:.1}): {}",
            cursor,
            spot.x,
            spot.y,
            spot.z,
            mover.pos.distance(spot),
            mover.pos.x,
            mover.pos.y,
            mover.pos.z,
            if ok { "in range" } else { "out of range" }
        );
    }
}

/// The left click (not a drag, not on the HUD) casts at the spot, the right click and `Escape` cancel. The click is taken from
/// the other systems (it must not select the unit under the cursor), and so is `Escape` (it must not drop the target).
#[allow(clippy::too_many_arguments)]
fn aim_input(
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    over_ui: Res<HudPointerOverUi>,
    mut script: ResMut<Script>,
    mut aiming: ResMut<Aiming>,
    windows: Query<&Window, With<PrimaryWindow>>,
    virtual_input: Option<Res<crate::ingame::virtual_input::VirtualInput>>,
    mut player: Query<&mut Mover, With<LocalPlayer>>,
    mut net: MessageWriter<NetCommand>,
    mut flight: ResMut<InFlight>,
    water: Option<Res<WaterQuery>>,
    mut pressed_at: Local<[Option<Vec2>; 2]>,
) {
    let (clicked, cancelled) = (std::mem::take(&mut script.clicked), std::mem::take(&mut script.cancelled));
    let Some(aim) = aiming.0.as_ref() else {
        *pressed_at = [None; 2];
        return;
    };
    let cursor = cursor_of(windows.single().ok(), &script, virtual_input.as_deref());
    let mut cast = clicked;
    let mut cancel = cancelled;
    if keys.just_pressed(KeyCode::Escape) {
        keys.clear_just_pressed(KeyCode::Escape);
        cancel = true;
    }
    for (i, button) in [MouseButton::Left, MouseButton::Right].into_iter().enumerate() {
        if mouse.just_pressed(button) {
            pressed_at[i] = if over_ui.0 { None } else { cursor };
            if i == 0 && !over_ui.0 {
                mouse.clear_just_pressed(button);
            }
        }
        if mouse.just_released(button) {
            let start = pressed_at[i].take();
            if let (Some(start), Some(end)) = (start, cursor)
                && start.distance(end) <= CLICK_SLOP
                && !over_ui.0
            {
                if i == 0 { cast = true } else { cancel = true }
            }
            if i == 0 && start.is_some() {
                mouse.clear_just_released(button);
            }
        }
    }
    if cancel {
        info!("ground: aiming skill {} cancelled", aim.press.skill);
        aiming.0 = None;
        return;
    }
    if !cast {
        return;
    }
    let Ok(mut mover) = player.single_mut() else { return };
    if let Some(plant) = aim.plant.as_ref() {
        // an item that places a doodad: the packet instead of a cast, the aiming goes on while the spot is not free
        match aim.spot {
            Some((spot, true)) if plant::place(plant, spot, &mover, &mut net) => aiming.0 = None,
            Some(_) => info!("plant: that spot cannot be used"),
            None => info!("plant: no ground under the cursor"),
        }
        return;
    }
    match aim.spot {
        Some((spot, ok)) if ok || spot_ok(&aim.skill, mover.pos, spot, SEND_SLACK, water.as_deref()) => {
            let press = aim.press;
            cast_at(&press, spot, &mut mover, &mut net, &mut flight);
            aiming.0 = None;
        }
        Some((spot, _)) => info!("ground: ({:.1}, {:.1}, {:.1}) is out of range of skill {}: not cast, still aiming", spot.x, spot.y, spot.z, aim.press.skill),
        None => info!("ground: no ground under the cursor: not cast"),
    }
}

fn dds_image(path: &str) -> Option<Image> {
    let bytes = aa_cry::read(path).ok()?;
    let dds = aa_cry::texture::Dds::parse(&bytes).ok()?;
    let (w, h) = (dds.width, dds.height);
    let mut image = Image::new(Extent3d { width: w, height: h, depth_or_array_layers: 1 }, TextureDimension::D2, dds.decode_rgba8(0, 0), TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::RENDER_WORLD);
    image.sampler = bevy::image::ImageSampler::linear();
    Some(image)
}

/// A decal material: the ring texture tinted, unlit, blended, drawn from both sides and pulled towards the camera a little so the
/// terrain under it (a lift of centimetres) cannot cut it.
fn decal_material(image: &Handle<Image>, color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        base_color_texture: Some(image.clone()),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        depth_bias: 12.0,
        ..default()
    }
}

/// The decal under the cursor: the ring of `target_correct` while the spot is in range, of `target_null` out of it. A card of the
/// 3D world (`aa_fx::decal::drape_quad`, like the area skills' ground decals): cells whose corners lie on the floor under them (brushes and
/// terrain, [`WorldCollision::ground_below`]) and on the water surface where there is water, lifted a few centimetres, depth tested
/// and drawn by the world camera, so it needs no HUD camera and hides behind what stands in front of it.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn reticle(
    mut commands: Commands,
    aiming: Res<Aiming>,
    mut assets: ResMut<ReticleAssets>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    collision: Option<Res<WorldCollision>>,
    terrain: Option<Res<TerrainQuery>>,
    water: Option<Res<WaterQuery>>,
    mut reticles: Query<(Entity, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>), With<Reticle>>,
) {
    let shown = aiming.0.as_ref().and_then(|a| a.spot.map(|(spot, ok)| (a.skill, spot, ok)));
    let Some((skill, spot, ok)) = shown else {
        for (_, mut v, _) in &mut reticles {
            if *v != Visibility::Hidden {
                *v = Visibility::Hidden;
            }
        }
        return;
    };
    if assets.good.is_none() {
        let Some(image) = dds_image(TEXTURE) else {
            warn!("ground: cannot read {TEXTURE}: no reticle");
            return;
        };
        let image = images.add(image);
        assets.mesh = Some(meshes.add(Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD)));
        // the decal materials' tints: target_correct Diffuse (0.18, 1, 0.44), target_null (0.74, 0.03, 0.01); the grey ring is lifted
        assets.good = Some(materials.add(decal_material(&image, Color::linear_rgba(0.4, 3.6, 1.4, 1.0))));
        assets.bad = Some(materials.add(decal_material(&image, Color::linear_rgba(3.6, 0.15, 0.05, 1.0))));
    }
    let material = if ok { assets.good.clone().unwrap() } else { assets.bad.clone().unwrap() };
    // the ring's outer edge is the area radius: the card is wider by the ink fraction
    let half = skill.radius.max(0.75) / INK;
    // the floor under a Bevy position: a brush or the terrain, the water surface over it
    let floor = |p: Vec3| -> Option<f32> {
        let (x, y) = (p.x, -p.z);
        let ground = collision
            .as_ref()
            .and_then(|c| c.ground_below(x, y, spot.z + REACH_UP, 0.0))
            .map(|g| g.height)
            .or_else(|| terrain.as_ref().and_then(|t| t.height_at(x, y)))
            .unwrap_or(spot.z)
            .clamp(spot.z - 2.0 * REACH_UP, spot.z + REACH_UP);
        let wet = water.as_ref().and_then(|w| w.water_at(Vec3::new(x, y, ground)).map(|(h, _)| h)).filter(|h| *h > ground);
        Some(wet.unwrap_or(ground))
    };
    // texture up is game north (-z in Bevy)
    let card = aa_fx::decal::drape_quad(cry_to_bevy(spot), Vec3::X * half, Vec3::NEG_Z * half, &floor, LIFT, aa_fx::decal::cells_for(half, half));
    let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; card.positions.len()])
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, card.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, card.uvs)
        .with_inserted_indices(Indices::U32(card.indices));
    if let Some(mut m) = meshes.get_mut(assets.mesh.as_ref().unwrap()) {
        *m = mesh;
    }
    match reticles.iter_mut().next() {
        Some((_, mut v, mut m)) => {
            if *v != Visibility::Inherited {
                *v = Visibility::Inherited;
            }
            if m.0 != material {
                m.0 = material;
            }
        }
        None => {
            commands.spawn((
                Name::new("ground target reticle"),
                Reticle,
                Mesh3d(assets.mesh.clone().unwrap()),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                Visibility::Inherited,
                // the card follows the cursor: its bounds change every frame
                NoFrustumCulling,
                NotShadowCaster,
                NotShadowReceiver,
            ));
        }
    }
}

/// What the server made of a spot: its `SkillStarted` / `SkillFired` carry the position target back.
fn log_server(mut events: MessageReader<NetEvent>, skills: Res<GroundSkills>) {
    for NetEvent(event) in events.read() {
        match event {
            Event::SkillStarted(s) if skills.0.contains_key(&s.skill_id) => {
                info!(
                    "ground: server: skill {} {} (result {}), target kind {} at {:?}, cast {} ms",
                    s.skill_id,
                    if s.failed() { "refused" } else { "started" },
                    s.result,
                    s.target.kind,
                    s.target.pos.map(|p| (p.x, p.y, p.z)),
                    s.cast_ms
                );
            }
            Event::SkillFired(s) if skills.0.contains_key(&s.skill_id) && s.target.kind != target_kind::UNIT => {
                info!("ground: server: skill {} fired at {:?}, delay {} ms", s.skill_id, s.target.pos.map(|p| (p.x, p.y, p.z)), s.delay_ms);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_types_pick_the_mode() {
        assert_eq!(classify(6, 2), Some(Mode::Ground), "Meteor Strike, Missile Rain");
        assert_eq!(classify(6, 4), Some(Mode::Ground), "Plant Seed, Dig");
        assert_eq!(classify(11, 2), Some(Mode::Ground));
        assert_eq!(classify(15, 2), Some(Mode::Ground));
        assert_eq!(classify(6, 1), Some(Mode::SelfPos), "centred on the caster");
        assert_eq!(classify(12, 4), Some(Mode::Water), "ship summon");
        for plain in [(0, 1), (4, 2), (5, 2), (7, 4), (14, 2), (16, 2)] {
            assert_eq!(classify(plain.0, plain.1), None, "{plain:?}");
        }
    }

    /// Open sea: the cursor ray crosses the water plane within range where no sea bed is within reach.
    #[test]
    fn a_ray_over_open_sea_lands_on_the_water_plane() {
        let sea = |_: Vec3| Some(100.0);
        // the camera 4 m over the sea looking 30 degrees down: the plane is 4 / tan(30) = 6.9 m ahead
        let dir = Vec3::new(0.0, 30f32.to_radians().cos(), -30f32.to_radians().sin());
        let spot = surface_along_ray(Vec3::new(10.0, 20.0, 104.0), dir, 400.0, sea).expect("the sea plane");
        assert!((spot.z - 100.0).abs() < 1e-3, "{spot:?}");
        assert!((spot.y - (20.0 + 4.0 / 30f32.to_radians().tan())).abs() < 0.01, "{spot:?}");
        // looking at the sky or over dry land there is no water
        assert!(surface_along_ray(Vec3::new(0.0, 0.0, 104.0), Vec3::Y, 400.0, sea).is_none());
        assert!(surface_along_ray(Vec3::new(0.0, 0.0, 104.0), dir, 400.0, |_| None).is_none());
        // a pond only: the ray that starts over land crosses the surface where the pond begins
        let pond = |p: Vec3| (p.y > 30.0).then_some(127.0);
        let spot = surface_along_ray(Vec3::new(0.0, 0.0, 140.0), Vec3::new(0.0, 0.96, -0.28).normalize(), 400.0, pond).expect("the pond");
        assert!((spot.z - 127.0).abs() < 1e-3 && spot.y > 30.0, "{spot:?}");
    }

    #[test]
    fn range_is_measured_in_3d_from_the_body() {
        let s = GroundSkill { mode: Mode::Ground, radius: 5.0, min_range: 0.0, max_range: 25.0 };
        let from = Vec3::new(100.0, 100.0, 10.0);
        assert!(in_range(&s, from, Vec3::new(125.0, 100.0, 10.0), 0.0));
        assert!(!in_range(&s, from, Vec3::new(126.0, 100.0, 10.0), 0.0));
        assert!(in_range(&s, from, Vec3::new(126.0, 100.0, 10.0), SEND_SLACK));
        // a spot 25 m away but 10 m higher is further
        assert!(!in_range(&s, from, Vec3::new(125.0, 100.0, 20.0), 0.0));
        let ring = GroundSkill { min_range: 10.0, max_range: 50.0, ..s };
        assert!(!in_range(&ring, from, Vec3::new(105.0, 100.0, 10.0), 0.0), "too close (Missile Rain)");
    }

    #[test]
    fn yaw_is_north_zero_counter_clockwise() {
        let o = Vec3::ZERO;
        assert!((yaw_to(o, Vec3::new(0.0, 5.0, 0.0)).unwrap()).abs() < 1e-6);
        assert!((yaw_to(o, Vec3::new(-5.0, 0.0, 0.0)).unwrap() - std::f32::consts::FRAC_PI_2).abs() < 1e-5, "west is +90 degrees");
        assert!((yaw_to(o, Vec3::new(5.0, 0.0, 0.0)).unwrap() + std::f32::consts::FRAC_PI_2).abs() < 1e-5);
        assert!(yaw_to(o, Vec3::new(0.0, 0.0, 9.0)).is_none());
    }

    #[derive(Resource, Default)]
    struct Sent(Vec<Command>);

    fn collect(mut commands: MessageReader<NetCommand>, mut sent: ResMut<Sent>) {
        sent.0.extend(commands.read().map(|c| c.0.clone()));
    }

    #[test]
    fn an_emote_skill_expresses_its_emotion_at_the_target() {
        let emotes = super::super::emote::Emotes::from_env();
        if emotes.is_empty_for_tests() {
            return;
        }
        let clap = emotes.command("clap").expect("the database has /clap");
        // /clap is skill 14460 of the emote tab
        assert_eq!(emotes.skill(14460), Some(clap));
        let mut app = App::new();
        app.add_message::<SkillPressed>()
            .add_message::<crate::hud::PortalBookUsed>()
            .add_message::<crate::hud::MusicItemUsed>()
            .add_message::<NetCommand>()
            .insert_resource(GroundSkills::default())
            .init_resource::<Aiming>()
            .init_resource::<HudPointerOverUi>()
            .init_resource::<InFlight>()
            .init_resource::<Script>()
            .init_resource::<Target>()
            .init_resource::<Units>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<Sent>()
            .insert_resource(emotes)
            .add_systems(Update, (route, collect).chain());
        app.world_mut().write_message(SkillPressed { skill: 14460, item: None, target_obj_id: 77, gcd: true, water: false, caster: None });
        // a skill that is no emote, and an item whose skill is one: plain uses
        app.world_mut().write_message(SkillPressed { skill: 11_001, item: None, target_obj_id: 5, gcd: true, water: false, caster: None });
        app.world_mut().write_message(SkillPressed { skill: 14460, item: Some((9, 3)), target_obj_id: 0, gcd: true, water: false, caster: None });
        app.update();
        let sent = std::mem::take(&mut app.world_mut().resource_mut::<Sent>().0);
        assert!(matches!(sent[0], Command::ExpressEmotion { emotion_id, target_obj_id: 77 } if emotion_id == clap), "{:?}", sent[0]);
        assert!(matches!(sent[1], Command::UseSkill { skill_id: 11_001, target_obj_id: 5 }), "{:?}", sent[1]);
        assert!(matches!(sent[2], Command::UseItem { skill_id: 14460, .. }), "{:?}", sent[2]);
        assert_eq!(sent.len(), 3);
    }

    #[test]
    fn a_sent_skill_that_waits_for_the_global_cooldown_is_in_flight() {
        let mut app = App::new();
        app.add_message::<SkillPressed>()
            .add_message::<crate::hud::PortalBookUsed>()
            .add_message::<crate::hud::MusicItemUsed>()
            .add_message::<NetCommand>()
            .insert_resource(GroundSkills::default())
            .init_resource::<Aiming>()
            .init_resource::<HudPointerOverUi>()
            .init_resource::<InFlight>()
            .init_resource::<Script>()
            .init_resource::<Target>()
            .init_resource::<Units>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<Sent>()
            .add_systems(Update, (route, collect).chain());
        let pending = |app: &App| app.world().resource::<InFlight>().pending(&aa_net::state::Skills::default(), std::time::Instant::now());
        // a skill that ignores the global cooldown does not hold the others back
        app.world_mut().write_message(SkillPressed { skill: 11_001, item: None, target_obj_id: 5, gcd: false, water: false, caster: None });
        app.update();
        assert!(!pending(&app));
        app.world_mut().write_message(SkillPressed { skill: 11_002, item: None, target_obj_id: 5, gcd: true, water: false, caster: None });
        app.update();
        assert!(pending(&app));
        assert_eq!(app.world().resource::<Sent>().0.len(), 2);
    }

    #[test]
    fn a_cannon_casts_at_the_spot_without_turning_or_stopping_the_gunner() {
        let fire = SkillPressed { skill: 10473, item: None, target_obj_id: 0, gcd: false, water: false, caster: Some((500, 16)) };
        let spot = Pos::new(10.0, 20.0, 1.0);
        assert!(matches!(
            fire.at(spot),
            Command::UseMateSkillAt { skill_id: 10473, mate_obj_id: 500, mount_skill_id: 16, pos } if (pos.x, pos.y) == (10.0, 20.0)
        ));
        assert!(matches!(fire.plain(), Command::UseMateSkill { mate_obj_id: 500, mount_skill_id: 16, .. }));
        let cast = |press: SkillPressed| {
            let mut app = App::new();
            app.add_message::<NetCommand>().init_resource::<InFlight>().init_resource::<Sent>();
            app.add_systems(
                Update,
                (
                    move |mut net: MessageWriter<NetCommand>, mut flight: ResMut<InFlight>| {
                        let mut mover = Mover { pos: Vec3::ZERO, facing: 0.0, vz: 0.0, grounded: true, velocity: Vec3::ZERO, swim: None, wading: 0.0 };
                        cast_at(&press, Vec3::new(10.0, 20.0, 1.0), &mut mover, &mut net, &mut flight);
                        assert_eq!(mover.facing != 0.0, press.caster.is_none(), "only a walking caster turns to the spot");
                    },
                    collect,
                )
                    .chain(),
            );
            app.update();
            std::mem::take(&mut app.world_mut().resource_mut::<Sent>().0)
        };
        let sent = cast(fire);
        assert_eq!(sent.len(), 1, "{sent:?}");
        assert!(matches!(sent[0], Command::UseMateSkillAt { mate_obj_id: 500, .. }));
        let sent = cast(SkillPressed { caster: None, skill: 10664, ..fire });
        assert!(matches!(sent.as_slice(), [Command::Move(_), Command::UseSkillAt { skill_id: 10664, .. }]), "{sent:?}");
    }

    /// A route app with the wagon's summon item (18660 -> slave 60, a land vehicle) and, when `out`, the wagon summoned.
    fn wagon_route(out: bool) -> App {
        let mut app = App::new();
        app.add_message::<SkillPressed>()
            .add_message::<crate::hud::PortalBookUsed>()
            .add_message::<crate::hud::MusicItemUsed>()
            .add_message::<NetCommand>()
            .insert_resource(GroundSkills(HashMap::from([(15802, GroundSkill { mode: Mode::Water, radius: 0.0, min_range: 0.0, max_range: 0.0 })])))
            .insert_resource(SummonItems(HashMap::from([(18660, (60, true)), (18000, (22, false))])))
            .init_resource::<Aiming>()
            .init_resource::<HudPointerOverUi>()
            .init_resource::<InFlight>()
            .init_resource::<Script>()
            .init_resource::<Target>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<Sent>();
        let mut units = Units::default();
        let mut slaves = aa_net::state::Slaves::default();
        if out {
            let unit = crate::ingame::vehicle::flow_tests::unit(500, aa_net::model::UnitKind::Slave { id: 9, tl_id: 7, template_id: 60, summoner_id: 47 });
            units.by_id.insert(500, unit);
            slaves.own = Some(aa_net::slave::SlaveCreated { owner_obj_id: 100, tl_id: 7, obj_id: 500, hide_spawn_effect: true, raw_id: 0, creator_name: String::new() });
        }
        app.insert_resource(units).insert_resource(slaves);
        app.world_mut().spawn((LocalPlayer { obj_id: 100 }, Mover { pos: Vec3::new(100.0, 200.0, 5.0), facing: 0.0, vz: 0.0, grounded: true, velocity: Vec3::ZERO, swim: None, wading: 0.0 }));
        app.add_systems(Update, (route, collect).chain());
        app
    }

    #[test]
    fn a_land_vehicle_summon_item_is_placed_seven_metres_ahead() {
        let mut app = wagon_route(false);
        app.world_mut().write_message(SkillPressed { skill: 15802, item: Some((9, 18660)), target_obj_id: 0, gcd: true, water: false, caster: None });
        app.update();
        let sent = std::mem::take(&mut app.world_mut().resource_mut::<Sent>().0);
        // facing north: 7 m up the y axis, no turn / stop packet before it
        assert!(
            matches!(sent.as_slice(), [Command::UseItemAt { item_id: 9, template_id: 18660, skill_id: 15802, pos }] if (pos.x - 100.0).abs() < 1e-3 && (pos.y - 207.0).abs() < 1e-3),
            "{sent:?}"
        );
        let spot = vehicle_summon_spot(Vec3::new(0.0, 0.0, 0.0), std::f32::consts::FRAC_PI_2);
        assert!((spot.x + 7.0).abs() < 1e-3 && spot.y.abs() < 1e-3, "facing +90 degrees is west: {spot:?}");
    }

    #[test]
    fn using_the_summon_item_again_recalls_the_vehicle_with_csdespawnslave() {
        let mut app = wagon_route(true);
        app.world_mut().write_message(SkillPressed { skill: 15802, item: Some((9, 18660)), target_obj_id: 0, gcd: true, water: false, caster: None });
        app.update();
        let sent = std::mem::take(&mut app.world_mut().resource_mut::<Sent>().0);
        assert!(matches!(sent.as_slice(), [Command::Slave(aa_net::slave_control::SlaveCommand::Despawn { obj_id: 500 })]), "{sent:?}");
        // a ship item keeps the plain cast
        app.world_mut().write_message(SkillPressed { skill: 15802, item: Some((9, 18000)), target_obj_id: 0, gcd: true, water: false, caster: None });
        app.update();
        let sent = std::mem::take(&mut app.world_mut().resource_mut::<Sent>().0);
        assert!(matches!(sent.as_slice(), [Command::UseItem { template_id: 18000, .. }]), "{sent:?}");
    }

    #[test]
    fn music_paper_and_sheet_use_local_controller_before_any_cast() {
        let mut app = App::new();
        app.add_message::<SkillPressed>()
            .add_message::<crate::hud::PortalBookUsed>()
            .add_message::<crate::hud::MusicItemUsed>()
            .add_message::<NetCommand>()
            .insert_resource(GroundSkills::default())
            .init_resource::<Aiming>()
            .init_resource::<HudPointerOverUi>()
            .init_resource::<InFlight>()
            .init_resource::<Script>()
            .init_resource::<Target>()
            .init_resource::<Units>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<Sent>()
            .add_systems(Update, (route, collect).chain());
        for template in [27902, 28051] {
            app.world_mut().write_message(SkillPressed { skill: 22215, item: Some((9_007_199_254_740_993, template)), target_obj_id: 0, gcd: true, water: false, caster: None });
            app.update();
            assert!(app.world().resource::<Sent>().0.is_empty());
            let opened: Vec<_> = app.world_mut().resource_mut::<Messages<crate::hud::MusicItemUsed>>().drain().collect();
            assert!(matches!(opened.as_slice(), [crate::hud::MusicItemUsed(9_007_199_254_740_993)]));
        }
        assert!(!app.world().resource::<InFlight>().pending(&default(), std::time::Instant::now()));
    }

    #[test]
    fn the_database_marks_the_player_skills() {
        let Some(skills) = load_skills() else { return };
        if skills.is_empty() {
            return;
        }
        let meteor = skills[&10664];
        assert_eq!((meteor.mode, meteor.radius, meteor.max_range), (Mode::Ground, 5.0, 25.0));
        let rain = skills[&13281];
        assert_eq!((rain.radius, rain.min_range, rain.max_range), (7.0, 10.0, 50.0));
    }
}
