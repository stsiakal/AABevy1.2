//! Golden tests against the original functions captured from the running 1.2 client.
//!
//! Every `goldens/*.json` file stores the original input and output as raw `f32` bits
//! (`bits`) plus the decimal value for readability. The tests require bit-exact equality.

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
        assert_bits(i, "direction_factor", got, &case["out"]);
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
            let got = rot.move_rot[i];
            let want_v = f_at(&want, 0x14 + i * 4);
            assert_eq!(
                got.to_bits(),
                want_v.to_bits(),
                "{name} {what}: got {got:?} ({:#010x}) want {want_v:?} ({:#010x})",
                got.to_bits(),
                want_v.to_bits()
            );
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

// ---- Quadruped / glider -----------------------------------------------------

fn quad() -> Value {
    serde_json::from_str(include_str!("goldens/quad.json")).expect("quad JSON")
}

fn ctl_from(bytes: &[u8]) -> QuadController {
    QuadController {
        dt: f_at(bytes, 0x00),
        input: [f_at(bytes, 0x0c), f_at(bytes, 0x10), f_at(bytes, 0x14)],
        jump_button: bytes[0x31],
        flags_5c: u32::from_le_bytes(bytes[0x5c..0x60].try_into().unwrap()),
        mode: u32::from_le_bytes(bytes[0x60..0x64].try_into().unwrap()),
        velocity: [f_at(bytes, 0x64), f_at(bytes, 0x68), f_at(bytes, 0x6c)],
        spare_velocity: [f_at(bytes, 0x8c), f_at(bytes, 0x90), f_at(bytes, 0x94)],
        airborne: bytes[0x98],
        jumped: bytes[0x99],
        quat: q_at(bytes, 0x48),
        movement_extra_70: f_at(bytes, 0x70),
        movement_extra_74: f_at(bytes, 0x74),
        movement_extra_78: f_at(bytes, 0x78),
        movement_extra_7c: f_at(bytes, 0x7c),
        val_a4: u32::from_le_bytes(bytes[0xa4..0xa8].try_into().unwrap()),
        flag_a8: bytes[0xa8],
        val_84: f_at(bytes, 0x84),
    }
}

fn block_from(bytes: &[u8]) -> QuadBlock {
    QuadBlock {
        field_04: f_at(bytes, 0x04),
        jump_scalar: f_at(bytes, 0x0c),
        jump_dir: [f_at(bytes, 0x9c), f_at(bytes, 0xa0), f_at(bytes, 0xa4)],
        jump_extra: [f_at(bytes, 0xd4), f_at(bytes, 0xd8), f_at(bytes, 0xdc)],
        speed_slow: f_at(bytes, 0x118),
        speed_normal: f_at(bytes, 0x11c),
        speed_fast: f_at(bytes, 0x120),
        startup_time: f_at(bytes, 0x128),
        fall_startup: f_at(bytes, 0x12c),
        fall_slow: f_at(bytes, 0x130),
        fall_normal: f_at(bytes, 0x134),
        fall_fast: f_at(bytes, 0x138),
        smooth_time: f_at(bytes, 0x13c),
        lift_count: i32::from_le_bytes(bytes[0x140..0x144].try_into().unwrap()),
        lift_height: f_at(bytes, 0x144),
        lift_valid: f_at(bytes, 0x148),
        lift_duration: f_at(bytes, 0x14c),
        lift_speed: f_at(bytes, 0x150),
        elapsed: f_at(bytes, 0x15c),
        lift_time: f_at(bytes, 0x160),
        smoothed_v: f_at(bytes, 0x168),
        smoothed_dv: f_at(bytes, 0x16c),
        block_101: bytes[0x101],
        fixed_flight: bytes[0x180],
        flag_188: bytes[0x188],
        flag_18a: bytes[0x18a],
        mounted: bytes[0xa8],
        stance_171: bytes[0x171],
        state_1c1: bytes[0x1c1],
        flag_1d0: bytes[0x1d0],
        flag_1d8: bytes[0x1d8],
        flag_1da: bytes[0x1da],
        flag_1d9: bytes[0x1d9],
        fixed_speed: f_at(bytes, 0x1d4),
    }
}

