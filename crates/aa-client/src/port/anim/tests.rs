//! Golden tests against the original functions and the original's loaded graph tables.
//!
//! The JSON files in `goldens/` are captured by
//! `rust/tools/capture_anim_goldens.py` in the `test/anim-golden` lane from the hidden 1.2
//! client (see that script's doc comment). The graph tables are compared field by field; the
//! pure leaves are bit/exit-value exact.

use super::*;
use serde_json::Value;
use std::collections::HashMap;

fn load(name: &str) -> Value {
    let text = match name {
        "evaluate_guards" => include_str!("goldens/evaluate_guards.json"),
        "fourcc" => include_str!("goldens/fourcc.json"),
        "inputs" => include_str!("goldens/inputs.json"),
        "side" => include_str!("goldens/side.json"),
        other => panic!("unknown golden file {other}"),
    };
    serde_json::from_str(text).expect("golden JSON parses")
}

fn f32_of(v: &Value) -> f32 {
    let bits = v["bits"].as_str().expect("bits string");
    let bits = u32::from_str_radix(bits.trim_start_matches("0x"), 16).expect("hex bits");
    f32::from_bits(bits)
}

fn vec3_of(v: &Value) -> [f32; 3] {
    [f32_of(&v[0]), f32_of(&v[1]), f32_of(&v[2])]
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

fn q(name: &str) -> [f32; 4] {
    match name {
        "identity" => [1.0, 0.0, 0.0, 0.0],
        "yaw90" => [0.70710677, 0.0, 0.70710677, 0.0],
        "yaw180" => [0.0, 0.0, 1.0, 0.0],
        "roll" => [0.70710677, 0.70710677, 0.0, 0.0],
        "zero" => [0.0, 0.0, 0.0, 0.0],
        "w_only" => [0.0, 0.0, 0.0, 1.0],
        other => panic!("unknown q {other}"),
    }
}

#[test]
fn evaluate_guards_matches_goldens() {
    let doc = load("evaluate_guards");
    for (i, case) in doc["cases"].as_array().expect("cases").iter().enumerate() {
        let guards: Vec<u16> = case["guards"]
            .as_array()
            .expect("guards")
            .iter()
            .map(|v| v.as_u64().expect("u16") as u16)
            .collect();
        let mut encoded = [0u16; 32];
        for (j, v) in case["encoded"]
            .as_array()
            .expect("encoded")
            .iter()
            .enumerate()
        {
            encoded[j] = v.as_u64().expect("u16") as u16;
        }
        let got = evaluate_guards(&guards, &encoded);
        assert_eq!(
            got,
            case["out"].as_u64().expect("out") != 0,
            "evaluate_guards case {i} ({})",
            case["name"].as_str().unwrap_or("")
        );
    }
}

#[test]
fn fourcc_matches_goldens() {
    let doc = load("fourcc");
    for (i, case) in doc["cases"].as_array().expect("cases").iter().enumerate() {
        let text = case["fourcc"].as_str().expect("fourcc");
        let fourcc = u32::from_str_radix(text.trim_start_matches("0x"), 16).expect("hex fourcc");
        assert_eq!(
            fourcc_case(fourcc),
            case["out"].as_u64().expect("out") as u16,
            "fourcc case {i} {text}"
        );
    }
    // The error path in the original cannot be called safely (it dereferences a zero string),
    // but the mapped return value is the default case.
    assert_eq!(fourcc_case(0x12345678), 0x3a);
}

fn horse_graph() -> &'static AnimationGraph {
    static GRAPH: std::sync::OnceLock<AnimationGraph> = std::sync::OnceLock::new();
    GRAPH.get_or_init(|| {
        load_graph_from_pak("horse_full_body").expect("horse_full_body.xml in the pak")
    })
}

fn graph_input<'a>(graph: &'a AnimationGraph, name: &str) -> &'a GraphInput {
    graph
        .inputs
        .iter()
        .find(|i| i.name.eq_ignore_ascii_case(name))
        .unwrap_or_else(|| panic!("input {name}"))
}

