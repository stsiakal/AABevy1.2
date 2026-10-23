//! Stage A combat leaves ported from the 1.2 `x2game.dll` decompile.
//!
//! Every item names the original function (`FUN_xxxxxxxx` in `x2game.dll`).  The goldens in
//! `goldens/` were recorded from the running 1.2 client through the client driver
//! (`rust/tools/capture_combat_goldens.py` in the `combat-golden` lane); the tests compare the
//! port with those recordings bit-exactly.
//!
//! Scope of this stage: the inline cooldown manager, the two GCD producers, the caster/target
//! equality leaves, the hit-reaction table and big-hit predicate, the unit-token parser, and the
//! action-slot cooldown resolver behind `GetActionCooldown`/`GetActionCooldownEx`.

#![allow(dead_code, unused_imports)]

pub mod cooldown;
pub mod equality;
pub mod reaction;
pub mod slot;
pub mod token;

#[cfg(test)]
mod tests;

pub use cooldown::{
    CooldownAppend, CooldownEntry, CooldownFull, CooldownManager, MAX_COOLDOWN_ENTRIES,
    SKILL_LIST_TAG, SkillDesc, custom_gcd_ms, default_gcd_ms,
};
pub use equality::{SkillCastTarget, SkillCaster, caster_eq, target_eq};
pub use reaction::{BIG_HIT_THRESHOLD, HitRecordView, Reaction, big_hit, reaction};
pub use slot::{ActionSlot, ItemAction, SlotCooldowns, get_action_cooldown, get_action_cooldown_ex};
pub use token::{
    EMPTY_TARGET_ID, TeamMember, TokenRef, TokenUnit, TokenWorld, TOKEN_KIND_NONE, TOKEN_KIND_OBJECT,
    TOKEN_KIND_TEAM_MEMBER, TOKEN_KIND_UNIT, parse_token,
};
