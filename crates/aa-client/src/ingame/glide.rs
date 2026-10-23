//! Gliders: the flight of the local player under a gliding buff, the controls, the movement packets, and the pose the
//! glider and the character play.
//!
//! # What was read, what is guessed
//!
//! **Read in the 10.x zone/client `x2game` decompile** (`x2game-dev_dedicate.dll.c`, the same `Player` class the 1.2 client has; the
//! 1.2 `x2game` shares the buff columns, the option and the strings, but its own functions were not traced):
//!
//! * A buff with the `gliding` flag starts the flight (`ClientBuffMan::ApplyAttrs` -> "[glide] create gliding buff" ->
//!   `sub_3925A430`), and its removal ends it ("[glide] destroy gliding buff", `CleanupGlider`). The flight is the buff, not a
//!   key: nothing but the buff's presence decides whether the player flies.
//! * `sub_3925A430` builds the flight's 17 numbers from the buff row: the three move speeds are the unit attributes
//!   `GlidingMoveSpeedSlow / Normal / Fast` (7, 8, 9: the column plus the buffs' modifiers, per mille), the turn rate is
//!   `gliding_rotate_speed` degrees a second as radians, the rest are the columns in the table's order
//!   (`startup_time`, `startup_speed`, `fall_speed_slow / normal / fast`, `smooth_time`, `lift_count`, `lift_height`,
//!   `lift_valid_time`, `lift_duration`, `lift_speed`, `land_height`, `sliding_time`) ([`GlideParams`]).
//! * `StartGlide(true)` sets the flight speed to `startup_speed`, the movement mode to 4 (gliding) and divides `lift_duration` by
//!   `lift_count` ([`Flight::open`]); with `sliding_time` above 0 and ground within `land_height` + 0.5 m below, the mode is 5
//!   (sliding) at once. Mode 5 lasts `sliding_time` (the per-frame update counts it down) and then the glider is put away
//!   (`CleanupGlider`). Mode 1 is the freefall `super::jump` already has.
//! * The glider model plays the events `loop` / `endloop`; the character's clips are `fist_ac_gliding_*` (the `.cal`: `idle`, `start`,
//!   `front`, `back`, `left`, `right`, `fl`, `fr`, `bl`, `br`, `sliding`, `end`, `spin`, `spin2`, `tumbling_front / back`), the glider
//!   model's own `.cal` names `start`, `loop`, `end`, `endloop`, `left`, `right`, `front`, `back`, `spin`, `sliding`.
//! * The option `glider_start_with_double_jump` (`OPTION_ITEM_USE_GLIDE_WITH_DOUBLE_JUMP`, native option 56 in 1.2) lets a second
//!   press of jump in the air use the glider; the buff's removal by landing is the server's (`remove_on_land` on 53 of the 55 gliding
//!   buffs, fired by the landing flags of a movement packet).
//!
//! **Guessed** (no 1.2 oracle at hand; each one is a named constant or a plain rule below so a capture of the real client can move it):
//!
//! * the keys: W flies at the `fast` move and fall speeds, S at the `slow` ones, nothing at `normal`; A / D turn at the buff's rate
//!   (the cvars `gliding_mouse_ad` / `gliding_mouse_ws` are mouse sensitivities for the same two axes; the mouse is not read here);
//! * the smoothing of speed changes (an exponential approach with `smooth_time` as the time constant);
//! * the lift: Space, `lift_count` times, within `lift_valid_time` seconds of the end of the opening, each rising at `lift_speed`
//!   for `lift_duration / lift_count` seconds or `lift_height` metres;
//! * the default of option 56 (taken as on, like its neighbour 55), the deploy height (the freefall's, 3 m after a jump), and the
//!   result a refused skill shows while gliding (`NOT_NOW`).

use std::collections::HashMap;
use std::sync::Arc;

use aa_anim::pose::BoundClip;
use aa_anim::{Animator, Cal, ClipLibrary};
use aa_net::model::{slot, stance};
use aa_net::state::{Buffs, Inventory};
use aa_net::{Command, MoveCommand, MoveState, NetCommand, Pos};
use aa_world::{TerrainQuery, WaterQuery, WorldCollision};
use bevy::prelude::*;

use super::anim::{Locomotion, Motion, bind_clip};
use super::camera::OrbitCamera;
use super::combat::{Acting, Dead};
use super::data::Db;
use super::dev::DevOptions;
use super::player::{self, LocalPlayer, Mover, PlayerInput, SendState};

/// The native option `OPTION_ITEM_USE_GLIDE_WITH_DOUBLE_JUMP`.
pub const DOUBLE_JUMP_OPTION: u32 = 56;
/// The skill the action bar shows as "Use Glider" (it has no effect of its own: the equipped glider's item skill is meant).
pub const USE_GLIDER_SKILL: u32 = 14901;
/// Interval between movement packets while flying (s).
const SEND_INTERVAL: f32 = 0.1;
/// Seconds without the buff after which the flight is over (the buff list lags a frame or two behind the packets).
const BUFF_GRACE: f32 = 0.3;
/// Seconds a deploy request waits for the server's answer before it may go out again.
const RETRY: f64 = 1.0;
/// Height above the ground (m) a second jump needs to open the glider when the character is not yet in a freefall.
pub const DEPLOY_HEIGHT: f32 = super::jump::FALL_START_HEIGHT_JUMPED;
/// How far under the feet a surface still counts as the ground of a flight (m).
const GROUND_REACH: f32 = 0.3;
/// A flight that has the ground this close under it and has not sliding to do lands (m).
const TOUCH: f32 = 0.02;

/// The numbers of one gliding buff ([`GlideParams::load_all`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlideParams {
    /// Move speed (m/s) at the slow, normal and fast keys (S, none, W).
    pub move_speed: [f32; 3],
    /// Fall speed (m/s, downwards) at the slow, normal and fast keys.
    pub fall_speed: [f32; 3],
    /// Turn rate (radians a second).
    pub rotate: f32,
    /// Seconds the opening lasts and the speed it begins with (m/s).
    pub startup_time: f32,
    pub startup_speed: f32,
    /// Time constant of the speed changes (s).
    pub smooth_time: f32,
    pub lift_count: u32,
    pub lift_height: f32,
    pub lift_valid_time: f32,
    pub lift_duration: f32,
    pub lift_speed: f32,
    /// Height above the ground (m) at which the flight turns into the slide, and how long the slide lasts (s).
    pub land_height: f32,
    pub sliding_time: f32,
}

impl Default for GlideParams {
    /// The defaults `sub_3925A430` starts from (and the row of most NPC gliders, e.g. buff 1371).
    fn default() -> Self {
        GlideParams {
            move_speed: [6.0, 8.0, 10.0],
            fall_speed: [1.0, 2.0, 3.0],
            rotate: 1.0,
            startup_time: 0.7,
            startup_speed: 3.0,
            smooth_time: 0.3,
            lift_count: 2,
            lift_height: 1.5,
            lift_valid_time: 10.0,
            lift_duration: 0.5,
            lift_speed: 3.0,
            land_height: 0.5,
            sliding_time: 1.0,
        }
    }
}

