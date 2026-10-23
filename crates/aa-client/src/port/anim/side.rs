//! Mount placement leaves from `x2game.dll`:
//!
//! * `FUN_3909ae90` (entry side): the reason byte sent in `CSMountMate` (1 left, 2 right,
//!   3 back, 4 front).
//! * `FUN_390a2c90` (approach side): the approach offset stored on the mount controller
//!   (`controller+0x20..0x28`).
//! * `FUN_3909b1a0` (side placement): the 1.5 m left/right candidate vector with the
//!   collision-checked fallback.
//!
//! All three read the same 4-float orientation block from the unit's `model+0x228` transform
//! (`FUN_390b3870`): the code below calls it `q = [q0, q1, q2, q3]`. The original arithmetic is
//! SSE `f32` and `rsqrtss`; the operation order is preserved and the reciprocal square root uses
//! the same instruction plus the same one-step Newton refinement.

use super::{ENTRY_DOT, FLT_MIN, NEG_ZERO, ONE, SIDE_DISTANCE, SIDE_SCALE, TWO};

/// A world-space position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

/// The output of `FUN_3933b270` for one unit: the point on the ground/attach point and the
/// unit's local basis. Only the fields the placement collision consumes are kept.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// `out[0..3]`: the unit position with the ground/attach-point offset applied.
    pub position: Vec3,
    /// `out[3..6]`: the unit's local side/basis vector.
    pub side: Vec3,
    /// `out[6]`: the ground/attach-point height.
    pub height: f32,
    /// `out[7]`: the attach-point scale/time value.
    pub extra: f32,
}

/// `rsqrtss` + one Newton step, the exact sequence at `x2game+0x909af0b` and `+0x90a2d77`:
/// `n = rsqrtss(x); n = (1.5 - n*x*n*0.5) * n`.
pub fn rsqrt(x: f32) -> f32 {
    #[cfg(target_arch = "x86_64")]
    {
        use core::arch::x86_64::{_mm_cvtss_f32, _mm_rsqrt_ss, _mm_set_ss};
        let n = unsafe { _mm_cvtss_f32(_mm_rsqrt_ss(_mm_set_ss(x))) };
        (1.5f32 - n * x * n * 0.5f32) * n
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let n = 1.0f32 / x.sqrt();
        (1.5f32 - n * x * n * 0.5f32) * n
    }
}

/// The three orientation terms the original computes from `q`:
/// `a = (q0*q1 - q2*q3) * 2`, `b = (q1*q1 + q3*q3) * 2 - 1`, `c = (q3*q0 + q2*q1) * 2`.
#[inline]
fn orientation_terms(q: [f32; 4]) -> (f32, f32, f32) {
    let a = (q[0] * q[1] - q[2] * q[3]) * TWO;
    let b = (q[1] * q[1] + q[3] * q[3]) * TWO - ONE;
    let c = (q[3] * q[0] + q[2] * q[1]) * TWO;
    (a, b, c)
}

/// `FUN_3909ae90` (entry side / `CSMountMate` reason).
///
/// `player` is the local player position (`[controller+0x14]` transform `+0x10`), `mate` the
/// mount position (`FUN_390b3850`), `q` the mount's orientation block. `seat1`/`seat2` are the
/// two `FUN_390c9f40(1/2)` occupancy results.
///
/// Returns 1 (left), 2 (right), 3 (back) or 4 (front). The seat override is: back uses seat 2,
/// front and both side bands use seat 1; an occupied seat forces 1.
pub fn entry_side(player: Vec3, mate: Vec3, q: [f32; 4], seat1: bool, seat2: bool) -> u8 {
    let (a, b, c) = orientation_terms(q);
    let dx = mate.x - player.x;
    let dy = mate.y - player.y;
    let dz = mate.z - player.z;
    let len_sq = dx * dx + dy * dy + dz * dz + FLT_MIN;
    let n = rsqrt(len_sq);
    let dot = dy * n * b + dx * n * a + dz * n * c;

    if dot >= ENTRY_DOT {
        // Back. The original copies seat2 into the seat1 flag in this branch only.
        if seat2 { 1 } else { 3 }
    } else if dot < -ENTRY_DOT {
        // Front.
        if seat1 { 1 } else { 4 }
    } else {
        let cross = dy * n * a - dx * n * b;
        let reason = if 0.0f32 <= cross { 2 } else { 1 };
        if seat1 { 1 } else { reason }
    }
}

/// `FUN_390a2c90` (approach side). Writes the offset vector to `controller+0x20..0x28`.
///
/// `reason` is the `FUN_3909ae90` result. The returned vector is the controller's local-space
/// approach offset; the player transform read by the original is dead code.
pub fn approach_side(reason: u8, q: [f32; 4]) -> [f32; 3] {
    let (a, b, c) = orientation_terms(q);
    // v = normalize((-b, a, 0)); the original writes the normalized pair and a zero z.
    let len_sq = a * a + b * b + FLT_MIN;
    let n = rsqrt(len_sq);
    let vx = -b * n;
    let vy = a * n;
    // The original's z is `(b*0 - a*0) * n`, which is a signed zero; keep the sign.
    let vz = (b * 0.0 - a * 0.0) * n;
    let flip = |v: f32| f32::from_bits(v.to_bits() ^ 0x8000_0000);

    match reason {
        2 => [flip(vx), flip(vy), flip(vz)],
        3 => [flip(a * TWO), flip(b * TWO), flip(c * TWO)],
        4 => [a * TWO, b * TWO, c * TWO],
        _ => [vx, vy, vz],
    }
}

/// `FUN_3909b1a0` (side placement).
///
/// `a` is the controller's placement frame, `b` the mount's, `q` the mount orientation. The
/// function builds the left candidate (`u - 1.5*s`) and returns it when `collision(a, b, -s)` is
/// false; otherwise it tries the right candidate (`u + 1.5*s`) with `collision(a, b, s)` and
/// returns it when free. When both collide it returns `None` (the caller falls back to the
/// saddle transform).
///
/// `collision` is `FUN_3933be30(a, b, direction, 0)`: `true` means the candidate is blocked.
pub fn side_placement<F>(
    a: &Placement,
    b: &Placement,
    q: [f32; 4],
    mut collision: F,
) -> Option<[f32; 3]>
where
    F: FnMut(&Placement, &Placement, [f32; 3]) -> bool,
{
    let (a_term, b_term, _) = orientation_terms(q);
    let len_sq = a_term * a_term + b_term * b_term;
    let (ux, uy, uz) = if len_sq > FLT_MIN {
        let n = rsqrt(len_sq);
        (a_term * n, b_term * n, 0.0f32)
    } else {
        (0.0f32, 0.0f32, 1.0f32)
    };
    // s = (u.y, -u.x, 0); in the original this is (b, -a, 0).
    let s = [uy, -ux, 0.0f32];
    let neg = |v: f32| f32::from_bits(v.to_bits() ^ 0x8000_0000);
    let neg_s = [neg(s[0]), neg(s[1]), neg(s[2])];

    if !collision(a, b, neg_s) {
        return Some([
            ux - SIDE_DISTANCE * s[0] * SIDE_SCALE,
            uy - SIDE_DISTANCE * s[1] * SIDE_SCALE,
            uz - SIDE_DISTANCE * s[2] * SIDE_SCALE,
        ]);
    }
    if !collision(a, b, s) {
        return Some([
            ux + SIDE_DISTANCE * s[0] * SIDE_SCALE,
            uy + SIDE_DISTANCE * s[1] * SIDE_SCALE,
            uz + SIDE_DISTANCE * s[2] * SIDE_SCALE,
        ]);
    }
    None
}
