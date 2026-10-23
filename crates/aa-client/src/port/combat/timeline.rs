//! The 1.2 client cast timeline: start, fire, end, stop, casting stop/delay, combo and the
//! client-only skill list.
//!
//! | Rust | original | VA |
//! |---|---|---|
//! | [`TimelineWorld::start_skill`] | `StartSkill` | `FUN_3939a810` |
//! | [`TimelineWorld::fire_skill`] | `FireSkill` | `FUN_3939b010` |
//! | [`TimelineWorld::end_skill`] | `EndSkill` | `FUN_39397ff0` |
//! | [`TimelineWorld::stop_skill`] | `StopSkill` | `FUN_39398410` |
//! | [`TimelineWorld::stop_casting`] | `StopCasting` | `FUN_39395b50` |
//! | [`TimelineWorld::casting_stopped`] | `SCCastingStopped` impl | `FUN_39396000` |
//! | [`TimelineWorld::casting_delayed`] | cast delay | `FUN_39395760` |
//! | [`TimelineWorld::combo_add`] | `SkillComboMan::Add` | `FUN_3939ce90` |
//! | [`TimelineWorld::combo_update`] | `SkillComboMan::Update` | `FUN_3939cb80` |
//! | [`TimelineWorld::update_client_only`] | `UpdateClientOnlySkills` | `FUN_3939bfd0` |
//!
//! Only the timeline state is modelled: unit cast slots, the target-keyed instance map, the
//! cooldown manager (stage 1), the combo vector and the client-only list.  Animation, camera and
//! controller calls are not represented.  The GCD producer is stage 1; [`TimelineWorld::start_skill`]
//! takes the already computed GCD in milliseconds.

use std::collections::BTreeMap;

use super::cooldown::{CooldownManager, SkillDesc};
use super::equality::{SkillCaster, SkillCastTarget};

/// The no-target sentinel (`DAT_3a177710`, static value 0).
pub const NO_TARGET: u32 = 0;
/// The auto-attack pseudo skill id (`DAT_39ac769c`, static value 2).
pub const AUTO_ATTACK_SKILL: u32 = 2;
/// UI event ids emitted by the timeline (`FUN_39202db0`).
pub const EVENT_SKILL_STARTED: u32 = 0x14;
pub const EVENT_SKILL_FIRED: u32 = 0x15;
pub const EVENT_SKILL_ENDED: u32 = 0x16;
pub const EVENT_AUTO_ATTACK_STOPPED: u32 = 0x17;
pub const EVENT_CASTING_STOPPED: u32 = 0x18;

/// One controller row from `SkillDesc+0x1e8..+0x1ec` (`FUN_3939c950` reads kind `0x18` and
/// sub-kind `0x30`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TimelineController {
    pub kind: i32,
    /// The sub-values read by the timeline (`values[0]` is the sub-kind).
    pub values: Vec<i32>,
}

/// The `SkillDesc` fields the timeline reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimelineDesc {
    pub id: u32,
    /// `+0x48`: cast time (ms) used by the script/client-only skill start.
    pub cast_time: i32,
    /// `+0x6c`: channel time (ms); `> 0` starts the channel/ticks.
    pub channel_time: i32,
    /// `+0x84`: cooldown tag id (`0` = skill list).
    pub cooldown_tag_id: u32,
    /// `+0xdc`: ignore the global cooldown in cooldown queries.
    pub ignore_global_cooldown: bool,
    /// `+0x9c`: custom-GCD-valid flag (the GCD itself is stage 1).
    pub custom_gcd_valid: bool,
    /// `+0x94`: custom GCD (ms).
    pub custom_gcd: i32,
    /// `+0x1b0`: target selection type.
    pub target_type: i32,
    /// `+0x1bc`: default-GCD flag.
    pub default_gcd: bool,
    /// `+0x13a`: show the cast bar / target casting time.
    pub show_cast_bar: bool,
    /// `+0xe8`: skill-category index (-1 = none).
    pub category_index: i32,
    /// `+0xe4`: skill-category id.
    pub category_id: i32,
    /// `+0x1e8..+0x1ec`: controller rows.
    pub controllers: Vec<TimelineController>,
}

