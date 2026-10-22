//! Original mode action bar over an owned slave/land vehicle (`x2ui/hud/mode_action_bar/mode_action.alb`).
//!
//! The bar is the original script's: it asks `X2Unit:GetModeActionsCount()` and establishes
//! `ISLOT_MODE_ACTION` slots (`mode_action.alb:62-106`). The native client fills those slots from
//! the current slave's `slave_mount_skills` rows; this module resolves the same static mapping to
//! `mount_skills` combat records and sends the mate-style mount-caster `CSStartSkill` (the server's
//! skill handler accepts a slave caster through `mount_authority`).

use super::api::{HudBridge, HudData, Shared};
use super::data::{any_int, GameData};
use aa_net::model::{Item, Unit, UnitKind};
use aa_net::slave_control::SlaveCommand;
use aa_net::state::{Inventory, Player, Slaves, Units};
use aa_net::{Command, NetCommand};
use aa_ui::UiRuntime;
use bevy::prelude::*;
use mlua::{Lua, Table, Value};
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Skill {
    pub combat: u32,
    pub first_seat: Option<u32>,
    pub target_type: u32,
    pub target_selection: u32,
}

#[derive(Default)]
pub(super) struct Tables {
    /// `slave_mount_skills`: slave template -> `mount_skills.id`s in table order.
    pub(super) skills: HashMap<u32, Vec<u32>>,
    /// The `mount_skills` rows those ids name (combat skill and seat/target context).
    pub(super) skill_rows: HashMap<u32, Skill>,
    /// Vehicle body by slave template (`vehicle_models.velocity`, `angVel`).
    pub(super) speeds: HashMap<u32, (f32, f32)>,
    /// Slave templates whose model is a land `VehicleModel` (the only ones with a gear panel).
    pub(super) land: HashSet<u32>,
    /// Slave templates whose model is a `ShipModel`.
    pub(super) ships: HashSet<u32>,
    /// Slave templates of cannons / harpoon guns (`slave_kind_id` 8), manned from the driver seat.
    pub(super) cannons: HashSet<u32>,
    /// Equipment slots, parts and hull rules of ships ([`super::slave_equipment`]).
    pub(super) equip: super::slave_equipment::Tables,
}

