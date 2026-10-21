//! Using doodads (a Memory Tome, a tree, a navi portrait ...): the generic "doodad func -> use" path.
//!
//! The client knows from the game database what each phase (`doodad_func_groups.id`, the doodad's current
//! `func_group_id`) of a doodad can do: its `doodad_funcs` rows. A func with a `func_skill_id` is used by casting that
//! skill at the doodad (`CSStartSkill` with a doodad target, [`aa_net::Command::UseDoodadSkill`]); the server's
//! `InteractionEffect` of the skill then runs `Doodad.Use(caster, skill)`, which finds the func of the current phase
//! with that skill and executes it (a Memory Tome's `DoodadFuncBinding` sets the recall point and answers
//! `SCCharacterReturnDistricts`, then the phase changes: `SCDoodadPhaseChanged`). The navi funcs have no skill and
//! their own packets (`CSNaviTeleport`, `CSNaviOpenPortal`).
//!
//! What the player sees is the original's:
//!
//! * the **dynamic action bar** (`dynamic_action_bar.lua`): while a usable doodad is in reach, its skills as 48 px
//!   buttons in the lower middle of the screen with the key under each (F, G, H, J: `do_interaction_1..4`).
//!   `DYNAMIC_ACTION_BAR_SHOW("doodad", id)` / `DYNAMIC_ACTION_BAR_HIDE`; the scripts ask `X2:GetDynamicActionCount`,
//!   `GetDynamicActionSkill(i)`, `IsDynamicActionSkillToggled(i)`, `X2Interaction:GetInteractionSkillIconPath(skill)`,
//!   `IsValidIconPath(path)`; a click is `X2:ExecuteDynamicAction(i)`. The key does the same.
//! * a **right click** on a doodad uses it: one use at once, several (a navi sign) through the interaction list of
//!   `interaction.lua` (`INTERACTION_START`, `X2Interaction:GetInteractionSkillCount/Id/Name`, `SelectInteraction`,
//!   `CancelInteraction`).
//! * **hovering** a usable doodad shows its name (`DRAW_DOODAD_TOOLTIP({name})`).
//!
//! The cast bar, the cast animation and the result come from the server's answers like for any skill (`cast.rs`,
//! `ingame::combat`, `portal.rs` for the new recall point).
//!
//! F goes to the doodad only when no loot is in reach (F loots) and no NPC is nearer than it (F talks).
//!
//! Dev flags: `--doodad-key N T` presses `do_interaction_N` T seconds after the HUD is up; `--doodad-rclick T` right
//! clicks the nearest usable doodad (as if the pointer were on it).

use std::collections::HashMap;

use aa_net::interaction::Interact;
use aa_net::state::{Doodads, Houses, Units};
use aa_net::{Command, NetCommand};
use aa_ui::UiRuntime;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use mlua::{Lua, Table, Value};
use rusqlite::Connection;

use super::api::{HudBridge, HudData, Shared};
use super::houses::{HouseState, INFO_SKILL, step_skill};
use super::{HudKeyboardFocus, HudPointerOverUi};

/// Skills the dynamic action bar leaves out (`dynamic_action_bar.lua`, `ShoulPassShow`).
const HIDDEN_SKILLS: [u32; 2] = [0, 10994];
/// Keys of the dynamic action bar (`do_interaction_1` ... ) that use a doodad.
const KEYS: usize = 4;
/// Reach of a doodad use without a skill range, and the slack on top of a skill's `max_range` (the range is to the
/// doodad's origin, a big model is touched from further away), metres.
const BASE_REACH: f32 = 4.0;
const REACH_SLACK: f32 = 1.0;
/// How much further away a doodad right behind the player may be than one in front of it and still be preferred, metres.
const BEHIND: f32 = 3.0;
/// How far a right click or the pointer picks doodads, metres.
const PICK_DISTANCE: f32 = 40.0;
/// A press that moves more than this many pixels before it is released was a camera drag.
const CLICK_SLOP: f32 = 6.0;
/// The ids of the navi uses in the interaction list (skill ids are below this).
const NAVI_ID: i64 = 1 << 32;
/// Interaction-list ids of `DoodadFuncPurchase` rows: `COIN_ID + doodad_funcs.id`.
const COIN_ID: i64 = 1 << 33;

/// A use without a skill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Navi {
    Teleport,
    OpenPortal,
}

impl Navi {
    fn label(self) -> &'static str {
        match self {
            Navi::Teleport => "Teleport",
            Navi::OpenPortal => "Open Worldgate",
        }
    }
}

/// What a doodad phase can be used for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Uses {
    /// The skills of its funcs, in func order, without repeats.
    pub skills: Vec<u32>,
    pub navi: Vec<Navi>,
    /// `DoodadFuncPurchase` ids (`doodad_funcs.id`), in func order.
    pub purchases: Vec<u32>,
}

/// One `DoodadFuncPurchase`: the item it sells and the coin item it spends.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Purchase {
    pub func_id: u32,
    pub item_id: u32,
    pub count: i32,
    pub coin_item_id: u32,
    pub coin_count: i32,
    pub currency_id: u32,
}

#[derive(Default)]
pub(super) struct Tables {
    /// `doodad_func_groups.id` -> its uses.
    uses: HashMap<u32, Uses>,
    /// `doodad_funcs.id` -> its purchase details.
    purchases: HashMap<u32, Purchase>,
    /// `doodad_almighties.id` -> English name.
    names: HashMap<u32, String>,
}