impl TimelineDesc {
    pub fn cooldown_desc(&self) -> SkillDesc {
        SkillDesc {
            id: self.id,
            cooldown_tag_id: self.cooldown_tag_id,
            ignore_global_cooldown: self.ignore_global_cooldown,
            big_hit: 0,
            tags: Vec::new(),
        }
    }
}

/// The per-unit cast state and cooldown manager (`Unit+0x2c48`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TimelineUnit {
    pub id: u32,
    pub cast_key: u32,
    pub cast_delay_key: u32,
    pub channel_key: u32,
    pub channel_delay_key: u32,
    pub auto_attack: bool,
    pub cooldowns: CooldownManager,
}

/// The target-keyed skill instance (`FUN_393953a0(key)`, map value `+0x18`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillInstance {
    pub key: u32,
    pub skill_id: u32,
    pub caster: SkillCaster,
    pub target: SkillCastTarget,
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

/// One `SkillComboMan` entry (0x18 bytes).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ComboEntry {
    /// `+0x00`: the current skill id (the entry key).
    pub key: u32,
    /// `+0x04`: duplicated current skill id.
    pub current_skill_id: u32,
    /// `+0x08`: next combo skill id.
    pub next_skill_id: u32,
    /// `+0x0c`: start (engine ms).
    pub start: u32,
    /// `+0x10`: duration (ms).
    pub duration: u32,
    /// `+0x14`: expiry margin (ms).
    pub margin: u32,
}

/// A client-only skill (`ClientSkill`, 0x2e0 bytes) reduced to the fields the update reads.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClientSkill {
    pub skill_id: u32,
    pub target_key: u32,
    /// `+0x138`: duration / normal total.
    pub normal_total: u32,
    /// `+0x13c`: channel/other total.
    pub computed_delay: u32,
    /// `+0x140`: second timing value.
    pub channel_total: u32,
    /// `+0x2d0`: fire timestamp.
    pub fire_time: u32,
    /// `+0x2d4`: end timestamp.
    pub end_time: u32,
    /// `+0x2d8`: fired flag.
    pub fired: bool,
    /// `+0x2d9`: local-damage done flag.
    pub damaged: bool,
    /// `+0x2da`: apply local damage at `end_time`.
    pub local_damage: bool,
}

/// Why `StartSkill` did or did not create an instance (the original only logs the failures).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartOutcome {
    Started,
    /// The server-mirror early path: UI event only, no instance.
    Mirrored,
    /// `DAT_39ac769c` with the no-target sentinel armed the auto-attack byte.
    AutoAttack,
    /// No-target with a non-auto-attack skill: the "impossible case".
    ImpossibleNoTarget,
    /// `FUN_396f6390` returned no desc.
    InvalidSkill,
    /// The caster unit could not be resolved and the early path created nothing.
    NoCaster,
}

/// The world state the timeline functions mutate.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TimelineWorld {
    pub descs: BTreeMap<u32, TimelineDesc>,
    pub units: BTreeMap<u32, TimelineUnit>,
    pub instances: BTreeMap<u32, SkillInstance>,
    pub combo: Vec<ComboEntry>,
    pub client_only: Vec<ClientSkill>,
    pub events: Vec<u32>,
    pub now: u32,
}

impl TimelineWorld {
    fn push_event(&mut self, id: u32) {
        self.events.push(id);
    }

    fn unit_id(caster: &SkillCaster) -> Option<u32> {
        match caster.kind {
            0 | 3 => Some(caster.words[0]),
            2 => Some(caster.words[3]),
            _ => None,
        }
    }

    fn unit_mut(&mut self, id: u32) -> Option<&mut TimelineUnit> {
        self.units.get_mut(&id)
    }

    fn clear_slots_for(&mut self, unit_id: u32, key: u32) {
        if let Some(unit) = self.units.get_mut(&unit_id) {
            if unit.cast_key == key {
                unit.cast_key = 0;
                unit.cast_delay_key = 0;
            }
            if unit.channel_key == key {
                unit.channel_key = 0;
                unit.channel_delay_key = 0;
            }
        }
    }

