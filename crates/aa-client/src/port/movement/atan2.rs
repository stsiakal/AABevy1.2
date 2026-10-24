//! MSVCR100 `__libm_sse2_atan2` (VA `0x78b38220`, RVA `0x98220`), the function the 1.2
//! client calls for every movement angle.
//!
//! The original is the SSE2 table-reduction implementation: it scales `y` by a reciprocal
//! square root of the exponent of `x`, reduces on a 166-entry `atan(2^k)` table, and evaluates
//! a degree-4 polynomial on the residual.  This is a straight transliteration of the
//! instruction sequence (`x2game` 0x3912bfb0 calls it at 0x3912c57d); every `f64` operation
//! below is in the original order, and `rcpss` is the hardware approximation.
//!
//! Inputs outside the SSE fast window (subnormals, huge/tiny exponents) use the original
//! scaling and special-value paths.  The captured goldens (`goldens/atan2.json`, 956 cases)
//! are taken from the live client's `MSVCR100.dll` with FTZ/DAZ cleared for the call.

use super::atan2_tables::ATAN;

const PI_2: f64 = f64::from_bits(0x3ff9_21fb_5444_2d18);
const PI_2_TAIL: f64 = f64::from_bits(0x3c91_a626_3314_5c07);
const PI: f64 = f64::from_bits(0x4009_21fb_5444_2d18);
const PI_TAIL: f64 = f64::from_bits(0x3ca1_a626_3314_5c07);
const PI_4: f64 = f64::from_bits(0x3fe9_21fb_5444_2d18);
const PI_4_TAIL: f64 = f64::from_bits(0x3c81_a626_3314_5c07);

const C0: f64 = f64::from_bits(0x3fbc_638f_7795_24c9);
const C1: f64 = f64::from_bits(0xbfc4_beb3_8c93_153f);
const C2: f64 = f64::from_bits(0x3fc6_2d29_e534_fa10);
const C3: f64 = f64::from_bits(0x4000_742d_a8a9_7a2d);

const SIGN: u64 = 1 << 63;
const SCALE_2_55: u64 = 0x4360_0000_0000_0000;
const EXP_CLEAR: u64 = 0x800f_ffff_ffff_ffff;
const CAP: u64 = 0x4040_0000_0000_0000;

#[inline]
fn word3(bits: u64) -> u32 {
    ((bits >> 48) & 0xffff) as u32
}

#[inline]
fn exponent_word(bits: u64) -> u32 {
    word3(bits) & 0x7ff0
}

#[inline]
fn mantissa_nonzero(bits: u64) -> bool {
    (bits as u32) != 0 || (((bits >> 32) as u32) & 0x000f_ffff) != 0
}

#[inline]
fn rcp_ss(v: f32) -> f32 {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use core::arch::x86_64::{_mm_cvtss_f32, _mm_rcp_ss, _mm_set_ss};
        _mm_cvtss_f32(_mm_rcp_ss(_mm_set_ss(v)))
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        1.0 / v
    }
}

/// `__libm_sse2_atan2(y, x)` from MSVCR100, bit-exact on the captured goldens.
pub fn libm_sse2_atan2(y: f64, x: f64) -> f64 {
    let yb = y.to_bits();
    let xb = x.to_bits();
    if exponent_word(yb).wrapping_sub(0x3870) <= 0xf00
        && exponent_word(xb).wrapping_sub(0x3870) <= 0xf00
    {
        return fast(y, x, yb, xb);
    }
    slow(y, x, yb, xb)
}