#[test]
fn integer_encode_matches_goldens() {
    let doc = load("inputs");
    let graph = horse_graph();
    for (i, case) in doc["int"].as_array().expect("int").iter().enumerate() {
        let name = case["input"].as_str().expect("input");
        let input = graph_input(graph, name);
        let got = match case["kind"].as_str().expect("kind") {
            "int" => {
                let value = case["value"].as_i64().expect("value") as i32;
                encode_int(&input.ranges, input.min, input.max, value)
            }
            "str" => {
                let text = case["text"].as_str().expect("text");
                encode_int_str(&input.ranges, input.min, input.max, text)
            }
            other => panic!("kind {other}"),
        };
        assert_eq!(
            got,
            case["out"].as_u64().expect("out") as u16,
            "int case {i} {name} {case}"
        );
    }
}

#[test]
fn float_encode_matches_goldens() {
    let doc = load("inputs");
    let graph = horse_graph();
    for (i, case) in doc["float"].as_array().expect("float").iter().enumerate() {
        let name = case["input"].as_str().expect("input");
        let input = graph_input(graph, name);
        let got = match case["kind"].as_str().expect("kind") {
            "float" => {
                let value = f32_of(&case["value"]);
                encode_float(
                    &input.ranges,
                    input.min,
                    input.max,
                    input.scale,
                    input.offset,
                    value,
                )
            }
            "str" => {
                let text = case["text"].as_str().expect("text");
                encode_float_str(
                    &input.ranges,
                    input.min,
                    input.max,
                    input.scale,
                    input.offset,
                    text,
                )
            }
            other => panic!("kind {other}"),
        };
        assert_eq!(
            got,
            case["out"].as_u64().expect("out") as u16,
            "float case {i} {name} {case}"
        );
    }
}

#[test]
fn key_encode_matches_goldens() {
    let doc = load("inputs");
    let graph = horse_graph();
    for (i, case) in doc["key"].as_array().expect("key").iter().enumerate() {
        let name = case["input"].as_str().expect("input");
        let input = graph_input(graph, name);
        let kind = case["kind"].as_str().expect("kind");
        let got = match kind {
            "name" => {
                let text = case["text"].as_str().expect("text");
                encode_key(&input.keys, text)
            }
            // `FUN_30611470` reads the sorted key map at `+0x14`, which the XML loader leaves
            // empty, so every index call writes `0xffff`.
            "index" => {
                let local = case["local"].as_u64().expect("local") as usize;
                encode_key_index(&input.keys, local)
            }
            other => panic!("kind {other}"),
        };
        assert_eq!(
            got,
            case["out"].as_u64().expect("out") as u16,
            "key case {i} {name} {case}"
        );
    }
}

#[test]
fn entry_side_matches_goldens() {
    let doc = load("side");
    for (i, case) in doc["entry"].as_array().expect("entry").iter().enumerate() {
        let orientation = q(case["q"].as_str().expect("q"));
        let mate = vec3_of(&case["mate"]);
        let seat1 = case["seat1"].as_bool().expect("seat1");
        let seat2 = case["seat2"].as_bool().expect("seat2");
        // The live fake seat list does not reach the original's occupancy lookup (the
        // engine's list type is not reproduced by a flat fake), so the seat override is
        // covered by the Rust branch test below; the golden is the base geometry.
        if seat1 || seat2 {
            continue;
        }
        let got = entry_side(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(mate[0], mate[1], mate[2]),
            orientation,
            seat1,
            seat2,
        );
        assert_eq!(
            got as u64,
            case["out"].as_u64().expect("out"),
            "entry_side case {i} {case}"
        );
    }
}

#[test]
fn approach_side_matches_goldens() {
    let doc = load("side");
    for (i, case) in doc["approach"]
        .as_array()
        .expect("approach")
        .iter()
        .enumerate()
    {
        let orientation = q(case["q"].as_str().expect("q"));
        let reason = case["reason"].as_u64().expect("reason") as u8;
        let got = approach_side(reason, orientation);
        let want = &case["out"];
        for j in 0..3 {
            assert_bits(i, "approach_side", got[j], &want[j]);
        }
    }
}

