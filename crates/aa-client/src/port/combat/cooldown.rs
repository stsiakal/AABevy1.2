//! The client cooldown manager inline in `Unit+0x2c48`.
//!
//! Original functions (all `__thiscall` on the manager unless noted):
//!
//! | Rust | original | VA |
//! |---|---|---|
//! | [`CooldownManager::clear_global`] | `FUN_39899d00` | `0x39899d00` |
//! | [`CooldownManager::reset`] | `FUN_39899d10` | `0x39899d10` |
//! | [`CooldownManager::append`] | `FUN_39899d40` | `0x39899d40` |
//! | [`custom_gcd_ms`] / [`CooldownManager::set_custom_gcd`] | `FUN_39899df0` | `0x39899df0` |
//! | [`CooldownManager::tick`] | `FUN_39899e50` | `0x39899e50` |
//! | [`CooldownManager::clear_skill`] | `FUN_39899ec0` | `0x39899ec0` |
//! | [`CooldownManager::clear_tag`] | `FUN_39899f00` | `0x39899f00` |
//! | [`default_gcd_ms`] / [`CooldownManager::set_default_gcd`] | `FUN_39899f30` | `0x39899f30` |
//! | [`CooldownManager::insert`] | `FUN_39899f90` | `0x39899f90` |
//! | [`CooldownManager::query`] | `FUN_3989a290` | `0x3989a290` |
//! | [`CooldownManager::is_on_cooldown`] | `FUN_3989a3b0` | `0x3989a3b0` |
//! | [`CooldownManager::global_pair`] | `FUN_3989a3e0` | `0x3989a3e0` |
//! | [`CooldownManager::state`] | `FUN_3989a400` | `0x3989a400` |
//! | [`CooldownManager::remaining`] | `FUN_3989a450` | `0x3989a450` |
//!
//! Layout, proved by `lea ecx,[esi+0x2c48]` at `0x3939aab9`/`0x3939aad7` and by the accesses in
//! `FUN_39899d10`/`FUN_39899d40`:
//!
//! ```text
//! +0x000 owner*        +0x004 unitId
//! +0x008 gcdRemaining  +0x00c gcdTotal
//! +0x010 tagCount      +0x014 tags[150]   (id, remaining, total)
//! +0x71c skillCount    +0x720 skills[150] (id, remaining, total)
//! ```
//!
//! All times are unsigned 32-bit milliseconds.  The decompiler prints them as `int`/`uint`; every
//! comparison (`jbe`, `ja`) and the tick subtraction treat them as unsigned, so the Rust fields
//! are `u32` (a packet may still carry a negative-looking value, which is the same bit pattern).

use core::mem::{offset_of, size_of};

pub const MAX_COOLDOWN_ENTRIES: usize = 150;
/// `DAT_3a5e12f0` (static `.data` value 0): a `SkillDesc` whose `cooldown_tag_id` is this value is
/// stored in the skill list keyed by skill id, otherwise in the tag list keyed by that tag id.
pub const SKILL_LIST_TAG: u32 = 0;

/// One `tags[]`/`skills[]` row; the original writes the id and remaining as one qword plus the
/// total at `+8`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CooldownEntry {
    pub id: u32,
    pub remaining_ms: u32,
    pub total_ms: u32,
}

/// `CooldownManager` inline at `Unit+0x2c48`, size `0xE28` (the difftest note's `0x1260` is too
/// large; the last member ends at `0x720 + 150*0xc = 0xE28`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CooldownManager {
    pub owner: u32,
    pub unit_id: u32,
    pub gcd_remaining_ms: u32,
    pub gcd_total_ms: u32,
    pub tag_count: i32,
    pub tags: [CooldownEntry; MAX_COOLDOWN_ENTRIES],
    pub skill_count: i32,
    pub skills: [CooldownEntry; MAX_COOLDOWN_ENTRIES],
}

impl Default for CooldownManager {
    fn default() -> Self {
        Self {
            owner: 0,
            unit_id: 0,
            gcd_remaining_ms: 0,
            gcd_total_ms: 0,
            tag_count: 0,
            tags: [CooldownEntry::default(); MAX_COOLDOWN_ENTRIES],
            skill_count: 0,
            skills: [CooldownEntry::default(); MAX_COOLDOWN_ENTRIES],
        }
    }
}