    /// `FUN_3939a810 StartSkill`.
    ///
    /// `gcd_ms` is the value the stage-1 producer wrote (`None` keeps the manager untouched,
    /// which is what the server-mirror and failure paths do).
    #[allow(clippy::too_many_arguments)]
    pub fn start_skill(
        &mut self,
        caster: &SkillCaster,
        target: &SkillCastTarget,
        skill_id: u32,
        target_key: u32,
        cast_time: u32,
        mirror: bool,
        gcd_ms: Option<u32>,
    ) -> StartOutcome {
        let caster_id = Self::unit_id(caster);
        let now = self.now;
        // Stop any previous instance for this target key (the original calls EndSkill).
        if let Some(caster_id) = self
            .instances
            .remove(&target_key)
            .and_then(|old| Self::unit_id(&old.caster))
        {
            self.clear_slots_for(caster_id, target_key);
        }
        if mirror {
            self.push_event(EVENT_SKILL_STARTED);
            return StartOutcome::Mirrored;
        }
        let Some(desc) = self.descs.get(&skill_id).cloned() else {
            return StartOutcome::InvalidSkill;
        };
        let Some(caster_id) = caster_id.filter(|id| self.units.contains_key(id)) else {
            return StartOutcome::NoCaster;
        };
        if target_key == NO_TARGET {
            if skill_id == AUTO_ATTACK_SKILL {
                if let Some(unit) = self.unit_mut(caster_id) {
                    unit.auto_attack = true;
                }
                return StartOutcome::AutoAttack;
            }
            return StartOutcome::ImpossibleNoTarget;
        }
        if let (Some(gcd), Some(unit)) = (gcd_ms, self.unit_mut(caster_id)) {
            unit.cooldowns.gcd_remaining_ms = gcd;
            unit.cooldowns.gcd_total_ms = gcd;
        }
        self.instances.insert(
            target_key,
            SkillInstance {
                key: target_key,
                skill_id: desc.id,
                caster: *caster,
                target: *target,
                normal_total: cast_time,
                computed_delay: 0,
                channel_total: 0,
                normal_start: now,
                channel_start: 0,
            },
        );
        if cast_time != 0 && let Some(unit) = self.unit_mut(caster_id) {
            unit.cast_key = target_key;
        }
        self.push_event(EVENT_SKILL_STARTED);
        StartOutcome::Started
    }

    /// `FUN_3939b010 FireSkill`.
    ///
    /// `error_flag` is the packet's non-zero skill-result byte; it stops the cast instead of
    /// firing.  `time7` lands at `+0x13c`, `time9` at `+0x140`, and the current engine time at
    /// `+0x148`.
    #[allow(clippy::too_many_arguments)]
    pub fn fire_skill(
        &mut self,
        caster: &SkillCaster,
        target: &SkillCastTarget,
        skill_id: u32,
        target_key: u32,
        time7: u32,
        time9: u32,
        error_flag: bool,
        gcd_ms: Option<u32>,
    ) {
        if error_flag {
            self.stop_casting(target_key);
            return;
        }
        let existing_ok = self
            .instances
            .get(&target_key)
            .map(|inst| {
                inst.skill_id == skill_id
                    && super::equality::caster_eq(&inst.caster, caster)
                    && super::equality::target_eq(&inst.target, target)
            })
            .unwrap_or(false);
        if !existing_ok {
            if self.instances.contains_key(&target_key) {
                self.end_skill(target_key);
            }
            self.start_skill(caster, target, skill_id, target_key, 0, false, gcd_ms);
        }
        let now = self.now;
        let Some(inst) = self.instances.get_mut(&target_key) else {
            return;
        };
        inst.computed_delay = time7;
        inst.channel_total = time9;
        inst.channel_start = now;
        let normal_total = inst.normal_total;
        let caster_id = Self::unit_id(&inst.caster);
        if normal_total != 0 && let Some(unit) = caster_id.and_then(|id| self.units.get_mut(&id)) {
            unit.cast_key = 0;
            unit.cast_delay_key = 0;
        }
        let channel_time = self
            .descs
            .get(&inst.skill_id)
            .map(|desc| desc.channel_time)
            .unwrap_or(0);
        if channel_time > 0 && let Some(unit) = caster_id.and_then(|id| self.units.get_mut(&id)) {
            unit.channel_key = target_key;
        }
        self.push_event(EVENT_SKILL_FIRED);
    }