fn quad_actor_from(bytes: &[u8]) -> QuadActor {
    QuadActor {
        quat: q_at(bytes, 0x2d0),
        velocity: [f_at(bytes, 0x2e0), f_at(bytes, 0x2e4), f_at(bytes, 0x2e8)],
        movement_scalar: f_at(bytes, 0x2e8),
        swim_a: [f_at(bytes, 0x96c), f_at(bytes, 0x970)],
        swim_b: [f_at(bytes, 0x974), f_at(bytes, 0x978)],
        val_9dc: f_at(bytes, 0x9dc),
        val_284: f_at(bytes, 0x284),
        stride: f_at(bytes, 0x40),
        falling: bytes[0x161],
        flag_78: bytes[0x78],
        flag_79: bytes[0x79],
        airborne: bytes[0x98],
        jumped: bytes[0x99],
        slide: bytes[0x1d9],
        ground_align_off: bytes[0x1c7],
        swim_flag: bytes[0x111],
        fixed_speed: f_at(bytes, 0x1d4),
        unit_stance: 0,
        flag_161: 0,
        val_5c: 0.0,
        movement_extra: [0.0; 3],
        val_918: 0,
    }
}

#[test]
fn quad_glide_matches_goldens() {
    let quats = [
        [0.0, 0.0, 0.0, 1.0],
        [0.1, 0.2, 0.3, 0.9],
        [0.5, -0.1, 0.2, 0.8],
        [0.0, 0.6, 0.0, 0.8],
        [-0.4, 0.0, 0.0, 0.9],
    ];
    let mut idx = 0usize;
    for case in quad()["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        if !name.starts_with("glide_") || name.starts_with("glide_straight") {
            continue;
        }
        let mut ctl = ctl_from(&hex_bytes(case["before"].as_str().unwrap()));
        let before_block = hex_bytes(case["before_block"].as_str().unwrap());
        let after_block = hex_bytes(case["after_block"].as_str().unwrap());
        let mut block = block_from(&after_block);
        block.elapsed = f_at(&before_block, 0x15c);
        block.lift_time = f_at(&before_block, 0x160);
        block.smoothed_v = f_at(&before_block, 0x168);
        block.smoothed_dv = f_at(&before_block, 0x16c);
        ctl.quat = quats[idx % quats.len()];
        idx += 1;
        let dt = f_at(&after_block, 0x15c) - f_at(&before_block, 0x15c);
        ctl.dt = dt;
        let ext = QuadExternals::default();
        glide(&mut ctl, &mut block, &ext, dt);
        let want = hex_bytes(case["after"].as_str().unwrap());
        assert_eq!(ctl.mode, u32::from_le_bytes(want[0x60..0x64].try_into().unwrap()), "{name} mode");
        for (j, off) in [0x64usize, 0x68, 0x6c].iter().enumerate() {
            assert_near(name, "velocity", ctl.velocity[j], f_at(&want, *off));
        }
        assert_near(name, "elapsed", block.elapsed, f_at(&after_block, 0x15c));
        assert_near(name, "smoothed_v", block.smoothed_v, f_at(&after_block, 0x168));
    }
}

