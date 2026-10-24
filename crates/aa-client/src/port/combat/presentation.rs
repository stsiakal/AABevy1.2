//! Client-side hit presentation and combat engagement.
//!
//! Ported functions:
//!
//! * `FUN_39398c80` — `SCUnitDamaged` dispatch: source-kind switch (skill / item / buff /
//!   passive buff / doodad), skill-record copy, reaction selection and the two presentation
//!   views, then `FUN_39391670` + `FUN_393968b0` + `FUN_39393f30`.
//! * `FUN_393992d0` — `SCUnitHealed` dispatch.
//! * `FUN_39391670` — enter combat for the source, target and (for a player pet) its owner.
//! * `FUN_393968b0` — aggro/threat link for the local player or its pet.
//! * `FUN_39393f30` — the combat-party tracker (`DAT_3a16830c`) first/last hitter fields.
//! * `FUN_3938a3c0` — buff state application and `FUN_39386e00` list append (32 buff /
//!   24 debuff / 24 hidden rows).
//!
//! Engine-only effects (particles, camera shake, floating text layout) are reported as event
//! flags; the goldens record the same flags from argument-recording stubs.  Every decision the
//! client itself makes is ported from the decompile.

use super::reaction::{HitRecordView, Reaction, big_hit, reaction};

/// `SkillResult`-style outcome used by the presentation dispatchers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DispatchOutcome {
    /// The source kind was invalid or the referenced object does not exist.
    Invalid,
    /// The hit was presented.
    Presented,
}

/// The `SCUnitDamaged` record (`0x188` bytes) as `FUN_39398c80` reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DamageRecord {
    /// `+0x00` source kind: 0 skill, 1 item, 2 buff, 3 passive buff, 4 doodad.
    pub source_kind: i32,
    /// `+0x04` source type id (buff/doodad/skill-instance id depending on the kind).
    pub source_type: u32,
    /// `+0x08` skill key / buff id / second id.
    pub key: u32,
    /// `+0x18` source block kind (`SkillCaster` type).
    pub caster_kind: i32,
    /// `+0x20` source unit id for caster kinds 0/3.
    pub source_unit: u32,
    /// `+0x2c` source unit id for caster kind 2.
    pub source_unit2: u32,
    /// `+0x3c` target unit id.
    pub target_id: u32,
    /// `+0x44` hit-type/aux word used by the buff branch.
    pub aux: i32,
    /// `+0x48` hit type (`FUN_393917f0` table).
    pub hit_type: i32,
    /// `+0x4c` absorbed.
    pub absorbed: i32,
    /// `+0x50` damage (also the "is this a real hit" gate of `FUN_39391670`).
    pub damage: i32,
    /// `+0x54` mana burn.
    pub mana_burn: i32,
    /// `+0x6e` engage-combat flag.
    pub engage: bool,
    /// `+0xdc` notification/combat-text flag.
    pub text_flag: bool,
    /// `SkillDesc+0x188` big-hit value (0 when the skill desc is not available).
    pub desc_big_hit: i32,
    /// The game-client mode (`game vtable+4`), `1` selects the victim view.
    pub client_mode: i32,
    /// The target unit's crit/skill column (`+0x5fc4`).
    pub critical: bool,
    /// The target unit carries a `0x10` weapon (`FUN_397cd5f0`).
    pub weapon: bool,
}

impl DamageRecord {
    pub fn source_unit_id(&self) -> u32 {
        match self.caster_kind {
            0 | 3 => self.source_unit,
            2 => self.source_unit2,
            _ => 0,
        }
    }

    pub fn view(&self) -> HitRecordView {
        HitRecordView {
            hit_type: self.hit_type,
            caster_kind: self.caster_kind as u32,
            caster_id_20: self.source_unit,
            caster_id_2c: self.source_unit2,
            target_id: self.target_id,
        }
    }
}

/// The `SCUnitHealed` record as `FUN_393992d0` reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HealRecord {
    pub source_kind: i32,
    pub source_type: u32,
    pub key: u32,
    pub caster_kind: i32,
    pub source_unit: u32,
    pub source_unit2: u32,
    /// `+0x10` item/skill id for the item branch.
    pub item_id: u32,
    /// `+0x38` target unit id (heal uses `+0x38`, not `+0x3c`).
    pub target_id: u32,
    /// `+0x40` (record dword 0x10) heal-amount? read as `+0x44` in the decompile.
    pub amount: i32,
    /// `+0x44`.
    pub aux: i32,
}

