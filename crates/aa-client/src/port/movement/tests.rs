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
        let mut forward = bool_of(&case, "forward");
        let mut back = bool_of(&case, "back");
        let mut left = bool_of(&case, "left");
        let mut right = bool_of(&case, "right");
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
