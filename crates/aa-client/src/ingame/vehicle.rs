//! Owned land vehicles (carts, cars, tractors): the local driver controls the slave and sends the
//! native `VehicleMove` body for the slave object (`CSMoveUnit` type 2). The server moves the slave
//! and its seated characters; `mount::pose_local_riders` keeps the driver on the seat.

use aa_net::model::{Movement, UnitKind};
use aa_net::slave_control::SlaveCommand;
use aa_net::state::{Player, Slaves, Units};
use aa_net::{Command, NetCommand};
use aa_world::{TerrainQuery, WorldCollision};
use bevy::prelude::*;

use super::anim::Motion;
use super::camera::OrbitCamera;
use super::data::{Db, VehicleBody};
use super::player::{PlayerInput, approach_angle, forward, placement};
use super::units::{RemoteUnit, Views};
use crate::hud::{HudKeyboardFocus, HudPointerOverUi};

const VEHICLE_STEP_UP: f32 = 0.5;
const VEHICLE_ACCEL: f32 = 8.0;
const VEHICLE_BRAKE: f32 = 14.0;
const VEHICLE_SEND_INTERVAL: f32 = 0.1;
const VEHICLE_ALIGN_RATE: f32 = 8.0;
/// The native vehicle move body has no jump/fall fields: it follows the terrain. Only the position
/// and the engine's yaw are sent (the quaternion's roll/pitch stay level until a real suspension
/// oracle exists).
#[derive(Component)]
pub struct DrivenSlave {
    body: VehicleBody,
    pos: Vec3,
    facing: f32,
    speed: f32,
    steering: f32,
    since_sent: f32,
    was_active: bool,
    sent_first: bool,
}

impl DrivenSlave {
    pub(super) fn new(pos: Vec3, facing: f32, body: VehicleBody) -> Self {
        Self { body, pos, facing, speed: 0.0, steering: 0.0, since_sent: 0.0, was_active: false, sent_first: false }
    }

    /// The mate-style object id and the exact client body; the session stamps the send time.
    fn packet(&self, obj_id: u32, _moving: bool, velocity: [i16; 3]) -> Command {
        let mut movement = Movement::vehicle(aa_net::Pos::new(self.pos.x, self.pos.y, self.pos.z), self.facing, 0);
        movement.velocity = velocity;
        if let Some(body) = &mut movement.vehicle {
            body.steering = self.steering;
            body.wheel_speeds = vec![self.speed; usize::from(self.body.wheels)];
        }
        Command::MoveVehicle { obj_id, movement }
    }
}

fn towards(current: f32, target: f32, max_delta: f32) -> f32 {
    if current < target {
        (current + max_delta).min(target)
    } else {
        (current - max_delta).max(target)
    }
}

/// How close the gunner must stand to a ship's cannon to man it (metres). Standing aboard is not
/// reported to the client, so being this near a cannon is the "aboard" test; the server decides who
/// may man it (`may_man_cannon`: the summoner, the ship's owner or anybody aboard).
pub(super) const CANNON_REACH: f32 = 4.5;

/// The timeline id of the cannon `player` can man: the nearest of `cannons` (timeline, position) within
/// [`CANNON_REACH`].
pub(super) fn nearest_cannon(player: Vec3, cannons: impl IntoIterator<Item = (u16, Vec3)>) -> Option<u16> {
    cannons
        .into_iter()
        .map(|(tl, pos)| (tl, pos.distance(player)))
        .filter(|(_, d)| *d <= CANNON_REACH)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(tl, _)| tl)
}

