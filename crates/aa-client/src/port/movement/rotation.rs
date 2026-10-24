//! `CPlayerRotation` from the 1.2 `x2game.dll` decompile.
//!
//! Ported functions:
//! * ctor `FUN_3913ded0` (0x3913ded0)
//! * bind `FUN_3913dfd0` (0x3913dfd0)
//! * dispatch `FUN_391407b0` (0x391407b0)
//! * `ProcessNormal` `FUN_3913ff90` (0x3913ff90)
//! * `ProcessLadderRotation` `FUN_3913e6a0` (0x3913e6a0)
//!
//! `ClampAngles` (`FUN_3913e8d0`) and the air/ladder rotation paths are documented
//! in `docs/research/port-movement/unknowns.md` and not ported here yet.

/// The actor values the rotation ctor and the rotation math read.
#[derive(Clone, Copy, Debug, Default)]
pub struct RotationActor {
    /// `actor+0x2b0..0x2e0`: four quaternions.
    pub quats: [[f32; 4]; 4],
    /// `actor+0x33c..0x35c`: four support quaternions.
    pub support: [[f32; 4]; 4],
    /// `actor+0x960..0x968`.
    pub swim: [f32; 3],
    /// `actor+0x154/+0x158/+0x15c`: view limit pair and direction.
    pub view_limit: [f32; 3],
    /// `actor+0x40`.
    pub stride: f32,
    /// `anim+0xc8..0xd0`: desired heading/velocity used by `ProcessNormal`.
    pub desired: [f32; 3],
    /// `anim+0x4`, `anim+0xa8`, `actor+0x171`, `actor+0x18b` dispatch bytes.
    pub anim_4: f32,
    pub anim_a8: u8,
    pub actor_171: u8,
    pub actor_18b: u8,
    /// `actor+0x1c7` (ground alignment flag).
    pub ground_align: bool,
    /// `actor+0x1db` (on ladder).
    pub on_ladder: bool,
    /// `actor->vtable[0x168]()`: movement stance (`7` selects the narrow pitch clamp).
    pub stance: u32,
    /// `actor->vtable[0x160](1)`: ground blend factor used by `ClampAngles`.
    pub ground_factor: f32,
    /// `game_cfg+0x80/+0x84`: pitch limits blended by the ground factor.
    pub ground_cvar: [f32; 2],
}

/// `CPlayerRotation` value fields. Pointer fields (+0x04/+0x08/+0x0c and the
/// quaternion pointers +0x44/+0x48/+0x4c) are not needed by the ported math.
#[derive(Clone, Copy, Debug, Default)]
pub struct CRotation {
    pub scale: f32,
    pub stride: f32,
    /// `+0x14`, `+0x18`, `+0x1c`.
    pub move_rot: [f32; 3],
    /// `+0x20..0x2c`.
    pub q20: [f32; 4],
    /// `+0x30..0x3c`.
    pub q30: [f32; 4],
    /// `+0x40..0x4c`.
    pub q40: [f32; 4],
    /// `+0x50..0x5c` (the output quaternion copied by ladder/bind).
    pub q50: [f32; 4],
    /// `+0x60..0x6c`.
    pub q60: [f32; 4],
    /// `+0x70..0x7c`.
    pub q70: [f32; 4],
    /// `+0x80..0x88`.
    pub swim: [f32; 3],
    /// `+0x8c`, `+0x90`, `+0x94`.
    pub view_limit: [f32; 3],
}

impl CRotation {
    /// `FUN_3913ded0`: copy the actor transform and the 3-float request vector.
    pub fn new(actor: &RotationActor, request_move: [f32; 3], scale: f32) -> Self {
        Self {
            scale,
            stride: actor.stride,
            move_rot: request_move,
            q20: actor.quats[0],
            q30: actor.quats[1],
            q40: actor.quats[2],
            q50: [0.0; 4],
            q60: actor.support[0],
            q70: actor.support[1],
            swim: actor.swim,
            view_limit: actor.view_limit,
        }
    }

