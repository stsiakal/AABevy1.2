//! The original 1.2 housing windows' data and interaction paths.
//!
//! `x2ui/housing/build_window.alb` is the retail "Start Construction" window: it asks `X2House` for the model
//! name, the construction steps, the tax certificate counts and calls `NextStepFromBuildCheck` /
//! `PrevStepFromBuildCheck` when it closes. We populate the same window from `SCConstructHouseTax` (see
//! [`sync_window`]) and answer its `X2House` calls from the received state and the static tables. Build and Cancel
//! therefore run the original Lua, not a rebuilt window.
//!
//! The under-construction house is the interaction point: right-clicking its unit casts the current build step's
//! `housing_build_steps.skill_id` (the server's `CraftEffect` validates it) and raises `HOUSE_INTERACTION_START`,
//! which opens the original material/progress window ([`right_click_build`]).

use aa_net::housing::{ConstructHouseTax, HouseSnapshot};
use aa_net::model::UnitKind;
use aa_net::state::{Player, Target, Units};
use aa_ui::UiRuntime;
use bevy::camera::primitives::Aabb;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use mlua::{Lua, Table, Value};

use super::api::{HudBridge, HudData, Request, Shared};
use super::{HudPointerOverUi, panels::SlotSnap};
use crate::ingame::placement::Placement;
use crate::ingame::units::Views;

/// The 1.2 tax certificate item (`Component::TaxCertificate` / `manager.rs TAX_CERTIFICATE`).
pub const TAX_ITEM: u32 = 31891;
/// The server's `COPPER_PER_TAX_CERTIFICATE` (10,000 copper = 1 gold).
pub const COPPER_PER_CERTIFICATE: i64 = 10_000;
/// The house-specific "Check Construction Info" skill (`skills.category_id = 3`, the housing category, range 6 m).
/// `11001` is the generic construction info skill (category 1, range 4 m); 1.2's housing interaction uses 15212.
pub const INFO_SKILL: u32 = 15212;
/// How near the view ray has to pass a house's middle for a right click, metres (as `targeting::PICK_RADIUS`).
const PICK_RADIUS: f32 = 1.1;
const CLICK_SLOP: f32 = 6.0;

/// One received house as the original getters read it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HouseState {
    pub tl_id: u16,
    pub template_id: u32,
    pub model_id: u32,
    pub all_actions: i32,
    pub current_actions: i32,
    pub owner_id: u32,
    pub name: String,
    pub owner_name: String,
    /// The wire permission (`HousePermission`: private 0, guild 1, public 2, family 3).
    pub permission: u8,
}

impl HouseState {
    pub(super) fn of(snapshot: &HouseSnapshot) -> HouseState {
        HouseState {
            permission: snapshot.permission,
            tl_id: snapshot.tl_id,
            template_id: snapshot.template_id,
            model_id: snapshot.model_id,
            all_actions: snapshot.all_actions,
            current_actions: snapshot.current_actions,
            owner_id: snapshot.owner_id,
            name: snapshot.name.clone(),
            owner_name: snapshot.owner_name.clone(),
        }
    }

    pub fn under_construction(&self) -> bool {
        self.current_actions < self.all_actions
    }
}

/// What `X2House` answers between frames.
#[derive(Clone, Debug, Default)]
pub struct HouseView {
    /// The last `SCConstructHouseTax` quote.
    pub quote: Option<ConstructHouseTax>,
    /// The design the open build window is for.
    pub design: u32,
    /// The house the material window / interaction is about.
    pub selected: Option<HouseState>,
    /// The material window is up for [`Self::material_house`].
    pub material_open: bool,
    pub material_house: Option<u32>,
    pub tax_item: u32,
}

/// The eight arguments `HOUSE_BUILD_INFO` -> `ShowBuildWindow(hType, bTax, hTax, heavyCnt, normalCnt, isHeavy,
/// deposit, taxItemType)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildWindowArgs {
    pub h_type: u32,
    pub base_tax: i32,
    pub total_tax: i32,
    pub heavy_houses: i32,
    pub normal_houses: i32,
    pub is_heavy: bool,
    pub deposit: i32,
    pub tax_item: u32,
}

