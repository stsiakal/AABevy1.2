//! Static animation-graph loader (`CAnimationGraph::Init`, `cryaction+0x3061f210`, and the
//! `LoadInputs`/`LoadStates`/`LoadTransitions`/`LoadHubStates` passes).
//!
//! The 1.2 client parses the XML under `game/animations/graphs/` into immutable tables:
//!
//! * inputs (`+0x10`): one `CIntegerInputValue`/`CFloatInputValue`/`CKeyedInputValue` per
//!   `<Inputs>` child, in document order; float inputs use the fixed 1/1024 precision, integer
//!   and float inputs get an ordered range list built from every guard interval, keyed inputs
//!   get the document-order key list;
//! * states (`+0x58`): `0xa0` bytes each, name/parent/cost/flags/guards/factory range;
//! * transitions (`+0x68`): sorted `0xc`-byte `{from,to,forceFollowChance,cost,transitionTime}`;
//! * hubs: the `Main Hub`/`Main_Hub` state id (`+0x204`);
//! * parameterisation: `[stance]`, `[dir]`, `[num]`, `[version]` copies appended after the
//!   originals in parameter/value order, with the template original removed.
//!
//! The live tables are captured as JSON goldens by `rust/tools/capture_anim_goldens.py` in the
//! `test/anim-golden` lane.

use std::collections::HashMap;

use roxmltree::{Document, Node};

use super::inputs::{encode_float_str, encode_int_str, encode_key, parse_float};

/// The input object type (`+0x08`: 0 integer, 1 float, 2 keyed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputKind {
    Int,
    Float,
    Keyed,
}

/// One `<Inputs>` child, as loaded into a `C*InputValue` object.
#[derive(Clone, Debug)]
pub struct GraphInput {
    /// Vector index (`+0x04`), also `input & 0x1f` in a guard.
    pub id: u16,
    pub name: String,
    pub kind: InputKind,
    /// `+0x0c`.
    pub signalled: bool,
    /// `+0x0d`.
    pub priority: u8,
    /// `forceGuard`: only these inputs contribute state guards.
    pub force_guard: bool,
    /// `mixinFilter`: keyed inputs with this set also contribute value guards.
    pub mixin_filter: bool,
    /// `defaultValue` text from the XML, if any.
    pub default_text: Option<String>,
    /// `+0x0e`, or `0xffff`.
    pub default_encoded: u16,
    /// `+0x28`/`+0x2c` (int and float objects).
    pub min: i32,
    pub max: i32,
    /// `+0x30`/`+0x34` (float objects only).
    pub scale: f32,
    pub offset: f32,
    /// `+0x18..+0x1c` ordered, merged inclusive ranges.
    pub ranges: Vec<(i32, i32)>,
    /// Keyed inputs: document-order name -> index.
    pub keys: Vec<(String, u16)>,
}

impl GraphInput {
    /// `EncodeInput(string)` through the object's virtual (`+0x04` entry).
    pub fn encode(&self, text: &str) -> u16 {
        match self.kind {
            InputKind::Int => encode_int_str(&self.ranges, self.min, self.max, text),
            InputKind::Float => encode_float_str(
                &self.ranges,
                self.min,
                self.max,
                self.scale,
                self.offset,
                text,
            ),
            InputKind::Keyed => encode_key(&self.keys, text),
        }
    }
}

/// A `<Template>` or other factory-producing child of a state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TemplateInfo {
    pub kind: String,
    pub name: String,
    /// `action` for action templates (guards), otherwise empty.
    pub action: String,
}

/// One state record (`0xa0` bytes).
#[derive(Clone, Debug)]
pub struct StateInfo {
    pub id: u16,
    pub name: String,
    pub parent: Option<u16>,
    /// `state+0xa`: factory count.
    pub factory_count: u16,
    /// `state+0xc`: cost - 1.
    pub cost_minus1: u16,
    /// `state+0x08..0x0c`: raw flag bytes.
    pub flags8: u8,
    pub flags9: u8,
    pub flags10: u8,
    /// `state+0xb`: guard count (always even).
    pub guard_count: u8,
    /// Number of guards produced by the state's own `<SelectWhen>` (before inheritance).
    pub select_guard_count: u8,
    /// True when every `<SelectWhen>` child is a bare element (no min/max/value).
    pub bare_select: bool,
    /// `state+0x10`: packed guards.
    pub guards: Vec<u16>,
    pub allow_select: bool,
    pub include_in_game: bool,
    pub can_mix: bool,
    /// Child factories (template/additive/signal/... in document order).
    pub templates: Vec<TemplateInfo>,
}