    /// `FUN_39397ff0 EndSkill`: remove the instance, clear the matching unit slot, always emit
    /// event `0x16`.
    pub fn end_skill(&mut self, key: u32) {
        if let Some(caster_id) = self
            .instances
            .remove(&key)
            .and_then(|inst| Self::unit_id(&inst.caster))
        {
            self.clear_slots_for(caster_id, key);
        }
        self.push_event(EVENT_SKILL_ENDED);
    }

    /// `FUN_39398410 StopSkill`: only the auto-attack pseudo skill.
    pub fn stop_skill(&mut self, unit_id: u32, skill_id: u32) {
        if skill_id != AUTO_ATTACK_SKILL {
            return;
        }
        if self.units.get(&unit_id).map(|unit| unit.auto_attack) == Some(true) {
            if let Some(unit) = self.units.get_mut(&unit_id) {
                unit.auto_attack = false;
            }
            self.push_event(EVENT_AUTO_ATTACK_STOPPED);
        }
    }

    /// `FUN_39395b50 StopCasting`: clear the slot that points at this key.  Returns whether a
    /// slot matched (the original returns 0 otherwise).
    pub fn stop_casting(&mut self, key: u32) -> bool {
        let Some(inst) = self.instances.get(&key) else {
            return false;
        };
        let Some(caster_id) = Self::unit_id(&inst.caster) else {
            return false;
        };
        let Some(unit) = self.units.get_mut(&caster_id) else {
            return false;
        };
        if key == unit.cast_key {
            unit.cast_key = 0;
            unit.cast_delay_key = 0;
            return true;
        }
        if key == unit.channel_key {
            unit.channel_key = 0;
            unit.channel_delay_key = 0;
            return true;
        }
        false
    }

    /// `FUN_39396000` (`SCCastingStopped`): stop, then add the packet duration as the skill
    /// cooldown and emit event `0x18`.
    pub fn casting_stopped(&mut self, key: u32, duration_ms: u32) {
        let Some(inst) = self.instances.get(&key) else {
            return;
        };
        let Some(caster_id) = Self::unit_id(&inst.caster) else {
            return;
        };
        let skill_id = inst.skill_id;
        if !self.units.contains_key(&caster_id) {
            return;
        }
        self.stop_casting(key);
        if duration_ms != 0
            && let (Some(desc), Some(unit)) =
                (self.descs.get(&skill_id).cloned(), self.units.get_mut(&caster_id))
        {
            unit.cooldowns.insert(false, &desc.cooldown_desc(), duration_ms);
        }
        self.push_event(EVENT_CASTING_STOPPED);
    }

    /// `FUN_39395760` cast delay: add `duration_ms` to the normal cast total while the cast is
    /// still running (`now < start + total`).
    pub fn casting_delayed(&mut self, key: u32, duration_ms: u32) {
        let Some(inst) = self.instances.get_mut(&key) else {
            return;
        };
        let sum = inst.normal_start.wrapping_add(inst.normal_total);
        let elapsed = sum.min(self.now);
        if sum != elapsed {
            inst.normal_total = inst.normal_total.wrapping_add(duration_ms);
        }
    }

