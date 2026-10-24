//! Quadruped / mounted / glider movement, ported from the 1.2 `x2game.dll` decompile.
//!
//! Ported functions (each names its FUN_ address):
//! * `FUN_3912dd40` dispatch
//! * `FUN_3912ac50` apply to the actor / graph
//! * `FUN_3912b330` ground movement and the jump impulse
//! * `FUN_3912c770` mounted / vehicle movement
//! * `FUN_3912d860` glider flight (lift, fall, steering, smoothing)
//! * `FUN_3912b280` straight glide
//! * `FUN_3912bfb0` final ground-align / speed pass
//!
//! Engine services the original reaches through vtables/globals (entity position, physics
//! status, ground rays, graph inputs, sounds, collision) are explicit inputs; their original
//! call sites are named in the comments.  Constants are the 1.2 `.data` values.

use super::direction_factor;
use super::MovementParams;

pub const K_ZERO: f32 = 0.0; // DAT_3999c484
pub const K_ONE: f32 = 1.0; // DAT_3999c488
pub const K_TWO: f32 = 2.0; // DAT_3999cffc
pub const K_HALF: f32 = 0.5; // DAT_3999c47c
pub const K_POINT_01: f32 = 0.01; // DAT_3999cf90
pub const K_POS_X: f32 = 0.1; // DAT_3999ccf0
pub const K_NEG_X: f32 = -0.1; // DAT_399a2f24
pub const K_RANGE_02: f32 = 0.2; // DAT_3999dd34
pub const K_ONE_POINT_FIVE: f32 = 1.5; // DAT_3999c478
pub const K_SQRT_EPS: f32 = 1e-10; // DAT_399aa82c
pub const K_GLIDE_SMOOTH_1: f32 = 0.48; // DAT_399a2998
pub const K_GLIDE_SMOOTH_2: f32 = 0.235; // DAT_399a2994
pub const K_SIXTH: f32 = 0.166_666_67;

/// `actor+0x50` animation block fields read by the movement functions (offsets relative to
/// the block, i.e. `+0x15c` is `actor+0x1ac`).
#[derive(Clone, Copy, Debug, Default)]
pub struct QuadBlock {
    /// `+0x0c`: jump scalar.
    pub jump_scalar: f32,
    /// `+0x9c..0xa4`: jump direction.
    pub jump_dir: [f32; 3],
    /// `+0xd4..0xdc`: external jump impulse.
    pub jump_extra: [f32; 3],
    /// `+0x118`: slow move speed.
    pub speed_slow: f32,
    /// `+0x11c`: normal move speed.
    pub speed_normal: f32,
    /// `+0x120`: fast move speed.
    pub speed_fast: f32,
    /// `+0x128`: startup time.
    pub startup_time: f32,
    /// `+0x12c`: startup fall speed.
    pub fall_startup: f32,
    /// `+0x130`: slow fall speed.
    pub fall_slow: f32,
    /// `+0x134`: normal fall speed.
    pub fall_normal: f32,
    /// `+0x138`: fast fall speed.
    pub fall_fast: f32,
    /// `+0x13c`: vertical smooth time.
    pub smooth_time: f32,
    /// `+0x140`: lifts left.
    pub lift_count: i32,
    /// `+0x144`: lift ray length.
    pub lift_height: f32,
    /// `+0x148`: earliest lift time.
    pub lift_valid: f32,
    /// `+0x14c`: lift duration.
    pub lift_duration: f32,
    /// `+0x150`: lift speed.
    pub lift_speed: f32,
    /// `+0x15c`: flight elapsed.
    pub elapsed: f32,
    /// `+0x160`: lift time left.
    pub lift_time: f32,
    /// `+0x168`: smoothed vertical velocity.
    pub smoothed_v: f32,
    /// `+0x16c`: smoothed vertical derivative.
    pub smoothed_dv: f32,
    /// `+0x101`: final-pass early-out.
    pub block_101: u8,
    /// `+0x180`: fixed-speed flight.
    pub fixed_flight: u8,
    /// `+0x188` / `+0x18a`: input-flight flags.
    pub flag_188: u8,
    pub flag_18a: u8,
    /// `+0xa8`: mounted flag.
    pub mounted: u8,
    /// `+0x171`: stance override.
    pub stance_171: u8,
    /// `+0x1c1`: movement state.
    pub state_1c1: u8,
    /// `+0x1d0` / `+0x1d8` / `+0x1da` / `+0x1d9`.
    pub flag_1d0: u8,
    pub flag_1d8: u8,
    pub flag_1da: u8,
    pub flag_1d9: u8,
    /// `+0x1d4`: fixed-flight speed.
    pub fixed_speed: f32,
}

