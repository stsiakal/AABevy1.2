//! One received, uniquely correlated current mate. Local commands never create a pet/unit.
use super::*;
use aa_net::Mates;
use aa_net::mate::{MateCommand, MateSpawn};
use aa_net::model::{Unit, UnitKind};
use aa_net::state::Target;
use mlua::MultiValue;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct Tables {
    levels: Vec<(u32, i32)>,
    skills: HashMap<u32, Skill>,
    seats: HashMap<u32, (bool, bool)>,
    equipment: HashMap<u32, [bool; 4]>,
}

struct Skill {
    combat: u32,
    first_seat: Option<u32>,
    level: i32,
    target_type: u32,
    target_selection: u32,
}

impl Tables {
    pub(super) fn load(conn: &Connection) -> rusqlite::Result<Self> {
        let mut out = Self::default();
        let mut query = conn.prepare("SELECT n.id,p.head,p.chest,p.waist,p.feet FROM npcs n JOIN mate_equip_slot_packs p ON p.id=n.mate_equip_slot_pack_id")?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            out.equipment.insert(
                any_int(row, 0)? as u32,
                [
                    any_flag(row, 1)?,
                    any_flag(row, 2)?,
                    any_flag(row, 3)?,
                    any_flag(row, 4)?,
                ],
            );
        }
        let mut query = conn.prepare("SELECT id,total_mate_exp FROM levels ORDER BY id")?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            out.levels.push((any_int(row, 0)? as u32, any_int(row, 1)? as i32));
        }
        let mut first = HashMap::new();
        let mut query = conn.prepare("SELECT id,attach_point_id,mount_skill_id,skill_id FROM mount_attached_skills")?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            first.entry(any_int(row, 2)? as u32).or_insert(any_int(row, 1)? as u32);
        }
        let mut query =
            conn.prepare("SELECT m.id,m.skill_id,s.ability_level,s.target_type_id,s.target_selection_id FROM mount_skills m INNER JOIN skills s ON s.id=m.skill_id ORDER BY m.id")?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            let id = any_int(row, 0)? as u32;
            out.skills.insert(
                id,
                Skill {
                    combat: any_int(row, 1)? as u32,
                    first_seat: first.get(&id).copied(),
                    level: any_int(row, 2)? as i32,
                    target_type: any_int(row, 3)? as u32,
                    target_selection: any_int(row, 4)? as u32,
                },
            );
        }
        let mut query = conn.prepare("SELECT owner_id,attach_point_id FROM model_bindings WHERE owner_type='Model' ORDER BY id")?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            let point = any_int(row, 1)?;
            let entry = out.seats.entry(any_int(row, 0)? as u32).or_default();
            if point == 1 {
                entry.0 = true;
            } else if point == 2 {
                entry.1 = true;
            }
        }
        Ok(out)
    }

    fn progress(&self, experience: i32) -> Option<[f64; 4]> {
        let mut level = 1;
        for &(next, total) in &self.levels {
            if next > 1 && next <= 100 && experience >= total {
                level = next;
            }
        }
        let start = self.levels.iter().find(|row| row.0 == level)?.1;
        let next = self.levels.iter().find(|row| row.0 == level + 1)?.1;
        let needed = next.wrapping_sub(start);
        let raw = (experience.wrapping_sub(start) as f32) / (needed as f32) * 100.0f32;
        let upper = ((raw + 100.0f32) - (raw - 100.0f32).abs()) * 0.5f32;
        let upper = f64::from(upper);
        let percent = ((upper + upper.abs()) * 0.5) as f32;
        Some([
            f64::from(percent),
            f64::from(experience as f32),
            f64::from(start as f32),
            f64::from(needed as f32),
        ])
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Pet {
    spawn: MateSpawn,
    object: u32,
    model: u32,
    template: u32,
    equipment: aa_net::model::Equipment,
    name: String,
    owner_name: String,
    level: u8,
    hp: i32,
    max_hp: i32,
    mp: i32,
    max_mp: i32,
    dead: bool,
    combat: bool,
    buffs: super::super::panels::BuffLists,
    mounted: bool,
    attached_to_own: bool,
    driving: bool,
    passenger: bool,
    seat1: bool,
    seat2: bool,
    /// The horizontal speed of the last received movement report (m/s).
    speed: f32,
    pos: Vec3,
    info: Option<super::super::stats::MateDetails>,
}

/// The selected unit when it is somebody's mount (the original `CanMount` tries the target first).
#[derive(Clone, Debug, PartialEq)]
struct TargetMount {
    obj_id: u32,
    tl_id: u16,
    model: u32,
    owner: u32,
    dead: bool,
    seat1: bool,
    seat2: bool,
    distance: f32,
    pos: Vec3,
}

/// Where the local player sits (the server's `SCUnitAttached` for its own object).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LocalMount {
    obj_id: u32,
    tl_id: u16,
    point: u8,
    own: bool,
}

