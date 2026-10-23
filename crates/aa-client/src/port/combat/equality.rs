//! Skill caster / target / object equality.
//!
//! * [`caster_eq`] — `FUN_39391da0` (VA `0x39391da0`, 129 bytes), `__thiscall(a, b)`.
//! * [`target_eq`] — `FUN_39391e30` (VA `0x39391e30`, 465 bytes), `__thiscall(a, b)`.
//!
//! Both functions only read their arguments.  The word indices below are the original's `int*`
//! indices (`a[2]` = byte `+0x08`); the Rust structs keep the same layout.

/// `SkillCaster` (`0x24` bytes): `+0` type, `+4` pad, then seven words from `+8`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SkillCaster {
    pub kind: u32,
    pub pad: u32,
    pub words: [u32; 7],
}

/// `SkillCastTarget` / `SkillObject` (`>= 0x38` bytes for type 2): `+0` type, `+4` pad, then the
/// words the equality function reads from `+8` through `+0x34`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SkillCastTarget {
    pub kind: u32,
    pub pad: u32,
    pub words: [u32; 12],
}

const _: () = {
    assert!(core::mem::size_of::<SkillCaster>() == 0x24);
    assert!(core::mem::size_of::<SkillCastTarget>() == 0x38);
};

impl SkillCaster {
    #[inline]
    fn word(&self, index: usize) -> u32 {
        self.words[index - 2]
    }
}

impl SkillCastTarget {
    #[inline]
    fn word(&self, index: usize) -> u32 {
        self.words[index - 2]
    }
}

#[inline]
fn float_eq(a: u32, b: u32) -> bool {
    f32::from_bits(a) == f32::from_bits(b)
}

/// `FUN_39391da0`: compare a `SkillCaster`.
///
/// Types 0, 1, 3 and 4 compare only the id word `+8`; type 2 also compares `+0xc..+0x18` and the
/// **low byte** of `+0x18`.  Any other type is never equal.
pub fn caster_eq(a: &SkillCaster, b: &SkillCaster) -> bool {
    if a.kind != b.kind {
        return false;
    }
    match a.kind {
        0 | 1 | 3 | 4 => a.word(2) == b.word(2),
        2 => {
            a.word(2) == b.word(2)
                && a.word(3) == b.word(3)
                && a.word(4) == b.word(4)
                && a.word(5) == b.word(5)
                && (a.word(6) as u8) == (b.word(6) as u8)
        }
        _ => false,
    }
}

/// `FUN_39391e30`: compare a `SkillCastTarget`/`SkillObject`.
///
/// * type 0 and 4: word `+8`;
/// * type 1: `+8..+0x14`, float `+0x18`, words `+0x20`, `+0x24`;
/// * type 2: `+8..+0x14`, floats `+0x18`, `+0x20..+0x34` (word `+0x1c` is not compared);
/// * type 3: words `+8`, `+0x10`, `+0x14`, `+0x18`, low byte of `+0x1c`;
/// * type 5: `+8..+0x14`, floats `+0x18`, `+0x1c`;
/// * every other type: equal.
///
/// The float comparisons are C `==` (`-0.0 == 0.0`, NaN is never equal).
pub fn target_eq(a: &SkillCastTarget, b: &SkillCastTarget) -> bool {
    if a.kind != b.kind {
        return false;
    }
    match a.kind {
        0 | 4 => a.word(2) == b.word(2),
        1 => {
            a.word(2) == b.word(2)
                && a.word(3) == b.word(3)
                && a.word(4) == b.word(4)
                && a.word(5) == b.word(5)
                && float_eq(a.word(6), b.word(6))
                && a.word(8) == b.word(8)
                && a.word(9) == b.word(9)
        }
        2 => {
            a.word(2) == b.word(2)
                && a.word(3) == b.word(3)
                && a.word(4) == b.word(4)
                && a.word(5) == b.word(5)
                && float_eq(a.word(6), b.word(6))
                && float_eq(a.word(8), b.word(8))
                && float_eq(a.word(9), b.word(9))
                && float_eq(a.word(10), b.word(10))
                && float_eq(a.word(11), b.word(11))
                && float_eq(a.word(12), b.word(12))
                && float_eq(a.word(13), b.word(13))
        }
        3 => {
            a.word(2) == b.word(2)
                && a.word(4) == b.word(4)
                && a.word(5) == b.word(5)
                && a.word(6) == b.word(6)
                && (a.word(7) as u8) == (b.word(7) as u8)
        }
        5 => {
            a.word(2) == b.word(2)
                && a.word(3) == b.word(3)
                && a.word(4) == b.word(4)
                && a.word(5) == b.word(5)
                && float_eq(a.word(6), b.word(6))
                && float_eq(a.word(7), b.word(7))
        }
        _ => true,
    }
}
