//! Phase B input state from the 1.2 `x2game.dll` decompile.
//!
//! `InputState` is the per-controllable-unit object at `ECX` of `FUN_39404d50`
//! (`UpdateMovement`); `structs.md` §1 documents the offsets.  It is a plain POD block
//! (no pointers are dereferenced by the port), so the golden JSON can round-trip it
//! through `to_bytes`/`from_bytes` byte for byte.
//!
//! All functions here name the original function they came from.


pub const INPUT_STATE_SIZE: usize = 0xC8;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct InputState {
    /// `+0x00` unit object id (`FUN_39360020(*this)`).
    pub unit_id: u32,
    /// `+0x04` accumulated dt until the next movement packet.
    pub send_timer: f32,
    /// `+0x08` cleared with `+0x09` as a u16 at the top of `UpdateMovement`.
    pub flag_08: u8,
    /// `+0x09` -> CSMoveUnit flag 0x20.
    pub flag_09: u8,
    /// `+0x0a` -> CSMoveUnit flag 0x80.
    pub flag_0a: u8,
    pub unknown_0b: u8,
    /// `+0x0c` requested (pre-smoothing) input velocity.
    pub requested_vel: [f32; 3],
    /// `+0x18` external move-to velocity.
    pub external_vel: [f32; 3],
    /// `+0x24` smoothed input velocity (`FUN_39402c50`).
    pub smoothed_vel: [f32; 3],
    /// `+0x30` byte tested by the combiner (`FUN_39402590` `state_30`).
    pub unknown_30: u8,
    /// `+0x31` autorun arm latch (0x394053xx).
    pub autorun_armed: u8,
    /// `+0x32` start-gate byte (`FUN_394039c0`).
    pub unknown_32: u8,
    pub pad_33: u8,
    /// `+0x34` smoothing factor, default 30.0.
    pub accel: f32,
    /// `+0x38` mounted/gliding keyboard-turn ramp.
    pub turn_accum: f32,
    /// `+0x3c` free-look / autorun latch.
    pub autorun_or_lock: u8,
    pub pad_3d: [u8; 3],
    /// `+0x40` mouse-axis accumulator while a movement starts.
    pub timer_40: f32,
    /// `+0x44` last sent rotation quaternion.
    pub last_quat: [f32; 4],
    /// `+0x54` last sent position.
    pub last_pos: [f32; 3],
    /// `+0x60` last sent rotation.
    pub last_rot: [f32; 3],
    /// `+0x6c` last sent velocity.
    pub last_vel: [f32; 3],
    /// `+0x78` last stance.
    pub last_stance: u8,
    /// `+0x79` last actor flags.
    pub last_flags: u8,
    pub pad_7a: u16,
    /// `+0x7c` last fall speed.
    pub last_fall_vel: f32,
    /// `+0x80` last support/gc id.
    pub last_support_id: u32,
    /// `+0x84` input flag word (bit 0 moving, 0x800 air, 0x1000 walk, ...).
    pub input_flags: u32,
    /// `+0x88` previous axes; the original shifts four bytes on each update.
    pub prev_axes: [u8; 4],
    /// `+0x8c` left.
    pub axis_left: u8,
    /// `+0x8d` right.
    pub axis_right: u8,
    /// `+0x8e` previous `axis_8a`.
    pub axis_prev_8a: u8,
    /// `+0x8f` previous `axis_8b`.
    pub axis_prev_8b: u8,
    /// `+0x90` previous `axis_8c`.
    pub axis_prev_8c: u8,
    /// `+0x91` previous `axis_8d`.
    pub axis_prev_8d: u8,
    pub pad_92: u16,
    /// `+0x94` turn-left, `+0x98` turn-right (floats).
    pub axis_vals: [f32; 2],
    /// `+0x9c..+0xa8` per-direction press start times.
    pub press_forward: f32,
    pub press_back: f32,
    pub press_left: f32,
    pub press_right: f32,
    /// `+0xac` turn-left press start.
    pub turn_l_start: f32,
    /// `+0xb0` turn-right press start.
    pub turn_r_start: f32,
    /// `+0xb4` external goto unit id (0 = none).
    pub goto_unit_id: u32,
    /// `+0xb8` request-submit flag.
    pub flags_b8: u8,
    /// `+0xb9` turn-delta flag.
    pub turn_flag: u8,
    /// `+0xba` one-shot flag.
    pub unknown_ba: u8,
    /// `+0xbb` derived moving flag.
    pub moving: u8,
    /// `+0xbc` mouse hold: forward.
    pub hold_fwd: u8,
    /// `+0xbd` mouse hold: back.
    pub hold_back: u8,
    /// `+0xbe` mouse hold: left.
    pub hold_left: u8,
    /// `+0xbf` mouse hold: right.
    pub hold_right: u8,
    /// `+0xc0` glide pitch accumulator.
    pub hold_timer: f32,
    /// `+0xc4` pointer to the two-id latch pair (kept as an address in the port).
    pub latch: u32,
}

