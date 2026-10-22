//! Owned ships (rowboat, clipper, merchant ship, fishing boat): boarding through the native
//! `CSBindSlave`/`CSDiscardSlave` seats, the helmsman's `ShipRequestMoveType` (type 4) and an
//! on-deck walking mode.
//!
//! The server's ship physics owns the hull: the client sends the two raw helm bytes and receives
//! the authoritative type 3 `ShipMoveType` (read into `Movement::ship`). The original mode action
//! bar and the slave information frame are driven by `hud/vehicles.rs`.

use aa_net::model::{UnitKind, stance};
use aa_net::state::{Player, Slaves, Units};
use aa_net::{Command, MoveCommand, MoveState, NetCommand, Pos};
use bevy::prelude::*;

use super::anim::{Motion, RUN_SPEED};
use super::data::Db;
use super::player::{Mover, PlayerInput, placement};
use super::units::{RemoteUnit, Views};
use crate::hud::HudKeyboardFocus;

/// Full helm input. The server normalises these by 127 (`ship_physics.rs`).
const HELM: i8 = 127;
const RESEND: f32 = 0.5;
const DECK_STEP: f32 = 1.5;
/// How far up a walking player steps onto the next bit of deck (a stair, a hatch coaming) and how far it drops to the lower deck.
const WALK_STEP_UP: f32 = 0.6;
const WALK_STEP_DOWN: f32 = 4.0;

#[derive(Default)]
pub(super) struct Helm {
    ship: u32,
    was_driver: bool,
    anchored: bool,
    last: Option<(i8, i8)>,
    since_sent: f32,
}

/// The local driver of an owned ship. The server's `ship_request` accepts only the driver's
/// attachment point, so a passenger sends nothing.
#[allow(clippy::too_many_arguments)]
pub fn drive_ship(
    time: Res<Time>,
    actions: Res<crate::settings::Actions>,
    keys: Res<ButtonInput<KeyCode>>,
    player: Res<Player>,
    units: Res<Units>,
    db: Option<Res<Db>>,
    typing: Option<Res<HudKeyboardFocus>>,
    mut out: MessageWriter<NetCommand>,
    mut input_state: ResMut<PlayerInput>,
    mut helm: Local<Helm>,
) {
    let Some(spawn) = player.spawn.as_ref() else { return };
    let Some(a) = spawn.unit.attached_to.filter(|a| a.point == 1) else {
        helm.was_driver = false;
        return;
    };
    let Some(unit) = units.get(a.obj_id) else { return };
    let Some(db) = db.as_deref() else { return };
    if !matches!(unit.kind, UnitKind::Slave { .. }) || !db.is_ship(unit.model_id) {
        helm.was_driver = false;
        return;
    }
    if helm.ship != a.obj_id {
        *helm = Helm { ship: a.obj_id, ..Helm::default() };
    }
    let typing = typing.is_some_and(|f| f.0);
    let held = |action: &str| !typing && actions.pressed(action, 0);
    let mut throttle = held("moveforward") as i32 as i8 * HELM - held("moveback") as i32 as i8 * HELM;
    let steering = held("turnleft") as i32 as i8 * HELM - held("turnright") as i32 as i8 * HELM;
    // No 1.2 ship mount skill is an anchor. Space is the helm's stop/anchor while seated (the
    // on-foot jump is owned by `control_player`; an attached driver never uses it).
    if keys.just_pressed(KeyCode::Space) && !typing {
        helm.anchored = !helm.anchored;
    }
    if throttle != 0 || steering != 0 {
        helm.anchored = false;
    }
    if helm.anchored {
        throttle = 0;
    }
    let input = (throttle, steering);
    helm.since_sent += time.delta_secs();
    let first = !helm.was_driver;
    if first || helm.last != Some(input) || (input != (0, 0) && helm.since_sent >= RESEND) {
        out.write(NetCommand(Command::SteerShip { obj_id: a.obj_id, throttle, steering }));
        helm.last = Some(input);
        helm.since_sent = 0.0;
    }
    helm.was_driver = true;
    *input_state = PlayerInput { moving: input != (0, 0), jumped: false, turned: steering != 0 };
}

/// A player standing on an owned ship's deck (not seated). The deck is the model's local hull box;
/// while the component exists `player::control_player` leaves the character to [`deck_control`].
/// Blocks deck re-entry for a moment after jumping off it.
#[derive(Component, Clone, Copy, Debug)]
pub struct DeckJumpUntil(f32);