impl BuildWindowArgs {
    pub fn of(quote: &ConstructHouseTax) -> BuildWindowArgs {
        BuildWindowArgs {
            h_type: quote.design_id,
            base_tax: quote.base_tax_money_amount,
            total_tax: quote.total_tax_money_amount,
            heavy_houses: quote.heavy_tax_house_count,
            normal_houses: quote.normal_tax_house_count,
            is_heavy: quote.is_heavy_tax_house,
            deposit: quote.deposit_tax_money_amount,
            tax_item: TAX_ITEM,
        }
    }
}

/// Fills [`HudData::house`] from the received resources before the scripts read it.
pub fn refresh(world: &mut World, data: &mut HudData) {
    data.house.tax_item = TAX_ITEM;
    data.house.quote = world.get_resource::<aa_net::Houses>().and_then(|h| h.construct_quote.clone());
    // keep the selected house while the server has not answered a target change (`Target.obj_id` is None)
    if let Some(obj) = world.get_resource::<Target>().and_then(|t| t.obj_id) {
        data.house.selected = (|| {
            let unit = world.get_resource::<Units>()?.by_id.get(&obj)?;
            let UnitKind::Housing { tl_id, .. } = unit.kind else { return None };
            world.get_resource::<aa_net::Houses>()?.by_tl_id.get(&tl_id).map(|h| HouseState::of(&h.snapshot))
        })();
    }
}

/// `SCHouseBuildProgress` while the original material window is up: refresh [`HouseView::selected`] and let
/// `housing_manager.alb` redraw its progress and next-material icons (`HOUSE_STEP_INFO_UPDATED`).
pub fn route_build_progress(
    mut events: MessageReader<aa_net::NetEvent>,
    bridge: NonSend<HudBridge>,
    houses: Option<Res<aa_net::Houses>>,
    rt: Option<NonSend<UiRuntime>>,
) {
    let Some(rt) = rt else { return };
    let Some(houses) = houses else { return };
    let mut updated = false;
    for aa_net::NetEvent(event) in events.read() {
        let aa_net::Event::Housing(aa_net::housing::HousingEvent::BuildProgress { tl_id, .. }) = event else {
            continue;
        };
        let mut d = bridge.0.borrow_mut();
        if !d.house.material_open || d.house.selected.as_ref().is_none_or(|h| h.tl_id != *tl_id) {
            continue;
        }
        if let Some(house) = houses.by_tl_id.get(tl_id) {
            d.house.selected = Some(HouseState::of(&house.snapshot));
            updated = true;
        }
    }
    if updated {
        rt.fire_event("HOUSE_STEP_INFO_UPDATED", ());
    }
}