impl Tables {
    pub(super) fn load(conn: &Connection) -> rusqlite::Result<Self> {
        let mut t = Tables::default();
        let mut q = conn.prepare("SELECT id, doodad_func_group_id, func_skill_id, actual_func_type FROM doodad_funcs ORDER BY doodad_func_group_id, id")?;
        let mut rows = q.query([])?;
        while let Some(r) = rows.next()? {
            let id = super::data::any_int(r, 0)? as u32;
            let group = super::data::any_int(r, 1)? as u32;
            let skill = super::data::any_int(r, 2)? as u32;
            let kind: Option<String> = r.get(3)?;
            t.add(id, group, skill, kind.as_deref().unwrap_or(""));
        }
        let mut q = conn.prepare(
            "SELECT f.id,f.doodad_func_group_id,p.item_id,p.count,p.coin_item_id,p.coin_count,p.currency_id \
             FROM doodad_funcs f JOIN doodad_func_purchases p ON p.id = f.actual_func_id \
             WHERE f.actual_func_type = 'DoodadFuncPurchase' ORDER BY f.doodad_func_group_id,f.id",
        )?;
        let mut rows = q.query([])?;
        while let Some(r) = rows.next()? {
            let purchase = Purchase {
                func_id: super::data::any_int(r, 0)? as u32,
                item_id: super::data::any_int(r, 2)? as u32,
                count: super::data::any_int(r, 3)? as i32,
                coin_item_id: super::data::any_int(r, 4)? as u32,
                coin_count: super::data::any_int(r, 5)? as i32,
                currency_id: super::data::any_int(r, 6)? as u32,
            };
            t.purchases.insert(purchase.func_id, purchase.clone());
            t.uses.entry(super::data::any_int(r, 1)? as u32).or_default().purchases.push(purchase.func_id);
        }
        let mut q = conn.prepare("SELECT idx, en_us FROM localized_texts WHERE tbl_name = 'doodad_almighties' AND tbl_column_name = 'name' AND en_us <> ''")?;
        let mut rows = q.query([])?;
        while let Some(r) = rows.next()? {
            t.names.insert(super::data::any_int(r, 0)? as u32, r.get(1)?);
        }
        Ok(t)
    }

    fn add(&mut self, _func_id: u32, group: u32, skill: u32, kind: &str) {
        // A purchase has its own coin-shop dialog and must not be cast as a skill.
        if kind == "DoodadFuncPurchase" {
            return;
        }
        let navi = match kind {
            "DoodadFuncNaviTeleport" => Some(Navi::Teleport),
            "DoodadFuncNaviOpenPortal" => Some(Navi::OpenPortal),
            _ => None,
        };
        if skill == 0 && navi.is_none() {
            return;
        }
        let uses = self.uses.entry(group).or_default();
        if skill != 0 {
            if !HIDDEN_SKILLS.contains(&skill) && !uses.skills.contains(&skill) {
                uses.skills.push(skill);
            }
        } else if let Some(n) = navi
            && !uses.navi.contains(&n)
        {
            uses.navi.push(n);
        }
    }

    /// What the phase `group` can be used for (`None`: nothing).
    pub(super) fn uses(&self, group: u32) -> Option<&Uses> {
        self.uses
            .get(&group)
            .filter(|u| !u.skills.is_empty() || !u.navi.is_empty() || !u.purchases.is_empty())
    }

    pub(super) fn purchase(&self, func_id: u32) -> Option<&Purchase> {
        self.purchases.get(&func_id)
    }

    pub(super) fn name(&self, template_id: u32) -> Option<&str> {
        self.names.get(&template_id).map(String::as_str)
    }
}

/// What a ray or the focus selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pick {
    Doodad(u32),
    House(u32),
}

/// The usable doodad or unfinished house in reach.
#[derive(Clone, Debug, PartialEq)]
struct Focus {
    obj_id: u32,
    skills: Vec<u32>,
    dist: f32,
    /// `Some` for an unfinished house: the bar's first skill builds it, the second opens the info window.
    house: Option<HouseState>,
}

/// One row of the interaction list.
#[derive(Clone, Debug, PartialEq)]
struct Entry {
    id: i64,
    name: String,
}

/// The coin shop the original `DLG_TASK_PURCHASE_COIN` dialog is confirming.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct CoinShop {
    pub doodad_obj_id: u32,
    pub purchase: Purchase,
}

#[derive(Default)]
pub(super) struct View {
    focus: Option<Focus>,
    /// The doodad/house and skills the bar was last shown for.
    shown: Option<(u32, Vec<u32>, bool)>,
    /// F goes to the doodad (it is nearer than any NPC).
    claims_key: bool,
    /// Dynamic action indices (1 based) the bar or a key asked for.
    execute: Vec<usize>,
    /// The interaction list: the doodad and its rows.
    menu: Option<(u32, Vec<Entry>)>,
    /// The ids picked from the list.
    chosen: Vec<i64>,
    /// The doodad or house under the pointer (its tooltip is up).
    hover: Option<Pick>,
    /// The doodad last right clicked: the bar is about it while it is in reach.
    preferred: Option<u32>,
    /// The open coin purchase and its original dialog request.
    pub(super) coin: Option<CoinShop>,
    coin_open: bool,
    /// Confirmed purchases: `(doodad, doodad_funcs.id)`.
    coin_buys: Vec<(u32, u32)>,
}

impl View {
    /// F (`do_interaction_1`) uses the doodad in reach instead of talking to an NPC.
    pub(super) fn claims_key(&self) -> bool {
        self.claims_key
    }
}

/// The reach of a doodad whose phase has `uses`, metres.
fn reach(data: &HudData, uses: &Uses) -> f32 {
    let range = data.game.get().and_then(|g| uses.skills.first().and_then(|s| g.skills.get(s))).map_or(0.0, |s| s.max_range as f32);
    if range > 0.0 { range.max(2.0) + REACH_SLACK } else { BASE_REACH }
}

fn to_vec(p: aa_net::Pos) -> Vec3 {
    Vec3::new(p.x, p.y, p.z)
}

