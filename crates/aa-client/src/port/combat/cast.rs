//! Read-only cast and combat queries from the 1.2 `x2game.dll` decompile.
//!
//! | Rust | original | VA |
//! |---|---|---|
//! | [`unit_casting_info`] | `UnitCastingInfo` | `FUN_394bcf60` |
//! | [`cast_progress`] | cast/cooldown progress query | `FUN_39395510` |
//! | [`can_cast`] | casting-cancel check | `FUN_39395d90` |
//! | [`can_channel`] | channeling-cancel check | `FUN_39395e40` |
//! | [`unit_combat_state`] | `UnitCombatState` | `FUN_394bd830` |
//! | [`is_first_hit_by_me_or_my_team`] | `IsFirstHitByMeOrMyTeam` | `FUN_394bd9f0` |
//!
//! These functions only read unit/manager state; the goldens in `goldens/cast.json` were
//! recorded from the original with a fake Lua callback object whose vtable captured the four
//! descriptors `FUN_394bbad0` pushes (name, total, current, visible).

/// The four cast-state keys stored on a unit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CastUnit {
    /// `Unit+0x3ac4`: target key of the active normal cast (`0` = none).
    pub cast_key: u32,
    /// `Unit+0x3ac8`: key of the active cast-delay/plot event (`0` = none).
    pub cast_delay_key: u32,
    /// `Unit+0x3acc`: target key of the active channel (`0` = none).
    pub channel_key: u32,
    /// `Unit+0x3ad0`: key of the active channel-delay/plot event (`0` = none).
    pub channel_delay_key: u32,
    /// `Unit+0x3bba`: in-combat byte.
    pub in_combat: bool,
    /// `Unit+0x5fe8`: first-hitter object id.
    pub first_hit_id: u32,
    /// `Unit+0x5fec`: first-hitter team/relation (`0` = self, `DAT_39d44c50` = my team).
    pub first_hit_team: u32,
}

/// The timing fields of a skill instance (`FUN_393953a0(key)`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SkillCastState {
    /// `+0x004`: skill id.
    pub skill_id: u32,
    /// `+0x138`: normal cast total (ms).
    pub normal_total: u32,
    /// `+0x13c`: computed delay written by `Fire`.
    pub computed_delay: u32,
    /// `+0x140`: channel total (ms).
    pub channel_total: u32,
    /// `+0x144`: normal cast start (engine ms).
    pub normal_start: u32,
    /// `+0x148`: channel start (engine ms).
    pub channel_start: u32,
}

/// The plot/effect cast state behind `FUN_3938c2e0(key)` (value `+0x18`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlotCastState {
    /// `+0x08`: plot/skill id.
    pub plot_id: u32,
    /// `+0x74`: cast-delay total.
    pub cast_total: u32,
    /// `+0x70`: cast-delay start.
    pub cast_start: u32,
    /// `+0x84`: channel-delay total.
    pub channel_total: u32,
    /// `+0x80`: channel-delay start.
    pub channel_start: u32,
}

/// The `SkillDesc` fields these queries read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CastDesc {
    /// `+0x104`: skill name (cast-bar text).
    pub name: String,
    /// `+0x13a`: show the target's casting time (visible flag for cast/channel states).
    pub show_target_casting_time: bool,
    /// `+0x15a`: the normal cast can be cancelled (`FUN_39395d90`).
    pub casting_cancelable: bool,
    /// `+0x15d`: the channel can be cancelled (`FUN_39395e40`).
    pub channeling_cancelable: bool,
    /// Skill-controller kinds from the `+0x1e8..+0x1ec` vector. `FUN_39395d90` treats kind
    /// `0x22` as "cancel allowed".
    pub controller_kinds: Vec<i32>,
}

/// The value `FUN_394bbad0` pushes: `(name, total, current, visible)`.
#[derive(Clone, Debug, PartialEq)]
pub enum CastingInfo {
    /// No live cast state (the original pushes nil).
    None,
    /// A live cast/channel state.
    Value {
        name: String,
        total: u32,
        current: u32,
        visible: bool,
    },
}

/// `FUN_394bcf60 UnitCastingInfo(token)`: resolve the token, then check the four cast states in
/// order normal cast > channel > cast delay > channel delay.
///
/// `lookup_desc` is `FUN_396f6390`; `lookup_state` is `FUN_393953a0`; `lookup_plot` is
/// `FUN_3938c2e0`.  The delay branches always push visible `true`.
pub fn unit_casting_info(
    is_unit: bool,
    unit: &CastUnit,
    now: u32,
    lookup_desc: impl Fn(u32) -> Option<CastDesc>,
    lookup_state: impl Fn(u32) -> Option<SkillCastState>,
    lookup_plot: impl Fn(u32) -> Option<PlotCastState>,
) -> CastingInfo {
    if !is_unit {
        return CastingInfo::None;
    }
    if let Some(state) = lookup_state(unit.cast_key) {
        let delta = now.wrapping_sub(state.normal_start);
        let current = state.normal_total.min(delta);
        let (name, visible) = lookup_desc(state.skill_id)
            .map(|desc| (desc.name, desc.show_target_casting_time))
            .unwrap_or_default();
        return CastingInfo::Value {
            name,
            total: state.normal_total,
            current,
            visible,
        };
    }
    if let Some(state) = lookup_state(unit.channel_key) {
        let delta = now.wrapping_sub(state.channel_start);
        let current = state.channel_total - state.channel_total.min(delta);
        let (name, visible) = lookup_desc(state.skill_id)
            .map(|desc| (desc.name, desc.show_target_casting_time))
            .unwrap_or_default();
        return CastingInfo::Value {
            name,
            total: state.channel_total,
            current,
            visible,
        };
    }
    if let Some(plot) = lookup_plot(unit.cast_delay_key) {
        let delta = now.wrapping_sub(plot.cast_start);
        let current = plot.cast_total.min(delta);
        let name = lookup_desc(plot.plot_id).map(|desc| desc.name).unwrap_or_default();
        return CastingInfo::Value {
            name,
            total: plot.cast_total,
            current,
            visible: true,
        };
    }
    if let Some(plot) = lookup_plot(unit.channel_delay_key) {
        let current = plot
            .channel_start
            .wrapping_add(plot.channel_total)
            .wrapping_sub(now);
        let name = lookup_desc(plot.plot_id).map(|desc| desc.name).unwrap_or_default();
        return CastingInfo::Value {
            name,
            total: plot.channel_total,
            current,
            visible: true,
        };
    }
    CastingInfo::None
}

