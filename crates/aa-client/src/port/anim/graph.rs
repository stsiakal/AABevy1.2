//! Static animation-graph loader (`CAnimationGraph::Init`, `cryaction+0x3061f210`, and the
//! `LoadInputs`/`LoadStates`/`LoadTransitions`/`LoadHubStates` passes).
//!
//! The 1.2 client parses the XML under `game/animations/graphs/` into immutable tables:
//!
//! * inputs (`+0x10`): one `CIntegerInputValue`/`CFloatInputValue`/`CKeyedInputValue` per
//!   `<Inputs>` child, in document order; float inputs use the fixed 1/1024 precision, integer
//!   and float inputs get an ordered range list built by `LoadInputs FUN_30611d30`;
//! * states (`+0x58`): `0xa0` bytes each, name/parent/cost/flags/guards/factory range;
//! * transitions (`+0x68`): sorted `0xc`-byte `{from,to,forceFollowChance,cost,transitionTime}`;
//! * hubs: the `Main Hub`/`Main_Hub` state id (`+0x204`);
//! * parameterisation: `[stance]`, `[dir]`, `[num]`, `[version]` copies appended after the
//!   originals in parameter/value order, with the template original removed.
//!
//! State ordering (`FUN_3060aed0`): the originals are sorted (root states first, then by the id
//! bytes) and emitted in dependency passes. A state is only emitted once its `extend` parent is
//! present; states whose parent sorts after them (the vehicle states in `player_full_body`)
//! therefore land after the parameterised copies. The copies of every pass follow that pass's
//! originals, which is exactly the live `player_full_body` permutation.
//!
//! Transitions with placeholders are instantiated **per shared parameter combination**, not by
//! zipping the endpoint lists: `[dir]_start -> [dir]_b_end` yields only the `b/bl/br` links
//! because `_b_end` has no `f/fl/fr/l/r` copies. This produces the 59 links that a zip misses.
//!
//! Input ranges (`LoadInputs` #5): every state criterion in `SelectWhen` (with the matching
//! `<Override>`), every literal template `SelectWhen`, every template `Guard` (parameter
//! substituted) and the template's `MoveSpeedLH`/`TurnSpeedLH` parameter pairs are inserted
//! into the original's interval map (`FUN_30655d50`/`FUN_30655be0`/`FUN_30655e40`), clipped
//! (`FUN_306560c0`) and gap-filled at the midpoint of each hole (`FUN_30655c40`). The live
//! range vectors are the resulting nodes.
//!
//! The live tables are captured as JSON goldens by `rust/tools/capture_anim_goldens.py` in the
//! `test/anim-golden` lane.

use std::collections::{HashMap, HashSet};

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
    /// `+0x18..+0x1c`, the `LoadInputs` interval map after clipping and gap filling.
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
    /// Inputs mentioned by the state's own `SelectWhen` (bare criteria included); inherited
    /// guards for these inputs are suppressed.
    pub mentioned_inputs: Vec<u16>,
}

/// One transition (`0xc` bytes).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransitionInfo {
    pub from: u16,
    pub to: u16,
    pub force_follow_chance: u16,
    /// `CachePass` weight (`entry+4` in the compiled `.ag`), or
    /// `link_cost + target.cost_minus1` for an XML-built graph.
    pub weight: u16,
    /// Link `cost - 1` (the loader's field at entry+6 before `CachePass`).
    pub cost_minus1: u16,
    /// Link `cost` (kept for the `CachePass` weight formula).
    pub link_cost: u16,
    pub transition_time: f32,
}

/// One 20-byte `CStateIndex` entry, serialized as 17 bytes by
/// `CAnimationGraph::Serialize` (`cryaction+0x6152b0`) through `FUN_30658930`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateIndexEntry {
    /// Input id (`entry+0`).
    pub input: u16,
    /// Encoded value (`entry+2`), already the byte used by `EvaluateGuards`.
    pub value: u16,
    /// Number of matching states (`entry+4`).
    pub count: u16,
    /// `entry+6`; the query walk uses only `count` and `start`.
    pub unk6: u16,
    /// `entry+8`; priority/score used by the debug output.
    pub priority: u8,
    /// Start index into the state-id pair array (`entry+0xc`).
    pub start: u32,
    /// End index into the state-id pair array (`entry+0x10`).
    pub end: u32,
}

/// `CStateIndex` (`CStateIndex::Query FUN_3065e050`, serialized by
/// `FUN_3065bc10`). The pair array is the flattened list of candidate state ids.
#[derive(Clone, Debug, Default)]
pub struct StateIndex {
    pub entries: Vec<StateIndexEntry>,
    /// `+0xb0`: state ids. The first `state_count` entries are the "all states"
    /// fallback used when the query matches no criterion.
    pub pairs: Vec<u16>,
    /// `+0xc0`: number of inputs searched.
    pub input_count: u16,
    /// `+0xc2`: number of fallback states.
    pub state_count: u16,
}

impl StateIndex {
    fn find(&self, input: u16, value: u16) -> Option<&StateIndexEntry> {
        self.entries
            .binary_search_by_key(&(input, value), |e| (e.input, e.value))
            .ok()
            .map(|i| &self.entries[i])
    }

