//! `FUN_39112690` (x2game.dll, 0x39112690, 1615 bytes): direction-speed factor.
//!
//! The original is `__thiscall` on the actor (`ECX = actor`) with `float dir[2]` as its only
//! stack argument. It reads the movement block at `actor+0x3ec` and the walk bit
//! `actor+0x40 & 0x1000`; the walk set is at `+0x454..+0x464`, the normal set at
//! `+0x43c..+0x44c`. It converts the input vector to eight `atan2` angles (MSVC
//! `__libm_sse2_atan2`, double precision, rounded to `f32`), picks a sector with the same
//! comparison chain, blends the two neighbouring speed factors, and divides by the walk
//! forward-diagonal factor when walking.
//!
//! All arithmetic below is the original SSE `f32` sequence, including the two-vector helper
//! `FUN_391119f0` used by three sectors. The only non-bit-exact part is `atan2`: Rust's
//! `f64::atan2` is a different libm implementation, so this function is tested with a 1e-6
//! tolerance (the captured f32 angles agree in every golden case).

use super::FLT_EPSILON;

/// The movement block at `actor+0x3ec` (`structs.md` §7), partial.
///
/// Offsets are relative to the block base; `actor+0x43c` is `+0x50`, and so on.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MovementParams {
    pub pad_00: [u8; 0x50],
    /// `actor+0x43c`
    pub forward_diag: f32,
    /// `actor+0x440`
    pub forward: f32,
    /// `actor+0x444`
    pub strafe: f32,
    /// `actor+0x448`
    pub backward_diag: f32,
    /// `actor+0x44c`
    pub backward: f32,
    /// `actor+0x450`
    pub grab: f32,
    /// `actor+0x454`
    pub walk_forward_diag: f32,
    /// `actor+0x458`
    pub walk_forward: f32,
    /// `actor+0x45c`
    pub walk_strafe: f32,
    /// `actor+0x460`
    pub walk_backward_diag: f32,
    /// `actor+0x464`
    pub walk_backward: f32,
}

const _: () = {
    assert!(core::mem::offset_of!(MovementParams, forward_diag) == 0x50);
    assert!(core::mem::offset_of!(MovementParams, walk_forward_diag) == 0x68);
    assert!(core::mem::offset_of!(MovementParams, walk_backward) == 0x78);
};

impl Default for MovementParams {
    fn default() -> Self {
        Self {
            pad_00: [0; 0x50],
            forward_diag: 0.0,
            forward: 0.0,
            strafe: 0.0,
            backward_diag: 0.0,
            backward: 0.0,
            grab: 0.0,
            walk_forward_diag: 0.0,
            walk_forward: 0.0,
            walk_strafe: 0.0,
            walk_backward_diag: 0.0,
            walk_backward: 0.0,
        }
    }
}

fn atan2_f32(y: f32, x: f32) -> f32 {
    (y as f64).atan2(x as f64) as f32
}