impl GlideParams {
    /// The gliding buffs of the game database (`buffs.gliding`, 55 rows in r208022's), by buff id.
    pub fn load_all() -> HashMap<u32, GlideParams> {
        let path = std::env::var("AA_DB").unwrap_or_else(|_| aa_anim::db::DEFAULT_DB.to_string());
        let mut out = HashMap::new();
        let Ok(conn) = rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI) else {
            warn!("glide: no game database, no gliding buff is known");
            return out;
        };
        let read = conn
            .prepare(
                "SELECT id, gliding_rotate_speed, gliding_startup_time, gliding_startup_speed, gliding_fall_speed_slow, gliding_fall_speed_normal, \
                 gliding_fall_speed_fast, gliding_smooth_time, gliding_lift_count, gliding_lift_height, gliding_lift_valid_time, gliding_lift_duration, \
                 gliding_lift_speed, gliding_land_height, gliding_sliding_time, gliding_move_speed_slow, gliding_move_speed_normal, gliding_move_speed_fast \
                 FROM buffs WHERE gliding = 't'",
            )
            .and_then(|mut st| {
                st.query_map([], |r| {
                    let f = |i: usize| -> rusqlite::Result<f32> { Ok(r.get::<_, Option<f64>>(i)?.unwrap_or(0.0) as f32) };
                    let id = r.get::<_, i64>(0)? as u32;
                    Ok((
                        id,
                        GlideParams {
                            rotate: f(1)?.to_radians(),
                            startup_time: f(2)?,
                            startup_speed: f(3)?,
                            fall_speed: [f(4)?, f(5)?, f(6)?],
                            smooth_time: f(7)?,
                            lift_count: f(8)?.max(0.0) as u32,
                            lift_height: f(9)?,
                            lift_valid_time: f(10)?,
                            lift_duration: f(11)?,
                            lift_speed: f(12)?,
                            land_height: f(13)?,
                            sliding_time: f(14)?,
                            move_speed: [f(15)?, f(16)?, f(17)?],
                        },
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()
            });
        match read {
            Ok(rows) => out.extend(rows),
            Err(e) => warn!("glide: buffs.gliding_*: {e}"),
        }
        out
    }
}

/// What the player asks of the glider in one frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    /// W 1, S -1.
    pub forward: f32,
    /// A (left) 1, D (right) -1.
    pub turn: f32,
    /// Jump was pressed this frame.
    pub lift: bool,
}

/// How the flight goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// The glider opens (`startup_time`): the speed it began with, no lift.
    Open,
    Gliding,
    /// Within `land_height` of the ground: the speed runs out over `sliding_time`.
    Sliding,
}

/// One frame of a flight, what the controller applies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    /// Radians a second to turn by (positive turns left).
    pub turn_rate: f32,
    /// Ground speed along the facing (m/s).
    pub speed: f32,
    /// Vertical speed (m/s, up is positive).
    pub vz: f32,
    pub phase: Phase,
    /// A lift is going on.
    pub lifting: bool,
    /// The flight is over: the slide ran out, or the ground is under the character and there is no slide.
    pub done: bool,
}

/// The state of a flight: pure numbers, stepped by the controller ([`control_glide`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flight {
    pub params: GlideParams,
    pub phase: Phase,
    /// Seconds since the glider opened.
    pub age: f32,
    /// Ground speed (m/s) and vertical speed (m/s, up positive).
    pub speed: f32,
    pub vz: f32,
    /// Lifts used.
    pub lifts: u32,
    lift_left: f32,
    lift_rise: f32,
    slide_left: f32,
    slide_from: f32,
}

/// Which of the three speed sets a forward key asks for: 0 slow (S), 1 normal, 2 fast (W).
pub fn speed_index(forward: f32) -> usize {
    if forward > 0.1 {
        2
    } else if forward < -0.1 {
        0
    } else {
        1
    }
}

/// One step of an exponential approach with time constant `tau` (instant for 0).
fn approach(value: f32, target: f32, dt: f32, tau: f32) -> f32 {
    if tau <= 0.0 { target } else { value + (target - value) * (1.0 - (-dt / tau).exp()) }
}

impl Flight {
    /// The glider opens: `vz` is the vertical speed the character had, `height` its height above the ground. Near the ground
    /// (`sliding_time` set and within `land_height` + 0.5 m, not rising) the flight begins as the slide.
    pub fn open(params: GlideParams, vz: f32, height: f32) -> Flight {
        let mut flight = Flight {
            params: GlideParams { lift_duration: params.lift_duration / if params.lift_count > 0 { params.lift_count as f32 } else { 1.0 }, ..params },
            phase: Phase::Open,
            age: 0.0,
            speed: params.startup_speed,
            vz,
            lifts: 0,
            lift_left: 0.0,
            lift_rise: 0.0,
            slide_left: 0.0,
            slide_from: 0.0,
        };
        if params.sliding_time > 0.0 && vz <= 0.0 && height <= params.land_height + 0.5 {
            flight.start_sliding();
        }
        flight
    }

    fn start_sliding(&mut self) {
        self.phase = Phase::Sliding;
        self.slide_left = self.params.sliding_time;
        self.slide_from = self.speed;
    }

    /// One frame: `height` is the distance over the ground now.
    pub fn step(&mut self, dt: f32, input: Input, height: f32) -> Step {
        let p = self.params;
        self.age += dt;
        let key = speed_index(input.forward);
        if self.phase == Phase::Open && self.age >= p.startup_time {
            self.phase = Phase::Gliding;
        }
        // the lift: Space, a few times, after the opening and within the valid time
        if input.lift
            && self.phase == Phase::Gliding
            && self.lifts < p.lift_count
            && self.lift_left <= 0.0
            && self.age - p.startup_time <= p.lift_valid_time
        {
            self.lifts += 1;
            self.lift_left = p.lift_duration;
            self.lift_rise = 0.0;
        }
        let lifting = self.lift_left > 0.0;
        // near the ground the glider slides
        if self.phase != Phase::Sliding && p.sliding_time > 0.0 && !lifting && self.vz <= 0.0 && height <= p.land_height {
            self.start_sliding();
        }
        match self.phase {
            Phase::Sliding => {
                self.slide_left -= dt;
                self.speed = self.slide_from * (self.slide_left / p.sliding_time.max(1e-3)).clamp(0.0, 1.0);
                // it comes down the rest of the way at the slow fall speed
                self.vz = if height > TOUCH { -p.fall_speed[0].max(0.5) } else { 0.0 };
            }
            phase => {
                let (move_target, fall_target) = if phase == Phase::Open { (p.startup_speed, p.fall_speed[1]) } else { (p.move_speed[key], p.fall_speed[key]) };
                self.speed = approach(self.speed, move_target, dt, p.smooth_time);
                if lifting {
                    self.vz = p.lift_speed;
                    self.lift_rise += p.lift_speed * dt;
                    self.lift_left -= dt;
                    if self.lift_rise >= p.lift_height {
                        self.lift_left = 0.0;
                    }
                    // the burst stops where it ends: the glider settles from rest, it does not coast up
                    if self.lift_left <= 0.0 {
                        self.vz = 0.0;
                    }
                } else {
                    // the opening takes the fall down to the glider's rate over half of it
                    let tau = if phase == Phase::Open { p.smooth_time.max(p.startup_time * 0.5) } else { p.smooth_time };
                    self.vz = approach(self.vz, -fall_target, dt, tau);
                }
            }
        }
        let done = match self.phase {
            Phase::Sliding => self.slide_left <= 0.0,
            _ => p.sliding_time <= 0.0 && height <= TOUCH && self.vz <= 0.0,
        };
        Step {
            turn_rate: if self.phase == Phase::Sliding { 0.0 } else { input.turn * p.rotate },
            speed: self.speed,
            vz: self.vz,
            phase: self.phase,
            lifting,
            done,
        }
    }
}

/// The pose the character and the glider play (see the clip names in the module documentation).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Pose {
    /// The opening: `start`.
    #[default]
    Start,
    Idle,
    Front,
    Back,
    Left,
    Right,
    FrontLeft,
    FrontRight,
    BackLeft,
    BackRight,
    /// A lift: `spin`.
    Lift,
    Slide,
}

