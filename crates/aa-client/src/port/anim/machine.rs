//! Animation-graph state machine (`CAnimationGraphState`, step 3).
//!
//! Ported from the 1.2 decompile:
//!
//! * `SetInput FUN_30632810`: writes the raw value (`+0xb0`), the encoded u16 (`+0x2e`, two
//!   banks of 32), the per-input change id (`+0x140`) and the changed bit (`+0x138`);
//! * `BasicQuery FUN_30630c20` / `CStateIndex::Query FUN_3065e050`: picks the selectable states
//!   whose packed guards (`EvaluateGuards FUN_305f7a80`) pass on the encoded bank, falling back
//!   to the graph's default state (`+0x180`);
//! * `PerformQuery FUN_306359e0`: walks the current state's outgoing transitions (`graph+0x68`,
//!   `{from,to,force,weight}`), keeps the entries whose target guards pass and chooses by
//!   `time % totalWeight`;
//! * `CAnimationGraphState::Update FUN_30639000`: dispatches to the initial / steady /
//!   transition / ephemeral update functions (`FUN_30636640`, `FUN_306357f0`, `FUN_30634d50`,
//!   `FUN_30634b70`), with the 24-entry pending ring (`+0x70c`) used by forced/queued states.
//!
//! The clip/factory side (`EnterState FUN_306352a0`, the animation descs) is not needed for the
//! selection golden: the state ids, transitions and the event list are what the graph exposes.

use std::collections::VecDeque;

use super::graph::{AnimationGraph, InputKind};
use super::guards::evaluate_guards;
use super::inputs::{encode_float, encode_int};

/// One graph event emitted by a state change (`FUN_30631f80` path).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GraphEvent {
    StateChange { from: Option<u16>, to: u16 },
    TransitionStarted { from: u16, to: u16 },
}

/// The four update functions selected through `+0x708`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateKind {
    Initial,
    Steady,
    Transition,
    Ephemeral,
}

/// One `CAnimationGraphState` (`0x7d0` bytes in the original).
#[derive(Clone, Debug)]
pub struct GraphState {
    pub inputs: Vec<InputValues>,
    /// `+0x2d`: current encoded bank (0/1); the two banks are compared at the start of Update.
    pub bank: usize,
    /// `+0x26`: current state (`None` = -1).
    pub current: Option<u16>,
    /// `+0x24`: next/target state.
    pub next: Option<u16>,
    /// `+0x28`: queued target state.
    pub queued: Option<u16>,
    /// `+0x2a`: pending transition target.
    pub pending: Option<u16>,
    /// `+0x2c`: force/teleport flag (`ForceTeleportToQueriedState FUN_30636480`).
    pub forced: bool,
    /// `+0x708`.
    pub update_kind: UpdateKind,
    /// `+0x250..+0x264`: query clock and per-state timer.
    pub time: f64,
    pub state_time: f64,
    pub transition_time: f64,
    /// `+0x70c`: the 24-entry pending-state ring.
    pub pending_ring: VecDeque<u16>,
    /// `+0x640`: event queue.
    pub events: Vec<GraphEvent>,
    /// `+0x138`: changed-input bitmask.
    pub changed: u32,
    /// `+0x13c`: change sequence counter.
    pub change_seq: u32,
    /// `+0x140`: change id per input.
    pub change_ids: Vec<u32>,
}

/// One input object's state (`CIntegerInputValue` / `CFloatInputValue` / `CKeyedInputValue`).
#[derive(Clone, Debug)]
pub struct InputValues {
    /// `+0xb0`: raw value (float bits, int, or key index).
    pub raw: u32,
    /// `+0x2e`: encoded values for both banks.
    pub encoded: [u16; 2],
}