/// One transition (`0xc` bytes).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransitionInfo {
    pub from: u16,
    pub to: u16,
    pub force_follow_chance: u16,
    /// Link `cost - 1` (the loader's field at entry+6 before `CachePass`).
    pub cost_minus1: u16,
    /// Link `cost` (kept for the `CachePass` weight formula).
    pub link_cost: u16,
    pub transition_time: f32,
}

/// The static graph tables.
#[derive(Clone, Debug)]
pub struct AnimationGraph {
    pub name: String,
    pub inputs: Vec<GraphInput>,
    pub states: Vec<StateInfo>,
    pub transitions: Vec<TransitionInfo>,
    /// `Main Hub`/`Main_Hub` state id (`graph+0x204`), `None` when absent.
    pub default_state: Option<u16>,
}

#[derive(Clone, Debug)]
struct RawState<'a> {
    node: Node<'a, 'a>,
    id: String,
    parent_name: Option<String>,
    allow_select: bool,
    include_in_game: bool,
    can_mix: bool,
    cost: u16,
    params: Vec<(String, Vec<String>)>,
    overrides: Vec<(String, Node<'a, 'a>)>,
}

/// A fully expanded state, before the global ordering.
#[derive(Clone, Debug)]
struct ExpandedState<'a> {
    node: Node<'a, 'a>,
    id: String,
    parent_name: Option<String>,
    allow_select: bool,
    include_in_game: bool,
    can_mix: bool,
    cost: u16,
    overrides: Vec<(String, Node<'a, 'a>)>,
    /// Placeholder values in parameter order, used to expand links on this state.
    combo: Vec<String>,
    is_copy: bool,
}

fn attr<'a>(node: Node<'a, 'a>, name: &str) -> Option<&'a str> {
    node.attribute(name)
}

fn attr_bool(node: Node, name: &str, default: bool) -> bool {
    match attr(node, name) {
        Some(v) => v == "1" || v.eq_ignore_ascii_case("true"),
        None => default,
    }
}

fn attr_u32(node: Node, name: &str, default: u32) -> u32 {
    attr(node, name)
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

fn child<'a>(node: Node<'a, 'a>, name: &str) -> Option<Node<'a, 'a>> {
    node.children()
        .find(|c| c.is_element() && c.tag_name().name().eq_ignore_ascii_case(name))
}

fn element_children<'a>(node: Node<'a, 'a>) -> impl Iterator<Item = Node<'a, 'a>> {
    node.children().filter(|c| c.is_element())
}

/// Parse an animation-graph XML document (`horse_full_body.xml`, `player_full_body.xml`, ...).
pub fn load_graph(name: &str, xml: &str) -> Result<AnimationGraph, String> {
    let doc = Document::parse(xml).map_err(|e| format!("{name}: XML parse error: {e}"))?;
    let root = doc.root_element();

    let inputs_node = child(root, "Inputs").ok_or_else(|| format!("{name}: no Inputs node"))?;
    let states_node = child(root, "States").ok_or_else(|| format!("{name}: no States node"))?;
    let transitions_node = child(root, "Transitions");

    let mut inputs = parse_inputs(inputs_node);
    let input_by_name: HashMap<String, usize> = inputs
        .iter()
        .enumerate()
        .map(|(i, input)| (input.name.to_ascii_lowercase(), i))
        .collect();

    let raw_states = parse_states(states_node);
    let expanded = expand_states(&raw_states);

    collect_ranges(&mut inputs, &input_by_name, &expanded);

    for input in &mut inputs {
        input.default_encoded = default_encoding(input);
    }

    let (ordered_names, ordered) = order_states(&expanded);
    let name_to_id: HashMap<String, u16> = ordered_names
        .iter()
        .enumerate()
        .map(|(i, name)| (name.to_ascii_lowercase(), i as u16))
        .collect();

    let mut states = Vec::with_capacity(ordered.len());
    for (i, raw) in ordered.iter().enumerate() {
        states.push(build_state(
            i as u16,
            raw,
            &input_by_name,
            &inputs,
            &name_to_id,
        ));
    }

    // `LoadStates` inherits the parent's guard list after the state's own criteria
    // (`extend`), so a state with no SelectWhen still carries the root's guards.
    let own: Vec<Vec<u16>> = states.iter().map(|s| s.guards.clone()).collect();
    for i in 0..states.len() {
        if states[i].bare_select {
            continue;
        }
        let mut guards = own[i].clone();
        let mut parent = states[i].parent;
        let mut seen = vec![false; states.len()];
        while let Some(pid) = parent {
            if seen[pid as usize] {
                break;
            }
            seen[pid as usize] = true;
            guards.extend_from_slice(&own[pid as usize]);
            parent = states[pid as usize].parent;
        }
        // Only the first guard pair keeps the 0x8000 sentinel bits; every later pair is stored
        // with them cleared (the live table's `0x2007/0x0007` inherited/extra guards).
        for (j, g) in guards.iter_mut().enumerate() {
            if j >= 2 {
                *g &= 0x7fff;
            }
        }
        states[i].guard_count = guards.len() as u8;
        states[i].guards = guards;
    }

    let transitions = build_transitions(
        transitions_node,
        &raw_states,
        &name_to_id,
        &ordered_names,
        &states,
    );

    let default_state = ordered_names
        .iter()
        .position(|n| n.eq_ignore_ascii_case("Main Hub") || n.eq_ignore_ascii_case("Main_Hub"))
        .map(|i| i as u16);

    Ok(AnimationGraph {
        name: name.to_string(),
        inputs,
        states,
        transitions,
        default_state,
    })
}