/// The doodad in reach whose phase has a skill to use that the bar is about: the one last right clicked, else the
/// nearest, a doodad behind the player counting up to [`BEHIND`] metres further away than one in front (an inn's
/// chairs around the Memory Tome).
fn find_focus(data: &HudData, doodads: &Doodads, units: &Units, houses: &Houses) -> Option<Focus> {
    let game = data.game.get()?;
    let forward = Vec2::new(-data.yaw.sin(), data.yaw.cos());
    let doodad_candidates = doodads.by_id.values().filter_map(|o| {
        let uses = game.doodad.uses(o.func_group_id)?;
        if uses.skills.is_empty() {
            return None;
        }
        let dist = data.pos.distance(to_vec(o.pos));
        if dist > reach(data, uses) {
            return None;
        }
        let toward = (to_vec(o.pos) - data.pos).truncate().normalize_or_zero();
        let score = if data.doodad.preferred == Some(o.obj_id) { f32::MIN } else { dist + BEHIND * 0.5 * (1.0 - forward.dot(toward)) };
        Some((score, Focus { obj_id: o.obj_id, skills: uses.skills.clone(), dist, house: None }))
    });
    let house_candidates = units.iter().filter_map(|u| {
        let aa_net::model::UnitKind::Housing { tl_id, .. } = u.kind else { return None };
        let state = HouseState::of(&houses.by_tl_id.get(&tl_id)?.snapshot);
        if !state.under_construction() {
            return None;
        }
        let skill = step_skill(game, state.template_id, state.current_actions)?;
        let range = game.skills.get(&skill).map_or(0.0, |s| s.max_range as f32);
        let dist = data.pos.distance(to_vec(u.pos));
        if dist > if range > 0.0 { range + REACH_SLACK } else { BASE_REACH } {
            return None;
        }
        let toward = (to_vec(u.pos) - data.pos).truncate().normalize_or_zero();
        let score = dist + BEHIND * 0.5 * (1.0 - forward.dot(toward));
        Some((score, Focus { obj_id: u.obj_id, skills: vec![skill, INFO_SKILL], dist, house: Some(state) }))
    });
    doodad_candidates.chain(house_candidates).min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, f)| f)
}

pub(super) fn install(lua: &Lua, bridge: &Shared) -> mlua::Result<()> {
    let hud: Table = lua.globals().get("HUD")?;
    let state = bridge.clone();
    hud.set(
        "coin_shop",
        lua.create_function(move |lua, ()| {
            let d = state.borrow();
            let Some(shop) = d.doodad.coin.as_ref() else { return Ok(None) };
            let Some(game) = d.game.get() else { return Ok(None) };
            let Some(info) = super::tips::item_tip(
                lua,
                game,
                &d.tips,
                shop.purchase.item_id,
                None,
                Some(i64::from(shop.purchase.count)),
                Default::default(),
            )? else {
                return Ok(None);
            };
            let out = lua.create_table()?;
            out.set("itemInfo", info)?;
            out.set("itemCount", shop.purchase.count)?;
            out.set("coinName", game.items.get(&shop.purchase.coin_item_id).map(|i| i.named.name.clone()).unwrap_or_default())?;
            out.set("coinCount", shop.purchase.coin_count)?;
            out.set("objId", shop.doodad_obj_id)?;
            out.set("funcId", shop.purchase.func_id)?;
            Ok(Some(out))
        })?,
    )?;
    let state = bridge.clone();
    hud.set(
        "coin_buy",
        lua.create_function(move |_, ()| {
            let mut d = state.borrow_mut();
            if let Some(shop) = d.doodad.coin.take() {
                d.doodad.coin_buys.push((shop.doodad_obj_id, shop.purchase.func_id));
            }
            Ok(())
        })?,
    )?;
    let state = bridge.clone();
    hud.set(
        "coin_close",
        lua.create_function(move |_, ()| {
            state.borrow_mut().doodad.coin = None;
            Ok(())
        })?,
    )?;
    lua.load(include_str!("doodad_coin.lua")).set_name("@doodad_coin.lua").exec()?;

    let x2: Table = lua.globals().get("X2")?;
    let state = bridge.clone();
    x2.set("GetDynamicActionCount", lua.create_function(move |_, _: Value| Ok(state.borrow().doodad.focus.as_ref().map_or(0, |f| f.skills.len())))?)?;
    let state = bridge.clone();
    x2.set(
        "GetDynamicActionSkill",
        lua.create_function(move |_, (_, i): (Value, usize)| Ok(state.borrow().doodad.focus.as_ref().and_then(|f| f.skills.get(i.wrapping_sub(1)).copied())))?,
    )?;
    x2.set("IsDynamicActionSkillToggled", lua.create_function(|_, _: mlua::MultiValue| Ok(false))?)?;
    let state = bridge.clone();
    x2.set(
        "ExecuteDynamicAction",
        lua.create_function(move |_, (_, i): (Value, usize)| {
            state.borrow_mut().doodad.execute.push(i);
            Ok(())
        })?,
    )?;

    let interaction: Table = lua.globals().get("X2Interaction")?;
    let state = bridge.clone();
    interaction.set(
        "GetInteractionSkillIconPath",
        lua.create_function(move |_, (_, skill): (Value, u32)| {
            let d = state.borrow();
            Ok(d.game.get().and_then(|g| g.skills.get(&skill)).map(|s| s.named.icon.clone()).filter(|p| !p.is_empty()).unwrap_or_else(|| "ui/icon/icon_skill_none.dds".into()))
        })?,
    )?;
    interaction.set("IsValidIconPath", lua.create_function(|_, (_, path): (Value, String)| Ok(aa_pak::Pak::global().exists(&path)))?)?;
    let state = bridge.clone();
    interaction.set("GetInteractionSkillCount", lua.create_function(move |_, _: Value| Ok(state.borrow().doodad.menu.as_ref().map_or(0, |m| m.1.len())))?)?;
    let state = bridge.clone();
    interaction.set(
        "GetInteractionSkillId",
        lua.create_function(move |_, (_, k): (Value, usize)| Ok(state.borrow().doodad.menu.as_ref().and_then(|m| m.1.get(k.wrapping_sub(1)).map(|e| e.id))))?,
    )?;
    let state = bridge.clone();
    interaction.set(
        "GetInteractionSkillName",
        lua.create_function(move |_, (_, k): (Value, usize)| Ok(state.borrow().doodad.menu.as_ref().and_then(|m| m.1.get(k.wrapping_sub(1)).map(|e| e.name.clone()))))?,
    )?;
    let state = bridge.clone();
    interaction.set(
        "SelectInteraction",
        lua.create_function(move |_, (_, id): (Value, i64)| {
            state.borrow_mut().doodad.chosen.push(id);
            Ok(())
        })?,
    )?;
    let prior: Option<mlua::Function> = interaction.get("CancelInteraction")?;
    let state = bridge.clone();
    interaction.set(
        "CancelInteraction",
        lua.create_function(move |_, args: mlua::MultiValue| {
            if state.borrow_mut().doodad.menu.take().is_some() {
                return Ok(());
            }
            match &prior {
                Some(f) => f.call::<()>(args),
                None => Ok(()),
            }
        })?,
    )?;
    Ok(())
}

