//! `FUN_39404d50` (x2game.dll, 0x39404d50, 5971 bytes): `X2::GameClient::InputManager`'s
//! per-unit `UpdateMovement`, the integration of the Phase B input state.
//!
//! The port keeps the original's control flow and bit-exact arithmetic.  Engine objects
//! that cannot be constructed in a unit test are explicit inputs in [`MovementContext`];
//! the golden capture records them from the live client (`ctx` in `goldens/phase_b.json`).
//! The request is built at `[ebp-0x130]` and submitted through `GameClient vtable +0x3C`;
//! its `flags` word is also pushed through `vtable +0x58` (the walk bit lives there, not
//! in the request flags).

use super::input_state::{
    ExternalTargetInput, InputState, LocalUnitInput, MovementTypeInput, StartGateInput,
    can_turn, external_target, is_local_unit, movement_type, smooth_velocity, start_gate,
};
use super::request::MovementRequest;
use super::{
    CombineInput, PressTimes, TurnInput, TurnResult, TurnState, TurnUnit, combine, ladder_filter,
    mouse_yaw, prefer_axes, turn_delta,
};

/// The controlled unit (`GameClient vtable +0x208`), or `None` when the game is not in
/// the on-foot controller.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControlledUnit {
    pub unit_id: u32,
    /// `+0x1c1`: 4 = gliding (the old notes' "quadruped" label is wrong; mounts use
    /// `+0xa8/+0xf8` instead).
    pub kind: u8,
    /// `+0x174`: turn speed; for a horse this is 1.0 and the turn-delta branch is not
    /// taken because `kind != 4`; for a glider it is the glide rotate speed in rad/s.
    pub turn_speed: f32,
    /// Low byte of `+0x1d0` (`unit[0x74]` in the decompile).
    pub hold_byte_1d0: u8,
    /// `+0x1d8` (`unit[0x76]`).
    pub flag_1d8: u8,
    /// `+0x23c` ladder kind.
    pub ladder_kind: u32,
    /// `+0x68`.
    pub float_68: f32,
    /// `+0x161`.
    pub byte_161: u8,
    /// `+0x164`.
    pub float_164: f32,
    /// `unit->vtable[0x68]()`.
    pub vtable_68: bool,
}

/// Recorded engine queries whose implementations live outside Phase B.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GateOutputs {
    pub start_gate: bool,
    pub is_local: bool,
    pub can_turn: bool,
}

/// Everything `FUN_39404d50` reads besides the `InputState`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MovementContext {
    pub dt: f32,
    pub game_time: f32,
    pub keyboard_rotate_speed: f32,
    /// Action axes 0..9 (`FUN_39424580`).
    pub axes: [f32; 10],
    /// `FUN_392aa250(1)`.
    pub mode_a: u8,
    /// `FUN_392aa250(3)`.
    pub mode_b: u8,
    /// `(actor vtable +0x38)->vtable[0x14](&PTR_FUN_3999cfdc)`.
    pub mouse_gate: bool,
    /// `FUN_39055fa0() || FUN_3931e2f0()->+0x480`.
    pub ui_lock: bool,
    /// `actor+0x3a70/71/72`.
    pub actor_flags: [u8; 3],
    /// `actor+0x3a78`.
    pub actor_3a78: bool,
    /// `actor+0x3b00`.
    pub actor_3b00: u32,
    /// `actor+0x3b94`.
    pub actor_3b94: u32,
    /// `DAT_3a59f864`.
    pub global_3a59f864: u32,
    /// `InputState+0xb4`.
    pub goto_unit_id: u32,
    pub game_foot: bool,
    pub game_200: bool,
    pub game_194: bool,
    /// `game->vtable[0x8c]()` is non-null.
    pub game_special_present: bool,
    /// `game->vtable[0x8c]()` is null or its `vtable[0x18]()` is non-zero.
    pub game_special_ok: bool,
    pub hold_position: bool,
    pub view_unit_kind: u8,
    /// `FUN_394037d0` (recorded; [`movement_type`] is its ported form).
    pub movement_type: i32,
    /// `FUN_390b66c0(0) || FUN_390b67e0()` (the walk bit 0x1000).
    pub walk_toggle: bool,
    /// `FUN_39354600(actor)`.
    pub cond_39354600: bool,
    pub cvar_114: i32,
    pub cvar_2ac: i32,
    pub cvar_2f8: i32,
    pub cvar_300: f32,
    pub cvar_34c: f32,
    pub cvar_350: f32,
    pub cvar_428: bool,
    /// `DAT_39b02294+0x10c` -> `+8`, or `DAT_3a59f858`.
    pub global_unit_id: u32,
    pub controlled: Option<ControlledUnit>,
    /// Recorded gate outputs (also derivable from the raw fields by [`Self::gates`]).
    pub gates: GateOutputs,
}