impl HealRecord {
    pub fn source_unit_id(&self) -> u32 {
        match self.caster_kind {
            0 | 3 => self.source_unit,
            2 => self.source_unit2,
            _ => 0,
        }
    }
}

/// What the client resolved for a damage/heal source.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SourceResolution {
    /// The skill instance exists (`FUN_393953a0`) and has a desc (`FUN_396f6390`).
    pub skill_instance: bool,
    pub desc_present: bool,
    /// The buff/passive/doodad template resolved.
    pub template_present: bool,
    /// The buff row resolved (`FUN_39387440`).
    pub buff_row: bool,
    /// The item path resolved (`FUN_39856520`).
    pub item_present: bool,
    /// The source unit exists (`FUN_39360020`).
    pub source_unit: bool,
    /// The target unit exists.
    pub target_unit: bool,
    /// The target is the local player (drives the big-hit UI event and the screen shake).
    pub target_is_local: bool,
}

/// The observable presentation of one damage record.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DamagePresentation {
    pub outcome: Option<DispatchOutcome>,
    /// Source kind accepted by the switch.
    pub kind_handled: bool,
    /// Kind 0: the 0x62-dword record was copied into `instance+0x168`.
    pub record_copied: bool,
    /// `FUN_393917f0` result for the skill/buff/doodad branches.
    pub reaction: Option<Reaction>,
    /// `FUN_39391950`: true selects the attacker-view path.
    pub attacker_view: bool,
    /// `FUN_393985f0` / `FUN_39398880` effect request.
    pub hit_effect: bool,
    pub floating_text: bool,
    /// The local victim "big hit" UI event 0x72 and the camera/UI shake.
    pub big_hit_event: bool,
    pub screen_shake: bool,
    /// `FUN_39391670` ran.
    pub engage: bool,
    /// `FUN_393968b0` ran.
    pub aggro: bool,
    /// `FUN_39393f30` ran.
    pub party_tracker: bool,
}

/// `FUN_39398c80`: decide the damage presentation.
pub fn damage_presentation(record: &DamageRecord, source: &SourceResolution) -> DamagePresentation {
    let mut out = DamagePresentation::default();
    if !source.target_unit {
        out.outcome = None;
        return out;
    }
    let kind_ok = match record.source_kind {
        0 => source.skill_instance && source.desc_present,
        1 => source.item_present,
        2 => source.template_present && source.source_unit && source.buff_row,
        3 => source.template_present,
        4 => source.template_present,
        _ => false,
    };
    if !kind_ok {
        out.outcome = Some(DispatchOutcome::Invalid);
        return out;
    }
    out.kind_handled = true;
    out.outcome = Some(DispatchOutcome::Presented);
    match record.source_kind {
        0 => {
            out.record_copied = true;
            let reaction = reaction(record.hit_type, record.weapon, record.critical);
            out.reaction = Some(reaction);
            out.attacker_view = big_hit(
                &record.view(),
                record.desc_big_hit,
                record.client_mode,
                |_| source.source_unit,
            );
            if out.attacker_view {
                out.floating_text = true;
            } else {
                out.hit_effect = source.source_unit;
            }
        }
        2 => {
            if record.aux == 2 {
                out.reaction = Some(reaction(record.hit_type, record.weapon, record.critical));
            }
            out.hit_effect = true;
        }
        3 => {
            out.hit_effect = true;
        }
        4 => {
            out.hit_effect = true;
            out.reaction = Some(reaction(record.hit_type, record.weapon, record.critical));
        }
        1 => {
            out.hit_effect = true;
        }
        _ => {}
    }
    // The three engagement calls run for every handled kind.
    out.engage = true;
    out.aggro = true;
    out.party_tracker = true;
    if record.target_is_local_placeholder(source) && record.damage != 0 {
        out.big_hit_event = true;
        out.screen_shake = true;
    }
    out
}

impl DamageRecord {
    fn target_is_local_placeholder(&self, source: &SourceResolution) -> bool {
        source.target_is_local
    }
}