/// The pose for a frame.
pub fn pose(phase: Phase, input: Input, lifting: bool) -> Pose {
    match phase {
        Phase::Open => Pose::Start,
        Phase::Sliding => Pose::Slide,
        Phase::Gliding if lifting => Pose::Lift,
        Phase::Gliding => {
            let (f, t) = (speed_index(input.forward) as i32 - 1, if input.turn > 0.1 { 1 } else if input.turn < -0.1 { -1 } else { 0 });
            match (f, t) {
                (1, 1) => Pose::FrontLeft,
                (1, -1) => Pose::FrontRight,
                (1, _) => Pose::Front,
                (-1, 1) => Pose::BackLeft,
                (-1, -1) => Pose::BackRight,
                (-1, _) => Pose::Back,
                (_, 1) => Pose::Left,
                (_, -1) => Pose::Right,
                _ => Pose::Idle,
            }
        }
    }
}

impl Pose {
    /// The character's clip (`fist_ac_gliding_<suffix>` in its `.cal`) and whether it loops.
    pub fn character_clip(self) -> (&'static str, bool) {
        match self {
            Pose::Start => ("fist_ac_gliding_start", false),
            Pose::Idle => ("fist_ac_gliding_idle", true),
            Pose::Front => ("fist_ac_gliding_front", true),
            Pose::Back => ("fist_ac_gliding_back", true),
            Pose::Left => ("fist_ac_gliding_left", true),
            Pose::Right => ("fist_ac_gliding_right", true),
            Pose::FrontLeft => ("fist_ac_gliding_fl", true),
            Pose::FrontRight => ("fist_ac_gliding_fr", true),
            Pose::BackLeft => ("fist_ac_gliding_bl", true),
            Pose::BackRight => ("fist_ac_gliding_br", true),
            Pose::Lift => ("fist_ac_gliding_spin", false),
            Pose::Slide => ("fist_ac_gliding_sliding", true),
        }
    }

    /// The glider model's clip (its `.cal`: `start`, `loop`, `front`, `back`, `left`, `right`, `spin`, `sliding`) and whether it loops.
    /// The diagonals take the side clip of the turn.
    pub fn glider_clip(self) -> (&'static str, bool) {
        match self {
            Pose::Start => ("start", false),
            Pose::Idle => ("loop", true),
            Pose::Front => ("front", true),
            Pose::Back => ("back", true),
            Pose::Left | Pose::FrontLeft | Pose::BackLeft => ("left", true),
            Pose::Right | Pose::FrontRight | Pose::BackRight => ("right", true),
            Pose::Lift => ("spin", false),
            Pose::Slide => ("sliding", true),
        }
    }
}

/// The flight the local player is in, with the buff that carries it.
#[derive(Clone, Copy, Debug)]
pub struct Active {
    pub flight: Flight,
    pub buff_id: u32,
    pub buff_index: u32,
    pub pose: Pose,
    /// Seconds the buff has been missing.
    missing: f32,
    /// The flight is over and the landing was sent; the buff is still there (seconds).
    after: Option<f32>,
}

/// The local player's glider: the flight, the numbers of every gliding buff and what the options said.
#[derive(Resource, Default)]
pub struct Gliding {
    pub active: Option<Active>,
    params: Option<HashMap<u32, GlideParams>>,
    /// When the last deploy request went out (s).
    asked: Option<f64>,
    since_sent: f32,
    sent_first: bool,
}

impl Gliding {
    /// The player is in the air under a glider.
    pub fn flying(&self) -> bool {
        self.active.is_some_and(|a| a.after.is_none())
    }

    /// The numbers of a gliding buff (loaded once).
    pub fn params_of(&mut self, buff: u32) -> Option<GlideParams> {
        self.params.get_or_insert_with(GlideParams::load_all).get(&buff).copied()
    }
}

/// The glider model of the character, spawned with the flight.
#[derive(Component)]
pub struct GliderModel;

/// Marks the stowed backpack or glider on a character's back: hidden while the glider is out.
#[derive(Component)]
pub struct StowedPack;

/// Whether option 56 is on (`glider_start_with_double_jump` registers 0 in 1.2).
pub fn double_jump_on(saved: Option<f64>) -> bool {
    saved.or_else(|| crate::settings::native_default(DOUBLE_JUMP_OPTION)).is_some_and(|v| v != 0.0)
}

/// Whether a jump press in the air opens the glider: the option is on, the character is in the air, in a freefall or `DEPLOY_HEIGHT`
/// over the ground, not swimming, and no glider is out.
pub fn may_deploy(option_on: bool, airborne: bool, swimming: bool, freefall: bool, height: f32, flying: bool) -> bool {
    option_on && airborne && !swimming && !flying && (freefall || height >= DEPLOY_HEIGHT)
}

/// The first gliding buff among a unit's buffs: `(buff id, index)`.
pub fn gliding_buff(buffs: &[aa_net::state::BuffEntry], mut is_gliding: impl FnMut(u32) -> bool) -> Option<(u32, u32)> {
    buffs.iter().find(|e| is_gliding(e.buff.buff_id)).map(|e| (e.buff.buff_id, e.buff.index))
}

/// Starts and ends the flight with the own gliding buff.
#[allow(clippy::too_many_arguments)]
pub fn track_flight(
    time: Res<Time>,
    buffs: Option<Res<Buffs>>,
    terrain: Res<TerrainQuery>,
    collision: Res<WorldCollision>,
    mut gliding: ResMut<Gliding>,
    mut state: ResMut<SendState>,
    mut player: Query<(&LocalPlayer, &mut Mover, Has<Dead>)>,
) {
    let dt = time.delta_secs().min(0.1);
    let Ok((local, mut mover, dead)) = player.single_mut() else {
        gliding.active = None;
        return;
    };
    let own: &[aa_net::state::BuffEntry] = buffs.as_deref().map_or(&[], |b| b.of(local.obj_id));
    let found = {
        let g = &mut *gliding;
        let params = g.params.get_or_insert_with(GlideParams::load_all);
        gliding_buff(own, |id| params.contains_key(&id))
    };
    if dead {
        gliding.active = None;
        return;
    }
    let Some(active) = gliding.active else {
        let Some((buff_id, buff_index)) = found else { return };
        let Some(params) = gliding.params_of(buff_id) else { return };
        if mover.swim.is_some() || !state.is_settled() {
            return;
        }
        let ground = player::floor_at(&terrain, &collision, mover.pos, GROUND_REACH);
        let height = if ground.is_finite() { (mover.pos.z - ground).max(0.0) } else { 0.0 };
        let flight = Flight::open(params, if mover.grounded { 0.0 } else { mover.vz }, height);
        info!("glide: buff {buff_id} opens the glider at {height:.1} m, fall {:.1} m/s, {:?}", mover.vz, flight.phase);
        mover.grounded = false;
        mover.swim = None;
        gliding.active = Some(Active { flight, buff_id, buff_index, pose: pose(flight.phase, Input::default(), false), missing: 0.0, after: None });
        gliding.since_sent = SEND_INTERVAL;
        gliding.sent_first = false;
        return;
    };
    let mut active = active;
    match (found, active.after) {
        (Some((id, index)), _) => {
            active.missing = 0.0;
            active.buff_id = id;
            active.buff_index = index;
        }
        (None, _) => active.missing += dt,
    }
    // the buff is gone (landing, a hit, the glider folded): the flight is over, in the air the character falls on
    if active.missing >= BUFF_GRACE || (active.after.is_some() && found.is_none()) {
        if active.after.is_none() {
            info!("glide: the buff is gone, {:.1} m/s", active.flight.vz);
            mover.grounded = false;
            mover.vz = active.flight.vz.min(0.0);
            state.set_air(true);
        }
        gliding.active = None;
        return;
    }
    if let Some(after) = active.after.as_mut() {
        *after += dt;
    }
    gliding.active = Some(active);
}

