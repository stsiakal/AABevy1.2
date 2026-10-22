//! Ladders and ropes: the use trigger, the climb state, the climb and exit clips, and the hanging
//! movement packet (`FUN_390c1330`, `FUN_391388f0`, `FUN_39135570`, `FUN_39137470`, `FUN_39132be0`).
//!
//! **Trigger.** 1.2 finds the ladder under the use trace and calls `Player::GrabOnLadder` with reason
//! 10 (`FUN_390c1330`): it stores the ladder object id and kind, copies the ladder frame, sets
//! `isOnLadder` and feeds the `isOnLadder` graph input (`FUN_39136dc0:50`). Here F
//! (`do_interaction_1`) grabs the nearest climb doodad within [`REACH`] metres: a doodad whose
//! current phase has a `DoodadFuncClimb` row, whose `doodad_func_climbs.climb_type_id` is the
//! client's kind (1/6 ladder, 2 rope, 3 wood, 4 net, 5 hanging rope, 7 mast, 8 guardrail,
//! 9 45-degree ladder, 10 big tree). The ladder's model bounds give the bottom and top; without them
//! a [`FALLBACK_HEIGHT`] ladder is used.
//!
//! **Climb.** W/S move up/down at [`CLIMB_SPEED`] times the own buffs' `ladder_speed_mul` (unit
//! attribute 0x5d, [`super::sprint::Sprint::ladder_speed_mul`]); A/D and the mouse do not move the
//! body. The clips are `fist_mo_<family>_up/down/idle` bound by path (the `.cal` only names the exit
//! clips: `fist_mo_ladder_end`, `_end_45`, `_end_70`, `_end_80`, `fist_mo_rope_end`). Reaching the
//! top or bottom, or pressing F, plays an exit clip; at the top the body steps onto the platform,
//! at the bottom it stands again. Space jumps off.
//!
//! **Packet.** Every hanging movement carries actor flag 0x40, the ladder's gc id and the 32-bit
//! climb data (`aa-net` writes the block in `UnitMove.Read` order). 1.2 uses the crouch stance for
//! climbing (`basicactor.lua:821`).

use std::sync::Arc;

use aa_anim::pose::{BoundClip, Rig, SampleOptions};
use aa_anim::{Animator, Cal, ClipLibrary};
use aa_net::model::{MoveSupport, stance};
use aa_net::state::{Doodads, Player};
use aa_net::{MoveCommand, MoveState, NetCommand, Pos};
use bevy::prelude::*;

use super::anim::{Locomotion, Motion};
use super::combat::{Acting, Dead};
use super::data::Db;
use super::player::{self, LocalPlayer, Mover, PlayerInput, SendState, forward};
use super::sprint::Sprint;

/// How far a ladder can be used from (metres), the doodad use reach of the original trace.
pub const REACH: f32 = 4.0;
/// The distance the feet are held off the ladder's centre (the capsule radius, metres).
pub const STANDOFF: f32 = 0.35;
/// Climb speed without buffs (m/s); the clips are paced to it.
pub const CLIMB_SPEED: f32 = 1.5;
/// A ladder whose model bounds cannot be read (or is not in the pak) is this tall (metres).
pub const FALLBACK_HEIGHT: f32 = 4.0;
/// Distance the body steps over the top on the way out (metres).
pub const TOP_STEP: f32 = 0.6;
/// The exit film the top plays for each ladder height band, after `FUN_3916c280`: short, 70-degree,
/// then the tall/80-degree one.
const EXIT_70_FROM: f32 = 3.5;
const EXIT_80_FROM: f32 = 7.0;

/// The clip family of a climb kind (`FUN_39132be0`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Ladder,
    Wood,
    Rope,
    Net,
    Mast,
}

impl Family {
    pub fn of(kind: u8) -> Family {
        match kind {
            2 | 5 => Family::Rope,
            3 => Family::Wood,
            4 => Family::Net,
            7 | 8 | 10 => Family::Mast,
            _ => Family::Ladder,
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            Family::Ladder => "ladder",
            Family::Wood => "climb",
            Family::Rope => "rope",
            Family::Net => "prope",
            Family::Mast => "mast",
        }
    }
}

/// The ladder the local player hangs on.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Ladder {
    pub obj_id: u32,
    /// `doodad_func_climbs.climb_type_id` (see the module documentation).
    pub kind: u8,
    /// Bottom centre and top height (world metres).
    pub bottom: Vec3,
    pub top: f32,
    /// The heading facing the ladder.
    pub facing: f32,
    /// An exit clip runs; no input moves the body until it ends.
    pub exiting: Option<Exit>,
}

/// An exit in progress: the seconds left and whether it leaves at the top.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Exit {
    pub left: f32,
    pub top: bool,
}

