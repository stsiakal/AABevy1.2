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
use aa_net::state::{Player, Units};
use aa_net::{Command, Event, MoveCommand, MoveState, NetCommand, NetEvent, Pos};
use aa_world::coords::bevy_to_cry;
use aa_world::{TerrainQuery, WaterQuery, WorldCollision};
use bevy::prelude::*;

use super::anim::{Locomotion, Motion, bind_clip, bind_clip_with};
use super::camera::OrbitCamera;
use super::data::{Db, MountMove, Seat};
use super::mount_anim::{self, Hop, Side};
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Gait {
    Idle,
    Walk,
    Run,
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

/// The clips of a rider: the seat idle and the walk/run gaits of the mount kind, then the get-on clip of the side.
struct RidingClips {
    idle: BoundClip,
    walk: Option<BoundClip>,
    run: Option<BoundClip>,
    geton: Option<BoundClip>,
}

/// Binds the first of `names` the character has, with its root motion played (the get-on and get-off clips lift the body).
fn bind_moving(character: &Character, animator: &Animator, clips: &ClipLibrary, names: &[String]) -> Option<BoundClip> {
    let cal = Cal::shared(&character.assets.cal).ok()?;
    names.iter().find_map(|name| bind_clip_with(&animator.rig, &cal, clips, name, false))
}

/// The get-on clip of a rider that boards from `side` (`None` for a rider that was already seated).
fn riding_clips(character: &Character, mount_model: u32, db: &Db, animator: &Animator, clips: &ClipLibrary, side: Option<Side>) -> Option<RidingClips> {
    let prefix = rider_prefix(db.mount_pose(mount_model))?;
    let cal = Cal::shared(&character.assets.cal).ok()?;
    let bind = |name: String| bind_clip(&animator.rig, &cal, clips, &name);
    let idle = bind(format!("{prefix}_ba_relaxed_idle")).or_else(|| bind("fist_ba_relaxed_idle".into()))?;
    let run = bind(format!("{prefix}_mo_normal_run_f"));
    let walk = bind(format!("{prefix}_mo_normal_walk_f")).or_else(|| run.clone());
    let geton = side.and_then(|side| bind_moving(character, animator, clips, &mount_anim::geton_names(prefix, side)));
    Some(RidingClips { idle, walk, run, geton })
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
    Some(RidingClips { idle, walk: None, run: None, geton: None })
}

/// A unit sitting on a mount.
#[derive(Component)]
pub struct Rider {
    mount: u32,
    point: u8,
    idle: BoundClip,
    walk: Option<BoundClip>,
    run: Option<BoundClip>,
    gait: Option<Gait>,
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
    idle: BoundClip,
    walk: Option<BoundClip>,
    run: Option<BoundClip>,
    gait: Option<Gait>,
    boarding: Option<Boarding>,
    /// A slave/vehicle seat rather than a mate's bone: the player sits at `seat` in the model space.
    vehicle: bool,
    seat: Seat,
    on_foot: Option<Locomotion>,
}

/// A get-on clip in progress: the rider stands at the approach point of its side and the clip lifts it onto the saddle.
struct Boarding {
    remaining: f32,
    side: Side,
    /// The root offset of the clip at its end (model space).
    end: Vec3,
}

impl Boarding {
    fn new(clip: Option<&BoundClip>, side: Option<Side>, animator: &Animator) -> Option<Boarding> {
        let (clip, side) = (clip?, side?);
        Some(Boarding { remaining: clip.clip.duration, side, end: mount_anim::root_offset(&animator.rig, clip, clip.clip.duration) })
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
                let RidingClips { idle, walk, run, geton } = clis;
                let boarding = Boarding::new(geton.as_ref(), side, animator);
                debug!("unit {} rides {} (model {}) at point {point}{}", unit.obj_id, mount_id, mount.model_id, if vehicle { " (slave seat)" } else { "" });
                commands.queue(move |world: &mut World| {
                    if let (Some(on), Some(mut animator)) = (geton, world.entity_mut(entity).get_mut::<Animator>()) {
                        animator.play_once_then(on, idle.clone(), 0.15);
                    }
                    let on_foot = world.entity_mut(entity).take::<Locomotion>();
                    world.entity_mut(entity).insert(Rider { mount: mount_id, point, idle, walk, run, gait: None, boarding, vehicle, seat, on_foot });
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
    mounts: Query<(&RemoteUnit, &Animator, &Motion), Without<Rider>>,
    globals: Query<&GlobalTransform>,
) {
    for (mut unit, mut transform, mut rider, mut animator) in &mut riders {
        let mount_entity = views.unit_entity(rider.mount);
        let mounted = mount_entity.and_then(|e| mounts.get(e).ok().map(|m| (e, m)));
        let (pos, facing, speed, tempo) = match mounted {
            Some((_e, (mount, mount_animator, motion))) if rider.vehicle => {
                let (pos, facing) = seat_world(Vec3::new(mount.pos.x, mount.pos.y, mount.pos.z), mount.facing, mount.scale(), rider.seat);
                (pos, facing, motion.speed, mount_animator.speed)
            }
            Some((e, (mount, mount_animator, motion))) => {
                // the seat bone of the mount's skeleton, in the world
                let bone = mount_animator.rig.skeleton.bones.iter().position(|b| b.name == seat_bone(rider.point)).and_then(|i| mount_animator.bones.get(i));
                let seat = bone.and_then(|b| globals.get(*b).ok()).or_else(|| globals.get(e).ok());
                let world = seat.map_or_else(|| aa_world::cry_to_bevy(mount.pos), |g| g.translation());
                (bevy_to_cry(world), mount.facing, motion.speed, mount_animator.speed)
            }
            None => {
                // the mount is not drawn: stay with where the server says it is
                let Some(m) = net.by_id.get(&rider.mount) else { continue };
                let (pos, facing) = if rider.vehicle {
                    seat_world(Vec3::new(m.pos.x, m.pos.y, m.pos.z), m.yaw(), m.scale, rider.seat)
                } else {
                    (Vec3::new(m.pos.x, m.pos.y, m.pos.z), m.yaw())
                };
                (pos, facing, 0.0, 1.0)
            }
        };
        // while the get-on clip plays the rider stands at the approach point of its side; the clip walks it onto the saddle
        let (pos, facing) = match &rider.boarding {
            Some(b) if !rider.vehicle => boarding_pose(b, pos, facing, unit.scale()),
            _ => (pos, facing),
        };
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
        let gait = if speed < 0.3 {
            Gait::Idle
        } else if speed < 3.0 || rider.run.is_none() {
            Gait::Walk
        } else {
            Gait::Run
        };
        if rider.gait != Some(gait) {
            let clip = match gait {
                Gait::Idle => Some(rider.idle.clone()),
                Gait::Walk => rider.walk.clone().or_else(|| rider.run.clone()),
                Gait::Run => rider.run.clone(),
            };
            if let Some(clip) = clip {
                animator.play(clip, true, 0.2);
                animator.random_idle = None;
            }
            rider.gait = Some(gait);
        }
        animator.speed = if gait == Gait::Idle { 1.0 } else { tempo };
    }
}

/// Where a rider stands (and which way it faces) while its get-on clip plays, for the saddle at `seat` and a mount facing `mount_facing`.
fn boarding_pose(boarding: &Boarding, seat: Vec3, mount_facing: f32, scale: f32) -> (Vec3, f32) {
    let facing = boarding.side.rider_facing(mount_facing);
    (mount_anim::approach_point(seat, facing, boarding.end, scale), facing)
}

/// Seats the local player when the server attached its own object (`SCUnitAttached`), and starts the
/// authored get-off clip on the detach that follows an unmount or dismiss.
#[allow(clippy::type_complexity)]
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
                let RidingClips { idle, walk, run, geton } = clis;
                let boarding = Boarding::new(geton.as_ref(), side, animator);
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
                        animator.play_once_then(on, idle.clone(), 0.15);
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
                    world.entity_mut(entity).insert(LocalRider { mount: mount_id, point, idle, walk, run, gait: None, boarding, vehicle, seat, on_foot });
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
    mounts: Query<(&RemoteUnit, &Animator, &Motion), (Without<Rider>, Without<LocalRider>)>,
    globals: Query<&GlobalTransform>,
) {
    for (mut mover, mut transform, mut rider, mut animator) in &mut locals {
        let mount_entity = views.unit_entity(rider.mount);
        let mounted = mount_entity.and_then(|e| mounts.get(e).ok().map(|m| (e, m)));
        let (pos, facing, speed, tempo) = match mounted {
            Some((_e, (mount, mount_animator, motion))) if rider.vehicle => {
                let (pos, facing) = seat_world(Vec3::new(mount.pos.x, mount.pos.y, mount.pos.z), mount.facing, mount.scale(), rider.seat);
                (pos, facing, motion.speed, mount_animator.speed)
            }
            Some((e, (mount, mount_animator, motion))) => {
                let bone = mount_animator.rig.skeleton.bones.iter().position(|b| b.name == seat_bone(rider.point)).and_then(|i| mount_animator.bones.get(i));
                let seat = bone.and_then(|b| globals.get(*b).ok()).or_else(|| globals.get(e).ok());
                let world = seat.map_or_else(|| aa_world::cry_to_bevy(mount.pos), |g| g.translation());
                (bevy_to_cry(world), mount.facing, motion.speed, mount_animator.speed)
            }
            None => {
                let Some(m) = net.by_id.get(&rider.mount) else { continue };
                let (pos, facing) = if rider.vehicle {
                    seat_world(Vec3::new(m.pos.x, m.pos.y, m.pos.z), m.yaw(), m.scale, rider.seat)
                } else {
                    (Vec3::new(m.pos.x, m.pos.y, m.pos.z), m.yaw())
                };
                (pos, facing, 0.0, 1.0)
            }
        };
        let (pos, facing) = match &rider.boarding {
            Some(b) if !rider.vehicle => boarding_pose(b, pos, facing, 1.0),
            _ => (pos, facing),
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
        let gait = if speed < 0.3 {
            Gait::Idle
        } else if speed < 3.0 || rider.run.is_none() {
            Gait::Walk
        } else {
            Gait::Run
        };
        if rider.gait != Some(gait) {
            let clip = match gait {
                Gait::Idle => Some(rider.idle.clone()),
                Gait::Walk => rider.walk.clone().or_else(|| rider.run.clone()),
                Gait::Run => rider.run.clone(),
            };
            if let Some(clip) = clip {
                animator.play(clip, true, 0.2);
                animator.random_idle = None;
            }
            rider.gait = Some(gait);
        }
        animator.speed = if gait == Gait::Idle { 1.0 } else { tempo };
    }
}

/// Plays the dismount hop: the entity stays at the anchor while the clip moves the body (the root motion), and where the last clip
/// leaves the body the unit stands from then on and its on-foot locomotion starts again.
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
    let step = drive.step(local_dir, drive.walking != shift);
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
    let moving = local_dir != Vec2::ZERO && actual.length() > 0.05;
    let airborne = !drive.grounded && !drive.swimming;
    drive.pos = pos;
    unit.pos = pos;
    unit.facing = drive.facing;
    *transform = placement(pos, drive.facing, unit.scale());
    if let Some(mut m) = motion {
        *m = Motion { speed: if moving { actual.length() } else { 0.0 }, local_dir, airborne, swimming: drive.swimming };
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
    use super::*;

    fn horse() -> MountMove {
        MountMove { walk: 1.8, run: 9.0, radius: 1.0, height: 1.0, jump_height: 1.3, gravity: 30.0, turn: 360.0_f32.to_radians() }
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
    fn a_get_on_stands_the_rider_on_the_approach_side() {
        let boarding = Boarding { remaining: 1.0, side: Side::Right, end: Vec3::new(0.0, 1.0, 1.888) };
        let (pos, facing) = boarding_pose(&boarding, Vec3::new(10.0, 10.0, 5.0), 0.0, 1.0);
        assert!((pos - Vec3::new(11.0, 10.0, 3.112)).length() < 1e-4, "{pos}");
        assert!((facing - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
    }
}