/// `FUN_391119f0` (x2game.dll, 0x391119f0, 172 bytes): the two-vector weight helper.
///
/// `ECX = p`, `EDX = a`, stack `b`, `EDI = out1`, `ESI = out2`; returns the two weights that
/// express `p` in the `a`/`b` basis, normalised to sum to one when the basis is not degenerate.
fn blend_pair(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> (f32, f32) {
    let cross_ab = b[0] * a[1] - a[0] * b[1];
    let den = 1.0 / cross_ab;
    let p1_a0 = p[1] * a[0];
    let w1 = (p[0] * a[1] - p1_a0) * den;
    let w2 = (p[1] * b[0] - p[0] * b[1]) * den;
    let sum = w2 + w1;
    if sum > FLT_EPSILON {
        let inv = 1.0 / sum;
        (inv * w1, inv * w2)
    } else {
        let out1 = if w1 < w2 { 0.0 } else { 1.0 };
        (out1, 1.0 - w1)
    }
}

/// The shared weight normalisation inside most sectors of `FUN_39112690`.
///
/// `v1` is the speed for `w1`, `v2` the speed for `w2`; the result is
/// `w2n * v2 + w1n * v1` in the original's exact operation order.
fn blend(w1: f32, w2: f32, v1: f32, v2: f32) -> f32 {
    let sum = w1 + w2;
    if sum > FLT_EPSILON {
        let inv = 1.0 / sum;
        (inv * w2) * v2 + (inv * w1) * v1
    } else {
        let w2n = if w2 < w1 { 0.0 } else { 1.0 };
        let w1n = 1.0 - w2;
        w2n * v2 + w1n * v1
    }
}

/// `FUN_39112690` with the actor fields split into plain values.
///
/// `walk` is `actor+0x40 & 0x1000`, `dir` is the `float dir[2]` the caller passes.
pub fn direction_factor(params: &MovementParams, walk: bool, dir: [f32; 2]) -> f32 {
    let (v1, v2, v3, v4, v5) = if walk {
        (
            params.walk_forward_diag,
            params.walk_forward,
            params.walk_strafe,
            params.walk_backward_diag,
            params.walk_backward,
        )
    } else {
        (
            params.forward_diag,
            params.forward,
            params.strafe,
            params.backward_diag,
            params.backward,
        )
    };

    let dx = dir[0];
    let dy = dir[1];

    let length_sq = dx * dx + dy * dy;
    if length_sq == 0.0 {
        return 0.0;
    }

    let zero_dy = dy * 0.0;
    let zero_dx = dx * 0.0;
    let neg_dy = -dy;
    let neg_dx = -dx;

    let a = atan2_f32(neg_dx, dy);
    let b = atan2_f32(neg_dy - dx, dy - dx);
    let c = atan2_f32(neg_dy, neg_dx);
    let d = atan2_f32(dx - dy, neg_dy - dx);
    let e = atan2_f32(zero_dx - neg_dx, zero_dx - dy);
    let f = atan2_f32(dy + dx, dx - dy);
    let g = atan2_f32(dy - zero_dx, zero_dy + dx);
    let h = atan2_f32(dy - dx, dy + dx);

    let mut result = if a >= 0.0 && b <= 0.0 && a != b {
        let w1 = zero_dy - dx;
        let w2 = dx - neg_dy;
        blend(w1, w2, v2, v1)
    } else if b >= 0.0 && c <= 0.0 {
        let w1 = neg_dy - dx;
        let w2 = zero_dx - neg_dy;
        blend(w1, w2, v3, v2)
    } else if c >= 0.0 && d <= 0.0 {
        let w1 = neg_dy - zero_dx;
        let w2 = neg_dx - neg_dy;
        blend(w1, w2, v5, v3)
    } else if d >= 0.0 && e <= 0.0 {
        let w1 = neg_dy - neg_dx;
        let w2 = neg_dx - zero_dy;
        blend(w1, w2, v4, v5)
    } else if e >= 0.0 && f <= 0.0 && e != f {
        let (out1, out2) = blend_pair([dx, dy], [1.0, -1.0], [0.0, -1.0]);
        out2 * v5 + out1 * v4
    } else if f >= 0.0 && g <= 0.0 {
        let w1 = zero_dx - dy;
        let w2 = dy + dx;
        blend(w1, w2, v5, v3)
    } else if g >= 0.0 && h <= 0.0 {
        let (out1, out2) = blend_pair([dx, dy], [1.0, 1.0], [1.0, 0.0]);
        out1 * v3 + out2 * v2
    } else if h >= 0.0 && a <= 0.0 {
        let (out1, out2) = blend_pair([dx, dy], [0.0, 1.0], [1.0, 1.0]);
        out1 * v2 + out2 * v1
    } else {
        0.0
    };

    if walk {
        result /= params.walk_forward_diag;
    }
    result
}