impl GraphState {
    /// `CAnimationGraphState::CAnimationGraphState` (`0x30639f80`): start in the initial update,
    /// current state `-1`, both banks `0xffff`.
    pub fn new(graph: &AnimationGraph) -> Self {
        let inputs = graph
            .inputs
            .iter()
            .map(|input| InputValues {
                raw: input.default_encoded as u32,
                // `CAnimationGraphState` starts with every input at its graph default;
                // an untouched 0xffff would make `CStateIndex::Query` skip its criterion.
                encoded: [input.default_encoded, input.default_encoded],
            })
            .collect();
        let change_ids = vec![0; graph.inputs.len()];
        Self {
            inputs,
            bank: 0,
            current: None,
            next: None,
            queued: None,
            pending: None,
            forced: false,
            update_kind: UpdateKind::Initial,
            time: 0.0,
            state_time: 0.0,
            transition_time: 0.0,
            pending_ring: VecDeque::new(),
            events: Vec::new(),
            changed: 0,
            change_seq: 0,
            change_ids,
        }
    }

    /// `CAnimationGraph::GetInputId` (`graph vtable +0x60`): index by name.
    pub fn input_id(graph: &AnimationGraph, name: &str) -> Option<u16> {
        graph
            .inputs
            .iter()
            .find(|i| i.name.eq_ignore_ascii_case(name))
            .map(|i| i.id)
    }

    /// `SetInput[key]` (`FUN_3063e0f0`) through `CKeyedInputValue::EncodeInputs` (`+0x6115d0`).
    pub fn set_key(&mut self, graph: &AnimationGraph, name: &str, value: &str) -> bool {
        let Some(input) = graph
            .inputs
            .iter()
            .find(|i| i.name.eq_ignore_ascii_case(name))
        else {
            return false;
        };
        if input.kind != InputKind::Keyed {
            return false;
        }
        let encoded = input.encode(value);
        self.write(input.id, encoded, encoded as u32);
        true
    }

    /// `SetInput[int]` (`FUN_3063e470`): raw int, encoded through `FUN_30604c50`.
    pub fn set_int(&mut self, graph: &AnimationGraph, name: &str, value: i32) -> bool {
        let Some(input) = graph
            .inputs
            .iter()
            .find(|i| i.name.eq_ignore_ascii_case(name))
        else {
            return false;
        };
        if input.kind != InputKind::Int {
            return false;
        }
        let encoded = encode_int(&input.ranges, input.min, input.max, value);
        self.write(input.id, encoded, value as u32);
        true
    }

    /// `SetInput[float]` (`FUN_3063e370`): raw f32 bits, encoded through `FUN_306052d0`.
    pub fn set_float(&mut self, graph: &AnimationGraph, name: &str, value: f32) -> bool {
        let Some(input) = graph
            .inputs
            .iter()
            .find(|i| i.name.eq_ignore_ascii_case(name))
        else {
            return false;
        };
        if input.kind != InputKind::Float {
            return false;
        }
        let encoded = encode_float(
            &input.ranges,
            input.min,
            input.max,
            input.scale,
            input.offset,
            value,
        );
        self.write(input.id, encoded, value.to_bits());
        true
    }

    /// `CAnimationGraphState::SetInput FUN_30632810`.
    fn write(&mut self, id: u16, encoded: u16, raw: u32) {
        let i = id as usize;
        if i >= self.inputs.len() {
            return;
        }
        let old = self.inputs[i].encoded[self.bank];
        self.inputs[i].encoded[self.bank] = encoded;
        self.inputs[i].raw = raw;
        self.change_ids[i] = self.change_seq;
        self.change_seq = self.change_seq.wrapping_add(1);
        if old != encoded {
            self.changed |= 1 << (id & 31);
        }
    }

    fn bank_values(&self) -> [u16; 32] {
        let mut out = [0xffff; 32];
        for (i, input) in self.inputs.iter().enumerate().take(32) {
            out[i] = input.encoded[self.bank];
        }
        out
    }

    fn guards_pass(&self, graph: &AnimationGraph, state: u16) -> bool {
        let encoded = self.bank_values();
        let info = &graph.states[state as usize];
        evaluate_guards(&info.guards, &encoded)
    }