/// Actor fields outside the animation block.
#[derive(Clone, Copy, Debug, Default)]
pub struct QuadActor {
    /// `actor+0x2d0`: movement quaternion.
    pub quat: [f32; 4],
    /// `actor+0x2e0`: movement velocity.
    pub velocity: [f32; 3],
    /// `actor+0x2e8`: movement scalar.
    pub movement_scalar: f32,
    /// `actor+0x96c` / `+0x974`: swim control pairs.
    pub swim_a: [f32; 2],
    pub swim_b: [f32; 2],
    /// `actor+0x9dc`.
    pub val_9dc: f32,
    /// `actor+0x284`.
    pub val_284: f32,
    /// `actor+0x40`.
    pub stride: f32,
    /// `actor+0x161`.
    pub falling: u8,
    /// `actor+0x78` / `+0x79`.
    pub flag_78: u8,
    pub flag_79: u8,
    /// `actor+0x98` / `+0x99`.
    pub airborne: u8,
    pub jumped: u8,
    /// `actor+0x1d9`.
    pub slide: u8,
    /// `actor+0x1c7`.
    pub ground_align_off: u8,
    /// `actor+0x111`.
    pub swim_flag: u8,
    /// `actor+0x160` / `+0x161`: jump event flags.
    pub flag_161: u8,
    /// `actor+0x5c`.
    pub val_5c: f32,
    /// `actor+0x2ec..0x304`: applied velocity extras.
    pub movement_extra: [f32; 3],
    /// `actor+0x918`.
    pub val_918: u32,
    /// `actor+0x1d4`.
    pub fixed_speed: f32,
    /// `actor+0x410` of the referenced unit.
    pub unit_stance: u32,
}

/// Game parameters the movement functions read from `DAT_39aefb98` (the cvar/game-params
/// object).  Offsets are the original `+0x194..+0x1bc` reads.
#[derive(Clone, Copy, Debug, Default)]
pub struct QuadConfig {
    /// `+0x194`: horizontal speed scale.
    pub speed_scale: f32,
    /// `+0x19c`: vertical speed scale (`FUN_3912c770`).
    pub vertical_scale: f32,
    /// `+0x1a0`: global movement scale (`FUN_3912c770`).
    pub movement_scale: f32,
    /// `+0x1b8`: movement acceleration time (`FUN_3912c770`).
    pub accel_time: f32,
    /// `+0x1bc`: idle friction time (`FUN_3912c770`).
    pub friction_time: f32,
    /// `+0xe8`: swim speed scale (`FUN_3912b330`).
    pub swim_scale: f32,
}

/// Controller object fields.
#[derive(Clone, Copy, Debug, Default)]
pub struct QuadController {
    /// `+0x00`: frame time / scale.
    pub dt: f32,
    /// `+0x0c..0x14`: request direction.
    pub input: [f32; 3],
    /// `+0x31`: jump button.
    pub jump_button: u8,
    /// `+0x5c`: bit 0 selects the strafe branch in `FUN_3912c770`.
    pub flags_5c: u32,
    /// `+0x60`: mode output.
    pub mode: u32,
    /// `+0x64..0x6c`: velocity output.
    pub velocity: [f32; 3],
    /// `+0x8c..0x94`: secondary velocity.
    pub spare_velocity: [f32; 3],
    /// `+0x98` / `+0x99`.
    pub airborne: u8,
    pub jumped: u8,
    /// `+0x48` quaternion value.
    pub quat: [f32; 4],
    /// `+0x70..0x7c`: apply extras.
    pub movement_extra_70: f32,
    pub movement_extra_74: f32,
    pub movement_extra_78: f32,
    pub movement_extra_7c: f32,
    /// `+0xa4` / `+0xa8`: apply extras.
    pub val_a4: u32,
    pub flag_a8: u8,
}

