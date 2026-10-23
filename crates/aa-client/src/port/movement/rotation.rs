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
        if actor.anim_4 == 0.0 || actor.anim_a8 == 0 {
            if actor.actor_171 == 1 {
                return RotationPath::Air;
            }
            if actor.actor_18b == 0 {
                self.process_normal(actor);
                return RotationPath::Normal;
            }
            self.process_ladder();
            return RotationPath::Ladder;
        }
        RotationPath::Air
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

        // Yaw rotation by this+0x1c around the desired heading (the live
        // function builds the same rotation from the desired vector).
        let angle = self.move_rot[2];
        let axis_len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        let (sn, cs) = (angle * 0.5).sin_cos();
        let ax = d[0] / axis_len * sn;
        let ay = d[1] / axis_len * sn;
        let az = d[2] / axis_len * sn;
        let q = self.q40;
        self.q40 = [
            cs * q[0] + ax * q[3] + ay * q[2] - az * q[1],
            cs * q[1] - ax * q[2] + ay * q[3] + az * q[0],
            cs * q[2] + ax * q[1] - ay * q[0] + az * q[3],
            cs * q[3] - ax * q[0] - ay * q[1] - az * q[2],
        ];

        // Combine q40 with q20 and build the pitch quaternion.
        let bq = self.q20;
        let s4c = (bq[3] * bq[3] + bq[1] * bq[1]) * 2.0 - 1.0;
        let l50 = (bq[0] * bq[1] - bq[2] * bq[3]) * 2.0;
        let f9 = (bq[0] * bq[3] + bq[2] * bq[1]) * 2.0;

        let a = self.q40;
        let na = neg(a[0]);
        let nb = neg(a[1]);
        let nc = neg(a[2]);
        let nd = a[3];
        let f10 = nd * l50 + f9 * nb + neg(s4c * nc);
        let f13 = nd * s4c + l50 * nc + neg(f9 * na);
        let f14 = nd * f9 + s4c * na + neg(l50 * nb);
        let f17 = nd * 0.0 + l50 * nd + neg(na * 0.0);
        let f8 = neg(f13 * nc) + f14 * nb;
        let f14b = neg(f14 * na) + f10 * nc;
        let f10b = neg(f10 * nb) + f13 * na;
        let f16 = neg(f17 * na) + f17 * nd;
        let l50b = f8 + f8 + l50;
        let s4cb = f14b + f14b + s4c;
        let f9b = f10b + f10b + f9;
        let _f44 = f16 + f16 + 0.0;
        let pitch = f9b.asin();
        let _ = (l50b, s4cb, pitch);

        // Rotate q20 by the pitch (this+0x14 + angle) * 0.5.
        let angle = (self.move_rot[0] + pitch) * 0.5;
        let (sn, cs) = angle.sin_cos();
        let q = self.q40;
        self.q20 = [
            sn * q[3] + (q[1] * 0.0 - q[2] * 0.0) + cs * q[0],
            (sn * q[2] - q[0] * 0.0) + q[3] * 0.0 + cs * q[1],
            (q[0] * 0.0 - sn * q[1]) + q[3] * 0.0 + cs * q[2],
            cs * q[3] - (sn * q[0] + q[1] * 0.0 + q[2] * 0.0),
        ];
    }
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