#[test]
fn quad_glide_straight_matches_goldens() {
    let quats = [
        [0.0, 0.0, 0.0, 1.0],
        [0.1, 0.2, 0.3, 0.9],
        [0.5, -0.1, 0.2, 0.8],
        [0.0, 0.6, 0.0, 0.8],
        [-0.4, 0.0, 0.0, 0.9],
    ];
    let mut idx = 0usize;
    for case in quad()["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        if !name.starts_with("glide_straight") {
            continue;
        }
        let mut ctl = ctl_from(&hex_bytes(case["before"].as_str().unwrap()));
        let mut block = QuadBlock::default();
        block.speed_normal = f32_of(&case["speed"]);
        ctl.quat = quats[idx % quats.len()];
        idx += 1;
        glide_straight(&mut ctl, &block);
        let want = hex_bytes(case["after"].as_str().unwrap());
        assert_eq!(ctl.mode, u32::from_le_bytes(want[0x60..0x64].try_into().unwrap()), "{name} mode");
        for (j, off) in [0x64usize, 0x68, 0x6c].iter().enumerate() {
            assert_bits(j, name, ctl.velocity[j], &serde_json::json!({"bits": format!("0x{:08x}", f_at(&want, *off).to_bits())}));
        }
    }
}

#[test]
fn quad_apply_matches_goldens() {
    for case in quad()["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        if !name.starts_with("apply_") {
            continue;
        }
        let ctl_before = hex_bytes(case["before"].as_str().unwrap());
        let actor_before = hex_bytes(case["before_actor"].as_str().unwrap());
        let ctl = ctl_from(&ctl_before);
        let mut actor = quad_actor_from(&[0u8; 0x1000]);
        actor.flag_161 = actor_before[0x161];
        actor.falling = actor_before[0x161];
        actor.val_5c = f_at(&actor_before, 0x5c);
        apply(&ctl, &mut actor);
        let want = hex_bytes(case["after_actor"].as_str().unwrap());
        assert_eq!(actor.flag_161, want[0x161], "{name} flag_161");
        for (j, off) in [0x2e0usize, 0x2e4, 0x2e8].iter().enumerate() {
            assert_bits(j, name, actor.velocity[j], &serde_json::json!({"bits": format!("0x{:08x}", f_at(&want, *off).to_bits())}));
        }
    }
}

#[test]
fn quad_dispatch_matches_goldens() {
    for case in quad()["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        if !name.starts_with("dispatch_") {
            continue;
        }
        let stance = case["stance"].as_u64().unwrap();
        if stance == 0 || case["mounted"].as_u64().unwrap() != 0 {
            continue;
        }
        let mut ctl = ctl_from(&hex_bytes(case["before"].as_str().unwrap()));
        // The captured block is the controller's `+0x08`; reconstruct it from the actor
        // snapshot, which has the block pointer at +0x50.
        let actor_before = hex_bytes(case["before_actor"].as_str().unwrap());
        // The block fields are deterministic in the capture; rebuild them here.
        let mut block = block_from(&{
            let mut b = [0u8; 0x400];
            for (off, val) in [(0x118usize, 8.0f32), (0x11c, 8.0), (0x120, 8.0), (0x128, 5.0),
                               (0x12c, 0.0), (0x15c, 1.0)] {
                b[off..off + 4].copy_from_slice(&val.to_le_bytes());
            }
            b
        });
        block.stance_171 = stance as u8;
        block.block_101 = 1;
        let mut actor = QuadActor::default();
        actor.ground_align_off = 1;
        ctl.quat = q_at(&actor_before, 0x2d0);
        let ext = QuadExternals::default();
        let mut graph = Vec::new();
        let _ = dispatch(
            &mut ctl,
            &mut block,
            &mut actor,
            &ext,
            &QuadConfig::default(),
            0.0,
            &mut graph,
        );
        let want = hex_bytes(case["after"].as_str().unwrap());
        assert_eq!(ctl.mode, u32::from_le_bytes(want[0x60..0x64].try_into().unwrap()), "{name} mode");
        for (j, off) in [0x64usize, 0x68, 0x6c].iter().enumerate() {
            assert_near(name, "velocity", ctl.velocity[j], f_at(&want, *off));
        }
    }
}

fn mounted_goldens() -> Value {
    serde_json::from_str(include_str!("goldens/mounted.json")).expect("mounted JSON")
}

fn f32_json(v: &Value) -> f32 {
    f32_of(v)
}