/// Results the original obtains from the engine.
#[derive(Clone, Copy, Debug, Default)]
pub struct QuadExternals {
    /// entity position (`entity vtable+0x98`).
    pub entity_pos: [f32; 3],
    /// entity orientation (`entity vtable+0xa0`, final pass).
    pub entity_quat: [f32; 4],
    /// physics orientation (`phys vtable+0x24`).
    pub phys_quat: [f32; 4],
    /// lift ray success in `FUN_3912d860`.
    pub lift_ray_hit: bool,
    /// ground ray `(vtable+0x158)` result in `FUN_3912c770`.
    pub move_ray: Option<[f32; 3]>,
    /// `FUN_39111d60` (controlled unit non-null).
    pub controlled_unit: bool,
    /// `FUN_391136c0` (mounted / special vehicle).
    pub mounted_unit: bool,
    /// Physics status velocity (`GetStatus` -> `+0x0c..0x14`); `None` when the entity has
    /// no physical entity or the status query fails.
    pub phys_velocity: Option<[f32; 3]>,
    /// `FUN_39113800` (speed multiplier, 0 when a UI flag blocks it).
    pub speed_multiplier: f32,
    /// `FUN_39111f40` (above support).
    pub above_support: bool,
    /// current animation contains `jump_in_place`.
    pub jump_in_place: bool,
    /// `actor->vtable+0xa8` swim flag.
    pub swimming: bool,
    /// `actor->vtable+0x68` swim-stance flag.
    pub is_swim_stance: bool,
}

fn rot(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let x = q[0];
    let y = q[1];
    let z = q[2];
    let w = q[3];
    let tx = 2.0 * (y * v[2] - z * v[1]);
    let ty = 2.0 * (z * v[0] - x * v[2]);
    let tz = 2.0 * (x * v[1] - y * v[0]);
    [
        v[0] + w * tx + (y * tz - z * ty),
        v[1] + w * ty + (z * tx - x * tz),
        v[2] + w * tz + (x * ty - y * tx),
    ]
}

fn rsqrt(len: f32) -> f32 {
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

fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let l = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    if l <= K_ZERO {
        return [0.0; 3];
    }
    let n = rsqrt(l);
    [v[0] * n, v[1] * n, v[2] * n]
}

/// `FUN_3912b280` (0x3912b280): straight glide.  Mode 1, velocity = quat * block+0x11c.
pub fn glide_straight(ctl: &mut QuadController, block: &QuadBlock) {
    ctl.mode = 1;
    let speed = block.speed_normal;
    let q = ctl.quat;
    ctl.velocity = [
        (q[0] * q[1] - q[2] * q[3]) * K_TWO * speed,
        ((q[3] * q[3] + q[1] * q[1]) * K_TWO - K_ONE) * speed,
        (q[3] * q[0] + q[2] * q[1]) * K_TWO * speed,
    ];
}

/// `FUN_3912d860` (0x3912d860): glider flight.
///
/// `dt` is the frame time the original reads from `gEnv -> timer vtable+0x30`.
pub fn glide(ctl: &mut QuadController, block: &mut QuadBlock, ext: &QuadExternals, dt: f32) {
    ctl.mode = 2;
    block.elapsed += dt;
    block.lift_time -= dt;

    let (move_speed, fall_speed);
    if block.elapsed <= block.startup_time {
        move_speed = block.speed_normal;
        fall_speed = block.fall_startup;
    } else if ctl.input[1] <= K_POS_X {
        if ctl.input[1] >= K_NEG_X {
            move_speed = block.speed_normal;
            fall_speed = -block.fall_normal;
        } else {
            move_speed = block.speed_slow;
            fall_speed = -block.fall_slow;
        }
    } else {
        move_speed = block.speed_fast;
        fall_speed = -block.fall_fast;
    }

    let q = ctl.quat;
    ctl.velocity = [
        (q[0] * q[1] - q[2] * q[3]) * K_TWO * move_speed,
        ((q[3] * q[3] + q[1] * q[1]) * K_TWO - K_ONE) * move_speed,
        (q[3] * q[0] + q[2] * q[1]) * K_TWO * move_speed,
    ];
    ctl.velocity[2] = fall_speed;

    if block.lift_count > 0
        && block.elapsed >= block.lift_valid
        && ctl.input[1] <= K_NEG_X
        && block.lift_time <= K_ZERO
        && ext.lift_ray_hit
    {
        block.lift_time = block.lift_count as f32 * block.lift_duration;
        block.lift_count -= 1;
    }

    let mut vertical = -block.smooth_time;
    if block.lift_time > K_ZERO {
        vertical *= K_RANGE_02;
        ctl.velocity[2] = block.lift_speed;
    }

    if (block.smoothed_v - ctl.velocity[2]).abs() >= K_POINT_01 {
        let mut smooth_dt = ext.entity_pos[0];
        if smooth_dt == 0.0 {
            smooth_dt = dt;
        }
        if block.smooth_time <= K_ZERO {
            if smooth_dt <= K_ZERO {
                block.smoothed_v = ctl.velocity[2];
                block.smoothed_dv = 0.0;
            } else {
                block.smoothed_dv = (ctl.velocity[2] - block.smoothed_v) / smooth_dt;
                block.smoothed_v = ctl.velocity[2];
            }
        } else {
            let inv = K_ONE / block.smooth_time;
            let x = inv * smooth_dt;
            let den = (x * K_GLIDE_SMOOTH_1 * x + x + K_ONE) + x * K_GLIDE_SMOOTH_2 * x * x;
            let scale = K_ONE / den;
            let delta = block.smoothed_v - ctl.velocity[2];
            let acc = (delta * inv + block.smoothed_dv) * smooth_dt;
            block.smoothed_dv = (block.smoothed_dv - acc * inv) * scale;
            block.smoothed_v = (acc + delta) * scale + ctl.velocity[2];
        }
        ctl.velocity[2] = block.smoothed_v;
    } else {
        block.smoothed_dv = 0.0;
        block.smoothed_v = ctl.velocity[2];
    }
    let _ = vertical;
}