impl Tables {
    pub(super) fn load(conn: &Connection) -> rusqlite::Result<Self> {
        let mut out = Self::default();
        let mut first = HashMap::new();
        let mut query = conn.prepare("SELECT mount_skill_id,attach_point_id FROM mount_attached_skills ORDER BY id")?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            let id = any_int(row, 0)? as u32;
            first.entry(id).or_insert(any_int(row, 1)? as u32);
        }
        let mut query = conn.prepare(
            "SELECT m.id,m.skill_id,s.target_type_id,s.target_selection_id FROM mount_skills m \
             INNER JOIN skills s ON s.id = m.skill_id ORDER BY m.id",
        )?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            let id = any_int(row, 0)? as u32;
            out.skill_rows.insert(
                id,
                Skill {
                    combat: any_int(row, 1)? as u32,
                    first_seat: first.get(&id).copied(),
                    target_type: any_int(row, 2)? as u32,
                    target_selection: any_int(row, 3)? as u32,
                },
            );
        }
        let mut query = conn.prepare("SELECT slave_id,mount_skill_id FROM slave_mount_skills ORDER BY id")?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            let slave = any_int(row, 0)? as u32;
            let skill = any_int(row, 1)? as u32;
            if slave != 0 && out.skill_rows.contains_key(&skill) {
                out.skills.entry(slave).or_default().push(skill);
            }
        }
        let mut query = conn.prepare(
            "SELECT s.id, v.velocity, v.angVel FROM slaves s \
             JOIN models m ON m.id = s.model_id \
             JOIN vehicle_models v ON v.id = m.sub_id AND m.sub_type = 'VehicleModel'",
        )?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            let slave = any_int(row, 0)? as u32;
            let speed = any_int(row, 1)? as f32;
            let turn = any_int(row, 2)? as f32;
            out.speeds.insert(slave, (speed.max(0.1), turn.abs().max(0.05)));
        }
        let mut query = conn.prepare(
            "SELECT s.id FROM slaves s JOIN models m ON m.id = s.model_id AND m.sub_type = 'VehicleModel'",
        )?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            out.land.insert(any_int(row, 0)? as u32);
        }
        let mut query = conn.prepare(
            "SELECT s.id FROM slaves s JOIN models m ON m.id = s.model_id AND m.sub_type = 'ShipModel'",
        )?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            out.ships.insert(any_int(row, 0)? as u32);
        }
        // Ships use `ship_models` (the server floats the hull); the mode bar shows the same
        // authored speed/turn values in the slave information frame (`slave_info.alb:77-90`).
        let mut query = conn.prepare(
            "SELECT s.id, h.velocity, h.steer_vel FROM slaves s \
             JOIN models m ON m.id = s.model_id AND m.sub_type = 'ShipModel' \
             JOIN ship_models h ON h.id = m.sub_id",
        )?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            let slave = any_int(row, 0)? as u32;
            let speed = any_int(row, 1)? as f32;
            let turn = any_int(row, 2)? as f32;
            out.speeds.insert(slave, (speed.max(0.1), turn.abs().max(0.05)));
        }
        let mut query = conn.prepare("SELECT id FROM slaves WHERE slave_kind_id = 8")?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            out.cannons.insert(any_int(row, 0)? as u32);
        }
        out.equip = super::slave_equipment::Tables::load(conn)?;
        Ok(out)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Current {
    pub obj_id: u32,
    pub tl_id: u16,
    /// The slave's database id (`UnitKind::Slave::id`), echoed by equipment requests.
    pub db_id: u32,
    pub template: u32,
    /// The parts it wears, by equip slot id (0..26).
    pub equipment: Vec<Option<Item>>,
    /// The local player's seat (`Some(1)` driver, `Some(2)` passenger), if boarded.
    pub point: Option<u8>,
    pub name: String,
    pub level: u8,
    pub hp: i32,
    pub max_hp: i32,
    pub pos: Vec3,
    pub speed: f32,
    pub turn: f32,
    /// A land `VehicleModel`: the information panel's gear/brake panel applies.
    pub land: bool,
    /// A `ShipModel`: the mode bar shows its `slave_mount_skills` without a player-seat filter.
    pub ship: bool,
    /// A cannon the player is manning (not the owned slave): the mode bar shows the cannon's skills.
    pub cannon: bool,
}

#[derive(Default)]
pub(super) struct View {
    pub current: Option<Current>,
    pushed: Option<Current>,
    /// The player's requested leave, sent by `update` after an identity recheck.
    pub(super) requests: Vec<(u32, u16, SlaveCommand)>,
    /// Part moves the Summon Info scripts asked for, sent by `update` after an identity recheck.
    pub(super) equip_requests: Vec<super::slave_equipment::Request>,
    /// The pointer held something at the last push (`UPDATE_SLAVE_EQUIPMENT_SLOT` follows changes).
    holding: bool,
}

fn current(world: &World, data: &HudData) -> Option<Current> {
    let units = world.get_resource::<Units>()?;
    let player = world.get_resource::<Player>()?.spawn.as_ref()?;
    let attached = player.unit.attached_to;
    let game = data.game.get();
    // The slave the mode bar and the information window belong to: the cannon whose gunner seat the player
    // holds, else the owned ship / vehicle.
    let manned = attached.and_then(|a| units.get(a.obj_id)).filter(|u| {
        matches!(u.kind, UnitKind::Slave { template_id, .. } if game.is_some_and(|g| g.vehicles.cannons.contains(&template_id)))
    });
    let (unit, tl_id) = match manned {
        Some(unit) => {
            let UnitKind::Slave { tl_id, .. } = unit.kind else { return None };
            (unit, tl_id)
        }
        None => {
            let own = world.get_resource::<Slaves>()?.own.as_ref()?;
            let unit = units.get(own.obj_id)?;
            let UnitKind::Slave { tl_id, .. } = unit.kind else { return None };
            if tl_id != own.tl_id {
                return None;
            }
            (unit, tl_id)
        }
    };
    let UnitKind::Slave { id: db_id, template_id, .. } = unit.kind else { return None };
    let cannon = manned.is_some();
    let point = attached.filter(|a| a.obj_id == unit.obj_id).map(|a| a.point);
    let (speed, turn) = game.and_then(|g| g.vehicles.speeds.get(&template_id).copied()).unwrap_or(if cannon { (0.0, 0.0) } else { (8.0, 1.0) });
    Some(Current {
        obj_id: unit.obj_id,
        tl_id,
        db_id,
        template: template_id,
        equipment: unit.equipment.slots[..usize::from(aa_net::slave_control::SLAVE_EQUIPMENT_SLOTS)].to_vec(),
        point,
        name: unit_name(unit, data),
        level: unit.level,
        hp: unit.hp,
        max_hp: unit.max_hp.max(unit.hp).max(1),
        pos: Vec3::new(unit.pos.x, unit.pos.y, unit.pos.z),
        speed,
        turn,
        land: game.is_some_and(|g| g.vehicles.land.contains(&template_id)),
        ship: game.is_some_and(|g| g.vehicles.ships.contains(&template_id)),
        cannon,
    })
}