const _: () = {
    assert!(size_of::<CooldownEntry>() == 0xC);
    assert!(offset_of!(CooldownManager, owner) == 0x000);
    assert!(offset_of!(CooldownManager, unit_id) == 0x004);
    assert!(offset_of!(CooldownManager, gcd_remaining_ms) == 0x008);
    assert!(offset_of!(CooldownManager, gcd_total_ms) == 0x00C);
    assert!(offset_of!(CooldownManager, tag_count) == 0x010);
    assert!(offset_of!(CooldownManager, tags) == 0x014);
    assert!(offset_of!(CooldownManager, skill_count) == 0x71C);
    assert!(offset_of!(CooldownManager, skills) == 0x720);
    assert!(size_of::<CooldownManager>() == 0xE28);
};

/// The `SkillDesc` fields this stage reads (offsets are those of the 1.2 `skills` row object).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkillDesc {
    /// `+0x000`: skill id.
    pub id: u32,
    /// `+0x084` (word `0x21`): the cooldown tag id; `0` selects the skill list.
    pub cooldown_tag_id: u32,
    /// `+0x0dc`: skip the global cooldown when starting the query.
    pub ignore_global_cooldown: bool,
    /// `+0x188`: big-hit presentation value (see [`super::reaction::big_hit`]).
    pub big_hit: i32,
    /// `+0x238..+0x23c`: the skill's tag vector (borrowed as a `Vec` here).
    pub tags: Vec<u32>,
}

/// The two lists of an `SCCooldowns` packet, as read by `FUN_39899d40`.
#[derive(Clone, Copy, Debug, Default)]
pub struct CooldownAppend<'a> {
    pub skills: &'a [CooldownEntry],
    pub tags: &'a [CooldownEntry],
}

/// Which list was full when `FUN_39899f90` logged and skipped the insert.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CooldownFull {
    Skill,
    Tag,
}

fn count_slots(count: i32) -> usize {
    count.clamp(0, MAX_COOLDOWN_ENTRIES as i32) as usize
}

/// `FUN_39899d00`: clear the global pair.
impl CooldownManager {
    pub fn clear_global(&mut self) {
        self.gcd_remaining_ms = 0;
        self.gcd_total_ms = 0;
    }

    /// `FUN_39899d10`: set `owner`/`unit_id` and zero the two counts and the global pair.  The
    /// entry arrays are deliberately not cleared (the original does not clear them either).
    pub fn reset(&mut self, owner: u32, unit_id: u32) {
        self.owner = owner;
        self.unit_id = unit_id;
        self.gcd_remaining_ms = 0;
        self.gcd_total_ms = 0;
        self.tag_count = 0;
        self.skill_count = 0;
    }

    /// `FUN_39899d40`: append the packet's skill list and then its tag list, each capped at 150.
    /// No compaction and no owner-disable check in the original.
    pub fn append(&mut self, lists: &CooldownAppend<'_>) {
        let mut skill_count = count_slots(self.skill_count);
        for entry in lists.skills {
            if skill_count >= MAX_COOLDOWN_ENTRIES {
                break;
            }
            self.skills[skill_count] = *entry;
            skill_count += 1;
        }
        self.skill_count = skill_count as i32;

        let mut tag_count = count_slots(self.tag_count);
        for entry in lists.tags {
            if tag_count >= MAX_COOLDOWN_ENTRIES {
                break;
            }
            self.tags[tag_count] = *entry;
            tag_count += 1;
        }
        self.tag_count = tag_count as i32;
    }

    /// `FUN_39899df0`: `custom_gcd_ms * 1000 / (1000 + attr(0x4a))` into `+8/+0xc`.  Does nothing
    /// when the owner's `+0x3af1` disable byte is set.
    pub fn set_custom_gcd(&mut self, owner_disabled: bool, attr: i32, custom_ms: i32) {
        if owner_disabled {
            return;
        }
        let value = custom_gcd_ms(custom_ms, attr);
        self.gcd_remaining_ms = value;
        self.gcd_total_ms = value;
    }

    /// `FUN_39899f30`: `1_000_000 / (1000 + attr(0x4a))` into `+8/+0xc`.
    pub fn set_default_gcd(&mut self, owner_disabled: bool, attr: i32) {
        if owner_disabled {
            return;
        }
        let value = default_gcd_ms(attr);
        self.gcd_remaining_ms = value;
        self.gcd_total_ms = value;
    }