/// `--doodad-key N T` and `--doodad-rclick T`.
#[derive(Resource, Default)]
struct Scripted {
    keys: Vec<(f32, usize)>,
    rclicks: Vec<f32>,
    /// `--doodad-near T`: log the doodads within 10 m at T seconds.
    near: Vec<f32>,
    since: f32,
}

impl Scripted {
    fn from_args() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let num = |i: usize| args.get(i).and_then(|v| v.parse::<f32>().ok());
        let mut s = Scripted::default();
        for (i, a) in args.iter().enumerate() {
            match a.as_str() {
                "--doodad-key" => {
                    if let (Some(n), Some(t)) = (num(i + 1), num(i + 2)) {
                        s.keys.push((t, n as usize));
                    }
                }
                "--doodad-rclick" => s.rclicks.extend(num(i + 1)),
                "--doodad-near" => s.near.extend(num(i + 1)),
                _ => {}
            }
        }
        s.keys.sort_by(|a, b| b.0.total_cmp(&a.0));
        s.rclicks.sort_by(|a, b| b.total_cmp(a));
        s.near.sort_by(|a, b| b.total_cmp(a));
        s
    }
}

/// The pointer over the 3D view.
#[derive(bevy::ecs::system::SystemParam)]
struct Pointer<'w, 's> {
    mouse: Res<'w, ButtonInput<MouseButton>>,
    over_ui: Res<'w, HudPointerOverUi>,
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    cameras: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<Camera3d>>,
}

impl Pointer<'_, '_> {
    /// The view ray under the cursor (not over the HUD).
    fn ray(&self) -> Option<Ray3d> {
        if self.over_ui.0 {
            return None;
        }
        let cursor = self.windows.single().ok()?.cursor_position()?;
        let (camera, transform) = self.cameras.iter().find(|(c, _)| c.is_active)?;
        camera.viewport_to_world(transform, cursor).ok()
    }
}

/// How far along `ray` it passes within `radius` of `centre` (game coordinates), if it does.
fn hit(ray: Ray3d, centre: Vec3, radius: f32) -> Option<f32> {
    let centre = aa_world::cry_to_bevy(centre);
    let along = (centre - ray.origin).dot(*ray.direction);
    (along > 0.0 && (ray.origin + *ray.direction * along).distance(centre) <= radius).then_some(along)
}

/// The usable doodad or house the ray picks, unless an NPC is in front of it.
fn pick(ray: Ray3d, data: &HudData, doodads: &Doodads, units: &Units, own: u32) -> Option<Pick> {
    let game = data.game.get()?;
    let mut best: Option<(f32, Pick)> = None;
    for o in doodads.by_id.values() {
        if game.doodad.uses(o.func_group_id).is_none() || data.pos.distance(to_vec(o.pos)) > PICK_DISTANCE {
            continue;
        }
        let scale = if o.scale > 0.0 { o.scale } else { 1.0 };
        let centre = to_vec(o.pos) + Vec3::Z * 0.8 * scale;
        if let Some(along) = hit(ray, centre, 1.3 * scale.max(0.7))
            && best.is_none_or(|(a, _)| along < a)
        {
            best = Some((along, Pick::Doodad(o.obj_id)));
        }
    }
    for u in units.iter().filter(|u| matches!(u.kind, aa_net::model::UnitKind::Housing { .. })) {
        if data.pos.distance(to_vec(u.pos)) > PICK_DISTANCE {
            continue;
        }
        let radius = match u.kind {
            aa_net::model::UnitKind::Housing { template_id, .. } => game.housing.housings.get(&template_id).map_or(2.0, |h| h.garden_radius.max(2.0)),
            _ => 2.0,
        };
        if let Some(along) = hit(ray, to_vec(u.pos) + Vec3::Z * 0.9 * u.scale.max(0.5), radius)
            && best.is_none_or(|(a, _)| along < a)
        {
            best = Some((along, Pick::House(u.obj_id)));
        }
    }
    let (along, picked) = best?;
    let npc_in_front = units.iter().filter(|u| u.obj_id != own && matches!(u.kind, aa_net::model::UnitKind::Npc { .. })).any(|u| {
        hit(ray, to_vec(u.pos) + Vec3::Z * 0.9 * u.scale.max(0.5), 1.1 * u.scale.max(0.7)).is_some_and(|a| a < along)
    });
    (!npc_in_front).then_some(picked)
}

/// The name, owner, permission and progress the original `DRAW_DOODAD_TOOLTIP` shows for a house.
struct HouseInfo {
    name: String,
    owner: String,
    permission: i64,
    current: i64,
    all: i64,
}

fn house_info(units: &Units, houses: &Houses, obj_id: u32) -> Option<HouseInfo> {
    let unit = units.by_id.get(&obj_id)?;
    let aa_net::model::UnitKind::Housing { tl_id, .. } = unit.kind else { return None };
    let house = houses.by_tl_id.get(&tl_id)?;
    let snapshot = &house.snapshot;
    Some(HouseInfo {
        name: if snapshot.name.is_empty() { unit.name.clone() } else { snapshot.name.clone() },
        owner: snapshot.owner_name.clone(),
        permission: i64::from(snapshot.permission),
        current: i64::from(snapshot.current_actions),
        all: i64::from(snapshot.all_actions),
    })
}

