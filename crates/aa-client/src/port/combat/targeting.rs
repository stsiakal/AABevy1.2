//! Targeting and combat state from the 1.2 client.
//!
//! Ported functions:
//!
//! * `FUN_39368630` — tab-target entry: pick the hostile/friendly selection state by the
//!   direction byte, ask [`select_next`] for a candidate and set the target.
//! * `FUN_39368520` — one tab step: expire/clear the seen-target history, collect candidates,
//!   retry once after clearing, remember the chosen id and its time.
//! * `FUN_393672c0` — candidate collection: fan (60°, 50 m) plus 4 m round catch, 20 m z limit,
//!   history exclusion, relation/visibility filters, distance sort.
//! * `FUN_39366cc0` / `FUN_39364be0` / `FUN_393548a0` — append and ascending-distance sort.
//! * `FUN_393642a0` / `FUN_39363ec0` / `FUN_39363f60` — the separate "previous target" list
//!   (newest first, `tab_targeting_history_max` = 10, oldest dropped).
//! * `FUN_394270a0` — the target setter (atomic `unit+0x1aa0`, target frames, watch/team sides).
//! * `FUN_39429e40` / `FUN_390f4960` — click selection: pick/clear a target.
//! * `FUN_390b36f0` / `FUN_390b3710` — the `unit+0x3bba` in-combat byte.
//! * `FUN_39354780` / `FUN_394bd9f0` — first hitter `unit+0x5fe8/+0x5fec` and the
//!   "first hit by me or my team" predicate.
//! * `FUN_39390ec0` / `FUN_39390f20` — the `CSSetForceAttackPacket` (0x4f) sender and the
//!   Ctrl+F eligibility gate; `FUN_391d3c60` is the server-side setter (`unit+0x3bbc`).
//!
//! The live goldens (`goldens/targeting.json`, `goldens/combat_state.json`) were recorded from
//! the hidden 1.2 client; each case stores the actor table the native loop saw, so the Rust
//! filter and math are compared bit-for-bit on the same input.

/// `DAT_3a169968`: the "no unit" id used by the targeting code.
pub const NO_UNIT: u32 = 0;
/// `DAT_3a5ab31c`: "nobody" in the first-hit fields.
pub const NO_FIRST_HIT: u32 = 0;
/// `DAT_39d44c50`: the local player's team marker in `unit+0x5fec`.
pub const MY_TEAM: u32 = 1;
/// `DAT_39ac74ac` is the friendly state (`*state == 1`), `0x3a169a4c` the hostile one.
pub const STATE_HOSTILE: i32 = 0;
pub const STATE_FRIENDLY: i32 = 1;

/// The seven `tab_targeting_*` cvars (`FUN_390f7b40`, object `DAT_39aefb98`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TargetingConfig {
    /// `tab_targeting_round_dist` (`+0x2d0`, 4.0).
    pub round_dist: f32,
    /// `tab_targeting_fan_dist` (`+0x2c8`, 50.0).
    pub fan_dist: f32,
    /// `tab_targeting_fan_angle` (`+0x2cc`, 60.0; the loop uses half of it).
    pub fan_angle_deg: f32,
    /// `tab_targeting_z_limit` (`+0x2c4`, 20.0).
    pub z_limit: f32,
    /// `tab_targeting_history_expire_time` (`+0x2c0`, 3.0 s).
    pub history_expire_s: f32,
    /// `tab_targeting_history_max` (`+0x2bc`, 10) — the *previous-target* list capacity.
    pub history_max: usize,
    /// `tab_targeting_dir` (`+0x2b8`, 0): selects the camera-relative basis.
    pub dir: i32,
}

impl Default for TargetingConfig {
    fn default() -> Self {
        Self {
            round_dist: 4.0,
            fan_dist: 50.0,
            fan_angle_deg: 60.0,
            z_limit: 20.0,
            history_expire_s: 3.0,
            history_max: 10,
            dir: 0,
        }
    }
}