fn unit_name(unit: &Unit, data: &HudData) -> String {
    if !unit.name.is_empty() {
        return unit.name.clone();
    }
    data.game
        .get()
        .and_then(|g| g.npcs.get(&unit.kind.template_id()))
        .map(|n| n.name.clone())
        .unwrap_or_default()
}

pub(super) fn refresh(world: &World, data: &mut HudData) {
    data.vehicles.current = current(world, data);
}

/// The visible vehicle skills of the current slave for its seat (native context filter).
fn displayed<'a>(game: &'a GameData, slave: &'a Current) -> impl Iterator<Item = (u32, &'a Skill)> + 'a {
    let context = slave.point.unwrap_or(0);
    game.vehicles
        .skills
        .get(&slave.template)
        .into_iter()
        .flatten()
        .filter_map(move |id| {
            let skill = game.vehicles.skill_rows.get(id)?;
            // A ship's mode actions are its mount skills (spyglass, sails, charge): their
            // `mount_attached_skills` rows name equipment attach points (80 telescope, 1 helm),
            // not the player's seat, so only the land-vehicle driver seat filter applies.
            let seat_ok = slave.ship
                || skill.first_seat.is_none()
                || skill.first_seat == Some(u32::from(context))
                || (context == 1 && skill.first_seat == Some(1));
            (slave.point.is_some() && seat_ok).then_some((*id, skill))
        })
}

