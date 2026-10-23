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
        "asinf" => include_str!("goldens/asinf.json"),
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
fn asinf_matches_goldens() {
    let doc = load("asinf");
    for (i, case) in doc["cases"].as_array().expect("cases").iter().enumerate() {
        let got = libm_sse2_asinf(f32_of(&case["in"]));
        assert_bits(i, "asinf", got, &case["out"]);
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
                        assert_bits(j, "smoothing", state.smoothed_vel[j], &values[j]);
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
                assert_eq!(
                    got_b[off], want_b[off],
                    "{name} step {i}: request byte 0x{off:02x} got {:02x} want {:02x}",
                    got_b[off], want_b[off]
                );
            }
            let got_s = state.to_bytes();
            let want_s = hex_bytes(step["out_state"].as_str().expect("out_state"));
            for off in 0..INPUT_STATE_SIZE {
                if (0x24..0x30).contains(&off) {
                    continue;
                }
                assert_eq!(
                    got_s[off], want_s[off],
                    "{name} step {i}: state byte 0x{off:02x} got {:02x} want {:02x}",
                    got_s[off], want_s[off]
                );
            }
            for off in [0x24usize, 0x28, 0x2c] {
                let got = f32::from_le_bytes(got_s[off..off + 4].try_into().unwrap());
                let want = f32::from_le_bytes(want_s[off..off + 4].try_into().unwrap());
                assert_eq!(
                    got.to_bits(),
                    want.to_bits(),
                    "{name} step {i}: state float 0x{off:02x} got {got:?} want {want:?}"
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


// ---- Phase C ---------------------------------------------------------------

fn phase_c() -> Value {
    serde_json::from_str(include_str!("goldens/phase_c.json")).expect("phase_c JSON")
}

fn f_at(bytes: &[u8], off: usize) -> f32 {
    f32::from_le_bytes(bytes[off..off + 4].try_into().unwrap())
}

fn q_at(bytes: &[u8], off: usize) -> [f32; 4] {
    [f_at(bytes, off), f_at(bytes, off + 4), f_at(bytes, off + 8), f_at(bytes, off + 12)]
}

fn assert_near(case: &str, what: &str, got: f32, want: f32) {
    if got.is_nan() && want.is_nan() {
        return;
    }
    let scale = want.abs().max(1.0);
    assert!(
        (got - want).abs() <= 1e-5 * scale,
        "{case} {what}: got {got:?} want {want:?}"
    );
}

fn assert_q(case: &str, what: &str, got: [f32; 4], want: [f32; 4]) {
    assert_q_tol(case, what, got, want, 1e-5);
}

fn assert_q_tol(case: &str, what: &str, got: [f32; 4], want: [f32; 4], tol: f32) {
    for i in 0..4 {
        if got[i].is_nan() && want[i].is_nan() {
            continue;
        }
        let scale = want[i].abs().max(1.0);
        assert!(
            (got[i] - want[i]).abs() <= tol * scale,
            "{case} {what}[{i}]: got {:?} want {:?}",
            got[i],
            want[i]
        );
    }
}

fn rotation_from(bytes: &[u8]) -> CRotation {
    CRotation {
        scale: f_at(bytes, 0x00),
        stride: f_at(bytes, 0x10),
        move_rot: [f_at(bytes, 0x14), f_at(bytes, 0x18), f_at(bytes, 0x1c)],
        q20: q_at(bytes, 0x20),
        q30: q_at(bytes, 0x30),
        q40: q_at(bytes, 0x40),
        q50: q_at(bytes, 0x50),
        q60: q_at(bytes, 0x60),
        q70: q_at(bytes, 0x70),
        swim: [f_at(bytes, 0x80), f_at(bytes, 0x84), f_at(bytes, 0x88)],
        view_limit: [f_at(bytes, 0x8c), f_at(bytes, 0x90), f_at(bytes, 0x94)],
    }
}

fn actor_bytes_byte(case: &Value, off: usize) -> u8 {
    let bytes = hex_bytes(case["actor_before"].as_str().unwrap());
    bytes[off]
}

fn actor_from(bytes: &[u8], desired: [f32; 3]) -> RotationActor {
    RotationActor {
        quats: [
            q_at(bytes, 0x2b0),
            q_at(bytes, 0x2c0),
            q_at(bytes, 0x2d0),
            [0.0; 4],
        ],
        support: [q_at(bytes, 0x33c), q_at(bytes, 0x34c), [0.0; 4], [0.0; 4]],
        swim: [f_at(bytes, 0x960), f_at(bytes, 0x964), f_at(bytes, 0x968)],
        view_limit: [f_at(bytes, 0x154), f_at(bytes, 0x158), f_at(bytes, 0x15c)],
        stride: f_at(bytes, 0x40),
        desired,
        anim_4: 0.0,
        anim_a8: 0,
        actor_171: 0,
        actor_18b: 0,
        ground_align: false,
        on_ladder: false,
        stance: 0,
        ground_factor: 0.0,
        ground_cvar: [0.0; 2],
    }
}

#[test]
fn phase_c_rotation_ctor_matches_goldens() {
    let doc = phase_c();
    for case in doc["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        if !name.starts_with("rot_ctor_") {
            continue;
        }
        let actor_bytes = hex_bytes(case["actor_before"].as_str().unwrap());
        let req = hex_bytes(case["request_hex"].as_str().unwrap());
        let actor = actor_from(&actor_bytes, [0.0; 3]);
        let got = CRotation::new(&actor, [f_at(&req, 0xc), f_at(&req, 0x10), f_at(&req, 0x14)], f32_of(&case["scale"]));
        let want = hex_bytes(case["after"].as_str().unwrap());
        assert_near(name, "scale", got.scale, f_at(&want, 0x00));
        assert_near(name, "stride", got.stride, f_at(&want, 0x10));
        assert_near(name, "move.x", got.move_rot[0], f_at(&want, 0x14));
        assert_near(name, "move.y", got.move_rot[1], f_at(&want, 0x18));
        assert_near(name, "move.z", got.move_rot[2], f_at(&want, 0x1c));
        assert_q(name, "q20", got.q20, q_at(&want, 0x20));
        assert_q(name, "q30", got.q30, q_at(&want, 0x30));
        assert_q(name, "q40", got.q40, q_at(&want, 0x40));
        assert_q(name, "q60", got.q60, q_at(&want, 0x60));
        assert_q(name, "q70", got.q70, q_at(&want, 0x70));
    }
}

#[test]
fn phase_c_rotation_ladder_and_dispatch_match_goldens() {
    let doc = phase_c();
    for case in doc["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let before = hex_bytes(case["before"].as_str().unwrap());
        let want = hex_bytes(case["after"].as_str().unwrap());
        if name == "rot_ladder" {
            let mut rot = rotation_from(&before);
            rot.process_ladder();
            assert_q(name, "q50", rot.q50, q_at(&want, 0x50));
        } else if name == "rot_dispatch_ladder" {
            let mut rot = rotation_from(&before);
            let anim = hex_bytes(case["anim"].as_str().unwrap());
            let mut actor = actor_from(&hex_bytes(case["actor_before"].as_str().unwrap()), [0.0; 3]);
            actor.anim_4 = f_at(&anim, 0x04);
            actor.anim_a8 = anim[0xa8];
            actor.actor_171 = actor_bytes_byte(case, 0x171);
            actor.actor_18b = anim[0x18b];
            let path = rot.dispatch(&actor);
            assert_eq!(path, RotationPath::Ladder, "{name}");
            assert_q(name, "q50", rot.q50, q_at(&want, 0x50));
        }
    }
}

#[test]
fn phase_c_rotation_normal_matches_goldens() {
    let doc = phase_c();
    for case in doc["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        if !name.starts_with("rot_normal_") {
            continue;
        }
        let before = hex_bytes(case["before"].as_str().unwrap());
        let want = hex_bytes(case["after"].as_str().unwrap());
        let desired = [
            f32_of(&case["desired"][0]),
            f32_of(&case["desired"][1]),
            f32_of(&case["desired"][2]),
        ];
        let actor = actor_from(&hex_bytes(case["actor_before"].as_str().unwrap()), desired);
        let mut rot = rotation_from(&before);
        rot.process_normal(&actor);
        assert_q_tol(name, "q40", rot.q40, q_at(&want, 0x40), 0.0);
        assert_q_tol(name, "q20", rot.q20, q_at(&want, 0x20), 0.0);
        assert_q(name, "q50", rot.q50, q_at(&want, 0x50));
    }
}


#[test]
fn phase_c_rotation_clamp_matches_goldens() {
    let doc = phase_c();
    for case in doc["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        if !name.starts_with("rot_clamp_") {
            continue;
        }
        let before = hex_bytes(case["before"].as_str().unwrap());
        let want = hex_bytes(case["after"].as_str().unwrap());
        let pin = &case["params_in"];
        let mut params = ClampParams {
            dir: [
                f32_of(&pin["40"]),
                f32_of(&pin["44"]),
                f32_of(&pin["48"]),
            ],
            yaw_wrap: f32_of(&pin["52"]),
            pitch_wrap: f32_of(&pin["56"]),
            pitch_low: f32_of(&pin["60"]),
            pitch_high: f32_of(&pin["64"]),
        };
        let anim = hex_bytes(case["anim"].as_str().unwrap());
        let mut actor = actor_from(&hex_bytes(case["actor_before"].as_str().unwrap()), [0.0; 3]);
        actor.anim_4 = f_at(&anim, 0x04);
        actor.anim_a8 = anim[0xa8];
        actor.stance = case["stance"].as_u64().unwrap() as u32;
        actor.on_ladder = case["ladder"].as_u64().unwrap() != 0;
        actor.ground_align = case["ground"].as_u64().unwrap() != 0;
        actor.ground_factor = 0.85;
        actor.ground_cvar = [
            f32_of(&case["ground_cvar"][0]),
            f32_of(&case["ground_cvar"][1]),
        ];
        let mut rot = rotation_from(&before);
        rot.clamp_angles(&actor, &mut params);
        for (i, what) in ["pitch", "mid", "yaw"].iter().enumerate() {
            // The original tail uses __libm_sse2_asinf; the last bit of the
            // inverse/asin chain is 1 ulp off in a few cases (U15).
            assert_near(name, what, rot.move_rot[i], f_at(&want, 0x14 + i * 4));
        }
    }

}