    /// `FUN_3913e6a0`: the ladder path copies the current orientation into the output.
    pub fn process_ladder(&mut self) {
        self.q50 = self.q40;
    }

    /// `FUN_391407b0`: branch selection.  Only the ladder branch and the plain
    /// normal branch are modelled; the air (`0x3913f020`) and ladder-transform
    /// (`0x3913f710`) paths are listed in the unknowns document.
    pub fn dispatch(&mut self, actor: &RotationActor) -> RotationPath {
        let path = if actor.anim_4 == 0.0 || actor.anim_a8 == 0 {
            if actor.actor_171 == 1 {
                RotationPath::Air
            } else if actor.actor_18b == 0 {
                self.process_normal(actor);
                RotationPath::Normal
            } else {
                self.process_ladder();
                RotationPath::Ladder
            }
        } else {
            RotationPath::Air
        };
        // `0x39140e25`: the tail of the dispatcher copies `+0x20..0x2c` into
        // `+0x30..0x3c` on every path, so the bound third quaternion is the first one.
        self.q30 = self.q20;
        path
    }

    /// `FUN_3913ff90`: align the current orientation to the desired direction and
    /// rebuild the movement quaternion.  Transcription of the decompiled scalar
    /// expressions (the ground-object branch at `actor+0x334` is not used here).
    pub fn process_normal(&mut self, actor: &RotationActor) {
        self.q50 = self.q40;
        let d = actor.desired;

        // Quaternion from the current orientation and the desired heading.
        let q = self.q40;
        let a = (q[2] * q[0] - q[3] * q[1]) * 2.0;
        let b = (q[3] * q[3] + q[0] * q[0]) * 2.0 - 1.0;
        let c = (q[3] * q[2] + q[1] * q[0]) * 2.0;

        let mut x = d[1] * a - d[2] * c;
        let mut y = d[2] * b - d[0] * a;
        let mut z = d[0] * c - d[1] * b;
        let n = rsqrt_newton(x * x + y * y + z * z + f32::MIN_POSITIVE);
        x *= n;
        y *= n;
        z *= n;

        let u = y * d[2] - z * d[1];
        let v = z * d[0] - d[2] * x;
        let w = d[1] * x - y * d[0];
        let t = y + u + d[2];

        let (mut qx, mut qy, mut qz, mut qw);
        if t <= 0.0 {
            if u < y || u < d[2] {
                if y < u || y < d[2] {
                    if d[2] < u || d[2] < y {
                        qx = 0.0;
                        qy = 0.0;
                        qz = 0.0;
                        qw = 1.0;
                    } else {
                        // z is the largest diagonal term.
                        let s = ((d[2] - u) - y + 1.0).sqrt();
                        let inv = 0.5 / s;
                        qx = (w + d[0]) * inv;
                        qy = (z + d[1]) * inv;
                        qz = s * 0.5;
                        qw = (v - x) * inv;
                    }
                } else {
                    // y is the largest diagonal term.
                    let s = ((y - d[2]) - u + 1.0).sqrt();
                    let inv = 0.5 / s;
                    qx = (x + v) * inv;
                    qy = s * 0.5;
                    qz = (z + d[1]) * inv;
                    qw = (d[0] - w) * inv;
                }
            } else {
                // u is the largest diagonal term.
                let s = ((u - y) - d[2] + 1.0).sqrt();
                let inv = 0.5 / s;
                qx = s * 0.5;
                qy = (x + v) * inv;
                qz = (d[0] + w) * inv;
                qw = (z - d[1]) * inv;
            }
        } else {
            let s = (t + 1.0).sqrt();
            let inv = 0.5 / s;
            qx = (z - d[1]) * inv;
            qy = (d[0] - w) * inv;
            qz = (v - x) * inv;
            qw = s * 0.5;
        }
        let _ = (&mut qx, &mut qy, &mut qz, &mut qw);
        // The decompile's branch expressions are kept verbatim above; the
        // branch body ordering is fixed below to the four stores it performs.
        self.q40 = [qx, qy, qz, qw];

        // `0x39140493`: q40 = q40 * q_z(this+0x1c), a local Z rotation.
        // The original builds q_rot = (0, 0, sin, cos) and post-multiplies it.
        let half = self.move_rot[2] * 0.5;
        let (sn, cs) = x87_sincos(half);
        let a = self.q40;
        self.q40 = [
            cs * a[0] + sn * a[1],
            cs * a[1] - sn * a[0],
            cs * a[2] + sn * a[3],
            cs * a[3] - sn * a[2],
        ];

        // `0x3914057e`: the SSE block builds A=(l50,s4c,f9,0), B=(-x,-y,-z,w)
        // and P = 2*cross(A,B)+A with the original lane shuffles.  The
        // decompiler's scalar expressions swap P.y/P.z, so the lane order is
        // reproduced literally here.
        let bq = self.q20;
        let s4c = (bq[3] * bq[3] + bq[1] * bq[1]) * 2.0 - 1.0;
        let l50 = (bq[0] * bq[1] - bq[2] * bq[3]) * 2.0;
        let f9 = (bq[0] * bq[3] + bq[2] * bq[1]) * 2.0;

        let a = self.q40;
        let aa = [l50, s4c, f9, 0.0];
        let bb = [-a[0], -a[1], -a[2], a[3]];
        let b_c9 = shuf(bb, bb, 0xc9);
        let b_12 = shuf(bb, bb, 0x12);
        let mut p0 = shuf(bb, bb, 0xff);
        let mut p2 = shuf(aa, aa, 0x12);
        let mut p3 = shuf(aa, aa, 0xc9);
        p0 = mul4(p0, aa);
        p2 = mul4(p2, b_c9);
        p3 = neg4(mul4(p3, b_12));
        p2 = add4(p2, p3);
        let q = add4(p0, p2);
        let mut r = mul4(shuf(q, q, 0xc9), b_12);
        r = neg4(r);
        let r2 = mul4(shuf(q, q, 0xd2), b_c9);
        r = add4(r, r2);
        r = add4(r, r);
        let p = add4(r, aa);
        let pitch = asinf(p[2]);

        // `0x391406a6`: q20 = q_x((this+0x14 + pitch)/2) * q40.
        let angle = (self.move_rot[0] + pitch) * 0.5;
        let (sn, cs) = x87_sincos(angle);
        let a = self.q40;
        self.q20 = [
            cs * a[0] + sn * a[3],
            cs * a[1] + sn * a[2],
            cs * a[2] - sn * a[1],
            cs * a[3] - sn * a[0],
        ];
    }
}