    /// `CStateIndex::Query FUN_3065e050`: binary-search every encoded input,
    /// then return the intersection of the matching state-id ranges. With no
    /// match the original returns `pairs[0..state_count]`.
    pub fn query(&self, encoded: &[u16; 32]) -> Vec<u16> {
        let mut ranges: Vec<&StateIndexEntry> = Vec::new();
        for i in 0..(self.input_count as usize).min(32) {
            let value = encoded[i];
            if value == 0xffff {
                continue;
            }
            if let Some(entry) = self.find(i as u16, value) {
                ranges.push(entry);
            }
        }
        if ranges.is_empty() {
            let n = (self.state_count as usize).min(self.pairs.len());
            return self.pairs[..n].to_vec();
        }
        if ranges.len() == 1 {
            return self.range(ranges[0]);
        }
        let mut out = self.range(ranges[0]);
        for entry in &ranges[1..] {
            let next = self.range(entry);
            let mut merged = Vec::with_capacity(out.len().min(next.len()));
            let (mut i, mut j) = (0, 0);
            while i < out.len() && j < next.len() {
                match out[i].cmp(&next[j]) {
                    std::cmp::Ordering::Less => i += 1,
                    std::cmp::Ordering::Greater => j += 1,
                    std::cmp::Ordering::Equal => {
                        merged.push(out[i]);
                        i += 1;
                        j += 1;
                    }
                }
            }
            out = merged;
        }
        if out.len() <= 1 {
            return out;
        }
        // `FUN_3065b6f0`: score every surviving state by the priority of each matched
        // criterion whose state range contains it, sort, and keep only the best score
        // (`+0xc4 == 0` trims the lower-scored tail). Without this the swim idle and
        // swim run states both survive the raw intersection.
        let mut scored: Vec<(u16, u32)> = out.iter().map(|&state| (state, 0)).collect();
        for entry in &ranges {
            let start = (entry.start as usize).min(self.pairs.len());
            let end = (start + entry.count as usize).min(self.pairs.len());
            let slice = &self.pairs[start..end];
            for (state, score) in &mut scored {
                if slice.binary_search(state).is_ok() {
                    *score += entry.priority as u32;
                }
            }
        }
        let best = scored.iter().map(|&(_, score)| score).max().unwrap_or(0);
        let mut states: Vec<u16> = scored
            .into_iter()
            .filter(|&(_, score)| score == best)
            .map(|(state, _)| state)
            .collect();
        // `FUN_3065b6f0` keeps the best-scored state; equal scores are resolved by the
        // internal vector order. The live graph leaves the later (larger) id first.
        states.sort_unstable_by(|a, b| b.cmp(a));
        states.truncate(1);
        states
    }

    fn range(&self, entry: &StateIndexEntry) -> Vec<u16> {
        let start = (entry.start as usize).min(self.pairs.len());
        let end = (start + entry.count as usize).min(self.pairs.len());
        self.pairs[start..end].to_vec()
    }
}

/// Which loader produced the graph: the compiled `.ag` path or the XML path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphSource {
    Ag,
    Xml,
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
    /// `CStateIndex` from the compiled `.ag` (the full criterion set).
    pub state_index: Option<StateIndex>,
    pub source: GraphSource,
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
    /// The raw `<State id>` this copy came from (`[stance]` placeholders intact).
    source_id: String,
    id: String,
    parent_name: Option<String>,
    allow_select: bool,
    include_in_game: bool,
    can_mix: bool,
    cost: u16,
    /// `<Override id>` key in parameter document order, comma separated.
    override_key: String,
    /// Placeholder values in the order they appear in the id.
    mark_values: Vec<String>,
    is_copy: bool,
}

#[derive(Clone, Debug, Default)]
struct TemplateLib {
    defs: HashMap<String, TemplateDef>,
}

#[derive(Clone, Debug)]
struct TemplateDef {
    params: Vec<String>,
    /// Literal `SelectWhen` criteria (a criterion with a `$parameter` is not parsed by the
    /// original's `LoadInputs` pass and contributes nothing).
    select_when: Vec<Criterion>,
    /// `Guard` criteria, `$parameter` placeholders intact.
    guard: Vec<Criterion>,
}

