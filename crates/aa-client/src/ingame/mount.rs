//! Riders: a unit the server attached to a mount (`SCUnitAttached`: a mate such as a horse or a lion) sits on the mount's seat
//! bone and plays the rider's clips of that kind of mount.
//!
//! The server moves a rider with its mount: it sends no movement for the rider, only for the mount. The rider's own clips
//! come from its character `.cal` (`horse_ba_relaxed_idle`, `horse_mo_normal_walk_f`, `horse_mo_normal_run_f`, and the
//! `lion_`, `wildboar_` and `pangolin_` sets; the `models.mount_pose_id` of the mount says which), the mount's from its own
//! `.cal` as for any creature. The seat is the mount's bone `bone_spine_driver` (attach point 1) or `bone_spine_passenger`.

use aa_anim::pose::BoundClip;
use aa_anim::{Animator, Cal, Character, ClipLibrary};
use aa_net::model::{Attachment, Equipment, ModelParams, Movement, UnitKind};
use aa_net::state::{Player, Units};
use aa_net::{Event, NetEvent, Pos};
use aa_world::coords::bevy_to_cry;
use bevy::prelude::*;

use super::anim::{Locomotion, Motion, bind_clip};
use super::data::Db;
use super::player::{LocalPlayer, Mover, placement};
use super::units::{RemoteUnit, Views};

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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Gait {
    Idle,
    Walk,
    Run,
}

/// The clips of a rider: the seat idle and the walk/run gaits of the mount's kind, then the
/// authored get-on / get-off one-shots (`global_player.cal`).
struct RidingClips {
    idle: BoundClip,
    walk: Option<BoundClip>,
    run: Option<BoundClip>,
    geton: Option<BoundClip>,
    getoff: Option<BoundClip>,
}

fn riding_clips(character: &Character, mount_model: u32, db: &Db, animator: &Animator, clips: &ClipLibrary) -> Option<RidingClips> {
    let prefix = rider_prefix(db.mount_pose(mount_model))?;
    let cal = Cal::shared(&character.assets.cal).ok()?;
    let bind = |name: String| bind_clip(&animator.rig, &cal, clips, &name);
    let idle = bind(format!("{prefix}_ba_relaxed_idle")).or_else(|| bind("fist_ba_relaxed_idle".into()))?;
    let run = bind(format!("{prefix}_mo_normal_run_f"));
    let walk = bind(format!("{prefix}_mo_normal_walk_f")).or_else(|| run.clone());
    // The authored set only has horse / lion / bear one-shots; other mount poses use the horse pair.
    let one = |side: &str| {
        bind(format!("fist_ac_{prefix}_{side}"))
            .or_else(|| bind(format!("fist_ac_horse_{side}")))
    };
    Some(RidingClips { idle, walk, run, geton: one("l_geton"), getoff: one("l_getoff") })
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
    /// Seconds of the get-on one-shot still to play before the gait clips take over.
    boarding: Option<f32>,
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
    boarding: Option<f32>,
    on_foot: Option<Locomotion>,
}

/// A get-off clip playing before the on-foot locomotion starts again.
#[derive(Component)]
pub struct Dismounting {
    remaining: f32,
    on_foot: Option<Locomotion>,
}