/// Load `game/animations/graphs/<name>.xml` from the 1.2 `game_pak`.
pub fn load_graph_from_pak(name: &str) -> Result<AnimationGraph, String> {
    let path = format!("game/animations/graphs/{name}.xml");
    let data = aa_pak::Pak::global()
        .read(&path)
        .map_err(|e| format!("{path}: {e}"))?;
    let xml = String::from_utf8_lossy(&data);
    load_graph(name, &xml)
}

fn parse_inputs(node: Node) -> Vec<GraphInput> {
    let mut out = Vec::new();
    for (id, input) in element_children(node).enumerate() {
        let tag = input.tag_name().name();
        let kind = if tag.eq_ignore_ascii_case("KeyState") {
            InputKind::Keyed
        } else if tag.eq_ignore_ascii_case("FloatState") {
            InputKind::Float
        } else if tag.eq_ignore_ascii_case("IntegerState") {
            InputKind::Int
        } else {
            continue;
        };
        let mut keys = Vec::new();
        if kind == InputKind::Keyed {
            for key in element_children(input) {
                if key.tag_name().name().eq_ignore_ascii_case("Key") {
                    keys.push((
                        attr(key, "value").unwrap_or("").to_string(),
                        keys.len() as u16,
                    ));
                }
            }
        }
        let (min, max, scale, offset) = if kind == InputKind::Float {
            let min = attr(input, "min")
                .and_then(|v| v.trim().parse::<f32>().ok())
                .unwrap_or(0.0);
            let max = attr(input, "max")
                .and_then(|v| v.trim().parse::<f32>().ok())
                .unwrap_or(0.0);
            // The live objects use the fixed 1/1024 precision: `(offset + value) / scale`.
            let scale = 1.0f32 / 1024.0;
            let offset = -min;
            (0, ((max - min) * 1024.0) as i32, scale, offset)
        } else {
            (
                attr_u32(input, "min", 0) as i32,
                attr_u32(input, "max", 0) as i32,
                0.0,
                0.0,
            )
        };
        out.push(GraphInput {
            id: id as u16,
            name: attr(input, "name").unwrap_or("").to_string(),
            kind,
            signalled: attr_bool(input, "signalled", false),
            priority: attr_u32(input, "priority", 1) as u8,
            force_guard: attr_bool(input, "forceGuard", false),
            mixin_filter: attr_bool(input, "mixinFilter", false),
            default_text: attr(input, "defaultValue").map(str::to_string),
            default_encoded: 0xffff,
            min,
            max,
            scale,
            offset,
            ranges: Vec::new(),
            keys,
        });
    }
    out
}

fn default_encoding(input: &GraphInput) -> u16 {
    match &input.default_text {
        Some(text) if !text.is_empty() => input.encode(text),
        _ => 0xffff,
    }
}

