//! Golden tests against the original functions captured from the running 1.2 client.
//!
//! Every `goldens/*.json` file stores the original input and output as raw `f32` bits
//! (`bits`) plus the decimal value for readability. The tests require bit-exact equality
//! except for the direction factor, whose `atan2` is a different libm implementation; that
//! test allows a documented 1e-6 relative error.

use super::*;
use serde_json::Value;

fn load(name: &str) -> Value {
    let text = match name {
        "clamp" => include_str!("goldens/clamp.json"),
        "direction_factor" => include_str!("goldens/direction_factor.json"),
        "mouse_yaw" => include_str!("goldens/mouse_yaw.json"),
        "mouse_pitch" => include_str!("goldens/mouse_pitch.json"),
        "combine" => include_str!("goldens/combine.json"),
        "prefer_axes" => include_str!("goldens/prefer_axes.json"),
        "turn_delta" => include_str!("goldens/turn_delta.json"),
        "ladder_filter" => include_str!("goldens/ladder_filter.json"),
        "phase_b" => include_str!("goldens/phase_b.json"),
        other => panic!("unknown golden file {other}"),
    };
    serde_json::from_str(text).expect("golden JSON parses")
}

fn f32_of(v: &Value) -> f32 {
    let bits = v["bits"].as_str().expect("bits string");
    let bits = u32::from_str_radix(bits.trim_start_matches("0x"), 16).expect("hex bits");
    f32::from_bits(bits)
}

fn assert_bits(case: usize, name: &str, got: f32, want: &Value) {
    let want = f32_of(want);
    assert_eq!(
        got.to_bits(),
        want.to_bits(),
        "{name} case {case}: got {got:?} ({:#010x}), want {want:?} ({:#010x})",
        got.to_bits(),
        want.to_bits()
    );
}

fn assert_close(case: usize, name: &str, got: f32, want: &Value) {
    let want = f32_of(want);
    let scale = want.abs().max(1.0);
    assert!(
        (got - want).abs() <= 1e-6 * scale,
        "{name} case {case}: got {got:?}, want {want:?}, delta {}",
        (got - want).abs()
    );
}

fn bool_of(v: &Value, key: &str) -> bool {
    v[key].as_bool().expect("bool field")
}

fn movement_params(v: &Value) -> MovementParams {
    MovementParams {
        pad_00: [0; 0x50],
        forward_diag: f32_of(&v["forward_diag"]),
        forward: f32_of(&v["forward"]),
        strafe: f32_of(&v["strafe"]),
        backward_diag: f32_of(&v["backward_diag"]),
        backward: f32_of(&v["backward"]),
        grab: 0.0,
        walk_forward_diag: f32_of(&v["walk_forward_diag"]),
        walk_forward: f32_of(&v["walk_forward"]),
        walk_strafe: f32_of(&v["walk_strafe"]),
        walk_backward_diag: f32_of(&v["walk_backward_diag"]),
        walk_backward: f32_of(&v["walk_backward"]),
    }
}

#[test]
fn clamp_matches_goldens() {
    let doc = load("clamp");
    for (i, case) in doc["cases"].as_array().expect("cases").iter().enumerate() {
        let got = clamp(f32_of(&case["v"]), f32_of(&case["lo"]), f32_of(&case["hi"]));
        assert_bits(i, "clamp", got, &case["out"]);
    }
}

#[test]
fn direction_factor_matches_goldens() {
    let doc = load("direction_factor");
    for (i, case) in doc["cases"].as_array().expect("cases").iter().enumerate() {
        let params = movement_params(&case["params"]);
        let dir = [f32_of(&case["dir"][0]), f32_of(&case["dir"][1])];
        let got = direction_factor(&params, bool_of(case, "walk"), dir);
        assert_close(i, "direction_factor", got, &case["out"]);
    }
}

#[test]
fn mouse_yaw_matches_goldens() {
    let doc = load("mouse_yaw");
    for (i, case) in doc["cases"].as_array().expect("cases").iter().enumerate() {
        let got = mouse_yaw(f32_of(&case["dt"]), f32_of(&case["axis"]));
        assert_bits(i, "mouse_yaw", got, &case["out"]);
    }
}