/// A press of jump in the air with a glider on the back uses it (option 56): the item's skill, as using the item would.
#[allow(clippy::too_many_arguments)]
pub fn request_deploy(
    time: Res<Time>,
    actions: Res<crate::settings::Actions>,
    typing: Option<Res<crate::hud::HudKeyboardFocus>>,
    inventory: Option<Res<Inventory>>,
    db: Option<Res<Db>>,
    terrain: Res<TerrainQuery>,
    state: Res<SendState>,
    mut gliding: ResMut<Gliding>,
    player: Query<(&Mover, Has<Dead>), With<LocalPlayer>>,
    mut out: MessageWriter<NetCommand>,
) {
    let Ok((mover, dead)) = player.single() else { return };
    if dead || typing.is_some_and(|f| f.0) || !actions.just_pressed("jump", 0) {
        return;
    }
    let now = time.elapsed_secs_f64();
    if gliding.asked.is_some_and(|t| now - t < RETRY) {
        return;
    }
    let ground = terrain.ground_at(mover.pos.x, mover.pos.y).map_or(mover.pos.z, |g| g.height);
    let airborne = !mover.grounded && mover.swim.is_none();
    if !may_deploy(double_jump_on(crate::settings::option_number("n:56")), airborne, mover.swim.is_some(), state.freefall(), mover.pos.z - ground, gliding.flying()) {
        return;
    }
    let Some(skill) = equipped_glider_skill(inventory.as_deref(), db.as_deref()) else { return };
    gliding.asked = Some(now);
    info!("glide: second jump in the air, glider skill {skill}");
    out.write(NetCommand(Command::UseSkill { skill_id: skill, target_obj_id: 0 }));
}

/// The use skill of the glider on the back (`items.use_skill_id` of a `backpack_type` 2 item).
pub fn equipped_glider_skill(inventory: Option<&Inventory>, db: Option<&Db>) -> Option<u32> {
    let item = inventory?.equipment.get(slot::BACKPACK)?.template_id;
    db?.glider(item).map(|g| g.use_skill).filter(|s| *s != 0)
}

/// The action bar's "Use Glider" (skill 14901, no effect of its own) uses the glider on the back.
pub fn redirect_use_glider(
    time: Res<Time>,
    mut messages: ResMut<Messages<NetCommand>>,
    mut cursor: Local<bevy::ecs::message::MessageCursor<NetCommand>>,
    inventory: Option<Res<Inventory>>,
    db: Option<Res<Db>>,
    mut gliding: ResMut<Gliding>,
) {
    let now = time.elapsed_secs_f64();
    let mut forward = Vec::new();
    for NetCommand(cmd) in cursor.read(&messages) {
        if matches!(cmd, Command::UseSkill { skill_id: USE_GLIDER_SKILL, .. })
            && !gliding.asked.is_some_and(|t| now - t < RETRY)
            && let Some(skill) = equipped_glider_skill(inventory.as_deref(), db.as_deref())
        {
            forward.push(skill);
        }
    }
    for skill in forward {
        gliding.asked = Some(now);
        info!("glide: Use Glider asks for skill {skill}");
        messages.write(NetCommand(Command::UseSkill { skill_id: skill, target_obj_id: 0 }));
    }
}

/// The flight: keys, turning, the move, the ground and the movement packets. It runs before `player::control_player`, which
/// leaves the character alone while a flight lasts.
#[allow(clippy::too_many_arguments)]
pub fn control_glide(
    time: Res<Time>,
    actions: Res<crate::settings::Actions>,
    typing: Option<Res<crate::hud::HudKeyboardFocus>>,
    dev: Res<DevOptions>,
    terrain: Res<TerrainQuery>,
    collision: Res<WorldCollision>,
    water: Res<WaterQuery>,
    mut gliding: ResMut<Gliding>,
    mut state: ResMut<SendState>,
    mut input_state: ResMut<PlayerInput>,
    mut cameras: Query<&mut OrbitCamera>,
    mut player: Query<(&LocalPlayer, &mut Mover, &mut Transform, &mut Motion, Option<&player::Stances>)>,
    mut out: MessageWriter<NetCommand>,
) {
    let Some(mut active) = gliding.active.filter(|a| a.after.is_none()) else { return };
    let Ok((local, mut mover, mut transform, mut motion, stances)) = player.single_mut() else { return };
    let body = stances.map_or_else(player::StanceBody::default, |s| s.get(2));
    let dt = time.delta_secs().min(0.1);
    let t = state.advance_clock(dt);
    let typing = typing.is_some_and(|f| f.0);
    let pressed = |action: &str, c: char| (!typing && actions.pressed(action, 0)) || dev.scripted(t, c);
    let input = Input {
        forward: pressed("moveforward", 'W') as i32 as f32 - pressed("moveback", 'S') as i32 as f32,
        turn: pressed("turnleft", 'A') as i32 as f32 - pressed("turnright", 'D') as i32 as f32,
        lift: (!typing && actions.just_pressed("jump", 0)) || (dev.scripted(t, 'J') && !dev.scripted(t - dt, 'J')),
    };
    let old = mover.pos;
    let ground = player::floor_at(&terrain, &collision, old, GROUND_REACH);
    let height = if ground.is_finite() { (old.z - ground).max(0.0) } else { f32::MAX };
    let step = active.flight.step(dt, input, height);
    // turning: the camera turns with the character, as on foot
    let before_facing = mover.facing;
    let d = step.turn_rate * dt;
    mover.facing += d;
    if let Ok(mut orbit) = cameras.single_mut() {
        orbit.yaw += d;
    }
    // along the facing, walls slide the glider
    let ahead = player::forward(mover.facing) * step.speed * dt;
    let moved = player::move_candidate(&terrain, &collision, old, ahead, false, false, body);
    let mut pos = Vec3::new(moved.x, moved.y, old.z + step.vz * dt);
    let floor = player::floor_at(&terrain, &collision, Vec3::new(pos.x, pos.y, old.z.max(pos.z)), GROUND_REACH);
    let mut touched = false;
    if floor.is_finite() && pos.z <= floor {
        pos.z = floor;
        touched = true;
    }
    let wet = water.water_at(pos).filter(|(surface, _)| pos.z < *surface);
    let landed_in_water = wet.is_some();
    let velocity = if dt > 0.0 { (pos - old) / dt } else { Vec3::ZERO };
    mover.velocity = velocity;
    mover.pos = pos;
    mover.vz = step.vz;
    mover.grounded = false;
    *transform = player::placement(pos, mover.facing, 1.0);
    *motion = Motion::default();
    *input_state = PlayerInput { moving: step.speed > 0.05, jumped: false, turned: (mover.facing - before_facing).abs() > 1e-4 };
    active.pose = pose(step.phase, input, step.lifting);
    // the flight ends at the ground (after the slide), or in water
    let done = (step.done && touched) || (step.done && floor.is_infinite()) || landed_in_water || (touched && active.flight.params.sliding_time <= 0.0);
    gliding.since_sent += dt;
    let due = gliding.since_sent >= SEND_INTERVAL || !gliding.sent_first;
    if due || done {
        gliding.since_sent = 0.0;
        gliding.sent_first = true;
        let to_wire = |v: f32| (v * 1024.0).round().clamp(-32767.0, 32767.0) as i16;
        let pressed_dir = [0i8, (input.forward.signum() * input.forward.abs().ceil() * 127.0) as i8, if step.lifting { 127 } else { 0 }];
        let base = MoveCommand {
            pos: Pos::new(pos.x, pos.y, pos.z),
            yaw: mover.facing,
            moving: true,
            stance: stance::FLY,
            velocity: [to_wire(velocity.x), to_wire(velocity.y), to_wire(velocity.z)],
            delta_movement: pressed_dir,
            ..MoveCommand::default()
        };
        let cmd = if done {
            // the landing: standing on the ground again, the speed it came down at
            MoveCommand { stance: stance::RELAXED, moving: false, velocity: [0; 3], delta_movement: [0; 3], ..base }.with_state(MoveState::Fall).landing(-step.vz.min(0.0))
        } else {
            base.with_state(MoveState::Fall)
        };
        out.write(NetCommand(Command::Move(cmd)));
    }
    if done {
        info!("glide: landed at ({:.1}, {:.1}, {:.1}) after {:.1} s{}", pos.x, pos.y, pos.z, active.flight.age, if landed_in_water { " (water)" } else { "" });
        mover.vz = 0.0;
        mover.velocity = Vec3::ZERO;
        mover.grounded = !landed_in_water;
        state.set_air(false);
        // the server removes the buff with the landing; the flight is over for the character now, and the buff list follows
        active.after = Some(0.0);
    }
    let _ = local;
    gliding.active = Some(active);
}