fn parse_states<'a>(node: Node<'a, 'a>) -> Vec<RawState<'a>> {
    let mut out = Vec::new();
    for state in element_children(node) {
        if !state.tag_name().name().eq_ignore_ascii_case("State") {
            continue;
        }
        let id = attr(state, "id").unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        let mut params = Vec::new();
        let mut overrides = Vec::new();
        if let Some(par) = child(state, "Parameterization") {
            for p in element_children(par) {
                if p.tag_name().name().eq_ignore_ascii_case("Parameter") {
                    let pid = attr(p, "id").unwrap_or("").to_string();
                    let values = element_children(p)
                        .filter(|v| v.tag_name().name().eq_ignore_ascii_case("Value"))
                        .map(|v| attr(v, "id").unwrap_or("").to_string())
                        .collect();
                    params.push((pid, values));
                } else if p.tag_name().name().eq_ignore_ascii_case("Override")
                    && let Some(value) = attr(p, "id")
                {
                    overrides.push((value.to_string(), p));
                }
            }
        }
        out.push(RawState {
            node: state,
            id,
            parent_name: attr(state, "extend").map(str::to_string),
            allow_select: attr_bool(state, "allowSelect", false),
            include_in_game: attr_bool(state, "includeInGame", true),
            can_mix: attr_bool(state, "canMix", false),
            cost: attr_u32(state, "cost", 100) as u16,
            params,
            overrides,
        });
    }
    out
}

fn placeholders(id: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = id;
    while let Some(open) = rest.find('[') {
        let Some(close) = rest[open..].find(']') else {
            break;
        };
        out.push(rest[open + 1..open + close].to_ascii_lowercase());
        rest = &rest[open + close + 1..];
    }
    out
}

fn default_param_values(name: &str) -> Vec<String> {
    let values: &[&str] = match name.to_ascii_lowercase().as_str() {
        "stance" => &["combat", "relaxed", "stealth"],
        "dir" => &["b", "bl", "br", "f", "fl", "fr", "l", "r"],
        "num" => &["1", "2", "3"],
        "version" => &["01", "02"],
        _ => &[],
    };
    values.iter().map(|v| (*v).to_string()).collect()
}

fn substitute_once(text: &str, value: &str) -> String {
    let mut out = text.to_string();
    if let Some(open) = out.find('[')
        && let Some(close) = out[open..].find(']')
    {
        out.replace_range(open..open + close + 1, value);
    }
    out
}

fn substitute_all(text: &str, combo: &[String]) -> String {
    let mut out = text.to_string();
    for value in combo {
        out = substitute_once(&out, value);
    }
    out
}

fn params_for<'a>(state: &'a RawState<'a>, marks: &[String]) -> Vec<Vec<String>> {
    let mut values: Vec<Vec<String>> = Vec::new();
    for mark in marks {
        if let Some((_, vals)) = state
            .params
            .iter()
            .find(|(id, _)| id.eq_ignore_ascii_case(mark))
        {
            values.push(vals.clone());
        } else {
            values.push(default_param_values(mark));
        }
    }
    values
}

fn combos(values: &[Vec<String>]) -> Vec<Vec<String>> {
    let mut out: Vec<Vec<String>> = vec![Vec::new()];
    for vals in values {
        let mut next = Vec::new();
        for combo in &out {
            for v in vals {
                let mut c = combo.clone();
                c.push(v.clone());
                next.push(c);
            }
        }
        out = next;
    }
    out
}

/// Combinations in **placeholder order** (the order the placeholders appear in the id), while
/// the nested loop follows the `<Parameterization>` document order (e.g. `Num` before `Stance`).
fn combos_for_marks(params: &[(String, Vec<String>)], marks: &[String]) -> Vec<Vec<String>> {
    // Nested loop order is the `<Parameterization>` document order (e.g. `Num` before `Stance`);
    // the returned combo is in placeholder order so `substitute_once` can consume it directly.
    let mut param_order: Vec<(String, Vec<String>)> = Vec::new();
    for (id, vals) in params {
        if marks.iter().any(|m| m.eq_ignore_ascii_case(id)) {
            param_order.push((id.clone(), vals.clone()));
        }
    }
    for mark in marks {
        if !param_order
            .iter()
            .any(|(id, _)| id.eq_ignore_ascii_case(mark))
        {
            param_order.push((mark.clone(), default_param_values(mark)));
        }
    }
    let values: Vec<Vec<String>> = param_order.iter().map(|(_, v)| v.clone()).collect();
    let in_param_order = combos(&values);
    let mut out = Vec::new();
    for combo in in_param_order {
        let mut mark_order = Vec::with_capacity(marks.len());
        for mark in marks {
            let idx = param_order
                .iter()
                .position(|(id, _)| id.eq_ignore_ascii_case(mark))
                .unwrap_or(0);
            mark_order.push(combo[idx].clone());
        }
        out.push(mark_order);
    }
    out
}