fn fast(y: f64, x: f64, yb: u64, xb: u64) -> f64 {
    // rcpss of the exponent of x builds the reduction scale.
    let shifted = xb.wrapping_add(0x0800u64 << 48) >> 29;
    let r = rcp_ss(f32::from_bits(shifted as u32));
    let mut v3 = ((r.to_bits() as u64) << 29).wrapping_add(0x3800u64 << 48);
    let w = f64::from_bits(v3) * y;
    v3 = w.to_bits().wrapping_add(0x8000u64 << 32) & (0x7fffu64 << 48);
    let v3_orig = v3;
    let v3f = f64::from_bits(v3);
    let cap = f64::from_bits(CAP);
    let v3_min = if v3f < cap { v3f } else { cap };
    let mask: u64 = if v3f <= cap { u64::MAX } else { 0 };

    let a = (word3(v3_orig) & 0x7ff0).wrapping_sub(0x3f9e);
    let y_abs = f64::from_bits(yb & !SIGN);
    let x_abs = f64::from_bits(xb & !SIGN);
    let x_trunc = f64::from_bits((xb & !SIGN) & !0xffff);
    let x_frac = x_abs - x_trunc;
    let x6 = x_trunc * v3_min;
    let x4 = y_abs * v3_min;
    let x7 = x_frac * v3_min;
    let mut num = f64::from_bits(y_abs.to_bits() & mask) - x6;
    num -= x7;

    let q = ((yb >> 63) as u32) | (((xb >> 63) as u32) << 1);
    if a <= 0x461 {
        return poly(y, x, yb, xb, v3_min, mask, num, x4, q);
    }
    let a2 = a.wrapping_add(0x3ae);
    if a2 > 0x3ae {
        return slow_compare(yb, xb);
    }
    branch_c(y, xb, v3_min, num, q)
}

/// Small-ratio branch: `atan(y/x)` around the quadrant constants (`x2game` 0x78b3840e).
fn branch_c(y: f64, xb: u64, v3_min: f64, num: f64, q: u32) -> f64 {
    let inv = 1.0 / f64::from_bits(xb & !SIGN);
    let (qsign, base_lo, base_hi, v3_low) = match q {
        0 => (0.0f64, 0.0, 0.0, false),
        1 => (-0.0, 0.0, 0.0, false),
        2 => (-0.0, PI_TAIL, PI, true),
        _ => (0.0, -PI_TAIL, -PI, true),
    };
    let v3 = f64::from_bits(v3_min.to_bits() ^ qsign.to_bits());
    let t = f64::from_bits((y.to_bits() & !SIGN) ^ qsign.to_bits()) * inv;
    let c = f64::from_bits(num.to_bits() ^ qsign.to_bits()) * inv;
    let mut low = base_lo + c;
    let high = if v3_low {
        low += v3;
        base_hi
    } else {
        base_hi + v3
    };

    let t2 = t * t;
    let mut c0t2 = C0 * t2;
    let mut t3 = C2 + t2;
    let t_cubed = t * t2;
    c0t2 += C1;
    t3 *= t2;
    let h = c0t2 * t_cubed;
    t3 += C3;
    let poly = h * t3;
    let mut res = poly + low;
    res += high;
    res
}

/// Main reduction branch (`x2game` 0x78b38350).
#[allow(clippy::too_many_arguments)]
fn poly(
    _y: f64,
    x: f64,
    _yb: u64,
    xb: u64,
    v3_min: f64,
    mask: u64,
    num: f64,
    x4: f64,
    q: u32,
) -> f64 {
    let x_abs = f64::from_bits(xb & !SIGN);
    let den = x4 + f64::from_bits(x_abs.to_bits() & mask);
    let _ = x;
    let z = num / den;

    let bit = if mask == 0 { 1 } else { 0 };
    let idx = word3(v3_min.to_bits()).wrapping_sub(0x3f9e).wrapping_add(bit) as usize;
    let (entry_hi, entry_lo) = ATAN[idx];
    let (base_hi, base_lo, negate) = match q {
        0 => (0.0, 0.0, false),
        1 => (0.0, 0.0, true),
        2 => (PI, PI_TAIL, true),
        _ => (-PI, -PI_TAIL, false),
    };
    let mut atan_hi = f64::from_bits(entry_hi);
    let mut atan_lo = f64::from_bits(entry_lo);
    if negate {
        atan_hi = -atan_hi;
        atan_lo = -atan_lo;
    }
    atan_hi += base_hi;
    atan_lo += base_lo;
    let zm = if negate { -z } else { z };

    let z2 = z * z;
    let mut c0z2 = C0 * z2;
    let mut t3 = C2 + z2;
    let t1 = zm + atan_hi;
    let mut t6 = atan_hi - t1;
    t6 += zm;
    c0z2 += C1;
    t3 *= z2;
    let zm3 = zm * z2;
    t6 += atan_lo;
    let h = c0z2 * zm3;
    t3 += C3;
    let j = h * t3;
    let k = j + t6;
    t1 + k
}

