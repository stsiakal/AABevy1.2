//! Choosing a target: a left click on a unit in the 3D view, `Tab` for the next nearest unit,
//! `Escape` to drop it (after the windows that close on it). The server answers with
//! `TargetChanged`, which the target frame follows (see `panels.rs`).

use aa_net::state::{Player, Target, Units};
use aa_net::{Command, NetCommand};
use aa_ui::UiRuntime;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::api::HudBridge;
use super::{HudKeyboardFocus, HudPointerOverUi};

/// The seven `tab_targeting_*` cvars (`FUN_390f7b40`): a 4 m round catch, then a 50 m fan whose half angle is
/// 60 degrees, with a 20 m height limit; the last [`TAB_HISTORY_MAX`] targets stay in the history for
/// [`TAB_HISTORY_EXPIRE`] seconds.
const TAB_ROUND_DIST: f32 = 4.0;
const TAB_FAN_DIST: f32 = 50.0;
const TAB_FAN_COS: f32 = 0.5;
const TAB_Z_LIMIT: f32 = 20.0;
const TAB_HISTORY_EXPIRE: f32 = 3.0;
const TAB_HISTORY_MAX: usize = 10;
/// How near the view ray has to pass a unit's middle for a click to select it, metres.
const PICK_RADIUS: f32 = 1.1;
/// A press that moves more than this many pixels before it is released was a camera drag.
const CLICK_SLOP: f32 = 6.0;

fn unit_centre(pos: aa_net::Pos, scale: f32) -> Vec3 {
    aa_world::cry_to_bevy(Vec3::new(pos.x, pos.y, pos.z + 0.9 * scale.max(0.5)))
}

/// Whether a unit at `at` is inside the Tab selection volume of `me` facing `facing`: the 4 m round catch, or
/// the 50 m / 60 degree fan below the 20 m height limit.
fn in_tab_volume(me: Vec3, facing: f32, at: Vec3) -> bool {
    let offset = at - me;
    let distance = offset.length();
    if distance <= TAB_ROUND_DIST {
        return true;
    }
    let flat = offset.truncate();
    let planar = flat.length();
    planar <= TAB_FAN_DIST
        && offset.z.abs() <= TAB_Z_LIMIT
        && (planar < 1.0 || crate::ingame::player::forward(facing).dot(flat.normalize_or_zero()) >= TAB_FAN_COS)
}

/// The Tab candidates among `me` facing `facing`, nearest first.
fn tab_candidates(me: Vec3, facing: f32, candidates: impl IntoIterator<Item = (u32, Vec3)>) -> Vec<(f32, u32)> {
    let mut near: Vec<(f32, u32)> = candidates
        .into_iter()
        .filter_map(|(id, at)| in_tab_volume(me, facing, at).then_some((me.distance(at), id)))
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    near
}

/// The target after `current`: the next entry in the nearest-first list (or the previous one backwards). With no
/// target in the list, the nearest unit the 3 s history does not hold yet, else the nearest unit.
fn next_tab_target(near: &[(f32, u32)], current: Option<u32>, history: &[u32], backwards: bool) -> Option<u32> {
    let len = near.len();
    if len == 0 {
        return None;
    }
    if let Some(i) = current.and_then(|t| near.iter().position(|(_, id)| *id == t)) {
        return Some(near[(i + if backwards { len - 1 } else { 1 }) % len].1);
    }
    if !backwards
        && let Some((_, id)) = near.iter().find(|(_, id)| !history.contains(id))
    {
        return Some(*id);
    }
    Some(near[if backwards { len - 1 } else { 0 }].1)
}

/// The last targets Tab picked, with the time each was picked (seconds).
#[derive(Default)]
pub(super) struct TabHistory(Vec<(u32, f32)>);

impl TabHistory {
    fn prune(&mut self, now: f32) {
        self.0.retain(|(_, at)| now - at < TAB_HISTORY_EXPIRE);
    }

    fn record(&mut self, id: u32, now: f32) {
        self.prune(now);
        self.0.retain(|(held, _)| *held != id);
        self.0.insert(0, (id, now));
        self.0.truncate(TAB_HISTORY_MAX);
    }

    fn ids(&self) -> Vec<u32> {
        self.0.iter().map(|(id, _)| *id).collect()
    }
}