fn expand_states<'a>(raw: &[RawState<'a>]) -> Vec<ExpandedState<'a>> {
    let mut out = Vec::new();
    for state in raw {
        if !state.include_in_game {
            continue;
        }
        let marks = placeholders(&state.id);
        if marks.is_empty() {
            out.push(ExpandedState {
                node: state.node,
                id: state.id.clone(),
                parent_name: state.parent_name.clone(),
                allow_select: state.allow_select,
                include_in_game: state.include_in_game,
                can_mix: state.can_mix,
                cost: state.cost,
                overrides: state.overrides.clone(),
                combo: Vec::new(),
                is_copy: false,
            });
            continue;
        }
        for combo in combos_for_marks(&state.params, &marks) {
            let mut id = state.id.clone();
            for value in &combo {
                id = substitute_once(&id, value);
            }
            out.push(ExpandedState {
                node: state.node,
                id,
                parent_name: state.parent_name.clone(),
                allow_select: state.allow_select,
                include_in_game: state.include_in_game,
                can_mix: state.can_mix,
                cost: state.cost,
                overrides: state.overrides.clone(),
                combo,
                is_copy: true,
            });
        }
    }
    out
}

fn raw_int(input: &GraphInput, text: &str) -> Option<i32> {
    let value = super::inputs::parse_int(text)?;
    let mut value = value;
    if value < input.min {
        value = input.min;
    } else if input.max <= value {
        value = input.max;
    }
    Some(value)
}

fn raw_float(input: &GraphInput, text: &str) -> Option<i32> {
    let value = super::inputs::parse_float(text)?;
    let mut f = (input.offset + value) / input.scale;
    let min_f = input.min as f32;
    if f < min_f {
        f = min_f;
    } else {
        let max_f = input.max as f32;
        if max_f <= f {
            f = max_f;
        }
    }
    Some(super::inputs::f32_to_i32_trunc(f))
}

fn range_text(criterion: Node, name: &str, input: &GraphInput) -> String {
    if let Some(value) = criterion.attribute(name) {
        return value.to_string();
    }
    if let Some(value) = criterion.attribute("value") {
        return value.to_string();
    }
    if input.kind == InputKind::Float {
        return format!("{}", input.min as f32 * input.scale - input.offset);
    }
    "0".to_string()
}