#[derive(Clone, Debug)]
struct Criterion {
    tag: String,
    value: Option<String>,
    min: Option<String>,
    max: Option<String>,
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

fn attr_i32(node: Node, name: &str, default: i32) -> i32 {
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
    load_graph_with_templates(name, xml, &TemplateLib::default())
}

fn load_graph_with_templates(
    name: &str,
    xml: &str,
    templates: &TemplateLib,
) -> Result<AnimationGraph, String> {
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
    let param_values = collect_param_values(&raw_states);
    let expanded = expand_states(&raw_states, &param_values);
    let canonical = canonical_names(transitions_node, &raw_states, &expanded);

    collect_ranges(
        &mut inputs,
        &input_by_name,
        &expanded,
        &canonical,
        templates,
    );

    for input in &mut inputs {
        input.default_encoded = default_encoding(input);
    }

    let ordered = order_states(&expanded);
    let ordered_names: Vec<String> = ordered
        .iter()
        .map(|s| canonical_name(&canonical, &s.id))
        .collect();
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
            &canonical,
            &input_by_name,
            &inputs,
            &name_to_id,
        ));
    }

    // `LoadStates` inherits the parent's guard list (`extend`) per input: a criterion the state
    // mentions itself (even a bare `<Vehicle/>`) suppresses the parent's guard for that input.
    let own: Vec<Vec<u16>> = states.iter().map(|s| s.guards.clone()).collect();
    for i in 0..states.len() {
        let mut guards = own[i].clone();
        let mut covered: Vec<u16> = states[i].mentioned_inputs.clone();
        let mut parent = states[i].parent;
        let mut seen = vec![false; states.len()];
        while let Some(pid) = parent {
            if seen[pid as usize] {
                break;
            }
            seen[pid as usize] = true;
            let parent_guards = &own[pid as usize];
            for pair in parent_guards.chunks(2) {
                let input = pair[0] & 0x1f;
                if !covered.contains(&input) {
                    covered.push(input);
                    guards.extend_from_slice(pair);
                }
            }
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
        &canonical,
        &name_to_id,
        &states,
        &param_values,
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
        state_index: None,
        source: GraphSource::Xml,
    })
}

/// Load a graph the way the 1.2 client does: the compiled `.ag` side copy when it
/// exists (`FUN_306261f0`), otherwise `game/animations/graphs/<name>.xml`.
pub fn load_graph_from_pak(name: &str) -> Result<AnimationGraph, String> {
    let pak = aa_pak::Pak::global();
    let ag = super::ag::ag_path(name);
    if pak.entry(&ag).is_some() {
        return super::ag::load_graph_ag_from_pak(name);
    }
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    let path = format!("game/animations/graphs/{stem}.xml");
    let data = pak.read(&path).map_err(|e| format!("{path}: {e}"))?;
    let xml = String::from_utf8_lossy(&data);
    let templates = load_template_lib();
    load_graph_with_templates(name, &xml, &templates)
}