/// One actor as the targeting loop reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TabActor {
    pub id: u32,
    pub pos: [f32; 3],
    /// `+0x3c26` mounted / attached.
    pub attached: bool,
    /// `+0x3c27` rider flag (not read by the tab loop, kept for the combat helpers).
    pub rider: bool,
    /// `+0x410` unit kind (3 and 6 are skipped).
    pub kind: i32,
    /// `+0x3bbb` hidden from targeting.
    pub hidden: bool,
    /// `+0x3a78` stealth: only selectable when `stealth_visible`.
    pub stealth: bool,
    /// `FUN_3935a2b0(local, actor)` result (stealth visibility).
    pub stealth_visible: bool,
    /// `FUN_39028240(actor+0x3c28)` relation bits; bit 0 = hostile.
    pub relation: u32,
    /// `FUN_39357650(actor)` result.
    pub selectable: bool,
}

/// A collected candidate: `(id, 3D distance)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candidate {
    pub id: u32,
    pub distance: f32,
}

/// One tab state (`*state` direction, `+4` last selection time, `+0x18` seen set).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TabState {
    pub dir: i32,
    pub last_time: f32,
    /// The `std::set<u32>` at `+0x18`: ids already visited in this cycle.
    pub seen: Vec<u32>,
}

impl TabState {
    pub fn new(dir: i32) -> Self {
        Self { dir, last_time: 0.0, seen: Vec::new() }
    }

    fn remember(&mut self, id: u32) {
        if !self.seen.contains(&id) {
            self.seen.push(id);
        }
    }
}

/// The second history: the list filled by `FUN_393642a0` and read by `FUN_39364190`.
/// Newest entry first; capacity `tab_targeting_history_max` (oldest dropped).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TargetHistory {
    pub ids: Vec<u32>,
}

impl TargetHistory {
    /// `FUN_39363ec0`: insert at the front unless it is already the front, trim the back.
    pub fn record(&mut self, id: u32, max: usize) {
        if self.ids.first() == Some(&id) {
            return;
        }
        self.ids.insert(0, id);
        while self.ids.len() > max {
            self.ids.pop();
        }
    }

    /// `FUN_39363f60`: drop the current target from the front, then return the first eligible
    /// older target within the fan distance (and remove it from the list).
    pub fn previous(
        &mut self,
        current: u32,
        dir: i32,
        cfg: &TargetingConfig,
        local: &TabActor,
        actors: &[TabActor],
    ) -> Option<u32> {
        if self.ids.first() == Some(&current) {
            self.ids.remove(0);
        }
        for index in 0..self.ids.len() {
            let id = self.ids[index];
            if id == current {
                continue;
            }
            let Some(actor) = actors.iter().find(|actor| actor.id == id) else {
                continue;
            };
            if actor.hidden
                || actor.kind == 3
                || (actor.stealth && !actor.stealth_visible)
                || !relation_matches(dir, actor.relation)
            {
                continue;
            }
            let dx = actor.pos[0] - local.pos[0];
            let dy = actor.pos[1] - local.pos[1];
            let dz = actor.pos[2] - local.pos[2];
            let distance = (dx * dx + dy * dy + dz * dz).sqrt();
            if distance <= cfg.fan_dist {
                self.ids.remove(index);
                return Some(id);
            }
        }
        None
    }
}

fn relation_matches(dir: i32, relation: u32) -> bool {
    // `FUN_39028240`'s second byte bit 0 (`0x100` in the dword) is the hostile bit.
    if dir == STATE_HOSTILE {
        relation & 0x100 != 0
    } else {
        relation & 0x100 == 0
    }
}