fn collect_ranges(
    inputs: &mut [GraphInput],
    by_name: &HashMap<String, usize>,
    states: &[ExpandedState<'_>],
) {
    // `LoadInputs` builds the range vector from the boundaries of every guard interval on an
    // input: sorted unique `min` and `max+1` values, consecutive boundaries forming one range.
    let mut boundaries: Vec<Vec<i32>> = vec![Vec::new(); inputs.len()];
    for state in states {
        let Some(select) = child(state.node, "SelectWhen") else {
            continue;
        };
        for criterion in element_children(select) {
            let name = criterion.tag_name().name().to_ascii_lowercase();
            let Some(&idx) = by_name.get(&name) else {
                continue;
            };
            let input = &inputs[idx];
            if input.kind == InputKind::Keyed {
                continue;
            }
            let has_min = criterion.attribute("min").is_some();
            let has_max = criterion.attribute("max").is_some();
            let has_value = criterion.attribute("value").is_some();
            let (min_text, max_text) = if !has_min && !has_max && !has_value {
                if input.kind == InputKind::Float {
                    (
                        format!("{}", input.min as f32 * input.scale - input.offset),
                        format!("{}", input.max as f32 * input.scale - input.offset),
                    )
                } else {
                    (format!("{}", input.min), format!("{}", input.max))
                }
            } else {
                (
                    range_text(criterion, "min", input),
                    range_text(criterion, "max", input),
                )
            };
            let min_text = substitute_all(&min_text, &state.combo);
            let max_text = substitute_all(&max_text, &state.combo);
            let (Some(mut min), Some(mut max)) = (
                match input.kind {
                    InputKind::Int => raw_int(input, &min_text),
                    InputKind::Float => raw_float(input, &min_text),
                    InputKind::Keyed => None,
                },
                match input.kind {
                    InputKind::Int => raw_int(input, &max_text),
                    InputKind::Float => raw_float(input, &max_text),
                    InputKind::Keyed => None,
                },
            ) else {
                continue;
            };
            if max < min {
                std::mem::swap(&mut min, &mut max);
            }
            boundaries[idx].push(min);
            boundaries[idx].push(max.saturating_add(1));
        }
    }
    // Template range attributes (`MoveSpeedLHMin/Max`, ...) add intervals just like a
    // `SelectWhen` criterion; an empty value is the input's zero point.
    for state in states {
        for (_kind, node) in template_nodes(state) {
            for (min_attr, max_attr, input_name, divisor) in [
                (
                    "MoveSpeedLHMin",
                    "MoveSpeedLHMax",
                    "RequestedMoveSpeedLH",
                    1.0f32,
                ),
                (
                    "MoveSpeedLXMin",
                    "MoveSpeedLXMax",
                    "RequestedMoveSpeedLX",
                    1.0f32,
                ),
                (
                    "MoveSpeedLYMin",
                    "MoveSpeedLYMax",
                    "RequestedMoveSpeedLY",
                    1.0f32,
                ),
                (
                    "TurnSpeedLHMin",
                    "TurnSpeedLHMax",
                    "RequestedTurnSpeedLZ",
                    2.0f32,
                ),
            ] {
                if node.attribute(min_attr).is_none() && node.attribute(max_attr).is_none() {
                    continue;
                }
                if divisor != 1.0
                    && node.attribute(min_attr).unwrap_or("").is_empty()
                    && node.attribute(max_attr).unwrap_or("").is_empty()
                {
                    continue;
                }
                let Some(&idx) = by_name.get(&input_name.to_ascii_lowercase()) else {
                    continue;
                };
                let input = &inputs[idx];
                if input.kind == InputKind::Keyed {
                    continue;
                }
                let min_text = node.attribute(min_attr).unwrap_or("");
                let max_text = node.attribute(max_attr).unwrap_or("");
                let both_empty = min_text.is_empty() && max_text.is_empty();
                let min_text = if min_text.is_empty() {
                    "0".to_string()
                } else {
                    format!("{}", min_text.parse::<f32>().unwrap_or(0.0) / divisor)
                };
                let max_text = if max_text.is_empty() {
                    "0".to_string()
                } else {
                    format!("{}", max_text.parse::<f32>().unwrap_or(0.0) / divisor)
                };
                let min_text = substitute_all(&min_text, &state.combo);
                let max_text = substitute_all(&max_text, &state.combo);
                let (Some(mut min), Some(mut max)) = (
                    match input.kind {
                        InputKind::Int => raw_int(input, &min_text),
                        InputKind::Float => raw_float(input, &min_text),
                        InputKind::Keyed => None,
                    },
                    match input.kind {
                        InputKind::Int => raw_int(input, &max_text),
                        InputKind::Float => raw_float(input, &max_text),
                        InputKind::Keyed => None,
                    },
                ) else {
                    continue;
                };
                if max < min {
                    std::mem::swap(&mut min, &mut max);
                }
                // A range attribute pair is a half-open (min, max] interval; an entirely empty
                // pair is the exact zero point (the live `MoveSpeedLH` [0,0] entry).
                let _ = both_empty;
                boundaries[idx].push(min);
                boundaries[idx].push(max.saturating_add(1));
            }
        }
    }
    for (input, mut points) in inputs.iter_mut().zip(boundaries) {
        if input.kind == InputKind::Keyed {
            continue;
        }
        points.sort_unstable();
        points.dedup();
        input.ranges = points
            .windows(2)
            .filter_map(|w| {
                let lo = w[0];
                let hi = w[1] - 1;
                (lo <= hi).then_some((lo, hi))
            })
            .collect();
    }
}

fn order_states<'a>(states: &[ExpandedState<'a>]) -> (Vec<String>, Vec<ExpandedState<'a>>) {
    let mut originals = Vec::new();
    let mut copies = Vec::new();
    for s in states {
        if s.is_copy {
            copies.push(s.clone());
        } else {
            originals.push(s.clone());
        }
    }
    // The live order is: root states (no `extend`) first, then every remaining original,
    // both groups sorted by the state id as a byte string (case-sensitive), with the
    // parameterised copies appended in generation order. This matches both graphs exactly.
    originals.sort_by(|a, b| {
        let a_root = a.parent_name.is_none();
        let b_root = b.parent_name.is_none();
        b_root
            .cmp(&a_root)
            .then_with(|| a.id.as_bytes().cmp(b.id.as_bytes()))
    });
    originals.extend(copies);
    let names = originals.iter().map(|s| s.id.clone()).collect();
    (names, originals)
}