/// The parameter block at `rotation+0x04` read by `ClampAngles`
/// (`FUN_3913e8d0`); the vector is at `+0x28` and the four limits follow it.
#[derive(Clone, Copy, Debug, Default)]
pub struct ClampParams {
    /// `+0x28..0x30`: desired direction.
    pub dir: [f32; 3],
    /// `+0x34`: yaw wrap limit.
    pub yaw_wrap: f32,
    /// `+0x38`: pitch wrap limit.
    pub pitch_wrap: f32,
    /// `+0x3c`: lower pitch clamp.
    pub pitch_low: f32,
    /// `+0x40`: upper pitch clamp.
    pub pitch_high: f32,
}

impl CRotation {
    /// `FUN_3913e8d0` (`CPlayerRotation::ClampAngles`): clamp pitch/yaw to the
    /// stance/ladder limits and, with ground alignment, to the ground normal.
    ///
    /// The original stores `pitch = clamp(pitch + asin(angle)) - asin(angle)` and
    /// `yaw = wrap(yaw + atan2(...))`; the asin/atan2 calls are the original's
    /// `__libm_sse2_*` functions.
    pub fn clamp_angles(&mut self, actor: &RotationActor, params: &mut ClampParams) {
        let (mut lo, mut hi);
        if actor.stance == 7 {
            lo = -35.0;
            hi = 45.0;
        } else {
            lo = -80.0;
            hi = 80.0;
        }
        if actor.on_ladder {
            lo = -40.0;
            hi = 80.0;
        }
        let deg = 0.017_453_292_f32;
        lo *= deg;
        hi *= deg;

        // q20 matrix row used to align the orientation to the direction.
        let bq = self.q20;
        let s4c = (bq[3] * bq[3] + bq[1] * bq[1]) * 2.0 - 1.0;
        let l50 = (bq[0] * bq[1] - bq[2] * bq[3]) * 2.0;
        let f9 = (bq[0] * bq[3] + bq[2] * bq[1]) * 2.0;
        let a = self.q40;
        let aa = [l50, s4c, f9, 0.0];
        let bb = [-a[0], -a[1], -a[2], a[3]];
        let p = quat_cross_plus(aa, bb);
        let asin0 = asinf(p[2]);
        let v = self.move_rot[0] + asin0;
        let clamped = if lo > v { lo } else if v > hi { hi } else { v };
        self.move_rot[0] = clamped - asin0;

        // Ground alignment: blend the two cvars by the actor's ground factor and
        // lower the two nonzero limits to the resulting angle.
        if actor.ground_align {
            let c80 = actor.ground_cvar[0];
            let c84 = actor.ground_cvar[1];
            let ground = ((1.0 - actor.ground_factor) * (c84 - c80) + c80) * deg;
            if params.pitch_wrap != 0.0 && params.pitch_wrap > ground {
                params.pitch_wrap = ground;
            }
            if params.yaw_wrap != 0.0 && params.yaw_wrap > ground {
                params.yaw_wrap = ground;
            }
        }

        if params.yaw_wrap + params.pitch_wrap + params.pitch_low + params.pitch_high == 0.0 {
            return;
        }
        let dir = params.dir;
        if dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2] <= 0.1 {
            return;
        }