/// `PreprocessTemplates FUN_3060f730`: the `Libs/AnimationGraphTemplates` library, keyed by
/// `AGTemplate name`. The loader only needs the declaration order of `<Param>`, the literal
/// `SelectWhen` criteria and the `Guard` criteria (the factory children are step 4 work).
fn load_template_lib() -> TemplateLib {
    let mut lib = TemplateLib::default();
    let pak = aa_pak::Pak::global();
    for path in pak.list("game/libs/animationgraphtemplates/") {
        if !path.to_ascii_lowercase().ends_with(".xml") {
            continue;
        }
        let Ok(data) = pak.read(path) else {
            continue;
        };
        let xml = String::from_utf8_lossy(&data);
        let Ok(doc) = Document::parse(&xml) else {
            continue;
        };
        let root = doc.root_element();
        if !root.tag_name().name().eq_ignore_ascii_case("AGTemplate") {
            continue;
        }
        let Some(name) = root.attribute("name") else {
            continue;
        };
        let mut def = TemplateDef {
            params: Vec::new(),
            select_when: Vec::new(),
            guard: Vec::new(),
        };
        for node in element_children(root) {
            if node.tag_name().name().eq_ignore_ascii_case("Param") {
                if let Some(param) = node.attribute("name") {
                    def.params.push(param.to_string());
                }
            } else if node.tag_name().name().eq_ignore_ascii_case("SelectWhen") {
                for c in element_children(node) {
                    if c.attributes().any(|a| a.value().contains('$')) {
                        continue;
                    }
                    def.select_when.push(Criterion {
                        tag: c.tag_name().name().to_string(),
                        value: c.attribute("value").map(str::to_string),
                        min: c.attribute("min").map(str::to_string),
                        max: c.attribute("max").map(str::to_string),
                    });
                }
            } else if node.tag_name().name().eq_ignore_ascii_case("Guard") {
                for c in element_children(node) {
                    def.guard.push(Criterion {
                        tag: c.tag_name().name().to_string(),
                        value: c.attribute("value").map(str::to_string),
                        min: c.attribute("min").map(str::to_string),
                        max: c.attribute("max").map(str::to_string),
                    });
                }
            }
        }
        lib.defs.insert(name.to_ascii_lowercase(), def);
    }
    lib
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
                attr_i32(input, "min", 0),
                attr_i32(input, "max", 0),
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

fn substitute_named(text: &str, marks: &[String], values: &[String]) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'[' {
            if let Some(close) = text[i..].find(']') {
                let name = &text[i + 1..i + close];
                if let Some(pos) = marks.iter().position(|m| m.eq_ignore_ascii_case(name)) {
                    out.push_str(&values[pos]);
                }
                i += close + 1;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn substitute_params(text: &str, params: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            let start = i + 1;
            let mut end = start;
            while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
                end += 1;
            }
            if end > start {
                let name = &text[start..end];
                if let Some(value) = params.get(name) {
                    out.push_str(value);
                }
                i = end;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// All `<Parameter>` value lists declared by any state, lowercased and de-duplicated
/// in document order. `PreprocessParameterization` uses this global pool for link and
/// state placeholders that the endpoint does not declare itself.
fn collect_param_values(raw: &[RawState<'_>]) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for state in raw {
        for (id, values) in &state.params {
            let entry = out.entry(id.to_ascii_lowercase()).or_default();
            for value in values {
                let lower = value.to_ascii_lowercase();
                if !entry.contains(&lower) {
                    entry.push(lower);
                }
            }
        }
    }
    out
}

/// Combinations in **placeholder order** while the nested loop follows the
/// `<Parameterization>` document order. Returns `(override_key, mark_values)`:
/// `override_key` joins the values in parameter document order (the `<Override id>` spelling),
/// `mark_values` are the values in id-placeholder order for `substitute_once`.
fn param_combos(
    params: &[(String, Vec<String>)],
    marks: &[String],
    global: &HashMap<String, Vec<String>>,
) -> Vec<(String, Vec<String>)> {
    let mut order: Vec<(String, Vec<String>)> = Vec::new();
    for (id, vals) in params {
        if marks.iter().any(|m| m.eq_ignore_ascii_case(id)) {
            order.push((id.clone(), vals.clone()));
        }
    }
    for mark in marks {
        if !order.iter().any(|(id, _)| id.eq_ignore_ascii_case(mark)) {
            let values = global
                .get(&mark.to_ascii_lowercase())
                .cloned()
                .unwrap_or_else(|| default_param_values(mark));
            order.push((mark.clone(), values));
        }
    }
    let mut combos: Vec<Vec<String>> = vec![Vec::new()];
    for (_, vals) in &order {
        let mut next = Vec::new();
        for combo in &combos {
            for v in vals {
                let mut c = combo.clone();
                c.push(v.clone());
                next.push(c);
            }
        }
        combos = next;
    }
    combos
        .into_iter()
        .map(|combo| {
            let mut marks_out = Vec::with_capacity(marks.len());
            for mark in marks {
                let idx = order
                    .iter()
                    .position(|(id, _)| id.eq_ignore_ascii_case(mark))
                    .unwrap_or(0);
                marks_out.push(combo[idx].to_ascii_lowercase());
            }
            let key = combo
                .iter()
                .map(|v| v.to_ascii_lowercase())
                .collect::<Vec<_>>()
                .join(",");
            (key, marks_out)
        })
        .collect()
}

fn expand_states<'a>(
    raw: &[RawState<'a>],
    global: &HashMap<String, Vec<String>>,
) -> Vec<ExpandedState<'a>> {
    let mut out = Vec::new();
    for state in raw {
        if !state.include_in_game {
            continue;
        }
        let marks = placeholders(&state.id);
        if marks.is_empty() {
            out.push(ExpandedState {
                node: state.node,
                source_id: state.id.clone(),
                id: state.id.clone(),
                parent_name: state.parent_name.clone(),
                allow_select: state.allow_select,
                include_in_game: state.include_in_game,
                can_mix: state.can_mix,
                cost: state.cost,
                override_key: String::new(),
                mark_values: Vec::new(),
                is_copy: false,
            });
            continue;
        }
        for (override_key, mark_values) in param_combos(&state.params, &marks, global) {
            // `<Override><ExcludeFromGraph/></Override>` removes that parameter combination.
            if state.overrides.iter().any(|(key, node)| {
                key.eq_ignore_ascii_case(&override_key)
                    && child(*node, "ExcludeFromGraph").is_some()
            }) {
                continue;
            }
            let mut id = state.id.clone();
            for value in &mark_values {
                id = substitute_once(&id, value);
            }
            let mut parent_name = state.parent_name.clone();
            if let Some(parent) = parent_name.as_mut() {
                *parent = substitute_named(parent, &marks, &mark_values);
            }
            out.push(ExpandedState {
                node: state.node,
                source_id: state.id.clone(),
                id,
                parent_name,
                allow_select: state.allow_select,
                include_in_game: state.include_in_game,
                can_mix: state.can_mix,
                cost: state.cost,
                override_key,
                mark_values,
                is_copy: true,
            });
        }
    }
    out
}

/// Canonical state spelling: `PreprocessLinksAndStates FUN_30655570` creates a case-insensitive
/// name map and the first spelling seen (links before states) is what the state record points to.
fn canonical_names(
    transitions: Option<Node>,
    raw: &[RawState<'_>],
    expanded: &[ExpandedState<'_>],
) -> HashMap<String, String> {
    let mut map = HashMap::new();
    // The state records are created first; link endpoints only add names that have no state
    // (the horse `.ag` side copy carries a few older lower-case spellings, which the XML loader
    // does not reproduce).
    for state in raw {
        map.entry(state.id.to_ascii_lowercase())
            .or_insert_with(|| state.id.clone());
    }
    for state in expanded {
        if !state.is_copy {
            continue;
        }
        map.entry(state.id.to_ascii_lowercase())
            .or_insert_with(|| state.id.clone());
    }
    if let Some(node) = transitions {
        for link in element_children(node) {
            if !link.tag_name().name().eq_ignore_ascii_case("Link") {
                continue;
            }
            for name in [link.attribute("from"), link.attribute("to")] {
                if let Some(name) = name
                    && !name.is_empty()
                {
                    map.entry(name.to_ascii_lowercase())
                        .or_insert_with(|| name.to_string());
                }
            }
        }
    }
    map
}

fn canonical_name(map: &HashMap<String, String>, name: &str) -> String {
    map.get(&name.to_ascii_lowercase())
        .cloned()
        .unwrap_or_else(|| name.to_string())
}

fn order_states<'a>(states: &[ExpandedState<'a>]) -> Vec<ExpandedState<'a>> {
    let mut originals = Vec::new();
    let mut copies = Vec::new();
    for (i, s) in states.iter().enumerate() {
        if s.is_copy {
            copies.push(i);
        } else {
            originals.push(i);
        }
    }
    // The live order is: root states (no `extend`) first, then every remaining original,
    // both groups sorted by the state id as a byte string (case-sensitive), with the
    // parameterised copies appended in generation order. States whose parent sorts after them
    // are deferred to the next dependency pass (`player_full_body`'s vehicle states).
    originals.sort_by(|&a, &b| {
        let a_root = states[a].parent_name.is_none();
        let b_root = states[b].parent_name.is_none();
        b_root
            .cmp(&a_root)
            .then_with(|| states[a].id.as_bytes().cmp(states[b].id.as_bytes()))
    });

    let mut ordered = Vec::with_capacity(states.len());
    let mut placed: HashSet<String> = HashSet::new();
    let mut remaining: Vec<usize> = originals;
    let mut copies_left: Vec<usize> = copies;
    loop {
        let mut next = Vec::new();
        for idx in remaining {
            let parent_ok = states[idx]
                .parent_name
                .as_ref()
                .is_none_or(|p| placed.contains(&p.to_ascii_lowercase()));
            if parent_ok {
                placed.insert(states[idx].id.to_ascii_lowercase());
                ordered.push(idx);
            } else {
                next.push(idx);
            }
        }
        // Copies are emitted after the originals of the dependency pass in which their
        // template state is resolvable; generation order is preserved.
        let mut copies_next = Vec::new();
        for idx in copies_left {
            let parent_ok = states[idx]
                .parent_name
                .as_ref()
                .is_none_or(|p| placed.contains(&p.to_ascii_lowercase()));
            if parent_ok {
                placed.insert(states[idx].id.to_ascii_lowercase());
                ordered.push(idx);
            } else {
                copies_next.push(idx);
            }
        }
        copies_left = copies_next;
        if next.is_empty() && copies_left.is_empty() {
            break;
        }
        if next.is_empty() && !copies_left.is_empty() {
            // Unresolvable parents (cycles): keep the remaining copies in generation order.
            ordered.extend(copies_left.drain(..));
            break;
        }
        remaining = next;
    }
    ordered.into_iter().map(|i| states[i].clone()).collect()
}

fn effective_override<'a>(state: &ExpandedState<'a>) -> Option<Node<'a, 'a>> {
    if state.override_key.is_empty() {
        return None;
    }
    child(state.node, "Parameterization")?
        .children()
        .filter(|c| c.is_element() && c.tag_name().name().eq_ignore_ascii_case("Override"))
        .find(|c| {
            c.attribute("id")
                .is_some_and(|id| id.eq_ignore_ascii_case(&state.override_key))
        })
}

/// The effective `SelectWhen`/`Guard` children: an `<Override>` of the state's parameter
/// combination replaces the base node when it provides that child.
fn effective_criteria<'a>(state: &ExpandedState<'a>, tag: &str) -> Vec<Node<'a, 'a>> {
    if let Some(over) = effective_override(state)
        && let Some(node) = child(over, tag)
    {
        return element_children(node).collect();
    }
    child(state.node, tag)
        .map(|node| element_children(node).collect())
        .unwrap_or_default()
}