#[derive(Component, Clone, Copy, Debug)]
pub struct DeckRide {
    ship: u32,
    local: Vec3,
    facing: f32,
    since_sent: f32,
    last: Vec3,
    sent: bool,
}

fn ship_world(root: Vec3, yaw: f32, local: Vec3) -> Vec3 {
    let (s, c) = yaw.sin_cos();
    Vec3::new(root.x + local.x * c - local.y * s, root.y + local.x * s + local.y * c, root.z + local.z)
}

/// Puts an on-foot player who stands inside an owned ship's hull on its deck. Seated players are
/// handled by the attachment (`mount::pose_local_riders`).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn board_deck(
    mut commands: Commands,
    time: Res<Time>,
    player: Res<Player>,
    units: Res<Units>,
    views: Res<Views>,
    ships: Query<&RemoteUnit>,
    slaves: Option<Res<Slaves>>,
    db: Option<Res<Db>>,
    locals: Query<(Entity, &Mover, Option<&DeckRide>, Option<&DeckJumpUntil>), With<super::player::LocalPlayer>>,
) {
    if player.spawn.as_ref().is_some_and(|p| p.unit.attached_to.is_some()) {
        return;
    }
    let Some(own) = slaves.as_deref().and_then(|s| s.own.as_ref()) else { return };
    let Some(ship) = units.get(own.obj_id) else { return };
    let UnitKind::Slave { tl_id, .. } = ship.kind else { return };
    if tl_id != own.tl_id {
        return;
    }
    let Some(db) = db.as_deref() else { return };
    if !db.is_ship(ship.model_id) {
        return;
    }
    let body = db.ship_body(ship.model_id);
    let Ok((entity, mover, riding, jumping)) = locals.single() else { return };
    if riding.is_some() {
        return;
    }
    if let Some(until) = jumping {
        if time.elapsed_secs() < until.0 {
            return;
        }
        commands.entity(entity).remove::<DeckJumpUntil>();
    }
    let (root, yaw) = views
        .unit_entity(ship.obj_id)
        .and_then(|e| ships.get(e).ok())
        .map_or_else(|| (Vec3::new(ship.pos.x, ship.pos.y, ship.pos.z), ship.yaw()), |v| (Vec3::new(v.pos.x, v.pos.y, v.pos.z), v.facing));
    let (s, c) = yaw.sin_cos();
    let d = mover.pos - root;
    let local = Vec3::new(
        d.x * c + d.y * s,
        -d.x * s + d.y * c,
        d.z,
    );
    // the hull's own deck where it is known (the main deck, the poop: not the helm's height everywhere), else the hull box
    let floor = db.ship_floor(ship.model_id);
    let deck = match &floor {
        Some(floor) => floor.height(local.x, local.y, local.z + DECK_STEP).filter(|h| local.z - h <= DECK_STEP),
        None => (local.x.abs() <= body.half[0] + 0.8 && local.y.abs() <= body.half[1] + 1.0 && (local.z - body.deck).abs() <= DECK_STEP).then_some(body.deck),
    };
    let Some(deck) = deck else { return };
    commands.entity(entity).insert(DeckRide {
        ship: ship.obj_id,
        local: Vec3::new(local.x, local.y, deck),
        facing: mover.facing - yaw,
        since_sent: 1.0,
        last: mover.pos,
        sent: false,
    });
    info!("standing on ship {} deck at local ({:.1}, {:.1})", ship.obj_id, local.x, local.y);
}

