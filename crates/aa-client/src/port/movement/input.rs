//! `FUN_39402590` (combiner), `FUN_39402690` (axis preference) and `FUN_39402f60`
//! (ladder-axis filter), all in x2game.dll.
//!
//! The originals use non-standard register arguments. `FUN_39402590` writes four flags
//! through two stack pointers and `ESI`/`EDI`; `FUN_39402f60` takes the unit in `EAX` and two
//! flag pointers in `ESI`/`EDI`. The Rust signatures spell every value out so the differential
//! harness can drive them from a scratch frame.

/// Inputs of `FUN_39402590` besides the four live action axes 0..3.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CombineInput {
    /// `[ebp+8]`: first input-mode byte (`FUN_392aa250(1)` in the caller).
    pub mode_a: u8,
    /// `[ebp+0xc]`: second input-mode byte (`FUN_392aa250(3)`).
    pub mode_b: u8,
    /// `[ebp+0x10]`: turn-right axis 5 (`FUN_39424580(5)`), passed by pointer.
    pub turn_right: f32,
    /// `[ebp+0x14]`: turn-left axis 4 (`FUN_39424580(4)`), passed by pointer.
    pub turn_left: f32,
    /// `[ebp+0x20]`: result of `FUN_39403a90` ("this is the local unit / may turn").
    pub can_turn: u8,
    /// `[ebp+0x24]`: `InputState+0x30`.
    pub state_30: u8,
    /// `[ebp+0x28]`: the axis-preference bit; drops the right turn axis.
    pub cancel_pref: u8,
}

/// Outputs of `FUN_39402590`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CombineOutput {
    /// `[ebp+0x18]`: forward flag (also set by `state_30` or axis 0).
    pub forward: bool,
    /// `ESI` byte: back flag (`axis1 > 0`).
    pub back: bool,
    /// `EDI` byte: left flag (`axis2 > 0`, or the turn-cancel conversion).
    pub left: bool,
    /// `[ebp+0x1c]` byte: right flag (`axis3 > 0`, or the turn-cancel conversion).
    pub right: bool,
    /// Possibly cleared `turn_right` value.
    pub turn_right: f32,
    /// Possibly cleared `turn_left` value.
    pub turn_left: f32,
}

/// `FUN_39402590` (x2game.dll, 0x39402590, 246 bytes).
///
/// `axes[0..4]` are the live analog values of `moveforward`, `moveback`, `moveleft`,
/// `moveright`; the original fetches them through `FUN_39424580(0..3)`. Forward/back/left/right
/// start from axes 0..3 and the two turn axes are cancelled into the strafe flags while the
/// local unit may turn. The four output bytes and both turn-axis floats are returned.
pub fn combine(axes: [f32; 4], input: CombineInput) -> CombineOutput {
    let mut forward = input.state_30 != 0;
    if axes[0] != 0.0 {
        forward = true;
    }
    let back = axes[1] > 0.0;
    let mut left = axes[2] > 0.0;
    let mut right = axes[3] > 0.0;
    let mut turn_right = input.turn_right;
    let mut turn_left = input.turn_left;

    if input.can_turn != 0 {
        if input.mode_a != 0 || input.mode_b != 0 {
            if turn_left > 0.0 {
                let mut drop_left = false;
                if turn_right > 0.0 {
                    if input.cancel_pref != 0 {
                        turn_right = 0.0;
                    } else {
                        // `je 0x39402655`: zero the left axis without setting its flag.
                        drop_left = true;
                    }
                }
                if !drop_left && turn_left > 0.0 {
                    left = true;
                }
                turn_left = 0.0;
            }
            if turn_right > 0.0 {
                right = true;
                turn_right = 0.0;
            }
        } else {
            return CombineOutput {
                forward,
                back,
                left,
                right,
                turn_right,
                turn_left,
            };
        }
    } else {
        left = false;
        right = false;
    }
    if input.mode_a != 0 && input.mode_b != 0 {
        forward = true;
    }

    CombineOutput {
        forward,
        back,
        left,
        right,
        turn_right,
        turn_left,
    }
}

/// Press-time block of the InputState used by `FUN_39402690` (`+0x9c..+0xa8`).
///
/// The four slots are written for axes 0..3 in order, i.e. forward, back, left, right.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PressTimes {
    /// `this+0x9c`: axis 0 (`moveforward`) press start.
    pub forward: f32,
    /// `this+0xa0`: axis 1 (`moveback`) press start.
    pub back: f32,
    /// `this+0xa4`: axis 2 (`moveleft`) press start.
    pub left: f32,
    /// `this+0xa8`: axis 3 (`moveright`) press start.
    pub right: f32,
}

/// `FUN_39402690` (x2game.dll, 0x39402690, 371 bytes), `__thiscall` on the InputState.
///
/// `now` is the game-time value the original reads from
/// `(*(DAT_3a5bb1d8+0x14)->vtable[0x2c])()`. For each of the four axes, a zero axis clears its
/// press time and a non-zero axis starts it when it was zero. When two opposite flags are both
/// set, the direction that was pressed earlier is dropped (the later press wins).
pub fn prefer_axes(
    axes: [f32; 4],
    now: f32,
    times: &mut PressTimes,
    forward: &mut bool,
    back: &mut bool,
    left: &mut bool,
    right: &mut bool,
) {
    let update = |axis: f32, time: &mut f32| {
        if axis == 0.0 {
            *time = 0.0;
        } else if *time == 0.0 {
            *time = now;
        }
    };
    update(axes[0], &mut times.forward);
    update(axes[1], &mut times.back);
    update(axes[2], &mut times.left);
    update(axes[3], &mut times.right);

    if *forward && *back {
        if times.back > times.forward {
            *forward = false;
        } else {
            *back = false;
        }
    }
    if *left && *right {
        if times.right > times.left {
            *left = false;
        } else {
            *right = false;
        }
    }
}

/// Result of `FUN_39402f60`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LadderOutput {
    pub turn_right: f32,
    pub turn_left: f32,
    pub right_flag: bool,
    pub left_flag: bool,
}

/// `FUN_39402f60` (x2game.dll, 0x39402f60, 126 bytes).
///
/// `EAX` is the controlled unit (`unit+0x23c` holds the ladder kind), `ESI`/`EDI` and the two
/// stack arguments are the two turn-axis floats and their flags. Kind 4 (net) and 8 (guardrail)
/// convert a positive axis to a flag and zero the axis; kinds 6 (hrope), 7 (mast) and 10
/// (bigtree) convert without zeroing. `unit = None` does nothing.
pub fn ladder_filter(
    unit_kind: Option<u32>,
    turn_right: f32,
    turn_left: f32,
    mut right_flag: bool,
    mut left_flag: bool,
) -> LadderOutput {
    let mut zero_axes = false;
    let mut keep_axes = false;
    if let Some(kind) = unit_kind {
        zero_axes = kind == 4 || kind == 8;
        keep_axes = kind == 6 || kind == 7 || kind == 10;
    }
    let mut turn_right = turn_right;
    let mut turn_left = turn_left;
    if zero_axes {
        if turn_right > 0.0 {
            right_flag = true;
            turn_right = 0.0;
        }
        if turn_left > 0.0 {
            left_flag = true;
            turn_left = 0.0;
        }
    }
    if keep_axes {
        if turn_right > 0.0 {
            right_flag = true;
        }
        if turn_left > 0.0 {
            left_flag = true;
        }
    }
    LadderOutput {
        turn_right,
        turn_left,
        right_flag,
        left_flag,
    }
}