/// `FUN_3912ac50` (0x3912ac50): copy the controller to the actor.
///
/// The animation-object branch (`actor+0x24` non-null) calls the graph; the captured paths
/// leave it null and exercise the field copy, which is transcribed here.  `actor+0x160` is
/// the "jumped" event byte.
pub fn apply(ctl: &QuadController, actor: &mut QuadActor) -> ApplyOutput {
    let mut out = ApplyOutput::default();
    if ctl.jumped == 0 {
        actor.flag_161 = 0;
    } else {
        out.jumped_event = true;
        actor.flag_161 = 1;
    }
    if ctl.airborne != 0 && actor.falling == 0 && actor.val_5c != 0.0 {
        actor.flag_161 = 1;
        actor.val_5c = 0.0;
    }
    actor.velocity = [ctl.spare_velocity[0], ctl.spare_velocity[1], ctl.spare_velocity[2]];
    actor.movement_scalar = ctl.velocity[2];
    actor.swim_a = [ctl.velocity[0], ctl.velocity[1]];
    actor.swim_b = [ctl.velocity[2], ctl.movement_extra_70];
    actor.movement_extra = [ctl.movement_extra_74, ctl.movement_extra_78, ctl.movement_extra_7c];
    actor.flag_78 = ctl.flag_a8;
    actor.val_918 = ctl.val_a4;
    out
}

/// Fields `FUN_3912ac50` writes to the actor.
#[derive(Clone, Copy, Debug, Default)]
pub struct ApplyOutput {
    pub jumped_event: bool,
}