fn slow_compare(yb: u64, xb: u64) -> f64 {
    if exponent_word(yb) as i32 > exponent_word(xb) as i32 {
        return f64::from_bits((yb & SIGN) | PI_2.to_bits());
    }
    // `word3(x) > 0x7fff` is the original's test for a negative x (zero-extended word).
    if xb & SIGN != 0 {
        return f64::from_bits((yb & SIGN) ^ PI.to_bits());
    }
    f64::from_bits(yb) / f64::from_bits(xb)
}

fn or_sign(yb: u64, value: f64) -> f64 {
    f64::from_bits((yb & SIGN) | value.to_bits())
}

fn slow(y: f64, x: f64, yb: u64, xb: u64) -> f64 {
    let y_exp = exponent_word(yb);
    let x_exp = exponent_word(xb);
    if y_exp == 0x7ff0 {
        return y_special(yb, xb);
    }
    if x_exp == 0x7ff0 {
        return x_special(yb, xb);
    }

    let scale = f64::from_bits(SCALE_2_55);
    let mut edx = 0x400i32;
    let mut yy = y;
    let mut xx = x;
    let mut ye = y_exp;
    let mut xe = x_exp;
    if ye == 0 {
        edx -= 0x370;
        yy = y * scale;
        ye = exponent_word(yy.to_bits());
        if ye == 0 {
            // y == ±0
            if xb & SIGN != 0 {
                return or_sign(yb, PI + PI_TAIL);
            }
            return f64::from_bits(yb);
        }
    }
    if xe == 0 {
        edx += 0x370;
        xx = x * scale;
        xe = exponent_word(xx.to_bits());
        if xe == 0 {
            // x == ±0
            return or_sign(yb, PI_2 + PI_2_TAIL);
        }
    }
    edx = edx.wrapping_add(ye as i32).wrapping_sub(xe as i32);
    if edx as u32 > 0x800 {
        return slow_compare(yb, xb);
    }
    edx += 0x3bf0;
    let y2 = (yy.to_bits() & EXP_CLEAR) | (((edx as u32) & 0xffff) as u64) << 48;
    let x2 = (xx.to_bits() & EXP_CLEAR) | 1.0f64.to_bits();
    fast(f64::from_bits(y2), f64::from_bits(x2), y2, x2)
}

fn y_special(yb: u64, xb: u64) -> f64 {
    if mantissa_nonzero(yb) {
        return f64::from_bits(yb);
    }
    if exponent_word(xb) >= 0x7ff0 {
        return both_special(yb, xb);
    }
    or_sign(yb, PI_2 + PI_2_TAIL)
}

fn both_special(yb: u64, xb: u64) -> f64 {
    if mantissa_nonzero(xb) {
        return f64::from_bits(xb);
    }
    if xb & SIGN != 0 {
        let sum = (PI_4 + PI_2) + (PI_4_TAIL + PI_2_TAIL);
        return or_sign(yb, sum);
    }
    or_sign(yb, PI_4 + PI_4_TAIL)
}

fn x_special(yb: u64, xb: u64) -> f64 {
    if mantissa_nonzero(xb) {
        return f64::from_bits(xb);
    }
    if xb & SIGN != 0 {
        return or_sign(yb, PI + PI_TAIL);
    }
    f64::from_bits(yb & SIGN)
}