pub(super) fn install(lua: &Lua, bridge: &Shared) -> mlua::Result<()> {
    let x2unit: Table = lua.globals().get("X2Unit")?;
    let state = bridge.clone();
    x2unit.set(
        "GetModeActionsCount",
        lua.create_function(move |_, _: Value| {
            let data = state.borrow();
            let Some(current) = &data.vehicles.current else { return Ok(0) };
            Ok(data.game.get().map_or(0, |g| displayed(g, current).count()))
        })?,
    )?;
    let state = bridge.clone();
    x2unit.set(
        "RefreshModeActionBar",
        lua.create_function(move |_, _: Value| {
            let _ = &state.borrow().vehicles.current;
            Ok(())
        })?,
    )?;

    let weapon: Table = lua.globals().get("X2SiegeWeapon")?;
    let state = bridge.clone();
    weapon.set(
        "IsMountedSiegeWeapon",
        lua.create_function(move |_, _: Value| Ok(state.borrow().vehicles.current.as_ref().is_some_and(|v| v.point.is_some())))?,
    )?;
    let state = bridge.clone();
    weapon.set(
        "IsDriver",
        lua.create_function(move |_, _: Value| Ok(state.borrow().vehicles.current.as_ref().is_some_and(|v| v.point == Some(1))))?,
    )?;
    let state = bridge.clone();
    weapon.set(
        "IsTargetMySiegeWeapon",
        lua.create_function(move |_, _: Value| {
            let data = state.borrow();
            let target = data.panels.target.as_ref().map(|t| t.obj_id);
            Ok(target.is_some() && target == data.vehicles.current.as_ref().map(|v| v.obj_id))
        })?,
    )?;
    let state = bridge.clone();
    weapon.set(
        "GetSiegeWeaponSpeed",
        lua.create_function(move |_, _: Value| Ok(state.borrow().vehicles.current.as_ref().map_or(0.0, |v| f64::from(v.speed))))?,
    )?;
    let state = bridge.clone();
    weapon.set(
        "GetSiegeWeaponTurnSpeed",
        lua.create_function(move |_, _: Value| Ok(state.borrow().vehicles.current.as_ref().map_or(0.0, |v| f64::from(v.turn))))?,
    )?;
    let state = bridge.clone();
    weapon.set(
        "DiscardSiegeWeapon",
        lua.create_function(move |_, _: Value| {
            let mut data = state.borrow_mut();
            if let Some(v) = &data.vehicles.current {
                let (owner, tl) = (data.character_id, v.tl_id);
                data.vehicles.requests.push((owner, tl, SlaveCommand::Unbind { tl_id: tl }));
            }
            Ok(())
        })?,
    )?;
    let state = bridge.clone();
    weapon.set(
        "GetMountedSiegeWeaponInfo",
        lua.create_function(move |lua, _: Value| {
            let data = state.borrow();
            let Some(v) = &data.vehicles.current else { return Ok(None) };
            let out = lua.create_table()?;
            out.set("unitId", v.obj_id.to_string())?;
            // `kind` is the SLAVE_KIND text key, `desc_name` the hull's name (slave_info.alb:10-12).
            let equip = data.game.get().map(|g| &g.vehicles.equip);
            out.set("kind", equip.and_then(|e| e.kinds.get(&v.template)).copied().unwrap_or("machine"))?;
            out.set("desc_name", equip.and_then(|e| e.names.get(&v.template)).map_or("", String::as_str))?;
            out.set("name", v.name.as_str())?;
            out.set("level", v.level)?;
            out.set("hp", v.hp)?;
            out.set("maxHp", v.max_hp)?;
            out.set("ownerName", "")?;
            Ok(Some(out))
        })?,
    )?;
    let state = bridge.clone();
    weapon.set(
        "GetVehicleInfo",
        lua.create_function(move |lua, _: Value| {
            let data = state.borrow();
            let Some(v) = &data.vehicles.current else { return Ok(None) };
            // `slave_info.alb:39-75` reads gear/handbrake/tractionCtrl; a ship has no gear panel
            // (the native getter answers nil there, and the script returns).
            if !v.land {
                return Ok(None);
            }
            let out = lua.create_table()?;
            out.set("gear", 0)?;
            out.set("handbrake", false)?;
            out.set("tractionCtrl", false)?;
            out.set("speed", v.speed)?;
            out.set("turnSpeed", v.turn)?;
            Ok(Some(out))
        })?,
    )?;
    super::slave_equipment::install(lua, bridge)?;

    let hud: Table = lua.globals().get("HUD")?;
    let state = bridge.clone();
    hud.set(
        "mode_slot",
        lua.create_function(move |lua, index: usize| {
            let data = state.borrow();
            let Some(current) = &data.vehicles.current else { return Ok(None) };
            let Some(game) = data.game.get() else { return Ok(None) };
            let Some((_, skill)) = displayed(game, current).nth(index.checked_sub(1).unwrap_or(usize::MAX)) else { return Ok(None) };
            let Some(info) = game.skills.get(&skill.combat) else { return Ok(None) };
            let out = lua.create_table()?;
            out.set("id", skill.combat)?;
            out.set("icon", info.named.icon.as_str())?;
            out.set("learned", true)?;
            out.set("canUse", skill.target_type == 6)?;
            out.set("type", "skill")?;
            Ok(Some(out))
        })?,
    )?;
    lua.load(
        r#"
        local content=AAUI.SlotContent
        AAUI.SlotContent=function(kind,index)
          if kind==ISLOT_MODE_ACTION then
            local slot=HUD.mode_slot(index)
            if slot then slot.tooltip=HUD.skill_tip(slot.id) end
            return slot
          end
          return content(kind,index)
        end
    "#,
    )
    .set_name("@vehicle_mode_bar")
    .exec()?;
    Ok(())
}