/// Finds the doodad in reach, shows / hides the bar, reads the keys, the right click and the pointer.
#[allow(clippy::too_many_arguments)]
fn track(
    rt: Option<NonSend<UiRuntime>>,
    bridge: NonSend<HudBridge>,
    doodads: Res<Doodads>,
    units: Res<Units>,
    houses: Res<Houses>,
    player: Res<aa_net::state::Player>,
    actions: Res<crate::settings::Actions>,
    focus_kb: Res<HudKeyboardFocus>,
    pointer: Pointer,
    time: Res<Time>,
    mut script: ResMut<Scripted>,
    mut pressed_at: Local<Option<Vec2>>,
) {
    let Some(rt) = rt else { return };
    let own = player.spawn.as_ref().map_or(0, |s| s.obj_id);
    script.since += time.delta_secs();
    let mut d = bridge.0.borrow_mut();
    let focus = find_focus(&d, &doodads, &units, &houses);
    let focused_house = focus.as_ref().and_then(|f| f.house.as_ref().map(|_| f.obj_id));
    let end_interaction = d.house.material_open && d.house.material_house != focused_house;
    if end_interaction {
        d.house.material_open = false;
        d.house.material_house = None;
    }
    // the bar
    let wanted = focus.as_ref().map(|f| (f.obj_id, f.skills.clone(), f.house.is_some()));
    let directing = d.dialog.directing;
    d.doodad.claims_key = focus.as_ref().is_some_and(|f| {
        !units.iter().any(|u| u.obj_id != own && !u.dead && matches!(u.kind, aa_net::model::UnitKind::Npc { .. }) && d.pos.distance(to_vec(u.pos)) < f.dist)
    });
    d.doodad.focus = focus;
    if d.doodad.shown != wanted && !directing {
        d.doodad.shown = wanted.clone();
        drop(d);
        match &wanted {
            Some((obj_id, skills, is_house)) => {
                info!("doodad: {obj_id} in reach, uses {skills:?}");
                let kind = if *is_house { "npc" } else { "doodad" };
                rt.fire_event("DYNAMIC_ACTION_BAR_SHOW", (kind, obj_id.to_string()));
            }
            None => rt.fire_event("DYNAMIC_ACTION_BAR_HIDE", ()),
        }
        d = bridge.0.borrow_mut();
    }
    if end_interaction {
        drop(d);
        rt.fire_event("HOUSE_INTERACTION_END", ());
        d = bridge.0.borrow_mut();
    }
    // the keys
    let typing = focus_kb.0 || rt.keyboard_captured();
    let loot = d.loot.nearest.is_some();
    for n in 1..=KEYS {
        if !typing && !loot && !directing && d.doodad.claims_key && actions.just_pressed(&format!("do_interaction_{n}"), 0) {
            d.doodad.execute.push(n);
        }
    }
    while script.keys.last().is_some_and(|k| script.since >= k.0) {
        let (_, n) = script.keys.pop().unwrap();
        info!("doodad: --doodad-key {n}");
        d.doodad.execute.push(n);
    }
    if script.near.last().is_some_and(|t| script.since >= *t) {
        script.near.pop();
        log_near(&d, &doodads);
    }
    // the pointer: a right click uses, a hover names
    let ray = pointer.ray();
    let cursor = pointer.windows.single().ok().and_then(|w| w.cursor_position());
    if pointer.mouse.just_pressed(MouseButton::Right) {
        *pressed_at = if pointer.over_ui.0 { None } else { cursor };
    }
    let mut clicked = None;
    if pointer.mouse.just_released(MouseButton::Right)
        && let (Some(start), Some(end), Some(ray)) = (pressed_at.take(), cursor, ray)
        && start.distance(end) <= CLICK_SLOP
    {
        clicked = match pick(ray, &d, &doodads, &units, own) {
            Some(Pick::Doodad(id)) => Some(id),
            _ => None,
        };
    }
    if script.rclicks.last().is_some_and(|t| script.since >= *t) {
        script.rclicks.pop();
        clicked = nearest_usable(&d, &doodads);
        info!("doodad: --doodad-rclick on {clicked:?}");
    }
    let hover = ray.and_then(|r| pick(r, &d, &doodads, &units, own));
    let hover_changed = hover != d.doodad.hover;
    d.doodad.hover = hover;
    let menu = clicked.is_some_and(|obj_id| right_click(&mut d, &doodads, obj_id));
    let coin = std::mem::take(&mut d.doodad.coin_open);
    let hover_name = match hover {
        Some(Pick::Doodad(id)) => doodads.by_id.get(&id).and_then(|o| d.game.get().and_then(|g| g.doodad.name(o.template_id)).map(str::to_string)),
        _ => None,
    };
    let hover_house = match hover {
        Some(Pick::House(id)) => house_info(&units, &houses, id),
        _ => None,
    };
    drop(d);
    if coin && let Err(error) = rt.exec("@doodad_coin", "OpenCoinShop()") {
        warn!("doodad: coin shop dialog: {error}");
    }
    if menu {
        rt.fire_event("INTERACTION_START", ());
    }
    if hover_changed {
        if let Some(house) = hover_house {
            let info = rt.lua.create_table().and_then(|t| {
                t.set("name", house.name)?;
                t.set("owner", house.owner)?;
                t.set("permission", house.permission)?;
                t.set("alignLeft", true)?;
                let progress = rt.lua.create_table()?;
                progress.set("curCount", house.current)?;
                progress.set("maxCount", house.all)?;
                t.set("progress", progress)?;
                Ok(t)
            });
            if let Ok(info) = info {
                rt.fire_event("DRAW_DOODAD_TOOLTIP", info);
            }
        } else {
            match hover_name {
                Some(name) => {
                    let info = rt.lua.create_table().and_then(|t| t.set("name", name).map(|()| t));
                    if let Ok(info) = info {
                        rt.fire_event("DRAW_DOODAD_TOOLTIP", info);
                    }
                }
                None => rt.fire_event("DRAW_DOODAD_SIGN_TAG", ()),
            }
        }
    }
}

/// `--doodad-near`: the doodads around the player, their phase and uses.
fn log_near(d: &HudData, doodads: &Doodads) {
    let Some(game) = d.game.get() else { return };
    let mut near: Vec<_> = doodads.by_id.values().map(|o| (d.pos.distance(to_vec(o.pos)), o)).filter(|(dist, _)| *dist <= 10.0).collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (dist, o) in near {
        let name = game.doodad.name(o.template_id).unwrap_or("?");
        info!("doodad: near {dist:.1} m: {name} ({}, template {}) phase {} uses {:?}", o.obj_id, o.template_id, o.func_group_id, game.doodad.uses(o.func_group_id));
    }
}