struct Request {
    owner: u32,
    /// The owned current pet the request was made for, when it is ours (`None`: a selected mount).
    identity: Option<(u32, u16, u32)>,
    obj_id: u32,
    tl_id: u16,
    command: MateCommand,
}

impl Pet {
    fn identity(&self) -> (u32, u16, u32) {
        (self.object, self.spawn.tl_id, self.spawn.id)
    }
}

#[derive(Default)]
pub(super) struct View {
    current: Option<Pet>,
    pushed: Option<Pet>,
    pushed_mount: Option<LocalMount>,
    local_state: Option<((u32, u16, u32), u8)>,
    // Native +58 starts at0; original local action/AI planner is not wired yet.
    commanded_skill: u32,
    mounted: bool,
    local_mount: Option<LocalMount>,
    target_mount: Option<TargetMount>,
    own_obj: u32,
    /// Shared native two-second vehicle-action cooldown (`_DAT_3a169944`).
    last_action_ms: Option<u64>,
    /// A mounted dismiss continues with `CSRemoveMate` after the server's detach.
    pending_dismiss: bool,
    requests: Vec<Request>,
}

fn current<'a>(mates: &Mates, units: &'a Units, owner: u32) -> Option<(&'a Unit, MateSpawn)> {
    let owned = mates.own.as_ref()?;
    let unit = units.get(owned.obj_id?)?;
    if owner == 0 || !matches!(unit.kind,UnitKind::Mate { tl_id,owner_id,.. } if tl_id!=0 && tl_id==owned.spawn.tl_id && owner_id==owner) {
        return None;
    }
    Some((unit, owned.spawn.clone()))
}

fn seat_occupancy(units: &Units, obj_id: u32) -> (bool, bool) {
    let mut seat1 = false;
    let mut seat2 = false;
    for unit in units.iter() {
        if let Some(a) = unit.attached_to.filter(|a| a.obj_id == obj_id) {
            if a.point == 1 {
                seat1 = true;
            } else if a.point == 2 {
                seat2 = true;
            }
        }
    }
    (seat1, seat2)
}

/// The horizontal speed of the last movement report, in m/s.
fn reported_speed(unit: &Unit) -> f32 {
    unit.movement.as_ref().map_or(0.0, |m| {
        let x = f32::from(m.velocity[0]) / 1024.0;
        let y = f32::from(m.velocity[1]) / 1024.0;
        x.hypot(y)
    })
}

pub(super) fn refresh(world: &World, data: &mut HudData) {
    let units = world.get_resource::<Units>();
    let player = world.get_resource::<Player>().and_then(|p| p.spawn.as_ref());
    let mates = world.get_resource::<Mates>();
    let own = player.and_then(|p| units.map(|u| u.get(p.obj_id).unwrap_or(&p.unit)));
    let local_mount = own.and_then(|rider| {
        let a = rider.attached_to?;
        let mount = units?.get(a.obj_id)?;
        let UnitKind::Mate { tl_id, .. } = mount.kind else { return None };
        Some(LocalMount {
            obj_id: a.obj_id,
            tl_id,
            point: a.point,
            own: mates.and_then(|m| m.own.as_ref()).is_some_and(|m| m.obj_id == Some(a.obj_id)),
        })
    });
    data.pets.owned.own_obj = player.map_or(0, |p| p.obj_id);
    data.pets.owned.mounted = local_mount.is_some();
    data.pets.owned.local_mount = local_mount;
    data.pets.owned.target_mount = (|| {
        let rider = own?;
        let units = units?;
        let target = world.get_resource::<Target>()?.obj_id?;
        let unit = units.get(target)?;
        let UnitKind::Mate { tl_id, owner_id, .. } = unit.kind else { return None };
        if some_own_pet(mates, target) {
            return None;
        }
        let (seat1, seat2) = seat_occupancy(units, target);
        let dx = f64::from(unit.pos.x - rider.pos.x);
        let dy = f64::from(unit.pos.y - rider.pos.y);
        let dz = f64::from(unit.pos.z - rider.pos.z);
        Some(TargetMount {
            obj_id: target,
            tl_id,
            model: unit.model_id,
            owner: owner_id,
            dead: unit.dead,
            seat1,
            seat2,
            distance: (dx * dx + dy * dy + dz * dz).sqrt() as f32,
            pos: Vec3::new(unit.pos.x, unit.pos.y, unit.pos.z),
        })
    })();
    let next = (|| {
        let units = units?;
        let mates = mates?;
        let (unit, spawn) = current(mates, units, data.character_id)?;
        let game = data.game.get();
        let buffs = world.get_resource::<aa_net::state::Buffs>().map_or(&[][..], |b| b.of(unit.obj_id));
        let native_buffs = world.get_resource::<aa_net::state::Buffs>().and_then(|b| b.native.of(unit.obj_id));
        let (max_hp, max_mp) = game
            .and_then(|g| super::super::stats::mate_max(g, unit, native_buffs))
            .unwrap_or((unit.max_hp, unit.max_mp));
        let me = world.get_resource::<Player>()?.spawn.as_ref()?;
        let rider = units.get(me.obj_id).unwrap_or(&me.unit);
        let mounted = rider
            .attached_to
            .and_then(|a| units.get(a.obj_id))
            .is_some_and(|u| matches!(u.kind, UnitKind::Mate { .. }));
        let attached_to_own = rider.attached_to.is_some_and(|a| a.obj_id == unit.obj_id);
        let driving = rider.attached_to.is_some_and(|a| a.obj_id == unit.obj_id && a.point == 1);
        let (seat1, seat2) = seat_occupancy(units, unit.obj_id);
        let name = if unit.name.is_empty() {
            game.and_then(|g| g.npcs.get(&unit.kind.template_id()))
                .map(|n| n.name.clone())
                .unwrap_or_default()
        } else {
            unit.name.clone()
        };
        let combat = data
            .pets
            .owned
            .current
            .as_ref()
            .filter(|p| p.object == unit.obj_id && p.spawn.tl_id == spawn.tl_id && p.spawn.id == spawn.id)
            .is_some_and(|p| p.combat);
        Some(Pet {
            spawn,
            object: unit.obj_id,
            model: unit.model_id,
            template: unit.kind.template_id(),
            equipment: unit.equipment.clone(),
            name,
            owner_name: unit.owner_name.clone(),
            level: unit.level,
            hp: unit.hp,
            max_hp: max_hp.max(unit.hp).max(1),
            mp: unit.mp,
            max_mp: max_mp.max(unit.mp),
            dead: unit.dead,
            combat,
            buffs: super::super::panels::buff_lists(game, buffs, data.start),
            mounted,
            attached_to_own,
            driving,
            passenger: seat2,
            seat1,
            seat2,
            speed: reported_speed(unit),
            pos: Vec3::new(unit.pos.x, unit.pos.y, unit.pos.z),
            info: game.and_then(|game| super::super::stats::mate_details(game, unit, native_buffs)),
        })
    })();
    if data.pets.owned.current.as_ref().map(Pet::identity) != next.as_ref().map(Pet::identity) {
        data.pets.owned.local_state = None;
        data.pets.owned.commanded_skill = 0;
        data.pets.owned.pending_dismiss = false;
    }
    data.pets.owned.current = next;
}