/// A position skill of the manned cannon goes through the ground reticle (`ground_target::route`: the
/// decal follows the cursor, the click casts); the cannon is the caster, so the cast is the slave-caster
/// `UseMateSkillAt` and the seated gunner is not turned. Other skills use [`use_action`].
pub(super) fn ground_press(data: &HudData, index: usize) -> Option<crate::ingame::ground_target::SkillPressed> {
    let current = data.vehicles.current.as_ref().filter(|c| c.cannon && c.point.is_some())?;
    let (mount_skill_id, skill) = displayed(data.game.get()?, current).nth(index.checked_sub(1)?)?;
    matches!(skill.target_type, 6 | 11 | 13 | 15).then_some(crate::ingame::ground_target::SkillPressed {
        skill: skill.combat,
        item: None,
        target_obj_id: 0,
        gcd: false,
        water: false,
        caster: Some((current.obj_id, mount_skill_id)),
    })
}

/// The click on a mode action slot: resolve the vehicle's mount skill and target like the pet bar.
pub(super) fn use_action(data: &HudData, index: usize, target_obj_id: u32, target_pos: Option<Vec3>) -> Option<Command> {
    let current = data.vehicles.current.as_ref()?;
    let game = data.game.get()?;
    let (mount_skill_id, skill) = displayed(game, current).nth(index.checked_sub(1)?)?;
    match skill.target_type {
        0 => Some(Command::UseMateSkill { skill_id: skill.combat, mate_obj_id: current.obj_id, mount_skill_id, target_obj_id: current.obj_id }),
        4 | 5 | 16 | 17 => (target_obj_id != 0 && target_obj_id != current.obj_id).then_some(Command::UseMateSkill {
            skill_id: skill.combat,
            mate_obj_id: current.obj_id,
            mount_skill_id,
            target_obj_id,
        }),
        6 | 11 | 13 | 15 => {
            if skill.target_selection != 1 && target_obj_id == 0 {
                return None;
            }
            let pos = if skill.target_selection == 1 { current.pos } else { target_pos? };
            Some(Command::UseMateSkillAt {
                skill_id: skill.combat,
                mate_obj_id: current.obj_id,
                mount_skill_id,
                pos: aa_net::Pos::new(pos.x, pos.y, pos.z),
            })
        }
        _ => {
            let target_obj_id = if target_obj_id == 0 { current.obj_id } else { target_obj_id };
            Some(Command::UseMateSkill { skill_id: skill.combat, mate_obj_id: current.obj_id, mount_skill_id, target_obj_id })
        }
    }
}

/// The key binding `builtin_vehicle_info` ("Open Pet Details"): the only thing that fires `SHOW_SLAVE_INFO` in the original
/// (stub `0x392043d0`), which `mode_action.alb` answers by toggling the Summon Info window. Boarding does not open it, and
/// the script hides it again whenever the mode bar has no skills (`MODE_ACTIONS_UPDATE`). Nothing happens without a slave.
pub(super) const INFO_KEY: &str = "builtin_vehicle_info";

pub(super) fn info_key(rt: &UiRuntime, bridge: &Shared) {
    if bridge.borrow().vehicles.current.is_some() {
        rt.fire_event("SHOW_SLAVE_INFO", ());
    }
}

pub(super) fn push(rt: &UiRuntime, bridge: &Shared) {
    let (old, next) = {
        let mut data = bridge.borrow_mut();
        if data.vehicles.current == data.vehicles.pushed {
            return;
        }
        let next = data.vehicles.current.clone();
        (std::mem::replace(&mut data.vehicles.pushed, next.clone()), next)
    };
    if old.is_none() && next.is_some() {
        rt.fire_event("SLAVE_SPAWN", ());
    }
    if old.as_ref().map(|v| (v.obj_id, v.point)) != next.as_ref().map(|v| (v.obj_id, v.point)) {
        rt.fire_event("MODE_ACTIONS_UPDATE", ());
    }
    if old.as_ref().is_some_and(|v| v.point.is_some()) && next.as_ref().is_none_or(|v| v.point.is_none()) {
        rt.fire_event("SIEGEWEAPON_UNBOUND", ());
    }
    match (&old, &next) {
        (Some(o), Some(n)) if o.obj_id == n.obj_id => {
            if o.name != n.name {
                rt.fire_event("UNIT_NAME_CHANGED", n.obj_id.to_string());
            }
            for slot in super::slave_equipment::changed(&o.equipment, &n.equipment) {
                rt.fire_event("SLAVE_UNIT_EQUIPMENT_CHANGED", (i64::from(slot), i64::from(slot)));
            }
        }
        // Another slave while the window is open: the same refresh `SHOW_SLAVE_INFO` runs.
        (_, Some(_)) => {
            let refresh = "if slaveInfoFrame ~= nil and slaveInfoFrame:IsVisible() then slaveInfoFrame:UpdateSlaveDefaultInfo() end";
            if let Err(e) = rt.lua.load(refresh).set_name("@slave_info_refresh").exec() {
                warn!("hud: slave info refresh: {e}");
            }
        }
        _ => {}
    }
    if let Err(e) = rt.refresh_slots() {
        warn!("hud: mode action slots: {e}");
    }
}

