//! `FUN_39402e00` (x2game.dll, 0x39402e00, 349 bytes): turn delta.
//!
//! `__thiscall` on the per-unit InputState; stack arguments `(unit, axis5, axis4, use_mouse)`.
//! On foot:
//!
//! ```text
//! axis = -axis5 + axis4
//! if axis == 0 { turn_accum = 0; return 0 }
//! rate = PI
//! if unit != NULL && unit->kind(+0x1c1) == 4 {   // 4 = GLIDING, not mounted
//!     turn_accum = f32(turn_accum + dt)
//!     turn_accum = clamp(turn_accum, 0, 3)
//!     rate = turn_accum * unit->turn_speed(+0x174) * (1/3)   // x87 80-bit
//! }
//! rate *= keyboard_rotate_speed                                // f32
//! dt = min(dt, 0.033)                                          // f32
//! return axis * dt * rate                                      // f32
//! ```
//!
//! **`unit+0x1c1 == 4` is the gliding stance** (`FUN_3911d6c0(1)` sets it while the glider
//! is deployed; `FUN_3912dd40` dispatches stance 4 to the glide integrator).  A mounted
//! unit is selected by `+0xa8`/`+0xf8`, not by this byte.  Live values captured in the
//! hidden 1.2 client: on foot `+0x1c1 = 0`, `+0x174 = 1.0`; riding a horse `+0x1c1 = 0`,
//! `+0x174 = 1.0`; gliding `+0x1c1 = 4`, `+0x174 = 0.5235988` rad/s (30 deg/s).  A horse
//! therefore uses the **on-foot branch: 90 deg/s** (`0.5 * pi`); only a glider takes the
//! ramped `turn_accum/3 * +0x174` branch.
//!
//! The mouse branch returns `FUN_3915c700()` (see [`super::mouse_yaw`]) instead of the keyboard
//! turn and triggers `FUN_394fab30()` when that delta is non-zero. The frame time and the cvar
//! are explicit inputs here; the original reads them from `DAT_3a5bb1d8+0x14` and
//! `DAT_39aefb98+0x308`.
//!
//! The glider `rate` is the only x87 80-bit intermediate. It is reproduced with `f64`
//! multiplication rounded once to `f32`, which can differ from the original's 64-bit x87
//! mantissa by at most one `f32` ulp in contrived cases; golden cases are bit-exact.

use super::{DT_CAP, ONE_THIRD, PI, clamp};

/// The `InputState+0x38` glider-turn ramp accumulator.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TurnState {
    pub turn_accum: f32,
}

/// The controlled unit fields `FUN_39402e00` reads (`NULL` = not mounted).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TurnUnit {
    /// `unit+0x1c1`; 4 is the gliding stance (not a mount).
    pub kind: u8,
    /// `unit+0x174`: the glider rotate speed in rad/s while gliding; 1.0 on foot and
    /// on a horse (where the branch is not taken).
    pub turn_speed: f32,
}

/// Stack arguments of `FUN_39402e00`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TurnInput {
    /// `[ebp+0xc]`: action axis 5 (`turnright`).
    pub axis5: f32,
    /// `[ebp+0x10]`: action axis 4 (`turnleft`).
    pub axis4: f32,
    /// The value the original obtains from `FUN_3915c700()` in the mouse branch.
    pub mouse_yaw: f32,
}

/// Result of [`turn_delta`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TurnResult {
    pub delta: f32,
    /// The original calls `FUN_394fab30()` when this is true.
    pub notify_mouse: bool,
}

/// `FUN_39402e00` as a pure function; `state.turn_accum` is the `InputState+0x38` field.
pub fn turn_delta(
    state: &mut TurnState,
    unit: Option<TurnUnit>,
    input: TurnInput,
    use_mouse: bool,
    dt: f32,
    keyboard_rotate_speed: f32,
) -> TurnResult {
    if use_mouse {
        let delta = input.mouse_yaw;
        return TurnResult {
            delta,
            notify_mouse: delta != 0.0,
        };
    }

    let axis = (-input.axis5) + input.axis4;
    if axis == 0.0 {
        state.turn_accum = 0.0;
        return TurnResult {
            delta: 0.0,
            notify_mouse: false,
        };
    }

    let mut rate = PI;
    if let Some(unit) = unit.filter(|unit| unit.kind == 4) {
        let accum = state.turn_accum + dt;
        let accum = clamp(accum, 0.0, 3.0);
        state.turn_accum = accum;
        rate = ((accum as f64) * (unit.turn_speed as f64) * (ONE_THIRD as f64)) as f32;
    }

    let rate = keyboard_rotate_speed * rate;
    let dt = if dt > DT_CAP { DT_CAP } else { dt };
    let delta = (axis * dt) * rate;
    TurnResult {
        delta,
        notify_mouse: false,
    }
}