/// `FUN_393672c0`: append every eligible actor to the candidate list.
///
/// `local` is the player whose camera/position drives the loop; `forward` is the horizontal
/// view direction **before** normalization (the native code derives it from the engine camera or
/// from the view matrix when `tab_targeting_dir != 0`; that derivation is engine work and is an
/// input here).
pub fn collect_candidates(
    state: &TabState,
    local: &TabActor,
    actors: &[TabActor],
    forward: [f32; 2],
    cfg: &TargetingConfig,
) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    let norm = 1.0 / (forward[0] * forward[0] + forward[1] * forward[1] + f32::MIN_POSITIVE).sqrt();
    let half_angle = cfg.fan_angle_deg * (std::f32::consts::PI / 180.0) * 0.5;
    // The native loop walks a std::map keyed by unit id, i.e. ascending id order.
    let mut ordered: Vec<&TabActor> = actors.iter().filter(|actor| actor.id != NO_UNIT).collect();
    ordered.sort_by_key(|actor| actor.id);
    for actor in ordered {
        if state.seen.contains(&actor.id)
            || actor.attached
            || actor.kind == 3
            || actor.kind == 6
            || actor.hidden
            || (actor.stealth && !actor.stealth_visible)
            || !relation_matches(state.dir, actor.relation)
            || !actor.selectable
        {
            continue;
        }
        let dx = actor.pos[0] - local.pos[0];
        let dy = actor.pos[1] - local.pos[1];
        let dz = actor.pos[2] - local.pos[2];
        if dz.abs() > cfg.z_limit {
            continue;
        }
        let distance = (dx * dx + dy * dy + dz * dz).sqrt();
        if distance > cfg.fan_dist {
            continue;
        }
        if distance >= cfg.round_dist {
            let inverse = 1.0 / (dx * dx + dy * dy + f32::MIN_POSITIVE).sqrt();
            let cross = (dy * forward[0] - forward[1] * dx) * inverse * norm;
            let dot = (dx * forward[0] + dy * forward[1]) * inverse * norm;
            let angle = cross.atan2(dot);
            if angle.abs() > half_angle {
                continue;
            }
        }
        candidates.push(Candidate { id: actor.id, distance });
    }
    // `FUN_39364be0` with `FUN_393548a0`: ascending distance.
    candidates.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));
    candidates
}

/// `FUN_39368520`: one tab step on a selection state.
///
/// `collect` receives the state (with any history insert already applied) and returns the
/// candidate list; the native function calls the collector at `+8` twice.
pub fn select_next<F>(
    state: &mut TabState,
    cfg: &TargetingConfig,
    now: f32,
    _local_id: u32,
    mut collect: F,
) -> Option<u32>
where
    F: FnMut(&TabState) -> Vec<Candidate>,
{
    let expired = cfg.history_expire_s < now - state.last_time;
    if expired {
        state.seen.clear();
    }
    let mut candidates = collect(state);
    if candidates.is_empty() {
        if expired {
            return None;
        }
        state.seen.clear();
        candidates = collect(state);
        if candidates.is_empty() {
            return None;
        }
    }
    let id = candidates[0].id;
    state.remember(id);
    state.last_time = now;
    Some(id)
}

/// Which of the two tab states an entry point uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabMode {
    Hostile,
    Friendly,
}

impl TabMode {
    pub fn from_byte(byte: u8) -> Self {
        if byte == 0 { TabMode::Hostile } else { TabMode::Friendly }
    }

    pub fn dir(self) -> i32 {
        match self {
            TabMode::Hostile => STATE_HOSTILE,
            TabMode::Friendly => STATE_FRIENDLY,
        }
    }
}

/// The two selection states the client keeps (`0x3a169a4c` hostile, `0x39ac74ac` friendly).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TabStates {
    pub hostile: TabState,
    pub friendly: TabState,
}

impl TabStates {
    pub fn new() -> Self {
        Self { hostile: TabState::new(STATE_HOSTILE), friendly: TabState::new(STATE_FRIENDLY) }
    }