/// `FUN_3912dd40` (0x3912dd40): choose the movement mode.
///
/// Returns the selected mode; the caller can compare it with the captured original.
pub fn dispatch(
    ctl: &mut QuadController,
    block: &mut QuadBlock,
    actor: &mut QuadActor,
    ext: &QuadExternals,
    cfg: &QuadConfig,
    back_scale: f32,
    graph: &mut Vec<(&'static str, [f32; 3])>,
) -> DispatchPath {
    if block.fixed_flight != 0 {
        return DispatchPath::FixedFlight;
    }
    if block.flag_188 != 0 || block.flag_18a != 0 {
        return DispatchPath::InputFlight;
    }
    let path = if block.mounted != 0 {
        mounted(ctl, ext, cfg, back_scale);
        DispatchPath::Mounted
    } else {
        match block.stance_171 {
            1 => {
                ctl.mode = 1;
                ctl.velocity = [0.0; 3];
                DispatchPath::Zero
            }
            4 => {
                glide(ctl, block, ext, ctl.dt);
                DispatchPath::Glide
            }
            5 => {
                glide_straight(ctl, block);
                DispatchPath::GlideStraight
            }
            _ => {
                if ext.is_swim_stance {
                    DispatchPath::Swim
                } else {
                    ground(ctl, block, actor, ext, graph);
                    DispatchPath::Ground
                }
            }
        }
    };
    if actor.ground_align_off == 0 {
        final_pass(ctl, block, actor, ext);
    }
    path
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DispatchPath {
    FixedFlight,
    InputFlight,
    Mounted,
    Zero,
    Glide,
    GlideStraight,
    Swim,
    Ground,
}

/// `FUN_3912bfb0` (0x3912bfb0): final ground align and cvar speed scale.
pub fn final_pass(
    ctl: &mut QuadController,
    block: &mut QuadBlock,
    actor: &mut QuadActor,
    ext: &QuadExternals,
) {
    if block.block_101 != 0 || actor.ground_align_off != 0 {
        return;
    }
    let q = ext.entity_quat;
    let len = q[3] * q[3] + q[0] * q[0] + q[1] * q[1] + q[2] * q[2];
    let n = rsqrt(len);
    let q = [q[0] * n, q[1] * n, q[2] * n, q[3] * n];
    let sx = f32::from_bits(q[0].to_bits() ^ 0x8000_0000);
    let sy = f32::from_bits(q[1].to_bits() ^ 0x8000_0000);
    let sz = f32::from_bits(q[2].to_bits() ^ 0x8000_0000);
    let sw = f32::from_bits(q[3].to_bits() ^ 0x8000_0000);
    let r = ctl.quat;
    let (mut a, mut b, mut c, mut d);
    if !ext.is_swim_stance {
        a = (sy * r[2] - sw * r[1]) + q[3] * r[0] + r[3] * sx;
        b = (sw * r[0] - sx * r[2]) + q[3] * r[1] + sy * r[3];
        c = r[1] * sx - sy * r[0];
        d = q[3] * r[3] - (sx * r[1] + sy * r[0] + sz * r[2]);
    } else {
        let u = (q[2] * q[0] + q[3] * q[1]) * K_TWO;
        let v = (q[3] * q[3] + q[1] * q[1]) * K_TWO - K_ONE;
        let w = (q[2] * q[1] - q[3] * q[0]) * K_TWO;
        let l = r[0] * r[0] + r[1] * r[1] + r[2] * r[2] + K_ONE;
        let ln = rsqrt(l);
        let lx = r[0] * ln;
        let ly = r[1] * ln;
        let lz = r[2] * ln;
        a = ly * w * K_TWO - r[2] * ln * u;
        b = r[2] * ln * v - w * lx;
        c = u * lx - ly * v;
        let l2 = a * a + b * b + c * c + K_ONE;
        let _ = rsqrt(l2);
        d = 0.0;
        if block.jump_scalar != 0.0 && block.mounted != 0 {
            a = (sy * r[2] - sw * r[1]) + q[3] * r[0] + d * r[3];
            b = (sw * r[0] - d * r[2]) + q[3] * r[1] + sy * r[3];
            c = (d * r[1] - sy * r[0]) + q[3] * r[2] + sw * r[3];
            d = q[3] * r[3] - (sy * r[1] + d * r[0] + sw * r[2]);
            ctl.spare_velocity = [a, b, c];
            return;
        }
        let r1 = r[1];
        a = (sy * r[2] - sw * r1) + q[3] * r[0] + sx * r[3];
        b = (sw * r[0] - sx * r[2]) + q[3] * r1 + sy * r[3];
        c = sx * r1 - sy * r[0];
        d = q[3] * r[3] - (sx * r1 + sy * r[0] + sz * r[2]);
    }
    let mut out = [a, b, c, d];
    let l = out[0] * out[0] + out[1] * out[1] + out[2] * out[2] + out[3] * out[3];
    let n = rsqrt(l);
    for v in &mut out {
        *v *= n;
    }
    ctl.spare_velocity = [out[0], out[1], out[2]];
}

/// `FUN_3912b330` (0x3912b330): ground movement and the jump impulse.
pub fn ground(
    ctl: &mut QuadController,
    block: &QuadBlock,
    actor: &QuadActor,
    ext: &QuadExternals,
    graph: &mut Vec<(&'static str, [f32; 3])>,
) {
    if !ext.controlled_unit {
        return;
    }
    let mut dir = [0.0f32; 3];
    let mut planar = [0.0f32; 3];
    if ctl.input[0] != 0.0 || ctl.input[1] != 0.0 {
        if !ext.mounted_unit {
            let input = [ctl.input[0], ctl.input[1], ctl.input[2]];
            let mut norm = input;
            let l = input[0] * input[0] + input[1] * input[1] + input[2] * input[2];
            if K_ONE < l {
                let inv = K_ONE / l.sqrt();
                norm = [input[0] * inv, input[1] * inv, input[2] * inv];
            }
            let factor = direction_factor(
                &MovementParams::default(),
                actor.stride.to_bits() & 0x1000 != 0,
                [norm[0], norm[1]],
            );
            let q = ctl.quat;
            let r = rot(q, norm);
            planar = [r[0] * factor, r[1] * factor, r[2] * factor];
        } else {
            let q = ctl.quat;
            let a = ctl.input[0];
            let b = ctl.input[1];
            planar = [
                (q[0] * q[0] + q[3] * q[3]) * K_TWO * b + (q[0] * q[1] - q[2] * q[3]) * K_TWO * a,
                (q[1] * q[3] + q[2] * q[0]) * K_TWO * a + (q[1] * q[1] + q[2] * q[2]) * K_TWO * b,
                (q[3] * q[1] + q[2] * q[0]) * K_TWO * b + (q[0] * q[1] - q[2] * q[3]) * K_TWO * a,
            ];
        }
        dir = planar;
    }
    let mut local = [0.0f32; 3];
    if planar != [0.0; 3] || ext.speed_multiplier != 0.0 {
        local = [
            planar[0] * ext.speed_multiplier,
            planar[1] * ext.speed_multiplier,
            planar[2] * ext.speed_multiplier,
        ];
    }
    let q = ctl.quat;
    let f = K_TWO;
    let horizontal = [
        (q[1] * q[1] + q[3] * q[3]) * f - K_ONE,
        (q[0] * q[1] - q[2] * q[3]) * f,
        (q[0] * q[1] + q[2] * q[3]) * f,
    ];
    ctl.mode = 1;
    let mut jump = [0.0f32; 3];
    if ctl.jump_button != 0 && ctl.airborne == 0 {
        if ctl.input[0].abs() > K_POS_X || ctl.input[1].abs() > K_POS_X {
            if block.jump_scalar > K_ZERO {
                ctl.mode = 7;
                let mag = (block.jump_dir[0] * block.jump_dir[0]
                    + block.jump_dir[1] * block.jump_dir[1]
                    + block.jump_dir[2] * block.jump_dir[2])
                    .sqrt();
                if mag > K_ZERO {
                    let height = actor.stride;
                    let n = (mag * K_TWO * height).sqrt() / mag;
                    let up = n - if ext.above_support { K_HALF } else { 0.0 };
                    jump = [block.jump_dir[0] * up, block.jump_dir[1] * up, block.jump_dir[2] * up];
                }
                if block.jump_extra[0] != 0.0
                    || block.jump_extra[1] != 0.0
                    || block.jump_extra[2] != 0.0
                {
                    let speed = (block.jump_extra[0] * block.jump_extra[0]
                        + block.jump_extra[1] * block.jump_extra[1]
                        + block.jump_extra[2] * block.jump_extra[2])
                        .sqrt();
                    let s = (block.jump_extra[2] - 0.25) * K_TWO;
                    let s = s.clamp(0.0, K_ONE);
                    jump = [
                        block.jump_extra[0] + (K_ONE - block.jump_extra[0]) * s,
                        block.jump_extra[1] + (K_ONE - block.jump_extra[1]) * s,
                        block.jump_extra[2] + (K_ONE - block.jump_extra[2]) * s,
                    ];
                    jump = [jump[0] * speed, jump[1] * speed, jump[2] * speed];
                }
                ctl.airborne = 1;
                ctl.jumped = 1;
            }
        } else if !ext.jump_in_place {
            ctl.airborne = 1;
            ctl.jumped = 1;
            if ext.mounted_unit {
                ctl.airborne = 1;
            }
        }
    }
    if ext.speed_multiplier != 0.0 {
        graph.push(("jumpVec", jump));
    }
    ctl.velocity = [
        horizontal[0] + local[0] + jump[0],
        horizontal[1] + local[1] + jump[1],
        horizontal[2] + local[2] + jump[2],
    ];
    ctl.spare_velocity = [0.0; 3];
}

/// `FUN_3912c770` (0x3912c770): mounted / vehicle movement.
///
/// `back_scale` is the actor movement block's `+0x5c` (`actor+0x448`, the backward
/// speed factor).  The controller quaternion at `ctl+0x44` is `actor+0x2b0`; the caller
/// keeps it in `ctl.quat` together with the actor's `+0x2b0` value.
pub fn mounted(
    ctl: &mut QuadController,
    ext: &QuadExternals,
    cfg: &QuadConfig,
    back_scale: f32,
) {
    if let Some(v) = ext.phys_velocity {
        ctl.spare_velocity = v;
    }
    let mut x = ctl.input[0];
    let mut y = ctl.input[1];
    let mut z = if ctl.flags_5c & 1 != 0 { K_ONE } else { K_ZERO };
    let len = y * y + x * x + z * z;
    if len > 1.192_092_9e-7 {
        let n = rsqrt(len);
        x *= n;
        y *= n;
        z *= n;
    } else {
        x = 0.0;
        y = 0.0;
        z = 0.0;
    }
    if !ext.mounted_unit {
        let scale = if 0.0 <= y { K_ONE } else { back_scale };
        x *= 0.9;
        y *= scale;
    }
    let s = cfg.movement_scale * cfg.speed_scale;
    let xv = s * x;
    let yv = s * y;
    let zv = s * (cfg.vertical_scale * z);

    let q = ctl.quat;
    let (qx, qy, qz, qw) = (q[0], q[1], q[2], q[3]);
    let r00 = (qx * qx + qw * qw) * K_TWO - K_ONE;
    let r01 = (qx * qy - qz * qw) * K_TWO;
    let r02 = (qx * qz + qy * qw) * K_TWO;
    let r10 = (qy * qx + qz * qw) * K_TWO;
    let r11 = (qw * qw + qy * qy) * K_TWO - K_ONE;
    let r12 = (qz * qy - qx * qw) * K_TWO;
    let r20 = (qz * qx - qy * qw) * K_TWO;
    let r21 = (qw * qx + qz * qy) * K_TWO;
    let r22 = (qz * qz + qw * qw) * K_TWO - K_ONE;
    let d0 = r02 * zv + (r01 * yv + r00 * xv);
    let d1 = r12 * zv + (r11 * yv + r10 * xv);
    let d2 = r22 * zv + (r21 * yv + r20 * xv);

    let mut out = [d0, d1, d2];
    if let Some(hit) = ext.move_ray {
        out[0] += hit[0];
        out[1] += hit[1];
        out[2] += hit[2];
    }

    let friction = if f32::from_bits(d0.to_bits() & 0x7fff_ffff) > 0.0
        || f32::from_bits(d1.to_bits() & 0x7fff_ffff) > 0.0
        || f32::from_bits(d2.to_bits() & 0x7fff_ffff) > 0.0
    {
        cfg.accel_time
    } else {
        cfg.friction_time
    };
    let rate = ctl.dt / friction;
    let decay = |v: f32, rate: f32| {
        let abs = f32::from_bits(v.to_bits() & 0x7fff_ffff);
        let scaled = rate * abs;
        let bits = v.to_bits() as i32;
        let sign = (((bits - 1) >> 31) + 1 + (bits >> 31)) as f32;
        if abs <= scaled {
            0.0
        } else {
            v - sign * scaled
        }
    };
    ctl.spare_velocity = [
        decay(ctl.spare_velocity[0], rate),
        decay(ctl.spare_velocity[1], rate),
        decay(ctl.spare_velocity[2], rate),
    ];
    let add = ctl.dt / cfg.accel_time;
    ctl.spare_velocity[0] = add * out[0] + ctl.spare_velocity[0];
    ctl.spare_velocity[1] = ctl.spare_velocity[1] + out[1] * add;
    ctl.spare_velocity[2] = ctl.spare_velocity[2] + out[2] * add;
    ctl.velocity = ctl.spare_velocity;
    ctl.mode = 2;
}

/// Straight glide helper used by tests and the dispatcher (`FUN_3912b280`).
pub fn glide_only(ctl: &mut QuadController, block: &QuadBlock) -> [f32; 3] {
    glide_straight(ctl, block);
    ctl.velocity
}