/// Walks the on-deck player in the ship frame and reports the world position; the ship keeps
/// moving underneath (`Units` carries the authoritative server pose).
#[allow(clippy::too_many_arguments)]
pub fn deck_control(
    time: Res<Time>,
    actions: Res<crate::settings::Actions>,
    player: Res<Player>,
    units: Res<Units>,
    views: Res<Views>,
    ships: Query<&RemoteUnit>,
    db: Option<Res<Db>>,
    mut commands: Commands,
    mut out: MessageWriter<NetCommand>,
    typing: Option<Res<HudKeyboardFocus>>,
    mut query: Query<(Entity, &mut Mover, &mut Transform, &mut Motion, &mut DeckRide), With<super::player::LocalPlayer>>,
) {
    let Ok((entity, mut mover, mut transform, mut motion, mut deck)) = query.single_mut() else { return };
    let seated = player.spawn.as_ref().is_some_and(|p| p.unit.attached_to.is_some());
    let Some(ship) = units.get(deck.ship) else {
        commands.entity(entity).remove::<DeckRide>();
        return;
    };
    // The visual hull is interpolated by `move_units`; follow that frame, not the last packet.
    let visual = views.unit_entity(deck.ship).and_then(|e| ships.get(e).ok());
    if seated {
        commands.entity(entity).remove::<DeckRide>();
        return;
    }
    let Some(db) = db.as_deref() else { return };
    let body = db.ship_body(ship.model_id);
    let floor = db.ship_floor(ship.model_id);
    let (root, yaw) = visual.map_or_else(
        || (Vec3::new(ship.pos.x, ship.pos.y, ship.pos.z), ship.yaw()),
        |v| (Vec3::new(v.pos.x, v.pos.y, v.pos.z), v.facing),
    );
    let dt = time.delta_secs().min(0.1);
    let typing = typing.is_some_and(|f| f.0);
    if !typing && actions.just_pressed("jump", 0) {
        // leave the deck; the on-foot controller takes over the fall next frame and
        // `DeckJumpUntil` keeps `board_deck` from catching the rising player again
        commands.entity(entity).remove::<DeckRide>();
        commands.entity(entity).insert(DeckJumpUntil(time.elapsed_secs() + 1.0));
        mover.grounded = false;
        mover.vz = 8.0;
        return;
    }
    let held = |action: &str| (!typing && actions.pressed(action, 0)) as i32 as f32;
    let (fwd, strafe) = (held("moveforward") - held("moveback"), held("moveright") - held("moveleft"));
    let turn = held("turnleft") - held("turnright");
    let mut local = deck.local;
    if fwd != 0.0 || strafe != 0.0 {
        deck.facing = (-strafe).atan2(fwd);
        let step = Vec2::new(strafe, fwd).normalize_or_zero() * RUN_SPEED * dt;
        let (x, y) = (local.x + step.x, local.y + step.y);
        match &floor {
            // walk where the hull has a deck within a step of the feet; a rail or the edge stops the walk
            Some(floor) => {
                if let Some(h) = floor.height(x, y, local.z + WALK_STEP_UP).filter(|h| local.z - h <= WALK_STEP_DOWN) {
                    local = Vec3::new(x, y, h);
                }
            }
            None => {
                local.x = x.clamp(-(body.half[0] + 0.8), body.half[0] + 0.8);
                local.y = y.clamp(-(body.half[1] + 1.0), body.half[1] + 1.0);
                local.z = body.deck;
            }
        }
    } else if turn != 0.0 {
        deck.facing += turn * 2.4 * dt;
    }
    if floor.is_none() {
        local.z = body.deck;
    }
    deck.local = local;
    let world = ship_world(root, yaw, local);
    let facing = yaw + deck.facing;
    mover.pos = world;
    mover.facing = facing;
    mover.vz = 0.0;
    mover.grounded = true;
    mover.velocity = Vec3::ZERO;
    mover.swim = None;
    *transform = placement(world, facing, 1.0);
    *motion = Motion { speed: if fwd != 0.0 || strafe != 0.0 { RUN_SPEED } else { 0.0 }, local_dir: Vec2::Y, airborne: false, swimming: false, auto: false };
    deck.since_sent += dt;
    let moved = world.distance(deck.last) > 0.02 || !deck.sent;
    if moved && deck.since_sent >= 0.1 {
        out.write(NetCommand(Command::Move(MoveCommand {
            pos: Pos::new(world.x, world.y, world.z),
            yaw: facing,
            moving: fwd != 0.0 || strafe != 0.0,
            stance: stance::RELAXED,
            ..MoveCommand::default()
        }.with_state(MoveState::Ground))));
        deck.last = world;
        deck.since_sent = 0.0;
        deck.sent = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deck_offsets_are_rotated_into_the_ship_frame() {
        let root = Vec3::new(100.0, 200.0, 10.0);
        // game yaw 90 degrees: forward is west (-x), right is north (+y)
        let p = ship_world(root, std::f32::consts::FRAC_PI_2, Vec3::new(2.0, 3.0, 1.5));
        assert!((p.x - 97.0).abs() < 1e-4, "{p:?}");
        assert!((p.y - 202.0).abs() < 1e-4, "{p:?}");
        assert!((p.z - 11.5).abs() < 1e-4, "{p:?}");
    }

    #[test]
    fn full_helm_is_the_native_signed_range() {
        assert_eq!((HELM, -HELM, 0), (127, -127, 0));
        assert_eq!(DECK_STEP, 1.5);
    }
}