        // `0x3913ec2f`: q40 row (a,b,c) and cross(dir, row).
        let q = self.q40;
        let ma = (q[1] * q[3] + q[2] * q[0]) * 2.0;
        let mb = (q[2] * q[1] - q[3] * q[0]) * 2.0;
        let mc = (q[2] * q[2] + q[3] * q[3]) * 2.0 - 1.0;
        let px = dir[0];
        let py = dir[1];
        let pz = dir[2];
        let v0 = pz * ma - mc * px;
        let v1 = mb * px - py * ma;
        let v2 = py * mc - pz * mb;
        let len = ((v0 * v0 + v2 * v2) + v1 * v1) + f32::MIN_POSITIVE;
        let n = rsqrt_newton(len);
        let v0 = v0 * n;
        let v1 = v1 * n;
        let v2 = v2 * n;

        // `0x3913ed82`: the custom matrix layout passed to `FUN_39093ef0`.
        let mut m = [
            v2,
            px,
            v0 * pz - v1 * py,
            v0,
            py,
            v1 * px - pz * v2,
            v1,
            pz,
            py * v2 - v0 * px,
        ];
        inverse3x3(&mut m);

        // `0x3913edb7`: q20 row (a2,b2,c2), then rotate the direction.
        let bq = self.q20;
        let a2 = (bq[2] * bq[1] + bq[3] * bq[0]) * 2.0;
        let b2 = (bq[0] * bq[1] - bq[2] * bq[3]) * 2.0;
        let c2 = (bq[3] * bq[3] + bq[1] * bq[1]) * 2.0 - 1.0;
        let out0 = m[1] * c2 + b2 * m[0] + a2 * m[2];
        let out1 = m[4] * c2 + b2 * m[3] + a2 * m[5];
        let out2 = m[7] * c2 + b2 * m[6] + a2 * m[8];