fn some_own_pet(mates: Option<&Mates>, obj_id: u32) -> bool {
    mates.and_then(|m| m.own.as_ref()).is_some_and(|m| m.obj_id == Some(obj_id))
}

fn now_ms(data: &HudData) -> u64 {
    data.start.elapsed().as_millis() as u64
}

/// The native shared two-second vehicle cooldown (`_DAT_3a169944`, compared with `1999 < dt`).
fn on_vehicle_cooldown(data: &HudData) -> bool {
    data.pets
        .owned
        .last_action_ms
        .is_some_and(|then| now_ms(data).saturating_sub(then) <= 1999)
}

/// The native controller/casting/channel gate of `CanDismiss` (reason 7 from `39357830`).
fn casting(data: &HudData) -> bool {
    data.casts.own != 0 && data.casts.casts.contains_key(&data.casts.own)
}

fn mount_seats(data: &HudData, model: u32) -> (bool, bool) {
    data.game
        .get()
        .and_then(|g| g.pets.owned.seats.get(&model))
        .copied()
        .unwrap_or((false, false))
}

fn own_pet_can_mount(data: &HudData) -> bool {
    let Some(pet) = &data.pets.owned.current else { return false };
    if pet.mounted || pet.dead || data.pos.distance(pet.pos) > 25.0 || casting(data) || on_vehicle_cooldown(data) {
        return false;
    }
    let (driver, _) = mount_seats(data, pet.model);
    driver && !pet.seat1
}

fn target_mount_point(data: &HudData, target: &TargetMount) -> Option<u8> {
    if target.dead || target.distance > 25.0 || casting(data) || on_vehicle_cooldown(data) {
        return None;
    }
    let (driver, passenger) = mount_seats(data, target.model);
    if target.owner == data.character_id && driver && !target.seat1 {
        Some(1)
    } else if passenger && !target.seat2 {
        Some(2)
    } else {
        None
    }
}

fn can_mount_any(data: &HudData) -> bool {
    if data.pets.owned.mounted || casting(data) || on_vehicle_cooldown(data) {
        return false;
    }
    own_pet_can_mount(data)
        || data
            .pets
            .owned
            .target_mount
            .as_ref()
            .is_some_and(|t| target_mount_point(data, t).is_some())
}

/// Native `FUN_3909ae90`: the approach-geometry reason byte for `CSMountMate`. It is the direction
/// from the mount to the player projected on the player's forward: `>= 0.96` is behind (3), `< -0.96`
/// in front (4), the band between is the left/right side (1/2 by the cross sign). An occupied
/// passenger seat forces 1 for a rear approach; an occupied driver seat forces 1 otherwise.
fn mount_reason(data: &HudData, mount_pos: Vec3, seat1: bool, seat2: bool) -> u8 {
    let diff = mount_pos - data.pos;
    let fwd = Vec2::new(-data.yaw.sin(), data.yaw.cos());
    let len = diff.truncate().length();
    if len <= 1e-5 {
        return 2;
    }
    let dot = -(diff.x * fwd.x + diff.y * fwd.y) / len;
    if 0.96 <= dot {
        if seat2 { 1 } else { 3 }
    } else if dot < -0.96 {
        if seat1 { 1 } else { 4 }
    } else if seat1 {
        1
    } else if diff.x * fwd.y - diff.y * fwd.x >= 0.0 {
        2
    } else {
        1
    }
}