/// Binds the newly summoned own slave once it is visible and sends `CSDiscardSlave` when the
/// player presses the primary interaction key while driving. `CSBindSlave`/`CSDiscardSlave` are the
/// native seat controls; the original client has no separate Lua "board" button (the target popup
/// only offers rename), so the vehicle item's own client bind is the trigger.
#[allow(clippy::too_many_arguments)]
pub fn vehicle_seat(
    time: Res<Time>,
    actions: Res<crate::settings::Actions>,
    player: Res<Player>,
    units: Res<Units>,
    slaves: Option<Res<Slaves>>,
    db: Option<Res<Db>>,
    views: Option<Res<Views>>,
    visuals: Query<&RemoteUnit>,
    mover: Query<&super::player::Mover, With<super::player::LocalPlayer>>,
    mut out: MessageWriter<NetCommand>,
    mut handled: Local<Option<(u16, u32)>>,
    mut last: Local<f32>,
) {
    let Some(spawn) = player.spawn.as_ref() else { return };
    let now = time.elapsed_secs();
    let attached = spawn.unit.attached_to;
    let riding_slave = attached.and_then(|a| units.get(a.obj_id)).filter(|u| matches!(u.kind, UnitKind::Slave { .. }));
    if let Some(slave) = riding_slave {
        let tl = match slave.kind { UnitKind::Slave { tl_id, .. } => tl_id, _ => return };
        let ship = db.as_deref().is_some_and(|db| db.is_ship(slave.model_id));
        let leave = actions.just_pressed("do_interaction_1", 0);
        let unsummon = ship && actions.just_pressed("do_interaction_2", 0);
        if leave && now - *last > 0.5 {
            out.write(NetCommand(Command::Slave(SlaveCommand::Unbind { tl_id: tl })));
            *handled = Some((tl, slave.obj_id));
            *last = now;
        } else if unsummon && now - *last > 0.5 {
            // `CSDestroySlave` (the native destroy/recall slot): the server saves and removes
            // the active slave; the summon item brings it back at the shoreline.
            out.write(NetCommand(Command::Slave(SlaveCommand::Dismiss { tl_id: tl })));
            *handled = Some((tl, slave.obj_id));
            *last = now;
        }
        return;
    }
    // Do not auto-board while another attachment (a mount) owns the player.
    if attached.is_some() {
        return;
    }
    // A cannon child slave in reach takes the interaction key before the ship's helm does: the key
    // binds its gunner seat (`CSBindSlave` on the cannon's timeline); the key again unbinds (above).
    if actions.just_pressed("do_interaction_1", 0)
        && now - *last > 0.5
        && let Some(db) = db.as_deref()
    {
        let me = mover.single().map_or_else(|_| Vec3::new(spawn.unit.pos.x, spawn.unit.pos.y, spawn.unit.pos.z), |m| m.pos);
        let cannons = units.by_id.values().filter_map(|u| {
            let UnitKind::Slave { tl_id, template_id, .. } = u.kind else { return None };
            // a child slave: its bearer (the ship) is the owner object of its unit state
            let bearer = u.state_attachments.and_then(|a| a.first_obj_id).filter(|b| *b != 0)?;
            (!u.dead && units.get(bearer).is_some() && db.is_cannon(template_id)).then(|| {
                let at = views
                    .as_deref()
                    .and_then(|v| v.unit_entity(u.obj_id))
                    .and_then(|e| visuals.get(e).ok())
                    .map_or(u.pos, |v| aa_net::Pos::new(v.pos.x, v.pos.y, v.pos.z));
                (tl_id, Vec3::new(at.x, at.y, at.z))
            })
        });
        if let Some(tl_id) = nearest_cannon(me, cannons) {
            out.write(NetCommand(Command::Slave(SlaveCommand::Bind { tl_id })));
            *last = now;
            return;
        }
    }
    if actions.just_pressed("do_interaction_1", 0) { debug!("vehicle_seat: F pressed, attached {:?}, own slave {:?}", attached, slaves.as_deref().and_then(|s| s.own.as_ref()).map(|o| (o.obj_id, o.tl_id))); }
    let Some(own) = slaves.as_deref().and_then(|s| s.own.as_ref()) else { return };
    let Some(unit) = units.get(own.obj_id) else { return };
    let UnitKind::Slave { tl_id, .. } = unit.kind else { return };
    if tl_id != own.tl_id {
        return;
    }
    let Some(db) = db.as_deref() else { return };
    let land = db.is_land_vehicle(unit.model_id);
    let ship = db.is_ship(unit.model_id);
    if !land && !ship {
        return;
    }
    let range = if ship { 60.0 } else { 30.0 };
    // `Units` keeps the announcement/spawn pose; the displayed hull is interpolated by
    // `move_units` (`Views` -> `RemoteUnit`), so use that for the interaction range.
    let here = views
        .as_deref()
        .and_then(|v| v.unit_entity(own.obj_id))
        .and_then(|e| visuals.get(e).ok())
        .map_or(unit.pos, |v| aa_net::Pos::new(v.pos.x, v.pos.y, v.pos.z));
    let near = Vec3::new(here.x, here.y, here.z).distance(Vec3::new(spawn.unit.pos.x, spawn.unit.pos.y, spawn.unit.pos.z)) <= range;
    if !near {
        return;
    }
    let manual = actions.just_pressed("do_interaction_1", 0);
    if ship && actions.just_pressed("do_interaction_2", 0) {
        // Unsummon the visible ship from the dock (the native destroy/recall slot).
        out.write(NetCommand(Command::Slave(SlaveCommand::Dismiss { tl_id })));
        *handled = Some((tl_id, own.obj_id));
        *last = now;
        return;
    }
    // Land vehicles keep the one-shot auto-board the item's own client bind triggers. A ship is
    // boarded with the interaction key: auto-boarding would pull a player who only stands on deck
    // off it into the helm seat.
    let unbound = *handled != Some((tl_id, own.obj_id));
    if (land && unbound) || (manual && (unbound || now - *last > 0.5)) {
        out.write(NetCommand(Command::Slave(SlaveCommand::Bind { tl_id })));
        *handled = Some((tl_id, own.obj_id));
        *last = now;
    }
}