pub(super) fn update(
    rt: Option<NonSend<UiRuntime>>,
    bridge: NonSend<HudBridge>,
    player: Res<Player>,
    slaves: Option<Res<Slaves>>,
    units: Res<Units>,
    inventory: Option<Res<Inventory>>,
    mut net: MessageWriter<NetCommand>,
) {
    let Some(rt) = rt else { return };
    let owner = player.spawn.as_ref().map_or(0, |p| p.character().id);
    let mut requests = Vec::new();
    {
        let mut data = bridge.0.borrow_mut();
        if owner == 0 || data.character_id != owner {
            if data.vehicles.current.is_some() || !data.vehicles.requests.is_empty() {
                data.vehicles = default();
            }
            return;
        }
        requests.append(&mut data.vehicles.requests);
    }
    send_equipment_requests(&bridge.0, owner, inventory.as_deref(), &mut net);
    let holding = rt.holding();
    if note_holding(&bridge.0, holding) {
        rt.fire_event("UPDATE_SLAVE_EQUIPMENT_SLOT", ());
    }
    // the cannon the player is manning is as valid a target of the leave request as the owned slave
    let manned = bridge.0.borrow().vehicles.current.as_ref().filter(|c| c.cannon).map(|c| (c.tl_id, c.obj_id));
    for (quoted_owner, tl_id, command) in requests {
        let object = manned.filter(|(tl, _)| *tl == tl_id).map(|(_, obj)| obj).or_else(|| {
            slaves.as_deref().and_then(|s| s.own.as_ref()).filter(|own| own.tl_id == tl_id).map(|own| own.obj_id)
        });
        let actual = object
            .and_then(|obj| units.get(obj))
            .is_some_and(|u| matches!(u.kind, UnitKind::Slave { tl_id: actual, .. } if actual == tl_id));
        if owner == quoted_owner && actual {
            net.write(NetCommand(Command::Slave(command)));
        }
    }
    push(&rt, &bridge.0);
}

/// Records the pointer's held state; true when it changed while a slave is owned (the equipment slots refresh).
/// The borrow ends here: `update` borrows the bridge again right after.
fn note_holding(bridge: &Shared, holding: bool) -> bool {
    let mut data = bridge.borrow_mut();
    if data.vehicles.holding == holding {
        return false;
    }
    data.vehicles.holding = holding;
    data.vehicles.current.is_some()
}