fn queue_mount(data: &mut HudData, obj_id: u32, tl_id: u16, point: u8, seats: (bool, bool), mount_pos: Vec3, own: Option<(u32, u16, u32)>) {
    if on_vehicle_cooldown(data) || casting(data) {
        return;
    }
    let reason = mount_reason(data, mount_pos, seats.0, seats.1);
    data.pets.owned.last_action_ms = Some(now_ms(data));
    let command = MateCommand::Mount { tl_id, point, reason };
    if command.validate().is_err() {
        return;
    }
    let owner = data.character_id;
    data.pets.owned.requests.push(Request { owner, identity: own, obj_id, tl_id, command });
}

fn mount_mate(data: &mut HudData) {
    if data.pets.owned.mounted || on_vehicle_cooldown(data) || casting(data) {
        return;
    }
    if let Some(target) = data.pets.owned.target_mount.clone()
        && let Some(point) = target_mount_point(data, &target)
    {
        queue_mount(data, target.obj_id, target.tl_id, point, (target.seat1, target.seat2), target.pos, None);
        return;
    }
    let Some(pet) = data.pets.owned.current.clone() else { return };
    if !own_pet_can_mount(data) {
        return;
    }
    let own = Some(pet.identity());
    queue_mount(data, pet.object, pet.spawn.tl_id, 1, (pet.seat1, pet.seat2), pet.pos, own);
}

fn unmount_mate(data: &mut HudData) {
    let Some(local) = data.pets.owned.local_mount else { return };
    if on_vehicle_cooldown(data) || casting(data) {
        return;
    }
    let command = MateCommand::Unmount { tl_id: local.tl_id, point: local.point, reason: 9 };
    if command.validate().is_err() {
        return;
    }
    data.pets.owned.last_action_ms = Some(now_ms(data));
    let owner = data.character_id;
    data.pets.owned.requests.push(Request { owner, identity: None, obj_id: local.obj_id, tl_id: local.tl_id, command });
}

fn dismiss_pet(data: &mut HudData) {
    let Some(pet) = data.pets.owned.current.clone() else { return };
    if on_vehicle_cooldown(data) || casting(data) {
        return;
    }
    if let Some(local) = data.pets.owned.local_mount.filter(|l| l.own && l.obj_id == pet.object) {
        // Native `39362070`: a mounted dismiss first unmounts with reason 10; the remove follows
        // after the server's detach (the model-action continuation).
        let command = MateCommand::Unmount { tl_id: local.tl_id, point: local.point, reason: 10 };
        if command.validate().is_err() {
            return;
        }
        data.pets.owned.pending_dismiss = true;
        data.pets.owned.last_action_ms = Some(now_ms(data));
        let owner = data.character_id;
        data.pets.owned.requests.push(Request { owner, identity: None, obj_id: local.obj_id, tl_id: local.tl_id, command });
        return;
    }
    let own = Some(pet.identity());
    let command = MateCommand::Dismiss { tl_id: pet.spawn.tl_id };
    if command.validate().is_err() {
        return;
    }
    data.pets.owned.last_action_ms = Some(now_ms(data));
    let owner = data.character_id;
    data.pets.owned.requests.push(Request { owner, identity: own, obj_id: pet.object, tl_id: pet.spawn.tl_id, command });
}

fn kick_passenger(data: &mut HudData) {
    let Some(pet) = data.pets.owned.current.clone() else { return };
    if !pet.passenger || on_vehicle_cooldown(data) || casting(data) {
        return;
    }
    // Native `390697d0` / `39361fc0` sends the passenger seat with reason 11. The lane rejects a
    // non-occupant's unmount, so this remains the exact request without a fake local detach.
    let command = MateCommand::Unmount { tl_id: pet.spawn.tl_id, point: 2, reason: 11 };
    if command.validate().is_err() {
        return;
    }
    data.pets.owned.last_action_ms = Some(now_ms(data));
    let owner = data.character_id;
    data.pets.owned.requests.push(Request { owner, identity: None, obj_id: pet.object, tl_id: pet.spawn.tl_id, command });
}