    pub fn state(&mut self, mode: TabMode) -> &mut TabState {
        match mode {
            TabMode::Hostile => &mut self.hostile,
            TabMode::Friendly => &mut self.friendly,
        }
    }
}

/// Result of the tab entry: the selected id and whether a `CSChangeTarget` is requested.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TabOutcome {
    pub target: Option<u32>,
    pub request_change: bool,
    /// UI event id: `2` (`TARGET_CHANGED`) when a target was selected.
    pub event: Option<u32>,
}

/// `FUN_39368630`: tab-target entry.  The byte is the state selector (0 = hostile, 1 =
/// friendly); the target is written locally (`FUN_39055fd0`) and the change is requested with
/// `FUN_39361bf0` → `CSChangeTarget` (0x2c), then UI event 2 is fired.
pub fn tab_select<F>(
    mode: TabMode,
    states: &mut TabStates,
    cfg: &TargetingConfig,
    now: f32,
    local: &TabActor,
    has_local: bool,
    mut collect: F,
) -> TabOutcome
where
    F: FnMut(&TabState) -> Vec<Candidate>,
{
    if !has_local {
        return TabOutcome::default();
    }
    // The tab entry never selects the local player itself; the native path removes it from the
    // candidate list before the first pick.
    let local_id = local.id;
    let selected = select_next(states.state(mode), cfg, now, local_id, |state| {
        collect(state).into_iter().filter(|candidate| candidate.id != local_id).collect()
    });
    match selected {
        Some(id) => TabOutcome { target: Some(id), request_change: true, event: Some(2) },
        None => TabOutcome::default(),
    }
}

// ---------------------------------------------------------------------------------------------
// Target setter and click selection

/// What the target setter has to update.  `FUN_394270a0` writes the 4-byte atom at `+0x1aa0`,
/// refreshes the target and target-of-target frames, and runs the watch/team side effects.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TargetSet {
    pub unit_id: u32,
    pub old_target: u32,
    pub new_target: u32,
    /// The unit is the local player, so the frames/UI paths run.
    pub local: bool,
    /// `FUN_39360020(target)` resolved.
    pub target_present: bool,
    /// target frame event: 2 normal, 3 target-of-target.
    pub frame_events: [Option<u32>; 2],
}

impl TargetSet {
    /// `FUN_394270a0(unitId, targetId)`.  `unit+0x1aa0` is written unconditionally; when a
    /// target exists and the unit is local, the target frame is refreshed (event 2) and the
    /// target-of-target frame only when the target changed (event 3).  The watch/team side
    /// effects (`FUN_39056540`, `FUN_39056430`, `FUN_393871e0`) are reported as flags.
    pub fn apply(&self) -> TargetSetResult {
        TargetSetResult {
            target: self.new_target,
            target_changed: self.old_target != self.new_target,
            request_frames: self.local && self.target_present,
            target_of_target_changed: self.local && self.target_present
                && self.old_target != self.new_target,
            target_side_effects: self.local && self.target_present,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TargetSetResult {
    pub target: u32,
    pub target_changed: bool,
    pub request_frames: bool,
    pub target_of_target_changed: bool,
    pub target_side_effects: bool,
}

/// `FUN_39429e40`: set (or clear) the local target from a click.
///
/// The picked object id/aux id are engine inputs; the guard is
/// `(hover != current || aux != current_aux) && (hover == none || unit(hover) && can_select)`.
/// When `hover` is none the target is cleared.  `FUN_390f4960` only calls this when the mouse
/// point changed or `mouse_clear_targeting` is on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClickSelect {
    pub hover: u32,
    pub hover_aux: u32,
    pub current: u32,
    pub current_aux: u32,
    pub hover_present: bool,
    pub can_select: bool,
    pub mouse_clear_targeting: bool,
}

impl ClickSelect {
    /// Returns the new target (`0` = cleared) or `None` when nothing changes.
    pub fn apply(&self) -> Option<u32> {
        if !self.mouse_clear_targeting
            && self.hover == self.current
            && self.hover_aux == self.current_aux
        {
            return None;
        }
        if self.hover != NO_UNIT && self.hover_present && !self.can_select {
            return None;
        }
        Some(self.hover)
    }
}

// ---------------------------------------------------------------------------------------------
// Combat state, first hit, force attack

/// `unit+0x3bba`; `FUN_390b36f0` only acts when it is 0, `FUN_390b3710` only when it is 1.
/// Both notify the unit vtable `+0x50`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CombatByte {
    pub value: bool,
}

impl CombatByte {
    /// `FUN_390b36f0`: returns whether the vtable notification ran.
    pub fn engage(&mut self) -> bool {
        if self.value {
            return false;
        }
        self.value = true;
        true
    }