const _: () = assert!(core::mem::size_of::<InputState>() == INPUT_STATE_SIZE);

impl Default for InputState {
    fn default() -> Self {
        Self {
            unit_id: 0,
            send_timer: 0.0,
            flag_08: 0,
            flag_09: 0,
            flag_0a: 0,
            unknown_0b: 0,
            requested_vel: [0.0; 3],
            external_vel: [0.0; 3],
            smoothed_vel: [0.0; 3],
            unknown_30: 0,
            autorun_armed: 0,
            unknown_32: 0,
            pad_33: 0,
            accel: 0.0,
            turn_accum: 0.0,
            autorun_or_lock: 0,
            pad_3d: [0; 3],
            timer_40: 0.0,
            last_quat: [0.0; 4],
            last_pos: [0.0; 3],
            last_rot: [0.0; 3],
            last_vel: [0.0; 3],
            last_stance: 0,
            last_flags: 0,
            pad_7a: 0,
            last_fall_vel: 0.0,
            last_support_id: 0,
            input_flags: 0,
            prev_axes: [0; 4],
            axis_left: 0,
            axis_right: 0,
            axis_prev_8a: 0,
            axis_prev_8b: 0,
            axis_prev_8c: 0,
            axis_prev_8d: 0,
            pad_92: 0,
            axis_vals: [0.0; 2],
            press_forward: 0.0,
            press_back: 0.0,
            press_left: 0.0,
            press_right: 0.0,
            turn_l_start: 0.0,
            turn_r_start: 0.0,
            goto_unit_id: 0,
            flags_b8: 0,
            turn_flag: 0,
            unknown_ba: 0,
            moving: 0,
            hold_fwd: 0,
            hold_back: 0,
            hold_left: 0,
            hold_right: 0,
            hold_timer: 0.0,
            latch: 0,
        }
    }
}

impl InputState {
    pub fn from_bytes(bytes: &[u8; INPUT_STATE_SIZE]) -> Self {
        unsafe { core::ptr::read_unaligned(bytes.as_ptr() as *const Self) }
    }

    pub fn to_bytes(self) -> [u8; INPUT_STATE_SIZE] {
        unsafe { core::ptr::read_unaligned(&self as *const Self as *const [u8; INPUT_STATE_SIZE]) }
    }
}

