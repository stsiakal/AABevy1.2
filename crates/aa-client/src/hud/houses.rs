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
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use mlua::{Lua, Table, Value};

use super::api::{HudBridge, HudData, Request, Shared};
use super::{HudPointerOverUi, panels::SlotSnap};
use crate::ingame::placement::Placement;

/// The 1.2 tax certificate item (`Component::TaxCertificate` / `manager.rs TAX_CERTIFICATE`).
pub const TAX_ITEM: u32 = 31891;
/// The server's `COPPER_PER_TAX_CERTIFICATE` (10,000 copper = 1 gold).
pub const COPPER_PER_CERTIFICATE: i64 = 10_000;
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
}

impl HouseState {
    fn of(snapshot: &HouseSnapshot) -> HouseState {
        HouseState {
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
        row.set("itemGrade", 1i64)?;
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
        let centre = aa_world::cry_to_bevy(Vec3::new(u.pos.x, u.pos.y, u.pos.z + 0.9 * u.scale.max(0.5)));
        let along = (centre - ray.origin).dot(*ray.direction);
        if along <= 0.0 {
            continue;
        }
        let miss = (ray.origin + *ray.direction * along).distance(centre);
        if miss <= PICK_RADIUS * u.scale.max(0.7) && best.is_none_or(|(a, _)| along < a) {
            best = Some((along, u.obj_id));
        }
    }
    let Some((_, obj_id)) = best else { return };
    let Some(unit) = units.by_id.get(&obj_id) else { return };
    let UnitKind::Housing { tl_id, template_id, .. } = unit.kind else { return };
    let Some(house) = houses.as_deref().and_then(|h| h.by_tl_id.get(&tl_id)) else { return };
    if !(house.snapshot.current_actions < house.snapshot.all_actions) {
        return;
    }
    let state = HouseState::of(&house.snapshot);
    let skill = {
        let d = bridge.0.borrow();
        let Some(game) = d.game.get() else { return };
        step_skill(game, template_id, state.current_actions)
    };
    let Some(skill) = skill else { return };
    {
        let mut d = bridge.0.borrow_mut();
        d.house.selected = Some(state);
    }
    info!("housing: right click on unit {obj_id} (tl {tl_id}): build step skill {skill}");
    net.write(aa_net::NetCommand(aa_net::Command::Target { obj_id }));
    net.write(aa_net::NetCommand(aa_net::Command::UseSkill { skill_id: skill, target_obj_id: obj_id }));
    if let Some(rt) = rt {
        rt.fire_event("HOUSE_INTERACTION_START", "housing");
    }
}

/// The `housing_build_steps.skill_id` of the step that owns `current_actions`.
fn step_skill(game: &super::data::GameData, design: u32, current_actions: i32) -> Option<u32> {
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