impl Ladder {
    fn height(&self, z: f32) -> f32 {
        (z - self.bottom.z).max(0.0)
    }

    /// The 32-bit `actor.climbData` of the movement: the body's height along the ladder in
    /// millimetres. 1.2's writer was not located, so this is our encoding of the same quantity
    /// (the server keeps the field but does not read it).
    fn climb_data(&self, z: f32) -> u32 {
        (self.height(z) * 1000.0).round().clamp(0.0, u32::MAX as f32) as u32
    }
}

/// The origin and top of a ladder doodad, from its model's bounds (`bbox`, CryEngine Z up) scaled by
/// the doodad's scale. `origin` is the doodad's world position; `height` the fallback when the model
/// cannot be read.
pub fn extent(bounds: Option<(Vec3, Vec3)>, origin: Vec3, scale: f32, fallback: f32) -> (f32, f32) {
    let scale = if scale > 0.0 { scale } else { 1.0 };
    match bounds {
        Some((min, max)) if max.z > min.z => (origin.z + min.z * scale, origin.z + max.z * scale),
        _ => (origin.z, origin.z + fallback),
    }
}

/// The bounds of a doodad's model, if the pak has it.
pub fn model_bounds(path: Option<&str>) -> Option<(Vec3, Vec3)> {
    let path = path?;
    aa_cry::Model::load(path).ok()?.bounds()
}

/// The nearest climbable doodad in reach: `(obj_id, kind, origin)`. `pos` is the player's feet.
pub fn nearest(pos: Vec3, reach: f32, doodads: impl IntoIterator<Item = (u32, Vec3, u8)>) -> Option<(u32, u8, Vec3)> {
    doodads
        .into_iter()
        .filter_map(|(id, origin, kind)| {
            let d = Vec2::new(origin.x - pos.x, origin.y - pos.y).length();
            (d <= reach).then_some((id, kind, origin, d))
        })
        .min_by(|a, b| a.3.total_cmp(&b.3))
        .map(|(id, kind, origin, _)| (id, kind, origin))
}

/// The heading that faces `to` from `pos` (0 faces north, counter-clockwise).
pub fn yaw_toward(pos: Vec3, to: Vec3) -> Option<f32> {
    let d = Vec2::new(to.x - pos.x, to.y - pos.y);
    (d.length() > 0.05).then(|| (-d.x).atan2(d.y))
}

/// The climb clips of one model family.
#[derive(Clone)]
pub struct FamilyClips {
    pub up: Option<BoundClip>,
    pub down: Option<BoundClip>,
    pub idle: Option<BoundClip>,
    /// Ground speed (m/s) the forward up clip covers per second of playback.
    pub speed: f32,
}

/// The climb and exit clips of the player's model. The `.cal` names only the exit clips, so the
/// rest are bound by path from the folder of the model's fist clips.
#[derive(Component, Clone)]
pub struct LadderClips {
    ladder: FamilyClips,
    wood: FamilyClips,
    rope: FamilyClips,
    net: FamilyClips,
    mast: FamilyClips,
    ladder_up_45: Option<BoundClip>,
    rope_front: Option<BoundClip>,
    net_front: Option<BoundClip>,
    exit_ladder: Option<BoundClip>,
    exit_45: Option<BoundClip>,
    exit_70: Option<BoundClip>,
    exit_80: Option<BoundClip>,
    exit_rope: Option<BoundClip>,
}

impl LadderClips {
    /// Binds the climb clips of the model whose skeleton and `.cal` are given; `None` when the pak
    /// cannot bind any of them.
    pub fn load(skeleton: &str, cal: &str, clips: &ClipLibrary) -> Option<LadderClips> {
        let rig = aa_anim::pose::shared_rig(skeleton).ok()?;
        let cal = Cal::shared(cal).ok()?;
        let folder = cal
            .animation("fist_ba_relaxed_idle")
            .or_else(|| cal.animations.values().find(|e| e.path.contains("/fist/")))
            .or_else(|| cal.animations.values().next())?
            .path
            .rsplit_once('/')?
            .0
            .to_string();
        let bind = |name: &str| bind_path(&rig, clips, &folder, name);
        let family = |f: Family| FamilyClips::load(&rig, clips, &folder, f);
        let mut set = LadderClips {
            ladder: family(Family::Ladder),
            wood: family(Family::Wood),
            rope: family(Family::Rope),
            net: family(Family::Net),
            mast: family(Family::Mast),
            ladder_up_45: bind("fist_mo_ladder_up_45"),
            rope_front: bind("fist_mo_rope_front_clip"),
            net_front: bind("fist_mo_prope_frontward_idle"),
            exit_ladder: bind("fist_mo_ladder_end"),
            exit_45: bind("fist_mo_ladder_end_45"),
            exit_70: bind("fist_mo_ladder_end_70"),
            exit_80: bind("fist_mo_ladder_end_80"),
            exit_rope: bind("fist_mo_rope_end"),
        };
        // a family without its own clips (the pak or the model lacks them) climbs like a ladder
        for f in [&mut set.wood, &mut set.rope, &mut set.net, &mut set.mast] {
            if f.idle.is_none() {
                *f = set.ladder.clone();
            }
        }
        set.ladder.idle.as_ref()?;
        Some(set)
    }