// ---------------------------------------------------------------------------------------------------------- the look

/// The character's gliding clips, bound to its rig.
#[derive(Component, Default)]
pub struct GlideClips {
    bound: HashMap<&'static str, Option<BoundClip>>,
    playing: Option<Pose>,
    was: bool,
}

#[allow(clippy::type_complexity)]
/// Plays the gliding clips on the local character while it flies, after `anim::drive_locomotion`, and gives the animator back to
/// the locomotion when the flight ends.
pub fn drive_glide(
    gliding: Res<Gliding>,
    library: Res<ClipLibrary>,
    mut commands: Commands,
    mut player: Query<(Entity, &mut Animator, &mut Locomotion, Option<&mut GlideClips>, Option<&Acting>, Has<Dead>), With<LocalPlayer>>,
) {
    let Ok((entity, mut animator, mut loco, clips, acting, dead)) = player.single_mut() else { return };
    let Some(mut clips) = clips else {
        commands.entity(entity).insert(GlideClips::default());
        return;
    };
    let flight = gliding.active.filter(|a| a.after.is_none());
    let Some(active) = flight else {
        if clips.was {
            clips.was = false;
            clips.playing = None;
            loco.invalidate();
        }
        return;
    };
    clips.was = true;
    if dead || acting.is_some_and(|a| !a.layered) || clips.playing == Some(active.pose) {
        return;
    }
    let (name, looped) = active.pose.character_clip();
    let clip = clips
        .bound
        .entry(name)
        .or_insert_with(|| bind_clip(&animator.rig, &loco.clips.cal, &library, name))
        .clone();
    // a model without a clip of its own (a mate's rider, a race without the diagonal) takes the front or idle one
    let clip = clip.or_else(|| {
        let (fallback, _) = Pose::Idle.character_clip();
        clips.bound.entry(fallback).or_insert_with(|| bind_clip(&animator.rig, &loco.clips.cal, &library, fallback)).clone()
    });
    if let Some(clip) = clip {
        let fade = if active.pose == Pose::Start { 0.1 } else { 0.25 };
        animator.play(clip, looped, fade);
        animator.random_idle = None;
    }
    clips.playing = Some(active.pose);
}

/// Spawns the opened glider on the character while it flies and removes it afterwards; hides the stowed one.
#[allow(clippy::too_many_arguments)]
pub fn show_glider(
    gliding: Res<Gliding>,
    inventory: Option<Res<Inventory>>,
    db: Option<Res<Db>>,
    library: Res<ClipLibrary>,
    mut commands: Commands,
    mut cry: aa_cry::scene::CryAssets,
    player: Query<Entity, With<LocalPlayer>>,
    mut models: Query<(Entity, &GliderModel, &mut Animator, Option<&mut GliderPlay>), Without<RemoteSide>>,
    children: Query<&Children>,
    mut stowed: Query<&mut Visibility, With<StowedPack>>,
) {
    let flying = gliding.active.is_some_and(|a| a.after.is_none());
    let Ok(owner) = player.single() else { return };
    for d in children.iter_descendants(owner) {
        if let Ok(mut v) = stowed.get_mut(d) {
            let wanted = if flying { Visibility::Hidden } else { Visibility::Inherited };
            if *v != wanted {
                *v = wanted;
            }
        }
    }
    let have = !models.is_empty();
    if !flying {
        for (entity, ..) in &models {
            commands.entity(entity).despawn();
        }
        return;
    }
    let Some(active) = gliding.active else { return };
    if !have {
        let model = inventory
            .as_deref()
            .and_then(|i| i.equipment.get(slot::BACKPACK))
            .map(|i| i.template_id)
            .and_then(|t| db.as_deref()?.glider(t))
            .and_then(|g| g.model);
        let Some(path) = model else { return };
        let Some(root) = super::creature::spawn(&mut commands, &mut cry, &library, &path, None, Transform::IDENTITY) else {
            debug!("glide: cannot draw the glider {path}");
            return;
        };
        commands.entity(root).insert((GliderModel, GliderPlay { cal: glider_cal(&path), playing: None }, ChildOf(owner)));
        return;
    }
    for (_, _, mut animator, play) in &mut models {
        let Some(mut play) = play else { continue };
        if play.playing == Some(active.pose) {
            continue;
        }
        let (name, looped) = active.pose.glider_clip();
        let clip = play.cal.as_ref().and_then(|cal| bind_clip(&animator.rig, cal, &library, name));
        if let Some(clip) = clip {
            animator.play(clip, looped, 0.15);
            animator.random_idle = None;
        }
        play.playing = Some(active.pose);
    }
}

/// The glider model's animation names and what it last played.
#[derive(Component)]
pub struct GliderPlay {
    cal: Option<Arc<Cal>>,
    playing: Option<Pose>,
}

/// The `.cal` of a glider model (`.chr` or the `.cdf` that names one): the clips are the `.chr`'s own.
fn glider_cal(path: &str) -> Option<Arc<Cal>> {
    let chr = if path.ends_with(".cdf") { aa_anim::Cdf::load(path).ok()?.model } else { path.to_string() };
    Cal::shared(&format!("{}.cal", chr.trim_end_matches(".chr"))).ok()
}

// ------------------------------------------------------------------------------------------------- other players

/// Marks a glider model that belongs to another player's character.
#[derive(Component)]
pub struct RemoteSide;

/// Another player's character under a gliding buff: the server's buff list says it flies (the movement packets carry stance
/// `FLY` and no ground flag, which `units` already keeps from drawing it as a runner in the air). The keys are not on the wire, so
/// the pose comes from its speed against the buff's normal move speed.
#[derive(Component)]
pub struct RemoteGlide {
    pub buff_id: u32,
    /// Seconds since the glider was seen opening.
    pub age: f32,
    /// Ground speed it moves at (m/s).
    pub speed: f32,
    pub pose: Pose,
    glider: Option<Entity>,
    playing: Option<Pose>,
    bound: HashMap<&'static str, Option<BoundClip>>,
}

