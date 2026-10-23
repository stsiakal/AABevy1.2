//! Stage A combat leaves ported from the 1.2 `x2game.dll` decompile.
//!
//! Every item names the original function (`FUN_xxxxxxxx` in `x2game.dll`).  The goldens in
//! `goldens/` were recorded from the running 1.2 client through the client driver
//! (`rust/tools/capture_combat_goldens.py` in the `combat-golden` lane); the tests compare the
//! port with those recordings bit-exactly.
//!
//! Scope: the inline cooldown manager, the two GCD producers, the caster/target equality leaves,
//! the hit-reaction table and big-hit predicate, the unit-token parser, the action-slot cooldown
//! resolver, and the read-only cast/combat queries (`cast`).

#![allow(dead_code, unused_imports)]

pub mod cast;
pub mod cooldown;
pub mod equality;
pub mod reaction;
pub mod slot;
pub mod timeline;
pub mod token;

#[cfg(test)]
mod tests;

pub use cast::{
    CastDesc, CastUnit, CastingInfo, PlotCastState, SkillCastState, can_cast, can_channel,
    cast_progress, is_first_hit_by_me_or_my_team, unit_casting_info, unit_combat_state,
};
pub use cooldown::{
    CooldownAppend, CooldownEntry, CooldownFull, CooldownManager, MAX_COOLDOWN_ENTRIES,
    SKILL_LIST_TAG, SkillDesc, custom_gcd_ms, default_gcd_ms,
};
pub use equality::{SkillCastTarget, SkillCaster, caster_eq, target_eq};
pub use reaction::{BIG_HIT_THRESHOLD, HitRecordView, Reaction, big_hit, reaction};
pub use timeline::{
    AUTO_ATTACK_SKILL, ClientSkill, ComboEntry, EVENT_AUTO_ATTACK_STOPPED, EVENT_CASTING_STOPPED,
    EVENT_SKILL_ENDED, EVENT_SKILL_FIRED, EVENT_SKILL_STARTED, NO_TARGET, SkillInstance,
    StartOutcome, TimelineController, TimelineDesc, TimelineUnit, TimelineWorld,
};
pub use slot::{ActionSlot, ItemAction, SlotCooldowns, get_action_cooldown, get_action_cooldown_ex};
pub use token::{
    EMPTY_TARGET_ID, TeamMember, TokenRef, TokenUnit, TokenWorld, TOKEN_KIND_NONE, TOKEN_KIND_OBJECT,
    TOKEN_KIND_TEAM_MEMBER, TOKEN_KIND_UNIT, parse_token,
};