/// Drives the slave whose driver seat holds the local player.
#[allow(clippy::too_many_arguments)]
pub fn drive_vehicle(
    time: Res<Time>,
    actions: Res<crate::settings::Actions>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut cameras: Query<&mut OrbitCamera>,
    player: Res<Player>,
    views: Res<Views>,
    terrain: Res<TerrainQuery>,
    collision: Res<WorldCollision>,
    mut out: MessageWriter<NetCommand>,
    mut input_state: ResMut<PlayerInput>,
    typing: Option<Res<HudKeyboardFocus>>,
    over_ui: Option<Res<HudPointerOverUi>>,
    mut slaves: Query<(&mut RemoteUnit, &mut Transform, Option<&mut Motion>, &mut DrivenSlave)>,
) {
    let Some(spawn) = player.spawn.as_ref() else { return };
    let Some(a) = spawn.unit.attached_to.filter(|a| a.point == 1) else { return };
    let Some(entity) = views.unit_entity(a.obj_id) else { return };
    let Ok((mut unit, mut transform, motion, mut drive)) = slaves.get_mut(entity) else { return };
    let Ok(mut orbit) = cameras.single_mut() else { return };
    let typing = typing.is_some_and(|f| f.0);
    let over_ui = over_ui.is_some_and(|o| o.0);
    let pressed = |action: &str| !typing && actions.pressed(action, 0);
    let dt = time.delta_secs().min(0.1);
    if dt <= 0.0 {
        return;
    }
    drive.since_sent += dt;
    let back = pressed("moveback");
    let both = mouse.pressed(MouseButton::Left) && mouse.pressed(MouseButton::Right) && !over_ui;
    let throttle = (pressed("moveforward") || both) as i32 as f32 - back as i32 as f32;
    let turn = pressed("turnleft") as i32 as f32 - pressed("turnright") as i32 as f32;
    let target = if throttle >= 0.0 { throttle * drive.body.speed } else { throttle * drive.body.speed * 0.5 };
    let rate = if throttle.abs() < f32::EPSILON || throttle.signum() != drive.speed.signum() { VEHICLE_BRAKE } else { VEHICLE_ACCEL };
    drive.speed = towards(drive.speed, target, rate * dt);
    let speed_ratio = (drive.speed.abs() / drive.body.speed).clamp(0.0, 1.0);
    let before_facing = drive.facing;
    if turn != 0.0 {
        let d = turn * drive.body.turn * dt * (0.35 + 0.65 * speed_ratio);
        drive.facing += d;
        orbit.yaw += d;
    }
    drive.steering = turn;
    if mouse.pressed(MouseButton::Right) && !over_ui {
        drive.facing = orbit.yaw;
    } else if turn == 0.0 && drive.speed.abs() > 0.1 {
        drive.facing = approach_angle(drive.facing, orbit.yaw, VEHICLE_ALIGN_RATE * dt);
    }
    let old = drive.pos;
    let mut pos = old;
    if drive.speed.abs() > 0.01 {
        let step = forward(drive.facing) * drive.speed * dt;
        let candidate = collision.slide(pos, step, 1.2, 1.8, VEHICLE_STEP_UP).pos;
        pos.x = candidate.x;
        pos.y = candidate.y;
    }
    let ground = super::player::floor_at(&terrain, &collision, Vec3::new(pos.x, pos.y, old.z), VEHICLE_STEP_UP);
    if ground.is_finite() {
        pos.z = ground;
    }
    let actual = if dt > 0.0 { Vec2::new(pos.x - old.x, pos.y - old.y) / dt } else { Vec2::ZERO };
    let moving = actual.length() > 0.05;
    drive.pos = pos;
    unit.pos = pos;
    unit.facing = drive.facing;
    *transform = placement(pos, drive.facing, unit.scale());
    if let Some(mut m) = motion {
        *m = Motion { speed: actual.length(), local_dir: Vec2::Y, airborne: false };
    }
    let turned = (drive.facing - before_facing).abs() > 1e-4;
    *input_state = PlayerInput { moving, jumped: false, turned };
    let active = moving || turned || drive.speed.abs() > 0.05;
    let due = drive.since_sent >= VEHICLE_SEND_INTERVAL;
    let first = !drive.sent_first && active;
    if (active && due) || (!active && drive.was_active) || first {
        let to_wire = |v: f32| (v * 1024.0).round().clamp(-32767.0, 32767.0) as i16;
        let velocity = [to_wire(actual.x), to_wire(actual.y), 0];
        out.write(NetCommand(drive.packet(a.obj_id, active, velocity)));
        drive.since_sent = 0.0;
        drive.sent_first = true;
    }
    drive.was_active = active;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body() -> VehicleBody {
        VehicleBody { speed: 15.0, turn: 1.0, wheels: 2 }
    }

    #[test]
    fn vehicle_packet_names_the_slave_and_carries_the_native_body() {
        let mut drive = DrivenSlave::new(Vec3::new(10.0, 20.0, 3.0), 0.5, body());
        drive.speed = 15.0;
        drive.steering = -1.0;
        let command = drive.packet(0x445566, true, [1024, 0, 0]);
        match command {
            Command::MoveVehicle { obj_id, movement } => {
                assert_eq!(obj_id, 0x445566);
                assert_eq!(movement.move_type, 2);
                assert_eq!(movement.velocity, [1024, 0, 0]);
                let body = movement.vehicle.unwrap();
                assert_eq!(body.steering, -1.0);
                assert_eq!(body.wheel_speeds, vec![15.0, 15.0]);
            }
            other => panic!("not a vehicle movement: {other:?}"),
        }
    }

    #[test]
    fn the_nearest_cannon_in_reach_is_manned() {
        let me = Vec3::new(10.0, 10.0, 2.0);
        let cannons = [(3, Vec3::new(13.0, 10.0, 2.0)), (4, Vec3::new(11.0, 10.0, 2.0)), (5, Vec3::new(40.0, 10.0, 2.0))];
        assert_eq!(nearest_cannon(me, cannons), Some(4));
        assert_eq!(nearest_cannon(me, [(5, Vec3::new(40.0, 10.0, 2.0)), (6, Vec3::new(10.0, 10.0, 2.0 + CANNON_REACH + 0.1))]), None);
        assert_eq!(nearest_cannon(me, []), None);
    }

    #[test]
    fn speed_is_braked_and_accelerated_towards_the_throttle() {
        assert_eq!(towards(0.0, 15.0, 8.0), 8.0);
        assert_eq!(towards(8.0, 15.0, 8.0), 15.0);
        assert_eq!(towards(15.0, 0.0, 14.0), 1.0);
        assert_eq!(towards(1.0, 0.0, 14.0), 0.0);
    }
}

