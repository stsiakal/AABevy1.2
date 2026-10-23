//! Hit-reaction selection.
//!
//! * [`reaction`] — `FUN_393917f0` (VA `0x393917f0`, 295 bytes).
//! * [`big_hit`] — `FUN_39391950` (VA `0x39391950`, 147 bytes).
//!
//! The anim ids are the four-byte values in `.data` at `0x39ac766c..0x39ac7698`; the
//! "none" reaction is `DAT_3a177728` (0).  The table's two inputs are:
//!
//! * `weapon` = `FUN_397cd5f0(*(u32*)(unit+0x1000))` — the equipped item is a `0x10` weapon;
//! * `critical` = `FUN_39354600(unit)` = `*(u8*)(unit+0x5fc4)`.
//!
//! Input `hitType` is the damage record's `+0x48`.

/// `DAT_3999c484`: the big-hit threshold is a float constant at `.rdata 0x3999c484`, whose value
/// is `0.0f` in the shipped 1.2 image (`comiss xmm0, [0x3999c484]; ja`).  The earlier map note
/// "> 1.0" was wrong; the `1.0f` at `0x3999c488` is a different constant.
pub const BIG_HIT_THRESHOLD: f32 = 0.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reaction {
    pub anim: u32,
    pub kind: u32,
}

const NONE_ANIM: u32 = 0;
const ANIM_HIT: u32 = 15;
const ANIM_WEAPON_A: u32 = 255;
const ANIM_WEAPON_B: u32 = 256;
const ANIM_FIVE: u32 = 13;
const ANIM_SIX: u32 = 12;
const ANIM_SEVEN_A: u32 = 11;
const ANIM_SEVEN_B: u32 = 10;
const ANIM_CRIT_A: u32 = 17;
const ANIM_CRIT_B: u32 = 14;
const ANIM_NORMAL_A: u32 = 387;
const ANIM_NORMAL_B: u32 = 386;

const fn reaction_of(anim: u32, kind: u32) -> Reaction {
    Reaction { anim, kind }
}

/// `FUN_393917f0`: map a hit type to `(anim id, reaction kind)`.
pub fn reaction(hit_type: i32, weapon: bool, critical: bool) -> Reaction {
    match hit_type {
        // these three fall through to the crit/normal selection
        1 | 9 | 0xD => fallback(weapon, critical),
        3 | 0xB | 0xF => reaction_of(ANIM_HIT, 3),
        4 | 10 | 0xE => reaction_of(if weapon { ANIM_WEAPON_A } else { ANIM_WEAPON_B }, 3),
        5 | 0x10 => reaction_of(ANIM_FIVE, 3),
        6 | 0x11 => reaction_of(ANIM_SIX, 1),
        7 | 0x14 => reaction_of(if weapon { ANIM_SEVEN_A } else { ANIM_SEVEN_B }, 1),
        _ => reaction_of(NONE_ANIM, 3),
    }
}

fn fallback(weapon: bool, critical: bool) -> Reaction {
    let anim = if critical {
        if weapon { ANIM_CRIT_A } else { ANIM_CRIT_B }
    } else if weapon {
        ANIM_NORMAL_A
    } else {
        ANIM_NORMAL_B
    };
    reaction_of(anim, 3)
}

/// The `HitRecord` fields [`big_hit`] reads (SCUnitDamaged payload, `0x188` bytes).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HitRecordView {
    /// `+0x48`: hit type.
    pub hit_type: i32,
    /// `+0x18`: `SkillCaster.type`.
    pub caster_kind: u32,
    /// `+0x20`: caster id for types 0/3.
    pub caster_id_20: u32,
    /// `+0x2c`: caster id for type 2.
    pub caster_id_2c: u32,
    /// `+0x3c`: target id.
    pub target_id: u32,
}

/// `FUN_39391950`: true when the "attacker view" presentation runs instead of the victim one.
///
/// * no hit type (`+0x48 == 0`) or `float(desc+0x188) > 0.0` -> true;
/// * otherwise resolve the caster id by type (0/3 -> `+0x20`, 2 -> `+0x2c`, otherwise the empty
///   id 0); if it differs from the target and both ids resolve through `FUN_39360020`, return
///   `client_mode != 1`, where `client_mode` is the game-client method `vtable+4`;
/// * any unresolved id -> true.
pub fn big_hit(
    record: &HitRecordView,
    desc_big_hit: i32,
    client_mode: i32,
    resolves: impl Fn(u32) -> bool,
) -> bool {
    if record.hit_type == 0 || (desc_big_hit as f32) > BIG_HIT_THRESHOLD {
        return true;
    }
    let caster_id = match record.caster_kind {
        0 | 3 => record.caster_id_20,
        2 => record.caster_id_2c,
        _ => 0,
    };
    if caster_id != record.target_id
        && resolves(record.target_id)
        && resolves(caster_id)
    {
        return client_mode != 1;
    }
    true
}