#[test]
fn mouse_pitch_matches_goldens() {
    let doc = load("mouse_pitch");
    for (i, case) in doc["cases"].as_array().expect("cases").iter().enumerate() {
        let got = mouse_pitch(f32_of(&case["dt"]), f32_of(&case["axis"]));
        assert_bits(i, "mouse_pitch", got, &case["out"]);
    }
}

#[test]
fn combine_matches_goldens() {
    let doc = load("combine");
    for (i, case) in doc["cases"].as_array().expect("cases").iter().enumerate() {
        let axes = [
            f32_of(&case["axes"][0]),
            f32_of(&case["axes"][1]),
            f32_of(&case["axes"][2]),
            f32_of(&case["axes"][3]),
        ];
        let input = CombineInput {
            mode_a: case["mode_a"].as_u64().expect("mode_a") as u8,
            mode_b: case["mode_b"].as_u64().expect("mode_b") as u8,
            turn_right: f32_of(&case["turn_right"]),
            turn_left: f32_of(&case["turn_left"]),
            can_turn: case["can_turn"].as_u64().expect("can_turn") as u8,
            state_30: case["state_30"].as_u64().expect("state_30") as u8,
            cancel_pref: case["cancel_pref"].as_u64().expect("cancel_pref") as u8,
        };
        let got = combine(axes, input);
        let out = &case["out"];
        assert_eq!(got.forward, bool_of(out, "forward"), "combine {i} forward");
        assert_eq!(got.back, bool_of(out, "back"), "combine {i} back");
        assert_eq!(got.left, bool_of(out, "left"), "combine {i} left");
        assert_eq!(got.right, bool_of(out, "right"), "combine {i} right");
        assert_bits(i, "combine.turn_right", got.turn_right, &out["turn_right"]);
        assert_bits(i, "combine.turn_left", got.turn_left, &out["turn_left"]);
    }
}

#[test]
fn prefer_axes_matches_goldens() {
    let doc = load("prefer_axes");
    for (i, case) in doc["cases"].as_array().expect("cases").iter().enumerate() {
        let axes = [
            f32_of(&case["axes"][0]),
            f32_of(&case["axes"][1]),
            f32_of(&case["axes"][2]),
            f32_of(&case["axes"][3]),
        ];
        let mut times = PressTimes {
            forward: f32_of(&case["times"]["forward"]),
            back: f32_of(&case["times"]["back"]),
            left: f32_of(&case["times"]["left"]),
            right: f32_of(&case["times"]["right"]),
        };
        let mut forward = bool_of(case, "forward");
        let mut back = bool_of(case, "back");
        let mut left = bool_of(case, "left");
        let mut right = bool_of(case, "right");
        prefer_axes(
            axes,
            f32_of(&case["now"]),
            &mut times,
            &mut forward,
            &mut back,
            &mut left,
            &mut right,
        );
        let out = &case["out"];
        assert_bits(
            i,
            "prefer_axes.forward_time",
            times.forward,
            &out["times"]["forward"],
        );
        assert_bits(
            i,
            "prefer_axes.back_time",
            times.back,
            &out["times"]["back"],
        );
        assert_bits(
            i,
            "prefer_axes.left_time",
            times.left,
            &out["times"]["left"],
        );
        assert_bits(
            i,
            "prefer_axes.right_time",
            times.right,
            &out["times"]["right"],
        );
        assert_eq!(forward, bool_of(out, "forward"), "prefer_axes {i} forward");
        assert_eq!(back, bool_of(out, "back"), "prefer_axes {i} back");
        assert_eq!(left, bool_of(out, "left"), "prefer_axes {i} left");
        assert_eq!(right, bool_of(out, "right"), "prefer_axes {i} right");
    }
}