/// The original `X2House` methods the housing windows call, over the static tables and [`HouseView`].
pub fn install(lua: &Lua, bridge: &Shared) -> mlua::Result<()> {
    let x2: Table = lua.globals().get("X2House")?;

    let b = bridge.clone();
    x2.set(
        "GetHousingModelName",
        lua.create_function(move |_, (_this, h_type): (Table, u32)| {
            Ok(b.borrow().game.get().and_then(|g| g.housing.housings.get(&h_type)).map(|h| h.name.clone()))
        })?,
    )?;

    let b = bridge.clone();
    x2.set(
        "GetHousingModelConstructionInfo",
        lua.create_function(move |lua, (_this, h_type): (Table, u32)| {
            let game = b.borrow().game.clone();
            match game.get() {
                Some(g) => steps_table(lua, g, h_type),
                None => lua.create_table(),
            }
        })?,
    )?;

    let b = bridge.clone();
    x2.set(
        "GetHouseConstructionStepInfo",
        lua.create_function(move |lua, _this: Table| {
            let d = b.borrow();
            let Some(selected) = d.house.selected.as_ref() else { return lua.create_table() };
            let game = d.game.clone();
            match game.get() {
                Some(g) => steps_table(lua, g, selected.template_id),
                None => lua.create_table(),
            }
        })?,
    )?;

    let b = bridge.clone();
    x2.set(
        "CountTaxItemForTax",
        lua.create_function(move |_, (_this, value): (Table, Value)| {
            let amount = match value {
                Value::Integer(n) => n,
                Value::Number(n) => n as i64,
                Value::String(s) => s.to_string_lossy().parse::<i64>().unwrap_or(0),
                _ => 0,
            };
            let unit = b
                .borrow()
                .game
                .get()
                .and_then(|g| g.items.get(&TAX_ITEM))
                .map_or(COPPER_PER_CERTIFICATE, |i| i.price.max(1));
            Ok((amount.max(0) + unit - 1) / unit)
        })?,
    )?;

    let b = bridge.clone();
    x2.set(
        "CountTaxItemInBag",
        lua.create_function(move |_, _this: Table| {
            let d = b.borrow();
            Ok(bag_count(&d.panels.bag, TAX_ITEM))
        })?,
    )?;

    let b = bridge.clone();
    x2.set(
        "NextStepFromBuildCheck",
        lua.create_function(move |_, _this: Table| {
            info!("housing: Build pressed in the original build window");
            b.borrow_mut().requests.push(Request::HouseBuild);
            Ok(())
        })?,
    )?;
    let b = bridge.clone();
    x2.set(
        "PrevStepFromBuildCheck",
        lua.create_function(move |_, _this: Table| {
            info!("housing: Cancel pressed in the original build window");
            b.borrow_mut().requests.push(Request::HouseCancel);
            Ok(())
        })?,
    )?;

    let b = bridge.clone();
    x2.set(
        "GetHouseName",
        lua.create_function(move |_, _this: Table| Ok(b.borrow().house.selected.as_ref().map_or_else(String::new, |h| h.name.clone())))?,
    )?;
    let b = bridge.clone();
    x2.set(
        "GetHouseOwnerName",
        lua.create_function(move |_, _this: Table| Ok(b.borrow().house.selected.as_ref().map_or_else(String::new, |h| h.owner_name.clone())))?,
    )?;
    let b = bridge.clone();
    x2.set(
        "GetHouseZoneName",
        lua.create_function(move |_, _this: Table| Ok(b.borrow().snapshot.zone_key_name.clone()))?,
    )?;
    x2.set("IsCastle", lua.create_function(move |_, _this: Table| Ok(false))?)?;
    let b = bridge.clone();
    x2.set(
        "IsHouseUnderConstruction",
        lua.create_function(move |_, _this: Table| Ok(b.borrow().house.selected.as_ref().is_some_and(HouseState::under_construction)))?,
    )?;
    let b = bridge.clone();
    x2.set(
        "IsMyHouse",
        lua.create_function(move |_, _this: Table| {
            let d = b.borrow();
            Ok(d.house.selected.as_ref().is_some_and(|h| h.owner_id != 0 && h.owner_id == d.character_id))
        })?,
    )?;
    let b = bridge.clone();
    x2.set(
        "GetHousePermission",
        lua.create_function(move |_, _this: Table| {
            let d = b.borrow();
            let Some(h) = d.house.selected.as_ref() else { return Ok((1, false)) };
            let always_public = d.game.get().and_then(|g| g.housing.housings.get(&h.template_id)).is_some_and(|t| t.always_public);
            // the scripts index their permission tables from 1; the wire value starts at 0 (private)
            Ok((super::doodad::lua_permission(h.permission), always_public))
        })?,
    )?;
    let b = bridge.clone();
    x2.set(
        "GetHouseBaseProgress",
        lua.create_function(move |_, _this: Table| Ok(b.borrow().house.selected.as_ref().map_or(0, |h| h.all_actions)))?,
    )?;
    let b = bridge.clone();
    x2.set(
        "GetHouseRemainProgress",
        lua.create_function(move |_, _this: Table| {
            Ok(b.borrow().house.selected.as_ref().map_or(0, |h| (h.all_actions - h.current_actions).max(0)))
        })?,
    )?;
    Ok(())
}