/// The pose of another player's glider: the opening, then fast / slow against the normal speed.
pub fn remote_pose(params: &GlideParams, age: f32, speed: f32) -> Pose {
    let [slow, normal, fast] = params.move_speed;
    if age < params.startup_time {
        Pose::Start
    } else if speed > normal + 0.5 * (fast - normal).max(0.5) {
        Pose::Front
    } else if speed < normal - 0.5 * (normal - slow).max(0.5) {
        Pose::Back
    } else {
        Pose::Idle
    }
}

/// Follows the buff lists of the drawn characters: one under a gliding buff gets a [`RemoteGlide`] (its [`Motion`] is emptied, so
/// the locomotion does not run it through the air), loses it with the buff, and its stowed glider is hidden meanwhile.
#[allow(clippy::type_complexity)]
pub fn track_remote_gliders(
    time: Res<Time>,
    buffs: Option<Res<Buffs>>,
    mut gliding: ResMut<Gliding>,
    mut commands: Commands,
    children: Query<&Children>,
    mut packs: Query<&mut Visibility, With<StowedPack>>,
    mut units: Query<(Entity, &super::units::RemoteUnit, &mut Motion, Option<&mut Locomotion>, Option<&mut RemoteGlide>)>,
) {
    let Some(buffs) = buffs else { return };
    let dt = time.delta_secs().min(0.1);
    for (entity, unit, mut motion, loco, glide) in &mut units {
        if !unit.is_character() {
            continue;
        }
        let found = buffs.of(unit.obj_id).iter().find(|e| gliding.params_of(e.buff.buff_id).is_some()).map(|e| e.buff.buff_id);
        let mut set_packs = |flying: bool| {
            for d in children.iter_descendants(entity) {
                if let Ok(mut v) = packs.get_mut(d) {
                    let wanted = if flying { Visibility::Hidden } else { Visibility::Inherited };
                    if *v != wanted {
                        *v = wanted;
                    }
                }
            }
        };
        match (found, glide) {
            (Some(buff_id), None) => {
                commands.entity(entity).insert(RemoteGlide { buff_id, age: 0.0, speed: motion.speed, pose: Pose::Start, glider: None, playing: None, bound: HashMap::new() });
            }
            (Some(buff_id), Some(mut g)) => {
                // the speed of the move before it was emptied
                if motion.speed > 0.0 {
                    g.speed = motion.speed;
                }
                g.buff_id = buff_id;
                g.age += dt;
                if let Some(params) = gliding.params_of(buff_id) {
                    g.pose = remote_pose(&params, g.age, g.speed);
                }
                *motion = Motion::default();
                set_packs(true);
            }
            (None, Some(g)) => {
                if let Some(glider) = g.glider.filter(|g| *g != Entity::PLACEHOLDER) {
                    commands.entity(glider).despawn();
                }
                commands.entity(entity).remove::<RemoteGlide>();
                if let Some(mut loco) = loco {
                    loco.invalidate();
                }
                set_packs(false);
            }
            (None, None) => {}
        }
    }
}

