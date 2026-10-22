//! Riders: a unit the server attached to a mount (`SCUnitAttached`: a mate such as a horse or a lion) sits on the mount's seat
//! bone and plays the rider's clips of that kind of mount.
//!
//! The server moves a rider with its mount: it sends no movement for the rider, only for the mount. The rider's own clips
//! come from its character `.cal` (`horse_ba_relaxed_idle`, `horse_mo_normal_walk_f`, `horse_mo_normal_run_f`, and the
//! `lion_`, `wildboar_` and `pangolin_` sets; the `models.mount_pose_id` of the mount says which), the mount's from its own
//! `.cal` as for any creature. The seat is the mount's bone `bone_spine_driver` (attach point 1) or `bone_spine_passenger`.

use std::collections::HashMap;

use aa_anim::pose::BoundClip;
use aa_anim::{Animator, Cal, Character, ClipLibrary};
use aa_net::model::{Attachment, Equipment, ModelParams, Movement, UnitKind, stance};
use aa_net::skill::controller_kind;
use aa_net::state::{Player, Units};
use aa_net::{Command, Event, MoveCommand, MoveState, NetCommand, NetEvent, Pos};
use aa_world::coords::bevy_to_cry;
use aa_world::{TerrainQuery, WaterQuery, WorldCollision};
use bevy::prelude::*;

use super::anim::{Locomotion, Motion, bind_clip, bind_clip_with};
use super::camera::OrbitCamera;
use super::data::{Db, MountMove, Seat};
use super::mount_anim::{self, Band, Hop, MateGait, Side};
use super::player::{LocalPlayer, Mover, PlayerInput, approach_angle, forward, placement, right};
use super::units::{RemoteUnit, Views};
use crate::hud::{HudKeyboardFocus, HudPointerOverUi};

/// The clip prefix of a rider on a mount of this kind (`models.mount_pose_id`).
fn rider_prefix(pose: u32) -> Option<&'static str> {
    match pose {
        1 => Some("horse"),
        2 => Some("lion"),
        3 => Some("bear"),
        4 => Some("wildboar"),
        10 => Some("pangolin"),
        _ => None,
    }
}

/// The bone of a mount a rider at `point` sits on (`model_attach_point_strings`: `$driver` is `bone_spine_driver`).
/// The seat bone in the world for the mount as it is placed **this frame**. The bone's `GlobalTransform` is last frame's (the
/// propagation runs after `Update`), so a mount that moved since then left the rider one step behind it: the jitter at speed. The
/// bone is therefore taken relative to the mount's previous global transform and re-applied to its current `Transform`.
fn seat_now(mount_now: &Transform, mount_before: Option<&GlobalTransform>, bone: Option<&GlobalTransform>, fallback: Vec3) -> Vec3 {
    match (mount_before, bone) {
        (Some(before), Some(bone)) => (mount_now.to_matrix() * before.to_matrix().inverse() * bone.to_matrix()).w_axis.truncate(),
        (None, Some(bone)) => bone.translation(),
        _ => aa_world::cry_to_bevy(fallback),
    }
}

fn seat_bone(point: u8) -> &'static str {
    if point <= 1 { "bone_spine_driver" } else { "bone_spine_passenger" }
}

/// A slave seat in world space: the model-local attach point rotated by the slave's yaw.
pub(super) fn seat_world(root: Vec3, yaw: f32, scale: f32, seat: Seat) -> (Vec3, f32) {
    let (x, y, z) = (seat.local[0] * scale, seat.local[1] * scale, seat.local[2] * scale);
    let (s, c) = yaw.sin_cos();
    let pos = Vec3::new(root.x + x * c - y * s, root.y + x * s + y * c, root.z + z);
    (pos, yaw + seat.yaw)
}

/// The mate whose driver seat holds the local player, if any: only then do the player's keys move
/// the mount. A passenger or an on-foot player returns `None`.
pub(super) fn driver_mount(player: &Player) -> Option<u32> {
    mounted_driver(player.spawn.as_ref().and_then(|spawn| spawn.unit.attached_to))
}

fn mounted_driver(attached: Option<Attachment>) -> Option<u32> {
    attached.filter(|a| a.point == 1).map(|a| a.obj_id)
}

/// What the rider plays (the `Horse_Driver*` states of `player_full_body.xml`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Ride {
    Idle,
    Walk,
    Run,
    Sprint,
    /// Running while the mount turns (`..._run_f_lturn/rturn`): a left turn.
    Turn(bool),
    /// In the air (`jump_f_start`, `jump_f_loop`).
    Air,
}

/// The clips of a rider on a mount and the state they are in.
struct RideAnim {
    idle: BoundClip,
    walk: Option<BoundClip>,
    run: Option<BoundClip>,
    sprint: Option<BoundClip>,
    idle_to_run: Option<BoundClip>,
    run_to_idle: Option<BoundClip>,
    jump_start: Option<BoundClip>,
    jump_loop: Option<BoundClip>,
    jump_land: Option<BoundClip>,
    turn_l: Option<BoundClip>,
    turn_r: Option<BoundClip>,
    state: Option<Ride>,
    band: Band,
}

/// What the rider copies from the mount each frame (`FUN_390dcc80`: `PseudoSpeed` and `RequestedTurnSpeedLZ` of the mount graph).
#[derive(Clone, Copy, Debug, Default)]
struct MountDrive {
    /// The mount PseudoSpeed (0 while it swims: the rider sits idle).
    pseudo: f32,
    airborne: bool,
    /// The mount turn rate (`RequestedTurnSpeedLZ`, degrees/s), positive to the left.
    turn: f32,
    /// The mount playback speed, which keeps the feet on the ground.
    tempo: f32,
}

/// The rider state for what the mount does: a jump in the air, else the gait band by the mount PseudoSpeed (walk 0.15-0.6, run from
/// 0.7), the run turn blend while the mount turns, the sprint tier from 1.25 (reached only by the 1.5 of auto-move). `have` says which clips exist: (turn, sprint, jump).
fn choose_ride(drive: MountDrive, prev: Band, have: (bool, bool, bool)) -> (Ride, Band) {
    if drive.airborne && have.2 {
        return (Ride::Air, prev);
    }
    let band = mount_anim::rider_band(drive.pseudo, prev);
    let ride = match band {
        Band::Idle => Ride::Idle,
        Band::Walk => Ride::Walk,
        Band::Run => {
            let turn = mount_anim::turn_dir(drive.turn);
            if turn != 0 && have.0 {
                Ride::Turn(turn > 0)
            } else if drive.pseudo >= mount_anim::SPRINT_PSEUDO && have.1 {
                Ride::Sprint
            } else {
                Ride::Run
            }
        }
    };
    (ride, band)
}

impl RideAnim {
    fn new(idle: BoundClip) -> RideAnim {
        RideAnim {
            idle,
            walk: None,
            run: None,
            sprint: None,
            idle_to_run: None,
            run_to_idle: None,
            jump_start: None,
            jump_loop: None,
            jump_land: None,
            turn_l: None,
            turn_r: None,
            state: None,
            band: Band::Idle,
        }
    }

    /// The loop clip of a grounded state, with the fallbacks of a pose that lacks some of the set.
    fn clip(&self, ride: Ride) -> BoundClip {
        let run = || self.run.clone().or_else(|| self.walk.clone());
        match ride {
            Ride::Idle | Ride::Air => self.idle.clone(),
            Ride::Walk => self.walk.clone().or_else(run).unwrap_or_else(|| self.idle.clone()),
            Ride::Run => run().unwrap_or_else(|| self.idle.clone()),
            Ride::Sprint => self.sprint.clone().or_else(run).unwrap_or_else(|| self.idle.clone()),
            Ride::Turn(left) => (if left { self.turn_l.clone() } else { self.turn_r.clone() }).or_else(run).unwrap_or_else(|| self.idle.clone()),
        }
    }

    /// The state the mount drive asks for.
    fn wanted(&mut self, drive: MountDrive) -> Ride {
        let (ride, band) = choose_ride(drive, self.band, (self.turn_l.is_some() || self.turn_r.is_some(), self.sprint.is_some(), self.jump_start.is_some()));
        self.band = band;
        ride
    }