/// The construction rows the original material/build views iterate: one per `housing_build_steps` row with the
/// step's material item (from its skill's `skill_effects`), required actions and labor use.
fn steps_table(lua: &Lua, game: &super::data::GameData, housing_id: u32) -> mlua::Result<Table> {
    let out = lua.create_table()?;
    let Some(house) = game.housing.housings.get(&housing_id) else { return Ok(out) };
    for (i, step) in house.build_steps.iter().enumerate() {
        let row = lua.create_table()?;
        row.set("step", i64::from(step.step.max(0) + 1))?;
        row.set("lookType", i64::from(step.consume_item))?;
        row.set("itemGrade", game.items.get(&step.consume_item).map_or(0, |item| item.fixed_grade.max(0)))?;
        row.set("consumeItemStack", i64::from(step.consume_count.max(0)))?;
        row.set("needActions", i64::from(step.num_actions.max(0)))?;
        row.set("isLaborPower", step.consume_item == 0 && step.labor > 0)?;
        out.raw_set(i + 1, row)?;
    }
    Ok(out)
}

fn bag_count(bag: &[Option<SlotSnap>], template: u32) -> i64 {
    bag.iter()
        .flatten()
        .filter(|slot| slot.id == template)
        .map(|slot| slot.count.max(0))
        .sum()
}

/// Opens the original build window when the tax quote answers the placement's request.
pub fn sync_window(
    mut placement: ResMut<Placement>,
    houses: Option<Res<aa_net::Houses>>,
    bridge: NonSend<HudBridge>,
    rt: Option<NonSend<UiRuntime>>,
) {
    let Some(houses) = houses else { return };
    let Some(session) = placement.session.as_mut() else { return };
    if !session.is_house() || session.window_open || !session.quote_requested {
        return;
    }
    let Some(design) = session.house_design() else { return };
    let Some(quote) = houses.construct_quote.clone().filter(|q| q.design_id == design) else { return };
    let args = BuildWindowArgs::of(&quote);
    session.quote = Some(quote.clone());
    session.window_open = true;
    {
        let mut d = bridge.0.borrow_mut();
        d.house.quote = Some(quote);
        d.house.design = design;
        d.house.tax_item = TAX_ITEM;
    }
    if let Some(rt) = rt {
        info!(
            "housing: build window for design {} (base {}, deposit {}, total {}, heavy {} / normal {})",
            args.h_type, args.base_tax, args.deposit, args.total_tax, args.heavy_houses, args.normal_houses
        );
        rt.fire_event(
            "HOUSE_BUILD_INFO",
            (
                i64::from(args.h_type),
                i64::from(args.base_tax),
                i64::from(args.total_tax),
                i64::from(args.heavy_houses),
                i64::from(args.normal_houses),
                args.is_heavy,
                i64::from(args.deposit),
                i64::from(args.tax_item),
            ),
        );
    }
}

