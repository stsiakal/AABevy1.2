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
pub const K_RAD_TO_DEG: f32 = 57.295_776; // DAT_399a44f4 0x42652ee0
pub const K_30: f32 = 30.0; // DAT_3999f7c0
pub const K_TURN_CUT: f32 = 0.006_666_667; // DAT_399acc40 0x3bda740e

/// `actor+0x50` animation block fields read by the movement functions (offsets relative to
/// the block, i.e. `+0x15c` is `actor+0x1ac`).
#[derive(Clone, Copy, Debug, Default)]
pub struct QuadBlock {
    /// `+0x04`: final-pass special-branch gate.
    pub field_04: f32,
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
    /// `+0x5c`: turn-speed scale (`FUN_3912bfb0`).
    pub final_scale: f32,
    /// `+0x60`: turn-speed cut (`FUN_3912bfb0`).
    pub final_cut: f32,
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
    /// `+0x84`: final-pass speed cut output.
    pub val_84: f32,
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
    /// `cfg+0x54 -> vtable+8()` result (non-zero enables the turn-speed scale).
    pub final_scale_flag: bool,
    /// `actor+0xc != 0` (the entity the movement is applied to).
    pub entity_present: bool,
    /// `FUN_390b3750(unit)` game object, vtable+0x8c result.
    pub unit_gate: bool,
    /// `FUN_390b3750(unit)` game object, vtable+4 result.
    pub unit_kind: i32,
    /// `[actor+0x3ec]+0x84` (jump height).
    pub jump_height: f32,
    /// `actor+0x40 & 0x1000` (walk flag used by `FUN_39112690`).
    pub walk: bool,
    /// `actor+0x3ec` movement parameters.
    pub params: MovementParams,
    /// `gEnv+0x96 == 0` (the swim scale is not suppressed).
    pub swim_gate: bool,
    /// `actor vtable+0x9c` returned an animation object.
    pub anim_present: bool,
    /// `actor+8 == DAT_39af92c8` (0): the `jumpVec` graph input is emitted.
    pub graph_push: bool,
    /// `cfg+0xe8` swim speed scale.
    pub cfg_swim_scale: f32,
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
        final_pass(ctl, block, actor, ext, cfg);
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
///
/// `ext.entity_quat` is the entity orientation from `entity vtable+0xa0`; `ext.final_scale_flag`
/// is the `cfg+0x54 -> vtable+8()` result.  Both the swim and the non-swim block compute
/// `conj(normalize(entity_quat)) * ctl+0x48`; the swim-only vector at `ctl+0x4c` is dead in the
/// original (its length only feeds a discarded reciprocal square root).  The swim block has one
/// visible shortcut: with `block+0x04 != 0` and `block+0xa8 != 0` the product is stored
/// **unnormalized** and skips the second normalization.
pub fn final_pass(
    ctl: &mut QuadController,
    block: &QuadBlock,
    actor: &QuadActor,
    ext: &QuadExternals,
    cfg: &QuadConfig,
) {
    if block.block_101 != 0 || actor.ground_align_off != 0 {
        return;
    }
    let eq = ext.entity_quat;
    let n = rsqrt(((eq[3] * eq[3] + eq[0] * eq[0]) + eq[1] * eq[1]) + eq[2] * eq[2]);
    let ex = eq[0] * n;
    let ey = eq[1] * n;
    let ez = eq[2] * n;
    let ew = eq[3] * n;

    let r = ctl.quat;
    let (x, y, z, w) = quat_turn(&r, ex, ey, ez, ew);

    // `block+0x04` is compared with `ucomiss`; unordered falls through to the normal branch.
    let jump_shortcut = matches!(
        block.field_04.partial_cmp(&K_ZERO),
        Some(core::cmp::Ordering::Less | core::cmp::Ordering::Greater)
    ) && block.mounted != 0;
    if ext.is_swim_stance && jump_shortcut {
        ctl.movement_extra_70 = x;
        ctl.movement_extra_74 = y;
        ctl.movement_extra_78 = z;
        ctl.movement_extra_7c = w;
        store_scale(ctl, ext, cfg);
        ctl.val_84 = cfg.final_cut;
        return;
    }

    let n = rsqrt(((x * x + w * w) + y * y) + z * z);
    ctl.movement_extra_70 = x * n;
    ctl.movement_extra_74 = y * n;
    ctl.movement_extra_78 = z * n;
    ctl.movement_extra_7c = w * n;
    store_scale(ctl, ext, cfg);
    ctl.val_84 = cfg.final_cut;
}

/// `conj(entity) * r` in the original's operation order (`x2game` 0x3912c3b1 / 0x3912c2ff).
fn quat_turn(r: &[f32; 4], ex: f32, ey: f32, ez: f32, ew: f32) -> (f32, f32, f32, f32) {
    let (r0, r1, r2, r3) = (r[0], r[1], r[2], r[3]);
    let x = ((ez * r1 - ey * r2) + ew * r0) - ex * r3;
    let y = ((ex * r2 - ez * r0) + ew * r1) - ey * r3;
    let z = ((ey * r0 - ex * r1) + ew * r2) - ez * r3;
    let w = ew * r3 - ((-ey * r1 - ex * r0) - ez * r2);
    (x, y, z, w)
}

/// The cvar turn-speed cut applied to `ctl+0x64..0x6c`.
fn store_scale(ctl: &mut QuadController, ext: &QuadExternals, cfg: &QuadConfig) {
    if !cfg_scale_enabled(ext, cfg) {
        return;
    }
    let dt = ctl.dt;
    let speed = if dt > K_ZERO {
        let (x, y, z, w) = (
            ctl.movement_extra_70,
            ctl.movement_extra_74,
            ctl.movement_extra_78,
            ctl.movement_extra_7c,
        );
        let y_arg = -((y * x - w * z) * K_TWO);
        let x_arg = (w * w + y * y) * K_TWO - K_ONE;
        let deg = (super::libm_sse2_atan2(y_arg as f64, x_arg as f64) as f32) * K_RAD_TO_DEG;
        (f32::from_bits(deg.to_bits() & 0x7fff_ffff)) / dt
    } else {
        K_ZERO
    };
    let t = clamp_unit((speed - K_30) * K_TURN_CUT);
    let s = clamp_unit(cfg.final_scale);
    let factor = (s - K_ONE) * t + K_ONE;
    ctl.velocity[0] *= factor;
    ctl.velocity[1] *= factor;
    ctl.velocity[2] *= factor;
}

/// Gate after the quaternion store: `cfg+0x60 == 0` (ordered) and `cfg+0x54 -> vtable+8()` non-zero.
fn cfg_scale_enabled(ext: &QuadExternals, cfg: &QuadConfig) -> bool {
    ext.final_scale_flag && cfg.final_cut == K_ZERO
}

/// The `comiss` clamp to `[0, 1]`; unordered (NaN) selects 1.
fn clamp_unit(v: f32) -> f32 {
    if K_ZERO > v {
        K_ZERO
    } else if K_ONE <= v || v.is_nan() {
        K_ONE
    } else {
        v
    }
}

/// `f32` value of the original's `x & 0x7fffffff` absolute-value mask.
fn abs_bits(v: f32) -> f32 {
    f32::from_bits(v.to_bits() & 0x7fff_ffff)
}

/// The `up` vector used for the jump impulse (`0x3912ba1c..0x3912ba93`).
///
/// All operations are SSE `f32` in the original instruction order.
fn up_jump(q: [f32; 4]) -> [f32; 3] {
    let (q0, q1, q2, q3) = (q[0], q[1], q[2], q[3]);
    [
        (q0 * q2 + q1 * q3) * K_TWO,
        (q2 * q1 - q0 * q3) * K_TWO,
        (q2 * q2 + q3 * q3) * K_TWO - K_ONE,
    ]
}

/// The `up` vector used by the ground projection (`0x3912b691..0x3912b878`).
///
/// The original forms each product in `f64` (`cvtps2pd`/`mulsd`) and rounds the sums once
/// to `f32` (`cvtpd2ps`), so this helper keeps the `f64` intermediates.
fn up_matrix(q: [f32; 4]) -> [f32; 3] {
    let q0 = q[0] as f64;
    let q1 = q[1] as f64;
    let q2 = q[2] as f64;
    let q3 = q[3] as f64;
    let two = 2.0f64;
    let p0 = q0 * (q0 * two);
    let p1 = q1 * (q1 * two);
    let p02 = q0 * (q2 * two);
    let p12 = q1 * (q2 * two);
    let p03 = q0 * (q3 * two);
    let p13 = q1 * (q3 * two);
    [
        (p02 + p13) as f32,
        (p12 - p03) as f32,
        ((1.0f64 - p0) - p1) as f32,
    ]
}

/// Quaternion rotation of `v` in the exact packed-SSE lane order of `0x3912b560..0x3912b5f0`
/// (the classic `xmm` shuffle sequence with `B = (v0, v1, v2, 0)`).
fn quat_rot(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let a = q;
    let b = [v[0], v[1], v[2], 0.0];
    let x4 = [a[1], a[2], a[0], a[3]];
    let x5 = [a[2], a[0], a[1], a[0]];
    let mut x2 = [
        a[3] * b[0],
        a[3] * b[1],
        a[3] * b[2],
        a[3] * b[3],
    ];
    let mut x1 = [b[1], b[2], b[0], b[3]];
    x1 = [-x1[0] * x5[0], -x1[1] * x5[1], -x1[2] * x5[2], -x1[3] * x5[3]];
    let x7 = [b[2], b[0], b[1], b[0]];
    let x7 = [x7[0] * x4[0], x7[1] * x4[1], x7[2] * x4[2], x7[3] * x4[3]];
    x1 = [x1[0] + x7[0], x1[1] + x7[1], x1[2] + x7[2], x1[3] + x7[3]];
    x1 = [x1[0] + x2[0], x1[1] + x2[1], x1[2] + x2[2], x1[3] + x2[3]];
    let mut l2 = [x1[1], x1[2], x1[0], x1[3]];
    let mut l1 = [x1[2], x1[0], x1[1], x1[3]];
    l2 = [-l2[0] * x5[0], -l2[1] * x5[1], -l2[2] * x5[2], -l2[3] * x5[3]];
    l1 = [l1[0] * x4[0], l1[1] * x4[1], l1[2] * x4[2], l1[3] * x4[3]];
    x2 = [l2[0] + l1[0], l2[1] + l1[1], l2[2] + l1[2], l2[3] + l1[3]];
    x2 = [x2[0] + x2[0], x2[1] + x2[1], x2[2] + x2[2], x2[3] + x2[3]];
    x2 = [x2[0] + b[0], x2[1] + b[1], x2[2] + b[2], x2[3] + b[3]];
    [x2[0], x2[1], x2[2]]
}

/// Raw `rsqrtss` (no Newton step) as the original uses it in the re-orthogonalisation tail.
fn rsqrt_raw(v: f32) -> f32 {
    #[cfg(target_arch = "x86_64")]
    {
        use core::arch::x86_64::{_mm_cvtss_f32, _mm_rsqrt_ss, _mm_set_ss};
        unsafe { _mm_cvtss_f32(_mm_rsqrt_ss(_mm_set_ss(v))) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        1.0 / v.sqrt()
    }
}

/// Outputs of `FUN_3912b330` that are not stored in the controller.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GroundOutput {
    /// Argument passed to the controlled unit's `vtable+0x2a8` (`Some(true)` for the jump
    /// event, `Some(false)` for the in-place cancel); `None` when the unit is not kind 1.
    pub unit_event: Option<bool>,
}

/// `FUN_3912b330` (0x3912b330): ground movement and the jump impulse.
///
/// The engine services are explicit inputs: `ext.entity_present` is `actor+0xc != 0`,
/// `ext.controlled_unit` is `FUN_390b3750(FUN_39111d60(actor)) != 0`, `ext.unit_gate` its
/// `vtable+0x8c`, `ext.unit_kind` its `vtable+4`, `ext.mounted_unit` is `FUN_391136c0`,
/// `ext.speed_multiplier` is `FUN_39113800`, `ext.jump_height` is `[actor+0x3ec]+0x84`,
/// `ext.params`/`ext.walk` feed `FUN_39112690`, `ext.swimming` is `actor vtable+0xa8`,
/// `ext.swim_gate` is `gEnv+0x96 == 0`, `ext.anim_present`/`ext.jump_in_place` are the
/// animation name query and `ext.graph_push` is `actor+8 == 0`.
pub fn ground(
    ctl: &mut QuadController,
    block: &QuadBlock,
    _actor: &QuadActor,
    ext: &QuadExternals,
    graph: &mut Vec<(&'static str, [f32; 3])>,
) -> GroundOutput {
    let mut out = GroundOutput::default();
    if !ext.entity_present {
        return out;
    }
    let q = ctl.quat;
    let mut local = [0.0f32; 3];
    if ctl.input[0] != 0.0 || ctl.input[1] != 0.0 {
        if ext.mounted_unit {
            // 0x3912b3c5..0x3912b4ca: planar input through the (transposed) quaternion axes.
            let (q0, q1, q2, q3) = (q[0], q[1], q[2], q[3]);
            let ix = ctl.input[0];
            let iy = ctl.input[1];
            let a = (q0 * q0 + q3 * q3) * K_TWO - K_ONE;
            let b = (q0 * q1 + q2 * q3) * K_TWO;
            let c = (q2 * q0 - q1 * q3) * K_TWO;
            let d = (q0 * q1 - q2 * q3) * K_TWO;
            let e = (q3 * q3 + q1 * q1) * K_TWO - K_ONE;
            let f = (q3 * q0 + q2 * q1) * K_TWO;
            local = [
                d * iy + a * ix,
                e * iy + b * ix,
                f * iy + c * ix,
            ];
        } else {
            // 0x3912b4d4..0x3912b622: normalise, direction factor, quaternion rotation.
            let input = [ctl.input[0], ctl.input[1], ctl.input[2]];
            let mut norm = input;
            let len = (input[0] * input[0] + input[1] * input[1] + input[2] * input[2]).sqrt();
            if K_ONE < len {
                let inv = K_ONE / len;
                norm = [input[0] * inv, input[1] * inv, input[2] * inv];
            }
            let factor = direction_factor(&ext.params, ext.walk, [norm[0], norm[1]]);
            let r = quat_rot(q, norm);
            local = [r[0] * factor, r[1] * factor, r[2] * factor];
        }
    }
    if abs_bits(local[0]) > K_ZERO
        || abs_bits(local[1]) > K_ZERO
        || abs_bits(local[2]) > K_ZERO
        || (ext.controlled_unit && ext.unit_gate)
    {
        let m = ext.speed_multiplier;
        local = [local[0] * m, local[1] * m, local[2] * m];
    }

    // 0x3912b87d..0x3912bc95: jump startup / in-place / airborne handling.
    ctl.mode = 1;
    let mut jump = [0.0f32; 3];
    if ctl.jump_button != 0 && ctl.airborne == 0 {
        if K_POS_X < abs_bits(ctl.input[0]) || K_POS_X < abs_bits(ctl.input[1]) {
            if block.jump_scalar > K_ZERO {
                ctl.mode = 7;
                let d = block.jump_dir;
                let mag = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                let mut scale = K_ZERO;
                if K_ZERO < mag {
                    let above = if block.field_04 != K_ZERO { K_HALF } else { K_ZERO };
                    scale = (mag * K_TWO * ext.jump_height).sqrt() / mag - above;
                }
                let up = up_jump(q);
                jump = [up[0] * mag * scale, up[1] * mag * scale, up[2] * mag * scale];
                let e = block.jump_extra;
                if e[0] * e[0] + e[1] * e[1] + e[2] * e[2] > K_ZERO {
                    let raw = (e[2] - 0.25) * K_TWO;
                    let s = if K_ZERO <= raw {
                        if raw < K_ONE {
                            raw
                        } else {
                            K_ONE
                        }
                    } else {
                        K_ZERO
                    };
                    let nx = e[0] + (-e[0]) * s;
                    let ny = e[1] + (-e[1]) * s;
                    let nz = e[2] + (K_ONE - e[2]) * s;
                    let jlen = (jump[0] * jump[0] + jump[1] * jump[1] + jump[2] * jump[2]).sqrt();
                    jump = [nx * jlen, ny * jlen, nz * jlen];
                }
                let up = up_matrix(q);
                let dot = up[0] * local[0] + up[1] * local[1] + up[2] * local[2];
                local = [local[0], local[1], local[2] - dot];
                ctl.airborne = 1;
                if ext.controlled_unit && ext.unit_kind == 1 {
                    out.unit_event = Some(true);
                }
                ctl.jumped = 1;
            }
        } else if !(ext.anim_present && ext.jump_in_place) {
            ctl.airborne = 1;
            ctl.jumped = 1;
            if ext.anim_present && ext.controlled_unit && ext.unit_kind == 1 {
                out.unit_event = Some(false);
            }
        }
    }

    // 0x3912bc95..0x3912bceb: swim speed scale.
    if ext.swimming && ext.swim_gate {
        let s = ext.cfg_swim_scale;
        local = [local[0] * s, local[1] * s, local[2] * s];
    }

    // 0x3912bceb..0x3912bd6d: pick the velocity that the tail operates on.
    let mut v = [0.0f32; 3];
    if block.jump_scalar == K_ZERO {
        let len2 = local[0] * local[0] + local[1] * local[1] + local[2] * local[2];
        if K_POINT_01 < len2 {
            v = local;
        }
    } else {
        v = local;
    }

    // 0x3912bd6d..0x3912bdff: remove the up component (always).
    let up = up_matrix(q);
    let dot = up[0] * v[0] + up[1] * v[1] + up[2] * v[2];
    v = [v[0], v[1], v[2] - dot];

    // 0x3912be02..0x3912be3a: graph jumpVec input.
    if ext.graph_push {
        graph.push(("jumpVec", jump));
    }

    // 0x3912be3a..0x3912bf59: remove the up component again and re-normalise to the old
    // length.  The branch skips this when `block+0x04 == 0` (ordered).
    if !(block.field_04 == K_ZERO) {
        let len0 = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        let dot = up[0] * v[0] + up[1] * v[1] + up[2] * v[2];
        v = [v[0], v[1], v[2] - dot];
        let len2 = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
        if !(K_SQRT_EPS > len2) {
            let rs = rsqrt_raw(len2);
            let t = ((rs * len2) * rs) * K_HALF;
            let scale = ((K_ONE_POINT_FIVE - t) * rs) * len0;
            v = [v[0] * scale, v[1] * scale, v[2] * scale];
        }
    }

    // 0x3912bf5d..0x3912bf9e.
    ctl.velocity = [v[0] + jump[0], v[1] + jump[1], v[2] + jump[2]];
    ctl.spare_velocity = [0.0; 3];
    out
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