    /// Plays the clip of this frame: the gaits by PseudoSpeed with the idle to run and run to idle transitions, the jump states.
    fn update(&mut self, animator: &mut Animator, drive: MountDrive) {
        let wanted = self.wanted(drive);
        let old = self.state;
        if old != Some(wanted) {
            let clip = self.clip(wanted);
            match (old, wanted) {
                (_, Ride::Air) => match (self.jump_start.clone(), self.jump_loop.clone()) {
                    (Some(start), Some(air)) => animator.play_once_then(start, air, 0.1),
                    (Some(start), None) => animator.play(start, false, 0.1),
                    _ => {}
                },
                (Some(Ride::Air), _) => match self.jump_land.clone() {
                    Some(land) => animator.play_once_then(land, clip, 0.1),
                    None => animator.play(clip, true, 0.2),
                },
                (Some(Ride::Idle), Ride::Run | Ride::Sprint) if self.idle_to_run.is_some() => {
                    animator.play_once_then(self.idle_to_run.clone().unwrap_or_else(|| clip.clone()), clip, 0.15);
                }
                (Some(Ride::Run | Ride::Sprint | Ride::Turn(_)), Ride::Idle) if self.run_to_idle.is_some() => {
                    animator.play_once_then(self.run_to_idle.clone().unwrap_or_else(|| clip.clone()), clip, 0.15);
                }
                _ => animator.play(clip, true, 0.2),
            }
            animator.random_idle = None;
            self.state = Some(wanted);
        }
        animator.speed = if matches!(wanted, Ride::Idle | Ride::Air) { 1.0 } else { drive.tempo };
    }
}

/// Gives every mate its [`MateGait`] (the run speed its PseudoSpeed is measured against: `game_stances` of its actor model).
pub fn tag_mates(mut commands: Commands, net: Res<Units>, db: Option<Res<Db>>, units: Query<(Entity, &RemoteUnit), Without<MateGait>>) {
    let Some(db) = db else { return };
    for (entity, unit) in &units {
        let Some(mate) = net.by_id.get(&unit.obj_id).filter(|u| matches!(u.kind, UnitKind::Mate { .. })) else { continue };
        commands.entity(entity).insert(MateGait::new(db.mount_move(mate.model_id).run));
    }
}

/// The mounting in progress (the original `MountHorse Controller`): the HUD starts it with [`MountFlow::begin`] after its checks, the
/// player walks to the mate while the controller runs ([`MountFlow::steer`], used by `control_player`), and
/// [`drive_mount_flow`] sends `CSMountMate` once the player is within 3 m. Any movement input cancels it.
///
/// The original announces every controller to the server with `CSCreateSkillController`: the kind when it starts and kind 0 when it
/// ends (measured: mount = `Create(8)`, `CSMountMate`, `Create(0)` 2 s later; see `docs/research/re-mounts.md`). The same goes for the
/// `UnMountHorseController`, which a detach (reason 9: 500 ms, reason 10: 1000 ms) or an unmounted dismiss starts; a dismiss sends
/// `CSRemoveMate` when that controller ends ([`MountFlow::start_unmount`]).
#[derive(Resource, Default)]
pub struct MountFlow {
    pending: Option<Flow>,
    unmount: Option<Unmount>,
    /// `CSCreateSkillController` kinds waiting to go out, in order.
    announce: Vec<u8>,
}

struct Flow {
    mount: u32,
    tl_id: u16,
    point: u8,
    /// Seconds since the controller started (the app clock when it did, until the first frame stamps it).
    started: Option<f32>,
    sent: Option<f32>,
    /// Where to walk this frame and whether slowly (`None`: standing).
    walk_to: Option<(Vec2, bool)>,
}

/// The `UnMountHorseController` in progress.
struct Unmount {
    seconds: f32,
    started: Option<f32>,
    /// The command the original continues with when the controller ends (the dismiss of a mate).
    then: Option<Command>,
}

impl MountFlow {
    /// Starts the controller for the mate `mount` (a new request replaces the one in progress).
    pub fn begin(&mut self, mount: u32, tl_id: u16, point: u8) {
        self.finish();
        self.announce.push(controller_kind::MOUNT_HORSE);
        self.pending = Some(Flow { mount, tl_id, point, started: None, sent: None, walk_to: None });
    }

    /// Ends the mounting in progress and reports it.
    fn finish(&mut self) {
        if self.pending.take().is_some() {
            self.announce.push(controller_kind::NONE);
        }
    }

    /// Starts the `UnMountHorseController` for an unmount of `reason` (9 or 10; others have no controller here), optionally with the
    /// command to send when it ends. A controller already running only takes the command.
    pub fn start_unmount(&mut self, reason: u8, then: Option<Command>) {
        if let Some(running) = self.unmount.as_mut() {
            if then.is_some() {
                running.then = then;
            }
            return;
        }
        let seconds = match reason {
            9 => 0.5,
            10 => 1.0,
            _ => return,
        };
        self.announce.push(controller_kind::UNMOUNT_HORSE);
        self.unmount = Some(Unmount { seconds, started: None, then });
    }

    /// Whether a mounting is in progress.
    #[cfg(test)]
    pub fn active(&self) -> bool {
        self.pending.is_some()
    }

    /// The auto-walk of this frame for the player at `pos`: turns `facing` to the approach point and returns whether to walk forward
    /// and whether slowly. Any key (`input`) cancels the controller.
    pub fn steer(&mut self, pos: Vec3, facing: &mut f32, input: bool) -> (bool, bool) {
        let Some(flow) = self.pending.as_mut() else { return (false, false) };
        if input {
            self.finish();
            return (false, false);
        }
        let Some((to, slow)) = flow.walk_to else { return (false, false) };
        *facing = mount_anim::bearing(pos.truncate(), to);
        (true, slow)
    }

    /// Runs the unmount controller clock: reports its end and returns the command it continues with.
    fn tick_unmount(&mut self, now: f32) -> Option<Command> {
        let unmount = self.unmount.as_mut()?;
        let started = *unmount.started.get_or_insert(now);
        if now - started < unmount.seconds {
            return None;
        }
        self.announce.push(controller_kind::NONE);
        self.unmount.take().and_then(|u| u.then)
    }
}

/// Starts the `UnMountHorseController` when the local player is detached by an unmount (reasons 9 and 10).
pub fn unmount_controller_on_detach(mut events: MessageReader<NetEvent>, player: Res<Player>, mut flow: ResMut<MountFlow>) {
    let own = player.spawn.as_ref().map_or(0, |s| s.obj_id);
    for NetEvent(event) in events.read() {
        if let Event::UnitDetached { obj_id, reason } = event
            && own != 0
            && *obj_id == own
        {
            flow.start_unmount(*reason, None);
        }
    }
}

/// Runs the mount controller: stops it when it is over, and sends `CSMountMate` (reason by the geometry of that moment) as soon as
/// the player is within 3 m of the mate. Also sends the `CSCreateSkillController` reports and what an ending unmount controller
/// continues with.
pub fn drive_mount_flow(
    time: Res<Time>,
    mut flow: ResMut<MountFlow>,
    net: Res<Units>,
    player: Res<Player>,
    movers: Query<&Mover, With<LocalPlayer>>,
    mut out: MessageWriter<NetCommand>,
) {
    drive_flow(&time, &mut flow, &net, &player, &movers, &mut out);
    if let Some(then) = flow.tick_unmount(time.elapsed_secs()) {
        out.write(NetCommand(then));
    }
    for sc_type in flow.announce.drain(..) {
        out.write(NetCommand(Command::SkillController { sc_type, fall_damage_immune: false }));
    }
}

