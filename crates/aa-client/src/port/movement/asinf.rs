//! MSVCR100 `__libm_sse2_asinf` (export ordinal 369, VA `0x78b36580`), the function the
//! original `CPlayerRotation::ProcessNormal` / `ClampAngles` call through the import thunk
//! `x2game!0x398b57d0`.
//!
//! The implementation is the SSE2 build shipped in `MSVCR100.dll` (the CRT the 1.2 client
//! links).  It is ported instruction by instruction from the disassembly: the branch
//! thresholds, the two constant tables at `0x78b36940` / `0x78b37140`, the `andps`/`pshufd`
//! mantissa manipulations and the final rounding-fixup chain are reproduced with `f32`
//! operations in the original order.  `#[inline]` helpers stand for the SSE lane operations;
//! only lane 0 is observable.

use super::asinf_tables::{COEF, POLY};

const C_1_6: f32 = f32::from_bits(0x3e2a_aaab);
const C_0_075: f32 = f32::from_bits(0x3d99_999a);
const C_0_044642858: f32 = f32::from_bits(0x3d36_db6e);
const PI_2: f32 = f32::from_bits(0x3fc9_0fdb);
const M4: f32 = f32::from_bits(0xb33b_bd2e);
const NEG_ZERO_MASK: u32 = 0x8000_0000;
const AND_MASK: u32 = 0x7fff_f000;
const AND_MASK_C: u32 = 0xffff_f000;

#[inline(always)]
fn f(bits: u32) -> f32 {
    f32::from_bits(bits)
}

#[inline(always)]
fn b(value: f32) -> u32 {
    value.to_bits()
}

#[inline(always)]
fn poly(index: u32) -> [f32; 4] {
    let i = index as usize;
    [
        f(POLY[i]),
        f(POLY[i + 1]),
        f(POLY[i + 2]),
        f(POLY[i + 3]),
    ]
}

#[inline(always)]
fn coef(index: u32) -> f32 {
    f(COEF[index as usize])
}

