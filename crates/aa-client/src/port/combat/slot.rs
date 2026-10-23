//! Action-slot cooldown queries.
//!
//! The two script bindings are thin UI wrappers:
//!
//! * `FUN_3943f310` (`X2Action:GetActionCooldown(slot)`) resolves the slot and passes the
//!   **global/skill max pair** to `FUN_3905aca0`, which invokes the Lua callback with
//!   `(remaining, total)`.
//! * `FUN_3943f420` (`GetActionCooldownEx(slot)`) calls the resolver `FUN_393f61c0` for four
//!   values and passes them to `FUN_3943ecb0`: `(global total, global remaining, skill total,
//!   skill remaining)` in the decompiled call order.  The port returns them as
//!   [`SlotCooldowns`] pairs instead of invoking a callback.
//!
//! `FUN_393f61c0` (VA `0x393f61c0`, 283 bytes) resolves a slot kind (`FUN_390898b0`, slot `+0`):
//!
//! * kind 1 (item): slot id at `+8` -> item object (`FUN_39695dc0`) -> skill id at item `+0x98`
//!   (`FUN_396f6390`); if that desc exists, its global pair (`FUN_3989a3e0`) and skill/tag pair
//!   (`FUN_3989a290(..., 1)`) are written.  If the item kind (`item+0x48`) is 9,
//!   `FUN_393a6d90` overwrites the skill pair with the item-use cooldown.
//! * kind 2 (spell): slot id -> `FUN_393925c0` target override -> desc; both pairs as above.
//! * any other kind (or no local player) returns 0 and leaves the pairs zero.
//!
//! Goldens call `FUN_393f61c0` directly; the seeding of the synthetic cooldown manager happens
//! inside one trampoline so the live per-frame tick cannot change the values between seed and
//! query.

use super::cooldown::{CooldownManager, SkillDesc};

/// The two fields of an action slot record (`0x10` bytes): `+0` kind, `+8` id.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ActionSlot {
    pub kind: u32,
    pub id: u32,
}

/// Item-specific inputs for the kind-1 branch: the skill id at item `+0x98`, the kind at
/// `+0x48`, and the item-use cooldown pair produced by `FUN_393a6d90` when that kind is 9.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ItemAction {
    pub skill_id: u32,
    pub kind: u32,
    pub cooldown: Option<(u32, u32)>,
}

/// The resolver's four values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SlotCooldowns {
    /// `FUN_3989a3e0` (global + tag + skill max).
    pub global: (u32, u32),
    /// `FUN_3989a290(desc, 1)` (tag + skill max only).
    pub skill: (u32, u32),
    /// The resolver's return value: 1 for a handled item/spell slot, 0 otherwise.
    pub handled: bool,
}

/// `FUN_393f61c0` as a pure function of the manager, the slot and the lookups.
pub fn slot_cooldowns(
    manager: &CooldownManager,
    slot: ActionSlot,
    mut skill_desc: impl FnMut(u32) -> Option<SkillDesc>,
    mut item_action: impl FnMut(u32) -> Option<ItemAction>,
    mut override_skill: impl FnMut(u32) -> u32,
) -> SlotCooldowns {
    let mut out = SlotCooldowns::default();
    match slot.kind {
        1 => {
            let Some(item) = item_action(slot.id) else {
                return out;
            };
            if let Some(desc) = skill_desc(item.skill_id) {
                out.global = manager.global_pair(&desc);
                out.skill = manager.query(&desc, true);
            }
            if item.kind == 9 && let Some(pair) = item.cooldown {
                out.skill = pair;
            }
            out.handled = true;
        }
        2 => {
            let id = override_skill(slot.id);
            if let Some(desc) = skill_desc(id) {
                out.global = manager.global_pair(&desc);
                out.skill = manager.query(&desc, true);
            }
            out.handled = true;
        }
        _ => {}
    }
    out
}

/// `FUN_3943f310`: the single pair passed to the Lua callback.
pub fn get_action_cooldown(
    manager: &CooldownManager,
    slot: ActionSlot,
    skill_desc: impl FnMut(u32) -> Option<SkillDesc>,
    item_action: impl FnMut(u32) -> Option<ItemAction>,
    override_skill: impl FnMut(u32) -> u32,
) -> (u32, u32) {
    slot_cooldowns(manager, slot, skill_desc, item_action, override_skill).global
}

/// `FUN_3943f420`: the four values passed to the Lua callback.
pub fn get_action_cooldown_ex(
    manager: &CooldownManager,
    slot: ActionSlot,
    skill_desc: impl FnMut(u32) -> Option<SkillDesc>,
    item_action: impl FnMut(u32) -> Option<ItemAction>,
    override_skill: impl FnMut(u32) -> u32,
) -> SlotCooldowns {
    slot_cooldowns(manager, slot, skill_desc, item_action, override_skill)
}