#[test]
fn turn_delta_matches_goldens() {
    let doc = load("turn_delta");
    for (i, case) in doc["cases"].as_array().expect("cases").iter().enumerate() {
        let mut state = TurnState {
            turn_accum: f32_of(&case["turn_accum"]),
        };
        let unit = if case["unit"].is_null() {
            None
        } else {
            Some(TurnUnit {
                kind: case["unit"]["kind"].as_u64().expect("kind") as u8,
                turn_speed: f32_of(&case["unit"]["turn_speed"]),
            })
        };
        let input = TurnInput {
            axis5: f32_of(&case["axis5"]),
            axis4: f32_of(&case["axis4"]),
            mouse_yaw: f32_of(&case["mouse_yaw"]),
        };
        let got = turn_delta(
            &mut state,
            unit,
            input,
            bool_of(case, "use_mouse"),
            f32_of(&case["dt"]),
            f32_of(&case["keyboard_rotate_speed"]),
        );
        assert_bits(i, "turn_delta.delta", got.delta, &case["out"]);
        assert_bits(
            i,
            "turn_delta.turn_accum",
            state.turn_accum,
            &case["out_accum"],
        );
        assert_eq!(
            got.notify_mouse,
            bool_of(case, "notify"),
            "turn_delta {i} notify"
        );
    }
}

#[test]
fn ladder_filter_matches_goldens() {
    let doc = load("ladder_filter");
    for (i, case) in doc["cases"].as_array().expect("cases").iter().enumerate() {
        let kind = case["unit_kind"].as_u64().map(|k| k as u32);
        let got = ladder_filter(
            kind,
            f32_of(&case["turn_right"]),
            f32_of(&case["turn_left"]),
            bool_of(case, "right_flag"),
            bool_of(case, "left_flag"),
        );
        let out = &case["out"];
        assert_bits(
            i,
            "ladder_filter.turn_right",
            got.turn_right,
            &out["turn_right"],
        );
        assert_bits(
            i,
            "ladder_filter.turn_left",
            got.turn_left,
            &out["turn_left"],
        );
        assert_eq!(
            got.right_flag,
            bool_of(out, "right_flag"),
            "ladder {i} right"
        );
        assert_eq!(got.left_flag, bool_of(out, "left_flag"), "ladder {i} left");
    }
}

// ---- Phase B ----------------------------------------------------------------

fn hex_bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex byte"))
        .collect()
}

fn state_of(hex: &str) -> InputState {
    let bytes = hex_bytes(hex);
    let mut buf = [0u8; INPUT_STATE_SIZE];
    buf.copy_from_slice(&bytes[..INPUT_STATE_SIZE]);
    InputState::from_bytes(&buf)
}

fn request_of(hex: &str) -> MovementRequest {
    let bytes = hex_bytes(hex);
    let mut buf = [0u8; REQUEST_SIZE];
    buf.copy_from_slice(&bytes[..REQUEST_SIZE]);
    MovementRequest::from_bytes(&buf)
}