    /// `BasicQuery FUN_30630c20`: `CStateIndex::Query FUN_3065e050` over the encoded
    /// bank. XML-only graphs fall back to the state guards.
    pub fn basic_query(&self, graph: &AnimationGraph) -> Vec<u16> {
        let encoded = self.bank_values();
        if let Some(index) = &graph.state_index {
            let states = index.query(&encoded);
            if !states.is_empty() {
                return states;
            }
        }
        let mut out = Vec::new();
        for state in &graph.states {
            if !state.allow_select {
                continue;
            }
            if evaluate_guards(&state.guards, &encoded) {
                out.push(state.id);
            }
        }
        if out.is_empty()
            && let Some(default) = graph.default_state
        {
            out.push(default);
        }
        out
    }

    /// `PerformQuery FUN_306359e0`: the immediate target is the `CStateIndex` candidate
    /// list. The current state stays selected while it is still a candidate; otherwise the
    /// first (or, when several remain, a clock-weighted) candidate is the target. The
    /// outgoing transition links are used later by the path finder, not to choose it.
    pub fn perform_query(&self, graph: &AnimationGraph) -> Option<u16> {
        let mut candidates = self.basic_query(graph);
        if candidates.is_empty() {
            return graph.default_state;
        }
        if let Some(current) = self.current
            && let Some(pos) = candidates.iter().position(|&s| s == current)
        {
            candidates.swap(0, pos);
            return Some(current);
        }
        if candidates.len() > 1 {
            let pick = (self.time * 1000.0) as usize % candidates.len();
            candidates.swap(0, pick);
        }
        candidates.first().copied()
    }

    /// `ForceTeleportToQueriedState FUN_30636480`: clear the queue, jump to `state`.
    pub fn force_state(&mut self, graph: &AnimationGraph, state: u16) {
        let from = self.current;
        self.pending_ring.clear();
        self.queued = None;
        self.forced = true;
        self.next = Some(state);
        self.pending = Some(state);
        self.learn_state(graph, state);
        self.events
            .push(GraphEvent::StateChange { from, to: state });
        self.update_kind = UpdateKind::Steady;
        self.state_time = 0.0;
    }

    fn learn_state(&mut self, graph: &AnimationGraph, state: u16) {
        self.current = Some(state);
        self.transition_time = 0.0;
        for t in graph.transitions.iter().filter(|t| t.from == state) {
            let _ = t;
        }
    }

    /// `CAnimationGraphState::Update FUN_30639000`, with the max-7-iterations loop.
    pub fn update(&mut self, graph: &AnimationGraph, dt: f64) {
        self.time += dt;
        self.state_time += dt;
        if self.update_kind == UpdateKind::Transition {
            self.transition_time -= dt;
        }
        for _ in 0..7 {
            let before = self.update_kind;
            let before_state = self.current;
            match self.update_kind {
                UpdateKind::Initial => self.update_initial(graph),
                UpdateKind::Steady => self.update_steady(graph),
                UpdateKind::Transition => self.update_transition(graph),
                UpdateKind::Ephemeral => self.update_ephemeral(graph),
            }
            if self.update_kind == before && self.current == before_state {
                break;
            }
        }
        self.changed = 0;
    }

    /// `Update_InitialState FUN_30636640`.
    fn update_initial(&mut self, graph: &AnimationGraph) {
        if let Some(default) = graph.default_state {
            let from = self.current;
            self.learn_state(graph, default);
            self.events
                .push(GraphEvent::StateChange { from, to: default });
            self.update_kind = UpdateKind::Steady;
            return;
        }
        if let Some(target) = self.perform_query(graph) {
            let from = self.current;
            self.learn_state(graph, target);
            self.events
                .push(GraphEvent::StateChange { from, to: target });
            self.update_kind = UpdateKind::Steady;
        }
    }

    /// `Update_SteadyState FUN_306357f0`.
    fn update_steady(&mut self, graph: &AnimationGraph) {
        for _ in 0..7 {
            let Some(target) = self.perform_query(graph) else {
                return;
            };
            if Some(target) == self.current {
                return;
            }
            let from = self.current.unwrap_or(target);
            self.next = Some(target);
            self.pending = Some(target);
            self.transition_time = graph
                .transitions
                .iter()
                .find(|t| t.from == from && t.to == target)
                .map(|t| t.transition_time as f64)
                .unwrap_or(0.0)
                .max(0.0);
            self.events
                .push(GraphEvent::TransitionStarted { from, to: target });
            self.update_kind = UpdateKind::Transition;
            if self.transition_time > 0.0 {
                return;
            }
        }
    }