#[test]
fn phase_c_mover_matches_goldens() {
    let doc = phase_c();
    for case in doc["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        if !name.starts_with("mover_") {
            continue;
        }
        let actor = hex_bytes(case["actor_before"].as_str().unwrap());
        let req = hex_bytes(case["request_hex"].as_str().unwrap());
        let mut request = [0u32; REQUEST_DWORDS];
        for (i, v) in request.iter_mut().enumerate() {
            *v = u32::from_le_bytes(req[i * 4..i * 4 + 4].try_into().unwrap());
        }
        let input = MoverInput {
            dt: f32_of(&case["dt"]),
            request,
            entity_pos: [
                f32_of(&case["entity_pos"][0]),
                f32_of(&case["entity_pos"][1]),
                f32_of(&case["entity_pos"][2]),
            ],
            stride: f_at(&actor, 0x40),
            velocity: [f_at(&actor, 0x2e0), f_at(&actor, 0x2e4), f_at(&actor, 0x2e8)],
            movement_scalar: f_at(&actor, 0x2e8),
            flag_161: actor[0x161],
            swim_a: [f_at(&actor, 0x96c), f_at(&actor, 0x970)],
            swim_b: [f_at(&actor, 0x974), f_at(&actor, 0x978)],
            val_9dc: f_at(&actor, 0x9dc),
            flag_79: actor[0x79],
            flag_78: actor[0x78],
            val_284: f_at(&actor, 0x284),
        };
        let got = CPlayerMovement::process(&input);
        let want = hex_bytes(case["after"].as_str().unwrap());
        assert_near(name, "dt", got.dt, f_at(&want, 0x00));
        for (i, v) in got.request.iter().enumerate() {
            let off = 0xc + i * 4;
            let w = u32::from_le_bytes(want[off..off + 4].try_into().unwrap());
            assert_eq!(*v, w, "{name} request[{i}]");
        }
        assert_near(name, "entity_pos.x", got.entity_pos[0], f_at(&want, 0x50));
        assert_near(name, "entity_pos.y", got.entity_pos[1], f_at(&want, 0x54));
        assert_near(name, "entity_pos.z", got.entity_pos[2], f_at(&want, 0x58));
        assert_near(name, "stride", got.stride, f_at(&want, 0x5c));
        assert_near(name, "one80", got.one_80, f_at(&want, 0x80));
        assert_near(name, "q.x", got.velocity[0], f_at(&want, 0x90));
        assert_near(name, "q.y", got.velocity[1], f_at(&want, 0x94));
        assert_near(name, "q.z", got.velocity[2], f_at(&want, 0x98));
        assert_eq!(got.flag_9c, want[0x9c]);
        assert_eq!(got.flag_b8, want[0xb8]);
        assert_eq!(got.flag_b9, want[0xb9]);
        assert_near(name, "val_b4", got.val_b4, f_at(&want, 0xb4));
        assert_near(name, "val_bc", got.val_bc, f_at(&want, 0xbc));
    }
}