/// `Tab`: the nearest enemy that is alive and not the one targeted, the next one after it when pressed again (the
/// client's "target nearest enemy"; with no enemy in range, any unit).
#[allow(clippy::too_many_arguments)]
pub fn tab_target(
    time: Res<Time>,
    actions: Res<crate::settings::Actions>,
    focus: Res<HudKeyboardFocus>,
    bridge: NonSend<HudBridge>,
    units: Res<Units>,
    target: Res<Target>,
    player: Res<Player>,
    mut script: ResMut<super::chat::ScriptedKeys>,
    mut history: Local<TabHistory>,
    mut net: MessageWriter<NetCommand>,
) {
    let scripted = script.take_tab();
    let mode = [
        ("cycle_hostile_forward", false, false),
        ("cycle_hostile_backward", true, false),
        ("cycle_friendly_forward", false, true),
        ("cycle_friendly_backward", true, true),
    ]
    .into_iter()
    .find(|(name, _, _)| actions.just_pressed(name, 0));
    if focus.0 || !(scripted || mode.is_some()) {
        return;
    }
    let (_, backwards, friendly) = mode.unwrap_or(("cycle_hostile_forward", false, false));
    let own = player.spawn.as_ref().map_or(0, |s| s.obj_id);
    let (me, yaw, shared) = {
        let d = bridge.0.borrow();
        (d.pos, d.yaw, d.game.clone())
    };
    let now = time.elapsed_secs();
    history.prune(now);
    let own_faction = player.spawn.as_ref().map_or(0, |s| s.character().faction_id);
    let mut near: Vec<(f32, u32, bool)> = units
        .iter()
        .filter(|u| u.obj_id != own)
        .filter(|u| !friendly || (!u.dead && super::plates::relation_of(shared.get(), own_faction, u) == "friendly"))
        .filter_map(|u| {
            let hostile = !u.dead && super::plates::relation_of(shared.get(), own_faction, u) == "hostile";
            let (distance, id) = tab_candidates(me, yaw, [(u.obj_id, Vec3::new(u.pos.x, u.pos.y, u.pos.z))]).pop()?;
            Some((distance, id, hostile))
        })
        .collect();
    if near.iter().any(|n| n.2) {
        near.retain(|n| n.2);
    }
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    let flat: Vec<(f32, u32)> = near.iter().map(|(d, id, _)| (*d, *id)).collect();
    let Some(next) = next_tab_target(&flat, target.obj_id, &history.ids(), backwards) else { return };
    history.record(next, now);
    net.write(NetCommand(Command::Target { obj_id: next }));
}

/// A left click (not a drag, not on the HUD) on a unit selects it.
#[allow(clippy::too_many_arguments)]
pub fn click_target(
    mouse: Res<ButtonInput<MouseButton>>,
    over_ui: Res<HudPointerOverUi>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    units: Res<Units>,
    player: Res<Player>,
    mut script_pick: ResMut<crate::script::ScriptPick>,
    mut net: MessageWriter<NetCommand>,
    mut pressed_at: Local<Option<Vec2>>,
) {
    // a pick the script runner made (`target pick ...`) is the click's result
    if let Some(obj_id) = script_pick.take_left() {
        net.write(NetCommand(Command::Target { obj_id }));
        return;
    }
    let Ok(window) = windows.single() else { return };
    let cursor = window.cursor_position();
    if mouse.just_pressed(MouseButton::Left) {
        *pressed_at = if over_ui.0 { None } else { cursor };
    }
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let (Some(start), Some(end)) = (pressed_at.take(), cursor) else { return };
    if start.distance(end) > CLICK_SLOP || over_ui.0 {
        return;
    }
    let Some((camera, transform)) = cameras.iter().find(|(c, _)| c.is_active) else { return;
    };
    let Ok(ray) = camera.viewport_to_world(transform, end) else { return };
    let own = player.spawn.as_ref().map_or(0, |s| s.obj_id);
    let mut best: Option<(f32, u32)> = None;
    for u in units.iter().filter(|u| u.obj_id != own) {
        let centre = unit_centre(u.pos, u.scale);
        let along = (centre - ray.origin).dot(*ray.direction);
        if along <= 0.0 {
            continue;
        }
        let miss = (ray.origin + *ray.direction * along).distance(centre);
        if miss <= PICK_RADIUS * u.scale.max(0.7) && best.is_none_or(|(a, _)| along < a) {
            best = Some((along, u.obj_id));
        }
    }
    if let Some((_, obj_id)) = best {
        net.write(NetCommand(Command::Target { obj_id }));
    }
}