/// The native pet action slot (`ISLOT_PET_ACTION`): resolve the displayed mount skill and choose the
/// target of its `target_type_id`. Self-target skills name the mate (the server's mount caster
/// resolves `SelfTarget` there); unit-target skills use the player's selection; position-targeted
/// skills (`Pos`, `BallisticPos`, `RelativePos`, `ArtilleryPos`) send the mate-caster position target
/// at the selected unit's spot (the mount skill rows use `target_selection_id = 2`). A source-position
/// row centers on the mate's own spot.
pub(super) fn use_pet_action(data: &HudData, index: usize, target_obj_id: u32, target_pos: Option<Vec3>) -> Option<aa_net::Command> {
    let pet = data.pets.owned.current.as_ref()?;
    let game = data.game.get()?;
    let (raw, skill) = displayed(&game.pets.owned, pet, index)?;
    if i32::from(pet.level) < skill.level {
        return None;
    }
    let mount_skill_id = *pet.spawn.skills.get(raw)?;
    if mount_skill_id == 0 {
        return None;
    }
    match skill.target_type {
        0 => Some(aa_net::Command::UseMateSkill {
            skill_id: skill.combat,
            mate_obj_id: pet.object,
            mount_skill_id,
            target_obj_id: pet.object,
        }),
        4 | 5 | 16 | 17 => {
            if target_obj_id == 0 || target_obj_id == pet.object {
                return None;
            }
            Some(aa_net::Command::UseMateSkill { skill_id: skill.combat, mate_obj_id: pet.object, mount_skill_id, target_obj_id })
        }
        6 | 11 | 13 | 15 => {
            let pos = if skill.target_selection == 1 {
                pet.pos
            } else {
                if target_obj_id == 0 {
                    return None;
                }
                target_pos?
            };
            Some(aa_net::Command::UseMateSkillAt {
                skill_id: skill.combat,
                mate_obj_id: pet.object,
                mount_skill_id,
                pos: aa_net::Pos::new(pos.x, pos.y, pos.z),
            })
        }
        _ => {
            let target_obj_id = if target_obj_id == 0 { pet.object } else { target_obj_id };
            Some(aa_net::Command::UseMateSkill { skill_id: skill.combat, mate_obj_id: pet.object, mount_skill_id, target_obj_id })
        }
    }
}

fn table(lua: &Lua, pet: &Pet) -> mlua::Result<Table> {
    let out = lua.create_table()?;
    out.set("unit_id", pet.object.to_string())?;
    out.set("name", pet.name.as_str())?;
    out.set("owner_name", pet.owner_name.as_str())?;
    out.set("level", pet.level)?;
    out.set("hp", pet.hp)?;
    out.set("max_hp", pet.max_hp)?;
    out.set("mp", pet.mp)?;
    out.set("max_mp", pet.max_mp)?;
    out.set("dead", pet.dead)?;
    out.set("in_combat", pet.combat)?;
    out.set("unit_type", "npc")?;
    out.set("detail", "mate")?;
    out.set("relation", "friendly")?;
    out.set("buffs", super::super::panels::buffs_table(lua, &pet.buffs)?)?;
    Ok(out)
}