/// `FUN_39395510`: progress of the active cast/channel, or a skill cooldown.
///
/// The kind is read from `*in_EAX`: `8` = `FUN_3989a450(manager, desc)` (cooldown remaining),
/// `7` = the four cast states (normal > channel > plot cast > plot channel).  Any other value
/// returns `None`.  The original uses unsigned arithmetic throughout.
pub fn cast_progress(
    unit: &CastUnit,
    now: u32,
    kind: u8,
    lookup_state: impl Fn(u32) -> Option<SkillCastState>,
    lookup_plot: impl Fn(u32) -> Option<PlotCastState>,
    cooldown_remaining: impl FnOnce() -> u32,
) -> Option<u32> {
    if kind == 8 {
        return Some(cooldown_remaining());
    }
    if kind != 7 {
        return None;
    }
    if let Some(state) = lookup_state(unit.cast_key) {
        let delta = now.wrapping_sub(state.normal_start);
        return Some(state.normal_total - state.normal_total.min(delta));
    }
    if let Some(state) = lookup_state(unit.channel_key) {
        let delta = now.wrapping_sub(state.channel_start);
        return Some(state.channel_total - state.channel_total.min(delta));
    }
    if let Some(plot) = lookup_plot(unit.cast_delay_key) {
        let delta = now.wrapping_sub(plot.cast_start);
        return Some(plot.cast_total - plot.cast_total.min(delta));
    }
    if let Some(plot) = lookup_plot(unit.channel_delay_key) {
        return Some(plot.channel_start.wrapping_add(plot.channel_total).wrapping_sub(now));
    }
    None
}

fn active_desc(
    unit: &CastUnit,
    channel: bool,
    lookup_desc: impl Fn(u32) -> Option<CastDesc>,
    lookup_state: impl Fn(u32) -> Option<SkillCastState>,
    lookup_plot: impl Fn(u32) -> Option<PlotCastState>,
) -> Option<CastDesc> {
    let key = if channel { unit.channel_key } else { unit.cast_key };
    if let Some(state) = lookup_state(key) {
        return lookup_desc(state.skill_id);
    }
    let plot_key = if channel {
        unit.channel_delay_key
    } else {
        unit.cast_delay_key
    };
    lookup_plot(plot_key).and_then(|plot| lookup_desc(plot.plot_id))
}

/// `FUN_39395d90`: whether the active normal cast can be cancelled.
pub fn can_cast(
    unit: &CastUnit,
    lookup_desc: impl Fn(u32) -> Option<CastDesc>,
    lookup_state: impl Fn(u32) -> Option<SkillCastState>,
    lookup_plot: impl Fn(u32) -> Option<PlotCastState>,
) -> bool {
    match active_desc(unit, false, lookup_desc, lookup_state, lookup_plot) {
        Some(desc) => {
            desc.casting_cancelable || desc.controller_kinds.contains(&0x22)
        }
        None => false,
    }
}

/// `FUN_39395e40`: whether the active channel can be cancelled.
pub fn can_channel(
    unit: &CastUnit,
    lookup_desc: impl Fn(u32) -> Option<CastDesc>,
    lookup_state: impl Fn(u32) -> Option<SkillCastState>,
    lookup_plot: impl Fn(u32) -> Option<PlotCastState>,
) -> bool {
    active_desc(unit, true, lookup_desc, lookup_state, lookup_plot)
        .map(|desc| desc.channeling_cancelable)
        .unwrap_or(false)
}

/// `FUN_394bd830 UnitCombatState(token)`: a non-unit token pushes `false`.
pub fn unit_combat_state(is_unit: bool, in_combat: bool) -> bool {
    is_unit && in_combat
}

/// `FUN_394bd9f0 IsFirstHitByMeOrMyTeam(token)`.
///
/// `none_id` is `DAT_3a5ab31c` (0).  A first-hit id equal to it returns `true` (the original
/// sets the pushed byte to 1 before the unit check).  Otherwise the local player must exist;
/// team `0` compares the hitter with the local unit id and any other team is `true` only when it
/// equals `my_team` (`DAT_39d44c50`).
pub fn is_first_hit_by_me_or_my_team(
    is_unit: bool,
    first_hit_id: u32,
    first_hit_team: u32,
    local_unit_id: u32,
    my_team: u32,
    none_id: u32,
) -> bool {
    if !is_unit {
        return false;
    }
    if first_hit_id == none_id {
        return true;
    }
    if local_unit_id == 0 {
        return false;
    }
    if first_hit_team == 0 {
        first_hit_id == local_unit_id
    } else {
        first_hit_team == my_team
    }
}