/// Seats the units the server attached to a mount and lets the ones that got off walk again.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn start_stop_riding(
    mut commands: Commands,
    net: Res<Units>,
    db: Option<Res<Db>>,
    clips: Res<ClipLibrary>,
    units: Query<(Entity, &RemoteUnit, Option<&Rider>, Option<&Character>, Option<&Animator>)>,
) {
    for (entity, unit, rider, character, animator) in &units {
        let attached = net.by_id.get(&unit.obj_id).and_then(|u| u.attached_to);
        match (attached, rider) {
            (Some(a), None) => {
                // wait for the animator (a unit far away has none) and the mount's data
                let (Some(db), Some(character), Some(animator), Some(mount)) = (db.as_deref(), character, animator, net.by_id.get(&a.obj_id)) else { continue };
                let Some(clis) = riding_clips(character, mount.model_id, db, animator, &clips) else { continue };
                let (mount_id, point) = (a.obj_id, a.point);
                let RidingClips { idle, walk, run, geton, .. } = clis;
                let boarding = geton.as_ref().map(|c| c.clip.duration);
                debug!("unit {} rides {} (model {}) at point {point}", unit.obj_id, mount_id, mount.model_id);
                commands.queue(move |world: &mut World| {
                    if let (Some(on), Some(mut animator)) = (geton, world.entity_mut(entity).get_mut::<Animator>()) {
                        animator.play_once_then(on, idle.clone(), 0.15);
                    }
                    let on_foot = world.entity_mut(entity).take::<Locomotion>();
                    world.entity_mut(entity).insert(Rider { mount: mount_id, point, idle, walk, run, gait: None, boarding, on_foot });
                });
            }
            (None, Some(_)) => {
                let getoff = (|| {
                    let character = character?;
                    let db = db.as_deref()?;
                    let animator = animator?;
                    let mount = net.by_id.get(&rider?.mount)?;
                    Some(riding_clips(character, mount.model_id, db, animator, &clips)?.getoff)
                })()
                .flatten();
                let remaining = getoff.as_ref().map_or(0.0, |c| c.clip.duration);
                commands.queue(move |world: &mut World| {
                    if let (Some(off), Some(mut animator)) = (getoff, world.entity_mut(entity).get_mut::<Animator>()) {
                        animator.play(off, false, 0.15);
                    }
                    let Some(rider) = world.entity_mut(entity).take::<Rider>() else { return };
                    if remaining > 0.0 {
                        world.entity_mut(entity).insert(Dismounting { remaining, on_foot: rider.on_foot });
                    } else if let Some(mut on_foot) = rider.on_foot {
                        on_foot.invalidate();
                        world.entity_mut(entity).insert(on_foot);
                    }
                    // it stands where the mount was: the movement packets of the unit itself carry on from here
                    if let Some(mut unit) = world.entity_mut(entity).get_mut::<RemoteUnit>() {
                        let (pos, facing) = (unit.pos, unit.facing);
                        unit.reset_track(pos, facing);
                    }
                    if let Some(mut motion) = world.entity_mut(entity).get_mut::<Motion>() {
                        *motion = Motion::default();
                    }
                });
            }
            _ => {}
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
                (Vec3::new(m.pos.x, m.pos.y, m.pos.z), m.yaw(), 0.0, 1.0)
            }
        };
        unit.pos = pos;
        unit.facing = facing;
        *transform = placement(pos, facing, unit.scale());
        if let Some(boarding) = &mut rider.boarding {
            *boarding -= time.delta_secs();
            if *boarding > 0.0 {
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

/// Seats the local player when the server attached its own object (`SCUnitAttached`), and starts the
/// authored get-off clip on the detach that follows an unmount or dismiss.
#[allow(clippy::type_complexity)]
pub fn start_stop_local_riding(
    mut commands: Commands,
    player: Res<Player>,
    net: Res<Units>,
    db: Option<Res<Db>>,
    clips: Res<ClipLibrary>,
    locals: Query<(Entity, &LocalPlayer, Option<&LocalRider>, Option<&Character>, Option<&Animator>)>,
) {
    let Some(spawn) = player.spawn.as_ref() else { return };
    let attached = spawn.unit.attached_to;
    for (entity, _local, rider, character, animator) in &locals {
        match (attached, rider) {
            (Some(a), None) => {
                let (Some(db), Some(character), Some(animator), Some(mount)) =
                    (db.as_deref(), character, animator, net.by_id.get(&a.obj_id))
                else {
                    continue;
                };
                let Some(clis) = riding_clips(character, mount.model_id, db, animator, &clips) else { continue };
                let RidingClips { idle, walk, run, geton, .. } = clis;
                let boarding = geton.as_ref().map(|c| c.clip.duration);
                let (mount_id, point) = (a.obj_id, a.point);
                debug!("local player rides {mount_id} at point {point}");
                commands.queue(move |world: &mut World| {
                    if let (Some(on), Some(mut animator)) = (geton, world.entity_mut(entity).get_mut::<Animator>()) {
                        animator.play_once_then(on, idle.clone(), 0.15);
                    }
                    let on_foot = world.entity_mut(entity).take::<Locomotion>();
                    world.entity_mut(entity).insert(LocalRider { mount: mount_id, point, idle, walk, run, gait: None, boarding, on_foot });
                });
            }
            (None, Some(rider)) => {
                let getoff = (|| {
                    let character = character?;
                    let db = db.as_deref()?;
                    let animator = animator?;
                    let mount = net.by_id.get(&rider.mount)?;
                    Some(riding_clips(character, mount.model_id, db, animator, &clips)?.getoff)
                })()
                .flatten();
                let remaining = getoff.as_ref().map_or(0.0, |c| c.clip.duration);
                commands.queue(move |world: &mut World| {
                    if let (Some(off), Some(mut animator)) = (getoff, world.entity_mut(entity).get_mut::<Animator>()) {
                        animator.play(off, false, 0.15);
                    }
                    let Some(rider) = world.entity_mut(entity).take::<LocalRider>() else { return };
                    if remaining > 0.0 {
                        world.entity_mut(entity).insert(Dismounting { remaining, on_foot: rider.on_foot });
                    } else if let Some(mut on_foot) = rider.on_foot {
                        on_foot.invalidate();
                        world.entity_mut(entity).insert(on_foot);
                    }
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
            Some((e, (mount, mount_animator, motion))) => {
                let bone = mount_animator.rig.skeleton.bones.iter().position(|b| b.name == seat_bone(rider.point)).and_then(|i| mount_animator.bones.get(i));
                let seat = bone.and_then(|b| globals.get(*b).ok()).or_else(|| globals.get(e).ok());
                let world = seat.map_or_else(|| aa_world::cry_to_bevy(mount.pos), |g| g.translation());
                (bevy_to_cry(world), mount.facing, motion.speed, mount_animator.speed)
            }
            None => {
                let Some(m) = net.by_id.get(&rider.mount) else { continue };
                (Vec3::new(m.pos.x, m.pos.y, m.pos.z), m.yaw(), 0.0, 1.0)
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
            *boarding -= time.delta_secs();
            if *boarding > 0.0 {
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

/// Restores the on-foot locomotion once the get-off one-shot finished.
pub fn finish_dismount(time: Res<Time>, mut commands: Commands, mut dismounting: Query<(Entity, &mut Dismounting)>) {
    for (entity, mut state) in &mut dismounting {
        state.remaining -= time.delta_secs();
        if state.remaining > 0.0 {
            continue;
        }
        if let Some(mut on_foot) = state.on_foot.take() {
            on_foot.invalidate();
            commands.entity(entity).insert(on_foot);
        }
        commands.entity(entity).remove::<Dismounting>();
    }
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