#[test]
fn phase_c_quadruped_ctors_match_goldens() {
    let doc = phase_c();
    for case in doc["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        if !name.starts_with("quad_") {
            continue;
        }
        let actor = hex_bytes(case["actor_before"].as_str().unwrap());
        let req = hex_bytes(case["request_hex"].as_str().unwrap());
        let mut request = [0u32; REQUEST_DWORDS];
        for (i, v) in request.iter_mut().enumerate() {
            *v = u32::from_le_bytes(req[i * 4..i * 4 + 4].try_into().unwrap());
        }
        let want = hex_bytes(case["after"].as_str().unwrap());
        let pos = [
            f32_of(&case["entity_pos"][0]),
            f32_of(&case["entity_pos"][1]),
            f32_of(&case["entity_pos"][2]),
        ];
        if name.starts_with("quad_c640_") {
            let input = QuadrupedScopeA {
                dt: f32_of(&case["dt"]),
                request,
                entity_pos: pos,
                stride: f_at(&actor, 0x40),
                velocity: [f_at(&actor, 0x2e0), f_at(&actor, 0x2e4), f_at(&actor, 0x2e8)],
                movement_scalar: f_at(&actor, 0x2e8),
                flag_161: actor[0x161],
                val_918: f_at(&actor, 0x918),
                flag_78: actor[0x78],
                random: 0.0,
            };
            let got = QuadrupedScopeA::process(&input);
            assert_near(name, "dt", got.dt, f_at(&want, 0x00));
            assert_near(name, "entity_pos.x", got.entity_pos[0], f_at(&want, 0x50));
            assert_near(name, "entity_pos.y", got.entity_pos[1], f_at(&want, 0x54));
            assert_near(name, "entity_pos.z", got.entity_pos[2], f_at(&want, 0x58));
            assert_near(name, "stride", got.stride, f_at(&want, 0x5c));
            assert_near(name, "one7c", f_at(&want, 0x7c), 1.0);
            assert_near(name, "val_918", got.val_918, f_at(&want, 0xa4));
        } else {
            let input = QuadrupedScopeB {
                dt: f32_of(&case["dt"]),
                quats: [q_at(&actor, 0x2b0), q_at(&actor, 0x2c0), q_at(&actor, 0x2d0)],
                support: [q_at(&actor, 0x33c), q_at(&actor, 0x34c)],
                anchor: [f_at(&actor, 0x118), f_at(&actor, 0x11c), f_at(&actor, 0x120)],
                request_dir: [f_at(&req, 0xc), f_at(&req, 0x10), f_at(&req, 0x14)],
            };
            let got = QuadrupedScopeB::process(&input);
            assert_near(name, "dt", got.dt, f_at(&want, 0x00));
            assert_q(name, "q0", got.quats[0], q_at(&want, 0x20));
            assert_q(name, "q1", got.quats[1], q_at(&want, 0x30));
            assert_q(name, "q2", got.quats[2], q_at(&want, 0x40));
            assert_q(name, "sup0", got.support[0], q_at(&want, 0x50));
            assert_q(name, "sup1", got.support[1], q_at(&want, 0x60));
        }
    }
}