fn template_nodes<'a>(state: &ExpandedState<'a>) -> Vec<(String, Node<'a, 'a>)> {
    let mut out = Vec::new();
    for child in element_children(state.node) {
        let tag = child.tag_name().name().to_string();
        if tag.eq_ignore_ascii_case("SelectWhen") || tag.eq_ignore_ascii_case("Parameterization") {
            continue;
        }
        out.push((tag, child));
    }
    for (value, node) in &state.overrides {
        if state.combo.iter().any(|v| v.eq_ignore_ascii_case(value)) {
            for child in element_children(*node) {
                out.push((child.tag_name().name().to_string(), child));
            }
        }
    }
    out
}

fn build_state(
    id: u16,
    state: &ExpandedState<'_>,
    by_name: &HashMap<String, usize>,
    inputs: &[GraphInput],
    name_to_id: &HashMap<String, u16>,
) -> StateInfo {
    let parent = state
        .parent_name
        .as_ref()
        .and_then(|p| name_to_id.get(&p.to_ascii_lowercase()).copied());

    let mut guards = Vec::new();
    let mut select_guard_count = 0u8;
    let mut bare_select = false;
    if let Some(select) = child(state.node, "SelectWhen") {
        bare_select = element_children(select).all(|c| {
            c.attribute("min").is_none()
                && c.attribute("max").is_none()
                && c.attribute("value").is_none()
        });
        for criterion in element_children(select) {
            let name = criterion.tag_name().name().to_ascii_lowercase();
            let Some(&idx) = by_name.get(&name) else {
                continue;
            };
            let input = &inputs[idx];
            let use_guard = match input.kind {
                InputKind::Keyed => {
                    criterion.attribute("value").is_some()
                        && (input.force_guard || input.mixin_filter)
                }
                InputKind::Int | InputKind::Float => input.force_guard,
            };
            if !use_guard {
                continue;
            }
            // Parameterised key values (`[stance]`, `[dir]`, `[num]`, `[version]`) are
            // enforced by the copy itself and do not become stored guards.
            if let Some(value) = criterion.attribute("value")
                && value.contains('[')
            {
                continue;
            }
            if criterion.attribute("min").is_none()
                && criterion.attribute("max").is_none()
                && criterion.attribute("value").is_none()
            {
                // Bare criteria only contribute through the parent's inherited guards.
                continue;
            }
            let min_text = criterion
                .attribute("min")
                .map(str::to_string)
                .or_else(|| criterion.attribute("value").map(str::to_string))
                .unwrap_or_else(|| "0".to_string());
            let max_text = criterion
                .attribute("max")
                .map(str::to_string)
                .or_else(|| criterion.attribute("value").map(str::to_string))
                .unwrap_or_else(|| "0".to_string());
            let min_text = substitute_all(&min_text, &state.combo);
            let max_text = substitute_all(&max_text, &state.combo);
            let min = input.encode(&min_text);
            let max = input.encode(&max_text);
            if min == 0xffff || max == 0xffff || max < min {
                continue;
            }
            // A keyed guard equal to the input's default is redundant; int/float guards
            // keep it (the live Alive state stores its Aiming 0..1 criterion as 0..0).
            if input.kind == InputKind::Keyed && min == max && min == input.default_encoded {
                continue;
            }
            push_guard_pair(&mut guards, idx as u16, min, max);
            select_guard_count += 2;
        }
    }

    let mut templates = Vec::new();
    for (kind, node) in template_nodes(state) {
        let action = node
            .attribute("action")
            .map(|a| substitute_all(a, &state.combo))
            .unwrap_or_default();
        let template_name = node
            .attribute("name")
            .or_else(|| node.attribute("value"))
            .or_else(|| node.attribute("animation"))
            .unwrap_or("")
            .to_string();
        if !action.is_empty()
            && let Some(&idx) = by_name.get("action")
        {
            let input = &inputs[idx];
            let value = input.encode(&action);
            if value != 0xffff && value != input.default_encoded {
                push_guard_pair(&mut guards, idx as u16, value, value);
            }
        }
        templates.push(TemplateInfo {
            kind,
            name: template_name,
            action,
        });
    }

    let guard_count = guards.len() as u8;
    let factory_count = templates.len() as u16;
    StateInfo {
        id,
        name: state.id.clone(),
        parent,
        factory_count,
        cost_minus1: state.cost.saturating_sub(1),
        flags8: u8::from(state.allow_select),
        flags9: 0,
        flags10: 0,
        guard_count,
        select_guard_count,
        bare_select,
        guards,
        allow_select: state.allow_select,
        include_in_game: state.include_in_game,
        can_mix: state.can_mix,
        templates,
    }
}