    /// `FUN_3939ce90 SkillComboMan::Add`.
    ///
    /// The current skill's controller kind `0x18`, sub-kind `0x30` supplies
    /// `values[1]` (next skill id) and `values[2]` (margin).  The next skill must have a desc;
    /// otherwise the original returns without changing the vector.
    pub fn combo_add(&mut self, unit_id: u32, desc: &TimelineDesc, duration_ms: u32) -> bool {
        let Some(values) = desc
            .controllers
            .iter()
            .find(|c| c.kind == 0x18 && c.values.first() == Some(&0x30))
            .map(|c| c.values.clone())
        else {
            return false;
        };
        let (Some(next_skill), Some(margin)) = (values.get(1).copied(), values.get(2).copied())
        else {
            return false;
        };
        let Some(next_desc) = self.descs.get(&(next_skill as u32)).cloned() else {
            return false;
        };
        let now = self.now;
        if let Some(entry) = self.combo.iter_mut().find(|entry| entry.key == desc.id) {
            entry.current_skill_id = desc.id;
            entry.next_skill_id = next_skill as u32;
            entry.start = now;
            entry.duration = duration_ms;
            entry.margin = margin as u32;
        } else {
            self.combo.push(ComboEntry {
                key: desc.id,
                current_skill_id: desc.id,
                next_skill_id: next_skill as u32,
                start: now,
                duration: duration_ms,
                margin: margin as u32,
            });
        }
        if let Some(unit) = self.units.get_mut(&unit_id) {
            unit.cooldowns.clear_skill(next_skill as u32);
            unit.cooldowns.insert(false, &next_desc.cooldown_desc(), duration_ms);
        }
        true
    }

    /// `FUN_3939cb80 SkillComboMan::Update`: drop entries whose
    /// `start + duration + margin` has passed and clear the next skill's cooldown.
    pub fn combo_update(&mut self) {
        let now = self.now;
        let mut cleared = Vec::new();
        self.combo.retain(|entry| {
            if now >= entry.start.wrapping_add(entry.duration).wrapping_add(entry.margin) {
                cleared.push(entry.next_skill_id);
                false
            } else {
                true
            }
        });
        if let Some(unit) = self.units.values_mut().next() {
            for skill_id in cleared {
                unit.cooldowns.clear_skill(skill_id);
            }
        }
    }

    /// `FUN_3939bfd0 UpdateClientOnlySkills`: fire at `+0x2d0`, mark the local damage at
    /// `+0x2d4`, and end/remove after `+0x2d4 + 1000 ms`.
    pub fn update_client_only(&mut self) {
        let now = self.now;
        let local_id = self.local_player_id();
        let mut fire = Vec::new();
        let mut remove = Vec::new();
        for (index, skill) in self.client_only.iter_mut().enumerate() {
            if !skill.fired {
                if skill.fire_time < now {
                    skill.fired = true;
                    fire.push((index, skill.skill_id, skill.target_key, skill.normal_total));
                    if skill.fire_time == skill.end_time {
                        continue;
                    }
                }
            } else if !skill.damaged {
                if skill.end_time < now {
                    skill.damaged = true;
                }
            } else if now > skill.end_time.wrapping_add(1000) {
                remove.push(index);
            }
        }
        for (index, skill_id, target_key, normal_total) in fire {
            let caster = SkillCaster {
                kind: 0,
                pad: 0,
                words: [local_id, 0, 0, 0, 0, 0, 0],
            };
            let target = SkillCastTarget {
                kind: 0,
                pad: 0,
                words: [target_key, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            };
            if !self.instances.contains_key(&target_key) {
                self.start_skill(
                    &caster,
                    &target,
                    skill_id,
                    target_key,
                    normal_total,
                    false,
                    None,
                );
            }
            if let Some(inst) = self.instances.get_mut(&target_key) {
                inst.computed_delay = self.client_only[index].computed_delay;
                inst.channel_total = self.client_only[index].channel_total;
                inst.channel_start = now;
            }
            self.push_event(EVENT_SKILL_FIRED);
        }
        for index in remove.into_iter().rev() {
            let skill = self.client_only.remove(index);
            self.end_skill(skill.target_key);
        }
    }

    fn local_player_id(&self) -> u32 {
        self.units.keys().next().copied().unwrap_or(0)
    }
}

/// A small helper for the goldens: `sqrt`-free integer GCD total check.
#[cfg(test)]
fn gcd_total(world: &TimelineWorld, unit_id: u32) -> u32 {
    world
        .units
        .get(&unit_id)
        .map(|unit| unit.cooldowns.gcd_total_ms)
        .unwrap_or(0)
}