pub(super) fn install(lua: &Lua, bridge: &Shared) -> mlua::Result<()> {
    info::install(lua, bridge)?;
    equipment::install(lua, bridge)?;
    let mate: Table = lua.globals().get("X2Mate")?;
    for name in [
        "IsPlayerPetExists",
        "IsPlayerMounting",
        "IsMountablePet",
        "IsHavePassengerSeat",
        "HavePassenger",
        "IsAttackablePet",
    ] {
        let state = bridge.clone();
        mate.set(
            name,
            lua.create_function(move |_, _: Value| {
                let data = state.borrow();
                if name == "IsPlayerMounting" {
                    return Ok(data.pets.owned.mounted);
                }
                let Some(pet) = &data.pets.owned.current else { return Ok(false) };
                let tables = data.game.get().map(|g| &g.pets.owned);
                Ok(match name {
                    "IsPlayerPetExists" => true,
                    "IsPlayerMounting" => pet.mounted,
                    "HavePassenger" => pet.passenger,
                    "IsAttackablePet" => tables.is_some_and(|t| pet.spawn.skills.iter().any(|id| t.skills.get(id).is_some_and(|s| s.first_seat.is_none()))),
                    "IsMountablePet" => tables.and_then(|t| t.seats.get(&pet.model)).is_some_and(|s| s.0),
                    "IsHavePassengerSeat" => tables.and_then(|t| t.seats.get(&pet.model)).is_some_and(|s| s.1),
                    _ => false,
                })
            })?,
        )?;
    }
    let state = bridge.clone();
    mate.set(
        "PetState",
        lua.create_function(move |_, _: Value| {
            let data = state.borrow();
            Ok(data.pets.owned.current.as_ref().filter(|p| !p.attached_to_own).and_then(|p| {
                let mode = data.pets.owned.local_state.filter(|s| s.0 == p.identity()).map_or(p.spawn.user_state, |s| s.1);
                ["idle", "passive", "protective", "aggressive"].get(mode as usize).copied()
            }))
        })?,
    )?;
    let state = bridge.clone();
    mate.set(
        "GetNumMountSkills",
        lua.create_function(move |_, _: Value| {
            let data = state.borrow();
            Ok(data.pets.owned.current.as_ref().zip(data.game.get()).map_or(0, |(p, g)| {
                p.spawn
                    .skills
                    .iter()
                    .filter(|id| g.pets.owned.skills.get(id).is_some_and(|s| s.first_seat.is_none() || s.first_seat == Some(1)))
                    .count()
            }))
        })?,
    )?;
    let state = bridge.clone();
    mate.set(
        "SetMateName",
        lua.create_function(move |_, (_, name): (Value, String)| {
            let mut data = state.borrow_mut();
            if let Some(pet) = &data.pets.owned.current {
                let identity = pet.identity();
                let (obj_id, tl_id) = (pet.object, pet.spawn.tl_id);
                let command = MateCommand::Rename { tl_id, name };
                if command.validate().is_ok() {
                    let owner = data.character_id;
                    data.pets.owned.requests.push(Request { owner, identity: Some(identity), obj_id, tl_id, command });
                }
            }
            Ok(())
        })?,
    )?;
    let state = bridge.clone();
    mate.set(
        "GetMountSkill",
        lua.create_function(move |lua, (_, index): (Value, usize)| {
            let entry = {
                let data = state.borrow();
                data.pets.owned.current.as_ref().zip(data.game.get()).and_then(|(pet, game)| {
                    let (_, skill) = displayed(&game.pets.owned, pet, index)?;
                    Some((
                        skill.combat,
                        skill.level,
                        i32::from(pet.level) >= skill.level,
                        skill.first_seat.is_some(),
                        skill.target_type == 6,
                    ))
                })
            };
            let Some((combat, level, learned, driver, usable)) = entry else {
                return Ok(None);
            };
            let hud: Table = lua.globals().get("HUD")?;
            let get: mlua::Function = hud.get("skill_tip")?;
            let Some(info) = get.call::<Option<Table>>(combat)? else { return Ok(None) };
            info.set("learned", learned)?;
            info.set("skillUsable", i32::from(usable))?;
            info.set("needdiver", driver)?;
            info.set("learnLevel", level)?;
            Ok(Some(info))
        })?,
    )?;
    let state = bridge.clone();
    mate.set(
        "GetAutoStartMountSkill",
        lua.create_function(move |_, _: MultiValue| {
            Ok(if state.borrow().pets.owned.current.is_some() {
                MultiValue::from_iter([Value::Boolean(false)])
            } else {
                MultiValue::new()
            })
        })?,
    )?;
    // Native `CanMount` 3906a440: the selected mount first, then the current own pet (3935aa10).
    let state = bridge.clone();
    mate.set(
        "CanMount",
        lua.create_function(move |_, _: Value| Ok(can_mount_any(&state.borrow())))?,
    )?;
    // Native `CanDismiss` 3906a560 + 39357830: a pet, no pending model action (here: our queued
    // dismiss) and the controller's casting/channel gate.
    let state = bridge.clone();
    mate.set(
        "CanDismiss",
        lua.create_function(move |_, _: Value| {
            let data = state.borrow();
            Ok(data.pets.owned.current.is_some() && !data.pets.owned.pending_dismiss && !casting(&data))
        })?,
    )?;
    let state = bridge.clone();
    mate.set(
        "MountMate",
        lua.create_function(move |_, _: Value| {
            mount_mate(&mut state.borrow_mut());
            Ok(())
        })?,
    )?;
    let state = bridge.clone();
    mate.set(
        "UnMountMate",
        lua.create_function(move |_, _: Value| {
            unmount_mate(&mut state.borrow_mut());
            Ok(())
        })?,
    )?;
    let state = bridge.clone();
    mate.set(
        "DismissPet",
        lua.create_function(move |_, _: Value| {
            dismiss_pet(&mut state.borrow_mut());
            Ok(())
        })?,
    )?;
    let state = bridge.clone();
    mate.set(
        "KickPassenger",
        lua.create_function(move |_, _: Value| {
            kick_passenger(&mut state.borrow_mut());
            Ok(())
        })?,
    )?;
    // Native 3906b899 -> 3906ac40: one f32 from the current mate's movement component; no values
    // without a pet. The wire's last movement report is the only runtime speed this client has.
    let state = bridge.clone();
    mate.set(
        "GetSpeedInfo",
        lua.create_function(move |_, _: Value| {
            Ok(match state.borrow().pets.owned.current.as_ref().map(|p| p.speed) {
                Some(speed) => MultiValue::from_iter([Value::Number(f64::from(speed))]),
                None => MultiValue::new(),
            })
        })?,
    )?;
    let state = bridge.clone();
    mate.set(
        "PassiveMode",
        lua.create_function(move |_, _: Value| {
            let mut data = state.borrow_mut();
            let Some(pet) = &data.pets.owned.current else { return Ok(false) };
            let identity = pet.identity();
            let (obj_id, tl_id) = (pet.object, pet.spawn.tl_id);
            let owner = data.character_id;
            data.pets.owned.local_state = Some((identity, 1));
            let command = MateCommand::UserState { tl_id, state: 1 };
            data.pets.owned.requests.push(Request { owner, identity: Some(identity), obj_id, tl_id, command });
            Ok(true)
        })?,
    )?;
    for (name, mode) in [("StandMode", 0), ("ProtectiveMode", 2), ("AggressiveMode", 3)] {
        let state = bridge.clone();
        mate.set(name, lua.create_function(move |_, _: Value| Ok(request_mode(&mut state.borrow_mut(), mode)))?)?;
    }
    let x2: Table = lua.globals().get("X2")?;
    let state = bridge.clone();
    x2.set(
        "GetPetExpToNextLevel",
        lua.create_function(move |_, _: Value| {
            let data = state.borrow();
            let progress = data
                .pets
                .owned
                .current
                .as_ref()
                .zip(data.game.get())
                .and_then(|(p, g)| g.pets.owned.progress(p.spawn.experience));
            Ok(progress.map_or_else(MultiValue::new, |values| MultiValue::from_iter(values.into_iter().map(Value::Number))))
        })?,
    )?;
    // Unit aliases use the same real unit getters as player/target/team in ingame.lua.
    let hud: Table = lua.globals().get("HUD")?;
    let state = bridge.clone();
    hud.set(
        "pet_slot",
        lua.create_function(move |lua, index: usize| {
            let data = state.borrow();
            let Some((pet, game)) = data.pets.owned.current.as_ref().zip(data.game.get()) else {
                return Ok(None);
            };
            let Some((_, skill)) = displayed(&game.pets.owned, pet, index) else {
                return Ok(None);
            };
            let Some(info) = game.skills.get(&skill.combat) else { return Ok(None) };
            let out = lua.create_table()?;
            out.set("id", skill.combat)?;
            out.set("icon", info.named.icon.as_str())?;
            out.set("learned", i32::from(pet.level) >= skill.level)?;
            out.set("canUse", skill.target_type == 6)?;
            out.set("type", "pet_skill")?;
            Ok(Some(out))
        })?,
    )?;
    hud.set("pet_target", {
        let state = bridge.clone();
        lua.create_function(move |_, ()| {
            let mut data = state.borrow_mut();
            if let Some(p) = &data.pets.owned.current {
                let obj_id = p.object;
                data.requests.push(super::super::api::Request::Target { obj_id });
            }
            Ok(())
        })?
    })?;
    lua.load(
        r#"
        local content=AAUI.SlotContent
        AAUI.SlotContent=function(kind,index)
          if kind==ISLOT_PET_ACTION then
            local slot=HUD.pet_slot(index)
            if slot then slot.tooltip=X2Mate:GetMountSkill(index) end
            return slot
          end
          return content(kind,index)
        end
        local target=X2Unit.TargetUnit
        X2Unit.TargetUnit=function(self,key)
          if HUD.playerpet and (key=='playerpet' or key==HUD.playerpet.unit_id) then HUD.pet_target()
          elseif target then return target(self,key) end
        end
        local info=X2Unit.GetUnitInfoById
        X2Unit.GetUnitInfoById=function(self,id)
          if HUD.playerpet and id==HUD.playerpet.unit_id then
            return {owner_name=HUD.playerpet.owner_name}
          end
          if info then return info(self,id) end
        end
    "#,
    )
    .set_name("@owned_pet.lua")
    .exec()?;
    let pet = bridge.borrow().pets.owned.current.clone();
    hud.set("playerpet", pet.as_ref().map(|pet| table(lua, pet)).transpose()?)?;
    bridge.borrow_mut().pets.owned.pushed = pet;
    Ok(())
}