fn push_guard_pair(guards: &mut Vec<u16>, input: u16, min: u16, max: u16) {
    let min_word = (((min as u32 + 0x100) * 0x20) & 0x7fe0) as u16 | 0x8000 | (input & 0x1f);
    let max_word = (((max as u32 & 0x3ff) << 5) as u16) | 0x8000 | (input & 0x1f);
    guards.push(min_word);
    guards.push(max_word);
}

fn endpoint_combos(name: &str, params: &[(String, Vec<String>)]) -> Vec<String> {
    let marks = placeholders(name);
    if marks.is_empty() {
        return vec![name.to_string()];
    }
    let mut out = Vec::new();
    for combo in combos_for_marks(params, &marks) {
        let mut id = name.to_string();
        for value in &combo {
            id = substitute_once(&id, value);
        }
        out.push(id);
    }
    out
}

fn build_transitions(
    node: Option<Node<'_, '_>>,
    raw_states: &[RawState<'_>],
    name_to_id: &HashMap<String, u16>,
    ordered_names: &[String],
    states: &[StateInfo],
) -> Vec<TransitionInfo> {
    let Some(node) = node else {
        return Vec::new();
    };
    let mut template_params: HashMap<String, Vec<(String, Vec<String>)>> = HashMap::new();
    for s in raw_states {
        template_params.insert(s.id.to_ascii_lowercase(), s.params.clone());
    }
    let final_by_name: HashMap<String, usize> = ordered_names
        .iter()
        .enumerate()
        .map(|(i, s)| (s.to_ascii_lowercase(), i))
        .collect();

    let expand = |name: &str| -> Vec<String> {
        let lower = name.to_ascii_lowercase();
        if final_by_name.contains_key(&lower) {
            return vec![lower];
        }
        let params = template_params.get(&lower).cloned().unwrap_or_default();
        endpoint_combos(name, &params)
            .into_iter()
            .filter(|id| final_by_name.contains_key(&id.to_ascii_lowercase()))
            .map(|id| id.to_ascii_lowercase())
            .collect()
    };

    let mut transitions = Vec::new();
    for link in element_children(node) {
        if !link.tag_name().name().eq_ignore_ascii_case("Link") {
            continue;
        }
        let (Some(from), Some(to)) = (link.attribute("from"), link.attribute("to")) else {
            continue;
        };
        let cost = attr_u32(link, "cost", 100) as u16;
        let force = attr_u32(link, "forceFollowChance", 0) as u16;
        let time = link
            .attribute("transitionTime")
            .and_then(parse_float)
            .unwrap_or(0.0);
        let froms = expand(from);
        let tos = expand(to);
        let pairs: Vec<(String, String)> = if froms.len() == tos.len() {
            froms.into_iter().zip(tos).collect()
        } else if froms.len() == 1 {
            let f = froms[0].clone();
            tos.into_iter().map(|t| (f.clone(), t)).collect()
        } else if tos.len() == 1 {
            let t = tos[0].clone();
            froms.into_iter().map(|f| (f, t.clone())).collect()
        } else {
            Vec::new()
        };
        for (f, t) in pairs {
            let (Some(&fr), Some(&to_id)) = (final_by_name.get(&f), final_by_name.get(&t)) else {
                continue;
            };
            let _ = &states[to_id];
            transitions.push(TransitionInfo {
                from: fr as u16,
                to: to_id as u16,
                force_follow_chance: force,
                cost_minus1: cost.saturating_sub(1),
                link_cost: cost,
                transition_time: time,
            });
        }
    }
    transitions.sort_by_key(|t| t.from);
    let _ = (name_to_id, states);
    transitions
}