/// The nearest doodad with a use within the pick distance (`--doodad-rclick`).
fn nearest_usable(d: &HudData, doodads: &Doodads) -> Option<u32> {
    let game = d.game.get()?;
    doodads
        .by_id
        .values()
        .filter(|o| game.doodad.uses(o.func_group_id).is_some())
        .map(|o| (d.pos.distance(to_vec(o.pos)), o.obj_id))
        .filter(|(dist, _)| *dist <= PICK_DISTANCE)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

/// A right click on doodad `obj_id`: one use is made at once, several open the interaction list (`true`).
fn right_click(d: &mut HudData, doodads: &Doodads, obj_id: u32) -> bool {
    let Some(o) = doodads.by_id.get(&obj_id) else { return false };
    let Some(game) = d.game.get() else { return false };
    let Some(uses) = game.doodad.uses(o.func_group_id).cloned() else { return false };
    let name = game.doodad.name(o.template_id).unwrap_or("?").to_string();
    let dist = d.pos.distance(to_vec(o.pos));
    let reach = reach(d, &uses);
    d.doodad.preferred = Some(obj_id);
    if dist > reach {
        info!("doodad: {name} ({obj_id}) is too far away to use ({dist:.1} m, reach {reach:.1} m)");
        return false;
    }
    let mut entries: Vec<Entry> = uses
        .skills
        .iter()
        .map(|s| Entry { id: i64::from(*s), name: game.skills.get(s).map(|i| i.named.name.clone()).unwrap_or_else(|| format!("skill {s}")) })
        .collect();
    entries.extend(uses.navi.iter().map(|n| Entry { id: NAVI_ID + *n as i64, name: n.label().into() }));
    for func in &uses.purchases {
        let Some(purchase) = game.doodad.purchase(*func) else { continue };
        if purchase.coin_item_id == 0 || purchase.coin_count <= 0 { continue; }
        let name = game.items.get(&purchase.item_id).map(|i| i.named.name.clone()).unwrap_or_else(|| format!("item {}", purchase.item_id));
        entries.push(Entry { id: COIN_ID + i64::from(*func), name });
    }
    if entries.is_empty() {
        return false;
    }
    if entries.len() == 1 {
        let entry = entries[0].clone();
        if entry.id >= COIN_ID {
            if let Some(purchase) = game.doodad.purchase((entry.id - COIN_ID) as u32).cloned() {
                d.doodad.coin = Some(CoinShop { doodad_obj_id: obj_id, purchase });
                d.doodad.coin_open = true;
            }
            return false;
        }
        d.doodad.menu = Some((obj_id, entries.clone()));
        d.doodad.chosen.push(entry.id);
        return false;
    }
    info!("doodad: {name} ({obj_id}) offers {:?}", entries.iter().map(|e| &e.name).collect::<Vec<_>>());
    d.doodad.menu = Some((obj_id, entries));
    true
}

/// The uses the bar, the keys and the list asked for become packets.
fn execute(
    rt: Option<NonSend<UiRuntime>>,
    bridge: NonSend<HudBridge>,
    doodads: Res<Doodads>,
    mut out: MessageWriter<NetCommand>,
) {
    let mut d = bridge.0.borrow_mut();
    let execute = std::mem::take(&mut d.doodad.execute);
    let chosen = std::mem::take(&mut d.doodad.chosen);
    let buys = std::mem::take(&mut d.doodad.coin_buys);
    let focus = d.doodad.focus.clone();
    let mut commands = Vec::new();
    let mut opened: Option<(u32, HouseState)> = None;
    let mut open_coin = false;
    for i in execute {
        let Some(f) = focus.as_ref() else { continue };
        if let Some(state) = &f.house {
            if i == 1 {
                if let Some(&skill_id) = f.skills.first() {
                    commands.push((f.obj_id, Command::Target { obj_id: f.obj_id }));
                    commands.push((f.obj_id, Command::UseSkill { skill_id, target_obj_id: f.obj_id }));
                    opened = Some((f.obj_id, state.clone()));
                }
            } else if i == 2 {
                opened = Some((f.obj_id, state.clone()));
            }
            continue;
        }
        if let Some(&skill_id) = f.skills.get(i.wrapping_sub(1)) {
            commands.push((f.obj_id, Command::UseDoodadSkill { skill_id, doodad_obj_id: f.obj_id }));
        }
    }
    if !chosen.is_empty()
        && let Some((obj_id, _)) = d.doodad.menu.take()
    {
        for id in chosen {
            if id >= COIN_ID {
                let func = (id - COIN_ID) as u32;
                if let Some(purchase) = d.game.get().and_then(|g| g.doodad.purchase(func)).cloned() {
                    d.doodad.coin = Some(CoinShop { doodad_obj_id: obj_id, purchase });
                    open_coin = true;
                }
                continue;
            }
            let command = match id - NAVI_ID {
                x if x == Navi::Teleport as i64 => Command::Interact(Interact::NaviTeleport { doodad_obj_id: obj_id }),
                x if x == Navi::OpenPortal as i64 => Command::Interact(Interact::NaviOpenPortal { doodad_obj_id: obj_id }),
                _ => Command::UseDoodadSkill { skill_id: id as u32, doodad_obj_id: obj_id },
            };
            commands.push((obj_id, command));
        }
    }
    if let Some((obj_id, state)) = &opened {
        d.house.selected = Some(state.clone());
        d.house.material_open = true;
        d.house.material_house = Some(*obj_id);
    }
    let game = d.game.get();
    for (obj_id, command) in commands {
        if let Some((house_obj, state)) = &opened
            && *house_obj == obj_id
        {
            info!("housing: use {} ({obj_id}): {command:?}", state.name);
        } else {
            let name = doodads.by_id.get(&obj_id).and_then(|o| game.and_then(|g| g.doodad.name(o.template_id))).unwrap_or("?");
            info!("doodad: use {name} ({obj_id}): {command:?}");
        }
        out.write(NetCommand(command));
    }
    drop(d);
    for (doodad_obj_id, func_id) in buys {
        info!("doodad: buy coin item {func_id} from {doodad_obj_id}");
        out.write(NetCommand(Command::Interact(Interact::BuyCoinItem { doodad_obj_id, func_id })));
    }
    if open_coin
        && let Some(rt) = rt.as_deref()
        && let Err(error) = rt.exec("@doodad_coin", "OpenCoinShop()")
    {
        warn!("doodad: coin shop dialog: {error}");
    }
    if opened.is_some()
        && let Some(rt) = rt.as_deref()
    {
        super::houses::open_material_window(rt);
    }
}

/// What the server made of a use, for the log: the cast at a doodad, the doodad's next phase, a new recall point.
fn results(mut events: MessageReader<aa_net::NetEvent>, player: Res<aa_net::state::Player>, doodads: Res<Doodads>) {
    let own = player.spawn.as_ref().map_or(0, |s| s.obj_id);
    for aa_net::NetEvent(e) in events.read() {
        match e {
            aa_net::Event::SkillStarted(s) if s.caster.obj_id == own && s.target.kind == aa_net::skill::target_kind::DOODAD => {
                info!("doodad: cast {} at doodad {} (result {}, {} ms)", s.skill_id, s.target.obj_id, s.result, s.cast_ms);
            }
            aa_net::Event::DoodadPhaseChanged { obj_id, func_group_id, time_left, .. } if doodads.by_id.contains_key(obj_id) => {
                debug!("doodad: {obj_id} is in phase {func_group_id} now ({time_left} ms left)");
            }
            aa_net::Event::Portal(aa_net::portal::PortalEvent::ReturnDistricts { portals, return_point_id }) => {
                info!("doodad: recall point is {return_point_id} ({} districts in the book)", portals.len());
            }
            _ => {}
        }
    }
}

/// The world left: nothing is in reach.
fn reset(bridge: NonSend<HudBridge>) {
    bridge.0.borrow_mut().doodad = View::default();
}

pub(super) fn plugin(app: &mut App) {
    app.insert_resource(Scripted::from_args())
        .add_systems(
            Update,
            (track, execute, results)
                .chain()
                .after(super::sync_hud)
                .after(aa_ui::UiInput)
                .before(super::dialog::talk_input)
                .run_if(super::hud_up)
                .run_if(in_state(crate::Screen::InGame)),
        )
        .add_systems(OnExit(crate::Screen::InGame), reset);
}

#[cfg(test)]
mod tests {
    use super::*;
    use aa_net::housing::{House, HouseSnapshot};
    use crate::hud::data::GameData;

    fn house_at(x: f32, y: f32) -> aa_net::Unit {
        aa_net::Unit {
            obj_id: 77,
            name: "Scarecrow Garden".into(),
            kind: aa_net::model::UnitKind::Housing { tl_id: 13, template_id: 267, build_step: 0 },
            owner_name: "Farmax".into(),
            pos: aa_net::Pos::new(x, y, 0.0),
            rotation: [0.0; 3],
            scale: 1.0,
            level: 1,
            model_id: 989,
            equipment: Default::default(),
            model_params: Default::default(),
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

    fn house_game() -> (HudData, Units, Houses) {
        let mut game = GameData::default();
        game.skills.insert(18553, Default::default());
        game.skills.get_mut(&18553).unwrap().max_range = 10;
        game.skills.insert(INFO_SKILL, Default::default());
        game.skills.get_mut(&INFO_SKILL).unwrap().max_range = 6;
        game.housing.housings.insert(267, Default::default());
        let template = game.housing.housings.get_mut(&267).unwrap();
        template.garden_radius = 4.0;
        template.build_steps.push(Default::default());
        template.build_steps[0].skill_id = 18553;
        template.build_steps[0].num_actions = 1;
        let mut data = HudData::default();
        data.game.0.set(game).ok().expect("one load");
        data.pos = Vec3::ZERO;
        let mut units = Units::default();
        units.by_id.insert(77, house_at(2.0, 0.0));
        let mut houses = Houses::default();
        houses.by_tl_id.insert(
            13,
            House {
                snapshot: HouseSnapshot {
                    tl_id: 13,
                    db_id: 1,
                    obj_id: 77,
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
                    x: 0,
                    y: 0,
                    z: 0.0,
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
            },
        );
        (data, units, houses)
    }

    #[test]
    fn an_unfinished_house_offers_build_and_info_actions() {
        let (data, units, houses) = house_game();
        let focus = find_focus(&data, &Doodads::default(), &units, &houses).expect("the house is in reach");
        assert_eq!(focus.obj_id, 77);
        assert_eq!(focus.skills, vec![18553, INFO_SKILL]);
        assert_eq!(focus.house.as_ref().map(|h| h.current_actions), Some(0));
    }

    #[test]
    fn phases_list_their_skills_and_navi_uses() {
        let mut t = Tables::default();
        t.add(1, 13066, 14602, "DoodadFuncBinding");
        t.add(1, 13066, 14602, "DoodadFuncBinding");
        t.add(2, 13067, 0, "DoodadFuncTimer");
        t.add(3, 1351, 0, "DoodadFuncNaviTeleport");
        t.add(4, 1351, 0, "DoodadFuncNaviOpenPortal");
        t.add(5, 1351, 0, "DoodadFuncNaviNaming");
        t.add(6, 7851, 15193, "DoodadFuncNaviTeleport");
        t.add(7, 9, 10994, "DoodadFuncUse");
        assert_eq!(t.uses(13066), Some(&Uses { skills: vec![14602], navi: vec![], purchases: vec![] }));
        assert_eq!(t.uses(13067), None);
        assert_eq!(t.uses(1351), Some(&Uses { skills: vec![], navi: vec![Navi::Teleport, Navi::OpenPortal], purchases: vec![] }));
        assert_eq!(t.uses(7851), Some(&Uses { skills: vec![15193], navi: vec![], purchases: vec![] }));
        assert_eq!(t.uses(9), None, "the bar hides skill 10994");
    }

    #[test]
    fn purchase_funcs_load_as_coin_shop_rows() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE doodad_funcs(id INT, doodad_func_group_id INT, actual_func_id INT, actual_func_type TEXT, func_skill_id INT);
             CREATE TABLE doodad_func_purchases(id INT, item_id INT, count INT, coin_item_id INT, coin_count INT, currency_id INT);
             CREATE TABLE doodad_almighties(id INT, name TEXT);
             CREATE TABLE localized_texts(tbl_name TEXT, tbl_column_name TEXT, idx INT, en_us TEXT);
             INSERT INTO doodad_funcs VALUES(10117,11355,41,'DoodadFuncPurchase',17537);
             INSERT INTO doodad_func_purchases VALUES(41,23614,1,23633,15,0);
             INSERT INTO doodad_funcs VALUES(10118,11355,42,'DoodadFuncUse',0);",
        )
        .unwrap();
        let t = Tables::load(&db).unwrap();
        let uses = t.uses(11355).expect("purchase phase is usable");
        assert_eq!(uses.purchases, vec![10117]);
        assert!(uses.skills.is_empty(), "the purchase skill must not be cast directly");
        assert_eq!(
            t.purchase(10117),
            Some(&Purchase { func_id: 10117, item_id: 23614, count: 1, coin_item_id: 23633, coin_count: 15, currency_id: 0 })
        );
    }

    #[test]
    fn a_single_purchase_right_click_opens_the_coin_dialog_state() {
        let mut game = super::super::data::GameData::default();
        game.doodad.uses.insert(5, Uses { skills: vec![], navi: vec![], purchases: vec![9] });
        game.doodad.purchases.insert(
            9,
            Purchase { func_id: 9, item_id: 100, count: 1, coin_item_id: 200, coin_count: 7, currency_id: 0 },
        );
        game.items.insert(
            100,
            super::super::data::ItemInfo {
                named: super::super::data::Named { name: "Hat".into(), icon: String::new() },
                ..default()
            },
        );
        let mut data = HudData::default();
        assert!(data.game.0.set(game).is_ok());
        let mut doodads = Doodads::default();
        doodads.by_id.insert(
            1,
            aa_net::model::Doodad {
                obj_id: 1,
                template_id: 1,
                owner_obj_id: 0,
                parent_obj_id: 0,
                attach_point: 0xff,
                pos: default(),
                rotation: [0.0; 4],
                scale: 1.0,
                has_loot_item: false,
                func_group_id: 5,
                owner_id: 0,
                ucc_id: 0,
                item_template_id: 0,
                type2: 0,
                time_left: 0,
                plant_time: 0,
                quest_glow: 0,
                family: 0,
                puzzle_group: 0,
                owner_type: 0,
                owner_db_id: 0,
                data: 0,
            },
        );
        assert!(!right_click(&mut data, &doodads, 1));
        let shop = data.doodad.coin.expect("coin shop state");
        assert_eq!((shop.doodad_obj_id, shop.purchase.func_id), (1, 9));
        assert!(data.doodad.coin_open);
    }

    #[test]
    #[ignore = "loads original world-stage assets; run in an isolated fixture process"]
    fn original_coin_dialog_confirms_a_purchase_func() -> Result<(), Box<dyn std::error::Error>> {
        let (rt, bridge, path, _inventory, _units, _npc) = super::super::pets::fixture::runtime(1920.0, 1080.0)?;
        let (func_id, item_id, purchase) = {
            let d = bridge.borrow();
            let game = d.game.get().ok_or("fixture game data missing")?;
            game.doodad
                .purchases
                .values()
                .find(|p| p.coin_item_id != 0 && p.coin_count > 0 && game.items.contains_key(&p.item_id))
                .map(|p| (p.func_id, p.item_id, p.clone()))
                .ok_or("no coin purchase fixture")?
        };
        bridge.borrow_mut().doodad.coin = Some(CoinShop { doodad_obj_id: 200, purchase });
        rt.lua
            .load(format!(
                r#"
                local original=X2DialogManager.RequestTaskDialog
                X2DialogManager.RequestTaskDialog=function(self,task,info,owner)
                  local w=original(self,task,info,owner)
                  if task==DLG_TASK_PURCHASE_COIN then COIN_DIALOG=w; COIN_INFO=info end
                  return w
                end
                OpenCoinShop()
                assert(COIN_DIALOG and COIN_DIALOG:IsVisible())
                assert(COIN_INFO.itemInfo.name and COIN_INFO.itemInfo.name~="")
                assert(COIN_INFO.coinName~="" and COIN_INFO.coinCount>0)
                assert(COIN_INFO.itemInfo.itemType=={item_id})
                "#
            ))
            .exec()?;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_non_send(rt)
            .insert_non_send(HudBridge(bridge.clone()))
            .insert_resource(Doodads::default())
            .add_message::<NetCommand>()
            .add_systems(Update, execute);
        super::super::expansion::fixture::click(app.world().non_send::<UiRuntime>(), "COIN_DIALOG.btnOk")?;
        app.update();
        let commands = app.world_mut().resource_mut::<Messages<NetCommand>>().drain().map(|v| v.0).collect::<Vec<_>>();
        assert!(matches!(
            commands.as_slice(),
            [Command::Interact(Interact::BuyCoinItem { doodad_obj_id: 200, func_id: f })] if *f == func_id
        ), "{commands:?}");
        drop(app);
        drop(bridge);
        std::fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn a_ray_hits_what_it_passes_near() {
        // game (x, y, z) -> bevy (x, z, -y): a ray along bevy -z from the origin passes game (0, 10, 0)
        let ray = Ray3d::new(Vec3::ZERO, Dir3::NEG_Z);
        assert!(hit(ray, Vec3::new(0.0, 10.0, 0.5), 1.0).is_some_and(|a| (a - 10.0).abs() < 1e-3));
        assert!(hit(ray, Vec3::new(3.0, 10.0, 0.0), 1.0).is_none());
        assert!(hit(ray, Vec3::new(0.0, -10.0, 0.0), 1.0).is_none(), "behind the camera");
    }

    #[test]
    fn the_real_tables_know_the_memory_tomes() {
        let Ok(conn) = Connection::open_with_flags(aa_anim::db::DEFAULT_DB, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY) else { return };
        let t = Tables::load(&conn).unwrap();
        assert_eq!(t.uses(13066).map(|u| u.skills.clone()), Some(vec![14602]));
        assert_eq!(t.name(850), Some("Desireen Memory Tome"));
    }
}