fn drive_flow(time: &Time, flow: &mut MountFlow, net: &Units, player: &Player, movers: &Query<&Mover, With<LocalPlayer>>, out: &mut MessageWriter<NetCommand>) {
    let Some(pending) = flow.pending.as_mut() else { return };
    let now = time.elapsed_secs();
    let started = *pending.started.get_or_insert(now);
    let elapsed = now - started;
    let (Ok(mover), Some(mount)) = (movers.single(), net.by_id.get(&pending.mount)) else {
        flow.finish();
        return;
    };
    let attached = player.spawn.as_ref().is_some_and(|s| s.unit.attached_to.is_some());
    // Once the request went out the controller runs on to its 2 s wait, mounted or not (measured on the original).
    if (attached && pending.sent.is_none()) || mount.dead {
        flow.finish();
        return;
    }
    let mount_pos = Vec3::new(mount.pos.x, mount.pos.y, mount.pos.z);
    let step = mount_anim::flow_step(mover.pos, mount_pos, mount.yaw(), elapsed, pending.sent);
    pending.walk_to = None;
    match step {
        mount_anim::Step::Walk { to, slow } => pending.walk_to = Some((to, slow)),
        mount_anim::Step::Send => {
            let (mut driver, mut passenger) = (false, false);
            for unit in net.iter() {
                if let Some(a) = unit.attached_to.filter(|a| a.obj_id == pending.mount) {
                    driver |= a.point == 1;
                    passenger |= a.point == 2;
                }
            }
            let reason = mount_anim::entry_reason(mover.pos, mount_pos, mount.yaw(), driver, passenger);
            out.write(NetCommand(Command::Mate(aa_net::mate::MateCommand::Mount { tl_id: pending.tl_id, point: pending.point, reason })));
            pending.sent = Some(elapsed);
        }
        mount_anim::Step::Wait => {}
        mount_anim::Step::Stop => flow.finish(),
    }
}

/// The `reason` byte of the `SCUnitAttached` that seated each rider (rider object id to mount object id and reason): it names the
/// side the rider got on from, which the get-on clip follows (the original reads it as the rider graph `VehicleSeat`).
#[derive(Resource, Default)]
pub struct AttachReasons(HashMap<u32, (u32, u8)>);

/// Remembers the reason of every attach, and forgets it again at the detach.
pub fn record_attach_reasons(mut events: MessageReader<NetEvent>, mut reasons: ResMut<AttachReasons>) {
    for NetEvent(event) in events.read() {
        match event {
            Event::UnitAttached { obj_id, mount, reason, .. } => {
                reasons.0.insert(*obj_id, (*mount, *reason));
            }
            Event::UnitDetached { obj_id, .. } => {
                reasons.0.remove(obj_id);
            }
            _ => {}
        }
    }
}

impl AttachReasons {
    /// The side `rider` got on `mount` from, when this client saw the attach happen (a rider already seated when it came into view
    /// has none and does not play a get-on clip).
    fn side(&self, rider: u32, mount: u32) -> Option<Side> {
        self.0.get(&rider).filter(|(m, _)| *m == mount).map(|(_, reason)| Side::of_reason(*reason))
    }
}

/// The clips of a rider: the gait set of the mount kind and the get-on clip of the side.
struct RidingClips {
    anim: RideAnim,
    geton: Option<BoundClip>,
}

/// Binds the first of `names` the character has, with its root motion played (the get-on and get-off clips lift the body).
fn bind_moving(character: &Character, animator: &Animator, clips: &ClipLibrary, names: &[String]) -> Option<BoundClip> {
    let cal = Cal::shared(&character.assets.cal).ok()?;
    names.iter().find_map(|name| bind_clip_with(&animator.rig, &cal, clips, name, false))
}

/// The clips of a rider on a mount of this model (`side`: the get-on clip, none for a rider that was already seated).
fn riding_clips(character: &Character, mount_model: u32, db: &Db, animator: &Animator, clips: &ClipLibrary, side: Option<Side>) -> Option<RidingClips> {
    let prefix = rider_prefix(db.mount_pose(mount_model))?;
    let cal = Cal::shared(&character.assets.cal).ok()?;
    let bind = |name: String| bind_clip(&animator.rig, &cal, clips, &name);
    let idle = bind(format!("{prefix}_ba_relaxed_idle")).or_else(|| bind("fist_ba_relaxed_idle".into()))?;
    let mut anim = RideAnim::new(idle);
    anim.run = bind(format!("{prefix}_mo_normal_run_f"));
    anim.walk = bind(format!("{prefix}_mo_normal_walk_f")).or_else(|| anim.run.clone());
    anim.sprint = bind(format!("{prefix}_mo_normal_sprint_f"));
    anim.idle_to_run = bind(format!("{prefix}_mo_relaxed_idletorun_f"));
    anim.run_to_idle = bind(format!("{prefix}_mo_relaxed_runtoidle_f"));
    anim.jump_start = bind(format!("{prefix}_mo_jump_f_start"));
    anim.jump_loop = bind(format!("{prefix}_mo_jump_f_loop"));
    anim.jump_land = bind(format!("{prefix}_mo_jump_f_land"));
    anim.turn_l = bind(format!("{prefix}_mo_normal_run_f_lturn"));
    anim.turn_r = bind(format!("{prefix}_mo_normal_run_f_rturn"));
    let geton = side.and_then(|side| bind_moving(character, animator, clips, &mount_anim::geton_names(prefix, side)));
    Some(RidingClips { anim, geton })
}

/// The dismount clips of a rider on a mount of this model: `getoff_start`, `_loop`, `_end` (the jump pipeline of the rider graph),
/// or the one-piece `getoff` for a pose that has no split set.
fn hop_clips(character: &Character, mount_model: u32, db: &Db, animator: &Animator, clips: &ClipLibrary) -> Vec<BoundClip> {
    let Some(prefix) = rider_prefix(db.mount_pose(mount_model)) else { return Vec::new() };
    let parts: Vec<BoundClip> = mount_anim::getoff_names(prefix).iter().filter_map(|names| bind_moving(character, animator, clips, names)).collect();
    if !parts.is_empty() {
        return parts;
    }
    let whole = [format!("fist_ac_{prefix}_l_getoff"), "fist_ac_horse_l_getoff".to_string()];
    bind_moving(character, animator, clips, &whole).into_iter().collect()
}

/// The plain relaxed idle for a character seated in a vehicle (no authored vehicle-specific rider
/// clips in the 1.2 data).
fn vehicle_rider_clips(character: &Character, animator: &Animator, clips: &ClipLibrary) -> Option<RidingClips> {
    let cal = Cal::shared(&character.assets.cal).ok()?;
    let idle = bind_clip(&animator.rig, &cal, clips, "fist_ba_relaxed_idle")?;
    Some(RidingClips { anim: RideAnim::new(idle), geton: None })
}

/// A unit sitting on a mount.
#[derive(Component)]
pub struct Rider {
    mount: u32,
    point: u8,
    anim: RideAnim,
    /// The get-on clip in progress.
    boarding: Option<Boarding>,
    /// A slave seat rather than a mate's bone (`seat` is in the slave's model space).
    vehicle: bool,
    seat: Seat,
    /// The locomotion the unit had on foot, given back when it gets off.
    on_foot: Option<Locomotion>,
}

/// The local player sitting on its own or somebody else's mount.
#[derive(Component)]
pub struct LocalRider {
    mount: u32,
    point: u8,
    anim: RideAnim,
    boarding: Option<Boarding>,
    /// A slave/vehicle seat rather than a mate's bone: the player sits at `seat` in the model space.
    vehicle: bool,
    seat: Seat,
    on_foot: Option<Locomotion>,
}

/// A get-on clip in progress. The rider is bound to the seat bone from the start (the clip uses movement control Entity: no root
/// motion moves the entity); the clip's skeletal pose starts beside the mount and ends in the saddle.
struct Boarding {
    remaining: f32,
}

impl Boarding {
    fn new(clip: Option<&BoundClip>) -> Option<Boarding> {
        Some(Boarding { remaining: clip?.clip.duration })
    }
}

/// The dismount hop (`getoff_start`, `_loop`, `_end`) playing before the on-foot locomotion starts again.
#[derive(Component)]
pub struct Dismounting {
    hop: Hop,
    on_foot: Option<Locomotion>,
}

/// A mate the local player drives from its driver seat. While this component exists the mount
/// ignores its received movement track and follows the player's input; every sent packet names the
/// mate object id (`CSMoveUnit`).
#[derive(Component)]
pub struct DrivenMount {
    body: MountMove,
    pos: Vec3,
    facing: f32,
    vz: f32,
    /// The ground speed (m/s, world x/y) the inertia carries: it follows the wanted one, never jumps to it.
    velocity: Vec2,
    grounded: bool,
    swimming: bool,
    walking: bool,
    autorun: bool,
    since_sent: f32,
    was_active: bool,
    sent_first: bool,
}