pub(super) fn push(rt: &UiRuntime, bridge: &Shared) {
    let (old, next, old_mount, new_mount) = {
        let mut data = bridge.borrow_mut();
        let mount = data.pets.owned.local_mount;
        let mounted = std::mem::replace(&mut data.pets.owned.pushed_mount, mount);
        if data.pets.owned.current == data.pets.owned.pushed && mounted == mount {
            return;
        }
        let next = data.pets.owned.current.clone();
        let old = std::mem::replace(&mut data.pets.owned.pushed, next.clone());
        (old, next, mounted, mount)
    };
    let result = (|| -> mlua::Result<()> {
        let hud: Table = rt.lua.globals().get("HUD")?;
        hud.set("playerpet", next.as_ref().map(|pet| table(&rt.lua, pet)).transpose()?)
    })();
    if let Err(error) = result {
        warn!("hud: owned pet: {error}");
        return;
    }
    let identity_changed = old.as_ref().map(Pet::identity) != next.as_ref().map(Pet::identity);
    info::push(rt, bridge);
    // `MOUNT_PET(isMyPet)` / `UNMOUNT_PET(isMyPet)`: the native event follows the local player's
    // own seat, including a passenger seat on somebody else's mount.
    match (old_mount, new_mount) {
        (None, Some(m)) => rt.fire_event("MOUNT_PET", m.own),
        (Some(m), None) => rt.fire_event("UNMOUNT_PET", m.own),
        (Some(a), Some(b)) if a.obj_id != b.obj_id => {
            rt.fire_event("UNMOUNT_PET", a.own);
            rt.fire_event("MOUNT_PET", b.own);
        }
        _ => {}
    }
    if identity_changed {
        if old.is_some() {
            rt.fire_event("DISMISS_PET", ());
        }
        if next.is_some() {
            rt.fire_event("SPAWN_PET", ());
        }
    } else if let (Some(old), Some(next)) = (&old, &next) {
        if old.passenger != next.passenger {
            rt.fire_event(if next.passenger { "PASSENGER_MOUNT_PET" } else { "PASSENGER_UNMOUNT_PET" }, ());
        }
        if old.name != next.name {
            rt.fire_event("UNIT_NAME_CHANGED", next.object.to_string());
        }
        if old.level != next.level {
            rt.fire_event("LEVEL_CHANGED", (next.level, next.object.to_string()));
        }
    }
    if identity_changed { info::push(rt, bridge); }
    equipment::push(rt, old.as_ref(), next.as_ref(), identity_changed);
    if let Err(error) = rt.refresh_slots() {
        warn!("hud: pet slots: {error}");
    }
}