/// `FUN_39402b30` (x2game.dll, 0x39402b30, 283 bytes): clear the per-unit input state.
///
/// The original leaves `+0x00`, `+0x31..+0x33`, `+0x3d..+0x3f`, `+0x81..+0x87`,
/// `+0xb1..+0xb7`, `+0xb9..+0xbb`, `+0xbd..+0xbf`, `+0xc1..+0xc3` and `+0xc4` untouched.
pub fn reset(state: &mut InputState) {
    state.send_timer = 0.0;
    state.flag_08 = 0;
    state.flag_09 = 0;
    state.flag_0a = 0;
    state.requested_vel = [0.0; 3];
    state.external_vel = [0.0; 3];
    state.smoothed_vel = [0.0; 3];
    state.unknown_30 = 0;
    state.accel = 30.0;
    state.turn_accum = 0.0;
    state.timer_40 = 0.0;
    state.last_quat = [0.0, 0.0, 0.0, 1.0];
    state.last_pos = [0.0; 3];
    state.last_rot = [0.0; 3];
    state.last_vel = [0.0; 3];
    state.last_stance = 0xff;
    state.last_flags = 0;
    state.last_fall_vel = 0.0;
    state.last_support_id &= 0xffffff00;
    state.prev_axes = [0; 4];
    state.axis_left = 0;
    state.axis_right = 0;
    state.axis_prev_8a = 0;
    state.axis_prev_8b = 0;
    state.axis_prev_8c = 0;
    state.axis_prev_8d = 0;
    state.axis_vals = [0.0; 2];
    state.press_forward = 0.0;
    state.press_back = 0.0;
    state.press_left = 0.0;
    state.press_right = 0.0;
    state.turn_l_start = 0.0;
    state.turn_r_start = 0.0;
    state.flags_b8 = 0;
    state.turn_flag = 0;
    state.unknown_ba = 0;
    state.moving = 0;
    state.hold_timer = 0.0;
    state.hold_fwd = 0;
    state.hold_back = 0;
    state.hold_left = 0;
    state.hold_right = 0;
    state.autorun_or_lock = 0;
    state.axis_prev_8a = 0;
    state.axis_prev_8b = 0;
    state.axis_prev_8c = 0;
    state.axis_prev_8d = 0;
}

/// `FUN_39402c50` (x2game.dll, 0x39402c50, 427 bytes): accelerate the desired velocity
/// into `state.smoothed_vel` (`+0x24..+0x2c`).
///
/// `dt` is the original's `min(game_dt, 0.033)` result.
pub fn smooth_velocity(state: &mut InputState, desired: [f32; 3], dt: f32) {
    let dt = if dt > super::DT_CAP { super::DT_CAP } else { dt };
    let len2 = desired[0] * desired[0] + desired[1] * desired[1] + desired[2] * desired[2];
    if len2 < 0.01 {
        state.smoothed_vel = [0.0; 3];
        return;
    }
    if state.accel <= 0.0 {
        state.smoothed_vel = desired;
        return;
    }
    let mut delta = [
        desired[0] - state.smoothed_vel[0],
        desired[1] - state.smoothed_vel[1],
        desired[2] - state.smoothed_vel[2],
    ];
    let len = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
    if len <= 1.0 {
        let scale = 1.0 - len * 0.55;
        delta[0] *= scale;
        delta[1] *= scale;
        delta[2] *= scale;
    }
    let step = state.accel * dt;
    let step = if step > 1.0 { 1.0 } else { step };
    state.smoothed_vel[0] += delta[0] * step;
    state.smoothed_vel[1] += delta[1] * step;
    state.smoothed_vel[2] += delta[2] * step;
}

/// `FUN_394037d0` (x2game.dll, 0x394037d0, 119 bytes): movement type for the request
/// (`+0x2c`): 1 normal, 0 blocked, 5 scripted, 6 special, or the value of the game's
/// `+0x8c` vtable object (`+0x2c`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MovementTypeInput {
    pub actor_present: bool,
    /// `game->vtable[0x8c]()` result object's `vtable[0x2c]()`, or -1 if absent.
    pub special_type: i32,
    /// `actor+0x3a78`.
    pub scripted: bool,
    /// `FUN_39354600(actor)`.
    pub blocked: bool,
    /// `actor+0x3b00`.
    pub actor_3b00: u32,
    /// `DAT_3a59f864`.
    pub global_3a59f864: u32,
}