/// `FUN_393992d0`: decide the heal presentation.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HealPresentation {
    pub outcome: Option<DispatchOutcome>,
    pub kind_handled: bool,
    pub heal_text: bool,
    /// Source/target combat byte update when the target was already in combat.
    pub engage_target: bool,
    /// `FUN_393958a0` ran.
    pub applied: bool,
}

pub fn heal_presentation(record: &HealRecord, source: &SourceResolution) -> HealPresentation {
    let mut out = HealPresentation::default();
    if !source.target_unit || !source.source_unit && record.source_kind == 0 {
        out.outcome = Some(DispatchOutcome::Invalid);
        return out;
    }
    let kind_ok = match record.source_kind {
        0 => source.skill_instance && source.desc_present,
        1 => source.desc_present,
        2 => source.template_present && source.source_unit && source.buff_row,
        3 => source.template_present,
        4 => source.template_present,
        _ => false,
    };
    if !kind_ok {
        out.outcome = Some(DispatchOutcome::Invalid);
        return out;
    }
    out.kind_handled = true;
    out.outcome = Some(DispatchOutcome::Presented);
    out.heal_text = true;
    out.engage_target = record.aux > 0;
    out.applied = true;
    out
}

// ---------------------------------------------------------------------------------------------
// Combat engagement

/// One unit as the engagement helpers read it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CombatUnit {
    pub id: u32,
    /// `+0x3c26`.
    pub attached: bool,
    /// `+0x3c27`.
    pub rider: bool,
    /// `+0x410`.
    pub kind: i32,
    /// `+0x420` owner/controller id.
    pub owner: u32,
    /// `+0x3bba`.
    pub in_combat: bool,
}

/// `FUN_39391670`: enter combat for the source, target and a player pet/slave owner.
///
/// Returns the units whose `+0x3bba` byte was flipped (the native code calls
/// `FUN_390b36f0` once per unit; repeated calls on the same unit are no-ops).
pub fn engage_combat(
    record_damage: i32,
    engage_flag: bool,
    source: Option<CombatUnit>,
    target: Option<CombatUnit>,
    owner: Option<CombatUnit>,
    local_owner: u32,
) -> Vec<u32> {
    let mut engaged = Vec::new();
    let (Some(mut source), Some(mut target)) = (source, target) else {
        return engaged;
    };
    if record_damage == 0
        || !engage_flag
        || source.attached
        || source.rider
        || target.attached
        || target.rider
    {
        return engaged;
    }
    if !source.in_combat {
        source.in_combat = true;
        engaged.push(source.id);
    }
    if !target.in_combat {
        target.in_combat = true;
        engaged.push(target.id);
    }
    if target.kind == 5
        && target.owner == local_owner
        && let Some(mut owner) = owner
        && !owner.attached
        && !owner.rider
        && !owner.in_combat
    {
        owner.in_combat = true;
        engaged.push(owner.id);
    }
    engaged
}

/// `FUN_393968b0`: the local player/pet aggro link.
///
/// The record's source id is added to the local player's aggro set when the target is the
/// local player or its pet, the source is not the local player, and the target is in combat.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AggroOutcome {
    pub add_source: Option<u32>,
    pub refresh: bool,
}

pub fn aggro_link(
    source_id: u32,
    target: Option<&CombatUnit>,
    local_id: u32,
    local_owner: u32,
) -> AggroOutcome {
    let Some(target) = target else {
        return AggroOutcome::default();
    };
    if source_id == 0 || source_id == local_id {
        return AggroOutcome::default();
    }
    let is_self_or_pet =
        (target.kind == 5 && target.owner == local_owner) || target.id == local_id;
    if !is_self_or_pet || !target.in_combat {
        return AggroOutcome::default();
    }
    AggroOutcome { add_source: Some(source_id), refresh: true }
}

/// One entry of the combat-party tracker (`DAT_3a16830c`, `+0x20` party id,
/// `+0x34` member count, `+0x44/+0x48/+0x4c` first/last hitter ids).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Party {
    pub id: u32,
    pub members: i32,
    pub first: u32,
    pub last: u32,
    pub extra: u32,
}