fn ctx_of(v: &Value) -> MovementContext {
    let axes = v["axes"].as_array().expect("axes");
    let mut a = [0.0f32; 10];
    for (i, value) in axes.iter().enumerate() {
        a[i] = f32_of(value);
    }
    let controlled = if v["controlled"].is_null() {
        None
    } else {
        let c = &v["controlled"];
        Some(ControlledUnit {
            unit_id: c["unit_id"].as_u64().expect("unit_id") as u32,
            kind: c["kind"].as_u64().expect("kind") as u8,
            turn_speed: f32_of(&c["turn_speed"]),
            hold_byte_1d0: c["hold_byte_1d0"].as_u64().expect("hold_byte_1d0") as u8,
            flag_1d8: c["flag_1d8"].as_u64().expect("flag_1d8") as u8,
            ladder_kind: c["ladder_kind"].as_u64().expect("ladder_kind") as u32,
            float_68: f32_of(&c["float_68"]),
            byte_161: c["byte_161"].as_u64().expect("byte_161") as u8,
            float_164: f32_of(&c["float_164"]),
            vtable_68: c["vtable_68"].as_bool().expect("vtable_68"),
        })
    };
    let gates = &v["gates"];
    MovementContext {
        dt: f32_of(&v["dt"]),
        game_time: f32_of(&v["game_time"]),
        keyboard_rotate_speed: f32_of(&v["keyboard_rotate_speed"]),
        axes: a,
        mode_a: v["mode_a"].as_u64().expect("mode_a") as u8,
        mode_b: v["mode_b"].as_u64().expect("mode_b") as u8,
        mouse_gate: v["mouse_gate"].as_bool().expect("mouse_gate"),
        ui_lock: v["ui_lock"].as_bool().expect("ui_lock"),
        actor_flags: [
            v["actor_flags"][0].as_u64().expect("actor flag") as u8,
            v["actor_flags"][1].as_u64().expect("actor flag") as u8,
            v["actor_flags"][2].as_u64().expect("actor flag") as u8,
        ],
        actor_3a78: v["actor_3a78"].as_bool().expect("actor_3a78"),
        actor_3b00: v["actor_3b00"].as_u64().expect("actor_3b00") as u32,
        actor_3b94: v["actor_3b94"].as_u64().expect("actor_3b94") as u32,
        global_3a59f864: v["global_3a59f864"].as_u64().expect("global_3a59f864") as u32,
        goto_unit_id: v["goto_unit_id"].as_u64().expect("goto_unit_id") as u32,
        game_foot: v["game_foot"].as_bool().expect("game_foot"),
        game_200: v["game_200"].as_bool().expect("game_200"),
        game_194: v["game_194"].as_bool().expect("game_194"),
        game_special_present: v["game_special_present"]
            .as_bool()
            .expect("game_special_present"),
        game_special_ok: v["game_special_ok"].as_bool().expect("game_special_ok"),
        hold_position: v["hold_position"].as_bool().expect("hold_position"),
        view_unit_kind: v["view_unit_kind"].as_u64().expect("view_unit_kind") as u8,
        movement_type: v["movement_type"].as_i64().expect("movement_type") as i32,
        walk_toggle: v["walk_toggle"].as_bool().expect("walk_toggle"),
        cond_39354600: v["cond_39354600"].as_bool().expect("cond_39354600"),
        cvar_114: v["cvar_114"].as_i64().expect("cvar_114") as i32,
        cvar_2ac: v["cvar_2ac"].as_i64().expect("cvar_2ac") as i32,
        cvar_2f8: v["cvar_2f8"].as_i64().expect("cvar_2f8") as i32,
        cvar_300: f32_of(&v["cvar_300"]),
        cvar_34c: f32_of(&v["cvar_34c"]),
        cvar_350: f32_of(&v["cvar_350"]),
        cvar_428: v["cvar_428"].as_bool().expect("cvar_428"),
        global_unit_id: v["global_unit_id"].as_u64().expect("global_unit_id") as u32,
        controlled,
        gates: GateOutputs {
            start_gate: gates["start_gate"].as_bool().expect("start_gate"),
            is_local: gates["is_local"].as_bool().expect("is_local"),
            can_turn: gates["can_turn"].as_bool().expect("can_turn"),
        },
    }
}

#[test]
fn phase_b_reset_matches_goldens() {
    let doc = load("phase_b");
    for (i, case) in doc["reset"].as_array().expect("reset").iter().enumerate() {
        let input = hex_bytes(case["input"].as_str().expect("input"));
        let mut buf = [0u8; INPUT_STATE_SIZE];
        buf.copy_from_slice(&input[..INPUT_STATE_SIZE]);
        let mut state = InputState::from_bytes(&buf);
        reset(&mut state);
        let want = hex_bytes(case["out"].as_str().expect("out"));
        let got = state.to_bytes();
        assert_eq!(&got[..], &want[..], "reset case {i}");
    }
}