impl DrivenMount {
    fn new(pos: Vec3, facing: f32, body: MountMove) -> Self {
        Self {
            body,
            pos,
            facing,
            vz: 0.0,
            velocity: Vec2::ZERO,
            grounded: true,
            swimming: false,
            walking: false,
            autorun: false,
            since_sent: 0.0,
            was_active: false,
            sent_first: false,
        }
    }

    /// The world-space ground step of this frame's input (m/s in x/y), before collision and gravity.
    fn step(&self, local_dir: Vec2, walk: bool) -> Vec2 {
        let speed = if self.swimming {
            self.body.run * 0.45
        } else if walk {
            self.body.walk
        } else {
            self.body.run
        };
        let mult = if local_dir.y < -0.1 {
            0.65
        } else if local_dir.x.abs() > local_dir.y.abs() {
            0.8
        } else {
            1.0
        };
        let (f, r) = (forward(self.facing), right(self.facing));
        (f * local_dir.y + r * local_dir.x) * speed * mult
    }

    /// Carries the speed towards `target` (m/s): `game_inertia_accel` while it speeds up, `game_inertia` while it slows down
    /// (an exponential approach; the semantics of the two values are inferred from Cry's player movement). Returns the speed.
    fn smooth(&mut self, target: Vec2, dt: f32) -> Vec2 {
        let rate = if target.length_squared() > self.velocity.length_squared() { self.body.accel } else { self.body.inertia };
        self.velocity += (target - self.velocity) * (1.0 - (-rate * dt).exp());
        if target == Vec2::ZERO && self.velocity.length_squared() < 0.0025 {
            self.velocity = Vec2::ZERO;
        }
        self.velocity
    }

    /// The packet for the mate: `CSMoveUnit` with the mount object id, the same unit body the
    /// player sends (the server's `apply_unit_move` handles a mate target through
    /// `on_mate_unit_move` and broadcasts the mount; the rider follows by attachment).
    pub(crate) fn packet(&self, obj_id: u32, moving: bool, velocity: Vec3, delta: [i8; 3], state: MoveState, landed: Option<f32>) -> Command {
        let to_wire = |v: f32| (v * 1024.0).round().clamp(-32767.0, 32767.0) as i16;
        let mut cmd = MoveCommand {
            pos: Pos::new(self.pos.x, self.pos.y, self.pos.z),
            yaw: self.facing,
            moving,
            stance: if self.swimming { stance::SWIM } else { stance::RELAXED },
            velocity: if moving { [to_wire(velocity.x), to_wire(velocity.y), to_wire(velocity.z)] } else { [0; 3] },
            delta_movement: delta,
            ..MoveCommand::default()
        }
        .with_state(state);
        if let Some(speed) = landed {
            cmd = cmd.landing(speed);
        }
        Command::MoveUnit { obj_id, command: cmd }
    }
}

const MOUNT_STEP_UP: f32 = 0.5;
const MOUNT_DROP: f32 = 0.6;
const MOUNT_CLIMB_LOOK: f32 = 0.5;
const MOUNT_MAX_CLIMB_TAN: f32 = 1.19;
const MOUNT_SEND_INTERVAL: f32 = 0.1;
const MOUNT_ALIGN_RATE: f32 = 12.0;

/// Seats the units the server attached to a mount and lets the ones that got off walk again.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn start_stop_riding(
    mut commands: Commands,
    net: Res<Units>,
    db: Option<Res<Db>>,
    clips: Res<ClipLibrary>,
    reasons: Res<AttachReasons>,
    units: Query<(Entity, &RemoteUnit, Option<&Rider>, Option<&Character>, Option<&Animator>)>,
) {
    for (entity, unit, rider, character, animator) in &units {
        let attached = net.by_id.get(&unit.obj_id).and_then(|u| u.attached_to);
        match (attached, rider) {
            (Some(a), None) => {
                // wait for the animator (a unit far away has none) and the mount's data
                let (Some(db), Some(character), Some(animator), Some(mount)) = (db.as_deref(), character, animator, net.by_id.get(&a.obj_id)) else { continue };
                let vehicle = matches!(mount.kind, UnitKind::Slave { .. });
                let side = if vehicle { None } else { reasons.side(unit.obj_id, a.obj_id) };
                let clis = if vehicle {
                    vehicle_rider_clips(character, animator, &clips)
                } else {
                    riding_clips(character, mount.model_id, db, animator, &clips, side)
                };
                let Some(clis) = clis else { continue };
                let (mount_id, point) = (a.obj_id, a.point);
                let seat = db.seat(mount.model_id, point);
                let RidingClips { anim, geton } = clis;
                let boarding = Boarding::new(geton.as_ref());
                debug!("unit {} rides {} (model {}) at point {point}{}", unit.obj_id, mount_id, mount.model_id, if vehicle { " (slave seat)" } else { "" });
                commands.queue(move |world: &mut World| {
                    if let (Some(on), Some(mut animator)) = (geton, world.entity_mut(entity).get_mut::<Animator>()) {
                        animator.play_once_then(on, anim.idle.clone(), 0.15);
                    }
                    let on_foot = world.entity_mut(entity).take::<Locomotion>();
                    world.entity_mut(entity).insert(Rider { mount: mount_id, point, anim, boarding, vehicle, seat, on_foot });
                });
            }
            (None, Some(rider)) => {
                let hop = if rider.vehicle {
                    Vec::new()
                } else {
                    (|| {
                        let mount = net.by_id.get(&rider.mount)?;
                        Some(hop_clips(character?, mount.model_id, db.as_deref()?, animator?, &clips))
                    })()
                    .unwrap_or_default()
                };
                let scale = unit.scale();
                commands.queue(move |world: &mut World| {
                    let Some(rider) = world.entity_mut(entity).take::<Rider>() else { return };
                    let (pos, facing) = world.entity(entity).get::<RemoteUnit>().map_or((Vec3::ZERO, 0.0), |u| (u.pos, u.facing));
                    start_hop(world, entity, hop, pos, facing, scale, rider.on_foot);
                    // it stands where the hop ends: the movement packets of the unit itself carry on from there
                    if let Some(mut motion) = world.entity_mut(entity).get_mut::<Motion>() {
                        *motion = Motion::default();
                    }
                });
            }
            _ => {}
        }
    }
}

/// Starts the dismount hop of `entity` from the saddle at `pos`, or gives the on-foot locomotion back at once when there is no clip.
fn start_hop(world: &mut World, entity: Entity, clips: Vec<BoundClip>, pos: Vec3, facing: f32, scale: f32, on_foot: Option<Locomotion>) {
    let mut hop = Hop::new(clips, pos, facing, scale);
    let first = world.entity(entity).get::<Animator>().and_then(|a| hop.next(&a.rig));
    match first {
        Some(clip) => {
            if let Some(mut animator) = world.entity_mut(entity).get_mut::<Animator>() {
                animator.play(clip, false, 0.15);
            }
            world.entity_mut(entity).insert(Dismounting { hop, on_foot });
        }
        None => {
            if let Some(mut on_foot) = on_foot {
                on_foot.invalidate();
                world.entity_mut(entity).insert(on_foot);
            }
            if let Some(mut unit) = world.entity_mut(entity).get_mut::<RemoteUnit>() {
                let (pos, facing) = (unit.pos, unit.facing);
                unit.reset_track(pos, facing);
            }
        }
    }
}