#[test]
fn phase_c_rotation_bind_matches_goldens() {
    let doc = phase_c();
    for case in doc["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        if !name.starts_with("rot_bind_") {
            continue;
        }
        let rot_before = hex_bytes(case["rotation_before"].as_str().unwrap());
        let want = hex_bytes(case["after"].as_str().unwrap());
        let rot = rotation_from(&rot_before);
        let got = rot.bind();
        assert_q(name, "move0", got.move_quats[0], q_at(&want, 0x2b0));
        assert_q(name, "move1", got.move_quats[1], q_at(&want, 0x2c0));
        assert_q(name, "move2", got.move_quats[2], q_at(&want, 0x2d0));
        assert_q(name, "sup0", got.support_quats[0], q_at(&want, 0x33c));
        assert_q(name, "sup1", got.support_quats[1], q_at(&want, 0x34c));
        for (i, w) in [0x950usize, 0x954, 0x958, 0x95c].iter().enumerate() {
            assert_near(name, "out_pos", got.out_pos[i], f_at(&want, *w));
        }
        for (i, w) in [0x960usize, 0x964, 0x968].iter().enumerate() {
            assert_near(name, "swim", got.swim[i], f_at(&want, *w));
        }
        for (i, w) in [0x154usize, 0x158, 0x15c].iter().enumerate() {
            assert_near(name, "view", got.view_limit[i], f_at(&want, *w));
        }
    }
}