/// Effective `<Template>` instances (`<Override>` templates replace the base template with the
/// same name).
fn effective_templates<'a>(state: &ExpandedState<'a>) -> Vec<Node<'a, 'a>> {
    let mut out: Vec<Node<'a, 'a>> = element_children(state.node)
        .filter(|c| c.tag_name().name().eq_ignore_ascii_case("Template"))
        .collect();
    if let Some(over) = effective_override(state) {
        for t in
            element_children(over).filter(|c| c.tag_name().name().eq_ignore_ascii_case("Template"))
        {
            if let Some(name) = t.attribute("name") {
                out.retain(|b| {
                    b.attribute("name")
                        .is_none_or(|n| !n.eq_ignore_ascii_case(name))
                });
            }
            out.push(t);
        }
    }
    out
}

fn build_state(
    id: u16,
    state: &ExpandedState<'_>,
    canonical: &HashMap<String, String>,
    by_name: &HashMap<String, usize>,
    inputs: &[GraphInput],
    name_to_id: &HashMap<String, u16>,
) -> StateInfo {
    let parent = state
        .parent_name
        .as_ref()
        .and_then(|p| name_to_id.get(&p.to_ascii_lowercase()).copied());

    let mut guards = Vec::new();
    let mut mentioned_inputs: Vec<u16> = Vec::new();
    let mut select_guard_count = 0u8;
    let mut bare_select = false;
    let select: Vec<Node> = effective_criteria(state, "SelectWhen");
    if !select.is_empty() {
        bare_select = select.iter().all(|c| {
            c.attribute("min").is_none()
                && c.attribute("max").is_none()
                && c.attribute("value").is_none()
        });
    }
    for criterion in select {
        let name = criterion.tag_name().name().to_ascii_lowercase();
        let Some(&idx) = by_name.get(&name) else {
            continue;
        };
        let input = &inputs[idx];
        if !mentioned_inputs.contains(&(idx as u16)) {
            mentioned_inputs.push(idx as u16);
        }
        let use_guard = match input.kind {
            // `mixinFilter` keyed criteria are mixin filters, not state guards; only
            // `forceGuard` inputs contribute the packed guard words.
            InputKind::Keyed => criterion.attribute("value").is_some() && input.force_guard,
            InputKind::Int | InputKind::Float => input.force_guard,
        };
        if !use_guard {
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
        let min_text = substitute_all(&min_text, &state.mark_values);
        let max_text = substitute_all(&max_text, &state.mark_values);
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

    let mut templates = Vec::new();
    for node in effective_templates(state) {
        let action = node
            .attribute("action")
            .map(|a| substitute_all(a, &state.mark_values))
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
            kind: node.tag_name().name().to_string(),
            name: template_name,
            action,
        });
    }

    let guard_count = guards.len() as u8;
    let factory_count = templates.len() as u16;
    StateInfo {
        id,
        name: canonical_name(canonical, &state.id),
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
        mentioned_inputs,
    }
}