/// Puts the riders on their mounts' seats and plays the rider clips for the gait of the mount.
#[allow(clippy::too_many_arguments)]
pub fn pose_riders(
    time: Res<Time>,
    net: Res<Units>,
    views: Res<Views>,
    mut riders: Query<(&mut RemoteUnit, &mut Transform, &mut Rider, &mut Animator)>,
    mounts: Query<(&RemoteUnit, &Animator, &Motion, Option<&MateGait>, &Transform), Without<Rider>>,
    globals: Query<&GlobalTransform>,
) {
    for (mut unit, mut transform, mut rider, mut animator) in &mut riders {
        let mount_entity = views.unit_entity(rider.mount);
        let mounted = mount_entity.and_then(|e| mounts.get(e).ok().map(|m| (e, m)));
        let (pos, facing, drive) = match mounted {
            Some((_e, (mount, mount_animator, _motion, _gait, _tf))) if rider.vehicle => {
                let (pos, facing) = seat_world(Vec3::new(mount.pos.x, mount.pos.y, mount.pos.z), mount.facing, mount.scale(), rider.seat);
                (pos, facing, MountDrive { tempo: mount_animator.speed, ..default() })
            }
            Some((e, (mount, mount_animator, motion, gait, mount_tf))) => {
                // the seat bone of the mount's skeleton, in the world
                let bone = mount_animator.rig.skeleton.bones.iter().position(|b| b.name == seat_bone(rider.point)).and_then(|i| mount_animator.bones.get(i));
                let world = seat_now(mount_tf, globals.get(e).ok(), bone.and_then(|b| globals.get(*b).ok()), mount.pos);
                let swimming = motion.swimming;
                let pseudo = if swimming { 0.0 } else { gait.map_or(0.0, |g| g.pseudo_of(motion)) };
                (bevy_to_cry(world), mount.facing, MountDrive { pseudo, airborne: motion.airborne && !swimming, turn: gait.map_or(0.0, |g| g.turn), tempo: mount_animator.speed })
            }
            None => {
                // the mount is not drawn: stay with where the server says it is
                let Some(m) = net.by_id.get(&rider.mount) else { continue };
                let (pos, facing) = if rider.vehicle {
                    seat_world(Vec3::new(m.pos.x, m.pos.y, m.pos.z), m.yaw(), m.scale, rider.seat)
                } else {
                    (Vec3::new(m.pos.x, m.pos.y, m.pos.z), m.yaw())
                };
                (pos, facing, MountDrive { tempo: 1.0, ..default() })
            }
        };
        // the rider is bound to the seat bone, also while the get-on clip plays
        unit.pos = pos;
        unit.facing = facing;
        *transform = placement(pos, facing, unit.scale());
        if let Some(boarding) = &mut rider.boarding {
            boarding.remaining -= time.delta_secs();
            if boarding.remaining > 0.0 {
                animator.speed = 1.0;
                continue;
            }
            rider.boarding = None;
        }
        rider.anim.update(&mut animator, drive);
    }
}

/// Seats the local player when the server attached its own object (`SCUnitAttached`), and starts the
/// authored get-off clip on the detach that follows an unmount or dismiss.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn start_stop_local_riding(
    mut commands: Commands,
    player: Res<Player>,
    net: Res<Units>,
    views: Res<Views>,
    db: Option<Res<Db>>,
    clips: Res<ClipLibrary>,
    reasons: Res<AttachReasons>,
    locals: Query<(Entity, &LocalPlayer, Option<&LocalRider>, Option<&Character>, Option<&Animator>)>,
) {
    let Some(spawn) = player.spawn.as_ref() else { return };
    let attached = spawn.unit.attached_to;
    for (entity, _local, rider, character, animator) in &locals {
        match (attached, rider) {
            (Some(a), None) => {
                let (Some(character), Some(animator), Some(mount)) = (character, animator, net.by_id.get(&a.obj_id))
                else {
                    continue;
                };
                let vehicle = matches!(mount.kind, UnitKind::Slave { .. });
                let side = if vehicle { None } else { reasons.side(spawn.obj_id, a.obj_id) };
                let clis = if vehicle {
                    vehicle_rider_clips(character, animator, &clips)
                } else {
                    let Some(db) = db.as_deref() else { continue };
                    riding_clips(character, mount.model_id, db, animator, &clips, side)
                };
                let Some(clis) = clis else { continue };
                let RidingClips { anim, geton } = clis;
                let boarding = Boarding::new(geton.as_ref());
                let (mount_id, point) = (a.obj_id, a.point);
                let body = if vehicle { None } else { db.as_deref().map(|db| db.mount_move(mount.model_id)) };
                let vehicle_body = if vehicle && point == 1 && db.as_deref().is_some_and(|db| db.is_land_vehicle(mount.model_id)) {
                    db.as_deref().map(|db| db.vehicle_body(mount.model_id))
                } else {
                    None
                };
                let seat = db.as_deref().map_or_else(Seat::default, |db| db.seat(mount.model_id, point));
                let (mount_pos, mount_yaw) = (Vec3::new(mount.pos.x, mount.pos.y, mount.pos.z), mount.yaw());
                let mount_entity = views.unit_entity(mount_id);
                debug!("local player rides {mount_id} at point {point}{}", if vehicle { " (vehicle)" } else { "" });
                commands.queue(move |world: &mut World| {
                    if let (Some(on), Some(mut animator)) = (geton, world.entity_mut(entity).get_mut::<Animator>()) {
                        animator.play_once_then(on, anim.idle.clone(), 0.15);
                    }
                    if point == 1
                        && let Some(mount_entity) = mount_entity
                    {
                        if let Some(body) = body {
                            world.entity_mut(mount_entity).insert(DrivenMount::new(mount_pos, mount_yaw, body));
                        } else if let Some(body) = vehicle_body {
                            world.entity_mut(mount_entity).insert(super::vehicle::DrivenSlave::new(mount_pos, mount_yaw, body));
                        }
                    }
                    let on_foot = world.entity_mut(entity).take::<Locomotion>();
                    world.entity_mut(entity).insert(LocalRider { mount: mount_id, point, anim, boarding, vehicle, seat, on_foot });
                });
            }
            (None, Some(rider)) => {
                let hop = if rider.vehicle {
                    Vec::new()
                } else {
                    (|| {
                        let mount = net.by_id.get(&rider.mount)?;
                        Some(hop_clips(character?, mount.model_id, db.as_deref()?, animator?, &clips))
                    })()
                    .unwrap_or_default()
                };
                let mount_entity = views.unit_entity(rider.mount);
                commands.queue(move |world: &mut World| {
                    if let Some(mount_entity) = mount_entity {
                        let mut mount = world.entity_mut(mount_entity);
                        mount.remove::<DrivenMount>();
                        mount.remove::<super::vehicle::DrivenSlave>();
                        if let Some(mut unit) = mount.get_mut::<RemoteUnit>() {
                            let (pos, facing) = (unit.pos, unit.facing);
                            unit.reset_track(pos, facing);
                        }
                        if let Some(mut motion) = mount.get_mut::<Motion>() {
                            *motion = Motion::default();
                        }
                    }
                    let Some(rider) = world.entity_mut(entity).take::<LocalRider>() else { return };
                    let (pos, facing) = world.entity(entity).get::<Mover>().map_or((Vec3::ZERO, 0.0), |m| (m.pos, m.facing));
                    start_hop(world, entity, hop, pos, facing, 1.0, rider.on_foot);
                });
            }
            _ => {}
        }
    }
}

/// Keeps the local player on its mount's seat and plays the rider clips for the mount's gait.
#[allow(clippy::type_complexity)]
pub fn pose_local_riders(
    time: Res<Time>,
    net: Res<Units>,
    views: Res<Views>,
    mut locals: Query<(&mut Mover, &mut Transform, &mut LocalRider, &mut Animator), Without<RemoteUnit>>,
    mounts: Query<(&RemoteUnit, &Animator, &Motion, Option<&MateGait>, &Transform), (Without<Rider>, Without<LocalRider>)>,
    globals: Query<&GlobalTransform>,
) {
    for (mut mover, mut transform, mut rider, mut animator) in &mut locals {
        let mount_entity = views.unit_entity(rider.mount);
        let mounted = mount_entity.and_then(|e| mounts.get(e).ok().map(|m| (e, m)));
        let (pos, facing, drive) = match mounted {
            Some((_e, (mount, mount_animator, _motion, _gait, _tf))) if rider.vehicle => {
                let (pos, facing) = seat_world(Vec3::new(mount.pos.x, mount.pos.y, mount.pos.z), mount.facing, mount.scale(), rider.seat);
                (pos, facing, MountDrive { tempo: mount_animator.speed, ..default() })
            }
            Some((e, (mount, mount_animator, motion, gait, mount_tf))) => {
                let bone = mount_animator.rig.skeleton.bones.iter().position(|b| b.name == seat_bone(rider.point)).and_then(|i| mount_animator.bones.get(i));
                let world = seat_now(mount_tf, globals.get(e).ok(), bone.and_then(|b| globals.get(*b).ok()), mount.pos);
                let swimming = motion.swimming;
                let pseudo = if swimming { 0.0 } else { gait.map_or(0.0, |g| g.pseudo_of(motion)) };
                (bevy_to_cry(world), mount.facing, MountDrive { pseudo, airborne: motion.airborne && !swimming, turn: gait.map_or(0.0, |g| g.turn), tempo: mount_animator.speed })
            }
            None => {
                let Some(m) = net.by_id.get(&rider.mount) else { continue };
                let (pos, facing) = if rider.vehicle {
                    seat_world(Vec3::new(m.pos.x, m.pos.y, m.pos.z), m.yaw(), m.scale, rider.seat)
                } else {
                    (Vec3::new(m.pos.x, m.pos.y, m.pos.z), m.yaw())
                };
                (pos, facing, MountDrive { tempo: 1.0, ..default() })
            }
        };
        mover.pos = pos;
        mover.facing = facing;
        mover.velocity = Vec3::ZERO;
        mover.vz = 0.0;
        mover.grounded = true;
        mover.swim = None;
        *transform = placement(pos, facing, 1.0);
        if let Some(boarding) = &mut rider.boarding {
            boarding.remaining -= time.delta_secs();
            if boarding.remaining > 0.0 {
                animator.speed = 1.0;
                continue;
            }
            rider.boarding = None;
        }
        rider.anim.update(&mut animator, drive);
    }
}