    fn family(&self, kind: u8) -> &FamilyClips {
        match Family::of(kind) {
            Family::Ladder => &self.ladder,
            Family::Wood => &self.wood,
            Family::Rope => &self.rope,
            Family::Net => &self.net,
            Family::Mast => &self.mast,
        }
    }

    /// The up clip of a kind (the 45-degree ladder has its own when the model has one).
    fn up(&self, kind: u8) -> Option<&BoundClip> {
        if kind == 9 && self.ladder_up_45.is_some() {
            self.ladder_up_45.as_ref()
        } else {
            self.family(kind).up.as_ref()
        }
    }

    /// The standing clip on the ladder (the hanging rope and the net have their own).
    fn idle(&self, kind: u8) -> Option<&BoundClip> {
        match kind {
            5 => self.rope_front.as_ref().or(self.rope.idle.as_ref()),
            4 => self.net_front.as_ref().or(self.net.idle.as_ref()),
            _ => self.family(kind).idle.as_ref(),
        }
    }

    /// The exit clip for a kind and a height along the ladder.
    fn exit(&self, kind: u8, height: f32) -> Option<&BoundClip> {
        match kind {
            9 => self.exit_45.as_ref().or(self.exit_ladder.as_ref()),
            1 | 6 => {
                if height >= EXIT_80_FROM {
                    self.exit_80.as_ref().or(self.exit_ladder.as_ref())
                } else if height >= EXIT_70_FROM {
                    self.exit_70.as_ref().or(self.exit_ladder.as_ref())
                } else {
                    self.exit_ladder.as_ref()
                }
            }
            2 | 4 | 5 => self.exit_rope.as_ref().or(self.exit_ladder.as_ref()),
            _ => self.exit_70.as_ref().or(self.exit_ladder.as_ref()),
        }
    }

    /// Seconds the exit clip runs (0 without clips: the body leaves at once).
    fn exit_seconds(&self, kind: u8, height: f32) -> f32 {
        self.exit(kind, height).map_or(0.0, |c| c.clip.duration)
    }
}

impl FamilyClips {
    fn load(rig: &Rig, clips: &ClipLibrary, folder: &str, family: Family) -> FamilyClips {
        let bind = |suffix: &str| bind_path(rig, clips, folder, &format!("fist_mo_{}_{suffix}", family.prefix()));
        let up = bind("up");
        let down = bind("down").or_else(|| bind_path(rig, clips, folder, "fist_mo_ladder_down"));
        let idle = bind("idle");
        let speed = up.as_ref().map_or(0.0, |c| clip_speed(rig, c));
        FamilyClips { up, down, idle, speed }
    }
}

/// Binds `<folder>/<name>.caf`; the `.cal` does not name the climb clips.
fn bind_path(rig: &Rig, clips: &ClipLibrary, folder: &str, name: &str) -> Option<BoundClip> {
    let path = format!("{folder}/{name}.caf");
    let clip = clips.bound(rig, &path, SampleOptions::default()).ok()?;
    super::anim::remember_file(&clip, &path);
    Some(clip)
}

/// Metres per second the root bone travels in a clip.
fn clip_speed(rig: &Rig, clip: &BoundClip) -> f32 {
    let root = rig.skeleton.bones.get(rig.root).map(|b| b.controller_id);
    let Some(track) = root.and_then(|c| clip.clip.track(c)) else { return 0.0 };
    let (Some(a), Some(b)) = (track.sample_pos(0.0), track.sample_pos(clip.clip.duration)) else { return 0.0 };
    let travelled = (b - a).length() * clip.options.position_scale;
    if clip.clip.duration > 0.0 { travelled / clip.clip.duration } else { 0.0 }
}

/// The result of one climb step: the new feet height and whether an exit began.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    pub z: f32,
    /// The exit playing: `Some(true)` at the top, `Some(false)` at the bottom.
    pub exit: Option<bool>,
}

/// W/S movement along a ladder of `[bottom, top]`: `up` is -1, 0 or 1 and `speed` m/s. A body that
/// runs into the bottom or top starts the exit (top) or stands again (bottom).
pub fn step(z: f32, bottom: f32, top: f32, up: f32, speed: f32, dt: f32) -> Step {
    let next = (z + up * speed * dt).clamp(bottom.min(top), top.max(bottom));
    let hitting = if up > 0.0 && next >= top {
        Some(true)
    } else if up < 0.0 && next <= bottom {
        Some(false)
    } else {
        None
    };
    Step { z: next, exit: hitting }
}