    /// `FUN_39899e50`: subtract `dt` from every remaining and from the global remaining, clamping
    /// at zero.  The comparison is unsigned (`jbe`), exactly `saturating_sub`.
    pub fn tick(&mut self, dt_ms: u32) {
        let tag_count = count_slots(self.tag_count);
        for entry in &mut self.tags[..tag_count] {
            entry.remaining_ms = entry.remaining_ms.saturating_sub(dt_ms);
        }
        let skill_count = count_slots(self.skill_count);
        for entry in &mut self.skills[..skill_count] {
            entry.remaining_ms = entry.remaining_ms.saturating_sub(dt_ms);
        }
        self.gcd_remaining_ms = self.gcd_remaining_ms.saturating_sub(dt_ms);
    }

    /// `FUN_39899ec0`: zero the remaining of the first skill row with this id.
    pub fn clear_skill(&mut self, id: u32) {
        let count = count_slots(self.skill_count);
        if let Some(entry) = self.skills[..count].iter_mut().find(|entry| entry.id == id) {
            entry.remaining_ms = 0;
        }
    }

    /// `FUN_39899f00`: zero the remaining of the first tag row with this id.
    pub fn clear_tag(&mut self, id: u32) {
        let count = count_slots(self.tag_count);
        if let Some(entry) = self.tags[..count].iter_mut().find(|entry| entry.id == id) {
            entry.remaining_ms = 0;
        }
    }

    /// `FUN_39899f90`: insert-or-raise a skill/tag cooldown.
    ///
    /// The list is chosen by `desc.cooldown_tag_id == SKILL_LIST_TAG` (the original compares
    /// `desc+0x84` with `DAT_3a5e12f0`); the skill list is keyed by skill id, the tag list by that
    /// tag id.  When the list is full it is compacted by dropping rows whose remaining reached
    /// zero; if it is still full the original logs and returns without inserting.
    pub fn insert(
        &mut self,
        owner_disabled: bool,
        desc: &SkillDesc,
        duration_ms: u32,
    ) -> Option<CooldownFull> {
        if owner_disabled || duration_ms == 0 {
            return None;
        }
        if desc.cooldown_tag_id == SKILL_LIST_TAG {
            let full = insert_entry(&mut self.skills, &mut self.skill_count, desc.id, duration_ms);
            if full {
                return Some(CooldownFull::Skill);
            }
        } else {
            let full = insert_entry(
                &mut self.tags,
                &mut self.tag_count,
                desc.cooldown_tag_id,
                duration_ms,
            );
            if full {
                return Some(CooldownFull::Tag);
            }
        }
        None
    }

    /// `FUN_3989a290`: the max of the (optional) global pair and the matching tag/skill rows.
    ///
    /// `ignore_global` only skips the global pair; the tag/skill rows are still scanned.  The
    /// update is strictly `>` (the original uses `jbe` to skip equal remaining values).
    pub fn query(&self, desc: &SkillDesc, ignore_global: bool) -> (u32, u32) {
        let (mut remaining, mut total) = if ignore_global {
            (0, 0)
        } else {
            (self.gcd_remaining_ms, self.gcd_total_ms)
        };

        let tag_count = count_slots(self.tag_count);
        for tag in &desc.tags {
            for entry in &self.tags[..tag_count] {
                if entry.id == *tag && remaining < entry.remaining_ms {
                    remaining = entry.remaining_ms;
                    total = entry.total_ms;
                }
            }
        }

        let skill_count = count_slots(self.skill_count);
        for entry in &self.skills[..skill_count] {
            if entry.id == desc.id && remaining < entry.remaining_ms {
                remaining = entry.remaining_ms;
                total = entry.total_ms;
            }
        }
        (remaining, total)
    }

    /// `FUN_3989a3b0`: "is this skill on cooldown" with an explicit global-ignore flag.
    pub fn is_on_cooldown(&self, desc: &SkillDesc, ignore_global: bool) -> bool {
        self.query(desc, ignore_global).0 != 0
    }

    /// `FUN_3989a3e0`: the global pair for a desc, using `desc.ignore_global_cooldown`.
    pub fn global_pair(&self, desc: &SkillDesc) -> (u32, u32) {
        self.query(desc, desc.ignore_global_cooldown)
    }