/// Plays the dismount hop: the entity stays at the anchor while the clip moves the body (the root motion), and where the last clip
/// leaves the body the unit stands from then on and its on-foot locomotion starts again.
#[allow(clippy::type_complexity)]
pub fn finish_dismount(
    time: Res<Time>,
    mut commands: Commands,
    mut dismounting: Query<(Entity, &mut Dismounting, &mut Animator, &mut Transform, Option<&mut Mover>, Option<&mut RemoteUnit>)>,
) {
    for (entity, mut state, mut animator, mut transform, mover, unit) in dismounting.iter_mut() {
        state.hop.remaining -= time.delta_secs();
        let mut over = false;
        if state.hop.remaining <= 0.0 {
            match state.hop.next(&animator.rig) {
                Some(clip) => animator.play(clip, false, 0.0),
                None => over = true,
            }
        }
        let (pos, facing, scale) = (state.hop.anchor, state.hop.facing, state.hop.scale);
        *transform = placement(pos, facing, scale);
        if let Some(mut mover) = mover {
            mover.pos = pos;
            mover.facing = facing;
            mover.velocity = Vec3::ZERO;
            mover.vz = 0.0;
        }
        if let Some(mut unit) = unit {
            unit.pos = pos;
            unit.facing = facing;
            if over {
                unit.reset_track(pos, facing);
            }
        }
        if !over {
            continue;
        }
        if let Some(mut on_foot) = state.on_foot.take() {
            on_foot.invalidate();
            commands.entity(entity).insert(on_foot);
        }
        commands.entity(entity).remove::<Dismounting>();
    }
}

/// Drives the mate the local player sits on as its driver: the player's keys move the mount and the
/// player's own movement packets are replaced by `CSMoveUnit` for the mate object (`MoveUnit`). The
/// mount's speed/body come from its actor model (`game_stances`, `actor_models`), the camera follows
/// the rider that `pose_local_riders` keeps on the seat, and a dismount removes `DrivenMount` and
/// gives control back to `control_player` at the seat position.
#[allow(clippy::too_many_arguments)]
pub fn drive_mount(
    time: Res<Time>,
    (keys, actions): (Res<ButtonInput<KeyCode>>, Res<crate::settings::Actions>),
    mouse: Res<ButtonInput<MouseButton>>,
    mut cameras: Query<&mut OrbitCamera>,
    player: Res<Player>,
    views: Res<Views>,
    (units, db): (Res<Units>, Option<Res<Db>>),
    terrain: Res<TerrainQuery>,
    collision: Res<WorldCollision>,
    water: Res<WaterQuery>,
    mut out: MessageWriter<NetCommand>,
    mut input_state: ResMut<PlayerInput>,
    typing: Option<Res<HudKeyboardFocus>>,
    over_ui: Option<Res<HudPointerOverUi>>,
    mut mounts: Query<(&mut RemoteUnit, &mut Transform, Option<&mut Motion>, Option<&mut DrivenMount>)>,
    mut commands: Commands,
) {
    let Some(mount_id) = driver_mount(&player) else { return };
    let Some(entity) = views.unit_entity(mount_id) else { return };
    let Ok((mut unit, mut transform, motion, drive)) = mounts.get_mut(entity) else { return };
    let Some(mut drive) = drive else {
        // The attach frame did not have the mount's entity ready; adopt it now.
        let Some(mount_unit) = units.get(mount_id) else { return };
        let body = db.as_deref().map_or_else(MountMove::default, |db| db.mount_move(mount_unit.model_id));
        let pos = Vec3::new(mount_unit.pos.x, mount_unit.pos.y, mount_unit.pos.z);
        commands.entity(entity).insert(DrivenMount::new(pos, mount_unit.yaw(), body));
        return;
    };
    let Ok(mut orbit) = cameras.single_mut() else { return };
    let typing = typing.is_some_and(|f| f.0);
    let over_ui = over_ui.is_some_and(|o| o.0);
    let pressed = |action: &str| !typing && actions.pressed(action, 0);
    let tapped = |action: &str| !typing && actions.just_pressed(action, 0);
    let dt = time.delta_secs().min(0.1);
    if dt <= 0.0 {
        return;
    }
    drive.since_sent += dt;
    if tapped("toggle_walk") {
        drive.walking = !drive.walking;
    }
    if tapped("autorun") {
        drive.autorun = !drive.autorun;
    }
    let back = pressed("moveback");
    if back {
        drive.autorun = false;
    }
    let both = mouse.pressed(MouseButton::Left) && mouse.pressed(MouseButton::Right) && !over_ui;
    let forward_input = (pressed("moveforward") || drive.autorun || both) as i32 as f32 - back as i32 as f32;
    let strafe = pressed("moveright") as i32 as f32 - pressed("moveleft") as i32 as f32;
    let turn = pressed("turnleft") as i32 as f32 - pressed("turnright") as i32 as f32;
    let jump = tapped("jump");
    let before_facing = drive.facing;
    if turn != 0.0 {
        let d = turn * drive.body.turn * dt;
        drive.facing += d;
        orbit.yaw += d;
    }
    let local_dir = Vec2::new(strafe, forward_input).normalize_or_zero();
    if mouse.pressed(MouseButton::Right) && !over_ui {
        drive.facing = orbit.yaw;
    } else if local_dir != Vec2::ZERO {
        drive.facing = approach_angle(drive.facing, orbit.yaw, MOUNT_ALIGN_RATE * dt);
    }
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let wanted = drive.step(local_dir, drive.walking != shift);
    let step = drive.smooth(wanted, dt);
    let old = drive.pos;
    let mut pos = old;
    if step != Vec2::ZERO {
        let candidate = collision.slide(pos, step * dt, drive.body.radius, drive.body.height, MOUNT_STEP_UP).pos;
        if terrain.height_at(candidate.x, candidate.y).is_some() {
            let ahead = Vec2::new(candidate.x, candidate.y) + step.normalize_or_zero() * MOUNT_CLIMB_LOOK;
            let rise = terrain.ground_at(ahead.x, ahead.y).map_or(0.0, |ground| ground.height - pos.z);
            let climbing = !drive.swimming && drive.grounded && rise > MOUNT_CLIMB_LOOK * MOUNT_MAX_CLIMB_TAN;
            let on_brush = climbing && collision.ground_below(candidate.x, candidate.y, pos.z, MOUNT_STEP_UP).is_some();
            if !climbing || on_brush {
                pos.x = candidate.x;
                pos.y = candidate.y;
            }
        }
    }
    let step_up = if drive.grounded || drive.swimming { MOUNT_STEP_UP } else { 0.05 };
    let ground = super::player::floor_at(&terrain, &collision, Vec3::new(pos.x, pos.y, old.z), step_up);
    let wet = water.water_at(pos).map(|(surface, _)| (surface, (surface - ground).max(0.0)));
    let want_swim = wet.is_some_and(|(_, depth)| depth > drive.body.height * 0.6);
    if want_swim {
        if !drive.swimming {
            drive.swimming = true;
            drive.grounded = false;
            drive.vz = 0.0;
        }
        if let Some((surface, _)) = wet {
            pos.z = surface - drive.body.height * 0.3;
        }
    } else if drive.swimming {
        drive.swimming = false;
        drive.grounded = false;
    }
    let mut landed = None;
    let mut jumped = false;
    if !drive.swimming {
        if drive.grounded {
            if pos.z - ground > MOUNT_DROP {
                drive.grounded = false;
                drive.vz = 0.0;
            } else {
                pos.z = ground;
            }
        }
        if drive.grounded && jump {
            drive.vz = (2.0 * drive.body.gravity * drive.body.jump_height).sqrt();
            drive.grounded = false;
            jumped = true;
        }
        if !drive.grounded {
            drive.vz -= drive.body.gravity * dt;
            pos.z += drive.vz * dt;
            if pos.z <= ground && drive.vz <= 0.0 {
                pos.z = ground;
                landed = Some(-drive.vz);
                drive.vz = 0.0;
                drive.grounded = true;
            }
        }
    }
    let actual = if dt > 0.0 { Vec2::new(pos.x - old.x, pos.y - old.y) / dt } else { Vec2::ZERO };
    let velocity = Vec3::new(actual.x, actual.y, if drive.swimming || !drive.grounded { drive.vz } else { 0.0 });
    // a mount that slides to a stop (`game_inertia`) is still moving
    let moving = (local_dir != Vec2::ZERO || drive.velocity != Vec2::ZERO) && actual.length() > 0.05;
    let airborne = !drive.grounded && !drive.swimming;
    drive.pos = pos;
    unit.pos = pos;
    unit.facing = drive.facing;
    *transform = placement(pos, drive.facing, unit.scale());
    if let Some(mut m) = motion {
        let speed = if moving { actual.length() } else { 0.0 };
        // the request, not the speed the inertia is at: coasting to a stop is idle, not a walk (cryaction `FUN_306747f0` zeroes it
        // under 0.5 m/s)
        let request = mount_anim::requested_pseudo(local_dir != Vec2::ZERO || drive.autorun, drive.walking != shift, drive.autorun, speed);
        *m = Motion { speed, local_dir, airborne, swimming: drive.swimming, auto: drive.autorun, request: Some(request) };
    }
    let turned = (drive.facing - before_facing).abs() > 1e-4;
    *input_state = PlayerInput { moving, jumped, turned };
    let active = moving || airborne || turned || (drive.swimming && moving);
    let due = drive.since_sent >= MOUNT_SEND_INTERVAL;
    let first = !drive.sent_first && active;
    let state = if drive.grounded || drive.swimming {
        MoveState::Ground
    } else if drive.vz > 0.0 {
        MoveState::Jump
    } else {
        MoveState::Fall
    };
    if (active && due) || (!active && drive.was_active) || first || landed.is_some() {
        let delta = [(local_dir.x * 127.0).round() as i8, (local_dir.y * 127.0).round() as i8, 0];
        out.write(NetCommand(drive.packet(mount_id, active, velocity, delta, state, landed)));
        drive.since_sent = 0.0;
        drive.sent_first = true;
    }
    drive.was_active = active;
}