/// The part moves the Summon Info scripts queued, as `CSChangeSlaveEquipment` for the slave the window shows.
pub(super) fn send_equipment_requests(bridge: &Shared, owner: u32, inventory: Option<&Inventory>, net: &mut MessageWriter<NetCommand>) {
    let (requests, current) = {
        let mut data = bridge.borrow_mut();
        (std::mem::take(&mut data.vehicles.equip_requests), data.vehicles.current.clone())
    };
    let (Some(current), Some(inventory)) = (current, inventory) else { return };
    let data = bridge.borrow();
    let Some(game) = data.game.get() else { return };
    for request in requests {
        match super::slave_equipment::build(&game.vehicles.equip, &current, owner, inventory, request) {
            Ok(request) => {
                info!("hud: slave equipment {:?} on slave {} (tl {})", request.second_location, current.obj_id, current.tl_id);
                net.write(NetCommand(Command::Slave(SlaveCommand::ChangeEquipment(Box::new(request)))));
            }
            Err(why) => debug!("hud: slave equipment refused: {why}"),
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        update
            .after(super::sync_hud)
            .before(aa_ui::UiFrame)
            .run_if(super::hud_up),
    )
    .add_systems(OnExit(crate::Screen::InGame), |bridge: NonSend<HudBridge>| {
        bridge.0.borrow_mut().vehicles = default()
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_with(template: u32, point: Option<u8>, skill: Skill, id: u32) -> HudData {
        let mut data = HudData::default();
        let mut game = GameData::default();
        game.vehicles.skills.insert(template, vec![id]);
        game.vehicles.skill_rows.insert(id, skill);
        assert!(data.game.0.set(game).is_ok());
        data.vehicles.current = Some(Current {
            obj_id: 500,
            tl_id: 12,
            db_id: 0,
            template,
            equipment: Vec::new(),
            point,
            name: "Tractor".into(),
            level: 1,
            hp: 100,
            max_hp: 100,
            pos: Vec3::new(10.0, 20.0, 30.0),
            speed: 4.0,
            turn: 0.4,
            land: true,
            ship: false,
            cannon: false,
        });
        data
    }

    #[test]
    fn mode_slots_exist_only_while_seated_and_use_the_slave_caster() {
        let skill = Skill { combat: 19094, first_seat: Some(1), target_type: 0, target_selection: 1 };
        let data = data_with(60, None, skill, 7);
        assert!(use_action(&data, 1, 0, None).is_none(), "no mode skills before boarding");
        let mut data = data_with(60, Some(1), skill, 7);
        let command = use_action(&data, 1, 0, None).expect("self vehicle skill");
        assert!(matches!(
            command,
            Command::UseMateSkill { skill_id: 19094, mate_obj_id: 500, mount_skill_id: 7, target_obj_id: 500 }
        ));
        data.vehicles.current.as_mut().unwrap().point = Some(2);
        assert!(use_action(&data, 1, 0, None).is_none(), "driver-only skill on the passenger seat");
    }

    #[test]
    fn ship_mount_skills_ignore_the_land_driver_seat_filter() {
        let skill = Skill { combat: 17649, first_seat: Some(80), target_type: 0, target_selection: 1 };
        let mut data = data_with(75, Some(1), skill, 122);
        data.vehicles.current.as_mut().unwrap().land = false;
        assert!(use_action(&data, 1, 0, None).is_none(), "land filter hides an equipment-point row");
        data.vehicles.current.as_mut().unwrap().ship = true;
        assert!(matches!(
            use_action(&data, 1, 0, None),
            Some(Command::UseMateSkill { skill_id: 17649, mate_obj_id: 500, mount_skill_id: 122, target_obj_id: 500 })
        ));
    }

    #[test]
    fn position_vehicle_skill_uses_the_target_spot_and_hostile_needs_a_target() {
        let skill = Skill { combat: 11328, first_seat: None, target_type: 6, target_selection: 2 };
        let data = data_with(60, Some(1), skill, 12);
        assert!(use_action(&data, 1, 0, Some(Vec3::new(1.0, 2.0, 3.0))).is_none(), "needs a target");
        let command = use_action(&data, 1, 987, Some(Vec3::new(1.0, 2.0, 3.0))).expect("position skill");
        assert!(matches!(
            command,
            Command::UseMateSkillAt { skill_id: 11328, mate_obj_id: 500, mount_skill_id: 12, pos } if (pos.x, pos.y, pos.z) == (1.0, 2.0, 3.0)
        ));
        let skill = Skill { combat: 11017, first_seat: None, target_type: 4, target_selection: 2 };
        let data = data_with(60, Some(1), skill, 2);
        assert!(use_action(&data, 1, 0, None).is_none());
        assert!(matches!(
            use_action(&data, 1, 987, None),
            Some(Command::UseMateSkill { target_obj_id: 987, .. })
        ));
    }

    #[test]
    fn holding_change_releases_the_bridge_borrow_and_speed_getters_always_answer_numbers() {
        let bridge: Shared = std::rc::Rc::new(std::cell::RefCell::new(HudData::default()));
        assert!(!note_holding(&bridge, false), "unchanged");
        assert!(bridge.try_borrow_mut().is_ok());
        assert!(!note_holding(&bridge, true), "no slave owned");
        bridge.borrow_mut().vehicles.current = data_with(60, None, Skill { combat: 1, first_seat: None, target_type: 0, target_selection: 1 }, 1).vehicles.current;
        assert!(!note_holding(&bridge, true), "unchanged again");
        assert!(note_holding(&bridge, false));
        assert!(bridge.try_borrow_mut().is_ok());
        // the Summon Info window multiplies the answers: nil would raise every OnUpdate
        let lua = Lua::new();
        let weapon = lua.create_table().unwrap();
        lua.globals().set("X2SiegeWeapon", weapon).unwrap();
        lua.globals().set("X2Unit", lua.create_table().unwrap()).unwrap();
        lua.globals().set("HUD", lua.create_table().unwrap()).unwrap();
        bridge.borrow_mut().vehicles.current = None;
        let _ = install(&lua, &bridge);
        let sum: f64 = lua.load("return X2SiegeWeapon:GetSiegeWeaponSpeed() + X2SiegeWeapon:GetSiegeWeaponTurnSpeed()").eval().unwrap();
        assert_eq!(sum, 0.0);
    }

    /// A world with the owned ship (obj 500) and a cannon child slave (obj 600, template 10) of it.
    fn deck(seat: Option<aa_net::model::Attachment>) -> (World, HudData) {
        let data = HudData { character_id: 47, ..HudData::default() };
        assert!(data.game.0.set(GameData::load(aa_anim::db::DEFAULT_DB).expect("the server database")).is_ok());
        let mut spawn = crate::hud::expansion::fixture::spawn();
        spawn.unit.attached_to = seat;
        let mut units = Units::default();
        let mut ship = spawn.unit.clone();
        ship.obj_id = 500;
        ship.name = "Ship".into();
        ship.kind = UnitKind::Slave { id: 9, tl_id: 12, template_id: 22, summoner_id: 47 };
        units.by_id.insert(500, ship);
        let mut cannon = spawn.unit.clone();
        cannon.obj_id = 600;
        cannon.name = "Cannon".into();
        cannon.kind = UnitKind::Slave { id: 10, tl_id: 13, template_id: 10, summoner_id: 47 };
        units.by_id.insert(600, cannon);
        let mut slaves = Slaves::default();
        slaves.apply(&aa_net::Event::PlayerSpawn(Box::new(spawn.clone())));
        slaves.apply(&aa_net::Event::Slave(aa_net::slave::SlaveEvent::Created(aa_net::slave::SlaveCreated {
            owner_obj_id: spawn.obj_id,
            tl_id: 12,
            obj_id: 500,
            hide_spawn_effect: false,
            raw_id: 0,
            creator_name: String::new(),
        })));
        let mut world = World::new();
        world.insert_resource(units);
        world.insert_resource(slaves);
        world.insert_resource(Player { spawn: Some(spawn) });
        (world, data)
    }

    #[test]
    fn the_mode_bar_follows_the_cannon_the_player_mans_and_casts_it_at_a_spot() {
        let (world, mut data) = deck(None);
        refresh(&world, &mut data);
        let ship = data.vehicles.current.clone().expect("the owned ship");
        assert_eq!((ship.obj_id, ship.cannon, ship.point), (500, false, None));
        assert_eq!(ship.db_id, 9);
        // seated at the cannon's gunner seat (point 1): the bar is the cannon's, not the ship's
        let (world, mut data) = deck(Some(aa_net::model::Attachment { obj_id: 600, point: 1 }));
        refresh(&world, &mut data);
        let gun = data.vehicles.current.clone().expect("the manned cannon");
        assert_eq!((gun.obj_id, gun.tl_id, gun.cannon, gun.point), (600, 13, true, Some(1)));
        let game = data.game.get().unwrap();
        let skills: Vec<_> = displayed(game, &gun).map(|(_, s)| s.combat).collect();
        assert_eq!(skills, [10473], "Fire Cannon");
        // Fire Cannon is a position skill: the ground reticle, cast by the cannon with its mount skill
        let press = ground_press(&data, 1).expect("ground press");
        assert_eq!((press.skill, press.caster), (10473, Some((600, 16))));
        assert!(ground_press(&data, 2).is_none());
        // the same click on the owned ship's bar never aims a cannon
        let (world, mut data) = deck(Some(aa_net::model::Attachment { obj_id: 500, point: 1 }));
        refresh(&world, &mut data);
        assert!(ground_press(&data, 1).is_none());
    }
}