/// `__libm_sse2_asinf(x)` from MSVCR100 `0x78b36580` (RVA `0x96580`).
pub fn libm_sse2_asinf(x: f32) -> f32 {
    let bits = x.to_bits();
    let abs = bits & 0x7fff_ffff;
    let sign = bits & NEG_ZERO_MASK;

    // 0x78b3691e / the tiny-argument path: asin(x) rounds to x below 2^-16.
    if abs <= 0x3780_0000 {
        return x;
    }

    // 0x78b3676c: 2^-16 <= |x| < 2^-4, plain 7th-order odd polynomial.
    if abs < 0x3d80_0000 {
        let x2 = x * x;
        let x3 = x * C_1_6;
        let x4 = x * C_0_044642858;
        let x5 = x * C_0_075;
        let x6 = x2;
        let x7 = x2 * x2;
        let x8 = x3 * x6;
        let x9 = x6 * x7;
        let x10 = x7 * x5;
        let x11 = x4 * x9;
        return ((x10 + x8) + x11) + x;
    }

    // 0x78b365b6: 2^-4 <= |x| <= sqrt(3)/2, table-assisted atan-like form.
    if abs <= 0x3f5d_b3d7 {
        let s = (1.0 - x * x).sqrt();
        let x2 = f((bits & 0xfffe_0000) | 0x0001_0000);
        let edx = (abs.wrapping_sub(0x3d80_0000) >> 16) & 0x1fe;
        let c = coef(edx / 2);
        let p = poly(edx);
        let xm6 = x * c;
        let xm5 = xm6 + x2 * s;
        let xm0 = ((x - x2) * (x + x2)) / xm5;
        let xm6 = xm6 - x2 * s;
        let xm7 = (C_1_6 * xm6) * (xm6 * xm6);
        let xm7 = xm7 + f(b(p[0]) ^ sign);
        (xm0 + xm7) + f(b(p[1]) ^ sign)
    } else if abs <= 0x3f7e_ffff {
        // 0x78b36663: sqrt(3)/2 < |x| <= 0.99607.
        let s = (1.0 - x * x).sqrt();
        let absx = f(abs);
        let trunc = f(abs & AND_MASK);
        let low = absx - trunc;
        let two_trunc = absx + trunc;
        let s2 = f((b(s) & 0xfffe_0000) | 0x0001_0000);
        let edx = ((b(s) >> 16).wrapping_sub(0x3d80)) & 0xfffe;
        let xm3 = s * coef(edx / 2);
        let xm6 = (trunc * s2 - xm3) + low * s2;
        let mut xm4 = (1.0 - trunc * trunc) - two_trunc * low;
        xm4 -= s2 * s2;
        let den = (xm3 + xm3) + xm6;
        let xm4 = xm4 / den;
        let mut xm5 = (C_1_6 * xm6) * (xm6 * xm6);
        let p = poly(edx);
        let v0 = M4 - p[0];
        let v1 = PI_2 - p[1];
        xm5 += v0;
        let mut a = xm4;
        let mut c = v1;
        let mut xm4 = xm4;
        xm4 -= c;
        c += xm4;
        a -= c;
        xm5 -= a;
        xm5 -= xm4;
        f(b(xm5) ^ sign)
    } else if abs < 0x3f80_0000 {
        // 0x78b367c9: 0.99607 < |x| < 1, near-one expansion.
        let s = (1.0 - x * x).sqrt();
        let hi = abs >> 16;
        let sign_bit = hi & 0x8000;
        let trunc_x = f(bits & AND_MASK_C);
        let trunc_s = f(b(s) & AND_MASK_C);
        let low = x - trunc_x;
        let two_x = trunc_x + trunc_x;
        let mut xm4 = 1.0 - trunc_x * trunc_x;
        xm4 -= two_x * low;
        xm4 -= trunc_s * trunc_s;
        let mut xm6 = trunc_s - s;
        let mut xm5 = s + s;
        xm4 -= low * low;
        let mut xm2 = s * C_1_6;
        xm5 += xm6;
        xm6 *= xm5;
        xm4 += xm6;
        xm4 /= s + s;
        let xm7 = s * C_0_075;
        let mut xm6 = s * s;
        xm2 *= xm6;
        xm6 *= xm6;
        xm2 += xm7 * xm6;

        let mut r = M4 - xm2;
        r -= xm4;
        let mut t = PI_2;
        let mut one = s;
        let d = s - t;
        t += d;
        one -= t;
        r -= one;
        r -= d;
        f(b(r) ^ (sign_bit << 16))
    } else if abs == 0x3f80_0000 {
        // 0x78b368f3: |x| == 1.
        f(sign | b(PI_2 + M4))
    } else if abs > 0x7f80_0000 {
        // NaN propagates unchanged (addss x, 0).
        x
    } else {
        // 0x78b368e2: |x| > 1 -> 0 * +inf.
        f(0xffc0_0000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_captured_values() {
        let cases = [
            (0u32, 0u32),
            (0x3586_37bd, 0x3586_37bd),
            (0x3a83_126f, 0x3a83_1270),
            (0x3cf5_c28f, 0x3cf5_cc00),
            (0x3d4c_cccd, 0x3d4c_e2ac),
            (0x3d80_0000, 0x3d80_155f),
            (0x3dcc_cccd, 0x3dcd_2494),
            (0x3f00_0000, 0x3f06_0a92),
            (0x3f5c_28f6, 0x3f84_83b8),
            (0x3f5e_b852, 0x3f87_10df),
            (0x3f66_6666, 0x3f8f_549b),
            (0x3f7d_70a4, 0x3fb6_f1e4),
            (0x3f7e_f9db, 0x3fbd_9c04),
            (0x3f7f_be77, 0x3fc3_5650),
            (0x3f7f_f972, 0x3fc7_4067),
            (0x3f80_0000, 0x3fc9_0fdb),
            (0xbf00_0000, 0xbf06_0a92),
            (0xbf80_0000, 0xbfc9_0fdb),
            (0x4000_0000, 0xffc0_0000),
        ];
        for (input, want) in cases {
            let got = libm_sse2_asinf(f(input));
            assert_eq!(
                b(got),
                want,
                "asinf({:#010x}) = {:#010x}, want {:#010x}",
                input,
                b(got),
                want
            );
        }
    }
}