/// Grabs a climb doodad when F is pressed on foot, and while on a ladder climbs, leaves and sends the
/// hanging movement packets. Runs before [`super::player::control_player`], which stands down while
/// the player has a [`Ladder`].
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn control_ladder(
    mut commands: Commands,
    time: Res<Time>,
    actions: Res<crate::settings::Actions>,
    typing: Option<Res<crate::hud::HudKeyboardFocus>>,
    mut player: Query<(Entity, &LocalPlayer, &mut Mover, &mut Transform, &mut Motion, Option<&Ladder>, Option<&LadderClips>, Has<Dead>)>,
    net_player: Res<Player>,
    doodads: Res<Doodads>,
    db: Option<Res<Db>>,
    terrain: Option<Res<aa_world::TerrainQuery>>,
    collision: Option<Res<aa_world::WorldCollision>>,
    sprint: Option<Res<Sprint>>,
    mut out: MessageWriter<NetCommand>,
    mut state: ResMut<SendState>,
    mut input_state: ResMut<PlayerInput>,
) {
    let Ok((entity, _local, mut mover, mut transform, mut motion, ladder, clips, dead)) = player.single_mut() else { return };
    if dead || !player::on_foot_control(&net_player) {
        if ladder.is_some() {
            commands.entity(entity).remove::<Ladder>();
            state.after_ladder();
        }
        return;
    }
    let typing = typing.is_some_and(|f| f.0);
    let dt = time.delta_secs().min(0.1);
    let used = !typing && actions.just_pressed("do_interaction_1", 0);
    let jump = !typing && actions.just_pressed("jump", 0);
    let up = if typing { 0.0 } else { actions.pressed("moveforward", 0) as i32 as f32 - actions.pressed("moveback", 0) as i32 as f32 };
    let speed = CLIMB_SPEED * sprint.as_ref().map_or(1.0, |s| s.ladder_speed_mul);

    let Some(ladder) = ladder else {
        // grab: the nearest climb doodad in reach, when its current phase is a climb
        if !used || mover.swim.is_some() {
            return;
        }
        let Some(db) = db else { return };
        let candidates: Vec<(u32, Vec3, u8, u32, u32, f32)> = doodads
            .by_id
            .values()
            .filter_map(|d| {
                let origin = Vec3::new(d.pos.x, d.pos.y, d.pos.z);
                db.ladder_kind(d.func_group_id).map(|kind| (d.obj_id, origin, kind, d.template_id, d.func_group_id, d.scale))
            })
            .collect();
        let Some((obj_id, kind, origin)) = nearest(mover.pos, REACH, candidates.iter().map(|c| (c.0, c.1, c.2))) else { return };
        let (_, _, _, template, group, scale) = candidates.into_iter().find(|c| c.0 == obj_id).expect("chosen from the candidates");
        let facing = yaw_toward(mover.pos, origin).unwrap_or(mover.facing);
        let bounds = model_bounds(db.doodad_phase_model(template, group).as_deref());
        let (bottom_z, top) = extent(bounds, origin, scale, FALLBACK_HEIGHT);
        let bottom = Vec3::new(origin.x, origin.y, bottom_z);
        let forward = forward(facing);
        let z = mover.pos.z.clamp(bottom_z, top);
        mover.pos = Vec3::new(origin.x - forward.x * STANDOFF, origin.y - forward.y * STANDOFF, z);
        mover.facing = facing;
        mover.vz = 0.0;
        mover.grounded = false;
        mover.velocity = Vec3::ZERO;
        mover.swim = None;
        *transform = player::placement(mover.pos, facing, 1.0);
        let ladder = Ladder { obj_id, kind, bottom, top, facing, exiting: None };
        commands.entity(entity).insert(ladder.clone());
        state.after_ladder();
        send(&mut out, &mover, Some(&ladder), 0.0, true);
        *motion = Motion { speed: 0.0, local_dir: Vec2::ZERO, airborne: false, swimming: false, auto: false, request: None, on_ladder: true };
        *input_state = PlayerInput::default();
        info!("ladder: grabbed doodad {obj_id} kind {kind}, bottom {bottom_z:.2} top {top:.2}");
        return;
    };

    let mut ladder = ladder.clone();
    let mut pos = mover.pos;
    // an exit clip runs to its end
    if let Some(exit) = ladder.exiting {
        let left = exit.left - dt;
        if left <= 0.0 {
            finish_exit(&mut commands, entity, &mut mover, &mut motion, &mut state, exit.top, terrain.as_deref(), collision.as_deref());
            *transform = player::placement(mover.pos, mover.facing, 1.0);
            return;
        }
        ladder.exiting = Some(Exit { left, ..exit });
        commands.entity(entity).insert(ladder.clone());
        *motion = Motion { speed: 0.0, local_dir: Vec2::ZERO, airborne: false, swimming: false, auto: false, request: None, on_ladder: true };
        send(&mut out, &mover, Some(&ladder), 0.0, false);
        return;
    }
    // Space jumps off; F plays the exit
    if jump {
        mover.grounded = false;
        mover.vz = 2.0;
        commands.entity(entity).remove::<Ladder>();
        state.after_ladder();
        *motion = Motion::default();
        send(&mut out, &mover, None, 0.0, false);
        return;
    }
    if used {
        begin_exit(&mut commands, entity, &ladder, &mover, &mut motion, clips, false);
        send(&mut out, &mover, Some(&ladder), 0.0, false);
        return;
    }
    let step = step(pos.z, ladder.bottom.z, ladder.top, up, speed, dt);
    pos.z = step.z;
    pos.x = ladder.bottom.x - forward(ladder.facing).x * STANDOFF;
    pos.y = ladder.bottom.y - forward(ladder.facing).y * STANDOFF;
    mover.pos = pos;
    mover.facing = ladder.facing;
    mover.vz = up * speed;
    mover.grounded = false;
    mover.velocity = Vec3::new(0.0, 0.0, mover.vz);
    *transform = player::placement(pos, ladder.facing, 1.0);
    *motion = Motion { speed: speed * up.abs(), local_dir: Vec2::new(0.0, up), airborne: false, swimming: false, auto: false, request: None, on_ladder: true };
    *input_state = PlayerInput { moving: up != 0.0, jumped: false, turned: false };
    if let Some(top) = step.exit {
        begin_exit(&mut commands, entity, &ladder, &mover, &mut motion, clips, top);
        send(&mut out, &mover, Some(&ladder), 0.0, false);
        return;
    }
    commands.entity(entity).insert(ladder.clone());
    state.ladder_clock(dt);
    if state.ladder_due() {
        send(&mut out, &mover, Some(&ladder), up, up != 0.0);
        state.ladder_sent();
    }
}