/// Right click on a house: an unfinished house casts its current build-step skill and opens the original
/// material window. The server's `CraftEffect` checks the skill and sends `SCHouseBuildProgress` back.
#[allow(clippy::too_many_arguments)]
pub fn right_click_build(
    mouse: Res<ButtonInput<MouseButton>>,
    over_ui: Res<HudPointerOverUi>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    units: Res<Units>,
    player: Res<Player>,
    houses: Option<Res<aa_net::Houses>>,
    views: Res<Views>,
    bounds: Query<(&Aabb, &GlobalTransform)>,
    children: Query<&Children>,
    bridge: NonSend<HudBridge>,
    rt: Option<NonSend<UiRuntime>>,
    mut net: MessageWriter<aa_net::NetCommand>,
    mut pressed_at: Local<Option<Vec2>>,
) {
    let Ok(window) = windows.single() else { return };
    let cursor = window.cursor_position();
    if mouse.just_pressed(MouseButton::Right) {
        *pressed_at = if over_ui.0 { None } else { cursor };
    }
    if !mouse.just_released(MouseButton::Right) {
        return;
    }
    let (Some(start), Some(end)) = (pressed_at.take(), cursor) else { return };
    if start.distance(end) > CLICK_SLOP || over_ui.0 {
        return;
    }
    let Some((camera, transform)) = cameras.iter().find(|(c, _)| c.is_active) else { return };
    let Ok(ray) = camera.viewport_to_world(transform, end) else { return };
    let own = player.spawn.as_ref().map_or(0, |s| s.obj_id);
    let mut best: Option<(f32, u32)> = None;
    for u in units.iter().filter(|u| u.obj_id != own && matches!(u.kind, UnitKind::Housing { .. })) {
        // the unit's real mesh bounds (a farm is a wide plot, so its centre-only sphere is not enough)
        let mesh_hit = views
            .unit_entity(u.obj_id)
            .and_then(|root| world_bounds(root, &children, &bounds))
            .and_then(|(min, max)| ray_aabb(&ray, min, max));
        let centre = aa_world::cry_to_bevy(Vec3::new(u.pos.x, u.pos.y, u.pos.z + 0.9 * u.scale.max(0.5)));
        let along = (centre - ray.origin).dot(*ray.direction);
        let sphere_hit = (along > 0.0 && (ray.origin + *ray.direction * along).distance(centre) <= PICK_RADIUS * u.scale.max(0.7))
            .then_some(along);
        if let Some(along) = mesh_hit.or(sphere_hit)
            && best.is_none_or(|(a, _)| along < a)
        {
            best = Some((along, u.obj_id));
        }
    }
    let Some((_, obj_id)) = best else {
        debug!("housing: right click picked no house unit");
        return;
    };
    let Some(unit) = units.by_id.get(&obj_id) else { return };
    let UnitKind::Housing { tl_id, .. } = unit.kind else { return };
    let Some(house) = houses.as_deref().and_then(|h| h.by_tl_id.get(&tl_id)) else {
        info!("housing: right click on unit {obj_id} (tl {tl_id}) refused: no received house state");
        return;
    };
    let mut commands = Vec::new();
    match start_construction(unit, house, &bridge.0, rt.as_deref(), &mut commands) {
        Ok(skill) => {
            info!("housing: right click on unit {obj_id} (tl {tl_id}) opened the material window and cast build skill {skill}");
            for command in commands {
                net.write(aa_net::NetCommand(command));
            }
        }
        Err(reason) => info!("housing: right click on unit {obj_id} (tl {tl_id}) refused: {reason}"),
    }
}

/// The client-side action of the original house interaction: select the house, ask for its material window and
/// cast the current construction step's skill. Returns the skill on success or the reason it was refused.
fn start_construction(
    unit: &aa_net::Unit,
    house: &aa_net::housing::House,
    bridge: &Shared,
    rt: Option<&UiRuntime>,
    out: &mut Vec<aa_net::Command>,
) -> Result<u32, &'static str> {
    let UnitKind::Housing { template_id, .. } = unit.kind else {
        return Err("the picked unit is not a house");
    };
    if !(house.snapshot.current_actions < house.snapshot.all_actions) {
        return Err("the house is already finished");
    }
    let state = HouseState::of(&house.snapshot);
    let skill = {
        let d = bridge.borrow();
        let game = d.game.get().ok_or("the game data is not loaded")?;
        step_skill(game, template_id, state.current_actions).ok_or("no build step skill for the current action count")?
    };
    {
        let mut d = bridge.borrow_mut();
        d.house.selected = Some(state);
        d.house.material_open = true;
        d.house.material_house = Some(unit.obj_id);
    }
    out.push(aa_net::Command::Target { obj_id: unit.obj_id });
    out.push(aa_net::Command::UseSkill { skill_id: skill, target_obj_id: unit.obj_id });
    if let Some(rt) = rt {
        open_material_window(rt);
    }
    Ok(skill)
}

/// Raises the original client's house interaction events and shows its material/progress window.
pub(super) fn open_material_window(rt: &UiRuntime) {
    rt.fire_event("HOUSE_INTERACTION_START", "housing");
    // `ShowMaterialWindow(true)` creates the original window but leaves it hidden in the 1.2 script;
    // the retail interaction shows it, so raise the already populated window.
    if let Err(e) = rt.exec("@house_interaction", "if housing.materialWindow then housing.materialWindow:Show(true) end") {
        warn!("housing: cannot show the material window: {e}");
    }
}

