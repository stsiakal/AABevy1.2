//! `FUN_3915c700` / `FUN_3915c7d0` (x2game.dll, 0x3915c700 / 0x3915c7d0, 193 bytes each).
//!
//! Both read the frame time from `(*(DAT_3a5bb1d8+0x14)->vtable[0x30])(0)`, cap it at 0.033,
//! scale it by `10.0 * 2π`, read axis 8 (`rotateyaw`) or axis 9 (`rotatepitch`) through
//! `FUN_39424580`, scale the axis by `-0.002`, and fold the two together.
//!
//! The fold is not a plain clamp in the source: it is
//! `t = ((a+c) - |a-c|) * 0.5` followed by `((t-c) + |t+c|) * 0.5` with `f32` operations,
//! which is `clamp(a, -c, c)` for finite inputs. The exact operation sequence is kept so the
//! results match `st0_f32` bit for bit. The functions return the value in x87 `ST0`; the
//! caller stores it with `fst dword`, so the stored value is the `f32` returned here.

use super::{DT_CAP, HALF, MOUSE_AXIS_SCALE, MOUSE_SCALE, TWO_PI};

fn mouse_delta(dt: f32, axis: f32) -> f32 {
    let dt = if dt > DT_CAP { DT_CAP } else { dt };
    let c = (dt * MOUSE_SCALE) * TWO_PI;
    let a = axis * MOUSE_AXIS_SCALE;
    let t = ((a + c) - (a - c).abs()) * HALF;
    // The second fold is x87 80-bit in the original: t and c are f32, but the two
    // additions and the final multiply keep the extended mantissa and are rounded to f32
    // only on return. f64 has enough precision to reproduce the stored f32 here.
    let t = t as f64;
    let c = c as f64;
    (((t - c) + (t + c).abs()) * 0.5) as f32
}

/// `FUN_3915c700`: yaw delta from mouse axis 8 (`mousedx`).
pub fn mouse_yaw(dt: f32, axis8: f32) -> f32 {
    mouse_delta(dt, axis8)
}

/// `FUN_3915c7d0`: pitch delta from mouse axis 9 (`mousedy`).
pub fn mouse_pitch(dt: f32, axis9: f32) -> f32 {
    mouse_delta(dt, axis9)
}