/// Starts the exit clip for `ladder` (at the top when `top`), or leaves at once without clips.
fn begin_exit(commands: &mut Commands, entity: Entity, ladder: &Ladder, mover: &Mover, motion: &mut Motion, clips: Option<&LadderClips>, top: bool) {
    let left = clips.map_or(0.0, |c| c.exit_seconds(ladder.kind, ladder.height(mover.pos.z)));
    commands.entity(entity).insert(Ladder { exiting: Some(Exit { left, top }), ..ladder.clone() });
    *motion = Motion { speed: 0.0, local_dir: Vec2::ZERO, airborne: false, swimming: false, auto: false, request: None, on_ladder: true };
}

/// Completes an exit: at the top the body steps over the ladder onto the platform, at the bottom it
/// stands; then the ladder is left and the move is reported without the hanging blocks.
#[allow(clippy::too_many_arguments)]
fn finish_exit(
    commands: &mut Commands,
    entity: Entity,
    mover: &mut Mover,
    motion: &mut Motion,
    state: &mut SendState,
    top: bool,
    terrain: Option<&aa_world::TerrainQuery>,
    collision: Option<&aa_world::WorldCollision>,
) {
    if top {
        let f = forward(mover.facing);
        let mut pos = mover.pos;
        pos.x += f.x * TOP_STEP;
        pos.y += f.y * TOP_STEP;
        pos.z += 0.05;
        if let (Some(terrain), Some(collision)) = (terrain, collision) {
            pos.z = player::floor_at(terrain, collision, pos, 0.5).max(pos.z);
        }
        mover.pos = pos;
        mover.grounded = false;
        mover.vz = 0.0;
    } else {
        mover.grounded = true;
        mover.vz = 0.0;
    }
    commands.entity(entity).remove::<Ladder>();
    state.after_ladder();
    *motion = Motion::default();
}

/// Writes a hanging movement for `mover` (actor flag 0x40, gc id, climb data); `ladder` `None` is a
/// release (no support block).
fn send(out: &mut MessageWriter<NetCommand>, mover: &Mover, ladder: Option<&Ladder>, up: f32, moving: bool) {
    let to_wire = |v: f32| (v * 1024.0).round().clamp(-32767.0, 32767.0) as i16;
    let support = ladder.map(|l| MoveSupport { obj_id: l.obj_id, climb: Some(l.climb_data(mover.pos.z)), ..MoveSupport::default() });
    let cmd = MoveCommand {
        pos: Pos::new(mover.pos.x, mover.pos.y, mover.pos.z),
        yaw: mover.facing,
        moving,
        stance: stance::CROUCH,
        velocity: [0, 0, to_wire(mover.vz)],
        delta_movement: [0, (up * 127.0) as i8, 0],
        state: MoveState::Hang,
        support,
        ..MoveCommand::default()
    };
    out.write(NetCommand(aa_net::Command::Move(cmd)));
}