/// World-space bounds of a unit's meshes (the root's descendants with an `Aabb`).
fn world_bounds(
    root: Entity,
    children: &Query<&Children>,
    bounds: &Query<(&Aabb, &GlobalTransform)>,
) -> Option<(Vec3, Vec3)> {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut found = false;
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if let Ok((aabb, global)) = bounds.get(entity) {
            found = true;
            let c = Vec3::from(aabb.center);
            let h = Vec3::from(aabb.half_extents);
            for sx in [-1.0, 1.0] {
                for sy in [-1.0, 1.0] {
                    for sz in [-1.0, 1.0] {
                        let p = global.transform_point(c + h * Vec3::new(sx, sy, sz));
                        min = min.min(p);
                        max = max.max(p);
                    }
                }
            }
        }
        if let Ok(kids) = children.get(entity) {
            stack.extend(kids.iter());
        }
    }
    found.then_some((min, max))
}

/// The distance along `ray` to the first world-axis-aligned box hit (`None` behind the camera or on a miss).
fn ray_aabb(ray: &Ray3d, min: Vec3, max: Vec3) -> Option<f32> {
    let d = *ray.direction;
    let (mut tmin, mut tmax) = (0.0f32, f32::INFINITY);
    for i in 0..3 {
        let o = ray.origin[i];
        if d[i].abs() < 1e-6 {
            if o < min[i] || o > max[i] {
                return None;
            }
            continue;
        }
        let t1 = (min[i] - o) / d[i];
        let t2 = (max[i] - o) / d[i];
        tmin = tmin.max(t1.min(t2));
        tmax = tmax.min(t1.max(t2));
        if tmax < tmin {
            return None;
        }
    }
    (tmax > 0.0).then_some(tmin.max(0.0))
}