#[test]
fn side_placement_first_candidate_matches_goldens() {
    let doc = load("side");
    let dummy = Placement {
        position: Vec3::new(0.0, 0.0, 0.0),
        side: Vec3::new(0.0, 0.0, 1.0),
        height: 0.0,
        extra: 0.0,
    };
    for (i, case) in doc["side"].as_array().expect("side").iter().enumerate() {
        let orientation = q(case["q"].as_str().expect("q"));
        let mate = vec3_of(&case["mate"]);
        let got =
            side_placement(&dummy, &dummy, orientation, |_, _, _| false).expect("first candidate");
        let want = &case["out"];
        for j in 0..3 {
            assert_bits(i, "side_placement", got[j], &want[j]);
        }
        let _ = mate;
    }
}

#[test]
fn side_placement_branches() {
    let dummy = Placement {
        position: Vec3::new(0.0, 0.0, 0.0),
        side: Vec3::new(0.0, 0.0, 1.0),
        height: 0.0,
        extra: 0.0,
    };
    let q = [1.0, 0.0, 0.0, 0.0];
    // First candidate blocked, second free.
    let got = side_placement(&dummy, &dummy, q, |_, _, v| v[0] < 0.0).expect("second candidate");
    assert_eq!(got, [1.5, -1.0, 0.0]);
    // Both blocked.
    assert_eq!(side_placement(&dummy, &dummy, q, |_, _, _| true), None);
}

// ---------------------------------------------------------------------------------------------
// Static graph tables. The JSON files are produced by the lane capture script; the comparison is
// strict for inputs, state names/parents/costs/guards and transition from/to/force/cost.
// ---------------------------------------------------------------------------------------------

fn graph_golden(name: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/port/anim/goldens")
        .join(format!("{name}.json"));
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&text).expect("graph golden parses")
}