    /// `FUN_3989a400`: 0 ready, 1 skill/tag cooldown, 2 global cooldown.
    pub fn state(&self, desc: &SkillDesc) -> u8 {
        let (remaining, _) = self.query(desc, true);
        if remaining != 0 {
            return 1;
        }
        if !desc.ignore_global_cooldown && self.gcd_remaining_ms != 0 {
            2
        } else {
            0
        }
    }

    /// `FUN_3989a450`: the remaining of the max pair, using `desc.ignore_global_cooldown`.
    pub fn remaining(&self, desc: &SkillDesc) -> u32 {
        self.query(desc, desc.ignore_global_cooldown).0
    }
}

/// One of the two insert lists.  Returns `true` when the list was full even after compaction.
fn insert_entry(
    entries: &mut [CooldownEntry; MAX_COOLDOWN_ENTRIES],
    count: &mut i32,
    key: u32,
    duration_ms: u32,
) -> bool {
    let current = *count;
    let scanned = count_slots(current);
    for entry in &mut entries[..scanned] {
        if entry.id == key {
            if entry.remaining_ms < duration_ms {
                entry.remaining_ms = duration_ms;
                entry.total_ms = duration_ms;
            }
            return false;
        }
    }

    if current == MAX_COOLDOWN_ENTRIES as i32 {
        let kept = compact_entries(entries, scanned);
        *count = kept as i32;
        if kept == MAX_COOLDOWN_ENTRIES {
            return true;
        }
    }

    let current = *count;
    if current >= 0 && current < MAX_COOLDOWN_ENTRIES as i32 {
        entries[current as usize] = CooldownEntry {
            id: key,
            remaining_ms: duration_ms,
            total_ms: duration_ms,
        };
        *count = current + 1;
    }
    false
}

/// Drop rows whose remaining is zero, preserving the order of the survivors.
fn compact_entries(
    entries: &mut [CooldownEntry; MAX_COOLDOWN_ENTRIES],
    count: usize,
) -> usize {
    let mut kept = 0;
    for i in 0..count {
        if entries[i].remaining_ms != 0 {
            entries[kept] = entries[i];
            kept += 1;
        }
    }
    kept
}

/// `FUN_39899df0` formula: 32-bit signed `custom * 1000`, unsigned 32-bit division by
/// `(1000 + attr)` where an attribute below `-999` is replaced with `-999` (denominator 1).
pub fn custom_gcd_ms(custom_ms: i32, attr: i32) -> u32 {
    let denominator = gcd_denominator(attr);
    custom_ms.wrapping_mul(1000) as u32 / denominator
}

/// `FUN_39899f30` formula: `1_000_000 / (1000 + attr)`, same `-999` floor.
pub fn default_gcd_ms(attr: i32) -> u32 {
    1_000_000 / gcd_denominator(attr)
}

fn gcd_denominator(attr: i32) -> u32 {
    let effective = if attr < -999 { -999 } else { attr };
    effective.wrapping_add(1000) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gcd_denominator_floor_and_wrap() {
        assert_eq!(gcd_denominator(-1000), 1);
        assert_eq!(gcd_denominator(-2000), 1);
        assert_eq!(gcd_denominator(-999), 1);
        assert_eq!(gcd_denominator(-1), 999);
        assert_eq!(gcd_denominator(0), 1000);
        assert_eq!(gcd_denominator(500), 1500);
        assert_eq!(gcd_denominator(i32::MAX), 0x8000_03E7);
    }

    #[test]
    fn custom_gcd_multiply_wraps_like_imul() {
        assert_eq!(custom_gcd_ms(3000, 500), 3000 * 1000 / 1500);
        assert_eq!(custom_gcd_ms(i32::MAX, 0), (i32::MAX.wrapping_mul(1000) as u32) / 1000);
        assert_eq!(custom_gcd_ms(i32::MIN, 0), (i32::MIN.wrapping_mul(1000) as u32) / 1000);
    }

    #[test]
    fn default_gcd_edges() {
        assert_eq!(default_gcd_ms(-12345), 1_000_000);
        assert_eq!(default_gcd_ms(0), 1000);
        assert_eq!(default_gcd_ms(500), 666);
        assert_eq!(default_gcd_ms(i32::MAX), 0);
    }
}