    /// `Update_TransitionState FUN_30634d50`.
    fn update_transition(&mut self, graph: &AnimationGraph) {
        if self.transition_time > 0.0 {
            return;
        }
        let Some(target) = self.pending.take().or(self.next) else {
            self.update_kind = UpdateKind::Steady;
            return;
        };
        let from = self.current;
        self.learn_state(graph, target);
        self.events
            .push(GraphEvent::StateChange { from, to: target });
        self.update_kind = UpdateKind::Steady;
    }

    /// `Update_EphemeralState FUN_30634b70`: walk the pending ring.
    fn update_ephemeral(&mut self, graph: &AnimationGraph) {
        if let Some(next) = self.pending_ring.pop_front() {
            let from = self.current;
            self.learn_state(graph, next);
            self.events.push(GraphEvent::StateChange { from, to: next });
        } else {
            self.update_kind = UpdateKind::Steady;
        }
    }

    /// Debug helper: the current state's name.
    pub fn current_name<'a>(&self, graph: &'a AnimationGraph) -> Option<&'a str> {
        self.current
            .map(|id| graph.states[id as usize].name.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::port::anim::graph::load_graph;

    const SYNTHETIC: &str = r#"<AnimationSetup version="1">
 <Inputs>
  <IntegerState name="Speed" signalled="0" priority="1" forceGuard="1" min="0" max="10"/>
 </Inputs>
 <States>
  <State id="Idle" allowSelect="1" cost="100">
   <SelectWhen><Speed value="0"/></SelectWhen>
  </State>
  <State id="Walk" allowSelect="1" cost="100">
   <SelectWhen><Speed value="5"/></SelectWhen>
  </State>
  <State id="Run" allowSelect="1" cost="100">
   <SelectWhen><Speed value="10"/></SelectWhen>
  </State>
 </States>
 <Transitions>
  <Link from="Idle" to="Walk" cost="100" transitionTime="0"/>
  <Link from="Walk" to="Run" cost="100" transitionTime="0"/>
  <Link from="Run" to="Walk" cost="100" transitionTime="0"/>
  <Link from="Walk" to="Idle" cost="100" transitionTime="0"/>
  <Link from="Run" to="Idle" cost="100" transitionTime="0"/>
 </Transitions>
</AnimationSetup>"#;

    fn id(graph: &AnimationGraph, name: &str) -> u16 {
        graph.states.iter().find(|s| s.name == name).unwrap().id
    }

    #[test]
    fn synthetic_idle_walk_run_stop() {
        let graph = load_graph("synthetic", SYNTHETIC).expect("synthetic graph");
        let mut state = GraphState::new(&graph);
        state.set_int(&graph, "Speed", 0);
        state.update(&graph, 0.0);
        assert_eq!(state.current, Some(id(&graph, "Idle")));

        state.set_int(&graph, "Speed", 5);
        state.update(&graph, 0.0);
        assert_eq!(state.current, Some(id(&graph, "Walk")));

        state.set_int(&graph, "Speed", 10);
        state.update(&graph, 0.0);
        assert_eq!(state.current, Some(id(&graph, "Run")));

        state.set_int(&graph, "Speed", 0);
        state.update(&graph, 0.0);
        assert_eq!(state.current, Some(id(&graph, "Idle")));

        assert!(state.events.contains(&GraphEvent::StateChange {
            from: Some(id(&graph, "Run")),
            to: id(&graph, "Idle"),
        }));
    }

    #[test]
    fn forced_state_clears_the_queue() {
        let graph = load_graph("synthetic", SYNTHETIC).expect("synthetic graph");
        let mut state = GraphState::new(&graph);
        state.set_int(&graph, "Speed", 0);
        state.update(&graph, 0.0);
        state.pending_ring.push_back(id(&graph, "Run"));
        state.force_state(&graph, id(&graph, "Walk"));
        assert_eq!(state.current, Some(id(&graph, "Walk")));
        assert!(state.pending_ring.is_empty());
    }
}