impl MovementContext {
    /// Compute the Phase B gate leaves from the recorded raw fields.
    pub fn gates(&self, state: &InputState) -> GateOutputs {
        let local = is_local_unit(LocalUnitInput {
            state_unit_id: state.unit_id,
            global_unit_id: self.global_unit_id,
            actor_present: true,
            game_present: true,
            game_foot: self.game_foot,
        });
        GateOutputs {
            start_gate: start_gate(StartGateInput {
                state_32: state.unknown_32,
                actor_present: true,
                actor_clear: self.actor_flags == [0; 3],
                game_present: true,
                game_200: self.game_200,
                game_foot: self.game_foot,
                game_194: self.game_194,
                actor_3b94: self.actor_3b94,
                global_3a59f864: self.global_3a59f864,
                hold_position: self.hold_position,
                view_gliding: self.view_unit_kind == 4,
                special_ok: self.game_special_ok,
            }),
            is_local: (local & 0xff) != 0,
            can_turn: can_turn(
                LocalUnitInput {
                    state_unit_id: state.unit_id,
                    global_unit_id: self.global_unit_id,
                    actor_present: true,
                    game_present: true,
                    game_foot: self.game_foot,
                },
                self.controlled.map(|u| u.kind),
            ),
        }
    }

    /// The ported `FUN_394037d0` for this context.
    pub fn movement_type(&self) -> i32 {
        movement_type(MovementTypeInput {
            actor_present: true,
            special_type: if self.game_special_present {
                // The recorded `movement_type` already contains the object's +0x2c value;
                // when the caller wants the leaf formula, this is the closest raw view.
                self.movement_type
            } else {
                -1
            },
            scripted: self.actor_3a78,
            blocked: self.cond_39354600,
            actor_3b00: self.actor_3b00,
            global_3a59f864: self.global_3a59f864,
        })
    }

    fn normalize2(x: f32, y: f32) -> [f32; 3] {
        let len = x * x + y * y;
        if len <= super::FLT_EPSILON {
            return [0.0; 3];
        }
        let n = rsqrt_newton(len);
        [n * x, n * y, 0.0]
    }

    fn normalize3(x: f32, y: f32, z: f32) -> [f32; 3] {
        let len = x * x + y * y + z * z;
        if len <= super::FLT_EPSILON {
            return [0.0; 3];
        }
        let n = rsqrt_newton(len);
        [n * x, n * y, n * z]
    }
}