/// The `housing_build_steps.skill_id` of the step that owns `current_actions`.
pub(super) fn step_skill(game: &super::data::GameData, design: u32, current_actions: i32) -> Option<u32> {
    let template = game.housing.housings.get(&design)?;
    let mut done = 0;
    for step in &template.build_steps {
        done += step.num_actions.max(0);
        if current_actions < done {
            return Some(step.skill_id);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn with_house(state: HouseState) -> Shared {
        let mut data = HudData::default();
        data.house.selected = Some(state);
        Rc::new(RefCell::new(data))
    }

    fn house() -> aa_net::housing::House {
        aa_net::housing::House {
            snapshot: HouseSnapshot {
                tl_id: 13,
                db_id: 1,
                obj_id: 16211,
                template_id: 267,
                model_id: 989,
                co_owner_id: 0,
                owner_id: 7,
                owner_name: "Farmax".into(),
                account_id: 1,
                permission: 0,
                all_actions: 1,
                current_actions: 0,
                tax: 0,
                x: aa_net::wire::coord_to_long(10508.29),
                y: aa_net::wire::coord_to_long(15517.18),
                z: 224.92,
                name: "Scarecrow Garden".into(),
                allow_recover: true,
                sell_price: 0,
                sell_to_player_id: 0,
                sell_to_name: String::new(),
            },
            demolished: false,
            build_pay: None,
            tax: None,
            sale: None,
        }
    }

    fn unit() -> aa_net::Unit {
        aa_net::Unit {
            obj_id: 16211,
            name: "Scarecrow Garden".into(),
            kind: UnitKind::Housing { tl_id: 13, template_id: 267, build_step: 0 },
            owner_name: "Farmax".into(),
            pos: aa_net::Pos::new(10508.29, 15517.18, 224.92),
            rotation: [0.0; 3],
            scale: 1.0,
            level: 1,
            model_id: 989,
            equipment: default(),
            model_params: default(),
            hp: 200,
            mp: 0,
            max_hp: 200,
            max_mp: 0,
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
            skills: vec![],
            passive_buffs: vec![],
            ability_exp: vec![],
            active_abilities: vec![],
            buffs: vec![],
            anim_action: 0,
            attached_to: None,
            house_posture: None,
            state_attachments: None,
        }
    }

    #[test]
    fn build_step_item_grade_comes_from_the_material_item() {
        let lua = Lua::new();
        let mut game = super::super::data::GameData::default();
        game.items.insert(8337, Default::default());
        game.items.get_mut(&8337).unwrap().fixed_grade = 3;
        game.housing.housings.insert(267, Default::default());
        let template = game.housing.housings.get_mut(&267).unwrap();
        template.build_steps.push(Default::default());
        template.build_steps[0].skill_id = 18553;
        template.build_steps[0].consume_item = 8337;
        let table = steps_table(&lua, &game, 267).unwrap();
        let row: Table = table.get(1).unwrap();
        assert_eq!(row.get::<i64>("itemGrade").unwrap(), 3);
    }

    #[test]
    fn the_build_window_arguments_are_the_quote_the_server_sent() {
        let quote = ConstructHouseTax {
            design_id: 267,
            heavy_tax_house_count: 1,
            normal_tax_house_count: 0,
            is_heavy_tax_house: true,
            base_tax_money_amount: 50000,
            deposit_tax_money_amount: 100000,
            total_tax_money_amount: 150000,
        };
        let args = BuildWindowArgs::of(&quote);
        assert_eq!(args.h_type, 267);
        assert_eq!((args.base_tax, args.deposit, args.total_tax), (50000, 100000, 150000));
        assert_eq!((args.heavy_houses, args.normal_houses, args.is_heavy), (1, 0, true));
        assert_eq!(args.tax_item, TAX_ITEM);
    }

    #[test]
    fn tax_certificate_count_is_ceiled_to_whole_items() {
        let count = |amount: i64| (amount.max(0) + COPPER_PER_CERTIFICATE - 1) / COPPER_PER_CERTIFICATE;
        assert_eq!(count(50000), 5);
        assert_eq!(count(100000), 10);
        assert_eq!(count(150000), 15);
        assert_eq!(count(1), 1);
        assert_eq!(count(0), 0);
    }

    #[test]
    fn the_bag_count_sums_the_tax_certificates() {
        let slot = |id: u32, count: i64| SlotSnap { id, count, ..Default::default() };
        let bag = vec![Some(slot(TAX_ITEM, 4)), Some(slot(1, 9)), None, Some(slot(TAX_ITEM, 6))];
        assert_eq!(bag_count(&bag, TAX_ITEM), 10);
    }

    #[test]
    fn only_an_unfinished_house_is_under_construction() {
        let mut h = HouseState { all_actions: 1, current_actions: 0, ..Default::default() };
        assert!(h.under_construction());
        h.current_actions = 1;
        assert!(!h.under_construction());
    }

    #[test]
    fn the_shared_house_view_holds_the_selected_snapshot() {
        let house = with_house(HouseState { tl_id: 7, name: "Scarecrow Garden".into(), ..Default::default() });
        assert_eq!(house.borrow().house.selected.as_ref().map(|h| h.name.as_str()), Some("Scarecrow Garden"));
    }

    #[test]
    fn a_right_click_on_an_unfinished_house_opens_the_material_window_and_casts_the_build_skill() {
        let Ok((rt, bridge, path)) = super::super::breath::fixture::runtime(1280.0, 720.0) else {
            return;
        };
        let (unit, house) = (unit(), house());
        let mut commands = Vec::new();
        let skill = start_construction(&unit, &house, &bridge, Some(&rt), &mut commands).expect("construction starts");
        assert_eq!(skill, 18553);
        assert!(matches!(
            commands.as_slice(),
            [aa_net::Command::Target { obj_id: 16211 }, aa_net::Command::UseSkill { skill_id: 18553, target_obj_id: 16211 }]
        ), "{commands:?}");
        assert!(rt.lua.load("return housing.materialWindow ~= nil and housing.materialWindow:IsVisible()").eval::<bool>().unwrap());
        assert_eq!(
            bridge.borrow().house.selected.as_ref().map(|h| (h.tl_id, h.current_actions, h.all_actions)),
            Some((13, 0, 1))
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn a_ray_hits_a_wide_house_box_not_only_its_centre() {
        let ray = Ray3d::new(Vec3::new(0.0, 2.0, 10.0), Dir3::NEG_Z);
        let along = ray_aabb(&ray, Vec3::new(-4.0, 0.0, -5.0), Vec3::new(4.0, 3.0, 5.0)).expect("the wall is hit");
        assert!((along - 5.0).abs() < 1e-4, "{along}");
        assert!(ray_aabb(&Ray3d::new(Vec3::new(5.0, 2.0, 10.0), Dir3::NEG_Z), Vec3::new(-4.0, 0.0, -5.0), Vec3::new(4.0, 3.0, 5.0)).is_none());
    }

    #[test]
    fn the_original_builder_end_hides_the_build_window() {
        let Ok((rt, bridge, path)) = super::super::breath::fixture::runtime(1280.0, 720.0) else {
            return;
        };
        {
            let mut d = bridge.borrow_mut();
            d.house.quote = Some(ConstructHouseTax {
                design_id: 267,
                heavy_tax_house_count: 1,
                normal_tax_house_count: 0,
                is_heavy_tax_house: true,
                base_tax_money_amount: 50000,
                deposit_tax_money_amount: 100000,
                total_tax_money_amount: 150000,
            });
            d.house.design = 267;
        }
        rt.lua
            .load("ShowBuildWindow(267, 50000, 150000, 1, 0, true, 100000, 31891)")
            .exec()
            .expect("the original build window opens");
        assert!(
            rt.lua.load("return housing.buildWindow ~= nil and housing.buildWindow:IsVisible()").eval::<bool>().unwrap(),
            "the Start Construction window is open before the server answer"
        );
        rt.fire_event("BUILDER_END", ());
        assert!(
            rt.lua
                .load("return housing.buildWindow == nil or not housing.buildWindow:IsVisible()")
                .eval::<bool>()
                .unwrap(),
            "BUILDER_END hides the original Start Construction window"
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn get_house_permission_answers_the_script_index_and_the_always_public_flag() {
        let lua = Lua::new();
        lua.globals().set("X2House", lua.create_table().unwrap()).unwrap();
        let bridge = with_house(HouseState { tl_id: 7, template_id: 267, permission: 0, ..Default::default() });
        install(&lua, &bridge).unwrap();
        let (index, always): (i64, bool) = lua.load("return X2House:GetHousePermission()").eval().unwrap();
        assert_eq!((index, always), (1, false), "private is index 1");
        bridge.borrow_mut().house.selected.as_mut().unwrap().permission = 3;
        let (index, _): (i64, bool) = lua.load("return X2House:GetHousePermission()").eval().unwrap();
        assert_eq!(index, 4, "family is index 4");
        // a house the template keeps public
        let mut game = super::super::data::GameData::default();
        game.housing.housings.insert(267, Default::default());
        game.housing.housings.get_mut(&267).unwrap().always_public = true;
        bridge.borrow().game.0.set(game).ok().expect("one load");
        let (_, always): (i64, bool) = lua.load("return X2House:GetHousePermission()").eval().unwrap();
        assert!(always);
    }

    #[test]
    fn the_original_x2house_calls_are_answered() {
        let lua = Lua::new();
        let x2 = lua.create_table().unwrap();
        lua.globals().set("X2House", x2).unwrap();
        let bridge = Rc::new(RefCell::new(HudData::default()));
        bridge.borrow_mut().house.selected = Some(HouseState {
            tl_id: 7,
            template_id: 267,
            all_actions: 1,
            current_actions: 0,
            owner_id: 9,
            name: "Scarecrow Garden".into(),
            owner_name: "Owner".into(),
            ..Default::default()
        });
        bridge.borrow_mut().character_id = 9;
        bridge.borrow_mut().panels.bag = vec![Some(SlotSnap { id: TAX_ITEM, count: 7, ..Default::default() })];
        install(&lua, &bridge).unwrap();
        let out: (i64, i64, i64, bool, bool, bool) = lua
            .load(
                "return X2House:CountTaxItemForTax(150000), X2House:CountTaxItemInBag(), X2House:GetHouseRemainProgress(),\
                 X2House:IsHouseUnderConstruction(), X2House:IsMyHouse(), X2House:IsCastle()",
            )
            .eval()
            .unwrap();
        assert_eq!(out, (15, 7, 1, true, true, false));
        let name: String = lua.load("return X2House:GetHouseName()").eval().unwrap();
        assert_eq!(name, "Scarecrow Garden");
        lua.load("X2House:NextStepFromBuildCheck()").exec().unwrap();
        lua.load("X2House:PrevStepFromBuildCheck()").exec().unwrap();
        assert_eq!(bridge.borrow().requests, vec![Request::HouseBuild, Request::HouseCancel]);
    }
}