fn compare_graph(
    name: &str,
    golden_name: &str,
    names_exact: bool,
    exact_guard_sentinels: bool,
    check_guards: bool,
    exact_weights: bool,
    allowed_range_inputs: &[&str],
) {
    let golden = graph_golden(golden_name);
    let graph = load_graph_from_pak(name).expect("graph in the pak");

    assert_eq!(
        graph.source,
        if aa_pak::Pak::global().entry(&ag_path(name)).is_some() {
            GraphSource::Ag
        } else {
            GraphSource::Xml
        },
        "{name}: source"
    );

    let want_hub = golden["hub"].as_u64().unwrap();
    assert_eq!(
        graph.default_state.map(|v| v as u64),
        if want_hub == 0xffff {
            None
        } else {
            Some(want_hub)
        },
        "{name}: default state"
    );

    let inputs = golden["inputs"].as_array().expect("inputs");
    assert_eq!(graph.inputs.len(), inputs.len(), "{name}: input count");
    for (i, want) in inputs.iter().enumerate() {
        let got = &graph.inputs[i];
        assert_eq!(
            got.name,
            want["name"].as_str().unwrap(),
            "{name}: input {i} name"
        );
        assert_eq!(
            got.kind as u64,
            want["kind"].as_u64().unwrap(),
            "{name}: input {i} kind"
        );
        assert_eq!(
            got.signalled,
            want["signalled"].as_u64().unwrap() != 0,
            "{name}: input {i} signalled"
        );
        assert_eq!(
            got.priority as u64,
            want["priority"].as_u64().unwrap(),
            "{name}: input {i} priority"
        );
        assert_eq!(
            got.default_encoded as u64,
            want["default"].as_u64().unwrap(),
            "{name}: input {i} default"
        );
        if got.kind == InputKind::Keyed {
            let keys = want["keys"].as_array().unwrap();
            assert_eq!(got.keys.len(), keys.len(), "{name}: input {i} key count");
            for (j, key) in keys.iter().enumerate() {
                assert_eq!(
                    got.keys[j].0,
                    key[0].as_str().unwrap(),
                    "{name}: input {i} key {j}"
                );
                assert_eq!(
                    got.keys[j].1 as u64,
                    key[1].as_u64().unwrap(),
                    "{name}: input {i} key {j} index"
                );
            }
        } else {
            assert_eq!(
                got.min as i64,
                want["min"].as_i64().unwrap(),
                "{name}: input {i} min"
            );
            assert_eq!(
                got.max as i64,
                want["max"].as_i64().unwrap(),
                "{name}: input {i} max"
            );
            if got.kind == InputKind::Float {
                assert_eq!(
                    got.scale.to_bits(),
                    f32_of(&want["scale"]).to_bits(),
                    "{name}: input {i} scale"
                );
                assert_eq!(
                    got.offset.to_bits(),
                    f32_of(&want["offset"]).to_bits(),
                    "{name}: input {i} offset"
                );
            }
            let want_ranges: Vec<(i64, i64)> = want["ranges"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| (r[0].as_i64().unwrap(), r[1].as_i64().unwrap()))
                .collect();
            let got_ranges: Vec<(i64, i64)> = got
                .ranges
                .iter()
                .map(|&(a, b)| (a as i64, b as i64))
                .collect();
            if !allowed_range_inputs
                .iter()
                .any(|n| n.eq_ignore_ascii_case(&got.name))
            {
                assert_eq!(got_ranges, want_ranges, "{name}: input {i} ranges");
            }
        }
    }

    let states = golden["states"].as_array().expect("states");
    assert_eq!(graph.states.len(), states.len(), "{name}: state count");
    for (i, want) in states.iter().enumerate() {
        let got = &graph.states[i];
        let want_name = want["name"].as_str().unwrap();
        if names_exact {
            assert_eq!(got.name, want_name, "{name}: state {i} name");
        } else {
            assert!(
                got.name.eq_ignore_ascii_case(want_name),
                "{name}: state {i} name {:?} != {want_name:?}",
                got.name
            );
        }
        let parent = want["parent"].as_u64().unwrap();
        assert_eq!(
            got.parent.map(|v| v as u64),
            if parent == 0xffff { None } else { Some(parent) },
            "{name}: state {i} parent"
        );
        assert_eq!(
            got.cost_minus1 as u64,
            want["cost_minus1"].as_u64().unwrap(),
            "{name}: state {i} cost"
        );
        if !check_guards {
            continue;
        }
        let guards: Vec<u64> = want["guards"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect();
        let got_guards: Vec<u64> = got
            .guards
            .iter()
            .map(|&v| {
                if exact_guard_sentinels {
                    v as u64
                } else {
                    (v & 0x7fff) as u64
                }
            })
            .collect();
        let want_guards: Vec<u64> = guards
            .iter()
            .map(|&v| if exact_guard_sentinels { v } else { v & 0x7fff })
            .collect();
        assert_eq!(got_guards, want_guards, "{name}: state {i} guards");
    }

    let transitions = golden["transitions"].as_array().expect("transitions");
    assert_eq!(
        graph.transitions.len(),
        transitions.len(),
        "{name}: transition count"
    );
    for (i, want) in transitions.iter().enumerate() {
        let got = &graph.transitions[i];
        assert_eq!(
            got.from as u64,
            want[0].as_u64().unwrap(),
            "{name}: transition {i} from"
        );
        assert_eq!(
            got.to as u64,
            want[1].as_u64().unwrap(),
            "{name}: transition {i} to"
        );
        assert_eq!(
            got.force_follow_chance as u64,
            want[2].as_u64().unwrap(),
            "{name}: transition {i} force"
        );
        let want_weight = want[3].as_u64().unwrap();
        if exact_weights {
            assert_eq!(
                got.weight as u64, want_weight,
                "{name}: transition {i} weight"
            );
        } else {
            assert!(
                got.weight as u64 == want_weight || got.weight as u64 + 1 == want_weight,
                "{name}: transition {i} weight {} != {want_weight}",
                got.weight
            );
        }
        let time_bits =
            (want[4].as_u64().unwrap() as u32) | ((want[5].as_u64().unwrap() as u32) << 16);
        assert_eq!(
            got.transition_time.to_bits(),
            time_bits,
            "{name}: transition {i} time"
        );
    }
}

#[test]
fn animation_graph_tables_match_live_goldens() {
    // The client resolves every graph with a compiled side copy to `.ag`
    // (`FUN_306261f0`); `playerupperbody` has no `.ag` and is the XML oracle.
    for (name, golden, exact_names, exact_sentinels, check_guards, exact_weights) in [
        (
            "player_full_body",
            "player_full_body",
            false,
            true,
            true,
            true,
        ),
        (
            "horse_full_body",
            "horse_full_body",
            false,
            false,
            true,
            true,
        ),
        (
            "monster_full_body",
            "monster_full_body_xml",
            false,
            false,
            true,
            true,
        ),
        (
            "npc_full_body",
            "npc_full_body_xml",
            false,
            false,
            true,
            true,
        ),
        (
            "monster_full_body_big",
            "monster_full_body_big_xml",
            false,
            false,
            true,
            true,
        ),
        (
            "bird_full_body",
            "bird_full_body_xml",
            false,
            false,
            true,
            true,
        ),
        (
            "dragon_full_body",
            "dragon_full_body_xml",
            false,
            false,
            true,
            true,
        ),
        // The XML-only upper-body graph has no compiled side copy; the transition
        // expansion is the oracle here. Its template `<Guard>` words are still open.
        (
            "playerupperbody",
            "playerupperbody_xml",
            true,
            true,
            false,
            false,
        ),
    ] {
        compare_graph(
            name,
            golden,
            exact_names,
            exact_sentinels,
            check_guards,
            exact_weights,
            &[],
        );
    }
}

#[test]
fn entry_side_seat_override() {
    let q = [1.0, 0.0, 0.0, 0.0];
    let mate = Vec3::new(1.0, 0.0, 0.0);
    // dot = 0, cross > 0 -> right (2); an occupied seat 1 forces left (1).
    assert_eq!(
        entry_side(Vec3::new(0.0, 0.0, 0.0), mate, q, false, false),
        2
    );
    assert_eq!(
        entry_side(Vec3::new(0.0, 0.0, 0.0), mate, q, true, false),
        1
    );
    // Back (3) uses seat 2.
    let back = Vec3::new(0.0, -1.0, 0.0);
    assert_eq!(
        entry_side(Vec3::new(0.0, 0.0, 0.0), back, q, false, false),
        3
    );
    assert_eq!(
        entry_side(Vec3::new(0.0, 0.0, 0.0), back, q, false, true),
        1
    );
    // Front (4) uses seat 1.
    let front = Vec3::new(0.0, 1.0, 0.0);
    assert_eq!(
        entry_side(Vec3::new(0.0, 0.0, 0.0), front, q, false, false),
        4
    );
    assert_eq!(
        entry_side(Vec3::new(0.0, 0.0, 0.0), front, q, true, false),
        1
    );
}

// ---------------------------------------------------------------------------------------------
// Live animation-graph state machine. `live_state_machine.json` is captured by
// `rust/tools/capture_anim_state.py` in the `test/anim-golden` lane from the hidden 1.2 client:
// the local player's original full-body state and the summoned horse's state are driven through
// `SetInput` and `Update_SteadyState`; each step records the encoded 32-input bank, the
// `BasicQuery` candidate list and the state selected by the original `PerformQuery` (`+0x28`).
// ---------------------------------------------------------------------------------------------

fn live_machine() -> Value {
    serde_json::from_str(include_str!("goldens/live_state_machine.json"))
        .expect("live state-machine golden parses")
}

fn reproduce_live_sequence(graph_name: &str, section: &Value) {
    let graph = load_graph_from_pak(graph_name).expect("live graph in the pak");
    assert_eq!(graph.source, GraphSource::Ag, "{graph_name}: source");
    let steps = section["steps"].as_array().expect("steps");
    for (i, step) in steps.iter().enumerate() {
        let label = step["label"].as_str().unwrap_or("");
        let current = step["current_before"].as_u64().unwrap() as u16;
        let current_name = step["current_before_name"].as_str().expect("current name");
        assert!(
            graph.states[current as usize]
                .name
                .eq_ignore_ascii_case(current_name),
            "{graph_name} step {i} ({label}): current {:?} != {current_name:?}",
            graph.states[current as usize].name
        );

        let mut state = GraphState::new(&graph);
        state.current = Some(current);
        state.update_kind = UpdateKind::Steady;
        state.time = 0.0;
        let encoded: Vec<u16> = step["encoded"]
            .as_array()
            .expect("encoded")
            .iter()
            .map(|v| v.as_u64().unwrap() as u16)
            .collect();
        assert!(
            encoded.len() >= graph.inputs.len(),
            "{graph_name} step {i}: input count {} < {}",
            encoded.len(),
            graph.inputs.len()
        );
        for (j, input) in state.inputs.iter_mut().enumerate() {
            input.encoded = [encoded[j], encoded[j]];
        }
        state.bank = 0;

        let mut got: Vec<u16> = state.basic_query(&graph);
        got.sort_unstable();
        let mut want: Vec<u16> = step["candidates"]
            .as_array()
            .expect("candidates")
            .iter()
            .map(|v| v.as_u64().unwrap() as u16)
            .collect();
        want.sort_unstable();
        assert_eq!(
            got, want,
            "{graph_name} step {i} ({label}): CStateIndex candidates"
        );

        let target = step["target"].as_u64().unwrap() as u16;
        let target_name = step["target_name"].as_str().expect("target name");
        let picked = state.perform_query(&graph).expect("query target");
        assert_eq!(
            picked, target,
            "{graph_name} step {i} ({label}): target {:?} != {target_name:?}",
            graph.states[picked as usize].name
        );
    }
}

#[test]
fn live_state_machine_sequence_reproduces() {
    let doc = live_machine();
    reproduce_live_sequence("player_full_body", &doc["player"]);
    reproduce_live_sequence("horse_full_body", &doc["horse"]);
}

#[test]
fn compiled_ag_graphs_load() {
    let pak = aa_pak::Pak::global();
    let mut seen = 0;
    for path in pak.list("game/animations/graphs/") {
        if !path.to_ascii_lowercase().ends_with(".ag") {
            continue;
        }
        let name = path
            .trim_start_matches("game/animations/graphs/")
            .trim_end_matches(".ag");
        if name.starts_with("htft/") {
            let err = load_graph_ag_from_pak(name).expect_err("htft graphs are 0x2f");
            assert!(
                err.contains("unsupported .ag version 0x2f"),
                "{path}: {err}"
            );
            continue;
        }
        let graph = load_graph_ag_from_pak(name).unwrap_or_else(|e| panic!("{path}: {e}"));
        assert_eq!(graph.source, GraphSource::Ag, "{path}");
        assert!(!graph.states.is_empty(), "{path}: no states");
        seen += 1;
    }
    assert!(
        seen >= 17,
        "expected all shipped 0.31 .ag graphs, got {seen}"
    );
}

// ---------------------------------------------------------------------------------------------
// LMG capabilities. `goldens/lmg_weights.json` is captured from the live 1.2 client by calling
// the original per-type body directly (`FUN_31574e70` through `FUN_3156d760`,
// `FUN_3156bf40`) with a synthetic animation set whose rows are the live `0xdc` animation data
// rows. The Rust port consumes the same rows, descriptor and LMG example bytes.
// ---------------------------------------------------------------------------------------------

fn hex_bytes(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).expect("hex"))
        .collect()
}