#[test]
fn quad_mounted_matches_goldens() {
    let doc = mounted_goldens();
    let cfg_of = |v: &Value| QuadConfig {
        speed_scale: f32_json(&v["404"]),
        vertical_scale: f32_json(&v["412"]),
        movement_scale: f32_json(&v["416"]),
        accel_time: f32_json(&v["440"]),
        friction_time: f32_json(&v["444"]),
        swim_scale: 0.0,
        final_scale: 0.0,
        final_cut: 0.0,
    };
    for case in doc["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let before = hex_bytes(case["before"].as_str().unwrap());
        let mut ctl = ctl_from(&before);
        ctl.input = [f32_json(&case["input"][0]), f32_json(&case["input"][1]), 0.0];
        ctl.dt = f32_json(&case["dt"]);
        ctl.flags_5c = case["stride_bit0"].as_u64().unwrap() as u32;
        ctl.quat = [
            f32_json(&case["quat"][0]),
            f32_json(&case["quat"][1]),
            f32_json(&case["quat"][2]),
            f32_json(&case["quat"][3]),
        ];
        ctl.spare_velocity = [
            f32_json(&case["spare"][0]),
            f32_json(&case["spare"][1]),
            f32_json(&case["spare"][2]),
        ];
        let ext = QuadExternals {
            mounted_unit: case["mounted"].as_bool().unwrap(),
            phys_velocity: if case["phys_ok"].as_bool().unwrap() {
                Some([
                    f32_json(&case["phys_velocity"][0]),
                    f32_json(&case["phys_velocity"][1]),
                    f32_json(&case["phys_velocity"][2]),
                ])
            } else {
                None
            },
            move_ray: if case["ray"].is_null() {
                None
            } else {
                Some([
                    f32_json(&case["ray"][0]),
                    f32_json(&case["ray"][1]),
                    f32_json(&case["ray"][2]),
                ])
            },
            ..QuadExternals::default()
        };
        let cfg = cfg_of(&case["config"]);
        mounted(&mut ctl, &ext, &cfg, f32_json(&case["back_scale"]));
        let want = hex_bytes(case["after"].as_str().unwrap());
        assert_eq!(ctl.mode, u32::from_le_bytes(want[0x60..0x64].try_into().unwrap()), "{name} mode");
        for (j, off) in [0x64usize, 0x68, 0x6c, 0x8c, 0x90, 0x94].iter().enumerate() {
            assert_bits(
                j,
                name,
                if j < 3 { ctl.velocity[j] } else { ctl.spare_velocity[j - 3] },
                &serde_json::json!({"bits": format!("0x{:08x}", f_at(&want, *off).to_bits())}),
            );
        }
    }
}