/// `Escape`: close the topmost window that closes on it; with none open, drop the target; else
/// the toolkit opens the system menu.
pub fn escape_key(
    keys: Res<ButtonInput<KeyCode>>,
    actions: Res<crate::settings::Actions>,
    focus: Res<HudKeyboardFocus>,
    rt: Option<NonSend<UiRuntime>>,
    target: Res<Target>,
    mut net: MessageWriter<NetCommand>,
    mut script: ResMut<super::chat::ScriptedKeys>,
) {
    let Some(rt) = rt else { return };
    let scripted = script.take_escape();
    let mapped = actions.just_pressed("open_config", 0);
    let physical = keys.just_pressed(KeyCode::Escape);
    let escape = scripted || physical || mapped;
    if focus.0 || rt.keyboard_captured() || !escape {
        return;
    }
    if mapped && !physical && !scripted {
        if let Err(e) = rt.hotkey("open_config") {
            warn!("hud: open_config: {e}");
        }
        return;
    }
    if super::repair::escape(&rt) { return; }
    if rt.holding() {
        rt.clear_cursor();
        return;
    }
    let window_open = {
        let u = aa_ui::lua::ui(&rt.lua);
        u.tree.paint_order().into_iter().any(|id| u.tree.get(id).alive && u.tree.widget(id).is_some_and(|w| w.close_on_escape) && u.tree.visible(id))
    };
    if !window_open && target.obj_id.is_some() {
        net.write(NetCommand(Command::Target { obj_id: 0 }));
    } else if window_open || mapped || scripted {
        if let Err(e) = rt.escape() {
        warn!("hud: escape: {e}");
    }
}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingame::virtual_input::{inject, VirtualInput};
    use bevy::input::InputPlugin;

    #[test]
    fn tab_uses_the_round_catch_and_the_forward_fan() {
        let me = Vec3::ZERO;
        // 4 m round: in front or behind, inside the catch
        assert!(in_tab_volume(me, 0.0, Vec3::new(0.0, 3.9, 0.0)));
        assert!(in_tab_volume(me, 0.0, Vec3::new(0.0, -3.9, 0.0)));
        assert!(!in_tab_volume(me, 0.0, Vec3::new(0.0, -4.1, 0.0)));
        // 50 m fan, 60 degrees to either side of the facing
        assert!(in_tab_volume(me, 0.0, Vec3::new(43.0, 25.0, 0.0)));
        assert!(!in_tab_volume(me, 0.0, Vec3::new(46.0, 25.0, 0.0)));
        assert!(!in_tab_volume(me, 0.0, Vec3::new(0.0, -10.0, 0.0)));
        assert!(!in_tab_volume(me, 0.0, Vec3::new(0.0, 51.0, 0.0)));
        // 20 m height limit
        assert!(!in_tab_volume(me, 0.0, Vec3::new(0.0, 10.0, 20.1)));
        assert!(in_tab_volume(me, 0.0, Vec3::new(0.0, 10.0, -19.9)));
    }

    #[test]
    fn tab_cycles_the_list_and_skips_the_three_second_history() {
        let near = [(1.0, 11), (2.0, 12), (3.0, 13)];
        assert_eq!(next_tab_target(&near, Some(11), &[], false), Some(12));
        assert_eq!(next_tab_target(&near, Some(13), &[], false), Some(11));
        assert_eq!(next_tab_target(&near, Some(11), &[], true), Some(13));
        assert_eq!(next_tab_target(&near, None, &[11, 12], false), Some(13));
        assert_eq!(next_tab_target(&near, None, &[11, 12, 13], false), Some(11));
        assert_eq!(next_tab_target(&[], None, &[], false), None);
        // the history keeps 10 entries for 3 s
        let mut history = TabHistory::default();
        for id in 0..12 {
            history.record(id, 0.0);
        }
        assert_eq!(history.0.len(), TAB_HISTORY_MAX);
        assert_eq!(history.0[0].0, 11);
        history.record(5, 2.9);
        assert_eq!(history.0[0].0, 5);
        history.record(6, 3.1);
        assert_eq!(history.ids(), vec![6, 5], "entries older than 3 s are gone");
    }

    /// A scripted `key escape` is a real Escape edge: the binding/input path (`settings::update_input`
    /// and `ButtonInput<KeyCode>`) sees it, and the open original window closes.
    #[test]
    fn a_scripted_escape_closes_the_open_window() -> Result<(), Box<dyn std::error::Error>> {
        let rt = aa_ui::UiRuntime::new("en_us", 1280.0, 720.0)?;
        rt.lua
            .load(
                r#"
            testWindow = UIParent:CreateWidget("window", "testEscape", "UIParent")
            testWindow:SetExtent(200, 100)
            testWindow:SetCloseOnEscape(true)
            testWindow:Show(true)
            hidden = 0
            testWindow:SetHandler("OnHide", function() hidden = hidden + 1 end)
            "#,
            )
            .set_name("@script_escape_test")
            .exec()?;
        let mut app = App::new();
        app.add_plugins(InputPlugin)
            .insert_resource(Time::<()>::default())
            .init_resource::<VirtualInput>()
            .init_resource::<crate::settings::Actions>()
            .init_resource::<HudKeyboardFocus>()
            .init_resource::<HudPointerOverUi>()
            .init_resource::<super::super::chat::ScriptedKeys>()
            .init_resource::<Target>()
            .add_message::<NetCommand>()
            .insert_non_send(crate::hud::api::HudBridge::default())
            .insert_non_send(rt)
            .add_systems(PreUpdate, inject.before(bevy::input::InputSystems))
            .add_systems(Update, (crate::settings::update_input, escape_key).chain());
        app.world_mut().resource_mut::<VirtualInput>().tap_name("escape");
        app.update();
        let rt = app.world().non_send::<aa_ui::UiRuntime>();
        let visible: bool = rt.lua.load("return testWindow:IsVisible()").eval()?;
        let hidden: i64 = rt.lua.load("return hidden").eval()?;
        assert!(!visible, "Escape closed the window");
        assert_eq!(hidden, 1, "its OnHide ran once");
        Ok(())
    }
}