pub fn movement_type(input: MovementTypeInput) -> i32 {
    if !input.actor_present {
        return 1;
    }
    if input.scripted {
        return 5;
    }
    if input.special_type != -1 {
        return input.special_type;
    }
    if input.blocked {
        return 0;
    }
    if input.actor_3b00 != input.global_3a59f864 {
        return 6;
    }
    1
}

/// `FUN_394039c0` (x2game.dll, 0x394039c0, 200 bytes): the new-movement / free-look gate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StartGateInput {
    /// `InputState+0x32`.
    pub state_32: u8,
    pub actor_present: bool,
    /// `FUN_390b37a0(actor)`: all of `actor+0x3a70/71/72` are zero.
    pub actor_clear: bool,
    pub game_present: bool,
    /// `game->vtable[0x200]()`.
    pub game_200: bool,
    /// `game->vtable[4]() == 1`.
    pub game_foot: bool,
    /// `game->vtable[0x194]()`.
    pub game_194: bool,
    /// `actor+0x3b94`.
    pub actor_3b94: u32,
    pub global_3a59f864: u32,
    /// `FUN_39402810(view unit)`.
    pub hold_position: bool,
    /// The view unit exists and its `+0x1c1` is 4 (gliding).
    pub view_gliding: bool,
    /// `game->vtable[0x8c]()` is null or its `vtable[0x18]()` is non-zero.
    pub special_ok: bool,
}

pub fn start_gate(input: StartGateInput) -> bool {
    if input.state_32 != 0 || !input.actor_present || !input.actor_clear || !input.game_present {
        return false;
    }
    if input.game_200 || !input.game_foot || input.game_194 {
        return false;
    }
    if input.actor_3b94 != input.global_3a59f864 {
        return false;
    }
    if input.hold_position || input.view_gliding {
        return false;
    }
    input.special_ok
}

/// `FUN_39403330` (x2game.dll, 0x39403330, 107 bytes): local-unit / may-turn test.
///
/// Returns the low 24 bits of the global unit id when `state.unit_id` is that unit,
/// else 1 when the game reports on-foot (`vtable[4]() == 1`), else 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LocalUnitInput {
    pub state_unit_id: u32,
    /// `DAT_39b02294+0x10c` -> `+8`, or `DAT_3a59f858` when the manager is absent.
    pub global_unit_id: u32,
    pub actor_present: bool,
    pub game_present: bool,
    pub game_foot: bool,
}

pub fn is_local_unit(input: LocalUnitInput) -> u32 {
    if input.global_unit_id == input.state_unit_id {
        return input.global_unit_id & 0xffffff00;
    }
    if input.actor_present && input.game_present && input.game_foot {
        return 1;
    }
    0
}

/// `FUN_39403a90` (x2game.dll, 0x39403a90, 87 bytes): the local unit may turn.
pub fn can_turn(input: LocalUnitInput, unit_kind: Option<u8>) -> bool {
    if input.global_unit_id != input.state_unit_id && (is_local_unit(input) & 0xff) == 0 {
        return false;
    }
    if unit_kind == Some(4) {
        return false;
    }
    true
}

/// `FUN_39402fe0` (x2game.dll, 0x39402fe0, 198 bytes): external move-to direction.
///
/// The original starts from the actor's view position, adds the path node's `+0x14`
/// offset when a path object exists, and zeroes the direction when the horizontal
/// distance to the comparison actor's view position is below `DAT_399a232c`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ExternalTargetInput {
    pub start_view_pos: [f32; 3],
    /// `path_node[+0x14]` added to `z` (0 when there is no path object).
    pub path_z: f32,
    pub end_view_pos: [f32; 3],
    /// `DAT_399a232c`.
    pub threshold: f32,
}

pub fn external_target(input: ExternalTargetInput) -> [f32; 3] {
    let mut out = input.start_view_pos;
    out[2] += input.path_z;
    let dx = out[0] - input.end_view_pos[0];
    let dy = out[1] - input.end_view_pos[1];
    if dx * dx + dy * dy < input.threshold {
        out = [0.0; 3];
    }
    out
}