#[test]
fn quad_final_pass_matches_goldens() {
    let doc: Value =
        serde_json::from_str(include_str!("goldens/final_pass.json")).expect("final JSON");
    for case in doc["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let before = hex_bytes(case["before"].as_str().unwrap());
        let mut ctl = ctl_from(&before);
        ctl.dt = f32_of(&case["dt"]);
        ctl.quat = [
            f32_of(&case["move_quat"][0]),
            f32_of(&case["move_quat"][1]),
            f32_of(&case["move_quat"][2]),
            f32_of(&case["move_quat"][3]),
        ];
        ctl.velocity = [
            f32_of(&case["velocity"][0]),
            f32_of(&case["velocity"][1]),
            f32_of(&case["velocity"][2]),
        ];
        let block = QuadBlock {
            field_04: f32_of(&case["block_jump"]),
            mounted: case["block_mounted"].as_bool().unwrap() as u8,
            block_101: case["block_101"].as_bool().unwrap() as u8,
            ..QuadBlock::default()
        };
        let actor = QuadActor {
            ground_align_off: case["ground_align_off"].as_bool().unwrap() as u8,
            ..QuadActor::default()
        };
        let ext = QuadExternals {
            entity_quat: [
                f32_of(&case["entity_quat"][0]),
                f32_of(&case["entity_quat"][1]),
                f32_of(&case["entity_quat"][2]),
                f32_of(&case["entity_quat"][3]),
            ],
            is_swim_stance: case["swim"].as_bool().unwrap(),
            final_scale_flag: case["cfg_flag"].as_u64().unwrap() != 0,
            ..QuadExternals::default()
        };
        let cfg = QuadConfig {
            final_scale: f32_of(&case["cfg_5c"]),
            final_cut: f32_of(&case["cfg_60"]),
            ..QuadConfig::default()
        };
        final_pass(&mut ctl, &block, &actor, &ext, &cfg);
        let want = hex_bytes(case["after"].as_str().unwrap());
        for (i, off) in [0x64usize, 0x68, 0x6c].iter().enumerate() {
            assert_bits(
                i,
                name,
                ctl.velocity[i],
                &serde_json::json!({"bits": format!("0x{:08x}", f_at(&want, *off).to_bits())}),
            );
        }
        for (i, off) in [0x70usize, 0x74, 0x78, 0x7c].iter().enumerate() {
            let got = [
                ctl.movement_extra_70,
                ctl.movement_extra_74,
                ctl.movement_extra_78,
                ctl.movement_extra_7c,
            ][i];
            assert_bits(
                i,
                name,
                got,
                &serde_json::json!({"bits": format!("0x{:08x}", f_at(&want, *off).to_bits())}),
            );
        }
        assert_bits(
            0,
            name,
            ctl.val_84,
            &serde_json::json!({"bits": format!("0x{:08x}", f_at(&want, 0x84).to_bits())}),
        );
    }
}

#[test]
fn atan2_matches_crt_goldens() {
    let doc: Value =
        serde_json::from_str(include_str!("goldens/atan2.json")).expect("atan2 JSON");
    let mut checked = 0usize;
    for case in doc["cases"].as_array().unwrap() {
        let y = f64_bits_of(case["y"].as_str().unwrap());
        let x = f64_bits_of(case["x"].as_str().unwrap());
        let want = hex_bytes(case["out"].as_str().unwrap());
        let want = u64::from_le_bytes(want[..8].try_into().unwrap());
        let got = libm_sse2_atan2(y, x).to_bits();
        assert_eq!(
            got, want,
            "atan2(y={:#018x}, x={:#018x}): got {got:#018x} want {want:#018x}",
            y.to_bits(),
            x.to_bits()
        );
        checked += 1;
    }
    assert!(checked >= 900, "only {checked} atan2 goldens");
}

fn f64_bits_of(hex: &str) -> f64 {
    let bits = u64::from_str_radix(hex.trim_start_matches("0x"), 16).expect("hex bits");
    f64::from_bits(bits)
}

#[test]
fn inverse3x3_matches_goldens() {
    let doc: Value =
        serde_json::from_str(include_str!("goldens/inverse3x3.json")).expect("inverse JSON");
    let mut checked = 0usize;
    for case in doc["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let mut m = [0.0f32; 9];
        for (i, v) in case["in"].as_array().unwrap().iter().enumerate() {
            m[i] = f32_of(v);
        }
        inverse3x3(&mut m);
        for (i, v) in case["out"].as_array().unwrap().iter().enumerate() {
            assert_bits(i, name, m[i], v);
        }
        checked += 1;
    }
    assert!(checked >= 20, "only {checked} inverse goldens");
}

