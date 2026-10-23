//! `FUN_3922aed0` (x2game.dll, 0x3922aed0, 39 bytes).
//!
//! ```
//! if (v < lo) return lo;
//! if (hi < v) return hi;
//! return v;
//! ```
//!
//! The original uses two `comiss`/`jbe` pairs, so a NaN `v` is returned unchanged and a NaN
//! bound cannot win the same comparison. The literal `<` form below has those semantics.

/// Clamp `v` to `lo..=hi`, bit-for-bit as `FUN_3922aed0`.
pub fn clamp(v: f32, lo: f32, hi: f32) -> f32 {
    if v < lo {
        lo
    } else if hi < v {
        hi
    } else {
        v
    }
}