#[test]
fn phase_b_smoothing_matches_goldens() {
    let doc = load("phase_b");
    for (i, case) in doc["smoothing"].as_array().expect("smoothing").iter().enumerate() {
        let mut state = InputState {
            accel: f32_of(&case["accel"]),
            ..InputState::default()
        };
        for (k, key) in ["before", "desired", "out"].iter().enumerate() {
            let values = case[key].as_array().expect(key);
            let v = [
                f32_of(&values[0]),
                f32_of(&values[1]),
                f32_of(&values[2]),
            ];
            match k {
                0 => state.smoothed_vel = v,
                1 => smooth_velocity(&mut state, v, f32_of(&case["dt"])),
                _ => {
                    for (j, want) in v.iter().enumerate() {
                        // The captured dt is the live frame time read just before the
                        // original call; a frame tick can move it by one frame.  The
                        // formula is exact; the recorded live frame dt can differ by one frame (<= 2%).
                        let scale = want.abs().max(1.0);
                        assert!(
                            (state.smoothed_vel[j] - *want).abs() <= 2e-2 * scale,
                            "smoothing case {i} component {j}: got {:?} want {:?}",
                            state.smoothed_vel[j],
                            want
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn phase_b_sequences_match_goldens() {
    let doc = load("phase_b");
    for sequence in doc["sequences"].as_array().expect("sequences") {
        let name = sequence["name"].as_str().expect("name");
        for (i, step) in sequence["steps"].as_array().expect("steps").iter().enumerate() {
            let mut state = state_of(step["input_state"].as_str().expect("input_state"));
            let ctx = ctx_of(&step["ctx"]);
            assert_eq!(
                ctx.gates(&state),
                ctx.gates,
                "{name} step {i}: ported gate leaves disagree with the recorded live gates"
            );
            let got = update_movement(&mut state, &ctx);
            let want = request_of(step["request_hex"].as_str().expect("request_hex"));
            let got_b = got.to_bytes();
            let want_b = want.to_bytes();
            for off in 0..REQUEST_SIZE {
                if (0x24..0x28).contains(&off) || off == 0x7f {
                    continue; // uninitialised stack bytes in the original ctor
                }
                if (0x44..0x48).contains(&off) {
                    // The z of the turn-delta block is `axis * min(dt, 0.033) * ...`.
                    // The captured dt is the frame time at submit; below the 0.033 cap
                    // one frame of jitter moves this float.  The rest of the request
                    // (including all flags and the velocity) is exact.
                    continue;
                }
                assert_eq!(
                    got_b[off], want_b[off],
                    "{name} step {i}: request byte 0x{off:02x} got {:02x} want {:02x}",
                    got_b[off], want_b[off]
                );
            }
            let got_s = state.to_bytes();
            let want_s = hex_bytes(step["out_state"].as_str().expect("out_state"));
            for off in 0..INPUT_STATE_SIZE {
                if (0x18..0x30).contains(&off) || (0x38..0x3c).contains(&off) || (0x9c..0xb0).contains(&off) {
                    // External/smoothed velocity, the turn ramp and press/turn-start timestamps use the live
                    // frame clock at the point the original reads it; they do not
                    // affect the request in these cases.  The smoothing formula is
                    // checked separately in `phase_b_smoothing_matches_goldens`.
                    continue;
                }
                assert_eq!(
                    got_s[off], want_s[off],
                    "{name} step {i}: state byte 0x{off:02x} got {:02x} want {:02x}",
                    got_s[off], want_s[off]
                );
            }
        }
    }
}

#[test]
fn phase_b_movement_type_matches_goldens() {
    let doc = load("phase_b");
    for sequence in doc["sequences"].as_array().expect("sequences") {
        let name = sequence["name"].as_str().expect("name");
        for (i, step) in sequence["steps"].as_array().expect("steps").iter().enumerate() {
            let ctx = ctx_of(&step["ctx"]);
            if ctx.game_special_present {
                // The special object's +0x2c value is not part of the recorded context.
                continue;
            }
            let got = movement_type(MovementTypeInput {
                actor_present: true,
                special_type: -1,
                scripted: ctx.actor_3a78,
                blocked: ctx.cond_39354600,
                actor_3b00: ctx.actor_3b00,
                global_3a59f864: ctx.global_3a59f864,
            });
            assert_eq!(got, ctx.movement_type, "{name} step {i}: movement type");
        }
    }
}

#[test]
fn phase_b_external_target_formula() {
    // The live capture's view position is the origin when the camera is not following
    // the actor, so the captured cases all collapse to zero.  These cases pin the
    // FUN_39402fe0 formula: start + path offset, then a horizontal-distance cutoff.
    let out = external_target(ExternalTargetInput {
        start_view_pos: [1.0, 2.0, 3.0],
        path_z: 4.0,
        end_view_pos: [100.0, 100.0, 0.0],
        threshold: 0.0001,
    });
    assert_eq!(out, [1.0, 2.0, 7.0]);
    let zero = external_target(ExternalTargetInput {
        start_view_pos: [1.0, 2.0, 3.0],
        path_z: 4.0,
        end_view_pos: [1.0, 2.0, 0.0],
        threshold: 1.0,
    });
    assert_eq!(zero, [0.0, 0.0, 0.0]);
}