fn push_guard_pair(guards: &mut Vec<u16>, input: u16, min: u16, max: u16) {
    let min_word = (((min as u32 + 0x100) * 0x20) & 0x7fe0) as u16 | 0x8000 | (input & 0x1f);
    let max_word = (((max as u32 & 0x3ff) << 5) as u16) | 0x8000 | (input & 0x1f);
    guards.push(min_word);
    guards.push(max_word);
}

fn endpoint_combos(
    name: &str,
    params: &[(String, Vec<String>)],
    global: &HashMap<String, Vec<String>>,
) -> Vec<String> {
    let marks = placeholders(name);
    if marks.is_empty() {
        return vec![name.to_string()];
    }
    let mut out = Vec::new();
    for (_, combo) in param_combos(params, &marks, global) {
        let mut id = name.to_string();
        for value in &combo {
            id = substitute_once(&id, value);
        }
        out.push(id);
    }
    out
}

fn mapping_value(key: &str, params: &HashMap<String, String>) -> String {
    if let Some(name) = key.strip_prefix('[').and_then(|k| k.strip_suffix(']')) {
        params
            .get(&name.to_ascii_lowercase())
            .cloned()
            .unwrap_or_default()
    } else {
        key.to_ascii_lowercase()
    }
}

fn build_transitions(
    node: Option<Node<'_, '_>>,
    raw_states: &[RawState<'_>],
    canonical: &HashMap<String, String>,
    name_to_id: &HashMap<String, u16>,
    states: &[StateInfo],
    global: &HashMap<String, Vec<String>>,
) -> Vec<TransitionInfo> {
    let Some(node) = node else {
        return Vec::new();
    };
    let mut params_by_name: HashMap<String, Vec<(String, Vec<String>)>> = HashMap::new();
    for s in raw_states {
        params_by_name
            .entry(s.id.to_ascii_lowercase())
            .or_insert_with(|| s.params.clone());
        let cname = canonical_name(canonical, &s.id).to_ascii_lowercase();
        params_by_name
            .entry(cname)
            .or_insert_with(|| s.params.clone());
    }

    let copies_of = |name: &str| -> Vec<(String, HashMap<String, String>)> {
        let lower = name.to_ascii_lowercase();
        if name_to_id.contains_key(&lower) {
            return vec![(lower, HashMap::new())];
        }
        let params = params_by_name.get(&lower).cloned().unwrap_or_default();
        let marks = placeholders(name);
        if marks.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (_, values) in param_combos(&params, &marks, global) {
            let mut id = name.to_string();
            for value in &values {
                id = substitute_once(&id, value);
            }
            let il = id.to_ascii_lowercase();
            if !name_to_id.contains_key(&il) {
                continue;
            }
            let map: HashMap<String, String> =
                marks.iter().cloned().zip(values.iter().cloned()).collect();
            out.push((il, map));
        }
        out
    };

    let expand_pair =
        |from: &str, to: &str, mappings: &[(String, String)]| -> Vec<(String, String)> {
            let froms = copies_of(from);
            let tos = copies_of(to);
            let mut out = Vec::new();
            let mut seen = HashSet::new();
            for (f, fmap) in &froms {
                for (t, tmap) in &tos {
                    // `<Mapping from="[stance]" to="[stance]"/>` joins the two endpoint
                    // copies on that parameter (either side may be a literal value). A link
                    // without mappings is instantiated for every pair of copies.
                    let compatible = mappings.iter().all(|(from_key, to_key)| {
                        let from_value = mapping_value(from_key, fmap);
                        let to_value = mapping_value(to_key, tmap);
                        from_value == to_value
                    });
                    if compatible && seen.insert((f.clone(), t.clone())) {
                        out.push((f.clone(), t.clone()));
                    }
                }
            }
            out
        };

    let mut transitions = Vec::new();
    for link in element_children(node) {
        if !link.tag_name().name().eq_ignore_ascii_case("Link") {
            continue;
        }
        let (Some(from), Some(to)) = (link.attribute("from"), link.attribute("to")) else {
            continue;
        };
        let mappings: Vec<(String, String)> = element_children(link)
            .filter(|c| c.tag_name().name().eq_ignore_ascii_case("Mapping"))
            .filter_map(|c| {
                Some((
                    c.attribute("from")?.to_string(),
                    c.attribute("to")?.to_string(),
                ))
            })
            .collect();
        let cost = attr_u32(link, "cost", 100) as u16;
        let force = attr_u32(link, "forceFollowChance", 0) as u16;
        let time = link
            .attribute("transitionTime")
            .and_then(parse_float)
            .unwrap_or(0.0);
        for (f, t) in expand_pair(from, to, &mappings) {
            let (Some(&fr), Some(&to_id)) = (name_to_id.get(&f), name_to_id.get(&t)) else {
                continue;
            };
            let target = &states[to_id as usize];
            // `CachePass FUN_305f9cc0`: entry+6 += target bit6 + target.cost_minus1 + 1.
            let weight =
                cost as u32 + target.cost_minus1 as u32 + ((target.flags8 >> 6) & 1) as u32;
            transitions.push(TransitionInfo {
                from: fr as u16,
                to: to_id as u16,
                force_follow_chance: force,
                weight: weight.min(0xffff) as u16,
                cost_minus1: cost.saturating_sub(1),
                link_cost: cost,
                transition_time: time,
            });
        }
    }
    transitions.sort_by_key(|t| (t.from, t.to));
    transitions
}

// ---------------------------------------------------------------------------------------------
// `LoadInputs FUN_30611d30`: the interval map (`FUN_30655d50` insert, `FUN_30655e40`
// successor normalisation, `FUN_30655770` predecessor normalisation, `FUN_306560c0` clip,
// `FUN_30655c40` midpoint gap fill).
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
struct RangeMap {
    nodes: Vec<(i32, i32)>,
}

impl RangeMap {
    fn insert(&mut self, mut start: i32, mut end: i32, merge: bool) {
        if end < start {
            std::mem::swap(&mut start, &mut end);
        }
        let mut lo = 0usize;
        let mut hi = self.nodes.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            if self.nodes[mid].0 < start {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let i = lo;
        if i < self.nodes.len() && self.nodes[i].0 == start {
            if self.nodes[i].1 != end {
                let old = self.nodes[i].1;
                self.nodes[i].1 = old.min(end);
                self.insert(old.min(end) + 1, old.max(end), merge);
            }
            return;
        }
        self.nodes.insert(i, (start, end));
        self.normalize(i);
        if merge && i > 0 {
            self.normalize(i - 1);
        }
    }

    /// `FUN_30655e40`: normalise a node against its successor.
    fn normalize(&mut self, i: usize) {
        if i + 1 >= self.nodes.len() {
            return;
        }
        let (_start, end) = self.nodes[i];
        let (next_start, next_end) = self.nodes[i + 1];
        if next_start > end {
            return;
        }
        if next_end < end {
            let old_end = end;
            self.nodes[i].1 = next_start - 1;
            self.insert(next_end + 1, old_end, false);
        } else if next_end == end {
            self.nodes[i].1 = next_start - 1;
        } else if next_start > 0 {
            let old_next_end = next_end;
            self.nodes[i].1 = next_start - 1;
            self.nodes[i + 1].1 = end;
            self.insert(end + 1, old_next_end, false);
        }
    }

    /// `FUN_306560c0`: keep only the part inside the input's `[min,max]`.
    fn clip(&mut self, lo: i32, hi: i32) {
        self.nodes.retain_mut(|(s, e)| {
            *s = (*s).max(lo);
            *e = (*e).min(hi);
            *s <= *e
        });
    }

    /// `FUN_30655c40`: close holes at the midpoint of each pair of adjacent intervals.
    fn gapfill(&mut self) {
        let mut i = 0;
        while i + 1 < self.nodes.len() {
            let (start, end) = self.nodes[i];
            let (next_start, next_end) = self.nodes[i + 1];
            if next_start != end + 1 {
                let mid = (end + next_start) / 2;
                self.nodes[i] = (start, mid);
                self.nodes.remove(i + 1);
                if mid + 1 <= next_end {
                    self.insert(mid + 1, next_end, false);
                }
            }
            i += 1;
        }
    }
}

fn raw_float(input: &GraphInput, value: f32) -> i32 {
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
    super::inputs::f32_to_i32_trunc(f)
}

fn raw_int(input: &GraphInput, value: i32) -> i32 {
    let mut value = value;
    if value < input.min {
        value = input.min;
    } else if input.max <= value {
        value = input.max;
    }
    value
}

fn parse_attr_float(text: &str) -> Option<f32> {
    if text.is_empty() {
        Some(0.0)
    } else {
        parse_float(text)
    }
}

fn parse_attr_int(text: &str) -> Option<i32> {
    if text.is_empty() {
        Some(0)
    } else {
        super::inputs::parse_int(text)
    }
}

fn add_criterion(input: &GraphInput, map: &mut RangeMap, value: &str, min: &str, max: &str) {
    if input.kind == InputKind::Keyed {
        return;
    }
    let (lo, hi) = if input.kind == InputKind::Float {
        let (mut mn, mut mx) = if !value.is_empty() {
            let Some(v) = parse_float(value) else {
                return;
            };
            let mn = v - input.scale * 0.5;
            (mn, mn + input.scale)
        } else {
            // An attribute that is present but empty is the zero point (`atof("") == 0`).
            let (Some(mn), Some(mx)) = (parse_attr_float(min), parse_attr_float(max)) else {
                return;
            };
            (mn, mx)
        };
        if mx < mn {
            std::mem::swap(&mut mn, &mut mx);
        }
        (raw_float(input, mn), raw_float(input, mx))
    } else {
        let (mut mn, mut mx) = if !value.is_empty() {
            let Some(v) = super::inputs::parse_int(value) else {
                return;
            };
            (v, v)
        } else {
            let (Some(mn), Some(mx)) = (parse_attr_int(min), parse_attr_int(max)) else {
                return;
            };
            (mn, mx)
        };
        if mx < mn {
            std::mem::swap(&mut mn, &mut mx);
        }
        (raw_int(input, mn), raw_int(input, mx))
    };
    if lo <= hi {
        map.insert(lo, hi, true);
    }
}

fn collect_ranges(
    inputs: &mut [GraphInput],
    by_name: &HashMap<String, usize>,
    states: &[ExpandedState<'_>],
    canonical: &HashMap<String, String>,
    templates: &TemplateLib,
) {
    let mut maps: Vec<RangeMap> = vec![RangeMap::default(); inputs.len()];

    for state in states {
        for tag in ["SelectWhen", "Guard"] {
            for criterion in effective_criteria(state, tag) {
                let name = criterion.tag_name().name().to_ascii_lowercase();
                let Some(&idx) = by_name.get(&name) else {
                    continue;
                };
                let input = &inputs[idx];
                if input.kind == InputKind::Keyed {
                    continue;
                }
                let mark_names = placeholders(&state.source_id);
                let value = criterion
                    .attribute("value")
                    .map(|v| substitute_named(v, &mark_names, &state.mark_values))
                    .unwrap_or_default();
                let min = criterion
                    .attribute("min")
                    .map(|v| substitute_named(v, &mark_names, &state.mark_values))
                    .unwrap_or_default();
                let max = criterion
                    .attribute("max")
                    .map(|v| substitute_named(v, &mark_names, &state.mark_values))
                    .unwrap_or_default();
                if criterion.attribute("value").is_none()
                    && criterion.attribute("min").is_none()
                    && criterion.attribute("max").is_none()
                {
                    continue;
                }
                add_criterion(input, &mut maps[idx], &value, &min, &max);
            }
        }

        for template in effective_templates(state) {
            let Some(name) = template.attribute("name") else {
                continue;
            };
            let Some(def) = templates.defs.get(&name.to_ascii_lowercase()) else {
                continue;
            };
            let params: HashMap<String, String> = template
                .attributes()
                .map(|a| {
                    (
                        a.name().to_string(),
                        substitute_named(
                            a.value(),
                            &placeholders(&state.source_id),
                            &state.mark_values,
                        ),
                    )
                })
                .collect();
            for c in &def.select_when {
                let Some(&idx) = by_name.get(&c.tag.to_ascii_lowercase()) else {
                    continue;
                };
                if inputs[idx].kind == InputKind::Keyed {
                    continue;
                }
                add_criterion(
                    &inputs[idx],
                    &mut maps[idx],
                    c.value.as_deref().unwrap_or(""),
                    c.min.as_deref().unwrap_or(""),
                    c.max.as_deref().unwrap_or(""),
                );
            }
            for c in &def.guard {
                let Some(&idx) = by_name.get(&c.tag.to_ascii_lowercase()) else {
                    continue;
                };
                if inputs[idx].kind == InputKind::Keyed {
                    continue;
                }
                let value = c.value.as_deref().map(|v| substitute_params(v, &params));
                let min = c.min.as_deref().map(|v| substitute_params(v, &params));
                let max = c.max.as_deref().map(|v| substitute_params(v, &params));
                add_criterion(
                    &inputs[idx],
                    &mut maps[idx],
                    value.as_deref().unwrap_or(""),
                    min.as_deref().unwrap_or(""),
                    max.as_deref().unwrap_or(""),
                );
            }
            let has = |p: &str| def.params.iter().any(|n| n.eq_ignore_ascii_case(p));
            for (stem, input_name) in [
                ("MoveSpeedLH", "RequestedMoveSpeedLH"),
                ("TurnSpeedLH", "RequestedTurnSpeedLZ"),
            ] {
                let (min_key, max_key) = (format!("{stem}Min"), format!("{stem}Max"));
                if !has(&min_key) || !has(&max_key) {
                    continue;
                }
                let Some(&idx) = by_name.get(&input_name.to_ascii_lowercase()) else {
                    continue;
                };
                let min = params.get(&min_key).map(String::as_str).unwrap_or("");
                let max = params.get(&max_key).map(String::as_str).unwrap_or("");
                add_criterion(&inputs[idx], &mut maps[idx], "", min, max);
            }
        }
    }

    for (input, map) in inputs.iter_mut().zip(maps.iter_mut()) {
        if input.kind == InputKind::Keyed {
            continue;
        }
        map.clip(input.min, input.max);
        map.gapfill();
        input.ranges = map.nodes.clone();
    }
    let _ = canonical;
}