/// Draws the glider of another player's character and plays the gliding clips on both.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn draw_remote_gliders(
    library: Res<ClipLibrary>,
    units: Option<Res<aa_net::state::Units>>,
    db: Option<Res<Db>>,
    mut commands: Commands,
    mut cry: aa_cry::scene::CryAssets,
    mut flyers: Query<(Entity, &super::units::RemoteUnit, &mut RemoteGlide, &mut Animator, Option<&Locomotion>), Without<GliderModel>>,
    mut gliders: Query<(&mut Animator, &mut GliderPlay), With<GliderModel>>,
) {
    for (entity, unit, mut glide, mut animator, loco) in &mut flyers {
        if glide.glider.is_none() {
            let model = units
                .as_deref()
                .and_then(|u| u.get(unit.obj_id))
                .and_then(|u| u.equipment.get(slot::BACKPACK))
                .map(|i| i.template_id)
                .and_then(|t| db.as_deref()?.glider(t))
                .and_then(|g| g.model);
            if let Some(path) = model
                && let Some(root) = super::creature::spawn(&mut commands, &mut cry, &library, &path, None, Transform::IDENTITY)
            {
                commands.entity(root).insert((GliderModel, RemoteSide, GliderPlay { cal: glider_cal(&path), playing: None }, ChildOf(entity)));
                glide.glider = Some(root);
            } else {
                // no glider known for it: the character still glides
                glide.glider = Some(Entity::PLACEHOLDER);
            }
        }
        if glide.playing == Some(glide.pose) {
            continue;
        }
        let pose = glide.pose;
        if let Some(loco) = loco {
            let (name, looped) = pose.character_clip();
            let g = &mut *glide;
            let clip = g.bound.entry(name).or_insert_with(|| bind_clip(&animator.rig, &loco.clips.cal, &library, name)).clone();
            if let Some(clip) = clip {
                animator.play(clip, looped, 0.25);
                animator.random_idle = None;
            }
        }
        if let Some(glider) = glide.glider.filter(|g| *g != Entity::PLACEHOLDER)
            && let Ok((mut ganim, mut play)) = gliders.get_mut(glider)
        {
            let (name, looped) = pose.glider_clip();
            if let Some(clip) = play.cal.as_ref().and_then(|cal| bind_clip(&ganim.rig, cal, &library, name)) {
                ganim.play(clip, looped, 0.15);
                ganim.random_idle = None;
            }
            play.playing = Some(pose);
        }
        glide.playing = Some(pose);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Buff 2098, Ezi's wings (move 7 / 9 / 11, fall 0.3 / 0.5 / 1.5, turn 60 deg/s, 3 lifts).
    fn ezi() -> GlideParams {
        GlideParams {
            move_speed: [7.0, 9.0, 11.0],
            fall_speed: [0.3, 0.5, 1.5],
            rotate: 60f32.to_radians(),
            startup_time: 1.5,
            startup_speed: 3.0,
            smooth_time: 0.5,
            lift_count: 3,
            lift_height: 1.5,
            lift_valid_time: 5.0,
            lift_duration: 0.5,
            lift_speed: 5.0,
            land_height: 1.0,
            sliding_time: 1.5,
        }
    }

    fn run(f: &mut Flight, seconds: f32, input: Input, height: f32) -> Step {
        let dt = 1.0 / 60.0;
        let mut last = f.step(dt, input, height);
        for _ in 1..(seconds / dt) as usize {
            last = f.step(dt, input, height);
        }
        last
    }

    #[test]
    fn the_glider_opens_at_the_startup_speed_and_settles_on_the_normal_rates() {
        let mut f = Flight::open(ezi(), -12.0, 50.0);
        assert_eq!((f.phase, f.speed, f.vz), (Phase::Open, 3.0, -12.0));
        let s = run(&mut f, 0.5, Input::default(), 50.0);
        assert_eq!(s.phase, Phase::Open);
        assert!((s.speed - 3.0).abs() < 1e-3, "the opening holds the speed it began with: {}", s.speed);
        // the fall is taken down towards the normal rate during the opening
        assert!(s.vz > -12.0 && s.vz < -0.4, "{}", s.vz);
        let s = run(&mut f, 6.0, Input::default(), 50.0);
        assert_eq!(s.phase, Phase::Gliding);
        assert!((s.speed - 9.0).abs() < 0.01, "normal speed {}", s.speed);
        assert!((s.vz + 0.5).abs() < 0.01, "normal fall {}", s.vz);
    }

    #[test]
    fn w_flies_fast_and_s_slow() {
        let mut f = Flight::open(ezi(), -1.0, 80.0);
        run(&mut f, 3.0, Input::default(), 80.0);
        let fast = run(&mut f, 4.0, Input { forward: 1.0, ..Input::default() }, 80.0);
        assert!((fast.speed - 11.0).abs() < 0.01 && (fast.vz + 1.5).abs() < 0.01, "{fast:?}");
        let slow = run(&mut f, 4.0, Input { forward: -1.0, ..Input::default() }, 80.0);
        assert!((slow.speed - 7.0).abs() < 0.01 && (slow.vz + 0.3).abs() < 0.01, "{slow:?}");
    }

    #[test]
    fn a_and_d_turn_at_the_buff_rate_and_not_while_sliding() {
        let mut f = Flight::open(ezi(), -1.0, 80.0);
        let s = run(&mut f, 2.0, Input { turn: 1.0, ..Input::default() }, 80.0);
        assert!((s.turn_rate - 60f32.to_radians()).abs() < 1e-6);
        let s = f.step(0.016, Input { turn: -1.0, ..Input::default() }, 80.0);
        assert!((s.turn_rate + 60f32.to_radians()).abs() < 1e-6);
        let s = f.step(0.016, Input { turn: -1.0, ..Input::default() }, 0.5);
        assert_eq!((s.phase, s.turn_rate), (Phase::Sliding, 0.0));
    }

    #[test]
    fn a_lift_rises_at_the_lift_speed_a_limited_number_of_times() {
        let mut f = Flight::open(ezi(), -1.0, 80.0);
        run(&mut f, 2.0, Input::default(), 80.0);
        let press = Input { lift: true, ..Input::default() };
        // lift_duration 0.5 / 3 lifts: a sixth of a second at 5 m/s each
        let s = f.step(1.0 / 60.0, press, 80.0);
        assert!(s.lifting && (s.vz - 5.0).abs() < 1e-6);
        let mut rise = 0.0;
        for _ in 0..30 {
            let s = f.step(1.0 / 60.0, Input::default(), 80.0);
            if s.vz > 0.0 {
                rise += s.vz / 60.0;
            }
        }
        assert!(rise > 0.5 && rise <= 1.5 + 0.1, "one lift rose {rise} m");
        for _ in 0..2 {
            f.step(1.0 / 60.0, press, 80.0);
            run(&mut f, 0.5, Input::default(), 80.0);
        }
        assert_eq!(f.lifts, 3);
        // the fourth press does nothing
        let s = f.step(1.0 / 60.0, press, 80.0);
        assert!(!s.lifting && s.vz < 0.0, "{s:?}");
    }

    #[test]
    fn a_lift_needs_the_opening_over_and_the_valid_time_not_run_out() {
        let mut f = Flight::open(ezi(), -1.0, 80.0);
        let press = Input { lift: true, ..Input::default() };
        assert!(!f.step(0.1, press, 80.0).lifting, "during the opening");
        let mut late = Flight::open(ezi(), -1.0, 500.0);
        run(&mut late, 1.5 + 5.5, Input::default(), 500.0);
        assert!(!late.step(0.016, press, 500.0).lifting, "after lift_valid_time");
        // a glider without lifts (Thunderbolt's, lift_count 0)
        let mut none = Flight::open(GlideParams { lift_count: 0, ..ezi() }, -1.0, 80.0);
        run(&mut none, 2.0, Input::default(), 80.0);
        assert!(!none.step(0.016, press, 80.0).lifting);
    }

    #[test]
    fn near_the_ground_the_glider_slides_and_the_flight_ends_after_sliding_time() {
        let mut f = Flight::open(ezi(), -1.0, 80.0);
        run(&mut f, 3.0, Input::default(), 80.0);
        let speed = f.speed;
        // within land_height: the slide begins with the speed it had and runs out over sliding_time (1.5 s)
        let s = f.step(1.0 / 60.0, Input::default(), 0.9);
        assert_eq!(s.phase, Phase::Sliding);
        assert!(!s.done && s.speed <= speed && s.speed > speed * 0.95);
        let mut last = s;
        let mut t = 0.0;
        while !last.done && t < 3.0 {
            last = f.step(1.0 / 60.0, Input::default(), 0.0);
            t += 1.0 / 60.0;
        }
        assert!(last.done && (t - 1.5f32).abs() < 0.05, "the slide lasted {t} s");
        assert_eq!(last.speed, 0.0);
    }

    #[test]
    fn without_a_slide_the_ground_ends_the_flight() {
        // Dragon wings (buff 2097): no lift, sliding_time 1.0 here, so use a row without one
        let p = GlideParams { sliding_time: 0.0, land_height: 0.0, ..ezi() };
        let mut f = Flight::open(p, -1.0, 50.0);
        run(&mut f, 2.0, Input::default(), 50.0);
        assert!(!f.step(0.016, Input::default(), 0.5).done);
        assert!(f.step(0.016, Input::default(), 0.0).done);
    }

    #[test]
    fn opened_on_the_ground_it_slides_at_once() {
        let f = Flight::open(ezi(), 0.0, 0.0);
        assert_eq!(f.phase, Phase::Sliding);
        // a rising character is not on the ground yet
        assert_eq!(Flight::open(ezi(), 5.0, 0.0).phase, Phase::Open);
        // nor one far above it
        assert_eq!(Flight::open(ezi(), -3.0, 40.0).phase, Phase::Open);
    }

    #[test]
    fn the_glider_comes_down_at_the_normal_rate_over_a_long_flight() {
        let mut f = Flight::open(ezi(), -1.0, 300.0);
        let (mut z, mut x) = (300.0f32, 0.0f32);
        for _ in 0..(20 * 60) {
            let s = f.step(1.0 / 60.0, Input::default(), z);
            z += s.vz / 60.0;
            x += s.speed / 60.0;
        }
        // 20 s: about 0.5 m/s down and 9 m/s forward after the opening
        assert!((300.0 - z - 0.5 * 20.0).abs() < 2.5, "fell {}", 300.0 - z);
        assert!((x - 9.0 * 20.0).abs() < 12.0, "flew {x}");
    }

    #[test]
    fn keys_pick_the_speed_set_and_the_pose() {
        assert_eq!((speed_index(1.0), speed_index(0.0), speed_index(-1.0)), (2, 1, 0));
        let g = |forward, turn| pose(Phase::Gliding, Input { forward, turn, lift: false }, false);
        assert_eq!(g(0.0, 0.0), Pose::Idle);
        assert_eq!(g(1.0, 0.0), Pose::Front);
        assert_eq!(g(-1.0, 0.0), Pose::Back);
        assert_eq!((g(0.0, 1.0), g(0.0, -1.0)), (Pose::Left, Pose::Right));
        assert_eq!((g(1.0, 1.0), g(1.0, -1.0)), (Pose::FrontLeft, Pose::FrontRight));
        assert_eq!((g(-1.0, 1.0), g(-1.0, -1.0)), (Pose::BackLeft, Pose::BackRight));
        assert_eq!(pose(Phase::Gliding, Input::default(), true), Pose::Lift);
        assert_eq!(pose(Phase::Open, Input::default(), false), Pose::Start);
        assert_eq!(pose(Phase::Sliding, Input::default(), false), Pose::Slide);
        assert_eq!(Pose::FrontLeft.character_clip().0, "fist_ac_gliding_fl");
        assert_eq!(Pose::BackRight.glider_clip().0, "right");
    }

    #[test]
    fn a_second_jump_opens_the_glider_only_high_in_the_air_with_the_option_on() {
        assert!(may_deploy(true, true, false, false, 4.0, false));
        assert!(may_deploy(true, true, false, true, 1.0, false), "a freefall counts whatever the height");
        assert!(!may_deploy(true, true, false, false, 1.3, false), "the top of a plain jump is too low");
        assert!(!may_deploy(false, true, false, false, 9.0, false), "option off");
        assert!(!may_deploy(true, false, false, false, 9.0, false), "on the ground");
        assert!(!may_deploy(true, true, true, false, 9.0, false), "swimming");
        assert!(!may_deploy(true, true, false, false, 9.0, true), "already flying");
        assert!(!double_jump_on(None) && double_jump_on(Some(1.0)) && !double_jump_on(Some(0.0)));
    }

    #[derive(Resource, Default)]
    struct Sent(Vec<MoveCommand>);

    fn collect(mut reader: MessageReader<NetCommand>, mut sent: ResMut<Sent>) {
        for NetCommand(cmd) in reader.read() {
            if let Command::Move(m) = cmd {
                sent.0.push(*m);
            }
        }
    }

    /// The controller over the real terrain: the glider comes down at its rate, flies along the facing, sends the FLY stance
    /// without a ground flag, slides near the ground and ends with one landing packet.
    #[test]
    fn a_flight_over_the_terrain_descends_flies_slides_and_lands_with_one_packet() {
        if !aa_world::pak_available() {
            return;
        }
        let terrain = TerrainQuery::new("main_world");
        let collision = WorldCollision::new("main_world", terrain.clone());
        let water = WaterQuery::new("main_world");
        let (x, y) = (21188.25f32, 9030.5f32);
        let ground = terrain.height_at(x, y).expect("terrain");
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .insert_resource(terrain)
            .insert_resource(collision)
            .insert_resource(water)
            .init_resource::<SendState>()
            .init_resource::<PlayerInput>()
            .init_resource::<Gliding>()
            .init_resource::<Sent>()
            .init_resource::<crate::settings::Actions>()
            .init_resource::<DevOptions>()
            .add_message::<NetCommand>()
            .add_systems(Update, (control_glide, collect).chain());
        let start = Vec3::new(x, y, ground + 12.0);
        app.world_mut().spawn((LocalPlayer { obj_id: 1 }, Mover { pos: start, facing: 0.0, vz: -1.0, grounded: false, velocity: Vec3::ZERO, swim: None, wading: 0.0 }, Transform::default(), Motion::default()));
        app.world_mut().spawn(OrbitCamera::default());
        let params = GlideParams { sliding_time: 1.0, land_height: 1.0, ..ezi() };
        app.world_mut().resource_mut::<Gliding>().active =
            Some(Active { flight: Flight::open(params, -1.0, 12.0), buff_id: 2098, buff_index: 1, pose: Pose::Start, missing: 0.0, after: None });
        app.world_mut().resource_mut::<SendState>().advance_clock(0.0);
        let mut landed_at = None;
        for frame in 0..(40 * 30) {
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_millis(33));
            app.update();
            if landed_at.is_none() && !app.world().resource::<Gliding>().flying() {
                landed_at = Some(frame);
                break;
            }
        }
        let landed = landed_at.expect("the glider lands");
        let mover = app.world_mut().query::<&Mover>().single(app.world()).expect("player");
        assert!(mover.grounded && mover.vz == 0.0);
        assert!((mover.pos.z - app.world().resource::<TerrainQuery>().height_at(mover.pos.x, mover.pos.y).unwrap()).abs() < 2.0, "down on the ground");
        // it flew north, along the facing 0: some metres, as the normal speed and the time say (no wall on this stretch is assumed)
        assert!(mover.pos.y > y + 5.0, "flew {}", mover.pos.y - y);
        let sent = &app.world().resource::<Sent>().0;
        assert!(sent.len() >= 5, "{} packets in {:.1} s", sent.len(), landed as f32 * 0.033);
        let (last, flying) = sent.split_last().unwrap();
        assert!(flying.iter().all(|m| m.stance == stance::FLY && m.state == MoveState::Fall && m.fall_vel == 0), "airborne packets");
        assert!(flying.iter().any(|m| m.velocity[2] < 0), "the descent is in the velocity");
        assert_eq!((last.state, last.stance, last.moving), (MoveState::Ground, stance::RELAXED, false));
        assert!(last.fall_vel < 3000, "a glider lands softly: {}", last.fall_vel);
    }

    #[test]
    fn another_players_pose_follows_its_speed() {
        let p = ezi();
        assert_eq!(remote_pose(&p, 0.5, 9.0), Pose::Start);
        assert_eq!(remote_pose(&p, 3.0, 9.0), Pose::Idle);
        assert_eq!(remote_pose(&p, 3.0, 11.0), Pose::Front);
        assert_eq!(remote_pose(&p, 3.0, 7.0), Pose::Back);
    }

    /// A drawn character whose buff list gets a gliding buff is a glider pilot, its stowed glider hides and its motion is emptied;
    /// the buff's removal (the landing) puts everything back.
    #[test]
    fn a_remote_character_glides_while_its_buff_lasts() {
        use aa_net::state::BuffEntry;
        if GlideParams::load_all().is_empty() {
            return;
        }
        let mut app = App::new();
        app.insert_resource(Time::<()>::default()).init_resource::<Gliding>().init_resource::<Buffs>().add_systems(Update, track_remote_gliders);
        let unit = app.world_mut().spawn((super::super::units::RemoteUnit::test_character(7), Motion { speed: 9.0, local_dir: Vec2::Y, ..default() })).id();
        let pack = app.world_mut().spawn((StowedPack, Visibility::Inherited, ChildOf(unit))).id();
        let step = |app: &mut App| {
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_millis(33));
            app.update();
        };
        step(&mut app);
        assert!(app.world().get::<RemoteGlide>(unit).is_none(), "no buff, no glider");
        let buff = aa_net::buff::Buff { buff_id: 2098, index: 3, ..Default::default() };
        app.world_mut().resource_mut::<Buffs>().by_unit.insert(7, vec![BuffEntry { buff, received: std::time::Instant::now() }]);
        step(&mut app);
        step(&mut app);
        let g = app.world().get::<RemoteGlide>(unit).expect("gliding");
        assert_eq!((g.buff_id, g.pose), (2098, Pose::Start));
        assert_eq!(app.world().get::<Motion>(unit).unwrap().speed, 0.0, "not run through the air");
        assert_eq!(*app.world().get::<Visibility>(pack).unwrap(), Visibility::Hidden);
        app.world_mut().resource_mut::<Buffs>().by_unit.insert(7, Vec::new());
        step(&mut app);
        assert!(app.world().get::<RemoteGlide>(unit).is_none(), "landed");
        assert_eq!(*app.world().get::<Visibility>(pack).unwrap(), Visibility::Inherited);
    }

    /// The database's gliding buffs are the ones the rules were read for (needs `AA_DB`).
    #[test]
    fn the_database_rows_become_flights() {
        let all = GlideParams::load_all();
        if all.is_empty() {
            eprintln!("no game database: skipped");
            return;
        }
        assert_eq!(all.len(), 55);
        // Ezi's wings and the Moonshadow wings
        assert_eq!(all[&2098], ezi());
        let moon = all[&3583];
        assert_eq!((moon.lift_count, moon.lift_height, moon.lift_speed, moon.lift_duration), (5, 3.0, 5.0, 1.0));
        assert_eq!(moon.move_speed, [6.0, 7.0, 8.0]);
        // every row can fly: positive speeds and a fall
        for (id, p) in &all {
            assert!(p.move_speed.iter().all(|s| *s > 0.0 || *id == 7206 || *id == 7387), "{id}: {p:?}");
        }
        // the lift of Fly Glider (1957) uses the defaults' two lifts; its fall at the fast key is 5 m/s
        assert_eq!(all[&1957].fall_speed, [1.0, 5.0, 3.0]);
    }
}