/// `AA_FAKE_MOUNT=1`, a check for the riders (the server has no command that summons a mount): a horse (model 139) and a
/// rider that looks like the local player appear beside the player, the rider is attached to the horse, and the horse runs a
/// circle at 5 m/s, reported ten times a second like the walkthrough's circle. `AA_FAKE_MOUNT=still` parks the horse 4 m from the
/// player instead (for a close look at the seat).
pub fn fake_mount(time: Res<Time>, player: Res<Player>, mut units: ResMut<Units>, mut events: MessageWriter<NetEvent>, mut state: Local<Option<(f32, f32)>>) {
    const HORSE: u32 = 9_000_001;
    const RIDER: u32 = 9_000_002;
    const RADIUS: f32 = 8.0;
    const SPEED: f32 = 5.0;
    if std::env::var("AA_FAKE_MOUNT").is_err() {
        return;
    }
    let Some(spawn) = &player.spawn else { return };
    let now = time.elapsed_secs();
    let still = std::env::var("AA_FAKE_MOUNT").is_ok_and(|v| v == "still");
    let centre = Pos::new(spawn.unit.pos.x + if still { 4.0 } else { RADIUS + 3.0 }, spawn.unit.pos.y, spawn.unit.pos.z);
    let (started, last_sent) = *state.get_or_insert_with(|| {
        let mut horse = spawn.unit.clone();
        horse.obj_id = HORSE;
        horse.name = "Horse".into();
        horse.kind = UnitKind::Mate { tl_id: 1, template_id: 0, owner_id: 0 };
        horse.model_id = 139;
        horse.equipment = Equipment::default();
        horse.model_params = ModelParams::default();
        horse.pos = Pos::new(centre.x, centre.y, centre.z);
        let mut rider = spawn.unit.clone();
        rider.obj_id = RIDER;
        rider.name = "Rider".into();
        rider.pos = horse.pos;
        units.by_id.insert(HORSE, horse);
        units.by_id.insert(RIDER, rider);
        (now, 0.0)
    });
    // seat the rider once the horse has been announced for a moment
    if now - started > 4.0
        && let Some(rider) = units.by_id.get_mut(&RIDER)
    {
        rider.attached_to.get_or_insert(Attachment { obj_id: HORSE, point: 1 });
    }
    if now - last_sent < 0.1 || now - started < 2.0 || (still && last_sent > 0.0) {
        return;
    }
    let t = if still { 0.0 } else { now - started };
    let angle = t * SPEED / RADIUS;
    let pos = Pos::new(centre.x + RADIUS * angle.cos() - RADIUS, centre.y + RADIUS * angle.sin(), centre.z);
    let yaw = (SPEED * angle.sin()).atan2(SPEED * angle.cos());
    let mut movement = Movement::unit(pos, yaw, (t * 1000.0) as u32);
    if !still {
        movement.velocity = [(-yaw.sin() * SPEED * 1024.0) as i16, (yaw.cos() * SPEED * 1024.0) as i16, 0];
        movement.delta_movement = [0, 127, 0];
    }
    if let Some(horse) = units.by_id.get_mut(&HORSE) {
        horse.pos = pos;
    }
    events.write(NetEvent(Event::UnitMoved { obj_id: HORSE, movement }));
    *state = Some((started, now));
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_seat_follows_the_mount_of_this_frame_not_the_last_one() {
        let before = GlobalTransform::from(Transform::from_xyz(0.0, 0.0, 0.0));
        let bone = GlobalTransform::from(Transform::from_xyz(0.0, 1.0, 2.0));
        let now = Transform::from_xyz(5.0, 0.0, 0.0);
        let seat = seat_now(&now, Some(&before), Some(&bone), Vec3::ZERO);
        assert!((seat - Vec3::new(5.0, 1.0, 2.0)).length() < 1e-4, "{seat:?}");
    }

    use super::*;

    fn horse() -> MountMove {
        MountMove { walk: 1.8, run: 9.0, radius: 1.0, height: 1.0, jump_height: 1.3, gravity: 30.0, turn: 360.0_f32.to_radians(), inertia: 4.0, accel: 11.0 }
    }

    #[test]
    fn mounted_forward_input_makes_a_mate_move_packet() {
        let mut drive = DrivenMount::new(Vec3::new(10.0, 20.0, 30.0), 0.0, horse());
        let step = drive.step(Vec2::Y, false);
        assert!((step.length() - 9.0).abs() < 1e-4, "horse run speed: {step:?}");
        drive.pos += Vec3::new(step.x, step.y, 0.0) * 0.1;
        let command = drive.packet(0x445566, true, Vec3::new(step.x, step.y, 0.0), [0, 127, 0], MoveState::Ground, None);
        match command {
            Command::MoveUnit { obj_id, command } => {
                assert_eq!(obj_id, 0x445566, "the packet names the mate object, not the player");
                assert!(command.moving);
                assert_eq!(command.yaw, 0.0);
                assert!((command.pos.y - 20.9).abs() < 0.01, "{:?}", command.pos);
                assert_eq!(command.velocity, [0, 9 * 1024, 0]);
                assert_eq!(command.delta_movement, [0, 127, 0]);
            }
            other => panic!("not a mate movement: {other:?}"),
        }
    }

    #[test]
    fn the_mount_speed_follows_the_wanted_one_with_the_inertia_of_its_actor_model() {
        let mut drive = DrivenMount::new(Vec3::ZERO, 0.0, horse());
        let run = Vec2::new(0.0, 9.0);
        // 0.1 s after the key press the horse is at 1 - e^(-1.1) = 67 % of the run (accel 11), not at 100 %
        let v = drive.smooth(run, 0.1);
        assert!((v.y - 9.0 * (1.0 - (-1.1_f32).exp())).abs() < 1e-3, "{v:?}");
        for _ in 0..30 {
            drive.smooth(run, 0.1);
        }
        assert!((drive.velocity.y - 9.0).abs() < 0.01, "it gets there");
        // letting go: inertia 4 slides it down slower than the 11 it sped up with
        let v = drive.smooth(Vec2::ZERO, 0.1);
        assert!((v.y - 9.0 * (-0.4_f32).exp()).abs() < 0.02, "{v:?}");
        for _ in 0..40 {
            drive.smooth(Vec2::ZERO, 0.1);
        }
        assert_eq!(drive.velocity, Vec2::ZERO, "and stops");
    }

    #[test]
    fn mount_walk_and_swim_speeds_follow_the_model() {
        let drive = DrivenMount::new(Vec3::ZERO, 0.0, horse());
        assert!((drive.step(Vec2::Y, true).length() - 1.8).abs() < 1e-4, "walk");
        assert!((drive.step(Vec2::Y, false).length() - 9.0).abs() < 1e-4, "run");
        let swimming = DrivenMount { swimming: true, ..DrivenMount::new(Vec3::ZERO, 0.0, horse()) };
        assert!((swimming.step(Vec2::Y, false).length() - 4.05).abs() < 1e-4, "swim is 45 % of the run");
    }

    #[test]
    fn only_the_driver_seat_drives_and_dismount_restores_on_foot_control() {
        assert_eq!(mounted_driver(Some(Attachment { obj_id: 201, point: 1 })), Some(201));
        assert_eq!(mounted_driver(Some(Attachment { obj_id: 201, point: 2 })), None);
        assert_eq!(mounted_driver(None), None);
        assert!(!super::super::player::on_foot_attached(Some(Attachment { obj_id: 201, point: 1 })));
        assert!(!super::super::player::on_foot_attached(Some(Attachment { obj_id: 201, point: 2 })));
        assert!(super::super::player::on_foot_attached(None));
    }
}