#[cfg(test)]
mod flow_tests {
    use super::*;
    use aa_net::model::{Attachment, CharacterState, Equipment, ModelParams, UnitKind};
    use aa_net::session::PlayerSpawn;
    use aa_net::slave::{SlaveCreated, SlaveEvent};
    use aa_net::{Event, NetEvent};
    use aa_net::state::Player;
    use aa_net::Pos;

    fn unit(obj_id: u32, kind: UnitKind) -> aa_net::model::Unit {
        aa_net::model::Unit {
            obj_id,
            name: format!("u{obj_id}"),
            kind,
            owner_name: String::new(),
            pos: Pos::new(0.0, 0.0, 0.0),
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

    fn spawn() -> PlayerSpawn {
        let mut state = CharacterState::default();
        state.character.id = 47;
        PlayerSpawn { obj_id: 100, state, unit: unit(100, UnitKind::Character { char_id: 47 }) }
    }

    #[test]
    fn own_visible_slave_is_bound_once() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(Player { spawn: Some(spawn()) })
            .insert_resource(crate::settings::Actions::default())
            .add_message::<NetEvent>()
            .add_message::<NetCommand>()
            .add_systems(Update, vehicle_seat);
        let mut units = Units::default();
        let mut slave = unit(500, UnitKind::Slave { id: 9, tl_id: 7, template_id: 60, summoner_id: 47 });
        slave.model_id = 1008; // tractor (VehicleModel)
        units.by_id.insert(500, slave);
        app.insert_resource(units);
        let db = Db::open().expect("the server database");
        app.insert_resource(db);
        let mut slaves = Slaves::default();
        slaves.apply(&Event::PlayerSpawn(Box::new(spawn())));
        slaves.apply(&Event::Slave(SlaveEvent::Created(SlaveCreated {
            owner_obj_id: 100,
            tl_id: 7,
            obj_id: 500,
            hide_spawn_effect: true,
            raw_id: 0,
            creator_name: "Tractor".into(),
        })));
        app.insert_resource(slaves);
        let drain = |app: &mut App| app.world_mut().resource_mut::<Messages<NetCommand>>().drain().map(|v| v.0).collect::<Vec<_>>();
        app.update();
        assert!(matches!(drain(&mut app).as_slice(), [Command::Slave(SlaveCommand::Bind { tl_id: 7 })]));
        app.update();
        assert!(drain(&mut app).is_empty(), "the same visible slave is not re-bound every frame");
        // A seated player is not offered a second bind.
        app.world_mut().resource_mut::<Player>().spawn.as_mut().unwrap().unit.attached_to = Some(Attachment { obj_id: 500, point: 1 });
        app.update();
        assert!(drain(&mut app).is_empty());
    }

    /// The shared deck: the owned ship (obj 500, tl 7, template 22) and a cannon child (obj 600, tl 8, template 10).
    fn deck(seat: Option<Attachment>, cannon_at: f32) -> App {
        let mut app = App::new();
        let mut player = spawn();
        player.unit.attached_to = seat;
        let actions = crate::settings::Actions::default();
        app.add_plugins(MinimalPlugins)
            .insert_resource(Player { spawn: Some(player) })
            .insert_resource(actions)
            .add_message::<NetEvent>()
            .add_message::<NetCommand>()
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs(1)))
            .add_systems(Update, vehicle_seat);
        let mut units = Units::default();
        let mut ship = unit(500, UnitKind::Slave { id: 9, tl_id: 7, template_id: 22, summoner_id: 47 });
        ship.model_id = 1249;
        units.by_id.insert(500, ship);
        let mut cannon = unit(600, UnitKind::Slave { id: 10, tl_id: 8, template_id: 10, summoner_id: 47 });
        cannon.model_id = 117;
        cannon.pos = Pos::new(cannon_at, 0.0, 0.0);
        cannon.state_attachments = Some(aa_net::model::UnitStateAttachments { first_point: 9, first_obj_id: Some(500), bonding_obj_id: None });
        units.by_id.insert(600, cannon);
        app.insert_resource(units);
        app.insert_resource(Db::open().expect("the server database"));
        let mut slaves = Slaves::default();
        slaves.apply(&Event::PlayerSpawn(Box::new(spawn())));
        slaves.apply(&Event::Slave(SlaveEvent::Created(SlaveCreated {
            owner_obj_id: 100,
            tl_id: 7,
            obj_id: 500,
            hide_spawn_effect: true,
            raw_id: 0,
            creator_name: "Ship".into(),
        })));
        app.insert_resource(slaves);
        // let the clock pass the half second the key repeat guard waits for
        for _ in 0..4 {
            app.update();
        }
        assert!(sent(&mut app).is_empty(), "nothing is sent without a key");
        app
    }

    /// The interaction key goes down for one frame.
    fn press(app: &mut App) -> Vec<Command> {
        app.world_mut().resource_mut::<crate::settings::Actions>().tap_for_test("do_interaction_1");
        app.update();
        sent(app)
    }

    fn sent(app: &mut App) -> Vec<Command> {
        app.world_mut().resource_mut::<Messages<NetCommand>>().drain().map(|v| v.0).collect()
    }

    #[test]
    fn the_interaction_key_mans_a_cannon_in_reach_before_the_helm_and_leaves_it_again() {
        // a cannon 3 m away is manned (CSBindSlave on the cannon's timeline), not the ship's helm
        let mut app = deck(None, 3.0);
        assert!(matches!(press(&mut app).as_slice(), [Command::Slave(SlaveCommand::Bind { tl_id: 8 })]));
        // out of reach the key boards the ship as before
        let mut app = deck(None, 30.0);
        assert!(matches!(press(&mut app).as_slice(), [Command::Slave(SlaveCommand::Bind { tl_id: 7 })]));
        // seated at the cannon, the key leaves its seat (CSDiscardSlave on the cannon, not the ship)
        let mut app = deck(Some(Attachment { obj_id: 600, point: 1 }), 0.0);
        assert!(matches!(press(&mut app).as_slice(), [Command::Slave(SlaveCommand::Unbind { tl_id: 8 })]));
    }
}