/// `FUN_39393f30`: update every party's first/last hitter fields.
pub fn party_last_hitter(
    source_id: u32,
    target_id: u32,
    local_id: u32,
    parties: &mut [Party],
) {
    for party in parties {
        if source_id == local_id {
            if target_id != local_id && party.members > 1 {
                party.first = target_id;
            }
        } else if source_id != party.id {
            if target_id != local_id {
                if target_id != party.id {
                    continue;
                }
            } else if party.members > 1 {
                party.last = source_id;
            }
        }
        if target_id == local_id && source_id != local_id && party.members > 1 {
            party.last = source_id;
        }
        if target_id == party.id && source_id != party.id && party.members > 1 {
            party.extra = source_id;
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Buff state

/// One buff list row (`0x68` bytes; only the first dword/id is modelled).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BuffRow {
    pub buff_id: u32,
}

/// The three per-unit buff lists (`+0x20/+0x28`, `+0xd28/+0xd30`, `+0x16f0/+0x16f8`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BuffLists {
    pub buffs: Vec<BuffRow>,
    pub debuffs: Vec<BuffRow>,
    pub hidden: Vec<BuffRow>,
}

pub const MAX_BUFFS: usize = 0x20;
pub const MAX_DEBUFFS: usize = 0x18;
pub const MAX_HIDDEN: usize = 0x18;

/// `FUN_39386e00`: reserve the next row in the list selected by `buff_desc+0xf4`.
///
/// Returns `(list, index)` or `None` when that list is full.
pub fn append_buff(lists: &mut BuffLists, desc_kind: i32, buff_id: u32) -> Option<BuffListKind> {
    let (list, limit, kind) = match desc_kind {
        1 => (&mut lists.buffs, MAX_BUFFS, BuffListKind::Buff),
        2 => (&mut lists.debuffs, MAX_DEBUFFS, BuffListKind::Debuff),
        3 => (&mut lists.hidden, MAX_HIDDEN, BuffListKind::Hidden),
        _ => return None,
    };
    if list.len() >= limit {
        return None;
    }
    list.push(BuffRow { buff_id });
    Some(kind)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuffListKind {
    Buff,
    Debuff,
    Hidden,
}

/// `FUN_3938a3c0` inputs as the client resolves them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BuffApply {
    /// `buff_desc+0xf4` (1 buff, 2 debuff, 3 hidden).
    pub desc_kind: i32,
    pub buff_id: u32,
    /// The packet's stack/charge argument.
    pub stack: i32,
    /// `FUN_393875b0(desc+0xf4, stack)` — true selects the stacked/fresh entry path.
    pub stacked: bool,
    /// `FUN_3989e080` returned an entry.
    pub entry_created: bool,
    /// `FUN_39389440` ran (attributes applied).
    pub attrs_applied: bool,
}

/// `FUN_3938a3c0` outcome.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BuffApplyOutcome {
    pub entry_created: bool,
    /// `FUN_39386e00` appended the row (kind 3 never appends).
    pub list_slot: Option<BuffListKind>,
    /// UI event `0x52 + (desc_kind != 1)`.
    pub ui_event: Option<u32>,
    /// `0x29` when the stacked buffer also requested the item/consume event.
    pub extra_event: Option<u32>,
    /// `FUN_39389440` + the three list refreshers.
    pub attrs_applied: bool,
    pub lists_refreshed: bool,
}

pub fn apply_buff(input: &BuffApply) -> BuffApplyOutcome {
    let mut out = BuffApplyOutcome::default();
    if !input.entry_created {
        return out;
    }
    out.entry_created = true;
    out.ui_event = Some(0x52 + u32::from(input.desc_kind != 1));
    out.attrs_applied = input.attrs_applied;
    out.lists_refreshed = true;
    if input.desc_kind == 3 {
        return out;
    }
    if input.stacked && input.desc_kind == 2 {
        out.extra_event = Some(0x29);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::port::combat::targeting::CombatByte;

    #[test]
    fn first_hit_and_engage_bits() {
        let mut unit = CombatByte::default();
        assert!(unit.engage());
        assert!(!unit.engage());
        assert!(unit.clear());
        assert!(!unit.clear());
    }

    #[test]
    fn buff_append_uses_the_three_lists() {
        let mut lists = BuffLists::default();
        assert_eq!(append_buff(&mut lists, 1, 1), Some(BuffListKind::Buff));
        assert_eq!(append_buff(&mut lists, 2, 2), Some(BuffListKind::Debuff));
        assert_eq!(append_buff(&mut lists, 3, 3), Some(BuffListKind::Hidden));
        assert_eq!(append_buff(&mut lists, 4, 4), None);
    }
}