#[cfg(test)]
mod attach_tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.add_message::<NetEvent>().init_resource::<AttachReasons>().add_systems(Update, record_attach_reasons);
        app
    }

    #[test]
    fn the_mount_controller_is_announced_at_its_start_and_its_end() {
        let mut flow = MountFlow::default();
        flow.begin(5, 1, 1);
        assert_eq!(flow.announce, [controller_kind::MOUNT_HORSE]);
        let mut facing = 0.0;
        flow.steer(Vec3::ZERO, &mut facing, true);
        assert!(!flow.active());
        assert_eq!(flow.announce, [controller_kind::MOUNT_HORSE, controller_kind::NONE]);
        flow.steer(Vec3::ZERO, &mut facing, true);
        assert_eq!(flow.announce.len(), 2, "an idle flow reports nothing");
    }

    #[test]
    fn the_unmount_controller_lasts_500_ms_for_reason_9_and_continues_a_dismiss_after_1_s_for_reason_10() {
        let mut flow = MountFlow::default();
        flow.start_unmount(9, None);
        assert_eq!(flow.announce, [controller_kind::UNMOUNT_HORSE]);
        assert!(flow.tick_unmount(10.0).is_none());
        assert!(flow.tick_unmount(10.4).is_none());
        assert!(flow.tick_unmount(10.5).is_none());
        assert_eq!(flow.announce, [controller_kind::UNMOUNT_HORSE, controller_kind::NONE]);
        let dismiss = Command::Mate(aa_net::mate::MateCommand::Dismiss { tl_id: 3 });
        flow.start_unmount(10, None);
        flow.start_unmount(10, Some(dismiss));
        assert_eq!(flow.announce.len(), 3, "a running controller is not started twice");
        assert!(flow.tick_unmount(20.0).is_none());
        assert!(matches!(flow.tick_unmount(21.0), Some(Command::Mate(aa_net::mate::MateCommand::Dismiss { tl_id: 3 }))));
        assert_eq!(flow.announce.last(), Some(&controller_kind::NONE));
        flow.start_unmount(3, None);
        assert_eq!(flow.announce.len(), 4, "other reasons have no controller");
    }

    #[test]
    fn the_attach_reason_picks_the_side_until_the_detach() {
        let mut app = app();
        app.world_mut().write_message(NetEvent(Event::UnitAttached { obj_id: 7, mount: 90, point: 1, reason: 2 }));
        app.update();
        let reasons = app.world().resource::<AttachReasons>();
        assert_eq!(reasons.side(7, 90), Some(Side::Right));
        assert_eq!(reasons.side(7, 91), None, "another mount is another attach");
        assert_eq!(reasons.side(8, 90), None, "a rider that was already seated has no get-on");
        app.world_mut().write_message(NetEvent(Event::UnitDetached { obj_id: 7, reason: 9 }));
        app.update();
        assert_eq!(app.world().resource::<AttachReasons>().side(7, 90), None);
    }

    #[test]
    fn a_get_on_only_times_the_clip() {
        assert!(Boarding::new(None).is_none(), "no clip, no boarding: the rider sits at once");
    }

    #[test]
    fn the_mount_walk_turns_to_the_mate_and_any_key_cancels_it() {
        let mut flow = MountFlow::default();
        assert_eq!(flow.steer(Vec3::ZERO, &mut 0.0, false), (false, false), "nothing in progress");
        flow.begin(5, 77, 1);
        assert!(flow.active());
        flow.pending.as_mut().unwrap().walk_to = Some((Vec2::new(0.0, 6.0), false));
        let mut facing = 1.0;
        assert_eq!(flow.steer(Vec3::ZERO, &mut facing, false), (true, false));
        assert!(facing.abs() < 1e-5, "faces north: {facing}");
        flow.pending.as_mut().unwrap().walk_to = Some((Vec2::new(0.0, 1.0), true));
        assert_eq!(flow.steer(Vec3::ZERO, &mut facing, false), (true, true), "slow near the mate");
        assert_eq!(flow.steer(Vec3::ZERO, &mut facing, true), (false, false), "a key press cancels");
        assert!(!flow.active());
    }

    #[test]
    fn the_rider_follows_the_mount_pseudo_speed() {
        let all = (true, true, true);
        let drive = |pseudo: f32| MountDrive { pseudo, tempo: 1.0, ..default() };
        assert_eq!(choose_ride(drive(0.0), Band::Run, all), (Ride::Idle, Band::Idle));
        assert_eq!(choose_ride(drive(0.2), Band::Idle, all), (Ride::Walk, Band::Walk), "a walking horse");
        assert_eq!(choose_ride(drive(1.0), Band::Walk, all), (Ride::Run, Band::Run));
        assert_eq!(choose_ride(drive(1.5), Band::Run, all).0, Ride::Sprint);
        assert_eq!(choose_ride(drive(1.5), Band::Run, (true, false, true)).0, Ride::Run, "no sprint clip: the run");
        assert_eq!(choose_ride(MountDrive { turn: 90.0, ..drive(1.0) }, Band::Run, all).0, Ride::Turn(true));
        assert_eq!(choose_ride(MountDrive { turn: -90.0, ..drive(1.0) }, Band::Run, all).0, Ride::Turn(false));
        assert_eq!(choose_ride(MountDrive { turn: 90.0, ..drive(0.4) }, Band::Walk, all).0, Ride::Walk, "turn blends are for the run");
        assert_eq!(choose_ride(MountDrive { airborne: true, ..drive(1.0) }, Band::Run, all).0, Ride::Air);
        assert_eq!(choose_ride(MountDrive { airborne: true, ..drive(1.0) }, Band::Run, (true, true, false)).0, Ride::Run, "no jump clips");
    }
}