#[test]
fn quad_ground_matches_goldens() {
    let doc: Value =
        serde_json::from_str(include_str!("goldens/ground.json")).expect("ground JSON");
    for case in doc["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let before = hex_bytes(case["before"].as_str().unwrap());
        let after = hex_bytes(case["after"].as_str().unwrap());
        let mut ctl = ctl_from(&before);
        ctl.input = [
            f32_of(&case["input"][0]),
            f32_of(&case["input"][1]),
            f32_of(&case["input"][2]),
        ];
        ctl.jump_button = case["jump_button"].as_u64().unwrap() as u8;
        ctl.airborne = case["airborne"].as_u64().unwrap() as u8;
        ctl.jumped = 0;
        ctl.quat = [
            f32_of(&case["quat"][0]),
            f32_of(&case["quat"][1]),
            f32_of(&case["quat"][2]),
            f32_of(&case["quat"][3]),
        ];
        let block = QuadBlock {
            field_04: f32_of(&case["field_04"]),
            jump_scalar: f32_of(&case["jump_scalar"]),
            jump_dir: [
                f32_of(&case["jump_dir"][0]),
                f32_of(&case["jump_dir"][1]),
                f32_of(&case["jump_dir"][2]),
            ],
            jump_extra: [
                f32_of(&case["jump_extra"][0]),
                f32_of(&case["jump_extra"][1]),
                f32_of(&case["jump_extra"][2]),
            ],
            ..QuadBlock::default()
        };
        let ext = QuadExternals {
            entity_present: true,
            controlled_unit: case["unit_present"].as_bool().unwrap(),
            mounted_unit: case["mounted_stance"].as_u64().unwrap() != 0,
            unit_gate: case["unit_gate"].as_u64().unwrap() != 0,
            unit_kind: case["unit_kind"].as_u64().unwrap() as i32,
            jump_height: f32_of(&case["jump_height"]),
            walk: case["walk"].as_u64().unwrap() != 0,
            params: movement_params(&case["params"]),
            speed_multiplier: f32_of(&case["speed_multiplier"]),
            swimming: case["swim"].as_u64().unwrap() != 0,
            swim_gate: case["game_96"].as_u64().unwrap() == 0,
            anim_present: !case["anim"].is_null(),
            jump_in_place: case["anim"]
                .as_str()
                .map(|s| s.contains("jump_in_place"))
                .unwrap_or(false),
            graph_push: false,
            cfg_swim_scale: f32_of(&case["swim_scale"]),
            ..QuadExternals::default()
        };
        let mut graph = Vec::new();
        let out = ground(&mut ctl, &block, &QuadActor::default(), &ext, &mut graph);
        assert_eq!(
            ctl.mode,
            u32::from_le_bytes(after[0x60..0x64].try_into().unwrap()),
            "{name} mode"
        );
        for (i, off) in [0x64usize, 0x68, 0x6c, 0x8c, 0x90, 0x94].iter().enumerate() {
            let got = if i < 3 {
                ctl.velocity[i]
            } else {
                ctl.spare_velocity[i - 3]
            };
            assert_eq!(
                got.to_bits(),
                f_at(&after, *off).to_bits(),
                "{name} velocity[{i}]: got {got:?}, want {:?}",
                f_at(&after, *off)
            );
        }
        assert_eq!(ctl.airborne, after[0x98], "{name} airborne");
        assert_eq!(ctl.jumped, after[0x99], "{name} jumped");
        let ev = case["unit_event"].as_u64().unwrap() as u32;
        let want_ev = if ev == 0xFFFF_FFFF {
            None
        } else {
            Some(ev != 0)
        };
        assert_eq!(out.unit_event, want_ev, "{name} unit_event");
        assert!(graph.is_empty(), "{name} graph");
    }
}

