//! Original mode action bar over an owned slave/land vehicle (`x2ui/hud/mode_action_bar/mode_action.alb`).
//!
//! The bar is the original script's: it asks `X2Unit:GetModeActionsCount()` and establishes
//! `ISLOT_MODE_ACTION` slots (`mode_action.alb:62-106`). The native client fills those slots from
//! the current slave's `slave_mount_skills` rows; this module resolves the same static mapping to
//! `mount_skills` combat records and sends the mate-style mount-caster `CSStartSkill` (the server's
//! skill handler accepts a slave caster through `mount_authority`).

use super::api::{HudBridge, HudData, Shared};
use super::data::{any_int, GameData};
use aa_net::model::{Unit, UnitKind};
use aa_net::slave_control::SlaveCommand;
use aa_net::state::{Player, Slaves, Units};
use aa_net::{Command, NetCommand};
use aa_ui::UiRuntime;
use bevy::prelude::*;
use mlua::{Lua, Table, Value};
use rusqlite::Connection;
use std::collections::HashMap;

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
        Ok(out)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Current {
    pub obj_id: u32,
    pub tl_id: u16,
    pub template: u32,
    /// The local player's seat (`Some(1)` driver, `Some(2)` passenger), if boarded.
    pub point: Option<u8>,
    pub name: String,
    pub level: u8,
    pub hp: i32,
    pub max_hp: i32,
    pub pos: Vec3,
    pub speed: f32,
    pub turn: f32,
}

#[derive(Default)]
pub(super) struct View {
    pub current: Option<Current>,
    pushed: Option<Current>,
    /// The player's requested leave, sent by `update` after an identity recheck.
    pub(super) requests: Vec<(u32, u16, SlaveCommand)>,
}

fn current(world: &World, data: &HudData) -> Option<Current> {
    let slaves = world.get_resource::<Slaves>()?;
    let own = slaves.own.as_ref()?;
    let units = world.get_resource::<Units>()?;
    let unit = units.get(own.obj_id)?;
    let UnitKind::Slave { tl_id, template_id, .. } = unit.kind else { return None };
    if tl_id != own.tl_id {
        return None;
    }
    let player = world.get_resource::<Player>()?.spawn.as_ref()?;
    let point = player.unit.attached_to.filter(|a| a.obj_id == own.obj_id).map(|a| a.point);
    let (speed, turn) = data
        .game
        .get()
        .and_then(|g| g.vehicles.speeds.get(&template_id).copied())
        .unwrap_or((8.0, 1.0));
    Some(Current {
        obj_id: own.obj_id,
        tl_id,
        template: template_id,
        point,
        name: unit_name(unit, data),
        level: unit.level,
        hp: unit.hp,
        max_hp: unit.max_hp.max(unit.hp).max(1),
        pos: Vec3::new(unit.pos.x, unit.pos.y, unit.pos.z),
        speed,
        turn,
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
            (slave.point.is_some()
                && (skill.first_seat.is_none()
                    || skill.first_seat == Some(u32::from(context))
                    || (context == 1 && skill.first_seat == Some(1))))
                .then_some((*id, skill))
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
        lua.create_function(move |_, _: Value| Ok(state.borrow().vehicles.current.as_ref().map(|v| f64::from(v.speed))))?,
    )?;
    let state = bridge.clone();
    weapon.set(
        "GetSiegeWeaponTurnSpeed",
        lua.create_function(move |_, _: Value| Ok(state.borrow().vehicles.current.as_ref().map(|v| f64::from(v.turn))))?,
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
            out.set("kind", v.template)?;
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
            let out = lua.create_table()?;
            out.set("speed", v.speed)?;
            out.set("turnSpeed", v.turn)?;
            Ok(Some(out))
        })?,
    )?;
    let state = bridge.clone();
    weapon.set(
        "GetSlaveCustomizingType",
        lua.create_function(move |_, _: Value| {
            let _ = state.borrow();
            Ok(None::<i64>)
        })?,
    )?;

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
    for (quoted_owner, tl_id, command) in requests {
        let actual = slaves
            .as_deref()
            .and_then(|s| s.own.as_ref())
            .filter(|own| own.tl_id == tl_id)
            .and_then(|own| units.get(own.obj_id))
            .is_some_and(|u| matches!(u.kind, UnitKind::Slave { tl_id: actual, .. } if actual == tl_id));
        if owner == quoted_owner && actual {
            net.write(NetCommand(Command::Slave(command)));
        }
    }
    push(&rt, &bridge.0);
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
            template,
            point,
            name: "Tractor".into(),
            level: 1,
            hp: 100,
            max_hp: 100,
            pos: Vec3::new(10.0, 20.0, 30.0),
            speed: 4.0,
            turn: 0.4,
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
}