/// Plays the climb / exit clips of a player on a [`Ladder`], and hands the animator back when it
/// leaves (`super::anim::drive_locomotion` skips a unit whose `Motion::on_ladder` is set).
#[allow(clippy::type_complexity)]
pub fn drive_ladder(
    mut player: Query<(&Mover, Option<&Ladder>, Option<&LadderClips>, &mut Animator, &mut Locomotion, Option<&Acting>, Has<Dead>), With<LocalPlayer>>,
    mut was_on: Local<bool>,
) {
    let Ok((mover, ladder, clips, mut animator, mut loco, acting, dead)) = player.single_mut() else { return };
    let Some(ladder) = ladder else {
        if *was_on {
            *was_on = false;
            loco.invalidate();
        }
        return;
    };
    *was_on = true;
    if dead || acting.is_some_and(|a| !a.layered) {
        return;
    }
    let Some(clips) = clips else { return };
    let height = ladder.height(mover.pos.z);
    let (clip, speed) = if ladder.exiting.is_some() {
        (clips.exit(ladder.kind, height), 0.0)
    } else if mover.vz > 0.05 {
        (clips.up(ladder.kind), clips.family(ladder.kind).speed)
    } else if mover.vz < -0.05 {
        (clips.family(ladder.kind).down.as_ref(), clips.family(ladder.kind).speed)
    } else {
        (clips.idle(ladder.kind), 0.0)
    };
    let Some(clip) = clip else { return };
    let playing = animator.current.as_ref().is_some_and(|p| Arc::ptr_eq(&p.clip.clip, &clip.clip));
    if !playing {
        animator.play(clip.clone(), ladder.exiting.is_none(), 0.2);
        animator.random_idle = None;
    }
    animator.speed = if speed > 0.1 && mover.vz.abs() > 0.05 {
        (mover.vz.abs() / speed).clamp(0.4, 2.0)
    } else {
        1.0
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_climb_kinds_pick_the_client_s_clip_families() {
        assert_eq!(Family::of(1), Family::Ladder);
        assert_eq!(Family::of(6), Family::Ladder);
        assert_eq!(Family::of(9), Family::Ladder);
        assert_eq!(Family::of(2), Family::Rope);
        assert_eq!(Family::of(5), Family::Rope);
        assert_eq!(Family::of(3), Family::Wood);
        assert_eq!(Family::of(4), Family::Net);
        assert_eq!(Family::of(7), Family::Mast);
        assert_eq!(Family::of(8), Family::Mast);
        assert_eq!(Family::of(10), Family::Mast);
    }

    /// The climb clips are not in the `.cal`; binding them by path gives the Nuian male's ladder,
    /// mast and rope clips (needs `AA_PAK`).
    #[test]
    fn a_nuian_man_has_the_climb_clips_by_path() {
        if !aa_cry::pak_available() {
            eprintln!("no pak: skipped");
            return;
        }
        let clips = LadderClips::load(
            "objects/characters/nuian/male/nude/nu_m_base.chr",
            "objects/characters/nuian/male/nude/nu_m_base.cal",
            &ClipLibrary::default(),
        )
        .expect("the nuian climb clips");
        assert!(clips.ladder.up.is_some() && clips.ladder.down.is_some() && clips.ladder.idle.is_some());
        assert!(clips.mast.idle.is_some() && clips.rope.idle.is_some() && clips.net.idle.is_some());
        assert!(clips.exit_ladder.is_some() && clips.exit_70.is_some() && clips.exit_45.is_some());
        assert!(clips.ladder.speed > 0.1, "the up clip moves the root at {:.2} m/s", clips.ladder.speed);
    }

    #[test]
    fn climbing_stops_at_the_bottom_and_top_and_asks_for_the_exit() {
        // 1.5 m/s up from 0 on a 4 m ladder: 1 m after 2/3 s, still climbing
        let s = step(0.0, 0.0, 4.0, 1.0, 1.5, 2.0 / 3.0);
        assert!((s.z - 1.0).abs() < 1e-5);
        assert_eq!(s.exit, None);
        // the frame that reaches the top clamps there and exits at the top
        let s = step(3.9, 0.0, 4.0, 1.0, 1.5, 0.1);
        assert_eq!((s.z, s.exit), (4.0, Some(true)));
        // down into the bottom exits at the bottom
        let s = step(0.05, 0.0, 4.0, -1.0, 1.5, 0.1);
        assert_eq!((s.z, s.exit), (0.0, Some(false)));
        // standing still never exits, also exactly at an end
        assert_eq!(step(4.0, 0.0, 4.0, 0.0, 1.5, 0.1), Step { z: 4.0, exit: None });
    }

    #[test]
    fn the_ladder_speed_multiplier_scales_the_climb() {
        let up = step(0.0, 0.0, 10.0, 1.0, CLIMB_SPEED, 1.0);
        let fast = step(0.0, 0.0, 10.0, 1.0, CLIMB_SPEED * 1.15, 1.0);
        assert!((up.z - 1.5).abs() < 1e-5);
        assert!((fast.z - 1.725).abs() < 1e-5);
    }

    #[test]
    fn the_nearest_climb_doodad_in_reach_is_taken() {
        let me = Vec3::new(10.0, 20.0, 5.0);
        let found = nearest(me, REACH, [(7, Vec3::new(13.0, 20.0, 5.0), 1), (8, Vec3::new(10.0, 22.0, 5.0), 4)]);
        assert_eq!(found, Some((8, 4, Vec3::new(10.0, 22.0, 5.0))));
        assert_eq!(nearest(me, REACH, [(7, Vec3::new(20.0, 20.0, 5.0), 1)]), None);
    }

    #[test]
    fn the_heading_faces_the_ladder() {
        let me = Vec3::new(0.0, 0.0, 0.0);
        // a ladder to the north is yaw 0; to the east (right) yaw -pi/2
        assert!(yaw_toward(me, Vec3::new(0.0, 2.0, 0.0)).unwrap().abs() < 1e-5);
        assert!((yaw_toward(me, Vec3::new(2.0, 0.0, 0.0)).unwrap() + std::f32::consts::FRAC_PI_2).abs() < 1e-5);
        assert_eq!(yaw_toward(me, me), None);
    }

    #[test]
    fn the_model_bounds_give_bottom_and_top_and_fall_back() {
        let (bottom, top) = extent(
            Some((Vec3::new(-0.5, -0.5, 0.0), Vec3::new(0.5, 0.5, 6.0))),
            Vec3::new(10.0, 20.0, 100.0),
            2.0,
            FALLBACK_HEIGHT,
        );
        assert_eq!((bottom, top), (100.0, 112.0));
        // no model: a fallback ladder from the doodad's origin
        assert_eq!(extent(None, Vec3::new(1.0, 2.0, 50.0), 1.0, FALLBACK_HEIGHT), (50.0, 54.0));
        assert_eq!(extent(Some((Vec3::ZERO, Vec3::ZERO)), Vec3::new(0.0, 0.0, 1.0), 1.0, 2.0), (1.0, 3.0));
    }

    #[test]
    fn the_climb_data_is_the_height_in_millimetres() {
        let ladder = Ladder { obj_id: 1, kind: 1, bottom: Vec3::new(0.0, 0.0, 10.0), top: 14.0, facing: 0.0, exiting: None };
        assert_eq!(ladder.climb_data(10.0), 0);
        assert_eq!(ladder.climb_data(11.234), 1234);
        assert_eq!(ladder.climb_data(9.0), 0);
    }

    use aa_net::model::{CharacterState, Equipment, ModelParams, UnitKind};
    use aa_net::session::PlayerSpawn;
    use aa_net::state::Player;
    use aa_net::{Command, NetCommand};

    fn unit(obj_id: u32) -> aa_net::model::Unit {
        aa_net::model::Unit {
            obj_id,
            name: format!("u{obj_id}"),
            kind: UnitKind::Character { char_id: 47 },
            owner_name: String::new(),
            pos: Pos::new(100.0, 200.0, 5.0),
            rotation: [0.0; 3],
            scale: 1.0,
            level: 1,
            model_id: 1,
            equipment: Equipment::default(),
            model_params: ModelParams::default(),
            hp: 100,
            mp: 100,
            max_hp: 100,
            max_mp: 100,
            dead: false,
            race_gender: 0,
            movement: None,
            faction_id: 0,
            expedition_id: 0,
            appellation: 0,
            flags: 0,
            gm_modes: [0; 4],
            bounty_money: None,
            pvp_honor: 0,
            pvp_kills: 0,
            skills: Vec::new(),
            passive_buffs: Vec::new(),
            ability_exp: Vec::new(),
            active_abilities: Vec::new(),
            buffs: Vec::new(),
            anim_action: 0,
            house_posture: None,
            state_attachments: None,
            attached_to: None,
        }
    }

    /// A ladder doodad (template 303's phase 51, a plain ladder: kind 1) or a non-climb doodad (phase 1).
    fn doodad(obj_id: u32, func_group_id: u32) -> aa_net::model::Doodad {
        aa_net::model::Doodad {
            obj_id,
            template_id: 303,
            owner_obj_id: 0,
            parent_obj_id: 0,
            attach_point: 0,
            pos: Pos::new(100.0, 202.0, 5.0),
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: 1.0,
            has_loot_item: false,
            func_group_id,
            owner_id: 0,
            ucc_id: 0,
            item_template_id: 0,
            type2: 0,
            time_left: 0,
            plant_time: 0,
            quest_glow: 0,
            family: 0,
            puzzle_group: 0,
            owner_type: 0,
            owner_db_id: 0,
            data: 0,
        }
    }

    fn app(func_group_id: u32) -> App {
        let mut state = CharacterState::default();
        state.character.id = 47;
        let spawn = PlayerSpawn { obj_id: 100, state, unit: unit(100) };
        let mut doodads = Doodads::default();
        doodads.by_id.insert(77, doodad(77, func_group_id));
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(Player { spawn: Some(spawn) })
            .insert_resource(crate::settings::Actions::default())
            .insert_resource(doodads)
            .insert_resource(Db::open().expect("the server database"))
            .insert_resource(SendState::default())
            .insert_resource(PlayerInput::default())
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_millis(100)))
            .add_message::<NetCommand>()
            .add_systems(Update, control_ladder);
        app.world_mut().spawn((
            LocalPlayer { obj_id: 100 },
            Mover { pos: Vec3::new(100.0, 200.0, 5.0), facing: 0.0, vz: 0.0, grounded: true, velocity: Vec3::ZERO, swim: None, wading: 0.0 },
            Transform::default(),
            Motion::default(),
        ));
        app
    }

    fn sent(app: &mut App) -> Vec<Command> {
        app.world_mut().resource_mut::<Messages<NetCommand>>().drain().map(|v| v.0).collect()
    }

    fn ladder_of(app: &mut App) -> Option<Ladder> {
        let mut q = app.world_mut().query::<&Ladder>();
        q.iter(app.world()).next().cloned()
    }

    #[test]
    fn f_grabs_the_climb_doodad_and_reports_the_hanging_block() {
        // template 303 phase 51 is the plain ladder (kind 1) in the server's database
        let mut app = app(51);
        assert_eq!(app.world().resource::<Db>().ladder_kind(51), Some(1));
        app.world_mut().resource_mut::<crate::settings::Actions>().tap_for_test("do_interaction_1");
        app.update();
        let ladder = ladder_of(&mut app).expect("the ladder is grabbed");
        assert_eq!((ladder.obj_id, ladder.kind), (77, 1));
        let moves: Vec<_> = sent(&mut app).into_iter().filter_map(|c| if let Command::Move(m) = c { Some(m) } else { None }).collect();
        assert_eq!(moves.len(), 1);
        let m = moves[0].to_movement(0);
        assert_eq!(m.actor_flags, aa_net::model::actor_flags::HANGING_FROM_OBJECT);
        let support = m.support.expect("the hanging support block");
        assert_eq!((support.obj_id, support.climb), (77, Some(0)));
    }

    #[test]
    fn w_climbs_at_the_base_speed_and_f_leaves_the_ladder() {
        let mut app = app(51);
        app.world_mut().resource_mut::<crate::settings::Actions>().tap_for_test("do_interaction_1");
        app.update();
        app.world_mut().resource_mut::<crate::settings::Actions>().clear_taps_for_test();
        let start = ladder_of(&mut app).expect("grabbed").bottom.z;
        app.world_mut().resource_mut::<crate::settings::Actions>().hold_for_test("moveforward");
        for _ in 0..4 {
            app.update();
        }
        let mut q = app.world_mut().query::<&Mover>();
        let z = q.single(app.world()).unwrap().pos.z;
        assert!(z > start + 0.1, "climbed from {start} to {z}");
        // the hanging packets carry the new height as climb data
        let m = sent(&mut app).into_iter().rev().find_map(|c| if let Command::Move(m) = c { Some(m) } else { None }).expect("a climb packet");
        assert!(m.support.unwrap().climb.unwrap() > 0);
        app.world_mut().resource_mut::<crate::settings::Actions>().release_for_test("moveforward");
        // F plays the exit; without clips it ends on the next frame and the body stands again
        app.world_mut().resource_mut::<crate::settings::Actions>().tap_for_test("do_interaction_1");
        app.update();
        app.world_mut().resource_mut::<crate::settings::Actions>().clear_taps_for_test();
        app.update();
        assert!(ladder_of(&mut app).is_none());
        let mut q = app.world_mut().query::<&Mover>();
        assert!(q.single(app.world()).unwrap().grounded);
    }

    #[test]
    fn a_non_climb_doodad_is_not_grabbed() {
        let mut app = app(1);
        assert_eq!(app.world().resource::<Db>().ladder_kind(1), None);
        app.world_mut().resource_mut::<crate::settings::Actions>().tap_for_test("do_interaction_1");
        app.update();
        assert!(ladder_of(&mut app).is_none());
        assert!(sent(&mut app).is_empty());
    }
}