#[test]
fn wrapper_frame_movement_matches_goldens() {
    let doc: Value =
        serde_json::from_str(include_str!("goldens/wrappers.json")).expect("wrapper JSON");
    for (i, case) in doc["cases"].as_array().unwrap().iter().enumerate() {
        let eff = &case["effective"];
        let bits = |v: &Value| -> u32 { (v.as_f64().expect("number") as f32).to_bits() };
        let one = bits(&eff["one_80"]);
        let sync = FrameSync {
            velocity: [
                bits(&eff["velocity"][0]),
                bits(&eff["velocity"][1]),
                bits(&eff["velocity"][2]),
            ],
            q70: [
                bits(&eff["q70"][0]),
                bits(&eff["q70"][1]),
                bits(&eff["q70"][2]),
                bits(&eff["q70"][3]),
            ],
            one_80: one,
            flag_84: eff["flag_84"].as_u64().unwrap() as u8,
            word_88: bits(&eff["word_88"]),
            flag_8c: eff["flag_8c"].as_u64().unwrap() as u8,
        };
        let (_, actor) = process_frame_movement(&MoverInput::default(), &sync);
        let out = hex_bytes(case["out"].as_str().unwrap());
        let mut got = Vec::new();
        for v in actor.field_308 {
            got.extend_from_slice(&v.to_le_bytes());
        }
        for v in actor.field_314 {
            got.extend_from_slice(&v.to_le_bytes());
        }
        got.push(actor.field_328);
        got.extend_from_slice(&actor.field_32c.to_le_bytes());
        got.push(actor.field_330);
        let mut want = Vec::new();
        want.extend_from_slice(&out[8..0x14]);
        want.extend_from_slice(&out[0x14..0x28]);
        want.extend_from_slice(&out[0x28..0x29]);
        want.extend_from_slice(&out[0x2c..0x30]);
        want.extend_from_slice(&out[0x30..0x31]);
        assert_eq!(got, want, "wrapper movement case {i}");
    }
}

#[test]
fn wrapper_frame_rotation_matches_goldens() {
    let doc: Value =
        serde_json::from_str(include_str!("goldens/wrappers.json")).expect("wrapper JSON");
    for (i, case) in doc["rotation_cases"].as_array().unwrap().iter().enumerate() {
        let q = |v: &Value| -> [f32; 4] {
            [
                f32_of(&v[0]),
                f32_of(&v[1]),
                f32_of(&v[2]),
                f32_of(&v[3]),
            ]
        };
        let vec3 = |v: &Value| -> [f32; 3] { [f32_of(&v[0]), f32_of(&v[1]), f32_of(&v[2])] };
        let actor = RotationActor {
            quats: [q(&case["q20"]), q(&case["q30"]), q(&case["q40"]), [0.0; 4]],
            support: [q(&case["q60"]), q(&case["q70"]), [0.0; 4], [0.0; 4]],
            swim: vec3(&case["swim"]),
            view_limit: vec3(&case["view"]),
            stride: 1.0,
            desired: [0.0; 3],
            anim_4: 0.0,
            anim_a8: 0,
            actor_171: 0,
            actor_18b: 1,
            ground_align: false,
            on_ladder: false,
            stance: 0,
            ground_factor: 0.0,
            ground_cvar: [0.0; 2],
        };
        let (_, bound, path) = process_frame_rotation(
            &actor,
            vec3(&case["request_move"]),
            f32_of(&case["scale"]),
        );
        assert_eq!(path, crate::port::movement::RotationPath::Ladder);
        let after = hex_bytes(case["after"].as_str().unwrap());
        let check = |off: usize, want: [f32; 4]| {
            for j in 0..4 {
                assert_eq!(
                    want[j].to_bits(),
                    f_at(&after, off + j * 4).to_bits(),
                    "rotation wrapper {i} +{off:#x}[{j}]"
                );
            }
        };
        check(0x2B0, bound.move_quats[0]);
        check(0x2C0, bound.move_quats[1]);
        check(0x2D0, bound.move_quats[2]);
        check(0x33C, bound.support_quats[0]);
        check(0x34C, bound.support_quats[1]);
        check(0x950, bound.out_pos);
        for j in 0..3 {
            assert_eq!(
                bound.swim[j].to_bits(),
                f_at(&after, 0x960 + j * 4).to_bits(),
                "rotation wrapper {i} swim[{j}]"
            );
            assert_eq!(
                bound.view_limit[j].to_bits(),
                f_at(&after, 0x154 + j * 4).to_bits(),
                "rotation wrapper {i} view[{j}]"
            );
        }
    }
}