#[cfg(test)]
mod tests;
mod info;

fn displayed<'a>(tables: &'a Tables, pet: &Pet, index: usize) -> Option<(usize, &'a Skill)> {
    let context = u32::from(pet.driving);
    pet.spawn
        .skills
        .iter()
        .enumerate()
        .filter_map(|(raw, id)| {
            let skill = tables.skills.get(id)?;
            (skill.first_seat.is_none() || skill.first_seat == Some(context) || (context == 0 && skill.first_seat == Some(1))).then_some((raw, skill))
        })
        .nth(index.checked_sub(1)?)
}

fn request_mode(data: &mut HudData, mode: u8) -> bool {
    let Some(pet) = &data.pets.owned.current else { return false };
    if mode >= 2
        && !data.game.get().is_some_and(|g| {
            pet.spawn
                .skills
                .iter()
                .any(|id| g.pets.owned.skills.get(id).is_some_and(|s| s.first_seat.is_none()))
        })
    {
        return false;
    }
    let identity = pet.identity();
    let old = data.pets.owned.local_state.filter(|s| s.0 == identity).map_or(pet.spawn.user_state, |s| s.1);
    if mode != 1 && old == mode && data.pets.owned.commanded_skill == 0 {
        return false;
    }
    let (obj_id, tl_id) = (pet.object, pet.spawn.tl_id);
    let command = MateCommand::UserState { tl_id, state: mode };
    let owner = data.character_id;
    data.pets.owned.local_state = Some((identity, mode));
    data.pets.owned.commanded_skill = 0;
    data.pets.owned.requests.push(Request { owner, identity: Some(identity), obj_id, tl_id, command });
    true
}

pub(super) fn event(data: &mut HudData, event: &Event) -> Option<(bool, u32)> {
    if let Event::CombatChanged { obj_id, engaged } = event {
        let pet = data.pets.owned.current.as_mut().filter(|p| p.object == *obj_id)?;
        pet.combat = *engaged;
        return Some((*engaged, *obj_id));
    }
    // `39362070`: a mounted dismiss only unmounts (reason 10); the model-action continuation
    // sends the remove after the server's detach.
    if let Event::UnitDetached { obj_id, .. } = event
        && data.pets.owned.pending_dismiss
        && *obj_id == data.pets.owned.own_obj
        && let Some(pet) = data.pets.owned.current.clone()
    {
        data.pets.owned.pending_dismiss = false;
        data.pets.owned.last_action_ms = Some(now_ms(data));
        let owner = data.character_id;
        data.pets.owned.requests.push(Request {
            owner,
            identity: Some(pet.identity()),
            obj_id: pet.object,
            tl_id: pet.spawn.tl_id,
            command: MateCommand::Dismiss { tl_id: pet.spawn.tl_id },
        });
    }
    None
}

pub(super) fn commands(data: &mut HudData, player: &Player, units: &Units, mates: &Mates, net: &mut MessageWriter<NetCommand>) {
    let owner = player.spawn.as_ref().map_or(0, |p| p.character().id);
    let actual = current(mates, units, owner).map(|(unit, spawn)| (unit.obj_id, spawn.tl_id, spawn.id));
    for request in std::mem::take(&mut data.pets.owned.requests) {
        if owner == 0 || owner != request.owner || data.character_id != owner {
            continue;
        }
        let valid = match request.identity {
            Some(identity) => actual == Some(identity),
            // A selected mount is not our `Mates::own`; only a live mate with the quoted timeline.
            None => units
                .get(request.obj_id)
                .is_some_and(|u| matches!(u.kind, UnitKind::Mate { tl_id, .. } if tl_id == request.tl_id) && !u.dead),
        };
        if valid {
            net.write(NetCommand(Command::Mate(request.command)));
        }
    }
}

mod equipment;

pub(super) fn equipment_slots(data: &HudData, event: &Event) -> Vec<i64> {
    equipment::slots(data, event)
}