        if params.pitch_wrap != 0.0 {
            let asin1 = asinf(out2);
            let v = self.move_rot[0] + asin1;
            let excess = params.pitch_wrap - v.abs();
            if excess < 0.0 {
                let sign = if v > 0.0 { 1.0 } else { -1.0 };
                self.move_rot[0] = sign * excess + self.move_rot[0];
            }
        }
        if params.pitch_low != 0.0 || params.pitch_high != 0.0 {
            let asin2 = asinf(out2);
            let v = self.move_rot[0] + asin2;
            if params.pitch_low <= v && params.pitch_low != 0.0 {
                self.move_rot[0] += params.pitch_low - v;
            }
            if v <= params.pitch_high && params.pitch_high != 0.0 {
                self.move_rot[0] += params.pitch_high - v;
            }
        }
        if params.yaw_wrap != 0.0 {
            let angle = super::libm_sse2_atan2(-out0 as f64, out1 as f64) as f32;
            let v = angle + self.move_rot[2];
            let excess = params.yaw_wrap - v.abs();
            if excess < 0.0 {
                let sign = if v > 0.0 { 1.0 } else { -1.0 };
                self.move_rot[2] = sign * excess + self.move_rot[2];
            }
        }
    }
}

/// The SIMD `P = 2*cross(A,B)+A` sequence shared by `ProcessNormal`
/// (`0x3914057e`) and `ClampAngles` (`0x3913ea6b`).
fn quat_cross_plus(aa: [f32; 4], bb: [f32; 4]) -> [f32; 4] {
    let b_c9 = shuf(bb, bb, 0xc9);
    let b_12 = shuf(bb, bb, 0x12);
    let mut p0 = shuf(bb, bb, 0xff);
    let mut p2 = shuf(aa, aa, 0x12);
    let mut p3 = shuf(aa, aa, 0xc9);
    p0 = mul4(p0, aa);
    p2 = mul4(p2, b_c9);
    p3 = neg4(mul4(p3, b_12));
    p2 = add4(p2, p3);
    let q = add4(p0, p2);
    let mut r = neg4(mul4(shuf(q, q, 0xc9), b_12));
    let r2 = mul4(shuf(q, q, 0xd2), b_c9);
    r = add4(r, r2);
    r = add4(r, r);
    add4(r, aa)
}

/// `FUN_39093ef0`: in-place inverse of the 9-float matrix layout used by
/// `ClampAngles`, with the original's determinant guard and scale.
pub fn inverse3x3(m: &mut [f32; 9]) {
    let a = m[0];
    let b = m[1];
    let c = m[2];
    let d = m[3];
    let e = m[4];
    let f = m[5];
    let g = m[6];
    let h = m[7];
    let i = m[8];

    let p = e * i - h * f;
    let q = c * h - b * i;
    let r = b * f - c * e;
    let s = g * f - d * i;
    let t = a * i - g * c;
    let u = d * c - a * f;
    let v = d * h - g * e;
    let w = g * b - a * h;
    let x = a * e - d * b;

    m[6] = v;
    m[7] = w;
    m[8] = x;
    m[0] = p;
    m[1] = q;
    m[2] = r;
    m[3] = s;
    m[4] = t;
    m[5] = u;

    let det = q * d + p * a + r * g;
    if det.abs() < 9.999_999_7e-21 {
        return;
    }
    let scale = 1.0 / det;
    for value in m.iter_mut() {
        *value *= scale;
    }
}

/// `FUN_39135ed0` (`CPlayer::ProcessFrameMovementRotation`): the frame
/// wrapper builds the rotation from the actor/request, dispatches it and binds
/// the normalised quaternions back to the actor.
pub fn process_frame_rotation(
    actor: &RotationActor,
    request_move: [f32; 3],
    scale: f32,
) -> (CRotation, BoundActor, RotationPath) {
    let mut rot = CRotation::new(actor, request_move, scale);
    let path = rot.dispatch(actor);
    let bound = rot.bind();
    (rot, bound, path)
}