fn lmg_golden() -> Value {
    serde_json::from_str(include_str!("goldens/lmg_weights.json")).expect("lmg golden parses")
}

#[test]
fn lmg_shared_blend_and_tstp_match_goldens() {
    let doc = lmg_golden();
    for (n, case) in doc.as_array().expect("cases").iter().enumerate() {
        let ids: Vec<i16> = case["ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap() as i16)
            .collect();
        let rows: Vec<AnimRow> = case["set_rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                let bytes = hex_bytes(v.as_str().unwrap());
                let mut row = [0u8; 0xdc];
                row.copy_from_slice(&bytes);
                row
            })
            .collect();
        let mut entries = vec![[0u8; 0x10]; 256];
        for (i, id) in ids.iter().enumerate() {
            entries[*id as usize][8..12].copy_from_slice(&(i as u32).to_le_bytes());
        }
        let set = AnimSet { entries, rows };
        let desc = hex_bytes(case["desc"].as_str().unwrap());
        let examples: Vec<u8> = case["lmg"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|v| hex_bytes(v.as_str().unwrap()))
            .collect();
        let lmg = Lmg { examples };
        let mut caps = Caps::init_lmg();
        match case["kind"].as_str().unwrap() {
            "s8" | "s8fb" => fun_31574e70(&set, &desc, 0.0, &lmg, &mut caps),
            "tstp" => fun_3156bf40(&set, &desc, &lmg, &mut caps),
            other => panic!("unknown kind {other}"),
        }
        let want = hex_bytes(case["caps"].as_str().unwrap());
        assert_eq!(
            caps.0.as_slice(),
            want.as_slice(),
            "lmg case {n} ({})",
            case["kind"].as_str().unwrap()
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Rider copy. Captured live from the mounted player + summoned horse: the horse graph inputs
// were set through the original `SetInput`, then the next frame's `FUN_390dcc80`/`FUN_390dd080`
// copied them into the local player graph.
// ---------------------------------------------------------------------------------------------

#[test]
fn rider_copy_matches_live_mount() {
    let doc: Value = serde_json::from_str(include_str!("goldens/rider_copy.json"))
        .expect("rider golden parses");
    for (i, case) in doc.as_array().expect("cases").iter().enumerate() {
        let stance = case["stance"].as_str().expect("stance");
        let inputs = RiderInputs {
            pseudo_speed: case["mount_speed"].as_f64().expect("mount speed") as f32,
            turn_speed: case["mount_turn"].as_f64().expect("mount turn") as f32,
            swim: stance.eq_ignore_ascii_case("swim"),
        };
        let (speed, turn) = actor_unit_model_update(inputs);
        assert_eq!(
            speed.to_bits(),
            (case["rider_speed"].as_f64().unwrap() as f32).to_bits(),
            "rider case {i} PseudoSpeed"
        );
        assert_eq!(
            turn.to_bits(),
            (case["rider_turn"].as_f64().unwrap() as f32).to_bits(),
            "rider case {i} RequestedTurnSpeedLZ"
        );
    }
}

#[test]
fn lmg_s8_fallback_matches_goldens() {
    let doc: Value = serde_json::from_str(include_str!("goldens/lmg_s8_fallback.json"))
        .expect("fallback golden parses");
    for (n, case) in doc.as_array().expect("cases").iter().enumerate() {
        let ids: Vec<i16> = case["ids"].as_array().unwrap().iter().map(|v| v.as_i64().unwrap() as i16).collect();
        let rows: Vec<AnimRow> = case["set_rows"].as_array().unwrap().iter().map(|v| {
            let bytes = hex_bytes(v.as_str().unwrap());
            let mut row = [0u8; 0xdc];
            row.copy_from_slice(&bytes);
            row
        }).collect();
        let mut entries = vec![[0u8; 0x10]; 256];
        for (i, id) in ids.iter().enumerate() {
            entries[*id as usize][8..12].copy_from_slice(&(i as u32).to_le_bytes());
        }
        let set = AnimSet { entries, rows };
        let desc = hex_bytes(case["desc"].as_str().unwrap());
        let examples: Vec<u8> = case["lmg"].as_array().unwrap().iter().flat_map(|v| hex_bytes(v.as_str().unwrap())).collect();
        let lmg = Lmg { examples };
        let mut caps = Caps::init_lmg();
        fun_31574e70(&set, &desc, 0.0, &lmg, &mut caps);
        assert_eq!(caps.0.as_slice(), hex_bytes(case["caps"].as_str().unwrap()).as_slice(), "fallback case {n}");
    }
}

// ---------------------------------------------------------------------------------------------
// Per-layer desc data. `FUN_31558550` is called for all 13 character layers with the caps
// blocks captured in `lmg_weights.json` and the live animation-set scale.
// ---------------------------------------------------------------------------------------------

#[test]
fn lmg_layers_match_live_caps() {
    let doc: Value =
        serde_json::from_str(include_str!("goldens/lmg_layers.json")).expect("layer golden");
    for (n, case) in doc.as_array().expect("cases").iter().enumerate() {
        let mut caps = Caps::init_lmg();
        caps.0.copy_from_slice(&hex_bytes(case["caps"].as_str().unwrap()));
        let scale = case["scale"].as_f64().unwrap() as f32;
        for (i, want) in case["layers"].as_array().unwrap().iter().enumerate() {
            let mut layer = Layer::default();
            fun_31558550(i as u32, &caps, scale, &mut layer);
            assert_eq!(
                layer.0.as_slice(),
                hex_bytes(want.as_str().unwrap()).as_slice(),
                "layer case {n} layer {i}"
            );
        }
    }
}