    /// `FUN_390b3710`.
    pub fn clear(&mut self) -> bool {
        if !self.value {
            return false;
        }
        self.value = false;
        true
    }
}

/// `FUN_39354780`: write `unit+0x5fe8/+0x5fec`.
pub fn set_first_hit(unit: &mut FirstHit, hitter: u32, team: u32) {
    unit.hitter = hitter;
    unit.team = team;
}

/// The two first-hit fields (`FUN_39354780`, read by `FUN_394bd9f0`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FirstHit {
    /// `+0x5fe8` first-hitter id (`DAT_3a5ab31c` = none).
    pub hitter: u32,
    /// `+0x5fec` first-hitter team/relation (`0` = self, `DAT_39d44c50` = my team).
    pub team: u32,
}

impl FirstHit {
    /// `FUN_394bd9f0 IsFirstHitByMeOrMyTeam`: true when a hitter is recorded and it was either
    /// the local player (`team == 0`) or a member of the local team (`team == MY_TEAM`).
    pub fn is_by_me_or_my_team(&self, local_id: u32) -> bool {
        if self.hitter == NO_FIRST_HIT {
            return false;
        }
        if self.team == 0 {
            self.hitter == local_id
        } else {
            self.team == MY_TEAM
        }
    }
}

/// `FUN_391d3c60` (`SCForceAttackSet`): set `unit+0x3bbc` and fire UI event 0xe8.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ForceAttackSet {
    pub unit_id: u32,
    pub on: bool,
    pub resolved: bool,
}

impl ForceAttackSet {
    pub fn apply(&self) -> Option<u32> {
        if !self.resolved {
            return None;
        }
        Some(0xe8)
    }
}

/// `FUN_39390f20`: Ctrl+F eligibility.  Returns false only when the current selection is the
/// local player's own unit, the special target state is active and the caster is moving.
pub fn can_toggle_force_attack(
    target_is_local: bool,
    moving: bool,
    special_state: bool,
) -> bool {
    if target_is_local && special_state {
        return moving;
    }
    true
}

/// `FUN_39390ec0`: the `CSSetForceAttackPacket` (0x4f) sender.
///
/// The packet body is one bool: `!unit+0x3bbc` (the client sends the *opposite* of the current
/// server-side byte, so a second Ctrl+F turns it off).  Returns the body when a local unit
/// exists.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ForceAttackRequest {
    pub body: [u8; 1],
}

impl ForceAttackRequest {
    pub const OPCODE: u16 = 0x04f;

    pub fn build(force_byte: bool) -> Self {
        Self { body: [if force_byte { 0 } else { 1 }] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_client_cvars() {
        let cfg = TargetingConfig::default();
        assert_eq!(cfg.round_dist, 4.0);
        assert_eq!(cfg.fan_dist, 50.0);
        assert_eq!(cfg.fan_angle_deg, 60.0);
        assert_eq!(cfg.z_limit, 20.0);
        assert_eq!(cfg.history_expire_s, 3.0);
        assert_eq!(cfg.history_max, 10);
    }
}