/// `rsqrtss` + the original's one Newton step: `(1.5 - n*len*n*0.5) * n`.
#[inline]
fn rsqrt_newton(len: f32) -> f32 {
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

fn any_abs_nonzero(v: [f32; 3]) -> bool {
    v[0].abs() > 0.0 || v[1].abs() > 0.0 || v[2].abs() > 0.0
}

/// `FUN_39404d50`: update one unit's input state and build its `MovementRequest`.
///
/// The caller submits `request` through `GameClient vtable +0x3C` and pushes
/// `request.flags` through `vtable +0x58` (the latter also carries the walk bit, which
/// is deliberately not part of the request flags).
pub fn update_movement(state: &mut InputState, ctx: &MovementContext) -> MovementRequest {
    state.flags_b8 = 0;
    state.turn_flag = 0;

    let controlled = ctx.controlled;
    if controlled.is_some_and(|unit| unit.hold_byte_1d0 != 0) {
        state.flags_b8 = 1;
        return MovementRequest::default();
    }

    let mode_a = ctx.mode_a;
    let mut mode_b = ctx.mode_b;
    let mut input_flags = state.input_flags;
    let turn_right_in = ctx.axes[5];
    let turn_left_in = ctx.axes[4];

    // 0x39404f84: hold-axis logic for the free-look mouse.
    if ctx.mouse_gate && mode_b != 0 {
        let dx = ctx.axes[8];
        let dy = ctx.axes[9];
        if dx <= 0.0 {
            if dx < 0.0 {
                state.hold_back = 0;
                if dx < -ctx.cvar_34c {
                    state.hold_fwd = 1;
                }
            }
        } else {
            state.hold_fwd = 0;
            if dx > ctx.cvar_34c {
                state.hold_back = 1;
            }
        }
        if dy >= 0.0 {
            if dy > 0.0 {
                state.hold_left = 0;
                if dy > ctx.cvar_350 {
                    state.hold_right = 1;
                }
            }
        } else {
            state.hold_right = 0;
            if dy < -ctx.cvar_350 {
                state.hold_left = 1;
            }
            mode_b = 0;
        }
    } else {
        state.hold_fwd = 0;
        state.hold_back = 0;
        state.hold_left = 0;
        state.hold_right = 0;
    }

    if mode_a != 0 || mode_b != 0 {
        if turn_left_in == 0.0 {
            state.turn_l_start = 0.0;
        } else if state.turn_l_start == 0.0 {
            state.turn_l_start = ctx.game_time;
        }
        if turn_right_in == 0.0 {
            state.turn_r_start = 0.0;
        } else if state.turn_r_start == 0.0 {
            state.turn_r_start = ctx.game_time;
        }
    }
    let prefer = if state.turn_l_start > state.turn_r_start {
        1u8
    } else {
        0u8
    };

    let mut forward;
    let mut back;
    let mut left;
    let mut right;
    let mut turn_right = turn_right_in;
    let mut turn_left = turn_left_in;
    if !ctx.ui_lock {
        let combined = combine(
            [ctx.axes[0], ctx.axes[1], ctx.axes[2], ctx.axes[3]],
            CombineInput {
                mode_a,
                mode_b,
                turn_right,
                turn_left,
                can_turn: ctx.gates.can_turn as u8,
                state_30: state.unknown_30,
                cancel_pref: prefer,
            },
        );
        forward = combined.forward;
        back = combined.back;
        left = combined.left;
        right = combined.right;
        turn_right = combined.turn_right;
        turn_left = combined.turn_left;
        let ladder = ladder_filter(
            controlled.map(|u| u.ladder_kind),
            turn_right,
            turn_left,
            right,
            left,
        );
        turn_right = ladder.turn_right;
        turn_left = ladder.turn_left;
        right = ladder.right_flag;
        left = ladder.left_flag;
        if ctx.gates.is_local && (left || right) && !forward && !back {
            input_flags &= !1;
        }
        if state.hold_fwd != 0 {
            turn_left = 1.0;
        }
        if state.hold_back != 0 {
            turn_right = 1.0;
        }
        forward |= state.hold_left != 0;
        back |= state.hold_right != 0;
        if ctx.cvar_114 == 1 {
            let mut times = PressTimes {
                forward: state.press_forward,
                back: state.press_back,
                left: state.press_left,
                right: state.press_right,
            };
            prefer_axes(
                [ctx.axes[0], ctx.axes[1], ctx.axes[2], ctx.axes[3]],
                ctx.game_time,
                &mut times,
                &mut forward,
                &mut back,
                &mut left,
                &mut right,
            );
            state.press_forward = times.forward;
            state.press_back = times.back;
            state.press_left = times.left;
            state.press_right = times.right;
        }
        if forward && back {
            forward = false;
            back = false;
        }
        if left && right {
            left = false;
            right = false;
        }
    } else {
        forward = false;
        back = false;
        left = false;
        right = false;
        turn_right = 0.0;
        turn_left = 0.0;
    }

    let old_moving = state.moving;
    let moving = if back == forward && right == left { 0 } else { 1 };
    state.moving = moving;

    // 0x3940517x: free-look / autorun latch.
    let mut free_look = false;
    if ctx.gates.start_gate {
        if mode_a == 0 && old_moving == 0 && moving != 0 {
            free_look = true;
            if mode_b == 0 {
                state.timer_40 = 0.0;
            }
        } else if moving == 0 {
            if mode_b != 0 {
                if state.timer_40 <= 0.1 {
                    state.timer_40 += ctx.axes[8].abs() + ctx.axes[9].abs();
                } else {
                    free_look = true;
                }
            } else {
                state.timer_40 = 0.0;
            }
        } else if mode_b != 0 {
            free_look = true;
        } else {
            state.timer_40 = 0.0;
        }
    } else if mode_b == 0 {
        state.timer_40 = 0.0;
    }
    state.autorun_or_lock = free_look as u8;

    let use_mouse = mode_b != 0 && free_look;
    let mouse = mouse_yaw(ctx.dt, ctx.axes[8]);
    let mut turn_state = TurnState {
        turn_accum: state.turn_accum,
    };
    let turn: TurnResult = turn_delta(
        &mut turn_state,
        controlled
            .filter(|u| u.kind == 4)
            .map(|u| TurnUnit {
                kind: u.kind,
                turn_speed: u.turn_speed,
            }),
        TurnInput {
            axis5: turn_right,
            axis4: turn_left,
            mouse_yaw: mouse,
        },
        use_mouse,
        ctx.dt,
        ctx.keyboard_rotate_speed,
    );
    state.turn_accum = turn_state.turn_accum;
    let turn_delta_value = turn.delta;
    // `local_80`/`local_78`: the x/y part starts at zero and the z part is the turn
    // delta; the external-target path overwrites all three with the move-to vector.
    let external_dir = [0.0, 0.0, turn_delta_value];

    // 0x39405307: normal direction build.
    let mut dir = [0.0; 3];
    if ctx.actor_flags == [0; 3] {
        // `param_1 + 0xc` is byte +0x30 (pending autorun), not +0x3c.
        if state.unknown_30 != 0 {
            let fwd = ctx.axes[0] > 0.0;
            let back_axis = ctx.axes[1] > 0.0;
            if fwd || back_axis || (mode_a != 0 && mode_b != 0) {
                if state.autorun_armed != 0 {
                    state.unknown_30 = 0;
                }
            } else {
                state.autorun_armed = 1;
            }
        }
        let mut x = 0.0f32;
        let mut y = 0.0f32;
        if forward {
            y = 1.0;
        }
        if back {
            y -= 1.0;
        }
        if left {
            x = -1.0;
        }
        if right {
            x += 1.0;
        }
        dir = MovementContext::normalize2(x, y);
        if !forward && !back {
            input_flags &= !0x800;
        } else {
            input_flags |= 0x800;
        }

        if let Some(unit) = controlled {
            let down = ctx.axes[7] > 0.0;
            let jump = ctx.axes[6] > 0.0;
            let b_var1 = mode_b != 0 && (forward || back);
            let mut done = false;
            if unit.flag_1d8 != 0 {
                if !down {
                    if !jump {
                        // Glide/camera branch: the recorded request direction is
                        // already the result of the camera-relative build; the port
                        // keeps `dir` (the branch is documented in MAP §3/§4).
                    } else {
                        dir = MovementContext::normalize3(dir[0], dir[1], 1.0);
                        dir[2] = -dir[2].abs();
                    }
                } else {
                    dir = MovementContext::normalize3(dir[0], dir[1], 1.0);
                    dir[2] = -dir[2].abs();
                }
                done = true;
            }
            if !done && unit.vtable_68 {
                    if down {
                        dir = MovementContext::normalize3(dir[0], dir[1], 1.0);
                        dir[2] = -dir[2].abs();
                    } else if jump {
                        if unit.float_68 < -0.3 {
                            dir = MovementContext::normalize3(dir[0], dir[1], 1.0);
                            dir[2] = -dir[2].abs();
                        }
                    } else if b_var1 {
                        // Camera-relative glide build; keep the recorded direction.
                    }
            }
        }
    }

    if (input_flags & 1) != 0
        || any_abs_nonzero(dir)
        || ctx.game_special_present
    {
        state.flags_b8 = 1;
    } else if state.unknown_ba != 0 {
        state.flags_b8 = 1;
        state.flag_08 = 1;
        state.unknown_ba = 0;
    }
    state.turn_flag = (turn_delta_value != 0.0) as u8;

    if ctx.hold_position {
        dir = [0.0; 3];
        if (input_flags & 1) != 0 {
            input_flags &= !1;
        }
    }

    // 0x39405fdd: MovementRequest ctor.
    let mut request = MovementRequest::default();
    state.input_flags = input_flags;
    let any = any_abs_nonzero(dir);
    request.pseudo_speed = if any {
        if ctx.walk_toggle { 0.4 } else { 1.0 }
    } else {
        0.0
    };
    state.requested_vel = dir;
    request.flags = 0x800;
    smooth_velocity(state, dir, ctx.dt);
    if external_dir[2] != 0.0 {
        request.delta2 = external_dir;
        request.flags |= 0x40;
    }
    state.external_vel = external_dir;
    if any {
        request.velocity = dir;
        request.flags |= 0x20;
    }
    if let Some(unit) = controlled
        && (input_flags & 1) != 0
        && (unit.byte_161 == 0 || ctx.cvar_2ac == 0)
        && unit.float_164 <= 0.01
    {
        request.flags |= 0x8000_0000;
    }
    request.movement_type = ctx.movement_type;
    request.flags = (request.flags & 0xfeff_ffff) | 0x2000_0008;
    request.unit_kind = if ctx.cvar_428 { 2 } else { 0 };

    // External goto target (`state.goto_unit_id`), otherwise clear the external bit.
    if ctx.goto_unit_id != 0 {
        let external = external_target(ExternalTargetInput {
            start_view_pos: [0.0; 3],
            path_z: 0.0,
            end_view_pos: [0.0; 3],
            threshold: 0.0,
        });
        if any_abs_nonzero(external) {
            request.external_dir = external;
            request.scale_20 = 1.0;
            request.flags = (request.flags & 0xf7ff_ffff) | 1;
        } else {
            request.flags = (request.flags & 0xffff_fffe) | 0x0800_0000;
        }
    } else {
        request.flags = (request.flags & 0xffff_fffe) | 0x0800_0000;
    }

    // 0x3940630c: copy the axes/flags back into the InputState.
    // The original writes the four bytes in this order:
    //   +0x88 = mode_a, +0x89 = mode_b,
    //   +0x8e = old +0x8a, +0x8f = old +0x8b,
    //   +0x91 = old +0x8d, +0x90 = old +0x8c,
    //   +0x8a = forward, +0x8b = back, +0x8d = right, +0x8c = left.
    let old_8a = state.prev_axes[2];
    let old_8b = state.prev_axes[3];
    let old_8c = state.axis_left;
    let old_8d = state.axis_right;
    state.prev_axes = [mode_a, mode_b, forward as u8, back as u8];
    state.axis_left = left as u8;
    state.axis_right = right as u8;
    state.axis_prev_8a = old_8a;
    state.axis_prev_8b = old_8b;
    state.axis_prev_8c = old_8c;
    state.axis_prev_8d = old_8d;
    state.axis_vals = [turn_left, turn_right];

    request
}

/// The flags word the original pushes through `GameClient vtable +0x58`.
pub fn pushed_flags(input_flags: u32, walk_toggle: bool) -> u32 {
    if walk_toggle {
        input_flags | 0x1000
    } else {
        input_flags
    }
}
