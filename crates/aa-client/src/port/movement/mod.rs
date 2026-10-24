//! Movement ported from the 1.2 `x2game.dll` decompile: Phase A leaves and the
//! Phase B input state (`InputState`, `UpdateMovement`).
//!
//! Every item names the original function it came from (`FUN_xxxxxxxx` in `x2game.dll`).
//! Frame time and cvars are explicit arguments here; the original reads them from
//! `DAT_3a5bb1d8+0x14` (frame timer vtable `+0x2c`/`+0x30`) and `DAT_39aefb98+0x308`
//! (`keyboard_rotate_speed`).

#![allow(dead_code, unused_imports)]

mod asinf;
mod asinf_tables;
mod clamp;
mod direction;
mod input;
mod input_state;
mod mouse;
mod mover;
mod quadruped;
mod rotation;
mod request;
mod turn;
mod update;

#[cfg(test)]
mod tests;

pub use asinf::libm_sse2_asinf;
pub use clamp::clamp;
pub use direction::{MovementParams, direction_factor};
pub use input::{
    CombineInput, CombineOutput, LadderOutput, PressTimes, combine, ladder_filter, prefer_axes,
};
pub use input_state::{
    ExternalTargetInput, INPUT_STATE_SIZE, InputState, LocalUnitInput, MovementTypeInput,
    StartGateInput, can_turn,
    external_target, is_local_unit, movement_type, reset, smooth_velocity, start_gate,
};
pub use mover::*;
pub use quadruped::*;
pub use request::{DEFAULT_REQUEST_PTR, MovementRequest, REQUEST_SIZE};
pub use rotation::*;
pub use update::{ControlledUnit, GateOutputs, MovementContext, pushed_flags, update_movement};
pub use mouse::{mouse_pitch, mouse_yaw};
pub use turn::{TurnInput, TurnResult, TurnState, TurnUnit, turn_delta};

pub const DT_CAP: f32 = 0.033;
pub const PI: f32 = core::f32::consts::PI;
pub const TWO_PI: f32 = core::f32::consts::TAU;
pub const HALF: f32 = 0.5;
pub const MOUSE_SCALE: f32 = 10.0;
pub const MOUSE_AXIS_SCALE: f32 = -0.002;
pub const MOUNT_TURN_RAMP: f32 = 3.0;
pub const ONE_THIRD: f32 = 0.333_333_34;
pub const FLT_EPSILON: f32 = 1.192_092_9e-7;