/// The actor fields written by `FUN_3913dfd0` (bind).
#[derive(Clone, Copy, Debug, Default)]
pub struct BoundActor {
    /// `+0x2b0..0x2dc`: four normalised quaternions in memory order.
    pub move_quats: [[f32; 4]; 4],
    /// `+0x33c..0x35c`: two normalised support quaternions.
    pub support_quats: [[f32; 4]; 2],
    /// `+0x950..0x95c`.
    pub out_pos: [f32; 4],
    /// `+0x960..0x968`.
    pub swim: [f32; 3],
    /// `+0x154..0x15c`.
    pub view_limit: [f32; 3],
}

impl CRotation {
    /// `FUN_3913dfd0`: write the normalised quaternions back to the actor.
    pub fn bind(&self) -> BoundActor {
        BoundActor {
            move_quats: [
                normalize4(self.q20),
                normalize4(self.q30),
                normalize4(self.q40),
                [0.0; 4],
            ],
            support_quats: [normalize4(self.q60), normalize4(self.q70)],
            out_pos: self.q50,
            swim: self.swim,
            view_limit: self.view_limit,
        }
    }
}

fn normalize4(q: [f32; 4]) -> [f32; 4] {
    let len = q[3] * q[3] + q[0] * q[0] + q[1] * q[1] + q[2] * q[2];
    let n = rsqrt_newton(len);
    [q[0] * n, q[1] * n, q[2] * n, q[3] * n]
}

/// `shufps` with the same lane selection as the original (`0xc9`, `0x12`,
/// `0xd2`, `0xff`).  Lanes 0/1 come from `d`, lanes 2/3 from `s`.
#[inline]
fn shuf(d: [f32; 4], s: [f32; 4], imm: u8) -> [f32; 4] {
    let i = |n: u8| ((imm >> n) & 3) as usize;
    [d[i(0)], d[i(2)], s[i(4)], s[i(6)]]
}

#[inline]
fn add4(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]]
}

#[inline]
fn mul4(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]]
}

#[inline]
fn neg4(a: [f32; 4]) -> [f32; 4] {
    [neg(a[0]), neg(a[1]), neg(a[2]), neg(a[3])]
}

/// `__libm_sse2_asinf` from MSVCR100, ported instruction by instruction in `asinf.rs`.
#[inline]
fn asinf(x: f32) -> f32 {
    super::libm_sse2_asinf(x)
}

/// x87 `fsincos` rounded to `f32` (the original stores both results as `f32`).
/// Computing in `f64` and rounding once reproduces the 64-bit x87 significand
/// except for double-rounding cases; the goldens verify the result.
#[inline]
fn x87_sincos(angle: f32) -> (f32, f32) {
    let (s, c) = (angle as f64).sin_cos();
    (s as f32, c as f32)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RotationPath {
    Normal,
    Ladder,
    Air,
}

#[inline]
fn neg(v: f32) -> f32 {
    f32::from_bits(v.to_bits() ^ 0x8000_0000)
}

/// `rsqrtss` + one Newton step, as in `0x3913dfd0`.
#[inline]
pub fn rsqrt_newton(len: f32) -> f32 {
    #[cfg(target_arch = "x86_64")]
    {
        use core::arch::x86_64::{_mm_cvtss_f32, _mm_rsqrt_ss, _mm_set_ss};
        let n = unsafe { _mm_cvtss_f32(_mm_rsqrt_ss(_mm_set_ss(len))) };
        (1.5 - n * len * n * 0.5) * n
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let n = 1.0 / len.sqrt();
        (1.5 - n * len * n * 0.5) * n
    }
}
